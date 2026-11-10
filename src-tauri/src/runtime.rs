use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::hidden::tokio_command;
use bob_core::{
    build_download_args, changelog_section, claim_downloads, clamp_parallel, cookie_lock_hint,
    detect_browsers, extract_urls, ffmpeg_args, is_clipboard_launch, iso_in_future,
    looks_like_auth, looks_like_cookie_lock, new_id, now_iso, output_for, parse_ffmpeg_seconds,
    parse_json_value_text, parse_media, parse_output_path, parse_progress, parse_search,
    parse_title_line, pick_browser, subtitle_rate_limit_only, subtitle_retry_delay_secs,
    subtitle_skip_warning, text, url_from_link, BobEvent, Config, ConvertJob, EnqueueItem,
    HistoryEntry, Job, MediaItem, SearchHit, ToolStatus, Watch, SUBTITLE_RATE_LIMIT_ATTEMPTS,
};
use futures_util::StreamExt;
use serde::Serialize;
use std::sync::Arc as StdArc;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::{Mutex, Notify};

use crate::Snapshot;

pub struct Control {
    pub notify: Notify,
    pub reason: AtomicU8,
}

pub struct Inner {
    pub config: Config,
    pub jobs: Vec<Job>,
    pub history: Vec<HistoryEntry>,
    pub watches: Vec<Watch>,
    pub tools: ToolStatus,
    pub ytdlp: Option<PathBuf>,
    pub ffmpeg_dir: Option<PathBuf>,
    pub ffmpeg_bin: Option<PathBuf>,
    pub ffprobe_bin: Option<PathBuf>,
    pub schedule_at: Option<String>,
    pub convert_job: Option<ConvertJob>,
    pub convert_stop: Option<Arc<Notify>>,
    pub controls: HashMap<String, Arc<Control>>,
    pub data_dir: PathBuf,
    pub logs: HashMap<String, String>,
    pub last_job_emit: HashMap<String, Instant>,
    pub last_update_check: Option<String>,
    pub ffmpeg_stamp: Option<String>,
    pub pending_installer: Option<PathBuf>,
    pub watch_running: HashSet<String>,
    pub ready: bool,
    pub whats_new: bool,
}

impl Default for Inner {
    fn default() -> Self {
        Self {
            config: Config::default(),
            jobs: Vec::new(),
            history: Vec::new(),
            watches: Vec::new(),
            tools: ToolStatus::default(),
            ytdlp: None,
            ffmpeg_dir: None,
            ffmpeg_bin: None,
            ffprobe_bin: None,
            schedule_at: None,
            convert_job: None,
            convert_stop: None,
            controls: HashMap::new(),
            data_dir: PathBuf::new(),
            logs: HashMap::new(),
            last_job_emit: HashMap::new(),
            last_update_check: None,
            ffmpeg_stamp: None,
            pending_installer: None,
            watch_running: HashSet::new(),
            ready: false,
            whats_new: false,
        }
    }
}

pub struct AppState {
    pub inner: StdArc<Mutex<Inner>>,
    pub minimize_to_tray: AtomicBool,
    pub update_busy: StdArc<AtomicBool>,
    pub tools_busy: StdArc<AtomicBool>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            inner: StdArc::new(Mutex::new(Inner::default())),
            minimize_to_tray: AtomicBool::new(true),
            update_busy: StdArc::new(AtomicBool::new(false)),
            tools_busy: StdArc::new(AtomicBool::new(false)),
        }
    }
}

pub(crate) struct Busy {
    flag: StdArc<AtomicBool>,
}

impl Drop for Busy {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

fn try_busy(flag: &StdArc<AtomicBool>) -> Option<Busy> {
    if flag.swap(true, Ordering::AcqRel) {
        None
    } else {
        Some(Busy { flag: flag.clone() })
    }
}

const PAUSE: u8 = 1;
const CANCEL: u8 = 2;
const RETRY: u8 = 3;
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn boot(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        prepare(&app).await;
        let app_tools = app.clone();
        tauri::async_runtime::spawn(async move {
            let _ = install_tools(&app_tools, false).await;
            crate::updates::startup(&app_tools).await;
        });
        let app_loop = app.clone();
        tauri::async_runtime::spawn(async move { ticker(app_loop).await });
        let app_clip = app.clone();
        tauri::async_runtime::spawn(async move { clipboard_loop(app_clip).await });
        start_http(app.clone());
        crate::drive::boot(app.clone());
        register_hotkey(&app);
        setup_tray(&app);
        let args: Vec<String> = std::env::args().skip(1).collect();
        open_args(&app, args);
    });
}

async fn prepare(app: &AppHandle) {
    let data_dir = app.path().app_data_dir().unwrap_or_else(|_| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("bob baixa")
    });
    let _ = std::fs::create_dir_all(&data_dir);
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.data_dir = data_dir.clone();
    let config_path = data_dir.join("config.json");
    let config_existed = config_path.is_file();
    if let Some(config) = read_json::<Config>(&config_path) {
        inner.config = config;
    }
    inner.config.parallel = clamp_parallel(inner.config.parallel);
    if inner.config.output_dir.trim().is_empty() {
        inner.config.output_dir = default_output();
    }
    inner.whats_new = config_existed && inner.config.seen_version != APP_VERSION;
    if let Some(mut jobs) = read_json::<Vec<Job>>(&data_dir.join("jobs.json")) {
        bob_core::resume_interrupted(&mut jobs);
        inner.jobs = jobs;
    }
    if let Some(history) = read_json::<Vec<HistoryEntry>>(&data_dir.join("history.json")) {
        inner.history = history;
    }
    if let Some(watches) = read_json::<Vec<Watch>>(&data_dir.join("watches.json")) {
        inner.watches = watches;
    }
    let jobs = inner.jobs.clone();
    let tools = inner.tools.clone();
    let history = inner.history.clone();
    let watches = public_watches(&inner.watches);
    let menu = inner.config.context_menu;
    let language = inner.config.language.clone();
    let minimize = inner.config.minimize_to_tray;
    inner.ready = true;
    drop(inner);
    app.state::<AppState>()
        .minimize_to_tray
        .store(minimize, Ordering::Relaxed);
    let _ = crate::shell_menu::sync(menu, &language);
    emit(app, &BobEvent::Jobs { jobs });
    emit(app, &BobEvent::Tools { tools });
    emit(app, &BobEvent::History { history });
    emit(app, &BobEvent::Watches { watches });
}

pub async fn snapshot(app: &AppHandle) -> Snapshot {
    let inner = app.state::<AppState>().inner.clone().lock_owned().await;
    Snapshot {
        config: inner.config.clone(),
        jobs: inner.jobs.clone(),
        history: inner.history.clone(),
        watches: public_watches(&inner.watches),
        tools: inner.tools.clone(),
        schedule_at: inner.schedule_at.clone(),
        convert_job: inner.convert_job.clone(),
        ready: inner.ready,
        whats_new: inner.whats_new,
        release_notes: release_notes(),
    }
}

pub fn release_notes() -> String {
    changelog_section(include_str!("../../CHANGELOG.md"), APP_VERSION).unwrap_or_default()
}

pub async fn ack_whats_new(app: &AppHandle) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.config.seen_version = APP_VERSION.into();
    inner.whats_new = false;
    save(&inner);
}

pub async fn set_config(app: &AppHandle, mut config: Config) -> Result<(), String> {
    if config.output_dir.trim().is_empty() {
        config.output_dir = default_output();
    }
    config.parallel = clamp_parallel(config.parallel);
    config.presets.truncate(24);
    for preset in &mut config.presets {
        preset.name = preset.name.trim().chars().take(40).collect();
    }
    config
        .presets
        .retain(|preset| !preset.id.trim().is_empty() && !preset.name.is_empty());
    crate::shell_menu::sync(config.context_menu, &config.language)?;
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config = config;
        save(&inner);
        let hotkeys = hotkey_bindings(&inner.config);
        let minimize = inner.config.minimize_to_tray;
        drop(inner);
        app.state::<AppState>()
            .minimize_to_tray
            .store(minimize, Ordering::Relaxed);
        sync_hotkeys(app, &hotkeys);
    }
    pump(app).await;
    Ok(())
}

