#[derive(Debug, Clone, PartialEq)]
pub struct RecordingHit {
    pub recording_id: String,
    pub release_id: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<u32>,
    /// 0 to 100.
    pub score: f64,
}

pub fn musicbrainz_user_agent(version: &str) -> String {
    format!("bob-baixa/{version} ( https://github.com/goszc/bob-baixa )")
}

pub fn pace_wait(last_ms: Option<u128>, now_ms: u128, gap_ms: u128) -> u128 {
    match last_ms {
        Some(last) => gap_ms.saturating_sub(now_ms.saturating_sub(last)),
        None => 0,
    }
}

pub fn mb_search_url(title: &str, artist: &str) -> String {
    let title = title.trim();
    let artist = artist.trim();
    let mut query = format!("recording:\"{}\"", title.replace('"', ""));
    if !artist.is_empty() {
        query.push_str(&format!(" AND artist:\"{}\"", artist.replace('"', "")));
    }
    format!(
        "https://musicbrainz.org/ws/2/recording?fmt=json&limit=5&query={}",
        percent(&query)
    )
}

pub fn cover_art_url(release_id: &str) -> String {
    format!(
        "https://coverartarchive.org/release/{}/front-500",
        release_id.trim()
    )
}

pub fn itunes_search_url(artist: &str, album: &str, title: &str) -> String {
    let term = [artist.trim(), album.trim(), title.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "https://itunes.apple.com/search?entity=album&limit=5&term={}",
        percent(&term)
    )
}

pub fn acoustid_lookup_url() -> &'static str {
    "https://api.acoustid.org/v2/lookup"
}

fn percent(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

pub fn parse_acoustid(body: &str) -> Vec<RecordingHit> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    if value.get("status").and_then(|item| item.as_str()) != Some("ok") {
        return Vec::new();
    }
    let Some(results) = value.get("results").and_then(|item| item.as_array()) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for result in results {
        let score = result.get("score").and_then(|item| item.as_f64()).unwrap_or(0.0) * 100.0;
        let Some(recordings) = result.get("recordings").and_then(|item| item.as_array()) else {
            continue;
        };
        for recording in recordings {
            if let Some(hit) = recording_from_acoustid(recording, score) {
                hits.push(hit);
            }
        }
    }
    hits.sort_by(|left, right| right.score.total_cmp(&left.score));
    hits
}

fn recording_from_acoustid(recording: &serde_json::Value, score: f64) -> Option<RecordingHit> {
    let recording_id = recording.get("id")?.as_str()?.to_string();
    let title = recording.get("title")?.as_str()?.trim().to_string();
    if title.is_empty() {
        return None;
    }
    let artist = first_name(recording.get("artists"));
    let duration = recording
        .get("duration")
        .and_then(|item| item.as_f64())
        .map(|value| value.round() as u32)
        .filter(|value| *value > 0);
    let (release_id, album) = release_from_acoustid(recording);
    Some(RecordingHit {
        recording_id,
        release_id,
        title,
        artist,
        album,
        duration,
        score,
    })
}

fn release_from_acoustid(recording: &serde_json::Value) -> (String, String) {
    let groups = recording
        .get("releasegroups")
        .and_then(|item| item.as_array())
        .or_else(|| recording.get("releases").and_then(|item| item.as_array()));
    let Some(groups) = groups else {
        return (String::new(), String::new());
    };
    for group in groups {
        let album = group
            .get("title")
            .and_then(|item| item.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if let Some(releases) = group.get("releases").and_then(|item| item.as_array()) {
            if let Some(id) = releases.iter().find_map(|release| release.get("id").and_then(|item| item.as_str())) {
                return (id.to_string(), album);
            }
        }
        if let Some(id) = group.get("id").and_then(|item| item.as_str()) {
            if group.get("releases").is_none() {
                return (id.to_string(), album);
            }
        }
    }
    (String::new(), String::new())
}

fn first_name(value: Option<&serde_json::Value>) -> String {
    value
        .and_then(|item| item.as_array())
        .and_then(|items| items.first())
        .and_then(|item| item.get("name").or_else(|| item.get("artist").and_then(|artist| artist.get("name"))))
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .trim()
        .to_string()
}

pub fn parse_musicbrainz_recordings(body: &str) -> Vec<RecordingHit> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(body) else {
        return Vec::new();
    };
    let Some(recordings) = value.get("recordings").and_then(|item| item.as_array()) else {
        return Vec::new();
    };
    let mut hits = Vec::new();
    for recording in recordings {
        let Some(recording_id) = recording.get("id").and_then(|item| item.as_str()) else {
            continue;
        };
        let Some(title) = recording.get("title").and_then(|item| item.as_str()) else {
            continue;
        };
        let title = title.trim().to_string();
        if title.is_empty() {
            continue;
        }
        let score = recording.get("score").and_then(|item| item.as_f64()).unwrap_or(0.0);
        let artist = musicbrainz_artist(recording);
        let duration = recording
            .get("length")
            .and_then(|item| item.as_u64())
            .map(|ms| (ms / 1000) as u32)
            .filter(|value| *value > 0);
        let (release_id, album) = musicbrainz_release(recording);
        hits.push(RecordingHit {
            recording_id: recording_id.to_string(),
            release_id,
            title,
            artist,
            album,
            duration,
            score,
        });
    }
    hits.sort_by(|left, right| right.score.total_cmp(&left.score));
    hits
}

