use rusqlite::{params, Row};

use crate::{
    error::{AppError, Result},
    models::Account,
};

use super::Db;

const ACCOUNT_COLUMNS: &str =
    "id, env_id, username, password_enc, remark, owner, status, created_at, updated_at";

fn row_to_account(row: &Row<'_>) -> rusqlite::Result<Account> {
    let password_enc: String = row.get(3)?;
    Ok(Account {
        id: row.get(0)?,
        env_id: row.get(1)?,
        username: row.get(2)?,
        has_password: !password_enc.is_empty(),
        password_enc,
        remark: row.get(4)?,
        owner: row.get(5)?,
        status: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

impl Db {
    pub fn list_accounts(&self, env_id: &str) -> Result<Vec<Account>> {
        self.with(|conn| {
            let mut stmt = conn.prepare(&format!(
                "SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE env_id = ?1 ORDER BY created_at"
            ))?;
            let rows = stmt.query_map(params![env_id], row_to_account)?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        })
    }

    pub fn get_account(&self, id: &str) -> Result<Account> {
        self.with(|conn| {
            conn.query_row(
                &format!("SELECT {ACCOUNT_COLUMNS} FROM accounts WHERE id = ?1"),
                params![id],
                row_to_account,
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => AppError::not_found("账号不存在"),
                other => other.into(),
            })
        })
    }

    pub fn insert_account(&self, account: &Account) -> Result<()> {
        self.with(|conn| {
            conn.execute(
                "INSERT INTO accounts (id, env_id, username, password_enc, remark, owner, status, created_at, updated_at)
                 VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                params![
                    account.id,
                    account.env_id,
                    account.username,
                    account.password_enc,
                    account.remark,
                    account.owner,
                    account.status,
                    account.created_at,
                    account.updated_at,
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _)
                    if err.code == rusqlite::ErrorCode::ConstraintViolation =>
                {
                    AppError::conflict("该环境内已存在同名账号")
                }
                other => other.into(),
            })?;
            Ok(())
        })
    }

    pub fn update_account(&self, account: &Account) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute(
                "UPDATE accounts SET username=?2, password_enc=?3, remark=?4, owner=?5, status=?6, updated_at=?7 WHERE id=?1",
                params![
                    account.id,
                    account.username,
                    account.password_enc,
                    account.remark,
                    account.owner,
                    account.status,
                    account.updated_at,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found("账号不存在"));
            }
            Ok(())
        })
    }

    pub fn delete_account(&self, id: &str) -> Result<()> {
        self.with(|conn| {
            let changed = conn.execute("DELETE FROM accounts WHERE id = ?1", params![id])?;
            if changed == 0 {
                return Err(AppError::not_found("账号不存在"));
            }
            Ok(())
        })
    }

    pub fn bulk_insert_accounts(&self, accounts: &[Account]) -> Result<u32> {
        self.with(|conn| {
            let tx = conn.transaction()?;
            let mut inserted = 0u32;
            for account in accounts {
                let changed = tx.execute(
                    "INSERT OR IGNORE INTO accounts (id, env_id, username, password_enc, remark, owner, status, created_at, updated_at)
                     VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
                    params![
                        account.id,
                        account.env_id,
                        account.username,
                        account.password_enc,
                        account.remark,
                        account.owner,
                        account.status,
                        account.created_at,
                        account.updated_at,
                    ],
                )?;
                inserted += changed as u32;
            }
            tx.commit()?;
            Ok(inserted)
        })
    }

    pub fn account_count_for_env(&self, env_id: &str) -> Result<u32> {
        self.with(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM accounts WHERE env_id = ?1",
                params![env_id],
                |row| row.get(0),
            )?;
            Ok(count as u32)
        })
    }

    pub fn account_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get(0))?;
            Ok(count as u32)
        })
    }
}
