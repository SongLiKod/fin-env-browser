-- Per-environment access control for panel users.
CREATE TABLE IF NOT EXISTS environment_members (
    env_id TEXT NOT NULL REFERENCES environments(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    access_level TEXT NOT NULL DEFAULT 'view',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY (env_id, user_id)
);

CREATE INDEX IF NOT EXISTS idx_environment_members_user
    ON environment_members(user_id);