pub async fn analyze(app: &AppHandle, urls: Vec<String>) -> Result<Vec<MediaItem>, String> {
    if urls.is_empty() {
        return Ok(Vec::new());
    }
    let bin = ytdlp_path(app).await?;
    let limit = StdArc::new(tokio::sync::Semaphore::new(3));
    let mut tasks = Vec::with_capacity(urls.len());
    for (index, url) in urls.into_iter().enumerate() {
        let bin = bin.clone();
        let limit = limit.clone();
        tasks.push(async move {
            let _permit = limit.acquire().await.map_err(|error| error.to_string())?;
            let item = analyze_one(&bin, url).await?;
            Ok::<_, String>((index, item))
        });
    }
    let mut items = futures_util::future::try_join_all(tasks).await?;
    items.sort_by_key(|(index, _)| *index);
    Ok(items.into_iter().map(|(_, item)| item).collect())
}

async fn analyze_one(bin: &Path, url: String) -> Result<MediaItem, String> {
    let output = capture(
        bin,
        &[
            "-J".into(),
            "--flat-playlist".into(),
            "--no-warnings".into(),
            "--skip-download".into(),
            url.clone(),
        ],
    )
    .await?;
    let mut item = parse_media(&parse_json_value_text(&output)?, &url);
    if item.kind == "video" && item.formats.is_empty() {
        let full = capture(
            bin,
            &[
                "-J".into(),
                "--no-warnings".into(),
                "--skip-download".into(),
                url.clone(),
            ],
        )
        .await?;
        item = parse_media(&parse_json_value_text(&full)?, &url);
    }
    Ok(item)
}

pub async fn search(app: &AppHandle, query: String) -> Result<Vec<SearchHit>, String> {
    let query = query.trim().to_string();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let bin = ytdlp_path(app).await?;
    let target = bob_core::search_target(&query);
    let output = capture(
        &bin,
        &[
            "-J".into(),
            "--flat-playlist".into(),
            "--no-warnings".into(),
            "--skip-download".into(),
            target,
        ],
    )
    .await?;
    Ok(parse_search(&parse_json_value_text(&output)?))
}

pub async fn enqueue(app: &AppHandle, items: Vec<EnqueueItem>) -> Result<(), String> {
    if items.is_empty() {
        return Ok(());
    }
    let language;
    let jobs_len;
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        language = inner.config.language.clone();
        if inner.ytdlp.is_none() || inner.ffmpeg_dir.is_none() {
            return Err(text(
                &language,
                "o yt-dlp e o ffmpeg ainda estão sendo preparados.",
                "yt-dlp and ffmpeg are still being prepared.",
            ));
        }
        let hold = inner.schedule_at.clone();
        let config = inner.config.clone();
        for item in items {
            if !(item.url.starts_with("http://") || item.url.starts_with("https://")) {
                continue;
            }
            inner
                .jobs
                .insert(0, Job::from_item(item, &config, hold.as_deref()));
        }
        jobs_len = inner.jobs.len();
        save(&inner);
        let jobs = inner.jobs.clone();
        drop(inner);
        emit(app, &BobEvent::Jobs { jobs });
        let _ = jobs_len;
    }
    pump(app).await;
    Ok(())
}

pub async fn job_action(app: &AppHandle, id: String, action: &str) -> Result<(), String> {
    let control = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.controls.get(&id).cloned()
    };
    if action == "remove" {
        signal(&control, CANCEL);
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.jobs.retain(|job| job.id != id);
        inner.controls.remove(&id);
        inner.logs.remove(&id);
        inner.last_job_emit.remove(&id);
        save(&inner);
        let jobs = inner.jobs.clone();
        drop(inner);
        emit(app, &BobEvent::Jobs { jobs });
        pump(app).await;
        return Ok(());
    }
    if matches!(action, "pause" | "cancel" | "retry") && control.is_some() {
        let reason = match action {
            "pause" => PAUSE,
            "retry" => RETRY,
            _ => CANCEL,
        };
        signal(&control, reason);
        return Ok(());
    }
    if action == "resume" && control.is_some() {
        return Ok(());
    }
    let mut changed = false;
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
            match action {
                "pause" => {
                    job.state = "paused".into();
                    changed = true;
                }
                "cancel" => {
                    job.state = "cancelled".into();
                    changed = true;
                }
                "resume" | "retry" => {
                    if action == "retry" {
                        job.progress = 0.0;
                        job.error = None;
                        job.warning = None;
                        job.downloaded = None;
                        job.speed = None;
                        job.eta = None;
                    }
                    job.state = "queued".into();
                    job.scheduled_at = None;
                    changed = true;
                }
                _ => {}
            }
        }
        if changed {
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
        }
    }
    if matches!(action, "resume" | "retry") {
        pump(app).await;
    }
    Ok(())
}

fn signal(control: &Option<Arc<Control>>, reason: u8) {
    let Some(control) = control else { return };
    control.reason.store(reason, Ordering::Relaxed);
    control.notify.notify_waiters();
    control.notify.notify_one();
}

pub async fn schedule_queue(app: &AppHandle, at: String) -> Result<(), String> {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.schedule_at = Some(at.clone());
    for job in &mut inner.jobs {
        if job.state == "queued" {
            job.state = "scheduled".into();
            job.scheduled_at = Some(at.clone());
        }
    }
    save(&inner);
    let jobs = inner.jobs.clone();
    drop(inner);
    emit(app, &BobEvent::Schedule { at: Some(at) });
    emit(app, &BobEvent::Jobs { jobs });
    Ok(())
}

pub async fn clear_schedule(app: &AppHandle) -> Result<(), String> {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.schedule_at = None;
    for job in &mut inner.jobs {
        if job.state == "scheduled" {
            job.state = "queued".into();
            job.scheduled_at = None;
        }
    }
    save(&inner);
    let jobs = inner.jobs.clone();
    drop(inner);
    emit(app, &BobEvent::Schedule { at: None });
    emit(app, &BobEvent::Jobs { jobs });
    pump(app).await;
    Ok(())
}

pub async fn add_watch(app: &AppHandle, url: String, every_minutes: u32) -> Result<(), String> {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.watches.insert(
        0,
        Watch {
            id: new_id("watch"),
            url,
            title: String::new(),
            every_minutes: every_minutes.clamp(5, 1440),
            enabled: true,
            last_checked: None,
            known: 0,
            seen: Vec::new(),
        },
    );
    let id = inner.watches[0].id.clone();
    save(&inner);
    let watches = public_watches(&inner.watches);
    drop(inner);
    emit(app, &BobEvent::Watches { watches });
    let app = app.clone();
    tauri::async_runtime::spawn(async move { check_watch(&app, id).await });
    Ok(())
}

pub async fn watch_action(app: &AppHandle, id: String, action: &str) -> Result<(), String> {
    match action {
        "remove" => {
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            inner.watches.retain(|watch| watch.id != id);
            save(&inner);
            let watches = public_watches(&inner.watches);
            drop(inner);
            emit(app, &BobEvent::Watches { watches });
        }
        "toggle" => {
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            if let Some(watch) = inner.watches.iter_mut().find(|watch| watch.id == id) {
                watch.enabled = !watch.enabled;
            }
            save(&inner);
            let watches = public_watches(&inner.watches);
            drop(inner);
            emit(app, &BobEvent::Watches { watches });
        }
        "check" => check_watch(app, id).await,
        _ => {}
    }
    Ok(())
}

