use std::collections::HashSet;

use base64::{
    engine::general_purpose::{STANDARD as B64, URL_SAFE},
    Engine,
};
use serde_json::Value;

use crate::models::{ProxyImportError, ProxyImportOptions, PROXY_PROTOCOLS};

#[derive(Debug, Clone)]
pub struct ParsedProxy {
    pub line: usize,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub duplicate: bool,
    pub duplicate_kind: Option<String>,
    pub unsupported: bool,
}

#[derive(Debug, Default)]
pub struct ParsedImport {
    pub entries: Vec<ParsedProxy>,
    pub errors: Vec<ProxyImportError>,
    pub total_lines: u32,
    pub unsupported: u32,
}

pub fn endpoint_key(protocol: &str, host: &str, port: u16, username: &str) -> String {
    format!(
        "{}://{}:{}|{}",
        protocol.trim().to_ascii_lowercase(),
        host.trim().to_ascii_lowercase(),
        port,
        username.trim()
    )
}

fn supported(protocol: &str) -> bool {
    PROXY_PROTOCOLS.contains(&protocol)
}

fn normalize_protocol(value: &str) -> Option<String> {
    let protocol = value.trim().to_ascii_lowercase();
    match protocol.as_str() {
        "http" => Some("http".into()),
        "https" => Some("https".into()),
        "socks5" | "socks5h" => Some("socks5".into()),
        _ => None,
    }
}

fn strip_inline_comment(value: &str) -> &str {
    let mut quote = None;
    for (index, character) in value.char_indices() {
        match character {
            '"' | '\'' if quote == Some(character) => quote = None,
            '"' | '\'' if quote.is_none() => quote = Some(character),
            '#' if quote.is_none()
                && value[..index]
                    .chars()
                    .last()
                    .is_some_and(char::is_whitespace) =>
            {
                return &value[..index];
            }
            _ => {}
        }
    }
    value
}

fn strip_quotes(value: &str) -> String {
    let value = strip_inline_comment(value).trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        value[1..value.len() - 1].to_string()
    } else {
        value.to_string()
    }
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            if let Some(hex) = value.get(i + 1..i + 3) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    out.push(byte);
                    i += 3;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).to_string()
}

fn mask_line(line: &str) -> String {
    if let Some((scheme, rest)) = line.split_once("://") {
        if let Some(at) = rest.find('@') {
            return format!("{scheme}://***@{}", &rest[at + 1..]);
        }
        return line.to_string();
    }
    if let Some(at) = line.rfind('@') {
        return format!("***@{}", &line[at + 1..]);
    }
    let parts = line.split(':').collect::<Vec<_>>();
    if parts.len() > 2 {
        return format!("{}:{}:***", parts[0], parts[1]);
    }
    line.to_string()
}

fn looks_like_json(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('{') || text.starts_with('[')
}

fn looks_like_clash(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim();
        line == "proxies:" || line.starts_with("proxies:")
    })
}

fn maybe_base64(text: &str) -> Option<String> {
    let compact = text
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect::<String>();
    if compact.len() < 16 {
        return None;
    }
    if !compact
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=' | '-' | '_'))
    {
        return None;
    }
    let decoded = B64
        .decode(&compact)
        .or_else(|_| URL_SAFE.decode(&compact))
        .ok()?;
    let value = String::from_utf8(decoded).ok()?;
    (value.contains(':') || value.contains("://")).then_some(value)
}

pub fn parse_proxy_text(text: &str, options: &ProxyImportOptions) -> ParsedImport {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    if text.trim().is_empty() {
        return ParsedImport::default();
    }
    if looks_like_json(&text) {
        if let Ok(value) = serde_json::from_str::<Value>(&text) {
            return parse_json(&value, options);
        }
    }
    if looks_like_clash(&text) {
        return parse_clash(&text, options);
    }
    if let Some(decoded) = maybe_base64(&text) {
        if looks_like_json(&decoded) {
            if let Ok(value) = serde_json::from_str::<Value>(&decoded) {
                return parse_json(&value, options);
            }
        }
        if looks_like_clash(&decoded) {
            return parse_clash(&decoded, options);
        }
        return parse_lines(&decoded, options);
    }
    parse_lines(&text, options)
}

