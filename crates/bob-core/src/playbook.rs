//! Queue placement, the offline pin, the 15-second preview and Gemini JSON.

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueSpot {
    Off,
    End,
    Next,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QueueInsert {
    pub queue_index: usize,
    pub order_at: usize,
}

pub fn keeps_local_copy(drive_only: bool, pinned: bool) -> bool {
    pinned || !drive_only
}

/// Bytes that cover about 15 seconds, clamped so a tiny or huge file stays reasonable.
pub fn preview_span(size: u64, duration: Option<f64>) -> u64 {
    const FLOOR: u64 = 64 * 1024;
    const CAP: u64 = 2 * 1024 * 1024;
    if size == 0 {
        return FLOOR;
    }
    let estimate = match duration {
        Some(seconds) if seconds > 1.0 => ((size as f64) * (15.0 / seconds)) as u64,
        _ => 256 * 1024,
    };
    estimate.clamp(FLOOR, CAP).min(size)
}

pub fn next_in_order(order: &[usize], index: usize, count: usize) -> Vec<usize> {
    let Some(pos) = order.iter().position(|item| *item == index) else {
        return Vec::new();
    };
    order.iter().skip(pos + 1).take(count).copied().collect()
}

pub fn place_download(queue_len: usize, order_len: usize, current_order_pos: Option<usize>, spot: QueueSpot) -> Option<QueueInsert> {
    match spot {
        QueueSpot::Off => None,
        QueueSpot::End => Some(QueueInsert { queue_index: queue_len, order_at: order_len }),
        QueueSpot::Next => Some(QueueInsert {
            queue_index: queue_len,
            order_at: current_order_pos.map(|pos| pos + 1).unwrap_or(order_len),
        }),
    }
}

pub fn queue_spot(enabled: bool, where_: &str) -> QueueSpot {
    if !enabled {
        QueueSpot::Off
    } else if where_.eq_ignore_ascii_case("next") {
        QueueSpot::Next
    } else {
        QueueSpot::End
    }
}

/// How many library tracks and how many new downloads a discovery shuffle wants.
pub fn discovery_split(total: usize, new_ratio: f64) -> (usize, usize) {
    let ratio = if new_ratio.is_finite() { new_ratio.clamp(0.0, 0.8) } else { 0.35 };
    let fresh = ((total as f64) * ratio).round() as usize;
    let fresh = fresh.min(total);
    (total - fresh, fresh)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeminiClock {
    pub minute: i64,
    pub used: u32,
    pub cooldown_until: i64,
}

impl GeminiClock {
    pub fn new() -> Self {
        Self { minute: 0, used: 0, cooldown_until: 0 }
    }
}

pub fn gemini_allow(clock: &GeminiClock, now: i64, rpm: u32) -> bool {
    if now < clock.cooldown_until {
        return false;
    }
    if now.div_euclid(60) != clock.minute {
        return true;
    }
    clock.used < rpm
}

pub fn gemini_hit(clock: &mut GeminiClock, now: i64) {
    let minute = now.div_euclid(60);
    if clock.minute != minute {
        clock.minute = minute;
        clock.used = 0;
    }
    clock.used += 1;
}

pub fn gemini_backoff(clock: &mut GeminiClock, now: i64) {
    clock.cooldown_until = now + 60;
}

pub fn parse_model_json(text: &str) -> Result<Value, String> {
    let trimmed = text.trim();
    let start_obj = trimmed.find('{');
    let start_arr = trimmed.find('[');
    let slice = match (start_obj, start_arr) {
        (Some(obj), Some(arr)) if arr < obj => {
            let end = trimmed.rfind(']').ok_or_else(|| "json incompleto".to_string())?;
            &trimmed[arr..=end]
        }
        (Some(obj), _) => {
            let end = trimmed.rfind('}').ok_or_else(|| "json incompleto".to_string())?;
            &trimmed[obj..=end]
        }
        (None, Some(arr)) => {
            let end = trimmed.rfind(']').ok_or_else(|| "json incompleto".to_string())?;
            &trimmed[arr..=end]
        }
        _ => return Err("sem json".into()),
    };
    serde_json::from_str(slice).map_err(|error| error.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Suggestion {
    pub artist: String,
    pub title: String,
}

pub fn suggestions_from_json(value: &Value) -> Vec<Suggestion> {
    let list = value.get("tracks").and_then(Value::as_array).or_else(|| value.as_array());
    let Some(list) = list else { return Vec::new() };
    list.iter()
        .filter_map(|item| {
            let title = item.get("title").and_then(Value::as_str)?.trim().to_string();
            if title.is_empty() {
                return None;
            }
            let artist = item.get("artist").and_then(Value::as_str).unwrap_or("").trim().to_string();
            Some(Suggestion { artist, title })
        })
        .collect()
}

pub fn cleaned_title(value: &Value) -> Option<Suggestion> {
    let title = value.get("title").and_then(Value::as_str)?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    Some(Suggestion {
        artist: value.get("artist").and_then(Value::as_str).unwrap_or("").trim().to_string(),
        title,
    })
}

pub fn meaning_text(value: &Value) -> (String, String) {
    let translation = value.get("translation").and_then(Value::as_str).unwrap_or("").trim().to_string();
    let meaning = value.get("meaning").and_then(Value::as_str).unwrap_or("").trim().to_string();
    (translation, meaning)
}

pub fn top_artists(history: &[(String, u32)], limit: usize) -> Vec<String> {
    let mut rows = history.to_vec();
    rows.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    rows.into_iter().map(|(name, _)| name).filter(|name| !name.trim().is_empty()).take(limit).collect()
}

pub fn radio_score(seed_artist: &str, seed_album: &str, artist: &str, album: &str, plays: u32) -> i32 {
    let mut score = plays.min(20) as i32;
    if !seed_artist.is_empty() && artist.eq_ignore_ascii_case(seed_artist) {
        score += 8;
    }
    if !seed_album.is_empty() && album.eq_ignore_ascii_case(seed_album) {
        score += 4;
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_pin_keeps_the_file_and_a_preview_is_about_fifteen_seconds() {
        assert!(keeps_local_copy(true, true));
        assert!(!keeps_local_copy(true, false));
        assert!(keeps_local_copy(false, false));
        let size = 4 * 1024 * 1024;
        let span = preview_span(size, Some(180.0));
        let expect = (size as f64) * (15.0 / 180.0);
        assert!((span as f64 - expect).abs() < 2.0);
        assert!(preview_span(10, Some(180.0)) <= 10);
    }

    #[test]
    fn prefetch_and_queue_placement() {
        let order = vec![2, 0, 4, 1];
        assert_eq!(next_in_order(&order, 0, 2), vec![4, 1]);
        assert!(next_in_order(&order, 9, 2).is_empty());
        assert_eq!(
            place_download(3, 3, Some(1), QueueSpot::End),
            Some(QueueInsert { queue_index: 3, order_at: 3 })
        );
        assert_eq!(
            place_download(3, 3, Some(1), QueueSpot::Next),
            Some(QueueInsert { queue_index: 3, order_at: 2 })
        );
        assert_eq!(place_download(3, 3, Some(1), QueueSpot::Off), None);
        assert_eq!(queue_spot(true, "next"), QueueSpot::Next);
        assert_eq!(discovery_split(10, 0.3), (7, 3));
    }

    #[test]
    fn gemini_json_survives_a_fence_and_the_minute_budget() {
        let raw = "```json\n{\"tracks\":[{\"artist\":\"A\",\"title\":\"Saudade\"}],\"translation\":\"oi\",\"meaning\":\"fala de saudade\"}\n```";
        let value = parse_model_json(raw).unwrap();
        assert_eq!(suggestions_from_json(&value), vec![Suggestion { artist: "A".into(), title: "Saudade".into() }]);
        assert_eq!(meaning_text(&value).1, "fala de saudade");
        let cleaned = parse_model_json("{\"artist\":\"B\",\"title\":\"Manhã\"}").unwrap();
        assert_eq!(cleaned_title(&cleaned).unwrap().title, "Manhã");
        let mut clock = GeminiClock::new();
        assert!(gemini_allow(&clock, 120, 15));
        gemini_hit(&mut clock, 120);
        clock.used = 15;
        assert!(!gemini_allow(&clock, 120, 15));
        gemini_backoff(&mut clock, 120);
        assert!(!gemini_allow(&clock, 130, 15));
        assert!(gemini_allow(&clock, 181, 15));
    }
}
