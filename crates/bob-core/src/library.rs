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

pub fn normalize_path_key(path: &str) -> String {
    path.trim()
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

/// Walk the download folder. Files outside the canonical root are ignored.
pub fn scan_media_dir(root: &Path) -> Vec<ScannedMedia> {
    let Ok(root) = root.canonicalize() else {
        return Vec::new();
    };
    if !root.is_dir() {
        return Vec::new();
    }
    let mut out = Vec::new();
    walk(&root, &root, 0, &mut out);
    out.sort_by(|left, right| {
        left.title
            .to_lowercase()
            .cmp(&right.title.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    out
}

fn walk(root: &Path, dir: &Path, depth: usize, out: &mut Vec<ScannedMedia>) {
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
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            walk(root, &path, depth + 1, out);
            continue;
        }
        if !meta.is_file() || !is_playable_media(&path) {
            continue;
        }
        let Ok(canon) = path.canonicalize() else {
            continue;
        };
        if !canon.starts_with(root) || canon == *root {
            continue;
        }
        out.push(describe(root, &canon, meta.len(), modified_ms(&meta)));
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
        let found = scan_media_dir(&dir);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].artist, "Artist");
        assert_eq!(found[0].title, "Song");
        assert_eq!(found[0].folder, "Disco");
        assert_eq!(found[0].album, "Disco");
        assert_eq!(found[0].kind, "audio");
        assert!(found[0].size > 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
