-- Per-environment Chromium launch arguments (JSON array).
-- Environment values override global arguments with the same switch name.
ALTER TABLE environments ADD COLUMN chromium_launch_args TEXT NOT NULL DEFAULT '[]';
ALTER TABLE environment_templates ADD COLUMN chromium_launch_args TEXT NOT NULL DEFAULT '[]';
