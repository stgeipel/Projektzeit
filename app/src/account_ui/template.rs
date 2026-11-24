use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    io::{Cursor, Read, Write},
    ops::Range,
    path::Path,
};

use chrono::NaiveTime;
use projektzeit_core::worktime_account::Period;
use projektzeit_storage::tracker::Kind;
use roxmltree::{Document, Node};
use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

use super::{
    dashboard::Slice,
    export::{excel_date, excel_time},
};

const WORKBOOK: &str = "xl/workbook.xml";
const RELS: &str = "xl/_rels/workbook.xml.rels";
const REL_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const PAUSE: &str = "— Pause —";
const FALLBACK: &str = "Für diese Auswahl bitte den Standardexport ohne Vorlage verwenden.";

type Cell<'a> = Node<'a, 'a>;
type Edits = Vec<(Range<usize>, String)>;

pub(super) fn copy(path: &Path, period: Period, rows: &[Slice]) -> Result<Vec<u8>, String> {
    let structure = |error: &dyn std::fmt::Display| {
        format!("Excel-Vorlage konnte nicht gelesen werden ({error}).")
    };
    let input = fs::File::open(path).map_err(|error| structure(&error))?;
    let mut archive = ZipArchive::new(input).map_err(|error| structure(&error))?;
    let workbook = read_xml(&mut archive, WORKBOOK)?;
    let book = document(&workbook)?;
    if book.descendants().any(|node| {
        node.has_tag_name("workbookPr") && matches!(node.attribute("date1904"), Some("1" | "true"))
    }) {
        return Err(format!(
            "Die Vorlage muss das Excel-Datumssystem 1900 verwenden. {FALLBACK}"
        ));
    }
    let sheets = book.descendants().filter(|node| node.has_tag_name("sheet"));
    let (index, sheet) = sheets
        .enumerate()
        .find(|(_, node)| node.attribute("name") == Some("blank"))
        .ok_or_else(layout_error)?;
    let relation = sheet.attribute((REL_NS, "id")).ok_or_else(layout_error)?;
    let relationships = read_xml(&mut archive, RELS)?;
    let relationships = document(&relationships)?;
    let target = relationships
        .descendants()
        .find(|node| {
            node.has_tag_name("Relationship")
                && node.attribute("Id") == Some(relation)
                && node
                    .attribute("Type")
                    .is_some_and(|kind| kind == format!("{REL_NS}/worksheet"))
                && node.attribute("TargetMode") != Some("External")
        })
        .and_then(|node| node.attribute("Target"))
        .ok_or_else(layout_error)?;
    let name = if let Some(absolute) = target.strip_prefix("/xl/worksheets/") {
        absolute
    } else {
        target
            .strip_prefix("worksheets/")
            .ok_or_else(layout_error)?
    };
    if name.is_empty() || name.contains(['/', '\\']) || !name.ends_with(".xml") {
        return Err(layout_error());
    }
    let target = format!("xl/worksheets/{name}");
    let sheet_xml = read_xml(&mut archive, &target)?;
    let changed_sheet = patch_sheet(&sheet_xml, period, rows)?;
    let mut book_edits = Vec::new();
    if let Some(view) = book
        .descendants()
        .find(|node| node.has_tag_name("workbookView"))
    {
        book_edits.push((
            view.range(),
            set_attributes(&workbook, view, &[("activeTab", index.to_string())]),
        ));
    }
    if let Some(calc) = book.descendants().find(|node| node.has_tag_name("calcPr")) {
        book_edits.push((
            calc.range(),
            set_attributes(
                &workbook,
                calc,
                &[("fullCalcOnLoad", "1".into()), ("calcMode", "auto".into())],
            ),
        ));
    } else {
        return Err(layout_error());
    }
    let changed_book = replace(workbook, book_edits);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let file = archive
            .by_index_raw(index)
            .map_err(|error| structure(&error))?;
        let name = file.name().map_err(|error| structure(&error))?;
        let replacement = match name.as_ref() {
            WORKBOOK => Some(&changed_book),
            name if name == target => Some(&changed_sheet),
            _ => None,
        };
        if let Some(replacement) = replacement {
            writer
                .start_file(name, SimpleFileOptions::default())
                .map_err(|error| structure(&error))?;
            writer
                .write_all(replacement.as_bytes())
                .map_err(|error| structure(&error))?;
        } else {
            writer
                .raw_copy_file(file)
                .map_err(|error| structure(&error))?;
        }
    }
    Ok(writer
        .finish()
        .map_err(|error| structure(&error))?
        .into_inner())
}

