use crate::{clean_artist_query, clean_track_query, lyric_duration_ok, parse_lrc, LrclibQuery, LrclibTrack};

pub const LYRIC_ACCEPT: i32 = 58;
pub const LYRIC_LIST: i32 = 36;

/// `live`, `remix`, `acoustic`, `sped`, `slowed`, or `other` when the title is a version.
pub fn version_kind(title: &str) -> Option<&'static str> {
    let lower = title.to_lowercase();
    if contains_marker(&lower, &["sped up", "sped-up", "nightcore", "chipmunk"]) {
        return Some("sped");
    }
    if contains_marker(&lower, &["slowed", "reverb", "bass boosted", "8d audio", "8d"]) {
        return Some("slowed");
    }
    if contains_marker(
        &lower,
        &["acoustic", "acústico", "acustico", "unplugged"],
    ) {
        return Some("acoustic");
    }
    if contains_marker(&lower, &["remix", "extended mix", "radio edit"]) {
        return Some("remix");
    }
    if contains_marker(&lower, &["ao vivo", "live at", "live from", "live session", "(live)", "[live]", "- live"])
    {
        return Some("live");
    }
    if contains_marker(&lower, &["karaoke", "instrumental", "cover"]) {
        return Some("other");
    }
    None
}

fn contains_marker(lower: &str, markers: &[&str]) -> bool {
    markers.iter().any(|marker| lower.contains(marker))
}

fn marker_text(text: &str) -> bool {
    version_kind(text).is_some()
}

/// Drop `(Live)`, `- Remix` and the other version tags so lyrics can fall back to the original.
pub fn base_recording_title(title: &str) -> String {
    let original = title.trim();
    let mut title = original.to_string();
    for _ in 0..6 {
        let trimmed = title.trim();
        let Some(open) = trimmed.rfind(['(', '[']) else {
            break;
        };
        let close = if trimmed[open..].starts_with('(') { ')' } else { ']' };
        if !trimmed.ends_with(close) {
            break;
        }
        let inner = trimmed[open + 1..trimmed.len() - 1].trim();
        if !marker_text(inner) && !marker_text(&format!("({inner})")) {
            break;
        }
        title = trimmed[..open].trim().to_string();
    }
    let suffixes = [
        " - live",
        " - ao vivo",
        " - remix",
        " - acoustic",
        " - acústico",
        " - acustico",
        " - sped up",
        " - slowed",
        " - radio edit",
        " - karaoke",
        " - nightcore",
    ];
    let lower = title.to_lowercase();
    for suffix in suffixes {
        if lower.ends_with(suffix) && title.len() > suffix.len() {
            title = title[..title.len() - suffix.len()].trim().to_string();
            break;
        }
    }
    let title = collapse(title.trim());
    if title.is_empty() {
        original.to_string()
    } else {
        title
    }
}

/// Same song with and without `feat.`, with and without the artist, and the original when this is a version.
pub fn lyrics_variants(query: &LrclibQuery) -> Vec<LrclibQuery> {
    let mut out = Vec::new();
    let titles = title_forms(&query.track_name);
    let artists = artist_forms(&query.artist_name);
    for title in &titles {
        for artist in &artists {
            push_variant(&mut out, query, title, artist);
        }
        push_variant(&mut out, query, title, "");
    }
    out
}

fn title_forms(title: &str) -> Vec<String> {
    let mut forms = Vec::new();
    let push = |forms: &mut Vec<String>, value: &str| {
        let value = collapse(value.trim());
        if value.is_empty() {
            return;
        }
        if forms.iter().any(|item| fold_lyric_name(item) == fold_lyric_name(&value)) {
            return;
        }
        forms.push(value);
    };
    push(&mut forms, title);
    let cleaned = clean_track_query(title);
    push(&mut forms, &cleaned);
    push(&mut forms, &strip_feat_local(&cleaned));
    push(&mut forms, &base_recording_title(&cleaned));
    push(&mut forms, &base_recording_title(&strip_feat_local(title)));
    push(&mut forms, &strip_part_label(title));
    push(&mut forms, &strip_part_label(&cleaned));
    push(&mut forms, &strip_part_label(&base_recording_title(&cleaned)));
    forms
}

