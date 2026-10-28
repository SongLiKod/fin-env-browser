use std::io::{Cursor, Read, Write};
use std::path::Path;

use crate::{
    audit_log::{actions, Audit},
    capability,
    db::settings::hash_api_key,
    error::{AppError, Result},
    fingerprint,
    models::{
        new_id, now_iso, Account, AccountExportRow, AccountInput, AlignProxyResult, ApiKey,
        AuditFilter, BatchCreateResult, BatchCreateTemplateInput, CreateEnvironmentInput,
        CreateTemplateFromEnvInput, CreatedApiKey, DashboardStats, Environment, EnvironmentFilter,
        EnvironmentIpInfo, EnvironmentTemplate, EnvironmentView, Extension, FingerprintSnapshot,
        Group, GroupInput, HostsEntryInput, MockRuleInput, Proxy, ProxyCheckResult, ProxyInput,
        Settings, ThemeConfig, UpdateAccountInput, UpdateEnvironmentInput, UpdateProxyInput,
        UpdateSettingsInput, UpdateTemplateInput, User, UserInput, UserUpdateInput,
    },
    package, paths, proxy_check,
    state::SharedState,
};

// ---------------------------------------------------------------------------
// Environment views & CRUD
// ---------------------------------------------------------------------------

pub fn environment_view(state: &SharedState, env: Environment) -> Result<EnvironmentView> {
    let proxy = match &env.proxy_id {
        Some(id) => state.db.get_proxy(id).ok(),
        None => None,
    };
    Ok(EnvironmentView {
        runtime: state.browsers.get_runtime(&env.id),
        account_count: state.db.account_count_for_env(&env.id)?,
        snapshot_count: state.db.snapshot_count_for_env(&env.id)?,
        access_level: match state.current_user() {
            Some(user) if user.role == "super_admin" => "manage".into(),
            Some(user) => state
                .db
                .environment_member_level(&env.id, &user.id)?
                .unwrap_or_else(|| "view".into()),
            None => "manage".into(),
        },
        proxy,
        env,
    })
}

pub fn list_environments(
    state: &SharedState,
    filter: &EnvironmentFilter,
) -> Result<Vec<EnvironmentView>> {
    let envs = state.db.list_environments(filter)?;
    envs.into_iter()
        .map(|env| environment_view(state, env))
        .collect()
}

pub fn list_environments_visible(
    state: &SharedState,
    filter: &EnvironmentFilter,
    user: &crate::models::User,
) -> Result<Vec<EnvironmentView>> {
    let allowed = state.accessible_environment_ids(user)?;
    let envs = state.db.list_environments(filter)?;
    let filtered = match allowed {
        Some(ids) => envs
            .into_iter()
            .filter(|env| ids.contains(&env.id))
            .collect::<Vec<_>>(),
        None => envs,
    };
    filtered
        .into_iter()
        .map(|env| environment_view(state, env))
        .collect()
}

pub fn assign_environment_manager(state: &SharedState, env_id: &str, user_id: &str) -> Result<()> {
    state
        .db
        .upsert_environment_member(env_id, user_id, "manage")?;
    Ok(())
}

pub fn get_environment(state: &SharedState, id: &str) -> Result<EnvironmentView> {
    environment_view(state, state.db.get_environment(id)?)
}

fn refresh_start_page(state: &SharedState, env: &Environment) -> bool {
    let proxy = env
        .proxy_id
        .as_ref()
        .and_then(|id| state.db.get_proxy(id).ok());
    let accounts = state.db.account_count_for_env(&env.id).unwrap_or(0);
    let snapshots = state.db.snapshot_count_for_env(&env.id).unwrap_or(0);
    let runtime = state.browsers.get_runtime(&env.id);
    crate::start_page::refresh_if_present(
        &state.data_dir,
        env,
        proxy.as_ref(),
        accounts,
        snapshots,
        runtime.as_ref(),
    )
    .is_ok()
}

pub fn create_environment(
    state: &SharedState,
    input: CreateEnvironmentInput,
    actor: &str,
) -> Result<Environment> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("环境名称不能为空"));
    }
    if name.chars().count() > 80 {
        return Err(AppError::bad_request("环境名称不能超过 80 个字符"));
    }
    if state.db.find_environment_by_name(name)?.is_some() {
        return Err(AppError::conflict("已存在同名环境"));
    }
    if let Some(group_id) = input.group_id.as_deref() {
        if !state.db.list_groups()?.iter().any(|g| g.id == group_id) {
            return Err(AppError::bad_request("指定的分组不存在"));
        }
    }
    if let Some(proxy_id) = input.proxy_id.as_deref() {
        state.db.get_proxy(proxy_id)?;
    }
    validate_test_env(input.test_env.as_deref().unwrap_or(""))?;
    let kernel = input.kernel.clone().unwrap_or_else(|| "chromium".into());
    if kernel != "chromium" {
        return Err(AppError::bad_request("V1 仅支持 Chromium 内核"));
    }
    let fingerprint = input
        .fingerprint
        .clone()
        .unwrap_or_else(|| fingerprint::template(input.template_os.as_deref().unwrap_or("win11")));
    let validation = fingerprint::validate(&fingerprint);
    if !validation.valid {
        return Err(validation_error(&validation, "新建环境指纹参数冲突"));
    }
    let id = new_id();
    let now = now_iso();
    let profile_dir = paths::env_profile_dir(&state.data_dir, &id);
    paths::ensure_dir(&profile_dir)?;
    let env = Environment {
        id,
        name: name.to_string(),
        group_id: input.group_id,
        status: "active".into(),
        profile_dir: profile_dir.to_string_lossy().to_string(),
        proxy_id: input.proxy_id,
        fingerprint_snapshot_id: None,
        project: input.project.unwrap_or_default().trim().to_string(),
        test_env: input.test_env.unwrap_or_default().trim().to_string(),
        account_role: input.account_role.unwrap_or_default().trim().to_string(),
        version: input.version.unwrap_or_default().trim().to_string(),
        case_id: input.case_id.unwrap_or_default().trim().to_string(),
        tags: clean_tags(input.tags),
        notes: input.notes.unwrap_or_default(),
        border_color: normalize_border_color(input.border_color)?,
        border_width: valid_border_width(input.border_width)?,
        kernel,
        chromium_path: input.chromium_path.filter(|p| !p.trim().is_empty()),
        auto_restart: input.auto_restart.unwrap_or(false),
        memory_limit_mb: valid_memory_limit(input.memory_limit_mb)?,
        idle_timeout_min: valid_idle(input.idle_timeout_min)?,
        fingerprint,
        ip_address: None,
        ip_country: None,
        ip_country_code: None,
        ip_city: None,
        ip_timezone: None,
        ip_source: None,
        ip_checked_at: None,
        created_at: now.clone(),
        updated_at: now,
        last_opened_at: None,
    };
    state.db.insert_environment(&env)?;
    Audit::env(
        &state.db,
        actor,
        actions::ENV_CREATE,
        &env,
        format!(
            "新建环境（分组 {}，指纹模板 {}，代理 {}）",
            env.group_id.as_deref().unwrap_or("无"),
            env.fingerprint.client_hints.platform,
            env.proxy_id.as_deref().unwrap_or("直连")
        ),
    )?;
    Ok(env)
}

