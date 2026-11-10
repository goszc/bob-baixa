use crate::{FormatOption, MediaItem, PlaylistEntry, SearchHit};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub status: String,
    pub downloaded: u64,
    pub total: Option<u64>,
    pub speed: Option<f64>,
    pub eta: Option<f64>,
    pub fraction: f64,
}

pub fn extract_urls(text: &str) -> Vec<String> {
    let mut urls = Vec::new();
    for token in text.split_whitespace() {
        let token = token.trim_matches(|c: char| "<>\"'()[]{},;".contains(c));
        let url = if let Some(link) = crate::parse_spotify(token) {
            crate::canonical(&link)
        } else if token.starts_with("http://") || token.starts_with("https://") {
            token.trim_end_matches(['.', ',', ';', ')']).to_string()
        } else {
            continue;
        };
        if !urls.iter().any(|existing| existing == &url) {
            urls.push(url);
        }
    }
    urls
}

pub fn is_time(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() {
        return false;
    }
    if value.chars().all(|c| c.is_ascii_digit()) {
        return true;
    }
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 2 && parts.len() != 3 {
        return false;
    }
    parts.iter().enumerate().all(|(index, part)| {
        !part.is_empty()
            && part.chars().all(|c| c.is_ascii_digit())
            && part.len() <= if index == 0 { 4 } else { 2 }
    })
}

pub fn section_arg(start: &str, end: &str) -> Option<String> {
    let start = start.trim();
    let end = end.trim();
    if is_time(start) && is_time(end) {
        Some(format!("*{start}-{end}"))
    } else {
        None
    }
}

pub fn normalize_rate(raw: &str) -> Option<String> {
    let compact = raw.trim().to_ascii_lowercase().replace(' ', "");
    if compact.is_empty() {
        return None;
    }
    let (number, suffix) = if compact.ends_with(['k', 'm', 'g']) {
        (&compact[..compact.len() - 1], &compact[compact.len() - 1..])
    } else {
        (compact.as_str(), "")
    };
    if number.is_empty()
        || number.starts_with('.')
        || number.ends_with('.')
        || number.matches('.').count() > 1
        || !number.chars().all(|c| c.is_ascii_digit() || c == '.')
    {
        return None;
    }
    Some(format!("{number}{}", suffix.to_ascii_uppercase()))
}

pub fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(value) = u8::from_str_radix(
                std::str::from_utf8(&bytes[index + 1..index + 3]).unwrap_or(""),
                16,
            ) {
                out.push(value);
                index += 3;
                continue;
            }
        }
        out.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn is_clipboard_launch(arg: &str) -> bool {
    let arg = arg.trim().trim_matches('"').trim_end_matches('/');
    arg.eq_ignore_ascii_case("--clipboard") || arg.eq_ignore_ascii_case("bobbaixa://clipboard")
}

pub fn context_menu_label(language: &str) -> &'static str {
    if language == "en" {
        "download copied link with bob baixa"
    } else {
        "baixar link copiado com bob baixa"
    }
}

pub fn context_menu_command(exe: &str) -> String {
    format!("\"{}\" --clipboard", exe.trim().trim_matches('"'))
}

/// HKCU keys for the per-user context menu. The NSIS uninstaller deletes the same paths.
pub const CONTEXT_MENU_KEYS: &[&str] = &[
    r"Software\Classes\Directory\Background\shell\BobBaixa",
    r"Software\Classes\DesktopBackground\Shell\BobBaixa",
    r"Software\Classes\LibraryFolder\background\shell\BobBaixa",
];

pub fn url_from_link(arg: &str) -> Option<String> {
    let rest = arg.trim().trim_matches('"').strip_prefix("bobbaixa://")?;
    let (_, query) = rest.split_once('?')?;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("url=") {
            let url = percent_decode(value);
            if url.starts_with("http://") || url.starts_with("https://") {
                return Some(url);
            }
        }
    }
    None
}

