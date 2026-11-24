//! Persistent desktop preferences and the in-memory work schedule draft.

use chrono::NaiveDate;
use projektzeit_core::worktime_account::WeeklySchedule;
use rusqlite::{Connection, OptionalExtension, params};

use super::{
    account_error,
    input::{duration, parse_target},
};

pub(super) const APPEARANCE_KEY: &str = "appearance.color_scheme";
const MINI_KEY: &str = "mini_timer.preferences";
const POSITION_KEY: &str = "mini_timer.position";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Point {
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MiniPreferences {
    pub auto_show: bool,
    pub topmost: bool,
    pub remember: bool,
    pub margin: u32,
}

impl Default for MiniPreferences {
    fn default() -> Self {
        Self {
            auto_show: true,
            topmost: true,
            remember: true,
            margin: 24,
        }
    }
}

impl MiniPreferences {
    pub fn load(db: &Connection) -> rusqlite::Result<Self> {
        let stored = get(db, MINI_KEY)?;
        let value = stored
            .as_deref()
            .and_then(|json| serde_json::from_str::<serde_json::Value>(json).ok());
        let Some(value) = value else {
            return Ok(Self::default());
        };
        Ok(Self {
            auto_show: value["auto_show"].as_bool().unwrap_or(true),
            topmost: value["topmost"].as_bool().unwrap_or(true),
            remember: value["remember"].as_bool().unwrap_or(true),
            margin: value["margin"]
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|&value| value <= 256)
                .unwrap_or(24),
        })
    }

    pub fn save(self, db: &Connection) -> rusqlite::Result<()> {
        put(
            db,
            MINI_KEY,
            &serde_json::json!({
                "auto_show": self.auto_show, "topmost": self.topmost, "remember": self.remember,
                "default_position": "bottom_right", "margin": self.margin,
            })
            .to_string(),
        )
    }
}

pub(super) fn get(db: &Connection, key: &str) -> rusqlite::Result<Option<String>> {
    db.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
        row.get(0)
    })
    .optional()
}

