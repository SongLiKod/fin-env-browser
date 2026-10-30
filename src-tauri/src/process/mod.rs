use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use tokio::{
    process::Command,
    sync::{mpsc, RwLock as AsyncRwLock},
    time::sleep,
};

use crate::{
    audit_log::{actions, Audit},
    cdp::{self, CdpClient, HarState},
    crypto::CryptoManager,
    db::Db,
    error::{AppError, Result},
    fingerprint,
    models::{
        now_iso, BrowserRuntime, CookieRecord, Environment, MockRule, OpenOptions, RecordingResult,
        ScreenshotResult, Settings,
    },
    paths,
};

#[cfg(windows)]
mod overlay;

mod broadcast;

use broadcast::BroadcastRuntime;

#[derive(Debug, Clone)]
struct ExitEvent {
    env_id: String,
    pid: u32,
    status: Option<i32>,
}

struct RecordingHandle {
    stop: Arc<AtomicBool>,
    dir: PathBuf,
    started_at: String,
    frames: Arc<std::sync::atomic::AtomicU32>,
    task: tokio::task::JoinHandle<()>,
}

pub struct BrowserInstance {
    pub env_id: String,
    pub pid: u32,
    pub debug_port: u16,
    pub page_ws: String,
    last_active: Mutex<Instant>,
    runtime: RwLock<BrowserRuntime>,
    client: Arc<CdpClient>,
    mocks: AsyncRwLock<Vec<MockRule>>,
    proxy_auth: Option<(String, String)>,
    recording: Mutex<Option<RecordingHandle>>,
    har: Arc<Mutex<HarState>>,
    closing: AtomicBool,
    captcha_last: Mutex<Option<Instant>>,
    memory_alert_last: Mutex<Option<Instant>>,
    current_url: Mutex<String>,
}

pub struct BrowserManager {
    data_dir: PathBuf,
    db: Db,
    crypto: Arc<CryptoManager>,
    settings: Arc<RwLock<Settings>>,
    instances: RwLock<HashMap<String, Arc<BrowserInstance>>>,
    app: Mutex<Option<tauri::AppHandle>>,
    exit_tx: mpsc::UnboundedSender<ExitEvent>,
    exit_rx: Mutex<Option<mpsc::UnboundedReceiver<ExitEvent>>>,
    monitor_started: AtomicBool,
    restarts: Mutex<HashMap<String, Vec<Instant>>>,
    launch_lock: tokio::sync::Mutex<()>,
    broadcast: AsyncRwLock<Option<Arc<BroadcastRuntime>>>,
}

impl BrowserManager {
    pub fn new(
        data_dir: PathBuf,
        db: Db,
        crypto: Arc<CryptoManager>,
        settings: Arc<RwLock<Settings>>,
    ) -> Self {
        let (exit_tx, exit_rx) = mpsc::unbounded_channel();
        Self {
            data_dir,
            db,
            crypto,
            settings,
            instances: RwLock::new(HashMap::new()),
            app: Mutex::new(None),
            exit_tx,
            exit_rx: Mutex::new(Some(exit_rx)),
            monitor_started: AtomicBool::new(false),
            restarts: Mutex::new(HashMap::new()),
            launch_lock: tokio::sync::Mutex::new(()),
            broadcast: AsyncRwLock::new(None),
        }
    }

    pub fn set_app_handle(&self, app: tauri::AppHandle) {
        if let Ok(mut guard) = self.app.lock() {
            *guard = Some(app);
        }
    }

    fn settings(&self) -> Settings {
        self.settings.read().map(|s| s.clone()).unwrap_or_default()
    }

    fn runtime_of(&self, env_id: &str) -> Option<BrowserRuntime> {
        self.instances
            .read()
            .ok()
            .and_then(|map| map.get(env_id).cloned())
            .and_then(|instance| instance.runtime.read().ok().map(|r| r.clone()))
    }

    pub fn get_runtime(&self, env_id: &str) -> Option<BrowserRuntime> {
        self.runtime_of(env_id)
    }

