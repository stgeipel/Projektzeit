# Worktime account

The standalone Slint window uses the same SQLite file and work timeline as the tracker foundation. It shows today's daily timeline and balance, daily and ISO-week or calendar-month summaries under Zeiten, and the monthly worktime account under Berichte. Projects lists the active and archived projects stored in the database. The main tracker and its floating timer can consume the same report service when they are introduced. This window configures and reports existing time records. Recording live sessions and confirming recovery intervals belong to the tracker.

## Configuration and correction

- Add a weekly schedule starting on its effective date. Enter Monday through Sunday allocations individually and check that they sum to the entered weekly total. A new effective date preserves the earlier schedule. To correct an existing schedule version, use its effective date and confirm the before/after preview.
- Set the opening balance to the balance already earned before its effective date. Enter signed durations such as `+24:30` or `-05:15`. Balances and adjustments before that date are excluded from subsequent calculations. Editing the opening supersedes the previous version and preserves the audit entry.
- Set or clear a target override for a particular date with a reason. An override can set the target to zero. Scheduled days use the configured allocation until explicitly overridden. There is no automatic holiday or absence policy.
- Add a signed manual adjustment with its effective date and reason. Use its number to edit or void it. Voiding removes its contribution at the original effective date and preserves the original entry, the void reason, and the timestamps in history. To offset an entry on a different date, add a new adjustment with the opposite sign.

Configuration changes and audit writes share one SQLite transaction. The preview compares balances as of the displayed date and the daily target on the change's effective date. If configuration or amounts at the preview cutoff change before confirmation, the user must review a fresh preview.

## Reports

Actual time sums the elapsed UTC time of closed work segments, including unassigned work. Breaks and project subtotals are excluded. Intervals are allocated to calendar dates in their workday's recorded timezone, splitting at local date changes, including DST. Milliseconds are summed for each date before discarding the fractional second. Open segments are treated as unclassified until the tracker supplies a trusted cutoff. If they can affect a displayed balance, the report marks it provisional.

The target sums the daily allocation in force on each date, with overrides applied. Dates after the report cutoff count toward the planned target. The current date's scheduled target is included in the provisional total through today. The weekly view follows ISO Monday–Sunday boundaries. The monthly opening is the previous month's closing, or the configured opening when the account starts during that month.

Example: an opening of +20:00 h, 160:00 h of actual work, and a 152:00 h target result in +28:00 h at month end. A -01:15 h adjustment reduces the closing to +26:45 h. Editing that adjustment to -00:45 h changes the closing to +27:15 h.

## XLSX

The time and report views export the displayed report snapshot to a new workbook with one sheet, `Arbeitszeitkonto`. Signed amounts are numeric minutes with fractional minutes for seconds, which Excel can sum regardless of its handling of negative time serials. An existing destination is preserved. The existing template file is kept separate, and the future project export can add this report as an optional sheet from the same service.

Schema migration 0002 adds the account tables and upgrades existing databases in place. Existing project, workday, timeline, and settings rows remain available.
