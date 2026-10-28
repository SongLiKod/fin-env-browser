use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use serde_json::{json, Value};
use tokio::{sync::mpsc, task::JoinHandle, time::sleep};

use crate::{
    audit_log::{actions, Audit},
    cdp::{CdpClient, CdpEvent},
    error::{AppError, Result},
    models::{new_id, now_iso, BroadcastStartInput, BroadcastStatus, OpenOptions},
};

use super::BrowserManager;

const BINDING: &str = "__finEnvBroadcast";
const MAX_ENVIRONMENTS: usize = 30;
const MAX_PAYLOAD: usize = 32 * 1024;
const NEW_TAB_GRACE_MS: u64 = 550;

#[derive(Default)]
struct BroadcastTabs {
    /// "envId|targetId" -> flat CDP session id
    sessions: HashMap<String, String>,
    /// "envId|sessionId" -> (envId, targetId)
    session_targets: HashMap<String, (String, String)>,
    /// master targetId -> tab metadata
    masters: HashMap<String, MasterTab>,
    /// master targetId -> envId -> slave targetId
    mapped: HashMap<String, HashMap<String, String>>,
    /// page targets present at startup, or already handled
    known: HashSet<String>,
    /// "envId|targetId" already paired/mapped
    paired: HashSet<String>,
    /// "envId|targetId" -> URL
    target_urls: HashMap<String, String>,
    /// "envId|targetId" -> (width, height)
    viewports: HashMap<String, (f64, f64)>,
    active_master: Option<String>,
}

#[derive(Clone)]
struct MasterTab {
    url: String,
    script_id: Option<String>,
}

pub struct BroadcastSession {
    pub id: String,
    pub master_env_id: String,
    pub env_ids: Vec<String>,
    pub started_at: String,
    pub paused: AtomicBool,
    pub forwarded: AtomicU64,
    stop: AtomicBool,
    browsers: HashMap<String, Arc<CdpClient>>,
    tabs: Mutex<BroadcastTabs>,
    input_tx: mpsc::UnboundedSender<BroadcastInput>,
    pub errors: Vec<String>,
}

struct BroadcastInput {
    master_target: String,
    value: Value,
}

pub struct BroadcastRuntime {
    pub session: Arc<BroadcastSession>,
    worker: Mutex<Option<JoinHandle<()>>>,
    listeners: Mutex<Vec<JoinHandle<()>>>,
    monitor: Mutex<Option<JoinHandle<()>>>,
}

impl BroadcastRuntime {
    fn stop_tasks(&self) {
        if let Ok(mut guard) = self.worker.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
        if let Ok(mut guard) = self.monitor.lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
        if let Ok(mut guard) = self.listeners.lock() {
            for handle in guard.drain(..) {
                handle.abort();
            }
        }
    }
}

const VIEWPORT_EXPR: &str = "({ w: Math.max(1, document.documentElement?.clientWidth || window.innerWidth || 1), h: Math.max(1, document.documentElement?.clientHeight || window.innerHeight || 1) })";

const CAPTURE_SCRIPT: &str = r#"
(() => {
  if (window.top !== window) return;
  if (window.__finEnvBroadcastInstalled) return;
  const binding = window.__finEnvBroadcast;
  if (typeof binding !== 'function') return;
  window.__finEnvBroadcastInstalled = true;
  try { delete window.__finEnvBroadcast; } catch (_) {}
  const send = (payload) => {
    try { binding(JSON.stringify(payload)); } catch (_) {}
  };
  const viewport = () => ({
    vw: Math.max(1, document.documentElement?.clientWidth || window.innerWidth || 1),
    vh: Math.max(1, document.documentElement?.clientHeight || window.innerHeight || 1),
  });
  const modifiers = (e) =>
    (e.altKey ? 1 : 0) |
    (e.ctrlKey ? 2 : 0) |
    (e.metaKey ? 4 : 0) |
    (e.shiftKey ? 8 : 0);
  const mouse = (type, e, extra = {}) => {
    if (!e.isTrusted) return;
    const v = viewport();
    send({
      kind: 'mouse',
      type,
      x: e.clientX,
      y: e.clientY,
      vw: v.vw,
      vh: v.vh,
      button: e.button,
      buttons: e.buttons || 0,
      clickCount: e.detail || 1,
      modifiers: modifiers(e),
      ...extra,
    });
  };
  const listeners = [];
  const on = (target, name, fn, opts) => {
    target.addEventListener(name, fn, opts);
    listeners.push([target, name, fn, opts]);
  };
  let lastMove = 0;
  on(window, 'mousemove', (e) => {
    const now = performance.now();
    if (now - lastMove < 40) return;
    lastMove = now;
    mouse('mousemove', e);
  }, { capture: true, passive: true });
  on(window, 'mousedown', (e) => mouse('mousedown', e), true);
  on(window, 'mouseup', (e) => mouse('mouseup', e), true);
  let lastScroll = 0;
  on(window, 'scroll', () => {
    const now = performance.now();
    if (now - lastScroll < 50) return;
    lastScroll = now;
    send({ kind: 'scroll', x: window.scrollX || 0, y: window.scrollY || 0 });
  }, { capture: true, passive: true });
  on(window, 'wheel', (e) => {
    if (!e.isTrusted) return;
    const v = viewport();
    send({
      kind: 'mouse',
      type: 'wheel',
      x: e.clientX,
      y: e.clientY,
      vw: v.vw,
      vh: v.vh,
      deltaX: e.deltaX,
      deltaY: e.deltaY,
      deltaMode: e.deltaMode,
      modifiers: modifiers(e),
    });
  }, { capture: true, passive: true });
  const key = (type, e) => {
    if (!e.isTrusted) return;
    if (e.isComposing || e.keyCode === 229) return;
    if (type === 'keydown' && (e.ctrlKey || e.metaKey) && (e.key === 'v' || e.key === 'V')) return;
    const printable =
      type === 'keydown' &&
      e.key.length === 1 &&
      !e.ctrlKey &&
      !e.metaKey &&
      !e.altKey;
    send({
      kind: 'key',
      type,
      key: e.key,
      code: e.code,
      keyCode: e.keyCode,
      text: printable ? e.key : null,
      modifiers: modifiers(e),
      autoRepeat: e.repeat,
    });
  };
  on(window, 'keydown', (e) => key('keydown', e), true);
  on(window, 'keyup', (e) => key('keyup', e), true);
  on(window, 'paste', (e) => {
    if (!e.isTrusted) return;
    const text = e.clipboardData?.getData('text/plain');
    if (text) send({ kind: 'text', text });
  }, true);
  on(window, 'compositionend', (e) => {
    if (!e.isTrusted) return;
    if (e.data) send({ kind: 'text', text: e.data });
  }, true);
  window.__finEnvBroadcastCleanup = () => {
    for (const [target, name, fn, opts] of listeners) {
      try { target.removeEventListener(name, fn, opts); } catch (_) {}
    }
    window.__finEnvBroadcastInstalled = false;
    try { delete window.__finEnvBroadcastCleanup; } catch (_) {}
  };
})();
"#;

