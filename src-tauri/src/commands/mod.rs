use std::path::{Path, PathBuf};

#[cfg(windows)]
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use tauri::State;

use crate::{
    audit_log::{actions, Audit},
    backup,
    capability::{self, EnvAccess, Permission},
    error::{AppError, Result},
    fingerprint, fingerprint_report,
    models::{
        now_iso, AccountExportRow, AccountInput, ApiKey, AuditFilter, CreateEnvironmentInput,
        CreatedApiKey, EnvironmentFilter, EnvironmentMember, EnvironmentMemberCandidate,
        EnvironmentMemberInput, GroupInput, HostsEntryInput, OpenOptions, ProxyInput, Settings,
        ThemeConfig, UpdateAccountInput, UpdateEnvironmentInput, UpdateProxyInput,
        UpdateSettingsInput, UserInput, UserUpdateInput,
    },
    package, paths, services,
    state::SharedState,
};

// ---------------------------------------------------------------------------
// Authentication & users
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn auth_status(state: State<'_, SharedState>) -> crate::models::SessionInfo {
    state.session_info()
}

#[tauri::command]
pub fn auth_login(
    state: State<'_, SharedState>,
    username: String,
    password: String,
) -> Result<crate::models::SessionInfo> {
    state.login(username.trim(), &password)
}

#[tauri::command]
pub fn auth_logout(state: State<'_, SharedState>) -> Result<()> {
    state.logout()
}

#[tauri::command]
pub fn auth_remember_load(
    state: State<'_, SharedState>,
) -> Result<Option<crate::models::RememberedLogin>> {
    let state = state.inner().clone();
    let Some(encrypted) = state.db.get_meta("remembered_login")? else {
        return Ok(None);
    };
    if encrypted.trim().is_empty() {
        return Ok(None);
    }
    let plain = state.crypto.decrypt(&encrypted)?;
    Ok(serde_json::from_str(&plain).ok())
}

#[tauri::command]
pub fn auth_remember_save(
    state: State<'_, SharedState>,
    username: String,
    password: String,
) -> Result<()> {
    let state = state.inner().clone();
    let payload = serde_json::to_string(&crate::models::RememberedLogin { username, password })?;
    let encrypted = state.crypto.encrypt(&payload)?;
    state.db.set_meta("remembered_login", &encrypted)?;
    Ok(())
}

#[tauri::command]
pub fn auth_remember_clear(state: State<'_, SharedState>) -> Result<()> {
    let state = state.inner().clone();
    state.db.set_meta("remembered_login", "")?;
    Ok(())
}

#[tauri::command]
pub fn auth_change_password(
    state: State<'_, SharedState>,
    old_password: String,
    new_password: String,
) -> Result<()> {
    state.change_password(&old_password, &new_password)
}

#[tauri::command]
pub fn user_list(state: State<'_, SharedState>) -> Result<Vec<crate::models::User>> {
    let state = state.inner().clone();
    state.require(Permission::UserManage)?;
    state.db.list_users()
}

#[tauri::command]
pub fn user_create(state: State<'_, SharedState>, input: UserInput) -> Result<crate::models::User> {
    let state = state.inner().clone();
    let actor = state.require(Permission::UserManage)?.username;
    services::create_user(&state, input, &actor)
}

#[tauri::command]
pub fn user_update(
    state: State<'_, SharedState>,
    id: String,
    input: UserUpdateInput,
) -> Result<crate::models::User> {
    let state = state.inner().clone();
    let actor = state.require(Permission::UserManage)?.username;
    services::update_user(&state, &id, input, &actor)
}

#[tauri::command]
pub fn user_delete(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::UserManage)?.username;
    services::delete_user(&state, &id, &actor)
}

#[tauri::command]
pub fn permission_matrix() -> Vec<crate::models::PermissionDef> {
    capability::permission_matrix()
}

// ---------------------------------------------------------------------------
// Environments
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn env_list(
    state: State<'_, SharedState>,
    filter: Option<EnvironmentFilter>,
) -> Result<Vec<crate::models::EnvironmentView>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    services::list_environments_visible(&state, &filter.unwrap_or_default(), &user)
}

#[tauri::command]
pub fn env_get(
    state: State<'_, SharedState>,
    id: String,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &id, EnvAccess::View)?;
    services::get_environment(&state, &id)
}

#[tauri::command]
pub fn template_list(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::models::EnvironmentTemplate>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    services::list_templates(&state)
}

#[tauri::command]
pub fn template_create_from_env(
    state: State<'_, SharedState>,
    input: crate::models::CreateTemplateFromEnvInput,
) -> Result<crate::models::EnvironmentTemplate> {
    let state = state.inner().clone();
    let user = state.require(Permission::CreateEnv)?;
    state.check_environment_access(&user, &input.env_id, EnvAccess::Operate)?;
    let actor = user.username;
    services::create_template_from_env(&state, input, &actor)
}

#[tauri::command]
pub fn template_update(
    state: State<'_, SharedState>,
    id: String,
    input: crate::models::UpdateTemplateInput,
) -> Result<crate::models::EnvironmentTemplate> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    services::update_template(&state, &id, input, &actor)
}

#[tauri::command]
pub fn template_delete(state: State<'_, SharedState>, id: String, confirm: bool) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "删除环境模板需要二次确认",
        ));
    }
    services::delete_template(&state, &id, &actor)
}

#[tauri::command]
pub fn template_batch_create(
    state: State<'_, SharedState>,
    input: crate::models::BatchCreateTemplateInput,
) -> Result<crate::models::BatchCreateResult> {
    let state = state.inner().clone();
    let user = state.require(Permission::CreateEnv)?;
    let actor = user.username.clone();
    let result = services::batch_create_from_template(&state, input, &actor)?;
    for env in &result.created {
        services::assign_environment_manager(&state, &env.id, &user.id)?;
    }
    Ok(result)
}

#[tauri::command]
pub async fn env_check_ip(
    state: State<'_, SharedState>,
    id: String,
) -> Result<crate::models::EnvironmentIpInfo> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &id, EnvAccess::View)?;
    let actor = user.username;
    services::check_environment_ip(&state, &id, &actor).await
}

#[tauri::command]
pub fn env_create(
    state: State<'_, SharedState>,
    input: CreateEnvironmentInput,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::CreateEnv)?;
    let env = services::create_environment(&state, input, &user.username)?;
    services::assign_environment_manager(&state, &env.id, &user.id)?;
    services::environment_view(&state, env)
}

#[tauri::command]
pub async fn env_update(
    state: State<'_, SharedState>,
    id: String,
    input: UpdateEnvironmentInput,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    state.check_environment_access(&user, &id, EnvAccess::Manage)?;
    let actor = user.username;
    let env = services::update_environment(&state, &id, input, &actor).await?;
    services::environment_view(&state, env)
}

#[tauri::command]
pub async fn env_delete(state: State<'_, SharedState>, id: String, confirm: bool) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::DeleteEnv)?;
    state.check_environment_access(&user, &id, EnvAccess::Manage)?;
    let actor = user.username;
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "删除环境需要二次确认",
        ));
    }
    services::delete_environment(&state, &id, &actor).await
}

#[tauri::command]
pub async fn env_bulk_delete(
    state: State<'_, SharedState>,
    ids: Vec<String>,
    confirm: bool,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::DeleteEnv)?;
    for id in &ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    let actor = user.username.clone();
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "批量删除需要二次确认",
        ));
    }
    services::bulk_delete_environments(&state, &ids, &actor).await
}

#[tauri::command]
pub fn env_clone(
    state: State<'_, SharedState>,
    id: String,
    name: Option<String>,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::CloneEnv)?;
    state.check_environment_access(&user, &id, EnvAccess::Operate)?;
    let env = services::clone_environment(&state, &id, name, &user.username)?;
    services::assign_environment_manager(&state, &env.id, &user.id)?;
    services::environment_view(&state, env)
}

