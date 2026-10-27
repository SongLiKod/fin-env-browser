use crate::{
    db::Db,
    error::Result,
    models::{AuditLog, Environment},
};

/// Central audit writer. Every process operation, sensitive field access and
/// high-risk mutation must go through this module, which appends to the
/// tamper-evident hash chain in SQLite.
pub struct Audit;

impl Audit {
    pub fn env(
        db: &Db,
        actor: &str,
        action: &str,
        env: &Environment,
        detail: impl AsRef<str>,
    ) -> Result<AuditLog> {
        let log = db.append_audit(
            actor,
            action,
            Some(&env.id).map(|id| id.as_str()),
            None,
            Some(env.project.as_str()).filter(|p| !p.is_empty()),
            Some(env.case_id.as_str()).filter(|c| !c.is_empty()),
            detail.as_ref(),
        )?;
        tracing::info!(
            audit_id = %log.id,
            actor,
            action,
            env_id = %env.id,
            "audit"
        );
        Ok(log)
    }

    pub fn global(db: &Db, actor: &str, action: &str, detail: impl AsRef<str>) -> Result<AuditLog> {
        let log = db.append_audit(actor, action, None, None, None, None, detail.as_ref())?;
        tracing::info!(audit_id = %log.id, actor, action, "audit");
        Ok(log)
    }

    pub fn env_account(
        db: &Db,
        actor: &str,
        action: &str,
        env: &Environment,
        account: &str,
        detail: impl AsRef<str>,
    ) -> Result<AuditLog> {
        let log = db.append_audit(
            actor,
            action,
            Some(&env.id).map(|id| id.as_str()),
            Some(account),
            Some(env.project.as_str()).filter(|p| !p.is_empty()),
            Some(env.case_id.as_str()).filter(|c| !c.is_empty()),
            detail.as_ref(),
        )?;
        tracing::info!(audit_id = %log.id, actor, action, env_id = %env.id, "audit");
        Ok(log)
    }
}

pub mod actions {
    pub const ENV_CREATE: &str = "env.create";
    pub const ENV_UPDATE: &str = "env.update";
    pub const ENV_DELETE: &str = "env.delete";
    pub const ENV_CLONE: &str = "env.clone";
    pub const ENV_IMPORT: &str = "env.import";
    pub const ENV_EXPORT: &str = "env.export";
    pub const ENV_IP_CHECK: &str = "env.ip_check";
    pub const ENV_OPEN: &str = "browser.open";
    pub const ENV_CLOSE: &str = "browser.close";
    pub const ENV_CRASH: &str = "browser.crash";
    pub const ENV_RECYCLE: &str = "browser.recycle";
    pub const ENV_IDLE_CLOSE: &str = "browser.idle_close";
    pub const ENV_MEMORY_CLOSE: &str = "browser.memory_close";
    pub const SCREENSHOT: &str = "browser.screenshot";
    pub const RECORDING: &str = "browser.recording";
    pub const PRE_SCRIPT: &str = "browser.pre_script";
    pub const COOKIE_READ: &str = "cookie.read";
    pub const COOKIE_WRITE: &str = "cookie.write";
    pub const ACCOUNT_CREATE: &str = "account.create";
    pub const ACCOUNT_UPDATE: &str = "account.update";
    pub const ACCOUNT_DELETE: &str = "account.delete";
    pub const ACCOUNT_REVEAL: &str = "account.reveal";
    pub const ACCOUNT_IMPORT: &str = "account.import";
    pub const ACCOUNT_EXPORT: &str = "account.export";
    pub const PROXY_CREATE: &str = "proxy.create";
    pub const PROXY_UPDATE: &str = "proxy.update";
    pub const PROXY_DELETE: &str = "proxy.delete";
    pub const PROXY_CHECK: &str = "proxy.check";
    pub const PROXY_ASSIGN: &str = "proxy.assign";
    pub const PROXY_ALIGN: &str = "proxy.align";
    pub const FINGERPRINT_UPDATE: &str = "fingerprint.update";
    pub const FINGERPRINT_DETECT: &str = "fingerprint.detect";
    pub const SNAPSHOT_CREATE: &str = "snapshot.create";
    pub const SNAPSHOT_RESTORE: &str = "snapshot.restore";
    pub const SNAPSHOT_DELETE: &str = "snapshot.delete";
    pub const HOSTS_UPDATE: &str = "hosts.update";
    pub const MOCK_UPDATE: &str = "mock.update";
    pub const EXTENSION_ADD: &str = "extension.add";
    pub const EXTENSION_ASSIGN: &str = "extension.assign";
    pub const CAPTCHA_HANDLE: &str = "captcha.handle";
    pub const SETTINGS_UPDATE: &str = "settings.update";
    pub const THEME_UPDATE: &str = "theme.update";
    pub const USER_CREATE: &str = "user.create";
    pub const USER_UPDATE: &str = "user.update";
    pub const USER_DELETE: &str = "user.delete";
    pub const LOGIN: &str = "auth.login";
    pub const LOGIN_FAILED: &str = "auth.login_failed";
    pub const LOGOUT: &str = "auth.logout";
    pub const PASSWORD_CHANGE: &str = "auth.password_change";
    pub const API_KEY_CREATE: &str = "api_key.create";
    pub const API_KEY_DELETE: &str = "api_key.delete";
    pub const API_START: &str = "api.start";
    pub const API_STOP: &str = "api.stop";
    pub const BACKUP_CREATE: &str = "backup.create";
    pub const BACKUP_RESTORE: &str = "backup.restore";
    pub const SYNC_PUSH: &str = "sync.push";
    pub const SYNC_PULL: &str = "sync.pull";
    pub const HAR_EXPORT: &str = "har.export";
    pub const SESSION_RECONNECT: &str = "browser.reconnect";
    pub const API_REQUEST: &str = "api.request";
}