fn target_key(env_id: &str, target_id: &str) -> String {
    format!("{env_id}|{target_id}")
}

fn session_key(env_id: &str, session_id: &str) -> String {
    format!("{env_id}|{session_id}")
}

fn target_id_from_ws(ws: &str) -> Option<String> {
    ws.rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn syncable_url(url: &str) -> bool {
    url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("file://")
        || url == "about:blank"
}

fn blank_url(url: &str) -> bool {
    url.is_empty() || url == "about:blank" || url.starts_with("chrome://newtab")
}

fn create_url(url: &str) -> &str {
    if syncable_url(url) {
        url
    } else {
        "about:blank"
    }
}

async fn browser_ws(debug_port: u16) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(AppError::from)?;
    let value: Value = client
        .get(format!("http://127.0.0.1:{debug_port}/json/version"))
        .send()
        .await?
        .json()
        .await?;
    value
        .get("webSocketDebuggerUrl")
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
        .ok_or_else(|| AppError::internal("浏览器未返回 CDP WebSocket 地址"))
}

async fn page_targets(client: &CdpClient) -> Result<Vec<Value>> {
    let value = client.call("Target.getTargets", json!({})).await?;
    Ok(value
        .get("targetInfos")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|info| info.get("type").and_then(|value| value.as_str()) == Some("page"))
        .collect())
}

async fn attach_target(
    client: &CdpClient,
    target_id: &str,
    focus_emulation: bool,
) -> Result<(String, (f64, f64))> {
    let attached = client
        .call(
            "Target.attachToTarget",
            json!({ "targetId": target_id, "flatten": true }),
        )
        .await?;
    let session_id = attached
        .get("sessionId")
        .and_then(|value| value.as_str())
        .ok_or_else(|| AppError::internal("无法附加浏览器 Tab"))?
        .to_string();
    let _ = client
        .call_on("Runtime.enable", json!({}), Some(&session_id))
        .await;
    let _ = client
        .call_on("Page.enable", json!({}), Some(&session_id))
        .await;
    if focus_emulation {
        let _ = client
            .call_on(
                "Emulation.setFocusEmulationEnabled",
                json!({ "enabled": true }),
                Some(&session_id),
            )
            .await;
    }
    let viewport = client
        .evaluate_on(VIEWPORT_EXPR, Some(&session_id))
        .await
        .ok()
        .and_then(viewport_from_value)
        .unwrap_or((1.0, 1.0));
    Ok((session_id, viewport))
}

fn viewport_from_value(value: Value) -> Option<(f64, f64)> {
    let width = value
        .get("w")
        .and_then(|value| value.as_f64())
        .filter(|value| *value > 0.0)?;
    let height = value
        .get("h")
        .and_then(|value| value.as_f64())
        .filter(|value| *value > 0.0)?;
    Some((width, height))
}

async fn install_capture(client: &CdpClient, session_id: &str) -> Result<Option<String>> {
    let _ = client
        .call_on(
            "Runtime.addBinding",
            json!({ "name": BINDING }),
            Some(session_id),
        )
        .await;
    let script = client
        .call_on(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": CAPTURE_SCRIPT }),
            Some(session_id),
        )
        .await?;
    let script_id = script
        .get("identifier")
        .and_then(|value| value.as_str())
        .map(ToString::to_string);
    client.evaluate_on(CAPTURE_SCRIPT, Some(session_id)).await?;
    let installed = client
        .evaluate_on(
            "window.__finEnvBroadcastInstalled === true",
            Some(session_id),
        )
        .await?;
    if installed.as_bool() != Some(true) {
        return Err(AppError::internal("无法安装 Tab 同步输入捕获脚本"));
    }
    Ok(script_id)
}

async fn reinstall_capture(client: &CdpClient, session_id: &str) {
    let _ = client
        .call_on(
            "Runtime.addBinding",
            json!({ "name": BINDING }),
            Some(session_id),
        )
        .await;
    let _ = client.evaluate_on(CAPTURE_SCRIPT, Some(session_id)).await;
}

