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

The workbook uses Excel's 1900 date system. `25 - Dez` contains July 1, 2026 in C15:C17. D6:D7 hold November 1, 2025 to December 31, 2026 on that sheet and November 1, 2025 to March 31, 2026 on `blank`. Validate the target period, metadata, and project mapping explicitly before writing a new copy.

## Phase 0 fidelity check

A temporary Python prototype updated C15, D15, F15, G15, I15, and K15 in the `blank` worksheet XML in a copy. It used numeric date and time serials and escaped inline text containing an umlaut and an ampersand. Reloading the result with openpyxl confirmed the supplied values, and comparison confirmed all original formula text, merge ranges, and style IDs. The other 13 OOXML package parts matched byte for byte after decompression. The original workbook was unchanged. Excel formula recalculation and print fidelity still require a native Excel test.

## Export approach

- Generate the standard workbook with `rust_xlsxwriter`. Use an OOXML package patch for the compatibility route, with a ZIP library and XML-aware cell updates anchored to a validated layout. Retain formulas, styles, merges, and unrelated parts. Refresh stale formula caches and verify Excel recalculates correctly. [rust_xlsxwriter capabilities](https://rustxlsxwriter.github.io/index.html)
- Preflight the date system, period, existing content, project slots, row capacity, and formula coverage. Warn before generating a compatibility workbook with source formula limitations. Offer the standard export when the selection cannot fit without misrepresenting totals. Keep the source workbook separate from any generated report.
- Design and test explicit representations for unassigned work and breaks. In particular, a blank F cell suppresses both duration formulas. A row that crosses midnight or a DST offset change may produce a different duration under `I-G` than under UTC subtraction. Verify exact supported representations and use the standard export when the template cannot express the interval accurately.
- Require the user to resolve pending activity descriptions or explicitly acknowledge them before final template export. Include every accepted row and preserve seconds in the underlying numeric values when the template representation supports them.
- Use a cleared or synthetic fixture for any checked-in golden tests. The supplied workbook contains source metadata that should stay local. Check the copied workbook's data, formulas, layout, and recalculated totals in Excel before shipping compatibility export.
