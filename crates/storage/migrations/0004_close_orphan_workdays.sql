-- Older schemas allowed a day with no ongoing booking. Such a day cannot
-- be resumed by the tracker. Retain the day and every committed booking
-- and leave it unfinished at the last known persisted boundary.
UPDATE workdays AS day
SET ended_at_utc_ms = max(
        day.started_at_utc_ms,
        coalesce(day.checkpoint_utc_ms, day.started_at_utc_ms),
        coalesce((SELECT max(e.ended_at_utc_ms) FROM timeline_entries AS e
                  WHERE e.workday_id = day.id), day.started_at_utc_ms)
    ),
    finalized = 0
WHERE day.ended_at_utc_ms IS NULL AND NOT EXISTS (
    SELECT 1 FROM timeline_entries AS e WHERE e.workday_id = day.id AND e.ended_at_utc_ms IS NULL
);
