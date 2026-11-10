use std::path::PathBuf;
use std::time::Duration;

use base64::Engine;

use crate::hidden::tokio_command;
use bob_core::{text, BobEvent, Config};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::runtime::{self, AppState};

#[cfg(test)]
const PUBKEY: &str = include_str!("../../packaging/updater.pub");
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
const CHECK_EVERY: Duration = Duration::from_secs(3 * 60 * 60);

pub async fn startup(app: &AppHandle) {
    let auto = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config.auto_update
    };
    if !auto {
        return;
    }
    let _ = run_checks(app, false).await;
}

pub async fn periodic(app: &AppHandle) {
    let due = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        if !inner.config.auto_update {
            return;
        }
        match inner
            .last_update_check
            .as_deref()
            .and_then(|value| chrono::DateTime::parse_from_rfc3339(value).ok())
        {
            Some(time) => {
                chrono::Utc::now().signed_duration_since(time)
                    > chrono::Duration::from_std(CHECK_EVERY).unwrap_or_default()
            }
            None => true,
        }
    };
    if due {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            startup(&app).await;
        });
    }
}

pub async fn check_now(app: &AppHandle) -> Result<(), String> {
    run_checks(app, true).await
}

async fn run_checks(app: &AppHandle, manual: bool) -> Result<(), String> {
    let Some(_busy) = runtime::acquire_update(app) else {
        return if manual {
            Err("já está verificando.".into())
        } else {
            Ok(())
        };
    };
    // Mark before the network calls so the ticker does not start another check every second.
    mark_checked(app).await;
    let language = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.config.language.clone()
    };
    let ytdlp = refresh_ytdlp(app).await.is_ok();
    let ffmpeg = refresh_ffmpeg(app).await.unwrap_or(false);
    let app_update = check_app(app).await.unwrap_or(false);
    if manual && !app_update {
        let message = if ytdlp || ffmpeg {
            text(&language, "ferramentas atualizadas", "tools updated")
        } else {
            text(&language, "tudo atualizado", "everything is up to date")
        };
        let _ = app.emit("bob", BobEvent::Toast { message });
    }
    Ok(())
}

pub async fn apply_update(app: &AppHandle) -> Result<(), String> {
    let Some(update) = pending_update(app).await? else {
        return Err("sem atualização".into());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    app.restart()
}

pub async fn refresh_ytdlp(app: &AppHandle) -> Result<(), String> {
    let Ok(bin) = runtime::ytdlp_path(app).await else {
        return Ok(());
    };
    let _ = tokio_command(&bin).arg("-U").output().await;
    if let Some(version) = command_line(&bin, "--version").await {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.tools.ytdlp = "ready".into();
        inner.tools.ytdlp_version = Some(version);
        inner.tools.app_version = Some(APP_VERSION.into());
        let tools = inner.tools.clone();
        drop(inner);
        runtime::emit(app, &BobEvent::Tools { tools });
    }
    Ok(())
}

async fn refresh_ffmpeg(app: &AppHandle) -> Result<bool, String> {
    if !cfg!(windows) {
        return Ok(false);
    }
    let Some(_busy) = runtime::acquire_tools(app) else {
        return Ok(false);
    };
    let client = client()?;
    let response = client
        .get("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/latest")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Ok(false);
    }
    let value: serde_json::Value =
        serde_json::from_slice(&response.bytes().await.map_err(|error| error.to_string())?)
            .map_err(|error| error.to_string())?;
    let published = value
        .get("published_at")
        .and_then(|item| item.as_str())
        .unwrap_or("")
        .to_string();
    if published.is_empty() {
        return Ok(false);
    }
    let (data_dir, stamp) = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        (inner.data_dir.clone(), inner.ffmpeg_stamp.clone())
    };
    if stamp.as_deref() == Some(published.as_str()) {
        return Ok(false);
    }
    let bin = data_dir.join("bin");
    if stamp.is_none() && bin.join("ffmpeg.exe").exists() {
        remember_ffmpeg(app, &published).await;
        return Ok(false);
    }
    let zip_path = bin.join("ffmpeg.zip");
    let _ = std::fs::create_dir_all(&bin);
    runtime::download_to(
        app,
        "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
        &zip_path,
        0.2,
        0.9,
    )
    .await?;
    runtime::unzip_ffmpeg(&zip_path, &bin)?;
    let _ = std::fs::remove_file(&zip_path);
    let version = command_line(&bin.join("ffmpeg.exe"), "-version").await;
    {
        let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
        inner.ffmpeg_dir = Some(bin.clone());
        inner.ffmpeg_bin = Some(bin.join("ffmpeg.exe"));
        let probe = bin.join("ffprobe.exe");
        if probe.exists() {
            inner.ffprobe_bin = Some(probe);
        }
        inner.ffmpeg_stamp = Some(published);
        inner.tools.ffmpeg = "ready".into();
        inner.tools.ffmpeg_version = version;
        inner.tools.app_version = Some(APP_VERSION.into());
        let tools = inner.tools.clone();
        drop(inner);
        runtime::emit(app, &BobEvent::Tools { tools });
    }
    Ok(true)
}

async fn check_app(app: &AppHandle) -> Result<bool, String> {
    let Some(update) = pending_update(app).await? else {
        return Ok(false);
    };
    let _ = app.emit(
        "bob",
        BobEvent::Update {
            version: update.version.clone(),
        },
    );
    Ok(true)
}

async fn pending_update(app: &AppHandle) -> Result<Option<tauri_plugin_updater::Update>, String> {
    let endpoint = {
        let inner = app.state::<AppState>().inner.clone().lock_owned().await;
        endpoint(&inner.config)
    };
    if endpoint.is_empty() {
        return Ok(None);
    }
    let url = url::Url::parse(&endpoint).map_err(|error| error.to_string())?;
    let updater = app
        .updater_builder()
        .endpoints(vec![url])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())?;
    updater.check().await.map_err(|error| error.to_string())
}

/// Tauri stores the minisign public key and the signature as one base64 line.
/// `minisign-verify` wants the decoded text, with its comment lines.
#[cfg(test)]
fn minisign_text(value: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.lines().nth(1).is_some() {
        return Ok(trimmed.to_string());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(trimmed)
        .map_err(|error| error.to_string())?;
    String::from_utf8(bytes).map_err(|error| error.to_string())
}

const UPDATE_ENDPOINT: &str =
    "https://github.com/goszc/bob-baixa/releases/latest/download/latest.json";

fn endpoint(config: &Config) -> String {
    let custom = config.update_url.trim();
    if !custom.is_empty() {
        return custom.to_string();
    }
    UPDATE_ENDPOINT.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use minisign_verify::PublicKey;

    #[test]
    fn public_key_decodes() {
        let text = minisign_text(PUBKEY).expect("public key text");
        PublicKey::decode(&text).expect("public key");
    }
}

fn client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent("bob-baixa")
        .build()
        .map_err(|error| error.to_string())
}

async fn command_line(bin: &PathBuf, arg: &str) -> Option<String> {
    let output = tokio_command(bin).arg(arg).output().await.ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let line = text.lines().next()?.trim();
    if arg == "-version" {
        Some(line.split_whitespace().nth(2).unwrap_or(line).to_string())
    } else {
        Some(line.to_string())
    }
}

async fn remember_ffmpeg(app: &AppHandle, published: &str) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.ffmpeg_stamp = Some(published.to_string());
}

async fn mark_checked(app: &AppHandle) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    inner.last_update_check = Some(bob_core::now_iso());
}
