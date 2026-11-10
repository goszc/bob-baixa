use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use bob_core::{
    base_recording_title, caption_is_lyrics, captions_to_lrc, cover_art_url, duplicate_groups,
    fold_lyric_name, fpcalc_args, is_already_economic, is_playable_media, lyrics_variants,
    media_header_ok, musicbrainz_user_agent, pace_wait, parse_acoustid, parse_itunes_artwork,
    parse_lrc, parse_musicbrainz_recordings, read_embedded, read_sibling_lrc, score_lyric_match,
    sibling_lrc_paths, write_track_tags, DupItem, LYRIC_LIST,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::hidden::{std_command, tokio_command};
use crate::runtime::{emit, save, AppState};

// Re-export the acoustid url through a type alias mistake? I'll call the function directly.
use bob_core::acoustid_lookup_url;

static CANCEL_ECONOMIC: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Identity {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<f64>,
    pub recording_id: String,
    pub release_id: String,
    pub score: f64,
    pub cover_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TagPreview {
    pub path: String,
    pub current_title: String,
    pub current_artist: String,
    pub current_album: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub release_id: String,
    pub cover_url: String,
    pub score: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TagEdit {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub release_id: String,
    pub cover_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricSpot {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub line: String,
    pub at: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIssues {
    pub duplicates: Vec<bob_core::DupGroup>,
    pub broken: Vec<String>,
}

fn gate() -> &'static Mutex<Option<u128>> {
    static GATE: OnceLock<Mutex<Option<u128>>> = OnceLock::new();
    GATE.get_or_init(|| Mutex::new(None))
}

fn unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn pace_musicbrainz() {
    let mut last = gate().lock().unwrap_or_else(|error| error.into_inner());
    let wait = pace_wait(*last, unix_ms(), 1100);
    if wait > 0 {
        std::thread::sleep(Duration::from_millis(wait as u64));
    }
    *last = Some(unix_ms());
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .user_agent(musicbrainz_user_agent(env!("CARGO_PKG_VERSION")))
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn data_dir(app: &tauri::AppHandle) -> PathBuf {
    app.state::<AppState>()
        .inner
        .try_lock()
        .ok()
        .map(|inner| inner.data_dir.clone())
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or_else(|| std::env::temp_dir().join("bob-baixa"))
}

fn identity_path(dir: &Path, file: &str) -> PathBuf {
    dir.join("identify").join(format!("{}.json", fnv(file)))
}

fn fnv(value: &str) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{hash:016x}")
}

pub fn read_identity(dir: &Path, file: &str) -> Option<Identity> {
    let text = std::fs::read_to_string(identity_path(dir, file)).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_identity(dir: &Path, file: &str, identity: &Identity) {
    let path = identity_path(dir, file);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(identity) {
        let _ = std::fs::write(path, text);
    }
}

pub async fn identify_track(
    app: &tauri::AppHandle,
    file: &Path,
    hint_title: &str,
    hint_artist: &str,
    write: bool,
) -> Option<Identity> {
    let dir = data_dir(app);
    let key = file.to_string_lossy().to_string();
    if !write {
        if let Some(hit) = read_identity(&dir, &key) {
            if !hit.title.trim().is_empty() {
                return Some(hit);
            }
        }
    }
    let file_buf = file.to_path_buf();
    let hint_title = hint_title.to_string();
    let hint_artist = hint_artist.to_string();
    let acoustid = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config.acoustid_key.clone()
    };
    let dir_for = dir.clone();
    let identified = tokio::task::spawn_blocking(move || {
        identify_blocking(&dir_for, &file_buf, &hint_title, &hint_artist, &acoustid, write)
    })
    .await
    .ok()
    .flatten();
    identified
}

fn identify_blocking(
    dir: &Path,
    file: &Path,
    hint_title: &str,
    hint_artist: &str,
    acoustid_key: &str,
    write: bool,
) -> Option<Identity> {
    let embedded = read_embedded(file);
    let title_hint = embedded
        .title
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| hint_title.to_string());
    let artist_hint = embedded
        .artist
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| hint_artist.to_string());
    let duration_hint = embedded.duration.map(|value| value.round() as u32);
    let http = client();
    let mut hits = Vec::new();
    if !acoustid_key.trim().is_empty() {
        if let Some(fpcalc) = ensure_fpcalc(dir) {
            if let Some((duration, fingerprint)) = run_fpcalc(&fpcalc, file) {
                if let Some(body) = acoustid_lookup(&http, acoustid_key.trim(), duration, &fingerprint) {
                    hits.extend(parse_acoustid(&body));
                }
            }
        }
    }
    if hits.is_empty() {
        let url = bob_core::mb_search_url(&base_recording_title(&title_hint), &artist_hint);
        if let Some(body) = mb_get(&http, &url) {
            hits.extend(parse_musicbrainz_recordings(&body));
        }
    }
    let chosen = choose_hit(&hits, &title_hint, &artist_hint, duration_hint)?;
    let mut cover_url = String::new();
    if !chosen.release_id.is_empty() {
        cover_url = cover_art_url(&chosen.release_id);
        if download_cover(&http, &cover_url).is_none() {
            cover_url.clear();
        }
    }
    if cover_url.is_empty() {
        let url = bob_core::itunes_search_url(&chosen.artist, &chosen.album, &chosen.title);
        if let Some(body) = plain_get(&http, &url) {
            cover_url = parse_itunes_artwork(&body).unwrap_or_default();
        }
    }
    let identity = Identity {
        title: chosen.title,
        artist: chosen.artist,
        album: chosen.album,
        duration: chosen.duration.map(|value| value as f64).or(embedded.duration),
        recording_id: chosen.recording_id,
        release_id: chosen.release_id,
        score: chosen.score,
        cover_url,
    };
    if write && file.is_file() {
        let cover = if identity.cover_url.is_empty() {
            None
        } else {
            download_cover(&http, &identity.cover_url)
        };
        let _ = write_track_tags(
            file,
            &identity.title,
            &identity.artist,
            &identity.album,
            cover.as_deref(),
        );
    }
    write_identity(dir, &file.to_string_lossy(), &identity);
    Some(identity)
}

fn choose_hit(
    hits: &[bob_core::RecordingHit],
    title: &str,
    artist: &str,
    duration: Option<u32>,
) -> Option<bob_core::RecordingHit> {
    let mut best: Option<(bob_core::RecordingHit, i32)> = None;
    for hit in hits {
        let matched = score_lyric_match(
            title,
            artist,
            duration,
            &hit.title,
            &hit.artist,
            hit.duration,
            false,
        );
        if hit.score < 80.0 && matched < LYRIC_LIST {
            continue;
        }
        let rank = matched + (hit.score / 5.0) as i32;
        let replace = best.as_ref().map(|(_, score)| rank > *score).unwrap_or(true);
        if replace {
            best = Some((hit.clone(), rank));
        }
    }
    best.map(|(hit, _)| hit)
}

fn acoustid_lookup(http: &reqwest::blocking::Client, key: &str, duration: u32, fingerprint: &str) -> Option<String> {
    let body = format!(
        "client={}&duration={}&fingerprint={}&meta=recordings+releasegroups+compress",
        form_escape(key),
        duration,
        form_escape(fingerprint)
    );
    http.post(acoustid_lookup_url())
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .ok()?
        .text()
        .ok()
}

fn form_escape(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn mb_get(http: &reqwest::blocking::Client, url: &str) -> Option<String> {
    pace_musicbrainz();
    let response = http
        .get(url)
        .header("accept", "application/json")
        .header("user-agent", musicbrainz_user_agent(env!("CARGO_PKG_VERSION")))
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().ok()
}

fn plain_get(http: &reqwest::blocking::Client, url: &str) -> Option<String> {
    let response = http.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().ok()
}

fn download_cover(http: &reqwest::blocking::Client, url: &str) -> Option<Vec<u8>> {
    if url.trim().is_empty() {
        return None;
    }
    let response = http.get(url).send().ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response.bytes().ok()?;
    if bytes.len() < 32 || bytes.len() > 4_000_000 {
        return None;
    }
    let bytes = bytes.to_vec();
    if bytes.starts_with(&[0xFF, 0xD8]) || bytes.starts_with(b"\x89PNG") {
        Some(bytes)
    } else {
        None
    }
}

fn find_fpcalc(dir: &Path) -> Option<PathBuf> {
    let bundled = dir.join("fpcalc").join(if cfg!(windows) { "fpcalc.exe" } else { "fpcalc" });
    if bundled.is_file() {
        return Some(bundled);
    }
    which(if cfg!(windows) { "fpcalc.exe" } else { "fpcalc" })
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn ensure_fpcalc(dir: &Path) -> Option<PathBuf> {
    if let Some(found) = find_fpcalc(dir) {
        return Some(found);
    }
    if !cfg!(windows) {
        return None;
    }
    let url = "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-windows-x86_64.zip";
    let http = client();
    let bytes = http.get(url).send().ok()?.bytes().ok()?;
    let dest_dir = dir.join("fpcalc");
    std::fs::create_dir_all(&dest_dir).ok()?;
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor).ok()?;
    for index in 0..zip.len() {
        let mut file = zip.by_index(index).ok()?;
        let name = file.name().to_string();
        if !name.to_ascii_lowercase().ends_with("fpcalc.exe") {
            continue;
        }
        let dest = dest_dir.join("fpcalc.exe");
        let mut out = std::fs::File::create(&dest).ok()?;
        std::io::copy(&mut file, &mut out).ok()?;
        return Some(dest);
    }
    None
}

fn raw_fingerprint(bin: &Path, file: &Path) -> Option<Vec<u32>> {
    let output = std_command(bin)
        .args(["-json", "-raw", "-length", "30", &file.to_string_lossy()])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let list = value.get("fingerprint")?.as_array()?;
    let hashes = list.iter().filter_map(|item| item.as_u64().map(|value| value as u32)).take(80).collect::<Vec<_>>();
    if hashes.len() < 12 {
        None
    } else {
        Some(hashes)
    }
}

fn run_fpcalc(bin: &Path, file: &Path) -> Option<(u32, String)> {
    let output = std_command(bin)
        .args(fpcalc_args(&file.to_string_lossy()))
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    let fingerprint = value.get("fingerprint")?.as_str()?.to_string();
    let duration = value.get("duration")?.as_f64()?.round() as u32;
    if fingerprint.is_empty() || duration == 0 {
        return None;
    }
    Some((duration, fingerprint))
}

pub async fn after_download(app: &tauri::AppHandle, path: &str, url: &str, title: &str, audio_only: bool) {
    let file = PathBuf::from(path);
    if !file.is_file() {
        return;
    }
    let grab = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config.grab_lyrics
    };
    if grab && !url.trim().is_empty() {
        let _ = grab_manual_lyrics(app, &file, url).await;
    }
    let (hint_artist, hint_title) = crate::hub::tidy_title(app, title).await;
    let hint = if hint_title.trim().is_empty() { title } else { hint_title.as_str() };
    let identity = identify_track(app, &file, hint, &hint_artist, true).await;
    let (artist, album, kind_title) = match &identity {
        Some(hit) => (hit.artist.clone(), hit.album.clone(), hit.title.clone()),
        None => (String::new(), String::new(), title.to_string()),
    };
    crate::loudness::apply_download(app, path, url).await;
    crate::ai::maybe_auto_lyrics(app, path).await;
    crate::drive::enqueue_local(
        app,
        &file,
        if kind_title.trim().is_empty() { title } else { &kind_title },
        &artist,
        &album,
        !audio_only,
    );
}

pub async fn grab_manual_lyrics(app: &tauri::AppHandle, file: &Path, url: &str) -> Result<(), String> {
    if let Some(existing) = read_sibling_lrc(file) {
        if caption_is_lyrics(&existing) || parse_lrc(&existing).lines.len() >= 4 {
            return Ok(());
        }
    }
    let (ytdlp, ffmpeg) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        (inner.ytdlp.clone(), inner.ffmpeg_dir.clone())
    };
    let Some(ytdlp) = ytdlp else {
        return Ok(());
    };
    let temp = std::env::temp_dir().join(format!("bob-lyrics-{}", fnv(&file.to_string_lossy())));
    let _ = std::fs::remove_dir_all(&temp);
    std::fs::create_dir_all(&temp).map_err(|error| error.to_string())?;
    let mut args = vec![
        "--skip-download".into(),
        "--no-write-auto-subs".into(),
        "--write-subs".into(),
        "--convert-subs".into(),
        "lrc".into(),
        "--sub-langs".into(),
        "pt.*,en.*,-live_chat".into(),
        "--no-playlist".into(),
        "-o".into(),
        temp.join("lyric.%(ext)s").to_string_lossy().into_owned(),
    ];
    if let Some(ffmpeg) = ffmpeg {
        args.push("--ffmpeg-location".into());
        args.push(ffmpeg.to_string_lossy().into_owned());
    }
    args.push("--".into());
    args.push(url.to_string());
    let _ = tokio_command(&ytdlp).args(&args).output().await;
    let best = best_caption(&temp);
    let _ = std::fs::remove_dir_all(&temp);
    if let Some(lrc) = best {
        let dest = sibling_lrc_paths(file).into_iter().next().ok_or_else(|| "sem caminho".to_string())?;
        if let Some(parent) = dest.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(dest, lrc).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn best_caption(dir: &Path) -> Option<String> {
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name()?.to_string_lossy().to_lowercase();
        if name.contains("live_chat") || name.contains(".auto.") {
            continue;
        }
        if name.ends_with(".lrc") || name.ends_with(".vtt") || name.ends_with(".srt") {
            files.push((name, path));
        }
    }
    files.sort_by(|left, right| {
        rank_caption(&left.0).cmp(&rank_caption(&right.0)).then(left.0.cmp(&right.0))
    });
    for (_, path) in files {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lrc = if path.extension().and_then(|ext| ext.to_str()).unwrap_or("").eq_ignore_ascii_case("lrc") {
            text
        } else {
            captions_to_lrc(&text)
        };
        if caption_is_lyrics(&lrc) {
            return Some(lrc);
        }
    }
    None
}

fn rank_caption(name: &str) -> u8 {
    if name.contains(".pt") {
        0
    } else if name.contains(".en") {
        1
    } else {
        2
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricChoice {
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<u32>,
    pub score: i32,
    pub synced: bool,
    pub preview: String,
    pub lrc: String,
}

pub fn variants_for(title: &str, artist: &str, album: &str, duration: Option<f64>) -> Vec<bob_core::LrclibQuery> {
    let duration = duration
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.round() as u32);
    let query = bob_core::LrclibQuery {
        track_name: title.to_string(),
        artist_name: artist.to_string(),
        album_name: album.to_string(),
        duration,
    };
    lyrics_variants(&query)
}

#[tauri::command]
pub async fn search_library_lyrics(root: String, query: String) -> Result<Vec<LyricSpot>, String> {
    let query = fold_lyric_name(&query);
    if query.chars().count() < 2 {
        return Ok(Vec::new());
    }
    let root = PathBuf::from(root.trim());
    tokio::task::spawn_blocking(move || scan_lyrics(&root, &query))
        .await
        .map_err(|error| error.to_string())
}

fn scan_lyrics(root: &Path, query: &str) -> Vec<LyricSpot> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if out.len() >= 40 {
            break;
        }
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !is_playable_media(&path) {
                continue;
            }
            let text = read_sibling_lrc(&path).or_else(|| read_embedded(&path).lyrics);
            let Some(text) = text else { continue };
            let parsed = parse_lrc(&text);
            let lines = if parsed.lines.is_empty() {
                text.lines()
                    .map(|line| bob_core::LyricLine { at: 0.0, text: line.trim().to_string() })
                    .filter(|line| !line.text.is_empty())
                    .collect::<Vec<_>>()
            } else {
                parsed.lines
            };
            let embedded = read_embedded(&path);
            for line in lines {
                if !fold_lyric_name(&line.text).contains(query) {
                    continue;
                }
                out.push(LyricSpot {
                    path: path.to_string_lossy().into_owned(),
                    title: embedded.title.clone().unwrap_or_else(|| {
                        path.file_stem().and_then(|name| name.to_str()).unwrap_or("").to_string()
                    }),
                    artist: embedded.artist.clone().unwrap_or_default(),
                    line: line.text,
                    at: line.at,
                });
                break;
            }
            if out.len() >= 40 {
                break;
            }
        }
    }
    out
}

