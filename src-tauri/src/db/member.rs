use rusqlite::{params, Row};

use crate::{
    error::{AppError, Result},
    models::{EnvironmentMember, EnvironmentMemberCandidate},
};

use super::{int_bool, Db};

fn row_to_member(row: &Row<'_>) -> rusqlite::Result<EnvironmentMember> {
    Ok(EnvironmentMember {
        user_id: row.get(0)?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        role: row.get(3)?,
        enabled: int_bool(row.get(4)?),
        access_level: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

impl Db {
    pub fn list_environment_members(&self, env_id: &str) -> Result<Vec<EnvironmentMember>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT u.id, u.username, u.display_name, u.role, u.enabled,
                        m.access_level, m.created_at, m.updated_at
                 FROM environment_members m
                 JOIN users u ON u.id = m.user_id
                 WHERE m.env_id = ?1
                 ORDER BY m.created_at",
            )?;
            let rows = stmt.query_map(params![env_id], row_to_member)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn list_environment_member_candidates(
        &self,
        env_id: &str,
    ) -> Result<Vec<EnvironmentMemberCandidate>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(
                "SELECT u.id, u.username, u.display_name, u.role, u.enabled, m.access_level
                 FROM users u
                 LEFT JOIN environment_members m ON m.user_id = u.id AND m.env_id = ?1
                 ORDER BY u.role, u.display_name, u.username",
            )?;
            let rows = stmt.query_map(params![env_id], |row| {
                Ok(EnvironmentMemberCandidate {
                    user_id: row.get(0)?,
                    username: row.get(1)?,
                    display_name: row.get(2)?,
                    role: row.get(3)?,
                    enabled: int_bool(row.get(4)?),
                    access_level: row.get(5)?,
                })
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn environment_member_level(&self, env_id: &str, user_id: &str) -> Result<Option<String>> {
        self.with(|conn| {
            match conn.query_row(
                "SELECT access_level FROM environment_members WHERE env_id = ?1 AND user_id = ?2",
                params![env_id, user_id],
                |row| row.get(0),
            ) {
                Ok(level) => Ok(Some(level)),
                Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
                Err(error) => Err(error.into()),
            }
        })
    }

    pub fn list_environment_ids_for_user(&self, user_id: &str) -> Result<Vec<String>> {
        self.with(|conn| {
            let mut stmt =
                conn.prepare("SELECT env_id FROM environment_members WHERE user_id = ?1")?;
            let rows = stmt.query_map(params![user_id], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn upsert_environment_member(
        &self,
        env_id: &str,
        user_id: &str,
        access_level: &str,
    ) -> Result<()> {
        self.with(|conn| {
            let now = crate::models::now_iso();
            conn.execute(
                "INSERT INTO environment_members (env_id, user_id, access_level, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?4)
                 ON CONFLICT(env_id, user_id) DO UPDATE SET
                   access_level = excluded.access_level,
                   updated_at = excluded.updated_at",
                params![env_id, user_id, access_level, now],
            )?;
            Ok(())
        })
    }

    pub fn remove_environment_member(&self, env_id: &str, user_id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "DELETE FROM environment_members WHERE env_id = ?1 AND user_id = ?2",
                params![env_id, user_id],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("环境成员不存在"));
            }
            Ok(())
        })
    }
}
