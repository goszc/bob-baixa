//! Gemini calls with the free-tier counters. A 429 tries the fallback model,
//! then the same heuristics the app already had.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use bob_core::{
    allowed, cosine, description_overlap, embed_document, estimate_tokens, extended_path, lrc_from_transcript, note_429,
    note_success, utc_day, Config, ModelUse, Route, Slot,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Manager;

use crate::hidden::tokio_command;
use crate::runtime::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageView {
    pub model: String,
    pub requests: u32,
    pub limit: u32,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Described {
    pub path: String,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub lyrics: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Ledger {
    #[serde(default)]
    models: HashMap<String, ModelUse>,
    #[serde(default)]
    embeddings: HashMap<String, Vec<f32>>,
    #[serde(default)]
    lyrics_day: String,
    #[serde(default)]
    lyrics_count: u32,
}

#[tauri::command]
pub async fn ai_usage(app: tauri::AppHandle) -> Result<Vec<UsageView>, String> {
    let (config, dir) = context(&app).await;
    let ledger = load(&dir);
    let now = chrono::Utc::now().timestamp();
    let day = utc_day(now);
    Ok(vec![
        view(&ledger, &config.gemini_flash, Slot::Flash, &day),
        view(&ledger, &config.gemini_bulk, Slot::Bulk, &day),
        view(&ledger, &config.gemini_embed, Slot::Embed, &day),
        view(&ledger, &config.gemini_transcribe, Slot::Transcribe, &day),
    ])
}

#[tauri::command]
pub async fn ai_wrapped(app: tauri::AppHandle, period: String, minutes: f64, artist: String, track: String) -> Result<String, String> {
    let facts = format!("período: {period}\nminutos: {:.0}\nartista: {artist}\nfaixa: {track}", minutes);
    let prompt = format!("escreva um resumo curto e divertido, em português brasileiro, no estilo wrapped, só com estes fatos:\n{facts}");
    let (config, dir) = context(&app).await;
    match flash_result(&config, &dir, &prompt, 400).await {
        Ok(text) => Ok(text),
        Err(message) => Err(message),
    }
}

#[tauri::command]
pub async fn ai_meaning(app: tauri::AppHandle, title: String, artist: String, lyrics: String) -> Result<String, String> {
    let prompt = format!("em poucas frases, em português brasileiro, o que {artist} — {title} parece dizer? use só a letra abaixo.\n{lyrics}");
    let (config, dir) = context(&app).await;
    if let Some(text) = flash(&config, &dir, &prompt, 500).await {
        return Ok(text);
    }
    let snippet: String = lyrics.chars().take(280).collect();
    if snippet.trim().is_empty() {
        Ok("sem letra por aqui pra explicar.".into())
    } else {
        Ok(snippet)
    }
}

#[tauri::command]
pub async fn ai_describe(app: tauri::AppHandle, query: String, tracks: Vec<Described>) -> Result<Vec<String>, String> {
    let (config, dir) = context(&app).await;
    let mut ledger = load(&dir);
    let mut scored: Vec<(f64, String)> = Vec::new();
    let query_embed = embed_query(&config, &mut ledger, &query).await;
    let mut fresh = 0u32;
    for track in &tracks {
        let document = embed_document(&track.artist, &track.title, &track.album, &track.lyrics);
        let mut rank = description_overlap(&query, &document);
        if let Some(query_embed) = &query_embed {
            if let Some(vector) = cached_or_embed(&config, &mut ledger, &document, &mut fresh, 24).await {
                rank = rank.max(f64::from(cosine(query_embed, &vector)));
            }
        }
        if rank > 0.15 {
            scored.push((rank, track.path.clone()));
        }
    }
    save(&dir, &ledger);
    scored.sort_by(|left, right| right.0.partial_cmp(&left.0).unwrap_or(std::cmp::Ordering::Equal));
    scored.truncate(20);
    Ok(scored.into_iter().map(|(_, path)| path).collect())
}

#[tauri::command]
pub async fn ai_similar(app: tauri::AppHandle, path: String, tracks: Vec<Described>) -> Result<Vec<String>, String> {
    let Some(seed) = tracks.iter().find(|track| track.path == path) else {
        return Ok(Vec::new());
    };
    let query = embed_document(&seed.artist, &seed.title, &seed.album, &seed.lyrics);
    let mut found = ai_describe(app, query, tracks).await?;
    found.retain(|item| item != &path);
    found.insert(0, path);
    Ok(found)
}

#[tauri::command]
pub async fn ai_transcribe(app: tauri::AppHandle, path: String) -> Result<String, String> {
    transcribe(&app, &path, true).await
}

pub async fn translate_batch(app: &tauri::AppHandle, lines: &[String], target: &str) -> Option<Vec<String>> {
    if lines.is_empty() {
        return None;
    }
    let joined = lines.join("\n");
    let prompt = format!("traduza para {target}, uma linha por linha, sem explicação:\n{joined}");
    let (config, dir) = context(app).await;
    let tokens = estimate_tokens(joined.len());
    let text = bulk(&config, &dir, &prompt, tokens).await.or(flash(&config, &dir, &prompt, tokens).await)?;
    let parts: Vec<String> = text.split('\n').map(|line| line.to_string()).collect();
    if parts.len() == lines.len() {
        Some(parts)
    } else {
        None
    }
}

pub async fn maybe_auto_lyrics(app: &tauri::AppHandle, path: &str) {
    let config = context(app).await.0;
    if !config.gemini_auto_lyrics || stored_key().trim().is_empty() {
        return;
    }
    let file = PathBuf::from(path);
    let lrc = file.with_extension("lrc");
    if lrc.is_file() {
        return;
    }
    let _ = transcribe(app, path, false).await;
}

async fn transcribe(app: &tauri::AppHandle, path: &str, manual: bool) -> Result<String, String> {
    let (config, dir) = context(app).await;
    let key = stored_key();
    if key.trim().is_empty() {
        return Err("cole a chave do gemini nas configurações.".into());
    }
    let mut ledger = load(&dir);
    let now = chrono::Utc::now().timestamp();
    let day = utc_day(now);
    if ledger.lyrics_day != day {
        ledger.lyrics_day = day.clone();
        ledger.lyrics_count = 0;
    }
    let budget = config.gemini_lyrics_budget.min(bob_core::limits(Slot::Transcribe).rpd);
    if !manual && ledger.lyrics_count >= budget {
        return Err("o limite diário de letras automáticas acabou.".into());
    }
    let model = config.gemini_transcribe.clone();
    let use_ = ledger.models.entry(model.clone()).or_insert_with(|| ModelUse::fresh(&day));
    if !allowed(use_, Slot::Transcribe, now, 1000) {
        return Err(bob_core::explain_block(&model, use_, now));
    }
    let ffmpeg = {
        app.state::<AppState>()
            .inner
            .clone()
            .lock_owned()
            .await
            .ffmpeg_bin
            .clone()
            .ok_or_else(|| "o ffmpeg ainda não está pronto.".to_string())?
    };
    let wav = std::env::temp_dir().join(format!("bob-lrc-{}.wav", std::process::id()));
    let status = tokio_command(&ffmpeg)
        .arg("-y")
        .arg("-i")
        .arg(extended_path(Path::new(path)))
        .arg("-t")
        .arg("360")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg("16000")
        .arg(extended_path(&wav))
        .output()
        .await
        .map_err(|error| error.to_string())?;
    if !status.status.success() {
        return Err("não consegui preparar o áudio pra transcrição.".into());
    }
    let bytes = tokio::fs::read(&wav).await.map_err(|error| error.to_string())?;
    let _ = tokio::fs::remove_file(&wav).await;
    if bytes.len() > 12_000_000 {
        return Err("o áudio ficou grande demais pra transcrever.".into());
    }
    let prompt = "transcreva a letra cantada. devolva só linhas LRC no formato [mm:ss.xx]texto. se não houver letra, responda [instrumental].";
    let text = generate_audio(&key, &model, prompt, &bytes).await;
    match text {
        Ok(body) => {
            let use_ = ledger.models.entry(model).or_insert_with(|| ModelUse::fresh(&utc_day(now)));
            note_success(use_, now, estimate_tokens(body.len() + bytes.len() / 8));
            ledger.lyrics_count += 1;
            save(&dir, &ledger);
            let lrc = if body.to_lowercase().contains("[instrumental]") {
                "[by:gerada por IA]\n[00:00.00]instrumental\n".to_string()
            } else {
                lrc_from_transcript(&body)
            };
            let target = PathBuf::from(path).with_extension("lrc");
            let _ = std::fs::write(&target, &lrc);
            Ok(lrc)
        }
        Err(status) => {
            if status == 429 {
                let use_ = ledger.models.entry(model.clone()).or_insert_with(|| ModelUse::fresh(&utc_day(now)));
                note_429(use_, now);
                save(&dir, &ledger);
            }
            Err(bob_core::explain_status(&model, status))
        }
    }
}

pub async fn complete_json(app: &tauri::AppHandle, prompt: &str) -> Option<Value> {
    let (config, dir) = context(app).await;
    let text = flash_body(&config, &dir, prompt, estimate_tokens(prompt.len()), true).await.ok()?;
    bob_core::parse_model_json(&text).ok()
}

fn stored_key() -> String {
    crate::drive::read_gemini_key().unwrap_or_default()
}

async fn bulk(config: &Config, dir: &Path, prompt: &str, tokens: u32) -> Option<String> {
    let key = stored_key();
    if key.trim().is_empty() {
        return None;
    }
    let mut ledger = load(dir);
    let now = chrono::Utc::now().timestamp();
    let day = utc_day(now);
    let model = config.gemini_bulk.clone();
    let use_ = ledger.models.get(&model).cloned().unwrap_or_else(|| ModelUse::fresh(&day));
    if !allowed(&use_, Slot::Bulk, now, tokens) {
        return None;
    }
    match generate_text(&key, &model, prompt, false).await {
        Ok(text) => {
            let use_ = ledger.models.entry(model).or_insert_with(|| ModelUse::fresh(&day));
            note_success(use_, now, estimate_tokens(prompt.len() + text.len()));
            save(dir, &ledger);
            Some(text)
        }
        Err(429) => {
            let use_ = ledger.models.entry(model).or_insert_with(|| ModelUse::fresh(&day));
            note_429(use_, now);
            save(dir, &ledger);
            None
        }
        Err(_) => None,
    }
}

async fn flash(config: &Config, dir: &Path, prompt: &str, tokens: u32) -> Option<String> {
    flash_body(config, dir, prompt, tokens, false).await.ok()
}

async fn flash_result(config: &Config, dir: &Path, prompt: &str, tokens: u32) -> Result<String, String> {
    flash_body(config, dir, prompt, tokens, false).await
}

async fn flash_body(config: &Config, dir: &Path, prompt: &str, tokens: u32, json_mode: bool) -> Result<String, String> {
    let key = stored_key();
    if key.trim().is_empty() {
        return Err("sem chave do gemini".into());
    }
    let mut ledger = load(dir);
    let now = chrono::Utc::now().timestamp();
    let day = utc_day(now);
    let primary = ledger.models.get(&config.gemini_flash).cloned().unwrap_or_else(|| ModelUse::fresh(&day));
    let fallback_use = ledger.models.get(&config.gemini_flash_fallback).cloned().unwrap_or_else(|| ModelUse::fresh(&day));
    let route = bob_core::route(&primary, Some(&fallback_use), Slot::Flash, now, tokens);
    let (model, fell_back) = match route {
        Route::Allow => (config.gemini_flash.clone(), false),
        Route::Fallback => (config.gemini_flash_fallback.clone(), true),
        Route::Heuristic => return Err(bob_core::explain_block(&config.gemini_flash, &primary, now)),
    };
    match generate_text(&key, &model, prompt, json_mode).await {
        Ok(text) => {
            let use_ = ledger.models.entry(model.clone()).or_insert_with(|| ModelUse::fresh(&day));
            note_success(use_, now, estimate_tokens(prompt.len() + text.len()));
            save(dir, &ledger);
            if fell_back {
                Ok(format!("{text}\n\nmodelo: {model} (reserva)"))
            } else {
                Ok(text)
            }
        }
        Err(status) => {
            if status == 429 {
                let use_ = ledger.models.entry(model.clone()).or_insert_with(|| ModelUse::fresh(&day));
                note_429(use_, now);
                save(dir, &ledger);
            }
            let detail = bob_core::explain_status(&model, status);
            if status == 0 {
                Err("sem internet".into())
            } else {
                Err(detail)
            }
        }
    }
}

async fn embed_query(config: &Config, ledger: &mut Ledger, query: &str) -> Option<Vec<f32>> {
    let mut fresh = 0;
    cached_or_embed(config, ledger, query, &mut fresh, 1).await
}

async fn cached_or_embed(config: &Config, ledger: &mut Ledger, document: &str, fresh: &mut u32, cap: u32) -> Option<Vec<f32>> {
    let key = format!("{:x}", fnv(document));
    if let Some(vector) = ledger.embeddings.get(&key) {
        return Some(vector.clone());
    }
    let key = stored_key();
    if *fresh >= cap || key.trim().is_empty() {
        return None;
    }
    let now = chrono::Utc::now().timestamp();
    let day = utc_day(now);
    let model = config.gemini_embed.clone();
    let use_ = ledger.models.entry(model.clone()).or_insert_with(|| ModelUse::fresh(&day));
    if !allowed(use_, Slot::Embed, now, estimate_tokens(document.len())) {
        return None;
    }
    let vector = embed_text(&key, &model, document).await.ok()?;
    let use_ = ledger.models.entry(model).or_insert_with(|| ModelUse::fresh(&day));
    note_success(use_, now, estimate_tokens(document.len()));
    ledger.embeddings.insert(key, vector.clone());
    *fresh += 1;
    Some(vector)
}

async fn generate_text(key: &str, model: &str, prompt: &str, json_mode: bool) -> Result<String, u16> {
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}");
    let mut body = json!({ "contents": [{ "parts": [{ "text": prompt }] }] });
    if json_mode {
        body["generationConfig"] = json!({ "responseMimeType": "application/json", "temperature": 0.4 });
    }
    post_json(&url, body).await
}

async fn generate_audio(key: &str, model: &str, prompt: &str, wav: &[u8]) -> Result<String, u16> {
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={key}");
    let body = json!({
        "contents": [{
            "parts": [
                { "text": prompt },
                { "inline_data": { "mime_type": "audio/wav", "data": base64::engine::general_purpose::STANDARD.encode(wav) } }
            ]
        }]
    });
    post_json(&url, body).await
}

async fn embed_text(key: &str, model: &str, text: &str) -> Result<Vec<f32>, u16> {
    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:embedContent?key={key}");
    let body = json!({ "content": { "parts": [{ "text": text }] } });
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(40)).build().map_err(|_| 0u16)?;
    let response = client.post(url).json(&body).send().await.map_err(|_| 0u16)?;
    let status = response.status().as_u16();
    if status == 429 {
        return Err(429);
    }
    if !response.status().is_success() {
        return Err(status);
    }
    let value: Value = response.json().await.map_err(|_| 0u16)?;
    let values = value.pointer("/embedding/values").and_then(Value::as_array).ok_or(0u16)?;
    Ok(values.iter().filter_map(|item| item.as_f64().map(|number| number as f32)).collect())
}