#[tauri::command]
pub async fn pin_lyrics(path: String, lrc: String) -> Result<(), String> {
    let text = lrc.trim();
    if text.is_empty() {
        return Err("letra vazia".into());
    }
    if path.starts_with("drive:") {
        return Err("essa faixa está no drive. baixe uma cópia para fixar a letra ao lado do arquivo.".into());
    }
    let file = PathBuf::from(&path);
    let dest = sibling_lrc_paths(&file)
        .into_iter()
        .next()
        .ok_or_else(|| "sem caminho".to_string())?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    std::fs::write(dest, text).map_err(|error| error.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn preview_tags(app: tauri::AppHandle, paths: Vec<String>) -> Result<Vec<TagPreview>, String> {
    let mut out = Vec::new();
    for path in paths.into_iter().take(40) {
        let file = PathBuf::from(&path);
        if !file.is_file() {
            continue;
        }
        let embedded = tokio::task::spawn_blocking({
            let file = file.clone();
            move || read_embedded(&file)
        })
        .await
        .map_err(|error| error.to_string())?;
        let hint = embedded.title.clone().unwrap_or_default();
        let artist = embedded.artist.clone().unwrap_or_default();
        let Some(identity) = identify_track(&app, &file, &hint, &artist, false).await else {
            continue;
        };
        out.push(TagPreview {
            path,
            current_title: embedded.title.unwrap_or_default(),
            current_artist: embedded.artist.unwrap_or_default(),
            current_album: embedded.album.unwrap_or_default(),
            title: identity.title,
            artist: identity.artist,
            album: identity.album,
            release_id: identity.release_id,
            cover_url: identity.cover_url,
            score: identity.score,
        });
    }
    Ok(out)
}

#[tauri::command]
pub async fn apply_tags(app: tauri::AppHandle, edits: Vec<TagEdit>) -> Result<u32, String> {
    let dir = data_dir(&app);
    let edits = edits.into_iter().take(40).collect::<Vec<_>>();
    tokio::task::spawn_blocking(move || {
        let http = client();
        let mut applied = 0u32;
        for edit in edits {
            let file = PathBuf::from(&edit.path);
            if !file.is_file() {
                continue;
            }
            let cover = if edit.cover_url.trim().is_empty() {
                None
            } else {
                download_cover(&http, &edit.cover_url)
            };
            if write_track_tags(&file, &edit.title, &edit.artist, &edit.album, cover.as_deref()).is_err() {
                continue;
            }
            write_identity(
                &dir,
                &edit.path,
                &Identity {
                    title: edit.title,
                    artist: edit.artist,
                    album: edit.album,
                    duration: None,
                    recording_id: String::new(),
                    release_id: edit.release_id,
                    score: 100.0,
                    cover_url: edit.cover_url,
                },
            );
            applied += 1;
        }
        applied
    })
    .await
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn library_issues(app: tauri::AppHandle, root: String) -> Result<LibraryIssues, String> {
    let dir = data_dir(&app);
    let root = PathBuf::from(root.trim());
    tokio::task::spawn_blocking(move || collect_issues(&dir, &root))
        .await
        .map_err(|error| error.to_string())
}

fn collect_issues(dir: &Path, root: &Path) -> LibraryIssues {
    let fpcalc = ensure_fpcalc(dir);
    let mut items = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(folder) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if !is_playable_media(&path) {
                continue;
            }
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            let mut header = [0u8; 16];
            let read = std::fs::File::open(&path)
                .and_then(|mut file| std::io::Read::read(&mut file, &mut header))
                .unwrap_or(0);
            let broken = size < 64 || !media_header_ok(&header[..read]);
            let embedded = read_embedded(&path);
            let key = path.to_string_lossy().to_string();
            let identity = read_identity(dir, &key);
            let format = path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
            let fingerprint = if items.len() < 120 {
                fpcalc.as_ref().and_then(|bin| raw_fingerprint(bin, &path)).unwrap_or_default()
            } else {
                Vec::new()
            };
            let lyrics = sibling_lrc_paths(&path).iter().any(|candidate| candidate.is_file());
            let cover = bob_core::embedded_cover(&path).is_some();
            items.push(DupItem {
                path: key,
                title: identity
                    .as_ref()
                    .map(|item| item.title.clone())
                    .filter(|value| !value.is_empty())
                    .or(embedded.title)
                    .unwrap_or_default(),
                artist: identity
                    .as_ref()
                    .map(|item| item.artist.clone())
                    .filter(|value| !value.is_empty())
                    .or(embedded.artist)
                    .unwrap_or_default(),
                duration: embedded.duration.map(|value| value.round() as u32),
                recording_id: identity.map(|item| item.recording_id).unwrap_or_default(),
                size,
                broken,
                format,
                fingerprint,
                lyrics,
                cover,
            });
        }
    }
    let broken = items
        .iter()
        .filter(|item| item.broken)
        .map(|item| item.path.clone())
        .collect();
    LibraryIssues {
        duplicates: duplicate_groups(&items),
        broken,
    }
}

#[tauri::command]
pub async fn start_economic(app: tauri::AppHandle, paths: Vec<String>, delete_originals: bool) -> Result<(), String> {
    if CANCEL_ECONOMIC.swap(false, Ordering::Relaxed) {
        /* cleared */
    }
    CANCEL_ECONOMIC.store(false, Ordering::Relaxed);
    let ffmpeg = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.ffmpeg_bin.clone()
    };
    let Some(ffmpeg) = ffmpeg else {
        return Err("o ffmpeg ainda não está pronto.".into());
    };
    tauri::async_runtime::spawn(async move {
        run_economic(app, ffmpeg, paths, delete_originals).await;
    });
    Ok(())
}

