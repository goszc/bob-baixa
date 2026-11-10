//! Local index, cover cache and ranged streaming for Drive-only playback.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use bob_core::{
    cache_victims, content_range_value, content_type_for, cover_matches, embedded_cover, file_stem, first_audio_note,
    image_type, lyric_matches, parse_byte_range, playback_kind, read_embedded, read_sibling_lrc, widen_range, CachePiece,
    DRIVE_CACHE_LIMIT, HEAD_BYTES, READ_AHEAD_BYTES,
};
use serde::{Deserialize, Serialize};

use super::{auth, bearer, http, log_drive, resumable, root_dir, stream_get};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LibraryRecord {
    id: String,
    name: String,
    kind: String,
    mime: String,
    title: String,
    artist: String,
    album: String,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(default)]
    lyrics: String,
    #[serde(default)]
    lyrics_id: String,
    #[serde(default)]
    cover_id: String,
    #[serde(default)]
    cover_mime: String,
    #[serde(default)]
    size: u64,
    #[serde(default)]
    parent: String,
    #[serde(default)]
    probed: bool,
}

impl LibraryRecord {
    fn bare(id: &str) -> Self {
        Self {
            id: id.to_string(),
            name: String::new(),
            kind: "audio".into(),
            mime: String::new(),
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: None,
            lyrics: String::new(),
            lyrics_id: String::new(),
            cover_id: String::new(),
            cover_mime: String::new(),
            size: 0,
            parent: String::new(),
            probed: false,
        }
    }
}

#[derive(Clone)]
pub struct ListedTrack {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<f64>,
    pub kind: String,
    pub size: u64,
    pub name: String,
    pub cover: String,
}

pub struct MediaStream {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub length: Option<usize>,
    pub body: Box<dyn Read + Send>,
}

pub fn cover_dir() -> PathBuf {
    let dir = data_root().join("drive-covers");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

pub fn local_cover_bytes(file: &Path) -> Option<Vec<u8>> {
    if let Some(bytes) = embedded_cover(file) {
        return Some(bytes);
    }
    let stem = file.file_stem()?.to_string_lossy();
    let parent = file.parent()?;
    for name in [
        format!("{stem}.cover.jpg"),
        format!("{stem}.cover.png"),
        format!("{stem}.jpg"),
        format!("{stem}.png"),
        "cover.jpg".into(),
        "cover.png".into(),
        "folder.jpg".into(),
    ] {
        let path = parent.join(name);
        if let Ok(bytes) = std::fs::read(&path) {
            if bytes.len() > 32 {
                return Some(bytes);
            }
        }
    }
    None
}

pub fn snapshot_local(
    file: &Path,
    id: &str,
    parent: &str,
    name: &str,
    title: &str,
    artist: &str,
    album: &str,
    video: bool,
    lyrics_id: &str,
    cover_id: &str,
) {
    let embedded = read_embedded(file);
    let mut lyrics = embedded.lyrics.unwrap_or_default();
    if lyrics.trim().is_empty() {
        if let Some(text) = read_sibling_lrc(file) {
            lyrics = text;
        }
    }
    let mut record = find_record(id).unwrap_or_else(|| LibraryRecord::bare(id));
    record.name = name.to_string();
    record.kind = if video { "video".into() } else { playback_kind(name, "", "audio").into() };
    record.mime = content_type_for("", name);
    record.title = embedded.title.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| title.to_string());
    record.artist = embedded.artist.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| artist.to_string());
    record.album = embedded.album.filter(|value| !value.trim().is_empty()).unwrap_or_else(|| album.to_string());
    record.duration = embedded.duration.or(record.duration);
    record.lyrics = lyrics;
    record.lyrics_id = lyrics_id.to_string();
    record.cover_id = cover_id.to_string();
    record.parent = parent.to_string();
    record.size = std::fs::metadata(file).map(|meta| meta.len()).unwrap_or(record.size);
    record.probed = true;
    if let Some(bytes) = local_cover_bytes(file) {
        record.cover_mime = image_type(&bytes).into();
        let _ = std::fs::write(cover_file_path(id), bytes);
    }
    upsert(record);
    log_drive(&format!("ficha local guardada para {id}"));
}

