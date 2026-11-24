//! Transactional account configuration, audit history, and report snapshots.

mod changes;
pub(crate) mod queries;

use std::{error::Error, fmt};

use chrono::NaiveDate;
use projektzeit_core::{
    Timestamp,
    worktime_account::{
        AccountError, Adjustment, OpeningBalance, Period, PeriodReport, Seconds, WeeklySchedule,
    },
};
use rusqlite::{Connection, TransactionBehavior};

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    Calculation(AccountError),
    InvalidStoredValue(&'static str),
    ReasonRequired,
    AdjustmentUnavailable(i64),
    OverrideUnavailable(NaiveDate),
    StalePreview,
    InvalidReportDates,
}

impl fmt::Display for StoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "{error}"),
            Self::Calculation(error) => write!(formatter, "{error}"),
            Self::InvalidStoredValue(column) => {
                write!(formatter, "invalid stored value in {column}")
            }
            Self::ReasonRequired => formatter.write_str("a reason is required"),
            Self::AdjustmentUnavailable(id) => {
                write!(formatter, "adjustment {id} is missing or voided")
            }
            Self::OverrideUnavailable(date) => {
                write!(formatter, "no target override exists for {date}")
            }
            Self::StalePreview => formatter.write_str("the account changed since the preview"),
            Self::InvalidReportDates => formatter.write_str("as-of date is later than today"),
        }
    }
}

