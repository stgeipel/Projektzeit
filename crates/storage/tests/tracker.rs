use chrono::DateTime;
use chrono_tz::Europe::Berlin;
use projektzeit_core::{
    Timestamp,
    worktime_account::{Adjustment, OpeningBalance, Period, WeeklySchedule},
};
use projektzeit_storage::{
    open,
    tracker::{self, Command, EntryInput, Error, Kind, Recovery},
    worktime_account::{self as account, Change},
};
use rusqlite::Connection;
use tempfile::tempdir;

fn at(s: &str) -> Timestamp {
    DateTime::parse_from_rfc3339(s).unwrap().to_utc()
}
fn monday(hour: &str) -> Timestamp {
    at(&format!("2026-11-23T{hour}:00+01:00"))
}
fn tuesday(hour: &str) -> Timestamp {
    at(&format!("2026-11-24T{hour}:00+01:00"))
}

fn account_setup(db: &mut Connection, opening: i64) {
    let today = monday("00:00").with_timezone(&Berlin).date_naive();
    for change in [
        Change::Schedule {
            effective_date: today,
            schedule: WeeklySchedule::new(136800, [27360, 27360, 27360, 27360, 27360, 0, 0])
                .unwrap(),
        },
        Change::Opening(OpeningBalance {
            effective_date: today,
            signed_seconds: opening,
        }),
    ] {
        let preview = account::preview_on(db, change, today, today, monday("00:00")).unwrap();
        account::apply(db, preview, monday("00:00")).unwrap();
    }
}

fn hours(db: &mut Connection, start: &str, end: &str, id: Option<i64>, kind: Kind) -> i64 {
    tracker::save_entry(
        db,
        id,
        EntryInput {
            kind,
            project: None,
            zone: Berlin,
            start: monday(start),
            end: monday(end),
            activity: "Buchung",
        },
        tuesday("07:00"),
    )
    .unwrap()
}

#[test]
fn manual_entries_settle_monday_and_immediately_recalculate_balances() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("account.sqlite");
    let mut db = open(&path).unwrap();
    account_setup(&mut db, 0);
    let first = hours(&mut db, "08:00", "12:00", None, Kind::Work);
    hours(&mut db, "12:00", "12:30", None, Kind::Break);
    let last = hours(&mut db, "12:30", "16:30", None, Kind::Work);
    let today = tuesday("07:00").with_timezone(&Berlin).date_naive();
    let week = Period::iso_week(today).unwrap();
    let month = Period::month(today).unwrap();
    for period in [week, month] {
        let report = account::report(&mut db, period, today, today, tuesday("07:00")).unwrap();
        assert_eq!(report.period.totals.actual, 28800);
        assert_eq!(report.period.finalized_totals.delta, 1440);
        assert_eq!(report.balance_as_of, Some(1440));
    }
    hours(&mut db, "12:30", "15:30", Some(last), Kind::Work);
    let monday = today.pred_opt().unwrap();
    for (period, point) in [(week, today), (month, today), (Period::day(monday), monday)] {
        let report = account::report(&mut db, period, point, today, tuesday("07:00")).unwrap();
        assert_eq!(report.period.finalized_totals.delta, -2160);
        assert_eq!(report.balance_as_of, Some(-2160));
    }
    drop(db);
    let mut db = open(&path).unwrap();
    assert_eq!(
        account::report(&mut db, week, today, today, tuesday("07:00"))
            .unwrap()
            .balance_as_of,
        Some(-2160)
    );
    assert_eq!(tracker::entry(&db, first).unwrap().activity, "Buchung");
}