fn read_xml<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    name: &str,
) -> Result<String, String> {
    let file = archive.by_name(name).map_err(|_| layout_error())?;
    let mut xml = String::new();
    const LIMIT: u64 = 4 * 1024 * 1024;
    file.take(LIMIT + 1)
        .read_to_string(&mut xml)
        .map_err(|_| layout_error())?;
    if xml.len() as u64 > LIMIT {
        return Err(layout_error());
    }
    Ok(xml)
}

fn document(xml: &str) -> Result<Document<'_>, String> {
    Document::parse(xml).map_err(|_| layout_error())
}

fn layout_error() -> String {
    format!("Die Vorlage entspricht nicht dem unterstützten Blatt „blank“. {FALLBACK}")
}

fn patch_sheet(xml: &str, period: Period, rows: &[Slice]) -> Result<String, String> {
    if rows.len() > 66 {
        return Err(format!("Die Vorlage hat 66 Zeilen. {FALLBACK}"));
    }
    let document = document(xml)?;
    let cells: BTreeMap<_, _> = document
        .descendants()
        .filter(|node| node.has_tag_name("c"))
        .filter_map(|cell| cell.attribute("r").map(|name| (name, cell)))
        .collect();
    let lookup = |address: &str| cells.get(address).copied().ok_or_else(layout_error);
    for address in 15..=80 {
        for column in ["C", "D", "F", "G", "I", "K"] {
            if has_data(lookup(&format!("{column}{address}"))?) {
                return Err("Das Blatt „blank“ enthält bereits Buchungen. Bitte eine leere Kopie auswählen.".into());
            }
        }
        for column in ["B", "J", "M"] {
            formula(lookup(&format!("{column}{address}"))?)?;
        }
    }
    for address in ["H8", "L8", "H9", "L9"] {
        if has_data(lookup(address)?) {
            return Err(layout_error());
        }
    }
    for (address, slot) in [("F82", "H7"), ("F83", "L7"), ("M86", "H7"), ("M87", "L7")] {
        let expression = format!("SUMIF(F15:F80,{slot},M15:M80)");
        if !formula(lookup(address)?)?
            .text()
            .is_some_and(|text| text.contains(&expression))
        {
            return Err(layout_error());
        }
    }
    let mut edits = Vec::new();
    for (address, date) in [("D6", period.start()), ("D7", period.end())] {
        let serial = excel_date(date);
        if serial < 1.0 {
            return Err(format!(
                "Das Datum ist im Excel-Datumssystem nicht darstellbar. {FALLBACK}"
            ));
        }
        edits.push(number(lookup(address)?, serial));
    }
    let mut names = Vec::<&str>::new();
    let mut totals = [0_i64; 2];
    for (index, slice) in rows.iter().enumerate() {
        let address = 15 + index;
        let work = slice.entry.kind == Kind::Work;
        if slice.entry.end.is_none() || (work && slice.entry.activity.trim().is_empty()) {
            return Err(format!(
                "Bitte offene Zeiten abschließen und Tätigkeiten ergänzen. {FALLBACK}"
            ));
        }
        let start = slice.start.with_timezone(&slice.entry.zone);
        let end = slice.end.with_timezone(&slice.entry.zone);
        let begin = excel_time(start.time());
        let finish = if end.date_naive() == start.date_naive() {
            excel_time(end.time())
        } else if end.time() == NaiveTime::MIN
            && start.date_naive().succ_opt() == Some(end.date_naive())
        {
            1.0
        } else {
            return Err(format!(
                "Ein Zeitintervall überschreitet einen Kalendertag. {FALLBACK}"
            ));
        };
        let millis = (slice.end - slice.start).num_milliseconds();
        let elapsed = millis as f64 / 86_400_000.0;
        if (finish - begin - elapsed).abs() > 0.000_000_001 {
            return Err(format!(
                "Eine Buchung liegt über einer Zeitumstellung. {FALLBACK}"
            ));
        }
        let project = if work {
            let name = slice
                .entry
                .project_name
                .as_deref()
                .unwrap_or("Ohne Projekt");
            let slot = match names.iter().position(|&existing| existing == name) {
                Some(slot) => slot,
                None => {
                    if names.len() == 2
                        || name == PAUSE
                        || name.contains(['*', '?', '~'])
                        || name.starts_with(['=', '<', '>'])
                        || names
                            .iter()
                            .any(|other| other.to_lowercase() == name.to_lowercase())
                    {
                        return Err(format!(
                            "Die Vorlage unterstützt zwei eindeutige Projektnamen ohne Excel-Suchoperatoren. {FALLBACK}"
                        ));
                    }
                    names.push(name);
                    names.len() - 1
                }
            };
            totals[slot] += millis;
            name
        } else {
            PAUSE
        };
        let activity = if work { &slice.entry.activity } else { "Pause" };
        for (column, value) in [("D", activity), ("F", project)] {
            edits.push(inline_text(lookup(&format!("{column}{address}"))?, value)?);
        }
        let date = excel_date(slice.date);
        let pause = if work { 0.0 } else { elapsed };
        for (column, value) in [("C", date), ("G", begin), ("I", finish), ("K", pause)] {
            edits.push(number(lookup(&format!("{column}{address}"))?, value));
        }
        for (column, value) in [("B", date), ("J", elapsed), ("M", elapsed - pause)] {
            edits.push(cached(
                lookup(&format!("{column}{address}"))?,
                xml,
                None,
                &value.to_string(),
                None,
            )?);
        }
    }
    let total = totals.iter().sum::<i64>();
    for (slot, (header, summary, name, time, percent, numeric)) in [
        ("H7", "L86", "E82", "F82", "G82", "M86"),
        ("L7", "L87", "E83", "F83", "G83", "M87"),
    ]
    .into_iter()
    .enumerate()
    {
        let caption = names.get(slot).copied().unwrap_or_default();
        edits.push(inline_text(lookup(header)?, caption)?);
        edits.push(inline_text(lookup(summary)?, caption)?);
        edits.push(cached(lookup(name)?, xml, Some("str"), caption, None)?);
        let text = if caption.is_empty() {
            String::new()
        } else {
            hours_text(totals[slot])
        };
        edits.push(cached(lookup(time)?, xml, Some("str"), &text, None)?);
        let ratio = if total == 0 {
            0.0
        } else {
            totals[slot] as f64 / total as f64
        };
        edits.push(cached(
            lookup(percent)?,
            xml,
            None,
            &ratio.to_string(),
            Some(if slot == 0 {
                "IFERROR(M86/M85,0)"
            } else {
                "IFERROR(M87/M85,0)"
            }),
        )?);
        edits.push(cached(
            lookup(numeric)?,
            xml,
            None,
            &(totals[slot] as f64 / 36_000.0).to_string(),
            None,
        )?);
    }
    edits.push(cached(
        lookup("M85")?,
        xml,
        None,
        &(total as f64 / 36_000.0).to_string(),
        None,
    )?);
    edits.push(cached(
        lookup("M82")?,
        xml,
        Some("str"),
        &hours_text(total),
        None,
    )?);
    Ok(replace(xml.to_owned(), edits))
}

