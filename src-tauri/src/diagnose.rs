//! A zip on the Desktop the user can send. Secrets stay out of it.

use std::io::Write;
use std::path::{Path, PathBuf};

use bob_core::{redact_json, redact_text};
use serde_json::Value;
use tauri::Manager;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::hidden::tokio_command;
use crate::runtime::AppState;

#[tauri::command]
pub async fn make_diagnostic(app: tauri::AppHandle) -> Result<String, String> {
    let desktop = desktop_dir()?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let name = format!("bob-baixa-diagnostico-{stamp}.zip");
    let destination = desktop.join(&name);
    let report = collect(&app).await;
    write_zip(&destination, &report)?;
    reveal(&destination)?;
    Ok(destination.to_string_lossy().into_owned())
}

struct Report {
    body: String,
    settings: String,
    drive_log: String,
    job_logs: String,
    queues: String,
}

async fn collect(app: &tauri::AppHandle) -> Report {
    let inner = app.state::<AppState>().inner.clone().lock_owned().await;
    let mut settings = serde_json::to_value(&inner.config).unwrap_or(Value::Null);
    redact_json(&mut settings);
    let settings = serde_json::to_string_pretty(&settings).unwrap_or_else(|_| "{}".into());
    let jobs: Vec<String> = inner
        .jobs
        .iter()
        .map(|job| {
            redact_text(&format!(
                "{} | {} | {:.0}% | {} | {}",
                job.state,
                job.title,
                job.progress * 100.0,
                job.error.clone().unwrap_or_default(),
                job.url
            ))
        })
        .collect();
    let failures: Vec<String> = inner
        .history
        .iter()
        .filter(|entry| !entry.ok)
        .take(20)
        .map(|entry| redact_text(&format!("{} | {} | {}", entry.at, entry.title, entry.detail.clone().unwrap_or_default())))
        .collect();
    let job_logs = inner
        .logs
        .iter()
        .map(|(id, text)| format!("## {id}\n{}", redact_text(&tail(text, 80))))
        .collect::<Vec<_>>()
        .join("\n\n");
    let data = inner.data_dir.clone();
    let output = inner.config.output_dir.clone();
    let ytdlp = inner.tools.ytdlp_version.clone().unwrap_or_else(|| "ausente".into());
    let ffmpeg = inner.tools.ffmpeg_version.clone().unwrap_or_else(|| "ausente".into());
    let ytdlp_bin = inner.ytdlp.clone();
    let ffmpeg_bin = inner.ffmpeg_bin.clone();
    drop(inner);

    let fpcalc = fpcalc_version(&data).await;
    let ytdlp = if let Some(bin) = ytdlp_bin {
        version_of(&bin, "--version").await.unwrap_or(ytdlp)
    } else {
        ytdlp
    };
    let ffmpeg = if let Some(bin) = ffmpeg_bin {
        version_of(&bin, "-version").await.unwrap_or(ffmpeg)
    } else {
        ffmpeg
    };
    let drive_log = read_redacted(&data.join("drive.log"), 400);
    let library = library_stats(&data.join("drive-library.json"));
    let local = local_stats(Path::new(&output));
    let upload = redact_text(&crate::drive::queue_report());
    let queues = format!("## envio para o drive\n{upload}\n\n## downloads\n{}\n\n## últimos erros\n{}", jobs.join("\n"), failures.join("\n"));
    let body = format!(
        "bob baixa {}\n{}\n\nyt-dlp: {}\nffmpeg: {}\nfpcalc: {}\n\nbiblioteca do drive:\n{}\n\nbiblioteca local:\n{}\n",
        env!("CARGO_PKG_VERSION"),
        os_version(),
        ytdlp,
        ffmpeg,
        fpcalc,
        library,
        local
    );
    Report { body, settings, drive_log, job_logs, queues }
}

fn library_stats(path: &Path) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return "sem ficha".into();
    };
    let records: Vec<Value> = serde_json::from_str(&text).unwrap_or_default();
    let lyrics = records.iter().filter(|record| nonempty(record, "lyrics")).count();
    let covers = records.iter().filter(|record| nonempty(record, "cover_id") || nonempty(record, "cover_mime")).count();
    let probed = records.iter().filter(|record| record.get("probed").and_then(Value::as_bool).unwrap_or(false)).count();
    format!("{} faixas, {lyrics} com letra, {covers} com capa, {probed} já lidas", records.len())
}