pub fn note_remote(
    id: &str,
    name: &str,
    mime: &str,
    kind: &str,
    title: &str,
    artist: &str,
    album: &str,
    size: u64,
    parent: &str,
) -> ListedTrack {
    let mut record = find_record(id).unwrap_or_else(|| LibraryRecord::bare(id));
    record.name = name.to_string();
    record.mime = mime.to_string();
    record.kind = kind.to_string();
    if record.title.trim().is_empty() {
        record.title = title.to_string();
    }
    if record.artist.trim().is_empty() {
        record.artist = artist.to_string();
    }
    if record.album.trim().is_empty() {
        record.album = album.to_string();
    }
    if record.size == 0 {
        record.size = size;
    }
    if record.parent.is_empty() {
        record.parent = parent.to_string();
    }
    upsert(record.clone());
    listed(&record)
}

pub fn lyrics_text(id: &str) -> Option<String> {
    let mut record = ensure_record(id).ok()?;
    if !record.lyrics.trim().is_empty() {
        return Some(record.lyrics.clone());
    }
    attach_sidecars(&mut record);
    if record.lyrics_id.is_empty() {
        upsert(record);
        return None;
    }
    let text = download_text(&record.lyrics_id).ok()?;
    record.lyrics = text.clone();
    upsert(record);
    Some(text)
}

pub fn describe(id: &str) -> ListedTrack {
    match ensure_record(id) {
        Ok(record) => listed(&record),
        Err(_) => ListedTrack {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            duration: None,
            kind: "audio".into(),
            size: 0,
            name: String::new(),
            cover: String::new(),
        },
    }
}

pub fn cover_file(id: &str) -> Option<PathBuf> {
    let path = cover_file_path(id);
    path.is_file().then_some(path)
}

pub fn open_media(id: &str, range: Option<&str>) -> Result<MediaStream, String> {
    let started = Instant::now();
    let id = safe_id(id)?;
    let record = find_record(&id).unwrap_or_else(|| LibraryRecord::bare(&id));
    if let Some(stream) = serve_cache(&id, &record, range) {
        log_drive(&first_audio_note(&id, started.elapsed().as_millis(), "cache"));
        return Ok(stream);
    }
    if let Some(stream) = serve_covering(&id, &record, range) {
        log_drive(&first_audio_note(&id, started.elapsed().as_millis(), "cache"));
        return Ok(stream);
    }
    let start = range.and_then(parse_byte_range).map(|parsed| parsed.start).unwrap_or(0);
    if start == 0 {
        if let Some(stream) = serve_head(&id, &record) {
            log_drive(&first_audio_note(&id, started.elapsed().as_millis(), "cabeça local"));
            return Ok(stream);
        }
        if let Some(stream) = serve_preview(&id, &record, range) {
            log_drive(&first_audio_note(&id, started.elapsed().as_millis(), "prévia"));
            return Ok(stream);
        }
    }
    let widened = widen_range(range, READ_AHEAD_BYTES);
    let stream = stream_drive(&id, &record, widened.as_deref().or(range))?;
    log_drive(&first_audio_note(&id, started.elapsed().as_millis(), "drive"));
    Ok(stream)
}