pub async fn update_environment(
    state: &SharedState,
    id: &str,
    input: UpdateEnvironmentInput,
    actor: &str,
) -> Result<Environment> {
    let mut env = state.db.get_environment(id)?;
    if let Some(name) = input.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::bad_request("环境名称不能为空"));
        }
        if let Some(existing) = state.db.find_environment_by_name(name)? {
            if existing.id != env.id {
                return Err(AppError::conflict("已存在同名环境"));
            }
        }
        env.name = name.to_string();
    }
    if let Some(group) = input.group_id {
        if let Some(group_id) = &group {
            if !state.db.list_groups()?.iter().any(|g| &g.id == group_id) {
                return Err(AppError::bad_request("指定的分组不存在"));
            }
        }
        env.group_id = group;
    }
    if let Some(status) = input.status {
        if !crate::models::ENV_STATUSES.contains(&status.as_str()) {
            return Err(AppError::bad_request("环境状态不合法"));
        }
        env.status = status;
    }
    if let Some(proxy) = input.proxy_id {
        if let Some(proxy_id) = &proxy {
            state.db.get_proxy(proxy_id)?;
        }
        if env.proxy_id != proxy {
            env.ip_address = None;
            env.ip_country = None;
            env.ip_country_code = None;
            env.ip_city = None;
            env.ip_timezone = None;
            env.ip_source = None;
            env.ip_checked_at = None;
        }
        env.proxy_id = proxy;
    }
    if let Some(project) = input.project {
        env.project = project.trim().to_string();
    }
    if let Some(test_env) = input.test_env {
        validate_test_env(&test_env)?;
        env.test_env = test_env.trim().to_string();
    }
    if let Some(role) = input.account_role {
        env.account_role = role.trim().to_string();
    }
    if let Some(version) = input.version {
        env.version = version.trim().to_string();
    }
    if let Some(case_id) = input.case_id {
        env.case_id = case_id.trim().to_string();
    }
    if let Some(tags) = input.tags {
        env.tags = clean_tags(tags);
    }
    if let Some(notes) = input.notes {
        env.notes = notes;
    }
    if let Some(color) = input.border_color {
        env.border_color = normalize_border_color(Some(color))?;
    }
    if let Some(width) = input.border_width {
        env.border_width = valid_border_width(Some(width))?;
    }
    if let Some(kernel) = input.kernel {
        if kernel != "chromium" {
            return Err(AppError::bad_request("V1 仅支持 Chromium 内核"));
        }
        env.kernel = kernel;
    }
    if let Some(path) = input.chromium_path {
        env.chromium_path = path.filter(|p| !p.trim().is_empty());
    }
    if let Some(auto_restart) = input.auto_restart {
        env.auto_restart = auto_restart;
    }
    if let Some(limit) = input.memory_limit_mb {
        env.memory_limit_mb = valid_memory_limit(limit)?;
    }
    if let Some(idle) = input.idle_timeout_min {
        env.idle_timeout_min = valid_idle(idle)?;
    }
    env.updated_at = now_iso();
    state.db.update_environment(&env)?;
    let _ = refresh_start_page(state, &env);
    if env.status == "disabled" && state.browsers.is_running(&env.id) {
        state
            .browsers
            .close(
                &env.id,
                actor,
                "环境被停用，自动关闭浏览器",
                actions::ENV_CLOSE,
            )
            .await?;
    }
    Audit::env(&state.db, actor, actions::ENV_UPDATE, &env, "更新环境信息")?;
    Ok(env)
}

pub fn list_templates(state: &SharedState) -> Result<Vec<EnvironmentTemplate>> {
    state.db.list_templates()
}

pub fn create_template_from_env(
    state: &SharedState,
    input: CreateTemplateFromEnvInput,
    actor: &str,
) -> Result<EnvironmentTemplate> {
    let env = state.db.get_environment(&input.env_id)?;
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("模板名称不能为空"));
    }
    if name.chars().count() > 80 {
        return Err(AppError::bad_request("模板名称不能超过 80 个字符"));
    }
    if state.db.find_template_by_name(name)?.is_some() {
        return Err(AppError::conflict("已存在同名环境模板"));
    }
    let now = now_iso();
    let template = EnvironmentTemplate {
        id: new_id(),
        name: name.to_string(),
        description: input.description.unwrap_or_default(),
        project: env.project.clone(),
        test_env: env.test_env.clone(),
        account_role: env.account_role.clone(),
        version: env.version.clone(),
        case_id: env.case_id.clone(),
        tags: env.tags.clone(),
        notes: env.notes.clone(),
        kernel: env.kernel.clone(),
        chromium_path: env.chromium_path.clone(),
        auto_restart: env.auto_restart,
        memory_limit_mb: env.memory_limit_mb,
        idle_timeout_min: env.idle_timeout_min,
        fingerprint: env.fingerprint.clone(),
        border_color: env.border_color.clone(),
        border_width: env.border_width,
        group_id: env.group_id.clone(),
        proxy_mode: if env.proxy_id.is_some() {
            "fixed".into()
        } else {
            "none".into()
        },
        proxy_id: env.proxy_id.clone(),
        extension_ids: state
            .db
            .extensions_for_env(&env.id)?
            .into_iter()
            .filter(|extension| extension.enabled)
            .map(|extension| extension.id)
            .collect(),
        created_at: now.clone(),
        updated_at: now,
    };
    state.db.insert_template(&template)?;
    Audit::global(
        &state.db,
        actor,
        actions::TEMPLATE_CREATE,
        format!("从环境 {} 创建模板 {}", env.name, template.name),
    )?;
    Ok(template)
}

pub fn update_template(
    state: &SharedState,
    id: &str,
    input: UpdateTemplateInput,
    actor: &str,
) -> Result<EnvironmentTemplate> {
    let mut template = state.db.get_template(id)?;
    if let Some(name) = input.name {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::bad_request("模板名称不能为空"));
        }
        if let Some(existing) = state.db.find_template_by_name(name)? {
            if existing.id != template.id {
                return Err(AppError::conflict("已存在同名环境模板"));
            }
        }
        template.name = name.to_string();
    }
    if let Some(description) = input.description {
        template.description = description;
    }
    if let Some(proxy_mode) = input.proxy_mode.as_deref() {
        match proxy_mode {
            "none" => {
                template.proxy_mode = "none".into();
                template.proxy_id = None;
            }
            "pool" => {
                if !state.db.list_proxies()?.iter().any(|proxy| proxy.enabled) {
                    return Err(AppError::bad_request("代理池模式需要至少一个启用代理"));
                }
                template.proxy_mode = "pool".into();
                template.proxy_id = None;
            }
            "fixed" => {
                let proxy_id = input
                    .proxy_id
                    .clone()
                    .or_else(|| template.proxy_id.clone())
                    .ok_or_else(|| AppError::bad_request("固定代理模式需要选择代理"))?;
                state.db.get_proxy(&proxy_id)?;
                template.proxy_mode = "fixed".into();
                template.proxy_id = Some(proxy_id);
            }
            _ => return Err(AppError::bad_request("不支持的代理策略")),
        }
    }
    template.updated_at = now_iso();
    state.db.update_template(&template)?;
    Audit::global(
        &state.db,
        actor,
        actions::TEMPLATE_UPDATE,
        format!("更新环境模板 {}", template.name),
    )?;
    Ok(template)
}

pub fn delete_template(state: &SharedState, id: &str, actor: &str) -> Result<()> {
    let template = state.db.get_template(id)?;
    state.db.delete_template(id)?;
    Audit::global(
        &state.db,
        actor,
        actions::TEMPLATE_DELETE,
        format!("删除环境模板 {}", template.name),
    )?;
    Ok(())
}

pub fn batch_create_from_template(
    state: &SharedState,
    input: BatchCreateTemplateInput,
    actor: &str,
) -> Result<BatchCreateResult> {
    if !(1..=50).contains(&input.count) {
        return Err(AppError::bad_request("单次批量创建数量需为 1-50"));
    }
    let template = state.db.get_template(&input.template_id)?;
    let prefix = input
        .name_prefix
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(&template.name)
        .to_string();
    if prefix.chars().count() > 60 {
        return Err(AppError::bad_request("环境名前缀不能超过 60 个字符"));
    }
    let start_index = input.start_index.unwrap_or(1).max(1);
    if start_index > 999_999 {
        return Err(AppError::bad_request("起始序号不能超过 999999"));
    }
    if template.proxy_mode == "fixed" && template.proxy_id.is_none() {
        return Err(AppError::bad_request(
            "模板的固定代理已被删除，请先编辑模板重新选择代理",
        ));
    }
    let proxy_pool = if template.proxy_mode == "pool" {
        let proxies = state
            .db
            .list_proxies()?
            .into_iter()
            .filter(|proxy| proxy.enabled)
            .collect::<Vec<_>>();
        if proxies.is_empty() {
            return Err(AppError::bad_request("模板使用代理池，但没有可用代理"));
        }
        proxies
    } else {
        Vec::new()
    };
    let extension_ids = if template.extension_ids.is_empty() {
        Vec::new()
    } else {
        let existing = state
            .db
            .list_extensions()?
            .into_iter()
            .filter(|extension| extension.enabled)
            .map(|extension| extension.id)
            .collect::<std::collections::HashSet<_>>();
        template
            .extension_ids
            .iter()
            .filter(|id| existing.contains(*id))
            .cloned()
            .collect::<Vec<_>>()
    };

    let mut created = Vec::new();
    let mut errors = Vec::new();
    for offset in 0..input.count {
        let index = start_index + offset;
        let mut candidate = format!("{prefix} {index:02}");
        let mut suffix = 2;
        while state.db.find_environment_by_name(&candidate)?.is_some() {
            candidate = format!("{prefix} {index:02}-{suffix}");
            suffix += 1;
        }
        let proxy_id = match template.proxy_mode.as_str() {
            "fixed" => template.proxy_id.clone(),
            "pool" => Some(proxy_pool[(offset as usize) % proxy_pool.len()].id.clone()),
            _ => None,
        };
        let name = candidate.clone();
        let input = CreateEnvironmentInput {
            name: candidate,
            group_id: template.group_id.clone(),
            proxy_id,
            project: Some(template.project.clone()),
            test_env: Some(template.test_env.clone()),
            account_role: Some(template.account_role.clone()),
            version: Some(template.version.clone()),
            case_id: Some(template.case_id.clone()),
            tags: template.tags.clone(),
            notes: Some(template.notes.clone()),
            border_color: Some(template.border_color.clone()),
            border_width: Some(template.border_width),
            kernel: Some(template.kernel.clone()),
            chromium_path: template.chromium_path.clone(),
            template_os: None,
            fingerprint: Some(template.fingerprint.clone()),
            auto_restart: Some(template.auto_restart),
            memory_limit_mb: template.memory_limit_mb,
            idle_timeout_min: template.idle_timeout_min,
        };
        match create_environment(state, input, actor) {
            Ok(env) => {
                if !extension_ids.is_empty() {
                    if let Err(error) = state.db.assign_extension(&env.id, &extension_ids) {
                        errors.push(format!("{}: 扩展分配失败: {}", env.name, error.message()));
                    }
                }
                created.push(env);
            }
            Err(error) => errors.push(format!("{name}: {}", error.message())),
        }
    }
    Audit::global(
        &state.db,
        actor,
        actions::TEMPLATE_BATCH_CREATE,
        format!(
            "使用模板 {} 批量创建 {} 个环境（成功 {}，失败 {}）",
            template.name,
            input.count,
            created.len(),
            errors.len()
        ),
    )?;
    Ok(BatchCreateResult { created, errors })
}

