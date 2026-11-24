use std::{cell::RefCell, fs, path::Path, rc::Rc};

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

fn backup(db: &Connection, path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Bitte einen Zielpfad für die Datensicherung eingeben.".into());
    }
    // An absolute filesystem path also rules out SQLite special names such as :memory:.
    let path =
        std::path::absolute(path).map_err(|error| format!("Ungültiger Zielpfad ({error})."))?;
    let destination = path
        .to_str()
        .ok_or_else(|| "Ungültiger Zielpfad.".to_owned())?;
    // Reserve the destination atomically so SQLite cannot replace an existing file.
    fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| format!("Sicherungsdatei konnte nicht erstellt werden ({error})."))?;
    if let Err(error) = db.execute("VACUUM INTO ?1", [destination]) {
        let _ = fs::remove_file(&path);
        return Err(format!("Datensicherung fehlgeschlagen ({error})."));
    }
    if let Err(error) = fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .and_then(|file| file.sync_all())
    {
        let _ = fs::remove_file(&path);
        return Err(format!(
            "Datensicherung konnte nicht gespeichert werden ({error})."
        ));
    }
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
    use std::{io, os::windows::ffi::OsStrExt, ptr};

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
}
