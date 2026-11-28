use std::collections::{HashMap, HashSet};
use std::time::Duration;

use chrono::{DateTime, Utc};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use serde_json::json;
use tauri::Emitter;

use crate::{
    audit_log::{actions, Audit},
    error::{AppError, Result},
    models::{new_id, now_iso, Proxy, ProxyImportOptions, ProxyRefreshResult, ProxySubscription},
    proxy_import,
    state::SharedState,
};

const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

pub fn start_scheduler(state: SharedState) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
            if let Err(error) = refresh_due_subscriptions(&state).await {
                tracing::warn!(error = %error.message(), "proxy subscription scheduler failed");
            }
        }
    });
}

pub async fn refresh_due_subscriptions(state: &SharedState) -> Result<()> {
    let subscriptions = state.db.list_auto_subscriptions()?;
    let mut refreshed = 0u32;
    for subscription in subscriptions {
        if refreshed >= 3 {
            break;
        }
        if !is_due(&subscription) {
            continue;
        }
        refreshed += 1;
        if let Err(error) = refresh(state, &subscription.id, "system").await {
            tracing::warn!(
                subscription = %subscription.name,
                error = %error.message(),
                "proxy subscription auto refresh failed"
            );
        }
    }
    Ok(())
}

fn is_due(subscription: &ProxySubscription) -> bool {
    if !subscription.enabled || !subscription.auto_refresh {
        return false;
    }
    let Some(last) = subscription.last_refresh_at.as_deref() else {
        return true;
    };
    DateTime::parse_from_rfc3339(last)
        .map(|time| {
            Utc::now()
                .signed_duration_since(time.with_timezone(&Utc))
                .num_minutes()
                >= subscription.refresh_interval_min as i64
        })
        .unwrap_or(true)
}

pub async fn refresh(
    state: &SharedState,
    subscription_id: &str,
    actor: &str,
) -> Result<ProxyRefreshResult> {
    let subscription = state.db.get_subscription(subscription_id)?;
    state.proxy_jobs.begin_refresh(subscription_id)?;
    let outcome = refresh_inner(state, &subscription, actor).await;
    state.proxy_jobs.end_refresh(subscription_id);
    match outcome {
        Ok(result) => {
            state.db.set_subscription_refresh_result(
                subscription_id,
                "ok",
                &result.message,
                &now_iso(),
            )?;
            emit_changed(state, "subscription_refresh", subscription_id);
            Ok(result)
        }
        Err(error) => {
            let _ = state.db.set_subscription_refresh_result(
                subscription_id,
                "error",
                error.message(),
                &now_iso(),
            );
            emit_changed(state, "subscription_error", subscription_id);
            Err(error)
        }
    }
}