pub async fn delete_environment(state: &SharedState, id: &str, actor: &str) -> Result<()> {
    let env = state.db.get_environment(id)?;
    if state.browsers.is_running(id) {
        state
            .browsers
            .close(id, actor, "删除环境前关闭浏览器", actions::ENV_CLOSE)
            .await?;
    }
    Audit::env(
        &state.db,
        actor,
        actions::ENV_DELETE,
        &env,
        format!("删除环境 {} 及其账号、快照、Hosts、Mock 数据", env.name),
    )?;
    state.db.delete_environment(id)?;
    let profile = std::path::PathBuf::from(&env.profile_dir);
    if profile.starts_with(paths::profiles_dir(&state.data_dir)) && profile.exists() {
        let _ = std::fs::remove_dir_all(profile);
    }
    Ok(())
}

pub fn clone_environment(
    state: &SharedState,
    id: &str,
    requested_name: Option<String>,
    actor: &str,
) -> Result<Environment> {
    let source = state.db.get_environment(id)?;
    let base = requested_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| format!("{} - 副本", source.name));
    let mut name = base.clone();
    let mut index = 2;
    while state.db.find_environment_by_name(&name)?.is_some() {
        name = format!("{base} {index}");
        index += 1;
    }
    let new_env_id = new_id();
    let now = now_iso();
    let profile_dir = paths::env_profile_dir(&state.data_dir, &new_env_id);
    paths::ensure_dir(&profile_dir)?;
    let mut env = source.clone();
    env.id = new_env_id.clone();
    env.name = name;
    env.profile_dir = profile_dir.to_string_lossy().to_string();
    env.fingerprint_snapshot_id = None;
    env.created_at = now.clone();
    env.updated_at = now.clone();
    env.last_opened_at = None;
    env.ip_address = None;
    env.ip_country = None;
    env.ip_country_code = None;
    env.ip_city = None;
    env.ip_timezone = None;
    env.ip_source = None;
    env.ip_checked_at = None;
    state.db.insert_environment(&env)?;
    for account in state.db.list_accounts(id)? {
        let mut copy = account.clone();
        copy.id = new_id();
        copy.env_id = new_env_id.clone();
        copy.created_at = now.clone();
        copy.updated_at = now.clone();
        let _ = state.db.insert_account(&copy);
    }
    for snapshot in state.db.list_snapshots(id)? {
        let mut copy = snapshot.clone();
        copy.id = new_id();
        copy.env_id = new_env_id.clone();
        copy.name = format!("{} (克隆)", snapshot.name);
        state.db.insert_snapshot(&copy)?;
    }
    let hosts: Vec<HostsEntryInput> = state
        .db
        .list_hosts(id)?
        .into_iter()
        .map(|h| HostsEntryInput {
            host: h.host,
            ip: h.ip,
            enabled: Some(h.enabled),
        })
        .collect();
    state.db.replace_hosts(&new_env_id, &hosts)?;
    let mocks: Vec<MockRuleInput> = state
        .db
        .list_mocks(id)?
        .into_iter()
        .map(|m| MockRuleInput {
            pattern: m.pattern,
            method: m.method,
            status_code: m.status_code,
            headers: m.headers,
            body: m.body,
            delay_ms: m.delay_ms,
            enabled: Some(m.enabled),
        })
        .collect();
    state.db.replace_mocks(&new_env_id, &mocks)?;
    let extension_ids: Vec<String> = state
        .db
        .extensions_for_env(id)?
        .into_iter()
        .map(|e| e.id)
        .collect();
    if !extension_ids.is_empty() {
        state
            .db
            .assign_extension(&extension_ids[0], std::slice::from_ref(&new_env_id))?;
        for extension_id in extension_ids.iter().skip(1) {
            state
                .db
                .assign_extension(extension_id, std::slice::from_ref(&new_env_id))?;
        }
    }
    Audit::env(
        &state.db,
        actor,
        actions::ENV_CLONE,
        &env,
        format!("从环境 {} 克隆，新环境 ID {}", source.name, env.id),
    )?;
    Ok(env)
}

pub async fn bulk_delete_environments(
    state: &SharedState,
    ids: &[String],
    actor: &str,
) -> Result<u32> {
    let mut count = 0;
    for id in ids {
        delete_environment(state, id, actor).await?;
        count += 1;
    }
    Ok(count)
}

fn validate_test_env(value: &str) -> Result<()> {
    if value.is_empty() || ["dev", "test", "pre", "prod"].contains(&value) {
        Ok(())
    } else {
        Err(AppError::bad_request(
            "测试环境仅支持 dev / test / pre / prod",
        ))
    }
}

fn normalize_border_color(value: Option<String>) -> Result<String> {
    let raw = value.unwrap_or_default().trim().to_string();
    if raw.is_empty() {
        return Ok(String::new());
    }
    let hex = raw.trim_start_matches('#');
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(format!("#{}", hex.to_ascii_uppercase()))
    } else {
        Err(AppError::bad_request(
            "边框颜色需为 6 位十六进制，例如 #2563EB",
        ))
    }
}

fn valid_border_width(value: Option<u32>) -> Result<u32> {
    match value {
        None | Some(0) => Ok(3),
        Some(width) if (1..=12).contains(&width) => Ok(width),
        _ => Err(AppError::bad_request("环境标识色粗细需为 1-12px")),
    }
}

fn clean_tags(tags: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = tags
        .into_iter()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty() && t.chars().count() <= 32)
        .collect();
    out.sort();
    out.dedup();
    out
}

fn valid_memory_limit(value: Option<u32>) -> Result<Option<u32>> {
    match value {
        Some(0) => Ok(None),
        Some(v) if v < 128 => Err(AppError::bad_request("单环境内存阈值不能低于 128MB")),
        Some(v) if v > 65536 => Err(AppError::bad_request("单环境内存阈值过大")),
        other => Ok(other),
    }
}

fn valid_idle(value: Option<u32>) -> Result<Option<u32>> {
    match value {
        Some(0) => Ok(None),
        Some(v) if v > 1440 => Err(AppError::bad_request("闲置超时不能超过 1440 分钟")),
        other => Ok(other),
    }
}

pub fn validation_error(report: &fingerprint::FingerprintValidation, prefix: &str) -> AppError {
    let errors = report
        .errors
        .iter()
        .map(|i| format!("{}: {}", i.field, i.message))
        .collect::<Vec<_>>()
        .join("; ");
    AppError::code_with("fingerprint_conflict", format!("{prefix}: {errors}"))
}

// ---------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------