#[tauri::command]
pub fn env_create_shortcut(state: State<'_, SharedState>, id: String) -> Result<String> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &id, EnvAccess::Operate)?;
    let env = state.db.get_environment(&id)?;
    #[cfg(windows)]
    {
        create_windows_shortcut(&env)
    }
    #[cfg(not(windows))]
    {
        let _ = env;
        Err(AppError::bad_request("桌面快捷方式目前仅支持 Windows"))
    }
}

#[cfg(windows)]
fn create_windows_shortcut(env: &crate::models::Environment) -> Result<String> {
    let exe = std::env::current_exe()
        .map_err(|error| AppError::internal(format!("无法定位 FinEnv 可执行文件: {error}")))?;
    let working_dir = exe
        .parent()
        .map(|path| path.to_string_lossy().to_string())
        .unwrap_or_default();
    let safe_name: String = env
        .name
        .chars()
        .map(|value| {
            if matches!(value, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') {
                '_'
            } else {
                value
            }
        })
        .collect();
    let safe_name = safe_name
        .trim()
        .trim_matches('.')
        .chars()
        .take(60)
        .collect::<String>();
    let shortcut_name = format!("FinEnv - {safe_name}.lnk");
    let quote = |value: &str| value.replace('\'', "''");
    let script = format!(
        "$desktop = [Environment]::GetFolderPath('Desktop');\
         $path = Join-Path $desktop '{shortcut}';\
         $shell = New-Object -ComObject WScript.Shell;\
         $link = $shell.CreateShortcut($path);\
         $link.TargetPath = '{exe}';\
         $link.Arguments = '--env={env_id}';\
         $link.WorkingDirectory = '{work}';\
         $link.IconLocation = '{exe},0';\
         $link.Description = 'FinEnv 环境: {name}';\
         $link.Save();\
         Write-Output $path",
        shortcut = quote(&shortcut_name),
        exe = quote(&exe.to_string_lossy()),
        env_id = quote(&env.id),
        work = quote(&working_dir),
        name = quote(&env.name),
    );
    let encoded: Vec<u8> = script
        .encode_utf16()
        .flat_map(|unit| unit.to_le_bytes())
        .collect();
    let output = std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-EncodedCommand",
            &B64.encode(encoded),
        ])
        .output()
        .map_err(|error| AppError::internal(format!("创建桌面快捷方式失败: {error}")))?;
    if !output.status.success() {
        return Err(AppError::internal(format!(
            "创建桌面快捷方式失败: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() {
        return Err(AppError::internal("创建桌面快捷方式失败: 未返回路径"));
    }
    Ok(path)
}

pub fn startup_env_from(args: &[String]) -> Option<String> {
    for (index, arg) in args.iter().enumerate() {
        if let Some(value) = arg.strip_prefix("--env=") {
            if !value.trim().is_empty() {
                return Some(value.to_string());
            }
        }
        if arg == "--env" {
            if let Some(value) = args.get(index + 1) {
                if !value.trim().is_empty() {
                    return Some(value.to_string());
                }
            }
        }
    }
    None
}

#[tauri::command]
pub fn startup_env() -> Option<String> {
    startup_env_from(&std::env::args().collect::<Vec<_>>())
}

#[tauri::command]
pub fn env_export(
    state: State<'_, SharedState>,
    ids: Vec<String>,
    path: String,
    passphrase: String,
    include_profiles: bool,
    confirm: bool,
) -> Result<usize> {
    let state = state.inner().clone();
    let user = state.require_sensitive(Permission::ExportSensitive, confirm)?;
    for id in &ids {
        state.check_environment_access(&user, id, EnvAccess::View)?;
    }
    let actor = user.username;
    let envs: Vec<_> = ids
        .iter()
        .map(|id| state.db.get_environment(id))
        .collect::<Result<Vec<_>>>()?;
    let count = package::export_environments(
        &state.db,
        &state.crypto,
        &state.data_dir,
        &ids,
        Path::new(&path),
        &passphrase,
        include_profiles,
    )?;
    for env in envs {
        Audit::env(
            &state.db,
            &actor,
            actions::ENV_EXPORT,
            &env,
            format!(
                "导出加密环境包（包含 profile: {}）至 {}",
                include_profiles, path
            ),
        )?;
    }
    Ok(count)
}

#[tauri::command]
pub fn env_import(
    state: State<'_, SharedState>,
    path: String,
    passphrase: String,
) -> Result<Vec<crate::models::EnvironmentView>> {
    let state = state.inner().clone();
    let user = state.require(Permission::CreateEnv)?;
    let envs = package::import_environments(
        &state.db,
        &state.crypto,
        &state.data_dir,
        Path::new(&path),
        &passphrase,
    )?;
    for env in &envs {
        services::assign_environment_manager(&state, &env.id, &user.id)?;
        Audit::env(
            &state.db,
            &user.username,
            actions::ENV_IMPORT,
            env,
            format!("从加密环境包 {} 导入", path),
        )?;
    }
    envs.into_iter()
        .map(|env| services::environment_view(&state, env))
        .collect()
}

#[tauri::command]
pub fn env_bulk_import(
    state: State<'_, SharedState>,
    paths: Vec<String>,
    passphrase: String,
) -> Result<Vec<crate::models::EnvironmentView>> {
    let mut all = Vec::new();
    for path in paths {
        let state = state.inner().clone();
        let user = state.require(Permission::CreateEnv)?;
        let envs = package::import_environments(
            &state.db,
            &state.crypto,
            &state.data_dir,
            Path::new(&path),
            &passphrase,
        )?;
        for env in &envs {
            services::assign_environment_manager(&state, &env.id, &user.id)?;
            Audit::env(
                &state.db,
                &user.username,
                actions::ENV_IMPORT,
                env,
                format!("批量导入加密环境包 {}", path),
            )?;
        }
        for env in envs {
            all.push(services::environment_view(&state, env)?);
        }
    }
    Ok(all)
}

#[tauri::command]
pub fn env_projects(state: State<'_, SharedState>) -> Result<Vec<String>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    state.db.environment_projects()
}

#[tauri::command]
pub fn env_tags(state: State<'_, SharedState>) -> Result<Vec<String>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    state.db.all_tags()
}

#[tauri::command]
pub fn env_member_list(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<EnvironmentMember>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.list_environment_members(&env_id)
}

#[tauri::command]
pub fn env_member_candidates(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<EnvironmentMemberCandidate>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ManageEnvMembers)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    state.db.list_environment_member_candidates(&env_id)
}

#[tauri::command]
pub fn env_member_set(
    state: State<'_, SharedState>,
    env_id: String,
    input: EnvironmentMemberInput,
) -> Result<Vec<EnvironmentMember>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ManageEnvMembers)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    if !capability::valid_env_access(&input.access_level) {
        return Err(AppError::bad_request(
            "访问级别需为 view / operate / manage",
        ));
    }
    let target = state.db.get_user(&input.user_id)?;
    if target.id == user.id {
        return Err(AppError::bad_request("不能修改自己的环境访问级别"));
    }
    state
        .db
        .upsert_environment_member(&env_id, &input.user_id, &input.access_level)?;
    Audit::global(
        &state.db,
        &user.username,
        actions::ENV_MEMBER_SET,
        format!(
            "设置环境成员：{}（{}）为 {}",
            target.display_name, target.username, input.access_level
        ),
    )?;
    state.db.list_environment_members(&env_id)
}