#[test]
fn shared_day_switches_pauses_finishes_and_resumes_without_double_counting() {
    let dir = tempdir().unwrap();
    let mut db = open(dir.path().join("day.sqlite")).unwrap();
    account_setup(&mut db, 0);
    let project =
        tracker::save_project(&db, None, "Neu", "Beschreibung", "#123456", monday("07:00"))
            .unwrap();
    let started = tracker::start(&mut db, Berlin, Some(project), monday("08:00")).unwrap();
    assert!(matches!(
        tracker::start(&mut db, Berlin, None, monday("08:01")),
        Err(Error::InvalidTransition)
    ));
    let first = tracker::transition(
        &mut db,
        started.entry_id,
        Command::Switch(None),
        monday("10:00"),
    )
    .unwrap()
    .unwrap();
    let current = tracker::active(&db).unwrap().unwrap();
    assert_eq!(
        tracker::entry(&db, first).unwrap().end,
        Some(current.started)
    );
    tracker::describe(&db, first, "Analyse", monday("10:01")).unwrap();
    let today = monday("11:00").with_timezone(&Berlin).date_naive();
    let underway = account::report_live(
        &mut db,
        Period::day(today),
        today,
        today,
        monday("11:00"),
        Some(current.entry_id),
    )
    .unwrap();
    assert_eq!(underway.period.totals.actual, 10800);
    assert_eq!(underway.balance_as_of, Some(0));
    tracker::transition(&mut db, current.entry_id, Command::Pause, monday("12:00")).unwrap();
    let pause = tracker::active(&db).unwrap().unwrap();
    let during_pause = account::report_live(
        &mut db,
        Period::day(today),
        today,
        today,
        monday("12:15"),
        Some(pause.entry_id),
    )
    .unwrap();
    assert_eq!(during_pause.period.totals.actual, 14400);
    tracker::transition(
        &mut db,
        pause.entry_id,
        Command::Resume(Some(project)),
        monday("12:30"),
    )
    .unwrap();
    let current = tracker::active(&db).unwrap().unwrap();
    tracker::transition(&mut db, current.entry_id, Command::Finish, monday("16:30")).unwrap();
    let done = account::report(&mut db, Period::day(today), today, today, monday("16:30")).unwrap();
    assert_eq!(done.period.totals.actual, 28800);
    assert_eq!(done.balance_as_of, Some(1440));
    assert!(tracker::day_finished(&db, today).unwrap());
    assert_eq!(tracker::last_project(&db).unwrap(), Some(project));
    assert_eq!(tracker::pending_descriptions(&db).unwrap().len(), 2);
    tracker::archive_project(&db, project, true, monday("16:31")).unwrap();
    assert!(matches!(
        tracker::start(&mut db, Berlin, Some(project), monday("17:00")),
        Err(Error::ArchivedProject)
    ));
    let reopened = tracker::start(&mut db, Berlin, None, monday("17:00")).unwrap();
    assert_eq!(reopened.workday_id, started.workday_id);
    assert_eq!(
        tracker::entry(&db, first).unwrap().project_name.as_deref(),
        Some("Neu")
    );
}

#[test]
fn recovery_and_invalid_edits_preserve_existing_timeline() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("restore.sqlite");
    let mut db = open(&path).unwrap();
    let session = tracker::start(&mut db, Berlin, None, monday("08:00")).unwrap();
    tracker::checkpoint(&db, session.entry_id, monday("09:00")).unwrap();
    drop(db);
    let mut db = open(path).unwrap();
    let session = tracker::active(&db).unwrap().unwrap();
    tracker::recover(
        &mut db,
        session.entry_id,
        Recovery::BreakSinceCheckpoint,
        tuesday("07:00"),
    )
    .unwrap();
    let first = tracker::entry(&db, session.entry_id).unwrap();
    assert_eq!(first.end, Some(monday("09:00")));
    let before = tracker::entries(&db, monday("07:00"), tuesday("08:00")).unwrap();
    assert_eq!(before[1].kind, Kind::Break);
    assert_eq!(before[1].end, Some(tuesday("07:00")));
    let entry = EntryInput {
        kind: Kind::Work,
        project: None,
        zone: Berlin,
        start: monday("08:15"),
        end: monday("08:45"),
        activity: "Konflikt",
    };
    assert!(matches!(
        tracker::save_entry(&mut db, None, entry, tuesday("08:00")),
        Err(Error::Overlap)
    ));
    assert!(matches!(
        tracker::save_bounds(
            &mut db,
            session.workday_id,
            monday("08:01"),
            monday("08:10"),
            tuesday("08:00")
        ),
        Err(Error::OpenEntry)
    ));
    tracker::delete_entry(&mut db, session.entry_id).unwrap();
    assert!(tracker::active(&db).unwrap().is_some());
}

#[test]
fn opening_and_adjustments_apply_to_finalized_balance() {
    let dir = tempdir().unwrap();
    let mut db = open(dir.path().join("balances.sqlite")).unwrap();
    account_setup(&mut db, 45000);
    hours(&mut db, "06:00", "21:36", None, Kind::Work);
    let today = tuesday("07:00").with_timezone(&Berlin).date_naive();
    let change = Change::AddAdjustment {
        adjustment: Adjustment {
            effective_date: today,
            signed_seconds: -3600,
        },
        reason: "Korrektur".into(),
    };
    let preview = account::preview(&mut db, change, today, tuesday("07:00")).unwrap();
    account::apply(&mut db, preview, tuesday("07:00")).unwrap();
    let report = account::report(
        &mut db,
        Period::month(today).unwrap(),
        today,
        today,
        tuesday("07:00"),
    )
    .unwrap();
    assert_eq!(report.period.finalized_totals.delta, 28800);
    assert_eq!(report.balance_as_of, Some(70200));
}

