use std::{
    collections::{HashMap, HashSet},
    sync::{OnceLock, RwLock},
    time::{Duration, Instant},
};

use base64::{engine::general_purpose::STANDARD as B64, Engine};
use futures_util::{stream, StreamExt};
use regex::Regex;
use serde_json::Value;

use crate::{
    audit_log::{actions, Audit},
    error::{AppError, Result},
    models::{
        new_id, now_iso, Proxy, ProxyCheckResult, ProxyDiscoveryAddInput,
        ProxyDiscoveryCandidate, ProxyDiscoveryInput, ProxyDiscoveryResult,
        ProxyDiscoverySourceStatus, ProxyImportError, ProxyImportOptions, ProxyImportResult,
        PROXY_PROTOCOLS,
    },
    proxy_check::check_proxy_core,
    proxy_import,
    proxy_subscription::raw_content_url,
    state::SharedState,
};

const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;
const MAX_PER_PAGE: usize = 500;
const STORE_TTL: Duration = Duration::from_secs(30 * 60);

#[derive(Debug, Clone)]
pub struct DiscoveredCandidate {
    pub candidate: ProxyDiscoveryCandidate,
    pub password: Option<String>,
    pub endpoint_key: String,
    pub created_at: Instant,
}

pub struct ProxyDiscoveryStore {
    inner: RwLock<HashMap<String, DiscoveredCandidate>>,
}

impl Default for ProxyDiscoveryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ProxyDiscoveryStore {
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(HashMap::new()),
        }
    }

    pub fn insert(&self, candidate: DiscoveredCandidate) {
        if let Ok(mut inner) = self.inner.write() {
            inner.insert(candidate.candidate.id.clone(), candidate);
        }
    }

    pub fn get(&self, id: &str) -> Option<DiscoveredCandidate> {
        self.inner
            .read()
            .ok()
            .and_then(|inner| inner.get(id).cloned())
    }

    pub fn cleanup(&self) {
        if let Ok(mut inner) = self.inner.write() {
            inner.retain(|_, candidate| candidate.created_at.elapsed() < STORE_TTL);
        }
    }
}

#[derive(Debug, Clone)]
enum DiscoveryTask {
    Search { engine: String, query: String },
    Github { query: String },
    Link { url: String },
}

#[derive(Debug)]
struct FoundProxy {
    parsed: proxy_import::ParsedProxy,
    source_url: String,
    source_label: String,
}

#[derive(Debug)]
struct SourceOutcome {
    status: ProxyDiscoverySourceStatus,
    entries: Vec<FoundProxy>,
}

pub async fn scan(state: &SharedState, input: &ProxyDiscoveryInput) -> Result<ProxyDiscoveryResult> {
    let options = discovery_options(input);
    let follow = input.follow_links.unwrap_or(true);
    let max_results = input.max_results.unwrap_or(200).clamp(1, 500);
    let tasks = build_tasks(input);
    let client = build_client()?;

    let outcomes = stream::iter(tasks)
        .map(|task| {
            let client = client.clone();
            let options = options.clone();
            async move { run_task(&client, task, &options, follow).await }
        })
        .buffer_unordered(6)
        .collect::<Vec<_>>()
        .await;

    let mut sources = Vec::new();
    let mut scanned = 0u32;
    let mut errors = Vec::new();
    let mut drafts: HashMap<String, FoundProxy> = HashMap::new();
    for outcome in outcomes {
        if !outcome.status.ok {
            errors.push(format!("{}: {}", outcome.status.target, outcome.status.message));
        }
        scanned += outcome.status.count;
        sources.push(outcome.status);
        for entry in outcome.entries {
            let key = merge_key(&entry.parsed);
            match drafts.get(&key) {
                Some(existing)
                    if existing.parsed.password.is_none() && entry.parsed.password.is_some() =>
                {
                    drafts.insert(key, entry);
                }
                Some(_) => {}
                None => {
                    drafts.insert(key, entry);
                }
            }
        }
    }

    let mut drafts = drafts.into_iter().collect::<Vec<_>>();
    drafts.sort_by(|(_, left), (_, right)| {
        left.parsed
            .host
            .cmp(&right.parsed.host)
            .then(left.parsed.port.cmp(&right.parsed.port))
    });
    drafts.truncate(max_results as usize);

    let keys = drafts
        .iter()
        .map(|(_, draft)| {
            proxy_import::endpoint_key(
                &draft.parsed.protocol,
                &draft.parsed.host,
                draft.parsed.port,
                &draft.parsed.username,
            )
        })
        .collect::<Vec<_>>();
    let existing = state
        .db
        .find_proxies_by_endpoint_keys(&keys)?
        .into_iter()
        .map(|proxy| (proxy.endpoint_key.clone(), proxy.id))
        .collect::<HashMap<_, _>>();

    state.proxy_discovery.cleanup();
    let mut candidates = Vec::new();
    let now = Instant::now();
    for (_, draft) in drafts {
        let id = new_id();
        let endpoint = proxy_import::endpoint_key(
            &draft.parsed.protocol,
            &draft.parsed.host,
            draft.parsed.port,
            &draft.parsed.username,
        );
        let candidate = ProxyDiscoveryCandidate {
            id: id.clone(),
            name: draft.parsed.name,
            protocol: draft.parsed.protocol,
            host: draft.parsed.host,
            port: draft.parsed.port,
            username: draft.parsed.username,
            has_password: draft
                .parsed
                .password
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
            source: draft.source_label,
            source_url: draft.source_url,
            duplicate: existing.contains_key(&endpoint),
            existing_id: existing.get(&endpoint).cloned(),
        };
        state.proxy_discovery.insert(DiscoveredCandidate {
            candidate: candidate.clone(),
            password: draft.parsed.password,
            endpoint_key: endpoint,
            created_at: now,
        });
        candidates.push(candidate);
    }

    Ok(ProxyDiscoveryResult {
        candidates,
        sources,
        scanned,
        errors,
    })
}

