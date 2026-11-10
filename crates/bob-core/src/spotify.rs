//! Spotify links without a user login. The public page, oEmbed and the
//! anonymous web-player token supply the metadata.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpotifyKind {
    Track,
    Album,
    Playlist,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyLink {
    pub kind: SpotifyKind,
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct MatchCandidate {
    pub title: String,
    pub channel: String,
    pub duration: Option<f64>,
}

pub fn parse_spotify(text: &str) -> Option<SpotifyLink> {
    let trimmed = text.trim().trim_matches(|c: char| "<>\"'()[]{},;".contains(c));
    if let Some(link) = parse_uri(trimmed) {
        return Some(link);
    }
    let lower = trimmed.to_ascii_lowercase();
    let host = lower.find("open.spotify.com/")?;
    let rest = &trimmed[host + "open.spotify.com/".len()..];
    let rest = rest.split(['?', '#']).next().unwrap_or(rest);
    let mut parts = rest.split('/').filter(|part| !part.is_empty());
    let mut kind = parts.next()?;
    if kind.starts_with("intl-") {
        kind = parts.next()?;
    }
    let id = parts.next()?.trim();
    kind_id(kind, id)
}

pub fn canonical(link: &SpotifyLink) -> String {
    let kind = match link.kind {
        SpotifyKind::Track => "track",
        SpotifyKind::Album => "album",
        SpotifyKind::Playlist => "playlist",
    };
    format!("https://open.spotify.com/{kind}/{}", link.id)
}

pub fn pick_match(artist: &str, title: &str, duration: Option<f64>, candidates: &[MatchCandidate]) -> Option<usize> {
    let mut best: Option<(i32, usize)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let score = score_candidate(artist, title, duration, candidate);
        if best.map(|(current, _)| score > current).unwrap_or(true) {
            best = Some((score, index));
        }
    }
    best.filter(|(score, _)| *score >= 4).map(|(_, index)| index)
}

fn score_candidate(artist: &str, title: &str, duration: Option<f64>, candidate: &MatchCandidate) -> i32 {
    let hay = format!("{} {}", candidate.title, candidate.channel).to_lowercase();
    let want_title = title.trim().to_lowercase();
    let want_artist = artist.trim().to_lowercase();
    let mut score = 0;
    if want_title.is_empty() {
        score -= 2;
    } else if hay.contains(&want_title) {
        score += 4;
    } else if token_overlap(&want_title, &hay) >= 0.5 {
        score += 2;
    } else {
        score -= 3;
    }
    if !want_artist.is_empty() && hay.contains(&want_artist) {
        score += 3;
    }
    if let (Some(want), Some(got)) = (duration, candidate.duration) {
        let delta = (want - got).abs();
        if delta <= 3.0 {
            score += 6;
        } else {
            return -20;
        }
    }
    if hay.contains("official audio") || hay.contains("- topic") || hay.contains(" topic") {
        score += 3;
    }
    if hay.contains("official video") && !hay.contains("audio") {
        score -= 1;
    }
    score
}

fn token_overlap(needle: &str, hay: &str) -> f64 {
    let tokens: Vec<&str> = needle.split(|c: char| !c.is_alphanumeric()).filter(|token| token.len() > 2).collect();
    if tokens.is_empty() {
        return 0.0;
    }
    let hits = tokens.iter().filter(|token| hay.contains(*token)).count();
    hits as f64 / tokens.len() as f64
}

fn parse_uri(text: &str) -> Option<SpotifyLink> {
    let rest = text.strip_prefix("spotify:")?;
    let mut parts = rest.split(':');
    let kind = parts.next()?;
    let id = parts.next()?.split('?').next().unwrap_or("");
    kind_id(kind, id)
}

fn kind_id(kind: &str, id: &str) -> Option<SpotifyLink> {
    let kind = match kind.to_ascii_lowercase().as_str() {
        "track" => SpotifyKind::Track,
        "album" => SpotifyKind::Album,
        "playlist" => SpotifyKind::Playlist,
        _ => return None,
    };
    if id.len() < 10 || id.len() > 40 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    Some(SpotifyLink { kind, id: id.to_string() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_urls_uris_and_share_queries() {
        let track = parse_spotify("https://open.spotify.com/track/11dFghVXANMlKmJXsNCbNl?si=abc").unwrap();
        assert_eq!(track.kind, SpotifyKind::Track);
        assert_eq!(canonical(&track), "https://open.spotify.com/track/11dFghVXANMlKmJXsNCbNl");
        let album = parse_spotify("spotify:album:6rqhFgbbKwnb9MLmUQDhG6").unwrap();
        assert_eq!(album.kind, SpotifyKind::Album);
        let list = parse_spotify("https://open.spotify.com/intl-pt/playlist/37i9dQZF1DXcBWIGoYBM5M").unwrap();
        assert_eq!(list.kind, SpotifyKind::Playlist);
        assert!(parse_spotify("https://open.spotify.com/artist/abc").is_none());
    }

    #[test]
    fn prefers_official_audio_within_three_seconds() {
        let candidates = vec![
            MatchCandidate { title: "Saudade official video".into(), channel: "vevo".into(), duration: Some(200.0) },
            MatchCandidate { title: "Saudade".into(), channel: "artista - topic".into(), duration: Some(184.0) },
            MatchCandidate { title: "outra coisa".into(), channel: "topic".into(), duration: Some(184.0) },
        ];
        assert_eq!(pick_match("artista", "saudade", Some(183.0), &candidates), Some(1));
        assert_eq!(pick_match("artista", "saudade", Some(20.0), &candidates), None);
    }
}
