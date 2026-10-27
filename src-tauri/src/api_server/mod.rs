use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex, RwLock,
};

use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, patch, post, put},
    Extension, Json, Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::oneshot;
use tower_http::cors::{Any, CorsLayer};

use crate::{
    audit_log::{actions, Audit},
    capability::{require_role, Permission},
    error::AppError,
    models::{
        now_iso, AuditFilter, CookieRecord, CreateEnvironmentInput, EnvironmentFilter, OpenOptions,
        ProxyInput, UpdateEnvironmentInput, UpdateProxyInput,
    },
    services,
    state::SharedState,
};

#[derive(Debug, Clone)]
pub struct ApiAuth {
    pub actor: String,
    pub role: String,
}

pub struct ApiServerHandle {
    shutdown: Mutex<Option<oneshot::Sender<()>>>,
    running: AtomicBool,
    bound: RwLock<Option<(String, u16)>>,
}

impl ApiServerHandle {
    pub fn new() -> Self {
        Self {
            shutdown: Mutex::new(None),
            running: AtomicBool::new(false),
            bound: RwLock::new(None),
        }
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn status(&self, settings: &crate::models::Settings) -> crate::models::ApiStatus {
        let bound = self.bound.read().ok().and_then(|b| b.clone());
        let (host, port) = bound.unwrap_or_else(|| (settings.api_host.clone(), settings.api_port));
        crate::models::ApiStatus {
            running: self.is_running(),
            host: host.clone(),
            port,
            base_url: format!(
                "http://{}:{}/api/v1",
                if host == "0.0.0.0" {
                    "127.0.0.1"
                } else {
                    host.as_str()
                },
                port
            ),
            message: if self.is_running() {
                "API 服务运行中".into()
            } else {
                "API 服务未运行".into()
            },
        }
    }

    pub async fn start(
        self: &Arc<Self>,
        state: SharedState,
    ) -> Result<crate::models::ApiStatus, AppError> {
        if self.is_running() {
            return Ok(self.status(&state.settings()));
        }
        let settings = state.settings();
        if !settings.api_enabled {
            return Err(AppError::bad_request("API 服务已在设置中关闭"));
        }
        let addr = format!("{}:{}", settings.api_host, settings.api_port);
        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| AppError::internal(format!("API 服务无法监听 {addr}: {e}")))?;
        let actual = listener
            .local_addr()
            .map_err(|e| AppError::internal(format!("读取 API 监听地址失败: {e}")))?;
        let (tx, rx) = oneshot::channel::<()>();
        let app = build_router(state.clone());
        let handle = self.clone();
        let host = settings.api_host.clone();
        let port = actual.port();
        self.running.store(true, Ordering::SeqCst);
        if let Ok(mut bound) = self.bound.write() {
            *bound = Some((host, port));
        }
        if let Ok(mut shutdown) = self.shutdown.lock() {
            *shutdown = Some(tx);
        }
        tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = rx.await;
            });
            if let Err(e) = server.await {
                tracing::error!(error = %e, "API server stopped unexpectedly");
            }
            handle.running.store(false, Ordering::SeqCst);
        });
        tracing::info!(addr = %addr, "HTTP API started");
        Ok(self.status(&state.settings()))
    }

    pub async fn stop(&self) {
        if let Ok(mut guard) = self.shutdown.lock() {
            if let Some(tx) = guard.take() {
                let _ = tx.send(());
            }
        }
        self.running.store(false, Ordering::SeqCst);
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    pub async fn restart(
        self: &Arc<Self>,
        state: SharedState,
    ) -> Result<crate::models::ApiStatus, AppError> {
        self.stop().await;
        self.start(state).await
    }
}

impl Default for ApiServerHandle {
    fn default() -> Self {
        Self::new()
    }
}

