CREATE TABLE projects (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL CHECK (length(trim(name)) > 0),
    short_name TEXT,
    color TEXT,
    customer TEXT,
    archived_at_utc_ms INTEGER,
    created_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE workdays (
    id INTEGER PRIMARY KEY,
    local_date TEXT NOT NULL CHECK (
        local_date GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'
        AND date(local_date, '+0 days') IS local_date
    ),
    timezone TEXT NOT NULL CHECK (length(trim(timezone)) > 0),
    started_at_utc_ms INTEGER NOT NULL,
    ended_at_utc_ms INTEGER CHECK (ended_at_utc_ms >= started_at_utc_ms),
    target_seconds_snapshot INTEGER CHECK (target_seconds_snapshot BETWEEN 0 AND 4294967295)
) STRICT;

CREATE UNIQUE INDEX one_open_workday ON workdays ((1)) WHERE ended_at_utc_ms IS NULL;
CREATE INDEX workdays_by_local_date ON workdays (local_date);

-- Work and breaks share one interval table so the same invariant covers both kinds.
CREATE TABLE timeline_entries (
    id INTEGER PRIMARY KEY,
    workday_id INTEGER NOT NULL REFERENCES workdays (id),
    kind TEXT NOT NULL CHECK (kind IN ('work', 'break')),
    project_id INTEGER REFERENCES projects (id),
    started_at_utc_ms INTEGER NOT NULL,
    ended_at_utc_ms INTEGER CHECK (ended_at_utc_ms >= started_at_utc_ms),
    activity_text TEXT,
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    work_status TEXT GENERATED ALWAYS AS (
        CASE
            WHEN kind = 'break' THEN NULL
            WHEN ended_at_utc_ms IS NULL THEN 'active'
            -- Unicode White_Space, matching Rust str::trim.
            WHEN activity_text IS NULL OR trim(activity_text, char(
                9,10,11,12,13,32,133,160,5760,8192,8193,8194,8195,8196,8197,
                8198,8199,8200,8201,8202,8232,8233,8239,8287,12288
            )) = '' THEN 'pending_description'
            ELSE 'complete'
        END
    ) VIRTUAL,
    CHECK (kind = 'work' OR (project_id IS NULL AND activity_text IS NULL))
) STRICT;

CREATE UNIQUE INDEX one_open_entry ON timeline_entries ((1)) WHERE ended_at_utc_ms IS NULL;
CREATE INDEX entries_by_workday_and_start ON timeline_entries (workday_id, started_at_utc_ms);
CREATE INDEX entries_by_start ON timeline_entries (started_at_utc_ms);
CREATE INDEX entries_by_project ON timeline_entries (project_id);

CREATE TRIGGER validate_entry_insert BEFORE INSERT ON timeline_entries
BEGIN
    SELECT RAISE(ABORT, 'timeline entry is outside its workday')
    WHERE NOT EXISTS (
        SELECT 1 FROM workdays AS day
        WHERE day.id = NEW.workday_id
          AND day.started_at_utc_ms <= NEW.started_at_utc_ms
          AND (day.ended_at_utc_ms IS NULL OR NEW.ended_at_utc_ms <= day.ended_at_utc_ms)
    );

    SELECT RAISE(ABORT, 'timeline entries overlap')
    WHERE NEW.ended_at_utc_ms IS NOT NEW.started_at_utc_ms AND EXISTS (
        SELECT 1 FROM timeline_entries AS entry
        WHERE entry.ended_at_utc_ms IS NOT entry.started_at_utc_ms
          AND (NEW.ended_at_utc_ms IS NULL OR entry.started_at_utc_ms < NEW.ended_at_utc_ms)
          AND (entry.ended_at_utc_ms IS NULL OR NEW.started_at_utc_ms < entry.ended_at_utc_ms)
    );
END;

CREATE TRIGGER validate_entry_update
BEFORE UPDATE OF workday_id, started_at_utc_ms, ended_at_utc_ms ON timeline_entries
BEGIN
    SELECT RAISE(ABORT, 'timeline entry is outside its workday')
    WHERE NOT EXISTS (
        SELECT 1 FROM workdays AS day
        WHERE day.id = NEW.workday_id
          AND day.started_at_utc_ms <= NEW.started_at_utc_ms
          AND (day.ended_at_utc_ms IS NULL OR NEW.ended_at_utc_ms <= day.ended_at_utc_ms)
    );

    SELECT RAISE(ABORT, 'timeline entries overlap')
    WHERE NEW.ended_at_utc_ms IS NOT NEW.started_at_utc_ms AND EXISTS (
        SELECT 1 FROM timeline_entries AS entry
        WHERE entry.id <> OLD.id
          AND entry.ended_at_utc_ms IS NOT entry.started_at_utc_ms
          AND (NEW.ended_at_utc_ms IS NULL OR entry.started_at_utc_ms < NEW.ended_at_utc_ms)
          AND (entry.ended_at_utc_ms IS NULL OR NEW.started_at_utc_ms < entry.ended_at_utc_ms)
    );
END;

CREATE TRIGGER validate_workday_bounds
BEFORE UPDATE OF started_at_utc_ms, ended_at_utc_ms ON workdays
BEGIN
    SELECT RAISE(ABORT, 'workday bounds exclude an existing timeline entry')
    WHERE EXISTS (
        SELECT 1 FROM timeline_entries AS entry
        WHERE entry.workday_id = OLD.id
          AND (entry.started_at_utc_ms < NEW.started_at_utc_ms
               OR (NEW.ended_at_utc_ms IS NOT NULL
                   AND (entry.ended_at_utc_ms IS NULL OR entry.ended_at_utc_ms > NEW.ended_at_utc_ms)))
    );
END;

CREATE TABLE settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
) STRICT;