#[tauri::command]
pub fn cancel_economic() {
    CANCEL_ECONOMIC.store(true, Ordering::Relaxed);
}

async fn run_economic(app: tauri::AppHandle, ffmpeg: PathBuf, paths: Vec<String>, delete_originals: bool) {
    let files = if paths.is_empty() {
        let root = app
            .state::<AppState>()
            .inner
            .clone()
            .lock_owned()
            .await
            .config
            .output_dir
            .clone();
        tokio::task::spawn_blocking(move || list_audio(&PathBuf::from(root)))
            .await
            .unwrap_or_default()
    } else {
        paths
    };
    let total = files.len() as u32;
    let mut done = 0u32;
    for path in files {
        if CANCEL_ECONOMIC.load(Ordering::Relaxed) {
            emit_economic(&app, done, total, "", "cancelled", None);
            return;
        }
        let file = PathBuf::from(&path);
        let ext = file.extension().and_then(|ext| ext.to_str()).unwrap_or("");
        let embedded = read_embedded(&file);
        let size = std::fs::metadata(&file).map(|meta| meta.len()).unwrap_or(0);
        if is_already_economic(ext, size, embedded.duration) {
            done += 1;
            emit_economic(&app, done, total, &path, "running", None);
            continue;
        }
        let dest = file.with_extension("opus");
        if dest == file {
            done += 1;
            continue;
        }
        emit_economic(&app, done, total, &path, "running", None);
        let status = tokio_command(&ffmpeg)
            .args([
                "-y",
                "-i",
                &path,
                "-vn",
                "-c:a",
                "libopus",
                "-b:a",
                "24k",
                "-map_metadata",
                "0",
            ])
            .arg(&dest)
            .output()
            .await;
        let ok = status.as_ref().map(|output| output.status.success()).unwrap_or(false) && dest.is_file();
        if !ok {
            let _ = std::fs::remove_file(&dest);
            emit_economic(&app, done, total, &path, "failed", Some("o ffmpeg não converteu essa faixa.".into()));
            done += 1;
            continue;
        }
        if let Some(lrc) = sibling_lrc_paths(&file).into_iter().next() {
            if lrc.is_file() {
                let next = dest.with_extension("lrc");
                let _ = std::fs::copy(&lrc, &next);
            }
        }
        if delete_originals && dest != file {
            let _ = std::fs::remove_file(&file);
            for sidecar in sibling_lrc_paths(&file) {
                if sidecar.is_file() && sidecar != dest.with_extension("lrc") {
                    let _ = std::fs::remove_file(sidecar);
                }
            }
            let from = path.clone();
            let to = dest.to_string_lossy().into_owned();
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                retarget_path(&app, &from, &to).await;
            });
        }
        done += 1;
        emit_economic(&app, done, total, &path, "running", None);
    }
    emit_economic(&app, done, total, "", "done", None);
}