fn musicbrainz_artist(recording: &serde_json::Value) -> String {
    recording
        .get("artist-credit")
        .and_then(|item| item.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    item.get("name")
                        .and_then(|name| name.as_str())
                        .or_else(|| {
                            item.get("artist")
                                .and_then(|artist| artist.get("name"))
                                .and_then(|name| name.as_str())
                        })
                })
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn musicbrainz_release(recording: &serde_json::Value) -> (String, String) {
    let Some(releases) = recording.get("releases").and_then(|item| item.as_array()) else {
        return (String::new(), String::new());
    };
    let preferred = releases.iter().find(|release| {
        release.get("status").and_then(|item| item.as_str()) == Some("Official")
    });
    let release = preferred.or_else(|| releases.first());
    let Some(release) = release else {
        return (String::new(), String::new());
    };
    let id = release.get("id").and_then(|item| item.as_str()).unwrap_or("").to_string();
    let title = release.get("title").and_then(|item| item.as_str()).unwrap_or("").trim().to_string();
    (id, title)
}

pub fn parse_itunes_artwork(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    let results = value.get("results")?.as_array()?;
    for result in results {
        let url = result.get("artworkUrl100").and_then(|item| item.as_str())?;
        if url.trim().is_empty() {
            continue;
        }
        return Some(url.replace("100x100", "600x600"));
    }
    None
}

pub fn fpcalc_args(path: &str) -> Vec<String> {
    vec!["-json".into(), "-length".into(), "120".into(), path.into()]
}

/// Names safe for a Drive folder or a Windows path segment.
pub fn folder_name(value: &str, fallback: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|ch| match ch {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => ' ',
            other => other,
        })
        .collect();
    let cleaned = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let cleaned = cleaned.trim_matches('.').trim();
    if cleaned.is_empty() {
        fallback.to_string()
    } else {
        cleaned.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_acoustid_and_musicbrainz_and_paces_requests() {
        let acoustid = r#"{"status":"ok","results":[{"score":0.92,"recordings":[{"id":"rec-1","title":"Canção","duration":181,"artists":[{"name":"Banda"}],"releasegroups":[{"title":"Disco","releases":[{"id":"rel-1"}]}]}]}]}"#;
        let hits = parse_acoustid(acoustid);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].title, "Canção");
        assert_eq!(hits[0].artist, "Banda");
        assert_eq!(hits[0].album, "Disco");
        assert_eq!(hits[0].release_id, "rel-1");
        assert!((hits[0].score - 92.0).abs() < 0.01);
        assert!(parse_acoustid(r#"{"status":"error"}"#).is_empty());

        let mb = r#"{"recordings":[{"id":"rec-2","score":97,"title":"Canção","length":180000,"artist-credit":[{"name":"Banda"}],"releases":[{"id":"rel-2","title":"Disco","status":"Official"},{"id":"rel-x","title":"Boot","status":"Bootleg"}]}]}"#;
        let hits = parse_musicbrainz_recordings(mb);
        assert_eq!(hits[0].release_id, "rel-2");
        assert_eq!(hits[0].duration, Some(180));
        assert!(mb_search_url("Canção", "Banda").contains("recording%3A"));
        assert_eq!(
            cover_art_url("rel-2"),
            "https://coverartarchive.org/release/rel-2/front-500"
        );
        assert_eq!(pace_wait(Some(1_000), 1_500, 1_100), 600);
        assert_eq!(pace_wait(Some(1_000), 3_000, 1_100), 0);
        assert_eq!(pace_wait(None, 3_000, 1_100), 0);
    }

    #[test]
    fn itunes_artwork_is_enlarged_and_folder_names_are_safe() {
        let body = r#"{"results":[{"artworkUrl100":"https://example.com/100x100bb.jpg","collectionName":"Disco"}]}"#;
        assert_eq!(
            parse_itunes_artwork(body).as_deref(),
            Some("https://example.com/600x600bb.jpg")
        );
        assert_eq!(folder_name("  A/B:C  ", "Avulsas"), "A B C");
        assert_eq!(folder_name("...", "Desconhecido"), "Desconhecido");
    }
}