/// Check one scanned candidate without persisting it. Results are returned to
/// the discovery dialog so users can see latency before adding nodes.
pub async fn check_candidate(
    state: &SharedState,
    candidate_id: &str,
) -> Result<ProxyCheckResult> {
    state.proxy_discovery.cleanup();
    let record = state
        .proxy_discovery
        .get(candidate_id)
        .ok_or_else(|| AppError::not_found("节点已过期，请重新扫描"))?;
    let password = record.password.clone().filter(|value| !value.is_empty());
    let proxy = Proxy {
        id: record.candidate.id.clone(),
        name: record.candidate.name.clone(),
        protocol: record.candidate.protocol.clone(),
        host: record.candidate.host.clone(),
        port: record.candidate.port,
        username: record.candidate.username.clone(),
        has_password: password.is_some(),
        enabled: true,
        ..Default::default()
    };
    Ok(check_proxy_core(&proxy, password.as_deref()).await)
}

pub fn add(
    state: &SharedState,
    input: &ProxyDiscoveryAddInput,
    actor: &str,
) -> Result<ProxyImportResult> {
    if input.candidate_ids.is_empty() {
        return Err(AppError::bad_request("请选择要添加的代理节点"));
    }
    state.proxy_discovery.cleanup();
    let on_duplicate = input
        .on_duplicate
        .as_deref()
        .filter(|value| matches!(*value, "skip" | "update" | "error"))
        .unwrap_or("skip");
    let enabled = input.enabled.unwrap_or(true);
    let group_name = input.group_name.clone().unwrap_or_default().trim().to_string();
    let tags = input.tags.clone().unwrap_or_default();
    let now = now_iso();

    let mut records = Vec::new();
    let mut seen = HashSet::new();
    for id in &input.candidate_ids {
        if !seen.insert(id.clone()) {
            continue;
        }
        let record = state
            .proxy_discovery
            .get(id)
            .ok_or_else(|| AppError::not_found("节点已过期，请重新扫描"))?;
        records.push(record);
    }

    let keys = records
        .iter()
        .map(|record| record.endpoint_key.clone())
        .collect::<Vec<_>>();
    let existing = state
        .db
        .find_proxies_by_endpoint_keys(&keys)?
        .into_iter()
        .map(|proxy| (proxy.endpoint_key.clone(), proxy))
        .collect::<HashMap<_, _>>();

    let mut inserts = Vec::new();
    let mut updates = Vec::new();
    let mut errors = Vec::new();
    let mut skipped = 0u32;
    let mut proxy_ids = Vec::new();
    let mut seen_keys = HashSet::new();

    for record in records {
        if !seen_keys.insert(record.endpoint_key.clone()) {
            skipped += 1;
            continue;
        }
        if let Some(existing) = existing.get(&record.endpoint_key) {
            if on_duplicate == "error" {
                errors.push(ProxyImportError {
                    line: 0,
                    raw: format!("{}:{}", record.candidate.host, record.candidate.port),
                    message: "代理已存在".into(),
                });
                skipped += 1;
                continue;
            }
            if on_duplicate == "skip" {
                skipped += 1;
                continue;
            }
            let mut proxy = existing.clone();
            proxy.protocol = record.candidate.protocol.clone();
            proxy.username = record.candidate.username.clone();
            if let Some(password) = record.password.as_deref().filter(|value| !value.is_empty()) {
                proxy.password_enc = state.crypto.encrypt(password)?;
                proxy.has_password = true;
                proxy.last_check_ok = None;
                proxy.last_check_message = Some("凭据已更新，待重新检测".into());
                proxy.fail_count = 0;
            }
            proxy.endpoint_key = record.endpoint_key.clone();
            proxy.updated_at = now.clone();
            proxy_ids.push(proxy.id.clone());
            updates.push(proxy);
            continue;
        }
        let mut proxy = Proxy {
            id: new_id(),
            name: record.candidate.name.clone(),
            protocol: record.candidate.protocol.clone(),
            host: record.candidate.host.clone(),
            port: record.candidate.port,
            username: record.candidate.username.clone(),
            password_enc: match record.password.as_deref().filter(|value| !value.is_empty()) {
                Some(password) => state.crypto.encrypt(password)?,
                None => String::new(),
            },
            has_password: record
                .password
                .as_deref()
                .is_some_and(|value| !value.is_empty()),
            enabled,
            last_check_at: None,
            latency_ms: None,
            exit_ip: None,
            exit_country: None,
            exit_timezone: None,
            group_name: group_name.clone(),
            tags: tags.clone(),
            note: format!("在线发现：{}", record.candidate.source),
            source: "discovery".into(),
            subscription_id: None,
            weight: 1,
            last_check_ok: None,
            last_check_message: None,
            last_check_city: None,
            fail_count: 0,
            disabled_reason: if enabled {
                String::new()
            } else {
                "manual".into()
            },
            endpoint_key: record.endpoint_key.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        proxy.endpoint_key = proxy.endpoint_key();
        proxy_ids.push(proxy.id.clone());
        inserts.push(proxy);
    }

    let (inserted, updated) = state.db.apply_proxy_import(&inserts, &updates)?;
    let result = ProxyImportResult {
        total: (inserted + updated + skipped) as u32,
        valid: (inserted + updated + skipped) as u32,
        inserted,
        updated,
        skipped,
        failed: errors.len() as u32,
        errors,
        proxy_ids,
    };
    let _ = Audit::global(
        &state.db,
        actor,
        actions::PROXY_DISCOVERY_ADD,
        format!(
            "在线发现导入代理：新增 {}，更新 {}，跳过 {}，失败 {}",
            result.inserted, result.updated, result.skipped, result.failed
        ),
    );
    Ok(result)
}

fn discovery_options(input: &ProxyDiscoveryInput) -> ProxyImportOptions {
    let protocol = input
        .default_protocol
        .as_deref()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| PROXY_PROTOCOLS.contains(&value.as_str()))
        .unwrap_or_else(|| "http".into());
    ProxyImportOptions {
        default_protocol: Some(protocol),
        enabled: Some(true),
        group_name: None,
        tags: None,
        name_prefix: None,
        on_duplicate: Some("skip".into()),
    }
}

