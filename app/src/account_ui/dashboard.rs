use chrono::{DateTime, NaiveDate, TimeDelta};
use chrono_tz::Tz;
use projektzeit_core::{TimeSpan, Timestamp, worktime_account::visit_local_intervals};
use rusqlite::{Connection, params};

use super::{
    account_error, dash, duration,
    view::{ProjectRow, SegmentRow},
};

#[derive(Default)]
pub(super) struct Timeline {
    pub rows: Vec<SegmentRow>,
    pub work_seconds: i64,
    pub break_seconds: i64,
}

pub(super) fn timeline(
    db: &Connection,
    date: NaiveDate,
    observed_at: Timestamp,
) -> Result<Timeline, String> {
    // All recorded zone offsets are strictly within ±24 hours. The final date
    // filter uses each workday's zone, including days spanning midnight or DST.
    let midnight = date.and_hms_opt(0, 0, 0).ok_or_else(read_error)?.and_utc();
    let lower = midnight - TimeDelta::days(1);
    let upper = (midnight + TimeDelta::days(2)).min(observed_at);
    if upper < lower {
        return Ok(Timeline::default());
    }
    let mut statement = db
        .prepare(
            "SELECT day.timezone, entry.started_at_utc_ms, entry.ended_at_utc_ms,
                entry.kind = 'break', entry.activity_text, project.name
         FROM timeline_entries AS entry
         JOIN workdays AS day ON day.id = entry.workday_id
         LEFT JOIN projects AS project ON project.id = entry.project_id
         WHERE entry.started_at_utc_ms <= ?1
           AND (entry.ended_at_utc_ms IS NULL OR entry.ended_at_utc_ms >= ?2)
         ORDER BY entry.started_at_utc_ms, entry.id",
        )
        .map_err(sql_error)?;
    let rows = statement
        .query_map(
            params![upper.timestamp_millis(), lower.timestamp_millis()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                ))
            },
        )
        .map_err(sql_error)?;
    let mut timeline = Timeline::default();
    let mut work_millis = 0;
    let mut break_millis = 0;
    for row in rows {
        let (zone, start, end, is_break, activity, project) = row.map_err(sql_error)?;
        let zone: Tz = zone.parse().map_err(|_| read_error())?;
        let start = DateTime::from_timestamp_millis(start).ok_or_else(read_error)?;
        let end = end
            .map(|value| DateTime::from_timestamp_millis(value).ok_or_else(read_error))
            .transpose()?;
        let upper_bound = end.unwrap_or(upper).min(upper);
        if upper_bound < start {
            continue;
        }
        let mut add_row = |start: Timestamp, finish: Timestamp| {
            let elapsed = (finish - start).num_milliseconds();
            if end.is_some() {
                if is_break {
                    break_millis += elapsed;
                } else {
                    work_millis += elapsed;
                }
            }
            let activity = activity
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty());
            let pending = !is_break && end.is_some() && activity.is_none();
            let label = if is_break {
                "Pause"
            } else {
                project.as_deref().unwrap_or("Ohne Projekt")
            };
            let detail = if end.is_none() {
                "Offen · Dauer noch nicht bestätigt"
            } else if is_break {
                ""
            } else {
                activity.unwrap_or("Beschreibung fehlt")
            };
            let local_time =
                |instant: Timestamp| instant.with_timezone(&zone).format("%H:%M").to_string();
            let time = if end.is_some() {
                format!("{} – {}", local_time(start), local_time(finish))
            } else {
                format!("{} – offen", local_time(start))
            };
            timeline.rows.push(SegmentRow {
                title: label.into(),
                detail: detail.into(),
                time: time.into(),
                amount: if end.is_some() {
                    duration(elapsed / 1000, false)
                } else {
                    dash()
                }
                .into(),
                pending,
            });
        };
        if start == upper_bound && start.with_timezone(&zone).date_naive() == date {
            add_row(start, start);
        } else {
            let span =
                TimeSpan::new(start.max(lower), Some(upper_bound)).map_err(|_| read_error())?;
            visit_local_intervals(span, zone, upper, |local_date, start, end| {
                if local_date == date {
                    add_row(start, end);
                }
                Ok(())
            })
            .map_err(account_error)?;
        }
    }
    timeline.work_seconds = work_millis / 1000;
    timeline.break_seconds = break_millis / 1000;
    Ok(timeline)
}

pub(super) fn projects(db: &Connection) -> Result<Vec<ProjectRow>, String> {
    let mut statement = db
        .prepare(
            "SELECT name, short_name, customer, archived_at_utc_ms IS NOT NULL
         FROM projects ORDER BY archived_at_utc_ms IS NOT NULL, name COLLATE NOCASE, id",
        )
        .map_err(sql_error)?;
    statement
        .query_map([], |row| {
            Ok(ProjectRow {
                name: row.get::<_, String>(0)?.into(),
                code: row.get::<_, Option<String>>(1)?.unwrap_or_default().into(),
                customer: row.get::<_, Option<String>>(2)?.unwrap_or_else(dash).into(),
                archived: row.get(3)?,
            })
        })
        .map_err(sql_error)?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(sql_error)
}

fn sql_error(error: rusqlite::Error) -> String {
    format!("Einträge konnten nicht geladen werden ({error}).")
}

fn read_error() -> String {
    "Ein gespeicherter Zeiteintrag enthält ein ungültiges Datum.".into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_splits_overnight_work_and_excludes_unconfirmed_time() {
        let folder = tempfile::tempdir().unwrap();
        let db = projektzeit_storage::open(folder.path().join("timeline.sqlite")).unwrap();
        let at = |text| {
            DateTime::parse_from_rfc3339(text)
                .unwrap()
                .timestamp_millis()
        };
        let start = at("2026-11-23T23:30:00+01:00");
        let end = at("2026-11-24T00:30:00+01:00");
        let after_break = at("2026-11-24T00:45:00+01:00");
        db.execute(
            "INSERT INTO workdays (id, local_date, timezone, started_at_utc_ms)
             VALUES (1, '2026-11-23', 'Europe/Berlin', ?1)",
            [start],
        )
        .unwrap();
        for (kind, start, end) in [
            ("work", start, Some(end)),
            ("break", end, Some(after_break)),
            ("work", after_break, None),
        ] {
            db.execute(
                "INSERT INTO timeline_entries (workday_id, kind, started_at_utc_ms,
                  ended_at_utc_ms, created_at_utc_ms, updated_at_utc_ms)
                 VALUES (1, ?1, ?2, ?3, ?2, ?2)",
                params![kind, start, end],
            )
            .unwrap();
        }

        let date = NaiveDate::from_ymd_opt(2026, 11, 24).unwrap();
        let now = DateTime::from_timestamp_millis(at("2026-11-24T01:00:00+01:00")).unwrap();
        let result = timeline(&db, date, now).unwrap();
        assert_eq!(result.work_seconds, 1_800);
        assert_eq!(result.break_seconds, 900);
        assert_eq!(result.rows.len(), 3);
        assert!(result.rows[0].pending);
        assert_eq!(result.rows[0].time, "00:00 – 00:30");
        assert_eq!(result.rows[1].title, "Pause");
        assert_eq!(result.rows[2].amount, "–");
    }
}
