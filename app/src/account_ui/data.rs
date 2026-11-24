use std::{cell::RefCell, fs, io, path::Path, rc::Rc};

use chrono_tz::Tz;
use rusqlite::Connection;
use slint::SharedString;

use super::{Controller, action, view::AccountWindow};

pub(super) fn initialize(window: &AccountWindow) {
    window.set_autostart_supported(cfg!(windows));
    match native::enabled() {
        Ok(enabled) => window.set_autostart(enabled),
        Err(error) => {
            window.set_feedback(format!("Autostart konnte nicht gelesen werden ({error}).").into())
        }
    }
}

pub(super) fn connect(window: &AccountWindow, controller: &Rc<RefCell<Controller>>) {
    window.on_change_autostart(action(window, controller, |state, window, enabled| {
        let result = native::set(enabled, &state.database_path, state.zone);
        window.set_autostart(native::enabled().unwrap_or(!enabled));
        result.map_err(|error| format!("Autostart konnte nicht geändert werden ({error})."))
    }));
    window.on_backup_to(action(
        window,
        controller,
        |state, window, path: SharedString| {
            backup(&state.connection, Path::new(path.trim()))?;
            window.set_feedback("SQLite-Datensicherung gespeichert.".into());
            Ok(())
        },
    ));
}

pub(super) fn lock_database(path: &Path) -> Result<fs::File, String> {
    let result = (|| -> io::Result<_> {
        let path = fs::canonicalize(path)?;
        let mut lock_name = path.into_os_string();
        lock_name.push(".projektzeit.lock");
        fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_name)
    })();
    let file = result
        .map_err(|error| format!("Datenbanksperre konnte nicht geöffnet werden ({error})."))?;
    file.try_lock().map_err(|error| match error {
        std::fs::TryLockError::WouldBlock => {
            "Diese Datenbank ist bereits in einem Projektzeit-Fenster geöffnet.".into()
        }
        std::fs::TryLockError::Error(error) => {
            format!("Datenbank konnte nicht gesperrt werden ({error}).")
        }
    })?;
    // Retain the sidecar itself. Deleting it could let another process lock a
    // different file while a prospective opener still holds this handle.
    Ok(file)
}

fn backup(db: &Connection, path: &Path) -> Result<(), String> {
    publish_file(path, |temporary| {
        let destination = temporary
            .path()
            .to_str()
            .ok_or_else(|| "Ungültiger Pfad für die Datensicherung.".to_owned())?;
        db.execute("VACUUM INTO ?1", [destination])
            .map_err(|error| format!("Datensicherung fehlgeschlagen ({error})."))?;
        Ok(())
    })
}

/// Write and flush a private file beside the destination before publishing it.
pub(super) fn publish_file(
    path: &Path,
    write: impl FnOnce(&mut tempfile::NamedTempFile) -> Result<(), String>,
) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Bitte einen Zielpfad eingeben.".into());
    }
    let path =
        std::path::absolute(path).map_err(|error| format!("Ungültiger Zielpfad ({error})."))?;
    let parent = path
        .parent()
        .ok_or_else(|| "Ungültiger Zielpfad.".to_owned())?;
    let mut file = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| format!("Temporäre Datei konnte nicht erstellt werden ({error})."))?;
    write(&mut file)?;
    file.as_file()
        .sync_all()
        .map_err(|error| format!("Datei konnte nicht gespeichert werden ({error})."))?;
    file.persist_noclobber(path).map_err(|failure| {
        if failure.error.kind() == io::ErrorKind::AlreadyExists {
            "Die Datei existiert bereits. Bitte einen neuen Dateinamen wählen.".into()
        } else {
            format!("Datei konnte nicht gespeichert werden ({}).", failure.error)
        }
    })?;
    Ok(())
}

#[cfg(not(windows))]
mod native {
    use super::*;
    pub fn enabled() -> std::io::Result<bool> {
        Ok(false)
    }
    pub fn set(_: bool, _: &Path, _: Tz) -> std::io::Result<()> {
        Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
    }
}

