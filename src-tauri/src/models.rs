use serde::{Deserialize, Deserializer, Serialize};

use crate::fingerprint::Fingerprint;

/// Distinguish `null` (clear the value) from an omitted field in update inputs.
fn double_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<Option<T>>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

// ---------------------------------------------------------------------------
// Shared enums / constants
// ---------------------------------------------------------------------------

pub const ACCOUNT_STATUSES: [&str; 4] = ["pending", "in_use", "frozen", "deprecated"];
pub const ENV_STATUSES: [&str; 3] = ["active", "disabled", "archived"];
pub const PROXY_PROTOCOLS: [&str; 3] = ["http", "https", "socks5"];
pub const ROLES: [&str; 4] = ["super_admin", "test_lead", "test_engineer", "viewer"];

pub fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

pub fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// ---------------------------------------------------------------------------
// Environments
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub group_id: Option<String>,
    pub status: String,
    pub profile_dir: String,
    pub proxy_id: Option<String>,
    pub fingerprint_snapshot_id: Option<String>,
    pub project: String,
    pub test_env: String,
    pub account_role: String,
    pub version: String,
    pub case_id: String,
    pub tags: Vec<String>,
    pub notes: String,
    pub kernel: String,
    pub chromium_path: Option<String>,
    pub auto_restart: bool,
    pub memory_limit_mb: Option<u32>,
    pub idle_timeout_min: Option<u32>,
    pub fingerprint: Fingerprint,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentView {
    #[serde(flatten)]
    pub env: Environment,
    pub proxy: Option<Proxy>,
    pub runtime: Option<BrowserRuntime>,
    pub account_count: u32,
    pub snapshot_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct EnvironmentFilter {
    pub search: Option<String>,
    pub group_id: Option<String>,
    pub status: Option<String>,
    pub project: Option<String>,
    pub test_env: Option<String>,
    pub tag: Option<String>,
    pub running_only: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEnvironmentInput {
    pub name: String,
    #[serde(default)]
    pub group_id: Option<String>,
    #[serde(default)]
    pub proxy_id: Option<String>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub test_env: Option<String>,
    #[serde(default)]
    pub account_role: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub case_id: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub kernel: Option<String>,
    #[serde(default)]
    pub chromium_path: Option<String>,
    #[serde(default)]
    pub template_os: Option<String>,
    #[serde(default)]
    pub fingerprint: Option<Fingerprint>,
    #[serde(default)]
    pub auto_restart: Option<bool>,
    #[serde(default)]
    pub memory_limit_mb: Option<u32>,
    #[serde(default)]
    pub idle_timeout_min: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEnvironmentInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub group_id: Option<Option<String>>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub proxy_id: Option<Option<String>>,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub test_env: Option<String>,
    #[serde(default)]
    pub account_role: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub case_id: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub kernel: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub chromium_path: Option<Option<String>>,
    #[serde(default)]
    pub auto_restart: Option<bool>,
    #[serde(default, deserialize_with = "double_option")]
    pub memory_limit_mb: Option<Option<u32>>,
    #[serde(default, deserialize_with = "double_option")]
    pub idle_timeout_min: Option<Option<u32>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: String,
    pub name: String,
    pub color: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub env_id: String,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_enc: String,
    pub has_password: bool,
    pub remark: String,
    pub owner: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInput {
    pub env_id: String,
    pub username: String,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAccountInput {
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub remark: Option<String>,
    #[serde(default)]
    pub owner: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

/// Account row used for batch import and encrypted export packages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountExportRow {
    pub username: String,
    pub password: String,
    #[serde(default)]
    pub remark: String,
    #[serde(default)]
    pub owner: String,
    #[serde(default = "default_account_status")]
    pub status: String,
}

fn default_account_status() -> String {
    "pending".into()
}

// ---------------------------------------------------------------------------
// Snapshots
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintSnapshot {
    pub id: String,
    pub env_id: String,
    pub name: String,
    pub fingerprint: Fingerprint,
    pub create_time: String,
}

// ---------------------------------------------------------------------------
// Proxies
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Proxy {
    pub id: String,
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_enc: String,
    pub has_password: bool,
    pub enabled: bool,
    pub last_check_at: Option<String>,
    pub latency_ms: Option<u64>,
    pub exit_ip: Option<String>,
    pub exit_country: Option<String>,
    pub exit_timezone: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Proxy {
    pub fn url(&self) -> String {
        format!("{}://{}:{}", self.protocol, self.host, self.port)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyInput {
    pub name: String,
    pub protocol: String,
    pub host: String,
    pub port: u16,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProxyInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub protocol: Option<String>,
    #[serde(default)]
    pub host: Option<String>,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyCheckResult {
    pub proxy_id: String,
    pub ok: bool,
    pub latency_ms: Option<u64>,
    pub exit_ip: Option<String>,
    pub country: Option<String>,
    pub timezone: Option<String>,
    pub city: Option<String>,
    pub message: String,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlignProxyResult {
    pub environment: EnvironmentView,
    pub check: ProxyCheckResult,
}

// ---------------------------------------------------------------------------
// Browser runtime
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntime {
    pub env_id: String,
    pub pid: u32,
    pub debug_port: u16,
    pub ws_endpoint: String,
    pub status: String,
    pub started_at: String,
    pub last_active_at: String,
    pub cpu_percent: f32,
    pub memory_mb: u64,
    pub current_url: String,
    pub recording: bool,
    pub har_recording: bool,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct OpenOptions {
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub pre_script: Option<String>,
    #[serde(default)]
    pub wait_until: Option<String>,
    #[serde(default)]
    pub headless: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotResult {
    pub env_id: String,
    pub path: String,
    pub data_url: String,
    pub width: u32,
    pub height: u32,
    pub captured_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieRecord {
    pub name: String,
    pub value: String,
    pub domain: String,
    pub path: String,
    #[serde(default)]
    pub secure: bool,
    #[serde(default)]
    pub http_only: bool,
    #[serde(default)]
    pub session: bool,
    #[serde(default)]
    pub expires: Option<f64>,
    #[serde(default)]
    pub same_site: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingResult {
    pub env_id: String,
    pub directory: String,
    pub frame_count: u32,
    pub duration_ms: u64,
    pub manifest_path: String,
    pub started_at: String,
    pub stopped_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdpInfo {
    pub env_id: String,
    pub debug_port: u16,
    pub ws_endpoint: String,
    pub http_endpoint: String,
    pub running: bool,
    pub playwright_example: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSnapshot {
    pub instances: Vec<BrowserRuntime>,
    pub total_memory_mb: u64,
    pub max_concurrent: u32,
    pub running_count: u32,
    pub sampled_at: String,
    pub free_memory_mb: u64,
}

// ---------------------------------------------------------------------------
// Audit
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditLog {
    pub id: String,
    pub actor: String,
    pub action: String,
    pub env_id: Option<String>,
    pub account: Option<String>,
    pub project: Option<String>,
    pub case_id: Option<String>,
    pub detail: String,
    pub created_at: String,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AuditFilter {
    pub search: Option<String>,
    pub env_id: Option<String>,
    pub action: Option<String>,
    pub actor: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditVerifyResult {
    pub valid: bool,
    pub checked: u64,
    pub broken_at: Option<String>,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Theme
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeConfig {
    pub mode: String,
    pub primary_color: String,
    pub density: String,
    #[serde(default)]
    pub config: serde_json::Value,
    pub updated_at: String,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: "auto".into(),
            primary_color: "#165DFF".into(),
            density: "comfortable".into(),
            config: serde_json::json!({}),
            updated_at: now_iso(),
        }
    }
}

// ---------------------------------------------------------------------------
// Users / auth
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserInput {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub password: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UserUpdateInput {
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub authenticated: bool,
    pub user: Option<User>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionDef {
    pub key: String,
    pub label: String,
    pub description: String,
    pub roles: Vec<String>,
}

// ---------------------------------------------------------------------------
// Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub max_concurrent_browsers: u32,
    pub idle_timeout_min: u32,
    pub memory_limit_mb: u32,
    pub memory_enforce: bool,
    pub auto_restart_on_crash: bool,
    pub api_enabled: bool,
    pub api_host: String,
    pub api_port: u16,
    pub chromium_path: Option<String>,
    pub default_start_url: String,
    pub detection_service_url: String,
    pub allow_sensitive_export: bool,
    pub callback_type: String,
    pub callback_url: Option<String>,
    #[serde(skip_serializing)]
    pub callback_token_enc: Option<String>,
    pub callback_token_set: bool,
    pub sync_enabled: bool,
    pub sync_kind: String,
    pub sync_endpoint: Option<String>,
    pub sync_username: Option<String>,
    #[serde(skip_serializing)]
    pub sync_password_enc: Option<String>,
    pub sync_password_set: bool,
    pub sync_remote_path: String,
    pub sync_last_at: Option<String>,
    pub close_to_tray: bool,
    pub global_shortcut: String,
    pub launch_at_login: bool,
    pub log_level: String,
    pub download_manifest_url: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_concurrent_browsers: 30,
            idle_timeout_min: 30,
            memory_limit_mb: 1500,
            memory_enforce: false,
            auto_restart_on_crash: false,
            api_enabled: true,
            api_host: "127.0.0.1".into(),
            api_port: 17321,
            chromium_path: None,
            default_start_url: "about:blank".into(),
            detection_service_url: "https://browserleaks.com/javascript".into(),
            allow_sensitive_export: true,
            callback_type: "generic".into(),
            callback_url: None,
            callback_token_enc: None,
            callback_token_set: false,
            sync_enabled: false,
            sync_kind: "webdav".into(),
            sync_endpoint: None,
            sync_username: None,
            sync_password_enc: None,
            sync_password_set: false,
            sync_remote_path: "finenv-backup.bin".into(),
            sync_last_at: None,
            close_to_tray: true,
            global_shortcut: "CmdOrCtrl+Shift+F".into(),
            launch_at_login: false,
            log_level: "info".into(),
            download_manifest_url: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSettingsInput {
    #[serde(default)]
    pub max_concurrent_browsers: Option<u32>,
    #[serde(default)]
    pub idle_timeout_min: Option<u32>,
    #[serde(default)]
    pub memory_limit_mb: Option<u32>,
    #[serde(default)]
    pub memory_enforce: Option<bool>,
    #[serde(default)]
    pub auto_restart_on_crash: Option<bool>,
    #[serde(default)]
    pub api_enabled: Option<bool>,
    #[serde(default)]
    pub api_host: Option<String>,
    #[serde(default)]
    pub api_port: Option<u16>,
    #[serde(default, deserialize_with = "double_option")]
    pub chromium_path: Option<Option<String>>,
    #[serde(default)]
    pub default_start_url: Option<String>,
    #[serde(default)]
    pub detection_service_url: Option<String>,
    #[serde(default)]
    pub allow_sensitive_export: Option<bool>,
    #[serde(default)]
    pub callback_type: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub callback_url: Option<Option<String>>,
    #[serde(default)]
    pub callback_token: Option<String>,
    #[serde(default)]
    pub sync_enabled: Option<bool>,
    #[serde(default)]
    pub sync_kind: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub sync_endpoint: Option<Option<String>>,
    #[serde(default, deserialize_with = "double_option")]
    pub sync_username: Option<Option<String>>,
    #[serde(default)]
    pub sync_password: Option<String>,
    #[serde(default)]
    pub sync_remote_path: Option<String>,
    #[serde(default)]
    pub close_to_tray: Option<bool>,
    #[serde(default)]
    pub global_shortcut: Option<String>,
    #[serde(default)]
    pub launch_at_login: Option<bool>,
    #[serde(default)]
    pub log_level: Option<String>,
    #[serde(default, deserialize_with = "double_option")]
    pub download_manifest_url: Option<Option<String>>,
}

// ---------------------------------------------------------------------------
// API keys
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiKey {
    pub id: String,
    pub name: String,
    pub prefix: String,
    pub role: String,
    pub enabled: bool,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatedApiKey {
    #[serde(flatten)]
    pub key: ApiKey,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiStatus {
    pub running: bool,
    pub host: String,
    pub port: u16,
    pub base_url: String,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Extensions / hosts / mocks / captcha
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Extension {
    pub id: String,
    pub name: String,
    pub path: String,
    pub enabled: bool,
    pub description: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostsEntry {
    pub id: String,
    pub env_id: String,
    pub host: String,
    pub ip: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HostsEntryInput {
    pub host: String,
    pub ip: String,
    #[serde(default)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MockRule {
    pub id: String,
    pub env_id: String,
    pub pattern: String,
    pub method: String,
    pub status_code: u16,
    pub headers: serde_json::Value,
    pub body: String,
    pub delay_ms: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MockRuleInput {
    pub pattern: String,
    #[serde(default = "default_any")]
    pub method: String,
    #[serde(default = "default_200")]
    pub status_code: u16,
    #[serde(default)]
    pub headers: serde_json::Value,
    #[serde(default)]
    pub body: String,
    #[serde(default)]
    pub delay_ms: u32,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn default_any() -> String {
    "ANY".into()
}
fn default_200() -> u16 {
    200
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptchaEvent {
    pub id: String,
    pub env_id: String,
    pub env_name: String,
    pub url: String,
    pub source: String,
    pub hint: String,
    pub screenshot_path: Option<String>,
    pub handled: bool,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Fingerprint reports
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintCheck {
    pub name: String,
    pub expected: String,
    pub actual: String,
    pub passed: bool,
    pub weight: u32,
    pub hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FingerprintReport {
    pub id: String,
    pub env_id: String,
    pub score: u32,
    pub checks: Vec<FingerprintCheck>,
    pub observed: serde_json::Value,
    pub configured: serde_json::Value,
    pub screenshot_path: Option<String>,
    pub external_url: String,
    pub created_at: String,
}

// ---------------------------------------------------------------------------
// Backup / sync
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupInfo {
    pub path: String,
    pub name: String,
    pub size: u64,
    pub encrypted: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncResult {
    pub ok: bool,
    pub message: String,
    pub synced_at: String,
    pub bytes: u64,
}

// ---------------------------------------------------------------------------
// Dashboard & app info
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardStats {
    pub env_count: u32,
    pub account_count: u32,
    pub proxy_count: u32,
    pub running_count: u32,
    pub snapshot_count: u32,
    pub audit_count: u32,
    pub total_memory_mb: u64,
    pub max_concurrent: u32,
    pub recent_audit: Vec<AuditLog>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub data_dir: String,
    pub os: String,
    pub arch: String,
    pub chromium_path: Option<String>,
    pub chromium_version: Option<String>,
    pub api_base_url: String,
    pub db_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromiumDetection {
    pub path: Option<String>,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: Option<String>,
    pub notes: Option<String>,
    pub download_url: Option<String>,
    pub message: String,
}