pub fn ensure_group(state: &SharedState, input: GroupInput, actor: &str) -> Result<Group> {
    let name = input.name.trim();
    if name.is_empty() {
        return Err(AppError::bad_request("分组名称不能为空"));
    }
    if state.db.list_groups()?.iter().any(|g| g.name == name) {
        return Err(AppError::conflict("分组名称已存在"));
    }
    let group = Group {
        id: new_id(),
        name: name.to_string(),
        color: input.color.unwrap_or_else(|| "#165DFF".into()),
        created_at: now_iso(),
    };
    state.db.create_group(&group)?;
    Audit::global(
        &state.db,
        actor,
        "group.create",
        format!("创建分组 {}", group.name),
    )?;
    Ok(group)
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

pub fn create_account(state: &SharedState, input: AccountInput, actor: &str) -> Result<Account> {
    let env = state.db.get_environment(&input.env_id)?;
    let username = input.username.trim();
    if username.is_empty() {
        return Err(AppError::bad_request("账号不能为空"));
    }
    let status = input.status.unwrap_or_else(|| "pending".into());
    if !crate::models::ACCOUNT_STATUSES.contains(&status.as_str()) {
        return Err(AppError::bad_request("账号状态不合法"));
    }
    let password = input.password.unwrap_or_default();
    let account = Account {
        id: new_id(),
        env_id: env.id.clone(),
        username: username.to_string(),
        password_enc: if password.is_empty() {
            String::new()
        } else {
            state.crypto.encrypt(&password)?
        },
        has_password: !password.is_empty(),
        remark: input.remark.unwrap_or_default(),
        owner: input.owner.unwrap_or_default(),
        status,
        created_at: now_iso(),
        updated_at: now_iso(),
    };
    state.db.insert_account(&account)?;
    Audit::env_account(
        &state.db,
        actor,
        actions::ACCOUNT_CREATE,
        &env,
        username,
        "新增账号台账记录（密码已加密）",
    )?;
    Ok(account)
}

pub fn update_account(
    state: &SharedState,
    id: &str,
    input: UpdateAccountInput,
    actor: &str,
) -> Result<Account> {
    let mut account = state.db.get_account(id)?;
    let env = state.db.get_environment(&account.env_id)?;
    if let Some(username) = input.username {
        let username = username.trim();
        if username.is_empty() {
            return Err(AppError::bad_request("账号不能为空"));
        }
        account.username = username.to_string();
    }
    if let Some(password) = input.password {
        account.password_enc = if password.is_empty() {
            String::new()
        } else {
            state.crypto.encrypt(&password)?
        };
        account.has_password = !password.is_empty();
    }
    if let Some(remark) = input.remark {
        account.remark = remark;
    }
    if let Some(owner) = input.owner {
        account.owner = owner;
    }
    if let Some(status) = input.status {
        if !crate::models::ACCOUNT_STATUSES.contains(&status.as_str()) {
            return Err(AppError::bad_request("账号状态不合法"));
        }
        account.status = status;
    }
    account.updated_at = now_iso();
    state.db.update_account(&account)?;
    Audit::env_account(
        &state.db,
        actor,
        actions::ACCOUNT_UPDATE,
        &env,
        &account.username,
        "更新账号台账记录",
    )?;
    Ok(account)
}

pub fn delete_account(state: &SharedState, id: &str, actor: &str) -> Result<()> {
    let account = state.db.get_account(id)?;
    let env = state.db.get_environment(&account.env_id)?;
    Audit::env_account(
        &state.db,
        actor,
        actions::ACCOUNT_DELETE,
        &env,
        &account.username,
        "删除账号台账记录",
    )?;
    state.db.delete_account(id)?;
    Ok(())
}

pub fn reveal_password(state: &SharedState, id: &str, actor: &str) -> Result<String> {
    let account = state.db.get_account(id)?;
    let env = state.db.get_environment(&account.env_id)?;
    let password = if account.password_enc.is_empty() {
        String::new()
    } else {
        state.crypto.decrypt(&account.password_enc)?
    };
    Audit::env_account(
        &state.db,
        actor,
        actions::ACCOUNT_REVEAL,
        &env,
        &account.username,
        "查看账号明文密码（高危操作）",
    )?;
    Ok(password)
}

pub fn import_accounts(
    state: &SharedState,
    env_id: &str,
    rows: &[AccountExportRow],
    actor: &str,
) -> Result<u32> {
    let env = state.db.get_environment(env_id)?;
    let count = package::import_account_rows(&state.db, &state.crypto, env_id, rows)?;
    Audit::env(
        &state.db,
        actor,
        actions::ACCOUNT_IMPORT,
        &env,
        format!("批量导入账号 {count} 条"),
    )?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Fingerprints & snapshots
// ---------------------------------------------------------------------------

pub fn update_fingerprint(
    state: &SharedState,
    env_id: &str,
    fp: fingerprint::Fingerprint,
    actor: &str,
) -> Result<Environment> {
    let validation = fingerprint::validate(&fp);
    if !validation.valid {
        return Err(validation_error(&validation, "指纹参数冲突"));
    }
    let mut env = state.db.get_environment(env_id)?;
    env.fingerprint = fp;
    env.fingerprint_snapshot_id = None;
    env.updated_at = now_iso();
    state.db.update_environment(&env)?;
    Audit::env(
        &state.db,
        actor,
        actions::FINGERPRINT_UPDATE,
        &env,
        format!(
            "更新指纹（UA {}，时区 {}，警告 {} 条）",
            env.fingerprint.user_agent,
            env.fingerprint.timezone,
            validation.warnings.len()
        ),
    )?;
    Ok(env)
}

pub async fn apply_fingerprint_if_running(state: &SharedState, env: &Environment) -> Result<()> {
    if state.browsers.is_running(&env.id) {
        state
            .browsers
            .apply_fingerprint(&env.id, &env.fingerprint)
            .await?;
    }
    Ok(())
}

pub fn create_snapshot(
    state: &SharedState,
    env_id: &str,
    name: Option<String>,
    actor: &str,
) -> Result<FingerprintSnapshot> {
    let env = state.db.get_environment(env_id)?;
    let snapshot = FingerprintSnapshot {
        id: new_id(),
        env_id: env.id.clone(),
        name: name.filter(|n| !n.trim().is_empty()).unwrap_or_else(|| {
            format!("快照 {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
        }),
        fingerprint: env.fingerprint.clone(),
        create_time: now_iso(),
    };
    state.db.insert_snapshot(&snapshot)?;
    state.db.set_environment_snapshot(env_id, &snapshot.id)?;
    Audit::env(
        &state.db,
        actor,
        actions::SNAPSHOT_CREATE,
        &env,
        format!("创建指纹快照 {} ({})", snapshot.name, snapshot.id),
    )?;
    Ok(snapshot)
}

pub async fn restore_snapshot(
    state: &SharedState,
    snapshot_id: &str,
    actor: &str,
) -> Result<Environment> {
    let snapshot = state.db.get_snapshot(snapshot_id)?;
    let validation = fingerprint::validate(&snapshot.fingerprint);
    if !validation.valid {
        return Err(validation_error(&validation, "快照指纹参数冲突"));
    }
    let mut env = state.db.get_environment(&snapshot.env_id)?;
    env.fingerprint = snapshot.fingerprint.clone();
    env.fingerprint_snapshot_id = Some(snapshot.id.clone());
    env.updated_at = now_iso();
    state.db.update_environment(&env)?;
    if state.browsers.is_running(&env.id) {
        state
            .browsers
            .apply_fingerprint(&env.id, &env.fingerprint)
            .await?;
    }
    Audit::env(
        &state.db,
        actor,
        actions::SNAPSHOT_RESTORE,
        &env,
        format!("从快照 {} ({}) 还原指纹", snapshot.name, snapshot.id),
    )?;
    Ok(env)
}

// ---------------------------------------------------------------------------
// Proxies
// ---------------------------------------------------------------------------

pub fn create_proxy(state: &SharedState, input: ProxyInput, actor: &str) -> Result<Proxy> {
    validate_proxy_input(&input)?;
    let now = now_iso();
    let password = input.password.unwrap_or_default();
    let proxy = Proxy {
        id: new_id(),
        name: input.name.trim().to_string(),
        protocol: input.protocol,
        host: input.host.trim().to_string(),
        port: input.port,
        username: input.username.unwrap_or_default(),
        password_enc: if password.is_empty() {
            String::new()
        } else {
            state.crypto.encrypt(&password)?
        },
        has_password: !password.is_empty(),
        enabled: input.enabled.unwrap_or(true),
        last_check_at: None,
        latency_ms: None,
        exit_ip: None,
        exit_country: None,
        exit_timezone: None,
        created_at: now.clone(),
        updated_at: now,
    };
    state.db.insert_proxy(&proxy)?;
    Audit::global(
        &state.db,
        actor,
        actions::PROXY_CREATE,
        format!(
            "新增代理 {} ({}，认证 {})",
            proxy.name,
            proxy.url(),
            if proxy.username.is_empty() {
                "无"
            } else {
                "有"
            }
        ),
    )?;
    Ok(proxy)
}

pub fn update_proxy(
    state: &SharedState,
    id: &str,
    input: UpdateProxyInput,
    actor: &str,
) -> Result<Proxy> {
    let mut proxy = state.db.get_proxy(id)?;
    if let Some(name) = input.name {
        if name.trim().is_empty() {
            return Err(AppError::bad_request("代理名称不能为空"));
        }
        proxy.name = name.trim().to_string();
    }
    if let Some(protocol) = input.protocol {
        if !crate::models::PROXY_PROTOCOLS.contains(&protocol.as_str()) {
            return Err(AppError::bad_request("代理协议仅支持 http/https/socks5"));
        }
        proxy.protocol = protocol;
    }
    if let Some(host) = input.host {
        if host.trim().is_empty() {
            return Err(AppError::bad_request("代理主机不能为空"));
        }
        proxy.host = host.trim().to_string();
    }
    if let Some(port) = input.port {
        if port == 0 {
            return Err(AppError::bad_request("代理端口不合法"));
        }
        proxy.port = port;
    }
    if let Some(username) = input.username {
        proxy.username = username;
    }
    if let Some(password) = input.password {
        proxy.password_enc = if password.is_empty() {
            String::new()
        } else {
            state.crypto.encrypt(&password)?
        };
        proxy.has_password = !password.is_empty();
    }
    if let Some(enabled) = input.enabled {
        proxy.enabled = enabled;
    }
    proxy.updated_at = now_iso();
    state.db.update_proxy(&proxy)?;
    Audit::global(
        &state.db,
        actor,
        actions::PROXY_UPDATE,
        format!("更新代理 {} ({})", proxy.name, proxy.url()),
    )?;
    Ok(proxy)
}

pub fn delete_proxy(state: &SharedState, id: &str, actor: &str) -> Result<()> {
    let proxy = state.db.get_proxy(id)?;
    let bound = state.db.proxy_bound_env_count(id)?;
    Audit::global(
        &state.db,
        actor,
        actions::PROXY_DELETE,
        format!(
            "删除代理 {} ({}，影响 {bound} 个环境)",
            proxy.name,
            proxy.url()
        ),
    )?;
    state.db.delete_proxy(id)?;
    Ok(())
}

/// Resolve and persist the last observed egress IP for an environment. Uses the
/// bound proxy when present, otherwise the host's direct connection.
pub async fn check_environment_ip(
    state: &SharedState,
    env_id: &str,
    actor: &str,
) -> Result<EnvironmentIpInfo> {
    let env = state.db.get_environment(env_id)?;
    let checked_at = now_iso();

    let (source, proxy_id, lookup, latency_ms) = match env.proxy_id.clone() {
        Some(proxy_id) => {
            let proxy = state.db.get_proxy(&proxy_id)?;
            let password = if proxy.password_enc.is_empty() {
                None
            } else {
                Some(state.crypto.decrypt(&proxy.password_enc)?)
            };
            let check = proxy_check::check_proxy(&state.db, &proxy, password.as_deref()).await?;
            let country_code = check
                .country
                .as_deref()
                .filter(|value| value.len() == 2)
                .map(ToString::to_string);
            let country = check
                .country
                .as_deref()
                .filter(|value| value.len() > 2)
                .map(ToString::to_string);
            (
                "proxy".to_string(),
                Some(proxy_id),
                proxy_check::ExitLookup {
                    ip: check.exit_ip.clone(),
                    country,
                    country_code: country_code.clone(),
                    city: check.city.clone(),
                    timezone: check.timezone.clone(),
                    message: check.message.clone(),
                },
                check.latency_ms,
            )
        }
        None => {
            let (lookup, latency) = proxy_check::check_direct().await?;
            ("direct".to_string(), None, lookup, Some(latency))
        }
    };

    let info = EnvironmentIpInfo {
        env_id: env_id.to_string(),
        ok: lookup.ip.is_some(),
        ip: lookup.ip,
        country: lookup.country,
        country_code: lookup.country_code,
        city: lookup.city,
        timezone: lookup.timezone,
        source,
        proxy_id,
        latency_ms,
        message: lookup.message,
        checked_at,
    };
    state.db.save_environment_ip(&info)?;
    if let Ok(updated) = state.db.get_environment(env_id) {
        let refreshed = refresh_start_page(state, &updated);
        if refreshed
            && state
                .browsers
                .get_runtime(env_id)
                .map(|runtime| runtime.current_url.contains("/start-pages/"))
                .unwrap_or(false)
        {
            let _ = state.browsers.reload(env_id).await;
        }
    }

    Audit::env(
        &state.db,
        actor,
        actions::ENV_IP_CHECK,
        &env,
        format!(
            "检测出口 IP：{}（{}，耗时 {}ms）",
            info.ip.as_deref().unwrap_or("未获取"),
            info.message,
            info.latency_ms.unwrap_or(0)
        ),
    )?;

    Ok(info)
}

pub async fn check_proxy(state: &SharedState, id: &str, actor: &str) -> Result<ProxyCheckResult> {
    let proxy = state.db.get_proxy(id)?;
    let password = if proxy.password_enc.is_empty() {
        None
    } else {
        Some(state.crypto.decrypt(&proxy.password_enc)?)
    };
    let result = proxy_check::check_proxy(&state.db, &proxy, password.as_deref()).await?;
    Audit::global(
        &state.db,
        actor,
        actions::PROXY_CHECK,
        format!(
            "检测代理 {}：{}，出口 {}，时区 {:?}",
            proxy.name,
            result.message,
            result.exit_ip.as_deref().unwrap_or("-"),
            result.timezone
        ),
    )?;
    Ok(result)
}

pub fn assign_proxy(
    state: &SharedState,
    env_ids: &[String],
    proxy_id: Option<&str>,
    actor: &str,
) -> Result<u32> {
    if let Some(id) = proxy_id {
        state.db.get_proxy(id)?;
    }
    let count = state.db.set_environment_proxy(env_ids, proxy_id)? as u32;
    Audit::global(
        &state.db,
        actor,
        actions::PROXY_ASSIGN,
        format!(
            "批量分配代理 {} 到 {} 个环境",
            proxy_id.unwrap_or("直连"),
            count
        ),
    )?;
    Ok(count)
}

pub async fn align_proxy(
    state: &SharedState,
    env_id: &str,
    actor: &str,
) -> Result<AlignProxyResult> {
    let env = state.db.get_environment(env_id)?;
    let proxy_id = env
        .proxy_id
        .clone()
        .ok_or_else(|| AppError::bad_request("环境未绑定代理"))?;
    let check = check_proxy(state, &proxy_id, actor).await?;
    let country = check
        .country
        .clone()
        .ok_or_else(|| AppError::bad_request("代理检测未返回国家，无法对齐时区"))?;
    let mut updated = env.clone();
    fingerprint::apply_country_locale(&mut updated.fingerprint, &country);
    let validation = fingerprint::validate(&updated.fingerprint);
    if !validation.valid {
        return Err(validation_error(&validation, "代理时区对齐后指纹冲突"));
    }
    updated.updated_at = now_iso();
    state.db.update_environment(&updated)?;
    let _ = refresh_start_page(state, &updated);
    if state.browsers.is_running(env_id) {
        state
            .browsers
            .apply_fingerprint(env_id, &updated.fingerprint)
            .await?;
    }
    Audit::env(
        &state.db,
        actor,
        actions::PROXY_ALIGN,
        &updated,
        format!(
            "按代理出口 {} 对齐时区 {} 与语言 {}",
            country,
            updated.fingerprint.timezone,
            updated.fingerprint.languages.join(",")
        ),
    )?;
    Ok(AlignProxyResult {
        environment: environment_view(state, updated)?,
        check,
    })
}

fn validate_proxy_input(input: &ProxyInput) -> Result<()> {
    if input.name.trim().is_empty() {
        return Err(AppError::bad_request("代理名称不能为空"));
    }
    if !crate::models::PROXY_PROTOCOLS.contains(&input.protocol.as_str()) {
        return Err(AppError::bad_request("代理协议仅支持 http/https/socks5"));
    }
    if input.host.trim().is_empty() {
        return Err(AppError::bad_request("代理主机不能为空"));
    }
    if input.port == 0 {
        return Err(AppError::bad_request("代理端口不合法"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings, theme, stats
// ---------------------------------------------------------------------------

const RESERVED_LAUNCH_ARGS: &[&str] = &[
    "--user-data-dir",
    "--profile-directory",
    "--remote-debugging-port",
    "--remote-debugging-address",
    "--remote-allow-origins",
    "--proxy-server",
    "--proxy-bypass-list",
    "--host-resolver-rules",
    "--load-extension",
    "--window-size",
    "--window-name",
    "--lang",
    "--accept-lang",
];

fn sanitize_launch_args(args: &[String]) -> Result<Vec<String>> {
    if args.len() > 64 {
        return Err(AppError::bad_request("附加启动参数不能超过 64 项"));
    }
    let mut out = Vec::new();
    for raw in args {
        let arg = raw.trim();
        if arg.is_empty() {
            continue;
        }
        if arg.len() > 512 {
            return Err(AppError::bad_request("单个启动参数不能超过 512 个字符"));
        }
        if !arg.starts_with("--") {
            return Err(AppError::bad_request(format!(
                "启动参数必须使用 --flag 或 --flag=value 格式: {arg}"
            )));
        }
        if arg.chars().any(|value| matches!(value, '\0' | '\r' | '\n')) {
            return Err(AppError::bad_request("启动参数不能包含控制字符"));
        }
        let name = arg.split('=').next().unwrap_or(arg);
        if RESERVED_LAUNCH_ARGS.contains(&name) {
            return Err(AppError::bad_request(format!(
                "不能覆盖环境隔离参数: {name}"
            )));
        }
        if !out.iter().any(|existing| existing == arg) {
            out.push(arg.to_string());
        }
    }
    Ok(out)
}

pub async fn update_settings(
    state: &SharedState,
    input: UpdateSettingsInput,
    actor: &str,
) -> Result<Settings> {
    let mut settings = state.settings();
    if let Some(v) = input.max_concurrent_browsers {
        if !(1..=100).contains(&v) {
            return Err(AppError::bad_request(
                "最大并发浏览器数量必须在 1 - 100 之间",
            ));
        }
        settings.max_concurrent_browsers = v;
    }
    if let Some(v) = input.idle_timeout_min {
        if v > 1440 {
            return Err(AppError::bad_request("闲置超时不能超过 1440 分钟"));
        }
        settings.idle_timeout_min = v;
    }
    if let Some(v) = input.memory_limit_mb {
        if v < 128 {
            return Err(AppError::bad_request("内存告警阈值不能低于 128MB"));
        }
        settings.memory_limit_mb = v;
    }
    if let Some(v) = input.memory_enforce {
        settings.memory_enforce = v;
    }
    if let Some(v) = input.auto_restart_on_crash {
        settings.auto_restart_on_crash = v;
    }
    if let Some(v) = input.api_enabled {
        settings.api_enabled = v;
    }
    if let Some(v) = input.api_host {
        if v.trim().is_empty() {
            return Err(AppError::bad_request("API 监听地址不能为空"));
        }
        settings.api_host = v.trim().to_string();
    }
    if let Some(v) = input.api_port {
        if v < 1024 {
            return Err(AppError::bad_request("API 端口必须大于 1024"));
        }
        settings.api_port = v;
    }
    let api_changed = settings.api_enabled != state.settings().api_enabled
        || settings.api_host != state.settings().api_host
        || settings.api_port != state.settings().api_port;
    if let Some(path) = input.chromium_path {
        settings.chromium_path = path.filter(|p| !p.trim().is_empty());
    }
    if let Some(args) = input.chromium_launch_args {
        settings.chromium_launch_args = sanitize_launch_args(&args)?;
    }
    if let Some(url) = input.default_start_url {
        settings.default_start_url = url.trim().to_string();
    }
    if let Some(url) = input.detection_service_url {
        settings.detection_service_url = url.trim().to_string();
    }
    if let Some(v) = input.allow_sensitive_export {
        settings.allow_sensitive_export = v;
    }
    if let Some(v) = input.callback_type {
        if !["generic", "zentao", "jira"].contains(&v.as_str()) {
            return Err(AppError::bad_request("回调类型仅支持 generic/zentao/jira"));
        }
        settings.callback_type = v;
    }
    if let Some(url) = input.callback_url {
        settings.callback_url = url.filter(|u| !u.trim().is_empty());
    }
    if let Some(token) = input.callback_token {
        settings.callback_token_enc = if token.is_empty() {
            Some(String::new())
        } else {
            Some(state.crypto.encrypt(&token)?)
        };
    }
    if let Some(v) = input.sync_enabled {
        settings.sync_enabled = v;
    }
    if let Some(v) = input.sync_kind {
        if !["webdav", "folder"].contains(&v.as_str()) {
            return Err(AppError::bad_request("同步类型仅支持 webdav/folder"));
        }
        settings.sync_kind = v;
    }
    if let Some(v) = input.sync_endpoint {
        settings.sync_endpoint = v.filter(|s| !s.trim().is_empty());
    }
    if let Some(v) = input.sync_username {
        settings.sync_username = v.filter(|s| !s.trim().is_empty());
    }
    if let Some(password) = input.sync_password {
        settings.sync_password_enc = if password.is_empty() {
            Some(String::new())
        } else {
            Some(state.crypto.encrypt(&password)?)
        };
    }
    if let Some(path) = input.sync_remote_path {
        if path.trim().is_empty() {
            return Err(AppError::bad_request("远端文件名不能为空"));
        }
        settings.sync_remote_path = path.trim().to_string();
    }
    if let Some(v) = input.close_to_tray {
        settings.close_to_tray = v;
    }
    if let Some(v) = input.global_shortcut {
        settings.global_shortcut = v;
    }
    if let Some(v) = input.launch_at_login {
        settings.launch_at_login = v;
    }
    if let Some(level) = input.log_level {
        if !["trace", "debug", "info", "warn", "error"].contains(&level.as_str()) {
            return Err(AppError::bad_request("日志级别不合法"));
        }
        settings.log_level = level;
    }
    if let Some(url) = input.download_manifest_url {
        settings.download_manifest_url = url.filter(|u| !u.trim().is_empty());
    }
    if settings.sync_enabled && settings.sync_endpoint.as_deref().unwrap_or("").is_empty() {
        return Err(AppError::bad_request("开启同步前请配置同步地址"));
    }
    settings.callback_token_set = settings
        .callback_token_enc
        .as_deref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    settings.sync_password_set = settings
        .sync_password_enc
        .as_deref()
        .map(|v| !v.is_empty())
        .unwrap_or(false);
    state.store_settings(&settings)?;
    if let Some(app) = state.app_handle() {
        crate::autostart::apply(&app, settings.launch_at_login);
    }
    if api_changed {
        if settings.api_enabled {
            let _ = state.api.restart(state.clone()).await;
            let _ = Audit::global(
                &state.db,
                actor,
                actions::API_START,
                format!(
                    "API 服务已重启于 {}:{}",
                    settings.api_host, settings.api_port
                ),
            );
        } else {
            state.api.stop().await;
            let _ = Audit::global(&state.db, actor, actions::API_STOP, "API 服务已停止");
        }
    }
    Audit::global(
        &state.db,
        actor,
        actions::SETTINGS_UPDATE,
        format!(
            "更新系统设置（最大并发 {}，闲置 {} 分钟，内存阈值 {}MB，自动关闭 {}）",
            settings.max_concurrent_browsers,
            settings.idle_timeout_min,
            settings.memory_limit_mb,
            settings.memory_enforce
        ),
    )?;
    Ok(settings)
}

pub fn save_theme(state: &SharedState, theme: ThemeConfig, actor: &str) -> Result<ThemeConfig> {
    if !["light", "dark", "auto"].contains(&theme.mode.as_str()) {
        return Err(AppError::bad_request("主题模式仅支持 light/dark/auto"));
    }
    if !["compact", "comfortable"].contains(&theme.density.as_str()) {
        return Err(AppError::bad_request("布局密度仅支持 compact/comfortable"));
    }
    if !is_hex_color(&theme.primary_color) {
        return Err(AppError::bad_request("主色必须是 #RRGGBB 格式"));
    }
    let mut theme = theme;
    theme.updated_at = now_iso();
    state.db.save_theme(&theme)?;
    Audit::global(
        &state.db,
        actor,
        actions::THEME_UPDATE,
        format!(
            "更新主题（{}，主色 {}，密度 {}）",
            theme.mode, theme.primary_color, theme.density
        ),
    )?;
    Ok(theme)
}

pub fn is_hex_color(value: &str) -> bool {
    let mut chars = value.chars();
    if chars.next() != Some('#') {
        return false;
    }
    let rest: String = chars.collect();
    rest.len() == 6 && rest.chars().all(|c| c.is_ascii_hexdigit())
}

pub async fn dashboard(state: &SharedState) -> Result<DashboardStats> {
    let settings = state.settings();
    let mut stats = state
        .db
        .dashboard_counts(settings.max_concurrent_browsers)?;
    let snapshot = state.browsers.resource_snapshot().await;
    stats.total_memory_mb = snapshot.total_memory_mb;
    stats.running_count = snapshot.running_count;
    Ok(stats)
}

// ---------------------------------------------------------------------------
// Users / API keys
// ---------------------------------------------------------------------------

pub fn create_user(state: &SharedState, input: UserInput, actor: &str) -> Result<User> {
    validate_username(&input.username)?;
    if !capability::role_exists(&input.role) {
        return Err(AppError::bad_request("角色不合法"));
    }
    if input.password.chars().count() < 8 {
        return Err(AppError::bad_request("密码至少需要 8 个字符"));
    }
    if state
        .db
        .list_users()?
        .iter()
        .any(|u| u.username == input.username)
    {
        return Err(AppError::conflict("用户名已存在"));
    }
    let user = User {
        id: new_id(),
        username: input.username,
        display_name: if input.display_name.trim().is_empty() {
            "未命名用户".into()
        } else {
            input.display_name.trim().to_string()
        },
        role: input.role,
        enabled: input.enabled.unwrap_or(true),
        created_at: now_iso(),
        last_login_at: None,
    };
    let hash = state.crypto.hash_password(&input.password)?;
    state.db.insert_user(&user, &hash)?;
    Audit::global(
        &state.db,
        actor,
        actions::USER_CREATE,
        format!("创建用户 {}（角色 {}）", user.username, user.role),
    )?;
    Ok(user)
}

pub fn update_user(
    state: &SharedState,
    id: &str,
    input: UserUpdateInput,
    actor: &str,
) -> Result<User> {
    let mut user = state.db.get_user(id)?;
    if let Some(name) = input.display_name {
        user.display_name = if name.trim().is_empty() {
            user.username.clone()
        } else {
            name.trim().to_string()
        };
    }
    if let Some(role) = input.role {
        if !capability::role_exists(&role) {
            return Err(AppError::bad_request("角色不合法"));
        }
        if user.role == "super_admin" && role != "super_admin" && state.db.super_admin_count()? <= 1
        {
            return Err(AppError::bad_request("必须保留至少一个超级管理员"));
        }
        user.role = role;
    }
    if let Some(enabled) = input.enabled {
        if !enabled && user.role == "super_admin" && state.db.super_admin_count()? <= 1 {
            return Err(AppError::bad_request("必须保留至少一个启用的超级管理员"));
        }
        user.enabled = enabled;
    }
    let password_hash = match input.password.filter(|p| !p.is_empty()) {
        Some(password) => {
            if password.chars().count() < 8 {
                return Err(AppError::bad_request("密码至少需要 8 个字符"));
            }
            Some(state.crypto.hash_password(&password)?)
        }
        None => None,
    };
    state.db.update_user(&user, password_hash.as_deref())?;
    if let Ok(mut session) = state.session.write() {
        if session.as_ref().map(|u| u.id.clone()) == Some(user.id.clone()) {
            *session = Some(user.clone());
        }
    }
    Audit::global(
        &state.db,
        actor,
        actions::USER_UPDATE,
        format!(
            "更新用户 {}（角色 {}，启用 {}）",
            user.username, user.role, user.enabled
        ),
    )?;
    Ok(user)
}

pub fn delete_user(state: &SharedState, id: &str, actor: &str) -> Result<()> {
    let user = state.db.get_user(id)?;
    if user.username == actor {
        return Err(AppError::bad_request("不能删除当前登录用户"));
    }
    if user.role == "super_admin" && state.db.super_admin_count()? <= 1 {
        return Err(AppError::bad_request("必须保留至少一个超级管理员"));
    }
    Audit::global(
        &state.db,
        actor,
        actions::USER_DELETE,
        format!("删除用户 {}", user.username),
    )?;
    state.db.delete_user(id)?;
    Ok(())
}

fn validate_username(username: &str) -> Result<()> {
    let len = username.chars().count();
    if !(3..=32).contains(&len)
        || !username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
    {
        return Err(AppError::bad_request(
            "用户名需为 3-32 位字母、数字、下划线、短横线或点",
        ));
    }
    Ok(())
}

pub fn create_api_key(
    state: &SharedState,
    name: &str,
    role: &str,
    actor: &str,
) -> Result<CreatedApiKey> {
    if !capability::role_exists(role) {
        return Err(AppError::bad_request("角色不合法"));
    }
    let secret = format!("finenv_{}", hex::encode(rand::random::<[u8; 24]>()));
    let prefix = secret.chars().take(16).collect::<String>();
    let key = ApiKey {
        id: new_id(),
        name: if name.trim().is_empty() {
            "未命名密钥".into()
        } else {
            name.trim().to_string()
        },
        prefix,
        role: role.to_string(),
        enabled: true,
        created_at: now_iso(),
        last_used_at: None,
    };
    state.db.insert_api_key(&key, &hash_api_key(&secret))?;
    Audit::global(
        &state.db,
        actor,
        actions::API_KEY_CREATE,
        format!("创建 API 密钥 {}（角色 {}）", key.name, key.role),
    )?;
    Ok(CreatedApiKey { key, secret })
}

// ---------------------------------------------------------------------------
// Extensions / hosts / mocks
// ---------------------------------------------------------------------------

pub fn add_extension(
    state: &SharedState,
    name: Option<String>,
    path: &str,
    actor: &str,
) -> Result<Extension> {
    let dir = Path::new(path);
    if !dir.is_dir() {
        return Err(AppError::bad_request("扩展目录不存在"));
    }
    let manifest_path = dir.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(AppError::bad_request("扩展目录缺少 manifest.json"));
    }
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(&manifest_path)?)
        .map_err(|e| AppError::bad_request(format!("扩展 manifest.json 格式无效: {e}")))?;
    let manifest_name = manifest
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("未命名扩展")
        .to_string();
    let extension = Extension {
        id: new_id(),
        name: name
            .filter(|n| !n.trim().is_empty())
            .unwrap_or(manifest_name),
        path: path.to_string(),
        enabled: true,
        description: manifest
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        created_at: now_iso(),
        assigned_count: 0,
    };
    state.db.insert_extension(&extension)?;
    Audit::global(
        &state.db,
        actor,
        actions::EXTENSION_ADD,
        format!("安装扩展 {} ({})", extension.name, extension.path),
    )?;
    Ok(extension)
}

pub fn update_extension(
    state: &SharedState,
    id: &str,
    input: crate::models::UpdateExtensionInput,
    actor: &str,
) -> Result<Extension> {
    let mut extension = state.db.get_extension(id)?;
    if let Some(name) = input.name {
        if name.trim().is_empty() {
            return Err(AppError::bad_request("扩展名称不能为空"));
        }
        extension.name = name.trim().to_string();
    }
    if let Some(description) = input.description {
        extension.description = description.trim().to_string();
    }
    if let Some(enabled) = input.enabled {
        extension.enabled = enabled;
    }
    state.db.update_extension(&extension)?;
    Audit::global(
        &state.db,
        actor,
        actions::EXTENSION_UPDATE,
        format!(
            "更新扩展 {}（{}）",
            extension.name,
            if extension.enabled {
                "启用"
            } else {
                "停用"
            }
        ),
    )?;
    Ok(extension)
}

pub fn assign_extension_to_all(
    state: &SharedState,
    extension_id: &str,
    actor: &str,
) -> Result<u32> {
    let extension = state.db.get_extension(extension_id)?;
    let env_ids = state
        .db
        .list_environments(&Default::default())?
        .into_iter()
        .map(|env| env.id)
        .collect::<Vec<_>>();
    state.db.assign_extension(extension_id, &env_ids)?;
    Audit::global(
        &state.db,
        actor,
        actions::EXTENSION_ASSIGN,
        format!(
            "扩展 {} 应用到全部 {} 个环境",
            extension.name,
            env_ids.len()
        ),
    )?;
    Ok(env_ids.len() as u32)
}

pub async fn install_extension_online(
    state: &SharedState,
    source: &str,
    env_ids: &[String],
    actor: &str,
) -> Result<Extension> {
    let store_id = parse_store_extension_id(source)?;
    for env_id in env_ids {
        state.db.get_environment(env_id)?;
    }
    let bytes = download_store_crx(&store_id).await?;
    let target = paths::extension_store_dir(&state.data_dir).join(format!(
        "{}-{}",
        store_id,
        &new_id()[..8]
    ));
    let manifest = match extract_crx(&bytes, &target) {
        Ok(manifest) => manifest,
        Err(error) => {
            let _ = std::fs::remove_dir_all(&target);
            return Err(error);
        }
    };
    let fallback_name = format!("Chrome 扩展 {}", &store_id[..8]);
    let name = manifest
        .get("name")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty() && !value.starts_with("__MSG_"))
        .unwrap_or(&fallback_name)
        .to_string();
    let manifest_version = manifest
        .get("manifest_version")
        .and_then(|value| value.as_u64())
        .unwrap_or(3);
    let mut description = manifest
        .get("description")
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty() && !value.starts_with("__MSG_"))
        .unwrap_or("Chrome 应用商店在线安装")
        .to_string();
    if manifest_version < 3 {
        description.push_str(" · Manifest V2，Chrome 137+ 可能不再加载");
    }
    let extension = Extension {
        id: new_id(),
        name,
        path: target.to_string_lossy().to_string(),
        enabled: true,
        description,
        created_at: now_iso(),
        assigned_count: env_ids.len() as u32,
    };
    if let Err(error) = state.db.insert_extension(&extension) {
        let _ = std::fs::remove_dir_all(&target);
        return Err(error);
    }
    if let Err(error) = state.db.assign_extension(&extension.id, env_ids) {
        let _ = state.db.delete_extension(&extension.id);
        let _ = std::fs::remove_dir_all(&target);
        return Err(error);
    }
    Audit::global(
        &state.db,
        actor,
        actions::EXTENSION_ADD,
        format!(
            "在线安装扩展 {}（Chrome 应用商店 {}，分配到 {} 个环境）",
            extension.name,
            store_id,
            env_ids.len()
        ),
    )?;
    Ok(extension)
}

pub fn parse_store_extension_id(source: &str) -> Result<String> {
    let value = source.trim();
    let valid = |candidate: &str| {
        candidate.len() == 32 && candidate.chars().all(|ch| ('a'..='p').contains(&ch))
    };
    if valid(value) {
        return Ok(value.to_string());
    }
    if let Ok(url) = url::Url::parse(value) {
        if let Some(host) = url.host_str() {
            let allowed = host == "chromewebstore.google.com"
                || host == "chrome.google.com"
                || host.ends_with(".google.com");
            if allowed {
                if let Some(segment) = url
                    .path_segments()
                    .and_then(|mut segments| segments.find(|segment| valid(segment)))
                {
                    return Ok(segment.to_string());
                }
            }
        }
    }
    Err(AppError::bad_request(
        "请输入 Chrome 应用商店链接或 32 位扩展 ID（a-p）",
    ))
}

pub fn store_extension_url(source: &str) -> Result<String> {
    let id = parse_store_extension_id(source)?;
    Ok(format!("https://chromewebstore.google.com/detail/{id}"))
}

async fn download_store_crx(store_id: &str) -> Result<Vec<u8>> {
    const MAX_CRX_BYTES: usize = 80 * 1024 * 1024;
    // CI and offline installations can point this at a local CRX mirror.
    let base = std::env::var("FINENV_CRX_BASE_URL")
        .map(|value| value.trim_end_matches('/').to_string())
        .unwrap_or_else(|_| "https://clients2.google.com".to_string());
    let url = format!(
        "{base}/service/update2/crx?response=redirect&os=win&arch=x86_64&os_arch=x86_64&nacl_arch=x86-64&prod=chromecrx&prodchannel=stable&prodversion=130.0.0.0&lang=zh-CN&acceptformat=crx2,crx3&x=id%3D{store_id}%26uc"
    );
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinEnv/1.0")
        .build()
        .map_err(AppError::from)?;
    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|error| AppError::bad_request(format!("Chrome 应用商店下载失败: {error}")))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "Chrome 应用商店下载失败: HTTP {}",
            response.status()
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|error| AppError::bad_request(format!("读取扩展安装包失败: {error}")))?;
    if bytes.is_empty() || bytes.len() > MAX_CRX_BYTES {
        return Err(AppError::bad_request("扩展安装包为空或超过 80MB 限制"));
    }
    Ok(bytes.to_vec())
}

fn crx_payload(bytes: &[u8]) -> Result<&[u8]> {
    if bytes.starts_with(b"PK\x03\x04") {
        return Ok(bytes);
    }
    if bytes.len() < 12 || &bytes[0..4] != b"Cr24" {
        return Err(AppError::bad_request("应用商店返回的不是有效的 CRX 扩展包"));
    }
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let (header_size, prefix) = match version {
        2 => {
            if bytes.len() < 16 {
                return Err(AppError::bad_request("CRX2 扩展包头不完整"));
            }
            let public_key =
                u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
            let signature =
                u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
            (public_key.saturating_add(signature), 16usize)
        }
        3 => {
            let size = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize;
            (size, 12usize)
        }
        other => return Err(AppError::bad_request(format!("不支持的 CRX 版本: {other}"))),
    };
    let offset = prefix.saturating_add(header_size);
    if offset >= bytes.len() {
        return Err(AppError::bad_request("CRX 扩展包数据不完整"));
    }
    let payload = &bytes[offset..];
    if !payload.starts_with(b"PK") {
        return Err(AppError::bad_request("CRX 中未找到 ZIP 数据"));
    }
    Ok(payload)
}

fn extract_crx(bytes: &[u8], target: &Path) -> Result<serde_json::Value> {
    const MAX_EXTRACTED_BYTES: u64 = 120 * 1024 * 1024;
    const MAX_ENTRIES: usize = 6000;

    paths::ensure_dir(target)?;
    let payload = crx_payload(bytes)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(payload))
        .map_err(|error| AppError::bad_request(format!("扩展包解压失败: {error}")))?;
    if archive.len() > MAX_ENTRIES {
        return Err(AppError::bad_request("扩展包文件数量过多"));
    }
    let mut extracted = 0u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| AppError::bad_request(format!("读取扩展文件失败: {error}")))?;
        let Some(relative) = entry.enclosed_name() else {
            continue;
        };
        if entry.is_dir() {
            paths::ensure_dir(&target.join(&relative))?;
            continue;
        }
        if entry
            .unix_mode()
            .map(|mode| mode & 0o170000 == 0o120000)
            .unwrap_or(false)
        {
            // Skip symlinks; they are not needed by extensions and can escape
            // the extraction directory on Unix.
            continue;
        }
        extracted = extracted.saturating_add(entry.size());
        if extracted > MAX_EXTRACTED_BYTES {
            return Err(AppError::bad_request("扩展解压后超过 120MB 限制"));
        }
        let destination = target.join(&relative);
        if let Some(parent) = destination.parent() {
            paths::ensure_dir(parent)?;
        }
        let mut output = std::fs::File::create(&destination)?;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = entry.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            output.write_all(&buffer[..read])?;
        }
    }
    let manifest_path = target.join("manifest.json");
    if !manifest_path.is_file() {
        return Err(AppError::bad_request("扩展包缺少 manifest.json"));
    }
    serde_json::from_slice(&std::fs::read(manifest_path)?)
        .map_err(|error| AppError::bad_request(format!("扩展 manifest.json 无效: {error}")))
}

