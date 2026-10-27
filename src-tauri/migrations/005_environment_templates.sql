-- Reusable environment templates for batch creation.
CREATE TABLE IF NOT EXISTS environment_templates (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
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
    border_color TEXT NOT NULL DEFAULT '',
    border_width INTEGER NOT NULL DEFAULT 3,
    group_id TEXT REFERENCES groups(id) ON DELETE SET NULL,
    proxy_mode TEXT NOT NULL DEFAULT 'none',
    proxy_id TEXT REFERENCES proxies(id) ON DELETE SET NULL,
    extension_ids TEXT NOT NULL DEFAULT '[]',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_templates_created ON environment_templates(created_at DESC);