pub fn list_media_public(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if is_playable_media(&path) {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    out
}

fn list_audio(root: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if bob_core::media_kind(&path) == "audio" {
                out.push(path.to_string_lossy().into_owned());
            }
        }
    }
    out
}

async fn retarget_path(app: &tauri::AppHandle, from: &str, to: &str) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let mut changed = false;
    for entry in &mut inner.history {
        if entry.path.as_deref().map(|path| bob_core::same_path(path, from)).unwrap_or(false) {
            entry.path = Some(to.to_string());
            changed = true;
        }
    }
    for job in &mut inner.jobs {
        if job.output_path.as_deref().map(|path| bob_core::same_path(path, from)).unwrap_or(false) {
            job.output_path = Some(to.to_string());
            changed = true;
        }
    }
    if changed {
        save(&inner);
        let jobs = inner.jobs.clone();
        let history = inner.history.clone();
        drop(inner);
        emit(app, &bob_core::BobEvent::Jobs { jobs });
        emit(app, &bob_core::BobEvent::History { history });
    }
}

fn emit_economic(app: &tauri::AppHandle, done: u32, total: u32, current: &str, state: &str, error: Option<String>) {
    emit(
        app,
        &bob_core::BobEvent::Economic {
            done,
            total,
            current: current.to_string(),
            state: state.to_string(),
            error,
        },
    );
}

