-- Per-environment taskbar badge text. Empty means the default color dot.
ALTER TABLE environments ADD COLUMN taskbar_badge_text TEXT NOT NULL DEFAULT '';
ALTER TABLE environment_templates ADD COLUMN taskbar_badge_text TEXT NOT NULL DEFAULT '';
