//! Time-tracking values and calculations without an implicit clock or database.

use std::{collections::BTreeMap, fmt};
pub mod worktime_account;

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;

pub type Timestamp = DateTime<Utc>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ProjectId(pub i64);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainError {
    EndBeforeStart,
    EntryOutsideWorkday,
    OverlappingEntries,
    FinishOutOfRange,
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::EndBeforeStart => "end precedes start",
            Self::EntryOutsideWorkday => "timeline entry is outside its workday",
            Self::OverlappingEntries => "timeline entries overlap",
            Self::FinishOutOfRange => "estimated finish is outside the supported timestamp range",
        })
    }
}

impl std::error::Error for DomainError {}

/// A half-open UTC interval. An open interval has no upper bound for validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TimeSpan {
    start: Timestamp,
    end: Option<Timestamp>,
}

impl TimeSpan {
    pub fn new(start: Timestamp, end: Option<Timestamp>) -> Result<Self, DomainError> {
        if end.is_some_and(|end| end < start) {
            return Err(DomainError::EndBeforeStart);
        }
        Ok(Self { start, end })
    }

    pub fn start(self) -> Timestamp {
        self.start
    }

    pub fn end(self) -> Option<Timestamp> {
        self.end
    }

    pub fn elapsed_at(self, as_of: Timestamp) -> TimeDelta {
        let end = self.end.map_or(as_of, |end| end.min(as_of));
        (end - self.start).max(TimeDelta::zero())
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.end != Some(self.start)
            && other.end != Some(other.start)
            && self.end.is_none_or(|end| other.start < end)
            && other.end.is_none_or(|end| self.start < end)
    }

    pub fn contains(self, other: Self) -> bool {
        other.start >= self.start
            && self
                .end
                .is_none_or(|end| other.end.is_some_and(|other_end| other_end <= end))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkStatus {
    Active,
    PendingDescription,
    Complete,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkSegment {
    span: TimeSpan,
    project: Option<ProjectId>,
    activity: Option<String>,
}

impl WorkSegment {
    pub fn new(span: TimeSpan, project: Option<ProjectId>, activity: Option<&str>) -> Self {
        Self {
            span,
            project,
            activity: activity
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_owned),
        }
    }

    pub fn span(&self) -> TimeSpan {
        self.span
    }

    pub fn project(&self) -> Option<ProjectId> {
        self.project
    }

    pub fn activity(&self) -> Option<&str> {
        self.activity.as_deref()
    }

    pub fn status(&self) -> WorkStatus {
        match (self.span.end(), self.activity()) {
            (None, _) => WorkStatus::Active,
            (Some(_), None) => WorkStatus::PendingDescription,
            (Some(_), Some(_)) => WorkStatus::Complete,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TimelineEntry {
    Work(WorkSegment),
    Break(TimeSpan),
}

impl TimelineEntry {
    pub fn span(&self) -> TimeSpan {
        match self {
            Self::Work(segment) => segment.span(),
            Self::Break(span) => *span,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Workday {
    pub span: TimeSpan,
    pub timezone: Tz,
    pub target_seconds_snapshot: Option<u32>,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct WorkdaySummary {
    pub wall_elapsed: TimeDelta,
    pub net_worked: TimeDelta,
    pub break_time: TimeDelta,
    pub unassigned_work: TimeDelta,
    pub project_time: BTreeMap<ProjectId, TimeDelta>,
    pub remaining: Option<TimeDelta>,
    pub estimated_finish: Option<Timestamp>,
}

impl Workday {
    /// The start date identifies the workday even when its entries cross midnight.
    pub fn local_date(self) -> NaiveDate {
        self.span.start().with_timezone(&self.timezone).date_naive()
    }

    /// Use one consistent cutoff for all intervals. Recovery must choose an
    /// approved cutoff before it asks this calculation to include open time.
    pub fn summarize(
        self,
        entries: &[TimelineEntry],
        as_of: Timestamp,
        additional_break_seconds: u32,
    ) -> Result<WorkdaySummary, DomainError> {
        let mut ordered: Vec<_> = entries.iter().collect();
        ordered.sort_unstable_by_key(|entry| entry.span().start());
        let mut previous = None::<TimeSpan>;
        let mut summary = WorkdaySummary {
            wall_elapsed: self.span.elapsed_at(as_of),
            ..WorkdaySummary::default()
        };

        for entry in ordered {
            let span = entry.span();
            if !self.span.contains(span) {
                return Err(DomainError::EntryOutsideWorkday);
            }
            if span.end() != Some(span.start()) {
                if previous.is_some_and(|previous| previous.overlaps(span)) {
                    return Err(DomainError::OverlappingEntries);
                }
                previous = Some(span);
            }

            let elapsed = span.elapsed_at(as_of);
            match entry {
                TimelineEntry::Work(segment) => {
                    summary.net_worked += elapsed;
                    match segment.project() {
                        Some(project) => {
                            *summary.project_time.entry(project).or_default() += elapsed
                        }
                        None => summary.unassigned_work += elapsed,
                    }
                }
                TimelineEntry::Break(_) => summary.break_time += elapsed,
            }
        }

        if let Some(target) = self.target_seconds_snapshot {
            let remaining =
                (TimeDelta::seconds(i64::from(target)) - summary.net_worked).max(TimeDelta::zero());
            summary.remaining = Some(remaining);
            if self.span.end().is_none() {
                let delay = remaining + TimeDelta::seconds(i64::from(additional_break_seconds));
                summary.estimated_finish = Some(
                    as_of
                        .checked_add_signed(delay)
                        .ok_or(DomainError::FinishOutOfRange)?,
                );
            }
        }
        Ok(summary)
    }
}
