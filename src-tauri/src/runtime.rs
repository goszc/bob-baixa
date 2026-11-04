use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bob_core::{
    build_download_args, cookie_lock_hint, detect_browsers, extract_urls, ffmpeg_args, iso_in_future, looks_like_auth,
    looks_like_cookie_lock, new_id, now_iso, output_for, parse_ffmpeg_seconds, parse_json_value_text, parse_media,
    parse_output_path, parse_progress, parse_search, parse_title_line, pick_browser, text, url_from_link, BobEvent, Config,
    ConvertJob, EnqueueItem, HistoryEntry, Job, MediaItem, SearchHit, ToolStatus, Watch,
};
use futures_util::StreamExt;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;
use tokio::io::{AsyncBufReadExt, BufReader};
use crate::hidden::tokio_command;
use std::sync::Arc as StdArc;
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
    pub last_emit: Instant,
    pub last_update_check: Option<String>,
    pub ffmpeg_stamp: Option<String>,
    pub pending_installer: Option<PathBuf>,
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
            last_emit: Instant::now(),
            last_update_check: None,
            ffmpeg_stamp: None,
            pending_installer: None,
        }
    }
}

pub struct AppState {
    pub inner: StdArc<Mutex<Inner>>,
}

impl Default for AppState {
    fn default() -> Self {
        Self { inner: StdArc::new(Mutex::new(Inner::default())) }
    }
}

const PAUSE: u8 = 1;
const CANCEL: u8 = 2;

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
        register_hotkey(&app);
        setup_tray(&app);
        let args: Vec<String> = std::env::args().skip(1).collect();
        open_args(&app, args);
    });
}

async fn prepare(app: &AppHandle) {
    let data_dir = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("bob baixa"));
    let _ = std::fs::create_dir_all(&data_dir);
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.data_dir = data_dir.clone();
    if let Some(config) = read_json::<Config>(&data_dir.join("config.json")) {
        inner.config = config;
    }
    if inner.config.output_dir.trim().is_empty() {
        inner.config.output_dir = default_output();
    }
    if let Some(mut jobs) = read_json::<Vec<Job>>(&data_dir.join("jobs.json")) {
        for job in &mut jobs {
            if job.state == "downloading" {
                job.state = "paused".into();
            }
        }
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
    let config = inner.config.clone();
    drop(inner);
    emit(app, &BobEvent::Jobs { jobs });
    emit(app, &BobEvent::Tools { tools });
    emit(app, &BobEvent::History { history });
    emit(app, &BobEvent::Watches { watches });
    let _ = config;
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
    }
}

pub async fn set_config(app: &AppHandle, mut config: Config) -> Result<(), String> {
    if config.output_dir.trim().is_empty() {
        config.output_dir = default_output();
    }
    config.parallel = config.parallel.clamp(1, 4);
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let hotkey_changed = inner.config.hotkey != config.hotkey;
        inner.config = config;
        save(&inner);
        let enabled = inner.config.hotkey;
        drop(inner);
        if hotkey_changed {
            if enabled {
                register_hotkey(app);
            } else {
                unregister_hotkey(app);
            }
        }
    }
    Ok(())
}

pub async fn analyze(app: &AppHandle, urls: Vec<String>) -> Result<Vec<MediaItem>, String> {
    let bin = ytdlp_path(app).await?;
    let mut items = Vec::new();
    for url in urls {
        let args = vec![
            "-J".into(),
            "--flat-playlist".into(),
            "--no-warnings".into(),
            "--skip-download".into(),
            url.clone(),
        ];
        let output = capture(&bin, &args).await?;
        let value = parse_json_value_text(&output)?;
        let mut item = parse_media(&value, &url);
        if item.kind == "video" && item.formats.is_empty() {
            let full = capture(
                &bin,
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
        items.push(item);
    }
    Ok(items)
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
            inner.jobs.insert(0, Job::from_item(item, &config, hold.as_deref()));
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
    match action {
        "pause" | "cancel" => {
            if let Some(control) = control {
                control.reason.store(if action == "pause" { PAUSE } else { CANCEL }, Ordering::Relaxed);
                control.notify.notify_waiters();
                control.notify.notify_one();
                return Ok(());
            }
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
                job.state = if action == "pause" { "paused" } else { "cancelled" }.into();
            }
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
        }
        "resume" | "retry" => {
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
                if action == "retry" {
                    job.progress = 0.0;
                    job.error = None;
                    job.downloaded = None;
                }
                job.state = "queued".into();
                job.scheduled_at = None;
            }
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
            pump(app).await;
        }
        "remove" => {
            if let Some(control) = control {
                control.reason.store(CANCEL, Ordering::Relaxed);
                control.notify.notify_one();
            }
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            inner.jobs.retain(|job| job.id != id);
            inner.controls.remove(&id);
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
        }
        _ => {}
    }
    Ok(())
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
    emit(app, &BobEvent::History { history: Vec::new() });
}

