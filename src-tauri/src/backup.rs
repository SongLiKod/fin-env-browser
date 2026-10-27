use std::{
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
};

use serde_json::json;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipArchive, ZipWriter};

use crate::{
    crypto::CryptoManager,
    db::Db,
    error::{AppError, Result},
    models::{now_iso, BackupInfo, Settings, SyncResult},
    paths,
};

const BACKUP_EXT: &str = "finenvbackup";

pub fn create_backup(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    include_profiles: bool,
    passphrase: Option<&str>,
) -> Result<Vec<u8>> {
    paths::ensure_dir(&paths::backups_dir(data_dir))?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let tmp_db = paths::backups_dir(data_dir).join(format!(".backup-{stamp}.sqlite"));
    db.backup_to(&tmp_db)?;
    let db_bytes = std::fs::read(&tmp_db)?;
    let _ = std::fs::remove_file(&tmp_db);

    let mut zip_bytes = Cursor::new(Vec::<u8>::new());
    {
        let mut zip = ZipWriter::new(&mut zip_bytes);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600);
        zip.start_file("database.sqlite", options)
            .map_err(|e| AppError::internal(format!("创建备份失败: {e}")))?;
        zip.write_all(&db_bytes)?;
        zip.start_file("meta.json", options)
            .map_err(|e| AppError::internal(format!("创建备份失败: {e}")))?;
        let meta = json!({
            "app": "FinEnv",
            "version": env!("CARGO_PKG_VERSION"),
            "createdAt": now_iso(),
            "includesProfiles": include_profiles,
            "envCount": db.environment_count().unwrap_or(0),
            "formatVersion": 1
        });
        zip.write_all(&serde_json::to_vec_pretty(&meta)?)?;
        if passphrase.filter(|value| !value.is_empty()).is_some() {
            zip.start_file("master.key", options)
                .map_err(|e| AppError::internal(format!("创建备份失败: {e}")))?;
            zip.write_all(&crypto.export_key()?)?;
        }
        let salt = paths::master_salt_path(data_dir);
        if salt.exists() {
            zip.start_file("master.salt", options)
                .map_err(|e| AppError::internal(format!("创建备份失败: {e}")))?;
            zip.write_all(&std::fs::read(&salt)?)?;
        }
        if include_profiles {
            let profiles = paths::profiles_dir(data_dir);
            if profiles.exists() {
                add_dir_to_zip(&mut zip, &profiles, "profiles", options)?;
            }
        }
        zip.finish()
            .map_err(|e| AppError::internal(format!("完成备份失败: {e}")))?;
    }
    let zip_bytes = zip_bytes.into_inner();
    crypto.encrypt_package_if_needed(&zip_bytes, passphrase)
}

pub fn restore_backup(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    bytes: &[u8],
    passphrase: Option<&str>,
    restore_profiles: bool,
) -> Result<String> {
    // Make sure the package is a valid encrypted/plain FinEnv package.
    let zip_bytes = crypto.decrypt_package_if_needed(bytes, passphrase)?;
    paths::ensure_dir(&paths::backups_dir(data_dir))?;
    let mut archive = ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| AppError::bad_request(format!("备份包不是有效 ZIP: {e}")))?;
    let temp_db =
        paths::backups_dir(data_dir).join(format!(".restore-{}.sqlite", uuid::Uuid::new_v4()));
    {
        let mut file = archive
            .by_name("database.sqlite")
            .map_err(|_| AppError::bad_request("备份包缺少 database.sqlite"))?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        std::fs::write(&temp_db, buffer)?;
    }
    {
        let conn = rusqlite::Connection::open(&temp_db)?;
        let check: String = conn.query_row("PRAGMA integrity_check", [], |row| row.get(0))?;
        if check != "ok" {
            let _ = std::fs::remove_file(&temp_db);
            return Err(AppError::bad_request(format!(
                "备份数据库完整性校验失败: {check}"
            )));
        }
    }
    let backup_key = match archive.by_name("master.key") {
        Ok(mut file) => {
            let mut key = [0u8; 32];
            file.read_exact(&mut key)
                .map_err(|_| AppError::bad_request("备份包中的 master.key 损坏"))?;
            Some(key)
        }
        Err(_) => None,
    };
    db.restore_from_file(&temp_db)?;
    let _ = std::fs::remove_file(&temp_db);
    let _ = db.clear_browser_sessions();
    if let Some(key) = backup_key {
        crypto.import_key(key)?;
    }

    let mut restored_profiles = 0u32;
    if restore_profiles {
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::internal(format!("读取备份条目失败: {e}")))?;
            let name = entry.name().to_string();
            if !entry.is_dir() && name.starts_with("profiles/") {
                let relative = name.trim_start_matches("profiles/");
                if relative.contains("..") {
                    continue;
                }
                let target = paths::profiles_dir(data_dir).join(relative);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let mut buffer = Vec::new();
                entry.read_to_end(&mut buffer)?;
                std::fs::write(target, buffer)?;
                restored_profiles += 1;
            }
        }
    }
    Ok(format!(
        "数据库已恢复，{} 个 profile 文件已还原。加密字段已使用备份包主密钥（若包含）重新接管；请重新登录并检查设置。",
        restored_profiles
    ))
}

