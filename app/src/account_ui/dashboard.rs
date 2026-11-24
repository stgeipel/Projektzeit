use chrono::{NaiveDate, TimeDelta};
use projektzeit_core::{
    TimeSpan, Timestamp,
    worktime_account::{Period, visit_local_intervals},
};
use projektzeit_storage::tracker::{self, Entry, Kind};
use rusqlite::Connection;

use super::{account_error, dash, duration, tracking::tracking_error, view::SegmentRow};

#[derive(Default)]
pub(super) struct Timeline {
    pub rows: Vec<SegmentRow>,
    pub work_seconds: i64,
    pub break_seconds: i64,
}

pub(super) struct Slice {
    pub entry: Entry,
    pub date: NaiveDate,
    pub start: Timestamp,
    pub end: Timestamp,
    pub counted: bool,
    pub uncertain: bool,
}

pub(super) fn slices(
    db: &Connection,
    period: Period,
    observed_at: Timestamp,
    trusted: Option<i64>,
) -> Result<Vec<Slice>, String> {
    let midnight = |date: NaiveDate| {
        date.and_hms_opt(0, 0, 0)
            .expect("midnight is representable")
            .and_utc()
    };
    let lower = midnight(period.start())
        .checked_sub_signed(TimeDelta::days(1))
        .ok_or_else(|| {
            account_error(projektzeit_core::worktime_account::AccountError::InvalidDateRange)
        })?;
    // All stored offsets are within ±24h of UTC. Keep committed endpoints even
    // when the system's clock has moved backwards.
    let upper = midnight(period.end())
        .checked_add_signed(TimeDelta::days(2))
        .ok_or_else(|| {
            account_error(projektzeit_core::worktime_account::AccountError::InvalidDateRange)
        })?;
    let session = tracker::active(db).map_err(tracking_error)?;
    let mut result = Vec::new();
    for entry in tracker::entries(db, lower, upper).map_err(tracking_error)? {
        let open = session
            .as_ref()
            .filter(|session| session.entry_id == entry.id);
        let uncertain = entry.end.is_none() && trusted != Some(entry.id);
        let from = entry.start.max(lower);
        let confirmed = entry.end.or_else(|| open.map(|session| session.checkpoint));
        let through = if entry.end.is_none() && !uncertain {
            observed_at.max(confirmed.unwrap_or(entry.start))
        } else {
            confirmed.unwrap_or(entry.start)
        }
        .min(upper);
        if through < from {
            continue;
        }
        let counted = !uncertain || through > from;
        let mut add = |date, start, end| {
            if (period.start()..=period.end()).contains(&date) {
                result.push(Slice {
                    entry: entry.clone(),
                    date,
                    start,
                    end,
                    counted,
                    uncertain,
                });
            }
        };
        if from == through {
            add(from.with_timezone(&entry.zone).date_naive(), from, through);
        } else {
            let span = TimeSpan::new(from, Some(through)).map_err(|e| e.to_string())?;
            visit_local_intervals(span, entry.zone, upper, |day, start, end| {
                add(day, start, end);
                Ok(())
            })
            .map_err(account_error)?;
        }
    }
    result.sort_by_key(|slice| (slice.date, slice.start, slice.entry.id));
    Ok(result)
}

impl Slice {
    pub(super) fn row(&self) -> SegmentRow {
        let entry = &self.entry;
        let local_time = |instant: Timestamp| {
            instant
                .with_timezone(&entry.zone)
                .format("%H:%M")
                .to_string()
        };
        let begin = local_time(self.start);
        let end = if entry.end.is_some() || (self.uncertain && self.counted) {
            local_time(self.end)
        } else {
            dash()
        };
        let pause = entry.kind == Kind::Break;
        let pending = !pause && entry.end.is_some() && entry.activity.trim().is_empty();
        SegmentRow {
            id: entry.id.to_string().into(),
            swatch: super::tracking::swatch(entry.project_color.as_deref()),
            day: super::input::display_date(self.date).into(),
            editable: entry.end.is_some(),
            title: if pause {
                "Pause"
            } else {
                entry.project_name.as_deref().unwrap_or("Ohne Projekt")
            }
            .into(),
            detail: if self.uncertain && self.counted {
                "Bis Speicherpunkt · Wiederaufnahme prüfen"
            } else if self.uncertain {
                "Offen · Wiederaufnahme prüfen"
            } else if entry.end.is_none() {
                if pause {
                    "Pause läuft"
                } else {
                    "Arbeit läuft"
                }
            } else if pending {
                "Beschreibung fehlt"
            } else {
                &entry.activity
            }
            .into(),
            time: format!(
                "{begin} – {}",
                if entry.end.is_some() || (self.uncertain && self.counted) {
                    end.as_str()
                } else {
                    "offen"
                }
            )
            .into(),
            begin: begin.into(),
            finish: end.into(),
            pause,
            pending,
            amount: if self.counted {
                duration((self.end - self.start).num_seconds(), false)
            } else {
                dash()
            }
            .into(),
        }
    }
}

pub(super) fn timeline(
    db: &Connection,
    date: NaiveDate,
    observed_at: Timestamp,
    trusted: Option<i64>,
) -> Result<Timeline, String> {
    let slices = slices(db, Period::day(date), observed_at, trusted)?;
    let mut result = Timeline::default();
    let mut work_millis = 0;
    let mut break_millis = 0;
    for slice in slices {
        if slice.counted {
            let elapsed = (slice.end - slice.start).num_milliseconds();
            match slice.entry.kind {
                Kind::Work => work_millis += elapsed,
                Kind::Break => break_millis += elapsed,
            }
        }
        result.rows.push(slice.row());
    }
    result.work_seconds = work_millis / 1000;
    result.break_seconds = break_millis / 1000;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use rusqlite::params;

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
        let result = timeline(&db, date, now, None).unwrap();
        assert_eq!(result.work_seconds, 1_800);
        assert_eq!(result.break_seconds, 900);
        assert_eq!(result.rows.len(), 3);
        assert!(result.rows[0].pending);
        assert_eq!(result.rows[0].time, "00:00 – 00:30");
        assert_eq!(result.rows[1].title, "Pause");
        assert_eq!(result.rows[2].amount, "–");
    }
}