#[cfg(windows)]
#[allow(unsafe_code)]
mod native {
    use std::{os::windows::ffi::OsStrExt, ptr};

    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW,
            RegSetKeyValueW,
        },
    };

    use super::*;

    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "Projektzeit";

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain([0]).collect()
    }

    pub fn enabled() -> io::Result<bool> {
        let key = wide(RUN_KEY);
        let name = wide(NAME);
        let mut length = 0;
        // The null buffer queries the size of an existing string value.
        let status = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ,
                ptr::null_mut(),
                ptr::null_mut(),
                &mut length,
            )
        };
        match status {
            ERROR_SUCCESS => Ok(true),
            ERROR_FILE_NOT_FOUND => Ok(false),
            other => Err(io::Error::from_raw_os_error(other as i32)),
        }
    }

    pub fn set(enabled: bool, database: &Path, zone: Tz) -> io::Result<()> {
        let key = wide(RUN_KEY);
        let name = wide(NAME);
        let status = if enabled {
            let executable = std::env::current_exe()?;
            let database = std::path::absolute(database)?;
            let mut command = vec![u16::from(b'"')];
            command.extend(executable.as_os_str().encode_wide());
            command.extend("\" --account-ui \"".encode_utf16());
            command.extend(database.as_os_str().encode_wide());
            command.extend("\" ".encode_utf16());
            command.extend(zone.name().encode_utf16());
            command.push(0);
            if command.len() > 260 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Programmpfad und Datenbankpfad sind zusammen zu lang.",
                ));
            }
            // The registry copies the null-terminated UTF-16 command line.
            unsafe {
                RegSetKeyValueW(
                    HKEY_CURRENT_USER,
                    key.as_ptr(),
                    name.as_ptr(),
                    REG_SZ,
                    command.as_ptr().cast(),
                    (command.len() * 2) as u32,
                )
            }
        } else {
            // Both UTF-16 names remain alive through the call.
            unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, key.as_ptr(), name.as_ptr()) }
        };
        if status == ERROR_SUCCESS || (!enabled && status == ERROR_FILE_NOT_FOUND) {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(status as i32))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backup_contains_committed_settings_and_protects_existing_files() {
        let folder = tempfile::tempdir().unwrap();
        let original_path = folder.path().join("data.sqlite");
        let backup_path = folder.path().join("backup.sqlite");
        let db = projektzeit_storage::open(&original_path).unwrap();
        db.execute("INSERT INTO settings VALUES ('backup-test', 'before')", [])
            .unwrap();
        backup(&db, &backup_path).unwrap();
        db.execute(
            "UPDATE settings SET value='after' WHERE key='backup-test'",
            [],
        )
        .unwrap();
        assert!(backup(&db, &backup_path).is_err());
        let restored = projektzeit_storage::open(&backup_path).unwrap();
        let value: String = restored
            .query_row(
                "SELECT value FROM settings WHERE key='backup-test'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(value, "before");
    }

    #[test]
    fn interrupted_write_keeps_failed_file_out_of_the_destination() {
        use std::io::Write;
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("backup.sqlite");
        let attempt = publish_file(&path, |temp| {
            temp.write_all(b"incomplete").unwrap();
            Err("interrupted".into())
        });
        assert!(attempt.is_err());
        assert!(!path.exists());
        assert_eq!(fs::read_dir(folder.path()).unwrap().count(), 0);
    }

    #[test]
    fn opening_same_database_twice_cannot_create_conflicting_desktop_sessions() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("data.sqlite");
        let _db = projektzeit_storage::open(&path).unwrap();
        let lock = lock_database(&path).unwrap();
        assert!(lock_database(&folder.path().join(".").join("data.sqlite")).is_err());
        drop(lock);
        assert!(lock_database(&path).is_ok());
    }
}
