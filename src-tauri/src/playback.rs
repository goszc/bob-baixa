use std::path::PathBuf;
use std::time::Duration;

use bob_core::{
    best_lrclib, hit_from_lrclib, is_playable_media, karalyr_get_url, karalyr_search_url, lrcmux_get_url,
    lrclib_get_url, lrclib_search_url, lyrics_cache_key, lyrics_from_cache, lyrics_ovh_url,
    lyrics_query, media_kind, parse_lrclib_payload, parse_lrcmux_payload, parse_lyrics_ovh,
    read_embedded, read_lyrics_cache, read_sibling_lrc, remote_hit_is_useful,
    resolve_lyrics, write_lyrics_cache, LrclibQuery, LyricInputs, LyricsCacheEntry, RemoteHit,
    ResolvedLyrics,
};
use serde::Serialize;
use tauri::Manager;

use crate::runtime::{emit, save, AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaInspect {
    pub exists: bool,
    pub kind: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<f64>,
    pub src: String,
    #[serde(default)]
    pub cover: String,
    #[serde(default)]
    pub gain: f64,
}

/// Inspect a file that was already downloaded. Nothing here talks to YouTube.
#[tauri::command]
pub async fn inspect_media(app: tauri::AppHandle, path: String) -> Result<MediaInspect, String> {
    if let Some(id) = path.strip_prefix("drive:") {
        let record = crate::drive::describe_drive(id);
        return Ok(MediaInspect {
            exists: true,
            kind: record.kind,
            title: record.title,
            artist: record.artist,
            album: record.album,
            duration: record.duration,
            src: format!("http://127.0.0.1:47821/drive/{id}"),
            cover: record.cover,
            gain: saved_gain(&app, &path),
        });
    }
    let file = PathBuf::from(&path);
    let exists = file.is_file() && is_playable_media(&file);
    if !exists {
        return Ok(MediaInspect {
            exists: false,
            kind: media_kind(&file).into(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: None,
            src: String::new(),
            cover: String::new(),
            gain: 0.0,
        });
    }
    app.asset_protocol_scope()
        .allow_file(&file)
        .map_err(|error| error.to_string())?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "janela indisponível".to_string())?;
    let src = window
        .convert_file_src(file.as_path(), None)
        .map_err(|error| error.to_string())?;
    let file_for_tags = file.clone();
    let embedded = tokio::task::spawn_blocking(move || read_embedded(&file_for_tags))
        .await
        .map_err(|error| error.to_string())?;
    Ok(MediaInspect {
        exists: true,
        kind: media_kind(&file).into(),
        title: embedded.title.unwrap_or_default(),
        artist: embedded.artist.unwrap_or_default(),
        album: embedded.album.unwrap_or_default(),
        duration: embedded.duration,
        src,
        cover: String::new(),
        gain: saved_gain(&app, &path),
    })
}

fn saved_gain(app: &tauri::AppHandle, path: &str) -> f64 {
    let dir = app
        .state::<crate::runtime::AppState>()
        .inner
        .try_lock()
        .ok()
        .map(|inner| inner.data_dir.clone())
        .unwrap_or_default();
    crate::loudness::gain_for(&dir, path).unwrap_or(0.0)
}

/// Embedded tags, sibling `.lrc`, then lrcmux, Karalyr, LRCLIB and lyrics.ovh.
#[tauri::command]
pub async fn load_lyrics(
    app: tauri::AppHandle,
    path: String,
    title: String,
    artist: String,
    duration: Option<f64>,
) -> Result<ResolvedLyrics, String> {
    if let Some(id) = path.strip_prefix("drive:") {
        if let Some(text) = crate::drive::lyrics_text(id) {
            let local = resolve_lyrics(LyricInputs {
                sibling: Some(&text),
                ..LyricInputs::default()
            });
            if local.source != "none" {
                return Ok(local);
            }
        }
    }
    let file = PathBuf::from(&path);
    if !file.is_file() {
        let query = lyrics_query(&bob_core::EmbeddedMedia::default(), &title, &artist, duration);
        return remote_lyrics(&app, query).await;
    }
    let file_for_read = file.clone();
    let (embedded, sibling) = tokio::task::spawn_blocking(move || {
        let embedded = read_embedded(&file_for_read);
        let sibling = read_sibling_lrc(&file_for_read);
        (embedded, sibling)
    })
    .await
    .map_err(|error| error.to_string())?;
    let local = resolve_lyrics(LyricInputs {
        embedded: embedded.lyrics.as_deref(),
        sibling: sibling.as_deref(),
        ..LyricInputs::default()
    });
    if local.source != "none" {
        return Ok(local);
    }
    let mut query = lyrics_query(&embedded, &title, &artist, duration);
    if let Some(identity) = crate::enrich::identify_track(&app, &file, &query.track_name, &query.artist_name, false).await {
        if !identity.title.trim().is_empty() {
            query.track_name = identity.title;
        }
        if !identity.artist.trim().is_empty() {
            query.artist_name = identity.artist;
        }
        if !identity.album.is_empty() {
            query.album_name = identity.album;
        }
    }
    remote_lyrics(&app, query).await
}

async fn remote_lyrics(app: &tauri::AppHandle, query: bob_core::LrclibQuery) -> Result<ResolvedLyrics, String> {
    let variants = bob_core::lyrics_variants(&query);
    let tried = bob_core::query_attempts(&variants);
    if query.track_name.trim().is_empty() {
        let mut empty = ResolvedLyrics::empty();
        empty.tried = tried;
        return Ok(empty);
    }
    let cache_dir = lyrics_cache_dir(app).await;
    let key = lyrics_cache_key(&query);
    let now = chrono::Utc::now().timestamp();
    if let Some(entry) = read_lyrics_cache(&cache_dir, &key, now) {
        let mut lyrics = lyrics_from_cache(&entry);
        if lyrics.source == "none" || !lyrics.has_lines() {
            lyrics.tried = tried.clone();
        }
        return Ok(lyrics);
    }
    let client = match lyrics_client() {
        Some(client) => client,
        None => {
            let mut empty = ResolvedLyrics::empty();
            empty.tried = tried;
            return Ok(empty);
        }
    };
    let mut chain = Chain::Miss;
    for (index, variant) in variants.iter().take(6).enumerate() {
        let step = if index == 0 {
            fetch_remote_chain(&client, variant).await
        } else {
            fetch_fallback(&client, variant).await
        };
        match step {
            Chain::Hit(hit) if remote_hit_is_useful(&hit) => {
                chain = Chain::Hit(hit);
                break;
            }
            Chain::Offline if matches!(chain, Chain::Miss) => chain = Chain::Offline,
            Chain::Miss | Chain::Hit(_) => chain = Chain::Miss,
            Chain::Offline => {}
        }
    }
    let entry = match chain {
        Chain::Hit(hit) => LyricsCacheEntry {
            fetched_at: now,
            found: true,
            instrumental: hit.instrumental && hit.synced.is_none() && hit.plain.is_none(),
            synced: hit.synced,
            plain: hit.plain,
            source: hit.source,
        },
        Chain::Miss => LyricsCacheEntry {
            fetched_at: now,
            found: false,
            instrumental: false,
            synced: None,
            plain: None,
            source: String::new(),
        },
        Chain::Offline => {
            let mut empty = ResolvedLyrics::empty();
            empty.tried = tried;
            return Ok(empty);
        }
    };
    let _ = write_lyrics_cache(&cache_dir, &key, &entry);
    let mut lyrics = lyrics_from_cache(&entry);
    if lyrics.source == "none" || !lyrics.has_lines() {
        lyrics.tried = tried;
    }
    Ok(lyrics)
}

async fn lyrics_cache_dir(app: &tauri::AppHandle) -> PathBuf {
    let inner = app.state::<AppState>().inner.clone().lock_owned().await;
    if inner.data_dir.as_os_str().is_empty() {
        std::env::temp_dir().join("bob-baixa-lyrics")
    } else {
        inner.data_dir.join("lyrics-cache")
    }
}

enum Chain {
    Hit(RemoteHit),
    Miss,
    Offline,
}

fn lyrics_client() -> Option<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    if let Ok(value) = reqwest::header::HeaderValue::from_str(concat!(
        "bob-baixa/",
        env!("CARGO_PKG_VERSION")
    )) {
        headers.insert("X-Karalyr-Client", value);
    }
    reqwest::Client::builder()
        .user_agent(concat!(
            "bob-baixa/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/goszc/bob-baixa)"
        ))
        .default_headers(headers)
        .timeout(Duration::from_secs(8))
        .build()
        .ok()
}