impl BrowserManager {
    pub async fn broadcast_start(
        self: &Arc<Self>,
        input: BroadcastStartInput,
        actor: &str,
    ) -> Result<BroadcastStatus> {
        if self.broadcast.read().await.is_some() {
            return Err(AppError::conflict("已有同步控制会话，请先停止"));
        }
        let mut seen = HashSet::new();
        let mut requested = Vec::new();
        for id in input.env_ids {
            if seen.insert(id.clone()) {
                requested.push(id);
            }
        }
        if requested.len() < 2 {
            return Err(AppError::bad_request("同步控制至少需要选择 2 个环境"));
        }
        if requested.len() > MAX_ENVIRONMENTS {
            return Err(AppError::bad_request(format!(
                "同步控制最多支持 {MAX_ENVIRONMENTS} 个环境"
            )));
        }

        let url = input
            .url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(url) = &url {
            if !syncable_url(url) {
                return Err(AppError::bad_request(
                    "同步访问仅支持 http/https/file/about:blank URL",
                ));
            }
        }
        let open_missing = input.open_missing.unwrap_or(true);
        let mut opened = Vec::new();
        let mut errors = Vec::new();
        let mut names = HashMap::new();

        for env_id in &requested {
            let env = match self.db.get_environment(env_id) {
                Ok(env) => env,
                Err(error) => {
                    errors.push(format!("{env_id}: {}", error.message()));
                    continue;
                }
            };
            names.insert(env.id.clone(), env.name.clone());
            if env.status == "disabled" {
                errors.push(format!("{}: 环境已停用", env.name));
                continue;
            }
            let running = self.runtime_of(env_id).is_some();
            if !running {
                if !open_missing {
                    errors.push(format!("{}: 未运行", env.name));
                    continue;
                }
                let options = OpenOptions {
                    url: url.clone(),
                    ..Default::default()
                };
                if let Err(error) = self.open(&env, &options, actor).await {
                    errors.push(format!("{}: {}", env.name, error.message()));
                    continue;
                }
            } else if let Some(url) = &url {
                if let Err(error) = self.navigate(env_id, url).await {
                    errors.push(format!("{}: {}", env.name, error.message()));
                    continue;
                }
            }
            if self
                .instances
                .read()
                .ok()
                .and_then(|map| map.get(env_id).cloned())
                .is_none()
            {
                errors.push(format!("{}: 浏览器实例不可用", env.name));
                continue;
            }
            opened.push(env_id.clone());
        }

        if opened.len() < 2 {
            return Err(AppError::bad_request(format!(
                "可用于同步控制的环境不足 2 个：{}",
                errors.join("；")
            )));
        }

        let master_env_id = match input.master_env_id {
            Some(master) if opened.contains(&master) => master,
            Some(_) => return Err(AppError::bad_request("指定的主控环境未成功启动")),
            None => opened[0].clone(),
        };

        let mut browsers: HashMap<String, Arc<CdpClient>> = HashMap::new();
        let mut initial_targets: HashMap<String, String> = HashMap::new();
        let mut tabs = BroadcastTabs::default();
        let mut ready = Vec::new();

        for env_id in &opened {
            let instance = match self.instance(env_id) {
                Ok(instance) => instance,
                Err(error) => {
                    errors.push(format!("{env_id}: {}", error.message()));
                    continue;
                }
            };
            let browser = match browser_ws(instance.debug_port).await {
                Ok(ws) => match CdpClient::connect(&ws).await {
                    Ok(client) => Arc::new(client),
                    Err(error) => {
                        errors.push(format!("{env_id}: {}", error.message()));
                        continue;
                    }
                },
                Err(error) => {
                    errors.push(format!("{env_id}: {}", error.message()));
                    continue;
                }
            };
            if let Err(error) = browser
                .call("Target.setDiscoverTargets", json!({ "discover": true }))
                .await
            {
                errors.push(format!("{env_id}: {}", error.message()));
                continue;
            }
            let targets = page_targets(&browser).await.unwrap_or_default();
            for info in &targets {
                if let Some(target_id) = info.get("targetId").and_then(|value| value.as_str()) {
                    tabs.known.insert(target_key(env_id, target_id));
                    if let Some(url) = info.get("url").and_then(|value| value.as_str()) {
                        tabs.target_urls
                            .insert(target_key(env_id, target_id), url.to_string());
                    }
                }
            }
            let initial = target_id_from_ws(&instance.page_ws).or_else(|| {
                targets
                    .first()
                    .and_then(|info| info.get("targetId"))
                    .and_then(|value| value.as_str())
                    .map(ToString::to_string)
            });
            let Some(initial) = initial else {
                errors.push(format!("{env_id}: 找不到可同步的页面 Tab"));
                continue;
            };
            tabs.known.insert(target_key(env_id, &initial));
            initial_targets.insert(env_id.clone(), initial);
            browsers.insert(env_id.clone(), browser);
            ready.push(env_id.clone());
        }

        if ready.len() < 2 {
            return Err(AppError::bad_request(format!(
                "可用于同步控制的浏览器 Tab 不足 2 个：{}",
                errors.join("；")
            )));
        }
        if !ready.contains(&master_env_id) {
            return Err(AppError::bad_request("主控环境的浏览器连接失败"));
        }

        for env_id in &ready {
            let browser = browsers.get(env_id).cloned().expect("browser exists");
            let target_id = initial_targets.get(env_id).cloned().expect("target exists");
            let (session_id, viewport) =
                match attach_target(&browser, &target_id, env_id != &master_env_id).await {
                    Ok(value) => value,
                    Err(error) => {
                        errors.push(format!("{env_id}: {}", error.message()));
                        continue;
                    }
                };
            let key = target_key(env_id, &target_id);
            tabs.sessions.insert(key.clone(), session_id.clone());
            tabs.session_targets.insert(
                session_key(env_id, &session_id),
                (env_id.clone(), target_id.clone()),
            );
            tabs.viewports.insert(key.clone(), viewport);
            tabs.paired.insert(key);
        }
        if tabs.sessions.len() < 2 {
            return Err(AppError::bad_request("无法附加足够的浏览器 Tab 用于同步"));
        }

        let master_target = initial_targets
            .get(&master_env_id)
            .cloned()
            .ok_or_else(|| AppError::bad_request("主控 Tab 不可用"))?;
        let master_session = tabs
            .sessions
            .get(&target_key(&master_env_id, &master_target))
            .cloned()
            .ok_or_else(|| AppError::bad_request("主控 Tab 会话不可用"))?;
        let master_browser = browsers
            .get(&master_env_id)
            .cloned()
            .ok_or_else(|| AppError::bad_request("主控浏览器连接不可用"))?;
        let script_id = install_capture(&master_browser, &master_session).await?;
        let _ = master_browser
            .call_on("Page.bringToFront", json!({}), Some(&master_session))
            .await;
        let master_url = tabs
            .target_urls
            .get(&target_key(&master_env_id, &master_target))
            .cloned()
            .unwrap_or_else(|| "about:blank".into());
        tabs.masters.insert(
            master_target.clone(),
            MasterTab {
                url: master_url,
                script_id,
            },
        );
        tabs.active_master = Some(master_target.clone());

        let mut mapping = HashMap::new();
        for env_id in &ready {
            if env_id == &master_env_id {
                continue;
            }
            if let Some(target_id) = initial_targets.get(env_id) {
                mapping.insert(env_id.clone(), target_id.clone());
            }
        }
        tabs.mapped.insert(master_target.clone(), mapping);

        let (input_tx, mut input_rx) = mpsc::unbounded_channel::<BroadcastInput>();
        let session = Arc::new(BroadcastSession {
            id: new_id(),
            master_env_id: master_env_id.clone(),
            env_ids: ready.clone(),
            started_at: now_iso(),
            paused: AtomicBool::new(false),
            forwarded: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            browsers: browsers.clone(),
            tabs: Mutex::new(tabs),
            input_tx,
            errors: errors.clone(),
        });

        let worker_session = session.clone();
        let worker = tokio::spawn(async move {
            while let Some(input) = input_rx.recv().await {
                if worker_session.paused.load(Ordering::SeqCst) {
                    continue;
                }
                forward_input(&worker_session, &input.master_target, &input.value).await;
            }
        });

        let mut listeners = Vec::new();
        for (env_id, browser) in &browsers {
            let mut rx = browser.subscribe();
            let session = session.clone();
            let env_id = env_id.clone();
            listeners.push(tokio::spawn(async move {
                loop {
                    match rx.recv().await {
                        Ok(event) => {
                            if session.stop.load(Ordering::SeqCst) {
                                break;
                            }
                            handle_browser_event(&session, &env_id, event).await;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(skipped, env_id = %env_id, "broadcast CDP events lagged");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }));
        }

        let monitor_session = session.clone();
        let monitor = tokio::spawn(async move {
            loop {
                sleep(Duration::from_millis(500)).await;
                if monitor_session.stop.load(Ordering::SeqCst) {
                    break;
                }
                monitor_active_tab(&monitor_session).await;
            }
        });

        let runtime = Arc::new(BroadcastRuntime {
            session: session.clone(),
            worker: Mutex::new(Some(worker)),
            listeners: Mutex::new(listeners),
            monitor: Mutex::new(Some(monitor)),
        });
        *self.broadcast.write().await = Some(runtime);

        let names_list = ready
            .iter()
            .map(|id| names.get(id).cloned().unwrap_or_else(|| id.clone()))
            .collect::<Vec<_>>();
        Audit::global(
            &self.db,
            actor,
            actions::BROADCAST_START,
            format!(
                "开启同步控制：主控 {}，参与 {} 个环境（{}），多 Tab 同步已启用",
                names
                    .get(&master_env_id)
                    .cloned()
                    .unwrap_or_else(|| master_env_id.clone()),
                ready.len(),
                names_list.join("、")
            ),
        )?;

        let status = self.broadcast_status().await;
        self.emit("broadcast://status", status.clone());
        Ok(status)
    }

    pub async fn broadcast_stop(
        self: &Arc<Self>,
        actor: &str,
        reason: &str,
    ) -> Result<BroadcastStatus> {
        let runtime = self.broadcast.write().await.take();
        let Some(runtime) = runtime else {
            return Ok(BroadcastStatus {
                active: false,
                id: None,
                master_env_id: None,
                env_ids: Vec::new(),
                paused: false,
                forwarded: 0,
                started_at: None,
                message: "当前没有正在运行的同步控制会话".into(),
                errors: Vec::new(),
            });
        };
        runtime.session.stop.store(true, Ordering::SeqCst);
        runtime.session.paused.store(true, Ordering::SeqCst);
        runtime.stop_tasks();

        let (sessions, masters) = {
            let tabs = match runtime.session.tabs.lock() {
                Ok(tabs) => tabs,
                Err(_) => {
                    return Err(AppError::internal("同步 Tab 状态锁失败"));
                }
            };
            (tabs.sessions.clone(), tabs.masters.clone())
        };

        for (env_id, browser) in &runtime.session.browsers {
            if env_id != &runtime.session.master_env_id {
                continue;
            }
            for (target_id, master) in &masters {
                if let Some(session_id) = sessions.get(&target_key(env_id, target_id)) {
                    let _ = browser
                        .evaluate_on(
                            "window.__finEnvBroadcastCleanup && window.__finEnvBroadcastCleanup()",
                            Some(session_id),
                        )
                        .await;
                    if let Some(script_id) = &master.script_id {
                        let _ = browser
                            .call_on(
                                "Page.removeScriptToEvaluateOnNewDocument",
                                json!({ "identifier": script_id }),
                                Some(session_id),
                            )
                            .await;
                    }
                    let _ = browser
                        .call_on(
                            "Runtime.removeBinding",
                            json!({ "name": BINDING }),
                            Some(session_id),
                        )
                        .await;
                }
            }
        }
        for (env_id, browser) in &runtime.session.browsers {
            if env_id == &runtime.session.master_env_id {
                continue;
            }
            for (key, session_id) in &sessions {
                if key.starts_with(&format!("{env_id}|")) {
                    let _ = browser
                        .call_on(
                            "Emulation.setFocusEmulationEnabled",
                            json!({ "enabled": false }),
                            Some(session_id),
                        )
                        .await;
                }
            }
        }

        Audit::global(
            &self.db,
            actor,
            actions::BROADCAST_STOP,
            format!(
                "停止同步控制：主控 {}，参与 {} 个环境（{}），累计转发 {} 次",
                runtime.session.master_env_id,
                runtime.session.env_ids.len(),
                reason,
                runtime.session.forwarded.load(Ordering::SeqCst)
            ),
        )?;

        let status = BroadcastStatus {
            active: false,
            id: None,
            master_env_id: None,
            env_ids: Vec::new(),
            paused: false,
            forwarded: 0,
            started_at: None,
            message: format!("同步控制已停止：{reason}"),
            errors: Vec::new(),
        };
        self.emit("broadcast://status", status.clone());
        Ok(status)
    }

    pub async fn broadcast_pause(
        self: &Arc<Self>,
        paused: bool,
        actor: &str,
    ) -> Result<BroadcastStatus> {
        let runtime = self.broadcast.read().await.clone();
        let Some(runtime) = runtime else {
            return Err(AppError::bad_request("当前没有正在运行的同步控制会话"));
        };
        runtime.session.paused.store(paused, Ordering::SeqCst);
        Audit::global(
            &self.db,
            actor,
            if paused {
                actions::BROADCAST_PAUSE
            } else {
                actions::BROADCAST_RESUME
            },
            format!(
                "{}同步控制：主控 {}",
                if paused { "暂停" } else { "恢复" },
                runtime.session.master_env_id
            ),
        )?;
        let status = self.broadcast_status().await;
        self.emit("broadcast://status", status.clone());
        Ok(status)
    }

    pub async fn broadcast_status(&self) -> BroadcastStatus {
        let runtime = self.broadcast.read().await.clone();
        match runtime {
            Some(runtime) => {
                let session = &runtime.session;
                BroadcastStatus {
                    active: true,
                    id: Some(session.id.clone()),
                    master_env_id: Some(session.master_env_id.clone()),
                    env_ids: session.env_ids.clone(),
                    paused: session.paused.load(Ordering::SeqCst),
                    forwarded: session.forwarded.load(Ordering::SeqCst),
                    started_at: Some(session.started_at.clone()),
                    message: if session.paused.load(Ordering::SeqCst) {
                        "同步控制已暂停".into()
                    } else {
                        "同步控制中".into()
                    },
                    errors: session.errors.clone(),
                }
            }
            None => BroadcastStatus {
                active: false,
                id: None,
                master_env_id: None,
                env_ids: Vec::new(),
                paused: false,
                forwarded: 0,
                started_at: None,
                message: "未开启同步控制".into(),
                errors: Vec::new(),
            },
        }
    }

    pub(crate) async fn broadcast_on_env_closed(self: &Arc<Self>, env_id: &str, actor: &str) {
        let active = self.broadcast.read().await.clone();
        if let Some(runtime) = active {
            if runtime.session.env_ids.iter().any(|id| id == env_id) {
                let _ = self.broadcast_stop(actor, "参与环境已关闭").await;
            }
        }
    }
}

async fn handle_browser_event(session: &Arc<BroadcastSession>, env_id: &str, event: CdpEvent) {
    match event.method.as_str() {
        "Target.targetCreated" => {
            if let Some(info) = event.params.get("targetInfo") {
                if info.get("type").and_then(|value| value.as_str()) == Some("page") {
                    handle_target_created(session, env_id, info).await;
                }
            }
        }
        "Target.targetDestroyed" => {
            if let Some(target_id) = event
                .params
                .get("targetId")
                .and_then(|value| value.as_str())
            {
                handle_target_destroyed(session, env_id, target_id).await;
            }
        }
        "Target.targetInfoChanged" => {
            if let Some(info) = event.params.get("targetInfo") {
                if info.get("type").and_then(|value| value.as_str()) == Some("page") {
                    handle_target_info_changed(session, env_id, info).await;
                }
            }
        }
        "Target.targetActivated" => {
            if let Some(info) = event.params.get("targetInfo") {
                if info.get("type").and_then(|value| value.as_str()) == Some("page")
                    && env_id == session.master_env_id
                {
                    if let Some(target_id) = info.get("targetId").and_then(|value| value.as_str()) {
                        set_active_master(session, target_id).await;
                    }
                }
            }
        }
        "Runtime.bindingCalled" => {
            if env_id != session.master_env_id
                || event.params.get("name").and_then(|value| value.as_str()) != Some(BINDING)
            {
                return;
            }
            let Some(session_id) = event.session_id.as_deref() else {
                return;
            };
            let target = {
                let tabs = match session.tabs.lock() {
                    Ok(tabs) => tabs,
                    Err(_) => return,
                };
                tabs.session_targets
                    .get(&session_key(env_id, session_id))
                    .map(|(_, target)| target.clone())
            };
            let Some(target) = target else {
                return;
            };
            let payload = event
                .params
                .get("payload")
                .and_then(|value| value.as_str())
                .unwrap_or("{}");
            if payload.len() > MAX_PAYLOAD {
                return;
            }
            let Ok(value) = serde_json::from_str::<Value>(payload) else {
                return;
            };
            if !value
                .get("kind")
                .and_then(|kind| kind.as_str())
                .map(|kind| matches!(kind, "mouse" | "key" | "text" | "scroll"))
                .unwrap_or(false)
            {
                return;
            }
            let _ = session.input_tx.send(BroadcastInput {
                master_target: target,
                value,
            });
        }
        "Page.loadEventFired" => {
            if env_id != session.master_env_id {
                return;
            }
            let Some(session_id) = event.session_id.as_deref() else {
                return;
            };
            let target = {
                let tabs = match session.tabs.lock() {
                    Ok(tabs) => tabs,
                    Err(_) => return,
                };
                tabs.session_targets
                    .get(&session_key(env_id, session_id))
                    .map(|(_, target)| target.clone())
            };
            if let Some(target) = target {
                reinstall_master_capture(session, &target).await;
            }
        }
        _ => {}
    }
}

async fn handle_target_created(session: &Arc<BroadcastSession>, env_id: &str, info: &Value) {
    let Some(target_id) = info.get("targetId").and_then(|value| value.as_str()) else {
        return;
    };
    let url = info
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or("about:blank")
        .to_string();
    let key = target_key(env_id, target_id);
    {
        let Ok(mut tabs) = session.tabs.lock() else {
            return;
        };
        if tabs.known.contains(&key) {
            return;
        }
        tabs.known.insert(key.clone());
        tabs.target_urls.insert(key, url.clone());
    }
    if env_id != session.master_env_id {
        let session_clone = session.clone();
        let env_owned = env_id.to_string();
        let target_owned = target_id.to_string();
        tokio::spawn(async move {
            sleep(Duration::from_millis(2500)).await;
            if session_clone.stop.load(Ordering::SeqCst) {
                return;
            }
            let key = target_key(&env_owned, &target_owned);
            let keep = session_clone
                .tabs
                .lock()
                .map(|tabs| tabs.paired.contains(&key))
                .unwrap_or(true);
            if !keep {
                if let Some(browser) = session_clone.browsers.get(&env_owned) {
                    let _ = browser
                        .call("Target.closeTarget", json!({ "targetId": target_owned }))
                        .await;
                }
            }
        });
        return;
    }
    let Some(browser) = session.browsers.get(env_id).cloned() else {
        return;
    };
    let (session_id, viewport) = match attach_target(&browser, target_id, false).await {
        Ok(value) => value,
        Err(error) => {
            tracing::warn!(env_id, target_id, error = %error, "broadcast attach new tab failed");
            return;
        }
    };
    let key = target_key(env_id, target_id);
    {
        let Ok(mut tabs) = session.tabs.lock() else {
            return;
        };
        tabs.sessions.insert(key.clone(), session_id.clone());
        tabs.session_targets.insert(
            session_key(env_id, &session_id),
            (env_id.to_string(), target_id.to_string()),
        );
        tabs.viewports.insert(key, viewport);
        tabs.masters.insert(
            target_id.to_string(),
            MasterTab {
                url: url.clone(),
                script_id: None,
            },
        );
        tabs.mapped.entry(target_id.to_string()).or_default();
    }
    if syncable_url(&url) {
        match install_capture(&browser, &session_id).await {
            Ok(script_id) => {
                if let Ok(mut tabs) = session.tabs.lock() {
                    if let Some(master) = tabs.masters.get_mut(target_id) {
                        master.script_id = script_id;
                    }
                }
            }
            Err(error) => {
                tracing::warn!(env_id, target_id, error = %error, "broadcast install new tab capture failed");
            }
        }
    }
    let session_clone = session.clone();
    let target_clone = target_id.to_string();
    tokio::spawn(async move {
        sleep(Duration::from_millis(NEW_TAB_GRACE_MS)).await;
        pair_master_tab(&session_clone, &target_clone).await;
    });
}

async fn handle_target_destroyed(session: &Arc<BroadcastSession>, env_id: &str, target_id: &str) {
    let key = target_key(env_id, target_id);
    let mapping = {
        let Ok(mut tabs) = session.tabs.lock() else {
            return;
        };
        tabs.known.remove(&key);
        tabs.paired.remove(&key);
        tabs.target_urls.remove(&key);
        tabs.viewports.remove(&key);
        if let Some(session_id) = tabs.sessions.remove(&key) {
            tabs.session_targets
                .remove(&session_key(env_id, &session_id));
        }
        if env_id == session.master_env_id {
            let mapping = tabs.mapped.remove(target_id).unwrap_or_default();
            tabs.masters.remove(target_id);
            if tabs.active_master.as_deref() == Some(target_id) {
                tabs.active_master = None;
            }
            Some(mapping)
        } else {
            for mapping in tabs.mapped.values_mut() {
                if mapping.get(env_id).map(String::as_str) == Some(target_id) {
                    mapping.remove(env_id);
                }
            }
            None
        }
    };
    if let Some(mapping) = mapping {
        for (slave_env, slave_target) in mapping {
            if let Some(browser) = session.browsers.get(&slave_env) {
                let _ = browser
                    .call("Target.closeTarget", json!({ "targetId": slave_target }))
                    .await;
            }
        }
    }
}

async fn handle_target_info_changed(session: &Arc<BroadcastSession>, env_id: &str, info: &Value) {
    let Some(target_id) = info.get("targetId").and_then(|value| value.as_str()) else {
        return;
    };
    let url = info
        .get("url")
        .and_then(|value| value.as_str())
        .unwrap_or("about:blank");
    let key = target_key(env_id, target_id);
    {
        let Ok(mut tabs) = session.tabs.lock() else {
            return;
        };
        tabs.target_urls.insert(key, url.to_string());
    }
    if env_id != session.master_env_id {
        return;
    }
    let (changed, mapping) = {
        let Ok(mut tabs) = session.tabs.lock() else {
            return;
        };
        let changed = if let Some(master) = tabs.masters.get_mut(target_id) {
            if master.url == url {
                false
            } else {
                master.url = url.to_string();
                true
            }
        } else {
            false
        };
        (
            changed,
            tabs.mapped.get(target_id).cloned().unwrap_or_default(),
        )
    };
    if !changed || !syncable_url(url) {
        return;
    }
    let needs_capture = {
        let tabs = match session.tabs.lock() {
            Ok(tabs) => tabs,
            Err(_) => return,
        };
        tabs.masters
            .get(target_id)
            .map(|master| master.script_id.is_none())
            .unwrap_or(false)
            && tabs
                .sessions
                .contains_key(&target_key(&session.master_env_id, target_id))
    };
    if needs_capture {
        let data = {
            let tabs = match session.tabs.lock() {
                Ok(tabs) => tabs,
                Err(_) => return,
            };
            tabs.sessions
                .get(&target_key(&session.master_env_id, target_id))
                .cloned()
                .and_then(|session_id| {
                    session
                        .browsers
                        .get(&session.master_env_id)
                        .cloned()
                        .map(|browser| (browser, session_id))
                })
        };
        if let Some((browser, session_id)) = data {
            match install_capture(&browser, &session_id).await {
                Ok(script_id) => {
                    if let Ok(mut tabs) = session.tabs.lock() {
                        if let Some(master) = tabs.masters.get_mut(target_id) {
                            master.script_id = script_id;
                        }
                    }
                }
                Err(error) => {
                    tracing::warn!(target_id, error = %error, "broadcast install capture after URL change failed");
                }
            }
        }
    }
    for (slave_env, slave_target) in mapping {
        let Some(browser) = session.browsers.get(&slave_env).cloned() else {
            continue;
        };
        let key = target_key(&slave_env, &slave_target);
        let existing = {
            let tabs = match session.tabs.lock() {
                Ok(tabs) => tabs,
                Err(_) => continue,
            };
            tabs.target_urls.get(&key).cloned()
        };
        if existing.as_deref() == Some(url) {
            continue;
        }
        let session_id = {
            let tabs = match session.tabs.lock() {
                Ok(tabs) => tabs,
                Err(_) => continue,
            };
            tabs.sessions.get(&key).cloned()
        };
        if let Some(session_id) = session_id {
            let _ = browser
                .call_on("Page.navigate", json!({ "url": url }), Some(&session_id))
                .await;
            if let Ok(mut tabs) = session.tabs.lock() {
                tabs.target_urls.insert(key, url.to_string());
            }
        }
    }
}

async fn pair_master_tab(session: &Arc<BroadcastSession>, master_target: &str) {
    if session.stop.load(Ordering::SeqCst) {
        return;
    }
    let (url, already) = {
        let Ok(tabs) = session.tabs.lock() else {
            return;
        };
        let Some(master) = tabs.masters.get(master_target) else {
            return;
        };
        (
            master.url.clone(),
            tabs.mapped.get(master_target).cloned().unwrap_or_default(),
        )
    };
    let mut url = url;
    for _ in 0..8 {
        if !url.is_empty() {
            break;
        }
        sleep(Duration::from_millis(150)).await;
        url = session
            .tabs
            .lock()
            .ok()
            .and_then(|tabs| {
                tabs.masters
                    .get(master_target)
                    .map(|master| master.url.clone())
            })
            .unwrap_or_default();
    }
    let blank = blank_url(&url);
    for env_id in &session.env_ids {
        if env_id == &session.master_env_id || already.contains_key(env_id) {
            continue;
        }
        let Some(browser) = session.browsers.get(env_id).cloned() else {
            continue;
        };
        let targets = page_targets(&browser).await.unwrap_or_default();
        let candidate = targets.into_iter().find(|info| {
            let Some(target_id) = info.get("targetId").and_then(|value| value.as_str()) else {
                return false;
            };
            let target_url = info
                .get("url")
                .and_then(|value| value.as_str())
                .unwrap_or("about:blank");
            let key = target_key(env_id, target_id);
            if already.values().any(|value| value == target_id) {
                return false;
            }
            let paired = session
                .tabs
                .lock()
                .ok()
                .map(|tabs| tabs.paired.contains(&key))
                .unwrap_or(true);
            if paired {
                return false;
            }
            if blank {
                blank_url(target_url)
            } else {
                target_url == url
            }
        });
        let target_id = if let Some(info) = candidate {
            let target_id = info
                .get("targetId")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            if target_id.is_empty() {
                continue;
            }
            target_id
        } else {
            let created = match browser
                .call("Target.createTarget", json!({ "url": create_url(&url) }))
                .await
            {
                Ok(value) => value,
                Err(_) => continue,
            };
            let target_id = created
                .get("targetId")
                .and_then(|value| value.as_str())
                .unwrap_or_default()
                .to_string();
            if target_id.is_empty() {
                continue;
            }
            target_id
        };
        let Ok((session_id, viewport)) = attach_target(&browser, &target_id, true).await else {
            continue;
        };
        let key = target_key(env_id, &target_id);
        {
            let Ok(mut tabs) = session.tabs.lock() else {
                continue;
            };
            tabs.known.insert(key.clone());
            tabs.paired.insert(key.clone());
            tabs.target_urls.insert(
                key.clone(),
                if blank {
                    "about:blank".into()
                } else {
                    url.clone()
                },
            );
            tabs.sessions.insert(key.clone(), session_id.clone());
            tabs.session_targets.insert(
                session_key(env_id, &session_id),
                (env_id.clone(), target_id.clone()),
            );
            tabs.viewports.insert(key, viewport);
            tabs.mapped
                .entry(master_target.to_string())
                .or_default()
                .insert(env_id.clone(), target_id.clone());
        }
        let should_activate = session
            .tabs
            .lock()
            .map(|tabs| tabs.active_master.as_deref() == Some(master_target))
            .unwrap_or(false);
        if should_activate {
            let _ = browser
                .call("Target.activateTarget", json!({ "targetId": target_id }))
                .await;
        }
    }
    let activation = session
        .tabs
        .lock()
        .ok()
        .filter(|tabs| tabs.active_master.as_deref() == Some(master_target))
        .and_then(|tabs| {
            tabs.mapped.get(master_target).map(|mapping| {
                mapping
                    .iter()
                    .map(|(env, target)| (env.clone(), target.clone()))
                    .collect::<Vec<_>>()
            })
        })
        .unwrap_or_default();
    activate_tabs(session, &activation).await;
}

async fn activate_tabs(session: &Arc<BroadcastSession>, targets: &[(String, String)]) {
    for (env_id, target_id) in targets {
        if let Some(browser) = session.browsers.get(env_id) {
            let _ = browser
                .call("Target.activateTarget", json!({ "targetId": target_id }))
                .await;
        }
    }
}

async fn set_active_master(session: &Arc<BroadcastSession>, target_id: &str) {
    let (mapping, master_session) = {
        let Ok(mut tabs) = session.tabs.lock() else {
            return;
        };
        if !tabs.masters.contains_key(target_id) || tabs.active_master.as_deref() == Some(target_id)
        {
            return;
        }
        tabs.active_master = Some(target_id.to_string());
        (
            tabs.mapped.get(target_id).cloned().unwrap_or_default(),
            tabs.sessions
                .get(&target_key(&session.master_env_id, target_id))
                .cloned(),
        )
    };
    if let (Some(browser), Some(session_id)) =
        (session.browsers.get(&session.master_env_id), master_session)
    {
        let _ = browser
            .call_on("Page.bringToFront", json!({}), Some(&session_id))
            .await;
    }
    let targets: Vec<(String, String)> = mapping.into_iter().collect();
    activate_tabs(session, &targets).await;
}

async fn monitor_active_tab(session: &Arc<BroadcastSession>) {
    let (master_env, tabs_snapshot) = {
        let Ok(tabs) = session.tabs.lock() else {
            return;
        };
        let snapshot = tabs
            .masters
            .keys()
            .filter_map(|target_id| {
                tabs.sessions
                    .get(&target_key(&session.master_env_id, target_id))
                    .map(|session_id| (target_id.clone(), session_id.clone()))
            })
            .collect::<Vec<_>>();
        (session.master_env_id.clone(), snapshot)
    };
    let Some(browser) = session.browsers.get(&master_env).cloned() else {
        return;
    };
    let mut active = None;
    let mut fallback = None;
    for (target_id, session_id) in tabs_snapshot {
        if let Ok(value) = browser
            .evaluate_on(
                "({ v: document.visibilityState, f: document.hasFocus() })",
                Some(&session_id),
            )
            .await
        {
            let visible = value
                .get("v")
                .and_then(|value| value.as_str())
                .map(|value| value == "visible")
                .unwrap_or(false);
            let focused = value
                .get("f")
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            if focused {
                active = Some(target_id.clone());
                break;
            }
            if visible && fallback.is_none() {
                fallback = Some(target_id);
            }
        }
    }
    if let Some(target_id) = active.or(fallback) {
        set_active_master(session, &target_id).await;
    }
}

async fn reinstall_master_capture(session: &Arc<BroadcastSession>, target_id: &str) {
    let data = {
        let Ok(tabs) = session.tabs.lock() else {
            return;
        };
        if !tabs.masters.contains_key(target_id) {
            return;
        }
        let key = target_key(&session.master_env_id, target_id);
        tabs.sessions.get(&key).cloned().and_then(|session_id| {
            session
                .browsers
                .get(&session.master_env_id)
                .cloned()
                .map(|browser| (browser, session_id))
        })
    };
    if let Some((browser, session_id)) = data {
        reinstall_capture(&browser, &session_id).await;
    }
}

async fn forward_input(session: &Arc<BroadcastSession>, master_target: &str, value: &Value) {
    let (mapping, viewports) = {
        let Ok(tabs) = session.tabs.lock() else {
            return;
        };
        (
            tabs.mapped.get(master_target).cloned().unwrap_or_default(),
            tabs.viewports.clone(),
        )
    };
    if mapping.is_empty() {
        return;
    }
    let mut dispatched = false;
    for (env_id, slave_target) in mapping {
        let Some(browser) = session.browsers.get(&env_id).cloned() else {
            continue;
        };
        let key = target_key(&env_id, &slave_target);
        let Some(session_id) = ({
            let tabs = match session.tabs.lock() {
                Ok(tabs) => tabs,
                Err(_) => continue,
            };
            tabs.sessions.get(&key).cloned()
        }) else {
            continue;
        };
        let Some(command) = build_command(value, &key, &viewports) else {
            continue;
        };
        if dispatch_command(browser, &session_id, command)
            .await
            .is_ok()
        {
            dispatched = true;
        }
    }
    if dispatched {
        session.forwarded.fetch_add(1, Ordering::SeqCst);
    }
}

async fn dispatch_command(
    browser: Arc<CdpClient>,
    session_id: &str,
    command: BroadcastCommand,
) -> Result<()> {
    match command {
        BroadcastCommand::Cdp(method, params) => {
            browser.call_on(method, params, Some(session_id)).await?;
        }
        BroadcastCommand::Eval(expression) => {
            browser.evaluate_on(&expression, Some(session_id)).await?;
        }
    }
    Ok(())
}

enum BroadcastCommand {
    Cdp(&'static str, Value),
    Eval(String),
}

fn build_command(
    value: &Value,
    target: &str,
    viewports: &HashMap<String, (f64, f64)>,
) -> Option<BroadcastCommand> {
    let kind = value.get("kind")?.as_str()?;
    match kind {
        "text" => {
            let text = value.get("text")?.as_str()?;
            if text.is_empty() || text.len() > 16_384 {
                return None;
            }
            Some(BroadcastCommand::Cdp(
                "Input.insertText",
                json!({ "text": text }),
            ))
        }
        "scroll" => {
            let x = value
                .get("x")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
                .max(0.0);
            let y = value
                .get("y")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0)
                .max(0.0);
            Some(BroadcastCommand::Eval(format!(
                "window.scrollTo({x:.3}, {y:.3})"
            )))
        }
        "key" => {
            let event_type = value.get("type")?.as_str()?;
            let cdp_type = if event_type == "keydown" {
                "keyDown"
            } else if event_type == "keyup" {
                "keyUp"
            } else {
                return None;
            };
            let key = value.get("key")?.as_str()?;
            let code = value.get("code").and_then(|v| v.as_str()).unwrap_or("");
            let key_code = value
                .get("keyCode")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, 255) as u32;
            let modifiers = value
                .get("modifiers")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, 15) as u32;
            let mut params = json!({
                "type": cdp_type,
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": key_code,
                "nativeVirtualKeyCode": key_code,
                "modifiers": modifiers,
                "autoRepeat": value.get("autoRepeat").and_then(|v| v.as_bool()).unwrap_or(false),
            });
            if cdp_type == "keyDown" {
                if let Some(text) = value.get("text").and_then(|v| v.as_str()) {
                    if !text.is_empty() && text.len() <= 32 {
                        params["text"] = json!(text);
                        params["unmodifiedText"] = json!(text);
                    }
                }
            }
            Some(BroadcastCommand::Cdp("Input.dispatchKeyEvent", params))
        }
        "mouse" => {
            let event_type = value.get("type")?.as_str()?;
            let master_width = value
                .get("vw")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(1.0);
            let master_height = value
                .get("vh")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite() && *v > 0.0)
                .unwrap_or(1.0);
            let (target_width, target_height) = viewports
                .get(target)
                .copied()
                .unwrap_or((master_width, master_height));
            let raw_x = value
                .get("x")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            let raw_y = value
                .get("y")
                .and_then(|v| v.as_f64())
                .filter(|v| v.is_finite())
                .unwrap_or(0.0);
            let x = (raw_x / master_width * target_width).clamp(0.0, target_width - 0.01);
            let y = (raw_y / master_height * target_height).clamp(0.0, target_height - 0.01);
            let modifiers = value
                .get("modifiers")
                .and_then(|v| v.as_i64())
                .unwrap_or(0)
                .clamp(0, 15) as u32;
            if event_type == "wheel" {
                let mode = value.get("deltaMode").and_then(|v| v.as_i64()).unwrap_or(0);
                let factor = match mode {
                    1 => 16.0,
                    2 => target_height,
                    _ => 1.0,
                };
                let delta_x = value
                    .get("deltaX")
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
                    * factor;
                let delta_y = value
                    .get("deltaY")
                    .and_then(|v| v.as_f64())
                    .filter(|v| v.is_finite())
                    .unwrap_or(0.0)
                    * factor;
                return Some(BroadcastCommand::Eval(scroll_expression(
                    x, y, delta_x, delta_y,
                )));
            }
            let cdp_type = match event_type {
                "mousemove" => "mouseMoved",
                "mousedown" => "mousePressed",
                "mouseup" => "mouseReleased",
                _ => return None,
            };
            let button = match value.get("button").and_then(|v| v.as_i64()) {
                Some(1) => "middle",
                Some(2) => "right",
                Some(3) => "back",
                Some(4) => "forward",
                _ => "left",
            };
            let click_count = value
                .get("clickCount")
                .and_then(|v| v.as_i64())
                .unwrap_or(1)
                .clamp(1, 3);
            Some(BroadcastCommand::Cdp(
                "Input.dispatchMouseEvent",
                json!({
                    "type": cdp_type,
                    "x": x,
                    "y": y,
                    "button": if cdp_type == "mouseMoved" { "none" } else { button },
                    "buttons": value.get("buttons").and_then(|v| v.as_i64()).unwrap_or(0).clamp(0, 31),
                    "clickCount": click_count,
                    "modifiers": modifiers,
                }),
            ))
        }
        _ => None,
    }
}

fn scroll_expression(x: f64, y: f64, delta_x: f64, delta_y: f64) -> String {
    format!(
        r#"(() => {{
            const x = {x:.3};
            const y = {y:.3};
            const dx = {delta_x:.3};
            const dy = {delta_y:.3};
            let node = document.elementFromPoint(x, y);
            while (node && node !== document.documentElement) {{
                const style = getComputedStyle(node);
                const canY = /(auto|scroll|overlay)/.test(style.overflowY) && node.scrollHeight > node.clientHeight;
                const canX = /(auto|scroll|overlay)/.test(style.overflowX) && node.scrollWidth > node.clientWidth;
                if (canX || canY) {{
                    if (canX) node.scrollLeft += dx;
                    if (canY) node.scrollTop += dy;
                    return;
                }}
                node = node.parentElement;
            }}
            window.scrollBy(dx, dy);
        }})()"#
    )
}