    pub fn list_runtimes(&self) -> Vec<BrowserRuntime> {
        self.instances
            .read()
            .map(|map| {
                map.values()
                    .filter_map(|i| i.runtime.read().ok().map(|r| r.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn is_running(&self, env_id: &str) -> bool {
        self.instances
            .read()
            .map(|m| m.contains_key(env_id))
            .unwrap_or(false)
    }

    pub fn running_count(&self) -> u32 {
        self.instances.read().map(|m| m.len() as u32).unwrap_or(0)
    }

    fn emit<T: serde::Serialize + Clone>(&self, event: &str, payload: T) {
        if let Ok(guard) = self.app.lock() {
            if let Some(app) = guard.as_ref() {
                use tauri::Emitter;
                let _ = app.emit(event, payload);
            }
        }
    }

    fn notify(&self, title: &str, body: &str) {
        if let Ok(guard) = self.app.lock() {
            if let Some(app) = guard.as_ref() {
                use tauri_plugin_notification::NotificationExt;
                let _ = app.notification().builder().title(title).body(body).show();
            }
        }
    }

    // -----------------------------------------------------------------------
    // Open / close
    // -----------------------------------------------------------------------

    pub async fn open(
        self: &Arc<Self>,
        env: &Environment,
        options: &OpenOptions,
        actor: &str,
    ) -> Result<BrowserRuntime> {
        if env.status == "disabled" {
            return Err(AppError::bad_request("环境已停用，无法打开"));
        }
        let _launch_guard = self.launch_lock.lock().await;
        if let Some(runtime) = self.runtime_of(&env.id) {
            if !runtime.ws_endpoint.is_empty() {
                return Ok(runtime);
            }
        }
        let settings = self.settings();
        if self.running_count() >= settings.max_concurrent_browsers {
            return Err(AppError::code_with(
                "capacity_limit",
                format!(
                    "已达到全局最大并发 {} 个浏览器实例，请先关闭闲置环境或调整资源上限",
                    settings.max_concurrent_browsers
                ),
            ));
        }

        let chrome = resolve_chromium(
            env.chromium_path.as_deref(),
            settings.chromium_path.as_deref(),
        )?;
        let profile_dir = if env.profile_dir.trim().is_empty() {
            paths::env_profile_dir(&self.data_dir, &env.id)
        } else {
            PathBuf::from(&env.profile_dir)
        };
        paths::ensure_dir(&profile_dir)?;
        paths::ensure_dir(&paths::screenshots_dir(&self.data_dir))?;
        paths::ensure_dir(&paths::recordings_dir(&self.data_dir))?;

        let proxy = match &env.proxy_id {
            Some(id) => Some(self.db.get_proxy(id)?),
            None => None,
        };
        let (proxy_url, proxy_auth) = if let Some(proxy) = &proxy {
            if !proxy.enabled {
                return Err(AppError::bad_request("环境绑定的代理已停用"));
            }
            let password = if proxy.password_enc.is_empty() {
                String::new()
            } else {
                self.crypto.decrypt(&proxy.password_enc)?
            };
            let auth = if !proxy.username.is_empty() {
                Some((proxy.username.clone(), password))
            } else {
                None
            };
            (Some(proxy.url()), auth)
        } else {
            (None, None)
        };

        let debug_port = free_port()?;
        let hosts = self.db.list_hosts(&env.id)?;
        let extensions = self
            .db
            .extensions_for_env(&env.id)?
            .into_iter()
            .filter(|e| e.enabled && Path::new(&e.path).exists())
            .map(|e| e.path)
            .collect::<Vec<_>>();
        let mocks = self.db.list_mocks(&env.id)?;
        let headless = options.headless.unwrap_or(false);
        let requested_url = options
            .url
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty());
        let configured_url = settings.default_start_url.trim();
        let use_start_page = !headless
            && requested_url.is_none()
            && (configured_url.is_empty() || configured_url == "about:blank");
        let start_page_path =
            use_start_page.then(|| paths::start_page_path(&self.data_dir, &env.id));
        let start_url = match &start_page_path {
            Some(path) => crate::start_page::file_url(path)?,
            None => requested_url
                .map(ToString::to_string)
                .or_else(|| (!configured_url.is_empty()).then(|| configured_url.to_string()))
                .unwrap_or_else(|| "about:blank".into()),
        };
        if start_page_path.is_none()
            && start_url != "about:blank"
            && !start_url.starts_with("http://")
            && !start_url.starts_with("https://")
        {
            return Err(AppError::bad_request(
                "启动地址仅支持 http://、https:// 或 about:blank",
            ));
        }
        let window_name = format!("FinEnv · {}", env.name);
        let launch_args =
            merge_launch_args(&settings.chromium_launch_args, &env.chromium_launch_args);

        let args = build_chromium_args(
            &env.fingerprint,
            &profile_dir,
            debug_port,
            proxy_url.as_deref(),
            &hosts,
            &extensions,
            &window_name,
            "about:blank",
            headless,
            &launch_args,
        );

        let mut command = Command::new(&chrome);
        command
            .args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|e| {
            AppError::internal(format!("无法启动 Chromium ({}): {e}", chrome.display()))
        })?;
        let pid = child.id().unwrap_or(0);
        if pid == 0 {
            let _ = child.kill().await;
            return Err(AppError::internal("Chromium 进程启动后未返回 PID"));
        }
        #[cfg(windows)]
        {
            let color = effective_border_color(env);
            let border_color = color.clone();
            tokio::spawn(async move { apply_window_border(pid, &border_color).await });
            overlay::spawn(
                pid,
                env.name.clone(),
                env.taskbar_badge_text.clone(),
                color,
                env.border_width,
            );
        }
        let tx = self.exit_tx.clone();
        let env_id_for_wait = env.id.clone();
        tokio::spawn(async move {
            let status = child.wait().await.ok().and_then(|s| s.code());
            let _ = tx.send(ExitEvent {
                env_id: env_id_for_wait,
                pid,
                status,
            });
        });

        let launched = async {
            let (ws, _page_url) = wait_for_cdp_page(debug_port, Duration::from_secs(20)).await?;
            let client = Arc::new(CdpClient::connect(&ws).await?);
            let _ = client.call("Page.enable", json!({})).await;
            let _ = client.call("Runtime.enable", json!({})).await;
            let _ = client.call("Network.enable", json!({})).await;
            let _ = client
                .call(
                    "Network.emulateNetworkConditions",
                    network_emulation_params(&env.network_profile),
                )
                .await;
            if let Some(geo) = &env.fingerprint.geolocation {
                let _ = client
                    .call(
                        "Browser.grantPermissions",
                        json!({ "permissions": ["geolocation"] }),
                    )
                    .await;
                let _ = client
                    .call(
                        "Emulation.setGeolocationOverride",
                        json!({
                            "latitude": geo.latitude,
                            "longitude": geo.longitude,
                            "accuracy": geo.accuracy
                        }),
                    )
                    .await;
            }
            // Apply the encrypted cookie snapshot (if one exists) so a synced
            // database or a freshly recreated profile keeps its login state.
            if let Ok(Some(raw)) = self.db.load_cookie_cache(&env.id) {
                if let Ok(plain) = self.crypto.decrypt(&raw) {
                    if let Ok(cookies) =
                        serde_json::from_str::<Vec<crate::models::CookieRecord>>(&plain)
                    {
                        if !cookies.is_empty() {
                            let _ = client
                                .call("Network.setCookies", cdp::cookies_to_cdp(&cookies))
                                .await;
                        }
                    }
                }
            }
            let started_at = now_iso();
            let runtime = BrowserRuntime {
                env_id: env.id.clone(),
                pid,
                debug_port,
                ws_endpoint: ws.clone(),
                status: "running".into(),
                started_at: started_at.clone(),
                last_active_at: started_at.clone(),
                cpu_percent: 0.0,
                memory_mb: 0,
                current_url: start_url.clone(),
                recording: false,
                har_recording: false,
                warnings: vec![],
            };
            if let Some(path) = &start_page_path {
                crate::start_page::write_at(
                    path,
                    env,
                    proxy.as_ref(),
                    self.db.account_count_for_env(&env.id)?,
                    self.db.snapshot_count_for_env(&env.id)?,
                    Some(&runtime),
                    &start_url,
                )?;
            }
            let instance = Arc::new(BrowserInstance {
                env_id: env.id.clone(),
                pid,
                debug_port,
                page_ws: ws,
                last_active: Mutex::new(Instant::now()),
                runtime: RwLock::new(runtime),
                client: client.clone(),
                mocks: AsyncRwLock::new(mocks),
                proxy_auth,
                recording: Mutex::new(None),
                har: Arc::new(Mutex::new(HarState::default())),
                closing: AtomicBool::new(false),
                captcha_last: Mutex::new(None),
                memory_alert_last: Mutex::new(None),
                current_url: Mutex::new(start_url.clone()),
            });
            // Start handling CDP events before enabling Fetch interception,
            // otherwise a paused navigation would wait for a handler that is
            // only installed later.
            self.start_event_handler(instance.clone());
            let mut warnings = Vec::new();
            if let Err(e) = client
                .call(
                    "Fetch.enable",
                    json!({
                        "patterns": [{ "urlPattern": "*", "requestStage": "Request" }],
                        "handleAuthRequests": true
                    }),
                )
                .await
            {
                warnings.push(format!("请求拦截启用失败，Mock 功能不可用: {e}"));
            }
            if let Err(e) = cdp::install_fingerprint(&client, &env.fingerprint).await {
                warnings.push(format!("指纹注入部分失败: {e}"));
            }
            let _ = client
                .call("Page.navigate", json!({ "url": start_url.clone() }))
                .await
                .map(|result| {
                    if let Some(error) = result.get("errorText").and_then(|v| v.as_str()) {
                        warnings.push(format!("页面导航失败: {error}"));
                    }
                });
            wait_for_ready(&client, Duration::from_secs(15)).await;
            if let Some(script) = options
                .pre_script
                .as_deref()
                .filter(|s| !s.trim().is_empty())
            {
                if let Err(e) = client.evaluate(script).await {
                    warnings.push(format!("前置脚本执行失败: {e}"));
                } else {
                    let _ = Audit::env(
                        &self.db,
                        actor,
                        actions::PRE_SCRIPT,
                        env,
                        "打开环境时执行前置脚本成功",
                    );
                }
            }
            if let Ok(mut runtime) = instance.runtime.write() {
                runtime.warnings = warnings;
                runtime.current_url = start_url.clone();
            }
            Ok::<_, AppError>(instance)
        }
        .await;

        let instance = match launched {
            Ok(instance) => instance,
            Err(e) => {
                kill_tree(pid).await;
                let _ = self.db.remove_browser_session(&env.id);
                return Err(AppError::internal(format!(
                    "Chromium 已启动但 CDP 连接失败: {e}"
                )));
            }
        };

        if let Ok(mut map) = self.instances.write() {
            map.insert(env.id.clone(), instance.clone());
        }
        self.db
            .save_browser_session(&instance.runtime.read().unwrap())?;
        self.db.touch_environment_opened(&env.id)?;
        self.emit(
            "browser://runtime",
            instance.runtime.read().unwrap().clone(),
        );
        Audit::env(
            &self.db,
            actor,
            actions::ENV_OPEN,
            env,
            format!(
                "打开环境（PID {}，调试端口 {}，代理 {}）",
                pid,
                debug_port,
                proxy_url.as_deref().unwrap_or("直连")
            ),
        )?;
        self.notify(
            "FinEnv 环境已启动",
            &format!("{} · PID {} · 端口 {}", env.name, pid, debug_port),
        );
        let runtime = instance
            .runtime
            .read()
            .map(|r| r.clone())
            .map_err(|_| AppError::internal("读取运行状态失败"))?;
        Ok(runtime)
    }

    pub async fn close(
        self: &Arc<Self>,
        env_id: &str,
        actor: &str,
        reason: &str,
        action: &str,
    ) -> Result<()> {
        let instance = self
            .instances
            .read()
            .ok()
            .and_then(|m| m.get(env_id).cloned());
        let Some(instance) = instance else {
            let _ = self.db.remove_browser_session(env_id);
            return Ok(());
        };
        instance.closing.store(true, Ordering::SeqCst);
        self.stop_recording_internal(&instance, true).await;
        let _ = self.finalize_har(&instance, false).await;
        if let Ok(cookies) = instance
            .client
            .call("Network.getAllCookies", json!({}))
            .await
        {
            let records = cdp::cookies_from_cdp(&cookies);
            if let Ok(raw) = serde_json::to_string(&records) {
                if let Ok(enc) = self.crypto.encrypt(&raw) {
                    let _ = self.db.save_cookie_cache(env_id, &enc);
                }
            }
        }
        let _ = instance.client.call("Browser.close", json!({})).await;
        sleep(Duration::from_millis(600)).await;
        kill_tree(instance.pid).await;
        if let Ok(mut map) = self.instances.write() {
            map.remove(env_id);
        }
        self.broadcast_on_env_closed(env_id, actor).await;
        let _ = self.db.remove_browser_session(env_id);
        let env = self.db.get_environment(env_id).ok();
        if let Some(env) = &env {
            Audit::env(
                &self.db,
                actor,
                action,
                env,
                format!("{reason}（PID {}）", instance.pid),
            )?;
        }
        self.emit(
            "browser://exit",
            json!({ "envId": env_id, "pid": instance.pid, "reason": reason }),
        );
        Ok(())
    }

    pub async fn force_recycle_all(self: &Arc<Self>, actor: &str) -> Result<u32> {
        let ids: Vec<String> = self
            .instances
            .read()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let count = ids.len() as u32;
        for id in ids {
            let _ = self
                .close(&id, actor, "一键强制回收全部浏览器", actions::ENV_RECYCLE)
                .await;
        }
        Ok(count)
    }

    // -----------------------------------------------------------------------
    // Monitoring
    // -----------------------------------------------------------------------

    pub fn start_monitor(self: &Arc<Self>) {
        if self.monitor_started.swap(true, Ordering::SeqCst) {
            return;
        }
        let rx = self.exit_rx.lock().ok().and_then(|mut g| g.take());
        let Some(mut rx) = rx else {
            return;
        };
        let manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        manager.monitor_tick().await;
                    }
                    event = rx.recv() => {
                        if let Some(event) = event {
                            manager.handle_exit(event).await;
                        }
                    }
                }
            }
        });
    }

    async fn monitor_tick(self: &Arc<Self>) {
        let instances: Vec<Arc<BrowserInstance>> = self
            .instances
            .read()
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default();
        if instances.is_empty() {
            return;
        }
        let settings = self.settings();
        let mut system = sysinfo::System::new_all();
        system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
        let pids: Vec<sysinfo::Pid> = instances
            .iter()
            .map(|i| sysinfo::Pid::from_u32(i.pid))
            .collect();
        let stats = process_tree_stats(&system, &pids);

        for instance in instances {
            let (cpu, memory) = stats.get(&instance.pid).copied().unwrap_or((0.0, 0));
            let alive = system
                .process(sysinfo::Pid::from_u32(instance.pid))
                .is_some();
            if !alive {
                self.handle_exit(ExitEvent {
                    env_id: instance.env_id.clone(),
                    pid: instance.pid,
                    status: None,
                })
                .await;
                continue;
            }
            if let Ok(mut runtime) = instance.runtime.write() {
                runtime.cpu_percent = (cpu * 100.0).round() / 100.0;
                runtime.memory_mb = memory;
            }
            if let Ok(runtime) = instance.runtime.read() {
                self.emit("browser://runtime", runtime.clone());
            }
            let env = match self.db.get_environment(&instance.env_id) {
                Ok(env) => env,
                Err(_) => continue,
            };
            let limit = env.memory_limit_mb.unwrap_or(settings.memory_limit_mb);
            if memory > limit as u64 {
                let should_alert = instance
                    .memory_alert_last
                    .lock()
                    .map(|mut last| {
                        let due = last
                            .map(|previous| previous.elapsed() >= Duration::from_secs(60))
                            .unwrap_or(true);
                        if due {
                            *last = Some(Instant::now());
                        }
                        due
                    })
                    .unwrap_or(true);
                if should_alert {
                    self.emit(
                        "resource://alert",
                        json!({
                            "level": "warning",
                            "envId": instance.env_id,
                            "message": format!("{} 内存 {}MB 超过阈值 {}MB", env.name, memory, limit)
                        }),
                    );
                    self.notify(
                        "FinEnv 内存告警",
                        &format!("{} 使用 {}MB（阈值 {}MB）", env.name, memory, limit),
                    );
                }
                if settings.memory_enforce {
                    let _ = self
                        .close(
                            &instance.env_id,
                            "system",
                            &format!("内存 {}MB 超过阈值 {}MB，自动关闭", memory, limit),
                            actions::ENV_MEMORY_CLOSE,
                        )
                        .await;
                    continue;
                }
            }
            let idle_min = env.idle_timeout_min.unwrap_or(settings.idle_timeout_min);
            if idle_min > 0 {
                let idle = instance
                    .last_active
                    .lock()
                    .map(|t| t.elapsed())
                    .unwrap_or_default();
                if idle >= Duration::from_secs(idle_min as u64 * 60) {
                    let _ = self
                        .close(
                            &instance.env_id,
                            "system",
                            &format!("闲置超过 {} 分钟，自动回收", idle_min),
                            actions::ENV_IDLE_CLOSE,
                        )
                        .await;
                }
            }
        }
    }

    async fn handle_exit(self: &Arc<Self>, event: ExitEvent) {
        let instance = self
            .instances
            .read()
            .ok()
            .and_then(|m| m.get(&event.env_id).cloned());
        let Some(instance) = instance else {
            let _ = self.db.remove_browser_session(&event.env_id);
            return;
        };
        if instance.pid != event.pid {
            return;
        }
        if instance.closing.load(Ordering::SeqCst) {
            if let Ok(mut map) = self.instances.write() {
                map.remove(&event.env_id);
            }
            let _ = self.db.remove_browser_session(&event.env_id);
            return;
        }
        if let Ok(mut map) = self.instances.write() {
            map.remove(&event.env_id);
        }
        self.broadcast_on_env_closed(&event.env_id, "system").await;
        let _ = self.db.remove_browser_session(&event.env_id);
        let env = match self.db.get_environment(&event.env_id) {
            Ok(env) => env,
            Err(_) => return,
        };
        let _ = Audit::env(
            &self.db,
            "system",
            actions::ENV_CRASH,
            &env,
            format!(
                "浏览器进程异常退出（PID {}，退出码 {:?}）",
                event.pid, event.status
            ),
        );
        self.emit(
            "browser://exit",
            json!({
                "envId": event.env_id,
                "pid": event.pid,
                "code": event.status,
                "reason": "crash"
            }),
        );
        self.notify(
            "FinEnv 浏览器进程退出",
            &format!("{} 的浏览器进程已退出", env.name),
        );
        let settings = self.settings();
        if (env.auto_restart || settings.auto_restart_on_crash) && self.allow_restart(&env.id) {
            let manager = self.clone();
            tokio::spawn(async move {
                sleep(Duration::from_millis(800)).await;
                if let Err(e) = manager.open(&env, &OpenOptions::default(), "system").await {
                    tracing::warn!(env_id = %env.id, error = %e, "auto restart failed");
                }
            });
        }
    }

    fn allow_restart(&self, env_id: &str) -> bool {
        let Ok(mut guard) = self.restarts.lock() else {
            return false;
        };
        let now = Instant::now();
        let list = guard.entry(env_id.to_string()).or_default();
        list.retain(|t| now.duration_since(*t) < Duration::from_secs(600));
        if list.len() >= 3 {
            return false;
        }
        list.push(now);
        true
    }

    pub async fn restore_sessions(self: &Arc<Self>) -> Result<u32> {
        let sessions = self.db.list_browser_sessions()?;
        let mut restored = 0;
        for session in sessions {
            if !cdp_http_alive(session.debug_port).await {
                let _ = self.db.remove_browser_session(&session.env_id);
                continue;
            }
            let env = match self.db.get_environment(&session.env_id) {
                Ok(env) => env,
                Err(_) => {
                    let _ = self.db.remove_browser_session(&session.env_id);
                    continue;
                }
            };
            let (ws, page_url) = match get_page_ws(session.debug_port).await {
                Ok((ws, page_url)) => (ws, page_url),
                Err(_) => {
                    let _ = self.db.remove_browser_session(&session.env_id);
                    continue;
                }
            };
            let client = match CdpClient::connect(&ws).await {
                Ok(client) => client,
                Err(_) => {
                    let _ = self.db.remove_browser_session(&session.env_id);
                    continue;
                }
            };
            let _ = client.call("Page.enable", json!({})).await;
            let _ = client.call("Runtime.enable", json!({})).await;
            let _ = client.call("Network.enable", json!({})).await;
            let _ = client
                .call(
                    "Network.emulateNetworkConditions",
                    network_emulation_params(&env.network_profile),
                )
                .await;
            if let Some(geo) = &env.fingerprint.geolocation {
                let _ = client
                    .call(
                        "Browser.grantPermissions",
                        json!({ "permissions": ["geolocation"] }),
                    )
                    .await;
                let _ = client
                    .call(
                        "Emulation.setGeolocationOverride",
                        json!({
                            "latitude": geo.latitude,
                            "longitude": geo.longitude,
                            "accuracy": geo.accuracy
                        }),
                    )
                    .await;
            }
            let _ = client
                .call(
                    "Fetch.enable",
                    json!({
                        "patterns": [{ "urlPattern": "*", "requestStage": "Request" }],
                        "handleAuthRequests": true
                    }),
                )
                .await;
            let _ = cdp::install_fingerprint(&client, &env.fingerprint).await;
            let mocks = self.db.list_mocks(&env.id).unwrap_or_default();
            let proxy_auth = match env.proxy_id.as_deref() {
                Some(proxy_id) => match self.db.get_proxy(proxy_id) {
                    Ok(proxy) if !proxy.username.is_empty() => {
                        let password = if proxy.password_enc.is_empty() {
                            String::new()
                        } else {
                            self.crypto.decrypt(&proxy.password_enc).unwrap_or_default()
                        };
                        Some((proxy.username, password))
                    }
                    _ => None,
                },
                None => None,
            };
            let mut runtime = session.clone();
            runtime.status = "running".into();
            runtime.ws_endpoint = ws.clone();
            runtime.current_url = page_url.clone();
            let instance = Arc::new(BrowserInstance {
                env_id: env.id.clone(),
                pid: session.pid,
                debug_port: session.debug_port,
                page_ws: ws,
                last_active: Mutex::new(Instant::now()),
                runtime: RwLock::new(runtime),
                client: Arc::new(client),
                mocks: AsyncRwLock::new(mocks),
                proxy_auth,
                recording: Mutex::new(None),
                har: Arc::new(Mutex::new(HarState::default())),
                closing: AtomicBool::new(false),
                captcha_last: Mutex::new(None),
                memory_alert_last: Mutex::new(None),
                current_url: Mutex::new(page_url),
            });
            if let Ok(mut map) = self.instances.write() {
                map.insert(env.id.clone(), instance.clone());
            }
            #[cfg(windows)]
            overlay::spawn(
                session.pid,
                env.name.clone(),
                env.taskbar_badge_text.clone(),
                effective_border_color(&env),
                env.border_width,
            );
            self.start_event_handler(instance);
            let _ = Audit::env(
                &self.db,
                "system",
                actions::SESSION_RECONNECT,
                &env,
                format!("面板重启后重新接管浏览器进程 PID {}", session.pid),
            );
            restored += 1;
        }
        Ok(restored)
    }

    // -----------------------------------------------------------------------
    // Runtime operations
    // -----------------------------------------------------------------------

    fn instance(&self, env_id: &str) -> Result<Arc<BrowserInstance>> {
        self.instances
            .read()
            .ok()
            .and_then(|m| m.get(env_id).cloned())
            .ok_or_else(|| AppError::bad_request("环境未打开，请先启动浏览器"))
    }

    fn touch(&self, instance: &BrowserInstance) {
        if let Ok(mut t) = instance.last_active.lock() {
            *t = Instant::now();
        }
        if let Ok(mut runtime) = instance.runtime.write() {
            runtime.last_active_at = now_iso();
        }
    }

    pub async fn screenshot(
        self: &Arc<Self>,
        env_id: &str,
        actor: &str,
    ) -> Result<ScreenshotResult> {
        let instance = self.instance(env_id)?;
        let data = capture_screenshot(&instance).await?;
        let (width, height) = cdp::png_dimensions(&data);
        let path = paths::screenshots_dir(&self.data_dir).join(format!(
            "{}-{}.png",
            env_id,
            chrono::Utc::now().timestamp_millis()
        ));
        let bytes = B64
            .decode(&data)
            .map_err(|_| AppError::internal("截图 base64 解码失败"))?;
        std::fs::write(&path, &bytes)?;
        self.touch(&instance);
        let env = self.db.get_environment(env_id)?;
        Audit::env(
            &self.db,
            actor,
            actions::SCREENSHOT,
            &env,
            format!("截图保存至 {}", path.display()),
        )?;
        let result = ScreenshotResult {
            env_id: env_id.to_string(),
            path: path.to_string_lossy().to_string(),
            data_url: format!("data:image/png;base64,{data}"),
            width,
            height,
            captured_at: now_iso(),
        };
        self.callback(
            "screenshot",
            json!({
                "envId": env_id,
                "envName": env.name,
                "path": result.path,
                "width": width,
                "height": height,
                "capturedAt": result.captured_at,
                "dataUrl": result.data_url
            }),
        )
        .await;
        Ok(result)
    }

    pub async fn cookies_get(
        self: &Arc<Self>,
        env_id: &str,
        actor: &str,
        audit: bool,
    ) -> Result<Vec<CookieRecord>> {
        let instance = self.instance(env_id)?;
        let value = instance
            .client
            .call("Network.getAllCookies", json!({}))
            .await?;
        let cookies = cdp::cookies_from_cdp(&value);
        if let Ok(raw) = serde_json::to_string(&cookies) {
            let enc = self.crypto.encrypt(&raw)?;
            self.db.save_cookie_cache(env_id, &enc)?;
        }
        self.touch(&instance);
        if audit {
            if let Ok(env) = self.db.get_environment(env_id) {
                Audit::env(
                    &self.db,
                    actor,
                    actions::COOKIE_READ,
                    &env,
                    format!("读取 Cookie {} 条（密文缓存已更新）", cookies.len()),
                )?;
            }
        }
        Ok(cookies)
    }

    pub async fn cookies_get_cached(&self, env_id: &str) -> Result<Vec<CookieRecord>> {
        if let Some(raw) = self.db.load_cookie_cache(env_id)? {
            let plain = self.crypto.decrypt(&raw)?;
            return Ok(serde_json::from_str(&plain).unwrap_or_default());
        }
        Ok(vec![])
    }

    pub async fn cookies_set(
        self: &Arc<Self>,
        env_id: &str,
        cookies: &[CookieRecord],
        actor: &str,
    ) -> Result<u32> {
        let instance = self.instance(env_id)?;
        instance
            .client
            .call("Network.setCookies", cdp::cookies_to_cdp(cookies))
            .await?;
        let current = instance
            .client
            .call("Network.getAllCookies", json!({}))
            .await?;
        let all = cdp::cookies_from_cdp(&current);
        let raw = serde_json::to_string(&all)?;
        let enc = self.crypto.encrypt(&raw)?;
        self.db.save_cookie_cache(env_id, &enc)?;
        self.touch(&instance);
        if let Ok(env) = self.db.get_environment(env_id) {
            Audit::env(
                &self.db,
                actor,
                actions::COOKIE_WRITE,
                &env,
                format!("写入 Cookie {} 条", cookies.len()),
            )?;
        }
        Ok(cookies.len() as u32)
    }

    pub async fn execute(self: &Arc<Self>, env_id: &str, script: &str) -> Result<Value> {
        let instance = self.instance(env_id)?;
        let value = instance.client.evaluate(script).await?;
        self.touch(&instance);
        Ok(value)
    }

    pub async fn reload(self: &Arc<Self>, env_id: &str) -> Result<()> {
        let instance = self.instance(env_id)?;
        instance
            .client
            .call("Page.reload", json!({ "ignoreCache": false }))
            .await?;
        Ok(())
    }

    pub async fn navigate(self: &Arc<Self>, env_id: &str, url: &str) -> Result<String> {
        if !url.starts_with("http://") && !url.starts_with("https://") && url != "about:blank" {
            return Err(AppError::bad_request("仅支持 http/https/about:blank URL"));
        }
        let instance = self.instance(env_id)?;
        let result = instance
            .client
            .call("Page.navigate", json!({ "url": url }))
            .await?;
        if let Some(error) = result.get("errorText").and_then(|v| v.as_str()) {
            return Err(AppError::bad_request(format!("页面导航失败: {error}")));
        }
        wait_for_ready(&instance.client, Duration::from_secs(15)).await;
        if let Ok(mut current) = instance.current_url.lock() {
            *current = url.to_string();
        }
        if let Ok(mut runtime) = instance.runtime.write() {
            runtime.current_url = url.to_string();
        }
        self.touch(&instance);
        self.emit(
            "browser://runtime",
            instance.runtime.read().unwrap().clone(),
        );
        Ok(url.to_string())
    }

    pub fn cdp_info(&self, env_id: &str) -> Result<crate::models::CdpInfo> {
        let instance = self.instance(env_id)?;
        Ok(crate::models::CdpInfo {
            env_id: env_id.to_string(),
            debug_port: instance.debug_port,
            ws_endpoint: instance.page_ws.clone(),
            http_endpoint: format!("http://127.0.0.1:{}", instance.debug_port),
            running: true,
            playwright_example: format!(
                "const browser = await chromium.connectOverCDP('http://127.0.0.1:{}');",
                instance.debug_port
            ),
        })
    }

    pub async fn apply_fingerprint(
        &self,
        env_id: &str,
        fp: &fingerprint::Fingerprint,
    ) -> Result<()> {
        let instance = self.instance(env_id)?;
        cdp::install_fingerprint(&instance.client, fp).await?;
        self.touch(&instance);
        Ok(())
    }

    pub async fn set_mocks(&self, env_id: &str, rules: Vec<MockRule>) -> Result<()> {
        let instance = self.instance(env_id)?;
        let mut guard = instance.mocks.write().await;
        *guard = rules;
        self.touch(&instance);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Recording
    // -----------------------------------------------------------------------

    pub async fn start_recording(
        self: &Arc<Self>,
        env_id: &str,
        interval_ms: u64,
        actor: &str,
    ) -> Result<RecordingResult> {
        let instance = self.instance(env_id)?;
        {
            let guard = instance
                .recording
                .lock()
                .map_err(|_| AppError::internal("录制状态锁失败"))?;
            if guard.is_some() {
                return Err(AppError::conflict("该环境正在录制"));
            }
        }
        let interval = interval_ms.clamp(200, 10_000);
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let dir = paths::recordings_dir(&self.data_dir).join(format!("{env_id}-{stamp}"));
        paths::ensure_dir(&dir)?;
        let stop = Arc::new(AtomicBool::new(false));
        let frames = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let started_at = now_iso();
        let client = instance.client.clone();
        let stop_clone = stop.clone();
        let frames_clone = frames.clone();
        let dir_clone = dir.clone();
        let task = tokio::spawn(async move {
            let mut index = 0u32;
            while !stop_clone.load(Ordering::SeqCst) {
                let begin = Instant::now();
                if let Ok(data) = client
                    .call(
                        "Page.captureScreenshot",
                        json!({ "format": "png", "fromSurface": true }),
                    )
                    .await
                {
                    if let Some(b64) = data.get("data").and_then(|v| v.as_str()) {
                        if let Ok(bytes) = B64.decode(b64) {
                            index += 1;
                            let path = dir_clone.join(format!("frame-{index:05}.png"));
                            if std::fs::write(path, bytes).is_ok() {
                                frames_clone.store(index, Ordering::SeqCst);
                            }
                        }
                    }
                }
                let elapsed = begin.elapsed();
                let wait = Duration::from_millis(interval).saturating_sub(elapsed);
                sleep(wait).await;
            }
        });
        if let Ok(mut runtime) = instance.runtime.write() {
            runtime.recording = true;
        }
        if let Ok(mut guard) = instance.recording.lock() {
            *guard = Some(RecordingHandle {
                stop,
                dir: dir.clone(),
                started_at: started_at.clone(),
                frames: frames.clone(),
                task,
            });
        }
        let env = self.db.get_environment(env_id)?;
        Audit::env(
            &self.db,
            actor,
            actions::RECORDING,
            &env,
            format!("开始录屏，间隔 {}ms，目录 {}", interval, dir.display()),
        )?;
        self.emit(
            "browser://runtime",
            instance.runtime.read().unwrap().clone(),
        );
        Ok(RecordingResult {
            env_id: env_id.to_string(),
            directory: dir.to_string_lossy().to_string(),
            frame_count: 0,
            duration_ms: 0,
            manifest_path: String::new(),
            started_at,
            stopped_at: now_iso(),
        })
    }

    async fn stop_recording_internal(
        self: &Arc<Self>,
        instance: &Arc<BrowserInstance>,
        write_manifest: bool,
    ) -> Option<RecordingResult> {
        let handle = instance.recording.lock().ok().and_then(|mut g| g.take());
        let handle = handle?;
        handle.stop.store(true, Ordering::SeqCst);
        let _ = tokio::time::timeout(Duration::from_secs(6), handle.task).await;
        let frame_count = handle.frames.load(Ordering::SeqCst);
        let stopped_at = now_iso();
        let duration_ms = chrono::DateTime::parse_from_rfc3339(&stopped_at)
            .ok()
            .and_then(|end| {
                chrono::DateTime::parse_from_rfc3339(&handle.started_at)
                    .ok()
                    .map(|start| (end - start).num_milliseconds().max(0) as u64)
            })
            .unwrap_or(0);
        let manifest_path = if write_manifest {
            let manifest = json!({
                "envId": instance.env_id,
                "startedAt": handle.started_at,
                "stoppedAt": stopped_at,
                "frameCount": frame_count,
                "durationMs": duration_ms,
                "frames": (1..=frame_count).map(|i| format!("frame-{i:05}.png")).collect::<Vec<_>>()
            });
            let path = handle.dir.join("manifest.json");
            let _ = std::fs::write(
                &path,
                serde_json::to_vec_pretty(&manifest).unwrap_or_default(),
            );
            path.to_string_lossy().to_string()
        } else {
            String::new()
        };
        if let Ok(mut runtime) = instance.runtime.write() {
            runtime.recording = false;
        }
        let result = RecordingResult {
            env_id: instance.env_id.clone(),
            directory: handle.dir.to_string_lossy().to_string(),
            frame_count,
            duration_ms,
            manifest_path,
            started_at: handle.started_at,
            stopped_at,
        };
        self.callback(
            "recording",
            json!({
                "envId": result.env_id,
                "directory": result.directory,
                "frameCount": result.frame_count,
                "durationMs": result.duration_ms,
                "manifest": result.manifest_path
            }),
        )
        .await;
        Some(result)
    }

    pub async fn stop_recording(
        self: &Arc<Self>,
        env_id: &str,
        actor: &str,
    ) -> Result<RecordingResult> {
        let instance = self.instance(env_id)?;
        let result = self
            .stop_recording_internal(&instance, true)
            .await
            .ok_or_else(|| AppError::bad_request("该环境当前没有进行中的录屏"))?;
        let env = self.db.get_environment(env_id)?;
        Audit::env(
            &self.db,
            actor,
            actions::RECORDING,
            &env,
            format!(
                "结束录屏，共 {} 帧，时长 {}ms，清单 {}",
                result.frame_count, result.duration_ms, result.manifest_path
            ),
        )?;
        self.emit(
            "browser://runtime",
            instance.runtime.read().unwrap().clone(),
        );
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // HAR
    // -----------------------------------------------------------------------

    pub async fn start_har(self: &Arc<Self>, env_id: &str, actor: &str) -> Result<()> {
        let instance = self.instance(env_id)?;
        {
            let mut har = instance
                .har
                .lock()
                .map_err(|_| AppError::internal("HAR 状态锁失败"))?;
            har.active = true;
            har.started_at = Some(now_iso());
            har.entries.clear();
            har.pending.clear();
        }
        if let Ok(mut runtime) = instance.runtime.write() {
            runtime.har_recording = true;
        }
        let env = self.db.get_environment(env_id)?;
        Audit::env(&self.db, actor, actions::HAR_EXPORT, &env, "开始 HAR 抓包")?;
        Ok(())
    }

    pub async fn stop_har(self: &Arc<Self>, env_id: &str, actor: &str) -> Result<String> {
        let instance = self.instance(env_id)?;
        {
            let mut har = instance
                .har
                .lock()
                .map_err(|_| AppError::internal("HAR 状态锁失败"))?;
            har.active = false;
        }
        if let Ok(mut runtime) = instance.runtime.write() {
            runtime.har_recording = false;
        }
        let path = self.finalize_har(&instance, true).await?;
        let env = self.db.get_environment(env_id)?;
        Audit::env(
            &self.db,
            actor,
            actions::HAR_EXPORT,
            &env,
            format!("HAR 导出至 {path}"),
        )?;
        Ok(path)
    }

    async fn finalize_har(
        &self,
        instance: &Arc<BrowserInstance>,
        explicit: bool,
    ) -> Result<String> {
        let (active, entries, har_json) = {
            let har = instance
                .har
                .lock()
                .map_err(|_| AppError::internal("HAR 状态锁失败"))?;
            (har.active, har.entries.len(), har.build())
        };
        if !active && !explicit {
            return Ok(String::new());
        }
        if entries == 0 && !explicit {
            return Ok(String::new());
        }
        let path = paths::har_dir(&self.data_dir).join(format!(
            "{}-{}.har",
            instance.env_id,
            chrono::Utc::now().timestamp_millis()
        ));
        std::fs::write(&path, serde_json::to_vec_pretty(&har_json)?)?;
        Ok(path.to_string_lossy().to_string())
    }

    pub async fn capture_captcha_screenshot(&self, env_id: &str) -> Result<Option<String>> {
        let instance = self.instance(env_id)?;
        let data = capture_screenshot(&instance).await?;
        let path = paths::screenshots_dir(&self.data_dir).join(format!(
            "captcha-{}-{}.png",
            env_id,
            chrono::Utc::now().timestamp_millis()
        ));
        let bytes = B64
            .decode(&data)
            .map_err(|_| AppError::internal("截图解码失败"))?;
        std::fs::write(&path, bytes)?;
        Ok(Some(path.to_string_lossy().to_string()))
    }

    // -----------------------------------------------------------------------
    // CDP event handling
    // -----------------------------------------------------------------------

    fn start_event_handler(self: &Arc<Self>, instance: Arc<BrowserInstance>) {
        let mut rx = instance.client.subscribe();
        let manager = self.clone();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => match event.method.as_str() {
                        "Fetch.requestPaused" => {
                            let instance = instance.clone();
                            tokio::spawn(async move {
                                if let Err(e) = handle_request_paused(&instance, event.params).await
                                {
                                    tracing::debug!(error = %e, "mock request handling failed");
                                }
                            });
                        }
                        "Fetch.authRequired" => {
                            let instance = instance.clone();
                            tokio::spawn(async move {
                                let params = event.params;
                                let request_id = params
                                    .get("requestId")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("")
                                    .to_string();
                                let is_proxy = params
                                    .get("authChallenge")
                                    .and_then(|challenge| challenge.get("source"))
                                    .and_then(|value| value.as_str())
                                    .map(|source| source.eq_ignore_ascii_case("proxy"))
                                    .unwrap_or(false);
                                let response = if is_proxy {
                                    if let Some((username, password)) = instance.proxy_auth.as_ref()
                                    {
                                        json!({
                                            "response": "ProvideCredentials",
                                            "username": username.clone(),
                                            "password": password.clone()
                                        })
                                    } else {
                                        json!({ "response": "Default" })
                                    }
                                } else {
                                    json!({ "response": "Default" })
                                };
                                let _ = instance
                                    .client
                                    .call(
                                        "Fetch.continueWithAuth",
                                        json!({
                                            "requestId": request_id,
                                            "authChallengeResponse": response
                                        }),
                                    )
                                    .await;
                            });
                        }
                        "Runtime.bindingCalled" => {
                            let params = event.params;
                            let name = params.get("name").and_then(|v| v.as_str());
                            if name == Some("__finEnvCaptcha") {
                                let manager = manager.clone();
                                let instance = instance.clone();
                                tokio::spawn(async move {
                                    let payload = params
                                        .get("payload")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("{}");
                                    let _ = manager.handle_captcha(&instance, payload).await;
                                });
                            }
                        }
                        "Network.requestWillBeSent" => {
                            manager.mark_activity(&instance.env_id);
                            if let Ok(mut har) = instance.har.lock() {
                                har.on_request(&event.params);
                            }
                        }
                        "Network.responseReceived" => {
                            if let Ok(mut har) = instance.har.lock() {
                                har.on_response(&event.params);
                            }
                        }
                        "Network.loadingFinished" => {
                            if let Ok(mut har) = instance.har.lock() {
                                har.on_finished(&event.params);
                            }
                        }
                        "Page.frameNavigated" => {
                            manager.mark_activity(&instance.env_id);
                            let params = event.params;
                            if params
                                .get("frame")
                                .and_then(|f| f.get("parentId"))
                                .is_none()
                            {
                                if let Some(url) = params
                                    .get("frame")
                                    .and_then(|f| f.get("url"))
                                    .and_then(|v| v.as_str())
                                {
                                    if let Ok(mut current) = instance.current_url.lock() {
                                        *current = url.to_string();
                                    }
                                    if let Ok(mut runtime) = instance.runtime.write() {
                                        runtime.current_url = url.to_string();
                                    }
                                }
                            }
                        }
                        "Page.javascriptDialogOpening" => {
                            let message = event
                                .params
                                .get("message")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            if message.to_ascii_lowercase().contains("captcha")
                                || message.contains("验证码")
                            {
                                let manager = manager.clone();
                                let instance = instance.clone();
                                tokio::spawn(async move {
                                    let payload = json!({ "url": instance.current_url.lock().map(|u| u.clone()).unwrap_or_default(), "hint": message }).to_string();
                                    let _ = manager.handle_captcha(&instance, &payload).await;
                                });
                            }
                        }
                        _ => {}
                    },
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        tracing::warn!(skipped, env_id = %instance.env_id, "CDP events lagged");
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    async fn handle_captcha(
        self: &Arc<Self>,
        instance: &Arc<BrowserInstance>,
        payload: &str,
    ) -> Result<()> {
        {
            let Ok(mut last) = instance.captcha_last.lock() else {
                return Ok(());
            };
            if let Some(previous) = *last {
                if previous.elapsed() < Duration::from_secs(8) {
                    return Ok(());
                }
            }
            *last = Some(Instant::now());
        }
        let parsed: Value =
            serde_json::from_str(payload).unwrap_or_else(|_| json!({ "hint": payload }));
        let url = parsed
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let hint = parsed
            .get("hint")
            .and_then(|v| v.as_str())
            .unwrap_or("检测到疑似验证码")
            .to_string();
        let screenshot = self
            .capture_captcha_screenshot(&instance.env_id)
            .await
            .ok()
            .flatten();
        let event =
            self.db
                .insert_captcha(&instance.env_id, &url, "auto", &hint, screenshot.as_deref())?;
        self.emit("captcha://new", event.clone());
        self.notify(
            "FinEnv 发现验证码",
            &format!(
                "{} · {} · 可在验证码聚合面板处理",
                event.env_name, event.url
            ),
        );
        self.callback(
            "captcha",
            json!({
                "envId": event.env_id,
                "envName": event.env_name,
                "url": event.url,
                "hint": event.hint,
                "screenshot": event.screenshot_path,
                "createdAt": event.created_at
            }),
        )
        .await;
        Ok(())
    }

    async fn callback(&self, event_type: &str, mut payload: Value) {
        let settings = self.settings();
        let Some(url) = settings
            .callback_url
            .clone()
            .filter(|u| !u.trim().is_empty())
        else {
            return;
        };
        payload["type"] = json!(event_type);
        payload["timestamp"] = json!(now_iso());
        payload["callbackType"] = json!(settings.callback_type);
        let token = settings
            .callback_token_enc
            .as_deref()
            .and_then(|enc| self.crypto.decrypt(enc).ok());
        tokio::spawn(async move {
            let client = reqwest::Client::new();
            let mut request = client
                .post(&url)
                .json(&payload)
                .timeout(Duration::from_secs(8));
            if let Some(token) = token.filter(|t| !t.is_empty()) {
                request = request.bearer_auth(token);
            }
            if let Err(e) = request.send().await {
                tracing::warn!(callback = %url, error = %e, "callback delivery failed");
            }
        });
    }

    pub async fn resource_snapshot(&self) -> crate::models::ResourceSnapshot {
        let instances = self.list_runtimes();
        let total_memory = instances.iter().map(|i| i.memory_mb).sum();
        let running = instances.len() as u32;
        crate::models::ResourceSnapshot {
            instances,
            total_memory_mb: total_memory,
            max_concurrent: self.settings().max_concurrent_browsers,
            running_count: running,
            sampled_at: now_iso(),
            free_memory_mb: free_memory_mb(),
        }
    }

    pub fn mark_activity(&self, env_id: &str) {
        if let Ok(map) = self.instances.read() {
            if let Some(instance) = map.get(env_id) {
                self.touch(instance);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Browser process helpers
// ---------------------------------------------------------------------------

/// Static Chromium flags shared by every environment. This list is also
/// exposed in Settings > System > Browser launch arguments so users can see
/// what FinEnv adds before appending their own flags.
pub const DEFAULT_CHROMIUM_ARGS: &[&str] = &[
    "--no-first-run",
    "--no-default-browser-check",
    "--disable-background-networking",
    "--disable-component-update",
    "--disable-sync",
    "--disable-default-apps",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-hang-monitor",
    "--disable-dev-shm-usage",
    "--disable-quic",
    "--dns-prefetch-disable",
    "--disable-features=Translate,OptimizationHints,MediaRouter,InterestFeedContentSuggestions,PreconnectToSearch,NetworkPrediction",
    "--no-pings",
];

pub fn default_chromium_args() -> Vec<String> {
    DEFAULT_CHROMIUM_ARGS
        .iter()
        .map(|arg| (*arg).to_string())
        .collect()
}

/// Combine global and per-environment custom switches. A later environment
/// value replaces an earlier global value with the same switch name, so the
/// per-environment setting always wins.
fn is_maximize_switch(name: &str) -> bool {
    matches!(name, "--start-maximized" | "--start-fullscreen" | "--kiosk")
}

fn merge_launch_args(global: &[String], environment: &[String]) -> Vec<String> {
    let environment_has_window_size = environment
        .iter()
        .any(|arg| arg.split('=').next() == Some("--window-size"));
    let environment_has_maximize = environment
        .iter()
        .any(|arg| is_maximize_switch(arg.split('=').next().unwrap_or(arg)));
    let mut merged = global
        .iter()
        .filter(|arg| {
            let name = arg.split('=').next().unwrap_or(arg);
            !(environment_has_window_size && is_maximize_switch(name))
                && !(environment_has_maximize && name == "--window-size")
        })
        .cloned()
        .collect::<Vec<_>>();
    for arg in environment {
        let name = arg.split('=').next().unwrap_or(arg);
        merged.retain(|existing| existing.split('=').next() != Some(name));
        merged.push(arg.clone());
    }
    merged
}

fn network_emulation_params(profile: &str) -> Value {
    let (offline, latency, download, upload, packet_loss) = match profile {
        "offline" => (true, 0, 0, 0, None),
        "3g" => (false, 400, 50_000, 50_000, None),
        "4g" => (false, 80, 524_288, 393_216, None),
        "5g" => (false, 20, 2_621_440, 1_310_720, None),
        "slow" => (false, 800, 25_600, 12_800, None),
        "lossy" => (false, 300, 131_072, 65_536, Some(10.0)),
        _ => (false, 0, -1, -1, None),
    };
    let mut params = json!({
        "offline": offline,
        "latency": latency,
        "downloadThroughput": download,
        "uploadThroughput": upload
    });
    if let Some(loss) = packet_loss {
        params["packetLoss"] = json!(loss);
    }
    params
}

#[allow(clippy::too_many_arguments)]
fn build_chromium_args(
    fp: &fingerprint::Fingerprint,
    profile_dir: &Path,
    port: u16,
    proxy_url: Option<&str>,
    hosts: &[crate::models::HostsEntry],
    extensions: &[String],
    window_name: &str,
    start_url: &str,
    headless: bool,
    extra_args: &[String],
) -> Vec<String> {
    let mut args = vec![
        format!("--user-data-dir={}", profile_dir.display()),
        "--profile-directory=Default".into(),
        format!("--remote-debugging-port={port}"),
        "--remote-debugging-address=127.0.0.1".into(),
        "--remote-allow-origins=*".into(),
    ];
    args.extend(DEFAULT_CHROMIUM_ARGS.iter().map(|arg| (*arg).to_string()));
    // Chromium ignores --start-maximized when an explicit --window-size is
    // present. Let the user's maximize/fullscreen/kiosk request take priority.
    let wants_maximized = extra_args
        .iter()
        .any(|arg| is_maximize_switch(arg.split('=').next().unwrap_or(arg)));
    let has_window_size = extra_args
        .iter()
        .any(|arg| arg.split('=').next() == Some("--window-size"));
    if !wants_maximized && !has_window_size {
        let min_width = if fp.client_hints.mobile { 320 } else { 800 };
        let min_height = if fp.client_hints.mobile { 480 } else { 600 };
        args.push(format!(
            "--window-size={},{}",
            fp.screen.avail_width.max(min_width),
            fp.screen.avail_height.max(min_height)
        ));
    }
    args.push(format!(
        "--lang={}",
        fp.languages
            .first()
            .cloned()
            .unwrap_or_else(|| "en-US".into())
    ));
    args.push(format!("--accept-lang={}", fp.languages.join(",")));
    if headless {
        args.push("--headless=new".into());
    }
    if !window_name.trim().is_empty() {
        args.push(format!("--window-name={window_name}"));
    }
    if let Some(proxy) = proxy_url {
        args.push(format!("--proxy-server={proxy}"));
        args.push("--proxy-bypass-list=<-loopback>".into());
    }
    if fp.webrtc.mode != "public" {
        args.push("--force-webrtc-ip-handling-policy=disable_non_proxied_udp".into());
        args.push("--webrtc-ip-handling-policy=disable_non_proxied_udp".into());
    }
    let enabled_hosts: Vec<String> = hosts
        .iter()
        .filter(|h| h.enabled)
        .map(|h| format!("MAP {} {}", h.host, h.ip))
        .collect();
    if !enabled_hosts.is_empty() {
        args.push(format!(
            "--host-resolver-rules={}",
            enabled_hosts.join(", ")
        ));
    }
    if !extensions.is_empty() {
        args.push(format!("--load-extension={}", extensions.join(",")));
    }
    args.extend(extra_args.iter().cloned());
    if !start_url.is_empty() {
        args.push(start_url.to_string());
    }
    args
}

fn resolve_chromium(env_path: Option<&str>, settings_path: Option<&str>) -> Result<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    let chrome_env = std::env::var("CHROME_PATH").ok();
    for path in [env_path, settings_path, chrome_env.as_deref()]
        .into_iter()
        .flatten()
    {
        if !path.trim().is_empty() {
            candidates.push(PathBuf::from(path));
        }
    }
    let bundled = paths::bundled_chromium_dir();
    candidates.push(bundled.join("chrome.exe"));
    candidates.push(bundled.join("chromium.exe"));
    candidates.push(bundled.join("chrome-win").join("chrome.exe"));
    candidates.push(bundled.join("Chromium.app/Contents/MacOS/Chromium"));
    candidates.push(bundled.join("chrome"));
    if let Ok(program_files) = std::env::var("PROGRAMFILES") {
        candidates.push(PathBuf::from(program_files).join("Google/Chrome/Application/chrome.exe"));
    }
    if let Ok(program_files_x86) = std::env::var("PROGRAMFILES(X86)") {
        candidates
            .push(PathBuf::from(program_files_x86).join("Google/Chrome/Application/chrome.exe"));
    }
    if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
        candidates
            .push(PathBuf::from(&local_app_data).join("Google/Chrome/Application/chrome.exe"));
        candidates.push(PathBuf::from(local_app_data).join("Chromium/Application/chrome.exe"));
    }
    if let Ok(program_files) = std::env::var("PROGRAMFILES") {
        candidates.push(PathBuf::from(program_files).join("Microsoft/Edge/Application/msedge.exe"));
    }
    candidates.push(PathBuf::from(
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ));
    candidates.push(PathBuf::from("/usr/bin/google-chrome"));
    candidates.push(PathBuf::from("/usr/bin/chromium"));
    candidates.push(PathBuf::from("/usr/bin/chromium-browser"));
    for candidate in &candidates {
        if candidate.is_file() {
            return Ok(candidate.clone());
        }
    }
    // Fall back to a PATH lookup.
    if let Ok(path_var) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path_var) {
            for name in ["chrome", "chromium", "chromium-browser", "google-chrome"] {
                let path = dir.join(name);
                if path.is_file() {
                    return Ok(path);
                }
            }
        }
    }
    Err(AppError::code_with(
        "chromium_not_found",
        "未找到 Chromium/Chrome。请在设置中指定 chromium 路径，或将锁定版本放入 chromium-bin/ 目录。",
    ))
}

pub fn detect_chromium_with(configured: Option<&str>) -> Option<PathBuf> {
    resolve_chromium(None, configured).ok()
}

pub fn chromium_version(path: &Path) -> Option<String> {
    let output = std::process::Command::new(path)
        .arg("--version")
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn free_port() -> Result<u16> {
    let listener = std::net::TcpListener::bind(("127.0.0.1", 0))
        .map_err(|e| AppError::internal(format!("无法分配调试端口: {e}")))?;
    let port = listener
        .local_addr()
        .map_err(|e| AppError::internal(format!("无法读取调试端口: {e}")))?
        .port();
    drop(listener);
    Ok(port)
}

async fn wait_for_cdp_page(port: u16, timeout: Duration) -> Result<(String, String)> {
    let deadline = Instant::now() + timeout;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
        .map_err(AppError::from)?;
    let mut last_error = String::new();
    while Instant::now() < deadline {
        match client
            .get(format!("http://127.0.0.1:{port}/json/list"))
            .send()
            .await
        {
            Ok(response) => {
                if let Ok(value) = response.json::<Value>().await {
                    if let Some(targets) = value.as_array() {
                        if let Some(target) = targets
                            .iter()
                            .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
                        {
                            if let Some(ws) =
                                target.get("webSocketDebuggerUrl").and_then(|v| v.as_str())
                            {
                                let url = target
                                    .get("url")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or("about:blank")
                                    .to_string();
                                return Ok((ws.to_string(), url));
                            }
                        }
                    }
                }
                last_error = "未发现可调试页面".into();
            }
            Err(e) => last_error = e.to_string(),
        }
        sleep(Duration::from_millis(250)).await;
    }
    Err(AppError::internal(format!(
        "等待 Chromium CDP 端口 {port} 超时: {last_error}"
    )))
}

async fn get_page_ws(port: u16) -> Result<(String, String)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(1500))
        .build()
        .map_err(AppError::from)?;
    let value: Value = client
        .get(format!("http://127.0.0.1:{port}/json/list"))
        .send()
        .await?
        .json()
        .await?;
    let targets = value
        .as_array()
        .ok_or_else(|| AppError::internal("CDP targets 格式异常"))?;
    let target = targets
        .iter()
        .find(|t| t.get("type").and_then(|v| v.as_str()) == Some("page"))
        .ok_or_else(|| AppError::internal("未找到页面 target"))?;
    let ws = target
        .get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::internal("页面 target 缺少调试地址"))?
        .to_string();
    let url = target
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("about:blank")
        .to_string();
    Ok((ws, url))
}

