//! SQLite commands and projections for a single shared time tracker.

mod entries;
pub use entries::{
    Entry, EntryInput, day_bounds, delete_entry, entries, entry, save_bounds, save_entry,
};

use std::fmt;

use chrono::{DateTime, NaiveDate};
use chrono_tz::Tz;
use projektzeit_core::Timestamp;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};

#[derive(Debug)]
pub enum Error {
    Sqlite(rusqlite::Error),
    InvalidRange,
    Overlap,
    InvalidTransition,
    ArchivedProject,
    InvalidProject,
    ActivityRequired,
    Missing,
    OpenEntry,
    InvalidData,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => error.fmt(f),
            Self::InvalidRange => f.write_str("invalid time range"),
            Self::Overlap => f.write_str("timeline overlaps existing entries"),
            Self::InvalidTransition => f.write_str("tracking state has changed"),
            Self::ArchivedProject => f.write_str("project is archived"),
            Self::InvalidProject => f.write_str("invalid project name or color"),
            Self::ActivityRequired => f.write_str("activity description required"),
            Self::Missing => f.write_str("entry no longer exists"),
            Self::OpenEntry => f.write_str("entry is still open"),
            Self::InvalidData => f.write_str("invalid stored tracking data"),
        }
    }
}

impl std::error::Error for Error {}