#[tauri::command]
pub fn env_member_remove(
    state: State<'_, SharedState>,
    env_id: String,
    user_id: String,
) -> Result<Vec<EnvironmentMember>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ManageEnvMembers)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    if user_id == user.id {
        return Err(AppError::bad_request("不能移除自己的环境访问权限"));
    }
    let target = state.db.get_user(&user_id)?;
    state.db.remove_environment_member(&env_id, &user_id)?;
    Audit::global(
        &state.db,
        &user.username,
        actions::ENV_MEMBER_REMOVE,
        format!(
            "移除环境成员：{}（{}）",
            target.display_name, target.username
        ),
    )?;
    state.db.list_environment_members(&env_id)
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn group_list(state: State<'_, SharedState>) -> Result<Vec<crate::models::Group>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    state.db.list_groups()
}

#[tauri::command]
pub fn group_create(
    state: State<'_, SharedState>,
    input: GroupInput,
) -> Result<crate::models::Group> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    services::ensure_group(&state, input, &actor)
}

#[tauri::command]
pub fn group_update(state: State<'_, SharedState>, id: String, input: GroupInput) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    if input.name.trim().is_empty() {
        return Err(AppError::bad_request("分组名称不能为空"));
    }
    state.db.update_group(
        &id,
        input.name.trim(),
        input.color.as_deref().unwrap_or("#165DFF"),
    )?;
    Audit::global(
        &state.db,
        &actor,
        "group.update",
        format!("更新分组 {}", input.name),
    )?;
    Ok(())
}

#[tauri::command]
pub fn group_delete(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    state.db.delete_group(&id)?;
    Audit::global(&state.db, &actor, "group.delete", format!("删除分组 {id}"))?;
    Ok(())
}

#[tauri::command]
pub fn env_assign_group(
    state: State<'_, SharedState>,
    ids: Vec<String>,
    group_id: Option<String>,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    for id in &ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    let actor = user.username.clone();
    let count = state.db.set_environment_group(&ids, group_id.as_deref())? as u32;
    Audit::global(
        &state.db,
        &actor,
        "env.assign_group",
        format!("批量调整分组，影响 {count} 个环境"),
    )?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn account_list(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::Account>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.list_accounts(&env_id)
}

#[tauri::command]
pub fn account_create(
    state: State<'_, SharedState>,
    input: AccountInput,
) -> Result<crate::models::Account> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    state.check_environment_access(&user, &input.env_id, EnvAccess::Operate)?;
    services::create_account(&state, input, &user.username)
}

#[tauri::command]
pub fn account_update(
    state: State<'_, SharedState>,
    id: String,
    input: UpdateAccountInput,
) -> Result<crate::models::Account> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    let account = state.db.get_account(&id)?;
    state.check_environment_access(&user, &account.env_id, EnvAccess::Operate)?;
    services::update_account(&state, &id, input, &user.username)
}

#[tauri::command]
pub fn account_delete(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    let account = state.db.get_account(&id)?;
    state.check_environment_access(&user, &account.env_id, EnvAccess::Operate)?;
    services::delete_account(&state, &id, &user.username)
}

#[tauri::command]
pub fn account_reveal(state: State<'_, SharedState>, id: String, confirm: bool) -> Result<String> {
    let state = state.inner().clone();
    let user = state.require_sensitive(Permission::ExportSensitive, confirm)?;
    let account = state.db.get_account(&id)?;
    state.check_environment_access(&user, &account.env_id, EnvAccess::View)?;
    services::reveal_password(&state, &id, &user.username)
}

#[tauri::command]
pub fn account_import(
    state: State<'_, SharedState>,
    env_id: String,
    rows: Vec<AccountExportRow>,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    services::import_accounts(&state, &env_id, &rows, &user.username)
}

#[tauri::command]
pub fn account_import_file(
    state: State<'_, SharedState>,
    env_id: String,
    path: String,
    passphrase: String,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let count = package::import_accounts_file(
        &state.db,
        &state.crypto,
        &env_id,
        Path::new(&path),
        &passphrase,
    )?;
    let env = state.db.get_environment(&env_id)?;
    Audit::env(
        &state.db,
        &user.username,
        actions::ACCOUNT_IMPORT,
        &env,
        format!("从文件导入账号 {count} 条"),
    )?;
    Ok(count)
}

#[tauri::command]
pub fn account_export(
    state: State<'_, SharedState>,
    env_ids: Vec<String>,
    path: String,
    passphrase: String,
    confirm: bool,
) -> Result<usize> {
    let state = state.inner().clone();
    let user = state.require_sensitive(Permission::ExportSensitive, confirm)?;
    for env_id in &env_ids {
        state.check_environment_access(&user, env_id, EnvAccess::View)?;
    }
    let actor = user.username;
    let count = package::export_accounts(
        &state.db,
        &state.crypto,
        &env_ids,
        Path::new(&path),
        &passphrase,
    )?;
    for env_id in &env_ids {
        if let Ok(env) = state.db.get_environment(env_id) {
            Audit::env(
                &state.db,
                &actor,
                actions::ACCOUNT_EXPORT,
                &env,
                format!("导出账号 {count} 条至加密包 {}", path),
            )?;
        }
    }
    Ok(count)
}

// ---------------------------------------------------------------------------
// Fingerprint & snapshots
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn fingerprint_template(operating_system: Option<String>) -> fingerprint::Fingerprint {
    fingerprint::template(operating_system.as_deref().unwrap_or("win11"))
}

#[tauri::command]
pub fn fingerprint_validate(fp: fingerprint::Fingerprint) -> fingerprint::FingerprintValidation {
    fingerprint::validate(&fp)
}

#[tauri::command]
pub async fn fingerprint_update(
    state: State<'_, SharedState>,
    env_id: String,
    fp: fingerprint::Fingerprint,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    let env = services::update_fingerprint(&state, &env_id, fp, &user.username)?;
    services::apply_fingerprint_if_running(&state, &env).await?;
    services::environment_view(&state, env)
}

#[tauri::command]
pub async fn fingerprint_detect(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<crate::models::FingerprintReport> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let actor = user.username;
    let env = state.db.get_environment(&env_id)?;
    if !state.browsers.is_running(&env_id) {
        state
            .browsers
            .open(&env, &OpenOptions::default(), &actor)
            .await?;
    }
    let settings = state.settings();
    fingerprint_report::detect(
        &state.db,
        &state.browsers,
        &state.data_dir,
        &env,
        &settings.detection_service_url,
        &actor,
    )
    .await
}

#[tauri::command]
pub fn fingerprint_reports(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::FingerprintReport>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.list_fingerprint_reports(&env_id, 20)
}

#[tauri::command]
pub fn snapshot_list(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::FingerprintSnapshot>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.list_snapshots(&env_id)
}

#[tauri::command]
pub fn snapshot_create(
    state: State<'_, SharedState>,
    env_id: String,
    name: Option<String>,
) -> Result<crate::models::FingerprintSnapshot> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    services::create_snapshot(&state, &env_id, name, &user.username)
}

#[tauri::command]
pub async fn snapshot_restore(
    state: State<'_, SharedState>,
    id: String,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    let snapshot = state.db.get_snapshot(&id)?;
    state.check_environment_access(&user, &snapshot.env_id, EnvAccess::Manage)?;
    let env = services::restore_snapshot(&state, &id, &user.username).await?;
    services::environment_view(&state, env)
}

#[tauri::command]
pub fn snapshot_delete(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    let snapshot = state.db.get_snapshot(&id)?;
    state.check_environment_access(&user, &snapshot.env_id, EnvAccess::Manage)?;
    state.db.delete_snapshot(&id)?;
    if let Ok(env) = state.db.get_environment(&snapshot.env_id) {
        Audit::env(
            &state.db,
            &user.username,
            actions::SNAPSHOT_DELETE,
            &env,
            format!("删除快照 {} ({})", snapshot.name, id),
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Proxies
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn proxy_list(state: State<'_, SharedState>) -> Result<Vec<crate::models::Proxy>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    state.db.list_proxies()
}

#[tauri::command]
pub fn proxy_create(
    state: State<'_, SharedState>,
    input: ProxyInput,
) -> Result<crate::models::Proxy> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditFingerprintProxy)?.username;
    services::create_proxy(&state, input, &actor)
}

#[tauri::command]
pub fn proxy_update(
    state: State<'_, SharedState>,
    id: String,
    input: UpdateProxyInput,
) -> Result<crate::models::Proxy> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditFingerprintProxy)?.username;
    services::update_proxy(&state, &id, input, &actor)
}