pub fn warm_preview(id: &str) -> Result<(), String> {
    let id = safe_id(id)?;
    let head = head_path(&id);
    let preview = preview_path(&id);
    if head.is_file() && preview.is_file() {
        return Ok(());
    }
    let record = find_record(&id).unwrap_or_else(|| LibraryRecord::bare(&id));
    let preview_span = bob_core::preview_span(record.size, record.duration);
    let mut span = preview_span.max(HEAD_BYTES);
    if record.size > 0 {
        span = span.min(record.size);
    }
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media");
    let response = stream_get(&url, Some(&format!("bytes=0-{}", span.saturating_sub(1))))?;
    let bytes = response.bytes().map_err(|error| error.to_string())?;
    if let Some(parent) = head.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let head_len = (HEAD_BYTES as usize).min(bytes.len());
    if head_len > 0 {
        std::fs::write(&head, &bytes[..head_len]).map_err(|error| error.to_string())?;
    }
    let preview_len = (preview_span as usize).min(bytes.len());
    if preview_len > 0 && !preview.is_file() {
        std::fs::write(&preview, &bytes[..preview_len]).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn clear_stream_cache() -> u64 {
    let dir = cache_dir();
    let mut freed = 0u64;
    let Ok(entries) = std::fs::read_dir(&dir) else { return 0 };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("");
        if name.ends_with(".preview") || name.ends_with(".head") {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            freed += meta.len();
        }
        let _ = std::fs::remove_file(path);
    }
    freed
}

pub fn cache_bytes() -> u64 {
    dir_bytes(&cache_dir()) + dir_bytes(&data_root().join("offline"))
}

fn dir_bytes(dir: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(dir) else { return 0 };
    entries.flatten().map(|entry| entry.metadata().map(|meta| meta.len()).unwrap_or(0)).sum()
}

fn preview_path(id: &str) -> PathBuf {
    cache_dir().join(format!("{id}.preview"))
}

fn head_path(id: &str) -> PathBuf {
    cache_dir().join(format!("{id}.head"))
}

fn serve_head(id: &str, record: &LibraryRecord) -> Option<MediaStream> {
    let path = head_path(id);
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let total = record.size.max(bytes.len() as u64);
    let mime = content_type_for(&record.mime, &record.name);
    let body = PreviewThenDrive {
        preview: std::io::Cursor::new(bytes),
        rest_at: std::fs::metadata(&path).ok()?.len(),
        id: id.to_string(),
        rest: None,
    };
    Some(stream_parts(200, &mime, None, Some(total as usize), Box::new(body)))
}

fn serve_preview(id: &str, record: &LibraryRecord, range: Option<&str>) -> Option<MediaStream> {
    let path = preview_path(id);
    if !path.is_file() {
        return None;
    }
    let bytes = std::fs::read(&path).ok()?;
    if bytes.is_empty() {
        return None;
    }
    let start = range.and_then(parse_byte_range).map(|parsed| parsed.start).unwrap_or(0);
    if start > 0 {
        return None;
    }
    let total = record.size.max(bytes.len() as u64);
    let mime = content_type_for(&record.mime, &record.name);
    let body = PreviewThenDrive {
        preview: std::io::Cursor::new(bytes),
        rest_at: std::fs::metadata(&path).ok()?.len(),
        id: id.to_string(),
        rest: None,
    };
    Some(stream_parts(200, &mime, None, Some(total as usize), Box::new(body)))
}

struct PreviewThenDrive {
    preview: std::io::Cursor<Vec<u8>>,
    rest_at: u64,
    id: String,
    rest: Option<Box<dyn Read + Send>>,
}

impl Read for PreviewThenDrive {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let got = std::io::Read::read(&mut self.preview, buf)?;
        if got > 0 {
            return Ok(got);
        }
        if self.rest.is_none() {
            let tail = open_tail(&self.id, self.rest_at).map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))?;
            self.rest = Some(tail);
        }
        self.rest.as_mut().unwrap().read(buf)
    }
}

fn open_tail(id: &str, start: u64) -> Result<Box<dyn Read + Send>, String> {
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media");
    let end = start.saturating_add(READ_AHEAD_BYTES.saturating_sub(1));
    let response = stream_get(&url, Some(&format!("bytes={start}-{end}")))?;
    Ok(Box::new(response))
}

pub fn repair_library() -> Result<(u32, u32, u32), String> {
    repair_capped(u32::MAX)
}

pub fn repair_some() -> Result<(u32, u32, u32), String> {
    repair_capped(6)
}

