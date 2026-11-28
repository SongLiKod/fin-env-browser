PRAGMA foreign_keys = ON;

ALTER TABLE proxies ADD COLUMN last_check_target TEXT NOT NULL DEFAULT '';