/// Drop `(Pt.1)`, `(Part 1)` and a trailing `pt. 1` so the original title is also searched.
pub fn strip_part_label(input: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let mut out = String::new();
    let mut index = 0;
    while index < chars.len() {
        if chars[index] == '(' || chars[index] == '[' {
            let close = if chars[index] == '(' { ')' } else { ']' };
            if let Some(rel) = chars[index + 1..].iter().position(|ch| *ch == close) {
                let inner: String = chars[index + 1..index + 1 + rel].iter().collect();
                if is_part_label(&inner) {
                    index += rel + 2;
                    continue;
                }
            }
        }
        out.push(chars[index]);
        index += 1;
    }
    strip_trailing_part(&collapse(out.trim()))
}

fn is_part_label(inner: &str) -> bool {
    let text = inner.trim().trim_matches('.').to_lowercase();
    let text = text.trim();
    for prefix in ["parte", "part", "pt"] {
        let Some(rest) = text.strip_prefix(prefix) else {
            continue;
        };
        let rest = rest.trim().trim_start_matches('.').trim();
        if !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit() || ch == '.') && rest.chars().any(|ch| ch.is_ascii_digit()) {
            return true;
        }
    }
    false
}

fn strip_trailing_part(title: &str) -> String {
    let lower = title.to_lowercase();
    for marker in [" parte ", " part ", " pt. ", " pt "] {
        let Some(index) = lower.rfind(marker) else {
            continue;
        };
        let rest = title[index + marker.len()..].trim().trim_end_matches('.');
        if rest.is_empty() || !rest.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }
        let head = title[..index].trim().trim_end_matches(|ch: char| matches!(ch, '-' | ',' | ' '));
        if !head.is_empty() {
            return head.to_string();
        }
    }
    title.to_string()
}

pub fn query_attempts(queries: &[LrclibQuery]) -> Vec<String> {
    queries
        .iter()
        .take(8)
        .map(|query| {
            let title = query.track_name.trim();
            let artist = query.artist_name.trim();
            if artist.is_empty() {
                title.to_string()
            } else {
                format!("{artist} — {title}")
            }
        })
        .collect()
}

fn artist_forms(artist: &str) -> Vec<String> {
    let mut forms = Vec::new();
    let push = |forms: &mut Vec<String>, value: &str| {
        let value = collapse(value.trim());
        if value.is_empty() || forms.iter().any(|item| fold_lyric_name(item) == fold_lyric_name(&value)) {
            return;
        }
        forms.push(value);
    };
    push(&mut forms, artist);
    push(&mut forms, &clean_artist_query(artist));
    push(&mut forms, &strip_feat_local(artist));
    forms
}

fn push_variant(out: &mut Vec<LrclibQuery>, query: &LrclibQuery, title: &str, artist: &str) {
    let title = collapse(title.trim());
    if title.is_empty() {
        return;
    }
    let artist = collapse(artist.trim());
    let duplicate = out.iter().any(|item| {
        fold_lyric_name(&item.track_name) == fold_lyric_name(&title)
            && fold_lyric_name(&item.artist_name) == fold_lyric_name(&artist)
    });
    if duplicate {
        return;
    }
    out.push(LrclibQuery {
        track_name: title,
        artist_name: artist,
        album_name: query.album_name.clone(),
        duration: query.duration,
    });
}