fn build_tasks(input: &ProxyDiscoveryInput) -> Vec<DiscoveryTask> {
    let mut tasks = Vec::new();
    let queries = input
        .search_queries
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let engines = input
        .engines
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "bing" | "duckduckgo" | "google"))
        .collect::<Vec<_>>();
    for engine in &engines {
        for query in &queries {
            tasks.push(DiscoveryTask::Search {
                engine: engine.clone(),
                query: query.clone(),
            });
        }
    }
    for query in &input.github_queries {
        if !query.trim().is_empty() {
            tasks.push(DiscoveryTask::Github {
                query: query.trim().to_string(),
            });
        }
    }
    for link in &input.friend_links {
        if link.trim().is_empty() {
            continue;
        }
        tasks.push(DiscoveryTask::Link {
            url: link.trim().to_string(),
        });
    }
    if tasks.is_empty() {
        tasks.push(DiscoveryTask::Search {
            engine: "bing".into(),
            query: "free proxy list".into(),
        });
        tasks.push(DiscoveryTask::Github {
            query: "free proxy list".into(),
        });
    }
    tasks.truncate(30);
    tasks
}

async fn run_task(
    client: &reqwest::Client,
    task: DiscoveryTask,
    options: &ProxyImportOptions,
    follow: bool,
) -> SourceOutcome {
    match task {
        DiscoveryTask::Search { engine, query } => {
            let url = search_url(&engine, &query);
            let label = match engine.as_str() {
                "google" => "Google",
                "duckduckgo" => "DuckDuckGo",
                _ => "Bing",
            };
            run_page(client, &url, label, options, follow, 5).await
        }
        DiscoveryTask::Github { query } => run_github(client, &query, options, follow).await,
        DiscoveryTask::Link { url } => run_page(client, &url, "友情链接", options, follow, 8).await,
    }
}