fn build_router(state: SharedState) -> Router {
    let public = Router::new().route("/health", get(health));
    let protected = Router::new()
        .route("/stats", get(stats))
        .route("/envs", get(list_envs).post(create_env))
        .route(
            "/envs/:id",
            get(get_env).patch(update_env).delete(delete_env),
        )
        .route("/envs/:id/open", post(open_env))
        .route("/envs/:id/close", post(close_env))
        .route("/envs/:id/status", get(env_status))
        .route("/envs/:id/screenshot", get(screenshot))
        .route("/envs/:id/cookies", get(get_cookies).put(set_cookies))
        .route("/envs/:id/fingerprint", put(update_fingerprint))
        .route("/envs/:id/snapshot", post(create_snapshot))
        .route("/envs/:id/snapshots", get(list_snapshots))
        .route("/envs/:id/restore-snapshot", post(restore_snapshot))
        .route("/envs/:id/cdp", get(cdp_info))
        .route("/envs/:id/navigate", post(navigate))
        .route("/envs/:id/execute", post(execute))
        .route("/envs/:id/recording/start", post(start_recording))
        .route("/envs/:id/recording/stop", post(stop_recording))
        .route("/envs/:id/har", post(start_har).get(export_har))
        .route("/envs/:id/proxy", post(set_env_proxy))
        .route("/envs/:id/align-proxy", post(align_proxy))
        .route("/envs/:id/detect", post(detect_fingerprint))
        .route("/envs/:id/hosts", get(list_hosts).put(set_hosts))
        .route("/envs/:id/mocks", get(list_mocks).put(set_mocks))
        .route("/proxies", get(list_proxies).post(create_proxy))
        .route("/proxies/:id", patch(update_proxy).delete(delete_proxy))
        .route("/proxies/:id/check", post(check_proxy))
        .route("/audit-logs", get(list_audit))
        .route("/audit-logs/verify", get(verify_audit))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth_middleware,
        ))
        .with_state(state.clone());
    Router::new()
        .nest("/api/v1", public.merge(protected))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
}

async fn auth_middleware(
    State(state): State<SharedState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string)
        .or_else(|| {
            request
                .headers()
                .get(header::AUTHORIZATION)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer ").map(ToString::to_string))
        });
    let Some(token) = token else {
        return AppError::unauthorized("缺少 API Key（Authorization: Bearer 或 X-API-Key）")
            .into_response();
    };
    match state.authenticate_api_key(&token) {
        Ok(Some(key)) => {
            let actor = format!("api:{}", key.name);
            if !matches!(request.method().as_str(), "GET" | "HEAD" | "OPTIONS") {
                let _ = Audit::global(
                    &state.db,
                    &actor,
                    actions::API_REQUEST,
                    format!("{} {}", request.method(), request.uri().path()),
                );
            }
            request.extensions_mut().insert(ApiAuth {
                actor,
                role: key.role,
            });
            next.run(request).await
        }
        Ok(None) => AppError::unauthorized("API Key 无效或已吊销").into_response(),
        Err(e) => e.into_response(),
    }
}

fn permit(auth: &ApiAuth, permission: Permission) -> Result<(), AppError> {
    require_role(&auth.role, permission)
}

fn confirmed(headers: &HeaderMap) -> bool {
    headers
        .get("x-finenv-confirm")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("true") || v == "1")
        .unwrap_or(false)
}

async fn health() -> Json<Value> {
    Json(json!({
        "status": "ok",
        "app": "FinEnv",
        "version": env!("CARGO_PKG_VERSION"),
        "time": now_iso()
    }))
}

async fn stats(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
) -> Result<Json<crate::models::DashboardStats>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(services::dashboard(&state).await?))
}

async fn list_envs(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Query(filter): Query<EnvironmentFilter>,
) -> Result<Json<Vec<crate::models::EnvironmentView>>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(services::list_environments(&state, &filter)?))
}

async fn get_env(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::EnvironmentView>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(services::get_environment(&state, &id)?))
}

async fn create_env(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Json(input): Json<CreateEnvironmentInput>,
) -> Result<(StatusCode, Json<crate::models::EnvironmentView>), AppError> {
    permit(&auth, Permission::CreateEnv)?;
    let env = services::create_environment(&state, input, &auth.actor)?;
    Ok((
        StatusCode::CREATED,
        Json(services::environment_view(&state, env)?),
    ))
}