pub fn parse_progress(line: &str) -> Option<Progress> {
    let rest = line.trim().strip_prefix("VIDIPROG\u{1f}")?;
    let parts: Vec<&str> = rest.split('\u{1f}').collect();
    if parts.len() < 6 {
        return None;
    }
    let status = parts[0].to_string();
    let downloaded = parse_u64(parts[1]).unwrap_or(0);
    let total = parse_u64(parts[2]).or_else(|| parse_u64(parts[3]));
    let speed = parse_f64(parts[4]);
    let eta = parse_f64(parts[5]);
    let fraction = if status == "finished" {
        1.0
    } else if let Some(total) = total.filter(|total| *total > 0) {
        (downloaded as f64 / total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };
    Some(Progress {
        status,
        downloaded,
        total,
        speed,
        eta,
        fraction,
    })
}

pub fn parse_output_path(line: &str) -> Option<String> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("VIDIPATH:") {
        let path = rest.trim().trim_matches('"');
        if !path.is_empty() && path != "NA" {
            return Some(path.to_string());
        }
    }
    const MARKERS: [&str; 3] = [
        "Destination: ",
        "Merging formats into \"",
        "[Merger] Merging formats into \"",
    ];
    for marker in MARKERS {
        if let Some(index) = line.find(marker) {
            let rest = line[index + marker.len()..]
                .trim()
                .trim_end_matches('"')
                .trim();
            if !rest.is_empty() {
                return Some(rest.to_string());
            }
        }
    }
    None
}

pub fn parse_title_line(line: &str) -> Option<String> {
    let title = line.trim().strip_prefix("VIDITITLE:")?.trim();
    if title.is_empty() || title == "NA" {
        None
    } else {
        Some(title.to_string())
    }
}

pub fn parse_ffmpeg_seconds(line: &str) -> Option<f64> {
    let line = line.trim();
    if let Some(value) = line.strip_prefix("out_time_us=") {
        return value.trim().parse::<f64>().ok().map(|n| n / 1_000_000.0);
    }
    if let Some(value) = line.strip_prefix("out_time_ms=") {
        return value.trim().parse::<f64>().ok().map(|n| n / 1_000_000.0);
    }
    None
}

pub fn search_target(query: &str) -> String {
    let clean = query.replace(['\n', '\r', '"'], " ");
    format!("ytsearch12:{clean}")
}

pub fn build_download_args(job: &crate::Job, ffmpeg_dir: &str) -> Vec<String> {
    let mut args = vec![
        "--newline".into(),
        "--no-colors".into(),
        "--encoding".into(),
        "utf-8".into(),
        "--windows-filenames".into(),
        "-c".into(),
        "--no-simulate".into(),
        "--no-quiet".into(),
        "--progress".into(),
        "--ffmpeg-location".into(),
        ffmpeg_dir.into(),
        "-P".into(),
        if job.output_dir.trim().is_empty() {
            ".".into()
        } else {
            job.output_dir.clone()
        },
        "-o".into(),
        if job.filename_template.trim().is_empty() {
            "%(title)s [%(id)s].%(ext)s".into()
        } else {
            job.filename_template.clone()
        },
        "--progress-template".into(),
        "download:VIDIPROG\u{1f}%(progress.status)s\u{1f}%(progress.downloaded_bytes)s\u{1f}%(progress.total_bytes)s\u{1f}%(progress.total_bytes_estimate)s\u{1f}%(progress.speed)s\u{1f}%(progress.eta)s".into(),
        "--print".into(),
        "before_dl:VIDITITLE:%(title)s".into(),
        "--print".into(),
        "after_move:VIDIPATH:%(filepath)s".into(),
    ];

    if job.audio_only {
        args.push("-f".into());
        args.push("ba/b".into());
        args.push("-x".into());
        args.push("--audio-format".into());
        let format = job.audio_format.clone().unwrap_or_else(|| "mp3".into());
        args.push(format.clone());
        if format != "flac" {
            if let Some(rate) = normalize_rate(&job.audio_bitrate) {
                args.push("--audio-quality".into());
                args.push(rate);
            }
        }
    } else if let Some(id) = job
        .format_id
        .as_deref()
        .filter(|id| !id.is_empty() && *id != "auto")
    {
        args.push("-f".into());
        args.push(format!("{id}+ba/{id}/b"));
        args.push("--merge-output-format".into());
        args.push(job.container.clone());
    } else {
        let (selector, merge) = format_selector(&job.quality, &job.container);
        args.push("-f".into());
        args.push(selector);
        if let Some(merge) = merge {
            args.push("--merge-output-format".into());
            args.push(merge);
        }
    }

    if let (Some(start), Some(end)) = (job.section_start.as_deref(), job.section_end.as_deref()) {
        if let Some(section) = section_arg(start, end) {
            args.push("--download-sections".into());
            args.push(section);
        }
    }
    if let Some(rate) = job.speed_limit.as_deref().and_then(normalize_rate) {
        args.push("--limit-rate".into());
        args.push(rate);
    }
    if job.subtitles {
        args.extend([
            "--write-subs".into(),
            "--write-auto-subs".into(),
            "--sub-langs".into(),
            if job.sub_langs.trim().is_empty() {
                "pt.*,en.*".into()
            } else {
                job.sub_langs.clone()
            },
        ]);
        if job.embed_subs {
            args.push("--embed-subs".into());
        }
    }
    if job.embed_thumbnail {
        args.extend([
            "--embed-thumbnail".into(),
            "--convert-thumbnails".into(),
            "jpg".into(),
        ]);
    }
    if job.embed_metadata {
        args.push("--embed-metadata".into());
    }
    if job.embed_chapters {
        args.push("--embed-chapters".into());
    }
    if job.sponsorblock {
        let cats = if job.sponsor_categories.is_empty() {
            "sponsor,intro,outro,selfpromo".into()
        } else {
            job.sponsor_categories.join(",")
        };
        args.push("--sponsorblock-remove".into());
        args.push(cats);
    }
    if !job.cookies_browser.is_empty()
        && job.cookies_browser != "none"
        && job.cookies_browser != "auto"
    {
        args.push("--cookies-from-browser".into());
        args.push(job.cookies_browser.clone());
    }
    args.push("--".into());
    args.push(job.url.clone());
    args
}