async fn fetch_remote_chain(client: &reqwest::Client, query: &LrclibQuery) -> Chain {
    let mut saw_miss = false;
    let mut saw_offline = false;
    let steps = [
        fetch_lrcmux(client, query).await,
        fetch_shaped(client, query, "karalyr", &karalyr_get_url(query), &karalyr_search_url(query)).await,
        fetch_shaped(client, query, "lrclib", &lrclib_get_url(query), &lrclib_search_url(query)).await,
        fetch_ovh(client, query).await,
    ];
    for step in steps {
        match step {
            Chain::Hit(hit) if remote_hit_is_useful(&hit) => return Chain::Hit(hit),
            Chain::Hit(_) | Chain::Miss => saw_miss = true,
            Chain::Offline => saw_offline = true,
        }
    }
    if saw_miss {
        Chain::Miss
    } else if saw_offline {
        Chain::Offline
    } else {
        Chain::Miss
    }
}

async fn fetch_fallback(client: &reqwest::Client, query: &LrclibQuery) -> Chain {
    let shaped = fetch_shaped(client, query, "lrclib", &lrclib_get_url(query), &lrclib_search_url(query)).await;
    if let Chain::Hit(hit) = &shaped {
        if remote_hit_is_useful(hit) {
            return shaped;
        }
    }
    match fetch_ovh(client, query).await {
        Chain::Hit(hit) if remote_hit_is_useful(&hit) => Chain::Hit(hit),
        Chain::Miss | Chain::Hit(_) => Chain::Miss,
        Chain::Offline if matches!(shaped, Chain::Miss) => Chain::Miss,
        Chain::Offline => Chain::Offline,
    }
}

