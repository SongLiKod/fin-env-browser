use rusqlite::{params, Row};

use crate::{
    crypto::CryptoManager,
    error::{AppError, Result},
    models::{
        new_id, now_iso, BrowserRuntime, CaptchaEvent, Extension, FingerprintReport,
        FingerprintSnapshot, HostsEntry, HostsEntryInput, MockRule, MockRuleInput,
    },
};

use super::{bool_int, int_bool, Db};

fn decode_json(
    stored: &str,
    crypto: Option<&CryptoManager>,
    column: usize,
) -> rusqlite::Result<serde_json::Value> {
    let plain = if stored.starts_with("v1:") {
        let Some(crypto) = crypto else {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "encrypted payload but crypto manager is not attached",
                )),
            ));
        };
        crypto.decrypt(stored).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                column,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.message().to_string(),
                )),
            )
        })?
    } else {
        stored.to_string()
    };
    serde_json::from_str(&plain).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })
}

// ---------------------------------------------------------------------------
// Fingerprint snapshots
// ---------------------------------------------------------------------------

impl Db {
    pub fn list_snapshots(&self, env_id: &str) -> Result<Vec<FingerprintSnapshot>> {
        let crypto = self.crypto();
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, env_id, name, fingerprint_json, create_time FROM fingerprint_snapshots
                 WHERE env_id = ?1 ORDER BY create_time DESC",
            )?;
            let rows = stmt.query_map(params![env_id], |row| {
                let fp = decode_json(&row.get::<_, String>(3)?, crypto.as_deref(), 3)?;
                Ok(FingerprintSnapshot {
                    id: row.get(0)?,
                    env_id: row.get(1)?,
                    name: row.get(2)?,
                    fingerprint: serde_json::from_value(fp)
                        .unwrap_or_else(|_| crate::fingerprint::template("win11")),
                    create_time: row.get(4)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_snapshot(&self, id: &str) -> Result<FingerprintSnapshot> {
        let crypto = self.crypto();
        self.with(|conn| {
            conn.query_row(
                "SELECT id, env_id, name, fingerprint_json, create_time FROM fingerprint_snapshots WHERE id = ?1",
                params![id],
                |row| {
                    let fp = decode_json(&row.get::<_, String>(3)?, crypto.as_deref(), 3)?;
                    Ok(FingerprintSnapshot {
                        id: row.get(0)?,
                        env_id: row.get(1)?,
                        name: row.get(2)?,
                        fingerprint: serde_json::from_value(fp)
                            .unwrap_or_else(|_| crate::fingerprint::template("win11")),
                        create_time: row.get(4)?,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("快照不存在"),
                other => other.into(),
            })
        })
    }

    pub fn insert_snapshot(&self, snapshot: &FingerprintSnapshot) -> Result<()> {
        let fp = self.encrypt_json(&serde_json::to_string(&snapshot.fingerprint)?)?;
        self.with(|conn| {
            conn.execute(
                "INSERT INTO fingerprint_snapshots (id, env_id, name, fingerprint_json, create_time)
                 VALUES (?1,?2,?3,?4,?5)",
                params![
                    snapshot.id,
                    snapshot.env_id,
                    snapshot.name,
                    fp,
                    snapshot.create_time
                ],
            )?;
            Ok(())
        })
    }

    pub fn delete_snapshot(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            tx.execute(
                "UPDATE environments SET fingerprint_snapshot_id = NULL WHERE fingerprint_snapshot_id = ?1",
                params![id],
            )?;
            let changed = tx.execute("DELETE FROM fingerprint_snapshots WHERE id = ?1", params![id])?;
            tx.commit()?;
            if changed == 0 {
                return Err(AppError::not_found("快照不存在"));
            }
            Ok(())
        })
    }

    pub fn snapshot_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM fingerprint_snapshots", [], |row| {
                    row.get(0)
                })?;
            Ok(count as u32)
        })
    }

    pub fn snapshot_count_for_env(&self, env_id: &str) -> Result<u32> {
        self.with(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM fingerprint_snapshots WHERE env_id = ?1",
                params![env_id],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }
}

// ---------------------------------------------------------------------------
// Fingerprint detection reports
// ---------------------------------------------------------------------------

