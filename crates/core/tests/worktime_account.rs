use chrono::{DateTime, NaiveDate, Utc};
use chrono_tz::{America::St_Johns, Europe::Berlin, Pacific::Apia};
use projektzeit_core::worktime_account::{
    Account, AccountError, Adjustment, Observations, OpeningBalance, Period, WeeklySchedule,
    attribute_work,
};
use projektzeit_core::{ProjectId, TimeSpan, TimelineEntry, Timestamp, WorkSegment};

fn date(value: &str) -> NaiveDate {
    value.parse().unwrap()
}

fn timestamp(value: &str) -> Timestamp {
    DateTime::parse_from_rfc3339(value)
        .unwrap()
        .with_timezone(&Utc)
}

fn schedule(hours: [u32; 7]) -> WeeklySchedule {
    WeeklySchedule::new(
        hours.iter().sum::<u32>() * 3600,
        hours.map(|hours| hours * 3600),
    )
    .unwrap()
}

fn account(effective: &str, initial: i64, weekday_hours: [u32; 7]) -> Account {
    let effective_date = date(effective);
    Account {
        opening: Some(OpeningBalance {
            effective_date,
            signed_seconds: initial,
        }),
        schedules: [(effective_date, schedule(weekday_hours))].into(),
        ..Account::default()
    }
}

#[test]
fn opening_and_monthly_carry_use_calendar_dates() {
    let account = account("2025-11-01", 20 * 3600, [8, 8, 8, 8, 6, 0, 0]);
    let november = date("2025-11-30");
    let december = date("2025-12-31");
    let mut observed = Observations::default();
    observed.actual.insert(november, 160 * 3600);
    let report = account
        .report(
            Period::month(november).unwrap(),
            november,
            december,
            &observed,
        )
        .unwrap();
    assert_eq!(report.totals.target, 152 * 3600);
    assert_eq!(report.totals.actual, 160 * 3600);
    assert_eq!(report.totals.delta, 8 * 3600);
    assert_eq!(report.opening_balance, Some(20 * 3600));
    assert_eq!(report.closing_balance, Some(28 * 3600));
    let next = account
        .report(
            Period::month(december).unwrap(),
            december,
            december,
            &observed,
        )
        .unwrap();
    assert_eq!(next.opening_balance, report.closing_balance);
    assert_eq!(next.totals.target, 176 * 3600);
}

#[test]
fn opening_cutoff_credits_negative_balances_and_adjustment_edits() {
    let mut account = account("2026-11-14", -5 * 3600, [0; 7]);
    let end = date("2026-11-30");
    let mut observed = Observations::default();
    observed.actual.insert(date("2026-11-13"), 123_456);
    observed.actual.insert(end, 3600);
    observed.approved_credits.insert(end, 90 * 60);
    assert_eq!(
        account.balance_as_of(end, &observed).unwrap(),
        Some(-150 * 60)
    );
    account.adjustments.push(Adjustment {
        effective_date: date("2026-11-13"),
        signed_seconds: 10_000,
    });
    account.adjustments.push(Adjustment {
        effective_date: end,
        signed_seconds: -75 * 60,
    });
    assert_eq!(
        account.balance_as_of(end, &observed).unwrap(),
        Some(-225 * 60)
    );
    account.adjustments[1].signed_seconds = -45 * 60;
    assert_eq!(
        account.balance_as_of(end, &observed).unwrap(),
        Some(-195 * 60)
    );
    let report = account
        .report(Period::month(end).unwrap(), end, end, &observed)
        .unwrap();
    assert_eq!(report.totals.target, 0);
    assert!(report.days[12].observed.is_none());
    assert!(report.days[13].observed.is_some());
}

