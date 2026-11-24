# Projektzeit pre-release audit

- Audit date · 24 November 2026
- Baseline · 27f862b
- Audited code · 1818c82 on pre-release-audit

## Executive summary

The desktop windows share one SQLite timeline and one Rust controller. The audit confirmed defects in clock recovery, midnight accounting, legacy migration, concurrent desktop access, file publication, the bundled SQLite version, and the accessibility of Slint attribution. The audit branch contains fixes and targeted regressions.

**Release decision · hold public distribution.** The candidate is ready for interactive Windows acceptance testing. Release still needs that test, particularly standby, focus, mixed-DPI placement, and autostart. The owner also needs to select the Projektzeit distribution license and confirm the notices.

## Findings

No Critical findings were confirmed. High findings affect data integrity, correct balances, recovery, dependency security, or release compliance. Reproductions refer to the baseline unless noted.

| ID | Severity | Affected files | Reproduction or evidence | Result and recommendation |
| --- | --- | --- | --- | --- |
| H1 | High | app/Cargo.toml, crates/storage/Cargo.toml, Cargo.lock | The locked libsqlite3-sys 0.35.0 matched RUSTSEC-2026-0459 for bundled SQLite. The advisory applies when opening an untrusted database and WAL together. | **Fixed.** Updated both consumers to rusqlite 0.41.1 with libsqlite3-sys 0.39.1 and SQLite 3.54.1. A new scan found zero matching vulnerabilities. [RustSec advisory](https://rustsec.org/advisories/RUSTSEC-2026-0459.html) |
| H2 | High | app/src/account_ui/tracking.rs, dashboard.rs, window/windows.rs, crates/storage/src/tracker/mod.rs, worktime_account/queries.rs | Checkpoint work at 08:30 and move wall time back to 08:10. Finishing at the checkpoint was rejected. Confirmed time could disappear from the view, and short forward jumps could pass the former gap check. | **Fixed.** Compare wall and monotonic readings, listen for native resume and clock-change notifications, and preserve the durable checkpoint. A rollback permits finishing at that point and rejects earlier ordinary transitions. Regression tests and a Linux crash/restart check passed. Test native Windows notification delivery before publishing. |
| H3 | High | crates/storage/src/worktime_account/queries.rs, tracker/mod.rs | Finish eight hours of Monday work at Tuesday 00:00. The old completion query treated Tuesday as finished and could charge its full target. An overnight manual entry could also offer a finalization action that found no eligible workday. | **Fixed.** Treat the recorded end as exclusive and use the same day-of-completion rule for manual eligibility and finalization. Tests preserve **+00:24** at midnight and verify a manual overnight booking. |
| H4 | High | crates/storage/migrations/0001_initial.sql, 0004_close_orphan_workdays.sql, tracker/mod.rs | A v3 database can hold an open workday with closed bookings and no open interval. The GUI shows an idle state but Start rejects the existing workday. | **Fixed.** Migration 0004 closes only an orphaned envelope at its last persisted boundary. It preserves its bookings and leaves that workday unfinalized. A legacy fixture verifies preservation and subsequent Start. |
| H5 | High | app/src/account_ui/data.rs, mod.rs, crates/storage/src/tracker/entries.rs | Open the same database twice. Keep an edit or delete form in one process and modify the same row in the other. A stale form can overwrite a later edit or operate on a reused SQLite row ID. | **Fixed for audited desktop instances.** A canonical sidecar lock prevents a second desktop owner and is released by the OS on process exit. The lock file stays on disk. Keep older builds and external writers away from the same live database. |
| H6 | High | app/src/account_ui/data.rs, export.rs, crates/storage/src/lib.rs | Interrupt a backup or XLSX export after the original code creates the final target and before writing completes. An empty backup can look usable and can initialize as an empty database when opened. | **Fixed.** Stage the complete file beside its destination, flush, then publish without overwriting. Fault injection verifies that the incomplete target stays absent. The restore test reopens a consistent snapshot from a live WAL source. |
| H7 | High | app/src/account_ui/view.rs | The Slint widget was below backup settings. The pinned Royalty-Free license requires an About screen accessible from the top-level menu or the alternate public badge route. | **Fixed in the application.** **Über Projektzeit** now opens a dedicated popup from the top bar. The Slint badge, version, and link were verified on Linux. The first-party distribution license remains a separate gate. [Slint Royalty-Free 2.0](https://github.com/slint-ui/slint/blob/v1.19.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md) |
| M1 | Medium | crates/storage/src/tracker/entries.rs | Start and finish within one stored millisecond, then edit the activity. The regular editor rejected the valid existing interval. | **Fixed.** The editor accepts an existing zero-length interval. New manual entries still require a positive duration. |
| M2 | Medium | app/src/account_ui/window/mod.rs, windows.rs, mod.rs | A move to a different DPI can emit move and resize notifications after placement. The controller could interpret a native adjustment as a custom position. The Win32 no-activate adapter treats style and subclass installation as best effort. | **Open runtime checks.** Verify saved coordinates, the work-area margin and foreground HWND on real monitors. Reproduce any failure before changing placement. |
| M3 | Medium | app/src/account_ui/mod.rs, tracking.rs, view.rs | Leave a weekly or monthly account snapshot on screen during a live interval. It refreshes on selection and commands, while daily clocks tick each second. Today's Slint table model is rebuilt on live pulses. | **Open.** Measure large histories before changing refresh scope. Finalized balances and committed changes are covered by regressions. |
| M4 | Medium | app/src/account_ui/template.rs, export.rs | A template copy retains unrelated sheets and metadata. Mapping XML has a read limit, while the ZIP copy buffers the output and preserves other workbook parts. | **Open.** Validate package byte budgets and embedded content if broadening support beyond the supplied template. Verify recalculation and print layout in Excel. |
| M5 | Medium | crates/storage/src/lib.rs, app/src/account_ui/data.rs, preferences.rs, mod.rs | On Windows, ACLs follow the selected directory. Position autosave ignores some failures and the autostart status checks registry-value presence. Moving an executable can leave a stale registration. | **Partially hardened.** New Unix database files use mode 0600, local paths are opened without SQLite URI interpretation, and exports use private staging. Verify Windows ACLs, an unavailable destination, and sign-in after moving or changing the app's configured paths. |
| L1 | Low | crates/core/src/lib.rs, worktime_account/mod.rs, crates/storage/src/tracker/mod.rs, app/src/account_ui/view.rs | The pure tracker and the older provisional balance API are mostly exercised by library tests. Desktop commands use the SQLite tracker and finalized report projection. The Slint macro contains multiple screens and both window definitions. | **Open.** Consolidate overlapping APIs when changing the underlying rules. The audit retains the existing architecture and layout. |
| L2 | Low | app/src/account_ui/input.rs, dashboard.rs, crates/core/src/worktime_account/attribution.rs | Individual rows truncate fractional seconds. Two subsecond rows can each display zero while their daily total displays one second. | **Documented.** A regression verifies that daily totals sum milliseconds before truncating. Both time export modes preserve fractional values in worksheet cells. |

## Fix commits

- c2de8fd · preserve confirmed time across clock changes, recovery, and midnight.
- c0e94a2 · patch SQLite, enforce desktop ownership of one database, stage file writes, repair legacy orphaned workdays, and add storage and accounting regressions.
- 1818c82 · expose Slint attribution at the top level and add the Windows release build to CI.

## Architecture and functional checks

- The core owns duration attribution, schedule resolution, and ledger arithmetic. Storage owns the durable timeline, settings, and immediate transactions. The Slint host translates input and projects state. The windows have independent visibility and positioning while sharing the session, selected project, and saved booking state. The controller owns transient forms and trust in the active clock.
- Start, switch, pause, resume, and finish share one timestamp per transition. The outgoing booking and its successor commit together. Unique partial indexes enforce one open workday and one open entry. Triggers reject nonempty overlaps and orphaned interval bounds. Project archival retains historical foreign keys. The same-day resume test preserves previous intervals.
- Finishing work persists a pending description with the booking. Dismissing the prompt leaves it recoverable. A template export requires complete work and the standard export labels unusual states. A test process exits with an uncommitted transition and without Rust destructors. Reopening the database retains the checkpoint and rolls back the partial write.
- UTC milliseconds measure elapsed time. Reports allocate it using each booking's stored timezone. The editor keeps an unchanged ambiguous DST endpoint and rejects a new ambiguous or nonexistent wall time. On a backward system clock change, already persisted endpoints remain visible.
- The mandatory regression uses eight net hours on Monday from 08:00 to 16:30 with a 30-minute break and a 07:36 target. It verifies **+00:24** in weekly, monthly, and accumulated balances on unfinished Tuesday. Correcting Monday to seven hours produces **−00:36**, including after SQLite reopen. Independent tests cover both signed adjustment directions, initial balances, rollover across calendar years, dated schedules, current and future days, and rounding.
- SQLite uses WAL, FULL synchronization, and foreign keys. Test fixtures cover v1 and v3 upgrades, repeated opens, foreign databases, and newer-schema rejection. Migration 0004 preserves booking rows. A rollback to an older application should use a pre-upgrade backup. Restore by closing the application and opening a completed snapshot as its database path.
- Handwritten unsafe code is restricted to Win32 and registry integration. The window subclass owns its callback and defers delivery to the Slint event loop. Generated Slint Rust has a separate scoped exemption. The reviewed expect calls depend on fixed array lengths, valid calendar dates, or a just-created component. The audit established no user-input panic in the exercised workflows.

## Performance

With no active session the one-second timer checks the clock in process and normally skips SQLite and view refresh. With active work it reads the session, today's bookings and applicable target, then publishes both projections. It checkpoints at intervals of at least 30 seconds during tracking and at transitions or clean shutdown. Backup, export, and larger reports run synchronously on the UI thread.

| Linux measurement | Observation |
| --- | --- |
| Baseline, idle, 15 seconds after warm-up, software rendering under Xvfb | 0.067% of one CPU core, approximately 54 MiB RSS, 3 threads, unchanged WAL metadata |
| Audited, idle, 15 seconds under the same Xvfb setup | 0.067% of one CPU core, approximately 54 MiB RSS, 3 threads, unchanged WAL metadata |
| Audited debug desktop build, active mini-timer, 35 seconds with no interaction | 1.798% of one CPU core, approximately 56 MiB RSS, 6 threads, 1 WAL modification |

The samples use disposable small databases and /proc CPU and memory counters. They are point measurements, not Windows release timings or a large-history benchmark. No performance optimization was applied without a measured bottleneck.

## Security and licensing

- cargo-audit 0.23.0 matched one initial vulnerability and eight maintenance advisories. The updated lockfile matched **zero known vulnerabilities** against RustSec database revision 1d328b6bb9f2f350bf65c1c81ab68781cb9e018a. Maintenance advisories remain for bincode, chrono, chrono-tz, pin-utils, ttf-parser, zbus_names, zvariant, and zvariant_derive. The lockfile includes optional and other-platform crates. Assess the actual feature graph when planning upgrades.
- cargo-deny 0.22.0 checked licenses for the all-features x86_64-pc-windows-msvc and x86_64-unknown-linux-gnu graphs under a temporary policy allowing the observed permissive options and the selected Slint Royalty-Free 2.0 expression. All third-party crates had an admissible option. The overall check failed because the three Projektzeit crates lack license declarations.
- Slint 1.19 offers Royalty-Free 2.0 as an alternative to GPL-3.0-only and Slint Software 3.0. Projektzeit integrates the toolkit as a desktop GUI, and the direct AboutSlint popup addresses attribution clause 2(a). Include the relevant notices and verify the popup on the distributed Windows executable. [Pinned license](https://github.com/slint-ui/slint/blob/v1.19.0/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md)
- Local paths and SQL bindings handle input and output. Exports write activity text as data and preserve existing destinations. A template-derived export also retains unrelated content from the selected source workbook.

## Verification and coverage gaps

| Check | Result |
| --- | --- |
| cargo fmt --all -- --check | Passed locally and checked in CI |
| cargo clippy --locked --workspace --no-default-features --all-targets -- -D warnings and the equivalent --all-features invocation | Both passed locally |
| cargo test --locked --workspace --no-default-features and the equivalent --all-features invocation | 37 and 52 local tests passed, respectively. The database permissions regression is Unix-only. |
| cargo build --release --locked -p projektzeit --features desktop-account | Passed on the native Windows GitHub Actions runner for code commit 1818c82. Windows formatting, Clippy, and all-features tests passed in the same run. [CI run](https://github.com/stgeipel/Projektzeit/actions/runs/45418745133) |
| Linux cross-target cargo check, x86_64-pc-windows-msvc | Passed, covering Windows-specific Rust types and APIs without native linking |
| Linux desktop | The Slint popup appeared from the top bar. Minimizing opened the mini-timer and Start worked there. After a forced kill, reopening displayed the same persisted two minutes with the undecided tail excluded. Finishing at the checkpoint and dismissing the description prompt kept the booking pending. |
| Interactive Windows | Requires a Windows desktop. CI builds and tests do not exercise real monitor, standby, registry sign-in, or focus behavior. |

Coverage gaps include Windows Task Manager termination and actual power events, mixed-DPI and disconnected displays, live timezone changes, low-disk checkpoint behavior, large-history profiling, and native Excel recalculation and print output. The app uses the timezone selected at startup for current views and retains each booking's original attribution zone.

## Release gates

1. Run the audited Windows release with disposable data in a path with spaces and Unicode. Check the About popup, and confirm the first-party license and bundled notices before public distribution.
2. Start in the mini-timer, switch project, dismiss a description prompt, pause, resume, and finish. Confirm exact boundaries and a single session. Verify pending descriptions, manual work and break edits, deletion confirmation, and archived-project references after reopening.
3. Minimize and restore from two monitors on different DPI settings. Check automatic visibility, topmost, saved position and reset, the 24-logical-pixel work-area margin, keyboard access, and foreground focus. Repeat after disconnecting a monitor.
4. Suspend for less than two minutes and then for longer while tracking. Exercise a forward and backward system clock change in the test environment. Force a process termination and confirm the durable checkpoint and explicit treatment of uncertain time.
5. Verify Windows autostart at sign-in, intended path and ACLs, and backup restoration of entries and settings. Verify native Excel recalculation and print output before advertising template fidelity.
