use std::str::FromStr;

use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Data model
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Fingerprint {
    pub version: u32,
    pub kernel: String,
    pub user_agent: String,
    pub platform: String,
    pub vendor: String,
    pub languages: Vec<String>,
    pub timezone: String,
    pub hardware_concurrency: u32,
    pub device_memory: u32,
    pub do_not_track: bool,
    pub color_scheme: String,
    pub touch_points: u32,
    pub screen: ScreenFingerprint,
    pub canvas: CanvasFingerprint,
    pub webgl: WebglFingerprint,
    pub audio: AudioFingerprint,
    pub fonts: Vec<String>,
    pub webrtc: WebrtcFingerprint,
    pub client_hints: ClientHints,
    pub plugins: Vec<PluginInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScreenFingerprint {
    pub width: u32,
    pub height: u32,
    pub avail_width: u32,
    pub avail_height: u32,
    pub color_depth: u32,
    pub pixel_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CanvasFingerprint {
    /// noise | off
    pub mode: String,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WebglFingerprint {
    pub vendor: String,
    pub renderer: String,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AudioFingerprint {
    /// noise | off
    pub mode: String,
    pub seed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct WebrtcFingerprint {
    /// disable | proxy | public
    pub mode: String,
    pub public_ip: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ClientHints {
    pub brands: Vec<BrandVersion>,
    pub full_version_list: Vec<BrandVersion>,
    pub platform: String,
    pub platform_version: String,
    pub architecture: String,
    pub bitness: String,
    pub model: String,
    pub mobile: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct BrandVersion {
    pub brand: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct PluginInfo {
    pub name: String,
    pub filename: String,
    pub description: String,
    pub mime_types: Vec<MimeTypeInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct MimeTypeInfo {
    pub mime_type: String,
    pub description: String,
    pub suffixes: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintIssue {
    pub field: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintValidation {
    pub valid: bool,
    pub errors: Vec<FingerprintIssue>,
    pub warnings: Vec<FingerprintIssue>,
}

impl Default for ScreenFingerprint {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            avail_width: 1920,
            avail_height: 1032,
            color_depth: 24,
            pixel_ratio: 1.0,
        }
    }
}

impl Default for CanvasFingerprint {
    fn default() -> Self {
        Self {
            mode: "noise".into(),
            seed: 1,
        }
    }
}

impl Default for WebglFingerprint {
    fn default() -> Self {
        Self {
            vendor: "Google Inc. (Intel)".into(),
            renderer:
                "ANGLE (Intel, Intel(R) UHD Graphics 630 (0x00003E92) Direct3D11 vs_5_0 ps_5_0, D3D11)"
                    .into(),
            seed: 1,
        }
    }
}

impl Default for AudioFingerprint {
    fn default() -> Self {
        Self {
            mode: "noise".into(),
            seed: 1,
        }
    }
}

impl Default for WebrtcFingerprint {
    fn default() -> Self {
        Self {
            mode: "disable".into(),
            public_ip: None,
        }
    }
}

impl Default for ClientHints {
    fn default() -> Self {
        let brands = chrome_brands("131.0.6778.86");
        Self {
            full_version_list: brands.clone(),
            brands,
            platform: "Windows".into(),
            platform_version: "15.0.0".into(),
            architecture: "x86".into(),
            bitness: "64".into(),
            model: String::new(),
            mobile: false,
        }
    }
}

impl Default for Fingerprint {
    fn default() -> Self {
        template("win11")
    }
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

fn chrome_brands(version: &str) -> Vec<BrandVersion> {
    let major = version.split('.').next().unwrap_or("131");
    vec![
        BrandVersion {
            brand: "Chromium".into(),
            version: major.into(),
        },
        BrandVersion {
            brand: "Google Chrome".into(),
            version: major.into(),
        },
        BrandVersion {
            brand: "Not_A Brand".into(),
            version: "24".into(),
        },
    ]
}

pub fn chrome_plugin_info() -> Vec<PluginInfo> {
    vec![
        PluginInfo {
            name: "PDF Viewer".into(),
            filename: "internal-pdf-viewer".into(),
            description: "Portable Document Format".into(),
            mime_types: vec![
                MimeTypeInfo {
                    mime_type: "application/pdf".into(),
                    description: "Portable Document Format".into(),
                    suffixes: "pdf".into(),
                },
                MimeTypeInfo {
                    mime_type: "text/pdf".into(),
                    description: "Portable Document Format".into(),
                    suffixes: "pdf".into(),
                },
            ],
        },
        PluginInfo {
            name: "Chrome PDF Viewer".into(),
            filename: "internal-pdf-viewer".into(),
            description: "Portable Document Format".into(),
            mime_types: vec![MimeTypeInfo {
                mime_type: "application/pdf".into(),
                description: "Portable Document Format".into(),
                suffixes: "pdf".into(),
            }],
        },
        PluginInfo {
            name: "Chromium PDF Viewer".into(),
            filename: "internal-pdf-viewer".into(),
            description: "Portable Document Format".into(),
            mime_types: vec![MimeTypeInfo {
                mime_type: "application/pdf".into(),
                description: "Portable Document Format".into(),
                suffixes: "pdf".into(),
            }],
        },
    ]
}

/// Generate a self-consistent fingerprint template. Unknown `os` values fall
/// back to Windows 11 so that the UI can always offer a usable default.
pub fn template(os: &str) -> Fingerprint {
    let mut rng = rand::thread_rng();
    let major: u32 = rng.gen_range(130..=131);
    let build: u32 = rng.gen_range(6700..=6900);
    let patch: u32 = rng.gen_range(50..=180);
    let full_version = format!("{major}.0.{build}.{patch}");
    let seed: u32 = rng.gen();
    let nav_seed = seed ^ 0x9E37_79B9;

    let (os_key, ua, platform, platform_version, architecture, fonts, gl_vendor, gl_renderer) =
        match os.to_ascii_lowercase().as_str() {
            "mac" | "macos" | "macos-arm" => (
                "mac",
                format!(
                    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{full_version} Safari/537.36"
                ),
                "MacIntel",
                "14.5.0",
                "arm",
                vec![
                    "Arial", "Arial Hebrew", "Avenir", "American Typewriter", "Courier New",
                    "Georgia", "Helvetica", "Helvetica Neue", "Lucida Grande", "Menlo", "Monaco",
                    "Optima", "Palatino", "PingFang SC", "SF Pro Display", "Times New Roman",
                    "Trebuchet MS", "Verdana",
                ],
                "Google Inc. (Apple)",
                "ANGLE (Apple, ANGLE Metal Renderer: Apple M2, Unspecified Version)",
            ),
            "linux" | "ubuntu" => (
                "linux",
                format!(
                    "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{full_version} Safari/537.36"
                ),
                "Linux x86_64",
                "6.5.0",
                "x86",
                vec![
                    "Cantarell", "DejaVu Sans", "DejaVu Serif", "FreeSans", "Liberation Mono",
                    "Liberation Sans", "Noto Sans", "Noto Serif", "Ubuntu", "WenQuanYi Micro Hei",
                ],
                "Google Inc. (Intel)",
                "ANGLE (Intel, Mesa Intel(R) UHD Graphics 620 (KBL GT2), OpenGL 4.6)",
            ),
            "win10" | "windows10" => (
                "win10",
                format!(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{full_version} Safari/537.36"
                ),
                "Win32",
                "10.0.0",
                "x86",
                windows_fonts(),
                "Google Inc. (NVIDIA)",
                "ANGLE (NVIDIA, NVIDIA GeForce GTX 1660 SUPER Direct3D11 vs_5_0 ps_5_0, D3D11)",
            ),
            _ => (
                "win11",
                format!(
                    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/{full_version} Safari/537.36"
                ),
                "Win32",
                "15.0.0",
                "x86",
                windows_fonts(),
                "Google Inc. (Intel)",
                "ANGLE (Intel, Intel(R) UHD Graphics 630 (0x00003E92) Direct3D11 vs_5_0 ps_5_0, D3D11)",
            ),
        };

    let (screen_w, screen_h, dpr) = match os_key {
        "mac" => (1728u32, 1117u32, 2.0),
        "linux" => (1920, 1080, 1.0),
        _ => (1920, 1080, 1.0),
    };
    let (timezone, languages) = match os_key {
        "mac" => (
            "America/Los_Angeles".to_string(),
            vec!["en-US".to_string(), "en".to_string()],
        ),
        "linux" => (
            "Europe/Berlin".to_string(),
            vec!["en-US".to_string(), "en".to_string()],
        ),
        _ => (
            "Asia/Shanghai".to_string(),
            vec!["zh-CN".to_string(), "zh".to_string(), "en".to_string()],
        ),
    };

    let brands = chrome_brands(&full_version);
    Fingerprint {
        version: 1,
        kernel: "chromium".into(),
        user_agent: ua,
        platform: platform.into(),
        vendor: "Google Inc.".into(),
        languages,
        timezone,
        hardware_concurrency: [4u32, 8, 12, 16][rng.gen_range(0..4)],
        device_memory: 8,
        do_not_track: false,
        color_scheme: if rng.gen_bool(0.5) { "light" } else { "dark" }.into(),
        touch_points: 0,
        screen: ScreenFingerprint {
            width: screen_w,
            height: screen_h,
            avail_width: screen_w,
            avail_height: if os_key == "mac" {
                screen_h - 25
            } else {
                screen_h - 48
            },
            color_depth: 24,
            pixel_ratio: dpr,
        },
        canvas: CanvasFingerprint {
            mode: "noise".into(),
            seed: nav_seed,
        },
        webgl: WebglFingerprint {
            vendor: gl_vendor.into(),
            renderer: gl_renderer.into(),
            seed,
        },
        audio: AudioFingerprint {
            mode: "noise".into(),
            seed: nav_seed.rotate_left(7),
        },
        fonts: fonts.into_iter().map(String::from).collect(),
        webrtc: WebrtcFingerprint {
            mode: "disable".into(),
            public_ip: None,
        },
        client_hints: ClientHints {
            brands,
            full_version_list: chrome_brands(&full_version),
            platform: match os_key {
                "mac" => "macOS".into(),
                "linux" => "Linux".into(),
                _ => "Windows".into(),
            },
            platform_version: platform_version.into(),
            architecture: architecture.into(),
            bitness: "64".into(),
            model: String::new(),
            mobile: false,
        },
        plugins: chrome_plugin_info(),
    }
}

fn windows_fonts() -> Vec<&'static str> {
    vec![
        "Arial",
        "Calibri",
        "Cambria",
        "Candara",
        "Comic Sans MS",
        "Consolas",
        "Constantia",
        "Corbel",
        "Courier New",
        "Georgia",
        "Impact",
        "Lucida Console",
        "Microsoft YaHei",
        "Segoe UI",
        "SimSun",
        "Tahoma",
        "Times New Roman",
        "Trebuchet MS",
        "Verdana",
    ]
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

fn issue(field: &str, message: impl Into<String>, severity: &str) -> FingerprintIssue {
    FingerprintIssue {
        field: field.into(),
        message: message.into(),
        severity: severity.into(),
    }
}

fn chrome_major(ua: &str) -> Option<u32> {
    let marker = "Chrome/";
    let start = ua.find(marker)? + marker.len();
    ua[start..].split('.').next()?.parse().ok()
}

fn first_brand_version(ch: &ClientHints) -> Option<u32> {
    ch.brands
        .iter()
        .find(|b| b.brand == "Google Chrome" || b.brand == "Chromium")
        .or_else(|| ch.brands.first())
        .and_then(|b| b.version.split('.').next()?.parse().ok())
}

pub fn validate(fp: &Fingerprint) -> FingerprintValidation {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    if fp.user_agent.trim().is_empty() {
        errors.push(issue("userAgent", "User-Agent 不能为空", "error"));
    } else {
        if !fp.user_agent.contains("Mozilla/5.0") {
            warnings.push(issue("userAgent", "UA 不符合常见浏览器格式", "warning"));
        }
        match chrome_major(&fp.user_agent) {
            Some(major) => {
                if let Some(ch) = first_brand_version(&fp.client_hints) {
                    if major != ch {
                        errors.push(issue(
                            "clientHints.brands",
                            format!("Client Hints 主版本 {ch} 与 UA 主版本 {major} 不一致"),
                            "error",
                        ));
                    }
                }
            }
            None => errors.push(issue(
                "userAgent",
                "V1 Chromium 内核的 UA 必须包含 Chrome/<版本>",
                "error",
            )),
        }
        let expected_platform = if fp.user_agent.contains("Windows NT") {
            ("Win32", "Windows")
        } else if fp.user_agent.contains("Macintosh") {
            ("MacIntel", "macOS")
        } else if fp.user_agent.contains("Linux") {
            ("Linux x86_64", "Linux")
        } else {
            ("", "")
        };
        if expected_platform.0.is_empty() {
            errors.push(issue("userAgent", "无法从 UA 识别操作系统", "error"));
        } else {
            if fp.platform != expected_platform.0 {
                errors.push(issue(
                    "platform",
                    format!(
                        "navigator.platform 应为 {}，当前为 {}",
                        expected_platform.0, fp.platform
                    ),
                    "error",
                ));
            }
            if fp.client_hints.platform != expected_platform.1 {
                errors.push(issue(
                    "clientHints.platform",
                    format!(
                        "Client Hints platform 应为 {}，当前为 {}",
                        expected_platform.1, fp.client_hints.platform
                    ),
                    "error",
                ));
            }
        }
        if fp.user_agent.contains("HeadlessChrome") {
            errors.push(issue(
                "userAgent",
                "禁止使用 HeadlessChrome UA，易被风控识别",
                "error",
            ));
        }
    }

    if fp.languages.is_empty() {
        errors.push(issue("languages", "至少需要一个语言", "error"));
    } else {
        for lang in &fp.languages {
            if lang.is_empty()
                || !lang
                    .chars()
                    .all(|c| c.is_ascii_alphabetic() || c == '-' || c == '_')
            {
                errors.push(issue(
                    "languages",
                    format!("语言标签 {lang} 格式不合法"),
                    "error",
                ));
            }
        }
        if (fp.languages.iter().any(|l| l == "zh-CN" || l == "zh"))
            && !fp.timezone.starts_with("Asia/")
        {
            warnings.push(issue(
                "timezone",
                "中文语言通常搭配 Asia/* 时区，请确认与代理 IP 对齐",
                "warning",
            ));
        }
    }

    if chrono_tz::Tz::from_str(&fp.timezone).is_err() {
        errors.push(issue(
            "timezone",
            format!("时区 {} 不是合法 IANA 时区", fp.timezone),
            "error",
        ));
    }

    if fp.screen.width < 800 || fp.screen.height < 600 {
        errors.push(issue("screen", "屏幕分辨率过小（最小 800x600）", "error"));
    }
    if fp.screen.width > 7680 || fp.screen.height > 4320 {
        errors.push(issue("screen", "屏幕分辨率超出合理范围", "error"));
    }
    if fp.screen.avail_width > fp.screen.width || fp.screen.avail_height > fp.screen.height {
        errors.push(issue("screen", "可用屏幕尺寸不能大于总屏幕尺寸", "error"));
    }
    if fp.screen.width > 0 && fp.screen.avail_width + 200 < fp.screen.width {
        warnings.push(issue(
            "screen",
            "可用宽度明显小于总宽度，可能触发特征检测",
            "warning",
        ));
    }
    if ![24u32, 30, 32].contains(&fp.screen.color_depth) {
        errors.push(issue("screen", "colorDepth 必须为 24/30/32", "error"));
    }
    if fp.screen.pixel_ratio < 0.5 || fp.screen.pixel_ratio > 4.0 {
        errors.push(issue(
            "screen",
            "devicePixelRatio 超出 0.5 - 4.0 范围",
            "error",
        ));
    }
    if fp.screen.pixel_ratio.fract() == 0.0 && fp.screen.pixel_ratio > 2.0 {
        warnings.push(issue(
            "screen",
            "高 DPR 通常出现在移动或高分屏设备",
            "warning",
        ));
    }

    if !(2..=32).contains(&fp.hardware_concurrency) {
        errors.push(issue(
            "hardwareConcurrency",
            "CPU 核心数必须在 2 - 32 之间",
            "error",
        ));
    }
    if ![2u32, 4, 8, 16, 32].contains(&fp.device_memory) {
        warnings.push(issue(
            "deviceMemory",
            "deviceMemory 建议使用 2/4/8/16/32 等常见值",
            "warning",
        ));
    }
    if fp.touch_points > 0 && !fp.client_hints.mobile {
        warnings.push(issue(
            "touchPoints",
            "桌面 UA 声明了触摸点，可能被识别为混合特征",
            "warning",
        ));
    }
    if fp.client_hints.mobile != fp.user_agent.contains("Mobile") {
        errors.push(issue(
            "clientHints.mobile",
            "Client Hints mobile 与 UA 的移动端标识不一致",
            "error",
        ));
    }

    if !["noise", "off"].contains(&fp.canvas.mode.as_str()) {
        errors.push(issue(
            "canvas.mode",
            "Canvas 模式仅支持 noise / off",
            "error",
        ));
    }
    if !["noise", "off"].contains(&fp.audio.mode.as_str()) {
        errors.push(issue("audio.mode", "Audio 模式仅支持 noise / off", "error"));
    }
    if fp.canvas.mode == "noise" && fp.canvas.seed == 0 {
        warnings.push(issue(
            "canvas.seed",
            "Canvas 噪声种子为 0，建议随机生成",
            "warning",
        ));
    }
    if fp.audio.mode == "noise" && fp.audio.seed == 0 {
        warnings.push(issue(
            "audio.seed",
            "Audio 噪声种子为 0，建议随机生成",
            "warning",
        ));
    }

    if fp.webgl.vendor.trim().is_empty() || fp.webgl.renderer.trim().is_empty() {
        errors.push(issue("webgl", "WebGL 厂商与渲染器不能为空", "error"));
    } else if !fp.webgl.renderer.contains("ANGLE")
        && !fp.webgl.renderer.contains("Mesa")
        && !fp.webgl.renderer.contains("OpenGL")
        && !fp.webgl.renderer.contains("Direct3D")
    {
        warnings.push(issue(
            "webgl.renderer",
            "WebGL renderer 不符合常见 ANGLE/Mesa 格式",
            "warning",
        ));
    }
    if fp.user_agent.contains("Windows NT") && fp.webgl.renderer.contains("Metal") {
        errors.push(issue(
            "webgl.renderer",
            "Windows UA 不应使用 Metal 渲染器",
            "error",
        ));
    }

    if fp.fonts.len() < 3 {
        errors.push(issue(
            "fonts",
            "字体列表至少需要 3 项，避免字体指纹过于独特",
            "error",
        ));
    }
    if !fp.fonts.iter().any(|f| f.eq_ignore_ascii_case("Arial"))
        && fp.user_agent.contains("Windows NT")
    {
        warnings.push(issue("fonts", "Windows 字体列表建议包含 Arial", "warning"));
    }

    if !["disable", "proxy", "public"].contains(&fp.webrtc.mode.as_str()) {
        errors.push(issue(
            "webrtc.mode",
            "WebRTC 模式仅支持 disable / proxy / public",
            "error",
        ));
    }
    if fp.webrtc.mode == "public" && fp.webrtc.public_ip.as_deref().unwrap_or("").is_empty() {
        errors.push(issue(
            "webrtc.publicIp",
            "WebRTC public 模式必须填写公网 IP",
            "error",
        ));
    } else if let Some(ip) = &fp.webrtc.public_ip {
        if !ip.is_empty() && ip.parse::<std::net::IpAddr>().is_err() {
            errors.push(issue(
                "webrtc.publicIp",
                "WebRTC 公网 IP 格式不合法",
                "error",
            ));
        }
    }

    if fp.client_hints.brands.is_empty() {
        errors.push(issue(
            "clientHints.brands",
            "Client Hints brands 不能为空",
            "error",
        ));
    } else if !fp.client_hints.brands.iter().any(|b| b.brand == "Chromium") {
        errors.push(issue(
            "clientHints.brands",
            "Client Hints brands 必须包含 Chromium",
            "error",
        ));
    }
    if fp.client_hints.full_version_list.is_empty() {
        warnings.push(issue(
            "clientHints.fullVersionList",
            "缺少 fullVersionList，自动化检测可能识别为异常",
            "warning",
        ));
    }
    if !["x86", "arm"].contains(&fp.client_hints.architecture.as_str()) {
        errors.push(issue(
            "clientHints.architecture",
            "Client Hints architecture 必须是 x86 或 arm",
            "error",
        ));
    }
    if !["64", "32"].contains(&fp.client_hints.bitness.as_str()) {
        errors.push(issue(
            "clientHints.bitness",
            "bitness 必须是 64 或 32",
            "error",
        ));
    }

    if fp.plugins.is_empty() {
        warnings.push(issue(
            "plugins",
            "插件列表为空，现代 Chrome 至少应有 PDF Viewer",
            "warning",
        ));
    }
    if fp.plugins.iter().any(|p| p.name.contains("Flash")) {
        errors.push(issue(
            "plugins",
            "现代 Chrome 不支持 Flash 插件，请移除",
            "error",
        ));
    }

    if !["light", "dark", "no-preference"].contains(&fp.color_scheme.as_str()) {
        errors.push(issue(
            "colorScheme",
            "colorScheme 仅支持 light / dark / no-preference",
            "error",
        ));
    }
    if fp.kernel != "chromium" {
        errors.push(issue(
            "kernel",
            "V1 版本仅支持 Chromium 内核（Firefox 将在 V1.1 接入）",
            "error",
        ));
    }

    FingerprintValidation {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

// ---------------------------------------------------------------------------
// Proxy alignment
// ---------------------------------------------------------------------------

/// Map an IP geolocation country code to a plausible timezone and language.
pub fn locale_for_country(country: &str) -> (&'static str, &'static [&'static str]) {
    match country.to_ascii_uppercase().as_str() {
        "CN" => ("Asia/Shanghai", &["zh-CN", "zh", "en"]),
        "HK" => ("Asia/Hong_Kong", &["zh-HK", "zh", "en"]),
        "TW" => ("Asia/Taipei", &["zh-TW", "zh", "en"]),
        "JP" => ("Asia/Tokyo", &["ja-JP", "ja", "en"]),
        "KR" => ("Asia/Seoul", &["ko-KR", "ko", "en"]),
        "SG" => ("Asia/Singapore", &["en-SG", "en", "zh-CN"]),
        "IN" => ("Asia/Kolkata", &["en-IN", "en", "hi"]),
        "AU" => ("Australia/Sydney", &["en-AU", "en"]),
        "GB" => ("Europe/London", &["en-GB", "en"]),
        "DE" => ("Europe/Berlin", &["de-DE", "de", "en"]),
        "FR" => ("Europe/Paris", &["fr-FR", "fr", "en"]),
        "NL" => ("Europe/Amsterdam", &["nl-NL", "nl", "en"]),
        "RU" => ("Europe/Moscow", &["ru-RU", "ru", "en"]),
        "BR" => ("America/Sao_Paulo", &["pt-BR", "pt", "en"]),
        "CA" => ("America/Toronto", &["en-CA", "en", "fr"]),
        "US" => ("America/New_York", &["en-US", "en"]),
        "ZZ" | "" => ("UTC", &["en-US", "en"]),
        _ => ("UTC", &["en-US", "en"]),
    }
}

pub fn timezone_for_country(country: &str) -> String {
    locale_for_country(country).0.to_string()
}

pub fn apply_country_locale(fp: &mut Fingerprint, country: &str) {
    let (tz, languages) = locale_for_country(country);
    fp.timezone = tz.to_string();
    fp.languages = languages.iter().map(|value| value.to_string()).collect();
}

// ---------------------------------------------------------------------------
// CDP injection script
// ---------------------------------------------------------------------------

/// JavaScript injected into every new document. It is intentionally defensive:
/// every override is wrapped in try/catch so one failing API cannot break pages.
pub fn injection_script(fp: &Fingerprint) -> String {
    let data = serde_json::to_string(fp).unwrap_or_else(|_| "{}".into());
    INJECTION_TEMPLATE.replace("__FINENV_FP__", &data)
}

const INJECTION_TEMPLATE: &str = r#"(() => {
  'use strict';
  const FP = __FINENV_FP__;
  const define = (obj, prop, value) => {
    try { Object.defineProperty(obj, prop, { get: () => value, configurable: true, enumerable: true }); } catch (e) {}
  };
  const defineFn = (obj, prop, fn) => {
    try { Object.defineProperty(obj, prop, { value: fn, configurable: true, writable: true }); } catch (e) {}
  };
  const rng = (seed) => {
    let s = seed >>> 0;
    return () => { s = (Math.imul(s, 1664525) + 1013904223) >>> 0; return s / 4294967296; };
  };

  try {
    define(Navigator.prototype, 'userAgent', FP.userAgent);
    define(Navigator.prototype, 'appVersion', FP.userAgent.replace(/^Mozilla\//, ''));
    define(Navigator.prototype, 'platform', FP.platform);
    define(Navigator.prototype, 'vendor', FP.vendor);
    define(Navigator.prototype, 'language', FP.languages[0] || 'en-US');
    define(Navigator.prototype, 'languages', Object.freeze(FP.languages.slice()));
    define(Navigator.prototype, 'hardwareConcurrency', FP.hardwareConcurrency);
    define(Navigator.prototype, 'deviceMemory', FP.deviceMemory);
    define(Navigator.prototype, 'maxTouchPoints', FP.touchPoints);
    define(Navigator.prototype, 'webdriver', false);
    define(Navigator.prototype, 'doNotTrack', FP.doNotTrack ? '1' : null);
    define(Navigator.prototype, 'pdfViewerEnabled', true);
  } catch (e) {}

  try {
    if (navigator.userAgentData) {
      const brands = FP.clientHints.brands.map(b => ({ brand: b.brand, version: b.version }));
      const full = FP.clientHints.fullVersionList.map(b => ({ brand: b.brand, version: b.version }));
      const high = {
        architecture: FP.clientHints.architecture,
        bitness: FP.clientHints.bitness,
        brands: brands,
        fullVersionList: full,
        mobile: FP.clientHints.mobile,
        model: FP.clientHints.model,
        platform: FP.clientHints.platform,
        platformVersion: FP.clientHints.platformVersion,
        uaFullVersion: (full.find(b => b.brand === 'Google Chrome') || full[0] || { version: '' }).version,
        wow64: false
      };
      define(Navigator.prototype, 'userAgentData', {
        brands: brands,
        mobile: FP.clientHints.mobile,
        platform: FP.clientHints.platform,
        getHighEntropyValues: () => Promise.resolve(high),
        toJSON: () => ({ brands, mobile: FP.clientHints.mobile, platform: FP.clientHints.platform })
      });
    }
  } catch (e) {}

  try {
    const screenValues = {
      width: FP.screen.width, height: FP.screen.height,
      availWidth: FP.screen.availWidth, availHeight: FP.screen.availHeight,
      colorDepth: FP.screen.colorDepth, pixelDepth: FP.screen.colorDepth,
      availLeft: 0, availTop: 0
    };
    Object.keys(screenValues).forEach(k => define(Screen.prototype, k, screenValues[k]));
    define(window, 'devicePixelRatio', FP.screen.pixelRatio);
    define(window, 'innerWidth', FP.screen.availWidth);
    define(window, 'innerHeight', FP.screen.availHeight);
    define(window, 'outerWidth', FP.screen.availWidth);
    define(window, 'outerHeight', FP.screen.availHeight);
  } catch (e) {}

  try {
    const pluginArray = FP.plugins.map(p => {
      const mimes = p.mimeTypes.map(m => ({ type: m.mimeType, description: m.description, suffixes: m.suffixes, enabledPlugin: null }));
      const plugin = { name: p.name, filename: p.filename, description: p.description, length: mimes.length };
      mimes.forEach((m, i) => { plugin[i] = m; m.enabledPlugin = plugin; });
      plugin.item = (i) => mimes[i] || null;
      plugin.namedItem = (n) => mimes.find(m => m.type === n) || null;
      return plugin;
    });
    pluginArray.item = (i) => pluginArray[i] || null;
    pluginArray.namedItem = (n) => pluginArray.find(p => p.name === n) || null;
    pluginArray.refresh = () => {};
    define(Navigator.prototype, 'plugins', pluginArray);
    const mimeArray = [];
    FP.plugins.forEach(p => p.mimeTypes.forEach(m => mimeArray.push({ type: m.mimeType, description: m.description, suffixes: m.suffixes })));
    mimeArray.item = (i) => mimeArray[i] || null;
    mimeArray.namedItem = (n) => mimeArray.find(m => m.type === n) || null;
    define(Navigator.prototype, 'mimeTypes', mimeArray);
  } catch (e) {}

  try {
    const offsetFor = (tz) => {
      try {
        const now = new Date();
        const local = new Date(now.toLocaleString('en-US', { timeZone: tz }));
        const utc = new Date(now.toLocaleString('en-US', { timeZone: 'UTC' }));
        return Math.round((utc.getTime() - local.getTime()) / 60000);
      } catch (e) { return null; }
    };
    const tzOffset = offsetFor(FP.timezone);
    if (tzOffset !== null) {
      const origOffset = Date.prototype.getTimezoneOffset;
      defineFn(Date.prototype, 'getTimezoneOffset', function () { return tzOffset; });
    }
    if (Intl && Intl.DateTimeFormat) {
      const orig = Intl.DateTimeFormat.prototype.resolvedOptions;
      defineFn(Intl.DateTimeFormat.prototype, 'resolvedOptions', function () {
        const o = orig.call(this);
        o.timeZone = FP.timezone;
        o.locale = FP.languages[0] || o.locale;
        return o;
      });
    }
  } catch (e) {}

  try {
    if (FP.canvas.mode === 'noise') {
      const origGetImageData = CanvasRenderingContext2D.prototype.getImageData;
      defineFn(CanvasRenderingContext2D.prototype, 'getImageData', function (x, y, w, h) {
        const image = origGetImageData.call(this, x, y, w, h);
        const r = rng((FP.canvas.seed + Math.floor(x * 7 + y * 13 + w * 17 + h * 19)) >>> 0);
        for (let i = 0; i < image.data.length; i += 4) {
          if (r() < 0.015) {
            image.data[i] = (image.data[i] + (r() < 0.5 ? -1 : 1)) & 255;
          }
        }
        return image;
      });
      const origToDataURL = HTMLCanvasElement.prototype.toDataURL;
      defineFn(HTMLCanvasElement.prototype, 'toDataURL', function (...args) {
        try {
          const copy = document.createElement('canvas');
          copy.width = this.width; copy.height = this.height;
          const ctx = copy.getContext('2d');
          if (ctx) {
            ctx.drawImage(this, 0, 0);
            const image = ctx.getImageData(0, 0, copy.width, copy.height);
            const r = rng((FP.canvas.seed + copy.width * 31 + copy.height * 37) >>> 0);
            for (let i = 0; i < image.data.length; i += 4) {
              if (r() < 0.012) image.data[i] = (image.data[i] + (r() < 0.5 ? -1 : 1)) & 255;
            }
            ctx.putImageData(image, 0, 0);
            return origToDataURL.apply(copy, args);
          }
        } catch (e) {}
        return origToDataURL.apply(this, args);
      });
    }
  } catch (e) {}

  try {
    const patchGL = (proto) => {
      if (!proto) return;
      const origGetParameter = proto.getParameter;
      defineFn(proto, 'getParameter', function (p) {
        if (p === 37445) return FP.webgl.vendor;
        if (p === 37446) return FP.webgl.renderer;
        return origGetParameter.call(this, p);
      });
      const origGetExtension = proto.getExtension;
      defineFn(proto, 'getExtension', function (name) {
        if (name === 'WEBGL_debug_renderer_info') {
          return { UNMASKED_VENDOR_WEBGL: 37445, UNMASKED_RENDERER_WEBGL: 37446 };
        }
        return origGetExtension.call(this, name);
      });
    };
    patchGL(window.WebGLRenderingContext && WebGLRenderingContext.prototype);
    patchGL(window.WebGL2RenderingContext && WebGL2RenderingContext.prototype);
  } catch (e) {}

  try {
    if (FP.audio.mode === 'noise' && window.AnalyserNode) {
      const patch = (name) => {
        const orig = AnalyserNode.prototype[name];
        if (!orig) return;
        defineFn(AnalyserNode.prototype, name, function (arr) {
          orig.call(this, arr);
          const r = rng((FP.audio.seed + arr.length) >>> 0);
          for (let i = 0; i < arr.length; i++) {
            arr[i] = arr[i] + (r() - 0.5) * 1e-6;
          }
        });
      };
      patch('getFloatFrequencyData');
      patch('getFloatTimeDomainData');
    }
    if (FP.audio.mode === 'noise' && window.OfflineAudioContext) {
      const origStart = OfflineAudioContext.prototype.startRendering;
      defineFn(OfflineAudioContext.prototype, 'startRendering', function () {
        return origStart.call(this).then((buffer) => {
          try {
            const r = rng(FP.audio.seed >>> 0);
            for (let channel = 0; channel < buffer.numberOfChannels; channel++) {
              const data = buffer.getChannelData(channel);
              for (let i = 0; i < data.length; i += 37) {
                data[i] = data[i] + (r() - 0.5) * 1e-7;
              }
            }
          } catch (e) {}
          return buffer;
        });
      });
    }
  } catch (e) {}

  try {
    const origCheck = FontFaceSet.prototype.check;
    defineFn(FontFaceSet.prototype, 'check', function (font, text) {
      const family = String(font || '').replace(/^.*?\s/, '').replace(/["']/g, '').split(',')[0].trim();
      if (FP.fonts.some(f => f.toLowerCase() === family.toLowerCase())) return true;
      return origCheck.call(this, font, text);
    });
  } catch (e) {}

  try {
    if (window.RTCPeerConnection && FP.webrtc.mode !== 'public') {
      const Orig = window.RTCPeerConnection;
      const Patched = function (...args) {
        const pc = new Orig(...args);
        try {
          const origAdd = pc.addEventListener.bind(pc);
          pc.addEventListener = function (type, listener, opts) {
            if (type === 'icecandidate' && typeof listener === 'function') {
              return origAdd(type, function (event) {
                if (!event.candidate || !event.candidate.candidate) return listener.call(this, event);
                const line = event.candidate.candidate;
                if (FP.webrtc.mode === 'disable' && /(typ srflx|typ relay|typ prflx)/.test(line)) return;
                return listener.call(this, event);
              }, opts);
            }
            return origAdd(type, listener, opts);
          };
        } catch (e) {}
        return pc;
      };
      Patched.prototype = Orig.prototype;
      Object.setPrototypeOf(Patched, Orig);
      window.RTCPeerConnection = Patched;
      if (window.webkitRTCPeerConnection) window.webkitRTCPeerConnection = Patched;
    }
  } catch (e) {}

  try {
    if (window.matchMedia) {
      const orig = window.matchMedia.bind(window);
      window.matchMedia = function (query) {
        if (FP.colorScheme !== 'no-preference' && /prefers-color-scheme/.test(query)) {
          return orig('(prefers-color-scheme: ' + FP.colorScheme + ')');
        }
        return orig(query);
      };
    }
  } catch (e) {}

  try { delete Navigator.prototype.__proto__?.webdriver; } catch (e) {}
})();"#;

// ---------------------------------------------------------------------------
// Browser-side probe used by the detection panel
// ---------------------------------------------------------------------------

pub const PROBE_SCRIPT: &str = r#"(async () => {
  const fnv = (str) => {
    let h = 0x811c9dc5;
    for (let i = 0; i < str.length; i++) { h ^= str.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
    return h.toString(16);
  };
  const result = {
    userAgent: navigator.userAgent,
    platform: navigator.platform,
    vendor: navigator.vendor,
    languages: Array.from(navigator.languages || []),
    language: navigator.language,
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    hardwareConcurrency: navigator.hardwareConcurrency,
    deviceMemory: navigator.deviceMemory || null,
    webdriver: navigator.webdriver,
    doNotTrack: navigator.doNotTrack,
    touchPoints: navigator.maxTouchPoints,
    plugins: Array.from(navigator.plugins || []).map(p => p.name),
    screen: {
      width: screen.width, height: screen.height,
      availWidth: screen.availWidth, availHeight: screen.availHeight,
      colorDepth: screen.colorDepth, pixelRatio: window.devicePixelRatio
    },
    clientHints: null,
    canvas: null, webgl: null, audio: null, fonts: {}
  };
  try {
    if (navigator.userAgentData) {
      const high = await navigator.userAgentData.getHighEntropyValues(['architecture','bitness','brands','fullVersionList','mobile','model','platform','platformVersion','uaFullVersion']);
      result.clientHints = high;
    }
  } catch (e) {}
  try {
    const c = document.createElement('canvas');
    c.width = 240; c.height = 60;
    const ctx = c.getContext('2d');
    ctx.textBaseline = 'top';
    ctx.font = '16px Arial';
    ctx.fillStyle = '#f60';
    ctx.fillRect(100, 1, 62, 20);
    ctx.fillStyle = '#069';
    ctx.fillText('FinEnv fingerprint probe', 2, 15);
    ctx.fillStyle = 'rgba(102,204,0,0.7)';
    ctx.fillText('FinEnv fingerprint probe', 4, 17);
    result.canvas = { hash: fnv(c.toDataURL()), length: c.toDataURL().length };
  } catch (e) { result.canvas = { error: String(e) }; }
  try {
    const c = document.createElement('canvas');
    const gl = c.getContext('webgl') || c.getContext('experimental-webgl');
    if (gl) {
      const ext = gl.getExtension('WEBGL_debug_renderer_info');
      result.webgl = {
        vendor: gl.getParameter(gl.VENDOR),
        renderer: gl.getParameter(gl.RENDERER),
        unmaskedVendor: ext ? gl.getParameter(ext.UNMASKED_VENDOR_WEBGL) : null,
        unmaskedRenderer: ext ? gl.getParameter(ext.UNMASKED_RENDERER_WEBGL) : null
      };
    }
  } catch (e) { result.webgl = { error: String(e) }; }
  try {
    const OfflineCtx = window.OfflineAudioContext || window.webkitOfflineAudioContext;
    if (OfflineCtx) {
      const ctx = new OfflineCtx(1, 4410, 44100);
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.type = 'triangle'; osc.frequency.value = 1000;
      gain.gain.value = 0.05;
      osc.connect(gain); gain.connect(ctx.destination);
      osc.start(0);
      const buffer = await ctx.startRendering();
      const data = buffer.getChannelData(0);
      let acc = 0;
      for (let i = 0; i < data.length; i += 17) acc += Math.abs(data[i]);
      result.audio = { hash: fnv(String(acc)) };
    }
  } catch (e) { result.audio = { error: String(e) }; }
  const probes = ['Arial','Courier New','Georgia','Times New Roman','Verdana','Segoe UI','Calibri','Helvetica','Menlo','DejaVu Sans','Noto Sans','Microsoft YaHei'];
  try {
    for (const f of probes) {
      try { result.fonts[f] = document.fonts.check('12px "' + f + '"'); } catch (e) { result.fonts[f] = null; }
    }
  } catch (e) {}
  return result;
})()"#;

// ---------------------------------------------------------------------------
// Report comparison
// ---------------------------------------------------------------------------

pub fn build_report_checks(
    fp: &Fingerprint,
    observed: &Value,
) -> (Vec<crate::models::FingerprintCheck>, u32) {
    use crate::models::FingerprintCheck;
    let mut checks = Vec::new();
    let s = |v: &Value| v.as_str().unwrap_or("").to_string();
    let n = |v: &Value| v.as_u64().unwrap_or(0);
    let f = |v: &Value| v.as_f64().unwrap_or(0.0);

    let mut push =
        |name: &str, expected: String, actual: String, passed: bool, weight: u32, hint: &str| {
            checks.push(FingerprintCheck {
                name: name.into(),
                expected,
                actual,
                passed,
                weight,
                hint: hint.into(),
            });
        };

    let ua = s(&observed["userAgent"]);
    push(
        "User-Agent",
        fp.user_agent.clone(),
        ua.clone(),
        ua == fp.user_agent,
        15,
        "CDP Emulation.setUserAgentOverride 未生效或页面已重置 UA",
    );
    let platform = s(&observed["platform"]);
    push(
        "navigator.platform",
        fp.platform.clone(),
        platform.clone(),
        platform == fp.platform,
        8,
        "检查注入脚本是否被 CSP 或页面覆盖",
    );
    let langs: Vec<String> = observed["languages"]
        .as_array()
        .map(|a| {
            a.iter()
                .map(|v| v.as_str().unwrap_or("").to_string())
                .collect()
        })
        .unwrap_or_default();
    push(
        "Languages",
        fp.languages.join(","),
        langs.join(","),
        langs == fp.languages,
        6,
        "Emulation.setLocaleOverride 与 navigator.languages 需同时设置",
    );
    let tz = s(&observed["timezone"]);
    push(
        "Timezone",
        fp.timezone.clone(),
        tz.clone(),
        tz == fp.timezone,
        10,
        "使用 Emulation.setTimezoneOverride 覆盖时区",
    );
    let sw = n(&observed["screen"]["width"]);
    let sh = n(&observed["screen"]["height"]);
    let aw = n(&observed["screen"]["availWidth"]);
    let ah = n(&observed["screen"]["availHeight"]);
    push(
        "Screen size",
        format!(
            "{}x{} (avail {}x{})",
            fp.screen.width, fp.screen.height, fp.screen.avail_width, fp.screen.avail_height
        ),
        format!("{sw}x{sh} (avail {aw}x{ah})"),
        sw == fp.screen.width as u64
            && sh == fp.screen.height as u64
            && aw == fp.screen.avail_width as u64
            && ah == fp.screen.avail_height as u64,
        10,
        "屏幕尺寸由 JS Hook 覆盖，若被页面覆盖会不一致",
    );
    let dpr = f(&observed["screen"]["pixelRatio"]);
    push(
        "devicePixelRatio",
        format!("{:.2}", fp.screen.pixel_ratio),
        format!("{dpr:.2}"),
        (dpr - fp.screen.pixel_ratio).abs() < 0.01,
        4,
        "检查 window.devicePixelRatio 覆盖",
    );
    let hc = n(&observed["hardwareConcurrency"]);
    push(
        "hardwareConcurrency",
        fp.hardware_concurrency.to_string(),
        hc.to_string(),
        hc == fp.hardware_concurrency as u64,
        5,
        "navigator.hardwareConcurrency 被页面重写",
    );
    let dm = n(&observed["deviceMemory"]);
    push(
        "deviceMemory",
        fp.device_memory.to_string(),
        dm.to_string(),
        dm == fp.device_memory as u64,
        4,
        "navigator.deviceMemory 被页面重写",
    );
    let gl_vendor = s(&observed["webgl"]["unmaskedVendor"]);
    let gl_renderer = s(&observed["webgl"]["unmaskedRenderer"]);
    push(
        "WebGL vendor",
        fp.webgl.vendor.clone(),
        gl_vendor.clone(),
        gl_vendor == fp.webgl.vendor || gl_vendor.is_empty(),
        7,
        "无法读取 unmasked vendor 时不计为失败，但应复核",
    );
    push(
        "WebGL renderer",
        fp.webgl.renderer.clone(),
        gl_renderer.clone(),
        gl_renderer == fp.webgl.renderer || gl_renderer.is_empty(),
        7,
        "WebGL renderer 与配置不一致",
    );
    let canvas_hash = s(&observed["canvas"]["hash"]);
    push(
        "Canvas",
        if fp.canvas.mode == "noise" {
            "noise enabled".into()
        } else {
            "native".into()
        },
        if canvas_hash.is_empty() {
            "unavailable".into()
        } else {
            format!("hash {canvas_hash}")
        },
        !canvas_hash.is_empty(),
        5,
        "Canvas 读取失败，可能被浏览器策略阻止",
    );
    let audio_hash = s(&observed["audio"]["hash"]);
    push(
        "Audio",
        if fp.audio.mode == "noise" {
            "noise enabled".into()
        } else {
            "native".into()
        },
        if audio_hash.is_empty() {
            "unavailable".into()
        } else {
            format!("hash {audio_hash}")
        },
        !audio_hash.is_empty(),
        4,
        "AudioContext 读取失败，可能被浏览器策略阻止",
    );
    let wd = observed["webdriver"].as_bool().unwrap_or(true);
    push(
        "navigator.webdriver",
        "false".into(),
        wd.to_string(),
        !wd,
        8,
        "webdriver=true 会直接暴露自动化特征",
    );
    let ch_platform = s(&observed["clientHints"]["platform"]);
    push(
        "Client Hints platform",
        fp.client_hints.platform.clone(),
        ch_platform.clone(),
        ch_platform.is_empty() || ch_platform == fp.client_hints.platform,
        5,
        "Client Hints 与配置不一致",
    );
    let font_values = &observed["fonts"];
    let found = fp
        .fonts
        .iter()
        .filter(|font| {
            font_values
                .get(font.as_str())
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        })
        .count();
    let expect_fonts = fp.fonts.len().min(5);
    push(
        "Fonts",
        format!("至少 {expect_fonts} 个配置字体可用"),
        format!("{found} 个可用"),
        found >= expect_fonts,
        5,
        "字体可用性与配置不一致，可能影响字体指纹",
    );
    let plugins = observed["plugins"].as_array().cloned().unwrap_or_default();
    push(
        "Plugins",
        format!("{} 个插件", fp.plugins.len()),
        format!("{} 个插件", plugins.len()),
        plugins.len() == fp.plugins.len(),
        3,
        "插件列表被浏览器或页面修改",
    );

    let total: u32 = checks.iter().map(|c| c.weight).sum();
    let earned: u32 = checks.iter().filter(|c| c.passed).map(|c| c.weight).sum();
    let score = earned.saturating_mul(100) / total.max(1);
    (checks, score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_templates_are_consistent() {
        for os in ["win11", "win10", "mac", "linux"] {
            let fp = template(os);
            let report = validate(&fp);
            assert!(
                report.valid,
                "template {os} should be valid: {:?}",
                report.errors
            );
            assert!(fp.user_agent.contains("Chrome/"));
            assert!(!fp.fonts.is_empty());
            assert!(!fp.client_hints.brands.is_empty());
        }
    }

    #[test]
    fn inconsistent_platform_and_hints_are_rejected() {
        let mut fp = template("win11");
        fp.platform = "MacIntel".to_string();
        fp.client_hints.platform = "macOS".into();
        let report = validate(&fp);
        assert!(!report.valid);
        assert!(report.errors.iter().any(|issue| issue.field == "platform"));
    }

    #[test]
    fn invalid_timezone_and_public_webrtc_are_rejected() {
        let mut fp = template("win11");
        fp.timezone = "Mars/Olympus".into();
        fp.webrtc.mode = "public".into();
        fp.webrtc.public_ip = Some("999.1.1.1".into());
        let report = validate(&fp);
        assert!(report.errors.iter().any(|issue| issue.field == "timezone"));
        assert!(report
            .errors
            .iter()
            .any(|issue| issue.field == "webrtc.publicIp"));
    }

    #[test]
    fn country_locale_alignment_uses_iana_timezones() {
        let (tz, languages) = locale_for_country("US");
        assert_eq!(tz, "America/New_York");
        assert_eq!(languages[0], "en-US");
        let mut fp = template("win11");
        apply_country_locale(&mut fp, "JP");
        assert_eq!(fp.timezone, "Asia/Tokyo");
        assert_eq!(fp.languages[0], "ja-JP");
        assert!(validate(&fp).valid);
    }

    #[test]
    fn injection_script_contains_expected_hooks() {
        let fp = template("mac");
        let script = injection_script(&fp);
        assert!(!script.contains("__FINENV_FP__"));
        assert!(script.contains("navigator"));
        assert!(script.contains("userAgent"));
        assert!(script.contains("RTCPeerConnection"));
        assert!(script.contains("getParameter"));
        assert!(script.contains("FontFaceSet"));
    }
}
