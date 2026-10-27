-- Adjustable thickness for the per-environment accent color in the panel.
ALTER TABLE environments ADD COLUMN border_width INTEGER NOT NULL DEFAULT 3;
