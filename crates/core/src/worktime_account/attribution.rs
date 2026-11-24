use std::collections::BTreeMap;

use chrono::{DateTime, LocalResult, NaiveDate, TimeZone, Utc};
use chrono_tz::{GapInfo, Tz};

use crate::{TimeSpan, TimelineEntry, Timestamp};

use super::{AccountError, Seconds, add};

/// Attribute work to local dates in each workday's recorded timezone. The caller
/// chooses a trusted cutoff and omits any open intervals awaiting classification.
/// Milliseconds from all pieces of one date are summed before rounding down to seconds.
pub fn attribute_work<'a>(
    entries: impl IntoIterator<Item = (Tz, &'a TimelineEntry)>,
    cutoff: Timestamp,
) -> Result<BTreeMap<NaiveDate, Seconds>, AccountError> {
    let mut millis_by_date = BTreeMap::new();
    for (zone, entry) in entries {
        if let TimelineEntry::Work(work) = entry {
            visit_local_intervals(work.span(), zone, cutoff, |date, start, end| {
                let total = millis_by_date.entry(date).or_default();
                *total = add(*total, (end - start).num_milliseconds())?;
                Ok(())
            })?;
        }
    }
    Ok(millis_by_date
        .into_iter()
        .map(|(date, millis)| (date, millis / 1000))
        .collect())
}

/// Visit the pieces of an interval in UTC order, splitting at local date changes.
pub fn visit_local_intervals(
    span: TimeSpan,
    zone: Tz,
    cutoff: Timestamp,
    mut visit: impl FnMut(NaiveDate, Timestamp, Timestamp) -> Result<(), AccountError>,
) -> Result<(), AccountError> {
    let mut cursor = span.start();
    let end = span.end().map_or(cutoff, |end| end.min(cutoff));
    while cursor < end {
        let date = cursor.with_timezone(&zone).date_naive();
        let midnight = date.succ_opt().ok_or(AccountError::InvalidDateRange)?;
        let next = boundary_after(zone, date, midnight, cursor)?.min(end);
        visit(date, cursor, next)?;
        cursor = next;
    }
    Ok(())
}

fn boundary_after(
    zone: Tz,
    date: NaiveDate,
    next_date: NaiveDate,
    cursor: Timestamp,
) -> Result<Timestamp, AccountError> {
    let midnight = next_date
        .and_hms_opt(0, 0, 0)
        .ok_or(AccountError::InvalidDateRange)?;
    let next = match zone.from_local_datetime(&midnight) {
        LocalResult::Single(time) => time.to_utc(),
        LocalResult::Ambiguous(first, second) => {
            let first = first.to_utc();
            if first > cursor {
                first
            } else {
                second.to_utc()
            }
        }
        LocalResult::None => GapInfo::new(&midnight, &zone)
            .and_then(|gap| gap.end)
            .map(|end| end.to_utc())
            .ok_or(AccountError::InvalidDateRange)?,
    };

    // A fall-back just after midnight can return to the previous calendar date.
    if let LocalResult::Ambiguous(first, second) = zone.from_local_datetime(
        &date
            .and_hms_opt(0, 0, 0)
            .ok_or(AccountError::InvalidDateRange)?,
    ) {
        let first = first.to_utc();
        let second = second.to_utc();
        let before_second = second.timestamp() - 1;
        if cursor >= first
            && cursor < second
            && timestamp(before_second)?.with_timezone(&zone).date_naive() < date
        {
            let mut low = cursor.timestamp();
            let mut high = before_second;
            while high - low > 1 {
                let middle = low + (high - low) / 2;
                if timestamp(middle)?.with_timezone(&zone).date_naive() == date {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            return timestamp(high);
        }
    }
    if next > cursor {
        Ok(next)
    } else {
        Err(AccountError::InvalidDateRange)
    }
}

fn timestamp(seconds: i64) -> Result<Timestamp, AccountError> {
    DateTime::<Utc>::from_timestamp(seconds, 0).ok_or(AccountError::InvalidDateRange)
}
