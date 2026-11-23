//! Effective-dated worktime accounting, independent of SQLite and the system clock.

mod attribution;

pub use attribution::attribute_work;

use std::{collections::BTreeMap, fmt};

use chrono::{Datelike, Days, Months, NaiveDate, Weekday};

pub type Seconds = i64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountError {
    ScheduleDoesNotAddUp,
    MissingOpening,
    MissingSchedule(NaiveDate),
    InvalidDateRange,
    NegativeActualOrCredit,
    Overflow,
}

impl fmt::Display for AccountError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScheduleDoesNotAddUp => {
                formatter.write_str("weekday targets must sum to the weekly target")
            }
            Self::MissingOpening => {
                formatter.write_str("the worktime account needs an opening balance")
            }
            Self::MissingSchedule(date) => write!(formatter, "no work schedule applies on {date}"),
            Self::InvalidDateRange => formatter.write_str("date outside the supported range"),
            Self::NegativeActualOrCredit => {
                formatter.write_str("actual work and approved credits must be nonnegative")
            }
            Self::Overflow => formatter.write_str("worktime account arithmetic overflow"),
        }
    }
}

impl std::error::Error for AccountError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WeeklySchedule {
    weekly_seconds: u32,
    weekdays: [u32; 7],
}

impl WeeklySchedule {
    pub fn new(weekly_seconds: u32, weekdays: [u32; 7]) -> Result<Self, AccountError> {
        if weekdays
            .iter()
            .map(|&seconds| u64::from(seconds))
            .sum::<u64>()
            != u64::from(weekly_seconds)
        {
            return Err(AccountError::ScheduleDoesNotAddUp);
        }
        Ok(Self {
            weekly_seconds,
            weekdays,
        })
    }

    pub fn weekly_seconds(self) -> u32 {
        self.weekly_seconds
    }

