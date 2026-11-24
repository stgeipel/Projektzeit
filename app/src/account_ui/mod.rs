//! Desktop presentation of worktime-account reports and recorded time.

mod dashboard;

mod export;
mod input;
// Slint generates the window bindings in this module.
#[allow(unsafe_code)]
mod view;

use std::{cell::RefCell, ffi::OsStr, path::PathBuf, rc::Rc, time::SystemTime};

use chrono::{DateTime, Datelike, Months, NaiveDate, TimeDelta};
use chrono_tz::Tz;
use projektzeit_core::{
    Timestamp,
    worktime_account::{AccountError, Period, Seconds},
};
use projektzeit_storage::worktime_account::{
    self as account, BalanceState, Change, Impact, Preview, Reports, StoreError,
};
use rusqlite::{Connection, OptionalExtension};
use slint::{ComponentHandle, ModelRc, VecModel};

use input::{display_date, duration, parse_change, parse_date};
use view::{AccountWindow, DayRow, EditFields, Metric};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(i32)]
enum Page {
    Today,
    Times,
    Projects,
    Reports,
    Settings,
    About,
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Range {
    Day,
    #[default]
    Week,
    Month,
}

impl Range {
    fn period(self, date: NaiveDate) -> Result<Period, AccountError> {
        match self {
            Self::Day => Ok(Period::day(date)),
            Self::Week => Period::iso_week(date),
            Self::Month => Period::month(date),
        }
    }

