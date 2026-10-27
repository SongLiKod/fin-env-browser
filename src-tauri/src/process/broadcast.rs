use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
};

use serde_json::{json, Value};
use tokio::{sync::mpsc, task::JoinHandle};

use crate::{
    audit_log::{actions, Audit},
    error::{AppError, Result},
    models::{new_id, now_iso, BroadcastStartInput, BroadcastStatus, OpenOptions},
};

use super::BrowserManager;

const BINDING: &str = "__finEnvBroadcast";
const MAX_ENVIRONMENTS: usize = 30;
const MAX_PAYLOAD: usize = 32 * 1024;

pub struct BroadcastSession {
    pub id: String,
    pub master_env_id: String,
    pub env_ids: Vec<String>,
    pub started_at: String,
    pub paused: AtomicBool,
    pub forwarded: AtomicU64,
    pub viewports: std::sync::RwLock<HashMap<String, (f64, f64)>>,
    pub script_id: Mutex<Option<String>>,
    pub errors: Vec<String>,
}

pub struct BroadcastRuntime {
    pub session: Arc<BroadcastSession>,
    tx: mpsc::UnboundedSender<Value>,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl BroadcastRuntime {
    fn stop_worker(&self) {
        if let Ok(mut guard) = self.worker.lock() {
            if let Some(worker) = guard.take() {
                worker.abort();
            }
        }
    }
}

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
        let mut env_ids = Vec::new();
        for id in input.env_ids {
            if seen.insert(id.clone()) {
                env_ids.push(id);
            }
        }
        if env_ids.len() < 2 {
            return Err(AppError::bad_request("同步控制至少需要选择 2 个环境"));
        }
        if env_ids.len() > MAX_ENVIRONMENTS {
            return Err(AppError::bad_request(format!(
                "同步控制最多支持 {MAX_ENVIRONMENTS} 个环境"
            )));
        }

        let url = input
            .url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(url) = &url {
            if !url.starts_with("http://") && !url.starts_with("https://") && url != "about:blank" {
                return Err(AppError::bad_request(
                    "同步访问仅支持 http/https/about:blank URL",
                ));
            }
        }
        let open_missing = input.open_missing.unwrap_or(true);
        let mut active = Vec::new();
        let mut errors = Vec::new();
        let mut names = HashMap::new();

        for env_id in &env_ids {
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
            active.push(env_id.clone());
        }

        if active.len() < 2 {
            return Err(AppError::bad_request(format!(
                "可用于同步控制的环境不足 2 个：{}",
                errors.join("；")
            )));
        }

        let master_env_id = match input.master_env_id {
            Some(master) if active.contains(&master) => master,
            Some(_) => {
                return Err(AppError::bad_request("指定的主控环境未成功启动"));
            }
            None => active[0].clone(),
        };

