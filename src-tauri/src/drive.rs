//! Google Drive via an embedded desktop OAuth client (PKCE, `drive.file`).
//! End users never paste a client id. If the build has no client id, the feature stays hidden.

use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bob_core::{
    chunk_step, code_challenge_s256, drive_file_link, folder_name, google_auth_url, google_token_exchange_body,
    google_token_refresh_body, oauth_callback_code, percent_encode, sibling_lrc_paths, stored_refresh,
    truncate_property, upload_metadata_json, ChunkStep, DrivePhase, DriveUploadState,
};
#[cfg(windows)]
use bob_core::credential_fits;
use tauri::Manager;
use tauri_plugin_opener::OpenerExt;
use serde::{Deserialize, Serialize};

use crate::runtime::{emit, save, AppState};

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
    #[serde(skip_serializing_if = "String::is_empty")]
    pub email: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Token {
    refresh_token: String,
    access_token: String,
    expires_at: i64,
    #[serde(default)]
    email: String,
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
    #[serde(default)]
    phase: String,
    #[serde(default)]
    error: String,
    #[serde(default)]
    file_id: String,
    #[serde(default)]
    sent: u64,
    #[serde(default)]
    total: u64,
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
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub cover: String,
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
        // 308 is "Resume Incomplete" on a Drive upload, not a redirect.
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.status() == reqwest::StatusCode::PERMANENT_REDIRECT {
                attempt.stop()
            } else if attempt.previous().len() >= 5 {
                attempt.error("muitos redirecionamentos")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

fn http_stream() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(15))
        .user_agent(concat!("bob-baixa/", env!("CARGO_PKG_VERSION")))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.status() == reqwest::StatusCode::PERMANENT_REDIRECT {
                attempt.stop()
            } else if attempt.previous().len() >= 5 {
                attempt.error("muitos redirecionamentos")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .unwrap_or_else(|_| reqwest::blocking::Client::new())
}

mod drive_play;

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
                if let Ok(mut loaded) = serde_json::from_str::<Vec<Upload>>(&text) {
                    for item in &mut loaded {
                        let mut state = item.state();
                        state.recover_interrupted();
                        item.apply_state(&state);
                    }
                    *queue = loaded;
                }
            }
        }
    }
    if started().swap(true, Ordering::Relaxed) {
        return;
    }
    let app = app.clone();
    log_drive("worker de envio iniciado");
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

struct Live {
    token: Token,
    source: String,
}

fn live() -> &'static Mutex<Option<Live>> {
    static LIVE: OnceLock<Mutex<Option<Live>>> = OnceLock::new();
    LIVE.get_or_init(|| Mutex::new(None))
}

fn secret_note() -> &'static Mutex<String> {
    static NOTE: OnceLock<Mutex<String>> = OnceLock::new();
    NOTE.get_or_init(|| Mutex::new(String::new()))
}

#[cfg(windows)]
fn note_secret(message: &str) {
    if let Ok(mut note) = secret_note().lock() {
        *note = message.to_string();
    }
}

fn remember(token: &Token, source: &str) {
    if let Ok(mut slot) = live().lock() {
        *slot = Some(Live {
            token: token.clone(),
            source: source.to_string(),
        });
    }
}

fn live_source() -> String {
    live()
        .lock()
        .ok()
        .and_then(|slot| slot.as_ref().map(|live| live.source.clone()))
        .unwrap_or_else(|| "memória".into())
}

fn load_token() -> Option<Token> {
    if let Ok(slot) = live().lock() {
        if let Some(live) = slot.as_ref() {
            if !live.token.refresh_token.is_empty() {
                return Some(live.token.clone());
            }
        }
    }
    let (refresh, source) = read_persisted()?;
    let token = Token {
        refresh_token: refresh,
        access_token: String::new(),
        expires_at: 0,
        email: read_email(),
    };
    remember(&token, &source);
    log_drive(&format!("refresh token lido de {source}"));
    Some(token)
}

fn store_token(token: &Token) -> Result<(), String> {
    remember(token, "memória");
    save_email(&token.email);
    match save_refresh(&token.refresh_token) {
        Ok(source) => {
            if let Ok(mut slot) = live().lock() {
                if let Some(live) = slot.as_mut() {
                    live.source = source.clone();
                }
            }
            log_drive(&format!("refresh token guardado e lido de volta ({source})"));
            Ok(())
        }
        Err(error) => {
            let message = if cfg!(windows) {
                format!("não consegui ler o acesso de volta no windows: {error}. o envio desta sessão segue na memória")
            } else {
                format!("não consegui guardar o acesso: {error}. o envio desta sessão segue na memória")
            };
            log_drive(&message);
            Err(message)
        }
    }
}

