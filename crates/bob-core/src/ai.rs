//! Free-tier Gemini routing. Limits follow the slot, even when the model id
//! is overridden. The day is UTC and a minute is a unix minute.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Flash,
    Bulk,
    Embed,
    Transcribe,
}

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub rpm: u32,
    pub rpd: u32,
    pub tpm: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    Allow,
    Fallback,
    Heuristic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelUse {
    pub day: String,
    pub requests: u32,
    pub tokens: u32,
    pub minute: i64,
    pub minute_count: u32,
    pub minute_tokens: u32,
    pub cooldown_until: i64,
}

impl ModelUse {
    pub fn fresh(day: &str) -> Self {
        Self {
            day: day.into(),
            requests: 0,
            tokens: 0,
            minute: 0,
            minute_count: 0,
            minute_tokens: 0,
            cooldown_until: 0,
        }
    }
}

pub fn limits(slot: Slot) -> Limits {
    match slot {
        Slot::Flash => Limits { rpm: 15, rpd: 500, tpm: 250_000 },
        Slot::Bulk => Limits { rpm: 30, rpd: 14_400, tpm: 16_000 },
        Slot::Embed => Limits { rpm: 100, rpd: 1_000, tpm: 0 },
        Slot::Transcribe => Limits { rpm: 5, rpd: 25, tpm: 0 },
    }
}

