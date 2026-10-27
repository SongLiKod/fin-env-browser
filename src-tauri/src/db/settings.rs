use rusqlite::{params, Row};
use sha2::{Digest, Sha256};

use crate::{
    error::{AppError, Result},
    models::{now_iso, ApiKey, Settings, ThemeConfig, User},
};

use super::{bool_int, int_bool, Db};

const SETTINGS_KEY: &str = "app";
const CALLBACK_TOKEN_KEY: &str = "callback_token_enc";
const SYNC_PASSWORD_KEY: &str = "sync_password_enc";

// ---------------------------------------------------------------------------
// Application settings
// ---------------------------------------------------------------------------

impl Db {
    pub fn load_settings(&self) -> Result<Settings> {
        self.with(|conn| {
            let mut settings: Settings = match conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![SETTINGS_KEY],
                |row| row.get::<_, String>(0),
            ) {
                Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
                Err(rusqlite::Error::QueryReturnedNoRows) => Settings::default(),
                Err(e) => return Err(e.into()),
            };
            settings.callback_token_enc = read_secret(conn, CALLBACK_TOKEN_KEY)?;
            settings.sync_password_enc = read_secret(conn, SYNC_PASSWORD_KEY)?;
            settings.callback_token_set = settings
                .callback_token_enc
                .as_deref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            settings.sync_password_set = settings
                .sync_password_enc
                .as_deref()
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            Ok(settings)
        })
    }

    pub fn get_meta(&self, key: &str) -> Result<Option<String>> {
        self.with(|conn| {
            match conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![format!("meta:{key}")],
                |row| row.get::<_, String>(0),
            ) {
                Ok(value) => Ok(Some(value)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    pub fn set_meta(&self, key: &str, value: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1,?2,?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![format!("meta:{key}"), value, now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn save_settings(&self, settings: &Settings) -> Result<()> {
        let mut serializable = settings.clone();
        let callback = serializable.callback_token_enc.clone();
        let sync_password = serializable.sync_password_enc.clone();
        serializable.callback_token_enc = None;
        serializable.sync_password_enc = None;
        serializable.callback_token_set =
            callback.as_deref().map(|v| !v.is_empty()).unwrap_or(false);
        serializable.sync_password_set = sync_password
            .as_deref()
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        let raw = serde_json::to_string(&serializable)?;
        self.with(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "INSERT INTO settings (key, value, updated_at) VALUES (?1,?2,?3)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
                params![SETTINGS_KEY, raw, now_iso()],
            )?;
            if let Some(secret) = callback {
                write_secret(&tx, CALLBACK_TOKEN_KEY, &secret)?;
            }
            if let Some(secret) = sync_password {
                write_secret(&tx, SYNC_PASSWORD_KEY, &secret)?;
            }
            tx.commit()?;
            Ok(())
        })
    }
}

fn read_secret(conn: &rusqlite::Connection, key: &str) -> Result<Option<String>> {
    match conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    ) {
        Ok(v) if !v.is_empty() => Ok(Some(v)),
        Ok(_) => Ok(None),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

fn write_secret(conn: &rusqlite::Connection, key: &str, value: &str) -> Result<()> {
    if value.is_empty() {
        conn.execute("DELETE FROM settings WHERE key = ?1", params![key])?;
    } else {
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES (?1,?2,?3)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
            params![key, value, now_iso()],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Theme config
// ---------------------------------------------------------------------------

impl Db {
    pub fn get_theme(&self) -> Result<ThemeConfig> {
        self.with(|conn| {
            conn.query_row(
                "SELECT mode, primary_color, density, config_json, updated_at FROM theme_config WHERE id = 1",
                [],
                |row| {
                    let config: String = row.get(3)?;
                    Ok(ThemeConfig {
                        mode: row.get(0)?,
                        primary_color: row.get(1)?,
                        density: row.get(2)?,
                        config: serde_json::from_str(&config).unwrap_or_default(),
                        updated_at: row.get(4)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::internal("主题配置缺失"),
                other => other.into(),
            })
        })
    }

    pub fn save_theme(&self, theme: &ThemeConfig) -> Result<()> {
        let config = serde_json::to_string(&theme.config)?;
        self.with(|conn| {
            conn.execute(
                "INSERT INTO theme_config (id, mode, primary_color, density, config_json, updated_at)
                 VALUES (1,?1,?2,?3,?4,?5)
                 ON CONFLICT(id) DO UPDATE SET mode=excluded.mode, primary_color=excluded.primary_color,
                 density=excluded.density, config_json=excluded.config_json, updated_at=excluded.updated_at",
                params![
                    theme.mode,
                    theme.primary_color,
                    theme.density,
                    config,
                    theme.updated_at
                ],
            )?;
            Ok(())
        })
    }
}

// ---------------------------------------------------------------------------
// Users
// ---------------------------------------------------------------------------

const USER_COLUMNS: &str = "id, username, display_name, role, enabled, created_at, last_login_at";

fn row_to_user(row: &Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: row.get(0)?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        role: row.get(3)?,
        enabled: int_bool(row.get(4)?),
        created_at: row.get(5)?,
        last_login_at: row.get(6)?,
    })
}

impl Db {
    pub fn list_users(&self) -> Result<Vec<User>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {USER_COLUMNS} FROM users ORDER BY created_at"
            ))?;
            let rows = stmt.query_map([], row_to_user)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_user(&self, id: &str) -> Result<User> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {USER_COLUMNS} FROM users WHERE id = ?1"),
                params![id],
                row_to_user,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("用户不存在"),
                other => other.into(),
            })
        })
    }

    pub fn get_user_with_hash(&self, username: &str) -> Result<Option<(User, String)>> {
        self.with(|conn| {
            match conn.query_row(
                &format!("SELECT {USER_COLUMNS}, password_hash FROM users WHERE username = ?1"),
                params![username],
                |row| {
                    let user = row_to_user(row)?;
                    let hash: String = row.get(7)?;
                    Ok((user, hash))
                },
            ) {
                Ok(v) => Ok(Some(v)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    pub fn insert_user(&self, user: &User, password_hash: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO users (id, username, display_name, role, password_hash, enabled, created_at, last_login_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    user.id,
                    user.username,
                    user.display_name,
                    user.role,
                    password_hash,
                    bool_int(user.enabled),
                    user.created_at,
                    user.last_login_at
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    AppError::conflict("用户名已存在")
                }
                other => other.into(),
            })?;
            Ok(())
        })
    }

    pub fn update_user(&self, user: &User, password_hash: Option<&str>) -> Result<()> {
        self.with(|conn| {
            let changed = if let Some(hash) = password_hash {
                conn.execute(
                    "UPDATE users SET display_name=?2, role=?3, enabled=?4, password_hash=?5 WHERE id=?1",
                    params![user.id, user.display_name, user.role, bool_int(user.enabled), hash],
                )?
            } else {
                conn.execute(
                    "UPDATE users SET display_name=?2, role=?3, enabled=?4 WHERE id=?1",
                    params![user.id, user.display_name, user.role, bool_int(user.enabled)],
                )?
            };
            if changed == 0 {
                return Err(AppError::not_found("用户不存在"));
            }
            Ok(())
        })
    }

    pub fn touch_user_login(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE users SET last_login_at = ?2 WHERE id = ?1",
                params![id, now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn delete_user(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute("DELETE FROM users WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("用户不存在"));
            }
            Ok(())
        })
    }

    pub fn super_admin_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM users WHERE role = 'super_admin' AND enabled = 1",
                [],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }
}

