use std::path::Path;

use crate::media::media_kind;

/// On-disk cache for Drive playback. Old slices are dropped once the folder passes this.
pub const DRIVE_CACHE_LIMIT: u64 = 256 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachePiece {
    pub name: String,
    pub bytes: u64,
    pub used_at: i64,
}

/// `video` or `audio` from the Drive mime, then the file name, then the stored kind.
pub fn playback_kind(name: &str, mime: &str, stored: &str) -> &'static str {
    let mime = mime.trim().to_ascii_lowercase();
    if mime.starts_with("video/") {
        return "video";
    }
    if mime.starts_with("audio/") || mime == "application/ogg" {
        return "audio";
    }
    let by_name = media_kind(Path::new(name));
    if by_name != "unknown" {
        return by_name;
    }
    if stored == "video" {
        "video"
    } else {
        "audio"
    }
}

/// Prefer a real audio/video type. Fall back to the file name when Drive says octet-stream.
pub fn content_type_for(upstream: &str, name: &str) -> String {
    let upstream = upstream.trim();
    let lower = upstream.to_ascii_lowercase();
    if lower.starts_with("video/") || lower.starts_with("audio/") || lower.starts_with("image/") || lower.starts_with("text/") {
        return upstream.to_string();
    }
    let name = name.to_ascii_lowercase();
    let mapped = if name.ends_with(".mp4") || name.ends_with(".m4v") {
        "video/mp4"
    } else if name.ends_with(".webm") {
        "video/webm"
    } else if name.ends_with(".mkv") {
        "video/x-matroska"
    } else if name.ends_with(".mov") {
        "video/quicktime"
    } else if name.ends_with(".m4a") || name.ends_with(".aac") {
        "audio/mp4"
    } else if name.ends_with(".mp3") {
        "audio/mpeg"
    } else if name.ends_with(".opus") || name.ends_with(".ogg") || name.ends_with(".oga") {
        "audio/ogg"
    } else if name.ends_with(".flac") {
        "audio/flac"
    } else if name.ends_with(".wav") {
        "audio/wav"
    } else if name.ends_with(".lrc") || name.ends_with(".txt") {
        "text/plain; charset=utf-8"
    } else if name.ends_with(".png") {
        "image/png"
    } else if name.ends_with(".jpg") || name.ends_with(".jpeg") {
        "image/jpeg"
    } else if upstream.is_empty() {
        "application/octet-stream"
    } else {
        return upstream.to_string();
    };
    mapped.to_string()
}

pub fn image_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG") {
        "image/png"
    } else if bytes.starts_with(b"GIF8") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.len() > 12 && &bytes[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

/// `Range: bytes=START-END` or `bytes=START-`.
pub fn parse_byte_range(header: &str) -> Option<ByteRange> {
    let value = header.trim();
    let value = value.strip_prefix("bytes=").or_else(|| value.strip_prefix("bytes:"))?;
    let value = value.split(',').next()?.trim();
    let (start, end) = value.split_once('-')?;
    let start = start.trim();
    if start.is_empty() {
        return None;
    }
    let start = start.parse().ok()?;
    let end = end.trim();
    let end = if end.is_empty() { None } else { Some(end.parse().ok()?) };
    if end.is_some_and(|end| end < start) {
        return None;
    }
    Some(ByteRange { start, end })
}

pub fn content_range_value(start: u64, end_inclusive: u64, total: u64) -> String {
    format!("bytes {start}-{end_inclusive}/{total}")
}

/// Oldest pieces until the folder fits. `protect` is the file currently being streamed.
pub fn cache_victims(pieces: &[CachePiece], limit: u64, protect: &str) -> Vec<String> {
    let mut ordered: Vec<&CachePiece> = pieces.iter().filter(|piece| piece.name != protect).collect();
    ordered.sort_by_key(|piece| piece.used_at);
    let mut used: u64 = pieces.iter().map(|piece| piece.bytes).sum();
    let mut victims = Vec::new();
    for piece in ordered {
        if used <= limit {
            break;
        }
        victims.push(piece.name.clone());
        used = used.saturating_sub(piece.bytes);
    }
    victims
}

pub fn file_stem(name: &str) -> String {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name).trim();
    match name.rfind('.') {
        Some(index) if index > 0 => name[..index].to_string(),
        _ => name.to_string(),
    }
}

