-- Per-environment network emulation profile (none/3g/4g/5g/slow/lossy/offline).
ALTER TABLE environments ADD COLUMN network_profile TEXT NOT NULL DEFAULT 'none';
ALTER TABLE environment_templates ADD COLUMN network_profile TEXT NOT NULL DEFAULT 'none';