#[test]
fn manual_finish_and_boundary_correction_settle_today_and_preserve_the_break() {
    let dir = tempdir().unwrap();
    let mut db = open(dir.path().join("bounds.sqlite")).unwrap();
    account_setup(&mut db, 0);
    let first = hours(&mut db, "08:00", "12:00", None, Kind::Work);
    hours(&mut db, "12:00", "12:30", None, Kind::Break);
    let last = hours(&mut db, "12:30", "16:30", None, Kind::Work);
    let now = monday("17:00");
    let today = now.with_timezone(&Berlin).date_naive();
    let period = Period::day(today);
    assert_eq!(
        account::report(&mut db, period, today, today, now)
            .unwrap()
            .balance_as_of,
        Some(0)
    );
    tracker::finalize_manual_day(&mut db, Berlin, now).unwrap();
    assert_eq!(
        account::report(&mut db, period, today, today, now)
            .unwrap()
            .balance_as_of,
        Some(1440)
    );

    let day = tracker::entry(&db, first).unwrap().workday_id;
    tracker::save_bounds(&mut db, day, monday("08:30"), monday("16:00"), now).unwrap();
    let before = account::report(&mut db, period, today, today, now).unwrap();
    assert_eq!(before.period.totals.actual, 7 * 3600);
    assert_eq!(before.balance_as_of, Some(-2160));
    assert!(tracker::save_bounds(&mut db, day, monday("07:30"), monday("12:20"), now).is_err());
    let entries = tracker::entries(&db, monday("07:00"), now).unwrap();
    assert_eq!(entries[0].start, monday("08:30"));
    assert_eq!(entries[1].end, Some(monday("12:30")));
    assert_eq!(
        tracker::entry(&db, last).unwrap().end,
        Some(monday("16:00"))
    );
}

#[test]
fn clock_rollback_preserves_the_checkpoint_and_allows_finishing_at_it() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("rollback.sqlite");
    let mut db = open(&path).unwrap();
    account_setup(&mut db, 0);
    let session = tracker::start(&mut db, Berlin, None, monday("08:00")).unwrap();
    tracker::checkpoint(&db, session.entry_id, monday("08:30")).unwrap();
    drop(db);

    let mut db = open(&path).unwrap();
    let rollback = monday("08:10");
    let day = rollback.with_timezone(&Berlin).date_naive();
    let totals =
        |db: &mut Connection| account::report(db, Period::day(day), day, day, rollback).unwrap();
    let recovering = totals(&mut db);
    assert_eq!(recovering.period.totals.actual, 1800);
    assert_eq!(recovering.period.finalized_totals.target, 0);
    assert!(recovering.period.excludes_open_time);
    for command in [Command::Pause, Command::Finish] {
        assert!(matches!(
            tracker::transition(&mut db, session.entry_id, command, rollback),
            Err(Error::InvalidRange)
        ));
    }
    assert!(matches!(
        tracker::recover(&mut db, session.entry_id, Recovery::Include, rollback),
        Err(Error::InvalidRange)
    ));
    let pending = tracker::recover(
        &mut db,
        session.entry_id,
        Recovery::FinishAtCheckpoint,
        rollback,
    )
    .unwrap();
    assert_eq!(pending, Some(session.entry_id));
    assert_eq!(
        tracker::entry(&db, session.entry_id).unwrap().end,
        Some(monday("08:30"))
    );
    assert_eq!(totals(&mut db).period.totals.actual, 1800);
}

#[test]
fn finishing_at_the_exact_midnight_leaves_the_new_day_unfinished() {
    let dir = tempdir().unwrap();
    let mut db = open(dir.path().join("midnight.sqlite")).unwrap();
    account_setup(&mut db, 0);
    let session = tracker::start(&mut db, Berlin, None, monday("16:00")).unwrap();
    tracker::transition(&mut db, session.entry_id, Command::Finish, tuesday("00:00")).unwrap();
    let today = tuesday("00:00").with_timezone(&Berlin).date_naive();
    assert!(tracker::day_finished(&db, today.pred_opt().unwrap()).unwrap());
    assert!(!tracker::day_finished(&db, today).unwrap());
    let report = account::report(
        &mut db,
        Period::iso_week(today).unwrap(),
        today,
        today,
        tuesday("00:00"),
    )
    .unwrap();
    assert_eq!(report.period.finalized_totals.actual, 28_800);
    assert_eq!(report.balance_as_of, Some(1_440));
}

