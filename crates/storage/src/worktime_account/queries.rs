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
) -> Result<Observations, StoreError> {
    let mut statement = db.prepare(
        "SELECT day.timezone, entry.started_at_utc_ms, entry.ended_at_utc_ms
         FROM timeline_entries AS entry JOIN workdays AS day ON day.id = entry.workday_id
         WHERE entry.kind = 'work' AND entry.started_at_utc_ms <= ?1
         ORDER BY entry.started_at_utc_ms",
    )?;
    let rows = statement.query_map([observed_at.timestamp_millis()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
        ))
    })?;
    let mut entries = Vec::new();
    let mut uncertain_since = None;
    for row in rows {
        let (zone, start, end) = row?;
        let zone: Tz = zone
            .parse()
            .map_err(|_| StoreError::InvalidStoredValue("timezone"))?;
        let start = parse_timestamp(start)?;
        if let Some(end) = end {
            let span = TimeSpan::new(start, Some(parse_timestamp(end)?))
                .map_err(|_| StoreError::InvalidStoredValue("ended_at_utc_ms"))?;
            entries.push((
                zone,
                TimelineEntry::Work(WorkSegment::new(span, None, None)),
            ));
        } else {
            // Until recovery can approve an observation cutoff, open elapsed time is uncertain.
            let date = start.with_timezone(&zone).date_naive();
            uncertain_since =
                Some(uncertain_since.map_or(date, |previous: NaiveDate| previous.min(date)));
        }
    }
    let actual = attribute_work(
        entries.iter().map(|(zone, entry)| (*zone, entry)),
        observed_at,
    )?;
    Ok(Observations {
        actual,
        uncertain_since,
        ..Observations::default()
    })
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