pub(super) fn put(db: &Connection, key: &str, value: &str) -> rusqlite::Result<()> {
    db.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

pub(super) fn load_position(db: &Connection) -> rusqlite::Result<Option<Point>> {
    Ok(get(db, POSITION_KEY)?.and_then(|value| {
        let (x, y) = value.split_once(',')?;
        Some(Point {
            x: x.parse().ok()?,
            y: y.parse().ok()?,
        })
    }))
}

pub(super) fn save_position(db: &Connection, position: Option<Point>) -> rusqlite::Result<()> {
    if let Some(position) = position {
        put(db, POSITION_KEY, &format!("{},{}", position.x, position.y))
    } else {
        db.execute("DELETE FROM settings WHERE key = ?1", [POSITION_KEY])?;
        Ok(())
    }
}

pub(super) struct ScheduleDraft {
    pub date: NaiveDate,
    pub input: [String; 8],
    pub candidate: Option<WeeklySchedule>,
    pub dirty: bool,
    pub message: String,
    pub invalid: bool,
}

impl ScheduleDraft {
    pub fn new(date: NaiveDate, schedule: Option<WeeklySchedule>) -> Self {
        let weekly = schedule.map_or(0, WeeklySchedule::weekly_seconds);
        let weekdays = schedule.map_or([0; 7], WeeklySchedule::weekdays);
        let mut draft = Self {
            date,
            input: std::array::from_fn(|index| {
                format_target(if index == 0 {
                    weekly
                } else {
                    weekdays[index - 1]
                })
            }),
            candidate: None,
            dirty: false,
            message: String::new(),
            invalid: false,
        };
        if schedule.is_none() {
            draft.input[0].clear();
        }
        draft
    }

    pub fn edit(&mut self, index: usize, text: String) {
        let Some(input) = self.input.get_mut(index) else {
            return;
        };
        *input = text;
        self.dirty = true;
        match self.validate(index) {
            Ok(schedule) => {
                self.candidate = Some(schedule);
                self.invalid = false;
                self.message.clear();
            }
            Err(message) => {
                self.candidate = None;
                self.failure(message);
            }
        }
    }

    fn validate(&mut self, index: usize) -> Result<WeeklySchedule, String> {
        let mut days = [0; 7];
        for (day, (seconds, text)) in days.iter_mut().zip(&self.input[1..]).enumerate() {
            *seconds = parse_target(text).map_err(|_| {
                format!(
                    "{}: Bitte eine nichtnegative Dauer wie 07:36 eingeben.",
                    ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"][day],
                )
            })?;
        }
        let total = if index == 0 {
            let total = parse_target(&self.input[0]).map_err(|_| {
                "Wochensoll: Bitte eine nichtnegative Dauer wie 38:00 eingeben.".to_owned()
            })?;
            days = distribute(total, days);
            for (text, seconds) in self.input[1..].iter_mut().zip(days) {
                *text = format_target(seconds);
            }
            total
        } else {
            let total = u32::try_from(days.iter().map(|&seconds| u64::from(seconds)).sum::<u64>())
                .map_err(|_| "Die Summe der Tagesziele ist zu groß.".to_owned())?;
            self.input[0] = format_target(total);
            total
        };
        WeeklySchedule::new(total, days).map_err(account_error)
    }

    pub fn saved(&mut self, today: NaiveDate) {
        self.date = today;
        self.dirty = false;
        self.candidate = None;
        self.invalid = false;
        self.message = "Arbeitszeit gespeichert.".into();
    }

    pub fn failure(&mut self, message: String) {
        self.invalid = true;
        self.message = message;
    }
}

fn format_target(seconds: u32) -> String {
    let formatted = duration(i64::from(seconds), false);
    formatted.trim_end_matches(" h").to_owned()
}

fn distribute(total: u32, current: [u32; 7]) -> [u32; 7] {
    let weights = if current == [0; 7] {
        [1, 1, 1, 1, 1, 0, 0]
    } else {
        current
    };
    let weight_total = weights.iter().map(|&value| u64::from(value)).sum::<u64>();
    let shares = weights.map(|weight| u64::from(weight) * u64::from(total));
    let mut result = shares.map(|share| (share / weight_total) as u32);
    let mut order = [0, 1, 2, 3, 4, 5, 6];
    order.sort_by_key(|&index| std::cmp::Reverse(shares[index] % weight_total));
    for &index in order
        .iter()
        .take((total - result.iter().sum::<u32>()) as usize)
    {
        result[index] += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weekly_and_day_edits_keep_the_schedule_consistent() {
        let date = NaiveDate::from_ymd_opt(2026, 11, 24).unwrap();
        let mut draft = ScheduleDraft::new(date, None);
        let initial = draft.input.clone();
        assert!(draft.candidate.is_none() && !draft.dirty);
        draft.edit(0, "38:00".into());
        assert_eq!(
            draft.candidate.unwrap().weekdays(),
            [27360, 27360, 27360, 27360, 27360, 0, 0]
        );
        draft.edit(6, "02:15".into());
        assert_eq!(draft.input[0], "40:15");
        assert_eq!(draft.candidate.unwrap().weekly_seconds(), 144900);
        draft.edit(6, "02:".into());
        assert_eq!(draft.input[6], "02:");
        assert!(draft.candidate.is_none() && draft.dirty && draft.invalid);
        assert_ne!(draft.input, initial);
        draft.edit(6, "01:00".into());
        assert_eq!(draft.candidate.unwrap().weekly_seconds(), 140400);
    }

    #[test]
    fn mini_preferences_and_signed_positions_survive_reopen() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("settings.sqlite");
        let db = projektzeit_storage::open(&path).unwrap();
        assert_eq!(
            MiniPreferences::load(&db).unwrap(),
            MiniPreferences::default()
        );
        let preferences = MiniPreferences {
            auto_show: false,
            topmost: false,
            remember: true,
            margin: 24,
        };
        let position = Some(Point { x: -1500, y: 256 });
        preferences.save(&db).unwrap();
        save_position(&db, position).unwrap();
        drop(db);
        let db = projektzeit_storage::open(path).unwrap();
        assert_eq!(MiniPreferences::load(&db).unwrap(), preferences);
        assert_eq!(load_position(&db).unwrap(), position);
        save_position(&db, None).unwrap();
        assert_eq!(load_position(&db).unwrap(), None);
    }
}
