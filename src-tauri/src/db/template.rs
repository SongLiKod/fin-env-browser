use rusqlite::{params, Row};

use crate::{
    crypto::CryptoManager,
    error::{AppError, Result},
    models::EnvironmentTemplate,
};

use super::{bool_int, int_bool, Db};

fn decrypt_fingerprint(
    stored: &str,
    crypto: Option<&CryptoManager>,
) -> rusqlite::Result<serde_json::Value> {
    let plain = if stored.starts_with("v1:") {
        let Some(crypto) = crypto else {
            return Err(rusqlite::Error::FromSqlConversionFailure(
                15,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "加密的模板指纹缺少主密钥",
                )),
            ));
        };
        crypto.decrypt(stored).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                15,
                rusqlite::types::Type::Text,
                Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    error.message(),
                )),
            )
        })?
    } else {
        stored.to_string()
    };
    serde_json::from_str(&plain).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(15, rusqlite::types::Type::Text, Box::new(error))
    })
}

const TEMPLATE_COLUMNS: &str = "id, name, description, project, test_env, account_role, version, \
    case_id, tags, notes, kernel, chromium_path, auto_restart, memory_limit_mb, idle_timeout_min, \
    fingerprint_json, border_color, border_width, group_id, proxy_mode, proxy_id, extension_ids, \
    chromium_launch_args, taskbar_badge_text, network_profile, created_at, updated_at";

fn row_to_template(
    row: &Row<'_>,
    crypto: Option<&CryptoManager>,
) -> rusqlite::Result<EnvironmentTemplate> {
    let tags: String = row.get(8)?;
    let fingerprint = decrypt_fingerprint(&row.get::<_, String>(15)?, crypto)?;
    let extension_ids: String = row.get(21)?;
    let launch_args: String = row.get(22)?;
    let taskbar_badge_text: String = row.get(23)?;
    let network_profile: String = row.get(24)?;
    Ok(EnvironmentTemplate {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        project: row.get(3)?,
        test_env: row.get(4)?,
        account_role: row.get(5)?,
        version: row.get(6)?,
        case_id: row.get(7)?,
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        notes: row.get(9)?,
        kernel: row.get(10)?,
        chromium_path: row.get(11)?,
        auto_restart: int_bool(row.get::<_, i64>(12)?),
        memory_limit_mb: row.get::<_, Option<i64>>(13)?.map(|value| value as u32),
        idle_timeout_min: row.get::<_, Option<i64>>(14)?.map(|value| value as u32),
        fingerprint: serde_json::from_value(fingerprint)
            .unwrap_or_else(|_| crate::fingerprint::template("win11")),
        border_color: row.get(16)?,
        border_width: row.get::<_, i64>(17)? as u32,
        group_id: row.get(18)?,
        proxy_mode: row.get(19)?,
        proxy_id: row.get(20)?,
        extension_ids: serde_json::from_str(&extension_ids).unwrap_or_default(),
        chromium_launch_args: serde_json::from_str(&launch_args).unwrap_or_default(),
        taskbar_badge_text,
        network_profile,
        created_at: row.get(25)?,
        updated_at: row.get(26)?,
    })
}

