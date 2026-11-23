//! SQLite connection setup and versioned schema migrations.

use std::{fmt, path::Path, time::Duration};

use rusqlite::{Connection, TransactionBehavior};

const APPLICATION_ID: i32 = 0x505A4549; // PZEI
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];

#[derive(Debug)]
pub enum OpenError {
    Sqlite(rusqlite::Error),
    ForeignDatabase,
    UnsupportedSchema(i32),
    WalUnavailable(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "{error}"),
            Self::ForeignDatabase => formatter.write_str("file is not a Projektzeit database"),
            Self::UnsupportedSchema(version) => {
                write!(formatter, "unsupported database schema version {version}")
            }
            Self::WalUnavailable(mode) => {
                write!(formatter, "SQLite WAL unavailable (mode: {mode})")
            }
        }
    }
}

impl std::error::Error for OpenError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlite(error) => Some(error),
            _ => None,
        }
    }
}

impl From<rusqlite::Error> for OpenError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

/// Open or create a local file, applying any pending migrations in one transaction.
pub fn open(path: impl AsRef<Path>) -> Result<Connection, OpenError> {
    let mut connection = Connection::open(path)?;
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "synchronous", "FULL")?;
    migrate(&mut connection)?;

    let mode: String = connection.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(OpenError::WalUnavailable(mode));
    }
    Ok(connection)
}

fn migrate(connection: &mut Connection) -> Result<(), OpenError> {
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let application_id: i32 =
        transaction.pragma_query_value(None, "application_id", |row| row.get(0))?;
    let version: i32 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;

    if application_id != APPLICATION_ID {
        let has_schema: bool =
            transaction.query_row("SELECT EXISTS (SELECT 1 FROM sqlite_schema)", [], |row| {
                row.get(0)
            })?;
        if application_id != 0 || version != 0 || has_schema {
            return Err(OpenError::ForeignDatabase);
        }
        transaction.pragma_update(None, "application_id", APPLICATION_ID)?;
    }

    let applied = usize::try_from(version).map_err(|_| OpenError::UnsupportedSchema(version))?;
    let pending = MIGRATIONS
        .get(applied..)
        .ok_or(OpenError::UnsupportedSchema(version))?;

    for (offset, migration) in pending.iter().enumerate() {
        transaction.execute_batch(migration)?;
        transaction.pragma_update(None, "user_version", applied + offset + 1)?;
    }
    transaction.commit()?;
    Ok(())
}