fn repair_capped(limit: u32) -> Result<(u32, u32, u32), String> {
    let token = bearer(false).map_err(|error| error)?;
    let files = list_app_files(&token.token)?;
    let mut tracks = 0u32;
    let mut lyrics = 0u32;
    let mut covers = 0u32;
    let medias: Vec<_> = files
        .iter()
        .filter(|file| file.kind_prop == "audio" || file.kind_prop == "video" || playback_kind(&file.name, &file.mime, "") == "video" || is_audio_name(&file.name))
        .filter(|file| file.kind_prop != "lyrics" && file.kind_prop != "cover")
        .cloned()
        .collect();
    for media in medias {
        if media.kind_prop == "lyrics" || media.kind_prop == "cover" {
            continue;
        }
        let kind = playback_kind(&media.name, &media.mime, &media.kind_prop);
        if kind == "audio" && playback_kind(&media.name, &media.mime, "") == "audio" && media.kind_prop != "audio" && media.kind_prop != "video" && !is_audio_name(&media.name) && !is_video_name(&media.name) {
            continue;
        }
        if !is_audio_name(&media.name) && !is_video_name(&media.name) && media.kind_prop != "audio" && media.kind_prop != "video" {
            continue;
        }
        if tracks >= limit {
            break;
        }
        tracks += 1;
        let mut record = find_record(&media.id).unwrap_or_else(|| LibraryRecord::bare(&media.id));
        record.name = media.name.clone();
        record.mime = media.mime.clone();
        record.kind = kind.into();
        record.size = media.size;
        record.parent = media.parent.clone();
        if record.title.trim().is_empty() {
            record.title = media.title.clone();
        }
        if record.artist.trim().is_empty() {
            record.artist = media.artist.clone();
        }
        if record.album.trim().is_empty() {
            record.album = media.album.clone();
        }
        if let Some(lyric) = files.iter().find(|file| same_parent(file, &media) && (file.media_prop == media.id || lyric_matches(&media.name, &file.name))) {
            record.lyrics_id = lyric.id.clone();
        }
        if let Some(cover) = files.iter().find(|file| same_parent(file, &media) && (file.media_prop == media.id || file.kind_prop == "cover" && cover_matches(&media.name, &file.name) || cover_matches(&media.name, &file.name))) {
            record.cover_id = cover.id.clone();
        }
        if record.lyrics.trim().is_empty() && !record.lyrics_id.is_empty() {
            if let Ok(text) = download_text(&record.lyrics_id) {
                record.lyrics = text;
                lyrics += 1;
            }
        }
        if cover_file(&media.id).is_none() && !record.cover_id.is_empty() {
            if let Ok(bytes) = download_bytes(&record.cover_id, 8 * 1024 * 1024) {
                record.cover_mime = image_type(&bytes).into();
                if std::fs::write(cover_file_path(&media.id), &bytes).is_ok() {
                    covers += 1;
                }
            }
        }
        if !record.probed && media.size > 0 && media.size < 400 * 1024 * 1024 {
            let temp = data_root().join(format!("probe-{}.bin", media.id));
            if download_to(&token.token, &media.id, &temp, media.size).is_ok() {
                let embedded = read_embedded(&temp);
                if record.title.trim().is_empty() {
                    record.title = embedded.title.unwrap_or_default();
                }
                if record.artist.trim().is_empty() {
                    record.artist = embedded.artist.unwrap_or_default();
                }
                if record.album.trim().is_empty() {
                    record.album = embedded.album.unwrap_or_default();
                }
                record.duration = embedded.duration.or(record.duration);
                if record.lyrics.trim().is_empty() {
                    if let Some(text) = embedded.lyrics.filter(|text| !text.trim().is_empty()).or_else(|| read_sibling_lrc(&temp)) {
                        record.lyrics = text.clone();
                        lyrics += 1;
                        if record.lyrics_id.is_empty() && !record.parent.is_empty() {
                            let lrc_path = data_root().join(format!("probe-{}.lrc", media.id));
                            if std::fs::write(&lrc_path, &text).is_ok() {
                                let lrc_name = format!("{}.lrc", file_stem(&media.name));
                                if let Ok(lyrics_id) = resumable(&token.token, &lrc_path, &lrc_name, "text/plain", &record.parent, &[("bobKind", "lyrics"), ("bobMedia", media.id.as_str())]) {
                                    record.lyrics_id = lyrics_id;
                                }
                                let _ = std::fs::remove_file(&lrc_path);
                            }
                        }
                    }
                }
                if cover_file(&media.id).is_none() {
                    if let Some(bytes) = embedded_cover(&temp) {
                        record.cover_mime = image_type(&bytes).into();
                        if std::fs::write(cover_file_path(&media.id), &bytes).is_ok() {
                            covers += 1;
                            if record.cover_id.is_empty() && !record.parent.is_empty() {
                                let ext = if record.cover_mime == "image/png" { "png" } else { "jpg" };
                                let cover_path = data_root().join(format!("probe-{}.{}", media.id, ext));
                                if std::fs::write(&cover_path, &bytes).is_ok() {
                                    let cover_name = format!("{}.cover.{ext}", file_stem(&media.name));
                                    if let Ok(cover_id) = resumable(&token.token, &cover_path, &cover_name, &record.cover_mime, &record.parent, &[("bobKind", "cover"), ("bobMedia", media.id.as_str())]) {
                                        record.cover_id = cover_id;
                                    }
                                    let _ = std::fs::remove_file(&cover_path);
                                }
                            }
                        }
                    }
                }
                record.probed = true;
                let _ = std::fs::remove_file(&temp);
            }
        }
        upsert(record);
        log_drive(&format!("reparo {}", media.name));
    }
    Ok((tracks, lyrics, covers))
}