async fn post_json(url: &str, body: Value) -> Result<String, u16> {
    let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(90)).build().map_err(|_| 0u16)?;
    let response = client.post(url).json(&body).send().await.map_err(|_| 0u16)?;
    let status = response.status().as_u16();
    if status == 429 {
        return Err(429);
    }
    if !response.status().is_success() {
        return Err(status);
    }
    let value: Value = response.json().await.map_err(|_| 0u16)?;
    value
        .pointer("/candidates/0/content/parts/0/text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(0)
}

fn view(ledger: &Ledger, model: &str, slot: Slot, day: &str) -> UsageView {
    let requests = ledger.models.get(model).filter(|use_| use_.day == day).map(|use_| use_.requests).unwrap_or(0);
    UsageView { model: model.to_string(), requests, limit: bob_core::limits(slot).rpd }
}

async fn context(app: &tauri::AppHandle) -> (Config, PathBuf) {
    let inner = app.state::<AppState>().inner.clone().lock_owned().await;
    (inner.config.clone(), inner.data_dir.clone())
}

fn ledger_path(dir: &Path) -> PathBuf {
    dir.join("ai-usage.json")
}

fn load(dir: &Path) -> Ledger {
    std::fs::read_to_string(ledger_path(dir)).ok().and_then(|text| serde_json::from_str(&text).ok()).unwrap_or_default()
}

fn save(dir: &Path, ledger: &Ledger) {
    let _ = std::fs::create_dir_all(dir);
    if let Ok(text) = serde_json::to_string(ledger) {
        let _ = std::fs::write(ledger_path(dir), text);
    }
}

fn fnv(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}
