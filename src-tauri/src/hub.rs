//! Offline pins, Gemini, trash, playlist import and the startup repair.

use std::path::{Path, PathBuf};

use bob_core::{cleaned_title, meaning_text, radio_score, suggestions_from_json, top_artists};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
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


async fn ask(app: &tauri::AppHandle, prompt: &str) -> Option<Value> {
    crate::ai::complete_json(app, prompt).await
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


pub fn radio_order(seed_artist: &str, seed_album: &str, rows: &[LibraryRow]) -> Vec<String> {
    let mut scored: Vec<(i32, String)> = rows
        .iter()
        .map(|row| (radio_score(seed_artist, seed_album, &row.artist, &row.album, row.plays), row.path.clone()))
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|left, right| right.0.cmp(&left.0));
    scored.into_iter().map(|(_, path)| path).take(40).collect()
}