async fn fetch_lrcmux(client: &reqwest::Client, query: &LrclibQuery) -> Chain {
    if query.artist_name.trim().is_empty() || query.track_name.trim().is_empty() {
        return Chain::Miss;
    }
    match fetch_body(client, &lrcmux_get_url(query)).await {
        Fetch::Offline => Chain::Offline,
        Fetch::Body(body) => parse_lrcmux_payload(&body).map(Chain::Hit).unwrap_or(Chain::Miss),
    }
}

async fn fetch_shaped(
    client: &reqwest::Client,
    query: &LrclibQuery,
    source: &str,
    get_url: &str,
    search_url: &str,
) -> Chain {
    let mut tracks = Vec::new();
    let mut offline = false;
    if !query.artist_name.trim().is_empty() {
        match fetch_body(client, get_url).await {
            Fetch::Offline => offline = true,
            Fetch::Body(body) => tracks = parse_lrclib_payload(&body),
        }
    }
    if tracks.is_empty() {
        match fetch_body(client, search_url).await {
            Fetch::Offline => offline = true,
            Fetch::Body(body) => tracks = parse_lrclib_payload(&body),
        }
    }
    if let Some(track) = best_lrclib(&tracks, query) {
        let hit = hit_from_lrclib(track, source);
        if remote_hit_is_useful(&hit) {
            return Chain::Hit(hit);
        }
    }
    if tracks.is_empty() && offline {
        Chain::Offline
    } else {
        Chain::Miss
    }
}

async fn fetch_ovh(client: &reqwest::Client, query: &LrclibQuery) -> Chain {
    if query.artist_name.trim().is_empty() || query.track_name.trim().is_empty() {
        return Chain::Miss;
    }
    match fetch_body(client, &lyrics_ovh_url(&query.artist_name, &query.track_name)).await {
        Fetch::Offline => Chain::Offline,
        Fetch::Body(body) => match parse_lyrics_ovh(&body) {
            Some(plain) => Chain::Hit(RemoteHit {
                source: "ovh".into(),
                synced: None,
                plain: Some(plain),
                instrumental: false,
            }),
            None => Chain::Miss,
        },
    }
}

enum Fetch {
    Body(String),
    Offline,
}

