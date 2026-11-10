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
    pub format: String,
    pub fingerprint: Vec<u32>,
    pub lyrics: bool,
    pub cover: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DupCopy {
    pub path: String,
    pub format: String,
    pub bitrate: u32,
    pub size: u64,
    pub lyrics: bool,
    pub cover: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DupGroup {
    pub reason: String,
    pub paths: Vec<String>,
    pub copies: Vec<DupCopy>,
    pub worse: String,
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
        groups.push(group_from("recording", items, &bucket));
    }
    let mut prints: Vec<(Vec<u32>, Vec<usize>)> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        if used[index] || item.broken || item.fingerprint.len() < 12 {
            continue;
        }
        if let Some((_, bucket)) = prints.iter_mut().find(|(key, _)| fingerprints_match(key, &item.fingerprint)) {
            bucket.push(index);
        } else {
            prints.push((item.fingerprint.clone(), vec![index]));
        }
    }
    for (_, bucket) in prints {
        if bucket.len() < 2 {
            continue;
        }
        for index in &bucket {
            used[*index] = true;
        }
        groups.push(group_from("fingerprint", items, &bucket));
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
            groups.push(group_from("similar", items, &bucket));
        }
    }
    groups
}

/// Chromaprint raw hashes of the same recording stay close even when the file name does not.
pub fn fingerprints_match(left: &[u32], right: &[u32]) -> bool {
    if left.len() < 12 || right.len() < 12 {
        return false;
    }
    for shift in 0..5 {
        if aligned(&left[shift..], right) || aligned(&right[shift..], left) {
            return true;
        }
    }
    false
}

fn aligned(left: &[u32], right: &[u32]) -> bool {
    let n = left.len().min(right.len()).min(64);
    if n < 12 {
        return false;
    }
    let same = (0..n).filter(|index| left[*index] == right[*index]).count();
    same * 5 >= n * 4
}

fn group_from(reason: &str, items: &[DupItem], bucket: &[usize]) -> DupGroup {
    let copies = bucket
        .iter()
        .map(|index| {
            let item = &items[*index];
            DupCopy {
                path: item.path.clone(),
                format: item.format.clone(),
                bitrate: bitrate(item),
                size: item.size,
                lyrics: item.lyrics,
                cover: item.cover,
            }
        })
        .collect::<Vec<_>>();
    let worse = copies
        .iter()
        .min_by_key(|copy| (quality(copy), copy.size, copy.path.clone()))
        .map(|copy| copy.path.clone())
        .unwrap_or_default();
    DupGroup {
        reason: reason.into(),
        paths: copies.iter().map(|copy| copy.path.clone()).collect(),
        copies,
        worse,
    }
}

fn bitrate(item: &DupItem) -> u32 {
    match item.duration.filter(|value| *value > 0) {
        Some(duration) => ((item.size.saturating_mul(8)) / duration as u64).min(u32::MAX as u64) as u32,
        None => 0,
    }
}

fn quality(copy: &DupCopy) -> i64 {
    let format = match copy.format.as_str() {
        "flac" | "wav" | "aiff" => 30,
        "mp3" | "m4a" | "mp4" | "aac" => 20,
        _ => 0,
    };
    copy.bitrate as i64 / 1000 + format + if copy.lyrics { 80 } else { 0 } + if copy.cover { 40 } else { 0 }
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
            format: "mp3".into(),
            fingerprint: Vec::new(),
            lyrics: false,
            cover: false,
        }
    }

    #[test]
    fn a_shared_fingerprint_groups_different_names_and_marks_the_worse_copy() {
        let mut kept = item("good.mp3", "I'll Keep Coming", "Low Roar", 180, "");
        kept.fingerprint = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        kept.size = 4_000_000;
        kept.lyrics = true;
        kept.cover = true;
        let mut worse = item("rip.opus", "keep coming live rip", "uploader", 181, "");
        worse.fingerprint = kept.fingerprint.clone();
        worse.format = "opus".into();
        worse.size = 400_000;
        let groups = duplicate_groups(&[kept, worse]);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].reason, "fingerprint");
        assert_eq!(groups[0].worse, "rip.opus");
        assert!(groups[0].copies.iter().any(|copy| copy.lyrics && copy.cover));
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
