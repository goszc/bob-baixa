//! Per-user Explorer context menu. HKCU only, so no administrator rights.
//! The NSIS uninstaller deletes [`bob_core::CONTEXT_MENU_KEYS`].

#[cfg(windows)]
use bob_core::{context_menu_command, context_menu_label, text, CONTEXT_MENU_KEYS};

pub fn sync(enabled: bool, language: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        apply(enabled, language)
    }
    #[cfg(not(windows))]
    {
        let _ = (enabled, language);
        Ok(())
    }
}

#[cfg(windows)]
fn apply(enabled: bool, language: &str) -> Result<(), String> {
    if !enabled {
        remove_all();
        return Ok(());
    }
    if let Err(error) = install(language) {
        remove_all();
        return Err(error);
    }
    Ok(())
}

#[cfg(windows)]
fn install(language: &str) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|error| error.to_string())?;
    let exe = exe.to_string_lossy().to_string();
    let label = context_menu_label(language);
    let command = context_menu_command(&exe);
    for key in CONTEXT_MENU_KEYS {
        write_default(key, label, language)?;
        write_named(key, "Icon", &exe, language)?;
        write_default(&format!(r"{key}\command"), &command, language)?;
    }
    Ok(())
}

#[cfg(windows)]
fn remove_all() {
    for key in CONTEXT_MENU_KEYS {
        let _ = crate::hidden::std_command("reg")
            .args(["delete", &format!(r"HKCU\{key}"), "/f"])
            .output();
    }
}

#[cfg(windows)]
fn write_default(key: &str, data: &str, language: &str) -> Result<(), String> {
    reg_add(key, None, data, language)
}

#[cfg(windows)]
fn write_named(key: &str, name: &str, data: &str, language: &str) -> Result<(), String> {
    reg_add(key, Some(name), data, language)
}

#[cfg(windows)]
fn reg_add(key: &str, name: Option<&str>, data: &str, language: &str) -> Result<(), String> {
    let mut cmd = crate::hidden::std_command("reg");
    cmd.arg("add").arg(format!(r"HKCU\{key}"));
    if let Some(name) = name {
        cmd.arg("/v").arg(name);
    } else {
        cmd.arg("/ve");
    }
    cmd.args(["/t", "REG_SZ", "/d", data, "/f"]);
    let output = cmd.output().map_err(|error| error.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = format!(
            "{} {}",
            String::from_utf8_lossy(&output.stderr).trim(),
            String::from_utf8_lossy(&output.stdout).trim()
        );
        let detail = detail.trim();
        let message = text(
            language,
            "não consegui registrar o menu de contexto.",
            "could not register the context menu.",
        );
        Err(if detail.is_empty() {
            message
        } else {
            format!("{message} {detail}")
        })
    }
}