async fn run_page(
    client: &reqwest::Client,
    url: &str,
    label: &str,
    options: &ProxyImportOptions,
    follow: bool,
    follow_limit: usize,
) -> SourceOutcome {
    let mut status = ProxyDiscoverySourceStatus {
        kind: label.into(),
        target: url.to_string(),
        ok: true,
        count: 0,
        message: String::new(),
    };
    let text = match fetch_text(client, url, None).await {
        Ok(text) => text,
        Err(error) => {
            status.ok = false;
            status.message = error.message().to_string();
            return SourceOutcome {
                status,
                entries: Vec::new(),
            };
        }
    };
    let mut entries = extract_proxies(&text, options)
        .into_iter()
        .map(|parsed| FoundProxy {
            parsed,
            source_url: url.to_string(),
            source_label: label.to_string(),
        })
        .collect::<Vec<_>>();
    if follow {
        let links = extract_links(&text, url)
            .into_iter()
            .take(follow_limit)
            .collect::<Vec<_>>();
        let fetched = fetch_pages(client, links).await;
        for (link, page) in fetched {
            if let Ok(page) = page {
                entries.extend(
                    extract_proxies(&page, options)
                        .into_iter()
                        .map(|parsed| FoundProxy {
                            parsed,
                            source_url: link.clone(),
                            source_label: label.to_string(),
                        }),
                );
            }
        }
    }
    status.count = entries.len() as u32;
    if entries.is_empty() {
        status.message = "未发现可用代理".into();
    }
    SourceOutcome { status, entries }
}

async fn run_github(
    client: &reqwest::Client,
    query: &str,
    options: &ProxyImportOptions,
    follow: bool,
) -> SourceOutcome {
    let mut status = ProxyDiscoverySourceStatus {
        kind: "GitHub".into(),
        target: query.to_string(),
        ok: true,
        count: 0,
        message: String::new(),
    };
    let mut api = match url::Url::parse("https://api.github.com/search/repositories") {
        Ok(url) => url,
        Err(_) => {
            status.ok = false;
            status.message = "GitHub API 地址无效".into();
            return SourceOutcome {
                status,
                entries: Vec::new(),
            };
        }
    };
    api.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("sort", "updated")
        .append_pair("per_page", "5");
    let body = match fetch_text(
        client,
        api.as_str(),
        Some("application/vnd.github+json"),
    )
    .await
    {
        Ok(body) => body,
        Err(error) => {
            status.ok = false;
            status.message = error.message().to_string();
            return SourceOutcome {
                status,
                entries: Vec::new(),
            };
        }
    };
    let value: Value = match serde_json::from_str(&body) {
        Ok(value) => value,
        Err(_) => {
            status.ok = false;
            status.message = "GitHub 返回内容不是 JSON（可能触发限流）".into();
            return SourceOutcome {
                status,
                entries: Vec::new(),
            };
        }
    };
    let items = value
        .get("items")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    if items.is_empty() {
        if let Some(message) = value.get("message").and_then(|value| value.as_str()) {
            status.ok = false;
            status.message = message.to_string();
        } else {
            status.message = "未找到仓库".into();
        }
        return SourceOutcome {
            status,
            entries: Vec::new(),
        };
    }

    let mut entries = Vec::new();
    for item in items.iter().take(5) {
        let Some(full_name) = item.get("full_name").and_then(|value| value.as_str()) else {
            continue;
        };
        let branch = item
            .get("default_branch")
            .and_then(|value| value.as_str())
            .unwrap_or("main");
        let raw = format!("https://raw.githubusercontent.com/{full_name}/{branch}/README.md");
        let Ok(text) = fetch_text(client, &raw, None).await else {
            continue;
        };
        entries.extend(
            extract_proxies(&text, options)
                .into_iter()
                .map(|parsed| FoundProxy {
                    parsed,
                    source_url: raw.clone(),
                    source_label: "GitHub".into(),
                }),
        );
        if follow {
            let links = extract_links(&text, &raw)
                .into_iter()
                .take(3)
                .collect::<Vec<_>>();
            let fetched = fetch_pages(client, links).await;
            for (link, page) in fetched {
                if let Ok(page) = page {
                    entries.extend(
                        extract_proxies(&page, options)
                            .into_iter()
                            .map(|parsed| FoundProxy {
                                parsed,
                                source_url: link.clone(),
                                source_label: "GitHub".into(),
                            }),
                    );
                }
            }
        }
    }
    status.count = entries.len() as u32;
    if entries.is_empty() {
        status.message = "仓库 README 未发现可用代理".into();
    }
    SourceOutcome { status, entries }
}

