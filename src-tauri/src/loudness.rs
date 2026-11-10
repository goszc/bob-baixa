//! ReplayGain measured with the Windows ffmpeg.exe. The number is applied
//! at playback, and a backfill walks the library that is already on disk.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bob_core::{extended_path, linear_from_db, parse_loudnorm, write_release_tags};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::hidden::tokio_command;
use crate::runtime::AppState;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PendingTag {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track: u32,
    pub cover: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Store {
    #[serde(default)]
    tags: HashMap<String, PendingTag>,
    #[serde(default)]
    gains: HashMap<String, f64>,
}

pub fn remember_tags(dir: &Path, tags: &[PendingTag]) {
    if dir.as_os_str().is_empty() {
        return;
    }
    let mut store = load(dir);
    for tag in tags {
        store.tags.insert(tag.url.clone(), tag.clone());
    }
    save(dir, &store);
}

pub fn gain_for(dir: &Path, key: &str) -> Option<f64> {
    load(dir).gains.get(key).copied()
}

pub fn remember_gain(dir: &Path, key: &str, db: f64) {
    let mut store = load(dir);
    store.gains.insert(key.to_string(), db);
    save(dir, &store);
}

pub fn alias_gain(dir: &Path, from: &str, to: &str) {
    let mut store = load(dir);
    if let Some(db) = store.gains.get(from).copied() {
        store.gains.insert(to.to_string(), db);
        save(dir, &store);
    }
}

pub async fn measure(ffmpeg: &Path, file: &Path) -> Option<f64> {
    let input = extended_path(file);
    let output = tokio_command(ffmpeg)
        .arg("-hide_banner")
        .arg("-i")
        .arg(&input)
        .arg("-map")
        .arg("0:a:0")
        .arg("-af")
        .arg("loudnorm=I=-14:TP=-1.5:LRA=11:print_format=json")
        .arg("-f")
        .arg("null")
        .arg("-")
        .output()
        .await
        .ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    parse_loudnorm(&stderr)
}

pub async fn apply_download(app: &tauri::AppHandle, path: &str, url: &str) {
    let (dir, ffmpeg) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        (inner.data_dir.clone(), inner.ffmpeg_bin.clone())
    };
    if let Some(tag) = load(&dir).tags.remove(url) {
        let mut store = load(&dir);
        store.tags.remove(url);
        save(&dir, &store);
        let cover = cover_bytes(&tag.cover).await;
        let file = PathBuf::from(path);
        let _ = tokio::task::spawn_blocking(move || {
            write_release_tags(&file, &tag.title, &tag.artist, &tag.album, tag.track, cover.as_deref())
        })
        .await;
    }
    let Some(ffmpeg) = ffmpeg else { return };
    let file = PathBuf::from(path);
    if let Some(db) = measure(&ffmpeg, &file).await {
        remember_gain(&dir, path, db);
    }
}

#[tauri::command]
pub async fn backfill_gains(app: tauri::AppHandle, root: String) -> Result<u32, String> {
    let (dir, ffmpeg) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        (
            inner.data_dir.clone(),
            inner.ffmpeg_bin.clone().ok_or_else(|| "o ffmpeg ainda não está pronto.".to_string())?,
        )
    };
    let mut files = Vec::new();
    collect_audio(&PathBuf::from(&root), &mut files);
    let known = load(&dir).gains;
    let mut done = 0u32;
    for file in files {
        let key = file.to_string_lossy().to_string();
        if known.contains_key(&key) {
            continue;
        }
        if let Some(db) = measure(&ffmpeg, &file).await {
            remember_gain(&dir, &key, db);
            done += 1;
        }
    }
    Ok(done)
}

#[tauri::command]
pub async fn track_gain(app: tauri::AppHandle, path: String) -> Result<f64, String> {
    let dir = app.state::<AppState>().inner.clone().lock_owned().await.data_dir.clone();
    Ok(gain_for(&dir, &path).unwrap_or(0.0))
}

pub fn linear(dir: &Path, key: &str) -> f64 {
    gain_for(dir, key).map(linear_from_db).unwrap_or(1.0)
}

fn collect_audio(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_audio(&path, out);
            continue;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("").to_ascii_lowercase();
        if name.ends_with(".mp3") || name.ends_with(".m4a") || name.ends_with(".opus") || name.ends_with(".flac") || name.ends_with(".ogg") || name.ends_with(".wav") || name.ends_with(".mp4") || name.ends_with(".webm") {
            out.push(path);
        }
    }
}

async fn cover_bytes(url: &str) -> Option<Vec<u8>> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return None;
    }
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().ok()?;
    let bytes = client.get(url).send().await.ok()?.bytes().await.ok()?;
    if bytes.len() > 32 && bytes.len() < 8_000_000 {
        Some(bytes.to_vec())
    } else {
        None
    }
}

fn path(dir: &Path) -> PathBuf {
    dir.join("gains.json")
}

fn load(dir: &Path) -> Store {
    std::fs::read_to_string(path(dir)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

fn save(dir: &Path, store: &Store) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(text) = serde_json::to_string(store) {
        let _ = std::fs::write(path(dir), text);
    }
}