pub async fn clear_history(app: &AppHandle) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.history.clear();
    save(&inner);
    drop(inner);
    emit(
        app,
        &BobEvent::History {
            history: Vec::new(),
        },
    );
}

pub async fn convert(
    app: &AppHandle,
    input: String,
    preset: String,
    bitrate: String,
) -> Result<(), String> {
    let ffmpeg = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if inner
            .convert_job
            .as_ref()
            .is_some_and(|job| job.state == "running")
        {
            return Err(text(
                &inner.config.language,
                "já tem uma conversão em andamento.",
                "a conversion is already running.",
            ));
        }
        inner.ffmpeg_bin.clone().ok_or_else(|| {
            text(
                &inner.config.language,
                "o ffmpeg ainda não está pronto.",
                "ffmpeg is not ready yet.",
            )
        })?
    };
    let folder = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config.output_dir.clone()
    };
    let _ = std::fs::create_dir_all(&folder);
    let output = output_for(&input, &preset, &folder);
    let stop = Arc::new(Notify::new());
    let job = ConvertJob {
        id: new_id("convert"),
        input: input.clone(),
        output: None,
        preset: preset.clone(),
        state: "running".into(),
        progress: 0.0,
        error: None,
    };
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.convert_stop = Some(stop.clone());
        inner.convert_job = Some(job.clone());
    }
    emit(app, &BobEvent::Convert { job: job.clone() });
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        run_convert(app, ffmpeg, input, output, preset, bitrate, stop).await;
    });
    Ok(())
}

pub async fn cancel_convert(app: &AppHandle) {
    let stop = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.convert_stop.clone()
    };
    if let Some(stop) = stop {
        stop.notify_waiters();
        stop.notify_one();
    }
}

pub fn show_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

pub fn minimize_on_close(app: &AppHandle) -> bool {
    app.state::<AppState>()
        .minimize_to_tray
        .load(Ordering::Relaxed)
}

pub fn open_args(app: &AppHandle, args: Vec<String>) {
    let clipboard = args.iter().any(|arg| is_clipboard_launch(arg));
    let urls: Vec<String> = args.iter().filter_map(|arg| url_from_link(arg)).collect();
    if clipboard {
        download_clipboard(app);
    }
    if urls.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Err(error) = enqueue(
            &app,
            urls.into_iter()
                .map(|url| EnqueueItem {
                    url,
                    ..EnqueueItem::default()
                })
                .collect(),
        )
        .await
        {
            toast(&app, &error);
        }
        show_main(&app);
    });
}

pub fn hotkey(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let enabled = app
            .state::<AppState>()
            .inner
            .clone()
            .lock_owned()
            .await
            .config
            .hotkey;
        if enabled {
            download_clipboard_now(&app).await;
        }
    });
}

pub fn download_clipboard(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        download_clipboard_now(&app).await;
    });
}

async fn download_clipboard_now(app: &AppHandle) {
    show_main(app);
    let language = app
        .state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .config
        .language
        .clone();
    match clipboard_url() {
        Some(url) => {
            if let Err(error) = enqueue(
                app,
                vec![EnqueueItem {
                    url,
                    ..EnqueueItem::default()
                }],
            )
            .await
            {
                toast(app, &error);
            }
        }
        None => toast(
            app,
            &text(
                &language,
                "nenhum link na área de transferência.",
                "no link on the clipboard.",
            ),
        ),
    }
}

pub async fn install_tools(app: &AppHandle, force: bool) -> Result<(), String> {
    let data_dir = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if inner.data_dir.as_os_str().is_empty() {
            return Err("pasta do aplicativo indisponível".into());
        }
        inner.data_dir.clone()
    };
    let busy = app.state::<AppState>().tools_busy.clone();
    let _busy =
        try_busy(&busy).ok_or_else(|| "as ferramentas já estão sendo preparadas.".to_string())?;
    let bin_dir = data_dir.join("bin");
    let _ = std::fs::create_dir_all(&bin_dir);
    let language = app
        .state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .config
        .language
        .clone();
    set_tools(app, "downloading", "downloading", Some(0.05), None).await;

    let ytdlp_name = if cfg!(windows) {
        "yt-dlp.exe"
    } else {
        "yt-dlp"
    };
    let ytdlp_path = bin_dir.join(ytdlp_name);
    if force || !file_ready(&ytdlp_path, 500_000) {
        if let Err(error) = download_to(app, ytdlp_url(), &ytdlp_path, 0.15, 0.4).await {
            if which("yt-dlp").is_none() && which("yt-dlp.exe").is_none() {
                set_tools(app, "error", "missing", None, Some(error.clone())).await;
                return Err(error);
            }
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ =
                    std::fs::set_permissions(&ytdlp_path, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    let ytdlp = if ytdlp_path.exists() {
        ytdlp_path
    } else {
        which("yt-dlp")
            .or_else(|| which("yt-dlp.exe"))
            .ok_or_else(|| {
                text(
                    &language,
                    "não foi possível obter o yt-dlp.",
                    "could not get yt-dlp.",
                )
            })?
    };

    let ffmpeg_bin = if cfg!(windows) {
        let exe = bin_dir.join("ffmpeg.exe");
        let probe = bin_dir.join("ffprobe.exe");
        if force || !file_ready(&exe, 500_000) {
            let zip_path = bin_dir.join("ffmpeg.zip");
            download_to(app, ffmpeg_zip_url(), &zip_path, 0.45, 0.9).await?;
            unzip_ffmpeg(&zip_path, &bin_dir)?;
            let _ = std::fs::remove_file(&zip_path);
            if !file_ready(&exe, 500_000) {
                let _ = std::fs::remove_file(&exe);
                return Err(text(
                    &language,
                    "o ffmpeg baixado veio incompleto.",
                    "the ffmpeg download was incomplete.",
                ));
            }
        }
        if file_ready(&exe, 500_000) {
            Some((bin_dir.clone(), exe, probe.exists().then_some(probe)))
        } else {
            None
        }
    } else {
        None
    };
    let (ffmpeg_dir, ffmpeg_bin, ffprobe) = if let Some(found) = ffmpeg_bin {
        found
    } else if let Some(path) = which("ffmpeg") {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let probe = dir.join(if cfg!(windows) {
            "ffprobe.exe"
        } else {
            "ffprobe"
        });
        (dir, path, probe.exists().then_some(probe))
    } else {
        set_tools(
            app,
            "ready",
            "error",
            None,
            Some(text(
                &language,
                "ffmpeg não encontrado.",
                "ffmpeg was not found.",
            )),
        )
        .await;
        return Err(text(
            &language,
            "ffmpeg não encontrado.",
            "ffmpeg was not found.",
        ));
    };

    let ytdlp_version = command_version(&ytdlp, "--version").await;
    let ffmpeg_version = command_version(&ffmpeg_bin, "-version").await;
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.ytdlp = Some(ytdlp);
        inner.ffmpeg_dir = Some(ffmpeg_dir);
        inner.ffmpeg_bin = Some(ffmpeg_bin);
        inner.ffprobe_bin = ffprobe;
        inner.tools = ToolStatus {
            ytdlp: "ready".into(),
            ffmpeg: "ready".into(),
            ytdlp_version,
            ffmpeg_version,
            message: None,
            progress: Some(1.0),
            app_version: Some(env!("CARGO_PKG_VERSION").into()),
        };
        let tools = inner.tools.clone();
        drop(inner);
        emit(app, &BobEvent::Tools { tools });
    }
    pump(app).await;
    Ok(())
}

pub async fn update_ytdlp(app: &AppHandle) -> Result<(), String> {
    let bin = ytdlp_path(app).await?;
    let language = app
        .state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .config
        .language
        .clone();
    let updated = tokio_command(&bin).arg("-U").output().await;
    let ok = updated
        .as_ref()
        .map(|output| output.status.success())
        .unwrap_or(false);
    if !ok {
        install_tools(app, true).await?;
    }
    let version = command_version(&bin, "--version").await;
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.tools.ytdlp = "ready".into();
    inner.tools.ytdlp_version = version;
    inner.tools.message = None;
    let tools = inner.tools.clone();
    drop(inner);
    emit(app, &BobEvent::Tools { tools });
    toast(
        &app,
        &text(&language, "yt-dlp atualizado", "yt-dlp updated"),
    );
    Ok(())
}

async fn ticker(app: AppHandle) {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        promote(&app).await;
        pump(&app).await;
        let due: Vec<String> = {
            let inner = app.state::<AppState>().inner.clone().lock_owned().await;
            inner
                .watches
                .iter()
                .filter(|watch| {
                    watch.enabled && watch_due(watch) && !inner.watch_running.contains(&watch.id)
                })
                .map(|watch| watch.id.clone())
                .collect()
        };
        for id in due {
            let app = app.clone();
            tauri::async_runtime::spawn(async move { check_watch(&app, id).await });
        }
        crate::updates::periodic(&app).await;
    }
}