async fn update_env(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(input): Json<UpdateEnvironmentInput>,
) -> Result<Json<crate::models::EnvironmentView>, AppError> {
    permit(&auth, Permission::EditEnv)?;
    let env = services::update_environment(&state, &id, input, &auth.actor).await?;
    Ok(Json(services::environment_view(&state, env)?))
}

async fn delete_env(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::DeleteEnv)?;
    if !confirmed(&headers) {
        return Err(AppError::code_with(
            "confirmation_required",
            "删除环境需要 X-FinEnv-Confirm: true",
        ));
    }
    services::delete_environment(&state, &id, &auth.actor).await?;
    Ok(Json(json!({ "deleted": true, "id": id })))
}

async fn open_env(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    body: Option<Json<OpenOptions>>,
) -> Result<Json<crate::models::BrowserRuntime>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let env = state.db.get_environment(&id)?;
    let options = body.map(|Json(v)| v).unwrap_or_default();
    let runtime = state.browsers.open(&env, &options, &auth.actor).await?;
    Ok(Json(runtime))
}

async fn close_env(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    state
        .browsers
        .close(
            &id,
            &auth.actor,
            "API 请求关闭环境",
            crate::audit_log::actions::ENV_CLOSE,
        )
        .await?;
    Ok(Json(json!({ "closed": true, "id": id })))
}

async fn env_status(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::ViewList)?;
    let _env = state.db.get_environment(&id)?;
    let runtime = state.browsers.get_runtime(&id);
    Ok(Json(json!({
        "running": runtime.is_some(),
        "runtime": runtime
    })))
}

async fn screenshot(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Query(query): Query<ScreenshotQuery>,
) -> Result<Response, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let result = state.browsers.screenshot(&id, &auth.actor).await?;
    if query.format.as_deref() == Some("json") {
        return Ok(Json(result).into_response());
    }
    let encoded = result
        .data_url
        .split_once(',')
        .map(|(_, b)| b)
        .ok_or_else(|| AppError::internal("截图数据格式无效"))?;
    let bytes = B64
        .decode(encoded)
        .map_err(|_| AppError::internal("截图解码失败"))?;
    let mut response = Response::new(Body::from(bytes));
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/png"));
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("inline; filename=\"{}-screenshot.png\"", id))
            .unwrap_or(HeaderValue::from_static("inline")),
    );
    Ok(response)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ScreenshotQuery {
    format: Option<String>,
}

async fn get_cookies(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::ExportSensitive)?;
    if !state.settings().allow_sensitive_export {
        return Err(AppError::forbidden("管理员已关闭高危导出权限"));
    }
    let env = state.db.get_environment(&id)?;
    let cookies: Vec<CookieRecord> = if state.browsers.is_running(&id) {
        state.browsers.cookies_get(&id, &auth.actor, true).await?
    } else {
        let cached = state.browsers.cookies_get_cached(&id).await?;
        Audit::env(
            &state.db,
            &auth.actor,
            actions::COOKIE_READ,
            &env,
            format!("API 读取加密 Cookie 缓存 {} 条（环境未运行）", cached.len()),
        )?;
        cached
    };
    Ok(Json(json!({
        "envId": env.id,
        "envName": env.name,
        "cookies": cookies
    })))
}

async fn set_cookies(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::ExportSensitive)?;
    if !state.settings().allow_sensitive_export {
        return Err(AppError::forbidden("管理员已关闭高危导出权限"));
    }
    let cookies: Vec<CookieRecord> =
        if let Some(array) = body.get("cookies").and_then(|v| v.as_array()) {
            serde_json::from_value(Value::Array(array.clone()))
                .map_err(|e| AppError::bad_request(format!("Cookie 格式无效: {e}")))?
        } else {
            serde_json::from_value(body)
                .map_err(|e| AppError::bad_request(format!("Cookie 格式无效: {e}")))?
        };
    let count = state
        .browsers
        .cookies_set(&id, &cookies, &auth.actor)
        .await?;
    Ok(Json(json!({ "updated": count })))
}

