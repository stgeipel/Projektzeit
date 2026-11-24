# Implementation plan and architecture decisions

## Repository structure

| Location | Responsibility |
| --- | --- |
| `crates/core` | UTC intervals, work status, schedule resolution, and balance calculations |
| `crates/storage` | SQLite migrations, transactional tracker commands, and account reports |
| `app/src/account_ui/tracking.rs` | Shared desktop controller for live tracking, history, and projects |
| `app/src/account_ui/view.rs` | The main Slint window and mini-timer |
| `app/src/account_ui/window` and `data.rs` | Window placement and lifecycle, Windows autostart, and SQLite snapshots |
| `app/src/account_ui/export.rs` and `template.rs` | XLSX reports and compatibility export |

Tests live with their owning crates. Both windows read snapshots from one Rust controller and send commands to the same SQLite connection. SQLite queries, platform calls, and exports live outside the domain crate.

## ADR 001 · Slint for the Windows prototype

Slint 1.19 is a viable candidate. Its Winit backend supports Windows. Separate `Window` components have independent visibility, and the window APIs expose position, minimize, and close handling. Bind the mini-timer's `always-on-top` to the user setting. Use one UI event loop and a shared Rust state model because Slint globals belong to each component instance. Start with Winit and the software renderer and measure memory and CPU on the target hardware. [Windows support](https://docs.slint.dev/latest/docs/slint/guide/platforms/desktop/windows/general/), [Slint Window](https://docs.slint.dev/latest/docs/slint/reference/window/window/), [Rust window API](https://docs.slint.dev/latest/docs/rust/slint/struct.Window), [Slint globals](https://docs.slint.dev/latest/docs/slint/reference/language/globals/)

Slint 1.19 documents a `SystemTrayIcon` component with native Windows tray behavior and an event loop that stays alive while the icon is visible. Try it before adding a separate tray crate. Keep native code behind the platform boundary. Use the window handle for any needed Win32 adjustments, such as changing topmost state with `SetWindowPos` if the binding is insufficient. For the platform prototype, exercise suspend and resume via `WM_POWERBROADCAST`, lock and unlock via WTS session notifications, and optional idle detection via `GetLastInputInfo`. [Slint tray](https://docs.slint.dev/latest/docs/slint/reference/window/systemtrayicon/), [SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos), [Power broadcasts](https://learn.microsoft.com/en-us/windows/win32/power/wm-powerbroadcast), [Session notifications](https://learn.microsoft.com/en-us/windows/win32/api/wtsapi32/nf-wtsapi32-wtsregistersessionnotification), [Idle input](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getlastinputinfo)

**Compile probe.** A temporary Slint 1.19.0 project passed `cargo check --target x86_64-pc-windows-msvc` from Linux. It exercised the API for two independently shown windows, a callback-driven topmost binding, physical positioning, raw Win32 handle access, and a native tray menu. It used `backend-winit`, `renderer-software`, `system-tray`, `raw-window-handle-06`, `std`, and `compat-1-2` with default features disabled. The check verified compilation through type checking. Linking and desktop behavior remain Windows checks.

**Licensing.** Slint offers GPLv3 and a royalty-free desktop license, among its options. GPLv3 is suitable if the distributed application will use GPLv3. For a different application license, the royalty-free desktop option permits distribution with the required attribution, which can be the Slint widget in an accessible About dialog or the official badge on a public webpage. Prefer the About dialog for a royalty-free distribution. Confirm the owner’s source license, the terms shipped with the pinned Slint release, and the attribution path before public distribution. The desktop uses Slint 1.19 and presents AboutSlint from the top-level About command. A license for Projektzeit itself still needs the owner’s selection. [Slint licensing terms](https://slint.dev/terms-and-conditions), [Royalty-free license v2](https://slint.dev/agreements/slint-royalty-free-license.pdf)

**Windows acceptance gate.** Test live pin/unpin, minimize-to-widget, hide and restore from tray, taskbar behavior, focus and keyboard access, and dragging and restoring after a monitor or DPI change. Verify suspend/lock handling even while the main window is hidden. The native Windows build runs in CI. Pinning, monitor placement, mixed DPI, and Windows session behavior need an interactive Windows environment. If a native adapter cannot close a specific gap, evaluate Tauri with a minimal two-window comparison before changing frameworks.

## ADR 002 · UTC timeline and transactional SQLite

- Persist signed Unix milliseconds in SQLite. Capture the action timestamp at the UI boundary, normalize once to milliseconds, and use that same value to end the current row and begin the next. All intervals are half-open. Identical timestamps can record a zero-duration event, including two actions within one millisecond.
- Store the IANA zone and start-local date on each workday, along with its optional target snapshot. Use UTC differences for elapsed time. A cross-midnight workday retains its start-local identity. For date-based reports, split intervals by local calendar boundaries in the recorded zone and sum UTC durations. An edit keeps an unchanged ambiguous endpoint and rejects new ambiguous or nonexistent local times.
- Use one typed timeline table for work and breaks. Partial unique indexes limit open workdays and open intervals. Triggers reject overlap across workdays and edits that strand entries outside their workday. Work status follows end time and trimmed description. An unassigned segment uses a nullable project ID and still contributes to net work.
- Use bundled SQLite, foreign keys, WAL, and `synchronous=FULL`. Migrations run under an immediate transaction and are tracked with `user_version` and an application ID. The tracker owns the write transaction and verifies that the target workday and segment still match when it executes. Emit the description prompt only after commit, keyed to the completed segment.
- Summaries accept an explicit observation time. The UI refreshes from committed timestamps without writing every second. Recovery detects an open workday at launch and after gaps in the controller timer. It asks the user how to classify time since the last checkpoint.

## Risks and planned checks

| Risk | Check or mitigation |
| --- | --- |
| Crash, suspend, lock, or backward clock adjustment can create uncertain time or invalid ordering | Reopen committed rows in tracker integration tests. Prompt for classification, keep the recorded boundaries available for correction, and test uncertain time before resuming live accumulation. |
| Switching before a prior description is entered can create misassignment or gaps | Commit the boundary first, open an unassigned segment if needed, and use segment IDs for independent prompts and late project selection. |
| Wall-clock timestamps at DST boundaries or after midnight can misstate local-day totals | Use the recorded zone to split reports. Require explicit resolution for ambiguous local edits. Keep exact UTC endpoints and test spring-forward, repeated hours, and cross-date periods. |
| WAL files and Windows file sharing affect backup and recovery | Create a consistent SQLite snapshot with VACUUM INTO and test reopening it while the source remains open. |
| Template dates, formulas, row capacity, and source metadata can change meaning or leak sample information | Preflight the specific layout and date range and exercise workbook fidelity in Excel. See the [workbook findings](workbook-compatibility.md). |
| Slint tray and native window behavior may differ from API-level expectations | Verify runtime behavior on Windows with the acceptance gate above. |
