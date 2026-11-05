use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;

use crate::lyrics::split_artist_title;
use crate::media::{is_playable_media, media_kind};
use crate::read_embedded;

/// One playable file under the configured download folder.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ScannedMedia {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub folder: String,
    pub duration: Option<f64>,
    pub kind: String,
    pub size: u64,
    pub modified_ms: u64,
}

/// `\\?\C:\...` and `\\?\UNC\...` are the same folder as the path without that prefix.
pub fn strip_verbatim(path: &str) -> String {
    let text = path.trim();
    let lower = text.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix(r"\\?\unc\").map(|_| &text[8..]) {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = lower.strip_prefix("//?/unc/").map(|_| &text[8..]) {
        return format!("//{rest}");
    }
    if lower.starts_with(r"\\?\") {
        return text[4..].to_string();
    }
    if lower.starts_with("//?/") {
        return text[4..].to_string();
    }
    text.to_string()
}

pub fn normalize_path_key(path: &str) -> String {
    strip_verbatim(path)
        .replace('\\', "/")
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/")
        .to_lowercase()
}

pub fn same_path(left: &str, right: &str) -> bool {
    normalize_path_key(left) == normalize_path_key(right)
}

/// True when `file` is strictly inside `root`. `bob baixa extra` is not inside `bob baixa`.
pub fn string_inside(root: &str, file: &str) -> bool {
    let root = normalize_path_key(root);
    let file = normalize_path_key(file);
    if root.is_empty() || file.is_empty() || file == root {
        return false;
    }
    file.starts_with(&(root + "/"))
}

pub fn path_within(root: &Path, file: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(file) = file.canonicalize() else {
        return false;
    };
    file.starts_with(&root) && file != root
}

/// Folder name safe on Windows and elsewhere. Empty when the title has no usable characters.
pub fn sanitize_folder_name(title: &str) -> String {
    let mut out = String::new();
    for ch in title.chars() {
        if ch.is_control() || matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    let collapsed = out.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut name: String = collapsed
        .trim()
        .trim_matches(|ch: char| ch == '.' || ch.is_whitespace())
        .chars()
        .take(80)
        .collect();
    name = name
        .trim()
        .trim_end_matches(|ch: char| ch == '.' || ch.is_whitespace())
        .to_string();
    if name.is_empty() {
        return String::new();
    }
    if is_reserved(&name) {
        name.push_str(" playlist");
    }
    name
}

/// `outputDir/playlist title` when both pieces exist. Otherwise the root, unchanged.
pub fn playlist_output_dir(root: &str, title: &str) -> String {
    let root = root.trim().trim_end_matches(['/', '\\']);
    if root.is_empty() {
        return String::new();
    }
    let name = sanitize_folder_name(title);
    if name.is_empty() {
        return root.to_string();
    }
    format!("{root}/{name}")
}

/// Walk the download folder the user configured.
///
/// On Windows, `canonicalize` rewrites a path to `\\?\...` and may also resolve a
/// folder junction. Those strings no longer match `outputDir`, so every playable
/// file (mp3 and mp4 alike) was dropped and the library stayed empty.
pub fn scan_media_dir(root: &Path) -> Vec<ScannedMedia> {
    let root = configured_root(root);
    if !root.is_dir() {
        return Vec::new();
    }
    let canonical_root = root.canonicalize().ok();
    let mut out = Vec::new();
    walk(&root, &root, canonical_root.as_deref(), 0, &mut out);
    out.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    out
}

fn configured_root(root: &Path) -> PathBuf {
    let text = strip_verbatim(&root.to_string_lossy());
    let trimmed = text.trim();
    let keep_drive = trimmed.len() <= 3 && trimmed.as_bytes().get(1) == Some(&b':');
    let text = if keep_drive {
        trimmed.to_string()
    } else {
        trimmed.trim_end_matches(['/', '\\']).to_string()
    };
    PathBuf::from(text)
}

fn walk(root: &Path, dir: &Path, canonical_root: Option<&Path>, depth: usize, out: &mut Vec<ScannedMedia>) {
    if depth > 8 || out.len() >= 5000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= 5000 {
            return;
        }
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            if !meta.is_file() || !is_playable_media(&path) || escapes(canonical_root, &path) {
                continue;
            }
            out.push(describe(root, &path, meta.len(), modified_ms(&meta)));
            continue;
        }
        if file_type.is_dir() {
            walk(root, &path, canonical_root, depth + 1, out);
            continue;
        }
        if !file_type.is_file() || !is_playable_media(&path) {
            continue;
        }
        let meta = entry.metadata().ok();
        let size = meta.as_ref().map(|item| item.len()).unwrap_or(0);
        let modified = meta.as_ref().map(modified_ms).unwrap_or(0);
        out.push(describe(root, &path, size, modified));
    }
}