fn is_audio_name(name: &str) -> bool {
    playback_kind(name, "", "") == "audio" && file_stem(name) != name.trim()
}

fn is_video_name(name: &str) -> bool {
    playback_kind(name, "", "") == "video"
}

fn same_parent(file: &RemoteFile, media: &RemoteFile) -> bool {
    !file.parent.is_empty() && file.parent == media.parent
}

fn listed(record: &LibraryRecord) -> ListedTrack {
    let raw = if record.title.trim().is_empty() { file_stem(&record.name) } else { record.title.clone() };
    let (artist, title) = if record.artist.trim().is_empty() {
        let (artist, title) = bob_core::split_artist_title(&raw);
        (artist, bob_core::clean_track_query(&title))
    } else {
        (record.artist.clone(), bob_core::clean_track_query(&raw))
    };
    ListedTrack {
        title: if title.is_empty() { raw } else { title },
        artist,
        album: record.album.clone(),
        duration: record.duration,
        kind: record.kind.clone(),
        size: record.size,
        name: record.name.clone(),
        cover: if cover_file(&record.id).is_some() {
            format!("http://127.0.0.1:47821/cover/{}", record.id)
        } else {
            String::new()
        },
    }
}

fn attach_sidecars(record: &mut LibraryRecord) {
    if record.parent.is_empty() {
        return;
    }
    let Ok(token) = bearer(false) else {
        return;
    };
    let Ok(files) = list_children(&token.token, &record.parent) else {
        return;
    };
    if record.lyrics_id.is_empty() {
        if let Some(file) = files.iter().find(|file| lyric_matches(&record.name, &file.name) || file.media_prop == record.id && file.name.to_ascii_lowercase().ends_with(".lrc")) {
            record.lyrics_id = file.id.clone();
        }
    }
    if record.cover_id.is_empty() {
        if let Some(file) = files.iter().find(|file| cover_matches(&record.name, &file.name) || file.kind_prop == "cover" && file.media_prop == record.id) {
            record.cover_id = file.id.clone();
        }
    }
    if cover_file(&record.id).is_none() && !record.cover_id.is_empty() {
        if let Ok(bytes) = download_bytes(&record.cover_id, 8 * 1024 * 1024) {
            record.cover_mime = image_type(&bytes).into();
            let _ = std::fs::write(cover_file_path(&record.id), bytes);
        }
    }
}

fn serve_covering(id: &str, record: &LibraryRecord, range: Option<&str>) -> Option<MediaStream> {
    let start = range.and_then(parse_byte_range).map(|parsed| parsed.start).unwrap_or(0);
    let prefix = format!("{id}-");
    let dir = cache_dir();
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.starts_with(&prefix) || !name.ends_with(".bin") {
            continue;
        }
        let middle = name.trim_start_matches(&prefix).trim_end_matches(".bin");
        let Some((raw_start, raw_end)) = middle.split_once('-') else {
            continue;
        };
        let (Ok(file_start), Ok(file_end)) = (raw_start.parse::<u64>(), raw_end.parse::<u64>()) else {
            continue;
        };
        if start < file_start || start > file_end {
            continue;
        }
        let mut file = std::fs::File::open(entry.path()).ok()?;
        let offset = start - file_start;
        file.seek(std::io::SeekFrom::Start(offset)).ok()?;
        let len = file_end - start + 1;
        let mime = content_type_for(&record.mime, &record.name);
        let total = record.size.max(file_end.saturating_add(1));
        return Some(stream_parts(
            206,
            &mime,
            Some(content_range_value(start, file_end, total)),
            Some(len as usize),
            Box::new(file.take(len)),
        ));
    }
    None
}

fn serve_cache(id: &str, record: &LibraryRecord, range: Option<&str>) -> Option<MediaStream> {
    let full = cache_dir().join(format!("{id}.bin"));
    if full.is_file() {
        return serve_file(&full, record, range, true);
    }
    let parsed = range.and_then(parse_byte_range)?;
    let end = parsed.end?;
    let slice = cache_dir().join(format!("{id}-{}-{end}.bin", parsed.start));
    if !slice.is_file() {
        return None;
    }
    serve_file(&slice, record, None, false).map(|mut stream| {
        stream.status = 206;
        stream.headers.push((
            "Content-Range".into(),
            content_range_value(parsed.start, end, record.size.max(end + 1)),
        ));
        stream
    })
}

