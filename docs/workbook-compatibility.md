# Excel workbook compatibility

The local source fixture is `Projektzeiterfassung - Template.xlsx`. It contains sample content and original workbook metadata. Its structure is the compatibility reference.

## Observed mapping

| Cell or range | Meaning and handling |
| --- | --- |
| B6:B9 and D6:D9 | Labels and values for project start, project end, designation, and customer. Values already exist on both sheets, including `blank`. |
| H6 and following rows | H6 is the merged project section header. `25 - Dez` uses project labels at H7:H9, and `blank` starts with H7 and L7. The slot layout and formulas differ. |
| B13, C13, D13, F13 | Entry labels for weekday, date, activity, and project. |
| G14, I14, J14, K14, M14 | Start, end, gross hours, break, and actual net hours. |
| Rows 15–80 | 66 entry slots. Write date and times as numeric Excel values. D, G, K and M are merged-range anchors. |
| B15:B80, J15:J80, M15:M80 | Existing weekday, gross, and net formulas. B is driven by C. J subtracts G from I. M subtracts K from J. Both duration formulas return blank when F is empty. |
| Row 82 onward | Existing totals and project summaries. Several F84:F87 formulas stop at row 42 and some share formulas divide by the total, which may be zero. |

The workbook uses Excel's 1900 date system. The compatibility export copies the selected file, makes `blank` the active worksheet, and writes the selected period to D6:D7. Other worksheets and package contents are kept in the copy, including existing sample worksheets and metadata.

## Implemented export

- A standard export produces a new `Zeiten` sheet with numeric dates, local timestamps, separate work and break durations, timezone, and booking status. It supports filtered day, week, and month views.
- A compatibility export requires the original blank layout and 1900 date system. It fills up to 66 rows and uses the two project slots covered by the supplied full-range summary formulas. Unassigned work uses its own slot. A break uses a distinct label and puts its full duration in K, yielding zero net work in M. The copy updates formula results and requests automatic recalculation.
- If the selected records exceed those limits, contain open or undescribed work, or cross an offset change that the template subtraction cannot represent, the app reports the reason and the standard export remains available. A row reaching local midnight uses Excel time serial 1 for its end when the elapsed duration matches.
- The copy preserves cell styles, merged ranges, and the existing timeline formulas. The two percentage cells use zero when the total is zero. Only the selected sheet and the workbook settings are rewritten in the ZIP package.

The supplied file was used in an end-to-end export, then reopened with openpyxl to compare cells, formula caches, styles, merges, and untouched package parts. Native Excel recalculation and print output still require verification.