fn default_protocol(options: &ProxyImportOptions) -> String {
    options
        .default_protocol
        .as_deref()
        .and_then(normalize_protocol)
        .unwrap_or_else(|| "http".into())
}

fn build_name(
    options: &ProxyImportOptions,
    index: usize,
    explicit: Option<String>,
    host: &str,
    port: u16,
) -> String {
    if let Some(name) = explicit
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    {
        return name;
    }
    let prefix = options.name_prefix.as_deref().unwrap_or("").trim();
    if prefix.is_empty() {
        format!("{host}:{port}")
    } else {
        format!("{prefix}{index:03}")
    }
}

fn push_duplicate(entries: &mut [ParsedProxy], seen: &mut HashSet<String>, index: usize) {
    let entry = &mut entries[index];
    let key = endpoint_key(&entry.protocol, &entry.host, entry.port, &entry.username);
    if !seen.insert(key) {
        entry.duplicate = true;
        entry.duplicate_kind = Some("in_batch".into());
    }
}

fn parse_lines(text: &str, options: &ProxyImportOptions) -> ParsedImport {
    let mut result = ParsedImport::default();
    let mut line_no = 0usize;
    for raw in text.lines() {
        line_no += 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        result.total_lines += 1;
        match parse_line(line, line_no, options, result.entries.len() + 1) {
            Ok(entry) => {
                if entry.unsupported {
                    result.unsupported += 1;
                    result.errors.push(ProxyImportError {
                        line: line_no,
                        raw: mask_line(line),
                        message: format!("不支持的代理协议: {}", entry.protocol),
                    });
                } else {
                    result.entries.push(entry);
                }
            }
            Err(message) => result.errors.push(ProxyImportError {
                line: line_no,
                raw: mask_line(line),
                message,
            }),
        }
    }
    let mut seen = HashSet::new();
    let len = result.entries.len();
    for index in 0..len {
        push_duplicate(&mut result.entries, &mut seen, index);
    }
    result
}

pub fn parse_proxy_line(
    line: &str,
    options: &ProxyImportOptions,
) -> Option<ParsedProxy> {
    parse_line(line, 1, options, 1)
        .ok()
        .filter(|entry| !entry.unsupported)
}

fn parse_line(
    line: &str,
    line_no: usize,
    options: &ProxyImportOptions,
    index: usize,
) -> std::result::Result<ParsedProxy, String> {
    if line.contains("://") {
        return parse_uri(line, line_no, options, index);
    }
    if let Ok((host, rest)) = split_host_port_with_rest(line) {
        let parts = rest.splitn(3, ':').collect::<Vec<_>>();
        if parts.len() >= 2 {
            let port = parse_port(parts[0])?;
            let username = parts[1].to_string();
            let password = parts.get(2).map(|v| v.to_string());
            return finish_entry(
                line_no,
                options,
                index,
                None,
                default_protocol(options),
                host,
                port,
                username,
                password,
            );
        }
    }
    if let Some((userinfo, hostport)) = line.rsplit_once('@') {
        let (username, password) = userinfo
            .split_once(':')
            .map(|(u, p)| (u.to_string(), Some(p.to_string())))
            .unwrap_or_else(|| (userinfo.to_string(), None));
        let (host, port) = split_host_port(hostport)?;
        return finish_entry(
            line_no,
            options,
            index,
            None,
            default_protocol(options),
            host,
            port,
            percent_decode(&username),
            password.map(|p| percent_decode(&p)),
        );
    }

    let (host, port) = split_host_port(line)?;
    finish_entry(
        line_no,
        options,
        index,
        None,
        default_protocol(options),
        host,
        port,
        String::new(),
        None,
    )
}