// ---------------------------------------------------------------------------
// API keys
// ---------------------------------------------------------------------------

const API_KEY_COLUMNS: &str = "id, name, prefix, role, enabled, created_at, last_used_at";

fn row_to_api_key(row: &Row<'_>) -> rusqlite::Result<ApiKey> {
    Ok(ApiKey {
        id: row.get(0)?,
        name: row.get(1)?,
        prefix: row.get(2)?,
        role: row.get(3)?,
        enabled: int_bool(row.get(4)?),
        created_at: row.get(5)?,
        last_used_at: row.get(6)?,
    })
}

pub fn hash_api_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

impl Db {
    pub fn list_api_keys(&self) -> Result<Vec<ApiKey>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {API_KEY_COLUMNS} FROM api_keys ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map([], row_to_api_key)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn insert_api_key(&self, key: &ApiKey, secret_hash: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO api_keys (id, name, prefix, key_hash, role, enabled, created_at, last_used_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    key.id,
                    key.name,
                    key.prefix,
                    secret_hash,
                    key.role,
                    bool_int(key.enabled),
                    key.created_at,
                    key.last_used_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn authenticate_api_key(&self, hash: &str) -> Result<Option<ApiKey>> {
        self.with(|conn| {
            let result = match conn.query_row(
                &format!(
                    "SELECT {API_KEY_COLUMNS} FROM api_keys WHERE key_hash = ?1 AND enabled = 1"
                ),
                params![hash],
                row_to_api_key,
            ) {
                Ok(key) => Some(key),
                Err(rusqlite::Error::QueryReturnedNoRows) => None,
                Err(e) => return Err(e.into()),
            };
            if let Some(key) = &result {
                conn.execute(
                    "UPDATE api_keys SET last_used_at = ?2 WHERE id = ?1",
                    params![key.id, now_iso()],
                )?;
            }
            Ok(result)
        })
    }

    pub fn delete_api_key(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute("DELETE FROM api_keys WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("API 密钥不存在"));
            }
            Ok(())
        })
    }
}