async fn promote(app: &AppHandle) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let mut changed = false;
    if let Some(at) = inner.schedule_at.clone() {
        if !iso_in_future(&at) {
            inner.schedule_at = None;
            for job in &mut inner.jobs {
                if job.state == "scheduled" && job.scheduled_at.as_deref() == Some(at.as_str()) {
                    job.state = "queued".into();
                    job.scheduled_at = None;
                }
            }
            changed = true;
            emit(app, &BobEvent::Schedule { at: None });
        }
    }
    for job in &mut inner.jobs {
        if job.state == "scheduled"
            && job
                .scheduled_at
                .as_deref()
                .map(|at| !iso_in_future(at))
                .unwrap_or(false)
        {
            job.state = "queued".into();
            job.scheduled_at = None;
            changed = true;
        }
    }
    if changed {
        save(&inner);
        let jobs = inner.jobs.clone();
        drop(inner);
        emit(app, &BobEvent::Jobs { jobs });
    }
}

fn schedule_pump(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        pump(&app).await;
    });
}

async fn pump(app: &AppHandle) {
    let ready = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.ytdlp.is_some() && inner.ffmpeg_dir.is_some()
    };
    if !ready {
        return;
    }
    let start = {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let parallel = inner.config.parallel;
        let start = claim_downloads(&mut inner.jobs, parallel);
        if start.is_empty() {
            Vec::new()
        } else {
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
            start
        }
    };
    for id in start {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { run_download(app, id).await });
    }
}

async fn run_download(app: AppHandle, id: String) {
    let control = Arc::new(Control {
        notify: Notify::new(),
        reason: AtomicU8::new(0),
    });
    let (bin, ffmpeg, job) = {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let Some(job) = inner.jobs.iter().find(|job| job.id == id).cloned() else {
            return;
        };
        if job.state != "downloading" {
            return;
        }
        let (Some(bin), Some(ffmpeg)) = (inner.ytdlp.clone(), inner.ffmpeg_dir.clone()) else {
            if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
                job.state = "queued".into();
            }
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(&app, &BobEvent::Jobs { jobs });
            return;
        };
        inner.controls.insert(id.clone(), control.clone());
        (bin, ffmpeg, job)
    };
    if !job.output_dir.trim().is_empty() {
        let _ = std::fs::create_dir_all(&job.output_dir);
    }

    let mut attempt = 0u32;
    let mut without_subtitles = false;
    let end = loop {
        let mut current = job.clone();
        if without_subtitles {
            current.subtitles = false;
        }
        match run_ytdlp_attempt(&app, &id, &bin, &ffmpeg, &current, &control).await {
            Attempt::Stopped(reason) => break Attempt::Stopped(reason),
            Attempt::Spawn(error) => break Attempt::Spawn(error),
            Attempt::Ok(path) => {
                let warning = if without_subtitles {
                    let language = app
                        .state::<AppState>()
                        .inner
                        .clone()
                        .lock_owned()
                        .await
                        .config
                        .language
                        .clone();
                    Some(subtitle_skip_warning(&language))
                } else {
                    None
                };
                break Attempt::Done { path, warning };
            }
            Attempt::Failed(tail) => {
                // Captions are requested before the video. A 429 there used to abort the file.
                if !without_subtitles && job.subtitles && subtitle_rate_limit_only(&tail) {
                    if attempt + 1 < SUBTITLE_RATE_LIMIT_ATTEMPTS {
                        attempt += 1;
                        if !backoff_or_stop(&control, subtitle_retry_delay_secs(attempt)).await {
                            break Attempt::Stopped(control.reason.load(Ordering::Relaxed));
                        }
                        continue;
                    }
                    without_subtitles = true;
                    continue;
                }
                break Attempt::Failed(tail);
            }
            Attempt::Done { .. } => break Attempt::Spawn("estado inesperado".into()),
        }
    };

    app.state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .controls
        .remove(&id);

    match end {
        Attempt::Spawn(error) => {
            finish(&app, &id, "failed", Some(error), None, None).await;
        }
        Attempt::Stopped(reason) => finish_stopped(&app, &id, reason).await,
        Attempt::Done { path, warning } => {
            finish(&app, &id, "done", None, path, warning).await;
        }
        Attempt::Failed(tail) => fail_download(&app, &id, &tail).await,
        Attempt::Ok(_) => {
            finish(&app, &id, "failed", Some("estado inesperado".into()), None, None).await;
        }
    }
}

enum Attempt {
    Spawn(String),
    Stopped(u8),
    Ok(Option<String>),
    Done {
        path: Option<String>,
        warning: Option<String>,
    },
    Failed(String),
}

async fn backoff_or_stop(control: &Control, secs: u64) -> bool {
    if control.reason.load(Ordering::Relaxed) != 0 {
        return false;
    }
    let sleep = tokio::time::sleep(Duration::from_secs(secs));
    tokio::pin!(sleep);
    tokio::select! {
        _ = &mut sleep => control.reason.load(Ordering::Relaxed) == 0,
        _ = control.notify.notified() => false,
    }
}

async fn finish_stopped(app: &AppHandle, id: &str, reason: u8) {
    if reason == RETRY {
        requeue(app, id, true).await;
    } else if reason == PAUSE {
        finish(app, id, "paused", None, None, None).await;
    } else {
        finish(app, id, "cancelled", None, None, None).await;
    }
}

async fn run_ytdlp_attempt(
    app: &AppHandle,
    id: &str,
    bin: &Path,
    ffmpeg: &Path,
    job: &Job,
    control: &Arc<Control>,
) -> Attempt {
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.logs.remove(id);
    }
    let args = build_download_args(job, &ffmpeg.to_string_lossy());
    let mut child = match {
        let mut cmd = tokio_command(bin);
        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        cmd.spawn()
    } {
        Ok(child) => child,
        Err(error) => return Attempt::Spawn(error.to_string()),
    };
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_out = app.clone();
    let id_out = id.to_string();
    let reader_out = tauri::async_runtime::spawn(async move {
        if let Some(stdout) = stdout {
            read_pipe(app_out, id_out, stdout, false).await;
        }
    });
    let app_err = app.clone();
    let id_err = id.to_string();
    let reader_err = tauri::async_runtime::spawn(async move {
        if let Some(stderr) = stderr {
            read_pipe(app_err, id_err, stderr, true).await;
        }
    });
    let notify = control.clone();
    let killer = tauri::async_runtime::spawn(async move {
        notify.notify.notified().await;
        if pid != 0 {
            kill_tree(pid);
        }
    });
    let status = child.wait().await;
    killer.abort();
    let _ = reader_out.await;
    let _ = reader_err.await;
    let reason = control.reason.load(Ordering::Relaxed);
    if reason != 0 {
        return Attempt::Stopped(reason);
    }
    let code_ok = status
        .as_ref()
        .map(|status| status.success())
        .unwrap_or(false);
    if code_ok {
        let path = {
            let inner = app.state::<AppState>().inner.clone().lock_owned().await;
            inner
                .jobs
                .iter()
                .find(|job| job.id == id)
                .and_then(|job| job.output_path.clone())
        };
        return Attempt::Ok(path);
    }
    let tail = app
        .state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .logs
        .get(id)
        .cloned()
        .unwrap_or_default();
    Attempt::Failed(tail)
}

