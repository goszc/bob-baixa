//! Google Drive via an embedded desktop OAuth client (PKCE, `drive.file`).
//! End users never paste a client id. If the build has no client id, the feature stays hidden.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bob_core::{code_challenge_s256, folder_name, sibling_lrc_paths};
use serde::{Deserialize, Serialize};
use tauri::Manager;

use crate::hidden::std_command;
use crate::runtime::{emit, save, AppState};

const SCOPE: &str = "https://www.googleapis.com/auth/drive.file";

struct Embedded;

fn client_id() -> Option<&'static str> {
    let _ = Embedded;
    option_env!("BOB_GOOGLE_CLIENT_ID").filter(|value| !value.trim().is_empty())
}

fn client_secret() -> &'static str {
    option_env!("BOB_GOOGLE_CLIENT_SECRET").unwrap_or("")
}

pub fn enabled() -> bool {
    client_id().is_some()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveStatus {
    pub enabled: bool,
    pub connected: bool,
    pub pending: u32,
    pub drive_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    refresh_token: String,
    access_token: String,
    expires_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Upload {
    path: String,
    title: String,
    artist: String,
    album: String,
    video: bool,
    delete_local: bool,
    attempts: u32,
    not_before: i64,
    session: Option<String>,
    offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveTrack {
    pub id: String,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: Option<f64>,
    pub kind: String,
    pub size: u64,
}

fn root_dir() -> &'static Mutex<PathBuf> {
    static DIR: OnceLock<Mutex<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| Mutex::new(PathBuf::new()))
}

fn queue_lock() -> &'static Mutex<Vec<Upload>> {
    static QUEUE: OnceLock<Mutex<Vec<Upload>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

fn started() -> &'static AtomicBool {
    static STARTED: AtomicBool = AtomicBool::new(false);
    &STARTED
}

fn now_secs() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

