PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    color TEXT NOT NULL DEFAULT '#165DFF',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS proxies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    protocol TEXT NOT NULL,
    host TEXT NOT NULL,
    port INTEGER NOT NULL,
    username TEXT NOT NULL DEFAULT '',
    password_enc TEXT NOT NULL DEFAULT '',
    enabled INTEGER NOT NULL DEFAULT 1,
    last_check_at TEXT,
    latency_ms INTEGER,
    exit_ip TEXT,
    exit_country TEXT,
    exit_timezone TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS environments (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    group_id TEXT REFERENCES groups(id) ON DELETE SET NULL,
    status TEXT NOT NULL DEFAULT 'active',
    profile_dir TEXT NOT NULL,
    proxy_id TEXT REFERENCES proxies(id) ON DELETE SET NULL,
    fingerprint_snapshot_id TEXT,
    project TEXT NOT NULL DEFAULT '',
    test_env TEXT NOT NULL DEFAULT '',
    account_role TEXT NOT NULL DEFAULT '',
    version TEXT NOT NULL DEFAULT '',
    case_id TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    notes TEXT NOT NULL DEFAULT '',
    kernel TEXT NOT NULL DEFAULT 'chromium',
    chromium_path TEXT,
    auto_restart INTEGER NOT NULL DEFAULT 0,
    memory_limit_mb INTEGER,
    idle_timeout_min INTEGER,
    fingerprint_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    last_opened_at TEXT
);

CREATE TABLE IF NOT EXISTS accounts (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    username TEXT NOT NULL,
    password_enc TEXT NOT NULL DEFAULT '',
    remark TEXT NOT NULL DEFAULT '',
    owner TEXT NOT NULL DEFAULT '',
    status TEXT NOT NULL DEFAULT 'pending',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    UNIQUE(env_id, username)
);

CREATE TABLE IF NOT EXISTS fingerprint_snapshots (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    name TEXT NOT NULL,
    fingerprint_json TEXT NOT NULL,
    create_time TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS fingerprint_reports (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    score INTEGER NOT NULL,
    checks_json TEXT NOT NULL,
    observed_json TEXT NOT NULL,
    configured_json TEXT NOT NULL,
    screenshot_path TEXT,
    external_url TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_logs (
    id TEXT PRIMARY KEY,
    actor TEXT NOT NULL,
    action TEXT NOT NULL,
    env_id TEXT,
    account TEXT,
    project TEXT,
    case_id TEXT,
    detail TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL,
    prev_hash TEXT NOT NULL,
    hash TEXT NOT NULL
);

CREATE TRIGGER IF NOT EXISTS audit_logs_no_update
BEFORE UPDATE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is append-only');
END;

CREATE TRIGGER IF NOT EXISTS audit_logs_no_delete
BEFORE DELETE ON audit_logs
BEGIN
    SELECT RAISE(ABORT, 'audit_logs is append-only');
END;

CREATE TABLE IF NOT EXISTS theme_config (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    mode TEXT NOT NULL DEFAULT 'auto',
    primary_color TEXT NOT NULL DEFAULT '#165DFF',
    density TEXT NOT NULL DEFAULT 'comfortable',
    config_json TEXT NOT NULL DEFAULT '{}',
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    display_name TEXT NOT NULL,
    role TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    last_login_at TEXT
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS api_keys (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prefix TEXT NOT NULL,
    key_hash TEXT NOT NULL UNIQUE,
    role TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL,
    last_used_at TEXT
);

CREATE TABLE IF NOT EXISTS extensions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    description TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS env_extensions (
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    extension_id TEXT NOT NULL REFERENCES extensions(id) ON DELETE CASCADE,
    PRIMARY KEY (env_id, extension_id)
);

CREATE TABLE IF NOT EXISTS hosts_entries (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    host TEXT NOT NULL,
    ip TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    UNIQUE(env_id, host)
);

CREATE TABLE IF NOT EXISTS mock_rules (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    pattern TEXT NOT NULL,
    method TEXT NOT NULL DEFAULT 'ANY',
    status_code INTEGER NOT NULL DEFAULT 200,
    headers_json TEXT NOT NULL DEFAULT '{}',
    body TEXT NOT NULL DEFAULT '',
    delay_ms INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS captcha_events (
    id TEXT PRIMARY KEY,
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    url TEXT NOT NULL DEFAULT '',
    source TEXT NOT NULL DEFAULT 'auto',
    hint TEXT NOT NULL DEFAULT '',
    screenshot_path TEXT,
    handled INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS env_cookies (
    env_id TEXT PRIMARY KEY REFERENCES environments(id) ON DELETE CASCADE,
    cookies_enc TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS browser_sessions (
    env_id TEXT PRIMARY KEY REFERENCES environments(id) ON DELETE CASCADE,
    pid INTEGER NOT NULL,
    debug_port INTEGER NOT NULL,
    ws_endpoint TEXT NOT NULL,
    started_at TEXT NOT NULL,
    last_active_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_env_group ON environments(group_id);
CREATE INDEX IF NOT EXISTS idx_env_status ON environments(status);
CREATE INDEX IF NOT EXISTS idx_env_project ON environments(project);
CREATE INDEX IF NOT EXISTS idx_accounts_env ON accounts(env_id);
CREATE INDEX IF NOT EXISTS idx_accounts_status ON accounts(status);
CREATE INDEX IF NOT EXISTS idx_snapshots_env ON fingerprint_snapshots(env_id);
CREATE INDEX IF NOT EXISTS idx_reports_env ON fingerprint_reports(env_id);
CREATE INDEX IF NOT EXISTS idx_audit_created ON audit_logs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_audit_env ON audit_logs(env_id);
CREATE INDEX IF NOT EXISTS idx_hosts_env ON hosts_entries(env_id);
CREATE INDEX IF NOT EXISTS idx_mocks_env ON mock_rules(env_id);
CREATE INDEX IF NOT EXISTS idx_captcha_env ON captcha_events(env_id, handled);
CREATE INDEX IF NOT EXISTS idx_api_keys_hash ON api_keys(key_hash);

INSERT OR IGNORE INTO theme_config (id, mode, primary_color, density, config_json, updated_at)
VALUES (1, 'auto', '#165DFF', 'comfortable', '{}', datetime('now'));