pub fn set_hosts(
    state: &SharedState,
    env_id: &str,
    entries: &[HostsEntryInput],
    actor: &str,
) -> Result<Vec<crate::models::HostsEntry>> {
    let env = state.db.get_environment(env_id)?;
    let result = state.db.replace_hosts(env_id, entries)?;
    Audit::env(
        &state.db,
        actor,
        actions::HOSTS_UPDATE,
        &env,
        format!("更新独立 Hosts {} 条（需重启环境生效）", result.len()),
    )?;
    Ok(result)
}

pub async fn set_mocks(
    state: &SharedState,
    env_id: &str,
    rules: &[MockRuleInput],
    actor: &str,
) -> Result<Vec<crate::models::MockRule>> {
    let env = state.db.get_environment(env_id)?;
    let records = state.db.replace_mocks(env_id, rules)?;
    if state.browsers.is_running(env_id) {
        state.browsers.set_mocks(env_id, records.clone()).await?;
    }
    Audit::env(
        &state.db,
        actor,
        actions::MOCK_UPDATE,
        &env,
        format!("更新 Mock 规则 {} 条（运行中立即生效）", records.len()),
    )?;
    Ok(records)
}

// ---------------------------------------------------------------------------
// Audit export
// ---------------------------------------------------------------------------

pub fn audit_export(
    state: &SharedState,
    filter: &AuditFilter,
    path: &Path,
    format: &str,
) -> Result<u64> {
    let logs = state.db.list_audit(filter)?;
    let count = logs.len() as u64;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if format.eq_ignore_ascii_case("csv") {
        let mut output =
            String::from("time,actor,action,envId,account,project,caseId,detail,hash\n");
        for log in &logs {
            output.push_str(&format!(
                "{},{},{},{},{},{},{},{},{}\n",
                csv_escape(&log.created_at),
                csv_escape(&log.actor),
                csv_escape(&log.action),
                csv_escape(log.env_id.as_deref().unwrap_or("")),
                csv_escape(log.account.as_deref().unwrap_or("")),
                csv_escape(log.project.as_deref().unwrap_or("")),
                csv_escape(log.case_id.as_deref().unwrap_or("")),
                csv_escape(&log.detail),
                csv_escape(&log.hash)
            ));
        }
        std::fs::write(path, output)?;
    } else {
        std::fs::write(path, serde_json::to_vec_pretty(&logs)?)?;
    }
    Ok(count)
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('"') || value.contains('\n') {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}
