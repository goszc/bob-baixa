//! Offline pins, Gemini, trash, playlist import and the startup repair.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use bob_core::{
    cleaned_title, gemini_allow, gemini_backoff, gemini_hit, meaning_text, parse_model_json, parse_spotify, pick_match, radio_score,
    spotify_url, suggestions_from_json, top_artists, Candidate, GeminiClock, SpotifyKind,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Manager;

use crate::runtime::{capture, AppState};

#[derive(Default, Serialize, Deserialize)]
struct Pins {
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    likes: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveSpace {
    pub usage: u64,
    pub limit: u64,
    pub offline: u32,
    pub cloud: u32,
    pub cache_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrashItem {
    pub id: String,
    pub name: String,
    pub title: String,
}

static CLOCK: Mutex<GeminiClock> = Mutex::new(GeminiClock { minute: 0, used: 0, cooldown_until: 0 });

pub fn pinned(path: &str) -> bool {
    let Ok(dir) = crate::drive::data_dir_public() else { return false };
    load_pins(&dir).paths.iter().any(|item| item == path)
}

pub fn startup(app: tauri::AppHandle) {
    tauri::async_runtime::spawn(async move {
        let _ = warm_some(&app, 12).await;
        if let Ok((tracks, lyrics, covers)) = tokio::task::spawn_blocking(crate::drive::repair_some).await.unwrap_or(Ok((0, 0, 0))) {
            if tracks + lyrics + covers > 0 {
                crate::runtime::emit(
                    &app,
                    &bob_core::BobEvent::Toast {
                        message: format!("conferi a biblioteca: {lyrics} letras e {covers} capas"),
                    },
                );
            }
        }
    });
}

#[tauri::command]
pub async fn set_offline(path: String, on: bool) -> Result<(), String> {
    let dir = crate::drive::data_dir_public()?;
    let mut pins = load_pins(&dir);
    pins.paths.retain(|item| item != &path);
    if on {
        if let Some(id) = path.strip_prefix("drive:") {
            let id = id.to_string();
            let _ = tokio::task::spawn_blocking(move || crate::drive::keep_offline(&id)).await;
        }
        pins.paths.push(path);
    }
    save_pins(&dir, &pins);
    Ok(())
}

#[tauri::command]
pub async fn offline_paths() -> Result<Vec<String>, String> {
    let dir = crate::drive::data_dir_public()?;
    Ok(load_pins(&dir).paths)
}

#[tauri::command]
pub async fn drive_space() -> Result<DriveSpace, String> {
    let account = tokio::task::spawn_blocking(|| {
        let token = crate::drive::access_token()?;
        crate::drive::account(&token)
    })
    .await
    .map_err(|error| error.to_string())??;
    let tracks = crate::drive::list_simple()?;
    let offline = tracks.iter().filter(|track| pinned(&track.path) || offline_file(&track.id).is_some()).count() as u32;
    Ok(DriveSpace {
        usage: account.0,
        limit: account.1,
        offline,
        cloud: tracks.len() as u32 - offline,
        cache_bytes: crate::drive::cache_bytes_public(),
    })
}

#[tauri::command]
pub async fn free_cache() -> Result<u64, String> {
    Ok(crate::drive::clear_cache_public())
}

#[tauri::command]
pub async fn warm_next(paths: Vec<String>) -> Result<(), String> {
    for path in paths.into_iter().take(2) {
        if let Some(id) = path.strip_prefix("drive:") {
            let _ = crate::drive::warm_public(id);
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn list_trash() -> Result<Vec<TrashItem>, String> {
    let rows = tokio::task::spawn_blocking(crate::drive::trashed).await.map_err(|error| error.to_string())??;
    Ok(rows.into_iter().map(|(id, name, title)| TrashItem { id, name, title }).collect())
}

#[tauri::command]
pub async fn restore_trash(id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::drive::set_trashed(&id, false))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn purge_trash(id: String) -> Result<(), String> {
    tokio::task::spawn_blocking(move || crate::drive::purge(&id))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn gemini_status() -> Result<bool, String> {
    Ok(crate::drive::read_gemini_key().is_some())
}

#[tauri::command]
pub async fn gemini_save_key(key: String) -> Result<(), String> {
    crate::drive::save_gemini_key(&key)
}

pub async fn tidy_title(app: &tauri::AppHandle, raw: &str) -> (String, String) {
    let value = clean_title(app.clone(), raw.to_string()).await.ok();
    value
        .and_then(|item| item.get("artist").and_then(Value::as_str).map(|artist| (artist.to_string(), item.get("title").and_then(Value::as_str).unwrap_or("").to_string())))
        .unwrap_or_else(|| bob_core::split_artist_title(&bob_core::clean_track_query(raw)))
}

#[tauri::command]
pub async fn clean_title(app: tauri::AppHandle, raw: String) -> Result<Value, String> {
    let prompt = format!("limpe este título do youtube. responda json {{\"artist\":\"\",\"title\":\"\"}}.\n{raw}");
    if let Some(value) = ask(&app, &prompt).await {
        if let Some(cleaned) = cleaned_title(&value) {
            return Ok(json!({ "artist": cleaned.artist, "title": cleaned.title, "source": "gemini" }));
        }
    }
    let (artist, title) = bob_core::split_artist_title(&bob_core::clean_track_query(&raw));
    Ok(json!({ "artist": artist, "title": title, "source": "regra" }))
}

#[tauri::command]
pub async fn lyric_meaning(app: tauri::AppHandle, title: String, artist: String, lyrics: String) -> Result<Value, String> {
    let prompt = format!("responda json {{\"translation\":\"\",\"meaning\":\"\"}}. traduza a letra para português brasileiro e explique o sentido em uma frase.\n{artist} — {title}\n{lyrics}");
    if let Some(value) = ask(&app, &prompt).await {
        let (translation, meaning) = meaning_text(&value);
        if !meaning.is_empty() || !translation.is_empty() {
            return Ok(json!({ "translation": translation, "meaning": meaning, "source": "gemini" }));
        }
    }
    Ok(json!({ "translation": "", "meaning": "sem a chave do gemini, a tradução usa o caminho antigo.", "source": "regra" }))
}

#[tauri::command]
pub async fn mood_mix(app: tauri::AppHandle, mood: String, library: Vec<LibraryRow>) -> Result<Vec<String>, String> {
    let catalog = catalog_text(&library);
    let prompt = format!("escolha faixas desta biblioteca para o clima \"{mood}\". json {{\"tracks\":[{{\"artist\":\"\",\"title\":\"\"}}]}}.\n{catalog}");
    if let Some(value) = ask(&app, &prompt).await {
        let picked = match_rows(&suggestions_from_json(&value), &library);
        if !picked.is_empty() {
            return Ok(picked);
        }
    }
    let needle = mood.to_lowercase();
    Ok(library
        .into_iter()
        .filter(|row| format!("{} {}", row.artist, row.title).to_lowercase().contains(needle.split_whitespace().next().unwrap_or("")))
        .take(20)
        .map(|row| row.path)
        .collect())
}

#[tauri::command]
pub async fn natural_queue(app: tauri::AppHandle, text: String, library: Vec<LibraryRow>) -> Result<Vec<String>, String> {
    let catalog = catalog_text(&library);
    let prompt = format!("monte uma fila para: \"{text}\". use só a biblioteca. json {{\"tracks\":[{{\"artist\":\"\",\"title\":\"\"}}]}}.\n{catalog}");
    if let Some(value) = ask(&app, &prompt).await {
        let picked = match_rows(&suggestions_from_json(&value), &library);
        if !picked.is_empty() {
            return Ok(picked);
        }
    }
    let words: Vec<String> = text.split_whitespace().map(|word| word.to_lowercase()).filter(|word| word.len() > 3).collect();
    Ok(library
        .into_iter()
        .filter(|row| {
            let hay = format!("{} {} {}", row.artist, row.title, row.album).to_lowercase();
            words.iter().any(|word| hay.contains(word))
        })
        .take(25)
        .map(|row| row.path)
        .collect())
}

#[tauri::command]
pub async fn discover_queries(app: tauri::AppHandle, artists: Vec<String>) -> Result<Vec<String>, String> {
    let known = top_artists(&artists.into_iter().map(|name| (name, 1)).collect::<Vec<_>>(), 8);
    let prompt = format!("sugira 6 músicas novas parecidas com estes artistas, que não sejam as mais óbvias. json {{\"tracks\":[{{\"artist\":\"\",\"title\":\"\"}}]}}.\n{}", known.join(", "));
    if let Some(value) = ask(&app, &prompt).await {
        let rows = suggestions_from_json(&value);
        if !rows.is_empty() {
            return Ok(rows.into_iter().map(|row| format!("{} {}", row.artist, row.title)).collect());
        }
    }
    Ok(known.into_iter().map(|artist| format!("{artist} similar")).collect())
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRow {
    pub path: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    #[serde(default)]
    pub plays: u32,
}

pub async fn review_spotify(bin: &Path, url: &str) -> Result<bob_core::MediaItem, String> {
    let link = parse_spotify(url).ok_or_else(|| "link do spotify não reconhecido".to_string())?;
    let client = reqwest::Client::builder().user_agent("Mozilla/5.0 bob-baixa").timeout(std::time::Duration::from_secs(25)).build().map_err(|error| error.to_string())?;
    let token = public_token(&client).await?;
    let (title, tracks) = load_public(&client, &token, &link).await?;
    let mut entries = Vec::new();
    let mut unmatched = Vec::new();
    for track in tracks {
        match search_one(bin, &track).await {
            Some(found) => entries.push(bob_core::PlaylistEntry {
                id: found.clone(),
                url: found,
                title: track.title.clone(),
                duration: track.duration,
                thumbnail: None,
                artist: track.artist.clone(),
                album: track.album.clone(),
                track: track.number,
                cover: String::new(),
            }),
            None => unmatched.push(format!("{} — {}", track.artist, track.title)),
        }
    }
    Ok(bob_core::MediaItem {
        kind: "playlist".into(),
        id: link.id.clone(),
        url: spotify_url(&link),
        title,
        channel: Some("spotify".into()),
        duration: None,
        thumbnail: None,
        view_count: None,
        formats: Vec::new(),
        entries,
        unmatched,
    })
}

struct Remote {
    title: String,
    artist: String,
    album: String,
    duration: Option<f64>,
    number: u32,
}

async fn ask(app: &tauri::AppHandle, prompt: &str) -> Option<Value> {
    let key = crate::drive::read_gemini_key()?;
    let model = {
        app.state::<AppState>().inner.clone().lock_owned().await.config.gemini_model.clone()
    };
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
    {
        let clock = CLOCK.lock().ok()?;
        if !gemini_allow(&clock, now, 15) {
            return None;
        }
    }
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}");
    let body = json!({
        "contents": [{ "parts": [{ "text": prompt }] }],
        "generationConfig": { "responseMimeType": "application/json", "temperature": 0.4 }
    });
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(40)).build().ok()?;
    let response = client.post(url).json(&body).send().await.ok()?;
    if response.status().as_u16() == 429 {
        if let Ok(mut clock) = CLOCK.lock() {
            gemini_backoff(&mut clock, now);
        }
        return None;
    }
    if !response.status().is_success() {
        return None;
    }
    let value: Value = response.json().await.ok()?;
    let text = value.pointer("/candidates/0/content/parts/0/text")?.as_str()?;
    if let Ok(mut clock) = CLOCK.lock() {
        gemini_hit(&mut clock, now);
    }
    parse_model_json(text).ok()
}

async fn warm_some(app: &tauri::AppHandle, limit: usize) -> Result<(), String> {
    let _ = app;
    let tracks = crate::drive::list_simple()?;
    for track in tracks.into_iter().take(limit) {
        let _ = crate::drive::warm_public(&track.id);
    }
    Ok(())
}

fn catalog_text(rows: &[LibraryRow]) -> String {
    rows.iter().take(80).map(|row| format!("{} — {}", row.artist, row.title)).collect::<Vec<_>>().join("\n")
}

fn match_rows(wants: &[bob_core::Suggestion], rows: &[LibraryRow]) -> Vec<String> {
    let mut out = Vec::new();
    for want in wants {
        let mut best: Option<(i32, String)> = None;
        for row in rows {
            let score = radio_score(&want.artist, "", &row.artist, &row.album, row.plays) + if row.title.to_lowercase().contains(&want.title.to_lowercase()) { 6 } else { 0 };
            if best.as_ref().map(|(current, _)| score > *current).unwrap_or(true) {
                best = Some((score, row.path.clone()));
            }
        }
        if let Some((score, path)) = best {
            if score >= 6 && !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

fn load_pins(dir: &Path) -> Pins {
    std::fs::read_to_string(dir.join("offline.json")).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

fn save_pins(dir: &Path, pins: &Pins) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(text) = serde_json::to_string(pins) {
        let _ = std::fs::write(dir.join("offline.json"), text);
    }
}

fn offline_file(id: &str) -> Option<PathBuf> {
    let dir = crate::drive::data_dir_public().ok()?.join("offline");
    let candidate = dir.join(id);
    candidate.is_file().then_some(candidate)
}

async fn public_token(client: &reqwest::Client) -> Result<String, String> {
    let value = client.get("https://open.spotify.com/get_access_token?reason=init&productType=web-player").send().await.map_err(|error| error.to_string())?.json::<Value>().await.map_err(|error| error.to_string())?;
    value.get("accessToken").and_then(Value::as_str).map(str::to_string).ok_or_else(|| "spotify sem token público".into())
}

async fn load_public(client: &reqwest::Client, token: &str, link: &bob_core::SpotifyRef) -> Result<(String, Vec<Remote>), String> {
    match link.kind {
        SpotifyKind::Track => {
            let value = spotify_get(client, token, &format!("https://api.spotify.com/v1/tracks/{}", link.id)).await?;
            let track = remote_from(&value, "");
            Ok((track.title.clone(), vec![track]))
        }
        SpotifyKind::Album => {
            let value = spotify_get(client, token, &format!("https://api.spotify.com/v1/albums/{}", link.id)).await?;
            let title = value.get("name").and_then(Value::as_str).unwrap_or("álbum").to_string();
            let tracks = page(client, token, value.get("tracks").cloned().unwrap_or(Value::Null), &title).await?;
            Ok((title, tracks))
        }
        SpotifyKind::Playlist => {
            let value = spotify_get(client, token, &format!("https://api.spotify.com/v1/playlists/{}?fields=name,tracks.items(track(name,duration_ms,track_number,artists(name),album(name))),tracks.next", link.id)).await?;
            let title = value.get("name").and_then(Value::as_str).unwrap_or("playlist").to_string();
            let tracks = page_playlist(client, token, value.get("tracks").cloned().unwrap_or(Value::Null)).await?;
            Ok((title, tracks))
        }
    }
}

async fn page(client: &reqwest::Client, token: &str, mut page: Value, album: &str) -> Result<Vec<Remote>, String> {
    let mut out = Vec::new();
    for _ in 0..30 {
        let items = page.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        if items.is_empty() { break; }
        for item in items {
            out.push(remote_from(&item, album));
        }
        let Some(next) = page.get("next").and_then(Value::as_str) else { break };
        page = spotify_get(client, token, next).await?;
    }
    Ok(out)
}

async fn page_playlist(client: &reqwest::Client, token: &str, mut page: Value) -> Result<Vec<Remote>, String> {
    let mut out = Vec::new();
    for _ in 0..30 {
        let items = page.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        if items.is_empty() { break; }
        for item in items {
            if let Some(track) = item.get("track") {
                if !track.is_null() {
                    out.push(remote_from(track, ""));
                }
            }
        }
        let Some(next) = page.get("next").and_then(Value::as_str) else { break };
        page = spotify_get(client, token, next).await?;
    }
    Ok(out)
}

fn remote_from(value: &Value, album: &str) -> Remote {
    Remote {
        title: value.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
        artist: value.get("artists").and_then(Value::as_array).map(|list| list.iter().filter_map(|item| item.get("name").and_then(Value::as_str)).collect::<Vec<_>>().join(", ")).unwrap_or_default(),
        album: value.get("album").and_then(|album| album.get("name")).and_then(Value::as_str).unwrap_or(album).to_string(),
        duration: value.get("duration_ms").and_then(Value::as_f64).map(|ms| ms / 1000.0),
        number: value.get("track_number").and_then(Value::as_u64).unwrap_or(0) as u32,
    }
}

async fn search_one(bin: &Path, track: &Remote) -> Option<String> {
    let query = format!("{} {} audio", track.artist, track.title);
    let output = capture(bin, &["-J".into(), "--flat-playlist".into(), "--no-warnings".into(), "--skip-download".into(), format!("ytsearch8:{query}")]).await.ok()?;
    let value = bob_core::parse_json_value_text(&output).ok()?;
    let hits = bob_core::parse_search(&value);
    let candidates: Vec<Candidate> = hits.iter().map(|hit| Candidate { title: hit.title.clone(), channel: hit.channel.clone().unwrap_or_default(), duration: hit.duration }).collect();
    let index = pick_match(&track.artist, &track.title, track.duration, &candidates)?;
    Some(hits[index].url.clone())
}

async fn spotify_get(client: &reqwest::Client, token: &str, url: &str) -> Result<Value, String> {
    client.get(url).bearer_auth(token).send().await.map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.json().await.map_err(|error| error.to_string())
}

pub fn radio_order(seed_artist: &str, seed_album: &str, rows: &[LibraryRow]) -> Vec<String> {
    let mut scored: Vec<(i32, String)> = rows
        .iter()
        .map(|row| (radio_score(seed_artist, seed_album, &row.artist, &row.album, row.plays), row.path.clone()))
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    scored.into_iter().map(|(_, path)| path).take(40).collect()
}