pub fn fold_lyric_name(value: &str) -> String {
    let mut out = String::new();
    let mut space = false;
    for ch in value.chars() {
        let ch = fold_char(ch);
        if ch.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if !ch.is_alphanumeric() {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(ch);
    }
    out
}

fn fold_char(ch: char) -> char {
    match ch {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' | 'Ä' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'É' | 'È' | 'Ê' | 'Ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'Í' | 'Ì' | 'Î' | 'Ï' => 'i',
        'ó' | 'ò' | 'ô' | 'õ' | 'ö' | 'Ó' | 'Ò' | 'Ô' | 'Õ' | 'Ö' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'Ú' | 'Ù' | 'Û' | 'Ü' => 'u',
        'ç' | 'Ç' => 'c',
        'ñ' | 'Ñ' => 'n',
        other => {
            let mut lower = other.to_lowercase();
            lower.next().unwrap_or(other)
        }
    }
}

fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_feat_local(input: &str) -> String {
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

/// Higher is closer. Duration is tolerant: a few seconds barely matter, a different edit is penalized.
pub fn score_lyric_match(
    query_title: &str,
    query_artist: &str,
    query_duration: Option<u32>,
    cand_title: &str,
    cand_artist: &str,
    cand_duration: Option<u32>,
    synced: bool,
) -> i32 {
    let left = fold_lyric_name(&base_recording_title(&strip_feat_local(query_title)));
    let right = fold_lyric_name(&base_recording_title(&strip_feat_local(cand_title)));
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let title = title_points(&left, &right);
    if title < 28 {
        return title;
    }
    let mut score = title;
    let artist_left = fold_lyric_name(&strip_feat_local(query_artist));
    let artist_right = fold_lyric_name(&strip_feat_local(cand_artist));
    if !artist_left.is_empty() && !artist_right.is_empty() {
        if artist_left == artist_right {
            score += 25;
        } else {
            let overlap = jaccard(&tokens(&artist_left), &tokens(&artist_right));
            if overlap >= 0.5 {
                score += (overlap * 20.0).round() as i32;
            } else if artist_right.contains(&artist_left) || artist_left.contains(&artist_right) {
                score += 8;
            } else {
                score -= 12;
            }
        }
    } else if artist_left.is_empty() {
        score -= 4;
    }
    if let (Some(wanted), Some(got)) = (query_duration, cand_duration) {
        let delta = wanted.abs_diff(got);
        if delta <= 5 {
            score += 18;
        } else if delta <= 15 {
            score += 10;
        } else if delta <= 30 {
            score += 0;
        } else if delta <= 60 {
            score -= 12;
        } else if delta > 120 {
            score -= 40;
        } else {
            score -= 18;
        }
    }
    if synced {
        score += 6;
    }
    match (version_kind(query_title), version_kind(cand_title)) {
        (Some(query), Some(cand)) if query != cand => score -= 14,
        (Some(_), None) => score -= 6,
        (None, Some(_)) => score -= 10,
        _ => {}
    }
    score.max(0)
}

fn title_points(left: &str, right: &str) -> i32 {
    if left == right {
        return 70;
    }
    let overlap = jaccard(&tokens(left), &tokens(right));
    let mut score = (overlap * 60.0).round() as i32;
    if right.contains(left) || left.contains(right) {
        score += 12;
    }
    score
}

fn tokens(value: &str) -> Vec<String> {
    value.split_whitespace().map(str::to_string).collect()
}

fn jaccard(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let mut shared = 0.0;
    for item in left {
        if right.iter().any(|other| other == item) {
            shared += 1.0;
        }
    }
    let union = (left.len() + right.len()) as f64 - shared;
    if union <= 0.0 {
        0.0
    } else {
        shared / union
    }
}

pub fn score_lrclib(query: &LrclibQuery, track: &LrclibTrack) -> i32 {
    let synced = track
        .synced
        .as_ref()
        .map(|text| !text.trim().is_empty())
        .unwrap_or(false);
    score_lyric_match(
        &query.track_name,
        &query.artist_name,
        query.duration,
        &track.track_name,
        &track.artist_name,
        track.duration,
        synced,
    )
}

/// Best track at or above [`LYRIC_ACCEPT`]. Ties prefer synced lyrics.
pub fn best_lrclib<'a>(tracks: &'a [LrclibTrack], query: &LrclibQuery) -> Option<&'a LrclibTrack> {
    let mut best: Option<(&LrclibTrack, i32)> = None;
    for track in tracks {
        if !lyric_duration_ok(query.duration, track.duration) {
            continue;
        }
        let score = score_lrclib(query, track);
        if score < LYRIC_ACCEPT {
            continue;
        }
        let replace = match best {
            None => true,
            Some((current, current_score)) => {
                score > current_score
                    || (score == current_score && has_synced(track) && !has_synced(current))
            }
        };
        if replace {
            best = Some((track, score));
        }
    }
    best.map(|(track, _)| track)
}

fn has_synced(track: &LrclibTrack) -> bool {
    track
        .synced
        .as_ref()
        .map(|text| text.contains('['))
        .unwrap_or(false)
}

