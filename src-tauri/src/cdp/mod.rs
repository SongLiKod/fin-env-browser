use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::{
    sync::{broadcast, mpsc, oneshot, Mutex},
    time::timeout,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{
    error::{AppError, Result},
    fingerprint::Fingerprint,
    models::{now_iso, MockRule},
};

#[derive(Debug, Clone)]
pub struct CdpEvent {
    pub method: String,
    pub params: Value,
    pub session_id: Option<String>,
}

type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<std::result::Result<Value, String>>>>>;

#[derive(Clone)]
pub struct CdpClient {
    tx: mpsc::UnboundedSender<String>,
    pending: PendingMap,
    next_id: Arc<AtomicU64>,
    closed: Arc<AtomicBool>,
    events: broadcast::Sender<CdpEvent>,
}

impl CdpClient {
    pub async fn connect(ws_url: &str) -> Result<Self> {
        let (stream, _) = timeout(Duration::from_secs(10), connect_async(ws_url))
            .await
            .map_err(|_| AppError::internal("CDP WebSocket 连接超时"))?
            .map_err(|e| AppError::internal(format!("CDP WebSocket 连接失败: {e}")))?;
        let (mut write, mut read) = stream.split();
        let (tx, mut rx) = mpsc::unbounded_channel::<String>();
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let closed = Arc::new(AtomicBool::new(false));
        let closed_writer = closed.clone();
        tokio::spawn(async move {
            while let Some(text) = rx.recv().await {
                if write.send(Message::Text(text)).await.is_err() {
                    closed_writer.store(true, Ordering::SeqCst);
                    break;
                }
            }
        });

        let pending_reader = pending.clone();
        let closed_reader = closed.clone();
        let (events, _) = broadcast::channel::<CdpEvent>(16384);
        let events_writer = events.clone();
        tokio::spawn(async move {
            while let Some(message) = read.next().await {
                match message {
                    Ok(Message::Text(text)) => {
                        if let Ok(value) = serde_json::from_str::<Value>(&text) {
                            if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                                let mut map = pending_reader.lock().await;
                                if let Some(sender) = map.remove(&id) {
                                    if let Some(error) = value.get("error") {
                                        let message = error
                                            .get("message")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("CDP 调用失败");
                                        let _ = sender.send(Err(message.to_string()));
                                    } else {
                                        let result =
                                            value.get("result").cloned().unwrap_or(Value::Null);
                                        let _ = sender.send(Ok(result));
                                    }
                                }
                            } else if let Some(method) =
                                value.get("method").and_then(|v| v.as_str())
                            {
                                let params = value.get("params").cloned().unwrap_or(Value::Null);
                                let session_id = value
                                    .get("sessionId")
                                    .and_then(|v| v.as_str())
                                    .map(ToString::to_string);
                                let _ = events_writer.send(CdpEvent {
                                    method: method.to_string(),
                                    params,
                                    session_id,
                                });
                            }
                        }
                    }
                    Ok(Message::Close(_)) | Err(_) => break,
                    _ => {}
                }
            }
            closed_reader.store(true, Ordering::SeqCst);
            let mut map = pending_reader.lock().await;
            for (_, sender) in map.drain() {
                let _ = sender.send(Err("CDP 连接已关闭".into()));
            }
        });

        Ok(Self {
            tx,
            pending,
            next_id: Arc::new(AtomicU64::new(1)),
            closed,
            events,
        })
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    pub async fn call(&self, method: &str, params: Value) -> Result<Value> {
        self.call_on(method, params, None).await
    }

    pub async fn call_on(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<Value> {
        if self.is_closed() {
            return Err(AppError::internal("CDP 连接已关闭"));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        {
            let mut map = self.pending.lock().await;
            map.insert(id, tx);
        }
        let payload = match session_id {
            Some(session_id) => {
                json!({ "id": id, "method": method, "params": params, "sessionId": session_id })
            }
            None => json!({ "id": id, "method": method, "params": params }),
        }
        .to_string();
        if self.tx.send(payload).is_err() {
            return Err(AppError::internal("CDP 消息发送失败"));
        }
        match timeout(Duration::from_secs(20), rx).await {
            Ok(Ok(Ok(value))) => Ok(value),
            Ok(Ok(Err(message))) => Err(AppError::code_with("cdp_error", message)),
            Ok(Err(_)) => Err(AppError::internal("CDP 响应通道关闭")),
            Err(_) => {
                let mut map = self.pending.lock().await;
                map.remove(&id);
                Err(AppError::internal(format!("CDP 调用 {method} 超时")))
            }
        }
    }

    pub async fn evaluate(&self, expression: &str) -> Result<Value> {
        self.evaluate_on(expression, None).await
    }

    pub async fn evaluate_on(&self, expression: &str, session_id: Option<&str>) -> Result<Value> {
        let result = self
            .call_on(
                "Runtime.evaluate",
                json!({
                    "expression": expression,
                    "returnByValue": true,
                    "awaitPromise": true,
                    "userGesture": true
                }),
                session_id,
            )
            .await?;
        if let Some(exception) = result.get("exceptionDetails") {
            let text = exception
                .get("exception")
                .and_then(|e| e.get("description"))
                .and_then(|v| v.as_str())
                .or_else(|| exception.get("text").and_then(|v| v.as_str()))
                .unwrap_or("脚本执行异常");
            return Err(AppError::bad_request(format!("脚本执行失败: {text}")));
        }
        Ok(result
            .get("result")
            .and_then(|r| r.get("value"))
            .cloned()
            .unwrap_or(Value::Null))
    }
}

/// Simple, dependency-free wildcard matcher (`*` matches any sequence).
pub fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.to_lowercase().chars().collect();
    let t: Vec<char> = text.to_lowercase().chars().collect();
    let (mut pi, mut ti) = (0usize, 0usize);
    let (mut star, mut mark) = (None, 0usize);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

pub fn find_mock<'a>(rules: &'a [MockRule], url: &str, method: &str) -> Option<&'a MockRule> {
    rules.iter().find(|rule| {
        rule.enabled
            && (rule.method.eq_ignore_ascii_case("ANY") || rule.method.eq_ignore_ascii_case(method))
            && wildcard_match(&rule.pattern, url)
    })
}

pub fn headers_to_cdp(value: &Value) -> Value {
    let mut out = Vec::new();
    if let Some(map) = value.as_object() {
        for (name, value) in map {
            if let Some(text) = value.as_str() {
                out.push(json!({ "name": name, "value": text }));
            } else {
                out.push(json!({ "name": name, "value": value.to_string() }));
            }
        }
    }
    Value::Array(out)
}

// ---------------------------------------------------------------------------
// Emulation helpers
// ---------------------------------------------------------------------------

pub async fn apply_emulation(client: &CdpClient, fp: &Fingerprint) -> Result<()> {
    let full_version = fp
        .client_hints
        .full_version_list
        .iter()
        .find(|b| b.brand == "Google Chrome")
        .or_else(|| fp.client_hints.full_version_list.first())
        .map(|b| b.version.clone())
        .unwrap_or_else(|| "131.0.0.0".into());
    let ua_metadata = json!({
        "brands": fp.client_hints.brands.clone(),
        "fullVersionList": fp.client_hints.full_version_list.clone(),
        "fullVersion": full_version,
        "platform": fp.client_hints.platform.clone(),
        "platformVersion": fp.client_hints.platform_version.clone(),
        "architecture": fp.client_hints.architecture.clone(),
        "model": fp.client_hints.model.clone(),
        "mobile": fp.client_hints.mobile,
        "bitness": fp.client_hints.bitness.clone(),
        "wow64": false
    });
    client
        .call(
            "Emulation.setUserAgentOverride",
            json!({
                "userAgent": fp.user_agent,
                "acceptLanguage": fp.languages.join(","),
                "platform": fp.platform,
                "userAgentMetadata": ua_metadata
            }),
        )
        .await?;
    let _ = client
        .call(
            "Emulation.setTimezoneOverride",
            json!({ "timezoneId": fp.timezone }),
        )
        .await;
    let _ = client
        .call(
            "Emulation.setLocaleOverride",
            json!({ "locale": fp.languages.first().cloned().unwrap_or_else(|| "en-US".into()) }),
        )
        .await;
    let _ = client
        .call(
            "Emulation.setHardwareConcurrencyOverride",
            json!({ "hardwareConcurrency": fp.hardware_concurrency }),
        )
        .await;
    let _ = client
        .call(
            "Emulation.setTouchEmulationEnabled",
            json!({ "enabled": fp.touch_points > 0, "maxTouchPoints": fp.touch_points }),
        )
        .await;
    let _ = client
        .call(
            "Emulation.setEmulatedMedia",
            json!({
                "features": [{
                    "name": "prefers-color-scheme",
                    "value": if fp.color_scheme == "no-preference" { "light" } else { fp.color_scheme.as_str() }
                }]
            }),
        )
        .await;
    Ok(())
}

pub const CAPTCHA_SCRIPT: &str = r#"(function () {
  if (window.__finEnvCaptchaInstalled) return;
  window.__finEnvCaptchaInstalled = true;
  const selectors = [
    'img[src*="captcha" i]', 'img[alt*="captcha" i]', 'img[id*="captcha" i]',
    'input[name*="captcha" i]', 'input[id*="captcha" i]',
    'iframe[src*="captcha" i]', '[class*="captcha" i]', '[id*="captcha" i]',
    'img[src*="verify" i]', '[class*="verify-code" i]'
  ];
  const report = (el) => {
    try {
      if (!window.__finEnvCaptcha) return;
      if (document.visibilityState !== 'visible') return;
      const hint = (el.outerHTML || '').replace(/\s+/g, ' ').slice(0, 240);
      window.__finEnvCaptcha(JSON.stringify({
        url: location.href,
        hint,
        visible: document.visibilityState
      }));
    } catch (e) {}
  };
  const scan = () => {
    if (document.visibilityState !== 'visible') return;
    for (const sel of selectors) {
      const el = document.querySelector(sel);
      if (el) { report(el); return; }
    }
  };
  let scheduled = false;
  const observer = new MutationObserver(() => {
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => { scheduled = false; scan(); }, 600);
  });
  try { observer.observe(document.documentElement, { childList: true, subtree: true }); } catch (e) {}
  setInterval(scan, 2500);
  scan();
})();"#;