#[tauri::command]
pub fn proxy_delete(state: State<'_, SharedState>, id: String, confirm: bool) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditFingerprintProxy)?.username;
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "删除代理需要二次确认",
        ));
    }
    services::delete_proxy(&state, &id, &actor)
}

#[tauri::command]
pub async fn proxy_check(
    state: State<'_, SharedState>,
    id: String,
) -> Result<crate::models::ProxyCheckResult> {
    let state = state.inner().clone();
    let actor = state.require(Permission::ViewList)?.username;
    services::check_proxy(&state, &id, &actor).await
}

#[tauri::command]
pub fn proxy_assign(
    state: State<'_, SharedState>,
    env_ids: Vec<String>,
    proxy_id: Option<String>,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    for id in &env_ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    services::assign_proxy(&state, &env_ids, proxy_id.as_deref(), &user.username)
}

#[tauri::command]
pub async fn proxy_align(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<crate::models::AlignProxyResult> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    services::align_proxy(&state, &env_id, &user.username).await
}

// ---------------------------------------------------------------------------
// Browser runtime
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn browser_open(
    state: State<'_, SharedState>,
    env_id: String,
    options: Option<OpenOptions>,
) -> Result<crate::models::BrowserRuntime> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let actor = user.username;
    let env = state.db.get_environment(&env_id)?;
    let runtime = state
        .browsers
        .open(&env, &options.unwrap_or_default(), &actor)
        .await?;
    Ok(runtime)
}

#[tauri::command]
pub async fn browser_close(state: State<'_, SharedState>, env_id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let actor = user.username;
    state
        .browsers
        .close(&env_id, &actor, "用户关闭环境", actions::ENV_CLOSE)
        .await
}

#[tauri::command]
pub fn browser_status(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Option<crate::models::BrowserRuntime>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    Ok(state.browsers.get_runtime(&env_id))
}

#[tauri::command]
pub fn browser_list_running(
    state: State<'_, SharedState>,
) -> Result<Vec<crate::models::BrowserRuntime>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    let allowed = state.accessible_environment_ids(&user)?;
    let runtimes = state.browsers.list_runtimes();
    Ok(match allowed {
        Some(ids) => runtimes
            .into_iter()
            .filter(|runtime| ids.contains(&runtime.env_id))
            .collect(),
        None => runtimes,
    })
}

#[tauri::command]
pub async fn browser_screenshot(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<crate::models::ScreenshotResult> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.screenshot(&env_id, &user.username).await
}

#[tauri::command]
pub async fn browser_cookies_get(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::CookieRecord>> {
    let state = state.inner().clone();
    let user = state.require_export()?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    let actor = user.username;
    if state.browsers.is_running(&env_id) {
        state.browsers.cookies_get(&env_id, &actor, true).await
    } else {
        let cookies = state.browsers.cookies_get_cached(&env_id).await?;
        if let Ok(env) = state.db.get_environment(&env_id) {
            Audit::env(
                &state.db,
                &actor,
                actions::COOKIE_READ,
                &env,
                format!("读取已加密 Cookie 缓存 {} 条（环境未运行）", cookies.len()),
            )?;
        }
        Ok(cookies)
    }
}

#[tauri::command]
pub async fn browser_cookies_set(
    state: State<'_, SharedState>,
    env_id: String,
    cookies: Vec<crate::models::CookieRecord>,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require_export()?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state
        .browsers
        .cookies_set(&env_id, &cookies, &user.username)
        .await
}

#[tauri::command]
pub async fn browser_execute(
    state: State<'_, SharedState>,
    env_id: String,
    script: String,
) -> Result<Value> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.execute(&env_id, &script).await
}

#[tauri::command]
pub async fn browser_navigate(
    state: State<'_, SharedState>,
    env_id: String,
    url: String,
) -> Result<String> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.navigate(&env_id, &url).await
}

#[tauri::command]
pub async fn browser_reload(state: State<'_, SharedState>, env_id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.reload(&env_id).await
}

#[tauri::command]
pub async fn broadcast_start(
    state: State<'_, SharedState>,
    input: crate::models::BroadcastStartInput,
) -> Result<crate::models::BroadcastStatus> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    for env_id in &input.env_ids {
        state.check_environment_access(&user, env_id, EnvAccess::Operate)?;
    }
    state.browsers.broadcast_start(input, &user.username).await
}

#[tauri::command]
pub async fn broadcast_status(
    state: State<'_, SharedState>,
) -> Result<crate::models::BroadcastStatus> {
    let state = state.inner().clone();
    state.require(Permission::OpenClose)?;
    Ok(state.browsers.broadcast_status().await)
}

#[tauri::command]
pub async fn broadcast_pause(
    state: State<'_, SharedState>,
    paused: bool,
) -> Result<crate::models::BroadcastStatus> {
    let state = state.inner().clone();
    let actor = state.require(Permission::OpenClose)?.username;
    state.browsers.broadcast_pause(paused, &actor).await
}

#[tauri::command]
pub async fn broadcast_stop(
    state: State<'_, SharedState>,
    reason: Option<String>,
) -> Result<crate::models::BroadcastStatus> {
    let state = state.inner().clone();
    let actor = state.require(Permission::OpenClose)?.username;
    state
        .browsers
        .broadcast_stop(&actor, reason.as_deref().unwrap_or("用户停止"))
        .await
}

#[tauri::command]
pub fn browser_cdp_info(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<crate::models::CdpInfo> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.cdp_info(&env_id)
}

#[tauri::command]
pub async fn browser_recording_start(
    state: State<'_, SharedState>,
    env_id: String,
    interval_ms: Option<u64>,
) -> Result<crate::models::RecordingResult> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let actor = user.username;
    state
        .browsers
        .start_recording(&env_id, interval_ms.unwrap_or(1000), &actor)
        .await
}

#[tauri::command]
pub async fn browser_recording_stop(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<crate::models::RecordingResult> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.stop_recording(&env_id, &user.username).await
}

#[tauri::command]
pub async fn browser_har_start(state: State<'_, SharedState>, env_id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.start_har(&env_id, &user.username).await
}

#[tauri::command]
pub async fn browser_har_stop(state: State<'_, SharedState>, env_id: String) -> Result<String> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.stop_har(&env_id, &user.username).await
}

#[tauri::command]
pub async fn browser_force_recycle_all(state: State<'_, SharedState>) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    let allowed = state.accessible_environment_ids(&user)?;
    let Some(allowed) = allowed else {
        return state.browsers.force_recycle_all(&user.username).await;
    };
    let ids = state
        .browsers
        .list_runtimes()
        .into_iter()
        .filter(|runtime| allowed.contains(&runtime.env_id))
        .map(|runtime| runtime.env_id)
        .collect::<Vec<_>>();
    let count = ids.len() as u32;
    for id in ids {
        let _ = state
            .browsers
            .close(
                &id,
                &user.username,
                "回收可访问的运行环境",
                actions::ENV_RECYCLE,
            )
            .await;
    }
    Ok(count)
}