pub async fn convert(app: &AppHandle, input: String, preset: String, bitrate: String) -> Result<(), String> {
    let ffmpeg = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.ffmpeg_bin.clone().ok_or_else(|| {
            text(&inner.config.language, "o ffmpeg ainda não está pronto.", "ffmpeg is not ready yet.")
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
    app.state::<AppState>().inner.clone().blocking_lock_owned().config.minimize_to_tray
}

pub fn open_args(app: &AppHandle, args: Vec<String>) {
    let urls: Vec<String> = args.iter().filter_map(|arg| url_from_link(arg)).collect();
    if urls.is_empty() {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let _ = enqueue(&app, urls.into_iter().map(|url| EnqueueItem { url, ..EnqueueItem::default() }).collect()).await;
        show_main(&app);
    });
}

pub fn hotkey(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let enabled = app.state::<AppState>().inner.clone().lock_owned().await.config.hotkey;
        if !enabled {
            return;
        }
        let language = app.state::<AppState>().inner.clone().lock_owned().await.config.language.clone();
        match clipboard_url() {
            Some(url) => {
                let _ = enqueue(&app, vec![EnqueueItem { url, ..EnqueueItem::default() }]).await;
                show_main(&app);
            }
            None => toast(
                &app,
                &text(&language, "nenhum link na área de transferência.", "no link on the clipboard."),
            ),
        }
    });
}