#[tauri::command]
pub async fn translate_lines(app: tauri::AppHandle, lines: Vec<String>, target: String) -> Result<Vec<String>, String> {
    let target = match target.trim() {
        "en" => "en",
        "es" => "es",
        "fr" => "fr",
        _ => "pt",
    };
    let dir = data_dir(&app).join("translate");
    let _ = std::fs::create_dir_all(&dir);
    let lines = lines.into_iter().take(400).collect::<Vec<_>>();
    if let Some(translated) = crate::ai::translate_batch(&app, &lines, target).await {
        return Ok(translated);
    }
    tokio::task::spawn_blocking(move || translate_all(&dir, &lines, target))
        .await
        .map_err(|error| error.to_string())?
}

fn translate_all(dir: &Path, lines: &[String], target: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::with_capacity(lines.len());
    let http = client();
    let mut batch = Vec::new();
    let mut chars = 0usize;
    let flush = |http: &reqwest::blocking::Client, dir: &Path, batch: &mut Vec<String>, out: &mut Vec<String>, target: &str| -> Result<(), String> {
        if batch.is_empty() {
            return Ok(());
        }
        let joined = batch.join("\n");
        let translated = translate_text(http, dir, &joined, target)?;
        let parts: Vec<&str> = translated.split('\n').collect();
        if parts.len() == batch.len() {
            out.extend(parts.into_iter().map(|line| line.to_string()));
        } else {
            for line in batch.iter() {
                out.push(translate_text(http, dir, line, target).unwrap_or_else(|_| line.clone()));
            }
        }
        batch.clear();
        Ok(())
    };
    for line in lines {
        if chars + line.len() > 420 && !batch.is_empty() {
            flush(&http, dir, &mut batch, &mut out, target)?;
            chars = 0;
        }
        chars += line.len() + 1;
        batch.push(line.clone());
    }
    flush(&http, dir, &mut batch, &mut out, target)?;
    Ok(out)
}