    fn step(self, date: NaiveDate, direction: i32) -> Option<NaiveDate> {
        match self {
            Self::Day => date.checked_add_signed(TimeDelta::days(i64::from(direction))),
            Self::Week => date.checked_add_signed(TimeDelta::weeks(i64::from(direction))),
            Self::Month if direction < 0 => date.checked_sub_months(Months::new(1)),
            Self::Month => date.checked_add_months(Months::new(1)),
        }
    }
}

const THEME_KEY: &str = "appearance.color_scheme";
const THEMES: [&str; 3] = ["system", "light", "dark"];

struct Controller {
    connection: Connection,
    zone: Tz,
    page: Page,
    selected: NaiveDate,
    range: Range,
    pending: Option<Preview>,
    displayed_report: Option<(NaiveDate, Reports)>,
}

pub(super) fn run(path: PathBuf, zone: Option<&OsStr>) -> Result<(), String> {
    let zone = match zone {
        Some(zone) => zone.to_str().and_then(|value| value.parse().ok()),
        None => iana_time_zone::get_timezone()
            .ok()
            .and_then(|value| value.parse().ok()),
    }
    .ok_or_else(|| {
        "Zeitzone unbekannt. Bitte eine IANA-Zeitzone angeben, z. B. Europe/Berlin.".to_owned()
    })?;
    let connection = projektzeit_storage::open(path)
        .map_err(|error| format!("Datenbank konnte nicht geöffnet werden ({error})."))?;
    let window = AccountWindow::new()
        .map_err(|error| format!("Fenster konnte nicht geöffnet werden ({error})."))?;
    let stored_theme: Option<String> = connection
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            [THEME_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(theme_error)?;
    let theme = THEMES
        .iter()
        .position(|&theme| Some(theme) == stored_theme.as_deref());
    window.set_scheme(theme.unwrap_or_default() as i32);
    let controller = Rc::new(RefCell::new(Controller {
        connection,
        zone,
        page: Page::Today,
        selected: local_date(now(), zone),
        range: Range::default(),
        pending: None,
        displayed_report: None,
    }));
    connect(&window, &controller);
    controller.borrow_mut().render(&window)?;
    window
        .run()
        .map_err(|error| format!("Fenster konnte nicht ausgeführt werden ({error})."))
}

fn connect(window: &AccountWindow, controller: &Rc<RefCell<Controller>>) {
    window.on_navigate(action(window, controller, |state, window, page| {
        state.page = match page {
            0 => Page::Today,
            1 => Page::Times,
            2 => Page::Projects,
            3 => Page::Reports,
            4 => Page::Settings,
            _ => Page::About,
        };
        state.clear_preview(window);
        state.render(window)
    }));
    window.on_select_range(action(window, controller, |state, window, index| {
        state.range = match index {
            0 => Range::Day,
            2 => Range::Month,
            _ => Range::Week,
        };
        state.render(window)
    }));
    window.on_change_scheme(action(window, controller, |state, window, index: i32| {
        let value = THEMES
            .get(index as usize)
            .ok_or_else(|| "Unbekanntes Erscheinungsbild.".to_owned())?;
        state
            .connection
            .execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                (THEME_KEY, value),
            )
            .map_err(theme_error)?;
        window.set_scheme(index);
        Ok(())
    }));
    window.on_step_period(action(window, controller, |state, window, direction| {
        let today = local_date(now(), state.zone);
        let range = state.report_range();
        let date = range
            .step(state.selected, direction)
            .ok_or_else(|| account_error(AccountError::InvalidDateRange))?;
        let selected = range.period(date).map_err(account_error)?;
        state.selected = if (selected.start()..=selected.end()).contains(&today) {
            today
        } else {
            selected.end()
        };
        state.render(window)
    }));
    window.on_go_to_date(action(
        window,
        controller,
        |state, window, selection: slint::SharedString| {
            state.selected = parse_date(&selection)?;
            state.render(window)
        },
    ));
    let refresh = action(window, controller, |state, window, ()| state.render(window));
    window.on_refresh(move || refresh(()));
    window.on_preview_edit(action(
        window,
        controller,
        |state, window, fields: EditFields| state.preview(window, parse_change(fields)?),
    ));
    let confirm = action(window, controller, |state, window, ()| {
        let preview = state
            .pending
            .take()
            .ok_or_else(|| "Bitte zuerst eine Vorschau erstellen.".to_owned())?;
        window.set_preview_ready(false);
        account::apply(&mut state.connection, preview, now()).map_err(store_error)?;
        state.render(window)?;
        window.set_feedback("Änderung gespeichert.".into());
        Ok(())
    });
    window.on_confirm_edit(move || confirm(()));
    let cancel = action(window, controller, |state, window, ()| {
        state.clear_preview(window);
        Ok(())
    });
    window.on_cancel_edit(move || cancel(()));
    window.on_export_to(action(
        window,
        controller,
        |state, window, path: slint::SharedString| {
            let (date, report) = state
                .displayed_report
                .as_ref()
                .ok_or_else(|| "Kein Bericht zum Exportieren vorhanden.".to_owned())?;
            export::save_account(&PathBuf::from(path.trim()), report, *date)?;
            window.set_feedback("Arbeitszeitkonto exportiert.".into());
            Ok(())
        },
    ));
}

fn action<A>(
    window: &AccountWindow,
    controller: &Rc<RefCell<Controller>>,
    perform: impl Fn(&mut Controller, &AccountWindow, A) -> Result<(), String> + 'static,
) -> impl Fn(A) + 'static {
    let window = window.as_weak();
    let controller = Rc::clone(controller);
    move |argument| {
        if let Some(window) = window.upgrade() {
            window.set_feedback("".into());
            if let Err(error) = perform(&mut controller.borrow_mut(), &window, argument) {
                window.set_feedback(error.into());
            }
        }
    }
}

impl Controller {
    fn report_range(&self) -> Range {
        match self.page {
            Page::Times => self.range,
            Page::Reports => Range::Month,
            _ => Range::Week,
        }
    }

    fn render(&mut self, window: &AccountWindow) -> Result<(), String> {
        window.set_page(self.page as i32);
        window.set_range_index(match self.range {
            Range::Day => 0,
            Range::Week => 1,
            Range::Month => 2,
        });
        self.displayed_report = None;
        window.set_has_report(false);
        window.set_notice("".into());
        window.set_rows(model(Vec::new()));
        window.set_segments(model(Vec::new()));
        window.set_subheading("".into());
        let observed_at = now();
        let today = local_date(observed_at, self.zone);
        match self.page {
            Page::Settings => {
                window.set_heading("Einstellungen".into());
                window.set_report_date(display_date(today).into());
                self.render_settings(window)
            }
            Page::About => {
                window.set_heading("Über Projektzeit".into());
                Ok(())
            }
            Page::Projects => {
                window.set_heading("Projekte".into());
                window.set_projects(model(dashboard::projects(&self.connection)?));
                Ok(())
            }
            Page::Today | Page::Times | Page::Reports => {
                self.render_report(window, today, observed_at)
            }
        }
    }