async fn fail_download(app: &AppHandle, id: &str, tail: &str) {
    let (language, retried, preference) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let job = inner.jobs.iter().find(|job| job.id == id);
        (
            inner.config.language.clone(),
            job.map(|job| job.cookie_retried).unwrap_or(true),
            job.map(|job| job.cookies_browser.clone())
                .unwrap_or_default(),
        )
    };
    if looks_like_cookie_lock(tail) {
        finish(app, id, "failed", Some(cookie_lock_hint(&language)), None, None).await;
        return;
    }
    let (update_retried, auto_update) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let job = inner.jobs.iter().find(|job| job.id == id);
        (
            job.map(|job| job.update_retried).unwrap_or(true),
            inner.config.auto_update,
        )
    };
    if auto_update && !update_retried && bob_core::looks_like_extractor(tail) {
        let _ = crate::updates::refresh_ytdlp(app).await;
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
            job.update_retried = true;
            job.state = "queued".into();
            job.error = None;
            job.warning = None;
            job.progress = 0.0;
        }
        save(&inner);
        let jobs = inner.jobs.clone();
        drop(inner);
        emit(app, &BobEvent::Jobs { jobs });
        schedule_pump(app);
        return;
    }
    let can_retry = !retried && (preference == "auto" || preference.is_empty());
    if bob_core::is_transient_download_error(tail) {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let secs = if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
            let secs = bob_core::apply_transient_retry(job);
            job.warning = Some(text(
                &language,
                &format!("sem rede ou o youtube pediu espera. nova tentativa em {secs}s."),
                &format!("offline or rate limited. retrying in {secs}s."),
            ));
            secs
        } else {
            0
        };
        if secs > 0 {
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
            schedule_pump(app);
            return;
        }
    }
    if can_retry && looks_like_auth(tail) {
        if let Some(browser) = pick_browser("auto", &detect_browsers()) {
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
                job.cookies_browser = browser;
                job.cookie_retried = true;
                job.state = "queued".into();
                job.error = None;
                job.warning = None;
                job.progress = 0.0;
            }
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
            schedule_pump(app);
            return;
        }
    }
    let message = if tail.trim().is_empty() {
        text(&language, "o download falhou.", "the download failed.")
    } else {
        tail.lines()
            .rev()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("erro")
            .to_string()
    };
    finish(app, id, "failed", Some(message), None, None).await;
}

pub async fn cache_thumb(app: &AppHandle, url: &str) -> Result<String, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("url inválida".into());
    }
    let dir = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if inner.data_dir.as_os_str().is_empty() {
            std::env::temp_dir().join("bob-baixa-thumbs")
        } else {
            inner.data_dir.join("thumbs")
        }
    };
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let path = dir.join(format!("{:x}.img", thumb_key(url)));
    if std::fs::metadata(&path)
        .map(|meta| meta.len() > 400)
        .unwrap_or(false)
    {
        return Ok(path.to_string_lossy().into_owned());
    }
    let client = reqwest::Client::builder()
        .user_agent("bob-baixa")
        .build()
        .map_err(|error| error.to_string())?;
    let bytes = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .bytes()
        .await
        .map_err(|error| error.to_string())?;
    tokio::fs::write(&path, &bytes)
        .await
        .map_err(|error| error.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

fn thumb_key(url: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in url.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

async fn read_pipe(
    app: AppHandle,
    id: String,
    pipe: impl tokio::io::AsyncRead + Unpin,
    is_err: bool,
) {
    let mut reader = BufReader::new(pipe);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        let Ok(n) = reader.read_until(b'\n', &mut buf).await else {
            break;
        };
        if n == 0 {
            break;
        }
        if buf.last() == Some(&b'\n') {
            buf.pop();
        }
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
        let line = String::from_utf8_lossy(&buf).into_owned();
        apply_line(&app, &id, &line, is_err).await;
    }
}

async fn apply_line(app: &AppHandle, id: &str, line: &str, is_err: bool) {
    let mut important = false;
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) else {
        return;
    };
    if let Some(progress) = parse_progress(&line) {
        job.progress = progress.fraction;
        job.speed = progress.speed;
        job.eta = progress.eta;
        job.downloaded = Some(progress.downloaded);
        job.total = progress.total;
    } else if let Some(path) = parse_output_path(&line) {
        job.output_path = Some(path);
        important = true;
    } else if let Some(title) = parse_title_line(&line) {
        if job.title == job.url || job.title.is_empty() {
            job.title = title;
        }
        important = true;
    } else if is_err {
        let log = inner.logs.entry(id.to_string()).or_default();
        log.push_str(line);
        log.push('\n');
        if log.len() > 4000 {
            *log = log[log.len() - 4000..].to_string();
        }
    }
    let should_emit = important
        || inner
            .last_job_emit
            .get(id)
            .map(|instant| instant.elapsed() > Duration::from_millis(180))
            .unwrap_or(true);
    if should_emit {
        inner.last_job_emit.insert(id.to_string(), Instant::now());
        let jobs = inner.jobs.clone();
        drop(inner);
        emit(app, &BobEvent::Jobs { jobs });
    }
}

async fn finish(
    app: &AppHandle,
    id: &str,
    state: &str,
    error: Option<String>,
    path: Option<String>,
    warning: Option<String>,
) {
    let mut notify_body = None;
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let language = inner.config.language.clone();
    let notifications = inner.config.notifications;
    let mut record = None;
    if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
        job.state = state.into();
        job.warning = None;
        if state == "done" {
            job.progress = 1.0;
            job.error = None;
            job.warning = warning.clone();
            if path.is_some() {
                job.output_path = path.clone();
            }
        }
        if let Some(error) = error.clone() {
            job.error = Some(error);
        }
        if state == "done" || state == "failed" {
            let note = if state == "done" {
                job.warning.clone().unwrap_or_else(|| text(&language, "concluído", "done"))
            } else {
                text(&language, "falhou", "failed")
            };
            record = Some(HistoryEntry {
                id: new_id("hist"),
                title: job.title.clone(),
                url: job.url.clone(),
                path: job.output_path.clone(),
                at: now_iso(),
                ok: state == "done",
                detail: if state == "done" {
                    job.warning.clone()
                } else {
                    job.error.clone()
                },
            });
            if notifications {
                notify_body = Some(format!("{} — {}", job.title, note));
            }
        }
    }
    if let Some(entry) = record {
        inner.history.insert(0, entry);
        inner.history.truncate(400);
    }
    inner.logs.remove(id);
    inner.last_job_emit.remove(id);
    save(&inner);
    let jobs = inner.jobs.clone();
    let history = inner.history.clone();
    drop(inner);
    emit(app, &BobEvent::Jobs { jobs });
    if state == "done" || state == "failed" {
        emit(app, &BobEvent::History { history });
    }
    if let Some(body) = notify_body {
        notify(app, &body);
    }
    if state == "done" {
        if let Some(path) = path.clone() {
            let url = {
                app.state::<AppState>()
                    .inner
                    .clone()
                    .lock_owned()
                    .await
                    .jobs
                    .iter()
                    .find(|job| job.id == id)
                    .map(|job| (job.url.clone(), job.title.clone(), job.audio_only))
            };
            if let Some((url, title, audio_only)) = url {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    crate::enrich::after_download(&app, &path, &url, &title, audio_only).await;
                });
            }
        }
    }
    schedule_pump(app);
}