fn http() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(120))
        .user_agent(concat!("bob-baixa/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

pub fn boot(app: tauri::AppHandle) {
    if !enabled() {
        return;
    }
    let dir = app
        .state::<AppState>()
        .inner
        .try_lock()
        .ok()
        .map(|inner| inner.data_dir.clone())
        .unwrap_or_default();
    if let Ok(mut slot) = root_dir().lock() {
        *slot = dir.clone();
    }
    if let Ok(mut queue) = queue_lock().lock() {
        if queue.is_empty() {
            if let Ok(text) = std::fs::read_to_string(dir.join("drive-queue.json")) {
                if let Ok(loaded) = serde_json::from_str::<Vec<Upload>>(&text) {
                    *queue = loaded;
                }
            }
        }
    }
    if started().swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    let _ = std::thread::Builder::new().name("drive-upload".into()).spawn(move || worker(app));
}

fn persist_queue() {
    let Ok(dir) = root_dir().lock() else { return };
    if dir.as_os_str().is_empty() {
        return;
    }
    let Ok(queue) = queue_lock().lock() else { return };
    if let Ok(text) = serde_json::to_string(&*queue) {
        let _ = std::fs::write(dir.join("drive-queue.json"), text);
    }
}

fn load_token() -> Option<Token> {
    let raw = read_secret()?;
    serde_json::from_str(&raw).ok()
}

fn store_token(token: &Token) {
    if let Ok(text) = serde_json::to_string(token) {
        let _ = write_secret(&text);
    }
}

fn read_secret() -> Option<String> {
    #[cfg(windows)]
    {
        let entry = keyring::Entry::new("bob-baixa", "google-drive").ok()?;
        return entry.get_password().ok();
    }
    #[cfg(not(windows))]
    {
        let dir = root_dir().lock().ok()?.clone();
        std::fs::read_to_string(dir.join("drive-token.json")).ok()
    }
}

fn write_secret(value: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        let entry = keyring::Entry::new("bob-baixa", "google-drive").map_err(|error| error.to_string())?;
        entry.set_password(value).map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let dir = root_dir().lock().map_err(|_| "pasta".to_string())?.clone();
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let path = dir.join("drive-token.json");
        std::fs::write(&path, value).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

fn clear_secret() {
    #[cfg(windows)]
    {
        if let Ok(entry) = keyring::Entry::new("bob-baixa", "google-drive") {
            let _ = entry.delete_credential();
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(dir) = root_dir().lock() {
            let _ = std::fs::remove_file(dir.join("drive-token.json"));
        }
    }
}

fn bearer() -> Result<String, String> {
    let mut token = load_token().ok_or_else(|| "drive desconectado".to_string())?;
    if token.access_token.is_empty() || token.expires_at < now_secs() + 60 {
        token = refresh(&token)?;
        store_token(&token);
    }
    Ok(token.access_token)
}

fn refresh(token: &Token) -> Result<Token, String> {
    let id = client_id().ok_or_else(|| "sem client id".to_string())?;
    let response = http()
        .post("https://oauth2.googleapis.com/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_pairs(&[
            ("client_id", id),
            ("client_secret", client_secret()),
            ("refresh_token", token.refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ]))
        .send()
        .map_err(|error| error.to_string())?;
    let body = response.text().map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let access = value.get("access_token").and_then(|item| item.as_str()).unwrap_or("");
    if access.is_empty() {
        return Err("não deu para renovar o acesso ao drive.".into());
    }
    let expires = value.get("expires_in").and_then(|item| item.as_i64()).unwrap_or(3600);
    let refresh_token = value
        .get("refresh_token")
        .and_then(|item| item.as_str())
        .filter(|value| !value.is_empty())
        .unwrap_or(token.refresh_token.as_str());
    Ok(Token {
        refresh_token: refresh_token.to_string(),
        access_token: access.to_string(),
        expires_at: now_secs() + expires - 30,
    })
}

pub fn enqueue_local(app: &tauri::AppHandle, file: &Path, title: &str, artist: &str, album: &str, video: bool) {
    if !enabled() || !file.is_file() {
        return;
    }
    boot(app.clone());
    let delete_local = app
        .state::<AppState>()
        .inner
        .try_lock()
        .map(|inner| inner.config.drive_only)
        .unwrap_or(false);
    let item = Upload {
        path: file.to_string_lossy().into_owned(),
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        video,
        delete_local,
        attempts: 0,
        not_before: 0,
        session: None,
        offset: 0,
    };
    if let Ok(mut queue) = queue_lock().lock() {
        if queue.iter().any(|queued| queued.path == item.path) {
            return;
        }
        queue.push(item);
    }
    persist_queue();
}

fn worker(app: tauri::AppHandle) {
    loop {
        std::thread::sleep(Duration::from_secs(2));
        if load_token().is_none() {
            continue;
        }
        let next = {
            let Ok(queue) = queue_lock().lock() else { continue };
            let now = now_secs();
            queue
                .iter()
                .position(|item| item.not_before <= now)
                .map(|index| (index, queue[index].clone()))
        };
        let Some((index, mut item)) = next else { continue };
        match upload_item(&item) {
            Ok(file_id) => {
                if let Ok(mut queue) = queue_lock().lock() {
                    if queue.get(index).map(|queued| queued.path == item.path).unwrap_or(false) {
                        queue.remove(index);
                    } else {
                        queue.retain(|queued| queued.path != item.path);
                    }
                }
                persist_queue();
                finish_upload(&app, &item, &file_id);
            }
            Err(message) => {
                item.attempts = item.attempts.saturating_add(1);
                let wait = bob_core::backoff_secs(item.attempts);
                item.not_before = now_secs() + wait as i64;
                item.session = None;
                if let Ok(mut queue) = queue_lock().lock() {
                    if let Some(slot) = queue.iter_mut().find(|queued| queued.path == item.path) {
                        *slot = item.clone();
                    }
                }
                persist_queue();
                if item.attempts == 1 || item.attempts % 4 == 0 {
                    emit(
                        &app,
                        &bob_core::BobEvent::Toast {
                            message: format!("drive: {message}"),
                        },
                    );
                }
            }
        }
    }
}

fn finish_upload(app: &tauri::AppHandle, item: &Upload, file_id: &str) {
    if item.delete_local {
        let _ = std::fs::remove_file(&item.path);
        for sidecar in sibling_lrc_paths(Path::new(&item.path)) {
            let _ = std::fs::remove_file(sidecar);
        }
        let target = format!("drive:{file_id}");
        let from = item.path.clone();
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            retarget(&app, &from, &target).await;
        });
    }
}

async fn retarget(app: &tauri::AppHandle, from: &str, to: &str) {
    let mut inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let mut changed = false;
    for entry in &mut inner.history {
        if entry.path.as_deref().map(|path| bob_core::same_path(path, from)).unwrap_or(false) {
            entry.path = Some(to.to_string());
            changed = true;
        }
    }
    for job in &mut inner.jobs {
        if job.output_path.as_deref().map(|path| bob_core::same_path(path, from)).unwrap_or(false) {
            job.output_path = Some(to.to_string());
            changed = true;
        }
    }
    if changed {
        save(&inner);
        let jobs = inner.jobs.clone();
        let history = inner.history.clone();
        drop(inner);
        emit(app, &bob_core::BobEvent::Jobs { jobs });
        emit(app, &bob_core::BobEvent::History { history });
    }
}

fn upload_item(item: &Upload) -> Result<String, String> {
    let file = PathBuf::from(&item.path);
    if !file.is_file() {
        return Err("arquivo sumiu antes do envio".into());
    }
    let token = bearer()?;
    let parent = music_folder(&token, &item.artist, &item.album, item.video)?;
    let name = file.file_name().and_then(|name| name.to_str()).unwrap_or("faixa").to_string();
    let mime = mime_for(&name);
    let id = resumable(&token, &file, &name, &mime, &parent, &[
        ("bobKind", if item.video { "video" } else { "audio" }),
        ("bobTitle", item.title.as_str()),
        ("bobArtist", item.artist.as_str()),
        ("bobAlbum", item.album.as_str()),
    ])?;
    if let Some(lrc) = sibling_lrc_paths(&file).into_iter().find(|path| path.is_file()) {
        let lrc_name = lrc.file_name().and_then(|name| name.to_str()).unwrap_or("letra.lrc");
        let _ = resumable(&token, &lrc, lrc_name, "text/plain", &parent, &[("bobKind", "lyrics")]);
    }
    Ok(id)
}

fn music_folder(token: &str, artist: &str, album: &str, video: bool) -> Result<String, String> {
    let root = folder(token, "root", "bob baixa")?;
    if video {
        return folder(token, &root, "Vídeos");
    }
    let music = folder(token, &root, "Músicas")?;
    let artist = folder(token, &music, &folder_name(artist, "Desconhecido"))?;
    folder(token, &artist, &folder_name(album, "Avulsas"))
}

fn folder(token: &str, parent: &str, name: &str) -> Result<String, String> {
    let name = folder_name(name, "bob baixa");
    let query = format!(
        "name = '{}' and mimeType = 'application/vnd.google-apps.folder' and '{}' in parents and trashed = false",
        name.replace('\'', "\\'"),
        parent
    );
    let url = format!(
        "https://www.googleapis.com/drive/v3/files?fields=files(id)&q={}",
        urlencoding(&query)
    );
    let response = http()
        .get(url)
        .header("authorization", auth(token))
        .send()
        .map_err(|error| error.to_string())?;
    if response.status().as_u16() == 401 {
        return Err("sessão do drive expirou".into());
    }
    let body = response.text().map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    if let Some(id) = value
        .pointer("/files/0/id")
        .and_then(|item| item.as_str())
        .filter(|id| !id.is_empty())
    {
        return Ok(id.to_string());
    }
    let created = http()
        .post("https://www.googleapis.com/drive/v3/files")
        .header("authorization", auth(token))
        .json(&serde_json::json!({
            "name": name,
            "mimeType": "application/vnd.google-apps.folder",
            "parents": [parent],
        }))
        .send()
        .map_err(|error| error.to_string())?;
    let body = created.text().map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    value
        .get("id")
        .and_then(|item| item.as_str())
        .map(str::to_string)
        .filter(|id| !id.is_empty())
        .ok_or_else(|| "não criei a pasta no drive".to_string())
}

fn resumable(
    token: &str,
    file: &Path,
    name: &str,
    mime: &str,
    parent: &str,
    props: &[(&str, &str)],
) -> Result<String, String> {
    let size = std::fs::metadata(file).map_err(|error| error.to_string())?.len();
    let mut properties = serde_json::Map::new();
    for (key, value) in props {
        properties.insert((*key).to_string(), serde_json::Value::String((*value).to_string()));
    }
    let meta = serde_json::json!({
        "name": name,
        "parents": [parent],
        "appProperties": properties,
    });
    let start = http()
        .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&fields=id")
        .header("authorization", auth(token))
        .header("x-upload-content-type", mime)
        .header("x-upload-content-length", size.to_string())
        .json(&meta)
        .send()
        .map_err(|error| error.to_string())?;
    if start.status().as_u16() == 429 || start.status().as_u16() == 503 {
        return Err("o drive pediu espera".into());
    }
    if !start.status().is_success() {
        return Err(format!("drive recusou o envio ({})", start.status()));
    }
    let session = start
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_string();
    if session.is_empty() {
        return Err("o drive não devolveu o endereço do envio".into());
    }
    let mut offset = 0u64;
    let mut handle = std::fs::File::open(file).map_err(|error| error.to_string())?;
    while offset < size {
        let chunk = std::cmp::min(8 * 1024 * 1024, size - offset);
        let mut buf = vec![0u8; chunk as usize];
        handle.seek(SeekFrom::Start(offset)).map_err(|error| error.to_string())?;
        handle.read_exact(&mut buf).map_err(|error| error.to_string())?;
        let end = offset + chunk - 1;
        let response = http()
            .put(&session)
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", mime)
            .header("content-length", chunk.to_string())
            .header("content-range", format!("bytes {offset}-{end}/{size}"))
            .body(buf)
            .send()
            .map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        if status == 308 {
            offset = end + 1;
            continue;
        }
        if status == 429 || status == 503 {
            return Err("o drive pediu espera".into());
        }
        if !(200..300).contains(&status) {
            return Err(format!("envio interrompido ({status})"));
        }
        let body = response.text().unwrap_or_default();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
        if let Some(id) = value.get("id").and_then(|item| item.as_str()) {
            return Ok(id.to_string());
        }
        offset = end + 1;
    }
    Err("o drive não confirmou o arquivo".into())
}

fn auth(token: &str) -> String {
    format!("Bearer {token}")
}

fn mime_for(name: &str) -> &'static str {
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".mp3") {
        "audio/mpeg"
    } else if lower.ends_with(".opus") || lower.ends_with(".ogg") {
        "audio/ogg"
    } else if lower.ends_with(".m4a") || lower.ends_with(".mp4") {
        if lower.ends_with(".m4a") { "audio/mp4" } else { "video/mp4" }
    } else if lower.ends_with(".webm") {
        "video/webm"
    } else if lower.ends_with(".flac") {
        "audio/flac"
    } else if lower.ends_with(".lrc") {
        "text/plain"
    } else {
        "application/octet-stream"
    }
}