    fn render_report(
        &mut self,
        window: &AccountWindow,
        today: NaiveDate,
        observed_at: Timestamp,
    ) -> Result<(), String> {
        let selected = if self.page == Page::Today {
            today
        } else {
            self.selected
        };
        let range = self.report_range();
        let period = range.period(selected).map_err(account_error)?;
        let (heading, subheading) = match self.page {
            Page::Today => ("Heute", weekday_date(today)),
            Page::Times if range == Range::Day => ("Zeiten", weekday_date(selected)),
            Page::Times if range == Range::Week => (
                "Zeiten",
                format!(
                    "Kalenderwoche {} · {} – {}",
                    selected.iso_week().week(),
                    display_date(period.start()),
                    display_date(period.end())
                ),
            ),
            Page::Times => ("Zeiten", month_year(selected)),
            _ => ("Berichte", month_year(selected)),
        };
        window.set_heading(heading.into());
        window.set_subheading(subheading.into());
        window.set_report_date(display_date(selected).into());

        let timeline = if self.page == Page::Today || range == Range::Day {
            dashboard::timeline(&self.connection, selected, observed_at)?
        } else {
            dashboard::Timeline::default()
        };
        window.set_segments(model(timeline.rows));
        if self.page == Page::Today {
            window.set_cards(model(today_cards(
                timeline.work_seconds,
                timeline.break_seconds,
                None,
            )));
            window.set_progress(0.0);
            window.set_progress_label("".into());
        }

        let as_of = selected.min(today);
        let reports = match account::report(&mut self.connection, period, as_of, today, observed_at)
        {
            Ok(reports) => reports,
            Err(StoreError::Calculation(
                error @ (AccountError::MissingOpening | AccountError::MissingSchedule(_)),
            )) => {
                window.set_notice(account_error(error).into());
                return Ok(());
            }
            Err(error) => return Err(store_error(error)),
        };
        let report = &reports.period;
        let (cards, mut metrics) = if self.page == Page::Today {
            let target = report
                .days
                .iter()
                .find(|day| day.date == today)
                .and_then(|day| day.planned_target);
            let target = target.map(i64::from);
            let work = timeline.work_seconds;
            window.set_progress(
                target
                    .filter(|&value| value > 0)
                    .map_or(0.0, |value| (work as f32 / value as f32).min(1.0)),
            );
            window.set_progress_label(
                target
                    .map_or_else(String::new, |value| {
                        format!("Tagessoll {}", duration(value, false))
                    })
                    .into(),
            );
            (
                today_cards(work, timeline.break_seconds, target),
                vec![metric(
                    "Wochendifferenz bis heute",
                    report.totals.delta,
                    true,
                )],
            )
        } else {
            (
                vec![
                    metric("Ist netto", report.totals.actual, false),
                    metric("Soll bis Stichtag", report.totals.target, false),
                    metric("Differenz", report.totals.delta, true),
                ],
                vec![
                    optional_metric("Anfangssaldo", report.opening_balance),
                    metric("Volles Plansoll", report.full_planned_target, false),
                    metric(
                        "Verbleibendes Plansoll",
                        report.remaining_planned_target,
                        false,
                    ),
                    metric("Manuelle Korrekturen", report.totals.adjustments, true),
                    optional_metric("Schlusssaldo bis Stichtag", report.closing_balance),
                ],
            )
        };
        metrics.push(optional_metric(
            &format!("Gesamtsaldo am {}", display_date(as_of)),
            reports.balance_as_of,
        ));
        if self.page == Page::Times && range != Range::Day {
            window.set_rows(model(
                report
                    .days
                    .iter()
                    .map(|day| DayRow {
                        date: format!(
                            "{} {}",
                            WEEKDAYS_SHORT[day.date.weekday().num_days_from_monday() as usize],
                            display_date(day.date)
                        )
                        .into(),
                        actual: day
                            .observed
                            .map_or_else(dash, |totals| duration(totals.actual, false))
                            .into(),
                        target: day
                            .planned_target
                            .map_or_else(dash, |target| duration(i64::from(target), false))
                            .into(),
                        delta: day
                            .observed
                            .map_or_else(dash, |totals| duration(totals.delta, true))
                            .into(),
                    })
                    .collect(),
            ));
        }

        let mut notice = if as_of < period.start() {
            "Zukünftiger Zeitraum. Angezeigt wird das Plansoll.".to_owned()
        } else if report.provisional || reports.balance_provisional {
            format!("Vorläufig · Stand {}", display_date(as_of))
        } else {
            String::new()
        };
        if report.excludes_open_time || reports.balance_excludes_open_time {
            notice.push_str(" Offene Arbeitssegmente sind bis zur Klärung ausgenommen.");
        }
        window.set_notice(notice.trim().into());
        window.set_cards(model(cards));
        window.set_metrics(model(metrics));
        window.set_has_report(true);
        self.displayed_report = Some((as_of, reports));
        Ok(())
    }