#[test]
fn unfinished_overnight_manual_entry_can_be_completed_on_its_end_date() {
    let dir = tempdir().unwrap();
    let mut db = open(dir.path().join("overnight.sqlite")).unwrap();
    let now = tuesday("07:00");
    account_setup(&mut db, 0);
    tracker::save_entry(
        &mut db,
        None,
        EntryInput {
            kind: Kind::Work,
            project: None,
            zone: Berlin,
            start: monday("22:30"),
            end: tuesday("05:30"),
            activity: "Nachtwache",
        },
        now,
    )
    .unwrap();
    let today = now.with_timezone(&Berlin).date_naive();
    assert!(!tracker::day_finished(&db, today).unwrap());
    assert!(tracker::can_finalize_manual_day(&db, today, now).unwrap());
    tracker::finalize_manual_day(&mut db, Berlin, now).unwrap();
    let report =
        account::report(&mut db, Period::iso_week(today).unwrap(), today, today, now).unwrap();
    assert_eq!(report.period.days[0].observed.unwrap().actual, 5400);
    assert_eq!(report.period.days[1].observed.unwrap().actual, 19_800);
    assert!(tracker::day_finished(&db, today).unwrap());
}

#[test]
fn interruption_during_a_transition_keeps_committed_wal_and_rolls_back_partial_change() {
    const CHILD_PATH: &str = "PROJEKTZEIT_TEST_RECOVERY_PATH";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        let mut db = open(path).unwrap();
        let started = tracker::start(&mut db, Berlin, None, monday("08:00")).unwrap();
        tracker::checkpoint(&db, started.entry_id, monday("08:30")).unwrap();
        let tx = db.transaction().unwrap();
        tx.execute(
            "UPDATE timeline_entries SET ended_at_utc_ms=?1 WHERE id=?2",
            (monday("09:00").timestamp_millis(), started.entry_id),
        )
        .unwrap();
        // Bypass Rust destructors and skip every explicit close/rollback.
        std::process::exit(0);
    }

    let folder = tempdir().unwrap();
    let path = folder.path().join("wal-recovery.sqlite");
    let process = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "interruption_during_a_transition_keeps_committed_wal_and_rolls_back_partial_change",
        ])
        .env(CHILD_PATH, &path)
        .output()
        .unwrap();
    assert!(
        process.status.success(),
        "{}",
        String::from_utf8_lossy(&process.stderr)
    );
    let mut db = open(&path).unwrap();
    let active = tracker::active(&db).unwrap().unwrap();
    assert_eq!(active.started, monday("08:00"));
    assert_eq!(active.checkpoint, monday("08:30"));
    assert_eq!(tracker::entry(&db, active.entry_id).unwrap().end, None);

    tracker::recover(
        &mut db,
        active.entry_id,
        Recovery::FinishAtCheckpoint,
        monday("09:05"),
    )
    .unwrap();
    assert_eq!(
        tracker::entry(&db, active.entry_id).unwrap().end,
        Some(monday("08:30"))
    );
    assert_eq!(tracker::pending_descriptions(&db).unwrap().len(), 1);
}

#[test]
fn changing_an_existing_zero_length_booking_can_complete_its_description() {
    let folder = tempdir().unwrap();
    let mut db = open(folder.path().join("zero.sqlite")).unwrap();
    let now = monday("08:00");
    let active = tracker::start(&mut db, Berlin, None, now).unwrap();
    tracker::transition(&mut db, active.entry_id, Command::Finish, now).unwrap();
    let input = || EntryInput {
        kind: Kind::Work,
        project: None,
        zone: Berlin,
        start: now,
        end: now,
        activity: "Versehentlicher Start",
    };
    assert!(matches!(
        tracker::save_entry(&mut db, None, input(), now),
        Err(Error::InvalidRange)
    ));
    tracker::save_entry(&mut db, Some(active.entry_id), input(), now).unwrap();
    assert!(tracker::pending_descriptions(&db).unwrap().is_empty());
}

#[test]
fn v3_migration_unblocks_an_orphaned_open_day_without_inventing_bookings() {
    let folder = tempdir().unwrap();
    let path = folder.path().join("older.sqlite");
    let mut db = open(&path).unwrap();
    let existing = hours(&mut db, "08:00", "09:00", None, Kind::Work);
    let workday = tracker::entry(&db, existing).unwrap().workday_id;
    db.execute(
        "UPDATE workdays SET ended_at_utc_ms=NULL WHERE id=?1",
        [workday],
    )
    .unwrap();
    db.pragma_update(None, "user_version", 3).unwrap();
    assert!(matches!(
        tracker::start(&mut db, Berlin, None, monday("10:00")),
        Err(Error::InvalidTransition)
    ));
    drop(db);

    let mut db = open(&path).unwrap();
    assert_eq!(
        tracker::day_bounds(&db, workday).unwrap().2,
        monday("09:00")
    );
    assert_eq!(tracker::entry(&db, existing).unwrap().activity, "Buchung");
    assert_eq!(
        tracker::start(&mut db, Berlin, None, monday("10:00"))
            .unwrap()
            .workday_id,
        workday
    );
}
