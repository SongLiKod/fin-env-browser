-- Per-environment plugin overrides. Global enabled_plugins remains the master switch.
CREATE TABLE IF NOT EXISTS env_plugins (
  env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
  plugin_id TEXT NOT NULL,
  enabled INTEGER NOT NULL DEFAULT 1,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (env_id, plugin_id)
);
