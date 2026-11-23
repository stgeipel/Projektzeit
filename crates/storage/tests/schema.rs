use std::path::Path;

use projektzeit_storage::{OpenError, open};
use rusqlite::{Connection, params};
use tempfile::tempdir;

fn add_day(db: &Connection, start: i64, end: Option<i64>) -> rusqlite::Result<i64> {
    db.execute(
        "INSERT INTO workdays (local_date, timezone, started_at_utc_ms, ended_at_utc_ms)
         VALUES ('2026-11-23', 'Europe/Berlin', ?1, ?2)",
        params![start, end],
    )?;
    Ok(db.last_insert_rowid())
}

fn add_entry(
    db: &Connection,
    day: i64,
    kind: &str,
    start: i64,
    end: Option<i64>,
    activity: Option<&str>,
) -> rusqlite::Result<i64> {
    db.execute(
        "INSERT INTO timeline_entries (
            workday_id, kind, started_at_utc_ms, ended_at_utc_ms,
            activity_text, created_at_utc_ms, updated_at_utc_ms
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?3, ?3)",
        params![day, kind, start, end, activity],
    )?;
    Ok(db.last_insert_rowid())
}

fn status(db: &Connection, id: i64) -> Option<String> {
    db.query_row(
        "SELECT work_status FROM timeline_entries WHERE id = ?1",
        [id],
        |row| row.get(0),
    )
    .unwrap()
}

fn pragma<T: rusqlite::types::FromSql>(db: &Connection, name: &str) -> T {
    db.pragma_query_value(None, name, |row| row.get(0)).unwrap()
}

#[test]
fn migration_is_repeatable_and_preserves_committed_recovery_state() {
    let directory = tempdir().unwrap();
    let path = directory.path().join("tracker.sqlite");

    let db = open(&path).unwrap();
    assert_eq!(pragma::<i32>(&db, "user_version"), 1);
    assert_eq!(pragma::<String>(&db, "journal_mode"), "wal");
    assert_eq!(pragma::<i32>(&db, "foreign_keys"), 1);
    assert_eq!(pragma::<i32>(&db, "synchronous"), 2);
    let day = add_day(&db, 0, None).unwrap();
    let pending = add_entry(&db, day, "work", 0, Some(1000), Some("\t\u{2003}")).unwrap();
    let active = add_entry(&db, day, "work", 1000, None, Some("Draft")).unwrap();
    drop(db);

    let db = open(&path).unwrap();
    assert_eq!(status(&db, pending).as_deref(), Some("pending_description"));
    assert_eq!(status(&db, active).as_deref(), Some("active"));
    db.execute(
        "UPDATE timeline_entries SET activity_text = 'Erfasst' WHERE id = ?1",
        [pending],
    )
    .unwrap();
    assert_eq!(status(&db, pending).as_deref(), Some("complete"));
}

#[test]
fn constraints_allow_touching_boundaries_and_reject_overlaps_and_stranded_entries() {
    let directory = tempdir().unwrap();
    let db = open(directory.path().join("tracker.sqlite")).unwrap();
    let active_day = add_day(&db, 0, None).unwrap();
    assert!(add_day(&db, 0, None).is_err());
    let historical_day = add_day(&db, 0, Some(1500)).unwrap();

    add_entry(&db, active_day, "work", 0, Some(1000), None).unwrap();
    let pause = add_entry(&db, active_day, "break", 1000, Some(1500), None).unwrap();
    assert_eq!(status(&db, pause), None);
    assert!(add_entry(&db, active_day, "break", 800, Some(1001), None).is_err());
    assert!(add_entry(&db, historical_day, "work", 900, Some(1100), None).is_err());
    assert!(add_entry(&db, historical_day, "work", 1500, None, None).is_err());
    assert!(add_entry(&db, active_day, "break", 1500, Some(1600), Some("Text")).is_err());
    assert!(
        db.execute(
            "UPDATE timeline_entries SET started_at_utc_ms = 900 WHERE id = ?1",
            [pause]
        )
        .is_err()
    );

    add_entry(&db, active_day, "work", 1500, None, None).unwrap();
    assert!(add_entry(&db, active_day, "break", 2000, None, None).is_err());
    assert!(
        db.execute(
            "UPDATE workdays SET ended_at_utc_ms = 2500 WHERE id = ?1",
            [active_day]
        )
        .is_err()
    );
    assert!(
        db.execute(
            "UPDATE workdays SET started_at_utc_ms = 1 WHERE id = ?1",
            [active_day]
        )
        .is_err()
    );
}

#[test]
fn a_failed_multi_statement_change_rolls_back_when_transaction_is_dropped() {
    let directory = tempdir().unwrap();
    let mut db = open(directory.path().join("tracker.sqlite")).unwrap();
    let day = add_day(&db, 0, None).unwrap();
    let active = add_entry(&db, day, "work", 0, None, None).unwrap();

    {
        let tx = db.transaction().unwrap();
        tx.execute(
            "UPDATE timeline_entries SET ended_at_utc_ms = 1000 WHERE id = ?1",
            [active],
        )
        .unwrap();
        assert!(add_entry(&tx, day, "break", 900, None, None).is_err());
    }

    assert_eq!(status(&db, active).as_deref(), Some("active"));
}

#[test]
fn refuses_foreign_and_newer_databases() {
    let directory = tempdir().unwrap();
    let foreign = directory.path().join("foreign.sqlite");
    let db = Connection::open(&foreign).unwrap();
    db.execute_batch("CREATE TABLE unrelated (id INTEGER)")
        .unwrap();
    drop(db);
    assert!(matches!(open(&foreign), Err(OpenError::ForeignDatabase)));

    let newer = directory.path().join("newer.sqlite");
    let db = open(&newer).unwrap();
    db.pragma_update(None, "user_version", 2).unwrap();
    drop(db);
    assert!(matches!(open(&newer), Err(OpenError::UnsupportedSchema(2))));
}

#[test]
fn requires_a_file_backed_database() {
    assert!(matches!(
        open(Path::new(":memory:")),
        Err(OpenError::WalUnavailable(_))
    ));
}