    fn render_settings(&mut self, window: &AccountWindow) -> Result<(), String> {
        let settings = account::settings(&mut self.connection).map_err(store_error)?;
        let opening = settings.opening.map_or_else(
            || "Startsaldo: noch nicht festgelegt".to_owned(),
            |opening| {
                format!(
                    "Startsaldo ab {} · {}",
                    display_date(opening.effective_date),
                    duration(opening.signed_seconds, true)
                )
            },
        );
        let mut lines = vec![opening.into()];
        if settings.schedules.is_empty() {
            lines.push("Wochenplan: noch nicht festgelegt".into());
        }
        for (date, schedule) in settings.schedules {
            lines.push(
                format!(
                    "Wochenplan ab {} · {} · Mo–So: {}",
                    display_date(date),
                    duration(i64::from(schedule.weekly_seconds()), false),
                    allocations(schedule.weekdays())
                )
                .into(),
            );
        }
        for target in settings.overrides {
            lines.push(
                format!(
                    "Tagessoll am {} · {} · {}",
                    display_date(target.date),
                    duration(i64::from(target.seconds), false),
                    target.reason
                )
                .into(),
            );
        }
        for entry in settings.adjustments {
            let status = if entry.voided { "storniert" } else { "aktiv" };
            lines.push(
                format!(
                    "Korrektur Nr. {} · {} · {} · {} · {status}",
                    entry.id,
                    display_date(entry.adjustment.effective_date),
                    duration(entry.adjustment.signed_seconds, true),
                    entry.reason
                )
                .into(),
            );
        }
        window.set_settings(model(lines));
        let history = account::audit_history(&self.connection).map_err(store_error)?;
        let mut events = Vec::new();
        for event in history {
            let time = DateTime::from_timestamp_millis(event.occurred_at_utc_ms)
                .map(|time| {
                    time.with_timezone(&self.zone)
                        .format("%d.%m.%Y %H:%M:%S")
                        .to_string()
                })
                .unwrap_or_else(dash);
            let name = match event.entity.as_str() {
                "opening" => "Startsaldo",
                "schedule" => "Wochenplan",
                "target" => "Tagessoll",
                _ => "Korrektur",
            };
            let verb = match event.action.as_str() {
                "add" => "erfasst",
                "edit" => "bearbeitet",
                "clear" => "zurückgesetzt",
                _ => "storniert",
            };
            let key = match event.entity.as_str() {
                "opening" => String::new(),
                "adjustment" => format!(" · Nr. {}", event.key),
                _ => parse_date(&event.key)
                    .map(|date| format!(" · {}", display_date(date)))
                    .unwrap_or_default(),
            };
            let reason = event
                .reason
                .map_or_else(String::new, |reason| format!(" · {reason}"));
            events.push(
                format!(
                    "{time} · {name} {verb}{key}{reason}\nVorher: {}\nNachher: {}",
                    audit_value(&event.entity, event.before_json.as_deref()),
                    audit_value(&event.entity, event.after_json.as_deref())
                )
                .into(),
            );
        }
        if events.is_empty() {
            events.push("Noch keine Änderungen.".into());
        }
        window.set_history(model(events));
        Ok(())
    }