async fn cdp_http_alive(port: u16) -> bool {
    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build()
    {
        Ok(c) => c,
        Err(_) => return false,
    };
    client
        .get(format!("http://127.0.0.1:{port}/json/version"))
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false)
}

async fn wait_for_ready(client: &CdpClient, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Ok(state) = client.evaluate("document.readyState").await {
            if state.as_str() == Some("complete") || state.as_str() == Some("interactive") {
                return;
            }
        }
        sleep(Duration::from_millis(150)).await;
    }
}

async fn capture_screenshot(instance: &BrowserInstance) -> Result<String> {
    let value = instance
        .client
        .call(
            "Page.captureScreenshot",
            json!({ "format": "png", "fromSurface": true, "captureBeyondViewport": false }),
        )
        .await?;
    value
        .get("data")
        .and_then(|v| v.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| AppError::internal("CDP 未返回截图数据"))
}

fn process_tree_stats(
    system: &sysinfo::System,
    roots: &[sysinfo::Pid],
) -> HashMap<u32, (f32, u64)> {
    let mut children: HashMap<sysinfo::Pid, Vec<sysinfo::Pid>> = HashMap::new();
    for (pid, process) in system.processes() {
        if let Some(parent) = process.parent() {
            children.entry(parent).or_default().push(*pid);
        }
    }
    let mut result = HashMap::new();
    for root in roots {
        let mut total_memory = 0u64;
        let mut total_cpu = 0f32;
        let mut stack = vec![*root];
        let mut seen = std::collections::HashSet::new();
        while let Some(pid) = stack.pop() {
            if !seen.insert(pid) {
                continue;
            }
            if let Some(process) = system.process(pid) {
                total_memory = total_memory.saturating_add(process.memory());
                total_cpu += process.cpu_usage();
            }
            if let Some(list) = children.get(&pid) {
                stack.extend(list.iter().copied());
            }
        }
        result.insert(root.as_u32(), (total_cpu, total_memory / 1024 / 1024));
    }
    result
}

