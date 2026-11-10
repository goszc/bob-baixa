use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::media::is_playable_media;

/// One lyric line. `at` is seconds from the start. Unsynced lines use `0`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub at: f64,
    pub text: String,
}

/// Lyrics ready for the player. `source` is `embedded`, `file`, `lrcmux`, `karalyr`, `lrclib`, `ovh`, or `none`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedLyrics {
    pub source: String,
    pub synced: bool,
    pub instrumental: bool,
    pub lines: Vec<LyricLine>,
    pub plain: String,
    /// Queries the player tried when nothing was found.
    #[serde(default)]
    pub tried: Vec<String>,
}

impl ResolvedLyrics {
    pub fn empty() -> Self {
        Self {
            source: "none".into(),
            synced: false,
            instrumental: false,
            lines: Vec::new(),
            plain: String::new(),
            tried: Vec::new(),
        }
    }

    pub fn has_lines(&self) -> bool {
        !self.lines.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedLrc {
    pub lines: Vec<LyricLine>,
    pub has_timestamps: bool,
}

/// What each source had before the order is applied.
#[derive(Debug, Clone, Copy, Default)]
pub struct LyricInputs<'a> {
    pub embedded: Option<&'a str>,
    pub sibling: Option<&'a str>,
    pub remote_synced: Option<&'a str>,
    pub remote_plain: Option<&'a str>,
    pub instrumental: bool,
    /// Remote provider name. Empty means `lrclib`, which keeps older callers stable.
    pub remote_source: &'a str,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EmbeddedMedia {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration: Option<f64>,
    pub lyrics: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LrclibQuery {
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration: Option<u32>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LrclibTrack {
    pub track_name: String,
    pub artist_name: String,
    pub album_name: String,
    pub duration: Option<u32>,
    pub instrumental: bool,
    pub plain: Option<String>,
    pub synced: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCacheEntry {
    pub fetched_at: i64,
    pub found: bool,
    pub instrumental: bool,
    pub synced: Option<String>,
    pub plain: Option<String>,
    /// Which remote provider produced this entry. Empty cache files stay compatible.
    #[serde(default)]
    pub source: String,
}

const HIT_TTL: i64 = 30 * 24 * 60 * 60;
const MISS_TTL: i64 = 24 * 60 * 60;

/// Embedded lyrics, then a sibling `.lrc`, then one remote payload.
/// The player tries lrcmux, Karalyr, LRCLIB and lyrics.ovh before filling that payload.
pub fn resolve_lyrics(input: LyricInputs<'_>) -> ResolvedLyrics {
    if let Some(text) = blank_none(input.embedded) {
        let resolved = from_text(text, "embedded");
        if resolved.has_lines() {
            return resolved;
        }
    }
    if let Some(text) = blank_none(input.sibling) {
        let resolved = from_text(text, "file");
        if resolved.has_lines() {
            return resolved;
        }
    }
    let remote_synced = blank_none(input.remote_synced);
    let remote_plain = blank_none(input.remote_plain);
    let source = remote_label(input.remote_source);
    if input.instrumental && remote_synced.is_none() && remote_plain.is_none() {
        return ResolvedLyrics {
            source: source.into(),
            synced: false,
            instrumental: true,
            lines: Vec::new(),
            plain: String::new(),
            tried: Vec::new(),
        };
    }
    if let Some(text) = remote_synced {
        let resolved = from_text(text, source);
        if resolved.has_lines() {
            return resolved;
        }
    }
    if let Some(text) = remote_plain {
        let resolved = from_text(text, source);
        if resolved.has_lines() {
            return resolved;
        }
    }
    ResolvedLyrics::empty()
}

fn remote_label(source: &str) -> &str {
    let source = source.trim();
    if source.is_empty() {
        "lrclib"
    } else {
        source
    }
}

pub fn lyrics_from_cache(entry: &LyricsCacheEntry) -> ResolvedLyrics {
    if !entry.found {
        return ResolvedLyrics::empty();
    }
    resolve_lyrics(LyricInputs {
        embedded: None,
        sibling: None,
        remote_synced: entry.synced.as_deref(),
        remote_plain: entry.plain.as_deref(),
        instrumental: entry.instrumental,
        remote_source: entry.source.as_str(),
    })
}

/// Index of the line that should be highlighted, if playback has reached one.
pub fn active_line(lines: &[LyricLine], position: f64) -> Option<usize> {
    if lines.is_empty() || !position.is_finite() || position < 0.0 {
        return None;
    }
    let mut found = None;
    for (index, line) in lines.iter().enumerate() {
        if line.at <= position + 0.05 {
            found = Some(index);
        } else {
            break;
        }
    }
    found
}

pub fn parse_lrc(input: &str) -> ParsedLrc {
    let input = input.trim_start_matches('\u{feff}');
    let mut offset_ms: i64 = 0;
    let mut stamped = Vec::new();
    for raw in input.lines() {
        let line = raw.trim().trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some(ms) = parse_offset(line) {
            offset_ms = ms;
            continue;
        }
        let (times, text) = split_timestamps(line);
        if times.is_empty() {
            continue;
        }
        let text = collapse_spaces(&strip_word_times(&text));
        if text.is_empty() {
            continue;
        }
        for time in times {
            stamped.push(LyricLine {
                at: time,
                text: text.clone(),
            });
        }
    }
    if stamped.is_empty() {
        return ParsedLrc {
            lines: Vec::new(),
            has_timestamps: false,
        };
    }
    let offset = offset_ms as f64 / 1000.0;
    for line in &mut stamped {
        line.at = (line.at + offset).max(0.0);
    }
    stamped.sort_by(|left, right| {
        left.at
            .partial_cmp(&right.at)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    ParsedLrc {
        lines: stamped,
        has_timestamps: true,
    }
}

/// `song.mp3` -> `song.lrc`, then `song.mp3.lrc`.
pub fn sibling_lrc_paths(media: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let replaced = media.with_extension("lrc");
    paths.push(replaced);
    if let Some(name) = media.file_name() {
        let mut extra = std::ffi::OsString::from(name);
        extra.push(".lrc");
        let candidate = match media.parent() {
            Some(parent) => parent.join(extra),
            None => PathBuf::from(extra),
        };
        if paths.iter().all(|path| path != &candidate) {
            paths.push(candidate);
        }
    }
    paths
}

pub fn read_sibling_lrc(media: &Path) -> Option<String> {
    for path in sibling_lrc_paths(media) {
        if let Some(text) = read_text_limited(&path) {
            return Some(text);
        }
        let upper = path.with_extension("LRC");
        if upper != path {
            if let Some(text) = read_text_limited(&upper) {
                return Some(text);
            }
        }
    }
    None
}

pub fn read_embedded(path: &Path) -> EmbeddedMedia {
    let mut media = EmbeddedMedia::default();
    if !is_playable_media(path) {
        return media;
    }
    let Ok(tagged) = lofty::read_from_path(path) else {
        return media;
    };
    use lofty::prelude::{AudioFile, TaggedFileExt};
    let duration = tagged.properties().duration();
    if !duration.is_zero() {
        media.duration = Some(duration.as_secs_f64());
    }
    if let Some(tag) = tagged.primary_tag() {
        consider_tag(&mut media, tag);
    }
    for tag in tagged.tags() {
        consider_tag(&mut media, tag);
    }
    media
}

fn consider_tag(media: &mut EmbeddedMedia, tag: &lofty::tag::Tag) {
    use lofty::prelude::Accessor;
    use lofty::tag::ItemKey;
    if media.title.is_none() {
        media.title = owned_text(tag.title());
    }
    if media.artist.is_none() {
        media.artist = owned_text(tag.artist());
    }
    if media.album.is_none() {
        media.album = owned_text(tag.album());
    }
    if media.lyrics.is_none() {
        let mut parts = Vec::new();
        for text in tag.get_strings(&ItemKey::Lyrics) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                parts.push(trimmed.to_string());
            }
        }
        if !parts.is_empty() {
            media.lyrics = Some(parts.join("\n"));
        }
    }
}

fn owned_text(value: Option<impl AsRef<str>>) -> Option<String> {
    value
        .map(|text| text.as_ref().trim().to_string())
        .filter(|text| !text.is_empty())
}

/// Split `Artist - Title`. The title is left whole when there is no separator.
pub fn split_artist_title(raw: &str) -> (String, String) {
    let raw = raw.trim();
    for sep in [" - ", " – ", " — ", " | "] {
        if let Some((artist, title)) = raw.split_once(sep) {
            let artist = artist.trim();
            let title = title.trim();
            if !artist.is_empty() && !title.is_empty() {
                return (artist.to_string(), title.to_string());
            }
        }
    }
    (String::new(), raw.to_string())
}

/// Drop YouTube decorations: official video, lyrics, topic, feat. credits, `[Video]`.
pub fn clean_track_query(title: &str) -> String {
    let mut title = collapse_spaces(title.trim());
    if title.is_empty() {
        return title;
    }
    title = strip_media_extension(&title);
    title = strip_bracket_ids(&title);
    title = strip_feat_clauses(&title);
    title = strip_pipe_noise(&title);
    title = strip_wrapped_noise(&title);
    title = strip_bare_suffix(&title);
    title = strip_wrapped_noise(&title);
    title = strip_bracket_ids(&title);
    collapse_spaces(title.trim())
}

fn strip_media_extension(title: &str) -> String {
    let lower = title.to_ascii_lowercase();
    const EXTS: &[&str] = &[".mp4", ".m4v", ".mkv", ".webm", ".mov", ".avi", ".mp3", ".m4a", ".flac", ".opus", ".ogg", ".wav"];
    for ext in EXTS {
        if lower.ends_with(ext) && title.len() > ext.len() {
            return title[..title.len() - ext.len()].trim().to_string();
        }
    }
    title.to_string()
}

fn strip_bracket_ids(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '[' {
            if let Some(rel) = chars[index + 1..].iter().position(|ch| *ch == ']') {
                let inner: String = chars[index + 1..index + 1 + rel].iter().collect();
                if is_youtube_id(inner.trim()) {
                    index += rel + 2;
                    continue;
                }
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    collapse_spaces(out.trim())
}

/// Channel names like `Banda - Topic` or `Banda VEVO` become `Banda`.
pub fn clean_artist_query(artist: &str) -> String {
    let mut artist = artist.trim().to_string();
    let lower = artist.to_lowercase();
    if let Some(rest) = lower.strip_suffix(" - topic") {
        if !rest.trim().is_empty() {
            let end = artist.len() - " - topic".len();
            artist = artist[..end].trim().to_string();
        }
    }
    let lower = artist.to_lowercase();
    if lower.ends_with("vevo") && artist.len() > 4 {
        let end = artist.len() - 4;
        let rest = artist[..end].trim().trim_end_matches(['-', ' ']);
        if !rest.is_empty() {
            artist = rest.to_string();
        }
    }
    clean_track_query(&artist)
}

pub fn lyrics_query(
    embedded: &EmbeddedMedia,
    hint_title: &str,
    hint_artist: &str,
    hint_duration: Option<f64>,
) -> LrclibQuery {
    let raw_title = embedded
        .title
        .clone()
        .filter(|title| !title.trim().is_empty())
        .unwrap_or_else(|| hint_title.trim().to_string());
    let (split_artist, split_title) = split_artist_title(&raw_title);
    let tagged_artist = embedded
        .artist
        .clone()
        .filter(|artist| !artist.trim().is_empty());
    let (title_base, fallback_artist) = if tagged_artist.is_some() {
        (raw_title, String::new())
    } else if !split_artist.is_empty() {
        (split_title, split_artist)
    } else {
        (raw_title, String::new())
    };
    let artist = tagged_artist
        .or_else(|| {
            let hint = hint_artist.trim();
            if hint.is_empty() {
                None
            } else {
                Some(hint.to_string())
            }
        })
        .unwrap_or(fallback_artist);
    let artist = clean_artist_query(&artist);
    let duration = embedded
        .duration
        .or(hint_duration)
        .filter(|value| value.is_finite() && *value > 0.0)
        .map(|value| value.round() as u32)
        .filter(|value| *value > 0);
    LrclibQuery {
        track_name: clean_track_query(&title_base),
        artist_name: artist,
        album_name: embedded.album.clone().unwrap_or_default(),
        duration,
    }
}

pub fn lrclib_get_url(query: &LrclibQuery) -> String {
    let mut url = format!(
        "https://lrclib.net/api/get?track_name={}&artist_name={}",
        percent_encode(&query.track_name),
        percent_encode(&query.artist_name)
    );
    if !query.album_name.trim().is_empty() {
        url.push_str("&album_name=");
        url.push_str(&percent_encode(&query.album_name));
    }
    if let Some(duration) = query.duration {
        url.push_str(&format!("&duration={duration}"));
    }
    url
}

pub fn karalyr_get_url(query: &LrclibQuery) -> String {
    hosted_get_url("https://karalyr.com", query)
}

pub fn karalyr_search_url(query: &LrclibQuery) -> String {
    hosted_search_url("https://karalyr.com", query)
}

pub fn lrcmux_get_url(query: &LrclibQuery) -> String {
    format!(
        "https://api.lrcmux.dev/get?artist={}&title={}&format=json",
        percent_encode(query.artist_name.trim()),
        percent_encode(query.track_name.trim())
    )
}

pub fn lyrics_ovh_url(artist: &str, title: &str) -> String {
    format!(
        "https://api.lyrics.ovh/v1/{}/{}",
        percent_encode(artist.trim()),
        percent_encode(title.trim())
    )
}

/// A remote hit after a provider responded. `source` is `lrcmux`, `karalyr`, `lrclib`, or `ovh`.
#[derive(Debug, Clone, PartialEq)]
pub struct RemoteHit {
    pub source: String,
    pub synced: Option<String>,
    pub plain: Option<String>,
    pub instrumental: bool,
}

pub fn hit_from_lrclib(track: &LrclibTrack, source: &str) -> RemoteHit {
    RemoteHit {
        source: source.to_string(),
        synced: track
            .synced
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        plain: track
            .plain
            .as_ref()
            .map(|text| text.trim().to_string())
            .filter(|text| !text.is_empty()),
        instrumental: track.instrumental,
    }
}

pub fn remote_hit_is_useful(hit: &RemoteHit) -> bool {
    hit.instrumental
        || hit
            .synced
            .as_ref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false)
        || hit
            .plain
            .as_ref()
            .map(|text| !text.trim().is_empty())
            .unwrap_or(false)
}

/// lrcmux `format=json`. Line `start` is milliseconds. Word timings are collapsed to one line.
pub fn parse_lrcmux_payload(body: &str) -> Option<RemoteHit> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let meta = value.get("meta")?.as_object()?;
    if meta
        .get("instrumental")
        .and_then(|item| item.as_bool())
        .unwrap_or(false)
    {
        return Some(RemoteHit {
            source: "lrcmux".into(),
            synced: None,
            plain: None,
            instrumental: true,
        });
    }
    let lines = value.get("lines")?.as_array()?;
    let level = meta
        .get("level")
        .and_then(|item| item.as_str())
        .unwrap_or("");
    let mut synced = String::new();
    let mut plain = String::new();
    let mut any_time = false;
    for line in lines {
        let text = line
            .get("text")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim();
        if text.is_empty() {
            continue;
        }
        if !plain.is_empty() {
            plain.push('\n');
        }
        plain.push_str(text);
        if let Some(start) = line.get("start").and_then(|item| item.as_f64()) {
            any_time = true;
            if matches!(level, "line" | "word") || level.is_empty() {
                synced.push_str(&lrc_stamp(start / 1000.0));
                synced.push_str(text);
                synced.push('\n');
            }
        }
    }
    if plain.trim().is_empty() {
        return None;
    }
    let synced = if any_time && !synced.trim().is_empty() {
        Some(synced)
    } else {
        None
    };
    Some(RemoteHit {
        source: "lrcmux".into(),
        synced,
        plain: Some(plain),
        instrumental: false,
    })
}

pub fn parse_lyrics_ovh(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let lyrics = value.get("lyrics")?.as_str()?.trim();
    if lyrics.is_empty() {
        None
    } else {
        Some(lyrics.to_string())
    }
}

fn hosted_get_url(origin: &str, query: &LrclibQuery) -> String {
    let mut url = format!(
        "{origin}/api/get?track_name={}&artist_name={}",
        percent_encode(&query.track_name),
        percent_encode(&query.artist_name)
    );
    if !query.album_name.trim().is_empty() {
        url.push_str("&album_name=");
        url.push_str(&percent_encode(&query.album_name));
    }
    if let Some(duration) = query.duration {
        url.push_str(&format!("&duration={duration}"));
    }
    url
}

fn hosted_search_url(origin: &str, query: &LrclibQuery) -> String {
    let mut url = format!(
        "{origin}/api/search?track_name={}",
        percent_encode(&query.track_name)
    );
    if !query.artist_name.trim().is_empty() {
        url.push_str("&artist_name=");
        url.push_str(&percent_encode(&query.artist_name));
    }
    if !query.album_name.trim().is_empty() {
        url.push_str("&album_name=");
        url.push_str(&percent_encode(&query.album_name));
    }
    url
}

fn lrc_stamp(seconds: f64) -> String {
    let total = if seconds.is_finite() && seconds > 0.0 {
        seconds
    } else {
        0.0
    };
    let mins = (total / 60.0).floor() as u32;
    let secs = total - f64::from(mins) * 60.0;
    format!("[{mins:02}:{secs:05.2}]")
}

pub fn lrclib_search_url(query: &LrclibQuery) -> String {
    let mut url = format!(
        "https://lrclib.net/api/search?track_name={}",
        percent_encode(&query.track_name)
    );
    if !query.artist_name.trim().is_empty() {
        url.push_str("&artist_name=");
        url.push_str(&percent_encode(&query.artist_name));
    }
    if !query.album_name.trim().is_empty() {
        url.push_str("&album_name=");
        url.push_str(&percent_encode(&query.album_name));
    }
    url
}

pub fn parse_lrclib_payload(body: &str) -> Vec<LrclibTrack> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    match value {
        serde_json::Value::Array(items) => items.iter().filter_map(parse_lrclib_track).collect(),
        serde_json::Value::Object(_) => parse_lrclib_track(&value).into_iter().collect(),
        _ => Vec::new(),
    }
}

pub fn pick_lrclib(tracks: &[LrclibTrack], duration: Option<u32>) -> Option<&LrclibTrack> {
    if tracks.is_empty() {
        return None;
    }
    if let Some(duration) = duration {
        let mut best: Option<&LrclibTrack> = None;
        let mut best_delta = u32::MAX;
        for track in tracks {
            let Some(candidate) = track.duration else {
                continue;
            };
            let delta = candidate.abs_diff(duration);
            let better = match best {
                None => true,
                Some(current) => {
                    delta < best_delta
                        || (delta == best_delta && has_synced(track) && !has_synced(current))
                }
            };
            if better {
                best = Some(track);
                best_delta = delta;
            }
        }
        if let Some(track) = best {
            return Some(track);
        }
    }
    tracks
        .iter()
        .find(|track| has_synced(track))
        .or(tracks.first())
}

pub fn lyrics_cache_key(query: &LrclibQuery) -> String {
    format!(
        "{}\n{}\n{}\n{}",
        normalize_key(&query.artist_name),
        normalize_key(&query.track_name),
        normalize_key(&query.album_name),
        query.duration.unwrap_or(0)
    )
}

pub fn cache_file_name(key: &str) -> String {
    format!("{:016x}.json", fnv64(key.as_bytes()))
}

pub fn cache_is_fresh(entry: &LyricsCacheEntry, now: i64) -> bool {
    let age = now.saturating_sub(entry.fetched_at);
    if age < 0 {
        return true;
    }
    let ttl = if entry.found { HIT_TTL } else { MISS_TTL };
    age < ttl
}

pub fn read_lyrics_cache(dir: &Path, key: &str, now: i64) -> Option<LyricsCacheEntry> {
    let text = std::fs::read_to_string(dir.join(cache_file_name(key))).ok()?;
    let entry = serde_json::from_str::<LyricsCacheEntry>(&text).ok()?;
    cache_is_fresh(&entry, now).then_some(entry)
}

pub fn write_lyrics_cache(dir: &Path, key: &str, entry: &LyricsCacheEntry) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let text = serde_json::to_string(entry)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    std::fs::write(dir.join(cache_file_name(key)), text)
}

fn from_text(text: &str, source: &str) -> ResolvedLyrics {
    let parsed = parse_lrc(text);
    if parsed.has_timestamps {
        let plain = parsed
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        return ResolvedLyrics {
            source: source.into(),
            synced: !parsed.lines.is_empty(),
            instrumental: false,
            lines: parsed.lines,
            plain,
            tried: Vec::new(),
        };
    }
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('['))
        .map(|line| LyricLine {
            at: 0.0,
            text: line.to_string(),
        })
        .collect::<Vec<_>>();
    let plain = lines
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    ResolvedLyrics {
        source: source.into(),
        synced: false,
        instrumental: false,
        lines,
        plain,
        tried: Vec::new(),
    }
}

fn blank_none(value: Option<&str>) -> Option<&str> {
    value.and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(text)
        }
    })
}