fn escapes(canonical_root: Option<&Path>, file: &Path) -> bool {
    let Some(root) = canonical_root else {
        return false;
    };
    match file.canonicalize() {
        Ok(canon) => !canon.starts_with(root),
        Err(_) => true,
    }
}

fn describe(root: &Path, file: &Path, size: u64, modified_ms: u64) -> ScannedMedia {
    let embedded = read_embedded(file);
    let stem = file
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .trim();
    let stem = strip_youtube_id(stem);
    let (stem_artist, stem_title) = split_artist_title(stem);
    let title = embedded
        .title
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            if stem_title.trim().is_empty() {
                stem.to_string()
            } else {
                stem_title
            }
        });
    let artist = embedded
        .artist
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(stem_artist);
    let folder = relative_folder(root, file);
    let album = embedded
        .album
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            folder
                .rsplit('/')
                .next()
                .filter(|value| !value.is_empty())
                .unwrap_or("")
                .to_string()
        });
    ScannedMedia {
        path: file.to_string_lossy().into_owned(),
        title,
        artist,
        album,
        folder,
        duration: embedded.duration,
        kind: media_kind(file).into(),
        size,
        modified_ms,
    }
}

fn relative_folder(root: &Path, file: &Path) -> String {
    let Some(parent) = file.parent() else {
        return String::new();
    };
    let Ok(rel) = parent.strip_prefix(root) else {
        return String::new();
    };
    rel.components()
        .filter_map(|component| component.as_os_str().to_str())
        .filter(|part| !part.is_empty() && *part != ".")
        .collect::<Vec<_>>()
        .join("/")
}

fn strip_youtube_id(stem: &str) -> &str {
    let Some(open) = stem.rfind('[') else {
        return stem;
    };
    if !stem.ends_with(']') {
        return stem;
    }
    let id = &stem[open + 1..stem.len() - 1];
    if is_youtube_id(id) {
        stem[..open].trim()
    } else {
        stem
    }
}

