use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub language: String,
    pub theme: String,
    #[serde(default)]
    pub theme_glass: bool,
    pub output_dir: String,
    pub filename_template: String,
    #[serde(default = "default_parallel")]
    pub parallel: u32,
    pub global_speed: String,
    pub clipboard_watch: bool,
    pub minimize_to_tray: bool,
    pub notifications: bool,
    pub hotkey: bool,
    pub quality: String,
    pub container: String,
    pub audio_only: bool,
    pub audio_format: String,
    pub audio_bitrate: String,
    pub subtitles: bool,
    pub embed_subs: bool,
    pub sub_langs: String,
    pub embed_thumbnail: bool,
    pub embed_metadata: bool,
    pub embed_chapters: bool,
    pub sponsorblock: bool,
    pub sponsor_categories: Vec<String>,
    pub cookies_browser: String,
    pub download_known_on_first_watch: bool,
    #[serde(default = "default_true")]
    pub auto_update: bool,
    #[serde(default)]
    pub update_url: String,
    /// Windows per-user context menu. Off until the user turns it on.
    #[serde(default)]
    pub context_menu: bool,
    #[serde(default)]
    pub presets: Vec<DownloadPreset>,
    /// Last version whose "o que há de novo" dialog was dismissed.
    #[serde(default)]
    pub seen_version: String,
    /// AcoustID application key. Empty skips the fingerprint and uses text search.
    #[serde(default)]
    pub acoustid_key: String,
    /// Lyrics translation target. `pt` means pt-BR.
    #[serde(default = "default_translate_lang")]
    pub translate_lang: String,
    /// After a download, fetch manual (not auto) subtitles and keep a `.lrc` when they look like lyrics.
    #[serde(default = "default_true")]
    pub grab_lyrics: bool,
    /// After a successful Drive upload, delete the local copy.
    #[serde(default)]
    pub drive_only: bool,
    #[serde(default = "default_hotkey_play")]
    pub hotkey_play: String,
    #[serde(default = "default_hotkey_next")]
    pub hotkey_next: String,
    #[serde(default = "default_hotkey_prev")]
    pub hotkey_prev: String,
    #[serde(default = "default_hotkey_like")]
    pub hotkey_like: String,
    /// User Gemini key. Empty keeps the non-AI heuristics.
    #[serde(default)]
    pub gemini_key: String,
    #[serde(default = "default_flash")]
    pub gemini_flash: String,
    #[serde(default = "default_flash_fallback")]
    pub gemini_flash_fallback: String,
    #[serde(default = "default_bulk")]
    pub gemini_bulk: String,
    #[serde(default = "default_embed")]
    pub gemini_embed: String,
    #[serde(default = "default_transcribe")]
    pub gemini_transcribe: String,
    #[serde(default)]
    pub gemini_auto_lyrics: bool,
    #[serde(default = "default_lyrics_budget")]
    pub gemini_lyrics_budget: u32,
    #[serde(default)]
    pub discord: bool,
    #[serde(default)]
    pub discord_client_id: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            language: "pt".into(),
            theme: "system".into(),
            theme_glass: false,
            output_dir: String::new(),
            filename_template: "%(title)s [%(id)s].%(ext)s".into(),
            parallel: 2,
            global_speed: String::new(),
            clipboard_watch: true,
            minimize_to_tray: true,
            notifications: true,
            hotkey: true,
            quality: "1080".into(),
            container: "mp4".into(),
            audio_only: false,
            audio_format: "mp3".into(),
            audio_bitrate: "192k".into(),
            subtitles: false,
            embed_subs: true,
            sub_langs: "pt.*,en.*".into(),
            embed_thumbnail: true,
            embed_metadata: true,
            embed_chapters: true,
            sponsorblock: false,
            sponsor_categories: vec![
                "sponsor".into(),
                "intro".into(),
                "outro".into(),
                "selfpromo".into(),
            ],
            cookies_browser: "auto".into(),
            download_known_on_first_watch: false,
            auto_update: true,
            update_url: String::new(),
            context_menu: false,
            presets: Vec::new(),
            seen_version: String::new(),
            acoustid_key: String::new(),
            translate_lang: default_translate_lang(),
            grab_lyrics: true,
            drive_only: false,
            hotkey_play: default_hotkey_play(),
            hotkey_next: default_hotkey_next(),
            hotkey_prev: default_hotkey_prev(),
            hotkey_like: default_hotkey_like(),
            gemini_key: String::new(),
            gemini_flash: default_flash(),
            gemini_flash_fallback: default_flash_fallback(),
            gemini_bulk: default_bulk(),
            gemini_embed: default_embed(),
            gemini_transcribe: default_transcribe(),
            gemini_auto_lyrics: false,
            gemini_lyrics_budget: default_lyrics_budget(),
            discord: false,
            discord_client_id: String::new(),
        }
    }
}

