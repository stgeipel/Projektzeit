use chrono::{DateTime, NaiveDate};
use projektzeit_core::{
    Timestamp,
    worktime_account::{Adjustment, OpeningBalance, Period, WeeklySchedule},
};
use projektzeit_storage::{
    open,
    worktime_account::{
        BalanceState, Change, StoreError, apply, audit_history, preview, report, settings,
    },
};
use rusqlite::{Connection, params};
use tempfile::tempdir;

fn date(text: &str) -> NaiveDate {
    text.parse().unwrap()
}

fn at(text: &str) -> Timestamp {
    DateTime::parse_from_rfc3339(text).unwrap().to_utc()
}

fn observed_at() -> Timestamp {
    at("2026-01-05T12:00:00+01:00")
}

fn as_of() -> NaiveDate {
    date("2026-01-04")
}

fn change(db: &mut Connection, change: Change) {
    let edit = preview(db, change, as_of(), observed_at()).unwrap();
    apply(db, edit, observed_at()).unwrap();
}

fn configure(db: &mut Connection) {
    change(
        db,
        Change::Schedule {
            effective_date: date("2025-12-29"),
            schedule: WeeklySchedule::new(0, [0; 7]).unwrap(),
        },
    );
    change(
        db,
        Change::Opening(OpeningBalance {
            effective_date: date("2025-12-29"),
            signed_seconds: 20 * 3600,
        }),
    );
}

fn add_workday(db: &Connection, start: &str, end: Option<&str>) -> i64 {
    db.execute(
        "INSERT INTO workdays (local_date, timezone, started_at_utc_ms, ended_at_utc_ms)
                VALUES (?1, 'Europe/Berlin', ?2, ?3)",
        params![
            &start[..10],
            at(start).timestamp_millis(),
            end.map(|time| at(time).timestamp_millis())
        ],
    )
    .unwrap();
    db.last_insert_rowid()
}

fn add_entry(db: &Connection, day: i64, kind: &str, start: &str, end: Option<&str>) -> i64 {
    db.execute("INSERT INTO timeline_entries
                (workday_id, kind, started_at_utc_ms, ended_at_utc_ms, created_at_utc_ms, updated_at_utc_ms)
                VALUES (?1, ?2, ?3, ?4, ?3, ?3)",
        params![day, kind, at(start).timestamp_millis(), end.map(|time| at(time).timestamp_millis())]).unwrap();
    db.last_insert_rowid()
}

#[test]
fn upgrades_v1_and_uses_corrected_timeline_for_later_balances() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("v1.sqlite");
    let db = Connection::open(&path).unwrap();
    db.execute_batch(include_str!("../migrations/0001_initial.sql"))
        .unwrap();
    db.pragma_update(None, "application_id", 0x505A4549)
        .unwrap();
    db.pragma_update(None, "user_version", 1).unwrap();
    let old_day = add_workday(
        &db,
        "2025-12-31T23:00:00+01:00",
        Some("2026-01-01T02:00:00+01:00"),
    );
    add_entry(
        &db,
        old_day,
        "work",
        "2025-12-31T23:00:00+01:00",
        Some("2026-01-01T00:30:00+01:00"),
    );
    add_entry(
        &db,
        old_day,
        "break",
        "2026-01-01T00:30:00+01:00",
        Some("2026-01-01T01:00:00+01:00"),
    );
    let corrected = add_entry(
        &db,
        old_day,
        "work",
        "2026-01-01T01:00:00+01:00",
        Some("2026-01-01T01:30:00+01:00"),
    );
    let open_day = add_workday(&db, "2026-01-04T08:00:00+01:00", None);
    add_entry(&db, open_day, "work", "2026-01-04T08:00:00+01:00", None);
    drop(db);

    let mut db = open(&path).unwrap();
    configure(&mut db);
    let january = Period::month(as_of()).unwrap();
    let before = report(&mut db, january, as_of(), date("2026-01-05"), observed_at()).unwrap();
    assert_eq!(before.period.opening_balance, Some(21 * 3600));
    assert_eq!(before.period.totals.actual, 3600);
    assert_eq!(before.balance_as_of, Some(22 * 3600));
    assert!(before.period.excludes_open_time && before.balance_provisional);

    db.execute(
        "UPDATE timeline_entries SET ended_at_utc_ms = ?1 WHERE id = ?2",
        params![
            at("2026-01-01T02:00:00+01:00").timestamp_millis(),
            corrected
        ],
    )
    .unwrap();
    let after = report(&mut db, january, as_of(), date("2026-01-05"), observed_at()).unwrap();
    assert_eq!(after.period.totals.actual, 5400);
    assert_eq!(after.balance_as_of, Some(22 * 3600 + 1800));
}