fn has_data(cell: Cell<'_>) -> bool {
    cell.children().any(|child| {
        child.has_tag_name("f")
            || ((child.has_tag_name("v") || child.has_tag_name("is"))
                && child
                    .descendants()
                    .any(|node| node.text().is_some_and(|text| !text.is_empty())))
    })
}

fn formula(cell: Cell<'_>) -> Result<Cell<'_>, String> {
    cell.children()
        .find(|child| child.has_tag_name("f"))
        .ok_or_else(layout_error)
}

fn hours_text(millis: i64) -> String {
    format!("{}h {}min", millis / 3_600_000, millis / 60_000 % 60)
}

fn number(cell: Cell<'_>, value: f64) -> (Range<usize>, String) {
    cell_content(cell, None, &format!("<v>{value}</v>"))
}

fn inline_text(cell: Cell<'_>, text: &str) -> Result<(Range<usize>, String), String> {
    if text.encode_utf16().count() > 32_767
        || text
            .chars()
            .any(|ch| ch.is_control() && !matches!(ch, '\t' | '\r' | '\n'))
    {
        return Err(format!(
            "Ein Textfeld kann nicht in Excel gespeichert werden. {FALLBACK}"
        ));
    }
    if text.is_empty() {
        Ok(cell_content(cell, None, ""))
    } else {
        Ok(cell_content(
            cell,
            Some("inlineStr"),
            &format!(
                "<is><t xml:space=\"preserve\">{}</t></is>",
                xml_escape(text)
            ),
        ))
    }
}

fn cached(
    cell: Cell<'_>,
    xml: &str,
    kind: Option<&str>,
    value: &str,
    new_formula: Option<&str>,
) -> Result<(Range<usize>, String), String> {
    let existing = &xml[formula(cell)?.range()];
    let expression =
        new_formula.map_or_else(|| existing.to_owned(), |text| format!("<f>{text}</f>"));
    Ok(cell_content(
        cell,
        kind,
        &format!("{expression}<v>{}</v>", xml_escape(value)),
    ))
}

fn cell_content(cell: Cell<'_>, kind: Option<&str>, content: &str) -> (Range<usize>, String) {
    let mut output = String::from("<c");
    for attr in cell.attributes().filter(|attr| attr.name() != "t") {
        write!(output, " {}=\"{}\"", attr.name(), xml_escape(attr.value())).unwrap();
    }
    if let Some(kind) = kind {
        write!(output, " t=\"{kind}\"").unwrap();
    }
    write!(output, ">{content}</c>").unwrap();
    (cell.range(), output)
}

fn set_attributes(xml: &str, node: Node<'_, '_>, values: &[(&str, String)]) -> String {
    let mut result = xml[node.range()].to_owned();
    let mut edits = Vec::new();
    let mut additions = String::new();
    for (name, value) in values {
        if let Some(attribute) = node.attributes().find(|attr| attr.name() == *name) {
            let range = attribute.range_value();
            edits.push((
                (range.start - node.range().start)..(range.end - node.range().start),
                value.clone(),
            ));
        } else {
            write!(additions, " {name}=\"{value}\"").unwrap();
        }
    }
    let end = result.find('>').unwrap();
    let end = if result.as_bytes()[end - 1] == b'/' {
        end - 1
    } else {
        end
    };
    edits.push((end..end, additions));
    result = replace(result, edits);
    result
}

fn replace(mut input: String, mut edits: Edits) -> String {
    edits.sort_unstable_by_key(|(range, _)| std::cmp::Reverse(range.start));
    for (range, value) in edits {
        input.replace_range(range, &value);
    }
    input
}

fn xml_escape(text: &str) -> String {
    text.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
        .replace("_x", "_x005F_x")
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::DateTime;
    use chrono_tz::Europe::Berlin;
    use projektzeit_storage::tracker::Entry;

    // Structural fixture only. The supplied workbook contains contributor data.
    fn empty_sheet() -> String {
        let mut sheet = String::from("<worksheet><sheetData>");
        for row in 15..=80 {
            write!(sheet, "<row r=\"{row}\">").unwrap();
            for column in ["C", "D", "F", "G", "I", "K"] {
                write!(sheet, "<c r=\"{column}{row}\" s=\"7\"/>").unwrap();
            }
            for column in ["B", "J", "M"] {
                write!(
                    sheet,
                    "<c r=\"{column}{row}\" t=\"str\"><f t=\"shared\" si=\"1\"/><v/></c>"
                )
                .unwrap();
            }
            sheet.push_str("</row>");
        }
        for address in ["D6", "D7", "H7", "L7", "H8", "L8", "H9", "L9", "L86", "L87"] {
            write!(sheet, "<c r=\"{address}\" s=\"7\"/>").unwrap();
        }
        for (address, expression) in [
            ("F82", "SUMIF(F15:F80,H7,M15:M80)"),
            ("F83", "SUMIF(F15:F80,L7,M15:M80)"),
            ("M86", "SUMIF(F15:F80,H7,M15:M80)"),
            ("M87", "SUMIF(F15:F80,L7,M15:M80)"),
            ("E82", "H7"),
            ("E83", "L7"),
            ("G82", "M86/M85"),
            ("G83", "M87/M85"),
            ("M85", "SUM(M15:M80)"),
            ("M82", "SUM(M15:M80)"),
        ] {
            write!(sheet, "<c r=\"{address}\"><f>{expression}</f><v/></c>").unwrap();
        }
        sheet.push_str(
            "</sheetData><mergeCells><mergeCell ref=\"D15:E15\"/></mergeCells></worksheet>",
        );
        sheet
    }

    fn slice(kind: Kind, start: &str, end: &str) -> Slice {
        let start = DateTime::parse_from_rfc3339(start).unwrap().to_utc();
        let end = DateTime::parse_from_rfc3339(end).unwrap().to_utc();
        Slice {
            entry: Entry {
                id: 1,
                workday_id: 1,
                kind,
                project: None,
                project_name: None,
                project_color: None,
                activity: "Prüfen & a < b".into(),
                zone: Berlin,
                start,
                end: Some(end),
            },
            date: start.with_timezone(&Berlin).date_naive(),
            start,
            end,
            counted: true,
        }
    }

    #[test]
    fn compatibility_export_keeps_styles_and_formulas_and_deducts_breaks() {
        let work = slice(
            Kind::Work,
            "2026-11-24T08:00:00+01:00",
            "2026-11-24T16:00:00+01:00",
        );
        let pause = slice(
            Kind::Break,
            "2026-11-24T16:00:00+01:00",
            "2026-11-24T16:30:00+01:00",
        );
        let period = Period::day(work.date);
        let original = empty_sheet();
        let patched = patch_sheet(&original, period, &[work, pause]).unwrap();
        let xml = document(&patched).unwrap();
        let cell = |id| {
            xml.descendants()
                .find(|node| node.attribute("r") == Some(id))
                .unwrap()
        };
        let value = |id| {
            cell(id)
                .descendants()
                .find(|node| node.has_tag_name("v"))
                .and_then(|value| value.text())
                .unwrap()
        };
        assert_eq!(value("M16"), "0");
        assert_eq!(value("M82"), "8h 0min");
        assert_eq!(cell("D15").attribute("s"), Some("7"));
        assert_eq!(
            cell("D15")
                .descendants()
                .find(|node| node.has_tag_name("t"))
                .unwrap()
                .text(),
            Some("Prüfen & a < b")
        );
        assert_eq!(formula(cell("J15")).unwrap().attribute("si"), Some("1"));
        assert!(patched.contains("<mergeCell ref=\"D15:E15\"/>"));
        assert!(patch_sheet(&patched, period, &[]).is_err());
    }

    #[test]
    fn compatibility_export_rejects_offset_changes_and_open_segments() {
        let mut work = slice(
            Kind::Work,
            "2026-10-25T02:00:00+02:00",
            "2026-10-25T03:00:00+01:00",
        );
        let period = Period::day(work.date);
        let sheet = empty_sheet();
        assert!(
            patch_sheet(&sheet, period, std::slice::from_ref(&work))
                .unwrap_err()
                .contains("Zeitumstellung")
        );
        work.entry.end = None;
        assert!(
            patch_sheet(&sheet, period, &[work])
                .unwrap_err()
                .contains("offene Zeiten")
        );
    }
}
