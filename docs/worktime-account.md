# Worktime account

**Arbeitszeitkonto** calculates weekly, monthly, and accumulated balances from the same SQLite records that **Heute** and **Zeiten** use. Adjusting an individual time booking changes actual work. An adjustment in the worktime account changes the accumulated balance on its effective date.

## Configuration and correction

- Add a weekly schedule starting on its effective date. Enter Monday through Sunday allocations individually and check that they sum to the entered weekly total. A new effective date preserves the earlier schedule. To correct an existing schedule version, use its effective date and confirm the before/after preview.
- Set the opening balance to the balance already earned before its effective date. Enter signed durations such as `+24:30` or `-05:15`. Balances and adjustments before that date are excluded from subsequent calculations. Editing the opening supersedes the previous version and preserves the audit entry.
- Set or clear a target override for a particular date with a reason. An override can set the target to zero. Scheduled days use the configured allocation until explicitly overridden. There is no automatic holiday or absence policy.
- Add a signed manual adjustment with its effective date and reason. Use its number to edit or void it. Voiding removes its contribution at the original effective date and preserves the original entry, the void reason, and the timestamps in history. To offset an entry on a different date, add a new adjustment with the opposite sign.

Configuration changes and audit writes share one SQLite transaction. The preview compares balances as of the displayed date and the daily target on the change's effective date. If configuration or amounts at the preview cutoff change before confirmation, the user must review a fresh preview.

## Reports

Actual time is elapsed work, including work without a project. Breaks are separate intervals on the same timeline. Intervals are attributed to calendar dates in the recorded timezone, including local midnight and DST transitions. Elapsed milliseconds are summed for each date before discarding the fractional second.

The due target and settled balance include past dates and explicitly completed workdays. The current unfinished day contributes to observed actual and daily progress. Its unfinished balance stays outside the settled total. A currently open workday stays unsettled across midnight. On restart the user classifies time since the last persisted checkpoint. Confirmed time through that checkpoint stays visible during recovery. The weekly view follows ISO Monday–Sunday boundaries. A month's carryover is the settled balance before the month, or the opening amount if the account starts during that month.

The full planned target includes future dates. Schedule resolution uses the plan in force on each date and applies any override. Manual balance adjustments contribute through the report cutoff, including adjustments dated today.

Example: an opening of +20:00 h, 160:00 h of actual work, and a 152:00 h target result in +28:00 h at month end. A -01:15 h adjustment reduces the closing to +26:45 h. Editing that adjustment to -00:45 h changes the closing to +27:15 h.

## XLSX

**Arbeitszeitkonto** exports a monthly account snapshot with signed numeric minutes. **Zeiten** exports the selected work and break entries with numeric dates and durations, or creates a copy of the supplied template. Existing destination files are preserved. [Workbook mapping and export limits](workbook-compatibility.md)

Migrations 0002 and 0003 add account configuration, project descriptions, workday checkpoints, and completion status. Migration 0004 closes legacy open workdays that have no ongoing booking at the last persisted boundary. Previously stored projects, workdays, timeline entries, and settings remain available.