fn save_refresh(refresh: &str) -> Result<String, String> {
    if refresh.trim().is_empty() {
        return Err("o google não devolveu o refresh token".into());
    }
    #[cfg(windows)]
    {
        forget_old_credential();
        if credential_fits(refresh) {
            match keyring_roundtrip(refresh) {
                Ok(()) => return Ok("keyring".into()),
                Err(error) => {
                    log_drive(&format!("keyring: {error}"));
                    if let Ok(entry) = keyring_entry("google-drive-refresh") {
                        let _ = entry.delete_credential();
                    }
                }
            }
        } else {
            log_drive("refresh token grande demais para o gerenciador de credenciais");
        }
        dpapi_roundtrip(refresh)?;
        return Ok("dpapi".into());
    }
    #[cfg(not(windows))]
    {
        file_roundtrip(refresh)?;
        Ok("arquivo".into())
    }
}

fn read_persisted() -> Option<(String, String)> {
    #[cfg(windows)]
    {
        match keyring_get() {
            Ok(raw) => {
                if let Some(refresh) = stored_refresh(&raw) {
                    return Some((refresh, "keyring".into()));
                }
                note_secret("o gerenciador devolveu um refresh token vazio");
            }
            Err(error) if secret_missing(&error) => {}
            Err(error) => note_secret(&format!("keyring: {error}")),
        }
        match dpapi_get() {
            Ok(raw) => {
                if let Some(refresh) = stored_refresh(&raw) {
                    return Some((refresh, "dpapi".into()));
                }
                note_secret("o arquivo dpapi não tinha o refresh token");
            }
            Err(error) if secret_missing(&error) => {}
            Err(error) => note_secret(&format!("dpapi: {error}")),
        }
        return None;
    }
    #[cfg(not(windows))]
    {
        let dir = root_dir().lock().ok()?.clone();
        for name in ["drive-refresh.txt", "drive-token.json"] {
            if let Ok(raw) = std::fs::read_to_string(dir.join(name)) {
                if let Some(refresh) = stored_refresh(&raw) {
                    return Some((refresh, "arquivo".into()));
                }
            }
        }
        None
    }
}

#[cfg(windows)]
fn secret_missing(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("no matching entry")
        || lower.contains("não achei")
        || lower.contains("os error 2")
        || lower.contains("not found")
        || lower.contains("no such file")
}

fn missing_reason() -> String {
    secret_note()
        .lock()
        .ok()
        .map(|note| note.clone())
        .filter(|note| !note.is_empty())
        .unwrap_or_else(|| "entre de novo na conta".into())
}

fn data_file(name: &str) -> Result<PathBuf, String> {
    let dir = root_dir().lock().map_err(|_| "não achei a pasta do app".to_string())?.clone();
    if dir.as_os_str().is_empty() {
        return Err("não achei a pasta do app".into());
    }
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir.join(name))
}

fn save_email(email: &str) {
    let email = email.trim();
    if email.is_empty() {
        return;
    }
    if let Ok(path) = data_file("drive-email.txt") {
        let _ = std::fs::write(path, email);
    }
}

pub fn save_gemini_key(value: &str) -> Result<(), String> {
    let value = value.trim();
    if value.is_empty() {
        #[cfg(windows)]
        if let Ok(entry) = keyring_entry("gemini-api-key") {
            let _ = entry.delete_credential();
        }
        if let Ok(dir) = root_dir().lock() {
            let _ = std::fs::remove_file(dir.join(if cfg!(windows) { "gemini-key.dpapi" } else { "gemini-key.txt" }));
        }
        return Ok(());
    }
    #[cfg(windows)]
    {
        if let Ok(entry) = keyring_entry("gemini-api-key") {
            if entry.set_password(value).is_ok() {
                if entry.get_password().ok().as_deref() == Some(value) {
                    return Ok(());
                }
            }
        }
        let path = data_file("gemini-key.dpapi")?;
        let protected = dpapi_protect(value.as_bytes())?;
        return std::fs::write(path, protected).map_err(|error| error.to_string());
    }
    #[cfg(not(windows))]
    {
        let path = data_file("gemini-key.txt")?;
        std::fs::write(&path, value).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
        }
        Ok(())
    }
}

pub fn read_gemini_key() -> Option<String> {
    #[cfg(windows)]
    {
        if let Ok(entry) = keyring_entry("gemini-api-key") {
            if let Ok(value) = entry.get_password() {
                if !value.trim().is_empty() {
                    return Some(value);
                }
            }
        }
        let dir = root_dir().lock().ok()?.clone();
        let bytes = std::fs::read(dir.join("gemini-key.dpapi")).ok()?;
        let plain = dpapi_unprotect(&bytes).ok()?;
        return String::from_utf8(plain).ok().filter(|value| !value.trim().is_empty());
    }
    #[cfg(not(windows))]
    {
        let dir = root_dir().lock().ok()?.clone();
        std::fs::read_to_string(dir.join("gemini-key.txt")).ok().filter(|value| !value.trim().is_empty())
    }
}

fn read_email() -> String {
    let Ok(dir) = root_dir().lock() else {
        return String::new();
    };
    std::fs::read_to_string(dir.join("drive-email.txt"))
        .unwrap_or_default()
        .trim()
        .to_string()
}

#[cfg(windows)]
fn keyring_entry(user: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("bob-baixa", user).map_err(|error| error.to_string())
}