#[tauri::command]
pub async fn browser_switch_proxy(
    state: State<'_, SharedState>,
    env_id: String,
    proxy_id: Option<String>,
    restart: bool,
) -> Result<crate::models::EnvironmentView> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditFingerprintProxy)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Manage)?;
    let running = state.browsers.is_running(&env_id);
    services::assign_proxy(
        &state,
        std::slice::from_ref(&env_id),
        proxy_id.as_deref(),
        &user.username,
    )?;
    if running && restart {
        state
            .browsers
            .close(
                &env_id,
                &user.username,
                "切换代理，重启浏览器",
                actions::ENV_CLOSE,
            )
            .await?;
        let env = state.db.get_environment(&env_id)?;
        state
            .browsers
            .open(&env, &OpenOptions::default(), &user.username)
            .await?;
    }
    let env = state.db.get_environment(&env_id)?;
    services::environment_view(&state, env)
}

#[tauri::command]
pub async fn resource_snapshot(
    state: State<'_, SharedState>,
) -> Result<crate::models::ResourceSnapshot> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    Ok(state.browsers.resource_snapshot().await)
}

// ---------------------------------------------------------------------------
// Hosts / Mock / Extensions / Captcha
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn hosts_list(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::HostsEntry>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.list_hosts(&env_id)
}

#[tauri::command]
pub fn hosts_set(
    state: State<'_, SharedState>,
    env_id: String,
    entries: Vec<HostsEntryInput>,
) -> Result<Vec<crate::models::HostsEntry>> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    services::set_hosts(&state, &env_id, &entries, &user.username)
}

#[tauri::command]
pub fn mock_list(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::MockRule>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.list_mocks(&env_id)
}

#[tauri::command]
pub async fn mock_set(
    state: State<'_, SharedState>,
    env_id: String,
    rules: Vec<crate::models::MockRuleInput>,
) -> Result<Vec<crate::models::MockRule>> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    services::set_mocks(&state, &env_id, &rules, &user.username).await
}

#[tauri::command]
pub fn extension_list(state: State<'_, SharedState>) -> Result<Vec<crate::models::Extension>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    state.db.list_extensions()
}

#[tauri::command]
pub fn extension_add(
    state: State<'_, SharedState>,
    name: Option<String>,
    path: String,
) -> Result<crate::models::Extension> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    services::add_extension(&state, name, &path, &actor)
}

#[tauri::command]
pub fn extension_install_archive(
    state: State<'_, SharedState>,
    path: String,
    name: Option<String>,
    env_ids: Option<Vec<String>>,
) -> Result<crate::models::Extension> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    let env_ids = env_ids.unwrap_or_default();
    for id in &env_ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    services::install_extension_archive(&state, &path, name, &env_ids, &user.username)
}

#[tauri::command]
pub fn extension_delete(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    let ext = state.db.get_extension(&id)?;
    state.db.delete_extension(&id)?;
    let path = PathBuf::from(&ext.path);
    if path.starts_with(paths::extensions_dir(&state.data_dir)) {
        let _ = std::fs::remove_dir_all(path);
    }
    Audit::global(
        &state.db,
        &actor,
        "extension.delete",
        format!("删除扩展 {}", ext.name),
    )?;
    Ok(())
}

#[tauri::command]
pub fn extension_assign(
    state: State<'_, SharedState>,
    extension_id: String,
    env_ids: Vec<String>,
) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    for id in &env_ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    let actor = user.username.clone();
    let ext = state.db.get_extension(&extension_id)?;
    state.db.assign_extension(&extension_id, &env_ids)?;
    Audit::global(
        &state.db,
        &actor,
        actions::EXTENSION_ASSIGN,
        format!(
            "扩展 {} 分配给 {} 个环境（需重启生效）",
            ext.name,
            env_ids.len()
        ),
    )?;
    Ok(())
}

#[tauri::command]
pub fn extension_unassign(
    state: State<'_, SharedState>,
    extension_id: String,
    env_ids: Vec<String>,
) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    for id in &env_ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    let ext = state.db.get_extension(&extension_id)?;
    state.db.unassign_extension(&extension_id, &env_ids)?;
    Audit::global(
        &state.db,
        &user.username,
        "extension.unassign",
        format!("取消扩展 {} 在 {} 个环境的分配", ext.name, env_ids.len()),
    )?;
    Ok(())
}

#[tauri::command]
pub async fn extension_install_online(
    state: State<'_, SharedState>,
    source: String,
    env_ids: Option<Vec<String>>,
) -> Result<crate::models::Extension> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    let env_ids = env_ids.unwrap_or_default();
    for id in &env_ids {
        state.check_environment_access(&user, id, EnvAccess::Manage)?;
    }
    services::install_extension_online(&state, &source, &env_ids, &user.username).await
}

#[tauri::command]
pub fn extension_update(
    state: State<'_, SharedState>,
    id: String,
    input: crate::models::UpdateExtensionInput,
) -> Result<crate::models::Extension> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    services::update_extension(&state, &id, input, &actor)
}

#[tauri::command]
pub fn extension_assign_all(state: State<'_, SharedState>, id: String) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::EditEnv)?;
    let allowed = state.accessible_environment_ids(&user)?;
    let Some(allowed) = allowed else {
        return services::assign_extension_to_all(&state, &id, &user.username);
    };
    let envs = state.db.list_environments(&Default::default())?;
    let mut ids = Vec::new();
    for env in envs {
        if allowed.contains(&env.id) {
            state.check_environment_access(&user, &env.id, EnvAccess::Manage)?;
            ids.push(env.id);
        }
    }
    let extension = state.db.get_extension(&id)?;
    state.db.assign_extension(&id, &ids)?;
    Audit::global(
        &state.db,
        &user.username,
        actions::EXTENSION_ASSIGN,
        format!(
            "扩展 {} 应用到可访问的 {} 个环境",
            extension.name,
            ids.len()
        ),
    )?;
    Ok(ids.len() as u32)
}

#[tauri::command]
pub fn extension_open_folder(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::EditEnv)?.username;
    let extension = state.db.get_extension(&id)?;
    let path = std::path::PathBuf::from(&extension.path);
    if !path.is_dir() {
        return Err(AppError::not_found("扩展目录不存在"));
    }
    open_folder(&path)?;
    let _ = Audit::global(
        &state.db,
        &actor,
        actions::ARTIFACT_OPEN_DIR,
        format!("打开扩展目录: {}", path.display()),
    );
    Ok(())
}

#[tauri::command]
pub async fn extension_open_store(
    state: State<'_, SharedState>,
    env_id: String,
    source: String,
) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let url = services::store_extension_url(&source)?;
    let env = state.db.get_environment(&env_id)?;
    if state.browsers.is_running(&env_id) {
        state.browsers.navigate(&env_id, &url).await?;
    } else {
        let options = crate::models::OpenOptions {
            url: Some(url),
            ..Default::default()
        };
        state.browsers.open(&env, &options, &user.username).await?;
    }
    Ok(())
}

#[tauri::command]
pub fn env_extensions(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::Extension>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    state.db.extensions_for_env(&env_id)
}

#[tauri::command]
pub fn captcha_list(
    state: State<'_, SharedState>,
    env_id: Option<String>,
) -> Result<Vec<crate::models::CaptchaEvent>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    if let Some(env_id) = env_id.as_deref() {
        state.check_environment_access(&user, env_id, EnvAccess::View)?;
    }
    let events = state.db.list_captcha(env_id.as_deref(), 200)?;
    if env_id.is_some() {
        return Ok(events);
    }
    let allowed = state.accessible_environment_ids(&user)?;
    Ok(match allowed {
        Some(ids) => events
            .into_iter()
            .filter(|event| ids.contains(&event.env_id))
            .collect(),
        None => events,
    })
}

#[tauri::command]
pub fn captcha_handle(state: State<'_, SharedState>, id: String, handled: bool) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    let event = state.db.get_captcha(&id)?;
    state.check_environment_access(&user, &event.env_id, EnvAccess::View)?;
    let actor = user.username;
    state.db.mark_captcha_handled(&id, handled)?;
    Audit::global(
        &state.db,
        &actor,
        actions::CAPTCHA_HANDLE,
        format!(
            "标记验证码事件 {} 为 {}",
            id,
            if handled { "已处理" } else { "未处理" }
        ),
    )?;
    Ok(())
}