async fn check_watch(app: &AppHandle, id: String) {
    if !begin_watch(app, &id).await {
        return;
    }
    check_watch_body(app, id.clone()).await;
    end_watch(app, &id).await;
}

async fn begin_watch(app: &AppHandle, id: &str) -> bool {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    if !inner.watch_running.insert(id.to_string()) {
        return false;
    }
    let Some(watch) = inner.watches.iter_mut().find(|watch| watch.id == id) else {
        inner.watch_running.remove(id);
        return false;
    };
    watch.last_checked = Some(now_iso());
    save(&inner);
    let watches = public_watches(&inner.watches);
    drop(inner);
    emit(app, &BobEvent::Watches { watches });
    true
}

async fn end_watch(app: &AppHandle, id: &str) {
    app.state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .watch_running
        .remove(id);
}

async fn requeue(app: &AppHandle, id: &str, reset: bool) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) else {
        return;
    };
    job.state = "queued".into();
    job.error = None;
    job.warning = None;
    job.speed = None;
    job.eta = None;
    if reset {
        job.progress = 0.0;
        job.downloaded = None;
    }
    inner.logs.remove(id);
    save(&inner);
    let jobs = inner.jobs.clone();
    drop(inner);
    emit(app, &BobEvent::Jobs { jobs });
    schedule_pump(app);
}

async fn check_watch_body(app: &AppHandle, id: String) {
    let (url, seen, download_known, language) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let Some(watch) = inner.watches.iter().find(|watch| watch.id == id) else {
            return;
        };
        (
            watch.url.clone(),
            watch.seen.clone(),
            inner.config.download_known_on_first_watch,
            inner.config.language.clone(),
        )
    };
    let Ok(bin) = ytdlp_path(app).await else {
        return;
    };
    let Ok(output) = capture(
        &bin,
        &[
            "-J".into(),
            "--flat-playlist".into(),
            "--playlist-end".into(),
            "15".into(),
            "--no-warnings".into(),
            "--skip-download".into(),
            url.clone(),
        ],
    )
    .await
    else {
        return;
    };
    let Ok(value) = parse_json_value_text(&output) else {
        return;
    };
    let media = parse_media(&value, &url);
    let entries = if media.entries.is_empty() && media.kind == "video" {
        vec![bob_core::PlaylistEntry {
            id: media.id.clone(),
            url: media.url.clone(),
            title: media.title.clone(),
            duration: media.duration,
            thumbnail: media.thumbnail.clone(),
        }]
    } else {
        media.entries.clone()
    };
    let first = seen.is_empty();
    let mut fresh = Vec::new();
    for entry in &entries {
        if !seen.iter().any(|id| id == &entry.id) {
            fresh.push(entry.clone());
        }
    }
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let Some(watch) = inner.watches.iter_mut().find(|watch| watch.id == id) else {
            return;
        };
        if media.title != "playlist"
            && !media.title.is_empty()
            && (watch.title.is_empty() || watch.title == watch.url)
        {
            watch.title = media.title.clone();
        }
        for entry in &entries {
            if !watch.seen.iter().any(|id| id == &entry.id) {
                watch.seen.push(entry.id.clone());
            }
        }
        watch.known = watch.seen.len() as u32;
        watch.last_checked = Some(now_iso());
        save(&inner);
        let watches = public_watches(&inner.watches);
        drop(inner);
        emit(app, &BobEvent::Watches { watches });
    }
    if first && !download_known {
        return;
    }
    let items = fresh
        .into_iter()
        .take(8)
        .map(|entry| EnqueueItem {
            url: entry.url,
            title: Some(entry.title),
            thumbnail: entry.thumbnail,
            duration: entry.duration,
            ..EnqueueItem::default()
        })
        .collect::<Vec<_>>();
    if !items.is_empty() {
        let _ = enqueue(app, items).await;
        toast(
            app,
            &text(
                &language,
                "vídeo novo na fila.",
                "a new video is in the queue.",
            ),
        );
    }
}

fn watch_due(watch: &Watch) -> bool {
    let Some(last) = watch.last_checked.as_deref() else {
        return true;
    };
    let Ok(time) = chrono::DateTime::parse_from_rfc3339(last) else {
        return true;
    };
    chrono::Utc::now() >= time + chrono::Duration::minutes(watch.every_minutes as i64)
}

async fn run_convert(
    app: AppHandle,
    ffmpeg: PathBuf,
    input: String,
    output: String,
    preset: String,
    bitrate: String,
    stop: Arc<Notify>,
) {
    let probe = app
        .state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .ffprobe_bin
        .clone();
    let duration = if let Some(probe) = probe {
        command_stdout(
            &probe,
            &[
                "-v".into(),
                "error".into(),
                "-show_entries".into(),
                "format=duration".into(),
                "-of".into(),
                "csv=p=0".into(),
                input.clone(),
            ],
        )
        .await
        .and_then(|text| text.trim().parse::<f64>().ok())
        .unwrap_or(0.0)
    } else {
        0.0
    };
    let args = ffmpeg_args(&input, &output, &preset, &bitrate);
    let mut cmd = tokio_command(&ffmpeg);
    cmd.args(&args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            set_convert(&app, "failed", 0.0, None, Some(error.to_string())).await;
            return;
        }
    };
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_progress = app.clone();
    let reader = tauri::async_runtime::spawn(async move {
        if let Some(stdout) = stdout {
            let mut reader = BufReader::new(stdout);
            let mut buf = Vec::new();
            let mut last = Instant::now() - Duration::from_secs(1);
            loop {
                buf.clear();
                let Ok(n) = reader.read_until(b'\n', &mut buf).await else {
                    break;
                };
                if n == 0 {
                    break;
                }
                let line = String::from_utf8_lossy(&buf);
                if let Some(seconds) = parse_ffmpeg_seconds(&line) {
                    let progress = if duration > 0.0 {
                        (seconds / duration).clamp(0.0, 0.99)
                    } else {
                        0.0
                    };
                    let now = Instant::now();
                    if now.duration_since(last) >= Duration::from_millis(200) {
                        last = now;
                        set_convert(&app_progress, "running", progress, None, None).await;
                    }
                }
            }
        }
    });
    let drain = tauri::async_runtime::spawn(async move {
        if let Some(stderr) = stderr {
            let mut reader = BufReader::new(stderr);
            let mut buf = Vec::new();
            loop {
                buf.clear();
                let Ok(n) = reader.read_until(b'\n', &mut buf).await else {
                    break;
                };
                if n == 0 {
                    break;
                }
            }
        }
    });
    let stop_task = stop.clone();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_flag = cancelled.clone();
    let killer = tauri::async_runtime::spawn(async move {
        stop_task.notified().await;
        cancelled_flag.store(true, Ordering::Relaxed);
        if pid != 0 {
            kill_tree(pid);
        }
    });
    let status = child.wait().await;
    killer.abort();
    reader.abort();
    drain.abort();
    let was_cancelled = cancelled.load(Ordering::Relaxed);
    if status
        .as_ref()
        .map(|status| status.success())
        .unwrap_or(false)
    {
        set_convert(&app, "done", 1.0, Some(output), None).await;
        let language = app
            .state::<AppState>()
            .inner
            .clone()
            .lock_owned()
            .await
            .config
            .language
            .clone();
        toast(
            &app,
            &text(&language, "conversão concluída", "conversion finished"),
        );
    } else if was_cancelled {
        set_convert(&app, "cancelled", 0.0, None, None).await;
    } else {
        let language = app
            .state::<AppState>()
            .inner
            .clone()
            .lock_owned()
            .await
            .config
            .language
            .clone();
        set_convert(
            &app,
            "failed",
            0.0,
            None,
            Some(text(&language, "a conversão falhou.", "conversion failed.")),
        )
        .await;
    }
}