pub async fn install_tools(app: &AppHandle, force: bool) -> Result<(), String> {
    let data_dir = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if inner.data_dir.as_os_str().is_empty() {
            return Err("pasta do aplicativo indisponível".into());
        }
        inner.data_dir.clone()
    };
    let bin_dir = data_dir.join("bin");
    let _ = std::fs::create_dir_all(&bin_dir);
    let language = app.state::<AppState>().inner.clone().lock_owned().await.config.language.clone();
    set_tools(app, "downloading", "downloading", Some(0.05), None).await;

    let ytdlp_name = if cfg!(windows) { "yt-dlp.exe" } else { "yt-dlp" };
    let ytdlp_path = bin_dir.join(ytdlp_name);
    if force || !ytdlp_path.exists() {
        if let Err(error) = download_to(app, ytdlp_url(), &ytdlp_path, 0.15, 0.4).await {
            if which("yt-dlp").is_none() && which("yt-dlp.exe").is_none() {
                set_tools(app, "error", "missing", None, Some(error.clone())).await;
                return Err(error);
            }
        } else {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&ytdlp_path, std::fs::Permissions::from_mode(0o755));
            }
        }
    }
    let ytdlp = if ytdlp_path.exists() {
        ytdlp_path
    } else {
        which("yt-dlp").or_else(|| which("yt-dlp.exe")).ok_or_else(|| {
            text(&language, "não foi possível obter o yt-dlp.", "could not get yt-dlp.")
        })?
    };

    let ffmpeg_bin = if cfg!(windows) {
        let exe = bin_dir.join("ffmpeg.exe");
        let probe = bin_dir.join("ffprobe.exe");
        if force || !exe.exists() {
            let zip_path = bin_dir.join("ffmpeg.zip");
            download_to(app, ffmpeg_zip_url(), &zip_path, 0.45, 0.9).await?;
            unzip_ffmpeg(&zip_path, &bin_dir)?;
            let _ = std::fs::remove_file(&zip_path);
        }
        if exe.exists() { Some((bin_dir.clone(), exe, probe.exists().then_some(probe))) } else { None }
    } else {
        None
    };
    let (ffmpeg_dir, ffmpeg_bin, ffprobe) = if let Some(found) = ffmpeg_bin {
        found
    } else if let Some(path) = which("ffmpeg") {
        let dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
        let probe = dir.join(if cfg!(windows) { "ffprobe.exe" } else { "ffprobe" });
        (dir, path, probe.exists().then_some(probe))
    } else {
        set_tools(app, "ready", "error", None, Some(text(&language, "ffmpeg não encontrado.", "ffmpeg was not found."))).await;
        return Err(text(&language, "ffmpeg não encontrado.", "ffmpeg was not found."));
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
    let language = app.state::<AppState>().inner.clone().lock_owned().await.config.language.clone();
    let updated = tokio_command(&bin).arg("-U").output().await;
    let ok = updated.as_ref().map(|output| output.status.success()).unwrap_or(false);
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
    toast(&app, &text(&language, "yt-dlp atualizado", "yt-dlp updated"));
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
                .filter(|watch| watch.enabled && watch_due(watch))
                .map(|watch| watch.id.clone())
                .collect()
        };
        for id in due {
            check_watch(&app, id).await;
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
        if job.state == "scheduled" && job.scheduled_at.as_deref().map(|at| !iso_in_future(at)).unwrap_or(false) {
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
    let mut start = Vec::new();
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let running = inner.jobs.iter().filter(|job| job.state == "downloading").count();
        let slots = (inner.config.parallel.max(1) as usize).saturating_sub(running);
        for job in &mut inner.jobs {
            if start.len() >= slots {
                break;
            }
            if job.state == "queued" {
                job.state = "downloading".into();
                job.error = None;
                start.push(job.id.clone());
            }
        }
        if !start.is_empty() {
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(app, &BobEvent::Jobs { jobs });
        }
    }
    for id in start {
        let app = app.clone();
        tauri::async_runtime::spawn(async move { run_download(app, id).await });
    }
}