async fn update_fingerprint(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(fp): Json<crate::fingerprint::Fingerprint>,
) -> Result<Json<crate::models::EnvironmentView>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    let env = services::update_fingerprint(&state, &id, fp, &auth.actor)?;
    services::apply_fingerprint_if_running(&state, &env).await?;
    Ok(Json(services::environment_view(&state, env)?))
}

async fn create_snapshot(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<crate::models::FingerprintSnapshot>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    let name = body.and_then(|Json(v)| {
        v.get("name")
            .and_then(|n| n.as_str())
            .map(ToString::to_string)
    });
    Ok(Json(services::create_snapshot(
        &state,
        &id,
        name,
        &auth.actor,
    )?))
}

async fn list_snapshots(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::models::FingerprintSnapshot>>, AppError> {
    permit(&auth, Permission::ViewList)?;
    state.db.get_environment(&id)?;
    Ok(Json(state.db.list_snapshots(&id)?))
}

async fn restore_snapshot(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<crate::models::EnvironmentView>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    let snapshot_id = body
        .get("snapshotId")
        .or_else(|| body.get("id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("缺少 snapshotId"))?;
    let snapshot = state.db.get_snapshot(snapshot_id)?;
    if snapshot.env_id != id {
        return Err(AppError::bad_request("快照不属于该环境"));
    }
    let env = services::restore_snapshot(&state, snapshot_id, &auth.actor).await?;
    Ok(Json(services::environment_view(&state, env)?))
}

async fn cdp_info(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::CdpInfo>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    Ok(Json(state.browsers.cdp_info(&id)?))
}

async fn navigate(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let url = body
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("缺少 url"))?;
    let current = state.browsers.navigate(&id, url).await?;
    Ok(Json(json!({ "url": current })))
}

async fn execute(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let script = body
        .get("script")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::bad_request("缺少 script"))?;
    let value = state.browsers.execute(&id, script).await?;
    Ok(Json(json!({ "result": value })))
}

async fn start_recording(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Result<Json<crate::models::RecordingResult>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let interval = body
        .and_then(|Json(v)| v.get("intervalMs").and_then(|i| i.as_u64()))
        .unwrap_or(1000);
    Ok(Json(
        state
            .browsers
            .start_recording(&id, interval, &auth.actor)
            .await?,
    ))
}

async fn stop_recording(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::RecordingResult>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    Ok(Json(state.browsers.stop_recording(&id, &auth.actor).await?))
}

async fn start_har(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    state.browsers.start_har(&id, &auth.actor).await?;
    Ok(Json(json!({ "recording": true, "envId": id })))
}

async fn export_har(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let path = state.browsers.stop_har(&id, &auth.actor).await?;
    Ok(Json(json!({ "path": path, "envId": id })))
}

async fn set_env_proxy(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    let proxy_id = body.get("proxyId").and_then(|v| v.as_str());
    let was_running = state.browsers.is_running(&id);
    let restart = body
        .get("restart")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let count = services::assign_proxy(&state, std::slice::from_ref(&id), proxy_id, &auth.actor)?;
    if was_running && restart {
        state
            .browsers
            .close(
                &id,
                &auth.actor,
                "API 切换代理并重启环境",
                crate::audit_log::actions::ENV_CLOSE,
            )
            .await?;
        let env = state.db.get_environment(&id)?;
        state
            .browsers
            .open(&env, &OpenOptions::default(), &auth.actor)
            .await?;
    }
    Ok(Json(json!({
        "updated": count,
        "restarted": was_running && restart,
        "restartRequired": was_running && !restart,
        "message": if was_running && !restart { "代理已绑定，运行中的环境需重启后生效" } else { "代理已绑定" }
    })))
}

async fn align_proxy(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::AlignProxyResult>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    Ok(Json(services::align_proxy(&state, &id, &auth.actor).await?))
}

async fn detect_fingerprint(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::FingerprintReport>, AppError> {
    permit(&auth, Permission::OpenClose)?;
    let env = state.db.get_environment(&id)?;
    if !state.browsers.is_running(&id) {
        state
            .browsers
            .open(&env, &OpenOptions::default(), &auth.actor)
            .await?;
    }
    let settings = state.settings();
    let report = crate::fingerprint_report::detect(
        &state.db,
        &state.browsers,
        &state.data_dir,
        &env,
        &settings.detection_service_url,
        &auth.actor,
    )
    .await?;
    Ok(Json(report))
}

async fn list_hosts(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::models::HostsEntry>>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(state.db.list_hosts(&id)?))
}

async fn set_hosts(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Vec<crate::models::HostsEntry>>, AppError> {
    permit(&auth, Permission::EditEnv)?;
    let entries: Vec<crate::models::HostsEntryInput> =
        if let Some(array) = body.get("entries").and_then(|v| v.as_array()) {
            serde_json::from_value(Value::Array(array.clone()))
                .map_err(|e| AppError::bad_request(format!("Hosts 格式无效: {e}")))?
        } else {
            serde_json::from_value(body)
                .map_err(|e| AppError::bad_request(format!("Hosts 格式无效: {e}")))?
        };
    Ok(Json(services::set_hosts(
        &state,
        &id,
        &entries,
        &auth.actor,
    )?))
}

async fn list_mocks(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<Vec<crate::models::MockRule>>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(state.db.list_mocks(&id)?))
}

async fn set_mocks(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> Result<Json<Vec<crate::models::MockRule>>, AppError> {
    permit(&auth, Permission::EditEnv)?;
    let rules: Vec<crate::models::MockRuleInput> =
        if let Some(array) = body.get("rules").and_then(|v| v.as_array()) {
            serde_json::from_value(Value::Array(array.clone()))
                .map_err(|e| AppError::bad_request(format!("Mock 格式无效: {e}")))?
        } else {
            serde_json::from_value(body)
                .map_err(|e| AppError::bad_request(format!("Mock 格式无效: {e}")))?
        };
    Ok(Json(
        services::set_mocks(&state, &id, &rules, &auth.actor).await?,
    ))
}

async fn list_proxies(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
) -> Result<Json<Vec<crate::models::Proxy>>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(state.db.list_proxies()?))
}

