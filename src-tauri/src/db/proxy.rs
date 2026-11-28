use std::io;

use rusqlite::{params, params_from_iter, types::Value, Row};

use crate::{
    crypto::CryptoManager,
    error::{AppError, Result},
    models::{
        Proxy, ProxyCheckResult, ProxyDeleteResult, ProxyFilter, ProxyPage, ProxyStats,
        ProxySubscription,
    },
};

use super::{bool_int, int_bool, Db};

const PROXY_COLUMNS: &str = "id, name, protocol, host, port, username, password_enc, enabled, \
    last_check_at, latency_ms, exit_ip, exit_country, exit_timezone, group_name, tags, note, \
    source, subscription_id, weight, last_check_ok, last_check_message, last_check_target, \
    last_check_city, fail_count, disabled_reason, endpoint_key, created_at, updated_at";

const SUBSCRIPTION_COLUMNS: &str = "s.id, s.name, s.url_enc, s.headers_enc, s.default_protocol, \
    s.group_name, s.tags, s.enabled, s.auto_refresh, s.refresh_interval_min, s.last_refresh_at, \
    s.last_refresh_status, s.last_refresh_message, s.created_at, s.updated_at";

fn row_to_proxy(row: &Row<'_>) -> rusqlite::Result<Proxy> {
    let password_enc: String = row.get("password_enc")?;
    let tags: String = row.get("tags")?;
    Ok(Proxy {
        id: row.get("id")?,
        name: row.get("name")?,
        protocol: row.get("protocol")?,
        host: row.get("host")?,
        port: row.get::<_, i64>("port")? as u16,
        username: row.get("username")?,
        has_password: !password_enc.is_empty(),
        password_enc,
        enabled: int_bool(row.get("enabled")?),
        last_check_at: row.get("last_check_at")?,
        latency_ms: row.get::<_, Option<i64>>("latency_ms")?.map(|v| v as u64),
        exit_ip: row.get("exit_ip")?,
        exit_country: row.get("exit_country")?,
        exit_timezone: row.get("exit_timezone")?,
        group_name: row.get("group_name")?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        note: row.get("note")?,
        source: row.get("source")?,
        subscription_id: row.get("subscription_id")?,
        weight: row.get::<_, i64>("weight")? as u32,
        last_check_ok: row
            .get::<_, Option<i64>>("last_check_ok")?
            .map(|value| value != 0),
        last_check_message: row.get("last_check_message")?,
        last_check_target: row.get("last_check_target").unwrap_or_default(),
        last_check_city: row.get("last_check_city")?,
        fail_count: row.get::<_, i64>("fail_count")? as u32,
        disabled_reason: row.get("disabled_reason")?,
        endpoint_key: row.get("endpoint_key")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn decrypt_field(
    crypto: Option<&CryptoManager>,
    value: &str,
    column: usize,
) -> rusqlite::Result<String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    let Some(crypto) = crypto else {
        return Err(rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(
                io::ErrorKind::InvalidData,
                "subscription encrypted but crypto manager is not attached",
            )),
        ));
    };
    crypto.decrypt(value).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(io::Error::new(io::ErrorKind::InvalidData, e.message())),
        )
    })
}

