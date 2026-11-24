use std::{fs::OpenOptions, io::Write, path::Path};

use chrono::NaiveDate;
use projektzeit_core::worktime_account::Seconds;
use projektzeit_storage::worktime_account::Reports;
use rust_xlsxwriter::{Format, Workbook, XlsxError};

use super::input::display_date;

pub(super) fn save_account(path: &Path, reports: &Reports, as_of: NaiveDate) -> Result<(), String> {
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("xlsx"))
    {
        return Err("Bitte einen Dateipfad mit der Endung .xlsx eingeben.".into());
    }

    let mut workbook = workbook(reports, as_of)
        .map_err(|error| format!("XLSX konnte nicht erstellt werden ({error})."))?;
    let data = workbook
        .save_to_buffer()
        .map_err(|error| format!("XLSX konnte nicht erstellt werden ({error})."))?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                "Die Datei existiert bereits. Bitte einen neuen Dateinamen wählen.".into()
            } else {
                format!("Datei konnte nicht erstellt werden ({error}).")
            }
        })?;
    if let Err(error) = file.write_all(&data).and_then(|()| file.sync_all()) {
        drop(file);
        let _ = std::fs::remove_file(path);
        return Err(format!("XLSX konnte nicht gespeichert werden ({error})."));
    }
    Ok(())
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