#[tauri::command]
pub fn captcha_handle_all(
    state: State<'_, SharedState>,
    ids: Vec<String>,
    handled: bool,
) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    let mut count = 0;
    for id in ids {
        let event = state.db.get_captcha(&id)?;
        state.check_environment_access(&user, &event.env_id, EnvAccess::View)?;
        state.db.mark_captcha_handled(&id, handled)?;
        count += 1;
    }
    if count > 0 {
        let _ = Audit::global(
            &state.db,
            &user.username,
            actions::CAPTCHA_HANDLE,
            format!(
                "{} {} 条验证码事件",
                if handled {
                    "标记已处理"
                } else {
                    "标记未处理"
                },
                count
            ),
        );
    }
    Ok(count)
}

#[tauri::command]
pub fn captcha_delete(state: State<'_, SharedState>, ids: Vec<String>) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    let screenshots = state.data_dir.join("screenshots");
    let canonical_screenshots = screenshots.canonicalize().ok();
    let mut count = 0;
    for id in ids {
        let event = state.db.get_captcha(&id)?;
        state.check_environment_access(&user, &event.env_id, EnvAccess::Operate)?;
        if let Some(path) = event.screenshot_path.as_deref() {
            if let Some(root) = &canonical_screenshots {
                if let Ok(file) = PathBuf::from(path).canonicalize() {
                    if file.starts_with(root) {
                        let _ = std::fs::remove_file(&file);
                    }
                }
            }
        }
        state.db.delete_captcha(&id)?;
        count += 1;
    }
    if count > 0 {
        let _ = Audit::global(
            &state.db,
            &user.username,
            actions::CAPTCHA_DELETE,
            format!("删除 {} 条验证码事件及截图", count),
        );
    }
    Ok(count)
}

#[tauri::command]
pub async fn captcha_capture(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Option<String>> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let actor = user.username;
    let path = state.browsers.capture_captcha_screenshot(&env_id).await?;
    state
        .db
        .insert_captcha(&env_id, "", "manual", "手动采集验证码", path.as_deref())?;
    Audit::global(
        &state.db,
        &actor,
        actions::CAPTCHA_HANDLE,
        format!("手动采集验证码截图：{env_id}"),
    )?;
    Ok(path)
}

fn open_folder(dir: &Path) -> Result<()> {
    #[cfg(target_os = "windows")]
    let opened = std::process::Command::new("explorer").arg(dir).spawn();
    #[cfg(target_os = "macos")]
    let opened = std::process::Command::new("open").arg(dir).spawn();
    #[cfg(all(unix, not(target_os = "macos")))]
    let opened = std::process::Command::new("xdg-open").arg(dir).spawn();
    #[cfg(not(any(target_os = "windows", target_os = "macos", unix)))]
    let opened: std::io::Result<std::process::Child> = Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "当前平台不支持打开目录",
    ));
    opened.map_err(|e| AppError::internal(format!("无法打开目录 {}: {e}", dir.display())))?;
    Ok(())
}

/// Open one of the application's artifact directories in the OS file manager.
/// The path is resolved from the data directory, never from the frontend.
#[tauri::command]
pub fn open_artifact_dir(state: State<'_, SharedState>, kind: String) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::ViewList)?.username;
    let dir = match kind.as_str() {
        "screenshots" => paths::screenshots_dir(&state.data_dir),
        "recordings" => paths::recordings_dir(&state.data_dir),
        "har" => paths::har_dir(&state.data_dir),
        "exports" => paths::exports_dir(&state.data_dir),
        "backups" => paths::backups_dir(&state.data_dir),
        "reports" => paths::reports_dir(&state.data_dir),
        "downloads" => paths::downloads_dir(&state.data_dir),
        other => return Err(AppError::bad_request(format!("不支持的目录类型: {other}"))),
    };
    paths::ensure_dir(&dir)?;
    open_folder(&dir)?;
    let _ = Audit::global(
        &state.db,
        &actor,
        actions::ARTIFACT_OPEN_DIR,
        format!("打开产物目录 {}: {}", kind, dir.display()),
    );
    Ok(())
}

/// Read a PNG/JPEG artifact (screenshot, captcha, detection report) as a data
/// URL. The path is restricted to the application data directory to prevent
/// this command from becoming an arbitrary file read primitive.
#[tauri::command]
pub fn read_artifact(state: State<'_, SharedState>, path: String) -> Result<String> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    let requested = PathBuf::from(&path);
    let canonical = requested
        .canonicalize()
        .map_err(|_| AppError::not_found("文件不存在"))?;
    let root = state
        .data_dir
        .canonicalize()
        .map_err(|_| AppError::internal("数据目录不可用"))?;
    if !canonical.starts_with(&root) {
        return Err(AppError::forbidden("只允许读取 FinEnv 数据目录内的文件"));
    }
    let meta = std::fs::metadata(&canonical)?;
    if meta.len() > 12 * 1024 * 1024 {
        return Err(AppError::bad_request("文件超过 12MB，无法在面板内预览"));
    }
    let mime = match canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        _ => return Err(AppError::bad_request("仅支持预览 PNG/JPEG 文件")),
    };
    let bytes = std::fs::read(canonical)?;
    let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
    Ok(format!("data:{mime};base64,{encoded}"))
}

// ---------------------------------------------------------------------------
// Settings, theme, API, audit
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn settings_get(state: State<'_, SharedState>) -> Result<Settings> {
    let state = state.inner().clone();
    state.require(Permission::SettingsManage)?;
    Ok(state.settings())
}

#[tauri::command]
pub async fn settings_update(
    state: State<'_, SharedState>,
    input: UpdateSettingsInput,
) -> Result<Settings> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    services::update_settings(&state, input, &actor).await
}

#[tauri::command]
pub fn plugin_list(state: State<'_, SharedState>) -> Result<Vec<crate::models::PluginInfo>> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    Ok(crate::plugins::list(&state.settings()))
}

#[tauri::command]
pub fn plugin_set_enabled(
    state: State<'_, SharedState>,
    id: String,
    enabled: bool,
) -> Result<Vec<crate::models::PluginInfo>> {
    let state = state.inner().clone();
    let user = state.require(Permission::SettingsManage)?;
    if !crate::plugins::is_valid(&id) {
        return Err(AppError::not_found("插件不存在"));
    }
    let mut settings = state.settings();
    settings.enabled_plugins.retain(|item| item != &id);
    if enabled {
        settings.enabled_plugins.push(id.clone());
        settings.enabled_plugins.sort();
        settings.enabled_plugins.dedup();
    }
    state.store_settings(&settings)?;
    let _ = Audit::global(
        &state.db,
        &user.username,
        actions::PLUGIN_UPDATE,
        format!("{} 插件 {}", if enabled { "启用" } else { "暂停" }, id),
    );
    Ok(crate::plugins::list(&settings))
}

#[tauri::command]
pub fn plugin_env_state(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::PluginInfo>> {
    let state = state.inner().clone();
    let user = state.require(Permission::ViewList)?;
    state.check_environment_access(&user, &env_id, EnvAccess::View)?;
    plugin_states_for_env(&state, &env_id)
}

#[tauri::command]
pub fn plugin_set_env_enabled(
    state: State<'_, SharedState>,
    env_id: String,
    id: String,
    enabled: bool,
) -> Result<Vec<crate::models::PluginInfo>> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    if !crate::plugins::is_valid(&id) {
        return Err(AppError::not_found("插件不存在"));
    }
    if !crate::plugins::is_enabled(&state.settings(), &id) {
        return Err(AppError::bad_request(
            "插件全局开关已关闭，无法在环境中启用",
        ));
    }
    state.db.set_env_plugin_enabled(&env_id, &id, enabled)?;
    if id == crate::plugins::VIDEO_SNIFFER {
        state.browsers.set_video_override(&env_id, enabled);
    }
    let _ = Audit::global(
        &state.db,
        &user.username,
        actions::PLUGIN_UPDATE,
        format!(
            "环境 {env_id} {} 插件 {}",
            if enabled { "启用" } else { "暂停" },
            id
        ),
    );
    plugin_states_for_env(&state, &env_id)
}

