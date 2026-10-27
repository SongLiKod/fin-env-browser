use super::Db;
use crate::{error::Result, models::DashboardStats};

impl Db {
    pub fn environment_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM environments", [], |row| row.get(0))?;
            Ok(count as u32)
        })
    }

    pub fn running_session_count(&self) -> Result<u32> {
        self.with(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM browser_sessions", [], |row| {
                    row.get(0)
                })?;
            Ok(count as u32)
        })
    }

    pub fn dashboard_counts(&self, max_concurrent: u32) -> Result<DashboardStats> {
        let env_count = self.environment_count()?;
        let account_count = self.account_count()?;
        let proxy_count = self.proxy_count()?;
        let running_count = self.running_session_count()?;
        let snapshot_count = self.snapshot_count()?;
        let audit_count = self.audit_count()?;
        let recent_audit = self.recent_audit(8)?;
        Ok(DashboardStats {
            env_count,
            account_count,
            proxy_count,
            running_count,
            snapshot_count,
            audit_count,
            total_memory_mb: 0,
            max_concurrent,
            recent_audit,
        })
    }
}