fn local_stats(root: &Path) -> String {
    if !root.is_dir() {
        return "pasta de destino vazia".into();
    }
    let mut audio = 0u32;
    let mut video = 0u32;
    let mut lyrics = 0u32;
    let mut seen = 0u32;
    walk(root, &mut |path| {
        seen += 1;
        if seen > 20000 {
            return;
        }
        let name = path.file_name().and_then(|name| name.to_str()).unwrap_or("").to_ascii_lowercase();
        if name.ends_with(".lrc") {
            lyrics += 1;
        } else if matches!(ext(&name).as_str(), "mp3" | "m4a" | "opus" | "flac" | "ogg" | "wav") {
            audio += 1;
        } else if matches!(ext(&name).as_str(), "mp4" | "mkv" | "webm" | "mov") {
            video += 1;
        }
    });
    format!("{audio} áudios, {video} vídeos, {lyrics} letras")
}

fn walk(dir: &Path, visit: &mut dyn FnMut(&Path)) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, visit);
        } else {
            visit(&path);
        }
    }
}

fn ext(name: &str) -> String {
    name.rsplit('.').next().unwrap_or("").to_string()
}

fn nonempty(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_str).is_some_and(|text| !text.trim().is_empty())
}

fn read_redacted(path: &Path, lines: usize) -> String {
    let Ok(text) = std::fs::read_to_string(path) else {
        return "sem log".into();
    };
    redact_text(&tail(&text, lines))
}

fn tail(text: &str, lines: usize) -> String {
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(lines);
    all[start..].join("\n")
}

async fn fpcalc_version(data: &Path) -> String {
    let bundled = data.join("fpcalc").join(if cfg!(windows) { "fpcalc.exe" } else { "fpcalc" });
    if let Some(version) = version_of(&bundled, "-version").await {
        return version;
    }
    "ausente".into()
}

async fn version_of(bin: &Path, arg: &str) -> Option<String> {
    if !bin.is_file() {
        return None;
    }
    let output = tokio_command(bin).arg(arg).output().await.ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let err = String::from_utf8_lossy(&output.stderr);
    let line = text.lines().chain(err.lines()).find(|line| !line.trim().is_empty())?.trim();
    if arg == "-version" {
        Some(line.split_whitespace().nth(2).unwrap_or(line).to_string())
    } else {
        Some(line.to_string())
    }
}

fn write_zip(path: &Path, report: &Report) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let file = std::fs::File::create(path).map_err(|error| error.to_string())?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    add(&mut zip, "diagnostico.txt", &report.body, options)?;
    add(&mut zip, "configuracoes.json", &report.settings, options)?;
    add(&mut zip, "drive.log", &report.drive_log, options)?;
    add(&mut zip, "filas.txt", &report.queues, options)?;
    add(&mut zip, "logs-de-download.txt", &report.job_logs, options)?;
    zip.finish().map_err(|error| error.to_string())?;
    Ok(())
}

fn add(zip: &mut ZipWriter<std::fs::File>, name: &str, text: &str, options: SimpleFileOptions) -> Result<(), String> {
    zip.start_file(name, options).map_err(|error| error.to_string())?;
    zip.write_all(text.as_bytes()).map_err(|error| error.to_string())?;
    Ok(())
}

fn desktop_dir() -> Result<PathBuf, String> {
    if let Some(dir) = dirs::desktop_dir() {
        if dir.is_dir() {
            return Ok(dir);
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        for extra in ["Desktop", "OneDrive\\Desktop", "OneDrive\\Área de Trabalho"] {
            let candidate = PathBuf::from(&profile).join(extra);
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
    }
    Err("não achei a área de trabalho.".into())
}

fn os_version() -> String {
    #[cfg(windows)]
    {
        let output = crate::hidden::std_command("cmd").args(["/c", "ver"]).output();
        let text = output.ok().map(|output| String::from_utf8_lossy(&output.stdout).trim().to_string()).unwrap_or_default();
        if text.is_empty() {
            "windows".into()
        } else {
            text
        }
    }
    #[cfg(not(windows))]
    {
        format!("não é windows ({})", std::env::consts::OS)
    }
}

fn reveal(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        let selected = path.to_string_lossy().replace('/', "\\");
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{selected}"))
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Ok(())
    }
}
