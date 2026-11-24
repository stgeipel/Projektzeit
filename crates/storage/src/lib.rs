//! SQLite connection setup and versioned schema migrations.

use std::{fmt, io, path::Path, time::Duration};

use rusqlite::{Connection, OpenFlags, TransactionBehavior};
pub mod tracker;
pub mod worktime_account;

const APPLICATION_ID: i32 = 0x505A4549; // PZEI
const MIGRATIONS: &[&str] = &[
    include_str!("../migrations/0001_initial.sql"),
    include_str!("../migrations/0002_worktime_account.sql"),
    include_str!("../migrations/0003_tracking.sql"),
    include_str!("../migrations/0004_close_orphan_workdays.sql"),
];

#[derive(Debug)]
pub enum OpenError {
    Sqlite(rusqlite::Error),
    File(io::Error),
    InvalidPath,
    ForeignDatabase,
    UnsupportedSchema(i32),
    WalUnavailable(String),
}

impl fmt::Display for OpenError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(error) => write!(formatter, "{error}"),
            Self::File(error) => write!(formatter, "{error}"),
            Self::InvalidPath => formatter.write_str("a local database path is required"),
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
            Self::File(error) => Some(error),
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
    let path = path.as_ref();
    if path.as_os_str().is_empty() || path.as_os_str() == ":memory:" {
        return Err(OpenError::InvalidPath);
    }
    let path = std::path::absolute(path).map_err(OpenError::File)?;
    #[cfg(unix)]
    create_private_if_missing(&path)?;
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_CREATE
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let mut connection = Connection::open_with_flags(path, flags)?;
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

#[cfg(unix)]
fn create_private_if_missing(path: &Path) -> Result<(), OpenError> {
    use std::os::unix::fs::OpenOptionsExt;

    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(OpenError::File(error)),
    }
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
        let next = i32::try_from(applied + offset + 1)
            .map_err(|_| OpenError::UnsupportedSchema(version))?;
        transaction.pragma_update(None, "user_version", next)?;
    }
    transaction.commit()?;
    Ok(())
}