fn split_host_port(value: &str) -> std::result::Result<(String, u16), String> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| "IPv6 地址缺少右方括号".to_string())?;
        let port = after
            .strip_prefix(':')
            .ok_or_else(|| "缺少端口".to_string())?;
        return Ok((host.to_string(), parse_port(port)?));
    }
    let (host, port) = value
        .rsplit_once(':')
        .ok_or_else(|| "缺少端口，应为 host:port".to_string())?;
    if host.is_empty() {
        return Err("主机不能为空".into());
    }
    Ok((host.to_string(), parse_port(port)?))
}

fn split_host_port_with_rest(value: &str) -> std::result::Result<(String, &str), String> {
    let value = value.trim();
    if let Some(rest) = value.strip_prefix('[') {
        let (host, after) = rest
            .split_once(']')
            .ok_or_else(|| "IPv6 地址缺少右方括号".to_string())?;
        let after = after
            .strip_prefix(':')
            .ok_or_else(|| "缺少端口".to_string())?;
        return Ok((host.to_string(), after));
    }
    let (host, rest) = value
        .split_once(':')
        .ok_or_else(|| "缺少端口，应为 host:port".to_string())?;
    if host.is_empty() {
        return Err("主机不能为空".into());
    }
    Ok((host.to_string(), rest))
}

fn parse_port(value: &str) -> std::result::Result<u16, String> {
    let port = value
        .trim()
        .parse::<u16>()
        .map_err(|_| format!("端口不合法: {value}"))?;
    if port == 0 {
        return Err("端口不合法".into());
    }
    Ok(port)
}