fn add_dir_to_zip(
    zip: &mut ZipWriter<&mut Cursor<Vec<u8>>>,
    dir: &Path,
    prefix: &str,
    options: SimpleFileOptions,
) -> Result<()> {
    for entry in walk(dir)? {
        let path = entry;
        if !path.is_file() {
            continue;
        }
        let relative = path
            .strip_prefix(dir)
            .map_err(|_| AppError::internal("遍历 profile 目录失败"))?;
        let name = format!("{prefix}/{}", relative.to_string_lossy().replace('\\', "/"));
        zip.start_file(name, options)
            .map_err(|e| AppError::internal(format!("写入备份条目失败: {e}")))?;
        zip.write_all(&std::fs::read(&path)?)?;
    }
    Ok(())
}

fn walk(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut result = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let entries = std::fs::read_dir(&current)?;
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                result.push(path);
            }
        }
    }
    Ok(result)
}

pub fn list_backups(data_dir: &Path) -> Result<Vec<BackupInfo>> {
    let dir = paths::backups_dir(data_dir);
    paths::ensure_dir(&dir)?;
    let mut items = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(BACKUP_EXT) {
            continue;
        }
        let metadata = entry.metadata()?;
        let encrypted = {
            use std::io::Read;
            let mut header = [0u8; 9];
            std::fs::File::open(&path)
                .and_then(|mut file| file.read_exact(&mut header))
                .map(|_| &header[..8] == b"FINENV01" && header[8] == b'E')
                .unwrap_or(false)
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .and_then(|secs| chrono::DateTime::from_timestamp(secs as i64, 0))
            .map(|d| d.to_rfc3339())
            .unwrap_or_else(now_iso);
        items.push(BackupInfo {
            path: path.to_string_lossy().to_string(),
            name,
            size: metadata.len(),
            encrypted,
            created_at: modified,
        });
    }
    items.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(items)
}

// ---------------------------------------------------------------------------
// Optional encrypted cloud sync (WebDAV or local folder)
// ---------------------------------------------------------------------------

pub async fn sync_push(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    settings: &Settings,
    passphrase: &str,
) -> Result<SyncResult> {
    if !settings.sync_enabled {
        return Err(AppError::bad_request("云同步未开启"));
    }
    if passphrase.chars().count() < 6 {
        return Err(AppError::bad_request("同步口令至少需要 6 个字符"));
    }
    let bytes = create_backup(db, crypto, data_dir, false, Some(passphrase))?;
    let size = bytes.len() as u64;
    match settings.sync_kind.as_str() {
        "folder" => {
            let endpoint = settings
                .sync_endpoint
                .as_deref()
                .ok_or_else(|| AppError::bad_request("未配置本地同步目录"))?;
            let dir = PathBuf::from(endpoint);
            std::fs::create_dir_all(&dir)?;
            let target = dir.join(&settings.sync_remote_path);
            std::fs::write(target, &bytes)?;
        }
        "webdav" => {
            let url = webdav_url(settings)?;
            let password = decrypt_sync_password(crypto, settings)?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(AppError::from)?;
            let mut request = client
                .put(&url)
                .header("Content-Type", "application/octet-stream")
                .body(bytes);
            if let Some(user) = settings.sync_username.as_deref().filter(|s| !s.is_empty()) {
                request = request.basic_auth(user, password.as_deref());
            }
            let response = request.send().await?;
            if !response.status().is_success() {
                return Err(AppError::bad_request(format!(
                    "WebDAV 上传失败: HTTP {}",
                    response.status()
                )));
            }
        }
        other => return Err(AppError::bad_request(format!("不支持的同步类型: {other}"))),
    }
    Ok(SyncResult {
        ok: true,
        message: format!("加密同步包已上传（{} bytes）", size),
        synced_at: now_iso(),
        bytes: size,
    })
}