pub fn format_selector(quality: &str, container: &str) -> (String, Option<String>) {
    let height = match quality {
        "2160" => Some(2160),
        "1440" => Some(1440),
        "1080" => Some(1080),
        "720" => Some(720),
        "480" => Some(480),
        "360" => Some(360),
        _ => None,
    };
    let filter = height
        .map(|value| format!("[height<={value}]"))
        .unwrap_or_default();
    let selector = match container {
        "webm" => format!(
            "bv*{filter}[ext=webm]+ba[ext=webm]/b{filter}[ext=webm]/bv*{filter}+ba/b{filter}"
        ),
        "mp4" => {
            format!("bv*{filter}[ext=mp4]+ba[ext=m4a]/b{filter}[ext=mp4]/bv*{filter}+ba/b{filter}")
        }
        _ => format!("bv*{filter}+ba/b{filter}"),
    };
    let merge = if container == "mp4" || container == "mkv" || container == "webm" {
        Some(container.to_string())
    } else {
        None
    };
    (selector, merge)
}

pub fn ffmpeg_args(input: &str, output: &str, preset: &str, bitrate: &str) -> Vec<String> {
    let rate = normalize_rate(bitrate).unwrap_or_else(|| "192K".into());
    let mut args = vec![
        "-y".into(),
        "-i".into(),
        input.into(),
        "-progress".into(),
        "pipe:1".into(),
        "-nostats".into(),
    ];
    match preset {
        "mp3" => args.extend([
            "-vn".into(),
            "-c:a".into(),
            "libmp3lame".into(),
            "-b:a".into(),
            rate,
        ]),
        "m4a" => args.extend([
            "-vn".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            rate,
        ]),
        "opus" => args.extend([
            "-vn".into(),
            "-c:a".into(),
            "libopus".into(),
            "-b:a".into(),
            rate,
        ]),
        "flac" => args.extend(["-vn".into(), "-c:a".into(), "flac".into()]),
        "webm" => args.extend([
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-crf".into(),
            "36".into(),
            "-b:v".into(),
            "0".into(),
            "-c:a".into(),
            "libopus".into(),
            "-b:a".into(),
            "128K".into(),
        ]),
        _ => args.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            "28".into(),
            "-preset".into(),
            "medium".into(),
            "-c:a".into(),
            "aac".into(),
            "-b:a".into(),
            "128K".into(),
            "-movflags".into(),
            "+faststart".into(),
        ]),
    }
    args.push(output.into());
    args
}

pub fn output_for(input: &str, preset: &str, folder: &str) -> String {
    let name = input.rsplit(['/', '\\']).next().unwrap_or(input);
    let stem = name.rsplit_once('.').map(|(stem, _)| stem).unwrap_or(name);
    let ext = match preset {
        "mp3" | "m4a" | "opus" | "flac" | "webm" => preset,
        _ => "mp4",
    };
    let dir = if folder.trim().is_empty() {
        "."
    } else {
        folder.trim()
    };
    format!("{dir}/{stem}.{ext}")
}

pub fn parse_media(value: &Value, source_url: &str) -> MediaItem {
    if value.get("entries").and_then(Value::as_array).is_some() {
        return playlist_item(value, source_url);
    }
    video_item(value, source_url)
}

