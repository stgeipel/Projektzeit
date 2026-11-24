use std::{io::Write, path::Path};

use chrono::{NaiveDate, Timelike};
use projektzeit_core::worktime_account::{Period, Seconds};
use projektzeit_storage::{tracker::Kind, worktime_account::Reports};
use rust_xlsxwriter::{Format, Workbook, XlsxError};

use super::{dashboard::Slice, input::display_date};

pub(super) fn save_account(path: &Path, reports: &Reports, as_of: NaiveDate) -> Result<(), String> {
    check_path(path)?;
    let mut workbook = workbook(reports, as_of).map_err(xlsx_error)?;
    let data = workbook.save_to_buffer().map_err(xlsx_error)?;
    write_new(path, &data)
}

fn check_path(path: &Path) -> Result<(), String> {
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xlsx"))
    {
        return Err("Bitte einen Dateipfad mit der Endung .xlsx eingeben.".into());
    }

    Ok(())
}

fn xlsx_error(error: XlsxError) -> String {
    format!("XLSX konnte nicht erstellt werden ({error}).")
}

fn write_new(path: &Path, data: &[u8]) -> Result<(), String> {
    super::data::publish_file(path, |file| {
        file.write_all(data)
            .map_err(|error| format!("XLSX konnte nicht gespeichert werden ({error})."))
    })
}

pub(super) fn save_times(
    path: &Path,
    template: &str,
    period: Period,
    rows: &[Slice],
) -> Result<(), String> {
    check_path(path)?;
    let data = if template.is_empty() {
        time_workbook(rows).map_err(xlsx_error)?
    } else {
        super::template::copy(Path::new(template), period, rows)?
    };
    write_new(path, &data)
}

fn time_workbook(rows: &[Slice]) -> Result<Vec<u8>, XlsxError> {
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    sheet.set_name("Zeiten")?;
    sheet.set_freeze_panes(1, 0)?;
    let header = Format::new().set_bold().set_background_color(0xDDF5EB);
    let date = Format::new().set_num_format("dd.mm.yyyy");
    let clock = Format::new().set_num_format("dd.mm.yyyy hh:mm:ss");
    let elapsed = Format::new().set_num_format("[h]:mm:ss");
    for (column, (label, width)) in [
        ("Datum", 14),
        ("Art", 12),
        ("Projekt", 24),
        ("Tätigkeit", 42),
        ("Beginn (lokal)", 24),
        ("Ende (lokal)", 24),
        ("Arbeit netto", 16),
        ("Pause", 13),
        ("Zeitzone", 22),
        ("Status", 27),
    ]
    .into_iter()
    .enumerate()
    {
        sheet.set_column_width(column as u16, width)?;
        sheet.write_string_with_format(0, column as u16, label, &header)?;
    }
    for (index, slice) in rows.iter().enumerate() {
        let row = (index + 1) as u32;
        let work = slice.entry.kind == Kind::Work;
        let start = slice.start.with_timezone(&slice.entry.zone);
        let end = slice.end.with_timezone(&slice.entry.zone);
        sheet.write_number_with_format(row, 0, excel_date(slice.date), &date)?;
        sheet.write_string(row, 1, if work { "Arbeit" } else { "Pause" })?;
        if work {
            sheet.write_string(
                row,
                2,
                slice
                    .entry
                    .project_name
                    .as_deref()
                    .unwrap_or("Ohne Projekt"),
            )?;
            sheet.write_string(row, 3, &slice.entry.activity)?;
        }
        sheet.write_number_with_format(
            row,
            4,
            excel_date(start.date_naive()) + excel_time(start.time()),
            &clock,
        )?;
        if slice.counted {
            sheet.write_number_with_format(
                row,
                5,
                excel_date(end.date_naive()) + excel_time(end.time()),
                &clock,
            )?;
            sheet.write_number_with_format(
                row,
                if work { 6 } else { 7 },
                (slice.end - slice.start).num_milliseconds() as f64 / 86_400_000.0,
                &elapsed,
            )?;
        }
        sheet.write_string(row, 8, slice.entry.zone.name())?;
        let status = if slice.uncertain && slice.counted {
            "Bis Speicherpunkt · Unbestätigt"
        } else if !slice.counted {
            "Unbestätigt"
        } else if slice.entry.end.is_none() {
            "Laufend · Momentaufnahme"
        } else if work && slice.entry.activity.is_empty() {
            "Beschreibung fehlt"
        } else {
            "Abgeschlossen"
        };
        sheet.write_string(row, 9, status)?;
    }
    workbook.save_to_buffer()
}

pub(super) fn excel_date(date: NaiveDate) -> f64 {
    let origin = NaiveDate::from_ymd_opt(1899, 12, 31).unwrap();
    let leap_bug = NaiveDate::from_ymd_opt(1900, 3, 1).unwrap();
    ((date - origin).num_days() + i64::from(date >= leap_bug)) as f64
}

pub(super) fn excel_time(time: chrono::NaiveTime) -> f64 {
    (f64::from(time.num_seconds_from_midnight()) + f64::from(time.nanosecond()) / 1e9) / 86_400.0
}