fn row_to_subscription(
    row: &Row<'_>,
    crypto: Option<&CryptoManager>,
) -> rusqlite::Result<ProxySubscription> {
    let url_enc: String = row.get("url_enc")?;
    let headers_enc: String = row.get("headers_enc")?;
    let tags: String = row.get("tags")?;
    let url = decrypt_field(crypto, &url_enc, 2)?;
    let headers = decrypt_field(crypto, &headers_enc, 3)?;
    Ok(ProxySubscription {
        id: row.get("id")?,
        name: row.get("name")?,
        url,
        url_enc,
        headers: (!headers.is_empty()).then_some(headers),
        headers_enc,
        default_protocol: row.get("default_protocol")?,
        group_name: row.get("group_name")?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        enabled: int_bool(row.get("enabled")?),
        auto_refresh: int_bool(row.get("auto_refresh")?),
        refresh_interval_min: row.get::<_, i64>("refresh_interval_min")? as u32,
        last_refresh_at: row.get("last_refresh_at")?,
        last_refresh_status: row.get("last_refresh_status")?,
        last_refresh_message: row.get("last_refresh_message")?,
        proxy_count: row.get::<_, i64>("proxy_count")? as u32,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn opt_bool_int(value: Option<bool>) -> Option<i64> {
    value.map(|value| if value { 1 } else { 0 })
}

fn placeholders(count: usize) -> String {
    (1..=count)
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",")
}

fn filter_clauses(filter: &ProxyFilter) -> (Vec<String>, Vec<Value>) {
    let mut clauses = Vec::new();
    let mut values = Vec::new();
    if let Some(search) = filter
        .search
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        let like = format!("%{search}%");
        clauses.push(
            "(name LIKE ? OR host LIKE ? OR username LIKE ? OR exit_ip LIKE ? OR note LIKE ?)"
                .into(),
        );
        for _ in 0..5 {
            values.push(Value::Text(like.clone()));
        }
    }
    if let Some(group) = filter
        .group_name
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        clauses.push("group_name = ?".into());
        values.push(Value::Text(group.to_string()));
    }
    if let Some(tag) = filter
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        clauses.push("tags LIKE ?".into());
        values.push(Value::Text(format!("%\"{tag}\"%")));
    }
    if let Some(source) = filter
        .source
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        clauses.push("source = ?".into());
        values.push(Value::Text(source.to_string()));
    }
    if let Some(subscription_id) = filter
        .subscription_id
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        clauses.push("subscription_id = ?".into());
        values.push(Value::Text(subscription_id.to_string()));
    }
    if let Some(enabled) = filter.enabled {
        clauses.push("enabled = ?".into());
        values.push(Value::Integer(bool_int(enabled)));
    }
    match filter.status.as_deref() {
        Some("ok") => clauses.push("enabled = 1 AND last_check_ok = 1".into()),
        Some("fail") => clauses.push("enabled = 1 AND last_check_ok = 0".into()),
        Some("untested") => clauses.push("enabled = 1 AND last_check_ok IS NULL".into()),
        Some("disabled") => clauses.push("enabled = 0".into()),
        Some("enabled") => clauses.push("enabled = 1".into()),
        _ => {}
    }
    if !filter.statuses.is_empty() {
        let groups = filter
            .statuses
            .iter()
            .filter_map(|status| match status.trim() {
                "ok" => Some("(enabled = 1 AND last_check_ok = 1)"),
                "fail" => Some("(enabled = 1 AND last_check_ok = 0)"),
                "untested" => Some("(enabled = 1 AND last_check_ok IS NULL)"),
                "disabled" => Some("(enabled = 0)"),
                "enabled" => Some("(enabled = 1)"),
                _ => None,
            })
            .collect::<Vec<_>>();
        if !groups.is_empty() {
            clauses.push(format!("({})", groups.join(" OR ")));
        }
    }
    if let Some(reason) = filter
        .disabled_reason
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    {
        clauses.push("disabled_reason = ?".into());
        values.push(Value::Text(reason.to_string()));
    }
    (clauses, values)
}

fn where_sql(clauses: &[String]) -> String {
    if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    }
}