#[tauri::command]
pub async fn search_lyrics(
    title: String,
    artist: String,
    album: String,
    duration: Option<f64>,
) -> Result<Vec<crate::enrich::LyricChoice>, String> {
    let client = lyrics_client().ok_or_else(|| "sem rede".to_string())?;
    let variants = crate::enrich::variants_for(&title, &artist, &album, duration);
    let mut choices = Vec::new();
    for variant in variants.into_iter().take(4) {
        let url = lrclib_search_url(&variant);
        if let Fetch::Body(body) = fetch_body(&client, &url).await {
            for track in parse_lrclib_payload(&body) {
                let score = bob_core::score_lrclib(&variant, &track);
                if score < bob_core::LYRIC_LIST {
                    continue;
                }
                let lrc = track.synced.clone().or(track.plain.clone()).unwrap_or_default();
                if lrc.trim().is_empty() && !track.instrumental {
                    continue;
                }
                choices.push(crate::enrich::LyricChoice {
                    source: "lrclib".into(),
                    title: track.track_name.clone(),
                    artist: track.artist_name.clone(),
                    album: track.album_name.clone(),
                    duration: track.duration,
                    score,
                    synced: track.synced.as_ref().map(|text| text.contains('[')).unwrap_or(false),
                    preview: bob_core::lyric_preview(&lrc, 4),
                    lrc,
                });
            }
        }
        if let Fetch::Body(body) = fetch_body(&client, &lrcmux_get_url(&variant)).await {
            if let Some(hit) = parse_lrcmux_payload(&body) {
                if remote_hit_is_useful(&hit) {
                    let lrc = hit.synced.clone().or(hit.plain.clone()).unwrap_or_default();
                    choices.push(crate::enrich::LyricChoice {
                        source: "lrcmux".into(),
                        title: variant.track_name.clone(),
                        artist: variant.artist_name.clone(),
                        album: variant.album_name.clone(),
                        duration: variant.duration,
                        score: 70,
                        synced: hit.synced.is_some(),
                        preview: bob_core::lyric_preview(&lrc, 4),
                        lrc,
                    });
                }
            }
        }
    }
    choices.sort_by(|left, right| right.score.cmp(&left.score));
    choices.truncate(12);
    Ok(choices)
}

#[tauri::command]
pub async fn scan_library(app: tauri::AppHandle, path: Option<String>) -> Result<Vec<bob_core::ScannedMedia>, String> {
    let configured = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config.output_dir.clone()
    };
    let chosen = path.unwrap_or_default();
    let root = if chosen.trim().is_empty() {
        configured
    } else {
        chosen
    };
    if root.trim().is_empty() {
        return Ok(Vec::new());
    }
    let path = PathBuf::from(root.trim());
    tokio::task::spawn_blocking(move || bob_core::scan_media_dir(&path))
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn delete_library_track(app: tauri::AppHandle, path: String) -> Result<(), String> {
    let (root, language) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        (inner.config.output_dir.clone(), inner.config.language.clone())
    };
    let outside = bob_core::text(
        &language,
        "essa faixa está fora da pasta da biblioteca.",
        "that track is outside the library folder.",
    );
    if !bob_core::string_inside(&root, &path) {
        return Err(outside.clone());
    }
    let file = PathBuf::from(&path);
    let root_buf = PathBuf::from(&root);
    if file.exists() && !bob_core::path_within(&root_buf, &file) {
        return Err(outside);
    }
    let path_for_disk = path.clone();
    tokio::task::spawn_blocking(move || remove_track_files(&path_for_disk))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| error.to_string())?;
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.history.retain(|entry| {
        entry
            .path
            .as_deref()
            .map(|item| !bob_core::same_path(item, &path))
            .unwrap_or(true)
    });
    for job in &mut inner.jobs {
        if job
            .output_path
            .as_deref()
            .map(|item| bob_core::same_path(item, &path))
            .unwrap_or(false)
        {
            job.output_path = None;
        }
    }
    save(&inner);
    let jobs = inner.jobs.clone();
    let history = inner.history.clone();
    drop(inner);
    emit(&app, &bob_core::BobEvent::Jobs { jobs });
    emit(&app, &bob_core::BobEvent::History { history });
    Ok(())
}

fn remove_track_files(path: &str) -> std::io::Result<()> {
    let file = PathBuf::from(path);
    if !file.is_file() {
        return Ok(());
    }
    let canon = file.canonicalize()?;
    std::fs::remove_file(&canon)?;
    for sidecar in bob_core::lyric_sidecars(&canon) {
        if sidecar.is_file() {
            let _ = std::fs::remove_file(sidecar);
        }
    }
    Ok(())
}

async fn fetch_body(client: &reqwest::Client, url: &str) -> Fetch {
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(_) => return Fetch::Offline,
    };
    let status = response.status().as_u16();
    if status == 404 || status == 204 {
        return Fetch::Body(String::new());
    }
    if !response.status().is_success() {
        return Fetch::Offline;
    }
    match response.text().await {
        Ok(body) if body.len() <= 1_000_000 => Fetch::Body(body),
        Ok(_) => Fetch::Body(String::new()),
        Err(_) => Fetch::Offline,
    }
}