#[cfg(windows)]
fn forget_old_credential() {
    if let Ok(entry) = keyring_entry("google-drive") {
        let _ = entry.delete_credential();
    }
}

#[cfg(windows)]
fn keyring_get() -> Result<String, String> {
    keyring_entry("google-drive-refresh")?.get_password().map_err(|error| error.to_string())
}

#[cfg(windows)]
fn keyring_roundtrip(refresh: &str) -> Result<(), String> {
    keyring_entry("google-drive-refresh")?
        .set_password(refresh)
        .map_err(|error| format!("gravar: {error}"))?;
    let back = keyring_entry("google-drive-refresh")?
        .get_password()
        .map_err(|error| format!("ler: {error}"))?;
    if back != refresh {
        return Err("o windows não devolveu o mesmo refresh token".into());
    }
    Ok(())
}

#[cfg(windows)]
fn dpapi_roundtrip(refresh: &str) -> Result<(), String> {
    let path = data_file("drive-refresh.dpapi")?;
    let protected = dpapi_protect(refresh.as_bytes())?;
    std::fs::write(&path, &protected).map_err(|error| error.to_string())?;
    let back = dpapi_get()?;
    if back != refresh {
        return Err("o dpapi não devolveu o mesmo refresh token".into());
    }
    Ok(())
}

#[cfg(windows)]
fn dpapi_get() -> Result<String, String> {
    let dir = root_dir().lock().map_err(|_| "não achei a pasta do app".to_string())?.clone();
    let path = dir.join("drive-refresh.dpapi");
    if !path.is_file() {
        return Err("não achei o arquivo dpapi".into());
    }
    let bytes = std::fs::read(&path).map_err(|error| error.to_string())?;
    let plain = dpapi_unprotect(&bytes)?;
    String::from_utf8(plain).map_err(|_| "o arquivo dpapi não é utf-8".into())
}

#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(data.len()).map_err(|_| "bloco grande demais".to_string())?,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptProtectData(&input, windows::core::PCWSTR::null(), None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            .map_err(|error| error.to_string())?;
        if output.pbData.is_null() || output.cbData == 0 {
            return Err("o windows devolveu um bloco vazio".into());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(bytes)
    }
}

#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Result<Vec<u8>, String> {
    use windows::Win32::Foundation::{LocalFree, HLOCAL};
    use windows::Win32::Security::Cryptography::{CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB};
    unsafe {
        let input = CRYPT_INTEGER_BLOB {
            cbData: u32::try_from(data.len()).map_err(|_| "bloco grande demais".to_string())?,
            pbData: data.as_ptr() as *mut u8,
        };
        let mut output = CRYPT_INTEGER_BLOB::default();
        CryptUnprotectData(&input, None, None, None, None, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
            .map_err(|error| error.to_string())?;
        if output.pbData.is_null() {
            return Err("o windows devolveu um bloco vazio".into());
        }
        let bytes = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(Some(HLOCAL(output.pbData.cast())));
        Ok(bytes)
    }
}

#[cfg(not(windows))]
fn file_roundtrip(refresh: &str) -> Result<(), String> {
    let path = data_file("drive-refresh.txt")?;
    std::fs::write(&path, refresh).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    let back = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    if back != refresh {
        return Err("o arquivo não devolveu o mesmo refresh token".into());
    }
    Ok(())
}

fn clear_secret() {
    if let Ok(mut slot) = live().lock() {
        *slot = None;
    }
    if let Ok(mut note) = secret_note().lock() {
        note.clear();
    }
    #[cfg(windows)]
    {
        for user in ["google-drive-refresh", "google-drive"] {
            if let Ok(entry) = keyring_entry(user) {
                let _ = entry.delete_credential();
            }
        }
    }
    if let Ok(dir) = root_dir().lock() {
        for name in ["drive-refresh.dpapi", "drive-refresh.txt", "drive-token.json", "drive-email.txt"] {
            let _ = std::fs::remove_file(dir.join(name));
        }
    }
}

struct Access {
    token: String,
    source: String,
}

fn session_ready() -> Result<(), String> {
    let token = load_token().ok_or_else(missing_reason)?;
    if token.refresh_token.is_empty() {
        return Err(missing_reason());
    }
    Ok(())
}

fn bearer(force: bool) -> Result<Access, String> {
    let mut token = load_token().ok_or_else(|| format!("sem acesso ao drive: {}", missing_reason()))?;
    let mut source = live_source();
    if force || token.access_token.is_empty() || token.expires_at < now_secs() + 60 {
        let email = token.email.clone();
        let previous = token.refresh_token.clone();
        token = refresh(&token)?;
        if token.email.is_empty() {
            token.email = email;
        }
        remember(&token, "memória");
        source = "memória".into();
        if token.refresh_token != previous {
            if let Err(error) = save_refresh(&token.refresh_token) {
                log_drive(&format!("refresh token novo não foi guardado: {error}"));
            }
        }
        save_email(&token.email);
        log_drive("acesso renovado na memória");
    }
    Ok(Access {
        token: token.access_token,
        source,
    })
}