fn parse_offset(line: &str) -> Option<i64> {
    let lower = line.to_ascii_lowercase();
    let rest = lower.strip_prefix("[offset:")?.strip_suffix(']')?;
    rest.trim().parse().ok()
}

fn split_timestamps(line: &str) -> (Vec<f64>, String) {
    let mut times = Vec::new();
    let mut rest = line;
    while let Some((seconds, consumed)) = parse_stamp(rest) {
        times.push(seconds);
        rest = &rest[consumed..];
    }
    (times, rest.to_string())
}

fn parse_stamp(value: &str) -> Option<(f64, usize)> {
    if !value.starts_with('[') {
        return None;
    }
    let end = value.find(']')?;
    let inner = &value[1..end];
    if inner.is_empty() || inner.chars().any(|ch| ch.is_alphabetic()) {
        return None;
    }
    let parts: Vec<&str> = inner.split(':').collect();
    let total = if parts.len() == 2 {
        let minutes: f64 = parts[0].parse().ok()?;
        let (seconds, fraction) = split_seconds(parts[1])?;
        minutes * 60.0 + seconds + fraction
    } else if parts.len() == 3 {
        let hours: f64 = parts[0].parse().ok()?;
        let minutes: f64 = parts[1].parse().ok()?;
        let (seconds, fraction) = split_seconds(parts[2])?;
        hours * 3600.0 + minutes * 60.0 + seconds + fraction
    } else {
        return None;
    };
    Some((total, end + 1))
}