async fn kill_tree(pid: u32) {
    if pid == 0 {
        return;
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .await;
    }
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status()
            .await;
        sleep(Duration::from_millis(300)).await;
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status()
            .await;
    }
}

fn free_memory_mb() -> u64 {
    let mut system = sysinfo::System::new_all();
    system.refresh_memory();
    system.available_memory() / 1024 / 1024
}

#[cfg(windows)]
const BORDER_PALETTE: [&str; 10] = [
    "#2563EB", "#7C3AED", "#DB2777", "#DC2626", "#EA580C", "#CA8A04", "#16A34A", "#0D9488",
    "#0891B2", "#475569",
];

#[cfg(windows)]
fn effective_border_color(env: &crate::models::Environment) -> String {
    let value = env.border_color.trim();
    let hex = value.trim_start_matches('#');
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return format!("#{}", hex.to_ascii_uppercase());
    }
    let hash = env.id.bytes().fold(0u32, |acc, byte| {
        acc.wrapping_mul(31).wrapping_add(byte as u32)
    });
    BORDER_PALETTE[(hash as usize) % BORDER_PALETTE.len()].to_string()
}

#[cfg(windows)]
struct WindowSearch {
    pid: u32,
    handles: Vec<windows_sys::Win32::Foundation::HWND>,
}

