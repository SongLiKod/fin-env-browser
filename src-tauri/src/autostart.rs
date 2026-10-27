use std::path::Path;

/// Best-effort OS autostart registration. Failures are logged but never block
/// saving settings.
pub fn apply(_app: &tauri::AppHandle, enabled: bool) {
    let exe = match std::env::current_exe() {
        Ok(path) => path,
        Err(e) => {
            tracing::warn!(error = %e, "cannot resolve executable for autostart");
            return;
        }
    };
    if let Err(e) = apply_platform(&exe, enabled) {
        tracing::warn!(error = %e, enabled, "autostart update failed");
    }
}

#[cfg(target_os = "windows")]
fn apply_platform(exe: &Path, enabled: bool) -> std::io::Result<()> {
    if enabled {
        std::process::Command::new("reg")
            .args([
                "add",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "FinEnv",
                "/t",
                "REG_SZ",
                "/d",
                &format!("\"{}\"", exe.display()),
                "/f",
            ])
            .status()?;
    } else {
        let _ = std::process::Command::new("reg")
            .args([
                "delete",
                "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run",
                "/v",
                "FinEnv",
                "/f",
            ])
            .status();
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn apply_platform(exe: &Path, enabled: bool) -> std::io::Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let path = std::path::PathBuf::from(home).join("Library/LaunchAgents/com.finenv.browser.plist");
    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\"><dict><key>Label</key><string>com.finenv.browser</string><key>ProgramArguments</key><array><string>{}</string></array><key>RunAtLoad</key><true/></dict></plist>\n",
            exe.display()
        );
        std::fs::write(path, content)?;
    } else {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn apply_platform(exe: &Path, enabled: bool) -> std::io::Result<()> {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let path = std::path::PathBuf::from(home).join(".config/autostart/finenv.desktop");
    if enabled {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = format!(
            "[Desktop Entry]\nType=Application\nName=FinEnv\nExec=\"{}\"\nX-GNOME-Autostart-enabled=true\n",
            exe.display()
        );
        std::fs::write(path, content)?;
    } else {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

#[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
fn apply_platform(_exe: &Path, _enabled: bool) -> std::io::Result<()> {
    Ok(())
}