impl Db {
    pub fn insert_fingerprint_report(&self, report: &FingerprintReport) -> Result<()> {
        let checks = self.encrypt_json(&serde_json::to_string(&report.checks)?)?;
        let observed = self.encrypt_json(&serde_json::to_string(&report.observed)?)?;
        let configured = self.encrypt_json(&serde_json::to_string(&report.configured)?)?;
        self.with(|conn| {
            conn.execute(
                "INSERT INTO fingerprint_reports (id, env_id, score, checks_json, observed_json,
                 configured_json, screenshot_path, external_url, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    report.id,
                    report.env_id,
                    report.score,
                    checks,
                    observed,
                    configured,
                    report.screenshot_path,
                    report.external_url,
                    report.created_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_fingerprint_reports(
        &self,
        env_id: &str,
        limit: u32,
    ) -> Result<Vec<FingerprintReport>> {
        let crypto = self.crypto();
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, env_id, score, checks_json, observed_json, configured_json,
                 screenshot_path, external_url, created_at FROM fingerprint_reports
                 WHERE env_id = ?1 ORDER BY created_at DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![env_id, limit.min(100) as i64], |row| {
                row_to_report(row, crypto.as_deref())
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }
}

fn row_to_report(
    row: &Row<'_>,
    crypto: Option<&CryptoManager>,
) -> rusqlite::Result<FingerprintReport> {
    Ok(FingerprintReport {
        id: row.get(0)?,
        env_id: row.get(1)?,
        score: row.get::<_, i64>(2)? as u32,
        checks: serde_json::from_value(decode_json(&row.get::<_, String>(3)?, crypto, 3)?)
            .unwrap_or_default(),
        observed: decode_json(&row.get::<_, String>(4)?, crypto, 4)?,
        configured: decode_json(&row.get::<_, String>(5)?, crypto, 5)?,
        screenshot_path: row.get(6)?,
        external_url: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// ---------------------------------------------------------------------------
// Hosts
// ---------------------------------------------------------------------------

impl Db {
    pub fn list_hosts(&self, env_id: &str) -> Result<Vec<HostsEntry>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, env_id, host, ip, enabled FROM hosts_entries WHERE env_id = ?1 ORDER BY host",
            )?;
            let rows = stmt.query_map(params![env_id], |row| {
                Ok(HostsEntry {
                    id: row.get(0)?,
                    env_id: row.get(1)?,
                    host: row.get(2)?,
                    ip: row.get(3)?,
                    enabled: int_bool(row.get(4)?),
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn replace_hosts(
        &self,
        env_id: &str,
        entries: &[HostsEntryInput],
    ) -> Result<Vec<HostsEntry>> {
        for entry in entries {
            if entry.host.trim().is_empty() {
                return Err(AppError::bad_request("Hosts 主机名不能为空"));
            }
            if entry.host.contains(char::is_whitespace) {
                return Err(AppError::bad_request("Hosts 主机名不能包含空白字符"));
            }
            if entry.ip.parse::<std::net::IpAddr>().is_err() {
                return Err(AppError::bad_request(format!(
                    "Hosts IP 地址不合法: {}",
                    entry.ip
                )));
            }
        }
        self.with(|conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM hosts_entries WHERE env_id = ?1", params![env_id])?;
            for entry in entries {
                tx.execute(
                    "INSERT INTO hosts_entries (id, env_id, host, ip, enabled) VALUES (?1,?2,?3,?4,?5)",
                    params![
                        new_id(),
                        env_id,
                        entry.host.trim(),
                        entry.ip.trim(),
                        bool_int(entry.enabled.unwrap_or(true))
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })?;
        self.list_hosts(env_id)
    }
}

// ---------------------------------------------------------------------------
// Mock rules
// ---------------------------------------------------------------------------

impl Db {
    pub fn list_mocks(&self, env_id: &str) -> Result<Vec<MockRule>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, env_id, pattern, method, status_code, headers_json, body, delay_ms, enabled
                 FROM mock_rules WHERE env_id = ?1 ORDER BY rowid",
            )?;
            let rows = stmt.query_map(params![env_id], row_to_mock)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn replace_mocks(&self, env_id: &str, rules: &[MockRuleInput]) -> Result<Vec<MockRule>> {
        for rule in rules {
            if rule.pattern.trim().is_empty() {
                return Err(AppError::bad_request("Mock 匹配规则不能为空"));
            }
            if rule.status_code < 100 || rule.status_code > 599 {
                return Err(AppError::bad_request("Mock 状态码必须在 100 - 599 之间"));
            }
            if rule.delay_ms > 60_000 {
                return Err(AppError::bad_request("Mock 延迟不能超过 60000ms"));
            }
        }
        self.with(|conn| {
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM mock_rules WHERE env_id = ?1", params![env_id])?;
            for rule in rules {
                tx.execute(
                    "INSERT INTO mock_rules (id, env_id, pattern, method, status_code, headers_json,
                     body, delay_ms, enabled) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![
                        new_id(),
                        env_id,
                        rule.pattern.trim(),
                        rule.method.to_uppercase(),
                        rule.status_code,
                        serde_json::to_string(&rule.headers)?,
                        rule.body,
                        rule.delay_ms,
                        bool_int(rule.enabled.unwrap_or(true))
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })?;
        self.list_mocks(env_id)
    }
}

fn row_to_mock(row: &Row<'_>) -> rusqlite::Result<MockRule> {
    Ok(MockRule {
        id: row.get(0)?,
        env_id: row.get(1)?,
        pattern: row.get(2)?,
        method: row.get(3)?,
        status_code: row.get::<_, i64>(4)? as u16,
        headers: serde_json::from_str(&row.get::<_, String>(5)?).unwrap_or_default(),
        body: row.get(6)?,
        delay_ms: row.get::<_, i64>(7)? as u32,
        enabled: int_bool(row.get(8)?),
    })
}

// ---------------------------------------------------------------------------
// Captcha events
// ---------------------------------------------------------------------------

impl Db {
    pub fn insert_captcha(
        &self,
        env_id: &str,
        url: &str,
        source: &str,
        hint: &str,
        screenshot: Option<&str>,
    ) -> Result<CaptchaEvent> {
        let event = CaptchaEvent {
            id: new_id(),
            env_id: env_id.to_string(),
            env_name: String::new(),
            url: url.to_string(),
            source: source.to_string(),
            hint: hint.to_string(),
            screenshot_path: screenshot.map(ToString::to_string),
            handled: false,
            created_at: now_iso(),
        };
        self.with(|conn| {
            conn.execute(
                "INSERT INTO captcha_events (id, env_id, url, source, hint, screenshot_path, handled, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6,0,?7)",
                params![
                    event.id,
                    event.env_id,
                    event.url,
                    event.source,
                    event.hint,
                    event.screenshot_path,
                    event.created_at
                ],
            )?;
            Ok(())
        })?;
        self.list_captcha(Some(env_id), 1)?
            .into_iter()
            .next()
            .ok_or_else(|| AppError::internal("验证码事件写入失败"))
    }

    pub fn list_captcha(&self, env_id: Option<&str>, limit: u32) -> Result<Vec<CaptchaEvent>> {
        self.with(|conn| {
            let sql = if env_id.is_some() {
                "SELECT c.id, c.env_id, e.name, c.url, c.source, c.hint, c.screenshot_path, c.handled, c.created_at
                 FROM captcha_events c JOIN environments e ON e.id = c.env_id
                 WHERE c.env_id = ?1 ORDER BY c.created_at DESC LIMIT ?2"
            } else {
                "SELECT c.id, c.env_id, e.name, c.url, c.source, c.hint, c.screenshot_path, c.handled, c.created_at
                 FROM captcha_events c JOIN environments e ON e.id = c.env_id
                 ORDER BY c.created_at DESC LIMIT ?1"
            };
            let mut stmt = conn.prepare(sql)?;
            let map = |row: &Row<'_>| {
                Ok(CaptchaEvent {
                    id: row.get(0)?,
                    env_id: row.get(1)?,
                    env_name: row.get(2)?,
                    url: row.get(3)?,
                    source: row.get(4)?,
                    hint: row.get(5)?,
                    screenshot_path: row.get(6)?,
                    handled: int_bool(row.get(7)?),
                    created_at: row.get(8)?,
                })
            };
            let rows = if let Some(env_id) = env_id {
                stmt.query_map(params![env_id, limit.min(500) as i64], map)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            } else {
                stmt.query_map(params![limit.min(500) as i64], map)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
            };
            Ok(rows)
        })
    }

    pub fn mark_captcha_handled(&self, id: &str, handled: bool) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE captcha_events SET handled = ?2 WHERE id = ?1",
                params![id, bool_int(handled)],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("验证码事件不存在"));
            }
            Ok(())
        })
    }
}

// ---------------------------------------------------------------------------
// Extensions
// ---------------------------------------------------------------------------

impl Db {
    pub fn list_extensions(&self) -> Result<Vec<Extension>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, path, enabled, description, created_at,
                        (SELECT COUNT(*) FROM env_extensions ex WHERE ex.extension_id = extensions.id)
                 FROM extensions ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(Extension {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    enabled: int_bool(row.get(3)?),
                    description: row.get(4)?,
                    created_at: row.get(5)?,
                    assigned_count: row.get::<_, i64>(6)? as u32,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_extension(&self, id: &str) -> Result<Extension> {
        self.with(|conn| {
            conn.query_row(
                "SELECT id, name, path, enabled, description, created_at,
                        (SELECT COUNT(*) FROM env_extensions ex WHERE ex.extension_id = extensions.id)
                 FROM extensions WHERE id = ?1",
                params![id],
                |row| {
                    Ok(Extension {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        path: row.get(2)?,
                        enabled: int_bool(row.get(3)?),
                        description: row.get(4)?,
                        created_at: row.get(5)?,
                        assigned_count: row.get::<_, i64>(6)? as u32,
                    })
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("扩展不存在"),
                other => other.into(),
            })
        })
    }

    pub fn insert_extension(&self, ext: &Extension) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO extensions (id, name, path, enabled, description, created_at)
                 VALUES (?1,?2,?3,?4,?5,?6)",
                params![
                    ext.id,
                    ext.name,
                    ext.path,
                    bool_int(ext.enabled),
                    ext.description,
                    ext.created_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_extension(&self, ext: &Extension) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE extensions SET name = ?2, path = ?3, enabled = ?4, description = ?5
                 WHERE id = ?1",
                params![
                    ext.id,
                    ext.name,
                    ext.path,
                    bool_int(ext.enabled),
                    ext.description
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("扩展不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_extension(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute("DELETE FROM extensions WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("扩展不存在"));
            }
            Ok(())
        })
    }

    pub fn assign_extension(&self, extension_id: &str, env_ids: &[String]) -> Result<()> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            for env_id in env_ids {
                tx.execute(
                    "INSERT OR IGNORE INTO env_extensions (env_id, extension_id) VALUES (?1,?2)",
                    params![env_id, extension_id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn unassign_extension(&self, extension_id: &str, env_ids: &[String]) -> Result<()> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            for env_id in env_ids {
                tx.execute(
                    "DELETE FROM env_extensions WHERE env_id = ?1 AND extension_id = ?2",
                    params![env_id, extension_id],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn extensions_for_env(&self, env_id: &str) -> Result<Vec<Extension>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT x.id, x.name, x.path, x.enabled, x.description, x.created_at
                 FROM extensions x JOIN env_extensions ex ON ex.extension_id = x.id
                 WHERE ex.env_id = ?1 ORDER BY x.name",
            )?;
            let rows = stmt.query_map(params![env_id], |row| {
                Ok(Extension {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    path: row.get(2)?,
                    enabled: int_bool(row.get(3)?),
                    description: row.get(4)?,
                    created_at: row.get(5)?,
                    assigned_count: 0,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }
}

// ---------------------------------------------------------------------------
// Encrypted cookie cache
// ---------------------------------------------------------------------------

impl Db {
    pub fn save_cookie_cache(&self, env_id: &str, encrypted: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO env_cookies (env_id, cookies_enc, updated_at) VALUES (?1,?2,?3)
                 ON CONFLICT(env_id) DO UPDATE SET cookies_enc = excluded.cookies_enc, updated_at = excluded.updated_at",
                params![env_id, encrypted, now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn load_cookie_cache(&self, env_id: &str) -> Result<Option<String>> {
        self.with(|conn| {
            match conn.query_row(
                "SELECT cookies_enc FROM env_cookies WHERE env_id = ?1",
                params![env_id],
                |row| row.get::<_, String>(0),
            ) {
                Ok(v) => Ok(Some(v)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }
}

// ---------------------------------------------------------------------------
// Browser sessions (for reconnecting after a panel restart)
// ---------------------------------------------------------------------------

impl Db {
    pub fn save_browser_session(&self, runtime: &BrowserRuntime) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO browser_sessions (env_id, pid, debug_port, ws_endpoint, started_at, last_active_at)
                 VALUES (?1,?2,?3,?4,?5,?6)
                 ON CONFLICT(env_id) DO UPDATE SET pid=excluded.pid, debug_port=excluded.debug_port,
                 ws_endpoint=excluded.ws_endpoint, last_active_at=excluded.last_active_at",
                params![
                    runtime.env_id,
                    runtime.pid,
                    runtime.debug_port,
                    runtime.ws_endpoint,
                    runtime.started_at,
                    runtime.last_active_at
                ],
            )?;
            Ok(())
        })
    }

    pub fn list_browser_sessions(&self) -> Result<Vec<BrowserRuntime>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT env_id, pid, debug_port, ws_endpoint, started_at, last_active_at FROM browser_sessions",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok(BrowserRuntime {
                    env_id: row.get(0)?,
                    pid: row.get::<_, i64>(1)? as u32,
                    debug_port: row.get::<_, i64>(2)? as u16,
                    ws_endpoint: row.get(3)?,
                    status: "running".into(),
                    started_at: row.get(4)?,
                    last_active_at: row.get(5)?,
                    cpu_percent: 0.0,
                    memory_mb: 0,
                    current_url: String::new(),
                    recording: false,
                    har_recording: false,
                    warnings: vec![],
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn remove_browser_session(&self, env_id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "DELETE FROM browser_sessions WHERE env_id = ?1",
                params![env_id],
            )?;
            Ok(())
        })
    }

    pub fn clear_browser_sessions(&self) -> Result<()> {
        self.with(|conn| {
            conn.execute("DELETE FROM browser_sessions", [])?;
            Ok(())
        })
    }
}