fn refresh(token: &Token) -> Result<Token, String> {
    let id = client_id().ok_or_else(|| "sem client id".to_string())?;
    let response = http()
        .post("https://oauth2.googleapis.com/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(google_token_refresh_body(id, client_secret(), token.refresh_token.as_str()))
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
    let next = Token {
        refresh_token: refresh_token.to_string(),
        access_token: access.to_string(),
        expires_at: now_secs() + expires - 30,
        email: token.email.clone(),
    };
    remember(&next, "memória");
    Ok(next)
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
        phase: DrivePhase::Waiting.as_str().to_string(),
        error: String::new(),
        file_id: String::new(),
        sent: 0,
        total: 0,
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
        std::thread::sleep(Duration::from_secs(1));
        if let Err(reason) = session_ready() {
            block_queue(&format!("sem acesso ao drive: {reason}"));
            continue;
        }
        release_blocked();
        let next = {
            let Ok(queue) = queue_lock().lock() else { continue };
            let now = now_secs();
            let states: Vec<DriveUploadState> = queue.iter().map(Upload::state).collect();
            bob_core::claim_index(&states, now).map(|index| queue[index].clone())
        };
        let Some(mut item) = next else { continue };
        let path = item.path.clone();
        log_drive(&format!("enviando {}", file_label(&path)));
        match upload_item(&item) {
            Ok(file_id) => {
                let mut state = item.state();
                state.succeed(&file_id);
                item.apply_state(&state);
                item.session = None;
                update_path(&path, item.clone());
                log_drive(&format!("enviado {} ({file_id})", file_label(&path)));
                finish_upload(&app, &item, &file_id);
            }
            Err(message) => {
                let mut state = item.state();
                let wait = bob_core::backoff_secs(state.attempts.saturating_add(1));
                state.fail(&message, now_secs(), wait);
                item.apply_state(&state);
                item.session = None;
                update_path(&path, item.clone());
                log_drive(&format!("falhou {}: {message}", file_label(&path)));
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

impl Upload {
    fn state(&self) -> DriveUploadState {
        DriveUploadState {
            phase: DrivePhase::parse(&self.phase),
            attempts: self.attempts,
            not_before: self.not_before,
            sent: self.sent,
            total: self.total,
            file_id: self.file_id.clone(),
            error: self.error.clone(),
            offset: self.offset,
        }
    }

    fn apply_state(&mut self, state: &DriveUploadState) {
        self.phase = state.phase.as_str().to_string();
        self.attempts = state.attempts;
        self.not_before = state.not_before;
        self.sent = state.sent;
        self.total = state.total;
        self.file_id = state.file_id.clone();
        self.error = state.error.clone();
        self.offset = state.offset;
    }
}

fn update_path(path: &str, item: Upload) {
    if let Ok(mut queue) = queue_lock().lock() {
        if let Some(slot) = queue.iter_mut().find(|queued| queued.path == path) {
            *slot = item;
        }
    }
    persist_queue();
}

fn finish_upload(app: &tauri::AppHandle, item: &Upload, file_id: &str) {
    if item.delete_local && !crate::hub::pinned(&item.path) {
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

fn block_queue(reason: &str) {
    let Ok(mut queue) = queue_lock().lock() else { return };
    let mut changed = false;
    for item in queue.iter_mut() {
        let phase = DrivePhase::parse(&item.phase);
        if phase != DrivePhase::Waiting && phase != DrivePhase::Uploading {
            continue;
        }
        let mut state = item.state();
        state.fail(reason, now_secs(), 3_600);
        item.apply_state(&state);
        changed = true;
    }
    drop(queue);
    if changed {
        persist_queue();
        log_drive(reason);
    }
}

fn release_blocked() {
    let Ok(mut queue) = queue_lock().lock() else { return };
    let mut changed = false;
    for item in queue.iter_mut() {
        if DrivePhase::parse(&item.phase) == DrivePhase::Failed && bob_core::token_gap(&item.error) {
            let mut state = item.state();
            state.retry();
            item.apply_state(&state);
            changed = true;
        }
    }
    drop(queue);
    if changed {
        persist_queue();
        log_drive("fila liberada com o acesso da memória");
    }
}

fn upload_item(item: &Upload) -> Result<String, String> {
    let file = PathBuf::from(&item.path);
    if !file.is_file() {
        return Err("arquivo sumiu antes do envio".into());
    }
    note_progress(&file, 0, std::fs::metadata(&file).map(|meta| meta.len()).unwrap_or(0));
    let mut access = bearer(false)?;
    log_drive(&format!("token {}", access.source));
    match upload_with(item, &file, &access.token) {
        Err(message) if message.contains("401") || message.contains("expirou") => {
            log_drive("acesso recusado, renovando");
            access = bearer(true)?;
            log_drive(&format!("token {}", access.source));
            upload_with(item, &file, &access.token)
        }
        other => other,
    }
}

fn upload_with(item: &Upload, file: &Path, token: &str) -> Result<String, String> {
    let parent = music_folder(token, &item.artist, &item.album, item.video)?;
    let name = file.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "faixa".into());
    let mime = mime_for(&name);
    let id = resumable(token, file, &name, mime, &parent, &[
        ("bobKind", if item.video { "video" } else { "audio" }),
        ("bobTitle", &truncate_property(&item.title)),
        ("bobArtist", &truncate_property(&item.artist)),
        ("bobAlbum", &truncate_property(&item.album)),
    ])?;
    let lyrics_id = match sibling_lrc_paths(file).into_iter().find(|path| path.is_file()) {
        Some(lrc) => {
            let lrc_name = lrc.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_else(|| "letra.lrc".into());
            match resumable(token, &lrc, &lrc_name, "text/plain", &parent, &[("bobKind", "lyrics"), ("bobMedia", &id)]) {
                Ok(lyrics_id) => lyrics_id,
                Err(error) => {
                    log_drive(&format!("letra não enviada: {error}"));
                    String::new()
                }
            }
        }
        None => String::new(),
    };
    let cover_id = upload_cover(token, file, &parent, &id, &name);
    drive_play::snapshot_local(file, &id, &parent, &name, &item.title, &item.artist, &item.album, item.video, &lyrics_id, &cover_id);
    Ok(id)
}

fn upload_cover(token: &str, file: &Path, parent: &str, id: &str, name: &str) -> String {
    let Some(bytes) = drive_play::local_cover_bytes(file) else {
        return String::new();
    };
    let mime = bob_core::image_type(&bytes);
    let ext = if mime == "image/png" { "png" } else { "jpg" };
    let stem = bob_core::file_stem(name);
    let cover_name = format!("{stem}.cover.{ext}");
    let path = drive_play::cover_dir().join(format!("upload-{id}.{ext}"));
    if std::fs::write(&path, &bytes).is_err() {
        return String::new();
    }
    let uploaded = resumable(token, &path, &cover_name, mime, parent, &[("bobKind", "cover"), ("bobMedia", id)]);
    let _ = std::fs::remove_file(&path);
    match uploaded {
        Ok(cover_id) => cover_id,
        Err(error) => {
            log_drive(&format!("capa não enviada: {error}"));
            String::new()
        }
    }
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
    let status = response.status().as_u16();
    log_drive(&format!("buscar pasta {name} em {parent} status {status}"));
    let body = response.text().map_err(|error| error.to_string())?;
    if status == 401 {
        return Err(format!("401: {}", api_message(&body)));
    }
    if !(200..300).contains(&status) {
        return Err(api_error(status, &body));
    }
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    if let Some(id) = value
        .pointer("/files/0/id")
        .and_then(|item| item.as_str())
        .filter(|id| !id.is_empty())
    {
        log_drive(&format!("pasta {name} id {id}"));
        return Ok(id.to_string());
    }
    let folder_body = serde_json::to_vec(&serde_json::json!({
        "name": name,
        "mimeType": "application/vnd.google-apps.folder",
        "parents": [parent],
    }))
    .map_err(|error| error.to_string())?;
    let created = http()
        .post("https://www.googleapis.com/drive/v3/files")
        .header("authorization", auth(token))
        .header("content-type", "application/json; charset=utf-8")
        .body(folder_body)
        .send()
        .map_err(|error| error.to_string())?;
    let status = created.status().as_u16();
    let body = created.text().map_err(|error| error.to_string())?;
    log_drive(&format!("criar pasta {name} status {status}"));
    if !(200..300).contains(&status) {
        return Err(api_error(status, &body));
    }
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    value
        .get("id")
        .and_then(|item| item.as_str())
        .map(str::to_string)
        .filter(|id| !id.is_empty())
        .map(|id| {
            log_drive(&format!("pasta {name} id {id}"));
            id
        })
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
    let meta = upload_metadata_json(name, parent, props);
    log_drive(&format!("metadata utf-8 {} bytes", meta.len()));
    let start = http()
        .post("https://www.googleapis.com/upload/drive/v3/files?uploadType=resumable&fields=id")
        .header("authorization", auth(token))
        .header("content-type", "application/json; charset=utf-8")
        .header("x-upload-content-type", mime)
        .header("x-upload-content-length", size.to_string())
        .body(meta.into_bytes())
        .send()
        .map_err(|error| error.to_string())?;
    let start_status = start.status().as_u16();
    log_drive(&format!("iniciar {name} status {start_status} bytes {size}"));
    if !(200..300).contains(&start_status) {
        let body = start.text().unwrap_or_default();
        return Err(match chunk_step(start_status, None, 0) {
            ChunkStep::Wait => "o drive pediu espera".into(),
            ChunkStep::Expired => format!("401: {}", api_message(&body)),
            _ => api_error(start_status, &body),
        });
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
    log_drive(&format!("sessão {session}"));
    note_progress(file, 0, size);
    if size == 0 {
        let response = http()
            .put(&session)
            .header("authorization", auth(token))
            .header("content-length", "0")
            .header("content-range", "bytes */0")
            .body(Vec::new())
            .send()
            .map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        log_drive(&format!("chunk bytes */0 status {status}"));
        return chunk_result(response, status);
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
            .header("authorization", auth(token))
            .header("content-type", mime)
            .header("content-length", chunk.to_string())
            .header("content-range", format!("bytes {offset}-{end}/{size}"))
            .body(buf)
            .send()
            .map_err(|error| error.to_string())?;
        let status = response.status().as_u16();
        log_drive(&format!("chunk bytes {offset}-{end}/{size} status {status}"));
        let range = response.headers().get("range").and_then(|value| value.to_str().ok()).map(str::to_string);
        match chunk_step(status, range.as_deref(), end) {
            ChunkStep::Continue(next) => {
                offset = next;
                note_progress(file, offset, size);
            }
            ChunkStep::Done => return chunk_result(response, status),
            ChunkStep::Expired => {
                let body = response.text().unwrap_or_default();
                return Err(format!("401: {}", api_message(&body)));
            }
            ChunkStep::Wait => return Err("o drive pediu espera".into()),
            ChunkStep::Rejected(code) => {
                let body = response.text().unwrap_or_default();
                return Err(api_error(code, &body));
            }
        }
    }
    Err("o drive não confirmou o arquivo".into())
}

fn chunk_result(response: reqwest::blocking::Response, status: u16) -> Result<String, String> {
    let body = response.text().unwrap_or_default();
    let value: serde_json::Value = serde_json::from_str(&body).unwrap_or(serde_json::Value::Null);
    value
        .get("id")
        .and_then(|item| item.as_str())
        .filter(|id| !id.is_empty())
        .map(str::to_string)
        .ok_or_else(|| api_error(status, &body))
}

fn note_progress(file: &Path, sent: u64, total: u64) {
    let key = file.to_string_lossy();
    if let Ok(mut queue) = queue_lock().lock() {
        if let Some(slot) = queue.iter_mut().find(|item| item.path == key) {
            let mut state = slot.state();
            if sent == 0 {
                state.begin(total);
            } else {
                state.progress(sent, total);
            }
            slot.apply_state(&state);
        }
    }
    persist_queue();
}

fn api_message(body: &str) -> String {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|value| value.pointer("/error/message").and_then(|item| item.as_str()).map(str::to_string))
        .unwrap_or_default()
}

fn api_error(status: u16, body: &str) -> String {
    let message = api_message(body);
    if message.is_empty() {
        format!("drive recusou ({status})")
    } else {
        format!("{status}: {message}")
    }
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

fn urlencoding(value: &str) -> String {
    percent_encode(value)
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
    let token = load_token();
    let pending = queue_lock()
        .lock()
        .map(|queue| queue.iter().filter(|item| item.phase != "sent").count() as u32)
        .unwrap_or(0);
    DriveStatus {
        enabled: enabled(),
        connected: token.is_some(),
        pending,
        drive_only,
        email: token.map(|item| item.email).unwrap_or_default(),
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
    let state = {
        let mut bytes = [0u8; 16];
        let _ = getrandom::getrandom(&mut bytes);
        base64url(&bytes)
    };
    let server = tiny_http::Server::http("127.0.0.1:0").map_err(|_| "não abri a porta local".to_string())?;
    let port = match server.server_addr() {
        tiny_http::ListenAddr::IP(addr) => addr.port(),
        _ => return Err("porta local inválida".into()),
    };
    let redirect = format!("http://127.0.0.1:{port}");
    let url = google_auth_url(id, &redirect, &challenge, &state);
    // cmd /c start splits on '&', so Google only saw client_id and reported a missing response_type.
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|_| "não consegui abrir o navegador para o login do google.".to_string())?;
    let code = tokio::task::spawn_blocking(move || wait_code(server, state))
        .await
        .map_err(|error| error.to_string())??;
    let mut token = exchange(id, &verifier, &code, &redirect)?;
    if let Ok(about) = about_account(&token.access_token) {
        token.email = about.email.clone();
        log_drive(&format!("conectado como {}", about.email));
    }
    store_token(&token)?;
    drive_status(app).await.pipe(Ok)
}

fn wait_code(server: tiny_http::Server, state: String) -> Result<String, String> {
    let Some(request) = server.recv_timeout(Duration::from_secs(180)).ok().flatten() else {
        return Err("o login no google expirou.".into());
    };
    let url = request.url().to_string();
    let code = oauth_callback_code(&url, &state).map_err(|error| error.to_string());
    let page = tiny_http::Response::from_string(
        "<html><body style='font-family:sans-serif'><p>pode fechar esta janela e voltar ao bob baixa.</p></body></html>",
    )
    .with_header(tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap());
    let _ = request.respond(page);
    code
}

fn exchange(id: &str, verifier: &str, code: &str, redirect: &str) -> Result<Token, String> {
    let response = http()
        .post("https://oauth2.googleapis.com/token")
        .header("content-type", "application/x-www-form-urlencoded")
        .body(google_token_exchange_body(id, client_secret(), code, verifier, redirect))
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
        email: String::new(),
    })
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveItemView {
    pub path: String,
    pub name: String,
    pub phase: String,
    pub percent: u32,
    pub error: String,
    pub link: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DriveAccount {
    pub email: String,
    pub name: String,
    pub usage: u64,
    pub limit: u64,
}

#[tauri::command]
pub async fn drive_queue() -> Vec<DriveItemView> {
    let Ok(queue) = queue_lock().lock() else {
        return Vec::new();
    };
    queue
        .iter()
        .map(|item| {
            let state = item.state();
            DriveItemView {
                path: item.path.clone(),
                name: file_label(&item.path),
                phase: state.phase.as_str().to_string(),
                percent: state.percent(),
                error: state.error.clone(),
                link: if state.file_id.is_empty() { String::new() } else { drive_file_link(&state.file_id) },
            }
        })
        .collect()
}

#[tauri::command]
pub async fn drive_retry(path: String) -> Result<(), String> {
    let mut found = false;
    if let Ok(mut queue) = queue_lock().lock() {
        if let Some(item) = queue.iter_mut().find(|item| item.path == path) {
            let mut state = item.state();
            state.retry();
            item.apply_state(&state);
            item.session = None;
            found = true;
        }
    }
    if !found {
        return Err("esse envio não está na fila".into());
    }
    persist_queue();
    log_drive(&format!("nova tentativa {}", file_label(&path)));
    Ok(())
}

#[tauri::command]
pub async fn drive_test(app: tauri::AppHandle) -> Result<DriveAccount, String> {
    boot(app);
    let access = bearer(false)?;
    log_drive(&format!("token {}", access.source));
    let account = about_account(&access.token)?;
    if let Some(mut token) = load_token() {
        token.email = account.email.clone();
        let _ = store_token(&token);
    }
    log_drive(&format!(
        "teste ok {} uso {} limite {}",
        account.email, account.usage, account.limit
    ));
    Ok(account)
}

#[tauri::command]
pub async fn drive_open(app: tauri::AppHandle, target: String) -> Result<(), String> {
    let target = target.trim();
    if target == "log" {
        let path = log_path();
        if !path.is_file() {
            log_drive("registro aberto");
        }
        return app
            .opener()
            .open_path(path.to_string_lossy().to_string(), None::<&str>)
            .map_err(|error| error.to_string());
    }
    if let Some(id) = target.strip_prefix("https://drive.google.com/file/d/").and_then(|rest| rest.split('/').next()) {
        if !id.is_empty() && id.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_') {
            return app.opener().open_url(drive_file_link(id), None::<&str>).map_err(|error| error.to_string());
        }
    }
    Err("link inválido".into())
}

fn about_account(token: &str) -> Result<DriveAccount, String> {
    let response = http()
        .get("https://www.googleapis.com/drive/v3/about?fields=user(emailAddress,displayName),storageQuota(limit,usage)")
        .header("authorization", auth(token))
        .send()
        .map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let body = response.text().map_err(|error| error.to_string())?;
    if !(200..300).contains(&status) {
        return Err(api_error(status, &body));
    }
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|error| error.to_string())?;
    let email = value.pointer("/user/emailAddress").and_then(|item| item.as_str()).unwrap_or("").to_string();
    if email.is_empty() {
        return Err("o drive não informou a conta".into());
    }
    Ok(DriveAccount {
        email,
        name: value.pointer("/user/displayName").and_then(|item| item.as_str()).unwrap_or("").to_string(),
        usage: value.pointer("/storageQuota/usage").and_then(|item| item.as_str()).and_then(|item| item.parse().ok()).unwrap_or(0),
        limit: value.pointer("/storageQuota/limit").and_then(|item| item.as_str()).and_then(|item| item.parse().ok()).unwrap_or(0),
    })
}

fn file_label(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.to_string())
}

pub fn queue_report() -> String {
    let Ok(queue) = queue_lock().lock() else {
        return "fila de envio indisponível".into();
    };
    if queue.is_empty() {
        return "fila de envio vazia".into();
    }
    queue
        .iter()
        .map(|item| {
            format!(
                "{} | {} | {} | tentativas {} | enviados {}/{} | {}",
                file_label(&item.path),
                if item.phase.is_empty() { "aguardando" } else { item.phase.as_str() },
                item.title,
                item.attempts,
                item.sent,
                item.total,
                item.error
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn log_path() -> PathBuf {
    root_dir().lock().map(|dir| dir.join("drive.log")).unwrap_or_else(|_| PathBuf::from("drive.log"))
}

fn log_drive(line: &str) {
    let path = log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(file, "{} {line}", now_secs());
    }
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
        let mut url = "https://www.googleapis.com/drive/v3/files?pageSize=200&fields=nextPageToken,files(id,name,mimeType,size,parents,appProperties)&q=trashed=false".to_string();
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
            let stored = file.pointer("/appProperties/bobKind").and_then(|item| item.as_str()).unwrap_or("");
            let name = file.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string();
            let mime = file.get("mimeType").and_then(|item| item.as_str()).unwrap_or("");
            let kind = bob_core::playback_kind(&name, mime, stored);
            if stored != "audio" && stored != "video" && kind == "audio" && bob_core::media_kind(std::path::Path::new(&name)) == "unknown" {
                continue;
            }
            if stored == "lyrics" || stored == "cover" {
                continue;
            }
            let id = file.get("id").and_then(|item| item.as_str()).unwrap_or("");
            if id.is_empty() {
                continue;
            }
            let parent = file.get("parents").and_then(|item| item.as_array()).and_then(|items| items.first()).and_then(|item| item.as_str()).unwrap_or("");
            let record = drive_play::note_remote(
                id,
                &name,
                mime,
                kind,
                file.pointer("/appProperties/bobTitle").and_then(|item| item.as_str()).unwrap_or(""),
                file.pointer("/appProperties/bobArtist").and_then(|item| item.as_str()).unwrap_or(""),
                file.pointer("/appProperties/bobAlbum").and_then(|item| item.as_str()).unwrap_or(""),
                file.get("size").and_then(|item| item.as_str()).and_then(|value| value.parse().ok()).unwrap_or(0),
                parent,
            );
            out.push(DriveTrack {
                path: format!("drive:{id}"),
                id: id.to_string(),
                title: record.title,
                artist: record.artist,
                album: record.album,
                duration: record.duration,
                kind: record.kind,
                size: record.size,
                name: record.name,
                cover: record.cover,
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

pub use drive_play::{cors, cover_file, describe, lyrics_text, open_media, ListedTrack};

pub fn describe_drive(id: &str) -> ListedTrack {
    describe(id)
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub tracks: u32,
    pub lyrics: u32,
    pub covers: u32,
}

#[tauri::command]
pub async fn drive_repair(app: tauri::AppHandle) -> Result<RepairReport, String> {
    if !enabled() {
        return Err("esta compilação não inclui o google drive.".into());
    }
    boot(app);
    let (tracks, lyrics, covers) = tokio::task::spawn_blocking(drive_play::repair_library)
        .await
        .map_err(|error| error.to_string())??;
    Ok(RepairReport { tracks, lyrics, covers })
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
    let token = bearer(false)?.token;
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
    let token = bearer(false)?.token;
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

pub fn data_dir_public() -> Result<PathBuf, String> {
    root_dir().lock().map(|dir| dir.clone()).map_err(|_| "não achei a pasta do app".to_string())
}

pub fn access_token() -> Result<String, String> {
    Ok(bearer(false)?.token)
}

pub fn account(token: &str) -> Result<(u64, u64), String> {
    let found = about_account(token)?;
    Ok((found.usage, found.limit))
}

pub struct SimpleTrack {
    pub id: String,
    pub path: String,
}

pub fn list_simple() -> Result<Vec<SimpleTrack>, String> {
    Ok(drive_play::ids()
        .into_iter()
        .map(|id| SimpleTrack { path: format!("drive:{id}"), id })
        .collect())
}

pub fn cache_bytes_public() -> u64 {
    drive_play::cache_bytes()
}

pub fn clear_cache_public() -> u64 {
    drive_play::clear_stream_cache()
}

pub fn keep_offline(id: &str) -> Result<(), String> {
    drive_play::keep_offline(id)
}

pub fn offline_copy(id: &str) -> Option<PathBuf> {
    drive_play::offline_copy(id)
}

pub fn warm_public(id: &str) -> Result<(), String> {
    drive_play::warm_preview(id)
}

pub fn repair_some() -> Result<(u32, u32, u32), String> {
    drive_play::repair_some()
}

pub fn set_trashed(id: &str, trashed: bool) -> Result<(), String> {
    let token = bearer(false)?.token;
    let response = http()
        .patch(format!("https://www.googleapis.com/drive/v3/files/{id}"))
        .header("authorization", auth(&token))
        .header("content-type", "application/json")
        .body(format!("{{\"trashed\":{trashed}}}"))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("lixeira recusou ({})", response.status()));
    }
    Ok(())
}

pub fn purge(id: &str) -> Result<(), String> {
    let token = bearer(false)?.token;
    let response = http()
        .delete(format!("https://www.googleapis.com/drive/v3/files/{id}"))
        .header("authorization", auth(&token))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("apagar recusou ({})", response.status()));
    }
    Ok(())
}

pub fn trashed() -> Result<Vec<(String, String, String)>, String> {
    let token = bearer(false)?.token;
    let query = bob_core::percent_encode("trashed = true");
    let response = http()
        .get(format!("https://www.googleapis.com/drive/v3/files?q={query}&fields=files(id,name)&pageSize=40"))
        .header("authorization", auth(&token))
        .send()
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("não li a lixeira ({})", response.status()));
    }
    let value: serde_json::Value = response.json().map_err(|error| error.to_string())?;
    Ok(value
        .get("files")
        .and_then(|item| item.as_array())
        .map(|files| {
            files
                .iter()
                .map(|file| {
                    (
                        file.get("id").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                        file.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                        file.get("name").and_then(|item| item.as_str()).unwrap_or("").to_string(),
                    )
                })
                .filter(|(id, _, _)| !id.is_empty())
                .collect()
        })
        .unwrap_or_default())
}

trait Pipe: Sized {
    fn pipe<T>(self, f: impl FnOnce(Self) -> T) -> T {
        f(self)
    }
}
impl<T> Pipe for T {}