    /// Daily allocation in Monday–Sunday order.
    pub fn weekdays(self) -> [u32; 7] {
        self.weekdays
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OpeningBalance {
    pub effective_date: NaiveDate,
    pub signed_seconds: Seconds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Adjustment {
    pub effective_date: NaiveDate,
    pub signed_seconds: Seconds,
}

#[derive(Debug, Default, Clone)]
pub struct Account {
    pub opening: Option<OpeningBalance>,
    pub schedules: BTreeMap<NaiveDate, WeeklySchedule>,
    pub overrides: BTreeMap<NaiveDate, u32>,
    /// Only active adjustments belong in this projection.
    pub adjustments: Vec<Adjustment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Period {
    start: NaiveDate,
    end: NaiveDate,
}

impl Period {
    pub fn iso_week(date: NaiveDate) -> Result<Self, AccountError> {
        let start = date
            .week(Weekday::Mon)
            .checked_first_day()
            .ok_or(AccountError::InvalidDateRange)?;
        let end = start
            .checked_add_days(Days::new(6))
            .ok_or(AccountError::InvalidDateRange)?;
        Ok(Self { start, end })
    }

    pub fn month(date: NaiveDate) -> Result<Self, AccountError> {
        let start = date.with_day(1).ok_or(AccountError::InvalidDateRange)?;
        let end = start
            .checked_add_months(Months::new(1))
            .and_then(|date| date.pred_opt())
            .ok_or(AccountError::InvalidDateRange)?;
        Ok(Self { start, end })
    }

    pub fn start(self) -> NaiveDate {
        self.start
    }

    pub fn end(self) -> NaiveDate {
        self.end
    }
}

#[derive(Debug, Default)]
pub struct Observations {
    /// Net work from the work timeline, including work with no project.
    pub actual: BTreeMap<NaiveDate, Seconds>,
    /// Explicit approved credits, supplied only by a feature that can approve them.
    pub approved_credits: BTreeMap<NaiveDate, Seconds>,
    /// Earliest local date with unclassified open time omitted from this projection.
    pub uncertain_since: Option<NaiveDate>,
}

#[derive(Debug, Default, Clone, Copy, Eq, PartialEq)]
pub struct Totals {
    pub actual: Seconds,
    pub target: Seconds,
    pub approved_credits: Seconds,
    pub delta: Seconds,
    pub adjustments: Seconds,
}

impl Totals {
    fn checked_add(self, other: Self) -> Result<Self, AccountError> {
        Ok(Self {
            actual: add(self.actual, other.actual)?,
            target: add(self.target, other.target)?,
            approved_credits: add(self.approved_credits, other.approved_credits)?,
            delta: add(self.delta, other.delta)?,
            adjustments: add(self.adjustments, other.adjustments)?,
        })
    }

    fn apply(self, opening: Seconds) -> Result<Seconds, AccountError> {
        add(add(opening, self.delta)?, self.adjustments)
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DayReport {
    pub date: NaiveDate,
    /// Absent on dates before the opening balance takes effect.
    pub planned_target: Option<u32>,
    /// Absent on future dates and dates before the account begins.
    pub observed: Option<Totals>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PeriodReport {
    pub period: Period,
    pub through: NaiveDate,
    pub days: Vec<DayReport>,
    pub totals: Totals,
    pub full_planned_target: Seconds,
    pub remaining_planned_target: Seconds,
    pub opening_balance: Option<Seconds>,
    pub closing_balance: Option<Seconds>,
    pub provisional: bool,
    pub excludes_open_time: bool,
}

impl Account {
    pub fn target_on(&self, date: NaiveDate) -> Option<u32> {
        self.overrides.get(&date).copied().or_else(|| {
            self.schedules
                .range(..=date)
                .next_back()
                .map(|(_, schedule)| {
                    schedule.weekdays[date.weekday().num_days_from_monday() as usize]
                })
        })
    }

    pub fn balance_as_of(
        &self,
        through: NaiveDate,
        observations: &Observations,
    ) -> Result<Option<Seconds>, AccountError> {
        let opening = self.opening.ok_or(AccountError::MissingOpening)?;
        if through < opening.effective_date {
            return Ok(None);
        }
        let adjustments = self.adjustments_by_date(opening.effective_date, through)?;
        let totals = dates(opening.effective_date, through).try_fold(
            Totals::default(),
            |totals, date| {
                totals.checked_add(self.daily_totals(date, observations, &adjustments)?)
            },
        )?;
        Ok(Some(totals.apply(opening.signed_seconds)?))
    }

    /// The cutoff is inclusive. Dates after it contribute only to the planned target.
    pub fn report(
        &self,
        period: Period,
        through: NaiveDate,
        today: NaiveDate,
        observations: &Observations,
    ) -> Result<PeriodReport, AccountError> {
        let opening = self.opening.ok_or(AccountError::MissingOpening)?;
        let through = through.min(period.end).min(today);
        let first_accounted = period.start.max(opening.effective_date);
        let opening_balance = if first_accounted > period.end {
            None
        } else if first_accounted == opening.effective_date {
            Some(opening.signed_seconds)
        } else {
            self.balance_as_of(
                period
                    .start
                    .pred_opt()
                    .ok_or(AccountError::InvalidDateRange)?,
                observations,
            )?
        };
        let adjustments = self.adjustments_by_date(first_accounted, through)?;
        let mut days = Vec::new();
        let mut totals = Totals::default();
        let mut full_planned_target = 0;
        for date in dates(period.start, period.end) {
            let planned_target = if date >= first_accounted {
                let target = self
                    .target_on(date)
                    .ok_or(AccountError::MissingSchedule(date))?;
                full_planned_target = add(full_planned_target, Seconds::from(target))?;
                Some(target)
            } else {
                None
            };
            let observed = if planned_target.is_some() && date <= through {
                let daily = self.daily_totals(date, observations, &adjustments)?;
                totals = totals.checked_add(daily)?;
                Some(daily)
            } else {
                None
            };
            days.push(DayReport {
                date,
                planned_target,
                observed,
            });
        }
        let closing_balance = match opening_balance {
            Some(opening) if first_accounted <= through => Some(totals.apply(opening)?),
            _ => None,
        };
        let excludes_open_time = closing_balance.is_some()
            && observations
                .uncertain_since
                .is_some_and(|date| date <= through);
        Ok(PeriodReport {
            period,
            through,
            days,
            totals,
            full_planned_target,
            remaining_planned_target: full_planned_target - totals.target,
            opening_balance,
            closing_balance,
            provisional: through < period.end || through == today || excludes_open_time,
            excludes_open_time,
        })
    }

    fn daily_totals(
        &self,
        date: NaiveDate,
        observations: &Observations,
        adjustments: &BTreeMap<NaiveDate, Seconds>,
    ) -> Result<Totals, AccountError> {
        let actual = observations.actual.get(&date).copied().unwrap_or_default();
        let approved_credits = observations
            .approved_credits
            .get(&date)
            .copied()
            .unwrap_or_default();
        if actual < 0 || approved_credits < 0 {
            return Err(AccountError::NegativeActualOrCredit);
        }
        let target = Seconds::from(
            self.target_on(date)
                .ok_or(AccountError::MissingSchedule(date))?,
        );
        Ok(Totals {
            actual,
            target,
            approved_credits,
            delta: add(actual, approved_credits)? - target,
            adjustments: adjustments.get(&date).copied().unwrap_or_default(),
        })
    }

    fn adjustments_by_date(
        &self,
        start: NaiveDate,
        end: NaiveDate,
    ) -> Result<BTreeMap<NaiveDate, Seconds>, AccountError> {
        let mut totals = BTreeMap::new();
        for adjustment in &self.adjustments {
            if (start..=end).contains(&adjustment.effective_date) {
                let total = totals.entry(adjustment.effective_date).or_default();
                *total = add(*total, adjustment.signed_seconds)?;
            }
        }
        Ok(totals)
    }
}

fn dates(start: NaiveDate, end: NaiveDate) -> impl Iterator<Item = NaiveDate> {
    start.iter_days().take_while(move |&date| date <= end)
}

pub(super) fn add(left: Seconds, right: Seconds) -> Result<Seconds, AccountError> {
    left.checked_add(right).ok_or(AccountError::Overflow)
}