pub async fn install_fingerprint(client: &CdpClient, fp: &Fingerprint) -> Result<()> {
    let source = format!(
        "{}\n{}",
        crate::fingerprint::injection_script(fp),
        CAPTCHA_SCRIPT
    );
    let _ = client
        .call("Runtime.addBinding", json!({ "name": "__finEnvCaptcha" }))
        .await;
    let _ = client
        .call(
            "Page.addScriptToEvaluateOnNewDocument",
            json!({ "source": source }),
        )
        .await;
    let _ = client
        .call("Runtime.evaluate", json!({ "expression": source }))
        .await;
    let _ = apply_emulation(client, fp).await;
    Ok(())
}

// ---------------------------------------------------------------------------
// Cookie helpers
// ---------------------------------------------------------------------------

pub fn cookies_from_cdp(value: &Value) -> Vec<crate::models::CookieRecord> {
    value
        .get("cookies")
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .map(|c| crate::models::CookieRecord {
                    name: c
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    value: c
                        .get("value")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    domain: c
                        .get("domain")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string(),
                    path: c
                        .get("path")
                        .and_then(|v| v.as_str())
                        .unwrap_or("/")
                        .to_string(),
                    secure: c.get("secure").and_then(|v| v.as_bool()).unwrap_or(false),
                    http_only: c.get("httpOnly").and_then(|v| v.as_bool()).unwrap_or(false),
                    session: c.get("session").and_then(|v| v.as_bool()).unwrap_or(false),
                    expires: c
                        .get("expires")
                        .and_then(|v| v.as_f64())
                        .filter(|v| *v > 0.0),
                    same_site: c
                        .get("sameSite")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub fn cookies_to_cdp(cookies: &[crate::models::CookieRecord]) -> Value {
    let items: Vec<Value> = cookies
        .iter()
        .map(|c| {
            let mut item = json!({
                "name": c.name,
                "value": c.value,
                "domain": c.domain,
                "path": c.path,
                "secure": c.secure,
                "httpOnly": c.http_only
            });
            if let Some(expires) = c.expires {
                item["expires"] = json!(expires);
            }
            if let Some(same_site) = &c.same_site {
                let normalized = match same_site.to_ascii_lowercase().as_str() {
                    "strict" => "Strict",
                    "lax" => "Lax",
                    "none" => "None",
                    _ => "Lax",
                };
                item["sameSite"] = json!(normalized);
            }
            item
        })
        .collect();
    json!({ "cookies": items })
}

pub fn png_dimensions(base64_data: &str) -> (u32, u32) {
    if let Ok(bytes) = B64.decode(base64_data) {
        if bytes.len() > 24 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
            let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
            let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
            return (width, height);
        }
    }
    (0, 0)
}

// ---------------------------------------------------------------------------
// HAR collection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct HarState {
    pub active: bool,
    pub started_at: Option<String>,
    pub entries: Vec<Value>,
    pub pending: HashMap<String, Value>,
    pub page_url: String,
}

impl HarState {
    pub fn on_request(&mut self, params: &Value) {
        if !self.active {
            return;
        }
        let Some(id) = params.get("requestId").and_then(|v| v.as_str()) else {
            return;
        };
        let request = params.get("request").cloned().unwrap_or(Value::Null);
        let timestamp = params
            .get("timestamp")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);
        let wall_time = params
            .get("wallTime")
            .and_then(|v| v.as_f64())
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis() as f64 / 1000.0);
        let url = request.get("url").and_then(|v| v.as_str()).unwrap_or("");
        self.page_url = url.to_string();
        let entry = json!({
            "startedDateTime": chrono::DateTime::from_timestamp(wall_time as i64, 0)
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(now_iso),
            "time": 0.0,
            "request": {
                "method": request.get("method").cloned().unwrap_or(json!("GET")),
                "url": url,
                "httpVersion": "HTTP/1.1",
                "cookies": [],
                "headers": headers_from_cdp(&request["headers"]),
                "queryString": [],
                "headersSize": -1,
                "bodySize": -1
            },
            "response": {
                "status": 0,
                "statusText": "",
                "httpVersion": "HTTP/1.1",
                "cookies": [],
                "headers": [],
                "content": { "size": 0, "mimeType": "x-unknown" },
                "redirectURL": "",
                "headersSize": -1,
                "bodySize": -1
            },
            "cache": {},
            "timings": { "send": 0, "wait": 0, "receive": 0 }
        });
        self.pending.insert(
            id.to_string(),
            json!({ "entry": entry, "started": timestamp }),
        );
    }

    pub fn on_response(&mut self, params: &Value) {
        if !self.active {
            return;
        }
        let Some(id) = params.get("requestId").and_then(|v| v.as_str()) else {
            return;
        };
        let response = params.get("response").cloned().unwrap_or(Value::Null);
        if let Some(record) = self.pending.get_mut(id) {
            let entry = &mut record["entry"];
            entry["response"]["status"] =
                json!(response.get("status").and_then(|v| v.as_i64()).unwrap_or(0));
            entry["response"]["statusText"] =
                response.get("statusText").cloned().unwrap_or(json!(""));
            entry["response"]["headers"] = headers_from_cdp(&response["headers"]);
            entry["response"]["content"]["mimeType"] = response
                .get("mimeType")
                .cloned()
                .unwrap_or(json!("x-unknown"));
            entry["response"]["content"]["size"] = json!(response
                .get("encodedDataLength")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0));
        }
    }

    pub fn on_finished(&mut self, params: &Value) {
        if !self.active {
            return;
        }
        let Some(id) = params.get("requestId").and_then(|v| v.as_str()) else {
            return;
        };
        if let Some(record) = self.pending.remove(id) {
            let mut entry = record["entry"].clone();
            let started = record["started"].as_f64().unwrap_or(0.0);
            let ended = params
                .get("timestamp")
                .and_then(|v| v.as_f64())
                .unwrap_or(started);
            let time = ((ended - started) * 1000.0).max(0.0);
            entry["time"] = json!(time);
            entry["timings"]["wait"] = json!(time);
            entry["response"]["bodySize"] = json!(params
                .get("encodedDataLength")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0));
            self.entries.push(entry);
        }
    }

    pub fn build(&self) -> Value {
        json!({
            "log": {
                "version": "1.2",
                "creator": { "name": "FinEnv", "version": env!("CARGO_PKG_VERSION") },
                "pages": [{
                    "startedDateTime": self.started_at.clone().unwrap_or_else(now_iso),
                    "id": "finenv-page",
                    "title": self.page_url,
                    "pageTimings": {}
                }],
                "entries": self.entries
            }
        })
    }
}

fn headers_from_cdp(headers: &Value) -> Value {
    let mut out = Vec::new();
    if let Some(map) = headers.as_object() {
        for (name, value) in map {
            out.push(json!({ "name": name, "value": value.as_str().unwrap_or("").to_string() }));
        }
    }
    Value::Array(out)
}