        let master = self
            .instances
            .read()
            .ok()
            .and_then(|map| map.get(&master_env_id).cloned())
            .ok_or_else(|| AppError::bad_request("主控环境不可用"))?;
        let focus_targets = self
            .instances
            .read()
            .map(|instances| {
                active
                    .iter()
                    .filter_map(|env_id| instances.get(env_id).cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for instance in focus_targets {
            let _ = instance
                .client
                .call(
                    "Emulation.setFocusEmulationEnabled",
                    json!({ "enabled": true }),
                )
                .await;
        }
        let _ = master
            .client
            .call("Runtime.addBinding", json!({ "name": BINDING }))
            .await;
        let binding = master
            .client
            .evaluate("typeof window.__finEnvBroadcast")
            .await?;
        if binding.as_str() != Some("function") {
            return Err(AppError::internal("无法安装同步控制输入通道"));
        }
        let script = master
            .client
            .call(
                "Page.addScriptToEvaluateOnNewDocument",
                json!({ "source": CAPTURE_SCRIPT }),
            )
            .await?;
        let script_id = script
            .get("identifier")
            .and_then(|value| value.as_str())
            .map(ToString::to_string);
        master.client.evaluate(CAPTURE_SCRIPT).await?;
        let installed = master
            .client
            .evaluate("window.__finEnvBroadcastInstalled === true")
            .await?;
        if installed.as_bool() != Some(true) {
            return Err(AppError::internal("无法安装同步控制输入捕获脚本"));
        }
        let _ = master.client.call("Page.bringToFront", json!({})).await;

        let (tx, mut rx) = mpsc::unbounded_channel::<Value>();
        let session = Arc::new(BroadcastSession {
            id: new_id(),
            master_env_id: master_env_id.clone(),
            env_ids: active.clone(),
            started_at: now_iso(),
            paused: AtomicBool::new(false),
            forwarded: AtomicU64::new(0),
            viewports: std::sync::RwLock::new(HashMap::new()),
            script_id: Mutex::new(script_id),
            errors: errors.clone(),
        });

        let viewports = {
            let instances = self
                .instances
                .read()
                .map(|guard| {
                    active
                        .iter()
                        .filter_map(|env_id| {
                            guard
                                .get(env_id)
                                .cloned()
                                .map(|instance| (env_id.clone(), instance))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let mut map = HashMap::new();
            for (env_id, instance) in instances {
                let value = instance
                    .client
                    .evaluate(
                        "({ w: Math.max(1, document.documentElement?.clientWidth || window.innerWidth || 1), h: Math.max(1, document.documentElement?.clientHeight || window.innerHeight || 1) })",
                    )
                    .await
                    .unwrap_or(Value::Null);
                let width = value
                    .get("w")
                    .and_then(|v| v.as_f64())
                    .filter(|v| *v > 0.0)
                    .unwrap_or(1.0);
                let height = value
                    .get("h")
                    .and_then(|v| v.as_f64())
                    .filter(|v| *v > 0.0)
                    .unwrap_or(1.0);
                map.insert(env_id, (width, height));
            }
            map
        };
        if let Ok(mut guard) = session.viewports.write() {
            *guard = viewports;
        }

        let weak = Arc::downgrade(self);
        let worker_session = session.clone();
        let worker = tokio::spawn(async move {
            while let Some(value) = rx.recv().await {
                if worker_session.paused.load(Ordering::SeqCst) {
                    continue;
                }
                let Some(manager) = weak.upgrade() else {
                    break;
                };
                if let Err(error) = manager.forward_broadcast(&worker_session, &value).await {
                    tracing::debug!(error = %error, "broadcast forwarding failed");
                }
            }
        });

        let runtime = Arc::new(BroadcastRuntime {
            session: session.clone(),
            tx,
            worker: Mutex::new(Some(worker)),
        });
        *self.broadcast.write().await = Some(runtime);

        let names_list = active
            .iter()
            .map(|id| names.get(id).cloned().unwrap_or_else(|| id.clone()))
            .collect::<Vec<_>>();
        Audit::global(
            &self.db,
            actor,
            actions::BROADCAST_START,
            format!(
                "开启同步控制：主控 {}，参与 {} 个环境（{}）",
                names
                    .get(&master_env_id)
                    .cloned()
                    .unwrap_or_else(|| master_env_id.clone()),
                active.len(),
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
        runtime.session.paused.store(true, Ordering::SeqCst);
        runtime.stop_worker();

        let focus_targets = self
            .instances
            .read()
            .map(|instances| {
                runtime
                    .session
                    .env_ids
                    .iter()
                    .filter_map(|env_id| instances.get(env_id).cloned())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        for instance in focus_targets {
            let _ = instance
                .client
                .call(
                    "Emulation.setFocusEmulationEnabled",
                    json!({ "enabled": false }),
                )
                .await;
        }

        if let Some(instance) = self
            .instances
            .read()
            .ok()
            .and_then(|map| map.get(&runtime.session.master_env_id).cloned())
        {
            let _ = instance
                .client
                .evaluate("window.__finEnvBroadcastCleanup && window.__finEnvBroadcastCleanup()")
                .await;
            if let Some(script_id) = runtime
                .session
                .script_id
                .lock()
                .ok()
                .and_then(|guard| guard.clone())
            {
                let _ = instance
                    .client
                    .call(
                        "Page.removeScriptToEvaluateOnNewDocument",
                        json!({ "identifier": script_id }),
                    )
                    .await;
            }
            let _ = instance
                .client
                .call("Runtime.removeBinding", json!({ "name": BINDING }))
                .await;
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

    pub(crate) async fn broadcast_handle_payload(self: &Arc<Self>, env_id: &str, payload: &str) {
        let runtime = self.broadcast.read().await.clone();
        let Some(runtime) = runtime else {
            return;
        };
        if runtime.session.master_env_id != env_id {
            return;
        }
        if runtime.session.paused.load(Ordering::SeqCst) || payload.len() > MAX_PAYLOAD {
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
        let _ = runtime.tx.send(value);
    }

    pub(crate) async fn broadcast_on_master_load(self: &Arc<Self>, env_id: &str) {
        let runtime = self.broadcast.read().await.clone();
        let Some(runtime) = runtime else {
            return;
        };
        if runtime.session.master_env_id != env_id {
            return;
        }
        let Some(instance) = self
            .instances
            .read()
            .ok()
            .and_then(|map| map.get(env_id).cloned())
        else {
            return;
        };
        let _ = instance
            .client
            .call("Runtime.addBinding", json!({ "name": BINDING }))
            .await;
        let _ = instance.client.evaluate(CAPTURE_SCRIPT).await;
    }

    pub(crate) async fn broadcast_on_env_closed(self: &Arc<Self>, env_id: &str, actor: &str) {
        let active = self.broadcast.read().await.clone();
        if let Some(runtime) = active {
            if runtime.session.env_ids.iter().any(|id| id == env_id) {
                let _ = self.broadcast_stop(actor, "参与环境已关闭").await;
            }
        }
    }

    async fn forward_broadcast(
        self: &Arc<Self>,
        session: &Arc<BroadcastSession>,
        value: &Value,
    ) -> Result<()> {
        let targets = match self.instances.read() {
            Ok(instances) => session
                .env_ids
                .iter()
                .filter(|id| id.as_str() != session.master_env_id)
                .filter_map(|id| instances.get(id).cloned())
                .collect::<Vec<_>>(),
            Err(_) => return Ok(()),
        };
        if targets.is_empty() {
            return Ok(());
        }
        let viewports = session
            .viewports
            .read()
            .map(|guard| guard.clone())
            .unwrap_or_default();
        let mut futures = Vec::new();
        for instance in targets {
            let command = build_command(value, &instance.env_id, &viewports);
            let Some(command) = command else {
                return Ok(());
            };
            futures.push(dispatch_command(instance, command));
        }
        let results = futures_util::future::join_all(futures).await;
        if results.iter().any(|result| result.is_ok()) {
            session.forwarded.fetch_add(1, Ordering::SeqCst);
        }
        if let Some(error) = results.iter().find_map(|result| result.as_ref().err()) {
            tracing::debug!(error = %error, "broadcast target dispatch failed");
        }
        Ok(())
    }
}

fn build_command(
    value: &Value,
    env_id: &str,
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
                .get(env_id)
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

enum BroadcastCommand {
    Cdp(&'static str, Value),
    Eval(String),
}

async fn dispatch_command(
    instance: Arc<super::BrowserInstance>,
    command: BroadcastCommand,
) -> Result<()> {
    match command {
        BroadcastCommand::Cdp(method, params) => {
            instance.client.call(method, params).await?;
        }
        BroadcastCommand::Eval(expression) => {
            instance.client.evaluate(&expression).await?;
        }
    }
    Ok(())
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