fn is_youtube_id(value: &str) -> bool {
    value.len() == 11
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn modified_ms(meta: &std::fs::Metadata) -> u64 {
    meta.modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .map(|time| time.as_millis() as u64)
        .unwrap_or(0)
}

fn is_reserved(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or(name);
    matches!(
        base.to_ascii_lowercase().as_str(),
        "con"
            | "prn"
            | "aux"
            | "nul"
            | "com1"
            | "com2"
            | "com3"
            | "com4"
            | "com5"
            | "com6"
            | "com7"
            | "com8"
            | "com9"
            | "lpt1"
            | "lpt2"
            | "lpt3"
            | "lpt4"
            | "lpt5"
            | "lpt6"
            | "lpt7"
            | "lpt8"
            | "lpt9"
    )
}

/// Sibling `.lrc` files that belong to a deleted track.
pub fn lyric_sidecars(media: &Path) -> Vec<PathBuf> {
    crate::lyrics::sibling_lrc_paths(media)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inside_check_rejects_a_sibling_prefix() {
        assert!(string_inside(
            r"C:\Downloads\bob baixa",
            r"C:\Downloads\bob baixa\faixa.mp3"
        ));
        assert!(string_inside(
            "c:/downloads/bob baixa",
            "C:/Downloads/bob baixa/aninhada/clipe.mp4"
        ));
        assert!(!string_inside(
            r"C:\Downloads\bob baixa",
            r"C:\Downloads\bob baixa extra\faixa.mp3"
        ));
        assert!(!string_inside("C:/Music", "C:/Music"));
        assert!(!string_inside("", "C:/Music/a.mp3"));
        assert!(same_path(r"C:\A\B.mp3", "c:/a/b.mp3"));
        assert!(string_inside(
            r"C:\Users\Gustavo\Downloads\bob baixa",
            r"\\?\C:\Users\Gustavo\Downloads\bob baixa\Don't Be so Serious [D56XxiS_pJ0].mp4"
        ));
        assert!(string_inside(
            r"\\?\C:\Users\Gustavo\Downloads\bob",
            r"C:\Users\Gustavo\Downloads\bob\faixa.mp3"
        ));
        assert!(!string_inside(
            r"C:\Users\Gustavo\Downloads\bob",
            r"\\?\C:\Users\Gustavo\Downloads\bob extra\a.mp4"
        ));
        assert!(string_inside(
            r"\\server\share\bob",
            r"\\?\UNC\server\share\bob\clipe.mp4"
        ));
        assert!(same_path(
            r"\\?\C:\Users\Gustavo\Downloads\bob\a.mp3",
            r"C:\Users\Gustavo\Downloads\bob\a.mp3"
        ));
        assert_eq!(
            strip_verbatim(r"\\?\C:\Users\Gustavo\Downloads\bob baixa\a.mp4"),
            r"C:\Users\Gustavo\Downloads\bob baixa\a.mp4"
        );
        assert_eq!(
            strip_verbatim(r"\\?\UNC\server\share\a.mp3"),
            r"\\server\share\a.mp3"
        );
    }

    #[test]
    fn playlist_folder_is_a_safe_child() {
        assert_eq!(sanitize_folder_name("  Lisboa / noite? "), "Lisboa noite");
        assert_eq!(sanitize_folder_name("con"), "con playlist");
        assert_eq!(sanitize_folder_name("..."), "");
        assert_eq!(
            playlist_output_dir(r"C:\Music\bob baixa", "A / B"),
            r"C:\Music\bob baixa/A B"
        );
        assert_eq!(playlist_output_dir("  ", "A"), "");
        assert_eq!(playlist_output_dir("C:/Music", "..."), "C:/Music");
    }

    #[test]
    fn scan_names_files_and_skips_the_rest() {
        let dir = std::env::temp_dir().join(format!("bob-scan-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("Disco")).unwrap();
        std::fs::write(
            dir.join("Disco").join("Artist - Song [dQw4w9WgXcQ].mp3"),
            b"nope",
        )
        .unwrap();
        std::fs::write(dir.join("nota.txt"), b"x").unwrap();
        std::fs::write(dir.join("Clip [abcdefghijk].mp4"), b"v").unwrap();
        let found = scan_media_dir(&dir);
        let root = dir.to_string_lossy().to_string();
        assert!(found.iter().all(|item| item.path.starts_with(&root)));
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].title, "Clip");
        assert_eq!(found[0].kind, "video");
        assert_eq!(found[0].folder, "");
        assert_eq!(found[1].artist, "Artist");
        assert_eq!(found[1].title, "Song");
        assert_eq!(found[1].folder, "Disco");
        assert_eq!(found[1].album, "Disco");
        assert_eq!(found[1].kind, "audio");
        assert!(found[1].size > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_folder_scans_as_empty() {
        let dir = std::env::temp_dir().join(format!("bob-scan-missing-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(scan_media_dir(&dir).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn scan_skips_a_symlink_that_leaves_the_folder() {
        let dir = std::env::temp_dir().join(format!("bob-scan-link-{}", std::process::id()));
        let outside = std::env::temp_dir().join(format!("bob-scan-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("fora.mp3"), b"x").unwrap();
        std::fs::write(dir.join("dentro.mp4"), b"v").unwrap();
        std::os::unix::fs::symlink(outside.join("fora.mp3"), dir.join("atalho.mp3")).unwrap();
        let found = scan_media_dir(&dir);
        assert_eq!(found.len(), 1);
        assert!(found[0].path.ends_with("dentro.mp4"));
        assert_eq!(found[0].kind, "video");
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_dir_all(&outside);
    }
}
