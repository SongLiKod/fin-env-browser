PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS proxy_subscriptions (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    url_enc TEXT NOT NULL,
    headers_enc TEXT NOT NULL DEFAULT '',
    default_protocol TEXT NOT NULL DEFAULT 'http',
    group_name TEXT NOT NULL DEFAULT '',
    tags TEXT NOT NULL DEFAULT '[]',
    enabled INTEGER NOT NULL DEFAULT 1,
    auto_refresh INTEGER NOT NULL DEFAULT 0,
    refresh_interval_min INTEGER NOT NULL DEFAULT 360,
    last_refresh_at TEXT,
    last_refresh_status TEXT,
    last_refresh_message TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

ALTER TABLE proxies ADD COLUMN group_name TEXT NOT NULL DEFAULT '';
ALTER TABLE proxies ADD COLUMN tags TEXT NOT NULL DEFAULT '[]';
ALTER TABLE proxies ADD COLUMN note TEXT NOT NULL DEFAULT '';
ALTER TABLE proxies ADD COLUMN source TEXT NOT NULL DEFAULT 'manual';
ALTER TABLE proxies ADD COLUMN subscription_id TEXT REFERENCES proxy_subscriptions(id) ON DELETE SET NULL;
ALTER TABLE proxies ADD COLUMN endpoint_key TEXT NOT NULL DEFAULT '';
ALTER TABLE proxies ADD COLUMN weight INTEGER NOT NULL DEFAULT 1;
ALTER TABLE proxies ADD COLUMN last_check_ok INTEGER;
ALTER TABLE proxies ADD COLUMN last_check_message TEXT;
ALTER TABLE proxies ADD COLUMN last_check_city TEXT;
ALTER TABLE proxies ADD COLUMN fail_count INTEGER NOT NULL DEFAULT 0;
ALTER TABLE proxies ADD COLUMN disabled_reason TEXT NOT NULL DEFAULT '';

UPDATE proxies
SET endpoint_key =
    lower(trim(protocol)) || '://' ||
    lower(trim(host)) || ':' || port || '|' ||
    trim(username);

CREATE INDEX IF NOT EXISTS idx_proxies_endpoint_key ON proxies(endpoint_key);
CREATE INDEX IF NOT EXISTS idx_proxies_subscription ON proxies(subscription_id);
CREATE INDEX IF NOT EXISTS idx_proxies_group ON proxies(group_name);
CREATE INDEX IF NOT EXISTS idx_proxies_source ON proxies(source);
CREATE INDEX IF NOT EXISTS idx_proxies_enabled ON proxies(enabled);
CREATE INDEX IF NOT EXISTS idx_proxies_last_check ON proxies(last_check_at);
CREATE INDEX IF NOT EXISTS idx_proxy_subscriptions_due
    ON proxy_subscriptions(enabled, auto_refresh, last_refresh_at);