pub fn lyric_matches(media_name: &str, lyric_name: &str) -> bool {
    let lyric = lyric_name.rsplit(['/', '\\']).next().unwrap_or(lyric_name);
    if !lyric.to_ascii_lowercase().ends_with(".lrc") {
        return false;
    }
    let media = media_name.rsplit(['/', '\\']).next().unwrap_or(media_name);
    let lyric_stem = file_stem(lyric);
    lyric_stem.eq_ignore_ascii_case(&file_stem(media)) || lyric_stem.eq_ignore_ascii_case(media)
}

pub fn cover_matches(media_name: &str, cover_name: &str) -> bool {
    let cover = cover_name.rsplit(['/', '\\']).next().unwrap_or(cover_name);
    let lower = cover.to_ascii_lowercase();
    if matches!(lower.as_str(), "cover.jpg" | "cover.jpeg" | "cover.png" | "folder.jpg" | "folder.png") {
        return true;
    }
    if !(lower.ends_with(".jpg") || lower.ends_with(".jpeg") || lower.ends_with(".png") || lower.ends_with(".webp")) {
        return false;
    }
    let media = file_stem(media_name);
    let stem = file_stem(cover);
    stem.eq_ignore_ascii_case(&media) || stem.eq_ignore_ascii_case(&format!("{media}.cover"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_mp4_on_drive_is_video_even_when_the_stored_kind_says_audio() {
        assert_eq!(playback_kind("Disappeared Completely [x0NpAqKoHv4].mp4", "application/octet-stream", "audio"), "video");
        assert_eq!(playback_kind("faixa.mp3", "video/mp4", "audio"), "video");
        assert_eq!(playback_kind("faixa.bin", "", "audio"), "audio");
        assert_eq!(content_type_for("application/octet-stream", "clip.mp4"), "video/mp4");
        assert_eq!(content_type_for("video/mp4", "clip.mp4"), "video/mp4");
    }

    #[test]
    fn range_requests_keep_their_bounds() {
        assert_eq!(parse_byte_range("bytes=0-1023"), Some(ByteRange { start: 0, end: Some(1023) }));
        assert_eq!(parse_byte_range("bytes=500-"), Some(ByteRange { start: 500, end: None }));
        assert_eq!(parse_byte_range("bytes=-100"), None);
        assert_eq!(content_range_value(0, 99, 1000), "bytes 0-99/1000");
    }

    #[test]
    fn the_cache_drops_the_oldest_piece_and_keeps_the_one_in_use() {
        let pieces = vec![
            CachePiece { name: "old.bin".into(), bytes: 80, used_at: 1 },
            CachePiece { name: "live.bin".into(), bytes: 50, used_at: 2 },
            CachePiece { name: "new.bin".into(), bytes: 40, used_at: 3 },
        ];
        assert_eq!(cache_victims(&pieces, 80, "live.bin"), vec!["old.bin".to_string(), "new.bin".to_string()]);
        assert!(cache_victims(&pieces, 80, "live.bin").iter().all(|name| name != "live.bin"));
    }

    #[test]
    fn sidecars_match_the_media_basename() {
        let media = "Disappeared Completely - No Place for Me (Pt.1) - Official Video [x0NpAqKoHv4].mp4";
        assert!(lyric_matches(media, "Disappeared Completely - No Place for Me (Pt.1) - Official Video [x0NpAqKoHv4].lrc"));
        assert!(lyric_matches(media, "Disappeared Completely - No Place for Me (Pt.1) - Official Video [x0NpAqKoHv4].mp4.lrc"));
        assert!(!lyric_matches(media, "outra.lrc"));
        assert!(cover_matches(media, "cover.jpg"));
        assert!(cover_matches(media, "Disappeared Completely - No Place for Me (Pt.1) - Official Video [x0NpAqKoHv4].cover.jpg"));
        assert!(!cover_matches(media, "poster.txt"));
    }
}
