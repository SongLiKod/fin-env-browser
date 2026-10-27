use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use serde_json::{json, Value};
use tokio::time::sleep;

use crate::{
    audit_log::{actions, Audit},
    cdp::{self, CdpClient},
    db::Db,
    error::{AppError, Result},
    fingerprint,
    models::{now_iso, Environment, FingerprintReport},
    paths,
    process::BrowserManager,
};

/// Run a fingerprint consistency report.
///
/// A fresh tab is opened against the configured external detection service so
/// the user's automation page is not disturbed. The probe itself runs through
/// CDP and measures the actual browser values, then compares them with the
/// configured fingerprint.
pub async fn detect(
    db: &Db,
    browsers: &Arc<BrowserManager>,
    data_dir: &Path,
    env: &Environment,
    detection_url: &str,
    actor: &str,
) -> Result<FingerprintReport> {
    let runtime = browsers
        .get_runtime(&env.id)
        .ok_or_else(|| AppError::bad_request("环境未打开，请先启动浏览器再检测"))?;
    let port = runtime.debug_port;
    let started = std::time::Instant::now();

    let target = create_target(port, detection_url).await;
    let (observed, screenshot_path, error) = match target {
        Ok((target_id, ws)) => {
            let result = probe_target(&ws, &env.fingerprint, data_dir, &env.id).await;
            let _ = close_target(port, &target_id).await;
            match result {
                Ok((observed, screenshot)) => (observed, Some(screenshot), None),
                Err(e) => (Value::Null, None, Some(e.message().to_string())),
            }
        }
        Err(e) => (Value::Null, None, Some(e.message().to_string())),
    };

    let (observed, error) = if observed.is_null() {
        // Fall back to probing the existing page so a blocked external service
        // still produces a useful local report.
        match browsers.execute(&env.id, fingerprint::PROBE_SCRIPT).await {
            Ok(value) => (
                value,
                error.or_else(|| Some("外部检测页不可用，已使用当前页面探针".into())),
            ),
            Err(e) => (
                Value::Null,
                Some(format!(
                    "指纹探针失败: {}; {}",
                    error.unwrap_or_default(),
                    e.message()
                )),
            ),
        }
    } else {
        (observed, error)
    };

    if observed.is_null() {
        return Err(AppError::bad_request(
            error.unwrap_or_else(|| "无法读取浏览器指纹".into()),
        ));
    }

    let (checks, score) = fingerprint::build_report_checks(&env.fingerprint, &observed);
    let report = FingerprintReport {
        id: crate::models::new_id(),
        env_id: env.id.clone(),
        score,
        checks,
        observed: observed.clone(),
        configured: serde_json::to_value(&env.fingerprint)?,
        screenshot_path,
        external_url: detection_url.to_string(),
        created_at: now_iso(),
    };
    db.insert_fingerprint_report(&report)?;
    Audit::env(
        db,
        actor,
        actions::FINGERPRINT_DETECT,
        env,
        format!(
            "指纹检测完成，评分 {score}/100，耗时 {}ms{}",
            started.elapsed().as_millis(),
            error
                .as_deref()
                .map(|e| format!("（{e}）"))
                .unwrap_or_default()
        ),
    )?;
    Ok(report)
}

async fn create_target(port: u16, url: &str) -> Result<(String, String)> {
    let encoded: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
    let endpoint = format!("http://127.0.0.1:{port}/json/new?{encoded}");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(AppError::from)?;
    // Chrome 111+ requires PUT for /json/new.
    let response = client
        .put(&endpoint)
        .send()
        .await
        .map_err(|e| AppError::internal(format!("创建检测标签页失败: {e}")))?;
    if !response.status().is_success() {
        return Err(AppError::internal(format!(
            "创建检测标签页失败: HTTP {}",
            response.status()
        )));
    }
    let value: Value = response
        .json()
        .await
        .map_err(|e| AppError::internal(format!("检测标签页响应解析失败: {e}")))?;
    let id = value
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::internal("检测标签页缺少 id"))?
        .to_string();
    let ws = value
        .get("webSocketDebuggerUrl")
        .and_then(|v| v.as_str())
        .ok_or_else(|| AppError::internal("检测标签页缺少调试地址"))?
        .to_string();
    Ok((id, ws))
}

async fn close_target(port: u16, id: &str) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(AppError::from)?;
    let _ = client
        .get(format!("http://127.0.0.1:{port}/json/close/{id}"))
        .send()
        .await;
    Ok(())
}

async fn probe_target(
    ws: &str,
    fp: &fingerprint::Fingerprint,
    data_dir: &Path,
    env_id: &str,
) -> Result<(Value, String)> {
    let client = CdpClient::connect(ws).await?;
    let _ = client.call("Page.enable", json!({})).await;
    let _ = client.call("Runtime.enable", json!({})).await;
    cdp::install_fingerprint(&client, fp).await?;
    // Allow the detection service (if any) to finish rendering.
    for _ in 0..40 {
        if let Ok(state) = client.evaluate("document.readyState").await {
            if state.as_str() == Some("complete") {
                break;
            }
        }
        sleep(Duration::from_millis(250)).await;
    }
    sleep(Duration::from_millis(1500)).await;
    let observed = client.evaluate(fingerprint::PROBE_SCRIPT).await?;
    let screenshot = client
        .call(
            "Page.captureScreenshot",
            json!({ "format": "png", "fromSurface": true, "captureBeyondViewport": false }),
        )
        .await
        .ok()
        .and_then(|value| {
            value
                .get("data")
                .and_then(|v| v.as_str())
                .map(ToString::to_string)
        });
    let screenshot_path = if let Some(data) = screenshot {
        if let Ok(bytes) = B64.decode(&data) {
            let path = paths::reports_dir(data_dir).join(format!(
                "detect-{}-{}.png",
                env_id,
                chrono::Utc::now().timestamp_millis()
            ));
            if std::fs::write(&path, bytes).is_ok() {
                Some(path.to_string_lossy().to_string())
            } else {
                None
            }
        } else {
            None
        }
    } else {
        None
    };
    Ok((observed, screenshot_path.unwrap_or_default()))
}