impl Db {
    pub fn list_proxies(&self) -> Result<Vec<Proxy>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {PROXY_COLUMNS} FROM proxies ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map([], row_to_proxy)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn query_proxies(&self, filter: &ProxyFilter) -> Result<ProxyPage> {
        self.with(|conn| {
            let (clauses, values) = filter_clauses(filter);
            let where_clause = where_sql(&clauses);
            let total: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM proxies{where_clause}"),
                params_from_iter(values.iter()),
                |row| row.get(0),
            )?;
            let limit = filter.limit.unwrap_or(100).clamp(1, 500);
            let offset = filter.offset.unwrap_or(0);
            let mut stmt = conn.prepare(&format!(
                "SELECT {PROXY_COLUMNS} FROM proxies{where_clause} ORDER BY created_at DESC LIMIT ? OFFSET ?"
            ))?;
            let mut query_values = values;
            query_values.push(Value::Integer(limit as i64));
            query_values.push(Value::Integer(offset as i64));
            let rows = stmt.query_map(params_from_iter(query_values.iter()), row_to_proxy)?;
            Ok(ProxyPage {
                items: rows.collect::<rusqlite::Result<Vec<_>>>()?,
                total: total as u32,
            })
        })
    }

    pub fn proxy_stats(&self) -> Result<ProxyStats> {
        self.with(|conn| {
            conn.query_row(
                "SELECT COUNT(*),
                        COALESCE(SUM(CASE WHEN enabled = 1 THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN enabled = 1 AND last_check_ok = 1 THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN enabled = 1 AND last_check_ok = 0 THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN enabled = 1 AND last_check_ok IS NULL THEN 1 ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN enabled = 0 THEN 1 ELSE 0 END), 0),
                        (SELECT COUNT(*) FROM proxy_subscriptions)
                 FROM proxies",
                [],
                |row| {
                    Ok(ProxyStats {
                        total: row.get::<_, i64>(0)? as u32,
                        enabled: row.get::<_, i64>(1)? as u32,
                        available: row.get::<_, i64>(2)? as u32,
                        failed: row.get::<_, i64>(3)? as u32,
                        untested: row.get::<_, i64>(4)? as u32,
                        disabled: row.get::<_, i64>(5)? as u32,
                        subscriptions: row.get::<_, i64>(6)? as u32,
                    })
                },
            )
            .map_err(Into::into)
        })
    }

    pub fn get_proxy(&self, id: &str) -> Result<Proxy> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {PROXY_COLUMNS} FROM proxies WHERE id = ?1"),
                params![id],
                row_to_proxy,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("代理不存在"),
                other => other.into(),
            })
        })
    }

    pub fn find_proxies_by_endpoint_keys(&self, keys: &[String]) -> Result<Vec<Proxy>> {
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        self.with(|conn| {
            let mut found = Vec::new();
            for chunk in keys.chunks(400) {
                let sql = format!(
                    "SELECT {PROXY_COLUMNS} FROM proxies WHERE endpoint_key IN ({})",
                    placeholders(chunk.len())
                );
                let mut stmt = conn.prepare(&sql)?;
                let rows = stmt.query_map(params_from_iter(chunk.iter()), row_to_proxy)?;
                found.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
            }
            Ok(found)
        })
    }

    pub fn insert_proxy(&self, proxy: &Proxy) -> Result<()> {
        self.with(|conn| {
            insert_proxy_row(conn, proxy)?;
            Ok(())
        })
    }

    pub fn update_proxy(&self, proxy: &Proxy) -> Result<()> {
        self.with(|conn| {
            let changed = update_proxy_row(conn, proxy)?;
            if changed == 0 {
                return Err(AppError::not_found("代理不存在"));
            }
            Ok(())
        })
    }

    pub fn update_proxies_bulk(&self, proxies: &[Proxy]) -> Result<u32> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let mut updated = 0u32;
            for proxy in proxies {
                updated += update_proxy_row(&tx, proxy)? as u32;
            }
            tx.commit()?;
            Ok(updated)
        })
    }

    pub fn apply_proxy_import(&self, inserts: &[Proxy], updates: &[Proxy]) -> Result<(u32, u32)> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let mut inserted = 0u32;
            let mut updated = 0u32;
            for proxy in inserts {
                inserted += insert_proxy_row(&tx, proxy)? as u32;
            }
            for proxy in updates {
                updated += update_proxy_row(&tx, proxy)? as u32;
            }
            tx.commit()?;
            Ok((inserted, updated))
        })
    }

    pub fn delete_proxy(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute("DELETE FROM proxies WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("代理不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_proxies(&self, ids: &[String]) -> Result<ProxyDeleteResult> {
        if ids.is_empty() {
            return Ok(ProxyDeleteResult::default());
        }
        self.with(|conn| {
            let in_sql = placeholders(ids.len());
            let affected_envs: i64 = conn.query_row(
                &format!("SELECT COUNT(*) FROM environments WHERE proxy_id IN ({in_sql})"),
                params_from_iter(ids.iter()),
                |row| row.get(0),
            )?;
            let deleted = conn.execute(
                &format!("DELETE FROM proxies WHERE id IN ({in_sql})"),
                params_from_iter(ids.iter()),
            )?;
            Ok(ProxyDeleteResult {
                deleted: deleted as u32,
                affected_envs: affected_envs as u32,
            })
        })
    }

    pub fn delete_disabled_proxies(&self, filter: &ProxyFilter) -> Result<ProxyDeleteResult> {
        fn known_status(value: &str) -> bool {
            matches!(
                value.trim(),
                "ok" | "fail" | "untested" | "disabled" | "enabled"
            )
        }
        let mut filter = filter.clone();
        let requested = !filter.statuses.is_empty();
        filter.statuses.retain(|value| known_status(value));
        if requested && filter.statuses.is_empty() {
            return Err(AppError::bad_request("清理状态不合法"));
        }
        if filter.statuses.is_empty() {
            match filter.status.as_deref() {
                Some(status) if known_status(status) => {
                    filter.status = Some(status.trim().to_string());
                }
                Some(_) => return Err(AppError::bad_request("清理状态不合法")),
                None => filter.status = Some("disabled".into()),
            }
        }
        self.with(|conn| {
            let (clauses, values) = filter_clauses(&filter);
            let where_clause = where_sql(&clauses);
            let affected_envs: i64 = conn.query_row(
                &format!(
                    "SELECT COUNT(*) FROM environments WHERE proxy_id IN (SELECT id FROM proxies{where_clause})"
                ),
                params_from_iter(values.iter()),
                |row| row.get(0),
            )?;
            let count = conn.execute(
                &format!("DELETE FROM proxies{where_clause}"),
                params_from_iter(values.iter()),
            )?;
            Ok(ProxyDeleteResult {
                deleted: count as u32,
                affected_envs: affected_envs as u32,
            })
        })
    }

    pub fn set_proxies_enabled(
        &self,
        ids: &[String],
        enabled: bool,
        disabled_reason: &str,
    ) -> Result<u32> {
        if ids.is_empty() {
            return Ok(0);
        }
        self.with(|conn| {
            let tx = conn.transaction()?;
            let mut updated = 0u32;
            for id in ids {
                updated += tx.execute(
                    "UPDATE proxies SET enabled=?2, disabled_reason=?3, updated_at=?4 WHERE id=?1",
                    params![
                        id,
                        bool_int(enabled),
                        if enabled { "" } else { disabled_reason },
                        crate::models::now_iso()
                    ],
                )? as u32;
            }
            tx.commit()?;
            Ok(updated)
        })
    }

    pub fn save_proxy_check(&self, result: &ProxyCheckResult) -> Result<()> {
        self.save_proxy_checks(std::slice::from_ref(result))
    }

    pub fn save_proxy_checks(&self, results: &[ProxyCheckResult]) -> Result<()> {
        if results.is_empty() {
            return Ok(());
        }
        self.with(|conn| {
            let tx = conn.transaction()?;
            for result in results {
                let ok = result.ok as i64;
                tx.execute(
                    "UPDATE proxies SET
                        last_check_at=?2,
                        latency_ms=?3,
                        exit_ip=CASE WHEN ?8=1 THEN ?4 ELSE exit_ip END,
                        exit_country=CASE WHEN ?8=1 THEN ?5 ELSE exit_country END,
                        exit_timezone=CASE WHEN ?8=1 THEN ?6 ELSE exit_timezone END,
                        last_check_city=CASE WHEN ?8=1 THEN ?7 ELSE last_check_city END,
                        last_check_ok=?8,
                        last_check_message=?9,
                        last_check_target=?10,
                        fail_count=CASE WHEN ?8=1 THEN 0 ELSE fail_count + 1 END,
                        updated_at=?2
                     WHERE id=?1",
                    params![
                        result.proxy_id,
                        result.checked_at,
                        result.latency_ms,
                        result.exit_ip,
                        result.country,
                        result.timezone,
                        result.city,
                        ok,
                        result.message,
                        result.target,
                    ],
                )?;
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn proxy_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM proxies", [], |row| row.get(0))?;
            Ok(count as u32)
        })
    }

    pub fn proxy_bound_env_count(&self, proxy_id: &str) -> Result<u32> {
        self.with(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM environments WHERE proxy_id = ?1",
                params![proxy_id],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }

    pub fn list_subscriptions(&self) -> Result<Vec<ProxySubscription>> {
        self.with(|conn| {
            let crypto = self.crypto();
            let mut stmt = conn.prepare(&format!(
                "SELECT {SUBSCRIPTION_COLUMNS},
                        (SELECT COUNT(*) FROM proxies p WHERE p.subscription_id = s.id) AS proxy_count
                 FROM proxy_subscriptions s ORDER BY s.created_at DESC"
            ))?;
            let rows = stmt.query_map([], |row| row_to_subscription(row, crypto.as_deref()))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_subscription(&self, id: &str) -> Result<ProxySubscription> {
        self.with(|conn| {
            let crypto = self.crypto();
            conn.query_row(
                &format!(
                    "SELECT {SUBSCRIPTION_COLUMNS},
                            (SELECT COUNT(*) FROM proxies p WHERE p.subscription_id = s.id) AS proxy_count
                     FROM proxy_subscriptions s WHERE s.id = ?1"
                ),
                params![id],
                |row| row_to_subscription(row, crypto.as_deref()),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("订阅不存在"),
                other => other.into(),
            })
        })
    }

    pub fn insert_subscription(&self, subscription: &ProxySubscription) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO proxy_subscriptions
                 (id, name, url_enc, headers_enc, default_protocol, group_name, tags, enabled,
                  auto_refresh, refresh_interval_min, last_refresh_at, last_refresh_status,
                  last_refresh_message, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
                params![
                    subscription.id,
                    subscription.name,
                    subscription.url_enc,
                    subscription.headers_enc,
                    subscription.default_protocol,
                    subscription.group_name,
                    serde_json::to_string(&subscription.tags).unwrap_or_else(|_| "[]".into()),
                    bool_int(subscription.enabled),
                    bool_int(subscription.auto_refresh),
                    subscription.refresh_interval_min,
                    subscription.last_refresh_at,
                    subscription.last_refresh_status,
                    subscription.last_refresh_message,
                    subscription.created_at,
                    subscription.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_subscription(&self, subscription: &ProxySubscription) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE proxy_subscriptions SET
                    name=?2, url_enc=?3, headers_enc=?4, default_protocol=?5, group_name=?6,
                    tags=?7, enabled=?8, auto_refresh=?9, refresh_interval_min=?10,
                    last_refresh_at=?11, last_refresh_status=?12, last_refresh_message=?13,
                    updated_at=?14
                 WHERE id=?1",
                params![
                    subscription.id,
                    subscription.name,
                    subscription.url_enc,
                    subscription.headers_enc,
                    subscription.default_protocol,
                    subscription.group_name,
                    serde_json::to_string(&subscription.tags).unwrap_or_else(|_| "[]".into()),
                    bool_int(subscription.enabled),
                    bool_int(subscription.auto_refresh),
                    subscription.refresh_interval_min,
                    subscription.last_refresh_at,
                    subscription.last_refresh_status,
                    subscription.last_refresh_message,
                    subscription.updated_at,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("订阅不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_subscription_row(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed =
                conn.execute("DELETE FROM proxy_subscriptions WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("订阅不存在"));
            }
            Ok(())
        })
    }

    pub fn detach_subscription_proxies(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE proxies SET subscription_id=NULL, source='manual',
                    disabled_reason=CASE
                        WHEN enabled=1 THEN ''
                        WHEN disabled_reason='subscription_missing' THEN 'manual'
                        ELSE disabled_reason
                    END,
                    updated_at=?2
                 WHERE subscription_id=?1",
                params![id, crate::models::now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn delete_proxies_by_subscription(&self, id: &str) -> Result<ProxyDeleteResult> {
        self.with(|conn| {
            let affected_envs: i64 = conn.query_row(
                "SELECT COUNT(*) FROM environments WHERE proxy_id IN
                 (SELECT id FROM proxies WHERE subscription_id=?1)",
                params![id],
                |row| row.get(0),
            )?;
            let deleted =
                conn.execute("DELETE FROM proxies WHERE subscription_id=?1", params![id])?;
            Ok(ProxyDeleteResult {
                deleted: deleted as u32,
                affected_envs: affected_envs as u32,
            })
        })
    }

    pub fn list_subscription_proxies(&self, id: &str) -> Result<Vec<Proxy>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {PROXY_COLUMNS} FROM proxies WHERE subscription_id=?1 ORDER BY created_at ASC"
            ))?;
            let rows = stmt.query_map(params![id], row_to_proxy)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn list_auto_subscriptions(&self) -> Result<Vec<ProxySubscription>> {
        self.with(|conn| {
            let crypto = self.crypto();
            let mut stmt = conn.prepare(&format!(
                "SELECT {SUBSCRIPTION_COLUMNS},
                        (SELECT COUNT(*) FROM proxies p WHERE p.subscription_id = s.id) AS proxy_count
                 FROM proxy_subscriptions s
                 WHERE s.enabled=1 AND s.auto_refresh=1 ORDER BY s.last_refresh_at ASC"
            ))?;
            let rows = stmt.query_map([], |row| row_to_subscription(row, crypto.as_deref()))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn set_subscription_refresh_result(
        &self,
        id: &str,
        status: &str,
        message: &str,
        at: &str,
    ) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE proxy_subscriptions SET last_refresh_at=?2, last_refresh_status=?3,
                 last_refresh_message=?4, updated_at=?2 WHERE id=?1",
                params![id, at, status, message],
            )?;
            Ok(())
        })
    }
}

fn insert_proxy_row(conn: &rusqlite::Connection, proxy: &Proxy) -> rusqlite::Result<usize> {
    conn.execute(
        "INSERT INTO proxies
         (id, name, protocol, host, port, username, password_enc, enabled, last_check_at,
          latency_ms, exit_ip, exit_country, exit_timezone, group_name, tags, note, source,
          subscription_id, weight, last_check_ok, last_check_message, last_check_target,
          last_check_city, fail_count, disabled_reason, endpoint_key, created_at, updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,
                 ?20,?21,?22,?23,?24,?25,?26,?27,?28)",
        params![
            proxy.id,
            proxy.name,
            proxy.protocol,
            proxy.host,
            proxy.port,
            proxy.username,
            proxy.password_enc,
            bool_int(proxy.enabled),
            proxy.last_check_at,
            proxy.latency_ms,
            proxy.exit_ip,
            proxy.exit_country,
            proxy.exit_timezone,
            proxy.group_name,
            serde_json::to_string(&proxy.tags).unwrap_or_else(|_| "[]".into()),
            proxy.note,
            proxy.source,
            proxy.subscription_id,
            proxy.weight,
            opt_bool_int(proxy.last_check_ok),
            proxy.last_check_message,
            proxy.last_check_target,
            proxy.last_check_city,
            proxy.fail_count,
            proxy.disabled_reason,
            proxy.endpoint_key,
            proxy.created_at,
            proxy.updated_at,
        ],
    )
}

fn update_proxy_row(conn: &rusqlite::Connection, proxy: &Proxy) -> rusqlite::Result<usize> {
    conn.execute(
        "UPDATE proxies SET
            name=?2, protocol=?3, host=?4, port=?5, username=?6, password_enc=?7, enabled=?8,
            last_check_at=?9, latency_ms=?10, exit_ip=?11, exit_country=?12, exit_timezone=?13,
            group_name=?14, tags=?15, note=?16, source=?17, subscription_id=?18, weight=?19,
            last_check_ok=?20, last_check_message=?21, last_check_target=?22, last_check_city=?23,
            fail_count=?24, disabled_reason=?25, endpoint_key=?26, updated_at=?27
         WHERE id=?1",
        params![
            proxy.id,
            proxy.name,
            proxy.protocol,
            proxy.host,
            proxy.port,
            proxy.username,
            proxy.password_enc,
            bool_int(proxy.enabled),
            proxy.last_check_at,
            proxy.latency_ms,
            proxy.exit_ip,
            proxy.exit_country,
            proxy.exit_timezone,
            proxy.group_name,
            serde_json::to_string(&proxy.tags).unwrap_or_else(|_| "[]".into()),
            proxy.note,
            proxy.source,
            proxy.subscription_id,
            proxy.weight,
            opt_bool_int(proxy.last_check_ok),
            proxy.last_check_message,
            proxy.last_check_target,
            proxy.last_check_city,
            proxy.fail_count,
            proxy.disabled_reason,
            proxy.endpoint_key,
            proxy.updated_at,
        ],
    )
}