#[test]
fn opening_versions_adjustment_edits_and_voids_have_atomic_history() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("account.sqlite");
    let mut db = open(&path).unwrap();
    configure(&mut db);
    let base = preview(
        &mut db,
        Change::Opening(OpeningBalance {
            effective_date: date("2026-01-01"),
            signed_seconds: -5 * 3600,
        }),
        as_of(),
        observed_at(),
    )
    .unwrap();
    assert_eq!(base.before().balance, BalanceState::Calculated(20 * 3600));
    assert_eq!(base.after().balance, BalanceState::Calculated(-5 * 3600));
    apply(&mut db, base, observed_at()).unwrap();

    let mut adjustment = Adjustment {
        effective_date: as_of(),
        signed_seconds: -75 * 60,
    };
    assert!(matches!(
        preview(
            &mut db,
            Change::AddAdjustment {
                adjustment,
                reason: " \t\u{2003}".into(),
            },
            as_of(),
            observed_at()
        ),
        Err(StoreError::ReasonRequired)
    ));
    change(
        &mut db,
        Change::AddAdjustment {
            adjustment,
            reason: "  Correction  ".into(),
        },
    );
    let id = settings(&mut db).unwrap().adjustments[0].id;
    drop(db);

    let mut db = open(&path).unwrap();
    adjustment.signed_seconds = -45 * 60;
    let corrected = preview(
        &mut db,
        Change::EditAdjustment {
            id,
            adjustment,
            reason: "Corrected duration".into(),
        },
        as_of(),
        observed_at(),
    )
    .unwrap();
    assert_eq!(
        corrected.before().balance,
        BalanceState::Calculated(-375 * 60)
    );
    assert_eq!(
        corrected.after().balance,
        BalanceState::Calculated(-345 * 60)
    );
    apply(&mut db, corrected, observed_at()).unwrap();
    change(
        &mut db,
        Change::VoidAdjustment {
            id,
            reason: "Duplicate".into(),
        },
    );
    assert!(settings(&mut db).unwrap().adjustments[0].voided);
    assert!(matches!(
        preview(
            &mut db,
            Change::VoidAdjustment {
                id,
                reason: "Again".into()
            },
            as_of(),
            observed_at()
        ),
        Err(StoreError::AdjustmentUnavailable(_))
    ));
    let history = audit_history(&db).unwrap();
    assert_eq!(history.len(), 6);
    assert_eq!(history[0].action, "void");
    assert!(history[1].before_json.as_ref().unwrap().contains("-4500"));
    assert!(history[1].after_json.as_ref().unwrap().contains("-2700"));
    let counts: (i64, i64) = db
        .query_row(
            "SELECT count(*), count(superseded_at_utc_ms) FROM opening_balance_versions",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(counts, (2, 1));
}