pub fn parse_search(value: &Value) -> Vec<SearchHit> {
    value
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| search_hit(entry, ""))
        .collect()
}

fn playlist_item(value: &Value, source_url: &str) -> MediaItem {
    let entries = value
        .get("entries")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| playlist_entry(entry, source_url))
        .collect::<Vec<_>>();
    MediaItem {
        kind: "playlist".into(),
        id: string(value, "id").unwrap_or_else(|| format!("pl-{}", source_url.len())),
        url: source_url.into(),
        title: string(value, "title").unwrap_or_else(|| "playlist".into()),
        channel: string(value, "channel").or_else(|| string(value, "uploader")),
        duration: None,
        thumbnail: thumb(value),
        view_count: None,
        formats: Vec::new(),
        entries,
        unmatched: Vec::new(),
    }
}

fn video_item(value: &Value, source_url: &str) -> MediaItem {
    MediaItem {
        kind: "video".into(),
        id: string(value, "id").unwrap_or_else(|| "video".into()),
        url: string(value, "webpage_url").unwrap_or_else(|| source_url.into()),
        title: string(value, "title").unwrap_or_else(|| source_url.into()),
        channel: string(value, "channel").or_else(|| string(value, "uploader")),
        duration: value.get("duration").and_then(Value::as_f64),
        thumbnail: thumb(value),
        view_count: value.get("view_count").and_then(Value::as_u64),
        formats: formats_of(value),
        entries: Vec::new(),
        unmatched: Vec::new(),
    }
}

fn playlist_entry(entry: &Value, source_url: &str) -> Option<PlaylistEntry> {
    let id = string(entry, "id").unwrap_or_default();
    let url = entry_url(entry, source_url, &id);
    if url.is_empty() {
        return None;
    }
    Some(PlaylistEntry {
        id: if id.is_empty() { url.clone() } else { id },
        title: string(entry, "title").unwrap_or_else(|| url.clone()),
        url,
        duration: entry.get("duration").and_then(Value::as_f64),
        thumbnail: thumb(entry),
        artist: String::new(),
        album: String::new(),
        track: 0,
        cover: String::new(),
    })
}

fn search_hit(entry: &Value, source_url: &str) -> Option<SearchHit> {
    let item = playlist_entry(entry, source_url)?;
    Some(SearchHit {
        id: item.id,
        title: item.title,
        url: item.url,
        channel: string(entry, "channel").or_else(|| string(entry, "uploader")),
        duration: item.duration,
        thumbnail: item.thumbnail,
    })
}

fn formats_of(value: &Value) -> Vec<FormatOption> {
    let mut formats = Vec::new();
    let Some(list) = value.get("formats").and_then(Value::as_array) else {
        return formats;
    };
    for format in list {
        let note = string(format, "format_note").unwrap_or_default();
        if note.to_ascii_lowercase().contains("storyboard") {
            continue;
        }
        let height = format.get("height").and_then(Value::as_u64).unwrap_or(0);
        if height == 0 {
            continue;
        }
        let vcodec = string(format, "vcodec").unwrap_or_else(|| "none".into());
        if vcodec == "none" {
            continue;
        }
        let ext = string(format, "ext").unwrap_or_default();
        let id = string(format, "format_id").unwrap_or_default();
        if id.is_empty() {
            continue;
        }
        let label = format!("{height}p · {ext}");
        if formats
            .iter()
            .any(|item: &FormatOption| item.label == label)
        {
            continue;
        }
        formats.push(FormatOption {
            id,
            label,
            height: Some(height),
            ext,
            fps: format.get("fps").and_then(Value::as_f64),
        });
    }
    formats.sort_by(|a, b| b.height.cmp(&a.height));
    formats.truncate(8);
    formats
}

fn entry_url(entry: &Value, source_url: &str, id: &str) -> String {
    for key in ["webpage_url", "url"] {
        if let Some(url) = string(entry, key) {
            if url.starts_with("http://") || url.starts_with("https://") {
                return url;
            }
        }
    }
    if !id.is_empty()
        && (source_url.contains("youtu") || string(entry, "ie_key").as_deref() == Some("Youtube"))
    {
        return format!("https://www.youtube.com/watch?v={id}");
    }
    string(entry, "url").unwrap_or_default()
}

