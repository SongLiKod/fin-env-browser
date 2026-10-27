use rusqlite::{params, params_from_iter, types::Value, Row};

use crate::{
    crypto::CryptoManager,
    error::{AppError, Result},
    models::{Environment, EnvironmentFilter, Group},
};

use super::{bool_int, int_bool, Db};

const ENV_COLUMNS: &str = "id, name, group_id, status, profile_dir, proxy_id, fingerprint_snapshot_id, \
    project, test_env, account_role, version, case_id, tags, notes, kernel, chromium_path, \
    auto_restart, memory_limit_mb, idle_timeout_min, fingerprint_json, created_at, updated_at, last_opened_at, \
    border_color, ip_address, ip_country, ip_country_code, ip_city, ip_timezone, ip_source, ip_checked_at";

fn decrypt_fingerprint(
    stored: &str,
    crypto: Option<&CryptoManager>,
) -> rusqlite::Result<serde_json::Value> {
    let plain = if stored.starts_with("v1:") {
        let Some(crypto) = crypto else {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                19,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "fingerprint encrypted but crypto manager is not attached",
                )),
            ));
        };
        crypto.decrypt(stored).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(
                19,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.message(),
                )),
            )
        })?
    } else {
        stored.to_string()
    };
    serde_json::from_str(&plain).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(
            19,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                e.to_string(),
            )),
        )
    })
}

fn row_to_env(row: &Row<'_>, crypto: Option<&CryptoManager>) -> rusqlite::Result<Environment> {
    let tags: String = row.get(12)?;
    let fp = decrypt_fingerprint(&row.get::<_, String>(19)?, crypto)?;
    Ok(Environment {
        id: row.get(0)?,
        name: row.get(1)?,
        group_id: row.get(2)?,
        status: row.get(3)?,
        profile_dir: row.get(4)?,
        proxy_id: row.get(5)?,
        fingerprint_snapshot_id: row.get(6)?,
        project: row.get(7)?,
        test_env: row.get(8)?,
        account_role: row.get(9)?,
        version: row.get(10)?,
        case_id: row.get(11)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        notes: row.get(13)?,
        kernel: row.get(14)?,
        chromium_path: row.get(15)?,
        auto_restart: int_bool(row.get::<_, i64>(16)?),
        memory_limit_mb: row.get::<_, Option<i64>>(17)?.map(|v| v as u32),
        idle_timeout_min: row.get::<_, Option<i64>>(18)?.map(|v| v as u32),
        fingerprint: serde_json::from_value(fp)
            .unwrap_or_else(|_| crate::fingerprint::template("win11")),
        border_color: row.get(23)?,
        ip_address: row.get(24)?,
        ip_country: row.get(25)?,
        ip_country_code: row.get(26)?,
        ip_city: row.get(27)?,
        ip_timezone: row.get(28)?,
        ip_source: row.get(29)?,
        ip_checked_at: row.get(30)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        last_opened_at: row.get(22)?,
    })
}

