# Projektzeit

Projektzeit is a German-language Windows desktop application for offline workday tracking. It uses Rust, Slint, and SQLite. The [product specification](PROJECT_SPEC.md) describes the product scope.

## Build and run

On Windows, install [Rust via rustup](https://rustup.rs/) and the [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with the Desktop development with C++ workload and a Windows SDK. Use the MSVC Rust target. On Linux, install rustup and a C compiler toolchain (for example, `build-essential` on Ubuntu). The toolchain file selects Rust 1.95.0. Cargo downloads the toolchain and dependencies on first use. SQLite is bundled, so it does not require a separate installation.

From the repository root, run these commands in PowerShell or a Linux shell.

```sh
cargo build --locked --workspace
cargo run --locked -p projektzeit -- --check-db ./projektzeit-dev.sqlite
```

The command opens or creates the specified database, runs pending migrations, and prints `SQLite-Datenbank ist bereit.` on success. Choose a path on a local disk in an existing directory. Use a disposable development file when testing migrations. Once Cargo has cached dependencies, add `--offline` to build or run without network access.

## Desktop application

On Windows, start the desktop window with:

```sh
cargo run --locked -p projektzeit --features desktop-account -- --account-ui ./projektzeit-dev.sqlite Europe/Berlin
```

The timezone argument is optional when the system IANA zone can be detected. On Linux, the desktop feature additionally needs a graphical session and Fontconfig development files (for example, `libfontconfig-dev` on Ubuntu).

- **Heute** starts the workday, switches projects, records breaks, and ends or resumes the day. Choose a project or **Ohne Projekt**. After leaving a work segment, save its activity description or use **Später** to enter it later. The interval is saved at the moment of the transition. Open workdays can be restored after a restart.
- **Zeiten** shows daily, weekly, and monthly history. Add or correct work and breaks, filter by project, and export XLSX. **Tagesgrenzen** adjusts a closed workday and any first or last booking that touches the corresponding boundary. Provide the local Excel template path for a compatible copy or leave it blank for a standard workbook.
- **Arbeitszeitkonto** shows the weekly and monthly actual, due and planned totals, carryover, and accumulated balance. Define the opening balance and review manual balance adjustments here. [Calculation rules](docs/worktime-account.md)
- **Einstellungen** contains working hours, projects, mini-timer options, appearance, Windows autostart, backups, and the Slint attribution. A valid edit under **Aktuelle Arbeitszeit** saves automatically and takes effect today. Use **Datierte Arbeitszeitregeln** to schedule a future plan or correct a historical one.

The mini-timer shares the current session with the main window. It opens on minimize by default and the automatic instance closes when you restore the main window. On Windows the initial position uses the monitor's work area with a 24-logical-pixel right and bottom margin. **Fensterposition zurücksetzen** restores that default.

The backup command publishes a fully written SQLite snapshot at a new path. The desktop holds an OS lock in a sidecar file beside the database to prevent two application instances from editing it together. To work with the snapshot, close the app and start it with that path as the database argument. Windows autostart opens the same database and timezone used when enabling it.

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-features --all-targets -- -D warnings
cargo test --locked --workspace --all-features
```

The CI workflow runs these checks on Windows and Linux and builds the Windows desktop executable. See the [implementation plan and architecture decisions](docs/architecture.md) and the [workbook compatibility findings](docs/workbook-compatibility.md).

The supplied workbook is a local reference in the starter archive. Keep it beside `PROJECT_SPEC.md` for inspection. It contains source metadata and is excluded from version control.
