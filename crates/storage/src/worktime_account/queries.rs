use chrono::{DateTime, NaiveDate};
use chrono_tz::Tz;
use projektzeit_core::{
    TimeSpan, TimelineEntry, Timestamp, WorkSegment,
    worktime_account::{
        Account, Adjustment, Observations, OpeningBalance, WeeklySchedule, attribute_work,
    },
};
use rusqlite::{Connection, OptionalExtension};

use super::{AdjustmentRecord, Settings, StoreError, TargetOverride};

impl Settings {
    pub(super) fn account(&self) -> Account {
        Account {
            opening: self.opening,
            schedules: self.schedules.iter().copied().collect(),
            overrides: self
                .overrides
                .iter()
                .map(|item| (item.date, item.seconds))
                .collect(),
            adjustments: self
                .adjustments
                .iter()
                .filter(|item| !item.voided)
                .map(|item| item.adjustment)
                .collect(),
        }
    }
}

pub(super) fn settings(db: &Connection) -> Result<Settings, StoreError> {
    let opening = db
        .query_row(
            "SELECT effective_date, signed_seconds FROM opening_balance_versions
         WHERE superseded_at_utc_ms IS NULL",
            [],
            |row| Ok((row.get::<_, String>(0)?, row.get(1)?)),
        )
        .optional()?
        .map(|(date, signed_seconds)| -> Result<_, StoreError> {
            Ok(OpeningBalance {
                effective_date: parse_date(&date)?,
                signed_seconds,
            })
        })
        .transpose()?;

    let mut schedules = db.prepare(
        "SELECT effective_date, weekly_seconds, monday, tuesday, wednesday, thursday,
                friday, saturday, sunday
         FROM work_schedules WHERE superseded_at_utc_ms IS NULL ORDER BY effective_date",
    )?;
    let schedules = schedules
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get(1)?,
                [
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ],
            ))
        })?
        .map(|row| -> Result<_, StoreError> {
            let (date, total, weekdays) = row?;
            Ok((parse_date(&date)?, WeeklySchedule::new(total, weekdays)?))
        })
        .collect::<Result<_, _>>()?;

    let mut overrides = db.prepare(
        "SELECT effective_date, target_seconds, reason FROM daily_target_overrides ORDER BY effective_date"
    )?;
    let overrides = overrides
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get(1)?, row.get(2)?))
        })?
        .map(|row| -> Result<_, StoreError> {
            let (date, seconds, reason) = row?;
            Ok(TargetOverride {
                date: parse_date(&date)?,
                seconds,
                reason,
            })
        })
        .collect::<Result<_, _>>()?;

    let mut adjustments = db.prepare(
        "SELECT id, effective_date, signed_seconds, reason, voided_at_utc_ms IS NOT NULL
         FROM balance_adjustments ORDER BY effective_date, id",
    )?;
    let adjustments = adjustments
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, String>(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .map(|row| -> Result<_, StoreError> {
            let (id, date, signed_seconds, reason, voided) = row?;
            Ok(AdjustmentRecord {
                id,
                adjustment: Adjustment {
                    effective_date: parse_date(&date)?,
                    signed_seconds,
                },
                reason,
                voided,
            })
        })
        .collect::<Result<_, _>>()?;

    Ok(Settings {
        opening,
        schedules,
        overrides,
        adjustments,
    })
}

pub(super) fn observations(
    db: &Connection,
    observed_at: Timestamp,
    trusted_entry: Option<i64>,
) -> Result<Observations, StoreError> {
    let active = crate::tracker::active(db).map_err(|error| match error {
        crate::tracker::Error::Sqlite(error) => StoreError::Sqlite(error),
        _ => StoreError::InvalidStoredValue("active workday"),
    })?;
    let open_day_since = db
        .query_row(
            "SELECT local_date FROM workdays WHERE ended_at_utc_ms IS NULL",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .map(|date| parse_date(&date))
        .transpose()?;
    let mut statement = db.prepare(
        "SELECT entry.id, day.timezone, entry.started_at_utc_ms, entry.ended_at_utc_ms
         FROM timeline_entries AS entry JOIN workdays AS day ON day.id = entry.workday_id
         WHERE entry.kind = 'work' ORDER BY entry.started_at_utc_ms",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, Option<i64>>(3)?,
        ))
    })?;
    let mut entries = Vec::new();
    let mut cutoff = observed_at;
    for row in rows {
        let (id, zone, start, end) = row?;
        let zone: Tz = zone
            .parse()
            .map_err(|_| StoreError::InvalidStoredValue("timezone"))?;
        let start = parse_timestamp(start)?;
        let end = match end {
            Some(end) => parse_timestamp(end)?,
            None => {
                let Some(session) = active.as_ref().filter(|session| session.entry_id == id) else {
                    continue;
                };
                if trusted_entry == Some(id) {
                    observed_at.max(session.checkpoint)
                } else {
                    session.checkpoint
                }
            }
        };
        // A clock rollback cannot shorten an already committed booking or checkpoint.
        cutoff = cutoff.max(end);
        let span = TimeSpan::new(start, Some(end))
            .map_err(|_| StoreError::InvalidStoredValue("ended_at_utc_ms"))?;
        entries.push((
            zone,
            TimelineEntry::Work(WorkSegment::new(span, None, None)),
        ));
    }
    let actual = attribute_work(entries.iter().map(|(zone, entry)| (*zone, entry)), cutoff)?;
    let uncertain_since = open_day_since.filter(|_| {
        active.as_ref().map(|active| active.entry_id) != trusted_entry || trusted_entry.is_none()
    });
    Ok(Observations {
        actual,
        uncertain_since,
        open_day_since,
        ..Observations::default()
    })
}

pub(crate) fn today_finished(db: &Connection, today: NaiveDate) -> Result<bool, StoreError> {
    let mut stmt = db.prepare(
        "SELECT timezone, started_at_utc_ms, ended_at_utc_ms FROM workdays
         WHERE finalized = 1 AND ended_at_utc_ms IS NOT NULL
           AND ended_at_utc_ms >= ?1 AND ended_at_utc_ms < ?2",
    )?;
    let midnight = today
        .and_hms_opt(0, 0, 0)
        .ok_or(StoreError::InvalidStoredValue("report date"))?
        .and_utc();
    let lower = (midnight - chrono::TimeDelta::days(1)).timestamp_millis();
    let upper = (midnight + chrono::TimeDelta::days(2)).timestamp_millis();
    for result in stmt.query_map([lower, upper], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
        ))
    })? {
        let (zone, start, end) = result?;
        let zone: Tz = zone
            .parse()
            .map_err(|_| StoreError::InvalidStoredValue("timezone"))?;
        if crate::tracker::finished_date(zone, start, end)
            .map_err(|_| StoreError::InvalidStoredValue("workday end"))?
            == today
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn parse_date(value: &str) -> Result<NaiveDate, StoreError> {
    value
        .parse()
        .map_err(|_| StoreError::InvalidStoredValue("effective_date"))
}

fn parse_timestamp(millis: i64) -> Result<Timestamp, StoreError> {
    DateTime::from_timestamp_millis(millis)
        .ok_or(StoreError::InvalidStoredValue("timeline timestamp"))
}
