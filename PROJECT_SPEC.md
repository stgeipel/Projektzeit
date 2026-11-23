# Projektzeit – Product and implementation specification

## Goal
Build a lightweight, offline-first Windows desktop time tracker using Rust, with a main application and a small floating always-on-top workday widget. UI language: German. No Pomodoro or focus timer. No reusable text templates. The widget displays net work time today, remaining time against configurable daily target, predicted finish time, break duration, and current project/segment elapsed time.

## Reference workbook (user-supplied)
`Projektzeiterfassung - Template.xlsx` has worksheets `25 - Dez` and `blank`. Relevant cells/columns: B6/D6 project start, B7/D7 project end, B8/D8 project designation, B9/D9 customer, H6+ project names. Entry table headers: B13 day, C13 date, D13 activity, F13 project, G14 start, I14 end, J14 hours, K14 break, M14 actual net working time. J and M contain formulas. The template's sheets are formatted and pre-populated; preserve its layout and formulas when implementing template-based export, and validate date/year assumptions instead of overwriting silently. Standard XLSX export is also required.

## User workflows
1. Start workday: create an active workday and select a project. Record wall-clock start timestamp immediately. Running segment starts. A workday has one active segment at most.
2. Switch project: on click, atomically record boundary timestamp and close the old segment, immediately start the next segment at the same timestamp, and show a mandatory activity-description dialog for the completed segment. Project selection for the next segment can be provided in the dialog; until chosen, time is tracked as unassigned work, never silently assigned to the wrong project. Dialog completion must not shift the boundary. Description is required (trimmed nonempty), and can be edited later. If dismissed, store a pending-description segment; do not lose work time or block continuing work. A new switch can happen even if a previous description is pending.
3. Start break: record break start immediately, close current work segment at that timestamp, prompt for mandatory description of closed segment; no work time accrues during break. Resume records break end and starts a new work segment for the chosen or previous project. Multiple breaks per day supported.
4. Finish day: immediately record end time, close work segment, show mandatory description for the final segment and daily summary. Pending descriptions are visibly flagged and must be resolved before final template export, or the export must explicitly warn and require user action; never silently omit them.
5. Edit/add entries: correct project, description, start/end, break, and add historical work. Recalculate aggregates without overlapping segments. Warn on conflicts.
6. Crash/restart: persist all transition events immediately in SQLite transactions. On restart, detect unfinished day/segment and ask whether to resume, correct, or end at an earlier time; never blindly count overnight, sleep or locked time as work. Detect system suspend/resume and optional idle time, ask user to classify time as work/break/correction. Do not auto-discard time.

## Time semantics
- Distinguish workday elapsed wall-clock time, net worked time, project time, break time, and unassigned work time.
- Net worked = sum of non-break work segments (including unassigned); project totals sum assigned segments. Break time is excluded from net worked.
- Remaining work = max(0, daily target - net worked). Configurable target by weekday, optionally disabled.
- Predicted finish = current time + remaining net work + additional planned/required break duration; label as estimate and explain assumptions. Do not assume that elapsed time equals net work.
- Show durations in hh:mm or hh:mm:ss, dates/times in local German format. Store UTC timestamps and IANA/local timezone context where necessary; handle DST and midnight transitions deliberately.
- Use timestamps rather than accumulating seconds on a UI tick; UI refresh never writes to DB every second.

## Main UI
- German language. Calm green/cream visual identity, accessible contrast, light/dark/system themes, responsive layouts.
- Navigation: Heute, Zeiten, Projekte, Berichte, Einstellungen.
- Heute: net worked today, remaining, break, estimated finish, current project/segment timer, switch/break/finish actions, daily entries and pending descriptions.
- Zeiten: day/week/month filters; add/edit/delete with confirmation, overlaps validation, duration and project totals.
- Projekte: CRUD, name, abbreviation, color, optional customer, archive rather than delete projects with entries, recent-project picker.
- Berichte: time by project and period, net work/break/unassigned totals, XLSX export (standard and workbook-template compatible), CSV optional.
- Einstellungen: weekday targets, planned breaks, start with Windows (off), show widget on minimize (on), widget always-on-top (on), widget size compact/expanded, movable and remembered position, minimize to tray, close-to-tray behavior, local DB location, theme, idle/suspend handling, daily backups and retention, optional shortcuts. No focus reminders or Pomodoro settings.

