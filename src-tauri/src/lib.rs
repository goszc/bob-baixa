mod ai;
mod diagnose;
mod discord;
mod drive;
mod enrich;
mod hub;
mod loudness;
mod spotify;
mod hidden;
mod media_session;
mod playback;
mod runtime;
mod shell_menu;
mod updates;

use bob_core::{
    Config, ConvertJob, EnqueueItem, HistoryEntry, Job, MediaItem, SearchHit, ToolStatus, Watch,
};
use runtime::AppState;
use serde::Serialize;
use tauri::Manager;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Snapshot {
    config: Config,
    jobs: Vec<Job>,
    history: Vec<HistoryEntry>,
    watches: Vec<Watch>,
    tools: ToolStatus,
    schedule_at: Option<String>,
    convert_job: Option<ConvertJob>,
    ready: bool,
    whats_new: bool,
    release_notes: String,
}

#[tauri::command]
async fn state(app: tauri::AppHandle) -> Snapshot {
    runtime::snapshot(&app).await
}

#[tauri::command]
async fn set_config(app: tauri::AppHandle, config: Config) -> Result<(), String> {
    runtime::set_config(&app, config).await
}

#[tauri::command]
async fn analyze(app: tauri::AppHandle, urls: Vec<String>) -> Result<Vec<MediaItem>, String> {
    runtime::analyze(&app, urls).await
}

#[tauri::command]
async fn search_youtube(app: tauri::AppHandle, query: String) -> Result<Vec<SearchHit>, String> {
    runtime::search(&app, query).await
}

#[tauri::command]
async fn enqueue(app: tauri::AppHandle, items: Vec<EnqueueItem>) -> Result<(), String> {
    runtime::enqueue(&app, items).await
}

#[tauri::command]
async fn job_action(app: tauri::AppHandle, id: String, action: String) -> Result<(), String> {
    runtime::job_action(&app, id, &action).await
}

#[tauri::command]
async fn schedule_queue(app: tauri::AppHandle, at: String) -> Result<(), String> {
    runtime::schedule_queue(&app, at).await
}

#[tauri::command]
async fn clear_schedule(app: tauri::AppHandle) -> Result<(), String> {
    runtime::clear_schedule(&app).await
}

#[tauri::command]
async fn convert(
    app: tauri::AppHandle,
    input: String,
    preset: String,
    bitrate: String,
) -> Result<(), String> {
    runtime::convert(&app, input, preset, bitrate).await
}

#[tauri::command]
async fn cancel_convert(app: tauri::AppHandle) {
    runtime::cancel_convert(&app).await;
}

#[tauri::command]
async fn add_watch(app: tauri::AppHandle, url: String, every_minutes: u32) -> Result<(), String> {
    runtime::add_watch(&app, url, every_minutes).await
}

#[tauri::command]
async fn watch_action(app: tauri::AppHandle, id: String, action: String) -> Result<(), String> {
    runtime::watch_action(&app, id, &action).await
}

#[tauri::command]
async fn clear_history(app: tauri::AppHandle) {
    runtime::clear_history(&app).await;
}

#[tauri::command]
async fn update_ytdlp(app: tauri::AppHandle) -> Result<(), String> {
    runtime::update_ytdlp(&app).await
}

#[tauri::command]
async fn check_updates(app: tauri::AppHandle) -> Result<(), String> {
    updates::check_now(&app).await
}

#[tauri::command]
async fn apply_update(app: tauri::AppHandle) -> Result<(), String> {
    updates::apply_update(&app).await
}

#[tauri::command]
async fn install_tools(app: tauri::AppHandle) -> Result<(), String> {
    runtime::install_tools(&app, true).await
}

#[tauri::command]
async fn ack_whats_new(app: tauri::AppHandle) {
    runtime::ack_whats_new(&app).await;
}

#[tauri::command]
fn set_window_material(window: tauri::WebviewWindow, dark: bool, glass: bool) {
    #[cfg(target_os = "windows")]
    {
        if glass {
            let tint = if dark {
                (16u8, 16, 18, 170)
            } else {
                (243, 241, 236, 176)
            };
            if window_vibrancy::apply_acrylic(&window, Some(tint)).is_err() {
                let _ = window_vibrancy::apply_mica(&window, Some(dark));
            }
        } else {
            let _ = window_vibrancy::clear_acrylic(&window);
            let _ = window_vibrancy::clear_mica(&window);
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (window, dark, glass);
    }
}

#[tauri::command]
async fn cache_thumb(app: tauri::AppHandle, url: String) -> Result<String, String> {
    runtime::cache_thumb(&app, &url).await
}

#[tauri::command]
async fn set_now_playing(
    app: tauri::AppHandle,
    title: String,
    artist: String,
    album: Option<String>,
    playing: Option<bool>,
    artwork: Option<String>,
) {
    let tooltip = if title.trim().is_empty() {
        "bob baixa".to_string()
    } else if artist.trim().is_empty() {
        title.clone()
    } else {
        format!("{title} — {artist}")
    };
    if let Some(tray) = app.tray_by_id("bob") {
        let _ = tray.set_tooltip(Some(tooltip));
    }
    let artwork = artwork_file(&app, artwork.unwrap_or_default()).await;
    let playing = playing.unwrap_or(false);
    let album = album.unwrap_or_default();
    let art = media_session::encode_artwork(artwork.as_deref());
    let discord_on = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if inner.config.discord {
            Some((discord_id(&inner.config.discord_client_id), title.clone(), artist.clone(), playing))
        } else {
            discord::clear();
            None
        }
    };
    if let Some((client_id, title, artist, playing)) = discord_on {
        let _ = discord::update(&client_id, &title, &artist, playing);
    }
    let _ = app.run_on_main_thread(move || {
        media_session::apply(
            media_session::NowPlaying {
                title,
                artist,
                album,
                playing,
            },
            art,
        );
    });
}