fn search_url(engine: &str, query: &str) -> String {
    let (base, params) = match engine {
        "google" => (
            "https://www.google.com/search",
            vec![("q", query), ("num", "30")],
        ),
        "duckduckgo" => (
            "https://html.duckduckgo.com/html/",
            vec![("q", query)],
        ),
        _ => (
            "https://www.bing.com/search",
            vec![("q", query), ("count", "30")],
        ),
    };
    let mut url = url::Url::parse(base).expect("static search url");
    {
        let mut pairs = url.query_pairs_mut();
        for (key, value) in params {
            pairs.append_pair(key, value);
        }
    }
    url.to_string()
}

fn build_client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) FinEnv/1.0")
        .build()
        .map_err(AppError::from)
}

async fn fetch_text(
    client: &reqwest::Client,
    url: &str,
    accept: Option<&str>,
) -> Result<String> {
    let target = raw_content_url(url).unwrap_or_else(|| url.to_string());
    let parsed = url::Url::parse(&target).map_err(|_| AppError::bad_request("抓取 URL 无效"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(AppError::bad_request("仅支持 http/https 抓取"));
    }
    let mut request = client.get(parsed);
    if let Some(accept) = accept {
        request = request.header(reqwest::header::ACCEPT, accept);
    }
    let response = request
        .send()
        .await
        .map_err(|e| AppError::bad_request(format!("抓取请求失败: {}", e.without_url())))?;
    if !response.status().is_success() {
        return Err(AppError::bad_request(format!(
            "抓取返回 HTTP {}",
            response.status()
        )));
    }
    if response.content_length().unwrap_or(0) > MAX_PAGE_BYTES as u64 {
        return Err(AppError::bad_request("页面超过 2MB 限制"));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|e| AppError::bad_request(format!("页面读取失败: {}", e.without_url())))?;
    if bytes.len() > MAX_PAGE_BYTES {
        return Err(AppError::bad_request("页面超过 2MB 限制"));
    }
    Ok(String::from_utf8_lossy(&bytes).to_string())
}

async fn fetch_pages(
    client: &reqwest::Client,
    links: Vec<String>,
) -> Vec<(String, Result<String>)> {
    stream::iter(links)
        .map(|link| {
            let client = client.clone();
            async move {
                let result = fetch_text(&client, &link, None).await;
                (link, result)
            }
        })
        .buffer_unordered(4)
        .collect()
        .await
}

fn merge_key(parsed: &proxy_import::ParsedProxy) -> String {
    format!(
        "{}://{}:{}",
        parsed.protocol.to_ascii_lowercase(),
        parsed.host.to_ascii_lowercase(),
        parsed.port
    )
}

fn extract_proxies(text: &str, options: &ProxyImportOptions) -> Vec<proxy_import::ParsedProxy> {
    let text = clean_text(text);
    let mut found: HashMap<String, proxy_import::ParsedProxy> = HashMap::new();
    for regex in proxy_regexes() {
        for matched in regex.find_iter(&text) {
            let prefix = &text[..matched.start()];
            if prefix.ends_with("//") || prefix.ends_with('@') {
                continue;
            }
            let candidate = trim_candidate(matched.as_str());
            if candidate.len() < 7 || candidate.len() > 512 {
                continue;
            }
            if candidate.contains("://") {
                let Ok(url) = url::Url::parse(candidate) else {
                    continue;
                };
                if url.port().is_none() || !(url.path().is_empty() || url.path() == "/") {
                    continue;
                }
            }
            let Some(parsed) = proxy_import::parse_proxy_line(candidate, options) else {
                continue;
            };
            if should_skip_proxy_host(&parsed.host) {
                continue;
            }
            let key = merge_key(&parsed);
            match found.get(&key) {
                Some(existing)
                    if existing.password.is_none() && parsed.password.is_some() =>
                {
                    found.insert(key, parsed);
                }
                Some(_) => {}
                None => {
                    found.insert(key, parsed);
                }
            }
            if found.len() >= MAX_PER_PAGE {
                break;
            }
        }
        if found.len() >= MAX_PER_PAGE {
            break;
        }
    }
    found.into_values().collect()
}

fn extract_links(html: &str, base: &str) -> Vec<String> {
    let mut links = Vec::new();
    let mut seen = HashSet::new();
    for regex in link_regexes() {
        for capture in regex.captures_iter(html) {
            let Some(value) = capture.get(1).map(|match_| match_.as_str()) else {
                continue;
            };
            let Some(link) = resolve_link(base, value) else {
                continue;
            };
            if seen.insert(link.clone()) {
                links.push(link);
            }
        }
    }
    links
}

fn resolve_link(base: &str, href: &str) -> Option<String> {
    let href = decode_entities(href).trim().to_string();
    if href.is_empty() || href.starts_with('#') || href.starts_with("javascript:") {
        return None;
    }
    let candidate = if href.starts_with("//") {
        format!("https:{href}")
    } else {
        href
    };
    let absolute = if candidate.starts_with("http://") || candidate.starts_with("https://") {
        candidate
    } else if let Ok(url) = url::Url::parse(&candidate) {
        if let Some(value) = url
            .query_pairs()
            .find(|(key, _)| key == "uddg" || key == "q")
            .map(|(_, value)| value.to_string())
            .filter(|value| value.starts_with("http"))
        {
            value
        } else if url.path() == "/ck/a" {
            let value = url
                .query_pairs()
                .find(|(key, _)| key == "u")
                .map(|(_, value)| value.to_string())?;
            decode_bing_url(&value)?
        } else {
            url::Url::parse(base).ok()?.join(&candidate).ok()?.to_string()
        }
    } else {
        url::Url::parse(base).ok()?.join(&candidate).ok()?.to_string()
    };
    let parsed = url::Url::parse(&absolute).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return None;
    }
    if parsed.path() == "/" && parsed.port().is_some_and(|port| port != 80 && port != 443) {
        return None;
    }
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    if host.is_empty() || should_skip_host(&host) || host.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let path = parsed.path().to_ascii_lowercase();
    if [".css", ".js", ".png", ".jpg", ".jpeg", ".gif", ".svg", ".ico", ".woff", ".woff2", ".pdf", ".zip"]
        .iter()
        .any(|extension| path.ends_with(extension))
    {
        return None;
    }
    Some(parsed.to_string())
}