async fn run_download(app: AppHandle, id: String) {
    let (bin, ffmpeg, job) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        match (inner.ytdlp.clone(), inner.ffmpeg_dir.clone(), inner.jobs.iter().find(|job| job.id == id).cloned()) {
            (Some(bin), Some(ffmpeg), Some(job)) => (bin, ffmpeg, job),
            _ => return,
        }
    };
    if !job.output_dir.trim().is_empty() {
        let _ = std::fs::create_dir_all(&job.output_dir);
    }
    let args = build_download_args(&job, &ffmpeg.to_string_lossy());
    let mut child = match {
        let mut cmd = tokio_command(&bin);
        cmd.args(&args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
        cmd.spawn()
    } {
        Ok(child) => child,
        Err(error) => {
            finish(&app, &id, "failed", Some(error.to_string()), None).await;
            return;
        }
    };
    let pid = child.id().unwrap_or(0);
    let control = Arc::new(Control { notify: Notify::new(), reason: AtomicU8::new(0) });
    app.state::<AppState>().inner.clone().lock_owned().await.controls.insert(id.clone(), control.clone());
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let app_out = app.clone();
    let id_out = id.clone();
    let reader = tauri::async_runtime::spawn(async move {
        if let Some(stdout) = stdout {
            read_pipe(app_out.clone(), id_out.clone(), stdout, false).await;
        }
        if let Some(stderr) = stderr {
            read_pipe(app_out, id_out, stderr, true).await;
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
    reader.abort();
    let reason = control.reason.load(Ordering::Relaxed);
    app.state::<AppState>().inner.clone().lock_owned().await.controls.remove(&id);
    let code_ok = status.as_ref().map(|status| status.success()).unwrap_or(false);
    if code_ok && reason == 0 {
        let path = {
            let inner = app.state::<AppState>().inner.clone().lock_owned().await;
            inner.jobs.iter().find(|job| job.id == id).and_then(|job| job.output_path.clone())
        };
        finish(&app, &id, "done", None, path).await;
    } else if reason == PAUSE {
        finish(&app, &id, "paused", None, None).await;
    } else if reason == CANCEL {
        finish(&app, &id, "cancelled", None, None).await;
    } else {
        let tail = app.state::<AppState>().inner.clone().lock_owned().await.logs.get(&id).cloned().unwrap_or_default();
        let (language, retried, preference) = {
            let inner = app.state::<AppState>().inner.clone().lock_owned().await;
            let job = inner.jobs.iter().find(|job| job.id == id);
            (
                inner.config.language.clone(),
                job.map(|job| job.cookie_retried).unwrap_or(true),
                job.map(|job| job.cookies_browser.clone()).unwrap_or_default(),
            )
        };
        if looks_like_cookie_lock(&tail) {
            finish(&app, &id, "failed", Some(cookie_lock_hint(&language)), None).await;
            return;
        }
        let (update_retried, auto_update) = {
            let inner = app.state::<AppState>().inner.clone().lock_owned().await;
            let job = inner.jobs.iter().find(|job| job.id == id);
            (job.map(|job| job.update_retried).unwrap_or(true), inner.config.auto_update)
        };
        if auto_update && !update_retried && bob_core::looks_like_extractor(&tail) {
            let _ = crate::updates::refresh_ytdlp(&app).await;
            let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
            if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
                job.update_retried = true;
                job.state = "queued".into();
                job.error = None;
                job.progress = 0.0;
            }
            save(&inner);
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(&app, &BobEvent::Jobs { jobs });
            schedule_pump(&app);
            return;
        }
        let can_retry = !retried && (preference == "auto" || preference.is_empty());
        if can_retry && looks_like_auth(&tail) {
            if let Some(browser) = pick_browser("auto", &detect_browsers()) {
                let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
                if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
                    job.cookies_browser = browser;
                    job.cookie_retried = true;
                    job.state = "queued".into();
                    job.error = None;
                    job.progress = 0.0;
                }
                save(&inner);
                let jobs = inner.jobs.clone();
                drop(inner);
                emit(&app, &BobEvent::Jobs { jobs });
                schedule_pump(&app);
                return;
            }
        }
        let message = if tail.trim().is_empty() {
            text(&language, "o download falhou.", "the download failed.")
        } else {
            tail.lines().rev().find(|line| !line.trim().is_empty()).unwrap_or("erro").to_string()
        };
        finish(&app, &id, "failed", Some(message), None).await;
    }
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
    if std::fs::metadata(&path).map(|meta| meta.len() > 400).unwrap_or(false) {
        return Ok(path.to_string_lossy().into_owned());
    }
    let client = reqwest::Client::builder().user_agent("bob-baixa").build().map_err(|error| error.to_string())?;
    let bytes = client.get(url).send().await.map_err(|error| error.to_string())?.error_for_status().map_err(|error| error.to_string())?.bytes().await.map_err(|error| error.to_string())?;
    tokio::fs::write(&path, &bytes).await.map_err(|error| error.to_string())?;
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

async fn read_pipe(app: AppHandle, id: String, pipe: impl tokio::io::AsyncRead + Unpin, is_err: bool) {
    let mut lines = BufReader::new(pipe).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let mut important = false;
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) else {
            break;
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
            let log = inner.logs.entry(id.clone()).or_default();
            log.push_str(&line);
            log.push('\n');
            if log.len() > 4000 {
                *log = log[log.len() - 4000..].to_string();
            }
        }
        let should_emit = important || inner.last_emit.elapsed() > Duration::from_millis(180);
        if should_emit {
            inner.last_emit = Instant::now();
            let jobs = inner.jobs.clone();
            drop(inner);
            emit(&app, &BobEvent::Jobs { jobs });
        }
    }
}

