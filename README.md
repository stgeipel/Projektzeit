# Projektzeit

Projektzeit is an offline-first Windows workday tracker in development. The [product specification](PROJECT_SPEC.md) defines the finished application. The workspace includes the Rust time-tracking foundation and a standalone German Slint desktop window for the worktime account. Continuous tracking commands, the main tracker, the mini-timer, and project exports are planned separately.

## Build and run

On Windows, install [Rust via rustup](https://rustup.rs/) and the [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/) with the Desktop development with C++ workload and a Windows SDK. Use the MSVC Rust target. On Linux, install rustup and a C compiler toolchain (for example, `build-essential` on Ubuntu). The toolchain file selects Rust 1.95.0. Cargo downloads the toolchain and dependencies on first use. SQLite is bundled, so it does not require a separate installation.

From the repository root, run these commands in PowerShell or a Linux shell.

```sh
cargo build --locked --workspace
cargo run --locked -p projektzeit -- --check-db ./projektzeit-dev.sqlite
```

The command opens or creates the specified database, runs pending migrations, and prints `SQLite-Datenbank ist bereit.` on success. Choose a path on a local disk in an existing directory. Use a disposable development file when testing migrations. Once Cargo has cached dependencies, add `--offline` to build or run without network access.

## Worktime account

On Windows, start the optional desktop window with:

```sh
cargo run --locked -p projektzeit --features desktop-account -- --account-ui ./projektzeit-dev.sqlite Europe/Berlin
```

The timezone argument is optional when the system IANA zone can be detected. On Linux, the desktop feature additionally needs a graphical session and Fontconfig development files (for example, `libfontconfig-dev` on Ubuntu).

Open **Einstellungen**, enter a weekly schedule (the seven daily durations must add up to the weekly total), and set the opening balance and its effective date. Review each preview before saving. Use **Woche** or **Monat** to select a cutoff date or save an account-only XLSX to a new file. [Worktime-account rules and examples](docs/worktime-account.md)

## Checks

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
```

The CI workflow runs the checks with `--all-features` on Windows and Linux. Add that flag to Clippy and tests to include the optional desktop and workbook tests. See the [implementation plan and architecture decisions](docs/architecture.md) and the [workbook compatibility findings](docs/workbook-compatibility.md).

The supplied workbook is a local reference in the starter archive. Keep it beside `PROJECT_SPEC.md` for inspection. It contains source metadata and is excluded from version control.