fn serve_file(path: &Path, record: &LibraryRecord, range: Option<&str>, allow_range: bool) -> Option<MediaStream> {
    let mut file = std::fs::File::open(path).ok()?;
    let _ = file.set_modified(SystemTime::now());
    let total = file.metadata().ok()?.len();
    let mime = content_type_for(&record.mime, &record.name);
    if allow_range {
        if let Some(parsed) = range.and_then(parse_byte_range) {
            if parsed.start >= total {
                return Some(error_stream(416, "faixa fora do arquivo"));
            }
            let end = parsed.end.unwrap_or(total.saturating_sub(1)).min(total.saturating_sub(1));
            file.seek(std::io::SeekFrom::Start(parsed.start)).ok()?;
            let len = end - parsed.start + 1;
            return Some(stream_parts(
                206,
                &mime,
                Some(content_range_value(parsed.start, end, total)),
                Some(len as usize),
                Box::new(file.take(len)),
            ));
        }
    }
    Some(stream_parts(200, &mime, None, Some(total as usize), Box::new(file)))
}

fn stream_drive(id: &str, record: &LibraryRecord, range: Option<&str>) -> Result<MediaStream, String> {
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media");
    log_drive(&format!("stream {id} range {}", range.unwrap_or("-")));
    let response = stream_get(&url, range)?;
    let status = response.status().as_u16();
    let upstream = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    let mime = content_type_for(upstream, &record.name);
    let content_range = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let length = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    let parsed = range.and_then(parse_byte_range);
    let full = parsed.is_none() || parsed.as_ref().is_some_and(|item| item.start == 0 && length.is_some_and(|len| record.size > 0 && len >= record.size));
    let final_path = if full {
        cache_dir().join(format!("{id}.bin"))
    } else if let Some(item) = &parsed {
        let end = item.end.or_else(|| length.map(|len| item.start + len - 1)).unwrap_or(item.start);
        cache_dir().join(format!("{id}-{}-{end}.bin", item.start))
    } else {
        cache_dir().join(format!("{id}.bin"))
    };
    let partial = cache_dir().join(format!("{id}.partial"));
    let file = std::fs::File::create(&partial).ok();
    let body = Box::new(CacheWriter {
        inner: response,
        file,
        partial,
        final_path,
        expected: length,
        written: 0,
        finished: false,
    });
    Ok(stream_parts(status, &mime, content_range, length.map(|value| value as usize), body))
}

struct CacheWriter<R> {
    inner: R,
    file: Option<std::fs::File>,
    partial: PathBuf,
    final_path: PathBuf,
    expected: Option<u64>,
    written: u64,
    finished: bool,
}

impl<R: Read> Read for CacheWriter<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let count = self.inner.read(buf)?;
        if count > 0 {
            if let Some(file) = self.file.as_mut() {
                let _ = file.write_all(&buf[..count]);
            }
            self.written += count as u64;
        }
        if count == 0 {
            self.finish();
        }
        Ok(count)
    }
}

impl<R> CacheWriter<R> {
    fn finish(&mut self) {
        if self.finished {
            return;
        }
        self.finished = true;
        if let Some(file) = self.file.take() {
            let _ = file.sync_all();
        }
        let complete = self.expected.is_some_and(|expected| expected > 0 && self.written == expected);
        if complete {
            let _ = std::fs::rename(&self.partial, &self.final_path);
            evict(self.final_path.file_name().and_then(|name| name.to_str()).unwrap_or(""));
        } else {
            let _ = std::fs::remove_file(&self.partial);
        }
    }
}

impl<R> Drop for CacheWriter<R> {
    fn drop(&mut self) {
        if !self.finished {
            self.file.take();
            let _ = std::fs::remove_file(&self.partial);
        }
    }
}

fn evict(protect: &str) {
    let dir = cache_dir();
    let mut pieces = Vec::new();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".partial") || name.ends_with(".head") || name.ends_with(".preview") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let used = meta.modified().ok().and_then(|time| time.duration_since(UNIX_EPOCH).ok()).map(|time| time.as_secs() as i64).unwrap_or(0);
        pieces.push(CachePiece { name, bytes: meta.len(), used_at: used });
    }
    for name in cache_victims(&pieces, DRIVE_CACHE_LIMIT, protect) {
        let _ = std::fs::remove_file(dir.join(name));
    }
}

