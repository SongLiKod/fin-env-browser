use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock},
};

use crate::{
    api_server::ApiServerHandle,
    audit_log::{actions, Audit},
    capability::{self, EnvAccess, Permission},
    crypto::CryptoManager,
    db::Db,
    error::{AppError, Result},
    models::{new_id, now_iso, SessionInfo, Settings, User},
    paths,
    process::BrowserManager,
};

pub type SharedState = Arc<AppState>;

pub struct AppState {
    pub data_dir: PathBuf,
    pub db: Db,
    pub crypto: Arc<CryptoManager>,
    pub settings: Arc<RwLock<Settings>>,
    pub session: RwLock<Option<User>>,
    pub browsers: Arc<BrowserManager>,
    pub api: Arc<ApiServerHandle>,
    pub app_handle: Mutex<Option<tauri::AppHandle>>,
}

impl AppState {
    pub fn initialize() -> Result<SharedState> {
        let data_dir = paths::data_dir();
        Self::initialize_at(data_dir)
    }

    pub fn initialize_at(data_dir: PathBuf) -> Result<SharedState> {
        paths::init_all(&data_dir)?;
        let crypto = Arc::new(CryptoManager::initialize(&data_dir)?);
        let db = Db::open(&paths::db_path(&data_dir))?;
        db.set_crypto(crypto.clone());
        ensure_admin(&db, &crypto)?;
        let settings = Arc::new(RwLock::new(db.load_settings()?));
        let browsers = Arc::new(BrowserManager::new(
            data_dir.clone(),
            db.clone(),
            crypto.clone(),
            settings.clone(),
        ));
        let api = Arc::new(ApiServerHandle::new());
        Ok(Arc::new(Self {
            data_dir,
            db,
            crypto,
            settings,
            session: RwLock::new(None),
            browsers,
            api,
            app_handle: Mutex::new(None),
        }))
    }

    pub fn attach_app(&self, app: tauri::AppHandle) {
        self.browsers.set_app_handle(app.clone());
        if let Ok(mut guard) = self.app_handle.lock() {
            *guard = Some(app);
        }
    }

