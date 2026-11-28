use std::time::{Duration, Instant};

use serde_json::Value;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::{
    db::Db,
    error::{AppError, Result},
    models::{now_iso, Proxy, ProxyCheckResult},
};

/// Check a proxy end to end without persisting the result. Batch checks use
/// this and flush all results in one transaction at the end.
pub async fn check_proxy_core(
    proxy: &Proxy,
    password: Option<&str>,
    target: &str,
) -> ProxyCheckResult {
    let checked_at = now_iso();
    let target = crate::models::normalize_proxy_check_target(Some(target));
    let connect_start = Instant::now();
    let tcp = timeout(
        Duration::from_secs(8),
        TcpStream::connect((proxy.host.as_str(), proxy.port)),
    )
    .await;
    match tcp {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            return ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(connect_start.elapsed().as_millis() as u64),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: format!("TCP 连接失败: {e}"),
                target: target.clone(),
                checked_at,
            };
        }
        Err(_) => {
            return ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(8000),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: "TCP 连接超时".into(),
                target: target.clone(),
                checked_at,
            };
        }
    }
    let tcp_latency = connect_start.elapsed().as_millis() as u64;

    let proxy_url = match password.filter(|p| !p.is_empty()) {
        Some(password) => build_proxy_url_with_auth(proxy, password, &target),
        None => build_proxy_url(proxy, &target),
    };
    let proxy_url = match proxy_url {
        Ok(url) => url,
        Err(e) => {
            return ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(tcp_latency),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: e.message().to_string(),
                target: target.clone(),
                checked_at,
            }
        }
    };
    let reqwest_proxy = match reqwest::Proxy::all(&proxy_url) {
        Ok(proxy) => proxy,
        Err(e) => {
            return ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(tcp_latency),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: format!("代理 URL 无效: {e}"),
                target: target.clone(),
                checked_at,
            }
        }
    };
    let client = match reqwest::Client::builder()
        .proxy(reqwest_proxy)
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinEnv/1.0")
        .build()
    {
        Ok(client) => client,
        Err(e) => {
            return ProxyCheckResult {
                proxy_id: proxy.id.clone(),
                ok: false,
                latency_ms: Some(tcp_latency),
                exit_ip: None,
                country: None,
                timezone: None,
                city: None,
                message: format!("检测客户端创建失败: {e}"),
                target: target.clone(),
                checked_at,
            }
        }
    };

    let http_start = Instant::now();
    let socks_auth_unsupported =
        proxy.protocol.eq_ignore_ascii_case("socks5") && !proxy.username.is_empty();
    let (first_url, second_url, target_label, first_label, second_label) = if target == "github" {
        (
            "https://github.com/robots.txt",
            "https://api.github.com/",
            "GitHub",
            "GitHub 主站",
            "GitHub API",
        )
    } else {
        (
            "https://www.google.com/generate_204",
            "https://www.youtube.com/robots.txt",
            "Google / YouTube",
            "Google",
            "YouTube",
        )
    };
    let (lookup, first_ok, second_ok) = if socks_auth_unsupported {
        (lookup_exit(&client).await, false, false)
    } else {
        tokio::join!(
            lookup_exit(&client),
            probe_https(&client, first_url),
            probe_https(&client, second_url)
        )
    };
    let has_exit = lookup.ip.is_some();
    let message = if socks_auth_unsupported {
        "Chromium 不支持 SOCKS5 用户名/密码认证，浏览器无法使用该代理；请改用 HTTP 代理或本地转发"
            .into()
    } else if first_ok && second_ok {
        if has_exit {
            format!("{target_label} 可访问，出口信息解析成功")
        } else {
            format!("{target_label} 可访问，但出口 IP 解析失败")
        }
    } else if first_ok {
        format!("{first_label} 可访问，但 {second_label} 不可访问")
    } else if second_ok {
        format!("{second_label} 可访问，但 {first_label} 不可访问")
    } else if has_exit {
        format!("出口信息解析成功，但 {target_label} 不可访问")
    } else {
        format!("{}；{target_label} 不可访问", lookup.message)
    };
    ProxyCheckResult {
        proxy_id: proxy.id.clone(),
        ok: first_ok && second_ok,
        latency_ms: Some(tcp_latency + http_start.elapsed().as_millis() as u64),
        exit_ip: lookup.ip,
        country: lookup.country_code.clone().or(lookup.country),
        timezone: lookup.timezone.clone().or_else(|| {
            lookup
                .country_code
                .as_deref()
                .map(crate::fingerprint::timezone_for_country)
        }),
        city: lookup.city,
        message,
        checked_at,
        target,
    }
}

async fn probe_https(client: &reqwest::Client, url: &str) -> bool {
    matches!(
        timeout(Duration::from_secs(8), client.get(url).send()).await,
        Ok(Ok(response))
            if response.status().is_success() || response.status().is_redirection()
    )
}

