use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, RwLock},
};

use rusqlite::Connection;

use crate::{
    crypto::CryptoManager,
    error::{AppError, Result},
};

pub mod account;
pub mod audit;
pub mod auxiliary;
pub mod env;
pub mod member;
pub mod proxy;
pub mod settings;
pub mod stats;
pub mod template;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::CryptoManager;
    use crate::models::{new_id, now_iso, Environment, EnvironmentIpInfo};

    fn temp_db() -> (Db, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("finenv-db-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let crypto = std::sync::Arc::new(CryptoManager::initialize(&dir).unwrap());
        let db = Db::open(&dir.join("test.sqlite3")).unwrap();
        db.set_crypto(crypto);
        (db, dir)
    }

    fn sample_environment() -> Environment {
        let now = now_iso();
        Environment {
            id: new_id(),
            name: "单元测试环境".into(),
            group_id: None,
            status: "active".into(),
            profile_dir: "C:/tmp/finenv-profile".into(),
            proxy_id: None,
            fingerprint_snapshot_id: None,
            project: "单元测试".into(),
            test_env: "test".into(),
            account_role: "tester".into(),
            version: "1.0".into(),
            case_id: "CASE-1".into(),
            tags: vec!["unit".into()],
            notes: String::new(),
            kernel: "chromium".into(),
            chromium_path: None,
            chromium_launch_args: Vec::new(),
            auto_restart: false,
            memory_limit_mb: None,
            idle_timeout_min: None,
            fingerprint: crate::fingerprint::template("win11"),
            border_color: "#2563EB".into(),
            border_width: 3,
            ip_address: None,
            ip_country: None,
            ip_country_code: None,
            ip_city: None,
            ip_timezone: None,
            ip_source: None,
            ip_checked_at: None,
            created_at: now.clone(),
            updated_at: now,
            last_opened_at: None,
        }
    }

    #[test]
    fn fingerprint_is_encrypted_at_rest_and_round_trips() {
        let (db, dir) = temp_db();
        let env = sample_environment();
        db.insert_environment(&env).unwrap();
        let raw: String = db
            .with(|conn| {
                Ok(conn.query_row(
                    "SELECT fingerprint_json FROM environments WHERE id = ?1",
                    rusqlite::params![env.id],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert!(raw.starts_with("v1:"), "fingerprint must not be plaintext");
        assert!(!raw.contains(&env.fingerprint.user_agent));
        let loaded = db.get_environment(&env.id).unwrap();
        assert_eq!(loaded.fingerprint.user_agent, env.fingerprint.user_agent);
        assert_eq!(loaded.fingerprint.timezone, env.fingerprint.timezone);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn account_password_and_cookie_cache_are_encrypted_at_rest() {
        use crate::models::Account;

        let (db, dir) = temp_db();
        let env = sample_environment();
        db.insert_environment(&env).unwrap();
        let crypto = db.crypto().expect("crypto attached");
        let password_enc = crypto.encrypt("P@ssw0rd-cookie-test").unwrap();
        let account = Account {
            id: new_id(),
            env_id: env.id.clone(),
            username: "tester".into(),
            password_enc: password_enc.clone(),
            has_password: true,
            remark: String::new(),
            owner: "QA".into(),
            status: "pending".into(),
            created_at: now_iso(),
            updated_at: now_iso(),
        };
        db.insert_account(&account).unwrap();
        let raw_password: String = db
            .with(|conn| {
                Ok(conn.query_row(
                    "SELECT password_enc FROM accounts WHERE id = ?1",
                    rusqlite::params![account.id],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert!(raw_password.starts_with("v1:"));
        assert!(!raw_password.contains("P@ssw0rd-cookie-test"));
        assert_eq!(
            crypto.decrypt(&raw_password).unwrap(),
            "P@ssw0rd-cookie-test"
        );

        let cookies = r#"[{"name":"SESSION","value":"plain-cookie"}]"#;
        let encrypted_cookies = crypto.encrypt(cookies).unwrap();
        db.save_cookie_cache(&env.id, &encrypted_cookies).unwrap();
        let raw_cookies: String = db
            .with(|conn| {
                Ok(conn.query_row(
                    "SELECT cookies_enc FROM env_cookies WHERE env_id = ?1",
                    rusqlite::params![env.id],
                    |row| row.get(0),
                )?)
            })
            .unwrap();
        assert!(raw_cookies.starts_with("v1:"));
        assert!(!raw_cookies.contains("plain-cookie"));
        assert_eq!(crypto.decrypt(&raw_cookies).unwrap(), cookies);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn audit_chain_verifies_and_rejects_mutation() {
        let (db, dir) = temp_db();
        db.append_audit("tester", "env.create", None, None, None, None, "one")
            .unwrap();
        db.append_audit(
            "tester",
            "env.update",
            Some("env-1"),
            None,
            Some("project"),
            Some("CASE-1"),
            "two",
        )
        .unwrap();
        let verify = db.verify_audit_chain().unwrap();
        assert!(verify.valid);
        assert_eq!(verify.checked, 2);
        let update = db.with(|conn| {
            Ok(conn.execute("UPDATE audit_logs SET detail='tampered' WHERE rowid=1", [])?)
        });
        assert!(update.is_err(), "audit trigger must reject updates");
        let delete = db.with(|conn| Ok(conn.execute("DELETE FROM audit_logs", [])?));
        assert!(delete.is_err(), "audit trigger must reject deletes");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn migration_and_reopen_are_idempotent() {
        let (db, dir) = temp_db();
        let path = db.path().to_path_buf();
        drop(db);
        let reopened = Db::open(&path).unwrap();
        assert_eq!(reopened.environment_count().unwrap(), 0);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn environment_identity_and_ip_round_trip() {
        let (db, dir) = temp_db();
        let env = sample_environment();
        db.insert_environment(&env).unwrap();
        let info = EnvironmentIpInfo {
            env_id: env.id.clone(),
            ok: true,
            ip: Some("203.0.113.10".into()),
            country: Some("Japan".into()),
            country_code: Some("JP".into()),
            city: Some("Tokyo".into()),
            timezone: Some("Asia/Tokyo".into()),
            source: "proxy".into(),
            proxy_id: None,
            latency_ms: Some(320),
            message: "出口 IP 解析成功".into(),
            checked_at: now_iso(),
        };
        db.save_environment_ip(&info).unwrap();
        let loaded = db.get_environment(&env.id).unwrap();
        assert_eq!(loaded.border_color, "#2563EB");
        assert_eq!(loaded.border_width, 3);
        assert_eq!(loaded.ip_address.as_deref(), Some("203.0.113.10"));
        assert_eq!(loaded.ip_country.as_deref(), Some("Japan"));
        assert_eq!(loaded.ip_country_code.as_deref(), Some("JP"));
        assert_eq!(loaded.ip_city.as_deref(), Some("Tokyo"));
        assert_eq!(loaded.ip_timezone.as_deref(), Some("Asia/Tokyo"));
        assert_eq!(loaded.ip_source.as_deref(), Some("proxy"));
        assert!(loaded.ip_checked_at.is_some());
        let mut widened = loaded;
        widened.border_width = 7;
        db.update_environment(&widened).unwrap();
        assert_eq!(db.get_environment(&env.id).unwrap().border_width, 7);
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Small wrapper around a single SQLite connection.
///
/// SQLite allows one writer at a time; serialising access through a mutex keeps
/// behaviour predictable for both Tauri commands and the HTTP API. All
/// operations are short, and WAL mode keeps readers fast.
#[derive(Clone)]
pub struct Db {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
    crypto: Arc<RwLock<Option<Arc<CryptoManager>>>>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;
             PRAGMA temp_store = MEMORY;",
        )?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
            path: path.to_path_buf(),
            crypto: Arc::new(RwLock::new(None)),
        };
        db.migrate()?;
        Ok(db)
    }

    /// Attach the local key manager so fingerprint JSON and report payloads
    /// are encrypted at rest. Writes before this point are plaintext legacy
    /// rows and remain readable through the fallback in `decrypt_json`.
    pub fn set_crypto(&self, crypto: Arc<CryptoManager>) {
        if let Ok(mut guard) = self.crypto.write() {
            *guard = Some(crypto);
        }
    }

    pub fn crypto(&self) -> Option<Arc<CryptoManager>> {
        self.crypto.read().ok().and_then(|guard| guard.clone())
    }

    pub fn encrypt_json(&self, plaintext: &str) -> Result<String> {
        match self.crypto() {
            Some(crypto) => crypto.encrypt(plaintext),
            None => Ok(plaintext.to_string()),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn with<T>(&self, f: impl FnOnce(&mut Connection) -> Result<T>) -> Result<T> {
        let mut guard = self.lock()?;
        f(&mut guard)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::internal("数据库锁已中毒"))
    }

    fn migrate(&self) -> Result<()> {
        let migrations: &[(&str, &str)] = &[
            ("001_init", include_str!("../../migrations/001_init.sql")),
            (
                "002_hardening",
                include_str!("../../migrations/002_hardening.sql"),
            ),
            (
                "003_environment_identity",
                include_str!("../../migrations/003_environment_identity.sql"),
            ),
            (
                "004_environment_border_width",
                include_str!("../../migrations/004_environment_border_width.sql"),
            ),
            (
                "005_environment_templates",
                include_str!("../../migrations/005_environment_templates.sql"),
            ),
            (
                "006_environment_members",
                include_str!("../../migrations/006_environment_members.sql"),
            ),
            (
                "007_environment_launch_args",
                include_str!("../../migrations/007_environment_launch_args.sql"),
            ),
        ];
        let current: i64 =
            self.with(|c| Ok(c.query_row("PRAGMA user_version", [], |row| row.get(0))?))?;
        for (index, (name, sql)) in migrations.iter().enumerate() {
            let version = (index + 1) as i64;
            if current < version {
                self.with(|c| {
                    let tx = c.transaction()?;
                    tx.execute_batch(sql)?;
                    tx.pragma_update(None, "user_version", version)?;
                    tx.commit()?;
                    tracing::info!(migration = name, version, "database migration applied");
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    pub fn backup_to(&self, destination: &Path) -> Result<()> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }
        self.with(|conn| {
            let mut destination_conn = Connection::open(destination)?;
            let backup = rusqlite::backup::Backup::new(conn, &mut destination_conn)?;
            backup.run_to_completion(50, std::time::Duration::from_millis(50), None)?;
            Ok(())
        })
    }

    /// Replace the live database with the contents of a backup file. Callers
    /// must close browser processes first and reload derived state afterwards.
    pub fn restore_from_file(&self, source: &Path) -> Result<()> {
        self.with(|live| {
            let source_conn = Connection::open(source)?;
            let backup = rusqlite::backup::Backup::new(&source_conn, live)?;
            backup.run_to_completion(50, std::time::Duration::from_millis(50), None)?;
            Ok(())
        })
    }
}

pub fn bool_int(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}

pub fn int_bool(value: i64) -> bool {
    value != 0
}
