use std::path::PathBuf;
use std::time::Duration;

use bob_core::{
    is_playable_media, lrclib_get_url, lrclib_search_url, lyrics_cache_key, lyrics_from_cache,
    lyrics_query, media_kind, parse_lrclib_payload, pick_lrclib, read_embedded, read_lyrics_cache,
    read_sibling_lrc, resolve_lyrics, write_lyrics_cache, LrclibQuery, LyricInputs,
    LyricsCacheEntry, ResolvedLyrics,
};
use serde::Serialize;
use tauri::Manager;

use crate::runtime::AppState;

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
}

/// Inspect a file that was already downloaded. Nothing here talks to YouTube.
#[tauri::command]
pub async fn inspect_media(app: tauri::AppHandle, path: String) -> Result<MediaInspect, String> {
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
    })
}

/// Embedded lyrics, then a sibling `.lrc`, then one cached LRCLIB lookup.
#[tauri::command]
pub async fn load_lyrics(
    app: tauri::AppHandle,
    path: String,
    title: String,
    artist: String,
    duration: Option<f64>,
) -> Result<ResolvedLyrics, String> {
    let file = PathBuf::from(&path);
    if !file.is_file() {
        return Ok(ResolvedLyrics::empty());
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
    let query = lyrics_query(&embedded, &title, &artist, duration);
    if query.track_name.trim().is_empty() {
        return Ok(ResolvedLyrics::empty());
    }
    let cache_dir = lyrics_cache_dir(&app).await;
    let key = lyrics_cache_key(&query);
    let now = chrono::Utc::now().timestamp();
    if let Some(entry) = read_lyrics_cache(&cache_dir, &key, now) {
        return Ok(lyrics_from_cache(&entry));
    }
    let Some(entry) = fetch_lrclib(&query, now).await else {
        return Ok(ResolvedLyrics::empty());
    };
    let _ = write_lyrics_cache(&cache_dir, &key, &entry);
    Ok(lyrics_from_cache(&entry))
}

async fn lyrics_cache_dir(app: &tauri::AppHandle) -> PathBuf {
    let inner = app.state::<AppState>().inner.clone().lock_owned().await;
    if inner.data_dir.as_os_str().is_empty() {
        std::env::temp_dir().join("bob-baixa-lyrics")
    } else {
        inner.data_dir.join("lyrics-cache")
    }
}

async fn fetch_lrclib(query: &LrclibQuery, now: i64) -> Option<LyricsCacheEntry> {
    let client = reqwest::Client::builder()
        .user_agent("bob-baixa/0.2.0 (https://github.com/goszc/bob-baixa)")
        .timeout(Duration::from_secs(8))
        .build()
        .ok()?;
    let mut tracks = Vec::new();
    if !query.artist_name.trim().is_empty() {
        match fetch_body(&client, &lrclib_get_url(query)).await {
            Fetch::Offline => return None,
            Fetch::Body(body) => tracks = parse_lrclib_payload(&body),
        }
    }
    if tracks.is_empty() {
        match fetch_body(&client, &lrclib_search_url(query)).await {
            Fetch::Offline => return None,
            Fetch::Body(body) => tracks = parse_lrclib_payload(&body),
        }
    }
    let picked = pick_lrclib(&tracks, query.duration);
    Some(match picked {
        Some(track) => LyricsCacheEntry {
            fetched_at: now,
            found: true,
            instrumental: track.instrumental && track.synced.is_none() && track.plain.is_none(),
            synced: track.synced.clone(),
            plain: track.plain.clone(),
        },
        None => LyricsCacheEntry {
            fetched_at: now,
            found: false,
            instrumental: false,
            synced: None,
            plain: None,
        },
    })
}

enum Fetch {
    Body(String),
    Offline,
}

async fn fetch_body(client: &reqwest::Client, url: &str) -> Fetch {
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(_) => return Fetch::Offline,
    };
    if response.status().as_u16() == 404 {
        return Fetch::Body(String::new());
    }
    if !response.status().is_success() {
        return Fetch::Body(String::new());
    }
    match response.text().await {
        Ok(body) if body.len() <= 1_000_000 => Fetch::Body(body),
        Ok(_) => Fetch::Body(String::new()),
        Err(_) => Fetch::Offline,
    }
}