fn translate_text(http: &reqwest::blocking::Client, dir: &Path, text: &str, target: &str) -> Result<String, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let key = format!("{}.txt", fnv(&format!("{target}\n{trimmed}")));
    let cache = dir.join(key);
    if let Ok(hit) = std::fs::read_to_string(&cache) {
        return Ok(hit);
    }
    if let Some(value) = google_translate(http, trimmed, target) {
        let _ = std::fs::write(&cache, &value);
        return Ok(value);
    }
    if let Some(value) = mymemory_translate(http, trimmed, target) {
        let _ = std::fs::write(&cache, &value);
        return Ok(value);
    }
    Err("não foi possível traduzir agora.".into())
}

fn google_translate(http: &reqwest::blocking::Client, text: &str, target: &str) -> Option<String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={}&dt=t&q={}",
        target,
        form_escape(text)
    );
    let body = plain_get(http, &url)?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    let rows = value.as_array()?.first()?.as_array()?;
    let mut out = String::new();
    for row in rows {
        if let Some(part) = row.as_array().and_then(|item| item.first()).and_then(|item| item.as_str()) {
            out.push_str(part);
        }
    }
    if out.trim().is_empty() {
        None
    } else {
        Some(out)
    }
}

fn mymemory_translate(http: &reqwest::blocking::Client, text: &str, target: &str) -> Option<String> {
    let pair = format!("autodetect|{target}");
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={}",
        form_escape(text),
        form_escape(&pair)
    );
    let body = plain_get(http, &url)?;
    let value: serde_json::Value = serde_json::from_str(&body).ok()?;
    let text = value.pointer("/responseData/translatedText")?.as_str()?.trim();
    if text.is_empty() || text.contains("MYMEMORY WARNING") {
        None
    } else {
        Some(text.to_string())
    }
}