/// WebVTT or SRT captions become one LRC line per cue.
pub fn captions_to_lrc(input: &str) -> String {
    let trimmed = input.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let parsed = parse_lrc(trimmed);
    if parsed.has_timestamps && parsed.lines.len() >= 2 {
        return trimmed.to_string();
    }
    let mut lines = Vec::new();
    let mut rows = trimmed.lines().peekable();
    while let Some(raw) = rows.next() {
        let line = raw.trim().trim_end_matches('\r');
        if line.is_empty() || line.eq_ignore_ascii_case("WEBVTT") || line.starts_with("NOTE") || line.starts_with("STYLE") || line.starts_with("Kind:") || line.starts_with("Language:") {
            continue;
        }
        if let Some(start) = cue_start(line) {
            let mut text = String::new();
            while let Some(next) = rows.peek() {
                let next_trim = next.trim().trim_end_matches('\r');
                if next_trim.is_empty() || cue_start(next_trim).is_some() {
                    break;
                }
                let next_trim = rows.next().unwrap().trim();
                let clean = strip_caption_tags(next_trim);
                if clean.is_empty() {
                    continue;
                }
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&clean);
            }
            if !text.is_empty() {
                lines.push(format!("{}{}", lrc_stamp(start), text));
            }
        }
    }
    lines.join("\n")
}

fn cue_start(line: &str) -> Option<f64> {
    let (left, _) = line.split_once("-->")?;
    parse_clock(left.trim())
}

fn parse_clock(value: &str) -> Option<f64> {
    let value = value.trim().trim_matches(|ch: char| ch == ',' || ch.is_whitespace());
    let (body, frac) = value
        .split_once(['.', ','])
        .map(|(body, frac)| (body, Some(frac)))
        .unwrap_or((value, None));
    let parts: Vec<&str> = body.split(':').collect();
    let seconds = if parts.len() == 3 {
        let hours: f64 = parts[0].parse().ok()?;
        let minutes: f64 = parts[1].parse().ok()?;
        let secs: f64 = parts[2].parse().ok()?;
        hours * 3600.0 + minutes * 60.0 + secs
    } else if parts.len() == 2 {
        let minutes: f64 = parts[0].parse().ok()?;
        let secs: f64 = parts[1].parse().ok()?;
        minutes * 60.0 + secs
    } else {
        return None;
    };
    let fraction = match frac {
        Some(frac) => {
            let digits: String = frac.chars().take(3).filter(|ch| ch.is_ascii_digit()).collect();
            if digits.is_empty() {
                0.0
            } else {
                let number: f64 = digits.parse().ok()?;
                number / 10f64.powi(digits.len() as i32)
            }
        }
        None => 0.0,
    };
    Some(seconds + fraction)
}