fn default_flash() -> String {
    "gemini-3.5-flash-lite".into()
}

fn default_flash_fallback() -> String {
    "gemini-3.1-flash-lite".into()
}

fn default_bulk() -> String {
    "gemma-4-31b-it".into()
}

fn default_embed() -> String {
    "gemini-embedding-2".into()
}

fn default_transcribe() -> String {
    "gemini-3.5-transcribe".into()
}

fn default_lyrics_budget() -> u32 {
    5
}

fn default_translate_lang() -> String {
    "pt".into()
}

fn default_hotkey_play() -> String {
    "ctrl+alt+p".into()
}

fn default_hotkey_next() -> String {
    "ctrl+alt+right".into()
}

fn default_hotkey_prev() -> String {
    "ctrl+alt+left".into()
}

fn default_hotkey_like() -> String {
    "ctrl+alt+l".into()
}

fn default_true() -> bool {
    true
}

fn default_parallel() -> u32 {
    2
}

fn default_bitrate() -> String {
    "192k".into()
}

fn default_sub_langs() -> String {
    "pt.*,en.*".into()
}

pub const PARALLEL_MIN: u32 = 1;
pub const PARALLEL_MAX: u32 = 4;

fn chosen_text(override_value: Option<String>, fallback: &str) -> String {
    override_value
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

pub fn clamp_parallel(value: u32) -> u32 {
    value.clamp(PARALLEL_MIN, PARALLEL_MAX)
}

/// Jobs are stored newest-first. Claim the oldest queued ones that still fit.
pub fn claim_downloads(jobs: &mut [Job], parallel: u32) -> Vec<String> {
    let mut left = clamp_parallel(parallel) as usize;
    left = left.saturating_sub(jobs.iter().filter(|job| job.state == "downloading").count());
    let mut ids = Vec::new();
    for job in jobs.iter_mut().rev() {
        if left == 0 {
            break;
        }
        if job.state == "queued" {
            if job
                .retry_at
                .as_deref()
                .map(iso_in_future)
                .unwrap_or(false)
            {
                continue;
            }
            job.state = "downloading".into();
            job.error = None;
            job.warning = None;
            job.retry_at = None;
            ids.push(job.id.clone());
            left -= 1;
        }
    }
    ids
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadPreset {
    pub id: String,
    pub name: String,
    pub quality: String,
    pub container: String,
    pub audio_only: bool,
    pub audio_format: String,
    #[serde(default = "default_bitrate")]
    pub audio_bitrate: String,
    #[serde(default)]
    pub subtitles: bool,
    #[serde(default = "default_true")]
    pub embed_subs: bool,
    #[serde(default = "default_sub_langs")]
    pub sub_langs: String,
    #[serde(default = "default_true")]
    pub embed_thumbnail: bool,
    #[serde(default = "default_true")]
    pub embed_metadata: bool,
    #[serde(default = "default_true")]
    pub embed_chapters: bool,
    #[serde(default)]
    pub sponsorblock: bool,
    #[serde(default)]
    pub sponsor_categories: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnqueueItem {
    pub url: String,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    #[serde(default)]
    pub format_id: Option<String>,
    #[serde(default)]
    pub section_start: Option<String>,
    #[serde(default)]
    pub section_end: Option<String>,
    #[serde(default)]
    pub speed_limit: Option<String>,
    #[serde(default)]
    pub scheduled_at: Option<String>,
    /// Search can ask for mp3 or mp4 without changing the saved settings.
    #[serde(default)]
    pub audio_only: Option<bool>,
    #[serde(default)]
    pub container: Option<String>,
    #[serde(default)]
    pub audio_format: Option<String>,
    /// Overrides the saved bitrate for this item. Used by modo econômico.
    #[serde(default)]
    pub audio_bitrate: Option<String>,
    /// When set, the file is saved under `outputDir/<sanitized title>`.
    #[serde(default)]
    pub playlist_title: Option<String>,
    #[serde(default)]
    pub tag_artist: Option<String>,
    #[serde(default)]
    pub tag_album: Option<String>,
    #[serde(default)]
    pub tag_track: Option<u32>,
    #[serde(default)]
    pub tag_cover: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    pub state: String,
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eta: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloaded: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Shown when the file is ready but something optional, such as subtitles, was skipped.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_path: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scheduled_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_end: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speed_limit: Option<String>,
    pub audio_only: bool,
    pub quality: String,
    pub container: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio_format: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format_id: Option<String>,
    #[serde(default)]
    pub audio_bitrate: String,
    #[serde(default)]
    pub subtitles: bool,
    #[serde(default)]
    pub embed_subs: bool,
    #[serde(default)]
    pub sub_langs: String,
    #[serde(default)]
    pub embed_thumbnail: bool,
    #[serde(default)]
    pub embed_metadata: bool,
    #[serde(default)]
    pub embed_chapters: bool,
    #[serde(default)]
    pub sponsorblock: bool,
    #[serde(default)]
    pub sponsor_categories: Vec<String>,
    #[serde(default)]
    pub cookies_browser: String,
    #[serde(default)]
    pub cookie_retried: bool,
    #[serde(default)]
    pub update_retried: bool,
    #[serde(default)]
    pub output_dir: String,
    #[serde(default)]
    pub filename_template: String,
    /// Transient failures (network, HTTP 429). The queue stays on disk and waits until `retry_at`.
    #[serde(default)]
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub at: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watch {
    pub id: String,
    pub url: String,
    pub title: String,
    pub every_minutes: u32,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<String>,
    pub known: u32,
    #[serde(default)]
    pub seen: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolStatus {
    pub ytdlp: String,
    pub ffmpeg: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ytdlp_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ffmpeg_version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub progress: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
}

impl Default for ToolStatus {
    fn default() -> Self {
        Self {
            ytdlp: "missing".into(),
            ffmpeg: "missing".into(),
            ytdlp_version: None,
            ffmpeg_version: None,
            message: None,
            progress: None,
            app_version: Some(env!("CARGO_PKG_VERSION").into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FormatOption {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u64>,
    pub ext: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fps: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistEntry {
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub track: u32,
    #[serde(default)]
    pub cover: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaItem {
    pub kind: String,
    pub id: String,
    pub url: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view_count: Option<u64>,
    pub formats: Vec<FormatOption>,
    pub entries: Vec<PlaylistEntry>,
    #[serde(default)]
    pub unmatched: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub id: String,
    pub title: String,
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thumbnail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConvertJob {
    pub id: String,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    pub preset: String,
    pub state: String,
    pub progress: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum BobEvent {
    Jobs { jobs: Vec<Job> },
    Tools { tools: ToolStatus },
    History { history: Vec<HistoryEntry> },
    Watches { watches: Vec<Watch> },
    Clipboard { url: String },
    Toast { message: String },
    Convert { job: ConvertJob },
    Schedule { at: Option<String> },
    Update { version: String },
    /// Batch Opus conversion. `state` is `running`, `done`, `failed`, or `cancelled`.
    Economic {
        done: u32,
        total: u32,
        current: String,
        state: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<String>,
    },
}

pub fn text(language: &str, pt: &str, en: &str) -> String {
    if language == "en" {
        en.to_string()
    } else {
        pt.to_string()
    }
}

pub fn new_id(prefix: &str) -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(1);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{millis:x}-{n}")
}

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

pub fn iso_in_future(value: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|time| time > chrono::Utc::now())
        .unwrap_or(false)
}

impl Job {
    pub fn from_item(item: EnqueueItem, config: &Config, hold_until: Option<&str>) -> Self {
        let waiting = item
            .scheduled_at
            .as_deref()
            .map(iso_in_future)
            .unwrap_or(false)
            || hold_until.map(iso_in_future).unwrap_or(false);
        let scheduled_at = item
            .scheduled_at
            .clone()
            .or_else(|| hold_until.filter(|_| waiting).map(str::to_string));
        let title = item
            .title
            .filter(|title| !title.trim().is_empty())
            .unwrap_or_else(|| item.url.clone());
        Self {
            id: new_id("job"),
            url: item.url,
            title,
            thumbnail: item.thumbnail,
            duration: item.duration,
            state: if waiting { "scheduled" } else { "queued" }.into(),
            progress: 0.0,
            speed: None,
            eta: None,
            downloaded: None,
            total: None,
            error: None,
            warning: None,
            output_path: None,
            created_at: now_iso(),
            scheduled_at,
            section_start: item.section_start,
            section_end: item.section_end,
            speed_limit: item
                .speed_limit
                .filter(|value| !value.trim().is_empty())
                .or_else(|| {
                    let global = config.global_speed.trim();
                    if global.is_empty() {
                        None
                    } else {
                        Some(global.to_string())
                    }
                }),
            audio_only: item.audio_only.unwrap_or(config.audio_only),
            quality: config.quality.clone(),
            container: chosen_text(item.container.clone(), &config.container),
            audio_format: Some(chosen_text(item.audio_format.clone(), &config.audio_format)),
            format_id: item
                .format_id
                .filter(|value| !value.is_empty() && value != "auto"),
            audio_bitrate: item
                .audio_bitrate
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| config.audio_bitrate.clone()),
            subtitles: config.subtitles,
            embed_subs: config.embed_subs,
            sub_langs: config.sub_langs.clone(),
            embed_thumbnail: config.embed_thumbnail,
            embed_metadata: config.embed_metadata,
            embed_chapters: config.embed_chapters,
            sponsorblock: config.sponsorblock,
            sponsor_categories: config.sponsor_categories.clone(),
            cookies_browser: config.cookies_browser.clone(),
            cookie_retried: false,
            update_retried: false,
            output_dir: item
                .playlist_title
                .as_deref()
                .filter(|title| !title.trim().is_empty())
                .map(|title| crate::playlist_output_dir(&config.output_dir, title))
                .filter(|dir| !dir.trim().is_empty())
                .unwrap_or_else(|| config.output_dir.clone()),
            filename_template: config.filename_template.clone(),
            attempts: 0,
            retry_at: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, state: &str) -> Job {
        let mut job = Job::from_item(
            EnqueueItem {
                url: format!("https://example.com/{id}"),
                ..EnqueueItem::default()
            },
            &Config::default(),
            None,
        );
        job.id = id.into();
        job.state = state.into();
        job
    }

    #[test]
    fn claim_keeps_the_parallel_cap_and_starts_the_oldest() {
        let mut jobs = vec![
            job("new", "queued"),
            job("mid", "downloading"),
            job("old", "queued"),
            job("paused", "paused"),
        ];
        let ids = claim_downloads(&mut jobs, 2);
        assert_eq!(ids, vec!["old".to_string()]);
        assert_eq!(
            jobs.iter().find(|item| item.id == "new").unwrap().state,
            "queued"
        );
        assert_eq!(
            jobs.iter().find(|item| item.id == "paused").unwrap().state,
            "paused"
        );

        let ids = claim_downloads(&mut jobs, 99);
        assert_eq!(ids, vec!["new".to_string()]);
        assert!(claim_downloads(&mut jobs, 4).is_empty());
    }

    #[test]
    fn economy_bitrate_and_playlist_folder_override_the_saved_config() {
        let config = Config {
            output_dir: r"C:\Music\bob baixa".into(),
            audio_bitrate: "192k".into(),
            audio_only: false,
            ..Config::default()
        };
        let job = Job::from_item(
            EnqueueItem {
                url: "https://youtu.be/abc".into(),
                audio_only: Some(true),
                audio_format: Some("opus".into()),
                audio_bitrate: Some("24k".into()),
                playlist_title: Some("Lisboa / noite?".into()),
                ..EnqueueItem::default()
            },
            &config,
            None,
        );
        assert!(job.audio_only);
        assert_eq!(job.audio_format.as_deref(), Some("opus"));
        assert_eq!(job.audio_bitrate, "24k");
        assert_eq!(job.output_dir, r"C:\Music\bob baixa/Lisboa noite");
        let plain = Job::from_item(
            EnqueueItem {
                url: "https://youtu.be/abc".into(),
                ..EnqueueItem::default()
            },
            &config,
            None,
        );
        assert_eq!(plain.audio_bitrate, "192k");
        assert_eq!(plain.output_dir, r"C:\Music\bob baixa");
    }

    #[test]
    fn retry_slot_opens_when_a_download_is_no_longer_running() {
        let mut jobs = vec![
            job("a", "downloading"),
            job("b", "queued"),
            job("c", "queued"),
        ];
        assert!(claim_downloads(&mut jobs, 1).is_empty());
        jobs[0].state = "paused".into();
        let ids = claim_downloads(&mut jobs, 1);
        assert_eq!(ids, vec!["c".to_string()]);
        assert_eq!(
            jobs.iter().find(|item| item.id == "b").unwrap().state,
            "queued"
        );
    }

    #[test]
    fn preset_and_old_config_roundtrip() {
        let preset = DownloadPreset {
            id: "p1".into(),
            name: "música mp3".into(),
            quality: "1080".into(),
            container: "mp4".into(),
            audio_only: true,
            audio_format: "mp3".into(),
            audio_bitrate: "192k".into(),
            subtitles: false,
            embed_subs: false,
            sub_langs: "pt.*,en.*".into(),
            embed_thumbnail: true,
            embed_metadata: true,
            embed_chapters: false,
            sponsorblock: false,
            sponsor_categories: Vec::new(),
        };
        let json = serde_json::to_value(&preset).unwrap();
        assert_eq!(json["audioOnly"], true);
        assert_eq!(json["name"], "música mp3");
        let back: DownloadPreset = serde_json::from_value(json).unwrap();
        assert_eq!(back, preset);

        let old = r#"{"language":"pt","theme":"system","outputDir":"","filenameTemplate":"x","globalSpeed":"","clipboardWatch":true,"minimizeToTray":true,"notifications":true,"hotkey":true,"quality":"1080","container":"mp4","audioOnly":false,"audioFormat":"mp3","audioBitrate":"192k","subtitles":false,"embedSubs":true,"subLangs":"pt.*","embedThumbnail":true,"embedMetadata":true,"embedChapters":true,"sponsorblock":false,"sponsorCategories":[],"cookiesBrowser":"auto","downloadKnownOnFirstWatch":false}"#;
        let config: Config = serde_json::from_str(old).unwrap();
        assert_eq!(config.parallel, 2);
        assert!(!config.context_menu);
        assert!(config.presets.is_empty());
        assert!(config.seen_version.is_empty());
        assert!(config.auto_update);
    }

    #[test]
    fn search_can_choose_mp3_or_mp4_without_replacing_the_saved_settings() {
        let mut config = Config::default();
        config.audio_only = true;
        config.audio_format = "flac".into();
        config.container = "webm".into();
        config.quality = "720".into();

        let video = Job::from_item(
            EnqueueItem {
                url: "https://youtu.be/abc".into(),
                audio_only: Some(false),
                container: Some("mp4".into()),
                ..EnqueueItem::default()
            },
            &config,
            None,
        );
        assert!(!video.audio_only);
        assert_eq!(video.container, "mp4");
        assert_eq!(video.audio_format.as_deref(), Some("flac"));
        assert_eq!(video.quality, "720");

        let audio = Job::from_item(
            EnqueueItem {
                url: "https://youtu.be/abc".into(),
                audio_only: Some(true),
                audio_format: Some("mp3".into()),
                ..EnqueueItem::default()
            },
            &config,
            None,
        );
        assert!(audio.audio_only);
        assert_eq!(audio.audio_format.as_deref(), Some("mp3"));
        assert_eq!(audio.container, "webm");
        assert!(audio.warning.is_none());
    }

    #[test]
    fn the_queue_is_still_there_after_a_restart_and_a_429_waits() {
        use crate::{apply_transient_retry, resume_interrupted};

        let jobs = vec![job("cut", "downloading"), job("wait", "queued")];
        let saved = serde_json::to_string(&jobs).unwrap();
        let mut loaded: Vec<Job> = serde_json::from_str(&saved).unwrap();
        assert_eq!(loaded[0].state, "downloading");
        assert_eq!(loaded[0].attempts, 0);
        resume_interrupted(&mut loaded);
        assert_eq!(loaded[0].state, "queued");
        assert_eq!(loaded[1].state, "queued");

        let secs = apply_transient_retry(&mut loaded[0]);
        assert_eq!(secs, 2);
        assert_eq!(loaded[0].attempts, 1);
        assert_eq!(loaded[0].state, "queued");
        assert!(loaded[0].retry_at.as_deref().map(iso_in_future).unwrap_or(false));
        let again = serde_json::to_string(&loaded).unwrap();
        let round: Vec<Job> = serde_json::from_str(&again).unwrap();
        assert_eq!(round[0].attempts, 1);
        assert!(round[0].retry_at.is_some());
        let ids = claim_downloads(&mut loaded, 2);
        assert_eq!(ids, vec!["wait".to_string()]);
        assert_eq!(loaded[0].state, "queued");

        let old = r#"{"id":"old","url":"https://example.com/old","title":"t","state":"queued","progress":0,"createdAt":"2026-01-01T00:00:00Z","audioOnly":true,"quality":"1080","container":"mp4","audioBitrate":"24k","outputDir":"C:/m","filenameTemplate":"x"}"#;
        let legacy: Job = serde_json::from_str(old).unwrap();
        assert_eq!(legacy.attempts, 0);
        assert!(legacy.retry_at.is_none());
    }
}
