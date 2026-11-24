ALTER TABLE projects ADD COLUMN description TEXT;
ALTER TABLE workdays ADD COLUMN checkpoint_utc_ms INTEGER;
ALTER TABLE workdays ADD COLUMN finalized INTEGER NOT NULL DEFAULT 1 CHECK (finalized IN (0, 1));
