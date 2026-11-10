use serde::Serialize;

use crate::{base_recording_title, fold_lyric_name, score_lyric_match};

#[derive(Debug, Clone)]
pub struct DupItem {
    pub path: String,
    pub title: String,
    pub artist: String,
    pub duration: Option<u32>,
    pub recording_id: String,
    pub size: u64,
    pub broken: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DupGroup {
    pub reason: String,
    pub paths: Vec<String>,
}

pub fn media_header_ok(header: &[u8]) -> bool {
    if header.len() < 8 {
        return false;
    }
    if header.starts_with(b"ID3")
        || header.starts_with(b"OggS")
        || header.starts_with(b"fLaC")
        || header.starts_with(b"RIFF")
        || header.starts_with(b"\x1aE\xdf\xa3")
    {
        return true;
    }
    if header.len() >= 8 && &header[4..8] == b"ftyp" {
        return true;
    }
    header[0] == 0xFF && header[1] & 0xE0 == 0xE0
}

pub fn is_already_economic(extension: &str, size: u64, duration_secs: Option<f64>) -> bool {
    if !extension.eq_ignore_ascii_case("opus") {
        return false;
    }
    let Some(duration) = duration_secs.filter(|value| *value >= 1.0) else {
        return false;
    };
    let bitrate = (size as f64) * 8.0 / duration;
    bitrate <= 40_000.0
}

pub fn duplicate_groups(items: &[DupItem]) -> Vec<DupGroup> {
    let mut groups = Vec::new();
    let mut used = vec![false; items.len()];
    let mut ids: Vec<(String, Vec<usize>)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let id = item.recording_id.trim();
        if id.is_empty() || item.broken {
            continue;
        }
        if let Some((_, bucket)) = ids.iter_mut().find(|(key, _)| key == id) {
            bucket.push(index);
        } else {
            ids.push((id.to_string(), vec![index]));
        }
    }
    for (_, bucket) in ids {
        if bucket.len() < 2 {
            continue;
        }
        for index in &bucket {
            used[*index] = true;
        }
        groups.push(DupGroup {
            reason: "recording".into(),
            paths: bucket.iter().map(|index| items[*index].path.clone()).collect(),
        });
    }
    for left in 0..items.len() {
        if used[left] || items[left].broken {
            continue;
        }
        let mut bucket = vec![left];
        for right in (left + 1)..items.len() {
            if used[right] || items[right].broken {
                continue;
            }
            if similar(&items[left], &items[right]) {
                bucket.push(right);
            }
        }
        if bucket.len() >= 2 {
            for index in &bucket {
                used[*index] = true;
            }
            groups.push(DupGroup {
                reason: "similar".into(),
                paths: bucket.iter().map(|index| items[*index].path.clone()).collect(),
            });
        }
    }
    groups
}

fn similar(left: &DupItem, right: &DupItem) -> bool {
    if !duration_close(left.duration, right.duration) {
        return false;
    }
    let score = score_lyric_match(
        &base_recording_title(&left.title),
        &left.artist,
        left.duration,
        &base_recording_title(&right.title),
        &right.artist,
        right.duration,
        false,
    );
    let same_title = fold_lyric_name(&base_recording_title(&left.title))
        == fold_lyric_name(&base_recording_title(&right.title))
        && !fold_lyric_name(&left.title).is_empty();
    same_title || score >= 80
}

fn duration_close(left: Option<u32>, right: Option<u32>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.abs_diff(right) <= 3,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(path: &str, title: &str, artist: &str, duration: u32, recording: &str) -> DupItem {
        DupItem {
            path: path.into(),
            title: title.into(),
            artist: artist.into(),
            duration: Some(duration),
            recording_id: recording.into(),
            size: 1000,
            broken: false,
        }
    }

    #[test]
    fn groups_the_same_recording_and_a_similar_title() {
        let items = vec![
            item("a.mp3", "Canção", "Banda", 180, "rec"),
            item("b.opus", "Canção", "Banda", 180, "rec"),
            item("c.mp3", "Outra", "Banda", 200, ""),
            item("d.mp3", "Outra (Live)", "Banda", 201, ""),
            item("e.mp3", "Longe", "Outro", 180, ""),
        ];
        let groups = duplicate_groups(&items);
        assert!(groups.iter().any(|group| group.reason == "recording" && group.paths.len() == 2));
        assert!(groups.iter().any(|group| group.reason == "similar" && group.paths == ["c.mp3", "d.mp3"]));
        assert!(!groups.iter().any(|group| group.paths.iter().any(|path| path == "e.mp3")));
    }

    #[test]
    fn economic_opus_and_broken_headers() {
        assert!(is_already_economic("opus", 180 * 24_000 / 8, Some(180.0)));
        assert!(!is_already_economic("opus", 180 * 128_000 / 8, Some(180.0)));
        assert!(!is_already_economic("mp3", 1000, Some(180.0)));
        assert!(media_header_ok(b"ID3\x03\x00\x00\x00\x00"));
        assert!(media_header_ok(b"\x00\x00\x00\x18ftypisom"));
        assert!(media_header_ok(b"OggSxxxx"));
        assert!(!media_header_ok(b"this is not audio!!"));
        assert!(!media_header_ok(b"short"));
    }
}