## Mini widget
- Independent lightweight window; configurable always-on-top, position, compact/expanded, pinning, drag, display after minimize.
- Main figures: net worked today, remaining today, pause total, predicted finish; current project and current segment elapsed time as secondary.
- Actions: project switch, break/resume, finish workday, open main app. Explicit labels/tooltips; no ambiguous stop icon.
- During break show pause state and resume. During no workday show Start.
- UI must not freeze while a description dialog is open; workday state and persistence continue correctly.

## Activity descriptions
- Required for every completed work segment. Prompt after switch, break, finish (not for a break itself).
- A description may be entered during an active segment and edited at completion. No saved templates or template checkbox. Optionally recent text suggestions without persisting explicit templates; omit in MVP if distracting.
- Pending descriptions visible in Heute/Zeiten and export validation.

## Data model (conceptual)
- projects(id, name, short_name, color, customer, archived_at, created_at)
- workdays(id, local_date, started_at_utc, ended_at_utc nullable, target_seconds_snapshot, status)
- work_segments(id, workday_id, project_id nullable, started_at_utc, ended_at_utc nullable, activity_text nullable, status [active, pending_description, complete], created_at, updated_at)
- breaks(id, workday_id, started_at_utc, ended_at_utc nullable)
- settings (key/value or typed config)
- transition audit/history optional for corrections.
Use constraints/indexes and transactional state changes; ensure at most one active workday/segment and no overlapping work/break intervals. Allow cross-midnight entries while reporting by local calendar date with explicit split rules.

## Technical direction
- Rust workspace with separated domain/core, persistence, platform integration, UI, export, tests.
- UI: Slint preferred, but validate licensing for public repo and Windows windowing/system tray/always-on-top feasibility before locking in. If Slint proves unsuitable, propose alternative (e.g. Tauri) with reasons; do not silently switch stack.
- SQLite via rusqlite; WAL, migrations, transactions, durable recovery.
- XLSX: rust_xlsxwriter for fresh exports; editing an existing XLSX template while preserving styles/formulas may require a different strategy/library. Prototype template fidelity early and report limitations.
- Windows APIs via windows-rs or compatible crates for tray, startup, window state, idle/suspend notifications. No cloud, account, backend, telemetry or subscription.
- Automated tests: time calculation, project switch timestamp, break/resume, finish, pending description, crash recovery, daylight-saving and midnight boundaries, overlapping edit validation, Excel output and sample workbook mapping.

## Implementation plan (incremental)
0. Inspect requirements and sample XLSX. Verify Slint license, multiwindow, tray, always-on-top, Excel template feasibility. Write ADRs and risk list; ask only truly blocking questions.
1. Scaffold Rust workspace, formatting, clippy, CI, SQLite schema/migrations, domain tests.
2. Implement transition engine and recovery, integration tests, local persistence.
3. Implement main UI Heute and widget, wire live state and window behavior.
4. Implement Zeiten/Projekte, editing, validation, settings, backups.
5. Implement reports, standard XLSX and template export with golden-file tests.
6. Polish Windows packaging, tray, startup, idle/suspend, keyboard accessibility, release instructions.

## Acceptance criteria
- Switching projects never stops net work clock or creates an unaccounted gap; boundary is captured on click.
- Pause immediately stops net work accumulation; resume starts it.
- Finish immediately stops the workday.
- Every completed work segment requires activity text to reach complete status, but an unfinished dialog never destroys time or stalls the next segment.
- Widget accurately displays worked/remaining/break/finish estimate and can stay on top.
- Crash/restart does not lose committed transitions or silently add overnight work.
- Data stored locally and reliably; export matches source column meanings and preserves template formulas where promised.
- No Pomodoro, no focus ring, no saved text templates.

## Instructions for Codex
First produce a short implementation plan, technical risk assessment, and proposed repository structure. Then implement only Phase 0/1 in a first small, reviewable commit. Do not implement the whole app at once. Keep documentation in English, user-visible UI in German. Add tests with every domain change. Avoid unrequested features.