pub async fn sync_pull(
    db: &Db,
    crypto: &CryptoManager,
    data_dir: &Path,
    settings: &Settings,
    passphrase: &str,
) -> Result<SyncResult> {
    if !settings.sync_enabled {
        return Err(AppError::bad_request("云同步未开启"));
    }
    let bytes = match settings.sync_kind.as_str() {
        "folder" => {
            let endpoint = settings
                .sync_endpoint
                .as_deref()
                .ok_or_else(|| AppError::bad_request("未配置本地同步目录"))?;
            let path = PathBuf::from(endpoint).join(&settings.sync_remote_path);
            std::fs::read(&path)
                .map_err(|e| AppError::bad_request(format!("读取同步包失败: {e}")))?
        }
        "webdav" => {
            let url = webdav_url(settings)?;
            let password = decrypt_sync_password(crypto, settings)?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .map_err(AppError::from)?;
            let mut request = client.get(&url);
            if let Some(user) = settings.sync_username.as_deref().filter(|s| !s.is_empty()) {
                request = request.basic_auth(user, password.as_deref());
            }
            let response = request.send().await?;
            if !response.status().is_success() {
                return Err(AppError::bad_request(format!(
                    "WebDAV 下载失败: HTTP {}",
                    response.status()
                )));
            }
            response.bytes().await?.to_vec()
        }
        other => return Err(AppError::bad_request(format!("不支持的同步类型: {other}"))),
    };
    let size = bytes.len() as u64;
    restore_backup(db, crypto, data_dir, &bytes, Some(passphrase), false)?;
    Ok(SyncResult {
        ok: true,
        message: format!("加密同步包已下载并恢复（{} bytes）", size),
        synced_at: now_iso(),
        bytes: size,
    })
}

pub async fn sync_test(settings: &Settings, password: Option<String>) -> Result<String> {
    match settings.sync_kind.as_str() {
        "folder" => {
            let endpoint = settings
                .sync_endpoint
                .as_deref()
                .ok_or_else(|| AppError::bad_request("未配置本地同步目录"))?;
            let dir = PathBuf::from(endpoint);
            std::fs::create_dir_all(&dir)?;
            let probe = dir.join(".finenv-write-test");
            std::fs::write(&probe, b"ok")?;
            let _ = std::fs::remove_file(&probe);
            Ok(format!("本地同步目录可写: {}", dir.display()))
        }
        "webdav" => {
            let url = webdav_url(settings)?;
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(15))
                .build()
                .map_err(AppError::from)?;
            let mut request =
                client.request(reqwest::Method::from_bytes(b"PROPFIND").unwrap(), &url);
            if let Some(user) = settings.sync_username.as_deref().filter(|s| !s.is_empty()) {
                request = request.basic_auth(user, password);
            }
            let response = request
                .header("Depth", "0")
                .send()
                .await
                .map_err(|e| AppError::bad_request(format!("WebDAV 连接失败: {e}")))?;
            if response.status().is_success() || response.status().as_u16() == 207 {
                Ok(format!("WebDAV 连接成功: {url}"))
            } else {
                Err(AppError::bad_request(format!(
                    "WebDAV 返回 HTTP {}",
                    response.status()
                )))
            }
        }
        other => Err(AppError::bad_request(format!("不支持的同步类型: {other}"))),
    }
}

