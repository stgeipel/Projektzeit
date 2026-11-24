use std::{env, path::PathBuf, process::ExitCode};

use projektzeit_storage::OpenError;

#[cfg(feature = "desktop-account")]
mod account_ui;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    let flag = args.next();
    let path = args.next();
    let extra = args.next();
    if flag.as_deref().is_some_and(|flag| flag == "--check-db")
        && extra.is_none()
        && let Some(ref path) = path
    {
        return match projektzeit_storage::open(PathBuf::from(path)) {
            Ok(_) => {
                println!("SQLite-Datenbank ist bereit.");
                ExitCode::SUCCESS
            }
            Err(error) => {
                match error {
                    OpenError::Sqlite(error) => eprintln!("SQLite-Fehler: {error}"),
                    OpenError::File(error) => eprintln!("Dateifehler: {error}"),
                    OpenError::InvalidPath => {
                        eprintln!("Bitte einen lokalen Datenbankpfad angeben.")
                    }
                    OpenError::ForeignDatabase => {
                        eprintln!("Die Datei ist keine Projektzeit-Datenbank.");
                    }
                    OpenError::UnsupportedSchema(version) => {
                        eprintln!("Die Datenbankversion {version} wird nicht unterstützt.");
                    }
                    OpenError::WalUnavailable(mode) => {
                        eprintln!("Der SQLite-WAL-Modus ist nicht verfügbar ({mode}).");
                    }
                }
                ExitCode::FAILURE
            }
        };
    }

    #[cfg(feature = "desktop-account")]
    if flag.as_deref().is_some_and(|flag| flag == "--account-ui")
        && args.next().is_none()
        && let Some(path) = path
    {
        return match account_ui::run(PathBuf::from(path), extra.as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::FAILURE
            }
        };
    }
    eprintln!("Aufruf: projektzeit --check-db <Datenbankpfad>");
    #[cfg(feature = "desktop-account")]
    eprintln!("       projektzeit --account-ui <Datenbankpfad> [Zeitzone]");
    ExitCode::from(2)
}