fn thumb(value: &Value) -> Option<String> {
    let mut best_width = 0u64;
    let mut best_url = None;
    if let Some(items) = value.get("thumbnails").and_then(Value::as_array) {
        for item in items {
            let Some(url) = string(item, "url").filter(|url| url.starts_with("http")) else {
                continue;
            };
            let width = item.get("width").and_then(Value::as_u64).unwrap_or(0);
            if best_url.is_none() || width >= best_width {
                best_width = width;
                best_url = Some(url);
            }
        }
    }
    if let Some(url) = best_url {
        return Some(url);
    }
    if let Some(url) = string(value, "thumbnail").filter(|url| url.starts_with("http")) {
        return Some(url);
    }
    youtube_id(string(value, "id").as_deref())
        .or_else(|| youtube_id(string(value, "url").as_deref()))
        .or_else(|| youtube_id(string(value, "webpage_url").as_deref()))
        .map(|id| format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
}

fn youtube_id(value: Option<&str>) -> Option<String> {
    let value = value?.trim();
    if value.len() == 11
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Some(value.to_string());
    }
    for marker in ["v=", "youtu.be/", "/shorts/", "/embed/", "/vi/"] {
        if let Some(start) = value.find(marker) {
            let id = &value[start + marker.len()..];
            let id: String = id.chars().take(11).collect();
            if id.len() == 11
                && id
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Some(id);
            }
        }
    }
    None
}

pub fn parse_json_value_text(text: &str) -> Result<Value, String> {
    let start = text.find(['{', '[']).ok_or_else(|| {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            "resposta vazia".into()
        } else {
            trimmed.chars().take(500).collect::<String>()
        }
    })?;
    serde_json::from_str(&text[start..]).map_err(|error| error.to_string())
}

pub fn looks_like_auth(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    [
        "sign in",
        "confirm your age",
        "age-restricted",
        "age restricted",
        "login required",
        "log in",
        "private video",
        "members-only",
        "use --cookies",
        "cookies-from-browser",
        "this video is unavailable",
        "requested content is not available",
    ]
    .iter()
    .any(|needle| text.contains(needle))
}

pub fn looks_like_cookie_lock(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    text.contains("database is locked")
        || text.contains("could not copy")
        || text.contains("cookie database")
        || text.contains("failed to decrypt")
        || (text.contains("cookie") && text.contains("locked"))
}

pub fn looks_like_extractor(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    text.contains("unable to extract")
        || text.contains("unsupported url")
        || text.contains("no video formats")
        || text.contains("not a bot")
        || text.contains("nsig")
        || text.contains("signature extraction failed")
        || text.contains("http error 403")
}

/// yt-dlp downloads captions before the video and does not retry HTTP 429.
/// This many attempts still include subtitles. After that, the file is saved without them.
pub const SUBTITLE_RATE_LIMIT_ATTEMPTS: u32 = 3;

pub fn subtitle_rate_limited(message: &str) -> bool {
    let text = message.to_ascii_lowercase();
    let captions = text.contains("subtitle");
    let limited = text.contains("429") || text.contains("too many requests");
    captions && limited
}

/// True only when every `ERROR:` line is a subtitle 429. A video or extractor error still fails the download.
pub fn subtitle_rate_limit_only(log: &str) -> bool {
    let mut saw = false;
    for line in log.lines() {
        let trimmed = line.trim();
        let lower = trimmed.to_ascii_lowercase();
        if !lower.starts_with("error:") {
            continue;
        }
        if subtitle_rate_limited(trimmed) {
            saw = true;
        } else {
            return false;
        }
    }
    saw
}

/// Seconds to wait before retry `attempt` (1, 2, 3…): 2s, 4s, 8s.
pub fn subtitle_retry_delay_secs(attempt: u32) -> u64 {
    2u64.saturating_pow(attempt.clamp(1, 5))
}

pub fn subtitle_skip_warning(language: &str) -> String {
    crate::text(
        language,
        "o arquivo foi baixado, mas as legendas não: o youtube limitou os pedidos (429).",
        "the file was downloaded, but the subtitles were not: youtube rate-limited the request (429).",
    )
}

pub fn cookie_lock_hint(language: &str) -> String {
    crate::text(
        language,
        "feche o navegador e tente de novo.",
        "close the browser and try again.",
    )
}

pub fn pick_browser(preference: &str, detected: &[String]) -> Option<String> {
    match preference {
        "" | "none" => None,
        "auto" => detected.first().cloned(),
        other => Some(other.to_string()),
    }
}