#[cfg(windows)]
unsafe extern "system" fn enum_window(
    hwnd: windows_sys::Win32::Foundation::HWND,
    lparam: windows_sys::Win32::Foundation::LPARAM,
) -> windows_sys::Win32::Foundation::BOOL {
    use windows_sys::Win32::UI::WindowsAndMessaging::{GetWindowThreadProcessId, IsWindowVisible};

    let search = unsafe { &mut *(lparam as *mut WindowSearch) };
    let mut window_pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut window_pid) };
    if window_pid == search.pid && unsafe { IsWindowVisible(hwnd) } != 0 {
        search.handles.push(hwnd);
    }
    1
}

#[cfg(windows)]
fn window_handles(pid: u32) -> Vec<windows_sys::Win32::Foundation::HWND> {
    use windows_sys::Win32::UI::WindowsAndMessaging::EnumWindows;

    let mut search = WindowSearch {
        pid,
        handles: Vec::new(),
    };
    unsafe {
        EnumWindows(
            Some(enum_window),
            &mut search as *mut WindowSearch as windows_sys::Win32::Foundation::LPARAM,
        );
    }
    search.handles
}

#[cfg(windows)]
fn parse_border_color(value: &str) -> Option<u32> {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u32::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u32::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u32::from_str_radix(&hex[4..6], 16).ok()?;
    Some(r | (g << 8) | (b << 16))
}

