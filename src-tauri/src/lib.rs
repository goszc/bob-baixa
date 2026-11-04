mod runtime;
mod updates;

use bob_core::{Config, ConvertJob, EnqueueItem, HistoryEntry, Job, MediaItem, SearchHit, ToolStatus, Watch};
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
async fn convert(app: tauri::AppHandle, input: String, preset: String, bitrate: String) -> Result<(), String> {
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
async fn cache_thumb(app: tauri::AppHandle, url: String) -> Result<String, String> {
    runtime::cache_thumb(&app, &url).await
}

#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let file = std::path::Path::new(&path);
    let name = file.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
    if !matches!(name.as_str(), "txt" | "list" | "urls" | "text") {
        return Err("só arquivos de texto".into());
    }
    let text = std::fs::read_to_string(file).map_err(|error| error.to_string())?;
    Ok(text.chars().take(200_000).collect())
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
                .with_handler(|app, _shortcut, event| {
                    if event.state == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        runtime::hotkey(app);
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
            read_text_file
        ])
        .run(tauri::generate_context!())
        .expect("bob baixa");
}
