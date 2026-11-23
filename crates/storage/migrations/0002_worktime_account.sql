CREATE TABLE work_schedules (
    id INTEGER PRIMARY KEY,
    effective_date TEXT NOT NULL CHECK (date(effective_date, '+0 days') IS effective_date),
    weekly_seconds INTEGER NOT NULL CHECK (weekly_seconds BETWEEN 0 AND 4294967295),
    monday INTEGER NOT NULL CHECK (monday >= 0),
    tuesday INTEGER NOT NULL CHECK (tuesday >= 0),
    wednesday INTEGER NOT NULL CHECK (wednesday >= 0),
    thursday INTEGER NOT NULL CHECK (thursday >= 0),
    friday INTEGER NOT NULL CHECK (friday >= 0),
    saturday INTEGER NOT NULL CHECK (saturday >= 0),
    sunday INTEGER NOT NULL CHECK (sunday >= 0),
    created_at_utc_ms INTEGER NOT NULL,
    superseded_at_utc_ms INTEGER,
    CHECK (weekly_seconds = monday + tuesday + wednesday + thursday + friday + saturday + sunday)
) STRICT;

CREATE UNIQUE INDEX active_schedule_by_date
ON work_schedules(effective_date) WHERE superseded_at_utc_ms IS NULL;

CREATE TABLE opening_balance_versions (
    id INTEGER PRIMARY KEY,
    effective_date TEXT NOT NULL CHECK (date(effective_date, '+0 days') IS effective_date),
    signed_seconds INTEGER NOT NULL,
    created_at_utc_ms INTEGER NOT NULL,
    superseded_at_utc_ms INTEGER
) STRICT;

CREATE UNIQUE INDEX one_active_opening_balance
ON opening_balance_versions((1)) WHERE superseded_at_utc_ms IS NULL;

CREATE TABLE daily_target_overrides (
    effective_date TEXT PRIMARY KEY CHECK (date(effective_date, '+0 days') IS effective_date),
    target_seconds INTEGER NOT NULL CHECK (target_seconds BETWEEN 0 AND 4294967295),
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL
) STRICT;

CREATE TABLE balance_adjustments (
    id INTEGER PRIMARY KEY,
    effective_date TEXT NOT NULL CHECK (date(effective_date, '+0 days') IS effective_date),
    signed_seconds INTEGER NOT NULL,
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    created_at_utc_ms INTEGER NOT NULL,
    updated_at_utc_ms INTEGER NOT NULL,
    voided_at_utc_ms INTEGER
) STRICT;

CREATE INDEX balance_adjustments_by_date ON balance_adjustments(effective_date);

CREATE TABLE account_audit_events (
    id INTEGER PRIMARY KEY,
    entity_type TEXT NOT NULL,
    entity_key TEXT NOT NULL,
    action TEXT NOT NULL,
    reason TEXT,
    old_value_json TEXT CHECK (json_valid(old_value_json)),
    new_value_json TEXT CHECK (json_valid(new_value_json)),
    occurred_at_utc_ms INTEGER NOT NULL
) STRICT;
