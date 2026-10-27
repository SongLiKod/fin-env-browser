use rusqlite::{params, params_from_iter, types::Value, Row};
use sha2::{Digest, Sha256};

use crate::{
    error::Result,
    models::{new_id, now_iso, AuditFilter, AuditLog, AuditVerifyResult},
};

use super::Db;

const AUDIT_COLUMNS: &str =
    "id, actor, action, env_id, account, project, case_id, detail, created_at, prev_hash, hash";

const GENESIS_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

fn row_to_audit(row: &Row<'_>) -> rusqlite::Result<AuditLog> {
    Ok(AuditLog {
        id: row.get(0)?,
        actor: row.get(1)?,
        action: row.get(2)?,
        env_id: row.get(3)?,
        account: row.get(4)?,
        project: row.get(5)?,
        case_id: row.get(6)?,
        detail: row.get(7)?,
        created_at: row.get(8)?,
        prev_hash: row.get(9)?,
        hash: row.get(10)?,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn canonical(
    actor: &str,
    action: &str,
    env_id: Option<&str>,
    account: Option<&str>,
    project: Option<&str>,
    case_id: Option<&str>,
    detail: &str,
    created_at: &str,
    prev_hash: &str,
) -> String {
    format!(
        "{prev_hash}|{actor}|{action}|{}|{}|{}|{}|{detail}|{created_at}",
        env_id.unwrap_or(""),
        account.unwrap_or(""),
        project.unwrap_or(""),
        case_id.unwrap_or(""),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn hash_entry(
    actor: &str,
    action: &str,
    env_id: Option<&str>,
    account: Option<&str>,
    project: Option<&str>,
    case_id: Option<&str>,
    detail: &str,
    created_at: &str,
    prev_hash: &str,
) -> String {
    let canonical = canonical(
        actor, action, env_id, account, project, case_id, detail, created_at, prev_hash,
    );
    hex::encode(Sha256::digest(canonical.as_bytes()))
}

impl Db {
    #[allow(clippy::too_many_arguments)]
    pub fn append_audit(
        &self,
        actor: &str,
        action: &str,
        env_id: Option<&str>,
        account: Option<&str>,
        project: Option<&str>,
        case_id: Option<&str>,
        detail: &str,
    ) -> Result<AuditLog> {
        let created_at = now_iso();
        self.with(|conn| {
            let tx = conn.transaction()?;
            let prev_hash: String = tx
                .query_row(
                    "SELECT hash FROM audit_logs ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .unwrap_or_else(|_| GENESIS_HASH.to_string());
            let hash = hash_entry(
                actor, action, env_id, account, project, case_id, detail, &created_at, &prev_hash,
            );
            let log = AuditLog {
                id: new_id(),
                actor: actor.to_string(),
                action: action.to_string(),
                env_id: env_id.map(ToString::to_string),
                account: account.map(ToString::to_string),
                project: project.map(ToString::to_string),
                case_id: case_id.map(ToString::to_string),
                detail: detail.to_string(),
                created_at: created_at.clone(),
                prev_hash,
                hash,
            };
            tx.execute(
                "INSERT INTO audit_logs (id, actor, action, env_id, account, project, case_id, detail, created_at, prev_hash, hash)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
                params![
                    log.id,
                    log.actor,
                    log.action,
                    log.env_id,
                    log.account,
                    log.project,
                    log.case_id,
                    log.detail,
                    log.created_at,
                    log.prev_hash,
                    log.hash,
                ],
            )?;
            tx.commit()?;
            Ok(log)
        })
    }

    pub fn list_audit(&self, filter: &AuditFilter) -> Result<Vec<AuditLog>> {
        let mut sql = format!("SELECT {AUDIT_COLUMNS} FROM audit_logs WHERE 1=1");
        let mut values: Vec<Value> = Vec::new();
        if let Some(search) = filter.search.as_deref().filter(|s| !s.trim().is_empty()) {
            sql.push_str(" AND (actor LIKE ? OR action LIKE ? OR detail LIKE ? OR account LIKE ? OR project LIKE ? OR case_id LIKE ?)");
            let like = format!("%{}%", search.trim());
            for _ in 0..6 {
                values.push(Value::Text(like.clone()));
            }
        }
        if let Some(env_id) = filter.env_id.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND env_id = ?");
            values.push(Value::Text(env_id.to_string()));
        }
        if let Some(action) = filter.action.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND action = ?");
            values.push(Value::Text(action.to_string()));
        }
        if let Some(actor) = filter.actor.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND actor = ?");
            values.push(Value::Text(actor.to_string()));
        }
        if let Some(from) = filter.from.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND created_at >= ?");
            values.push(Value::Text(from.to_string()));
        }
        if let Some(to) = filter.to.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND created_at <= ?");
            values.push(Value::Text(to.to_string()));
        }
        sql.push_str(" ORDER BY created_at DESC, rowid DESC LIMIT ? OFFSET ?");
        let limit = filter.limit.unwrap_or(200).min(2000) as i64;
        let offset = filter.offset.unwrap_or(0) as i64;
        values.push(Value::Integer(limit));
        values.push(Value::Integer(offset));

        self.with(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map(params_from_iter(values.iter()), row_to_audit)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn recent_audit(&self, limit: u32) -> Result<Vec<AuditLog>> {
        self.list_audit(&AuditFilter {
            limit: Some(limit),
            ..Default::default()
        })
    }

    pub fn audit_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM audit_logs", [], |row| row.get(0))?;
            Ok(count as u32)
        })
    }

    pub fn verify_audit_chain(&self) -> Result<AuditVerifyResult> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {AUDIT_COLUMNS} FROM audit_logs ORDER BY created_at ASC, rowid ASC"
            ))?;
            let mut rows = stmt.query([])?;
            let mut prev = GENESIS_HASH.to_string();
            let mut checked = 0u64;
            while let Some(row) = rows.next()? {
                let log = row_to_audit(row)?;
                let expected = hash_entry(
                    &log.actor,
                    &log.action,
                    log.env_id.as_deref(),
                    log.account.as_deref(),
                    log.project.as_deref(),
                    log.case_id.as_deref(),
                    &log.detail,
                    &log.created_at,
                    &prev,
                );
                if log.prev_hash != prev || expected != log.hash {
                    return Ok(AuditVerifyResult {
                        valid: false,
                        checked,
                        broken_at: Some(log.id),
                        message: "审计日志哈希链断裂，记录可能被篡改".into(),
                    });
                }
                prev = log.hash;
                checked += 1;
            }
            Ok(AuditVerifyResult {
                valid: true,
                checked,
                broken_at: None,
                message: if checked == 0 {
                    "审计日志为空".into()
                } else {
                    format!("已校验 {checked} 条审计日志，哈希链完整")
                },
            })
        })
    }
}