pub fn utc_day(unix: i64) -> String {
    let days = unix.div_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn estimate_tokens(chars: usize) -> u32 {
    ((chars / 4).max(1)) as u32
}

pub fn route(primary: &ModelUse, fallback: Option<&ModelUse>, slot: Slot, now: i64, tokens: u32) -> Route {
    if allowed(primary, slot, now, tokens) {
        Route::Allow
    } else if fallback.is_some_and(|use_| allowed(use_, slot, now, tokens)) {
        Route::Fallback
    } else {
        Route::Heuristic
    }
}

pub fn allowed(use_: &ModelUse, slot: Slot, now: i64, tokens: u32) -> bool {
    let limits = limits(slot);
    if now < use_.cooldown_until {
        return false;
    }
    let day = utc_day(now);
    let requests = if use_.day == day { use_.requests } else { 0 };
    let spent = if use_.day == day { use_.tokens } else { 0 };
    if requests >= limits.rpd {
        return false;
    }
    if limits.tpm > 0 && spent.saturating_add(tokens) > limits.tpm && requests > 0 && use_.day == day {
        // daily token budget is not a published cap; TPM is the per-minute one
    }
    let minute = now.div_euclid(60);
    let (count, minute_tokens) = if use_.minute == minute {
        (use_.minute_count, use_.minute_tokens)
    } else {
        (0, 0)
    };
    if count >= limits.rpm {
        return false;
    }
    if limits.tpm > 0 && minute_tokens.saturating_add(tokens) > limits.tpm {
        return false;
    }
    true
}

pub fn note_success(use_: &mut ModelUse, now: i64, tokens: u32) {
    roll(use_, now);
    use_.requests += 1;
    use_.tokens = use_.tokens.saturating_add(tokens);
    use_.minute_count += 1;
    use_.minute_tokens = use_.minute_tokens.saturating_add(tokens);
}

pub fn note_429(use_: &mut ModelUse, now: i64) {
    roll(use_, now);
    use_.cooldown_until = now + 60;
}

fn roll(use_: &mut ModelUse, now: i64) {
    let day = utc_day(now);
    if use_.day != day {
        *use_ = ModelUse::fresh(&day);
    }
    let minute = now.div_euclid(60);
    if use_.minute != minute {
        use_.minute = minute;
        use_.minute_count = 0;
        use_.minute_tokens = 0;
    }
}

pub fn cosine(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    if len == 0 {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut a = 0.0f32;
    let mut b = 0.0f32;
    for index in 0..len {
        dot += left[index] * right[index];
        a += left[index] * left[index];
        b += right[index] * right[index];
    }
    if a == 0.0 || b == 0.0 {
        return 0.0;
    }
    dot / (a.sqrt() * b.sqrt())
}

pub fn embed_document(artist: &str, title: &str, album: &str, lyrics: &str) -> String {
    let snippet: String = lyrics.chars().take(800).collect();
    format!("artista: {artist}\ntítulo: {title}\nálbum: {album}\nletra: {snippet}")
}

pub fn description_overlap(query: &str, document: &str) -> f64 {
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| token.chars().count() > 2)
        .map(|token| token.to_lowercase())
        .collect();
    if tokens.is_empty() {
        return 0.0;
    }
    let hay = document.to_lowercase();
    let hits = tokens.iter().filter(|token| hay.contains(token.as_str())).count();
    hits as f64 / tokens.len() as f64
}

pub fn lrc_from_transcript(body: &str) -> String {
    let mut out = String::from("[by:gerada por IA]\n");
    let mut stamp = 0.0f64;
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.contains(']') {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let minutes = (stamp / 60.0) as u32;
        let seconds = stamp - f64::from(minutes) * 60.0;
        out.push_str(&format!("[{minutes:02}:{seconds:05.2}]{line}\n"));
        stamp += 4.0;
    }
    out
}

pub fn wrapped_fallback(period: &str, minutes: f64, artist: &str, track: &str) -> String {
    let minutes = minutes.round() as u32;
    if artist.is_empty() && track.is_empty() {
        return format!("neste {period} o bob baixa ficou quieto. bota uma faixa pra tocar.");
    }
    format!("neste {period} você ouviu cerca de {minutes} minutos. no topo, {artist} com {track}.")
}

pub fn split_batch(items: &[String], max_items: usize, max_tokens: u32) -> Vec<Vec<String>> {
    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut tokens = 0u32;
    for item in items {
        let cost = estimate_tokens(item.len());
        if !current.is_empty() && (current.len() >= max_items || tokens.saturating_add(cost) > max_tokens) {
            batches.push(std::mem::take(&mut current));
            tokens = 0;
        }
        current.push(item.clone());
        tokens = tokens.saturating_add(cost);
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_day_falls_back_then_gives_up() {
        let now = 1_700_000_000;
        let mut primary = ModelUse::fresh(&utc_day(now));
        primary.requests = 500;
        let fallback = ModelUse::fresh(&utc_day(now));
        assert_eq!(route(&primary, Some(&fallback), Slot::Flash, now, 10), Route::Fallback);
        let mut full = fallback.clone();
        full.requests = 500;
        assert_eq!(route(&primary, Some(&full), Slot::Flash, now, 10), Route::Heuristic);
        note_429(&mut primary, now);
        primary.requests = 0;
        assert_eq!(route(&primary, None, Slot::Flash, now, 10), Route::Heuristic);
        assert_eq!(route(&primary, None, Slot::Flash, now + 61, 10), Route::Allow);
    }

    #[test]
    fn bulk_stays_inside_the_token_minute() {
        let now = 1_700_000_000;
        let mut use_ = ModelUse::fresh(&utc_day(now));
        note_success(&mut use_, now, 15_000);
        assert!(!allowed(&use_, Slot::Bulk, now, 2_000));
        assert!(allowed(&use_, Slot::Bulk, now + 60, 2_000));
    }

    #[test]
    fn transcript_is_marked_as_generated() {
        let lrc = lrc_from_transcript("primeira linha\n[00:08.00]segunda");
        assert!(lrc.starts_with("[by:gerada por IA]"));
        assert!(lrc.contains("[00:08.00]segunda"));
    }

    #[test]
    fn batches_stop_before_the_small_model_fills_up() {
        let items = vec!["palavra ".repeat(400); 3];
        let batches = split_batch(&items, 8, 1500);
        assert!(batches.len() >= 2);
        assert!(batches.iter().all(|batch| batch.len() <= 8));
    }
}
