use std::path::Path;

/// `audio`, `video`, or `unknown` from the file extension the downloader writes.
pub fn media_kind(path: &Path) -> &'static str {
    match extension(path).as_str() {
        "mp4" | "mkv" | "webm" | "mov" | "m4v" | "avi" => "video",
        "mp3" | "m4a" | "aac" | "flac" | "opus" | "ogg" | "oga" | "wav" => "audio",
        _ => "unknown",
    }
}

pub fn is_playable_media(path: &Path) -> bool {
    matches!(media_kind(path), "audio" | "video")
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn kind_matches_formats_the_app_writes() {
        assert_eq!(media_kind(Path::new(r"C:\Música\faixa.mp3")), "audio");
        assert_eq!(media_kind(Path::new("/tmp/a.M4A")), "audio");
        assert_eq!(media_kind(Path::new("/tmp/a.opus")), "audio");
        assert_eq!(media_kind(Path::new("/tmp/a.flac")), "audio");
        assert_eq!(media_kind(Path::new("/tmp/a.wav")), "audio");
        assert_eq!(media_kind(Path::new("/tmp/a.mp4")), "video");
        assert_eq!(media_kind(Path::new("/tmp/a.webm")), "video");
        assert_eq!(media_kind(Path::new("/tmp/a.mkv")), "video");
        assert_eq!(media_kind(Path::new("/tmp/a.txt")), "unknown");
        assert!(!is_playable_media(Path::new("/tmp/a.txt")));
        assert!(is_playable_media(Path::new("/tmp/a.mp3")));
    }
}