async fn set_convert(
    app: &AppHandle,
    state: &str,
    progress: f64,
    output: Option<String>,
    error: Option<String>,
) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    if let Some(job) = inner.convert_job.as_mut() {
        job.state = state.into();
        job.progress = progress;
        if output.is_some() {
            job.output = output;
        }
        job.error = error;
        let job = job.clone();
        drop(inner);
        emit(app, &BobEvent::Convert { job });
    }
}

async fn clipboard_loop(app: AppHandle) {
    let mut last = String::new();
    loop {
        tokio::time::sleep(Duration::from_millis(1500)).await;
        let watch = app
            .state::<AppState>()
            .inner
            .clone()
            .lock_owned()
            .await
            .config
            .clipboard_watch;
        if !watch {
            continue;
        }
        if let Some(url) = clipboard_url() {
            if url != last {
                last = url.clone();
                emit(&app, &BobEvent::Clipboard { url });
            }
        }
    }
}

fn clipboard_url() -> Option<String> {
    let mut clipboard = arboard::Clipboard::new().ok()?;
    let text = clipboard.get_text().ok()?;
    extract_urls(&text).into_iter().next()
}

fn start_http(app: AppHandle) {
    std::thread::spawn(move || {
        let Ok(server) = tiny_http::Server::http("127.0.0.1:47821") else {
            return;
        };
        for mut request in server.incoming_requests() {
            let method = request.method().as_str().to_string();
            let url = request.url().to_string();
            if url.starts_with("/drive/") || url.starts_with("/cover/") {
                if method == "OPTIONS" {
                    let mut response = tiny_http::Response::from_string(String::new()).with_status_code(tiny_http::StatusCode(204));
                    for (name, value) in crate::drive::cors() {
                        if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
                            response.add_header(header);
                        }
                    }
                    let _ = request.respond(response);
                    continue;
                }
            }
            if method == "GET" && url.starts_with("/cover/") {
                let id = url.trim_start_matches("/cover/").split(['?', '#']).next().unwrap_or("");
                if let Some(path) = crate::drive::cover_file(id) {
                    if let Ok(file) = std::fs::File::open(&path) {
                        let length = file.metadata().ok().map(|meta| meta.len() as usize);
                        let mut headers = Vec::new();
                        for (name, value) in crate::drive::cors() {
                            if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
                                headers.push(header);
                            }
                        }
                        if let Ok(header) = tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/jpeg"[..]) {
                            headers.push(header);
                        }
                        let response = tiny_http::Response::new(tiny_http::StatusCode(200), headers, file, length, None);
                        let _ = request.respond(response);
                        continue;
                    }
                }
            }
            if method == "GET" && url.starts_with("/drive/") {
                let id = url.trim_start_matches("/drive/");
                let range = request.headers().iter().find_map(|header| {
                    header
                        .field
                        .equiv("Range")
                        .then(|| header.value.as_str().to_string())
                });
                match crate::drive::open_media(id, range.as_deref()) {
                    Ok(stream) => {
                        let mut headers = Vec::new();
                        for (name, value) in stream.headers {
                            if let Ok(header) = tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()) {
                                headers.push(header);
                            }
                        }
                        let response = tiny_http::Response::new(
                            tiny_http::StatusCode(stream.status),
                            headers,
                            stream.body,
                            stream.length,
                            None,
                        );
                        let _ = request.respond(response);
                    }
                    Err(message) => {
                        let _ = request.respond(tiny_http::Response::from_string(message).with_status_code(tiny_http::StatusCode(502)));
                    }
                }
                continue;
            }
            let mut body = String::new();
            {
                use std::io::Read;
                let _ = request.as_reader().take(65_536).read_to_string(&mut body);
            }
            let (status, payload) = if method == "OPTIONS" {
                (204, String::new())
            } else if method == "GET" && url.starts_with("/v1/health") {
                (200, r#"{"ok":true,"name":"bob baixa"}"#.into())
            } else if method == "POST" && url.starts_with("/v1/download") {
                let parsed = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| {
                        value
                            .get("url")
                            .and_then(|url| url.as_str())
                            .map(str::to_string)
                    })
                    .filter(|url| url.starts_with("http://") || url.starts_with("https://"));
                if let Some(url) = parsed {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = enqueue(
                            &app,
                            vec![EnqueueItem {
                                url,
                                ..EnqueueItem::default()
                            }],
                        )
                        .await;
                    });
                    (200, r#"{"ok":true}"#.into())
                } else {
                    (400, r#"{"ok":false}"#.into())
                }
            } else {
                (404, r#"{"ok":false}"#.into())
            };
            let response = tiny_http::Response::from_string(payload)
                .with_status_code(tiny_http::StatusCode(status))
                .with_header(header("Access-Control-Allow-Origin", "*"))
                .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
                .with_header(header("Access-Control-Allow-Headers", "content-type"))
                .with_header(header("Content-Type", "application/json"));
            let _ = request.respond(response);
        }
    });
}

fn hotkey_bindings(config: &Config) -> Vec<(String, String)> {
    let mut bindings = Vec::new();
    if config.hotkey {
        bindings.push(("ctrl+shift+b".into(), "paste".into()));
    }
    for (shortcut, action) in [
        (config.hotkey_play.as_str(), "toggle"),
        (config.hotkey_next.as_str(), "next"),
        (config.hotkey_prev.as_str(), "previous"),
        (config.hotkey_like.as_str(), "like"),
    ] {
        let shortcut = shortcut.trim();
        if !shortcut.is_empty() {
            bindings.push((shortcut.to_string(), action.to_string()));
        }
    }
    bindings
}

fn normalize_shortcut(value: &str) -> String {
    value
        .split('+')
        .map(|part| part.trim().to_ascii_lowercase())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("+")
}

fn shortcut_map() -> &'static std::sync::Mutex<Vec<(String, String)>> {
    static MAP: std::sync::OnceLock<std::sync::Mutex<Vec<(String, String)>>> = std::sync::OnceLock::new();
    MAP.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn register_hotkey(app: &AppHandle) {
    let bindings = app
        .state::<AppState>()
        .inner
        .try_lock()
        .map(|inner| hotkey_bindings(&inner.config))
        .unwrap_or_default();
    sync_hotkeys(app, &bindings);
}

fn sync_hotkeys(app: &AppHandle, bindings: &[(String, String)]) {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
    let mut known = shortcut_map().lock().unwrap_or_else(|error| error.into_inner());
    for (shortcut, _) in known.iter() {
        if let Ok(parsed) = shortcut.parse::<Shortcut>() {
            let _ = app.global_shortcut().unregister(parsed);
        }
    }
    known.clear();
    for (shortcut, action) in bindings {
        let Ok(parsed) = shortcut.parse::<Shortcut>() else {
            continue;
        };
        if app.global_shortcut().register(parsed.clone()).is_ok() {
            known.push((normalize_shortcut(&parsed.to_string()), action.clone()));
        }
    }
}

pub fn on_global_shortcut(app: &AppHandle, shortcut: &str) {
    let key = normalize_shortcut(shortcut);
    let action = shortcut_map()
        .lock()
        .ok()
        .and_then(|map| map.iter().find(|(shortcut, _)| shortcut == &key).map(|(_, action)| action.clone()));
    match action.as_deref() {
        Some("paste") => hotkey(app),
        Some(action) => {
            let _ = app.emit("player-hotkey", action.to_string());
        }
        None => {}
    }
}