    pub fn app_handle(&self) -> Option<tauri::AppHandle> {
        self.app_handle.lock().ok().and_then(|g| g.clone())
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn reload_settings(&self) -> Result<Settings> {
        let settings = self.db.load_settings()?;
        if let Ok(mut guard) = self.settings.write() {
            *guard = settings.clone();
        }
        Ok(settings)
    }

    pub fn store_settings(&self, settings: &Settings) -> Result<()> {
        self.db.save_settings(settings)?;
        if let Ok(mut guard) = self.settings.write() {
            *guard = settings.clone();
        }
        Ok(())
    }

    pub fn current_user(&self) -> Option<User> {
        self.session.read().ok().and_then(|s| s.clone())
    }

    pub fn session_info(&self) -> SessionInfo {
        match self.current_user() {
            Some(user) => SessionInfo {
                authenticated: true,
                permissions: capability::permission_strings(&user.role),
                user: Some(user),
            },
            None => SessionInfo {
                authenticated: false,
                user: None,
                permissions: vec![],
            },
        }
    }

    pub fn require_authenticated(&self) -> Result<User> {
        self.current_user()
            .ok_or_else(|| AppError::unauthorized("请先登录 FinEnv"))
    }

    pub fn require(&self, permission: Permission) -> Result<User> {
        let user = self.require_authenticated()?;
        capability::require_role(&user.role, permission)?;
        Ok(user)
    }

    /// Returns `None` for super administrators (all environments) and a set of
    /// explicitly assigned environment IDs for every other role.
    pub fn accessible_environment_ids(&self, user: &User) -> Result<Option<HashSet<String>>> {
        if user.role == "super_admin" {
            return Ok(None);
        }
        Ok(Some(
            self.db
                .list_environment_ids_for_user(&user.id)?
                .into_iter()
                .collect(),
        ))
    }

    pub fn check_environment_access(
        &self,
        user: &User,
        env_id: &str,
        required: EnvAccess,
    ) -> Result<()> {
        if user.role == "super_admin" {
            return Ok(());
        }
        let Some(level) = self.db.environment_member_level(env_id, &user.id)? else {
            return Err(AppError::forbidden("当前团队成员未被分配到该环境"));
        };
        if !capability::env_access_allows(&level, required) {
            return Err(AppError::forbidden(format!(
                "当前成员在该环境只有「{}」权限，无法执行「{}」操作",
                level_label(&level),
                required.label()
            )));
        }
        Ok(())
    }

    pub fn require_sensitive(&self, permission: Permission, confirmed: bool) -> Result<User> {
        let user = self.require(permission)?;
        if !self.settings().allow_sensitive_export {
            return Err(AppError::forbidden(
                "管理员已关闭高危导出权限（可在设置中重新开启）",
            ));
        }
        if !confirmed {
            return Err(AppError::code_with(
                "confirmation_required",
                "该操作属于高危操作，需要二次确认",
            ));
        }
        Ok(user)
    }

    /// Permission + global high-risk switch, without an interactive
    /// confirmation. Used by automation-friendly read paths such as
    /// `GET /cookies` where the API key itself is the explicit grant.
    pub fn require_export(&self) -> Result<User> {
        let user = self.require(Permission::ExportSensitive)?;
        if !self.settings().allow_sensitive_export {
            return Err(AppError::forbidden(
                "管理员已关闭高危导出权限（可在设置中重新开启）",
            ));
        }
        Ok(user)
    }

    pub fn login(&self, username: &str, password: &str) -> Result<SessionInfo> {
        let found = self.db.get_user_with_hash(username)?;
        let Some((user, hash)) = found else {
            let _ = Audit::global(
                &self.db,
                username,
                actions::LOGIN_FAILED,
                "用户名或密码错误",
            );
            return Err(AppError::unauthorized("用户名或密码错误"));
        };
        if !user.enabled {
            let _ = Audit::global(&self.db, username, actions::LOGIN_FAILED, "账号已被禁用");
            return Err(AppError::forbidden("账号已被禁用"));
        }
        if !self.crypto.verify_password(password, &hash) {
            let _ = Audit::global(
                &self.db,
                username,
                actions::LOGIN_FAILED,
                "用户名或密码错误",
            );
            return Err(AppError::unauthorized("用户名或密码错误"));
        }
        self.db.touch_user_login(&user.id)?;
        if let Ok(mut session) = self.session.write() {
            *session = Some(user.clone());
        }
        Audit::global(
            &self.db,
            &user.username,
            actions::LOGIN,
            format!("{} 登录 FinEnv", capability::role_label(&user.role)),
        )?;
        Ok(self.session_info())
    }

    pub fn logout(&self) -> Result<()> {
        if let Some(user) = self.current_user() {
            Audit::global(&self.db, &user.username, actions::LOGOUT, "退出登录")?;
        }
        if let Ok(mut session) = self.session.write() {
            *session = None;
        }
        Ok(())
    }

    pub fn change_password(&self, old_password: &str, new_password: &str) -> Result<()> {
        let user = self.require_authenticated()?;
        if new_password.chars().count() < 8 {
            return Err(AppError::bad_request("新密码至少需要 8 个字符"));
        }
        let (_, hash) = self
            .db
            .get_user_with_hash(&user.username)?
            .ok_or_else(|| AppError::not_found("用户不存在"))?;
        if !self.crypto.verify_password(old_password, &hash) {
            return Err(AppError::bad_request("当前密码不正确"));
        }
        let new_hash = self.crypto.hash_password(new_password)?;
        let mut updated = user.clone();
        updated.display_name = user.display_name.clone();
        self.db.update_user(&updated, Some(&new_hash))?;
        Audit::global(
            &self.db,
            &user.username,
            actions::PASSWORD_CHANGE,
            "修改登录密码",
        )?;
        Ok(())
    }

    pub fn authenticate_api_key(&self, secret: &str) -> Result<Option<crate::models::ApiKey>> {
        let hash = crate::db::settings::hash_api_key(secret);
        self.db.authenticate_api_key(&hash)
    }
}

fn level_label(level: &str) -> &'static str {
    match level {
        "view" => "查看",
        "operate" => "操作",
        "manage" => "管理",
        _ => "未知",
    }
}

fn ensure_admin(db: &Db, crypto: &CryptoManager) -> Result<()> {
    if !db.list_users()?.is_empty() {
        return Ok(());
    }
    let username = std::env::var("FIN_ENV_ADMIN_USER").unwrap_or_else(|_| "admin".into());
    let password = std::env::var("FIN_ENV_ADMIN_PASSWORD").unwrap_or_else(|_| "FinEnv@2026".into());
    let now = now_iso();
    let user = User {
        id: new_id(),
        username: username.clone(),
        display_name: "本地管理员".into(),
        role: "super_admin".into(),
        enabled: true,
        created_at: now.clone(),
        last_login_at: None,
    };
    let hash = crypto.hash_password(&password)?;
    db.insert_user(&user, &hash)?;
    let _ = db.append_audit(
        "system",
        "auth.bootstrap",
        None,
        Some(&username),
        None,
        None,
        "初始化本地超级管理员账号。请登录后立即修改默认密码。",
    );
    tracing::warn!(
        username,
        "created initial super admin (change the default password after first login)"
    );
    Ok(())
}