pub fn detect_browsers() -> Vec<String> {
    let local = std::env::var_os("LOCALAPPDATA").map(std::path::PathBuf::from);
    let roaming = std::env::var_os("APPDATA").map(std::path::PathBuf::from);
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(std::path::PathBuf::from);
    detect_browsers_in(local.as_deref(), roaming.as_deref(), home.as_deref())
}

pub fn detect_browsers_in(
    local: Option<&std::path::Path>,
    roaming: Option<&std::path::Path>,
    home: Option<&std::path::Path>,
) -> Vec<String> {
    let mut found = Vec::new();
    let mut mark = |id: &str, path: std::path::PathBuf| {
        if path.is_dir() && !found.iter().any(|item: &String| item == id) {
            found.push(id.to_string());
        }
    };
    if let Some(local) = local {
        mark("chrome", local.join("Google/Chrome/User Data"));
        mark("edge", local.join("Microsoft/Edge/User Data"));
        mark("brave", local.join("BraveSoftware/Brave-Browser/User Data"));
        mark("vivaldi", local.join("Vivaldi/User Data"));
        mark("chromium", local.join("Chromium/User Data"));
        mark("opera", local.join("Opera Software/Opera Stable"));
    }
    if let Some(roaming) = roaming {
        mark("firefox", roaming.join("Mozilla/Firefox/Profiles"));
        mark("firefox", roaming.join("Mozilla/Firefox"));
        mark("opera", roaming.join("Opera Software/Opera Stable"));
    }
    if let Some(home) = home {
        let config = home.join(".config");
        mark("chrome", config.join("google-chrome"));
        mark("edge", config.join("microsoft-edge"));
        mark("brave", config.join("BraveSoftware/Brave-Browser"));
        mark("vivaldi", config.join("vivaldi"));
        mark("chromium", config.join("chromium"));
        mark("opera", config.join("opera"));
        mark("firefox", home.join(".mozilla/firefox"));
    }
    [
        "chrome", "edge", "brave", "firefox", "opera", "vivaldi", "chromium",
    ]
    .into_iter()
    .filter(|id| found.iter().any(|item| item == id))
    .map(str::to_string)
    .collect()
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

fn parse_u64(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() || value == "NA" || value == "None" {
        return None;
    }
    value.parse::<f64>().ok().map(|n| n as u64)
}

fn parse_f64(value: &str) -> Option<f64> {
    let value = value.trim();
    if value.is_empty() || value == "NA" || value == "None" {
        return None;
    }
    value
        .parse::<f64>()
        .ok()
        .filter(|n| n.is_finite() && *n >= 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Config, EnqueueItem, Job};

    fn sample_job() -> Job {
        Job::from_item(
            EnqueueItem {
                url: "https://example.com/v".into(),
                title: Some("teste".into()),
                section_start: Some("1:20".into()),
                section_end: Some("3:45".into()),
                speed_limit: Some("2m".into()),
                ..EnqueueItem::default()
            },
            &Config {
                audio_only: false,
                quality: "1080".into(),
                container: "mp4".into(),
                subtitles: true,
                embed_subs: true,
                sub_langs: "pt.*,en.*".into(),
                embed_thumbnail: true,
                embed_metadata: true,
                embed_chapters: true,
                sponsorblock: true,
                sponsor_categories: vec!["sponsor".into(), "intro".into()],
                cookies_browser: "chrome".into(),
                output_dir: "C:/Downloads/bob baixa".into(),
                ..Config::default()
            },
            None,
        )
    }

    #[test]
    fn extracts_many_urls() {
        let urls =
            extract_urls("veja https://youtu.be/a, e\nhttps://youtu.be/a https://example.com/b.");
        assert_eq!(
            urls,
            vec![
                "https://youtu.be/a".to_string(),
                "https://example.com/b".to_string()
            ]
        );
    }

    #[test]
    fn accepts_clip_times() {
        assert!(is_time("1:20"));
        assert!(is_time("1:02:03"));
        assert!(is_time("90"));
        assert!(!is_time("1h20"));
        assert_eq!(section_arg("1:20", "3:45").as_deref(), Some("*1:20-3:45"));
    }

    #[test]
    fn rates_and_deep_link() {
        assert_eq!(normalize_rate("2m").as_deref(), Some("2M"));
        assert_eq!(normalize_rate("192k").as_deref(), Some("192K"));
        assert!(normalize_rate("rápido").is_none());
        let url = url_from_link("bobbaixa://download?url=https%3A%2F%2Fyoutu.be%2Fabc").unwrap();
        assert_eq!(url, "https://youtu.be/abc");
        assert!(is_clipboard_launch("--clipboard"));
        assert!(is_clipboard_launch("bobbaixa://clipboard/"));
        assert!(!is_clipboard_launch(
            "bobbaixa://download?url=https%3A%2F%2Fyoutu.be%2Fabc"
        ));
        assert_eq!(
            context_menu_label("pt"),
            "baixar link copiado com bob baixa"
        );
        assert_eq!(
            context_menu_command(r"C:\Users\ana\AppData\Local\bob baixa\bob-baixa.exe"),
            r#""C:\Users\ana\AppData\Local\bob baixa\bob-baixa.exe" --clipboard"#
        );
        assert_eq!(CONTEXT_MENU_KEYS.len(), 3);
    }

    #[test]
    fn parses_progress_and_paths() {
        let line = "VIDIPROG\u{1f}downloading\u{1f}500\u{1f}1000\u{1f}NA\u{1f}2048\u{1f}12";
        let progress = parse_progress(line).unwrap();
        assert_eq!(progress.downloaded, 500);
        assert_eq!(progress.total, Some(1000));
        assert!((progress.fraction - 0.5).abs() < 0.001);
        assert_eq!(
            parse_output_path("VIDIPATH:C:/v.mp4").as_deref(),
            Some("C:/v.mp4")
        );
        assert_eq!(
            parse_output_path("[Merger] Merging formats into \"C:/out.mkv\"").as_deref(),
            Some("C:/out.mkv")
        );
        assert_eq!(parse_ffmpeg_seconds("out_time_us=2000000"), Some(2.0));
    }

    #[test]
    fn download_args_cover_the_main_flows() {
        let job = sample_job();
        let args = build_download_args(&job, "C:/tools");
        let joined = args.join(" ");
        assert!(joined.contains("--download-sections *1:20-3:45"));
        assert!(joined.contains("--limit-rate 2M"));
        assert!(joined.contains("--merge-output-format mp4"));
        assert!(joined.contains("height<=1080"));
        assert!(joined.contains("--write-subs"));
        assert!(joined.contains("--embed-subs"));
        assert!(joined.contains("--sponsorblock-remove sponsor,intro"));
        assert!(joined.contains("--cookies-from-browser chrome"));
        assert!(joined.contains("--embed-thumbnail"));
        assert!(joined.contains("--embed-chapters"));
        assert!(args.last().unwrap().ends_with("https://example.com/v"));

        let mut audio = sample_job();
        audio.audio_only = true;
        audio.audio_format = Some("mp3".into());
        audio.audio_bitrate = "192k".into();
        audio.section_start = None;
        audio.section_end = None;
        audio.speed_limit = None;
        let audio_args = build_download_args(&audio, "C:/tools").join(" ");
        assert!(audio_args.contains("--audio-format mp3"));
        assert!(audio_args.contains("--audio-quality 192K"));
        assert!(audio_args.contains("-x"));

        let mut bare = sample_job();
        bare.subtitles = false;
        let bare_args = build_download_args(&bare, "C:/tools").join(" ");
        assert!(!bare_args.contains("--write-subs"));
        assert!(!bare_args.contains("--embed-subs"));
    }

    #[test]
    fn subtitle_429_retries_then_skips_without_hiding_other_errors() {
        let only = "ERROR: Unable to download video subtitles for 'pt': HTTP Error 429: Too Many Requests\n";
        assert!(subtitle_rate_limited(only));
        assert!(subtitle_rate_limit_only(only));
        assert!(subtitle_rate_limit_only(
            "WARNING: something else\nERROR: Unable to download video subtitles for 'en': HTTP Error 429: Too Many Requests\n"
        ));
        assert!(!subtitle_rate_limit_only(
            "ERROR: Unable to download video subtitles for 'pt': HTTP Error 429: Too Many Requests\nERROR: Unable to download video data: HTTP Error 403: Forbidden\n"
        ));
        assert!(!subtitle_rate_limit_only(
            "ERROR: HTTP Error 429: Too Many Requests\n"
        ));
        assert!(!subtitle_rate_limit_only("ERROR: Sign in to confirm your age\n"));
        assert!(!subtitle_rate_limit_only(""));
        assert!(!looks_like_extractor(only));
        assert!(looks_like_extractor(
            "ERROR: HTTP Error 403: Forbidden\n"
        ));
        assert_eq!(SUBTITLE_RATE_LIMIT_ATTEMPTS, 3);
        assert_eq!(subtitle_retry_delay_secs(1), 2);
        assert_eq!(subtitle_retry_delay_secs(2), 4);
        assert_eq!(subtitle_retry_delay_secs(3), 8);
        let warning = subtitle_skip_warning("pt");
        assert!(warning.contains("429"));
        assert!(warning.contains("legendas"));
        assert!(!subtitle_skip_warning("en").contains("legendas"));
    }

    #[test]
    fn parses_video_playlist_and_search() {
        let video = serde_json::json!({
            "id": "abc",
            "title": "Lisboa",
            "webpage_url": "https://youtu.be/abc",
            "duration": 12.0,
            "channel": "arquivo",
            "thumbnail": "https://img/a.jpg",
            "formats": [
                {"format_id": "18", "ext": "mp4", "height": 360, "vcodec": "avc1", "format_note": "360p"},
                {"format_id": "sb", "ext": "mhtml", "height": 0, "vcodec": "none", "format_note": "storyboard"}
            ]
        });
        let item = parse_media(&video, "https://youtu.be/abc");
        assert_eq!(item.kind, "video");
        assert_eq!(item.formats.len(), 1);
        assert_eq!(item.formats[0].label, "360p · mp4");

        let playlist = serde_json::json!({
            "id": "PL1",
            "title": "lista",
            "entries": [
                {"id": "a", "title": "um", "url": "a"},
                {"id": "b", "title": "dois", "webpage_url": "https://youtu.be/b"}
            ]
        });
        let list = parse_media(&playlist, "https://youtube.com/playlist?list=PL1");
        assert_eq!(list.kind, "playlist");
        assert_eq!(list.entries[0].url, "https://www.youtube.com/watch?v=a");
        assert_eq!(list.entries[1].url, "https://youtu.be/b");

        let hits = parse_search(&playlist);
        assert_eq!(hits.len(), 2);
    }

    #[test]
    fn playlist_entries_keep_thumbnails_and_cookies_retry_rules() {
        let playlist = serde_json::json!({
            "id": "PL1",
            "title": "lista",
            "entries": [{
                "id": "YF4wa9gB0cA",
                "title": "lisboa",
                "thumbnails": [
                    {"url": "https://i.ytimg.com/vi/YF4wa9gB0cA/default.jpg", "width": 120},
                    {"url": "https://i.ytimg.com/vi/YF4wa9gB0cA/hqdefault.jpg", "width": 480}
                ]
            }]
        });
        let list = parse_media(&playlist, "https://www.youtube.com/playlist?list=PL1");
        assert_eq!(
            list.entries[0].thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/YF4wa9gB0cA/hqdefault.jpg")
        );
        let bare =
            serde_json::json!({"id": "dQw4w9WgXcQ", "title": "sem capa", "url": "dQw4w9WgXcQ"});
        let hit = parse_search(&serde_json::json!({"entries": [bare]}));
        assert_eq!(
            hit[0].thumbnail.as_deref(),
            Some("https://i.ytimg.com/vi/dQw4w9WgXcQ/hqdefault.jpg")
        );
        assert!(looks_like_auth("ERROR: Sign in to confirm your age"));
        assert!(looks_like_extractor(
            "ERROR: unable to extract player response"
        ));
        assert!(looks_like_cookie_lock(
            "failed to copy cookie database: database is locked"
        ));
        assert_eq!(
            pick_browser("auto", &["edge".into()]).as_deref(),
            Some("edge")
        );
        assert_eq!(pick_browser("none", &["chrome".into()]), None);
        let mut job = sample_job();
        job.cookies_browser = "auto".into();
        let args = build_download_args(&job, "C:/tools").join(" ");
        assert!(!args.contains("--cookies-from-browser"));
        job.cookies_browser = "brave".into();
        let args = build_download_args(&job, "C:/tools").join(" ");
        assert!(args.contains("--cookies-from-browser brave"));
    }

    #[test]
    fn converter_presets() {
        let mp3 = ffmpeg_args("in.mp4", "out.mp3", "mp3", "320k");
        assert!(mp3.contains(&"libmp3lame".into()));
        assert!(mp3.contains(&"320K".into()));
        let mp4 = ffmpeg_args("in.mkv", "out.mp4", "mp4", "128k").join(" ");
        assert!(mp4.contains("libx264"));
        assert!(mp4.contains("-crf 28"));
        assert_eq!(
            output_for("C:/Videos/ensaio.mkv", "mp3", "C:/out"),
            "C:/out/ensaio.mp3"
        );
    }
}