/// Windows 11 exposes a per-window border color through DWM. Apply it once the
/// Chromium top-level window exists; unsupported systems silently ignore it.
#[cfg(windows)]
async fn apply_window_border(pid: u32, color: &str) {
    use windows_sys::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWA_BORDER_COLOR};

    let Some(colorref) = parse_border_color(color) else {
        return;
    };
    for _ in 0..20 {
        {
            let handles = window_handles(pid);
            if !handles.is_empty() {
                for hwnd in handles {
                    let _ = unsafe {
                        DwmSetWindowAttribute(
                            hwnd,
                            DWMWA_BORDER_COLOR as u32,
                            &colorref as *const u32 as *const std::ffi::c_void,
                            std::mem::size_of::<u32>() as u32,
                        )
                    };
                }
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
}

async fn handle_request_paused(instance: &Arc<BrowserInstance>, params: Value) -> Result<()> {
    let request_id = params
        .get("requestId")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if request_id.is_empty() {
        return Err(AppError::internal("Fetch 事件缺少 requestId"));
    }
    let request = params.get("request").cloned().unwrap_or(Value::Null);
    let url = request
        .get("url")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let method = request
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("GET")
        .to_string();
    let rules = instance.mocks.read().await.clone();
    if let Some(rule) = cdp::find_mock(&rules, &url, &method) {
        if rule.delay_ms > 0 {
            sleep(Duration::from_millis(rule.delay_ms as u64)).await;
        }
        let mut headers = cdp::headers_to_cdp(&rule.headers);
        if rule
            .headers
            .get("content-type")
            .and_then(|v| v.as_str())
            .is_none()
        {
            if let Some(array) = headers.as_array_mut() {
                array.push(
                    json!({ "name": "Content-Type", "value": "application/json; charset=utf-8" }),
                );
            }
        }
        let body = B64.encode(rule.body.as_bytes());
        if let Err(e) = instance
            .client
            .call(
                "Fetch.fulfillRequest",
                json!({
                    "requestId": request_id.clone(),
                    "responseCode": rule.status_code,
                    "responseHeaders": headers,
                    "body": body
                }),
            )
            .await
        {
            tracing::warn!(error = %e, url = %url, "mock fulfill failed, continuing request");
            instance
                .client
                .call("Fetch.continueRequest", json!({ "requestId": request_id }))
                .await?;
        }
    } else {
        instance
            .client
            .call("Fetch.continueRequest", json!({ "requestId": request_id }))
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod launch_args_tests {
    use super::merge_launch_args;

    #[test]
    fn environment_switch_replaces_global_switch() {
        let global = vec!["--disable-gpu".to_string(), "--lang=en-US".to_string()];
        let environment = vec![
            "--disable-gpu=false".to_string(),
            "--autoplay-policy=no-user-gesture-required".to_string(),
        ];
        let merged = merge_launch_args(&global, &environment);
        assert!(merged.contains(&"--lang=en-US".to_string()));
        assert!(merged.contains(&"--disable-gpu=false".to_string()));
        assert!(!merged.contains(&"--disable-gpu".to_string()));
        assert_eq!(
            merged
                .iter()
                .filter(|arg| arg.starts_with("--disable-gpu"))
                .count(),
            1
        );
        assert_eq!(
            merge_launch_args(&["--start-maximized".to_string()], &[]),
            vec!["--start-maximized".to_string()]
        );
    }

    #[test]
    fn environment_window_size_overrides_global_maximize() {
        let merged = merge_launch_args(
            &["--start-maximized".to_string(), "--lang=en-US".to_string()],
            &["--window-size=800,600".to_string()],
        );
        assert_eq!(
            merged,
            vec![
                "--lang=en-US".to_string(),
                "--window-size=800,600".to_string()
            ]
        );
    }

    #[test]
    fn environment_maximize_overrides_global_window_size() {
        let merged = merge_launch_args(
            &[
                "--window-size=1280,720".to_string(),
                "--lang=en-US".to_string(),
            ],
            &["--start-maximized".to_string()],
        );
        assert_eq!(
            merged,
            vec!["--lang=en-US".to_string(), "--start-maximized".to_string()]
        );
    }
}
