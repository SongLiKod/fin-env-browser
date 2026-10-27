use crate::{
    audit_log::Audit,
    backup,
    error::Result,
    models::{new_id, now_iso},
    paths,
    state::SharedState,
};

/// The desktop binary cannot run code before it is replaced. Instead, on the
/// first launch of a new version we snapshot every environment's fingerprint,
/// create a local database backup, and record the version. This guarantees a
/// rollback point before the user edits any environment after an upgrade.
pub fn upgrade_backup(state: &SharedState) -> Result<Option<String>> {
    let current = env!("CARGO_PKG_VERSION");
    let last = state.db.get_meta("last_app_version")?;
    if last.as_deref() == Some(current) {
        return Ok(None);
    }
    let mut created_backup = None;
    if let Some(previous) = last.as_deref().filter(|v| !v.is_empty()) {
        let environments = state.db.list_environments(&Default::default())?;
        for mut env in environments {
            let snapshot = crate::models::FingerprintSnapshot {
                id: new_id(),
                env_id: env.id.clone(),
                name: format!("升级前自动快照 {previous} → {current}"),
                fingerprint: env.fingerprint.clone(),
                create_time: now_iso(),
            };
            if state.db.insert_snapshot(&snapshot).is_ok() {
                env.fingerprint_snapshot_id = Some(snapshot.id.clone());
                let _ = state.db.update_environment(&env);
            }
        }
        let bytes = backup::create_backup(&state.db, &state.crypto, &state.data_dir, false, None)?;
        let path = paths::backups_dir(&state.data_dir).join(format!(
            "upgrade-{current}-{}.finenvbackup",
            chrono::Utc::now().format("%Y%m%d-%H%M%S")
        ));
        std::fs::write(&path, &bytes)?;
        created_backup = Some(path.to_string_lossy().to_string());
        let _ = Audit::global(
            &state.db,
            "system",
            "backup.upgrade",
            format!(
                "检测到版本 {previous} → {current}，已创建升级前环境快照与数据库备份 {}",
                path.display()
            ),
        );
    }
    state.db.set_meta("last_app_version", current)?;
    Ok(created_backup)
}