fn decode_bing_url(value: &str) -> Option<String> {
    let raw = value.strip_prefix("a1")?;
    let mut encoded = raw.replace('-', "+").replace('_', "/");
    while encoded.len() % 4 != 0 {
        encoded.push('=');
    }
    let bytes = B64.decode(encoded).ok()?;
    String::from_utf8(bytes).ok()
}

fn should_skip_host(host: &str) -> bool {
    [
        "bing.com",
        "google.com",
        "googleusercontent.com",
        "gstatic.com",
        "duckduckgo.com",
        "microsoft.com",
        "facebook.com",
        "twitter.com",
        "x.com",
        "youtube.com",
        "instagram.com",
        "linkedin.com",
        "w3.org",
    ]
    .iter()
    .any(|blocked| host == *blocked || host.ends_with(&format!(".{blocked}")))
}

fn should_skip_proxy_host(host: &str) -> bool {
    let host = host.to_ascii_lowercase();
    should_skip_host(&host)
        || host == "github.com"
        || host.ends_with(".github.com")
        || host == "raw.githubusercontent.com"
        || host == "gist.githubusercontent.com"
        || host == "example.com"
        || host == "localhost"
}

fn trim_candidate(value: &str) -> &str {
    value.trim_end_matches(|character: char| {
        matches!(
            character,
            ')' | ',' | '.' | ';' | '\'' | '"' | ']' | '}' | '>' | '\\'
        )
    })
}

