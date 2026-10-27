-- Hardening migration: ensure append-only audit guarantees also exist for
-- databases created by earlier builds of FinEnv.
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

CREATE INDEX IF NOT EXISTS idx_env_last_opened ON environments(last_opened_at DESC);
CREATE INDEX IF NOT EXISTS idx_captcha_created ON captcha_events(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_reports_created ON fingerprint_reports(created_at DESC);
