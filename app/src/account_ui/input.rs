use chrono::{Datelike, NaiveDate};
use projektzeit_core::worktime_account::{Adjustment, OpeningBalance, Seconds, WeeklySchedule};
use projektzeit_storage::worktime_account::Change;

use super::{account_error, view::EditFields};

pub(super) fn parse_date(text: &str) -> Result<NaiveDate, String> {
    let text = text.trim();
    NaiveDate::parse_from_str(text, "%d.%m.%Y")
        .or_else(|_| NaiveDate::parse_from_str(text, "%Y-%m-%d"))
        .ok()
        .filter(|date| (1..=9999).contains(&date.year()))
        .ok_or_else(|| "Bitte ein gültiges Datum im Format TT.MM.JJJJ eingeben.".into())
}

pub(super) fn parse_change(fields: EditFields) -> Result<Change, String> {
    let adjustment_id = || {
        fields
            .entry_id
            .trim()
            .parse::<i64>()
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| "Bitte eine gültige Korrektur-Nr. eingeben.".to_owned())
    };
    if fields.kind == 6 {
        return Ok(Change::VoidAdjustment {
            id: adjustment_id()?,
            reason: fields.reason.to_string(),
        });
    }
    let date = parse_date(&fields.date)?;
    let reason = fields.reason.to_string();
    let amount = || parse_duration(&fields.amount);
    Ok(match fields.kind {
        0 => {
            let weekdays = [
                fields.monday,
                fields.tuesday,
                fields.wednesday,
                fields.thursday,
                fields.friday,
                fields.saturday,
                fields.sunday,
            ]
            .map(|field| parse_duration(&field).and_then(nonnegative))
            .into_iter()
            .collect::<Result<Vec<_>, _>>()?;
            let weekdays = weekdays.try_into().expect("there are seven weekday fields");
            let schedule =
                WeeklySchedule::new(nonnegative(amount()?)?, weekdays).map_err(account_error)?;
            Change::Schedule {
                effective_date: date,
                schedule,
            }
        }
        1 => Change::Opening(OpeningBalance {
            effective_date: date,
            signed_seconds: amount()?,
        }),
        2 => Change::SetTarget {
            date,
            seconds: nonnegative(amount()?)?,
            reason,
        },
        3 => Change::ClearTarget { date, reason },
        4 | 5 => {
            let adjustment = Adjustment {
                effective_date: date,
                signed_seconds: amount()?,
            };
            if fields.kind == 4 {
                Change::AddAdjustment { adjustment, reason }
            } else {
                Change::EditAdjustment {
                    id: adjustment_id()?,
                    adjustment,
                    reason,
                }
            }
        }
        _ => return Err("Bitte eine Änderungsart auswählen.".into()),
    })
}

pub(super) fn display_date(date: NaiveDate) -> String {
    date.format("%d.%m.%Y").to_string()
}

pub(super) fn duration(value: Seconds, signed: bool) -> String {
    let sign = if value < 0 {
        "-"
    } else if signed && value > 0 {
        "+"
    } else {
        ""
    };
    let magnitude = value.unsigned_abs();
    let (hours, minutes, seconds) = (magnitude / 3600, magnitude / 60 % 60, magnitude % 60);
    if seconds == 0 {
        format!("{sign}{hours:02}:{minutes:02} h")
    } else {
        format!("{sign}{hours:02}:{minutes:02}:{seconds:02} h")
    }
}

fn nonnegative(value: Seconds) -> Result<u32, String> {
    u32::try_from(value).map_err(|_| "Das Soll muss eine gültige, nichtnegative Dauer sein.".into())
}

fn parse_duration(text: &str) -> Result<Seconds, String> {
    duration_value(text.trim())
        .ok_or_else(|| "Bitte eine Dauer im Format hh:mm oder hh:mm:ss eingeben.".into())
}

fn duration_value(text: &str) -> Option<Seconds> {
    let (negative, text) = if let Some(value) = text.strip_prefix('-') {
        (true, value)
    } else {
        (false, text.strip_prefix('+').unwrap_or(text))
    };
    let mut fields = text.split(':');
    let hours_field = fields.next()?;
    if hours_field.is_empty() || !hours_field.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let hours = hours_field.parse::<u64>().ok()?;
    let minutes = sexagesimal(fields.next()?)?;
    let seconds = match fields.next() {
        Some(value) => sexagesimal(value)?,
        None => 0,
    };
    if fields.next().is_some() {
        return None;
    }
    let seconds = hours
        .checked_mul(3600)?
        .checked_add(minutes * 60 + seconds)?;
    if negative && seconds == i64::MIN.unsigned_abs() {
        Some(i64::MIN)
    } else {
        i64::try_from(seconds)
            .ok()
            .map(|value| if negative { -value } else { value })
    }
}

fn sexagesimal(text: &str) -> Option<u64> {
    if text.len() != 2 || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    text.parse::<u64>().ok().filter(|&value| value < 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_durations_round_trip_with_seconds_when_needed() {
        for value in [i64::MIN, -4500, 0, 1, 7350, i64::MAX] {
            let rendered = duration(value, true);
            assert_eq!(
                parse_duration(rendered.strip_suffix(" h").unwrap()).unwrap(),
                value
            );
        }
        assert_eq!(duration(45_000, true), "+12:30 h");
        assert_eq!(duration(-11_700, true), "-03:15 h");
        assert_eq!(duration(0, true), "00:00 h");
        for invalid in ["1", "+-1:30", "3:60", "2:03:70", "-1:4", "1:00:00:00"] {
            assert!(parse_duration(invalid).is_err(), "{invalid}");
        }
    }
}
