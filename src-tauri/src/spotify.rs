//! Public Spotify metadata, then a YouTube Music match. No user login.

use std::path::Path;

use bob_core::{canonical, parse_search, pick_match, MatchCandidate, MediaItem, PlaylistEntry, SpotifyKind, SpotifyLink};
use serde_json::Value;

use crate::runtime::capture;

pub async fn resolve(bin: &Path, link: &SpotifyLink) -> Result<MediaItem, String> {
    let client = reqwest::Client::builder()
        .user_agent("Mozilla/5.0 bob-baixa")
        .timeout(std::time::Duration::from_secs(25))
        .build()
        .map_err(|error| error.to_string())?;
    let token = public_token(&client).await?;
    let (title, cover, tracks) = catalog(&client, &token, link).await?;
    let mut entries = Vec::new();
    let mut unmatched = Vec::new();
    for track in tracks {
        match search_one(bin, &track).await {
            Some(url) => entries.push(PlaylistEntry {
                id: url.clone(),
                url,
                title: track.title.clone(),
                duration: track.duration,
                thumbnail: if track.cover.is_empty() { None } else { Some(track.cover.clone()) },
                artist: track.artist.clone(),
                album: track.album.clone(),
                track: track.number,
                cover: if track.cover.is_empty() { cover.clone() } else { track.cover.clone() },
            }),
            None => unmatched.push(format!("{} — {}", track.artist, track.title)),
        }
    }
    Ok(MediaItem {
        kind: "playlist".into(),
        id: link.id.clone(),
        url: canonical(link),
        title,
        channel: Some("spotify".into()),
        duration: None,
        thumbnail: if cover.is_empty() { None } else { Some(cover) },
        view_count: None,
        formats: Vec::new(),
        entries,
        unmatched,
    })
}

struct RemoteTrack {
    title: String,
    artist: String,
    album: String,
    duration: Option<f64>,
    cover: String,
    number: u32,
}

async fn public_token(client: &reqwest::Client) -> Result<String, String> {
    let value = client
        .get("https://open.spotify.com/get_access_token?reason=init&productType=web-player")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .json::<Value>()
        .await
        .map_err(|error| error.to_string())?;
    value
        .get("accessToken")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| "não consegui ler o spotify sem login.".into())
}

async fn catalog(client: &reqwest::Client, token: &str, link: &SpotifyLink) -> Result<(String, String, Vec<RemoteTrack>), String> {
    match link.kind {
        SpotifyKind::Track => {
            let value = get_json(client, token, &format!("https://api.spotify.com/v1/tracks/{}", link.id)).await?;
            let track = track_from(&value, "");
            let title = track.title.clone();
            let cover = track.cover.clone();
            Ok((title, cover, vec![track]))
        }
        SpotifyKind::Album => {
            let value = get_json(client, token, &format!("https://api.spotify.com/v1/albums/{}", link.id)).await?;
            let title = text(&value, "name");
            let cover = image(&value);
            let album_artist = artists(&value);
            let mut tracks = page_tracks(client, token, value.get("tracks").cloned().unwrap_or(Value::Null), &title, &album_artist).await?;
            for track in &mut tracks {
                if track.cover.is_empty() {
                    track.cover = cover.clone();
                }
                if track.album.is_empty() {
                    track.album = title.clone();
                }
            }
            Ok((title, cover, tracks))
        }
        SpotifyKind::Playlist => {
            let value = get_json(client, token, &format!("https://api.spotify.com/v1/playlists/{}?fields=name,images,tracks.items(track(name,duration_ms,track_number,artists(name),album(name,images))),tracks.next,tracks.total", link.id)).await?;
            let title = text(&value, "name");
            let cover = image(&value);
            let tracks = page_playlist(client, token, value.get("tracks").cloned().unwrap_or(Value::Null)).await?;
            Ok((title, cover, tracks))
        }
    }
}

async fn page_tracks(client: &reqwest::Client, token: &str, mut page: Value, album: &str, artist: &str) -> Result<Vec<RemoteTrack>, String> {
    let mut out = Vec::new();
    for _ in 0..40 {
        let items = page.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        if items.is_empty() {
            break;
        }
        for item in items {
            let mut track = track_from(&item, album);
            if track.artist.is_empty() {
                track.artist = artist.to_string();
            }
            out.push(track);
        }
        let Some(next) = page.get("next").and_then(Value::as_str) else { break };
        if next.is_empty() {
            break;
        }
        page = get_json(client, token, next).await?;
    }
    Ok(out)
}

async fn page_playlist(client: &reqwest::Client, token: &str, mut page: Value) -> Result<Vec<RemoteTrack>, String> {
    let mut out = Vec::new();
    for _ in 0..40 {
        let items = page.get("items").and_then(Value::as_array).cloned().unwrap_or_default();
        if items.is_empty() {
            break;
        }
        for item in items {
            let Some(track) = item.get("track") else { continue };
            if track.is_null() {
                continue;
            }
            out.push(track_from(track, ""));
        }
        let Some(next) = page.get("next").and_then(Value::as_str) else { break };
        if next.is_empty() {
            break;
        }
        page = get_json(client, token, next).await?;
    }
    Ok(out)
}

fn track_from(value: &Value, album_fallback: &str) -> RemoteTrack {
    let album = value.get("album").map(text_name).filter(|name| !name.is_empty()).unwrap_or_else(|| album_fallback.to_string());
    let cover = value.get("album").map(image).filter(|url| !url.is_empty()).unwrap_or_default();
    RemoteTrack {
        title: text(value, "name"),
        artist: artists(value),
        album,
        duration: value.get("duration_ms").and_then(Value::as_f64).map(|ms| ms / 1000.0),
        cover,
        number: value.get("track_number").and_then(Value::as_u64).unwrap_or(0) as u32,
    }
}

async fn search_one(bin: &Path, track: &RemoteTrack) -> Option<String> {
    let query = format!("{} {} audio", track.artist, track.title);
    let output = capture(bin, &["-J".into(), "--flat-playlist".into(), "--no-warnings".into(), "--skip-download".into(), format!("ytsearch8:{query}")]).await.ok()?;
    let value = bob_core::parse_json_value_text(&output).ok()?;
    let hits = parse_search(&value);
    let candidates: Vec<MatchCandidate> = hits
        .iter()
        .map(|hit| MatchCandidate {
            title: hit.title.clone(),
            channel: hit.channel.clone().unwrap_or_default(),
            duration: hit.duration,
        })
        .collect();
    let index = pick_match(&track.artist, &track.title, track.duration, &candidates)?;
    Some(hits[index].url.clone())
}

async fn get_json(client: &reqwest::Client, token: &str, url: &str) -> Result<Value, String> {
    client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())
}

fn text(value: &Value, key: &str) -> String {
    value.get(key).and_then(Value::as_str).unwrap_or("").to_string()
}

fn text_name(value: &Value) -> String {
    text(value, "name")
}

fn artists(value: &Value) -> String {
    value
        .get("artists")
        .and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|artist| artist.get("name").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_default()
}

fn image(value: &Value) -> String {
    value
        .get("images")
        .and_then(Value::as_array)
        .and_then(|list| list.first())
        .and_then(|image| image.get("url"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}