fn webdav_url(settings: &Settings) -> Result<String> {
    let endpoint = settings
        .sync_endpoint
        .as_deref()
        .ok_or_else(|| AppError::bad_request("未配置 WebDAV 地址"))?;
    let base = url::Url::parse(endpoint)
        .map_err(|e| AppError::bad_request(format!("WebDAV 地址无效: {e}")))?;
    base.join(&settings.sync_remote_path)
        .map(|u| u.to_string())
        .map_err(|e| AppError::bad_request(format!("远端文件名无效: {e}")))
}

fn decrypt_sync_password(crypto: &CryptoManager, settings: &Settings) -> Result<Option<String>> {
    settings
        .sync_password_enc
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|enc| crypto.decrypt(enc))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::CryptoManager,
        db::Db,
        models::{new_id, now_iso, Account, Environment},
    };

    #[test]
    fn encrypted_backup_is_portable_and_restores_secrets() {
        let source_dir =
            std::env::temp_dir().join(format!("finenv-bk-src-{}", uuid::Uuid::new_v4()));
        let target_dir =
            std::env::temp_dir().join(format!("finenv-bk-dst-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&source_dir).unwrap();
        std::fs::create_dir_all(&target_dir).unwrap();
        let source_crypto = std::sync::Arc::new(CryptoManager::initialize(&source_dir).unwrap());
        let source_db = Db::open(&source_dir.join("source.sqlite3")).unwrap();
        source_db.set_crypto(source_crypto.clone());
        let crypto = source_crypto;

        let now = now_iso();
        let env = Environment {
            id: new_id(),
            name: "portable backup".into(),
            group_id: None,
            status: "active".into(),
            profile_dir: source_dir.join("profile").to_string_lossy().to_string(),
            proxy_id: None,
            fingerprint_snapshot_id: None,
            project: "backup".into(),
            test_env: "test".into(),
            account_role: "tester".into(),
            version: "1".into(),
            case_id: "CASE-BK".into(),
            tags: vec![],
            notes: String::new(),
            kernel: "chromium".into(),
            chromium_path: None,
            auto_restart: false,
            memory_limit_mb: None,
            idle_timeout_min: None,
            fingerprint: crate::fingerprint::template("win11"),
            border_color: "#2563EB".into(),
            ip_address: None,
            ip_country: None,
            ip_country_code: None,
            ip_city: None,
            ip_timezone: None,
            ip_source: None,
            ip_checked_at: None,
            created_at: now.clone(),
            updated_at: now.clone(),
            last_opened_at: None,
        };
        source_db.insert_environment(&env).unwrap();
        let account = Account {
            id: new_id(),
            env_id: env.id.clone(),
            username: "portable".into(),
            password_enc: crypto.encrypt("portable-secret").unwrap(),
            has_password: true,
            remark: String::new(),
            owner: "QA".into(),
            status: "pending".into(),
            created_at: now.clone(),
            updated_at: now,
        };
        source_db.insert_account(&account).unwrap();

        let bytes = create_backup(
            &source_db,
            crypto.as_ref(),
            &source_dir,
            false,
            Some("portable-passphrase"),
        )
        .unwrap();
        let target_crypto = CryptoManager::initialize(&target_dir).unwrap();
        let target_db = Db::open(&target_dir.join("target.sqlite3")).unwrap();
        target_db.set_crypto(std::sync::Arc::new(target_crypto));
        let target_crypto = target_db.crypto().unwrap();
        restore_backup(
            &target_db,
            target_crypto.as_ref(),
            &target_dir,
            &bytes,
            Some("portable-passphrase"),
            false,
        )
        .unwrap();
        let restored = target_db.get_environment(&env.id).unwrap();
        assert_eq!(restored.fingerprint.user_agent, env.fingerprint.user_agent);
        let restored_account = target_db.get_account(&account.id).unwrap();
        assert_eq!(
            target_db
                .crypto()
                .unwrap()
                .decrypt(&restored_account.password_enc)
                .unwrap(),
            "portable-secret"
        );
        let _ = std::fs::remove_dir_all(source_dir);
        let _ = std::fs::remove_dir_all(target_dir);
    }
}