fn parse_uri(
    line: &str,
    line_no: usize,
    options: &ProxyImportOptions,
    index: usize,
) -> std::result::Result<ParsedProxy, String> {
    let url = url::Url::parse(line).map_err(|e| format!("代理 URL 无效: {e}"))?;
    let protocol = match normalize_protocol(url.scheme()) {
        Some(protocol) => protocol,
        None => {
            return Ok(ParsedProxy {
                line: line_no,
                name: build_name(options, index, None, "", 0),
                protocol: url.scheme().to_string(),
                host: url.host_str().unwrap_or("").to_string(),
                port: url.port().unwrap_or(0),
                username: String::new(),
                password: None,
                duplicate: false,
                duplicate_kind: None,
                unsupported: true,
            })
        }
    };
    let host = url
        .host_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "主机不能为空".to_string())?
        .to_string();
    let port = url
        .port_or_known_default()
        .ok_or_else(|| "缺少端口".to_string())?;
    let username = percent_decode(url.username());
    let password = url.password().map(percent_decode);
    let explicit_name = url
        .fragment()
        .map(percent_decode)
        .filter(|value| !value.trim().is_empty());
    finish_entry(
        line_no,
        options,
        index,
        explicit_name,
        protocol,
        host,
        port,
        username,
        password,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_entry(
    line: usize,
    options: &ProxyImportOptions,
    index: usize,
    explicit_name: Option<String>,
    protocol: String,
    host: String,
    port: u16,
    username: String,
    password: Option<String>,
) -> std::result::Result<ParsedProxy, String> {
    let host = host.trim().to_string();
    if host.is_empty() || host.contains('/') || host.contains(' ') {
        return Err("主机不合法".into());
    }
    if port == 0 {
        return Err("端口不合法".into());
    }
    let protocol = normalize_protocol(&protocol).unwrap_or(protocol);
    if !supported(&protocol) {
        return Ok(ParsedProxy {
            line,
            name: build_name(options, index, explicit_name, &host, port),
            protocol,
            host,
            port,
            username,
            password,
            duplicate: false,
            duplicate_kind: None,
            unsupported: true,
        });
    }
    Ok(ParsedProxy {
        line,
        name: build_name(options, index, explicit_name, &host, port),
        protocol,
        host,
        port,
        username,
        password,
        duplicate: false,
        duplicate_kind: None,
        unsupported: false,
    })
}

fn parse_json(value: &Value, options: &ProxyImportOptions) -> ParsedImport {
    let list = if let Some(array) = value.as_array() {
        array.clone()
    } else if let Some(array) = value.get("proxies").and_then(|v| v.as_array()) {
        array.clone()
    } else if let Some(array) = value.get("servers").and_then(|v| v.as_array()) {
        array.clone()
    } else {
        Vec::new()
    };
    let mut result = ParsedImport {
        total_lines: list.len() as u32,
        ..Default::default()
    };
    for (index, item) in list.iter().enumerate() {
        let line = index + 1;
        let object = match item.as_object() {
            Some(object) => object,
            None => {
                result.errors.push(ProxyImportError {
                    line,
                    raw: "[JSON 节点]".into(),
                    message: "JSON 节点不是对象".into(),
                });
                continue;
            }
        };
        let mut protocol = object
            .get("type")
            .or_else(|| object.get("protocol"))
            .and_then(|v| v.as_str())
            .unwrap_or_else(|| options.default_protocol.as_deref().unwrap_or("http"))
            .to_string();
        if protocol.eq_ignore_ascii_case("http")
            && object
                .get("tls")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            protocol = "https".into();
        }
        let host = object
            .get("server")
            .or_else(|| object.get("host"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let port = object
            .get("port")
            .or_else(|| object.get("server_port"))
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
            })
            .and_then(|value| u16::try_from(value).ok())
            .unwrap_or(0);
        let username = object
            .get("username")
            .or_else(|| object.get("user"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let password = object
            .get("password")
            .or_else(|| object.get("pass"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        let name = object
            .get("name")
            .or_else(|| object.get("remarks"))
            .and_then(|v| v.as_str())
            .map(ToString::to_string);
        if object.get("method").is_some() {
            result.unsupported += 1;
            result.errors.push(ProxyImportError {
                line,
                raw: "[JSON 节点]".into(),
                message: "不支持 Shadowsocks 等非 HTTP/SOCKS5 节点".into(),
            });
            continue;
        }
        match finish_entry(
            line,
            options,
            index + 1,
            name,
            protocol.to_string(),
            host.to_string(),
            port,
            username.to_string(),
            password,
        ) {
            Ok(entry) if entry.unsupported => {
                result.unsupported += 1;
                result.errors.push(ProxyImportError {
                    line,
                    raw: "[JSON 节点]".into(),
                    message: format!("不支持的代理协议: {}", entry.protocol),
                });
            }
            Ok(entry) => result.entries.push(entry),
            Err(message) => result.errors.push(ProxyImportError {
                line,
                raw: "[JSON 节点]".into(),
                message,
            }),
        }
    }
    dedupe_entries(&mut result);
    result
}

fn parse_clash(text: &str, options: &ProxyImportOptions) -> ParsedImport {
    let mut result = ParsedImport::default();
    let mut in_proxies = false;
    let mut current: Option<(usize, Vec<(String, String)>)> = None;
    let mut index = 0usize;

    let flush = |current: &mut Option<(usize, Vec<(String, String)>)>,
                 result: &mut ParsedImport,
                 options: &ProxyImportOptions,
                 index: &mut usize| {
        let Some((line, fields)) = current.take() else {
            return;
        };
        result.total_lines += 1;
        *index += 1;
        let get = |key: &str| {
            fields
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value.clone())
        };
        let mut protocol = get("type")
            .or_else(|| get("protocol"))
            .unwrap_or_else(|| {
                options
                    .default_protocol
                    .clone()
                    .unwrap_or_else(|| "http".into())
            });
        if protocol.eq_ignore_ascii_case("http")
            && get("tls").is_some_and(|value| value.eq_ignore_ascii_case("true"))
        {
            protocol = "https".into();
        }
        let host = get("server").or_else(|| get("host")).unwrap_or_default();
        let port = get("port")
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(0);
        let username = get("username").or_else(|| get("user")).unwrap_or_default();
        let password = get("password").or_else(|| get("pass"));
        let name = get("name");
        let raw = format!("{host}:{port}");
        match finish_entry(
            line,
            options,
            *index,
            name,
            protocol,
            host,
            port,
            username,
            password,
        ) {
            Ok(entry) if entry.unsupported => {
                result.unsupported += 1;
                result.errors.push(ProxyImportError {
                    line,
                    raw: format!("{}:{}", entry.host, entry.port),
                    message: format!("不支持的代理协议: {}", entry.protocol),
                });
            }
            Ok(entry) => result.entries.push(entry),
            Err(message) => result.errors.push(ProxyImportError {
                line,
                raw,
                message,
            }),
        }
    };

    for (line_index, raw) in text.lines().enumerate() {
        let line_no = line_index + 1;
        let trimmed = strip_inline_comment(raw.trim()).trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = raw.len() - raw.trim_start().len();
        if !raw.starts_with(' ') && !raw.starts_with('\t') && trimmed.ends_with(':') {
            if in_proxies {
                flush(&mut current, &mut result, options, &mut index);
                in_proxies = false;
            }
            if trimmed == "proxies:" {
                in_proxies = true;
            }
            continue;
        }
        if !in_proxies {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            flush(&mut current, &mut result, options, &mut index);
            current = Some((line_no, Vec::new()));
            if let Some((key, value)) = rest.split_once(':') {
                current
                    .as_mut()
                    .unwrap()
                    .1
                    .push((key.trim().to_string(), strip_quotes(value)));
            }
            continue;
        }
        if indent == 0 {
            flush(&mut current, &mut result, options, &mut index);
            in_proxies = false;
            continue;
        }
        if let Some(current) = current.as_mut() {
            if let Some((key, value)) = trimmed.split_once(':') {
                current.1.push((key.trim().to_string(), strip_quotes(value)));
            }
        }
    }
    flush(&mut current, &mut result, options, &mut index);
    dedupe_entries(&mut result);
    result
}

fn dedupe_entries(result: &mut ParsedImport) {
    let mut seen = HashSet::new();
    let len = result.entries.len();
    for index in 0..len {
        push_duplicate(&mut result.entries, &mut seen, index);
    }
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
    fn parses_common_line_formats() {
        let parsed = parse_proxy_text(
            "1.2.3.4:8080\n1.2.3.4:8081:user:pass\nuser2:pass2@1.2.3.4:8082\nsocks5://u:p@1.2.3.4:1080\n",
            &options(),
        );
        assert_eq!(parsed.entries.len(), 4);
        assert_eq!(parsed.entries[1].username, "user");
        assert_eq!(parsed.entries[2].password.as_deref(), Some("pass2"));
        assert_eq!(parsed.entries[3].protocol, "socks5");
    }

    #[test]
    fn keeps_at_sign_in_password_for_colon_format() {
        let parsed = parse_proxy_text("1.2.3.4:8080:user:p@ss", &options());
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].username, "user");
        assert_eq!(parsed.entries[0].password.as_deref(), Some("p@ss"));
    }

    #[test]
    fn parses_ipv6_and_rejects_bad_ports() {
        let parsed = parse_proxy_text("[2001:db8::1]:8080\nhost:0\n", &options());
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].host, "2001:db8::1");
        assert_eq!(parsed.errors.len(), 1);
    }

    #[test]
    fn parses_base64_and_dedupes() {
        let text = B64.encode("1.2.3.4:8080\n1.2.3.4:8080\n");
        let parsed = parse_proxy_text(&text, &options());
        assert_eq!(parsed.entries.len(), 2);
        assert!(parsed.entries[1].duplicate);
    }

    #[test]
    fn parses_clash_http_nodes_and_skips_ss() {
        let text = "proxies:\n  - name: \"US-1\"\n    type: http\n    server: 1.2.3.4\n    port: 8080\n    username: u\n    password: p\n  - name: \"SS\"\n    type: ss\n    server: 5.6.7.8\n    port: 8388\n";
        let parsed = parse_proxy_text(text, &options());
        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].name, "US-1");
        assert_eq!(parsed.unsupported, 1);
    }
}