fn stream_parts(status: u16, mime: &str, content_range: Option<String>, length: Option<usize>, body: Box<dyn Read + Send>) -> MediaStream {
    let mut headers = cors();
    headers.push(("Content-Type".into(), mime.to_string()));
    headers.push(("Accept-Ranges".into(), "bytes".into()));
    if let Some(value) = content_range {
        headers.push(("Content-Range".into(), value));
    }
    MediaStream { status, headers, length, body }
}

fn error_stream(status: u16, message: &str) -> MediaStream {
    let bytes = message.as_bytes().to_vec();
    let length = bytes.len();
    MediaStream {
        status,
        headers: cors(),
        length: Some(length),
        body: Box::new(std::io::Cursor::new(bytes)),
    }
}

pub fn cors() -> Vec<(String, String)> {
    vec![
        ("Access-Control-Allow-Origin".into(), "*".into()),
        ("Access-Control-Allow-Methods".into(), "GET, HEAD, OPTIONS".into()),
        ("Access-Control-Allow-Headers".into(), "Range".into()),
        ("Access-Control-Expose-Headers".into(), "Content-Range, Accept-Ranges, Content-Length, Content-Type".into()),
    ]
}

fn ensure_record(id: &str) -> Result<LibraryRecord, String> {
    if let Some(record) = find_record(id) {
        if !record.name.is_empty() {
            return Ok(record);
        }
    }
    let access = bearer(false)?;
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?fields=id,name,mimeType,size,parents,appProperties");
    let response = http().get(url).header("authorization", auth(&access.token)).send().map_err(|error| error.to_string())?;
    let body = response.text().map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let name = value.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string();
    let mime = value.get("mimeType").and_then(|item| item.as_str()).unwrap_or("");
    let stored = value.pointer("/appProperties/bobKind").and_then(|item| item.as_str()).unwrap_or("");
    let parent = value.get("parents").and_then(|item| item.as_array()).and_then(|items| items.first()).and_then(|item| item.as_str()).unwrap_or("");
    Ok(note_record(
        id,
        &name,
        mime,
        playback_kind(&name, mime, stored),
        value.pointer("/appProperties/bobTitle").and_then(|item| item.as_str()).unwrap_or(""),
        value.pointer("/appProperties/bobArtist").and_then(|item| item.as_str()).unwrap_or(""),
        value.pointer("/appProperties/bobAlbum").and_then(|item| item.as_str()).unwrap_or(""),
        value.get("size").and_then(|item| item.as_str()).and_then(|item| item.parse().ok()).unwrap_or(0),
        parent,
    ))
}

fn note_record(id: &str, name: &str, mime: &str, kind: &str, title: &str, artist: &str, album: &str, size: u64, parent: &str) -> LibraryRecord {
    let listed = note_remote(id, name, mime, kind, title, artist, album, size, parent);
    let _ = listed;
    find_record(id).unwrap_or_else(|| LibraryRecord::bare(id))
}

#[derive(Clone)]
struct RemoteFile {
    id: String,
    name: String,
    mime: String,
    size: u64,
    parent: String,
    kind_prop: String,
    media_prop: String,
    title: String,
    artist: String,
    album: String,
}

fn list_app_files(token: &str) -> Result<Vec<RemoteFile>, String> {
    list_query(token, "trashed = false")
}

fn list_children(token: &str, parent: &str) -> Result<Vec<RemoteFile>, String> {
    list_query(token, &format!("'{parent}' in parents and trashed = false"))
}