impl Db {
    pub fn list_environments(&self, filter: &EnvironmentFilter) -> Result<Vec<Environment>> {
        let mut sql = format!("SELECT {ENV_COLUMNS} FROM environments WHERE 1=1");
        let mut values: Vec<Value> = Vec::new();

        if let Some(search) = filter.search.as_deref().filter(|s| !s.trim().is_empty()) {
            sql.push_str(
                " AND (name LIKE ? OR project LIKE ? OR tags LIKE ? OR notes LIKE ? OR case_id LIKE ? OR account_role LIKE ? OR version LIKE ? OR test_env LIKE ?)",
            );
            let like = format!("%{}%", search.trim());
            for _ in 0..8 {
                values.push(Value::Text(like.clone()));
            }
        }
        if let Some(group) = filter.group_id.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND group_id = ?");
            values.push(Value::Text(group.to_string()));
        }
        if let Some(status) = filter.status.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND status = ?");
            values.push(Value::Text(status.to_string()));
        }
        if let Some(project) = filter.project.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND project = ?");
            values.push(Value::Text(project.to_string()));
        }
        if let Some(test_env) = filter.test_env.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND test_env = ?");
            values.push(Value::Text(test_env.to_string()));
        }
        if let Some(tag) = filter.tag.as_deref().filter(|s| !s.is_empty()) {
            sql.push_str(" AND tags LIKE ?");
            values.push(Value::Text(format!("%\"{tag}\"%")));
        }
        if filter.running_only.unwrap_or(false) {
            sql.push_str(
                " AND EXISTS (SELECT 1 FROM browser_sessions s WHERE s.env_id = environments.id)",
            );
        }
        sql.push_str(" ORDER BY created_at DESC");

        self.with(|conn| {
            let mut stmt = conn.prepare(&sql)?;
            let crypto = self.crypto();
            let rows = stmt.query_map(params_from_iter(values.iter()), |row| {
                row_to_env(row, crypto.as_deref())
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_environment(&self, id: &str) -> Result<Environment> {
        let crypto = self.crypto();
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {ENV_COLUMNS} FROM environments WHERE id = ?1"),
                params![id],
                |row| row_to_env(row, crypto.as_deref()),
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::not_found(format!("环境 {id} 不存在"))
                }
                other => other.into(),
            })
        })
    }

    pub fn find_environment_by_name(&self, name: &str) -> Result<Option<Environment>> {
        let crypto = self.crypto();
        self.with(|conn| {
            match conn.query_row(
                &format!("SELECT {ENV_COLUMNS} FROM environments WHERE name = ?1"),
                params![name],
                |row| row_to_env(row, crypto.as_deref()),
            ) {
                Ok(env) => Ok(Some(env)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(e) => Err(e.into()),
            }
        })
    }

    pub fn insert_environment(&self, env: &Environment) -> Result<()> {
        let tags = serde_json::to_string(&env.tags)?;
        let fp = self.encrypt_json(&serde_json::to_string(&env.fingerprint)?)?;
        self.with(|conn| {
            conn.execute(
                "INSERT INTO environments (id, name, group_id, status, profile_dir, proxy_id,
                 fingerprint_snapshot_id, project, test_env, account_role, version, case_id, tags,
                 notes, kernel, chromium_path, auto_restart, memory_limit_mb, idle_timeout_min,
                 fingerprint_json, created_at, updated_at, last_opened_at, border_color, ip_address,
                 ip_country, ip_country_code, ip_city, ip_timezone, ip_source, ip_checked_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27,?28,?29,?30,?31)",
                params![
                    env.id,
                    env.name,
                    env.group_id,
                    env.status,
                    env.profile_dir,
                    env.proxy_id,
                    env.fingerprint_snapshot_id,
                    env.project,
                    env.test_env,
                    env.account_role,
                    env.version,
                    env.case_id,
                    tags,
                    env.notes,
                    env.kernel,
                    env.chromium_path,
                    bool_int(env.auto_restart),
                    env.memory_limit_mb,
                    env.idle_timeout_min,
                    fp,
                    env.created_at,
                    env.updated_at,
                    env.last_opened_at,
                    env.border_color,
                    env.ip_address,
                    env.ip_country,
                    env.ip_country_code,
                    env.ip_city,
                    env.ip_timezone,
                    env.ip_source,
                    env.ip_checked_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_environment(&self, env: &Environment) -> Result<()> {
        let tags = serde_json::to_string(&env.tags)?;
        let fp = self.encrypt_json(&serde_json::to_string(&env.fingerprint)?)?;
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE environments SET name=?2, group_id=?3, status=?4, profile_dir=?5,
                 proxy_id=?6, fingerprint_snapshot_id=?7, project=?8, test_env=?9, account_role=?10,
                 version=?11, case_id=?12, tags=?13, notes=?14, kernel=?15, chromium_path=?16,
                 auto_restart=?17, memory_limit_mb=?18, idle_timeout_min=?19, fingerprint_json=?20,
                 updated_at=?21, last_opened_at=?22, border_color=?23, ip_address=?24,
                 ip_country=?25, ip_country_code=?26, ip_city=?27, ip_timezone=?28, ip_source=?29,
                 ip_checked_at=?30 WHERE id=?1",
                params![
                    env.id,
                    env.name,
                    env.group_id,
                    env.status,
                    env.profile_dir,
                    env.proxy_id,
                    env.fingerprint_snapshot_id,
                    env.project,
                    env.test_env,
                    env.account_role,
                    env.version,
                    env.case_id,
                    tags,
                    env.notes,
                    env.kernel,
                    env.chromium_path,
                    bool_int(env.auto_restart),
                    env.memory_limit_mb,
                    env.idle_timeout_min,
                    fp,
                    env.updated_at,
                    env.last_opened_at,
                    env.border_color,
                    env.ip_address,
                    env.ip_country,
                    env.ip_country_code,
                    env.ip_city,
                    env.ip_timezone,
                    env.ip_source,
                    env.ip_checked_at,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("环境不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_environment(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute("DELETE FROM environments WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("环境不存在"));
            }
            Ok(())
        })
    }

    pub fn set_environment_proxy(
        &self,
        env_ids: &[String],
        proxy_id: Option<&str>,
    ) -> Result<usize> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let mut changed = 0usize;
            for id in env_ids {
                changed += tx.execute(
                    "UPDATE environments SET proxy_id = ?2, updated_at = ?3,
                     ip_address = NULL, ip_country = NULL, ip_country_code = NULL, ip_city = NULL,
                     ip_timezone = NULL, ip_source = NULL, ip_checked_at = NULL WHERE id = ?1",
                    params![id, proxy_id, crate::models::now_iso()],
                )?;
            }
            tx.commit()?;
            Ok(changed)
        })
    }

    pub fn save_environment_ip(&self, info: &crate::models::EnvironmentIpInfo) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE environments SET ip_address=?2, ip_country=?3, ip_country_code=?4,
                 ip_city=?5, ip_timezone=?6, ip_source=?7, ip_checked_at=?8
                 WHERE id=?1",
                params![
                    info.env_id,
                    info.ip,
                    info.country,
                    info.country_code,
                    info.city,
                    info.timezone,
                    info.source,
                    info.checked_at,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("环境不存在"));
            }
            Ok(())
        })
    }

    pub fn set_environment_group(
        &self,
        env_ids: &[String],
        group_id: Option<&str>,
    ) -> Result<usize> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let mut changed = 0usize;
            for id in env_ids {
                changed += tx.execute(
                    "UPDATE environments SET group_id = ?2, updated_at = ?3 WHERE id = ?1",
                    params![id, group_id, crate::models::now_iso()],
                )?;
            }
            tx.commit()?;
            Ok(changed)
        })
    }

    pub fn set_environment_snapshot(&self, env_id: &str, snapshot_id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "UPDATE environments SET fingerprint_snapshot_id = ?2, updated_at = ?3 WHERE id = ?1",
                params![env_id, snapshot_id, crate::models::now_iso()],
            )?;
            Ok(())
        })
    }

    pub fn touch_environment_opened(&self, env_id: &str) -> Result<()> {
        let now = crate::models::now_iso();
        self.with(|conn| {
            conn.execute(
                "UPDATE environments SET last_opened_at = ?2, updated_at = ?2 WHERE id = ?1",
                params![env_id, now],
            )?;
            Ok(())
        })
    }

    pub fn environment_projects(&self) -> Result<Vec<String>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT project FROM environments WHERE project <> '' ORDER BY project",
            )?;
            let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn all_tags(&self) -> Result<Vec<String>> {
        let envs = self.list_environments(&Default::default())?;
        let mut tags: Vec<String> = envs.into_iter().flat_map(|e| e.tags).collect();
        tags.sort();
        tags.dedup();
        Ok(tags)
    }

    // -- groups -------------------------------------------------------------

    pub fn list_groups(&self) -> Result<Vec<Group>> {
        self.with(|conn| {
            let mut stmt =
                conn.prepare("SELECT id, name, color, created_at FROM groups ORDER BY name")?;
            let rows = stmt.query_map([], |row| {
                Ok(Group {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    color: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn create_group(&self, group: &Group) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO groups (id, name, color, created_at) VALUES (?1,?2,?3,?4)",
                params![group.id, group.name, group.color, group.created_at],
            )?;
            Ok(())
        })
    }

    pub fn update_group(&self, id: &str, name: &str, color: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE groups SET name = ?2, color = ?3 WHERE id = ?1",
                params![id, name, color],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("分组不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_group(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            conn.execute("DELETE FROM groups WHERE id = ?1", params![id])?;
            Ok(())
        })
    }
}