async fn finish(app: &AppHandle, id: &str, state: &str, error: Option<String>, path: Option<String>) {
    let mut notify_body = None;
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let language = inner.config.language.clone();
    let notifications = inner.config.notifications;
    let mut record = None;
    if let Some(job) = inner.jobs.iter_mut().find(|job| job.id == id) {
        job.state = state.into();
        if state == "done" {
            job.progress = 1.0;
            job.error = None;
            if path.is_some() {
                job.output_path = path.clone();
            }
        }
        if let Some(error) = error.clone() {
            job.error = Some(error);
        }
        if state == "done" || state == "failed" {
            record = Some(HistoryEntry {
                id: new_id("hist"),
                title: job.title.clone(),
                url: job.url.clone(),
                path: job.output_path.clone(),
                at: now_iso(),
                ok: state == "done",
                detail: job.error.clone(),
            });
            if notifications {
                notify_body = Some(format!(
                    "{} — {}",
                    job.title,
                    if state == "done" {
                        text(&language, "concluído", "done")
                    } else {
                        text(&language, "falhou", "failed")
                    }
                ));
            }
        }
    }
    if let Some(entry) = record {
        inner.history.insert(0, entry);
        inner.history.truncate(400);
    }
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
    schedule_pump(app);
}

async fn check_watch(app: &AppHandle, id: String) {
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
    let Ok(bin) = ytdlp_path(app).await else { return };
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
    let Ok(value) = parse_json_value_text(&output) else { return };
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
        if media.title != "playlist" && !media.title.is_empty() && (watch.title.is_empty() || watch.title == watch.url) {
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
            &text(&language, "vídeo novo na fila.", "a new video is in the queue."),
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

async fn run_convert(app: AppHandle, ffmpeg: PathBuf, input: String, output: String, preset: String, bitrate: String, stop: Arc<Notify>) {
    let probe = app.state::<AppState>().inner.clone().lock_owned().await.ffprobe_bin.clone();
    let duration = if let Some(probe) = probe {
        command_stdout(&probe, &["-v".into(), "error".into(), "-show_entries".into(), "format=duration".into(), "-of".into(), "csv=p=0".into(), input.clone()]).await
            .and_then(|text| text.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    } else {
        0.0
    };
    let args = ffmpeg_args(&input, &output, &preset, &bitrate);
    let mut cmd = tokio_command(&ffmpeg);
    cmd.args(&args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(error) => {
            set_convert(&app, "failed", 0.0, None, Some(error.to_string())).await;
            return;
        }
    };
    let pid = child.id().unwrap_or(0);
    let stdout = child.stdout.take();
    let app_progress = app.clone();
    let reader = tauri::async_runtime::spawn(async move {
        if let Some(stdout) = stdout {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if let Some(seconds) = parse_ffmpeg_seconds(&line) {
                    let progress = if duration > 0.0 { (seconds / duration).clamp(0.0, 0.99) } else { 0.0 };
                    set_convert(&app_progress, "running", progress, None, None).await;
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
    let was_cancelled = cancelled.load(Ordering::Relaxed);
    if status.as_ref().map(|status| status.success()).unwrap_or(false) {
        set_convert(&app, "done", 1.0, Some(output), None).await;
        let language = app.state::<AppState>().inner.clone().lock_owned().await.config.language.clone();
        toast(&app, &text(&language, "conversão concluída", "conversion finished"));
    } else if was_cancelled {
        set_convert(&app, "cancelled", 0.0, None, None).await;
    } else {
        let language = app.state::<AppState>().inner.clone().lock_owned().await.config.language.clone();
        set_convert(&app, "failed", 0.0, None, Some(text(&language, "a conversão falhou.", "conversion failed."))).await;
    }
}

async fn set_convert(app: &AppHandle, state: &str, progress: f64, output: Option<String>, error: Option<String>) {
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
        let watch = app.state::<AppState>().inner.clone().lock_owned().await.config.clipboard_watch;
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
            let mut body = String::new();
            let _ = request.as_reader().read_to_string(&mut body);
            let (status, payload) = if method == "OPTIONS" {
                (204, String::new())
            } else if method == "GET" && url.starts_with("/v1/health") {
                (200, r#"{"ok":true,"name":"bob baixa"}"#.into())
            } else if method == "POST" && url.starts_with("/v1/download") {
                let parsed = serde_json::from_str::<serde_json::Value>(&body)
                    .ok()
                    .and_then(|value| value.get("url").and_then(|url| url.as_str()).map(str::to_string))
                    .filter(|url| url.starts_with("http://") || url.starts_with("https://"));
                if let Some(url) = parsed {
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = enqueue(&app, vec![EnqueueItem { url, ..EnqueueItem::default() }]).await;
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

fn register_hotkey(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyB);
    let _ = app.global_shortcut().register(shortcut);
}

fn unregister_hotkey(app: &AppHandle) {
    use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut};
    let shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::KeyB);
    let _ = app.global_shortcut().unregister(shortcut);
}

fn setup_tray(app: &AppHandle) {
    use tauri::menu::{Menu, MenuItem};
    use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
    let Ok(open) = MenuItem::with_id(app, "open", "abrir", true, None::<&str>) else { return };
    let paste = MenuItem::with_id(app, "paste", "baixar da área de transferência", true, None::<&str>);
    let quit = MenuItem::with_id(app, "quit", "sair", true, None::<&str>);
    let Ok(paste) = paste else { return };
    let Ok(quit) = quit else { return };
    let Ok(menu) = Menu::with_items(app, &[&open, &paste, &quit]) else { return };
    let icon = tauri::include_image!("icons/32x32.png");
    let _ = TrayIconBuilder::new()
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
        text(&inner.config.language, "o yt-dlp ainda não está pronto.", "yt-dlp is not ready yet.")
    })
}

async fn capture(bin: &Path, args: &[String]) -> Result<String, String> {
    let mut cmd = tokio_command(bin);
    cmd.args(args).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).kill_on_drop(true);
    let output = tokio::time::timeout(Duration::from_secs(180), cmd.output())
        .await
        .map_err(|_| "tempo esgotado".to_string())?
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        let out = String::from_utf8_lossy(&output.stdout);
        let message = format!("{err}\n{out}").trim().to_string();
        return Err(if message.is_empty() { "o yt-dlp falhou".into() } else { message });
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

async fn set_tools(app: &AppHandle, ytdlp: &str, ffmpeg: &str, progress: Option<f64>, message: Option<String>) {
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

pub(crate) async fn download_to(app: &AppHandle, url: &str, dest: &Path, start: f64, end: f64) -> Result<(), String> {
    let client = reqwest::Client::builder().user_agent("bob-baixa").build().map_err(|error| error.to_string())?;
    let response = client.get(url).send().await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("http {}", response.status()));
    }
    let total = response.content_length().unwrap_or(0);
    let mut stream = response.bytes_stream();
    let mut file = tokio::fs::File::create(dest).await.map_err(|error| error.to_string())?;
    let mut got = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        use tokio::io::AsyncWriteExt;
        file.write_all(&chunk).await.map_err(|error| error.to_string())?;
        got += chunk.len() as u64;
        if total > 0 {
            let ratio = start + (end - start) * (got as f64 / total as f64);
            set_tools(app, "downloading", "downloading", Some(ratio), None).await;
        }
    }
    Ok(())
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
        if !(name.contains("/bin/") || name.ends_with("ffmpeg.exe") || name.ends_with("ffprobe.exe")) {
            continue;
        }
        let mut out = std::fs::File::create(bin_dir.join(file_name)).map_err(|error| error.to_string())?;
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
    let Ok(path) = std::env::var("PATH") else { return None };
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

fn save(inner: &Inner) {
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
    let _ = app.emit("bob", BobEvent::Toast { message: message.to_string() });
}

fn notify(app: &AppHandle, body: &str) {
    let _ = app.notification().builder().title("bob baixa").body(body).show();
}

fn header(name: &str, value: &str) -> tiny_http::Header {
    tiny_http::Header::from_bytes(name.as_bytes(), value.as_bytes()).expect("header")
}

fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let _ = crate::hidden::std_command("taskkill").args(["/F", "/T", "/PID", &pid.to_string()]).output();
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