impl From<rusqlite::Error> for Error {
    fn from(error: rusqlite::Error) -> Self {
        match error.sqlite_error_code() {
            Some(rusqlite::ErrorCode::ConstraintViolation) => {
                let text = error.to_string();
                if text.contains("overlap") {
                    Self::Overlap
                } else if text.contains("workday bounds") || text.contains("outside its workday") {
                    Self::InvalidRange
                } else {
                    Self::Sqlite(error)
                }
            }
            _ => Self::Sqlite(error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Work,
    Break,
}

impl Kind {
    fn sql(self) -> &'static str {
        match self {
            Self::Work => "work",
            Self::Break => "break",
        }
    }

    fn read(value: &str) -> Result<Self, Error> {
        match value {
            "work" => Ok(Self::Work),
            "break" => Ok(Self::Break),
            _ => Err(Error::InvalidData),
        }
    }
}

#[derive(Debug)]
pub struct Project {
    pub id: i64,
    pub name: String,
    pub description: String,
    pub color: String,
    pub archived: bool,
}

pub fn projects(db: &Connection) -> Result<Vec<Project>, Error> {
    let mut stmt = db.prepare(
        "SELECT id, name, coalesce(description, ''), coalesce(color, '#0E5A47'), archived_at_utc_ms IS NOT NULL
         FROM projects ORDER BY archived_at_utc_ms IS NOT NULL, name COLLATE NOCASE, id",
    )?;
    let values = stmt
        .query_map([], |row| {
            Ok(Project {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                color: row.get(3)?,
                archived: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(values)
}

pub fn save_project(
    db: &Connection,
    id: Option<i64>,
    name: &str,
    description: &str,
    color: &str,
    at: Timestamp,
) -> Result<i64, Error> {
    let name = name.trim();
    let color = color.trim();
    if name.is_empty()
        || name.chars().count() > 100
        || !color
            .strip_prefix('#')
            .is_some_and(|hex| hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(Error::InvalidProject);
    }
    if let Some(id) = id {
        if db.execute(
            "UPDATE projects SET name=?1, description=?2, color=?3 WHERE id=?4",
            params![name, description.trim(), color, id],
        )? == 0
        {
            return Err(Error::Missing);
        }
        Ok(id)
    } else {
        db.execute("INSERT INTO projects (name, description, color, created_at_utc_ms) VALUES (?1,?2,?3,?4)",
            params![name, description.trim(), color, at.timestamp_millis()])?;
        Ok(db.last_insert_rowid())
    }
}

pub fn archive_project(
    db: &Connection,
    id: i64,
    archive: bool,
    at: Timestamp,
) -> Result<(), Error> {
    if db.execute(
        "UPDATE projects SET archived_at_utc_ms=?1 WHERE id=?2",
        params![archive.then(|| at.timestamp_millis()), id],
    )? == 0
    {
        return Err(Error::Missing);
    }
    Ok(())
}

fn check_project(db: &Connection, project: Option<i64>, active_only: bool) -> Result<(), Error> {
    if let Some(id) = project {
        match db
            .query_row(
                "SELECT archived_at_utc_ms IS NOT NULL FROM projects WHERE id=?1",
                [id],
                |row| row.get::<_, bool>(0),
            )
            .optional()?
        {
            None => return Err(Error::Missing),
            Some(true) if active_only => return Err(Error::ArchivedProject),
            _ => {}
        }
    }
    Ok(())
}

pub fn last_project(db: &Connection) -> Result<Option<i64>, Error> {
    Ok(db.query_row(
        "SELECT project.id FROM settings JOIN projects AS project ON settings.value = CAST(project.id AS TEXT)
         WHERE settings.key = 'tracking.last_project' AND project.archived_at_utc_ms IS NULL",
        [], |row| row.get(0),
    ).optional()?)
}

pub fn remember_project(db: &Connection, project: Option<i64>) -> Result<(), Error> {
    db.execute(
        "INSERT INTO settings (key,value) VALUES ('tracking.last_project', ?1)
         ON CONFLICT(key) DO UPDATE SET value=excluded.value",
        [project.map_or_else(String::new, |id| id.to_string())],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct Session {
    pub workday_id: i64,
    pub entry_id: i64,
    pub kind: Kind,
    pub project: Option<i64>,
    pub started: Timestamp,
    pub checkpoint: Timestamp,
}

fn timestamp(value: i64) -> Result<Timestamp, Error> {
    DateTime::from_timestamp_millis(value).ok_or(Error::InvalidData)
}

pub fn active(db: &Connection) -> Result<Option<Session>, Error> {
    db.query_row(
        "SELECT d.id, e.id, e.kind, e.project_id, e.started_at_utc_ms,
                max(coalesce(d.checkpoint_utc_ms, e.started_at_utc_ms), e.started_at_utc_ms)
         FROM workdays d JOIN timeline_entries e ON e.workday_id=d.id
         WHERE d.ended_at_utc_ms IS NULL AND e.ended_at_utc_ms IS NULL",
        [],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get::<_, String>(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    )
    .optional()?
    .map(
        |(workday_id, entry_id, kind, project, started, checkpoint)| {
            Ok(Session {
                workday_id,
                entry_id,
                kind: Kind::read(&kind)?,
                project,
                started: timestamp(started)?,
                checkpoint: timestamp(checkpoint)?,
            })
        },
    )
    .transpose()
}

pub fn day_finished(db: &Connection, date: NaiveDate) -> Result<bool, Error> {
    super::worktime_account::queries::today_finished(db, date).map_err(|error| match error {
        super::worktime_account::StoreError::Sqlite(error) => Error::Sqlite(error),
        _ => Error::InvalidData,
    })
}

/// Mark a closed, manually recorded local workday as complete.
pub fn finalize_manual_day(db: &mut Connection, zone: Tz, at: Timestamp) -> Result<(), Error> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if active(&tx)?.is_some() {
        return Err(Error::InvalidTransition);
    }
    let date = at.with_timezone(&zone).date_naive().to_string();
    if tx.execute(
        "UPDATE workdays SET finalized=1 WHERE local_date=?1 AND timezone=?2
         AND ended_at_utc_ms IS NOT NULL AND ended_at_utc_ms <= ?3 AND finalized=0",
        params![date, zone.name(), at.timestamp_millis()],
    )? == 0
    {
        return Err(Error::Missing);
    }
    tx.commit()?;
    Ok(())
}

pub fn start(
    db: &mut Connection,
    zone: Tz,
    project: Option<i64>,
    at: Timestamp,
) -> Result<Session, Error> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if active(&tx)?.is_some()
        || tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM workdays WHERE ended_at_utc_ms IS NULL)",
            [],
            |row| row.get::<_, bool>(0),
        )?
    {
        return Err(Error::InvalidTransition);
    }
    check_project(&tx, project, true)?;
    let date = at.with_timezone(&zone).date_naive().to_string();
    let ms = at.timestamp_millis();
    let previous = tx
        .query_row(
            "SELECT id FROM workdays WHERE local_date=?1 AND timezone=?2 AND ended_at_utc_ms <= ?3
         ORDER BY ended_at_utc_ms DESC, id DESC LIMIT 1",
            params![date, zone.name(), ms],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    let day = if let Some(day) = previous {
        tx.execute("UPDATE workdays SET ended_at_utc_ms=NULL, finalized=0, checkpoint_utc_ms=?1 WHERE id=?2", params![ms,day])?;
        day
    } else {
        tx.execute(
            "INSERT INTO workdays (local_date,timezone,started_at_utc_ms,checkpoint_utc_ms,finalized)
             VALUES (?1,?2,?3,?3,0)", params![date,zone.name(),ms],
        )?;
        tx.last_insert_rowid()
    };
    insert_open(&tx, day, Kind::Work, project, ms)?;
    remember_project(&tx, project)?;
    let session = active(&tx)?.ok_or(Error::InvalidTransition)?;
    tx.commit()?;
    Ok(session)
}

#[derive(Clone, Copy)]
pub enum Command {
    Switch(Option<i64>),
    Pause,
    Resume(Option<i64>),
    Finish,
}

pub fn transition(
    db: &mut Connection,
    expected: i64,
    command: Command,
    at: Timestamp,
) -> Result<Option<i64>, Error> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let session = active(&tx)?
        .filter(|session| session.entry_id == expected)
        .ok_or(Error::InvalidTransition)?;
    let next = match (session.kind, command) {
        (Kind::Work, Command::Switch(project)) => Some((Kind::Work, project)),
        (Kind::Work, Command::Pause) => Some((Kind::Break, None)),
        (Kind::Break, Command::Resume(project)) => Some((Kind::Work, project)),
        (_, Command::Finish) => None,
        _ => return Err(Error::InvalidTransition),
    };
    let pending = close(&tx, &session, at.timestamp_millis(), next)?;
    tx.commit()?;
    Ok(pending)
}

#[derive(Clone, Copy)]
pub enum Recovery {
    Include,
    BreakSinceCheckpoint,
    FinishAtCheckpoint,
}

pub fn recover(
    db: &mut Connection,
    expected: i64,
    recovery: Recovery,
    at: Timestamp,
) -> Result<Option<i64>, Error> {
    let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let session = active(&tx)?
        .filter(|session| session.entry_id == expected)
        .ok_or(Error::InvalidTransition)?;
    if at < session.checkpoint {
        return Err(Error::InvalidRange);
    }
    let pending = match recovery {
        Recovery::Include => {
            tx.execute(
                "UPDATE workdays SET checkpoint_utc_ms=?1 WHERE id=?2",
                params![at.timestamp_millis(), session.workday_id],
            )?;
            None
        }
        Recovery::FinishAtCheckpoint => {
            close(&tx, &session, session.checkpoint.timestamp_millis(), None)?
        }
        Recovery::BreakSinceCheckpoint => {
            // Keep the entire unobserved interval and explicitly classify it as a break.
            let boundary = session.checkpoint.timestamp_millis();
            let pending = close(&tx, &session, boundary, Some((Kind::Break, None)))?;
            let pause = active(&tx)?.ok_or(Error::InvalidTransition)?;
            let project = if session.kind == Kind::Work
                && check_project(&tx, session.project, true).is_ok()
            {
                session.project
            } else {
                last_project(&tx)?
            };
            close(
                &tx,
                &pause,
                at.timestamp_millis(),
                Some((Kind::Work, project)),
            )?;
            pending
        }
    };
    tx.commit()?;
    Ok(pending)
}

pub fn checkpoint(db: &Connection, entry: i64, at: Timestamp) -> Result<(), Error> {
    db.execute(
        "UPDATE workdays SET checkpoint_utc_ms=?1
         WHERE ended_at_utc_ms IS NULL AND coalesce(checkpoint_utc_ms,started_at_utc_ms) <= ?1
         AND id=(SELECT workday_id FROM timeline_entries WHERE id=?2 AND ended_at_utc_ms IS NULL)",
        params![at.timestamp_millis(), entry],
    )?;
    Ok(())
}

fn close(
    db: &Connection,
    session: &Session,
    ms: i64,
    next: Option<(Kind, Option<i64>)>,
) -> Result<Option<i64>, Error> {
    if ms < session.started.timestamp_millis() {
        return Err(Error::InvalidRange);
    }
    if let Some((_, project)) = next {
        check_project(db, project, true)?;
    }
    db.execute(
        "UPDATE timeline_entries SET ended_at_utc_ms=?1, updated_at_utc_ms=?1 WHERE id=?2",
        params![ms, session.entry_id],
    )?;
    if let Some((kind, project)) = next {
        insert_open(db, session.workday_id, kind, project, ms)?;
        db.execute(
            "UPDATE workdays SET checkpoint_utc_ms=?1 WHERE id=?2",
            params![ms, session.workday_id],
        )?;
        if kind == Kind::Work {
            remember_project(db, project)?;
        }
    } else {
        db.execute(
            "UPDATE workdays SET ended_at_utc_ms=?1, checkpoint_utc_ms=?1, finalized=1 WHERE id=?2",
            params![ms, session.workday_id],
        )?;
    }
    let pending = db.query_row(
        "SELECT work_status IS 'pending_description' FROM timeline_entries WHERE id=?1",
        [session.entry_id],
        |row| row.get::<_, bool>(0),
    )?;
    Ok(pending.then_some(session.entry_id))
}

fn insert_open(
    db: &Connection,
    day: i64,
    kind: Kind,
    project: Option<i64>,
    ms: i64,
) -> Result<(), Error> {
    db.execute(
        "INSERT INTO timeline_entries (workday_id,kind,project_id,started_at_utc_ms,created_at_utc_ms,updated_at_utc_ms)
         VALUES (?1,?2,?3,?4,?4,?4)", params![day,kind.sql(),project,ms],
    )?;
    Ok(())
}

pub fn describe(db: &Connection, entry: i64, activity: &str, at: Timestamp) -> Result<(), Error> {
    let activity = activity.trim();
    if activity.is_empty() {
        return Err(Error::ActivityRequired);
    }
    if db.execute(
        "UPDATE timeline_entries SET activity_text=?1, updated_at_utc_ms=?2
        WHERE id=?3 AND kind='work' AND ended_at_utc_ms IS NOT NULL",
        params![activity, at.timestamp_millis(), entry],
    )? == 0
    {
        return Err(Error::Missing);
    }
    Ok(())
}

pub fn pending_descriptions(db: &Connection) -> Result<Vec<(i64, String)>, Error> {
    let mut stmt = db.prepare(
        "SELECT e.id, coalesce(p.name, 'Ohne Projekt') || ' · ' || d.local_date
         FROM timeline_entries e JOIN workdays d ON d.id=e.workday_id
         LEFT JOIN projects p ON p.id=e.project_id
         WHERE e.work_status='pending_description' ORDER BY e.ended_at_utc_ms DESC,e.id DESC",
    )?;
    Ok(stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<Result<_, _>>()?)
}