/// Check and persist one proxy. Keeps the existing single-proxy behaviour.
pub async fn check_proxy(
    db: &Db,
    proxy: &Proxy,
    password: Option<&str>,
    target: &str,
) -> Result<ProxyCheckResult> {
    let result = check_proxy_core(proxy, password, target).await;
    db.save_proxy_check(&result)?;
    Ok(result)
}

pub struct ExitLookup {
    pub ip: Option<String>,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub timezone: Option<String>,
    pub message: String,
}

/// Resolve the public egress IP without a proxy. Used for environments that are
/// not bound to a proxy so the management panel can still show their real IP.
pub async fn check_direct() -> Result<(ExitLookup, u64)> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinEnv/1.0")
        .build()
        .map_err(AppError::from)?;
    let started = Instant::now();
    let lookup = lookup_exit(&client).await;
    Ok((lookup, started.elapsed().as_millis() as u64))
}

async fn lookup_exit(client: &reqwest::Client) -> ExitLookup {
    let mut message = String::from("正在解析出口 IP");
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
                    if exit_ip.is_some() {
                        message = "出口 IP 解析成功".into();
                    }
                }
            }
            Err(e) => message = format!("出口信息 JSON 解析失败: {e}"),
        },
        Err(e) => {
            message = format!("出口 HTTP 请求失败: {e}");
        }
    }

    if exit_ip.is_none() {
        if let Ok(Ok(response)) = timeout(
            Duration::from_secs(6),
            client.get("https://ipinfo.io/json").send(),
        )
        .await
        {
            if let Ok(value) = response.json::<Value>().await {
                exit_ip = value
                    .get("ip")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                country = value
                    .get("country")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                country_code = country.clone();
                timezone = value
                    .get("timezone")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                city = value
                    .get("city")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                if exit_ip.is_some() {
                    message = "备用 HTTPS 出口检测成功".into();
                }
            }
        }
    }

    if exit_ip.is_none() {
        if let Ok(Ok(response)) = timeout(
            Duration::from_secs(6),
            client.get("https://api.ipify.org?format=json").send(),
        )
        .await
        {
            if let Ok(value) = response.json::<Value>().await {
                exit_ip = value
                    .get("ip")
                    .and_then(|v| v.as_str())
                    .map(ToString::to_string);
                if exit_ip.is_some() {
                    message = "备用 HTTPS 出口检测成功".into();
                }
            }
        }
    }

    if exit_ip.is_none() {
        if let Ok(response) = client
            .get("http://ip-api.com/json/?fields=status,message,country,countryCode,timezone,city,query")
            .send()
            .await
        {
            if let Ok(value) = response.json::<Value>().await {
                if value.get("status").and_then(|v| v.as_str()) == Some("success") {
                    exit_ip = value
                        .get("query")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    country = value
                        .get("country")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    country_code = value
                        .get("countryCode")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    timezone = value
                        .get("timezone")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    city = value
                        .get("city")
                        .and_then(|v| v.as_str())
                        .map(ToString::to_string);
                    message = "备用出口检测成功".into();
                }
            }
        }
    }

    if exit_ip.is_none() && message == "正在解析出口 IP" {
        message = "无法解析出口 IP".into();
    }

    ExitLookup {
        ip: exit_ip,
        country,
        country_code,
        city,
        timezone,
        message,
    }
}

fn check_socks_scheme(target: &str) -> &'static str {
    // Google/YouTube DNS is commonly poisoned locally, so use remote DNS.
    // GitHub is reachable with local DNS and some public SOCKS5 proxies do not
    // implement remote hostname resolution, so use local DNS there.
    if target == "github" {
        "socks5"
    } else {
        "socks5h"
    }
}

fn build_proxy_url(proxy: &Proxy, target: &str) -> Result<String> {
    let scheme = match proxy.protocol.as_str() {
        "http" => "http",
        "https" => "https",
        // For the in-app checker (reqwest). Chromium's --proxy-server uses
        // Proxy::url() and keeps socks5.
        "socks5" => check_socks_scheme(target),
        other => return Err(AppError::bad_request(format!("不支持的代理协议: {other}"))),
    };
    let mut url = url::Url::parse(&format!("{scheme}://{}:{}", proxy.url_host(), proxy.port))
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
pub fn build_proxy_url_with_auth(proxy: &Proxy, password: &str, target: &str) -> Result<String> {
    let scheme = match proxy.protocol.as_str() {
        "http" => "http",
        "https" => "https",
        "socks5" => check_socks_scheme(target),
        other => return Err(AppError::bad_request(format!("不支持的代理协议: {other}"))),
    };
    let mut url = url::Url::parse(&format!("{scheme}://{}:{}", proxy.url_host(), proxy.port))
        .map_err(|e| AppError::bad_request(format!("代理地址无效: {e}")))?;
    if !proxy.username.is_empty() {
        url.set_username(&proxy.username)
            .map_err(|_| AppError::bad_request("代理用户名无效"))?;
        url.set_password(Some(password))
            .map_err(|_| AppError::bad_request("代理密码无效"))?;
    }
    Ok(url.to_string())
}
