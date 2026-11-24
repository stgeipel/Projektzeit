use chrono_tz::Tz;
use projektzeit_core::Timestamp;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

use super::{Error, Kind, check_project};

pub struct EntryInput<'a> {
    pub kind: Kind,
    pub project: Option<i64>,
    pub zone: Tz,
    pub start: Timestamp,
    pub end: Timestamp,
    pub activity: &'a str,
}

pub fn save_entry(
    db: &mut Connection,
    id: Option<i64>,
    input: EntryInput<'_>,
    at: Timestamp,
) -> Result<i64, Error> {
    let start = input.start.timestamp_millis();
    let end = input.end.timestamp_millis();
    let now = at.timestamp_millis();
    if start >= end || end > now {
        return Err(Error::InvalidRange);
    }
    let (project, activity) = match input.kind {
        Kind::Work => {
            let activity = input.activity.trim();
            if activity.is_empty() {
                return Err(Error::ActivityRequired);
            }
            (input.project, Some(activity))
        }
        Kind::Break => (None, None),
    };
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    check_project(&tx, project, false)?;
    let date = input
        .start
        .with_timezone(&input.zone)
        .date_naive()
        .to_string();
    let prior = id.map(|id| closed_entry_day(&tx, id)).transpose()?;
    let day = tx
        .query_row(
            "SELECT id FROM workdays WHERE local_date=?1 AND timezone=?2
         ORDER BY id IS ?3 DESC, ended_at_utc_ms IS NULL DESC, id DESC LIMIT 1",
            params![date, input.zone.name(), prior],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let day = if let Some(day) = day {
        tx.execute(
            "UPDATE workdays SET started_at_utc_ms=min(started_at_utc_ms,?1),
            ended_at_utc_ms=max(ended_at_utc_ms,?2) WHERE id=?3",
            params![start, end, day],
        )?;
        day
    } else {
        tx.execute(
            "INSERT INTO workdays (local_date,timezone,started_at_utc_ms,ended_at_utc_ms,finalized)
            VALUES (?1,?2,?3,?4,0)",
            params![date, input.zone.name(), start, end],
        )?;
        tx.last_insert_rowid()
    };
    let id = if let Some(id) = id {
        tx.execute("UPDATE timeline_entries SET workday_id=?1, kind=?2, project_id=?3, started_at_utc_ms=?4,
                   ended_at_utc_ms=?5, activity_text=?6, updated_at_utc_ms=?7 WHERE id=?8",
                   params![day,input.kind.sql(),project,start,end,activity,now,id])?;
        if let Some(prior) = prior.filter(|prior| *prior != day) {
            prune(&tx, prior)?;
        }
        id
    } else {
        tx.execute("INSERT INTO timeline_entries (workday_id,kind,project_id,started_at_utc_ms,ended_at_utc_ms,
                   activity_text,created_at_utc_ms,updated_at_utc_ms) VALUES (?1,?2,?3,?4,?5,?6,?7,?7)",
                   params![day,input.kind.sql(),project,start,end,activity,now])?;
        tx.last_insert_rowid()
    };
    tx.commit()?;
    Ok(id)
}

pub fn delete_entry(db: &mut Connection, id: i64) -> Result<(), Error> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let day = closed_entry_day(&tx, id)?;
    tx.execute("DELETE FROM timeline_entries WHERE id=?1", [id])?;
    prune(&tx, day)?;
    tx.commit()?;
    Ok(())
}