impl Error for StoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            Self::Calculation(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<AccountError> for StoreError {
    fn from(error: AccountError) -> Self {
        Self::Calculation(error)
    }
}

#[derive(Clone, Debug)]
pub enum Change {
    Schedule {
        effective_date: NaiveDate,
        schedule: WeeklySchedule,
    },
    Opening(OpeningBalance),
    SetTarget {
        date: NaiveDate,
        seconds: u32,
        reason: String,
    },
    ClearTarget {
        date: NaiveDate,
        reason: String,
    },
    AddAdjustment {
        adjustment: Adjustment,
        reason: String,
    },
    EditAdjustment {
        id: i64,
        adjustment: Adjustment,
        reason: String,
    },
    VoidAdjustment {
        id: i64,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BalanceState {
    Unconfigured,
    BeforeOpening,
    MissingSchedule(NaiveDate),
    Calculated(Seconds),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Impact {
    pub balance: BalanceState,
    pub target_at_effective_date: Option<u32>,
    pub provisional: bool,
}

#[derive(Debug)]
pub struct Preview {
    change: Change,
    revision: i64,
    as_of: NaiveDate,
    observed_at: Timestamp,
    effective_date: NaiveDate,
    before: Impact,
    after: Impact,
}

impl Preview {
    pub fn as_of(&self) -> NaiveDate {
        self.as_of
    }
    pub fn effective_date(&self) -> NaiveDate {
        self.effective_date
    }
    pub fn before(&self) -> Impact {
        self.before
    }
    pub fn after(&self) -> Impact {
        self.after
    }
}

#[derive(Debug)]
pub struct TargetOverride {
    pub date: NaiveDate,
    pub seconds: u32,
    pub reason: String,
}

#[derive(Debug)]
pub struct AdjustmentRecord {
    pub id: i64,
    pub adjustment: Adjustment,
    pub reason: String,
    pub voided: bool,
}

#[derive(Debug)]
pub struct Settings {
    pub opening: Option<OpeningBalance>,
    pub schedules: Vec<(NaiveDate, WeeklySchedule)>,
    pub overrides: Vec<TargetOverride>,
    pub adjustments: Vec<AdjustmentRecord>,
}

#[derive(Debug)]
pub struct AuditEvent {
    pub id: i64,
    pub entity: String,
    pub key: String,
    pub action: String,
    pub reason: Option<String>,
    pub before_json: Option<String>,
    pub after_json: Option<String>,
    pub occurred_at_utc_ms: i64,
}

#[derive(Debug)]
pub struct Reports {
    pub period: PeriodReport,
    pub balance_as_of: Option<Seconds>,
    pub balance_provisional: bool,
    pub balance_excludes_open_time: bool,
}

pub fn settings(db: &mut Connection) -> Result<Settings, StoreError> {
    let transaction = db.transaction()?;
    let settings = queries::settings(&transaction)?;
    transaction.commit()?;
    Ok(settings)
}

/// Return the schedule in force on this local calendar date.
pub fn schedule_at(db: &Connection, date: NaiveDate) -> Result<Option<WeeklySchedule>, StoreError> {
    use rusqlite::OptionalExtension;

    db.query_row(
        "SELECT weekly_seconds, monday, tuesday, wednesday, thursday, friday, saturday, sunday
         FROM work_schedules
         WHERE effective_date <= ?1 AND superseded_at_utc_ms IS NULL
         ORDER BY effective_date DESC LIMIT 1",
        [date.to_string()],
        |row| {
            Ok((
                row.get(0)?,
                [
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                ],
            ))
        },
    )
    .optional()?
    .map(|(weekly, weekdays)| WeeklySchedule::new(weekly, weekdays).map_err(StoreError::from))
    .transpose()
}

/// Save a plan effective today without replacing earlier or future dates.
pub fn save_current_schedule(
    db: &mut Connection,
    today: NaiveDate,
    schedule: WeeklySchedule,
    occurred_at: Timestamp,
) -> Result<(), StoreError> {
    let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    if schedule_at(&transaction, today)? != Some(schedule) {
        changes::write(
            &transaction,
            Change::Schedule {
                effective_date: today,
                schedule,
            },
            occurred_at.timestamp_millis(),
        )?;
    }
    transaction.commit()?;
    Ok(())
}

pub fn audit_history(db: &Connection) -> Result<Vec<AuditEvent>, StoreError> {
    let mut statement = db.prepare(
        "SELECT id, entity_type, entity_key, action, reason, old_value_json, new_value_json, occurred_at_utc_ms
         FROM account_audit_events ORDER BY id DESC"
    )?;
    let events = statement
        .query_map([], |row| {
            Ok(AuditEvent {
                id: row.get(0)?,
                entity: row.get(1)?,
                key: row.get(2)?,
                action: row.get(3)?,
                reason: row.get(4)?,
                before_json: row.get(5)?,
                after_json: row.get(6)?,
                occurred_at_utc_ms: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

pub fn report(
    db: &mut Connection,
    period: Period,
    as_of: NaiveDate,
    today: NaiveDate,
    observed_at: Timestamp,
) -> Result<Reports, StoreError> {
    report_live(db, period, as_of, today, observed_at, None)
}

pub fn report_live(
    db: &mut Connection,
    period: Period,
    as_of: NaiveDate,
    today: NaiveDate,
    observed_at: Timestamp,
    trusted_entry: Option<i64>,
) -> Result<Reports, StoreError> {
    if as_of > today {
        return Err(StoreError::InvalidReportDates);
    }
    let transaction = db.transaction()?;
    let account = queries::settings(&transaction)?.account();
    let mut observations = queries::observations(&transaction, observed_at, trusted_entry)?;
    observations.today_finished = queries::today_finished(&transaction, today)?;
    let period = account.report(period, as_of, today, &observations)?;
    let balance_as_of = if period.through == as_of && period.closing_balance.is_some() {
        period.closing_balance
    } else {
        account.finalized_balance_as_of(as_of, today, &observations)?
    };
    let balance_excludes_open_time = balance_as_of.is_some()
        && observations
            .uncertain_since
            .is_some_and(|date| date <= as_of);
    let balance_provisional = balance_as_of.is_some()
        && ((as_of == today && !observations.today_finished) || balance_excludes_open_time);
    transaction.commit()?;
    Ok(Reports {
        period,
        balance_as_of,
        balance_provisional,
        balance_excludes_open_time,
    })
}

pub fn preview(
    db: &mut Connection,
    change: Change,
    as_of: NaiveDate,
    observed_at: Timestamp,
) -> Result<Preview, StoreError> {
    let transaction = db.transaction()?;
    let preview = changes::preview(&transaction, change, as_of, observed_at)?;
    transaction.commit()?;
    Ok(preview)
}

/// Recheck the preview inside the write transaction. Call only after confirmation.
pub fn apply(
    db: &mut Connection,
    presented: Preview,
    occurred_at: Timestamp,
) -> Result<(), StoreError> {
    let transaction = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let revision = changes::revision(&transaction)?;
    if revision != presented.revision {
        return Err(StoreError::StalePreview);
    }
    let fresh = changes::preview(
        &transaction,
        presented.change,
        presented.as_of,
        presented.observed_at,
    )?;
    if fresh.before != presented.before || fresh.after != presented.after {
        return Err(StoreError::StalePreview);
    }
    changes::write(&transaction, fresh.change, occurred_at.timestamp_millis())?;
    transaction.commit()?;
    Ok(())
}

pub fn daily_target(db: &Connection, date: NaiveDate) -> Result<Option<u32>, StoreError> {
    use chrono::Datelike;
    use rusqlite::OptionalExtension;
    let override_seconds = db
        .query_row(
            "SELECT target_seconds FROM daily_target_overrides WHERE effective_date=?1",
            [date.to_string()],
            |row| row.get::<_, u32>(0),
        )
        .optional()?;
    match override_seconds {
        Some(seconds) => Ok(Some(seconds)),
        None => Ok(schedule_at(db, date)?
            .map(|schedule| schedule.weekdays()[date.weekday().num_days_from_monday() as usize])),
    }
}