fn form_pairs(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(key, value)| format!("{}={}", urlencoding(key), urlencoding(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn urlencoding(value: &str) -> String {
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

fn base64url(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut index = 0;
    while index + 3 <= bytes.len() {
        let n = ((bytes[index] as u32) << 16) | ((bytes[index + 1] as u32) << 8) | bytes[index + 2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        index += 3;
    }
    out
}

#[tauri::command]
pub async fn drive_status(app: tauri::AppHandle) -> DriveStatus {
    let drive_only = app
        .state::<AppState>()
        .inner
        .clone()
        .lock_owned()
        .await
        .config
        .drive_only;
    let pending = queue_lock().lock().map(|queue| queue.len() as u32).unwrap_or(0);
    DriveStatus {
        enabled: enabled(),
        connected: load_token().is_some(),
        pending,
        drive_only,
    }
}

#[tauri::command]
pub async fn drive_connect(app: tauri::AppHandle) -> Result<DriveStatus, String> {
    let id = client_id().ok_or_else(|| "esta compilação não inclui o google drive.".to_string())?;
    boot(app.clone());
    let verifier = {
        let mut bytes = [0u8; 32];
        let _ = getrandom::getrandom(&mut bytes);
        base64url(&bytes)
    };
    let challenge = code_challenge_s256(&verifier);
    let server = tiny_http::Server::http("127.0.0.1:0").map_err(|_| "não abri a porta local".to_string())?;
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
        _ => return Err("porta local inválida".into()),
    };
    let redirect = format!("http://127.0.0.1:{port}");
    let url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method=S256&access_type=offline&prompt=consent",
        urlencoding(id),
        urlencoding(&redirect),
        urlencoding(SCOPE),
        urlencoding(&challenge)
    );
    open_browser(&url);
    let code = tokio::task::spawn_blocking(move || wait_code(server))
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "o login no google expirou.".to_string())?;
    let token = exchange(id, &verifier, &code, &redirect)?;
    store_token(&token);
    drive_status(app).await.pipe(Ok)
}

fn wait_code(server: tiny_http::Server) -> Option<String> {
    let request = server.recv_timeout(Duration::from_secs(180)).ok()??;
    let url = request.url().to_string();
    let code = url.split("code=").nth(1).and_then(|rest| rest.split('&').next()).map(|value| {
        urlencoding_decode(value)
    });
    let page = tiny_http::Response::from_string(
        "<html><body style='font-family:sans-serif'><p>pode fechar esta janela e voltar ao bob baixa.</p></body></html>",
    )
    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap());
    let _ = request.respond(page);
    code.filter(|value| !value.is_empty())
}

fn urlencoding_decode(value: &str) -> String {
    let bytes = value.replace('+', " ");
    let mut out = Vec::new();
    let raw = bytes.as_bytes();
    let mut index = 0;
    while index < raw.len() {
        if raw[index] == b'%' && index + 2 < raw.len() {
            if let Ok(byte) = u8::from_str_radix(std::str::from_utf8(&raw[index + 1..index + 3]).unwrap_or(""), 16) {
                out.push(byte);
                index += 3;
                continue;
            }
        }
        out.push(raw[index]);
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn exchange(id: &str, verifier: &str, code: &str, redirect: &str) -> Result<Token, String> {
    let response = http()
        .post("https://oauth2.googleapis.com/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(form_pairs(&[
            ("client_id", id),
            ("client_secret", client_secret()),
            ("code", code),
            ("code_verifier", verifier),
            ("redirect_uri", redirect),
            ("grant_type", "authorization_code"),
        ]))
        .send()
        .map_err(|error| error.to_string())?;
    let body = response.text().map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|_| body.clone())?;
    let access = value.get("access_token").and_then(|item| item.as_str()).unwrap_or("");
    let refresh = value.get("refresh_token").and_then(|item| item.as_str()).unwrap_or("");
    if access.is_empty() || refresh.is_empty() {
        return Err("o google não devolveu o acesso. confira se o client do drive está ativo.".into());
    }
    let expires = value.get("expires_in").and_then(|item| item.as_i64()).unwrap_or(3600);
    Ok(Token {
        refresh_token: refresh.to_string(),
        access_token: access.to_string(),
        expires_at: now_secs() + expires - 30,
    })
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    let _ = std_command("cmd").args(["/C", "start", "", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std_command("open").arg(url).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let _ = std_command("xdg-open").arg(url).spawn();
}

#[tauri::command]
pub async fn drive_disconnect(app: tauri::AppHandle) -> DriveStatus {
    clear_secret();
    drive_status(app).await
}

#[tauri::command]
pub async fn drive_tracks() -> Result<Vec<DriveTrack>, String> {
    if !enabled() {
        return Ok(Vec::new());
    }
    let Some(token) = load_token() else {
        return Ok(Vec::new());
    };
    tokio::task::spawn_blocking(move || list_tracks(&token))
        .await
        .map_err(|error| error.to_string())?
}

fn list_tracks(token: &Token) -> Result<Vec<DriveTrack>, String> {
    let access = if token.expires_at < now_secs() + 30 {
        refresh(token)?.access_token
    } else {
        token.access_token.clone()
    };
    let mut page = String::new();
    let mut out = Vec::new();
    loop {
        let mut url = "https://www.googleapis.com/drive/v3/files?pageSize=200&fields=nextPageToken,files(id,name,mimeType,size,appProperties)&q=trashed=false".to_string();
        if !page.is_empty() {
            url.push_str("&pageToken=");
            url.push_str(&urlencoding(&page));
        }
        let response = http()
            .get(url)
            .header("authorization", auth(&access))
            .send()
            .map_err(|error| error.to_string())?;
        let body = response.text().map_err(|error| error.to_string())?;
        let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
        let Some(files) = value.get("files").and_then(|item| item.as_array()) else {
            break;
        };
        for file in files {
            let kind = file.pointer("/appProperties/bobKind").and_then(|item| item.as_str()).unwrap_or("");
            if kind != "audio" && kind != "video" {
                continue;
            }
            let id = file.get("id").and_then(|item| item.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            out.push(DriveTrack {
                path: format!("drive:{id}"),
                id: id.to_string(),
                title: file.pointer("/appProperties/bobTitle").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                artist: file.pointer("/appProperties/bobArtist").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                album: file.pointer("/appProperties/bobAlbum").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                duration: None,
                kind: kind.to_string(),
                size: file.get("size").and_then(|item| item.as_str()).and_then(|value| value.parse().ok()).unwrap_or(0),
            });
        }
        match value.get("nextPageToken").and_then(|item| item.as_str()) {
            Some(next) if !next.is_empty() => page = next.to_string(),
            _ => break,
        }
    }
    if let Ok(dir) = root_dir().lock() {
        if !dir.as_os_str().is_empty() {
            if let Ok(text) = serde_json::to_string(&out) {
                let _ = std::fs::write(dir.join("drive-index.json"), text);
            }
        }
    }
    Ok(out)
}

pub fn proxy(id: &str, range: Option<&str>) -> (u16, Vec<(String, String)>, Vec<u8>) {
    let id = id.split(['?', '#']).next().unwrap_or(id).trim();
    if id.is_empty() || !id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
        return (400, Vec::new(), b"id".to_vec());
    }
    let Ok(token) = bearer() else {
        return (401, Vec::new(), b"drive".to_vec());
    };
    let url = format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media");
    let mut request = http().get(url).header("authorization", format!("Bearer {token}"));
    if let Some(range) = range {
        request = request.header("range", range);
    } else {
        request = request.header("range", "bytes=0-");
    }
    let Ok(response) = request.send() else {
        return (502, Vec::new(), b"rede".to_vec());
    };
    let status = response.status().as_u16();
    let mut headers = Vec::new();
    for name in ["content-type", "content-range", "accept-ranges", "content-length"] {
        if let Some(value) = response.headers().get(name).and_then(|value| value.to_str().ok()) {
            headers.push((name.to_string(), value.to_string()));
        }
    }
    let Ok(bytes) = response.bytes() else {
        return (502, headers, Vec::new());
    };
    if bytes.len() > 16 * 1024 * 1024 {
        return (status, headers, bytes[..16 * 1024 * 1024].to_vec());
    }
    (status, headers, bytes.to_vec())
}

#[tauri::command]
pub async fn drive_upload_library(app: tauri::AppHandle) -> Result<u32, String> {
    if !enabled() {
        return Err("esta compilação não inclui o google drive.".into());
    }
    boot(app.clone());
    let root = app.state::<AppState>().inner.clone().lock_owned().await.config.output_dir.clone();
    let files = tokio::task::spawn_blocking(move || crate::enrich::list_media_public(&PathBuf::from(root)))
        .await
        .map_err(|error| error.to_string())?;
    let mut count = 0u32;
    for file in files {
        let path = PathBuf::from(&file);
        let video = bob_core::media_kind(&path) == "video";
        enqueue_local(&app, &path, path.file_stem().and_then(|n| n.to_str()).unwrap_or(""), "", "", video);
        count += 1;
    }
    Ok(count)
}

#[tauri::command]
pub async fn drive_push_state(json: String) -> Result<(), String> {
    if load_token().is_none() {
        return Ok(());
    }
    let json = json.chars().take(2_000_000).collect::<String>();
    tokio::task::spawn_blocking(move || push_state(&json))
        .await
        .map_err(|error| error.to_string())?
}

fn push_state(json: &str) -> Result<(), String> {
    let token = bearer()?;
    let root = folder(&token, "root", "bob baixa")?;
    let app_folder = folder(&token, &root, "app")?;
    let dir = root_dir().lock().map_err(|_| "pasta".to_string())?.clone();
    let path = dir.join("biblioteca.json");
    std::fs::write(&path, json).map_err(|error| error.to_string())?;
    let existing = find_named(&token, &app_folder, "biblioteca.json")?;
    if let Some(id) = existing {
        let response = http()
            .patch(format!("https://www.googleapis.com/upload/drive/v3/files/{id}?uploadType=media"))
            .header("authorization", format!("Bearer {token}"))
            .header("content-type", "application/json")
            .body(json.to_string())
            .send()
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            return Err("não atualizei a biblioteca no drive".into());
        }
        return Ok(());
    }
    let _ = resumable(&token, &path, "biblioteca.json", "application/json", &app_folder, &[("bobKind", "state")])?;
    Ok(())
}

fn find_named(token: &str, parent: &str, name: &str) -> Result<Option<String>, String> {
    let query = format!("name = '{name}' and '{parent}' in parents and trashed = false");
    let url = format!(
        "https://www.googleapis.com/drive/v3/files?fields=files(id)&q={}",
        urlencoding(&query)
    );
    let body = http()
        .get(url)
        .header("authorization", auth(token))
        .send()
        .map_err(|error| error.to_string())?
        .text()
        .map_err(|error| error.to_string())?;
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    Ok(value.pointer("/files/0/id").and_then(|item| item.as_str()).map(str::to_string))
}

#[tauri::command]
pub async fn drive_pull_state() -> Result<Option<String>, String> {
    if load_token().is_none() {
        return Ok(None);
    }
    tokio::task::spawn_blocking(pull_state).await.map_err(|error| error.to_string())?
}

fn pull_state() -> Result<Option<String>, String> {
    let token = bearer()?;
    let root = folder(&token, "root", "bob baixa")?;
    let app_folder = folder(&token, &root, "app")?;
    let Some(id) = find_named(&token, &app_folder, "biblioteca.json")? else {
        return Ok(None);
    };
    let response = http()
        .get(format!("https://www.googleapis.com/drive/v3/files/{id}?alt=media"))
        .header("authorization", auth(&token))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Ok(None);
    }
    let text = response.text().map_err(|error| error.to_string())?;
    Ok(Some(text))
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}
