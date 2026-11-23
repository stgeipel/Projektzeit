//! Standalone desktop entry point for the worktime-account feature.

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
use rusqlite::Connection;
use slint::{ComponentHandle, ModelRc, VecModel};

use input::{display_date, duration, parse_change, parse_date};
use view::{AccountWindow, DayRow, EditFields, Metric};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(i32)]
enum Page {
    Today,
    Week,
    Month,
    Settings,
    About,
}

struct Controller {
    connection: Connection,
    zone: Tz,
    page: Page,
    selected: NaiveDate,
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
    let controller = Rc::new(RefCell::new(Controller {
        connection,
        zone,
        page: Page::Today,
        selected: local_date(now(), zone),
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
            1 => Page::Week,
            2 => Page::Month,
            3 => Page::Settings,
            _ => Page::About,
        };
        state.clear_preview(window);
        state.render(window)
    }));
    window.on_step_period(action(
        window,
        controller,
        |state, window, direction: i32| {
            let today = local_date(now(), state.zone);
            let selected = if state.page == Page::Week {
                let date = state
                    .selected
                    .checked_add_signed(TimeDelta::weeks(i64::from(direction)));
                date.and_then(|date| Period::iso_week(date).ok())
            } else {
                let date = if direction < 0 {
                    state.selected.checked_sub_months(Months::new(1))
                } else {
                    state.selected.checked_add_months(Months::new(1))
                };
                date.and_then(|date| Period::month(date).ok())
            }
            .ok_or_else(|| account_error(AccountError::InvalidDateRange))?;
            state.selected = if (selected.start()..=selected.end()).contains(&today) {
                today
            } else {
                selected.end()
            };
            state.render(window)
        },
    ));
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
    fn render(&mut self, window: &AccountWindow) -> Result<(), String> {
        window.set_page(self.page as i32);
        self.displayed_report = None;
        window.set_has_report(false);
        window.set_exportable(false);
        let observed_at = now();
        let today = local_date(observed_at, self.zone);
        match self.page {
            Page::Settings => {
                window.set_report_date(display_date(today).into());
                self.render_settings(window)
            }
            Page::About => Ok(()),
            Page::Today | Page::Week | Page::Month => {
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
        window.set_report_date(display_date(selected).into());
        let period = if self.page == Page::Month {
            Period::month(selected)
        } else {
            Period::iso_week(selected)
        }
        .map_err(account_error)?;
        let heading = match self.page {
            Page::Today => format!("Heute · {}", display_date(today)),
            Page::Week => format!(
                "KW {} / {} · {} bis {}",
                selected.iso_week().week(),
                selected.iso_week().year(),
                display_date(period.start()),
                display_date(period.end())
            ),
            _ => format!(
                "Monat · {} bis {}",
                display_date(period.start()),
                display_date(period.end())
            ),
        };
        window.set_heading(heading.into());
        window.set_notice("".into());
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
        let daily = report
            .days
            .iter()
            .find(|item| item.date == today)
            .and_then(|item| item.observed);
        let mut metrics = if self.page == Page::Today {
            let mut rows = match daily {
                Some(daily) => vec![
                    metric("Heute gearbeitet", daily.actual, false),
                    metric("Tagessoll", daily.target, false),
                    metric(
                        "Heute noch zu arbeiten",
                        (daily.target - daily.actual).max(0),
                        false,
                    ),
                ],
                None => vec![Metric {
                    label: "Heute".into(),
                    value: "Vor Kontobeginn".into(),
                }],
            };
            rows.push(metric(
                "Wochendifferenz bis heute",
                report.totals.delta,
                true,
            ));
            rows
        } else {
            vec![
                optional_metric("Anfangssaldo", report.opening_balance),
                metric("Ist netto bis Stichtag", report.totals.actual, false),
                metric("Soll bis Stichtag", report.totals.target, false),
                metric("Volles Plansoll", report.full_planned_target, false),
                metric(
                    "Verbleibendes Plansoll nach Stichtag",
                    report.remaining_planned_target,
                    false,
                ),
                metric("Differenz bis Stichtag", report.totals.delta, true),
                metric("Manuelle Korrekturen", report.totals.adjustments, true),
                optional_metric("Schlusssaldo bis Stichtag", report.closing_balance),
            ]
        };
        metrics.push(optional_metric(
            &format!("Gesamtsaldo am {}", display_date(as_of)),
            reports.balance_as_of,
        ));
        let days = if self.page == Page::Week {
            report
                .days
                .iter()
                .map(|day| DayRow {
                    date: format!(
                        "{} {}",
                        ["Mo", "Di", "Mi", "Do", "Fr", "Sa", "So"]
                            [day.date.weekday().num_days_from_monday() as usize],
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
                .collect()
        } else {
            Vec::new()
        };
        let mut notice = if report.provisional || reports.balance_provisional {
            if as_of < report.period.start() {
                "Zukünftiger Zeitraum. Angezeigt wird das Plansoll.".to_owned()
            } else {
                format!(
                    "Vorläufig bis {}. Das Soll nach dem Stichtag steht separat im Plansoll.",
                    display_date(as_of)
                )
            }
        } else if self.page == Page::Month {
            "Der Anfangssaldo entspricht dem Übertrag aus dem Vormonat oder dem Startsaldo bei Kontobeginn.".to_owned()
        } else {
            String::new()
        };
        if report.excludes_open_time || reports.balance_excludes_open_time {
            notice.push_str(" Offene Arbeitssegmente sind bis zur Klärung ausgenommen.");
        }
        window.set_notice(notice.trim().into());
        window.set_metrics(model(metrics));
        window.set_rows(model(days));
        window.set_has_report(true);
        window.set_exportable(self.page != Page::Today);
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