#[test]
fn configuration_preview_is_rejected_after_a_concurrent_edit() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("account.sqlite");
    let mut first = open(&path).unwrap();
    configure(&mut first);
    let proposed = preview(
        &mut first,
        Change::Schedule {
            effective_date: date("2026-01-01"),
            schedule: WeeklySchedule::new(5 * 3600, [3600, 3600, 3600, 3600, 3600, 0, 0]).unwrap(),
        },
        as_of(),
        observed_at(),
    )
    .unwrap();
    assert_eq!(
        proposed.after().balance,
        BalanceState::Calculated(18 * 3600)
    );

    let mut second = open(&path).unwrap();
    change(
        &mut second,
        Change::SetTarget {
            date: date("2026-01-01"),
            seconds: 0,
            reason: "Approved exception".into(),
        },
    );
    assert!(matches!(
        apply(&mut first, proposed, observed_at()),
        Err(StoreError::StalePreview)
    ));
    assert_eq!(settings(&mut first).unwrap().schedules.len(), 1);
    assert_eq!(audit_history(&first).unwrap().len(), 3);
}

#[test]
fn audit_write_failure_rolls_back_configuration_and_supersession() {
    let directory = tempdir().unwrap();
    let mut db = open(directory.path().join("account.sqlite")).unwrap();
    configure(&mut db);
    db.execute_batch(
        "CREATE TRIGGER reject_audit BEFORE INSERT ON account_audit_events
                      BEGIN SELECT RAISE(ABORT, 'test audit write failure'); END;",
    )
    .unwrap();
    let edit = preview(
        &mut db,
        Change::Opening(OpeningBalance {
            effective_date: date("2026-01-01"),
            signed_seconds: 0,
        }),
        as_of(),
        observed_at(),
    )
    .unwrap();
    assert!(apply(&mut db, edit, observed_at()).is_err());
    let stored = settings(&mut db).unwrap().opening.unwrap();
    assert_eq!(stored.signed_seconds, 20 * 3600);
    assert_eq!(stored.effective_date, date("2025-12-29"));
    assert_eq!(audit_history(&db).unwrap().len(), 2);
}

#[test]
fn automatic_schedule_edits_preserve_past_and_future_targets_across_restart() {
    use projektzeit_storage::worktime_account::{save_current_schedule, schedule_at};

    let folder = tempdir().unwrap();
    let path = folder.path().join("account.sqlite");
    let mut db = open(&path).unwrap();
    configure(&mut db);
    let today = date("2026-01-05");
    let tomorrow = date("2026-01-06");
    let yesterday = date("2026-01-04");
    let upcoming =
        WeeklySchedule::new(72_000, [14_400, 14_400, 14_400, 14_400, 14_400, 0, 0]).unwrap();
    change(
        &mut db,
        Change::Schedule {
            effective_date: tomorrow,
            schedule: upcoming,
        },
    );
    let current =
        WeeklySchedule::new(144_000, [28_800, 28_800, 28_800, 28_800, 28_800, 0, 0]).unwrap();
    let corrected =
        WeeklySchedule::new(147_600, [32_400, 28_800, 28_800, 28_800, 28_800, 0, 0]).unwrap();
    save_current_schedule(&mut db, today, current, observed_at()).unwrap();
    save_current_schedule(&mut db, today, corrected, observed_at()).unwrap();
    let length = audit_history(&db).unwrap().len();
    save_current_schedule(&mut db, today, corrected, observed_at()).unwrap();
    assert_eq!(audit_history(&db).unwrap().len(), length);
    drop(db);

    let mut db = open(path).unwrap();
    assert_eq!(
        schedule_at(&db, yesterday)
            .unwrap()
            .unwrap()
            .weekly_seconds(),
        0
    );
    assert_eq!(schedule_at(&db, today).unwrap(), Some(corrected));
    assert_eq!(schedule_at(&db, tomorrow).unwrap(), Some(upcoming));
    let day = report(&mut db, Period::day(today), today, today, observed_at()).unwrap();
    assert_eq!(day.period.totals.target, 32_400);
    let old = report(
        &mut db,
        Period::day(yesterday),
        yesterday,
        today,
        observed_at(),
    )
    .unwrap();
    assert_eq!(old.period.totals.target, 0);
}