fn plugin_states_for_env(
    state: &SharedState,
    env_id: &str,
) -> Result<Vec<crate::models::PluginInfo>> {
    let settings = state.settings();
    let mut plugins = crate::plugins::list(&settings);
    for plugin in &mut plugins {
        plugin.enabled = plugin.enabled
            && state
                .db
                .env_plugin_enabled(env_id, &plugin.id)?
                .unwrap_or(true);
    }
    Ok(plugins)
}

#[tauri::command]
pub async fn video_list(
    state: State<'_, SharedState>,
    env_id: String,
) -> Result<Vec<crate::models::SniffedVideo>> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let _ = state.browsers.scan_videos(&env_id).await;
    Ok(state.browsers.video_list(&env_id))
}

#[tauri::command]
pub fn video_clear(state: State<'_, SharedState>, env_id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.clear_videos(&env_id);
    Ok(())
}

#[tauri::command]
pub async fn video_download(
    state: State<'_, SharedState>,
    env_id: String,
    video_id: String,
) -> Result<String> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    let path = match state.browsers.download_video(&env_id, &video_id).await {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!(env_id = %env_id, video_id = %video_id, error = %error, "video download failed");
            return Err(error);
        }
    };
    if let Ok(env) = state.db.get_environment(&env_id) {
        let _ = Audit::env(
            &state.db,
            &user.username,
            actions::VIDEO_DOWNLOAD,
            &env,
            format!("下载视频 {}", video_id),
        );
    }
    Ok(path)
}

#[tauri::command]
pub async fn video_record_start(state: State<'_, SharedState>, env_id: String) -> Result<String> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.start_video_recording(&env_id).await
}

#[tauri::command]
pub async fn video_record_stop(state: State<'_, SharedState>, env_id: String) -> Result<()> {
    let state = state.inner().clone();
    let user = state.require(Permission::OpenClose)?;
    state.check_environment_access(&user, &env_id, EnvAccess::Operate)?;
    state.browsers.stop_video_recording(&env_id).await
}

#[tauri::command]
pub fn api_status(state: State<'_, SharedState>) -> Result<crate::models::ApiStatus> {
    let state = state.inner().clone();
    state.require(Permission::SettingsManage)?;
    Ok(state.api.status(&state.settings()))
}

#[tauri::command]
pub async fn api_restart(state: State<'_, SharedState>) -> Result<crate::models::ApiStatus> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    let status = state.api.restart(state.clone()).await?;
    Audit::global(
        &state.db,
        &actor,
        actions::API_START,
        format!("手动重启 API 服务：{}", status.base_url),
    )?;
    Ok(status)
}

#[tauri::command]
pub fn api_key_list(state: State<'_, SharedState>) -> Result<Vec<ApiKey>> {
    let state = state.inner().clone();
    state.require(Permission::ApiKeyManage)?;
    state.db.list_api_keys()
}

#[tauri::command]
pub fn api_key_create(
    state: State<'_, SharedState>,
    name: String,
    role: String,
) -> Result<CreatedApiKey> {
    let state = state.inner().clone();
    let actor = state.require(Permission::ApiKeyManage)?.username;
    services::create_api_key(&state, &name, &role, &actor)
}

#[tauri::command]
pub fn api_key_revoke(state: State<'_, SharedState>, id: String) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::ApiKeyManage)?.username;
    state.db.delete_api_key(&id)?;
    Audit::global(
        &state.db,
        &actor,
        actions::API_KEY_DELETE,
        format!("吊销 API 密钥 {id}"),
    )?;
    Ok(())
}

#[tauri::command]
pub fn theme_get(state: State<'_, SharedState>) -> Result<ThemeConfig> {
    state.db.get_theme()
}

#[tauri::command]
pub fn theme_save(state: State<'_, SharedState>, theme: ThemeConfig) -> Result<ThemeConfig> {
    let state = state.inner().clone();
    let actor = state.require_authenticated()?.username;
    services::save_theme(&state, theme, &actor)
}

#[tauri::command]
pub fn theme_export(state: State<'_, SharedState>, path: String) -> Result<()> {
    let state = state.inner().clone();
    state.require_authenticated()?;
    let theme = state.db.get_theme()?;
    if let Some(parent) = Path::new(&path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&theme)?)?;
    Ok(())
}

#[tauri::command]
pub fn theme_import(state: State<'_, SharedState>, path: String) -> Result<ThemeConfig> {
    let state = state.inner().clone();
    let actor = state.require_authenticated()?.username;
    let value: Value = serde_json::from_slice(&std::fs::read(path)?)
        .map_err(|e| AppError::bad_request(format!("主题文件不是有效 JSON: {e}")))?;
    let theme = ThemeConfig {
        mode: value
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("auto")
            .to_string(),
        primary_color: value
            .get("primaryColor")
            .and_then(|v| v.as_str())
            .unwrap_or("#165DFF")
            .to_string(),
        density: value
            .get("density")
            .and_then(|v| v.as_str())
            .unwrap_or("comfortable")
            .to_string(),
        config: value.get("config").cloned().unwrap_or(json!({})),
        updated_at: now_iso(),
    };
    services::save_theme(&state, theme, &actor)
}

#[tauri::command]
pub fn audit_list(
    state: State<'_, SharedState>,
    filter: Option<AuditFilter>,
) -> Result<Vec<crate::models::AuditLog>> {
    let state = state.inner().clone();
    state.require(Permission::AuditView)?;
    state.db.list_audit(&filter.unwrap_or_default())
}

#[tauri::command]
pub fn audit_verify(state: State<'_, SharedState>) -> Result<crate::models::AuditVerifyResult> {
    let state = state.inner().clone();
    state.require(Permission::AuditView)?;
    state.db.verify_audit_chain()
}

#[tauri::command]
pub fn audit_clear(state: State<'_, SharedState>, confirm: bool) -> Result<u32> {
    let state = state.inner().clone();
    let user = state.require(Permission::AuditView)?;
    if user.role != "super_admin" {
        return Err(AppError::forbidden("只有超级管理员可以清空审计日志"));
    }
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "清空审计日志需要二次确认",
        ));
    }
    let count = state.db.clear_audit_logs()?;
    let _ = state.db.set_meta(
        "audit_last_clear",
        &json!({
            "actor": user.username,
            "count": count,
            "clearedAt": now_iso()
        })
        .to_string(),
    );
    tracing::warn!(actor = %user.username, count, "audit logs cleared manually");
    Ok(count)
}