async fn artwork_file(app: &tauri::AppHandle, artwork: String) -> Option<String> {
    let artwork = artwork.trim();
    if artwork.is_empty() {
        return None;
    }
    if artwork.starts_with("https://") || artwork.starts_with("http://") {
        #[cfg(windows)]
        {
            return runtime::cache_thumb(app, artwork).await.ok();
        }
        #[cfg(not(windows))]
        {
            let _ = app;
            return None;
        }
    }
    std::path::Path::new(artwork)
        .is_file()
        .then(|| artwork.to_string())
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let file = std::path::Path::new(&path);
    let name = file
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(name.as_str(), "txt" | "list" | "urls" | "text") {
        return Err("só arquivos de texto".into());
    }
    let text = std::fs::read_to_string(file).map_err(|error| error.to_string())?;
    Ok(text.chars().take(200_000).collect())
}

#[tauri::command]
async fn open_mini(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::{WebviewUrl, WebviewWindowBuilder};
    if let Some(window) = app.get_webview_window("mini") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_always_on_top(true);
        let _ = window.set_focus();
        return Ok(());
    }
    let window = WebviewWindowBuilder::new(&app, "mini", WebviewUrl::App("index.html?mini=1".into()))
        .title("bob baixa")
        .inner_size(440.0, 92.0)
        .min_inner_size(320.0, 84.0)
        .resizable(true)
        .always_on_top(true)
        .decorations(false)
        .shadow(true)
        .skip_taskbar(false)
        .visible(true)
        .build()
        .map_err(|error| error.to_string())?;
    let _ = window.set_always_on_top(true);
    #[cfg(target_os = "windows")]
    {
        let _ = window_vibrancy::apply_acrylic(&window, Some((18, 18, 20, 190)));
    }
    Ok(())
}

#[tauri::command]
fn focus_main(app: tauri::AppHandle) {
    runtime::show_main(&app);
}

#[tauri::command]
async fn close_mini(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("mini") {
        let _ = window.close();
    }
    Ok(())
}

#[tauri::command]
async fn discord_presence(app: tauri::AppHandle, title: String, artist: String, playing: bool) -> Result<(), String> {
    let (enabled, client_id) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        (inner.config.discord, discord_id(&inner.config.discord_client_id))
    };
    if !enabled {
        discord::clear();
        return Ok(());
    }
    discord::update(&client_id, &title, &artist, playing)
}

fn discord_id(configured: &str) -> String {
    bob_core::discord_client_id(configured, option_env!("BOB_DISCORD_APP_ID").unwrap_or(""))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            runtime::show_main(app);
            runtime::open_args(app, args);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        runtime::on_global_shortcut(app, &shortcut.to_string());
                    }
                })
                .build(),
        )
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if runtime::minimize_on_close(window.app_handle()) {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            runtime::boot(app.handle().clone());
            media_session::install(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            state,
            set_config,
            analyze,
            search_youtube,
            enqueue,
            job_action,
            schedule_queue,
            clear_schedule,
            convert,
            cancel_convert,
            add_watch,
            watch_action,
            clear_history,
            update_ytdlp,
            check_updates,
            apply_update,
            install_tools,
            cache_thumb,
            set_window_material,
            read_text_file,
            ack_whats_new,
            playback::inspect_media,
            playback::load_lyrics,
            playback::search_lyrics,
            playback::scan_library,
            playback::delete_library_track,
            enrich::search_library_lyrics,
            enrich::pin_lyrics,
            enrich::preview_tags,
            enrich::apply_tags,
            enrich::library_issues,
            enrich::start_economic,
            enrich::cancel_economic,
            enrich::translate_lines,
            drive::drive_status,
            drive::drive_connect,
            drive::drive_disconnect,
            drive::drive_queue,
            drive::drive_retry,
            drive::drive_test,
            drive::drive_open,
            drive::drive_tracks,
            drive::drive_refresh,
            drive::save_listens,
            drive::load_listens,
            drive::drive_upload_library,
            drive::drive_push_state,
            drive::drive_pull_state,
            drive::drive_repair,
            hub::set_offline,
            hub::offline_paths,
            hub::drive_space,
            hub::free_cache,
            hub::warm_next,
            hub::list_trash,
            hub::restore_trash,
            hub::purge_trash,
            hub::gemini_status,
            hub::gemini_save_key,
            hub::clean_title,
            hub::lyric_meaning,
            hub::mood_mix,
            hub::natural_queue,
            hub::discover_queries,
            ai::ai_usage,
            ai::ai_wrapped,
            ai::ai_meaning,
            ai::ai_describe,
            ai::ai_similar,
            ai::ai_transcribe,
            loudness::track_gain,
            loudness::backfill_gains,
            open_mini,
            close_mini,
            focus_main,
            discord_presence,
            diagnose::make_diagnostic,
            set_now_playing
        ])
        .run(tauri::generate_context!())
        .expect("bob baixa");
}