    fn preview(&mut self, window: &AccountWindow, change: Change) -> Result<(), String> {
        let observed_at = now();
        let today = local_date(observed_at, self.zone);
        let description = match &change {
            Change::Schedule { schedule, .. } => format!(
                "Wochenplan: {} · Mo–So: {}",
                duration(i64::from(schedule.weekly_seconds()), false),
                allocations(schedule.weekdays())
            ),
            Change::Opening(opening) => {
                format!("Startsaldo: {}", duration(opening.signed_seconds, true))
            }
            Change::SetTarget {
                seconds, reason, ..
            } => format!(
                "Tagessoll: {} · {}",
                duration(i64::from(*seconds), false),
                reason.trim()
            ),
            Change::ClearTarget { reason, .. } => {
                format!("Tagessoll zurücksetzen · {}", reason.trim())
            }
            Change::AddAdjustment { adjustment, reason } => format!(
                "Neue Korrektur: {} · {}",
                duration(adjustment.signed_seconds, true),
                reason.trim()
            ),
            Change::EditAdjustment {
                id,
                adjustment,
                reason,
            } => format!(
                "Korrektur Nr. {id} bearbeiten: {} · {}",
                duration(adjustment.signed_seconds, true),
                reason.trim()
            ),
            Change::VoidAdjustment { id, reason } => format!(
                "Korrektur Nr. {id} aus der Saldenberechnung rückwirkend entfernen · {}",
                reason.trim()
            ),
        };
        let preview = account::preview(&mut self.connection, change, today, observed_at)
            .map_err(store_error)?;
        window.set_preview_text(format!("{description}\nGültig ab / am {}\n\nVorläufiger Gesamtsaldo am {}\nVorher: {}\nNachher: {}\n\nTagessoll am Gültigkeitsdatum: {} → {}",
            display_date(preview.effective_date()), display_date(preview.as_of()),
            impact(preview.before()), impact(preview.after()),
            target(preview.before()), target(preview.after())).into());
        self.pending = Some(preview);
        window.set_preview_ready(true);
        Ok(())
    }

    fn clear_preview(&mut self, window: &AccountWindow) {
        self.pending = None;
        window.set_preview_ready(false);
    }
}

fn audit_value(entity: &str, json: Option<&str>) -> String {
    let Some(json) = json else { return dash() };
    render_audit_value(entity, json)
        .unwrap_or_else(|| "Details konnten nicht gelesen werden.".into())
}

fn render_audit_value(entity: &str, json: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(json).ok()?;
    let date_key = if entity == "target" {
        "date"
    } else {
        "effective_date"
    };
    let date = parse_date(value[date_key].as_str()?).ok()?;
    let summary = match entity {
        "opening" => duration(value["signed_seconds"].as_i64()?, true),
        "schedule" => {
            let total = duration(value["weekly_seconds"].as_i64()?, false);
            let weekdays = value["weekdays"]
                .as_array()?
                .iter()
                .map(serde_json::Value::as_u64)
                .map(|seconds| u32::try_from(seconds?).ok())
                .collect::<Option<Vec<_>>>()?;
            format!(
                "{total} · Mo–So: {}",
                allocations(weekdays.try_into().ok()?)
            )
        }
        "target" => format!(
            "{} · {}",
            duration(value["seconds"].as_i64()?, false),
            value["reason"].as_str()?
        ),
        "adjustment" => {
            let status = if value["voided_at_utc_ms"].is_null() {
                "aktiv"
            } else {
                "storniert"
            };
            format!(
                "{} · {} · {status}",
                duration(value["signed_seconds"].as_i64()?, true),
                value["reason"].as_str()?
            )
        }
        _ => return None,
    };
    Some(format!("{} · {summary}", display_date(date)))
}

fn allocations(values: [u32; 7]) -> String {
    values
        .map(|seconds| duration(i64::from(seconds), false))
        .join(" / ")
}