#[tauri::command]
pub fn audit_export(
    state: State<'_, SharedState>,
    path: String,
    format: Option<String>,
    filter: Option<AuditFilter>,
) -> Result<u64> {
    let state = state.inner().clone();
    let actor = state.require(Permission::AuditView)?.username;
    let count = services::audit_export(
        &state,
        &filter.unwrap_or_default(),
        Path::new(&path),
        format.as_deref().unwrap_or("json"),
    )?;
    Audit::global(
        &state.db,
        &actor,
        "audit.export",
        format!("导出审计日志 {count} 条至 {path}"),
    )?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Dashboard / app / update
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn dashboard(state: State<'_, SharedState>) -> Result<crate::models::DashboardStats> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    services::dashboard(&state).await
}

#[tauri::command]
pub fn app_info(state: State<'_, SharedState>) -> crate::models::AppInfo {
    let settings = state.settings();
    let chromium = crate::process::detect_chromium_with(settings.chromium_path.as_deref());
    let api = state.api.status(&settings);
    crate::models::AppInfo {
        version: env!("CARGO_PKG_VERSION").into(),
        data_dir: state.data_dir.to_string_lossy().to_string(),
        os: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        chromium_path: chromium.map(|p| p.to_string_lossy().to_string()),
        chromium_version: None,
        chromium_default_args: crate::process::default_chromium_args(),
        api_base_url: api.base_url,
        db_path: state.db.path().to_string_lossy().to_string(),
    }
}

#[tauri::command]
pub fn chromium_detect(state: State<'_, SharedState>) -> Result<crate::models::ChromiumDetection> {
    let state = state.inner().clone();
    state.require(Permission::ViewList)?;
    let settings = state.settings();
    let path = crate::process::detect_chromium_with(settings.chromium_path.as_deref());
    let version = path.as_deref().and_then(crate::process::chromium_version);
    Ok(crate::models::ChromiumDetection {
        path: path.map(|p| p.to_string_lossy().to_string()),
        version,
        message: "检测完成".into(),
    })
}

#[tauri::command]
pub async fn update_check(state: State<'_, SharedState>) -> Result<crate::models::UpdateInfo> {
    let state = state.inner().clone();
    state.require(Permission::SettingsManage)?;
    let settings = state.settings();
    let current = env!("CARGO_PKG_VERSION").to_string();
    let Some(url) = settings
        .download_manifest_url
        .filter(|u| !u.trim().is_empty())
    else {
        return Ok(crate::models::UpdateInfo {
            update_available: false,
            current_version: current,
            latest_version: None,
            notes: None,
            download_url: None,
            message: "未配置更新清单地址".into(),
        });
    };
    let value: Value = reqwest::Client::new()
        .get(url)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let latest = value
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if latest.is_empty() {
        return Err(AppError::bad_request("更新清单缺少 version 字段"));
    }
    Ok(crate::models::UpdateInfo {
        update_available: version_is_newer(&latest, &current),
        current_version: current,
        latest_version: Some(latest),
        notes: value
            .get("notes")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        download_url: value
            .get("url")
            .and_then(|v| v.as_str())
            .map(ToString::to_string),
        message: "已检查更新清单".into(),
    })
}

fn version_is_newer(latest: &str, current: &str) -> bool {
    fn parts(value: &str) -> Vec<u64> {
        value
            .trim_start_matches('v')
            .split(['.', '-', '+'])
            .take(3)
            .map(|part| part.parse::<u64>().unwrap_or(0))
            .collect()
    }
    let latest = parts(latest);
    let current = parts(current);
    for index in 0..3 {
        let left = latest.get(index).copied().unwrap_or(0);
        let right = current.get(index).copied().unwrap_or(0);
        if left != right {
            return left > right;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Backup / sync
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn backup_create(
    state: State<'_, SharedState>,
    path: Option<String>,
    passphrase: Option<String>,
    include_profiles: bool,
) -> Result<crate::models::BackupInfo> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    let path = path
        .filter(|p| !p.trim().is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            paths::backups_dir(&state.data_dir).join(format!(
                "finenv-{}.finenvbackup",
                chrono::Utc::now().format("%Y%m%d-%H%M%S")
            ))
        });
    let pass = passphrase.filter(|p| !p.trim().is_empty());
    let bytes = backup::create_backup(
        &state.db,
        &state.crypto,
        &state.data_dir,
        include_profiles,
        pass.as_deref(),
    )?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &bytes)?;
    Audit::global(
        &state.db,
        &actor,
        actions::BACKUP_CREATE,
        format!(
            "创建本地备份 {}（{} bytes，包含 profile: {}，加密: {}）",
            path.display(),
            bytes.len(),
            include_profiles,
            pass.is_some()
        ),
    )?;
    Ok(crate::models::BackupInfo {
        path: path.to_string_lossy().to_string(),
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
        size: bytes.len() as u64,
        encrypted: pass.is_some(),
        created_at: now_iso(),
    })
}

#[tauri::command]
pub async fn backup_restore(
    state: State<'_, SharedState>,
    path: String,
    passphrase: Option<String>,
    restore_profiles: bool,
    confirm: bool,
) -> Result<String> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "恢复备份会覆盖当前数据库，需要二次确认",
        ));
    }
    state.browsers.force_recycle_all(&actor).await?;
    let bytes = std::fs::read(&path)?;
    let pass = passphrase.filter(|p| !p.trim().is_empty());
    let message = backup::restore_backup(
        &state.db,
        &state.crypto,
        &state.data_dir,
        &bytes,
        pass.as_deref(),
        restore_profiles,
    )?;
    state.reload_settings()?;
    let _ = state.logout();
    let _ = Audit::global(
        &state.db,
        &actor,
        actions::BACKUP_RESTORE,
        format!("从 {} 恢复备份", path),
    );
    Ok(format!("{message} 当前会话已退出，请重新登录。"))
}

#[tauri::command]
pub fn backup_list(state: State<'_, SharedState>) -> Result<Vec<crate::models::BackupInfo>> {
    let state = state.inner().clone();
    state.require(Permission::SettingsManage)?;
    backup::list_backups(&state.data_dir)
}

#[tauri::command]
pub fn backup_delete(state: State<'_, SharedState>, path: String, confirm: bool) -> Result<()> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "删除备份文件需要二次确认",
        ));
    }
    backup::delete_backup(&state.data_dir, &path)?;
    let _ = Audit::global(
        &state.db,
        &actor,
        actions::BACKUP_DELETE,
        format!("删除本地备份 {}", path),
    );
    Ok(())
}

#[tauri::command]
pub async fn sync_test(state: State<'_, SharedState>, password: Option<String>) -> Result<String> {
    let state = state.inner().clone();
    state.require(Permission::SettingsManage)?;
    let settings = state.settings();
    let password = password.or_else(|| {
        settings
            .sync_password_enc
            .as_deref()
            .and_then(|enc| state.crypto.decrypt(enc).ok())
    });
    backup::sync_test(&settings, password).await
}

#[tauri::command]
pub async fn sync_push(
    state: State<'_, SharedState>,
    passphrase: String,
) -> Result<crate::models::SyncResult> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    let settings = state.settings();
    let result = backup::sync_push(
        &state.db,
        &state.crypto,
        &state.data_dir,
        &settings,
        &passphrase,
    )
    .await?;
    Audit::global(
        &state.db,
        &actor,
        actions::SYNC_PUSH,
        format!("推送加密同步包（{} bytes）", result.bytes),
    )?;
    let mut updated = settings;
    updated.sync_last_at = Some(result.synced_at.clone());
    state.store_settings(&updated)?;
    Ok(result)
}

#[tauri::command]
pub async fn sync_pull(
    state: State<'_, SharedState>,
    passphrase: String,
    confirm: bool,
) -> Result<crate::models::SyncResult> {
    let state = state.inner().clone();
    let actor = state.require(Permission::SettingsManage)?.username;
    if !confirm {
        return Err(AppError::code_with(
            "confirmation_required",
            "拉取同步包会覆盖本地数据库，需要二次确认",
        ));
    }
    state.browsers.force_recycle_all(&actor).await?;
    let settings = state.settings();
    let result = backup::sync_pull(
        &state.db,
        &state.crypto,
        &state.data_dir,
        &settings,
        &passphrase,
    )
    .await?;
    state.reload_settings()?;
    let _ = state.logout();
    let _ = Audit::global(
        &state.db,
        &actor,
        actions::SYNC_PULL,
        format!("拉取加密同步包（{} bytes）", result.bytes),
    );
    Ok(result)
}