fn closed_entry_day(db: &Connection, id: i64) -> Result<i64, Error> {
    let (day, closed) = db
        .query_row(
            "SELECT workday_id, ended_at_utc_ms IS NOT NULL FROM timeline_entries WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()?
        .ok_or(Error::Missing)?;
    if closed {
        Ok(day)
    } else {
        Err(Error::OpenEntry)
    }
}

fn prune(db: &Connection, day: i64) -> Result<(), Error> {
    db.execute(
        "DELETE FROM workdays WHERE id=?1 AND ended_at_utc_ms IS NOT NULL
        AND NOT EXISTS(SELECT 1 FROM timeline_entries WHERE workday_id=?1)",
        [day],
    )?;
    Ok(())
}

pub fn save_bounds(
    db: &mut Connection,
    day: i64,
    start: Timestamp,
    end: Timestamp,
    at: Timestamp,
) -> Result<(), Error> {
    if start >= end || end > at {
        return Err(Error::InvalidRange);
    }
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let (zone, old_start, old_end) = day_bounds(&tx, day)?;
    let date = start.with_timezone(&zone).date_naive().to_string();
    let start = start.timestamp_millis();
    let end = end.timestamp_millis();
    let old_start = old_start.timestamp_millis();
    let old_end = old_end.timestamp_millis();

    // Apply the union first so the interval trigger can validate both edge edits.
    tx.execute(
        "UPDATE workdays SET started_at_utc_ms=min(started_at_utc_ms,?1),
         ended_at_utc_ms=max(ended_at_utc_ms,?2) WHERE id=?3",
        params![start, end, day],
    )?;
    let edge = |ordering| {
        // Ordering is selected by the caller from the two fixed SQL clauses below.
        let sql = format!(
            "SELECT id, started_at_utc_ms, ended_at_utc_ms FROM timeline_entries
            WHERE workday_id=?1 ORDER BY {ordering} LIMIT 1"
        );
        tx.query_row(&sql, [day], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .optional()
    };
    let first = edge("started_at_utc_ms, id")?;
    let last = edge("ended_at_utc_ms DESC, id DESC")?;
    for (id, original_start, original_end) in first
        .into_iter()
        .chain(last.filter(|last| first != Some(*last)))
    {
        let corrected_start =
            if first.is_some_and(|first| first.0 == id) && original_start == old_start {
                start
            } else {
                original_start
            };
        let corrected_end = if last.is_some_and(|last| last.0 == id) && original_end == old_end {
            end
        } else {
            original_end
        };
        if corrected_start > corrected_end
            || (corrected_start == corrected_end && original_start != original_end)
        {
            return Err(Error::InvalidRange);
        }
        if (corrected_start, corrected_end) != (original_start, original_end) {
            tx.execute(
                "UPDATE timeline_entries SET started_at_utc_ms=?1, ended_at_utc_ms=?2,
                 updated_at_utc_ms=?3 WHERE id=?4",
                params![corrected_start, corrected_end, at.timestamp_millis(), id],
            )?;
        }
    }
    tx.execute(
        "UPDATE workdays SET local_date=?1, started_at_utc_ms=?2, ended_at_utc_ms=?3,
         checkpoint_utc_ms=CASE WHEN checkpoint_utc_ms IS NULL THEN NULL ELSE ?3 END WHERE id=?4",
        params![date, start, end, day],
    )?;
    tx.commit()?;
    Ok(())
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub id: i64,
    pub workday_id: i64,
    pub kind: Kind,
    pub project: Option<i64>,
    pub project_name: Option<String>,
    pub project_color: Option<String>,
    pub activity: String,
    pub zone: Tz,
    pub start: Timestamp,
    pub end: Option<Timestamp>,
}

pub fn entries(db: &Connection, lower: Timestamp, upper: Timestamp) -> Result<Vec<Entry>, Error> {
    let mut statement = db.prepare(
        "SELECT e.id, e.workday_id, e.kind, e.project_id, p.name, coalesce(e.activity_text,''), d.timezone,
                e.started_at_utc_ms, e.ended_at_utc_ms, p.color
         FROM timeline_entries e JOIN workdays d ON d.id=e.workday_id
         LEFT JOIN projects p ON p.id=e.project_id
         WHERE e.started_at_utc_ms < ?2 AND (e.ended_at_utc_ms IS NULL OR e.ended_at_utc_ms >= ?1)
         ORDER BY e.started_at_utc_ms, e.id",
    )?;
    statement
        .query_map(
            params![lower.timestamp_millis(), upper.timestamp_millis()],
            row_entry,
        )?
        .map(|row| parse_entry(row?))
        .collect()
}

pub fn entry(db: &Connection, id: i64) -> Result<Entry, Error> {
    let raw = db.query_row(
        "SELECT e.id, e.workday_id, e.kind, e.project_id, p.name, coalesce(e.activity_text,''), d.timezone,
                e.started_at_utc_ms, e.ended_at_utc_ms, p.color
         FROM timeline_entries e JOIN workdays d ON d.id=e.workday_id
         LEFT JOIN projects p ON p.id=e.project_id WHERE e.id=?1",
        [id], row_entry,
    ).optional()?.ok_or(Error::Missing)?;
    parse_entry(raw)
}

struct RawEntry {
    id: i64,
    day: i64,
    kind: String,
    project: Option<i64>,
    project_name: Option<String>,
    project_color: Option<String>,
    activity: String,
    zone: String,
    start: i64,
    end: Option<i64>,
}

fn row_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawEntry> {
    Ok(RawEntry {
        id: row.get(0)?,
        day: row.get(1)?,
        kind: row.get(2)?,
        project: row.get(3)?,
        project_name: row.get(4)?,
        activity: row.get(5)?,
        zone: row.get(6)?,
        start: row.get(7)?,
        end: row.get(8)?,
        project_color: row.get(9)?,
    })
}

fn parse_entry(raw: RawEntry) -> Result<Entry, Error> {
    Ok(Entry {
        id: raw.id,
        workday_id: raw.day,
        kind: Kind::read(&raw.kind)?,
        project: raw.project,
        project_name: raw.project_name,
        project_color: raw.project_color,
        activity: raw.activity,
        zone: raw.zone.parse().map_err(|_| Error::InvalidData)?,
        start: super::timestamp(raw.start)?,
        end: raw.end.map(super::timestamp).transpose()?,
    })
}

pub fn day_bounds(db: &Connection, day: i64) -> Result<(Tz, Timestamp, Timestamp), Error> {
    let (zone, start, end) = db
        .query_row(
            "SELECT timezone, started_at_utc_ms, ended_at_utc_ms FROM workdays WHERE id=?1",
            [day],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            },
        )
        .optional()?
        .ok_or(Error::Missing)?;
    let zone = zone.parse().map_err(|_| Error::InvalidData)?;
    Ok((
        zone,
        super::timestamp(start)?,
        super::timestamp(end.ok_or(Error::OpenEntry)?)?,
    ))
}
