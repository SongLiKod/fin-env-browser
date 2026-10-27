use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::{
    db::Db,
    error::{AppError, Result},
    models::{now_iso, Proxy, ProxyCheckResult},
};

/// Check a proxy end to end: TCP reachability, then an HTTPS request through
/// the proxy to resolve the exit IP, country and timezone.
pub async fn check_proxy(
    db: &Db,
    proxy: &Proxy,
    password: Option<&str>,
) -> Result<ProxyCheckResult> {
    let checked_at = now_iso();
    let connect_start = Instant::now();
    let tcp = timeout(
        Duration::from_secs(8),
        TcpStream::connect((proxy.host.as_str(), proxy.port)),
    )
    .await;
    match tcp {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            let result = ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(connect_start.elapsed().as_millis() as u64),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: format!("TCP 连接失败: {e}"),
                checked_at,
            };
            db.save_proxy_check(&result)?;
            return Ok(result);
        }
        Err(_) => {
            let result = ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(8000),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: "TCP 连接超时".into(),
                checked_at,
            };
            db.save_proxy_check(&result)?;
            return Ok(result);
        }
    }
    let tcp_latency = connect_start.elapsed().as_millis() as u64;

    let proxy_url = match password.filter(|p| !p.is_empty()) {
        Some(password) => build_proxy_url_with_auth(proxy, password),
        None => build_proxy_url(proxy),
    };
    let proxy_url = match proxy_url {
        Ok(url) => url,
        Err(e) => {
            return Ok(ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(tcp_latency),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: e.message().to_string(),
                checked_at,
            })
        }
    };
    let reqwest_proxy = reqwest::Proxy::all(&proxy_url)
        .map_err(|e| AppError::bad_request(format!("代理 URL 无效: {e}")))?;
    let client = reqwest::Client::builder()
        .proxy(reqwest_proxy)
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinEnv/1.0")
        .build()
        .map_err(AppError::from)?;

    let http_start = Instant::now();
    let mut message = String::from("代理连通");
    let mut exit_ip = None;
    let mut country = None;
    let mut country_code = None;
    let mut timezone = None;
    let mut city = None;

    match client.get("https://ipwho.is/").send().await {
        Ok(response) => match response.json::<Value>().await {
            Ok(value) => {
                if value.get("success").and_then(|v| v.as_bool()) == Some(false) {
                    message = value
                        .get("message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("出口信息解析失败")
                        .to_string();
                } else {
                    exit_ip = value
                        .get("ip")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    country = value
                        .get("country")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    country_code = value
                        .get("country_code")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    timezone = value
                        .get("timezone")
                        .and_then(|v| v.get("id"))
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string)
                        .or_else(|| {
                            value
                                .get("timezone")
                                .and_then(|v| v.as_str())
                                .map(ToString::to_string)
                        });
                    city = value
                        .get("city")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                }
            }
            Err(e) => message = format!("出口信息 JSON 解析失败: {e}"),
        },
        Err(e) => {
            message = format!("代理 HTTP 请求失败: {e}");
        }
    }

    if exit_ip.is_none() {
        if let Ok(response) = client.get("http://ip-api.com/json/?fields=status,message,country,countryCode,timezone,city,query").send().await {
            if let Ok(value) = response.json::<Value>().await {
                if value.get("status").and_then(|v| v.as_str()) == Some("success") {
                    exit_ip = value.get("query").and_then(|v| v.as_str()).map(ToString::to_string);
                    country = value.get("country").and_then(|v| v.as_str()).map(ToString::to_string);
                    country_code = value
                        .get("countryCode")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    timezone = value
                        .get("timezone")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    city = value.get("city").and_then(|v| v.as_str()).map(ToString::to_string);
                    message = "代理连通（备用出口检测）".into();
                }
            }
        }
    }

    let ok = exit_ip.is_some();
    if !ok && message == "代理连通" {
        message = "代理可连接，但未能解析出口 IP".into();
    }
    let result = ProxyCheckResult {
        proxy_id: proxy.id.clone(),
        ok,
        latency_ms: Some(tcp_latency + http_start.elapsed().as_millis() as u64),
        exit_ip,
        country: country_code.clone().or(country),
        timezone: timezone.clone().or_else(|| {
            country_code
                .as_deref()
                .map(crate::fingerprint::timezone_for_country)
        }),
        city,
        message,
        checked_at,
    };
    db.save_proxy_check(&result)?;
    Ok(result)
}

fn build_proxy_url(proxy: &Proxy) -> Result<String> {
    let scheme = match proxy.protocol.as_str() {
        "http" => "http",
        "https" => "https",
        "socks5" => "socks5",
        other => return Err(AppError::bad_request(format!("不支持的代理协议: {other}"))),
    };
    let mut url = url::Url::parse(&format!("{scheme}://{}:{}", proxy.host, proxy.port))
        .map_err(|e| AppError::bad_request(format!("代理地址无效: {e}")))?;
    if !proxy.username.is_empty() {
        url.set_username(&proxy.username)
            .map_err(|_| AppError::bad_request("代理用户名无效"))?;
        // The password itself is supplied by the caller through Proxy::basic_auth
        // when present; keeping it out of the URL avoids logging it.
    }
    Ok(url.to_string())
}

/// Same as `build_proxy_url` but with decrypted credentials embedded. Used by
/// the connectivity checker only, never persisted.
pub fn build_proxy_url_with_auth(proxy: &Proxy, password: &str) -> Result<String> {
    let scheme = match proxy.protocol.as_str() {
        "http" => "http",
        "https" => "https",
        "socks5" => "socks5",
        other => return Err(AppError::bad_request(format!("不支持的代理协议: {other}"))),
    };
    let mut url = url::Url::parse(&format!("{scheme}://{}:{}", proxy.host, proxy.port))
        .map_err(|e| AppError::bad_request(format!("代理地址无效: {e}")))?;
    if !proxy.username.is_empty() {
        url.set_username(&proxy.username)
            .map_err(|_| AppError::bad_request("代理用户名无效"))?;
        url.set_password(Some(password))
            .map_err(|_| AppError::bad_request("代理密码无效"))?;
    }
    Ok(url.to_string())
}