fn setup_tray(app: &AppHandle) {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let Ok(open) = MenuItem::with_id(app, "open", "abrir", true, None::<&str>) else {
        return;
    };
    let paste = MenuItem::with_id(
        app,
        "paste",
        "baixar da área de transferência",
        true,
        None::<&str>,
    );
    let quit = MenuItem::with_id(app, "quit", "sair", true, None::<&str>);
    let Ok(paste) = paste else { return };
    let Ok(quit) = quit else { return };
    let Ok(menu) = Menu::with_items(app, &[&open, &paste, &quit]) else {
        return;
    };
    let icon = tauri::include_image!("icons/32x32.png");
    let _ = TrayIconBuilder::with_id("bob")
        .icon(icon)
        .menu(&menu)
        .tooltip("bob baixa")
        .on_menu_event(|app, event| match event.id.as_ref() {
            "open" => show_main(app),
            "paste" => hotkey(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main(tray.app_handle());
            }
        })
        .build(app);
}

pub(crate) async fn ytdlp_path(app: &AppHandle) -> Result<PathBuf, String> {
    let inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.ytdlp.clone().ok_or_else(|| {
        text(
            &inner.config.language,
            "o yt-dlp ainda não está pronto.",
            "yt-dlp is not ready yet.",
        )
    })
}

async fn capture(bin: &Path, args: &[String]) -> Result<String, String> {
    let mut cmd = tokio_command(bin);
    cmd.args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(180), cmd.output())
        .await
        .map_err(|_| "tempo esgotado".to_string())?
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let message = format!("{err}\n{out}").trim().to_string();
        return Err(if message.is_empty() {
            "o yt-dlp falhou".into()
        } else {
            message
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

async fn command_stdout(bin: &Path, args: &[String]) -> Option<String> {
    let output = tokio_command(bin).args(args).output().await.ok()?;
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn command_version(bin: &Path, arg: &str) -> Option<String> {
    let output = tokio_command(bin).arg(arg).output().await.ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    if arg == "-version" {
        Some(line.split_whitespace().nth(2).unwrap_or(line).to_string())
    } else {
        Some(line.to_string())
    }
}

async fn set_tools(
    app: &AppHandle,
    ytdlp: &str,
    ffmpeg: &str,
    progress: Option<f64>,
    message: Option<String>,
) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.tools.ytdlp = ytdlp.into();
    inner.tools.ffmpeg = ffmpeg.into();
    inner.tools.app_version = Some(env!("CARGO_PKG_VERSION").into());
    inner.tools.progress = progress;
    inner.tools.message = message;
    let tools = inner.tools.clone();
    drop(inner);
    emit(app, &BobEvent::Tools { tools });
}

pub(crate) async fn download_to(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    start: f64,
    end: f64,
) -> Result<(), String> {
    let part = partial_path(dest);
    let result = download_part(app, url, &part, start, end).await;
    if let Err(error) = result {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(error);
    }
    if tokio::fs::try_exists(dest).await.unwrap_or(false) {
        tokio::fs::remove_file(dest)
            .await
            .map_err(|error| error.to_string())?;
    }
    if let Err(error) = tokio::fs::rename(&part, dest).await {
        let _ = tokio::fs::remove_file(&part).await;
        return Err(error.to_string());
    }
    Ok(())
}

async fn download_part(
    app: &AppHandle,
    url: &str,
    dest: &Path,
    start: f64,
    end: f64,
) -> Result<(), String> {
    let client = reqwest::Client::builder()
        .user_agent("bob-baixa")
        .build()
        .map_err(|error| error.to_string())?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("http {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest)
        .await
        .map_err(|error| error.to_string())?;
    let mut got = 0u64;
    use tokio::io::AsyncWriteExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        file.write_all(&chunk)
            .await
            .map_err(|error| error.to_string())?;
        got += chunk.len() as u64;
        if total > 0 {
            let ratio = start + (end - start) * (got as f64 / total as f64);
            set_tools(app, "downloading", "downloading", Some(ratio), None).await;
        }
    }
    file.flush().await.map_err(|error| error.to_string())?;
    Ok(())
}

fn partial_path(dest: &Path) -> PathBuf {
    let name = dest
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("download");
    dest.with_file_name(format!("{name}.part"))
}

fn file_ready(path: &Path, min_bytes: u64) -> bool {
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.len() >= min_bytes)
        .unwrap_or(false)
}

pub(crate) fn acquire_tools(app: &AppHandle) -> Option<Busy> {
    try_busy(&app.state::<AppState>().tools_busy.clone())
}

pub(crate) fn acquire_update(app: &AppHandle) -> Option<Busy> {
    try_busy(&app.state::<AppState>().update_busy.clone())
}

pub(crate) fn unzip_ffmpeg(zip_path: &Path, bin_dir: &Path) -> Result<(), String> {
    let file = std::fs::File::open(zip_path).map_err(|error| error.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|error| error.to_string())?;
    let mut found = false;
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = entry.name().replace('\\', "/");
        let file_name = name.rsplit('/').next().unwrap_or("");
        if file_name != "ffmpeg.exe" && file_name != "ffprobe.exe" {
            continue;
        }
        if !(name.contains("/bin/")
            || name.ends_with("ffmpeg.exe")
            || name.ends_with("ffprobe.exe"))
        {
            continue;
        }
        let mut out =
            std::fs::File::create(bin_dir.join(file_name)).map_err(|error| error.to_string())?;
        std::io::copy(&mut entry, &mut out).map_err(|error| error.to_string())?;
        found = true;
    }
    if found {
        Ok(())
    } else {
        Err("o pacote do ffmpeg não tinha o executável".into())
    }
}

fn ytdlp_url() -> &'static str {
    if cfg!(windows) {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp.exe"
    } else {
        "https://github.com/yt-dlp/yt-dlp/releases/latest/download/yt-dlp_linux"
    }
}

fn ffmpeg_zip_url() -> &'static str {
    "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip"
}

fn which(name: &str) -> Option<PathBuf> {
    let Ok(path) = std::env::var("PATH") else {
        return None;
    };
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn default_output() -> String {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bob baixa")
        .to_string_lossy()
        .into_owned()
}

fn public_watches(watches: &[Watch]) -> Vec<Watch> {
    watches
        .iter()
        .map(|watch| Watch {
            known: watch.seen.len() as u32,
            seen: Vec::new(),
            ..watch.clone()
        })
        .collect()
}

pub(crate) fn save(inner: &Inner) {
    if inner.data_dir.as_os_str().is_empty() {
        return;
    }
    let _ = std::fs::create_dir_all(&inner.data_dir);
    write_json(&inner.data_dir.join("config.json"), &inner.config);
    write_json(&inner.data_dir.join("jobs.json"), &inner.jobs);
    write_json(&inner.data_dir.join("history.json"), &inner.history);
    write_json(&inner.data_dir.join("watches.json"), &inner.watches);
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_json<T: Serialize>(path: &Path, value: &T) {
    if let Ok(text) = serde_json::to_string_pretty(value) {
        let _ = std::fs::write(path, text);
    }
}

pub(crate) fn emit(app: &AppHandle, event: &BobEvent) {
    let _ = app.emit("bob", event);
}

fn toast(app: &AppHandle, message: &str) {
    let _ = app.emit(
        "bob",
        BobEvent::Toast {
            message: message.to_string(),
        },
    );
}

fn notify(app: &AppHandle, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title("bob baixa")
        .body(body)
        .show();
}

fn header(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header")
}

fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = crate::hidden::std_command("taskkill")
            .args(["/F", "/T", "/PID", &pid.to_string()])
            .output();
    }
    #[cfg(unix)]
    {
        unsafe {
            libc::kill(-(pid as i32), libc::SIGKILL);
            libc::kill(pid as i32, libc::SIGKILL);
        }
    }
}

// re-export parser used above; defined in bob-core as a free function after this comment is wrong.
// The function lives in bob-core::parse and is pub.

#[cfg(test)]
mod tests {
    use super::release_notes;

    #[test]
    fn bundled_notes_match_this_version() {
        let notes = release_notes();
        assert!(notes.contains("## 0.4.1"), "{notes}");
        assert!(notes.contains("\\\\?\\") || notes.contains("biblioteca"));
        assert!(!notes.contains("## 0.4.0"));
    }
}