async fn refresh_inner(
    state: &SharedState,
    subscription: &ProxySubscription,
    actor: &str,
) -> Result<ProxyRefreshResult> {
    let text = fetch_subscription(subscription).await?;
    let options = ProxyImportOptions {
        default_protocol: Some(subscription.default_protocol.clone()),
        enabled: Some(true),
        group_name: Some(subscription.group_name.clone()),
        tags: Some(subscription.tags.clone()),
        name_prefix: None,
        on_duplicate: Some("skip".into()),
    };
    let parsed = proxy_import::parse_proxy_text(&text, &options);
    if parsed.entries.is_empty() {
        return Err(AppError::bad_request(
            "订阅内容中没有可用的 HTTP/HTTPS/SOCKS5 代理",
        ));
    }
    if parsed.entries.len() > 5000 {
        return Err(AppError::bad_request("单个订阅最多支持 5000 个节点"));
    }

    let existing = state.db.list_subscription_proxies(&subscription.id)?;
    let existing_map = existing
        .iter()
        .map(|proxy| (proxy.endpoint_key.clone(), proxy.clone()))
        .collect::<HashMap<_, _>>();
    let feed_keys = parsed
        .entries
        .iter()
        .filter(|entry| !entry.duplicate)
        .map(|entry| {
            proxy_import::endpoint_key(&entry.protocol, &entry.host, entry.port, &entry.username)
        })
        .collect::<HashSet<_>>();
    let all_existing = state
        .db
        .find_proxies_by_endpoint_keys(&feed_keys.iter().cloned().collect::<Vec<_>>())?
        .into_iter()
        .map(|proxy| (proxy.endpoint_key.clone(), proxy))
        .collect::<HashMap<_, _>>();

    let now = now_iso();
    let mut inserts = Vec::new();
    let mut updates = Vec::new();
    let mut added = 0u32;
    let mut updated = 0u32;
    let mut unchanged = 0u32;
    let mut disabled = 0u32;
    let mut restored = 0u32;
    let mut skipped = 0u32;
    let mut handled = HashSet::new();

    for entry in &parsed.entries {
        if entry.duplicate {
            skipped += 1;
            continue;
        }
        let key =
            proxy_import::endpoint_key(&entry.protocol, &entry.host, entry.port, &entry.username);
        if !handled.insert(key.clone()) {
            skipped += 1;
            continue;
        }
        if let Some(existing) = existing_map.get(&key) {
            let mut proxy = existing.clone();
            let mut changed = false;
            if proxy.protocol != entry.protocol {
                proxy.protocol = entry.protocol.clone();
                changed = true;
            }
            if proxy.username != entry.username {
                proxy.username = entry.username.clone();
                changed = true;
            }
            if let Some(password) = entry.password.as_deref() {
                if !password.is_empty() {
                    let current = state
                        .crypto
                        .decrypt(&proxy.password_enc)
                        .unwrap_or_default();
                    if current != password {
                        proxy.password_enc = state.crypto.encrypt(password)?;
                        proxy.has_password = true;
                        proxy.last_check_ok = None;
                        proxy.last_check_message = Some("凭据已更新，待重新检测".into());
                        proxy.fail_count = 0;
                        changed = true;
                    }
                }
            }
            if !proxy.enabled && proxy.disabled_reason == "subscription_missing" {
                proxy.enabled = true;
                proxy.disabled_reason.clear();
                restored += 1;
                changed = true;
            }
            if changed {
                proxy.endpoint_key = key;
                proxy.updated_at = now.clone();
                updates.push(proxy);
                updated += 1;
            } else {
                unchanged += 1;
            }
            continue;
        }
        if all_existing.contains_key(&key) {
            skipped += 1;
            continue;
        }
        let password = entry.password.clone().unwrap_or_default();
        let mut proxy = Proxy {
            id: new_id(),
            name: entry.name.clone(),
            protocol: entry.protocol.clone(),
            host: entry.host.clone(),
            port: entry.port,
            username: entry.username.clone(),
            password_enc: if password.is_empty() {
                String::new()
            } else {
                state.crypto.encrypt(&password)?
            },
            has_password: !password.is_empty(),
            enabled: true,
            last_check_at: None,
            latency_ms: None,
            exit_ip: None,
            exit_country: None,
            exit_timezone: None,
            group_name: subscription.group_name.clone(),
            tags: subscription.tags.clone(),
            note: String::new(),
            source: "subscription".into(),
            subscription_id: Some(subscription.id.clone()),
            weight: 1,
            last_check_ok: None,
            last_check_message: None,
            last_check_target: String::new(),
            last_check_city: None,
            fail_count: 0,
            disabled_reason: String::new(),
            endpoint_key: String::new(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        proxy.endpoint_key = proxy.endpoint_key();
        inserts.push(proxy);
        added += 1;
    }

    for proxy in &existing {
        if feed_keys.contains(&proxy.endpoint_key) {
            continue;
        }
        if !proxy.enabled && proxy.disabled_reason == "subscription_missing" {
            continue;
        }
        if !proxy.enabled {
            skipped += 1;
            continue;
        }
        let mut proxy = proxy.clone();
        proxy.enabled = false;
        proxy.disabled_reason = "subscription_missing".into();
        proxy.updated_at = now.clone();
        updates.push(proxy);
        disabled += 1;
    }

    let (inserted, _updated_rows) = state.db.apply_proxy_import(&inserts, &updates)?;
    let result = ProxyRefreshResult {
        subscription_id: subscription.id.clone(),
        added: inserted,
        updated,
        unchanged,
        disabled,
        restored,
        skipped,
        unsupported: parsed.unsupported,
        total_in_feed: feed_keys.len() as u32,
        message: format!(
            "订阅刷新完成：新增 {added}，更新 {updated}，停用 {disabled}，恢复 {restored}，跳过 {skipped}"
        ),
    };
    let _ = Audit::global(
        &state.db,
        actor,
        actions::PROXY_SUBSCRIPTION_REFRESH,
        format!(
            "刷新订阅 {}：新增 {}，更新 {}，停用 {}，恢复 {}，跳过 {}，不支持 {}",
            subscription.name,
            result.added,
            result.updated,
            result.disabled,
            result.restored,
            result.skipped,
            result.unsupported
        ),
    );
    Ok(result)
}

async fn fetch_subscription(subscription: &ProxySubscription) -> Result<String> {
    let target = raw_content_url(&subscription.url).unwrap_or_else(|| subscription.url.clone());
    let url = url::Url::parse(&target).map_err(|_| AppError::bad_request("订阅 URL 无效"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AppError::bad_request("订阅 URL 仅支持 http/https"));
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("FinEnv/1.0 (+proxy-subscription)")
        .build()
        .map_err(AppError::from)?;
    let username = url.username().to_string();
    let password = url.password().map(ToString::to_string);
    let mut request = client.get(url);
    if !username.is_empty() {
        request = request.basic_auth(username, password);
    }
    if let Some(headers) = subscription
        .headers
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        request = request.headers(parse_headers(headers)?);
    }
    let response = request
        .send()
        .await
        .map_err(|e| AppError::bad_request(format!("订阅请求失败: {}", e.without_url())))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "订阅返回 HTTP {}",
            response.status()
        )));
    }
    if response.content_length().unwrap_or(0) > MAX_BODY_BYTES as u64 {
        return Err(AppError::bad_request("订阅响应超过 4MB 限制"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::bad_request(format!("订阅内容读取失败: {e}")))?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(AppError::bad_request("订阅响应超过 4MB 限制"));
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

pub fn validate_headers(value: &str) -> Result<()> {
    parse_headers(value).map(|_| ())
}

fn parse_headers(value: &str) -> Result<HeaderMap> {
    let parsed: serde_json::Value = serde_json::from_str(value)
        .map_err(|_| AppError::bad_request("订阅请求头必须是 JSON 对象"))?;
    let object = parsed
        .as_object()
        .ok_or_else(|| AppError::bad_request("订阅请求头必须是 JSON 对象"))?;
    if object.len() > 20 {
        return Err(AppError::bad_request("订阅请求头不能超过 20 个"));
    }
    let mut headers = HeaderMap::new();
    for (name, value) in object {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| AppError::bad_request(format!("请求头名称无效: {name}")))?;
        let value = value
            .as_str()
            .ok_or_else(|| AppError::bad_request(format!("请求头 {name} 的值必须是字符串")))?;
        let value = HeaderValue::from_str(value)
            .map_err(|_| AppError::bad_request(format!("请求头 {name} 的值无效")))?;
        headers.insert(name, value);
    }
    Ok(headers)
}

fn emit_changed(state: &SharedState, reason: &str, subscription_id: &str) {
    let Some(app) = state.app_handle() else {
        return;
    };
    let _ = app.emit(
        "proxy://changed",
        json!({ "reason": reason, "subscriptionId": subscription_id }),
    );
}

/// GitHub 网页版文件链接（/blob/）返回的是 HTML，订阅和在线抓取都需要
/// 自动转换为 raw 文件地址才能直接解析代理列表。
pub fn raw_content_url(value: &str) -> Option<String> {
    let url = url::Url::parse(value).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    if host != "github.com" && host != "www.github.com" {
        return None;
    }
    let segments = url.path_segments()?.collect::<Vec<_>>();
    if segments.len() < 5 || segments[2] != "blob" {
        return None;
    }
    let owner = segments[0];
    let repo = segments[1];
    let branch = segments[3];
    let path = segments[4..].join("/");
    if owner.is_empty() || repo.is_empty() || branch.is_empty() || path.is_empty() {
        return None;
    }
    Some(format!(
        "https://raw.githubusercontent.com/{owner}/{repo}/{branch}/{path}"
    ))
}

#[cfg(test)]
mod tests {
    use super::raw_content_url;

    #[test]
    fn converts_github_blob_links_to_raw() {
        assert_eq!(
            raw_content_url("https://github.com/HankNovic/ProxyClean/blob/main/SOCKS5.txt")
                .as_deref(),
            Some("https://raw.githubusercontent.com/HankNovic/ProxyClean/main/SOCKS5.txt")
        );
        assert!(raw_content_url(
            "https://raw.githubusercontent.com/HankNovic/ProxyClean/main/SOCKS5.txt"
        )
        .is_none());
        assert!(raw_content_url("https://github.com/HankNovic/ProxyClean").is_none());
    }
}