impl Db {
    pub fn list_templates(&self) -> Result<Vec<EnvironmentTemplate>> {
        let crypto = self.crypto();
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {TEMPLATE_COLUMNS} FROM environment_templates ORDER BY created_at DESC"
            ))?;
            let rows = stmt.query_map([], |row| row_to_template(row, crypto.as_deref()))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_template(&self, id: &str) -> Result<EnvironmentTemplate> {
        let crypto = self.crypto();
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {TEMPLATE_COLUMNS} FROM environment_templates WHERE id = ?1"),
                params![id],
                |row| row_to_template(row, crypto.as_deref()),
            )
            .map_err(|error| match error {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::not_found(format!("环境模板 {id} 不存在"))
                }
                other => other.into(),
            })
        })
    }

    pub fn find_template_by_name(&self, name: &str) -> Result<Option<EnvironmentTemplate>> {
        let crypto = self.crypto();
        self.with(|conn| {
            match conn.query_row(
                &format!("SELECT {TEMPLATE_COLUMNS} FROM environment_templates WHERE name = ?1"),
                params![name],
                |row| row_to_template(row, crypto.as_deref()),
            ) {
                Ok(template) => Ok(Some(template)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(error) => Err(error.into()),
            }
        })
    }

    pub fn insert_template(&self, template: &EnvironmentTemplate) -> Result<()> {
        let tags = serde_json::to_string(&template.tags)?;
        let fingerprint = self.encrypt_json(&serde_json::to_string(&template.fingerprint)?)?;
        let extension_ids = serde_json::to_string(&template.extension_ids)?;
        let launch_args = serde_json::to_string(&template.chromium_launch_args)?;
        self.with(|conn| {
            conn.execute(
                "INSERT INTO environment_templates (id, name, description, project, test_env,
                 account_role, version, case_id, tags, notes, kernel, chromium_path, auto_restart,
                 memory_limit_mb, idle_timeout_min, fingerprint_json, border_color, border_width,
                 group_id, proxy_mode, proxy_id, extension_ids, chromium_launch_args,
                 taskbar_badge_text, network_profile, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23,?24,?25,?26,?27)",
                params![
                    template.id,
                    template.name,
                    template.description,
                    template.project,
                    template.test_env,
                    template.account_role,
                    template.version,
                    template.case_id,
                    tags,
                    template.notes,
                    template.kernel,
                    template.chromium_path,
                    bool_int(template.auto_restart),
                    template.memory_limit_mb,
                    template.idle_timeout_min,
                    fingerprint,
                    template.border_color,
                    template.border_width,
                    template.group_id,
                    template.proxy_mode,
                    template.proxy_id,
                    extension_ids,
                    launch_args,
                    template.taskbar_badge_text,
                    template.network_profile,
                    template.created_at,
                    template.updated_at,
                ],
            )?;
            Ok(())
        })
    }

    pub fn update_template(&self, template: &EnvironmentTemplate) -> Result<()> {
        let tags = serde_json::to_string(&template.tags)?;
        let fingerprint = self.encrypt_json(&serde_json::to_string(&template.fingerprint)?)?;
        let extension_ids = serde_json::to_string(&template.extension_ids)?;
        let launch_args = serde_json::to_string(&template.chromium_launch_args)?;
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE environment_templates SET name=?2, description=?3, project=?4,
                 test_env=?5, account_role=?6, version=?7, case_id=?8, tags=?9, notes=?10,
                 kernel=?11, chromium_path=?12, auto_restart=?13, memory_limit_mb=?14,
                 idle_timeout_min=?15, fingerprint_json=?16, border_color=?17, border_width=?18,
                 group_id=?19, proxy_mode=?20, proxy_id=?21, extension_ids=?22,
                 chromium_launch_args=?23, taskbar_badge_text=?24, network_profile=?25,
                 updated_at=?26
                 WHERE id=?1",
                params![
                    template.id,
                    template.name,
                    template.description,
                    template.project,
                    template.test_env,
                    template.account_role,
                    template.version,
                    template.case_id,
                    tags,
                    template.notes,
                    template.kernel,
                    template.chromium_path,
                    bool_int(template.auto_restart),
                    template.memory_limit_mb,
                    template.idle_timeout_min,
                    fingerprint,
                    template.border_color,
                    template.border_width,
                    template.group_id,
                    template.proxy_mode,
                    template.proxy_id,
                    extension_ids,
                    launch_args,
                    template.taskbar_badge_text,
                    template.network_profile,
                    template.updated_at,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("环境模板不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_template(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "DELETE FROM environment_templates WHERE id = ?1",
                params![id],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("环境模板不存在"));
            }
            Ok(())
        })
    }
}