async fn create_proxy(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Json(input): Json<ProxyInput>,
) -> Result<(StatusCode, Json<crate::models::Proxy>), AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    Ok((
        StatusCode::CREATED,
        Json(services::create_proxy(&state, input, &auth.actor)?),
    ))
}

async fn update_proxy(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    Json(input): Json<UpdateProxyInput>,
) -> Result<Json<crate::models::Proxy>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    Ok(Json(services::update_proxy(
        &state,
        &id,
        input,
        &auth.actor,
    )?))
}

async fn delete_proxy(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    permit(&auth, Permission::EditFingerprintProxy)?;
    if !confirmed(&headers) {
        return Err(AppError::code_with(
            "confirmation_required",
            "删除代理需要 X-FinEnv-Confirm: true",
        ));
    }
    services::delete_proxy(&state, &id, &auth.actor)?;
    Ok(Json(json!({ "deleted": true, "id": id })))
}

async fn check_proxy(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Path(id): Path<String>,
) -> Result<Json<crate::models::ProxyCheckResult>, AppError> {
    permit(&auth, Permission::ViewList)?;
    Ok(Json(services::check_proxy(&state, &id, &auth.actor).await?))
}

async fn list_audit(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
    Query(filter): Query<AuditFilter>,
) -> Result<Json<Vec<crate::models::AuditLog>>, AppError> {
    permit(&auth, Permission::AuditView)?;
    Ok(Json(state.db.list_audit(&filter)?))
}