fn impact(impact: Impact) -> String {
    let value = match impact.balance {
        BalanceState::Unconfigured => "Startsaldo fehlt".into(),
        BalanceState::BeforeOpening => "Vor Kontobeginn".into(),
        BalanceState::MissingSchedule(date) => {
            format!("Wochenplan fehlt am {}", display_date(date))
        }
        BalanceState::Calculated(value) => duration(value, true),
    };
    if impact.provisional {
        format!("{value} (ohne offene Arbeitssegmente)")
    } else {
        value
    }
}

fn target(impact: Impact) -> String {
    impact
        .target_at_effective_date
        .map_or_else(dash, |value| duration(i64::from(value), false))
}

const WEEKDAYS_SHORT: [&str; 7] = ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"];
const WEEKDAYS: [&str; 7] = [
    "Montag",
    "Dienstag",
    "Mittwoch",
    "Donnerstag",
    "Freitag",
    "Samstag",
    "Sonntag",
];
const MONTHS: [&str; 12] = [
    "Januar",
    "Februar",
    "März",
    "April",
    "Mai",
    "Juni",
    "Juli",
    "August",
    "September",
    "Oktober",
    "November",
    "Dezember",
];

fn weekday_date(date: NaiveDate) -> String {
    format!(
        "{}, {}. {}",
        WEEKDAYS[date.weekday().num_days_from_monday() as usize],
        date.day(),
        month_year(date)
    )
}

fn month_year(date: NaiveDate) -> String {
    format!("{} {}", MONTHS[date.month0() as usize], date.year())
}

fn today_cards(work: Seconds, break_time: Seconds, target: Option<Seconds>) -> Vec<Metric> {
    vec![
        metric("Gearbeitet · netto", work, false),
        Metric {
            label: "Rest bis Tagessoll".into(),
            value: target
                .map_or_else(dash, |value| duration((value - work).max(0), false))
                .into(),
        },
        metric("Pausen", break_time, false),
    ]
}

fn theme_error(error: rusqlite::Error) -> String {
    format!("Erscheinungsbild konnte nicht geladen oder gespeichert werden ({error}).")
}

fn metric(label: &str, value: Seconds, signed: bool) -> Metric {
    Metric {
        label: label.into(),
        value: duration(value, signed).into(),
    }
}

fn optional_metric(label: &str, value: Option<Seconds>) -> Metric {
    Metric {
        label: label.into(),
        value: value
            .map_or_else(dash, |value| duration(value, true))
            .into(),
    }
}

fn dash() -> String {
    "–".into()
}

fn model<T: Clone + 'static>(values: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(values))
}

fn now() -> Timestamp {
    SystemTime::now().into()
}

fn local_date(time: Timestamp, zone: Tz) -> NaiveDate {
    time.with_timezone(&zone).date_naive()
}

fn account_error(error: AccountError) -> String {
    match error {
        AccountError::ScheduleDoesNotAddUp => {
            "Die sieben Tagessollwerte müssen zusammen das Wochensoll ergeben.".into()
        }
        AccountError::MissingOpening => {
            "Bitte zuerst den Startsaldo unter Einstellungen erfassen.".into()
        }
        AccountError::MissingSchedule(date) => format!(
            "Am {} fehlt ein Wochenplan. Bitte unter Einstellungen ergänzen.",
            display_date(date)
        ),
        _ => format!("Zeitkonto konnte nicht berechnet werden ({error})."),
    }
}

fn store_error(error: StoreError) -> String {
    match error {
        StoreError::Calculation(error) => account_error(error),
        StoreError::ReasonRequired => "Bitte eine Begründung eingeben.".into(),
        StoreError::AdjustmentUnavailable(id) => {
            format!("Korrektur Nr. {id} fehlt oder wurde bereits storniert.")
        }
        StoreError::OverrideUnavailable(date) => format!(
            "Für den {} besteht kein abweichendes Tagessoll.",
            display_date(date)
        ),
        StoreError::StalePreview => {
            "Das Konto wurde inzwischen geändert. Bitte eine neue Vorschau erstellen.".into()
        }
        _ => format!("Zeitkonto konnte nicht geladen oder gespeichert werden ({error})."),
    }
}
