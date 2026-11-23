use std::{env, path::PathBuf, process::ExitCode};

use projektzeit_storage::OpenError;

fn main() -> ExitCode {
    let mut args = env::args_os().skip(1);
    if let (Some(flag), Some(path), None) = (args.next(), args.next(), args.next())
        && flag == "--check-db"
    {
        return match projektzeit_storage::open(PathBuf::from(path)) {
            Ok(_) => {
                println!("SQLite-Datenbank ist bereit.");
                ExitCode::SUCCESS
            }
            Err(error) => {
                match error {
                    OpenError::Sqlite(error) => eprintln!("SQLite-Fehler: {error}"),
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

    eprintln!("Aufruf: projektzeit --check-db <Datenbankpfad>");
    ExitCode::from(2)
}
