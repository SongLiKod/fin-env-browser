use std::path::{Path, PathBuf};

use crate::error::{AppError, Result};

pub const APP_DIR_NAME: &str = "FinEnv";

/// Resolve the data directory. `FIN_ENV_DATA_DIR` is honoured for CI and tests.
pub fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("FIN_ENV_DATA_DIR") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR_NAME)
}

pub fn ensure_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)
        .map_err(|e| AppError::internal(format!("无法创建目录 {}: {e}", path.display())))
}

pub fn db_path(data_dir: &Path) -> PathBuf {
    data_dir.join("finenv.sqlite3")
}

pub fn master_salt_path(data_dir: &Path) -> PathBuf {
    data_dir.join("master.salt")
}

pub fn profiles_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("profiles")
}

pub fn env_profile_dir(data_dir: &Path, env_id: &str) -> PathBuf {
    profiles_dir(data_dir).join(env_id)
}

pub fn screenshots_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("screenshots")
}

pub fn recordings_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("recordings")
}

pub fn backups_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("backups")
}

pub fn reports_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("reports")
}

pub fn start_pages_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("start-pages")
}

pub fn start_page_path(data_dir: &Path, env_id: &str) -> PathBuf {
    start_pages_dir(data_dir).join(format!("{env_id}.html"))
}

pub fn logs_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("logs")
}

pub fn har_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("har")
}

pub fn exports_dir(data_dir: &Path) -> PathBuf {
    data_dir.join("exports")
}

pub fn bundled_chromium_dir() -> PathBuf {
    // dev: src-tauri -> repo/chromium-bin; packaged: beside the executable
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.join("chromium-bin"))
        .unwrap_or_else(|| PathBuf::from("chromium-bin"));
    if dev.exists() {
        return dev;
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("chromium-bin")))
        .unwrap_or(dev)
}

pub fn init_all(data_dir: &Path) -> Result<()> {
    for dir in [
        data_dir.to_path_buf(),
        profiles_dir(data_dir),
        screenshots_dir(data_dir),
        recordings_dir(data_dir),
        backups_dir(data_dir),
        reports_dir(data_dir),
        start_pages_dir(data_dir),
        logs_dir(data_dir),
        har_dir(data_dir),
        exports_dir(data_dir),
    ] {
        ensure_dir(&dir)?;
    }
    Ok(())
}