async fn verify_audit(
    State(state): State<SharedState>,
    Extension(auth): Extension<ApiAuth>,
) -> Result<Json<crate::models::AuditVerifyResult>, AppError> {
    permit(&auth, Permission::AuditView)?;
    Ok(Json(state.db.verify_audit_chain()?))
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status =
            StatusCode::from_u16(self.http_status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = json!({
            "error": {
                "code": self.code(),
                "message": self.message()
            }
        });
        (status, Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use crate::{services, state::AppState};

    fn free_port() -> u16 {
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.local_addr().unwrap().port()
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn api_auth_crud_and_audit_flow() {
        let dir = std::env::temp_dir().join(format!("finenv-api-{}", uuid::Uuid::new_v4()));
        let state = AppState::initialize_at(dir.clone()).unwrap();
        let mut settings = state.settings();
        settings.api_enabled = true;
        settings.api_host = "127.0.0.1".into();
        settings.api_port = free_port();
        state.store_settings(&settings).unwrap();
        let status = state.api.start(state.clone()).await.unwrap();
        let base = status.base_url;

        let super_key = services::create_api_key(&state, "super-test", "super_admin", "test")
            .unwrap()
            .secret;
        let viewer_key = services::create_api_key(&state, "viewer-test", "viewer", "test")
            .unwrap()
            .secret;
        let client = reqwest::Client::new();

        let health: serde_json::Value = client
            .get(format!("{base}/health"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(health["status"], "ok");

        let unauthorized = client.get(format!("{base}/envs")).send().await.unwrap();
        assert_eq!(unauthorized.status().as_u16(), 401);

        let list: serde_json::Value = client
            .get(format!("{base}/envs"))
            .bearer_auth(&super_key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(list.as_array().unwrap().len(), 0);

        let created = client
            .post(format!("{base}/envs"))
            .bearer_auth(&super_key)
            .json(&serde_json::json!({
                "name": "API integration",
                "project": "ci",
                "testEnv": "test",
                "templateOs": "win11",
                "tags": ["api"]
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(created.status().as_u16(), 201);
        let created: serde_json::Value = created.json().await.unwrap();
        let env_id = created["id"].as_str().unwrap().to_string();

        let snapshot = client
            .post(format!("{base}/envs/{env_id}/snapshot"))
            .bearer_auth(&super_key)
            .json(&serde_json::json!({ "name": "api snapshot" }))
            .send()
            .await
            .unwrap();
        assert_eq!(snapshot.status().as_u16(), 200);
        let snapshots: serde_json::Value = client
            .get(format!("{base}/envs/{env_id}/snapshots"))
            .bearer_auth(&super_key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(snapshots.as_array().unwrap().len(), 1);

        let viewer_create = client
            .post(format!("{base}/envs"))
            .bearer_auth(&viewer_key)
            .json(&serde_json::json!({ "name": "forbidden" }))
            .send()
            .await
            .unwrap();
        assert_eq!(viewer_create.status().as_u16(), 403);

        let viewer_delete = client
            .delete(format!("{base}/envs/{env_id}"))
            .bearer_auth(&viewer_key)
            .header("X-FinEnv-Confirm", "true")
            .send()
            .await
            .unwrap();
        assert_eq!(viewer_delete.status().as_u16(), 403);

        let missing_confirm = client
            .delete(format!("{base}/envs/{env_id}"))
            .bearer_auth(&super_key)
            .send()
            .await
            .unwrap();
        assert_eq!(missing_confirm.status().as_u16(), 400);

        let deleted = client
            .delete(format!("{base}/envs/{env_id}"))
            .bearer_auth(&super_key)
            .header("X-FinEnv-Confirm", "true")
            .send()
            .await
            .unwrap();
        assert_eq!(deleted.status().as_u16(), 200);

        let verify: serde_json::Value = client
            .get(format!("{base}/audit-logs/verify"))
            .bearer_auth(&super_key)
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(verify["valid"].as_bool().unwrap_or(false));

        state.api.stop().await;
        drop(state);
        let _ = std::fs::remove_dir_all(dir);
    }
}