fn strip_caption_tags(text: &str) -> String {
    let mut out = String::new();
    let mut skip = false;
    for ch in text.chars() {
        if ch == '<' {
            skip = true;
            continue;
        }
        if ch == '>' {
            skip = false;
            continue;
        }
        if !skip {
            out.push(ch);
        }
    }
    collapse(out.trim())
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

/// Word-by-word auto captions and `[Music]` cues are not lyrics.
pub fn caption_is_lyrics(lrc: &str) -> bool {
    let parsed = parse_lrc(lrc);
    if parsed.lines.len() < 4 {
        return false;
    }
    let words: Vec<usize> = parsed
        .lines
        .iter()
        .map(|line| line.text.split_whitespace().count())
        .collect();
    let single = words.iter().filter(|count| **count <= 1).count();
    if single * 100 / words.len() > 55 {
        return false;
    }
    let average = words.iter().sum::<usize>() as f64 / words.len() as f64;
    if average < 2.0 {
        return false;
    }
    let noise = parsed
        .lines
        .iter()
        .filter(|line| {
            let text = line.text.to_lowercase();
            text.contains("[music]")
                || text.contains("[música]")
                || text.contains("[musica]")
                || text.contains("[applause]")
                || text.contains("[laughter]")
                || text.trim() == "♪"
        })
        .count();
    noise * 2 < parsed.lines.len()
}

pub fn lyric_preview(text: &str, limit: usize) -> String {
    let parsed = parse_lrc(text);
    let lines: Vec<&str> = if parsed.lines.is_empty() {
        text.lines().map(str::trim).filter(|line| !line.is_empty()).collect()
    } else {
        parsed.lines.iter().map(|line| line.text.as_str()).collect()
    };
    lines.into_iter().take(limit).collect::<Vec<_>>().join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(title: &str, artist: &str, duration: u32) -> LrclibQuery {
        LrclibQuery {
            track_name: title.into(),
            artist_name: artist.into(),
            album_name: String::new(),
            duration: Some(duration),
        }
    }

    fn track(title: &str, artist: &str, duration: u32, synced: bool) -> LrclibTrack {
        LrclibTrack {
            track_name: title.into(),
            artist_name: artist.into(),
            album_name: String::new(),
            duration: Some(duration),
            instrumental: false,
            plain: Some("linha".into()),
            synced: synced.then(|| "[00:01.00]linha".into()),
        }
    }

    #[test]
    fn exact_match_beats_another_song_and_clears_the_bar() {
        let wanted = query("Canção", "Banda", 180);
        let exact = score_lrclib(&wanted, &track("Canção", "Banda", 182, true));
        let other = score_lrclib(&wanted, &track("Outra coisa", "Banda", 181, true));
        assert!(exact >= LYRIC_ACCEPT);
        assert!(other < LYRIC_LIST);
        assert!(exact > other);
    }

    #[test]
    fn feat_and_missing_artist_still_rank() {
        let with_feat = query("Canção (feat. Convidado)", "Banda", 180);
        let stripped = score_lyric_match(
            &with_feat.track_name,
            &with_feat.artist_name,
            with_feat.duration,
            "Canção",
            "Banda",
            Some(180),
            true,
        );
        let no_artist = score_lyric_match("Canção", "", Some(180), "Canção", "Banda", Some(180), true);
        assert!(stripped >= LYRIC_ACCEPT);
        assert!(no_artist >= LYRIC_LIST);
        let variants = lyrics_variants(&with_feat);
        assert!(variants.iter().any(|item| fold_lyric_name(&item.track_name) == "cancao" && item.artist_name.is_empty()));
        assert!(variants.iter().any(|item| !item.artist_name.is_empty()));
    }

    #[test]
    fn a_live_version_can_fall_back_to_the_original() {
        assert_eq!(version_kind("Canção (Live at SP)"), Some("live"));
        assert_eq!(version_kind("Canção - Remix"), Some("remix"));
        assert_eq!(base_recording_title("Canção (Live)"), "Canção");
        assert_eq!(base_recording_title("Canção - Nightcore"), "Canção");
        let live = score_lyric_match("Canção (Live)", "Banda", Some(200), "Canção", "Banda", Some(186), true);
        assert!(live >= LYRIC_ACCEPT, "live fallback score was {live}");
        let variants = lyrics_variants(&query("Canção (Acoustic)", "Banda", 190));
        assert!(variants.iter().any(|item| item.track_name == "Canção"));
    }

    #[test]
    fn closer_duration_wins_when_the_title_is_the_same() {
        let wanted = query("Canção", "Banda", 180);
        let tracks = vec![
            track("Canção", "Banda", 400, true),
            track("Canção", "Banda", 181, false),
        ];
        let best = best_lrclib(&tracks, &wanted).unwrap();
        assert_eq!(best.duration, Some(181));
        assert!(best_lrclib(&[], &wanted).is_none());
    }

    #[test]
    fn manual_captions_become_lrc_and_auto_captions_are_rejected() {
        let vtt = "\
WEBVTT

00:00:01.000 --> 00:00:04.000
primeira linha da canção

00:00:04.200 --> 00:00:08.000
segunda linha continua

00:00:08.200 --> 00:00:12.000
terceira linha da letra

00:00:12.200 --> 00:00:16.000
quarta linha fecha o verso
";
        let lrc = captions_to_lrc(vtt);
        assert!(lrc.contains("[00:01.00]primeira linha da canção"));
        assert!(caption_is_lyrics(&lrc));
        let auto = "\
1
00:00:01,000 --> 00:00:01,200
eu

2
00:00:01,200 --> 00:00:01,400
não

3
00:00:01,400 --> 00:00:01,600
sei

4
00:00:01,600 --> 00:00:01,800
não
";
        let auto_lrc = captions_to_lrc(auto);
        assert!(!caption_is_lyrics(&auto_lrc));
        assert_eq!(
            lyric_preview("[00:01.00]oi\n[00:02.00]olá", 1),
            "oi"
        );
    }
}