fn split_seconds(value: &str) -> Option<(f64, f64)> {
    if value.is_empty() {
        return None;
    }
    let (whole, frac) = value.split_once(['.', ':']).unwrap_or((value, ""));
    if whole.is_empty() || !whole.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    let seconds: f64 = whole.parse().ok()?;
    if frac.is_empty() {
        return Some((seconds, 0.0));
    }
    if !frac.chars().all(|ch| ch.is_ascii_digit()) || frac.len() > 3 {
        return None;
    }
    let number: f64 = frac.parse().ok()?;
    let scale = 10f64.powi(frac.len() as i32);
    Some((seconds, number / scale))
}

fn strip_word_times(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '<' {
            if let Some(end) = chars[index + 1..].iter().position(|ch| *ch == '>') {
                let inner: String = chars[index + 1..index + 1 + end].iter().collect();
                if looks_like_time(&inner) {
                    index += end + 2;
                    continue;
                }
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    out
}

fn looks_like_time(value: &str) -> bool {
    parse_stamp(&format!("[{value}]")).is_some()
}

fn collapse_spaces(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_feat_clauses(input: &str) -> String {
    let lower = input.to_lowercase();
    const MARKERS: &[&str] = &[
        " (feat.",
        " (feat ",
        " (ft.",
        " (ft ",
        " (featuring ",
        " [feat.",
        " [feat ",
        " [ft.",
        " [ft ",
        " [featuring ",
        " feat.",
        " feat ",
        " featuring ",
        " ft. ",
        " ft ",
    ];
    let mut cut = None;
    for marker in MARKERS {
        if let Some(index) = lower.find(marker) {
            cut = Some(cut.map_or(index, |current: usize| current.min(index)));
        }
    }
    match cut {
        Some(index) if index > 0 => input[..index].trim().to_string(),
        _ => input.to_string(),
    }
}

fn strip_pipe_noise(input: &str) -> String {
    if !input.contains('|') {
        return input.to_string();
    }
    let kept: Vec<&str> = input
        .split('|')
        .map(str::trim)
        .filter(|part| !part.is_empty() && !segment_is_noise(part))
        .collect();
    if kept.is_empty() {
        input.to_string()
    } else {
        kept.join(" ")
    }
}

fn segment_is_noise(part: &str) -> bool {
    let trimmed = part.trim_matches(|ch: char| matches!(ch, '(' | ')' | '[' | ']'));
    is_title_noise(&trimmed.to_lowercase())
}

fn strip_wrapped_noise(title: &str) -> String {
    let mut title = title.to_string();
    loop {
        let trimmed = title.trim();
        let Some(open) = trimmed.rfind(['(', '[']) else {
            break;
        };
        let closer = if trimmed[open..].starts_with('(') {
            ')'
        } else {
            ']'
        };
        if !trimmed.ends_with(closer) {
            break;
        }
        let inner = trimmed[open + 1..trimmed.len() - 1].trim().to_lowercase();
        if !is_title_noise(&inner) && !is_youtube_id(&inner) {
            break;
        }
        title = trimmed[..open].trim().to_string();
        if title.is_empty() {
            break;
        }
    }
    title
}

fn strip_bare_suffix(title: &str) -> String {
    const PHRASES: &[&str] = &[
        "official music video",
        "official lyric video",
        "official audio",
        "official video",
        "video oficial",
        "áudio oficial",
        "audio oficial",
        "clipe oficial",
        "official lyrics",
        "with lyrics",
        "lyric video",
        "lyrics video",
        "lyrics",
    ];
    let mut title = title.to_string();
    loop {
        let lower = title.to_lowercase();
        let mut next = None;
        for phrase in PHRASES {
            if lower.ends_with(phrase) && title.len() > phrase.len() {
                let end = title.len() - phrase.len();
                let head = title[..end].trim_end_matches(|ch: char| {
                    ch.is_whitespace() || matches!(ch, '-' | '|' | '–' | '—' | ':')
                });
                if !head.is_empty() && head.len() < title.len() {
                    next = Some(head.to_string());
                    break;
                }
            }
        }
        match next {
            Some(value) => title = value,
            None => break,
        }
    }
    title
}

fn is_youtube_id(value: &str) -> bool {
    value.len() == 11
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

fn is_title_noise(inner: &str) -> bool {
    const WORDS: &[&str] = &[
        "official",
        "video",
        "audio",
        "áudio",
        "lyrics",
        "lyric",
        "letra",
        "visualizer",
        "clipe",
        "oficial",
        "hd",
        "hq",
        "4k",
        "remaster",
        "remastered",
        "topic",
        "music",
        "legendado",
    ];
    let words: Vec<&str> = inner
        .split(|ch: char| !ch.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .collect();
    !words.is_empty()
        && words
            .iter()
            .all(|word| WORDS.contains(word) || word.chars().all(|ch| ch.is_ascii_digit()))
}

fn has_synced(track: &LrclibTrack) -> bool {
    track
        .synced
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false)
}

fn parse_lrclib_track(value: &serde_json::Value) -> Option<LrclibTrack> {
    let obj = value.as_object()?;
    let has_body = obj.contains_key("plainLyrics")
        || obj.contains_key("syncedLyrics")
        || obj.contains_key("instrumental")
        || obj.contains_key("trackName");
    if !has_body {
        return None;
    }
    let track_name = obj
        .get("trackName")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string();
    if track_name.is_empty() && !obj.contains_key("instrumental") {
        return None;
    }
    Some(LrclibTrack {
        track_name,
        artist_name: obj
            .get("artistName")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        album_name: obj
            .get("albumName")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim()
            .to_string(),
        duration: obj
            .get("duration")
            .and_then(|item| {
                item.as_u64()
                    .or_else(|| item.as_f64().map(|value| value.round() as u64))
            })
            .map(|value| value as u32),
        instrumental: obj
            .get("instrumental")
            .and_then(|item| item.as_bool())
            .unwrap_or(false),
        plain: json_text(obj.get("plainLyrics")),
        synced: json_text(obj.get("syncedLyrics")),
    })
}

fn json_text(value: Option<&serde_json::Value>) -> Option<String> {
    value
        .and_then(|item| item.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn normalize_key(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn fnv64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in bytes {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn percent_encode(value: &str) -> String {
    let mut out = String::new();
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn read_text_limited(path: &Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() || meta.len() > 1_000_000 {
        return None;
    }
    let text = std::fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bob-baixa-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn parses_lrc_timestamps_metadata_and_word_times() {
        let parsed = parse_lrc(
            "\u{feff}[ar:Banda]\n[ti:Faixa]\n[offset:+500]\n[00:01.50]primeira\n[00:03]segunda\n[01:02.005]terceira\n[00:10.00][00:20.00]<00:10.00>noite <00:10.40>acesa\n",
        );
        assert!(parsed.has_timestamps);
        assert_eq!(parsed.lines.len(), 5);
        assert!((parsed.lines[0].at - 2.0).abs() < 0.001);
        assert_eq!(parsed.lines[0].text, "primeira");
        assert!((parsed.lines[1].at - 3.5).abs() < 0.001);
        assert!((parsed.lines[2].at - 10.5).abs() < 0.001);
        assert_eq!(parsed.lines[2].text, "noite acesa");
        assert!((parsed.lines[4].at - 62.505).abs() < 0.001);
    }

    #[test]
    fn plain_text_is_not_synced() {
        let parsed = parse_lrc("só um texto\noutro");
        assert!(!parsed.has_timestamps);
        assert!(parsed.lines.is_empty());
    }

    #[test]
    fn active_line_follows_playback() {
        let lines = vec![
            LyricLine {
                at: 1.0,
                text: "a".into(),
            },
            LyricLine {
                at: 2.0,
                text: "b".into(),
            },
            LyricLine {
                at: 4.0,
                text: "c".into(),
            },
        ];
        assert_eq!(active_line(&[], 1.0), None);
        assert_eq!(active_line(&lines, 0.2), None);
        assert_eq!(active_line(&lines, 1.0), Some(0));
        assert_eq!(active_line(&lines, 3.2), Some(1));
        assert_eq!(active_line(&lines, 9.0), Some(2));
    }

    #[test]
    fn resolution_order_is_embedded_then_sibling_then_lrclib() {
        let embedded = resolve_lyrics(LyricInputs {
            embedded: Some("letra gravada no arquivo"),
            sibling: Some("[00:01.00]do lrc"),
            remote_synced: Some("[00:01.00]da rede"),
            remote_plain: Some("plain"),
            instrumental: false,
            remote_source: "",
        });
        assert_eq!(embedded.source, "embedded");
        assert!(!embedded.synced);
        assert_eq!(embedded.lines[0].text, "letra gravada no arquivo");

        let sibling = resolve_lyrics(LyricInputs {
            embedded: Some("   "),
            sibling: Some("[00:02.00]arquivo ao lado"),
            remote_synced: Some("[00:01.00]da rede"),
            ..LyricInputs::default()
        });
        assert_eq!(sibling.source, "file");
        assert!(sibling.synced);
        assert_eq!(sibling.lines[0].text, "arquivo ao lado");

        let remote = resolve_lyrics(LyricInputs {
            remote_synced: Some("[00:04.00]sincronizada"),
            remote_plain: Some("sem tempo"),
            ..LyricInputs::default()
        });
        assert_eq!(remote.source, "lrclib");
        assert!(remote.synced);
        assert_eq!(remote.lines[0].text, "sincronizada");

        let plain = resolve_lyrics(LyricInputs {
            remote_plain: Some("só o texto"),
            ..LyricInputs::default()
        });
        assert_eq!(plain.source, "lrclib");
        assert!(!plain.synced);

        let instrumental = resolve_lyrics(LyricInputs {
            instrumental: true,
            ..LyricInputs::default()
        });
        assert!(instrumental.instrumental);
        assert!(instrumental.lines.is_empty());
        assert_eq!(instrumental.source, "lrclib");

        assert_eq!(
            resolve_lyrics(LyricInputs::default()),
            ResolvedLyrics::empty()
        );
    }

    #[test]
    fn embedded_lrc_stays_synced() {
        let resolved = resolve_lyrics(LyricInputs {
            embedded: Some("[00:01.00]dentro\n[00:02.00]do arquivo"),
            sibling: Some("[00:01.00]lado"),
            ..LyricInputs::default()
        });
        assert_eq!(resolved.source, "embedded");
        assert!(resolved.synced);
        assert_eq!(resolved.lines.len(), 2);
    }

    #[test]
    fn sibling_paths_and_read() {
        let dir = temp_dir("sibling");
        let media = dir.join("faixa.mp3");
        std::fs::write(&media, b"nao-audio").unwrap();
        let paths = sibling_lrc_paths(&media);
        assert_eq!(paths[0], dir.join("faixa.lrc"));
        assert_eq!(paths[1], dir.join("faixa.mp3.lrc"));
        assert!(read_sibling_lrc(&media).is_none());
        std::fs::write(dir.join("faixa.mp3.lrc"), "[00:01.00]ao lado\n").unwrap();
        assert!(read_sibling_lrc(&media).unwrap().contains("ao lado"));
        std::fs::write(dir.join("faixa.lrc"), "[00:01.00]preferido\n").unwrap();
        assert!(read_sibling_lrc(&media).unwrap().contains("preferido"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn uppercase_lrc_extension_is_read() {
        let dir = temp_dir("upper");
        let media = dir.join("faixa.mp3");
        std::fs::write(&media, b"x").unwrap();
        std::fs::write(dir.join("faixa.LRC"), "[00:01.00]maiusculo\n").unwrap();
        assert!(read_sibling_lrc(&media).unwrap().contains("maiusculo"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_and_garbage_files_do_not_panic() {
        let dir = temp_dir("missing");
        let missing = dir.join("nao-existe.mp3");
        assert!(read_embedded(&missing).lyrics.is_none());
        assert!(read_sibling_lrc(&missing).is_none());
        let text = dir.join("nota.txt");
        std::fs::write(&text, b"oi").unwrap();
        assert!(read_embedded(&text).lyrics.is_none());
        let fake = dir.join("falso.mp3");
        std::fs::write(&fake, b"isto nao e um mp3").unwrap();
        assert!(read_embedded(&fake).lyrics.is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reads_lyrics_embedded_in_an_mp3() {
        let dir = temp_dir("embedded");
        let path = dir.join("faixa.mp3");
        let mut frame = vec![0u8; 209];
        frame[0] = 0xFF;
        frame[1] = 0xFB;
        frame[2] = 0x52;
        frame[3] = 0xC4;
        let mut file = frame.clone();
        file.extend_from_slice(&frame);
        std::fs::write(&path, &file).unwrap();
        use lofty::config::WriteOptions;
        use lofty::prelude::*;
        use lofty::tag::{Tag, TagType};
        let mut tag = Tag::new(TagType::Id3v2);
        tag.set_title("Canção".to_string());
        tag.set_artist("Banda".to_string());
        tag.set_album("Disco".to_string());
        assert!(tag.insert_text(
            ItemKey::Lyrics,
            "[00:01.50]linha um\n[00:03.00]linha dois".to_string()
        ));
        tag.save_to_path(&path, WriteOptions::new()).unwrap();
        let read = read_embedded(&path);
        assert_eq!(read.title.as_deref(), Some("Canção"));
        assert_eq!(read.artist.as_deref(), Some("Banda"));
        assert_eq!(read.album.as_deref(), Some("Disco"));
        let lyrics = read.lyrics.expect("lyrics");
        assert!(lyrics.contains("linha um"));
        assert!(lyrics.contains("[00:01.50]"));
        let resolved = resolve_lyrics(LyricInputs {
            embedded: Some(&lyrics),
            sibling: Some("[00:01.00]nao usar"),
            remote_synced: Some("[00:01.00]nao usar"),
            ..LyricInputs::default()
        });
        assert_eq!(resolved.source, "embedded");
        assert!(resolved.synced);
        assert_eq!(resolved.lines[0].text, "linha um");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn splits_artist_and_cleans_youtube_suffixes() {
        assert_eq!(
            split_artist_title("Banda - Canção"),
            ("Banda".into(), "Canção".into())
        );
        assert_eq!(
            split_artist_title("Banda – Canção"),
            ("Banda".into(), "Canção".into())
        );
        assert_eq!(
            split_artist_title("O que fazer em Lisboa"),
            (String::new(), "O que fazer em Lisboa".into())
        );
        assert_eq!(clean_track_query("Canção (Official Video)"), "Canção");
        assert_eq!(clean_track_query("Canção (Official Audio) [4K]"), "Canção");
        assert_eq!(clean_track_query("Canção (Live)"), "Canção (Live)");
        assert_eq!(
            clean_track_query("Canção (feat. Alguém) [Video]"),
            "Canção"
        );
        assert_eq!(
            clean_track_query("Canção feat. Alguém | Official Video"),
            "Canção"
        );
        assert_eq!(clean_track_query("Canção lyrics"), "Canção");
        assert_eq!(clean_artist_query("Banda - Topic"), "Banda");
        assert_eq!(clean_artist_query("Banda VEVO"), "Banda");
        let raw = "Disappeared Completely - No Place for Me (Pt.1) - Official Video [x0NpAqKoHv4].mp4";
        let (artist, title) = split_artist_title(raw);
        assert_eq!(artist, "Disappeared Completely");
        assert_eq!(clean_track_query(&title), "No Place for Me (Pt.1)");
        let query = LrclibQuery {
            track_name: clean_track_query(&title),
            artist_name: artist,
            album_name: String::new(),
            duration: Some(210),
        };
        let variants = crate::lyrics_variants(&query);
        assert!(variants.iter().any(|item| item.track_name == "No Place for Me (Pt.1)"));
        assert!(variants.iter().any(|item| item.track_name == "No Place for Me"));
        let tried = crate::query_attempts(&variants);
        assert!(tried.iter().any(|item| item.contains("No Place for Me") && !item.contains("Pt.1")));
    }

    #[test]
    fn remote_providers_parse_without_scraping() {
        let query = LrclibQuery {
            track_name: "Canção".into(),
            artist_name: "Banda".into(),
            album_name: String::new(),
            duration: Some(180),
        };
        assert!(lrcmux_get_url(&query).starts_with("https://api.lrcmux.dev/get?"));
        assert!(karalyr_get_url(&query).starts_with("https://karalyr.com/api/get?"));
        assert_eq!(
            lyrics_ovh_url("Banda", "Canção"),
            "https://api.lyrics.ovh/v1/Banda/Can%C3%A7%C3%A3o"
        );
        let mux = parse_lrcmux_payload(
            r#"{"meta":{"level":"line","instrumental":false},"lines":[{"text":"oi","start":1500}]}"#,
        )
        .unwrap();
        assert_eq!(mux.source, "lrcmux");
        assert_eq!(mux.synced.as_deref(), Some("[00:01.50]oi\n"));
        assert!(parse_lrcmux_payload(r#"{"meta":{"instrumental":true},"lines":[]}"#)
            .unwrap()
            .instrumental);
        assert_eq!(
            parse_lyrics_ovh(r#"{"lyrics":"linha um\nlinha dois"}"#).as_deref(),
            Some("linha um\nlinha dois")
        );
        assert!(parse_lyrics_ovh(r#"{"error":"No lyrics found"}"#).is_none());
        let shaped = hit_from_lrclib(
            &parse_lrclib_payload(
                r#"{"trackName":"Canção","artistName":"Banda","syncedLyrics":"[00:01.00]oi","plainLyrics":"oi","instrumental":false}"#,
            )[0],
            "karalyr",
        );
        assert!(remote_hit_is_useful(&shaped));
        assert_eq!(shaped.source, "karalyr");
        let resolved = resolve_lyrics(LyricInputs {
            remote_synced: shaped.synced.as_deref(),
            remote_source: "karalyr",
            ..LyricInputs::default()
        });
        assert_eq!(resolved.source, "karalyr");
    }

    #[test]
    fn query_prefers_tags_over_the_filename() {
        let embedded = EmbeddedMedia {
            title: Some("Canção (Official Video)".into()),
            artist: Some("Banda".into()),
            album: Some("Disco".into()),
            duration: Some(125.4),
            lyrics: None,
        };
        let query = lyrics_query(&embedded, "arquivo - errado", "", None);
        assert_eq!(query.track_name, "Canção");
        assert_eq!(query.artist_name, "Banda");
        assert_eq!(query.album_name, "Disco");
        assert_eq!(query.duration, Some(125));

        let from_title = lyrics_query(
            &EmbeddedMedia::default(),
            "Banda - Canção (Lyric Video)",
            "",
            Some(10.2),
        );
        assert_eq!(from_title.artist_name, "Banda");
        assert_eq!(from_title.track_name, "Canção");
        assert_eq!(from_title.duration, Some(10));
    }

    #[test]
    fn lrclib_urls_are_encoded() {
        let query = LrclibQuery {
            track_name: "Canção azul".into(),
            artist_name: "Banda & Cia".into(),
            album_name: String::new(),
            duration: Some(180),
        };
        assert_eq!(
            lrclib_get_url(&query),
            "https://lrclib.net/api/get?track_name=Can%C3%A7%C3%A3o%20azul&artist_name=Banda%20%26%20Cia&duration=180"
        );
        assert_eq!(
            lrclib_search_url(&query),
            "https://lrclib.net/api/search?track_name=Can%C3%A7%C3%A3o%20azul&artist_name=Banda%20%26%20Cia"
        );
    }

    #[test]
    fn parses_lrclib_object_array_and_errors() {
        let one = parse_lrclib_payload(
            r#"{"id":1,"trackName":"Canção","artistName":"Banda","albumName":"Disco","duration":180,"instrumental":false,"plainLyrics":"oi","syncedLyrics":"[00:01.00]oi"}"#,
        );
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].track_name, "Canção");
        assert_eq!(one[0].synced.as_deref(), Some("[00:01.00]oi"));
        let many = parse_lrclib_payload(
            r#"[{"trackName":"A","artistName":"B","duration":10,"syncedLyrics":"[00:01.00]a"},{"trackName":"C","artistName":"D","duration":40,"plainLyrics":"c"}]"#,
        );
        assert_eq!(many.len(), 2);
        assert!(parse_lrclib_payload(r#"{"statusCode":404,"message":"Not Found"}"#).is_empty());
        assert!(parse_lrclib_payload("not json").is_empty());
    }

    #[test]
    fn picks_the_lrclib_hit_closest_to_the_duration() {
        let tracks = parse_lrclib_payload(
            r#"[{"trackName":"A","duration":100,"syncedLyrics":"[00:01.00]longe"},{"trackName":"B","duration":200,"plainLyrics":"perto"},{"trackName":"C","duration":200,"syncedLyrics":"[00:01.00]certo"}]"#,
        );
        let picked = pick_lrclib(&tracks, Some(198)).unwrap();
        assert_eq!(picked.track_name, "C");
        let any = pick_lrclib(&tracks, None).unwrap();
        assert_eq!(any.track_name, "A");
        assert!(pick_lrclib(&[], Some(1)).is_none());
    }

    #[test]
    fn cache_roundtrip_respects_ttl() {
        let dir = temp_dir("cache");
        let key = lyrics_cache_key(&LrclibQuery {
            track_name: "Canção".into(),
            artist_name: "Banda".into(),
            album_name: String::new(),
            duration: Some(180),
        });
        assert_eq!(cache_file_name(&key).len(), 21);
        let hit = LyricsCacheEntry {
            fetched_at: 1_000,
            found: true,
            instrumental: false,
            synced: Some("[00:01.00]oi".into()),
            plain: Some("oi".into()),
            source: String::new(),
        };
        write_lyrics_cache(&dir, &key, &hit).unwrap();
        assert_eq!(
            read_lyrics_cache(&dir, &key, 1_000 + 10).as_ref(),
            Some(&hit)
        );
        assert!(read_lyrics_cache(&dir, &key, 1_000 + HIT_TTL + 5).is_none());
        let miss = LyricsCacheEntry {
            fetched_at: 5_000,
            found: false,
            instrumental: false,
            synced: None,
            plain: None,
            source: String::new(),
        };
        write_lyrics_cache(&dir, &key, &miss).unwrap();
        assert_eq!(
            read_lyrics_cache(&dir, &key, 5_000 + 60).as_ref(),
            Some(&miss)
        );
        assert!(read_lyrics_cache(&dir, &key, 5_000 + MISS_TTL + 1).is_none());
        let resolved = lyrics_from_cache(&hit);
        assert_eq!(resolved.source, "lrclib");
        assert!(resolved.synced);
        assert_eq!(lyrics_from_cache(&miss), ResolvedLyrics::empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn same_song_uses_the_same_cache_key() {
        let left = lyrics_cache_key(&LrclibQuery {
            track_name: "Canção  Azul".into(),
            artist_name: "Banda".into(),
            album_name: String::new(),
            duration: Some(10),
        });
        let right = lyrics_cache_key(&LrclibQuery {
            track_name: "canção azul".into(),
            artist_name: " banda ".into(),
            album_name: String::new(),
            duration: Some(10),
        });
        assert_eq!(left, right);
    }
}
