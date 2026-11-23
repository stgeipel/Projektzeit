use chrono::{DateTime, NaiveDate, TimeDelta};
use chrono_tz::Europe::Berlin;
use projektzeit_core::{
    DomainError, ProjectId, TimeSpan, TimelineEntry, Timestamp, WorkSegment, WorkStatus, Workday,
};

fn utc(value: &str) -> Timestamp {
    DateTime::parse_from_rfc3339(value).unwrap().to_utc()
}

fn hours(value: i64) -> Timestamp {
    utc("2026-11-23T07:00:00Z") + TimeDelta::hours(value)
}

fn span(start: i64, end: Option<i64>) -> TimeSpan {
    TimeSpan::new(hours(start), end.map(hours)).unwrap()
}

fn work(start: i64, end: Option<i64>, project: Option<i64>) -> TimelineEntry {
    TimelineEntry::Work(WorkSegment::new(
        span(start, end),
        project.map(ProjectId),
        None,
    ))
}

fn day(end: Option<i64>, target: Option<u32>) -> Workday {
    Workday {
        span: span(0, end),
        timezone: Berlin,
        target_seconds_snapshot: target,
    }
}

#[test]
fn shared_switch_boundary_keeps_unassigned_work_in_the_net_total() {
    let entries = [work(1, None, None), work(0, Some(1), Some(1))];
    let summary = day(None, Some(8 * 3600))
        .summarize(&entries, hours(2), 1800)
        .unwrap();

    assert_eq!(summary.wall_elapsed, TimeDelta::hours(2));
    assert_eq!(summary.net_worked, TimeDelta::hours(2));
    assert_eq!(summary.project_time[&ProjectId(1)], TimeDelta::hours(1));
    assert_eq!(summary.unassigned_work, TimeDelta::hours(1));
    assert_eq!(summary.remaining, Some(TimeDelta::hours(6)));
    assert_eq!(
        summary.estimated_finish,
        Some(hours(8) + TimeDelta::minutes(30))
    );
}

#[test]
fn break_accrues_until_resume_and_finished_day_stays_fixed() {
    let on_break = [
        work(0, Some(2), Some(1)),
        TimelineEntry::Break(span(2, None)),
    ];
    let paused = day(None, None).summarize(&on_break, hours(3), 0).unwrap();
    assert_eq!(paused.wall_elapsed, TimeDelta::hours(3));
    assert_eq!(paused.net_worked, TimeDelta::hours(2));
    assert_eq!(paused.break_time, TimeDelta::hours(1));
    assert_eq!(paused.remaining, None);
    assert_eq!(paused.estimated_finish, None);

    let finished = day(Some(5), Some(3 * 3600))
        .summarize(
            &[
                on_break[0].clone(),
                TimelineEntry::Break(span(2, Some(3))),
                work(3, Some(5), Some(1)),
            ],
            hours(81),
            0,
        )
        .unwrap();
    assert_eq!(finished.wall_elapsed, TimeDelta::hours(5));
    assert_eq!(finished.net_worked, TimeDelta::hours(4));
    assert_eq!(finished.break_time, TimeDelta::hours(1));
    assert_eq!(finished.remaining, Some(TimeDelta::zero()));
    assert_eq!(finished.estimated_finish, None);
}

#[test]
fn description_is_pending_until_trimmed_text_exists() {
    let closed = span(0, Some(1));
    for text in [None, Some(""), Some("\t\u{2003}\u{a0}\r\n")] {
        assert_eq!(
            WorkSegment::new(closed, None, text).status(),
            WorkStatus::PendingDescription
        );
    }

    let complete = WorkSegment::new(closed, None, Some("  Test durchgeführt  "));
    assert_eq!(complete.activity(), Some("Test durchgeführt"));
    assert_eq!(complete.status(), WorkStatus::Complete);
    assert_eq!(
        WorkSegment::new(span(0, None), None, Some("Draft")).status(),
        WorkStatus::Active
    );
}

#[test]
fn intervals_validate_order_bounds_and_overlaps() {
    assert_eq!(
        TimeSpan::new(hours(2), Some(hours(1))),
        Err(DomainError::EndBeforeStart)
    );
    let open = span(1, None);
    assert!(!open.overlaps(span(0, Some(1))));
    assert!(!open.overlaps(span(2, Some(2))));
    assert!(open.overlaps(span(0, Some(2))));
    assert_eq!(open.elapsed_at(hours(0)), TimeDelta::zero());
    assert_eq!(
        day(None, None).summarize(
            &[work(0, Some(2), None), TimelineEntry::Break(span(1, None))],
            hours(3),
            0,
        ),
        Err(DomainError::OverlappingEntries)
    );
    assert_eq!(
        day(Some(2), None).summarize(&[work(1, None, None)], hours(2), 0),
        Err(DomainError::EntryOutsideWorkday)
    );
}

#[test]
fn daylight_saving_changes_use_elapsed_utc_time() {
    let spring = TimeSpan::new(
        utc("2026-03-29T01:30:00+01:00"),
        Some(utc("2026-03-29T03:30:00+02:00")),
    )
    .unwrap();
    let fall = TimeSpan::new(
        utc("2026-10-25T02:30:00+02:00"),
        Some(utc("2026-10-25T02:30:00+01:00")),
    )
    .unwrap();
    for span in [spring, fall] {
        assert_eq!(span.elapsed_at(span.end().unwrap()), TimeDelta::hours(1));
    }
}

#[test]
fn midnight_keeps_start_date_and_elapsed_duration() {
    let start = utc("2026-11-23T23:30:00+01:00");
    let end = utc("2026-11-24T00:30:00+01:00");
    let span = TimeSpan::new(start, Some(end)).unwrap();
    let day = Workday {
        span,
        timezone: Berlin,
        target_seconds_snapshot: None,
    };

    assert_eq!(
        day.local_date(),
        NaiveDate::from_ymd_opt(2026, 11, 23).unwrap()
    );
    assert_eq!(
        day.summarize(&[TimelineEntry::Break(span)], end, 0)
            .unwrap()
            .break_time,
        TimeDelta::hours(1)
    );
}