fn proxy_regexes() -> &'static Vec<Regex> {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        [
            r#"(?i)\b(?:https?|socks5h?)://[^\s<>"'`]+"#,
            r#"\b(?:\d{1,3}\.){3}\d{1,3}:\d{1,5}(?::[^\s<>"'`]+:[^\s<>"'`]+)?"#,
            r#"(?i)\b(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}:\d{1,5}(?::[^\s<>"'`]+:[^\s<>"'`]+)?"#,
            r#"(?i)\b[^\s<>"'`@:]+:[^\s<>"'`@]+@(?:(?:\d{1,3}\.){3}\d{1,3}|(?:[a-z0-9](?:[a-z0-9-]*[a-z0-9])?\.)+[a-z]{2,}):\d{1,5}"#,
        ]
        .iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect()
    })
}

fn link_regexes() -> &'static Vec<Regex> {
    static REGEXES: OnceLock<Vec<Regex>> = OnceLock::new();
    REGEXES.get_or_init(|| {
        [
            r#"(?is)href\s*=\s*["']([^"']+)["']"#,
            r#"(?is)href\s*=\s*([^\s>]+)"#,
            r#"(?is)\]\((https?://[^)\s]+)\)"#,
            r#"(?is)(https?://[^\s<>"'`]+)"#,
        ]
        .iter()
        .filter_map(|pattern| Regex::new(pattern).ok())
        .collect()
    })
}

fn clean_text(html: &str) -> String {
    static SCRIPT: OnceLock<Regex> = OnceLock::new();
    static STYLE: OnceLock<Regex> = OnceLock::new();
    static TAG: OnceLock<Regex> = OnceLock::new();
    let script = SCRIPT.get_or_init(|| Regex::new(r"(?is)<script\b.*?</script>").unwrap());
    let style = STYLE.get_or_init(|| Regex::new(r"(?is)<style\b.*?</style>").unwrap());
    let tag = TAG.get_or_init(|| Regex::new(r"(?is)<[^>]+>").unwrap());
    let without_scripts = script.replace_all(html, " ");
    let without_styles = style.replace_all(&without_scripts, " ");
    let without_tags = tag.replace_all(&without_styles, "");
    decode_entities(&without_tags)
}

fn decode_entities(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut index = 0;
    while let Some(relative) = value[index..].find('&') {
        let start = index + relative;
        output.push_str(&value[index..start]);
        let Some(end_relative) = value[start..].find(';') else {
            output.push_str(&value[start..]);
            return output;
        };
        let end = start + end_relative;
        if end - start > 10 {
            output.push_str(&value[start..=end]);
            index = end + 1;
            continue;
        }
        let entity = &value[start + 1..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ if entity.starts_with("#x") || entity.starts_with("#X") => {
                u32::from_str_radix(&entity[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
            }
            _ if entity.starts_with('#') => entity[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32),
            _ => None,
        };
        if let Some(character) = decoded {
            output.push(character);
        } else {
            output.push_str(&value[start..=end]);
        }
        index = end + 1;
    }
    output.push_str(&value[index..]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> ProxyImportOptions {
        ProxyImportOptions {
            default_protocol: Some("http".into()),
            ..Default::default()
        }
    }

    #[test]
    fn extracts_proxies_from_mixed_html() {
        let html = r#"
            <html><script>var x = "1.1.1.1:1";</script>
            <p>US: http://user:pass@1.2.3.4:8080</p>
            <code>5.6.7.8:3128</code>
            <a href="https://example.com/list">list</a>
            </html>
        "#;
        let found = extract_proxies(html, &options());
        assert_eq!(found.len(), 2);
        assert!(found.iter().any(|proxy| proxy.host == "1.2.3.4"));
    }

    #[test]
    fn decodes_common_entities() {
        assert_eq!(
            decode_entities("a&amp;b&#x2F;c&#47;d&#64;e"),
            "a&b/c/d@e"
        );
    }
}