fn list_query(token: &str, query: &str) -> Result<Vec<RemoteFile>, String> {
    let mut page = String::new();
    let mut out = Vec::new();
    loop {
        let mut url = format!(
            "https://www.googleapis.com/drive/v3/files?pageSize=200&fields=nextPageToken,files(id,name,mimeType,size,parents,appProperties)&q={}",
            urlencoding(query)
        );
        if !page.is_empty() {
            url.push_str("&pageToken=");
            url.push_str(&urlencoding(&page));
        }
        let response = http().get(url).header("authorization", auth(token)).send().map_err(|error| error.to_string())?;
        let body = response.text().map_err(|error| error.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
        let Some(files) = value.get("files").and_then(|item| item.as_array()) else {
            break;
        };
        for file in files {
            out.push(RemoteFile {
                id: file.get("id").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                name: file.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                mime: file.get("mimeType").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                size: file.get("size").and_then(|item| item.as_str()).and_then(|item| item.parse().ok()).unwrap_or(0),
                parent: file.get("parents").and_then(|item| item.as_array()).and_then(|items| items.first()).and_then(|item| item.as_str()).unwrap_or("").to_string(),
                kind_prop: file.pointer("/appProperties/bobKind").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                media_prop: file.pointer("/appProperties/bobMedia").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                title: file.pointer("/appProperties/bobTitle").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                artist: file.pointer("/appProperties/bobArtist").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                album: file.pointer("/appProperties/bobAlbum").and_then(|item| item.as_str()).unwrap_or("").to_string(),
            });
        }
        match value.get("nextPageToken").and_then(|item| item.as_str()) {
            Some(next) if !next.is_empty() => page = next.to_string(),
            _ => break,
        }
    }
    Ok(out)
}

fn download_text(id: &str) -> Result<String, String> {
    let bytes = download_bytes(id, 1_000_000)?;
    String::from_utf8(bytes).map_err(|_| "letra ilegível".into())
}

fn download_bytes(id: &str, max: usize) -> Result<Vec<u8>, String> {
    let access = bearer(false)?;
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media");
    let response = http().get(url).header("authorization", auth(&access.token)).send().map_err(|error| error.to_string())?;
    let bytes = response.bytes().map_err(|error| error.to_string())?;
    if bytes.len() > max {
        return Err("arquivo grande demais".into());
    }
    Ok(bytes.to_vec())
}

fn download_to(_token: &str, id: &str, dest: &Path, size: u64) -> Result<(), String> {
    if size > 400 * 1024 * 1024 {
        return Err("grande demais para sondar".into());
    }
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media");
    let mut response = stream_get(&url, None)?;
    let mut file = std::fs::File::create(dest).map_err(|error| error.to_string())?;
    std::io::copy(&mut response, &mut file).map_err(|error| error.to_string())?;
    Ok(())
}

fn find_record(id: &str) -> Option<LibraryRecord> {
    load_library().into_iter().find(|record| record.id == id)
}

fn upsert(record: LibraryRecord) {
    let mut records = load_library();
    if let Some(slot) = records.iter_mut().find(|item| item.id == record.id) {
        *slot = record;
    } else {
        records.push(record);
    }
    save_library(&records);
}

fn load_library() -> Vec<LibraryRecord> {
    let Ok(text) = std::fs::read_to_string(library_path()) else {
        return Vec::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn save_library(records: &[LibraryRecord]) {
    if let Some(parent) = library_path().parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(records) {
        let _ = std::fs::write(library_path(), text);
    }
}

pub fn keep_offline(id: &str) -> Result<(), String> {
    let id = safe_id(id)?;
    let dest = data_root().join("offline").join(&id);
    if dest.is_file() {
        return Ok(());
    }
    if let Some(parent) = dest.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let access = bearer(false)?;
    let size = find_record(&id).map(|record| record.size).unwrap_or(0);
    download_to(&access.token, &id, &dest, size)
}

pub fn offline_copy(id: &str) -> Option<PathBuf> {
    let id = safe_id(id).ok()?;
    let path = data_root().join("offline").join(id);
    path.is_file().then_some(path)
}

pub fn ids() -> Vec<String> {
    load_library().into_iter().map(|record| record.id).collect()
}

fn library_path() -> PathBuf {
    data_root().join("drive-library.json")
}

fn cache_dir() -> PathBuf {
    let dir = data_root().join("drive-cache");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn cover_file_path(id: &str) -> PathBuf {
    cover_dir().join(format!("{id}.img"))
}

fn data_root() -> PathBuf {
    root_dir().lock().ok().map(|dir| dir.clone()).filter(|dir| !dir.as_os_str().is_empty()).unwrap_or_else(|| std::env::temp_dir().join("bob-baixa"))
}

fn safe_id(id: &str) -> Result<String, String> {
    let id = id.split(['?', '#']).next().unwrap_or(id).trim();
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
        return Err("id".into());
    }
    Ok(id.to_string())
}

fn urlencoding(value: &str) -> String {
    bob_core::percent_encode(value)
}
