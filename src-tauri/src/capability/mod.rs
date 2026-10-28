use serde::{Deserialize, Serialize};

use crate::{
    error::{AppError, Result},
    models::{PermissionDef, ROLES},
};

/// Fine-grained IPC / HTTP permissions. Every Tauri command and API handler
/// calls `AppState::require` or `AuthContext::require` with one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    ViewList,
    OpenClose,
    CreateEnv,
    EditEnv,
    DeleteEnv,
    CloneEnv,
    EditFingerprintProxy,
    ExportSensitive,
    AuditView,
    SettingsManage,
    UserManage,
    ApiKeyManage,
    ManageEnvMembers,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EnvAccess {
    View,
    Operate,
    Manage,
}

impl EnvAccess {
    pub fn label(&self) -> &'static str {
        match self {
            Self::View => "查看",
            Self::Operate => "操作",
            Self::Manage => "管理",
        }
    }
}

pub fn valid_env_access(level: &str) -> bool {
    matches!(level, "view" | "operate" | "manage")
}

pub fn env_access_rank(level: &str) -> Option<u8> {
    match level {
        "view" => Some(0),
        "operate" => Some(1),
        "manage" => Some(2),
        _ => None,
    }
}

pub fn env_access_allows(level: &str, required: EnvAccess) -> bool {
    env_access_rank(level)
        .map(|rank| rank >= required as u8)
        .unwrap_or(false)
}

impl Permission {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::ViewList => "view_list",
            Self::OpenClose => "open_close",
            Self::CreateEnv => "create_env",
            Self::EditEnv => "edit_env",
            Self::DeleteEnv => "delete_env",
            Self::CloneEnv => "clone_env",
            Self::EditFingerprintProxy => "edit_fingerprint_proxy",
            Self::ExportSensitive => "export_sensitive",
            Self::AuditView => "audit_view",
            Self::SettingsManage => "settings_manage",
            Self::UserManage => "user_manage",
            Self::ApiKeyManage => "api_key_manage",
            Self::ManageEnvMembers => "manage_env_members",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::ViewList => "查看列表",
            Self::OpenClose => "打开/关闭环境",
            Self::CreateEnv => "创建环境",
            Self::EditEnv => "编辑环境",
            Self::DeleteEnv => "删除环境",
            Self::CloneEnv => "克隆环境",
            Self::EditFingerprintProxy => "修改指纹与代理",
            Self::ExportSensitive => "导出 Cookie/账号",
            Self::AuditView => "查看审计日志",
            Self::SettingsManage => "系统设置",
            Self::UserManage => "用户与角色",
            Self::ApiKeyManage => "API 密钥管理",
            Self::ManageEnvMembers => "管理环境成员",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::ViewList => "查看环境、账号、代理等列表",
            Self::OpenClose => "启动、关闭、回收浏览器进程",
            Self::CreateEnv => "新建指纹环境",
            Self::EditEnv => "修改环境基础信息与分组",
            Self::DeleteEnv => "删除环境及其账号数据",
            Self::CloneEnv => "复制已有环境",
            Self::EditFingerprintProxy => "修改指纹参数、绑定代理、快照还原",
            Self::ExportSensitive => "查看密码、导出 Cookie/账号、环境包",
            Self::AuditView => "查询和导出审计日志",
            Self::SettingsManage => "资源限制、API 服务、备份同步等设置",
            Self::UserManage => "新增用户、分配角色、重置密码",
            Self::ApiKeyManage => "创建与吊销 API 密钥",
            Self::ManageEnvMembers => "为环境分配团队成员与访问级别",
        }
    }
}

pub fn role_exists(role: &str) -> bool {
    ROLES.contains(&role)
}

/// Role → permission mapping. `super_admin` always has every permission.
pub fn role_permissions(role: &str) -> Vec<Permission> {
    use Permission::*;
    match role {
        "super_admin" => vec![
            ViewList,
            OpenClose,
            CreateEnv,
            EditEnv,
            DeleteEnv,
            CloneEnv,
            EditFingerprintProxy,
            ExportSensitive,
            AuditView,
            SettingsManage,
            UserManage,
            ApiKeyManage,
            ManageEnvMembers,
        ],
        "test_lead" => vec![
            ViewList,
            OpenClose,
            CreateEnv,
            EditEnv,
            CloneEnv,
            EditFingerprintProxy,
            ExportSensitive,
            AuditView,
            SettingsManage,
            ManageEnvMembers,
        ],
        "test_engineer" => vec![
            ViewList,
            OpenClose,
            CreateEnv,
            EditEnv,
            CloneEnv,
            EditFingerprintProxy,
        ],
        "viewer" => vec![ViewList],
        _ => vec![],
    }
}

pub fn has_permission(role: &str, permission: Permission) -> bool {
    role_permissions(role).contains(&permission)
}

pub fn require_role(role: &str, permission: Permission) -> Result<()> {
    if !role_exists(role) {
        return Err(AppError::forbidden(format!("未知角色: {role}")));
    }
    if has_permission(role, permission) {
        Ok(())
    } else {
        Err(AppError::forbidden(format!(
            "当前角色不具备「{}」权限",
            permission.label()
        )))
    }
}

pub fn permission_matrix() -> Vec<PermissionDef> {
    [
        Permission::ViewList,
        Permission::OpenClose,
        Permission::CreateEnv,
        Permission::EditEnv,
        Permission::DeleteEnv,
        Permission::CloneEnv,
        Permission::EditFingerprintProxy,
        Permission::ExportSensitive,
        Permission::AuditView,
        Permission::SettingsManage,
        Permission::UserManage,
        Permission::ApiKeyManage,
        Permission::ManageEnvMembers,
    ]
    .into_iter()
    .map(|perm| PermissionDef {
        key: perm.as_str().into(),
        label: perm.label().into(),
        description: perm.description().into(),
        roles: ROLES
            .iter()
            .filter(|role| has_permission(role, perm))
            .map(|r| r.to_string())
            .collect(),
    })
    .collect()
}

pub fn permission_strings(role: &str) -> Vec<String> {
    role_permissions(role)
        .into_iter()
        .map(|p| p.as_str().to_string())
        .collect()
}

pub fn role_label(role: &str) -> &'static str {
    match role {
        "super_admin" => "超级管理员",
        "test_lead" => "测试负责人",
        "test_engineer" => "测试工程师",
        "viewer" => "只读查看者",
        _ => "未知角色",
    }
}
