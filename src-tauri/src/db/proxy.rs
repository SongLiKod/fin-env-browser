use rusqlite::{params, Row};

use crate::{
    error::{AppError, Result},
    models::{Proxy, ProxyCheckResult},
};

use super::{bool_int, int_bool, Db};

const PROXY_COLUMNS: &str = "id, name, protocol, host, port, username, password_enc, enabled, \
    last_check_at, latency_ms, exit_ip, exit_country, exit_timezone, created_at, updated_at";

fn row_to_proxy(row: &Row<'_>) -> rusqlite::Result<Proxy> {
    let password_enc: String = row.get(6)?;
    Ok(Proxy {
        id: row.get(0)?,
        name: row.get(1)?,
        protocol: row.get(2)?,
        host: row.get(3)?,
        port: row.get::<_, i64>(4)? as u16,
        username: row.get(5)?,
        has_password: !password_enc.is_empty(),
        password_enc,
        enabled: int_bool(row.get(7)?),
        last_check_at: row.get(8)?,
        latency_ms: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
        exit_ip: row.get(10)?,
        exit_country: row.get(11)?,
        exit_timezone: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
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

    pub fn insert_proxy(&self, proxy: &Proxy) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO proxies (id, name, protocol, host, port, username, password_enc,
                 enabled, last_check_at, latency_ms, exit_ip, exit_country, exit_timezone,
                 created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15)",
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
                    proxy.created_at,
                    proxy.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_proxy(&self, proxy: &Proxy) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE proxies SET name=?2, protocol=?3, host=?4, port=?5, username=?6,
                 password_enc=?7, enabled=?8, last_check_at=?9, latency_ms=?10, exit_ip=?11,
                 exit_country=?12, exit_timezone=?13, updated_at=?14 WHERE id=?1",
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
                    proxy.updated_at,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("代理不存在"));
            }
            Ok(())
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

    pub fn save_proxy_check(&self, result: &ProxyCheckResult) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE proxies SET last_check_at=?2, latency_ms=?3, exit_ip=?4,
                 exit_country=?5, exit_timezone=?6, updated_at=?2 WHERE id=?1",
                params![
                    result.proxy_id,
                    result.checked_at,
                    result.latency_ms,
                    result.exit_ip,
                    result.country,
                    result.timezone,
                ],
            )?;
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
}
