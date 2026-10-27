-- Environment identity: per-environment accent color and last observed egress IP.
-- IP columns are informational and intentionally not encrypted (not a credential).
ALTER TABLE environments ADD COLUMN border_color TEXT NOT NULL DEFAULT '';
ALTER TABLE environments ADD COLUMN ip_address TEXT;
ALTER TABLE environments ADD COLUMN ip_country TEXT;
ALTER TABLE environments ADD COLUMN ip_country_code TEXT;
ALTER TABLE environments ADD COLUMN ip_city TEXT;
ALTER TABLE environments ADD COLUMN ip_timezone TEXT;
ALTER TABLE environments ADD COLUMN ip_source TEXT;
ALTER TABLE environments ADD COLUMN ip_checked_at TEXT;

CREATE INDEX IF NOT EXISTS idx_env_ip_checked ON environments(ip_checked_at DESC);