fn workbook(reports: &Reports, as_of: NaiveDate) -> Result<Workbook, XlsxError> {
    let report = &reports.period;
    let mut workbook = Workbook::new();
    let sheet = workbook.add_worksheet();
    let header = Format::new().set_bold().set_background_color(0xE7EFE3);
    let number = Format::new().set_num_format("0.########");
    sheet.set_name("Arbeitszeitkonto")?;
    sheet.set_column_width(0, 39)?;
    sheet.set_column_width(1, 52)?;
    sheet.write_string_with_format(0, 0, "Arbeitszeitkonto", &header)?;
    sheet.write_string(1, 0, "Zeitraum")?;
    sheet.write_string(
        1,
        1,
        format!(
            "{} – {}",
            display_date(report.period.start()),
            display_date(report.period.end())
        ),
    )?;
    sheet.write_string(2, 0, "Konto-Stichtag")?;
    sheet.write_string(2, 1, display_date(as_of))?;
    sheet.write_string(3, 0, "Zeitraum erfasst bis")?;
    sheet.write_string(3, 1, display_date(report.through))?;
    sheet.write_string(4, 0, "Einheit")?;
    sheet.write_string(
        4,
        1,
        "Minuten (vorzeichenbehaftet, Sekunden als Bruchteile)",
    )?;
    sheet.write_string(5, 0, "Status")?;
    sheet.write_string(
        5,
        1,
        if report.provisional || reports.balance_provisional {
            "Vorläufig"
        } else {
            "Abgeschlossener Zeitraum"
        },
    )?;
    if report.excludes_open_time || reports.balance_excludes_open_time {
        sheet.write_string(6, 1, "Offene Segmente fließen erst nach Bestätigung ein.")?;
    }
    for (index, (label, value)) in [
        ("Anfangssaldo", report.opening_balance),
        ("Ist netto", Some(report.totals.actual)),
        ("Fälliges Soll", Some(report.finalized_totals.target)),
        ("Volles Plansoll", Some(report.full_planned_target)),
        (
            "Verbleibendes Plansoll",
            Some(report.remaining_planned_target),
        ),
        (
            "Genehmigte Gutschriften",
            Some(report.totals.approved_credits),
        ),
        ("Abgeschlossener Saldo", Some(report.finalized_totals.delta)),
        ("Manuelle Korrekturen", Some(report.totals.adjustments)),
        ("Schlusssaldo", report.closing_balance),
        ("Gesamtsaldo am Stichtag", reports.balance_as_of),
    ]
    .into_iter()
    .enumerate()
    {
        let row = 8 + index as u32;
        sheet.write_string(row, 0, label)?;
        if let Some(value) = value {
            sheet.write_number_with_format(row, 1, minutes(value), &number)?;
        }
    }
    Ok(workbook)
}

fn minutes(seconds: Seconds) -> f64 {
    seconds as f64 / 60.0
}

#[cfg(test)]
mod tests {
    use std::{fs, io::Read};

    use chrono::NaiveDate;
    use projektzeit_core::worktime_account::{
        Account, Adjustment, Observations, OpeningBalance, Period, WeeklySchedule,
    };
    use tempfile::tempdir;
    use zip::ZipArchive;

    use super::*;

    #[test]
    fn workbook_uses_signed_numeric_minutes_from_the_displayed_report() {
        let date = NaiveDate::from_ymd_opt(2025, 11, 1).unwrap();
        let target = WeeklySchedule::new(3600, [0, 0, 0, 0, 0, 3600, 0]).unwrap();
        let account = Account {
            opening: Some(OpeningBalance {
                effective_date: date,
                signed_seconds: -300 * 60,
            }),
            schedules: [(date, target)].into(),
            adjustments: vec![Adjustment {
                effective_date: date,
                signed_seconds: -45 * 60,
            }],
            ..Account::default()
        };
        let observations = Observations {
            actual: [(date, 90 * 60 + 15)].into(),
            today_finished: true,
            ..Observations::default()
        };
        let period = account
            .report(Period::month(date).unwrap(), date, date, &observations)
            .unwrap();
        let reports = Reports {
            balance_as_of: period.closing_balance,
            balance_provisional: true,
            balance_excludes_open_time: false,
            period,
        };

        let folder = tempdir().unwrap();
        let path = folder.path().join("account.xlsx");
        save_account(&path, &reports, date).unwrap();
        let original = fs::read(&path).unwrap();
        assert!(save_account(&path, &reports, date).is_err());
        assert_eq!(fs::read(&path).unwrap(), original);

        let mut archive = ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
        let mut sheet = String::new();
        archive
            .by_name("xl/worksheets/sheet1.xml")
            .unwrap()
            .read_to_string(&mut sheet)
            .unwrap();
        let numeric_cell = |coordinate| -> f64 {
            let (_, cell) = sheet.split_once(&format!("<c r=\"{coordinate}\"")).unwrap();
            let (cell, _) = cell.split_once("</c>").unwrap();
            let (_, value) = cell.split_once("<v>").unwrap();
            value.strip_suffix("</v>").unwrap().parse().unwrap()
        };
        assert_eq!(numeric_cell("B9"), -300.0);
        assert_eq!(numeric_cell("B10"), 90.25);
        assert_eq!(numeric_cell("B11"), 60.0);
        assert_eq!(numeric_cell("B15"), 30.25);
        assert_eq!(numeric_cell("B16"), -45.0);
        assert_eq!(numeric_cell("B17"), -314.75);
        assert_eq!(numeric_cell("B18"), -314.75);
    }
}