#[test]
fn weekly_progress_and_midweek_schedule_change_respect_effective_dates() {
    assert_eq!(
        WeeklySchedule::new(100, [1; 7]),
        Err(AccountError::ScheduleDoesNotAddUp)
    );
    let mut account = account("2026-11-23", 0, [8, 8, 8, 8, 6, 0, 0]);
    let wednesday = date("2026-11-25");
    account
        .schedules
        .insert(wednesday, schedule([7, 7, 7, 7, 7, 0, 0]));
    account.overrides.insert(date("2026-11-26"), 0);
    let mut observed = Observations::default();
    observed.actual.insert(wednesday, 3600);
    observed.actual.insert(date("2026-11-27"), 3600);
    let report = account
        .report(
            Period::iso_week(wednesday).unwrap(),
            wednesday,
            wednesday,
            &observed,
        )
        .unwrap();
    assert_eq!(report.totals.actual, 3600);
    assert_eq!(report.totals.target, 23 * 3600);
    assert_eq!(report.full_planned_target, 30 * 3600);
    assert_eq!(report.remaining_planned_target, 7 * 3600);
    assert!(report.provisional);
    assert_eq!(report.days[3].planned_target, Some(0));
    assert_eq!(report.days[3].observed, None);
}

#[test]
fn leap_day_and_iso_week_53_at_year_boundary() {
    let leap_day = date("2024-02-29");
    let leap_month = Period::month(leap_day).unwrap();
    let account = account("2024-02-01", 0, [1; 7]);
    let report = account
        .report(
            leap_month,
            leap_day,
            date("2024-03-01"),
            &Observations::default(),
        )
        .unwrap();
    assert_eq!(report.days.len(), 29);
    assert_eq!(report.totals.target, 29 * 3600);
    assert!(!report.provisional);
    let week = Period::iso_week(date("2027-01-01")).unwrap();
    assert_eq!(week.start(), date("2026-12-28"));
    assert_eq!(week.end(), date("2027-01-03"));
}

fn span(start: &str, end: &str) -> TimeSpan {
    TimeSpan::new(timestamp(start), Some(timestamp(end))).unwrap()
}

#[test]
fn actual_counts_each_work_segment_once_at_local_midnight_and_dst() {
    let entries = [
        TimelineEntry::Work(WorkSegment::new(
            span("2026-03-28T23:30:00+01:00", "2026-03-29T00:30:00+01:00"),
            Some(ProjectId(1)),
            None,
        )),
        TimelineEntry::Break(span(
            "2026-03-29T00:30:00+01:00",
            "2026-03-29T01:30:00+01:00",
        )),
        TimelineEntry::Work(WorkSegment::new(
            span("2026-03-29T01:30:00+01:00", "2026-03-29T03:30:00+02:00"),
            None,
            None,
        )),
        TimelineEntry::Work(WorkSegment::new(
            span("2026-10-25T02:30:00+02:00", "2026-10-25T02:30:00+01:00"),
            None,
            None,
        )),
    ];
    let actual = attribute_work(
        entries.iter().map(|entry| (Berlin, entry)),
        timestamp("2026-11-30T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(
        actual,
        [
            (date("2026-03-28"), 1800),
            (date("2026-03-29"), 5400),
            (date("2026-10-25"), 3600)
        ]
        .into()
    );
}

#[test]
fn local_date_attribution_handles_a_skipped_date_and_a_rollback_across_midnight() {
    let skipped = TimelineEntry::Work(WorkSegment::new(
        span("2011-12-29T23:30:00-10:00", "2011-12-31T00:30:00+14:00"),
        None,
        None,
    ));
    // Newfoundland ended DST at 00:01, moving back to 23:01 the previous date.
    let repeated = TimelineEntry::Work(WorkSegment::new(
        span("2009-10-31T23:59:00-02:30", "2009-11-01T00:01:00-03:30"),
        None,
        None,
    ));
    let actual = attribute_work(
        [(Apia, &skipped), (St_Johns, &repeated)],
        timestamp("2026-11-30T00:00:00Z"),
    )
    .unwrap();
    assert_eq!(
        actual,
        [
            (date("2009-10-31"), 3600),
            (date("2009-11-01"), 120),
            (date("2011-12-29"), 1800),
            (date("2011-12-31"), 1800),
        ]
        .into()
    );
}
