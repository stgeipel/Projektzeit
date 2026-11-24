//! Desktop presentation of worktime-account reports and recorded time.

mod dashboard;

mod data;
mod export;
mod input;
mod preferences;
mod template;
mod tracking;
mod window;
// Slint generates the window bindings in this module.
#[allow(unsafe_code)]
mod view;

use std::{
    cell::RefCell,
    ffi::OsStr,
    path::PathBuf,
    rc::Rc,
    time::{Duration, SystemTime},
};

use chrono::{DateTime, Datelike, Months, NaiveDate, TimeDelta, Timelike};
use chrono_tz::Tz;
use projektzeit_core::{
    Timestamp,
    worktime_account::{AccountError, Period, Seconds},
};
use projektzeit_storage::worktime_account::{
    self as account, BalanceState, Change, Impact, Preview, Reports, StoreError,
};
use rusqlite::Connection;
use slint::{CloseRequestResponse, ComponentHandle, ModelRc, Timer, TimerMode, VecModel};

use input::{display_date, duration, parse_change, parse_date};
use preferences::{APPEARANCE_KEY, MiniPreferences, Point, ScheduleDraft};
use view::{
    AccountWindow, AdjustmentRow, DashboardBalance, DayRow, EditFields, Metric, MiniOptions,
    MiniTimer, ScheduleInputs,
};
use window::{Visibility, WindowAction, WindowState};

#[derive(Clone, Copy, Eq, PartialEq)]
#[repr(i32)]
enum Page {
    Today,
    Times,
    Account,
    Settings,
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

const THEMES: [&str; 3] = ["system", "light", "dark"];

struct Controller {
    connection: Connection,
    database_path: PathBuf,
    tracking: tracking::Tracking,
    zone: Tz,
    page: Page,
    selected: NaiveDate,
    range: Range,
    dashboard_range: Range,
    pending: Option<Preview>,
    displayed_report: Option<(NaiveDate, Reports)>,
    scheme: i32,
    schedule: ScheduleDraft,
    dated_draft: EditFields,
    schedule_save: Timer,
    mini: Option<MiniTimer>,
    mini_preferences: MiniPreferences,
    mini_state: WindowState,
    saved_position: Option<Point>,
    placed_position: Option<Point>,
    placing_mini: bool,
    position_save: Timer,
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
    let connection = projektzeit_storage::open(&path)
        .map_err(|error| format!("Datenbank konnte nicht geöffnet werden ({error})."))?;
    window::initialize()
        .map_err(|error| format!("Fenstersystem konnte nicht gestartet werden ({error})."))?;
    let window = AccountWindow::new()
        .map_err(|error| format!("Fenster konnte nicht geöffnet werden ({error})."))?;
    let stored_theme = preferences::get(&connection, APPEARANCE_KEY).map_err(preference_error)?;
    let scheme = THEMES
        .iter()
        .position(|&theme| Some(theme) == stored_theme.as_deref())
        .unwrap_or_default() as i32;
    let mini_preferences = MiniPreferences::load(&connection).map_err(preference_error)?;
    let saved_position = preferences::load_position(&connection).map_err(preference_error)?;
    let today = local_date(now(), zone);
    let schedule = ScheduleDraft::new(
        today,
        account::schedule_at(&connection, today).map_err(store_error)?,
    );
    let tracking = tracking::Tracking::load(&connection, now())?;
    window.set_database_path(path.display().to_string().into());
    data::initialize(&window);
    let controller = Rc::new(RefCell::new(Controller {
        connection,
        database_path: path,
        tracking,
        zone,
        page: Page::Today,
        selected: today,
        range: Range::default(),
        dashboard_range: Range::Day,
        pending: None,
        displayed_report: None,
        scheme,
        schedule,
        dated_draft: EditFields {
            date: display_date(today).into(),
            monday: "00:00".into(),
            tuesday: "00:00".into(),
            wednesday: "00:00".into(),
            thursday: "00:00".into(),
            friday: "00:00".into(),
            saturday: "00:00".into(),
            sunday: "00:00".into(),
            ..Default::default()
        },
        schedule_save: Timer::default(),
        mini: None,
        mini_preferences,
        mini_state: WindowState::default(),
        saved_position,
        placed_position: None,
        placing_mini: false,
        position_save: Timer::default(),
    }));
    connect(&window, &controller);
    controller.borrow_mut().render(&window)?;
    window
        .run()
        .map_err(|error| format!("Fenster konnte nicht ausgeführt werden ({error})."))
}

fn connect(window: &AccountWindow, controller: &Rc<RefCell<Controller>>) {
    window.on_navigate(action(window, controller, |state, window, page| {
        state.schedule_save.stop();
        let saved = state.save_schedule(window);
        state.page = match page {
            0 => Page::Today,
            1 => Page::Times,
            2 => Page::Account,
            _ => Page::Settings,
        };
        state.clear_preview(window);
        state.render(window)?;
        saved
    }));
    connect_desktop(window, controller);
    tracking::connect(window, controller);
    data::connect(window, controller);
    window.on_select_range(action(window, controller, |state, window, index| {
        state.range = match index {
            0 => Range::Day,
            2 => Range::Month,
            _ => Range::Week,
        };
        state.render(window)
    }));
    window.on_select_dashboard_range(action(window, controller, |state, window, index| {
        state.dashboard_range = match index {
            1 => Range::Week,
            2 => Range::Month,
            _ => Range::Day,
        };
        let at = now();
        state.render_today(window, local_date(at, state.zone), at)
    }));
    let browse_today = action(window, controller, |state, window, ()| {
        state.selected = local_date(now(), state.zone);
        state.range = Range::Day;
        state.page = Page::Times;
        state.render(window)
    });
    window.on_browse_today(move || browse_today(()));
    window.on_change_scheme(action(window, controller, |state, window, index: i32| {
        let value = THEMES
            .get(index as usize)
            .ok_or_else(|| "Unbekanntes Erscheinungsbild.".to_owned())?;
        preferences::put(&state.connection, APPEARANCE_KEY, value).map_err(preference_error)?;
        state.scheme = index;
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
        if !state.schedule.dirty {
            let today = local_date(now(), state.zone);
            state.schedule = ScheduleDraft::new(
                today,
                account::schedule_at(&state.connection, today).map_err(store_error)?,
            );
        }
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
    let save_export = action(
        window,
        controller,
        |state, window, (path, template): (slint::SharedString, slint::SharedString)| {
            let path = PathBuf::from(path.trim());
            if state.page == Page::Times {
                let rows = state.selected_slices(now())?;
                let period = state.range.period(state.selected).map_err(account_error)?;
                export::save_times(&path, template.trim(), period, &rows)?;
                window.set_feedback("Zeiten exportiert.".into());
            } else {
                let (date, report) = state
                    .displayed_report
                    .as_ref()
                    .ok_or_else(|| "Kein Bericht zum Exportieren vorhanden.".to_owned())?;
                export::save_account(&path, report, *date)?;
                window.set_feedback("Arbeitszeitkonto exportiert.".into());
            }
            Ok(())
        },
    );
    window.on_export_to(move |path, template| save_export((path, template)));
}

fn connect_desktop(window: &AccountWindow, controller: &Rc<RefCell<Controller>>) {
    let show_mini = action(window, controller, |state, window, ()| {
        state.show_mini(window, Visibility::Manual)
    });
    window.on_show_mini(move || show_mini(()));
    window.on_native_minimized(action(window, controller, |state, window, minimized| {
        let change = state
            .mini_state
            .minimize_changed(minimized, state.mini_preferences.auto_show);
        state.apply_window_action(window, change)
    }));
    let interrupted = action(window, controller, |state, window, ()| {
        state.clock_interrupted(window)
    });
    window.on_native_clock_interrupted(move || interrupted(()));
    window::observe_main(
        window.window(),
        {
            let main = window.as_weak();
            move |minimized| {
                if let Some(window) = main.upgrade() {
                    window.invoke_native_minimized(minimized);
                }
            }
        },
        {
            let main = window.as_weak();
            move || {
                if let Some(window) = main.upgrade() {
                    window.invoke_native_clock_interrupted();
                }
            }
        },
    );

    let moved = action(window, controller, {
        let controller = Rc::downgrade(controller);
        move |state, window, ()| {
            if state.placing_mini {
                state.place_mini(window);
            } else if state.mini_state.visibility != Visibility::Hidden
                && state.mini_preferences.remember
            {
                let controller = controller.clone();
                state.position_save.start(
                    TimerMode::SingleShot,
                    Duration::from_millis(300),
                    move || {
                        if let Some(controller) = controller.upgrade() {
                            let _ = controller.borrow_mut().remember_mini_position();
                        }
                    },
                );
            }
            Ok(())
        }
    });
    window.on_mini_moved(move || moved(()));
    let closed = action(window, controller, |state, _, ()| {
        state.position_save.stop();
        let result = state.remember_mini_position();
        state.mini_state.visibility = Visibility::Hidden;
        state.placing_mini = false;
        result
    });
    window.on_mini_closed(move || closed(()));
    let change_mini_option = action(window, controller, |state, window, (index, value)| {
        state.set_mini_option(window, index, value)
    });
    window.on_change_mini_option(move |index, value| change_mini_option((index, value)));
    let reset = action(window, controller, |state, window, ()| {
        preferences::save_position(&state.connection, None).map_err(preference_error)?;
        state.saved_position = None;
        state.position_save.stop();
        if state.mini_state.visibility != Visibility::Hidden {
            state.place_mini(window);
        }
        window.set_feedback("Fensterposition zurückgesetzt.".into());
        Ok(())
    });
    window.on_reset_mini_position(move || reset(()));

    let edit = action(window, controller, {
        let controller = Rc::downgrade(controller);
        move |state, window, (index, value): (i32, slint::SharedString)| {
            state.schedule_save.stop();
            state.schedule.edit(index as usize, value.to_string());
            state.show_schedule(window);
            if state.schedule.candidate.is_some() {
                let controller = controller.clone();
                let main = window.as_weak();
                state.schedule_save.start(
                    TimerMode::SingleShot,
                    Duration::from_millis(500),
                    move || {
                        if let (Some(controller), Some(window)) =
                            (controller.upgrade(), main.upgrade())
                        {
                            let mut state = controller.borrow_mut();
                            if let Err(error) = state
                                .save_schedule(&window)
                                .and_then(|()| state.render(&window))
                            {
                                window.set_feedback(error.into());
                            }
                        }
                    },
                );
            }
            Ok(())
        }
    });
    window.on_edit_schedule(move |index, value| edit((index, value)));
    window.on_capture_dated(action(
        window,
        controller,
        |state, window, fields: EditFields| {
            window.set_dated_draft(fields.clone());
            state.dated_draft = fields;
            Ok(())
        },
    ));

    let state = Rc::downgrade(controller);
    let main = window.as_weak();
    window.window().on_close_requested(move || {
        if let (Some(state), Some(window)) = (state.upgrade(), main.upgrade()) {
            let mut state = state.borrow_mut();
            state.schedule_save.stop();
            if let Err(error) = state.save_schedule(&window) {
                window.set_feedback(error.into());
                return CloseRequestResponse::KeepWindowShown;
            }
            if let Err(error) = state
                .checkpoint()
                .and_then(|()| state.remember_mini_position())
            {
                window.set_feedback(error.into());
                return CloseRequestResponse::KeepWindowShown;
            }
        }
        CloseRequestResponse::HideWindow
    });
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
            let mut controller = controller.borrow_mut();
            if let Some(mini) = &controller.mini {
                mini.set_feedback("".into());
            }
            if let Err(error) = perform(&mut controller, &window, argument) {
                window.set_feedback(error.clone().into());
                if let Some(mini) = &controller.mini {
                    mini.set_feedback(error.into());
                }
            }
        }
    }
}

impl Controller {
    fn show_schedule(&self, window: &AccountWindow) {
        window.set_schedule(ScheduleInputs {
            values: model(
                self.schedule
                    .input
                    .iter()
                    .map(|text| text.as_str().into())
                    .collect(),
            ),
            date: display_date(self.schedule.date).into(),
            message: self.schedule.message.as_str().into(),
            invalid: self.schedule.invalid,
        });
    }

    fn save_schedule(&mut self, window: &AccountWindow) -> Result<(), String> {
        let Some(schedule) = self.schedule.candidate else {
            return Ok(());
        };
        let observed_at = now();
        let today = local_date(observed_at, self.zone);
        if let Err(error) =
            account::save_current_schedule(&mut self.connection, today, schedule, observed_at)
        {
            let message = format!(
                "Arbeitszeit konnte nicht gespeichert werden. Eingaben bleiben erhalten. ({error})"
            );
            self.schedule.failure(message.clone());
            self.show_schedule(window);
            return Err(message);
        }
        self.schedule.saved(today);
        self.show_schedule(window);
        Ok(())
    }

    fn show_mini(&mut self, main: &AccountWindow, visibility: Visibility) -> Result<(), String> {
        if self.mini.is_none() {
            let mini = window::create_mini(|| {
                let mini = MiniTimer::new()?;
                let _ = mini.window();
                Ok::<_, slint::PlatformError>(mini)
            })
            .map_err(window_error)?;
            let weak = main.as_weak();
            mini.on_open_main(move || {
                if let Some(main) = weak.upgrade() {
                    let _ = main.show();
                    window::restore_main(main.window());
                }
            });
            tracking::connect_mini(&mini, main);
            let weak = main.as_weak();
            mini.on_set_pin(move |pinned| {
                if let Some(main) = weak.upgrade() {
                    main.invoke_change_mini_option(1, pinned);
                }
            });
            let weak = main.as_weak();
            mini.window().on_close_requested(move || {
                if let Some(main) = weak.upgrade() {
                    main.invoke_mini_closed();
                }
                CloseRequestResponse::HideWindow
            });
            let weak = main.as_weak();
            window::observe_mini(mini.window(), move || {
                if let Some(main) = weak.upgrade() {
                    main.invoke_mini_moved();
                }
            });
            self.mini = Some(mini);
        }
        self.sync_tracking(main, now())?;
        let mini = self
            .mini
            .as_ref()
            .expect("the mini-timer was created above");
        mini.set_pinned(self.mini_preferences.topmost);
        if !mini.window().is_visible() {
            if visibility == Visibility::Automatic {
                window::show_without_activation(mini.window(), || mini.show())
                    .map_err(window_error)?;
            } else {
                mini.show().map_err(window_error)?;
            }
            self.mini_state.visibility = visibility;
            self.place_mini(main);
        } else if visibility == Visibility::Manual {
            self.mini_state.visibility = visibility;
            mini.window().set_minimized(false);
        }
        Ok(())
    }

    fn place_mini(&mut self, main: &AccountWindow) {
        let Some(mini) = &self.mini else { return };
        self.placed_position = window::place(
            main.window(),
            mini.window(),
            self.saved_position
                .filter(|_| self.mini_preferences.remember),
            self.mini_preferences.margin,
        );
        self.placing_mini = self.placed_position.is_none();
    }

    fn remember_mini_position(&mut self) -> Result<(), String> {
        if self.mini_state.visibility == Visibility::Hidden
            || self.placing_mini
            || !self.mini_preferences.remember
        {
            return Ok(());
        }
        if let Some(mini) = &self.mini {
            let point = window::position(mini.window());
            if self.placed_position != Some(point) && self.saved_position != Some(point) {
                preferences::save_position(&self.connection, Some(point))
                    .map_err(preference_error)?;
                self.saved_position = Some(point);
            }
        }
        Ok(())
    }

    fn apply_window_action(
        &mut self,
        main: &AccountWindow,
        action: WindowAction,
    ) -> Result<(), String> {
        match action {
            WindowAction::ShowAutomatically => self.show_mini(main, Visibility::Automatic),
            WindowAction::Hide => {
                self.position_save.stop();
                let saved = self.remember_mini_position();
                if let Some(mini) = &self.mini {
                    mini.hide().map_err(window_error)?;
                }
                self.mini_state.visibility = Visibility::Hidden;
                self.placing_mini = false;
                saved
            }
            WindowAction::Keep => Ok(()),
        }
    }

    fn set_mini_option(
        &mut self,
        main: &AccountWindow,
        index: i32,
        value: bool,
    ) -> Result<(), String> {
        let mut preferences = self.mini_preferences;
        match index {
            0 => preferences.auto_show = value,
            1 => preferences.topmost = value,
            2 => preferences.remember = value,
            _ => return Ok(()),
        }
        let transaction = self.connection.transaction().map_err(preference_error)?;
        preferences.save(&transaction).map_err(preference_error)?;
        if !preferences.remember {
            preferences::save_position(&transaction, None).map_err(preference_error)?;
        }
        transaction.commit().map_err(preference_error)?;
        self.mini_preferences = preferences;
        if !preferences.remember {
            self.saved_position = None;
        }
        self.show_mini_options(main);
        if let Some(mini) = &self.mini {
            mini.set_pinned(preferences.topmost);
        }
        if index == 0 {
            self.apply_window_action(main, self.mini_state.preference_changed(value))?;
        }
        main.set_feedback("Mini-Timer-Einstellungen gespeichert.".into());
        Ok(())
    }

    fn show_mini_options(&self, window: &AccountWindow) {
        window.set_mini_options(MiniOptions {
            auto_show: self.mini_preferences.auto_show,
            topmost: self.mini_preferences.topmost,
            remember: self.mini_preferences.remember,
        });
    }

    fn report_range(&self) -> Range {
        match self.page {
            Page::Times => self.range,
            Page::Account => Range::Month,
            _ => Range::Week,
        }
    }

    fn render(&mut self, window: &AccountWindow) -> Result<(), String> {
        window.set_page(self.page as i32);
        window.set_scheme(self.scheme);
        self.show_mini_options(window);
        window.set_range_index(match self.range {
            Range::Day => 0,
            Range::Week => 1,
            Range::Month => 2,
        });
        window.set_show_entry_form(self.tracking.editor.is_some());
        self.displayed_report = None;
        window.set_has_report(false);
        window.set_notice("".into());
        window.set_subheading("".into());
        let observed_at = now();
        let today = local_date(observed_at, self.zone);
        self.refresh_pending()?;
        self.sync_tracking(window, observed_at)?;
        match self.page {
            Page::Settings => {
                window.set_heading("Einstellungen".into());
                window.set_report_date(display_date(today).into());
                if !self.schedule.dirty && self.schedule.date != today {
                    self.schedule = ScheduleDraft::new(
                        today,
                        account::schedule_at(&self.connection, today).map_err(store_error)?,
                    );
                }
                self.show_schedule(window);
                window.set_dated_draft(self.dated_draft.clone());
                self.show_projects(window);
                self.render_settings(window)
            }
            Page::Today => {
                window.set_heading(greeting(observed_at.with_timezone(&self.zone).hour()).into());
                window.set_subheading(weekday_date(today).into());
                self.render_today(window, today, observed_at)
            }
            Page::Times => self.render_times(window, today, observed_at),
            Page::Account => {
                window.set_heading("Arbeitszeitkonto".into());
                window.set_subheading(month_year(self.selected).into());
                window.set_report_date(display_date(self.selected).into());
                self.render_settings(window)?;
                self.render_account(window, today, observed_at)
            }
        }
    }

    fn report(
        &mut self,
        period: Period,
        selected: NaiveDate,
        today: NaiveDate,
        observed_at: Timestamp,
    ) -> Result<Reports, StoreError> {
        account::report_live(
            &mut self.connection,
            period,
            selected.min(today),
            today,
            observed_at,
            self.tracking.trusted(),
        )
    }

    fn render_today(
        &mut self,
        window: &AccountWindow,
        today: NaiveDate,
        at: Timestamp,
    ) -> Result<(), String> {
        let week = optional_report(self.report(
            Period::iso_week(today).map_err(account_error)?,
            today,
            today,
            at,
        ))?;
        let month = optional_report(self.report(
            Period::month(today).map_err(account_error)?,
            today,
            today,
            at,
        ))?;
        window.set_dashboard_date(display_date(today).into());
        let (index, report, period) = match self.dashboard_range {
            Range::Day => (0, None, String::new()),
            Range::Week => (
                1,
                week.as_ref(),
                format!("Kalenderwoche {}", today.iso_week().week()),
            ),
            Range::Month => (2, month.as_ref(), month_year(today)),
        };
        window.set_dashboard_range(index);
        window.set_dashboard_balance(DashboardBalance {
            due: report
                .map_or_else(dash, |report| {
                    duration(report.period.finalized_totals.target, false)
                })
                .into(),
            actual: report
                .map_or_else(dash, |report| duration(report.period.totals.actual, false))
                .into(),
            delta: report
                .map_or_else(dash, |report| {
                    duration(report.period.finalized_totals.delta, true)
                })
                .into(),
            period: period.into(),
        });
        let balances = if week.is_some() || month.is_some() {
            vec![
                optional_metric(
                    "Diese Woche",
                    week.as_ref().map(|week| week.period.finalized_totals.delta),
                ),
                optional_metric(
                    "Dieser Monat",
                    month
                        .as_ref()
                        .map(|month| month.period.finalized_totals.delta),
                ),
                optional_metric(
                    "Gesamtsaldo",
                    week.or(month).and_then(|report| report.balance_as_of),
                ),
            ]
        } else {
            Vec::new()
        };
        window.set_summary(model(balances));
        Ok(())
    }

    fn render_times(
        &mut self,
        window: &AccountWindow,
        today: NaiveDate,
        at: Timestamp,
    ) -> Result<(), String> {
        window.set_heading("Zeiten".into());
        window.set_report_date(display_date(self.selected).into());
        let period = self.range.period(self.selected).map_err(account_error)?;
        window.set_subheading(
            match self.range {
                Range::Day => weekday_date(self.selected),
                Range::Week => format!(
                    "Kalenderwoche {} · {} – {}",
                    self.selected.iso_week().week(),
                    display_date(period.start()),
                    display_date(period.end())
                ),
                Range::Month => month_year(self.selected),
            }
            .into(),
        );
        self.show_filters(window);
        self.record_rows(window, at)?;
        let mut rows = Vec::new();
        if self.range != Range::Day {
            let mut daily = std::collections::BTreeMap::<NaiveDate, i64>::new();
            for slice in self.selected_slices(at)? {
                if slice.counted && slice.entry.kind == projektzeit_storage::tracker::Kind::Work {
                    *daily.entry(slice.date).or_default() +=
                        (slice.end - slice.start).num_milliseconds();
                }
            }
            let report = optional_report(self.report(period, self.selected, today, at))?;
            for date in period
                .start()
                .iter_days()
                .take_while(|date| *date <= period.end())
            {
                let day = report
                    .as_ref()
                    .and_then(|report| report.period.days.iter().find(|day| day.date == date));
                let target = account::daily_target(&self.connection, date).map_err(store_error)?;
                rows.push(DayRow {
                    date: display_date(date).into(),
                    actual: if date > today {
                        dash()
                    } else {
                        duration(daily.get(&date).copied().unwrap_or_default() / 1000, false)
                    }
                    .into(),
                    target: target
                        .map_or_else(dash, |target| duration(i64::from(target), false))
                        .into(),
                    delta: day
                        .filter(|day| day.finalized && !self.tracking.filtered())
                        .and_then(|day| day.observed)
                        .map_or_else(dash, |totals| duration(totals.delta, true))
                        .into(),
                });
            }
        }
        window.set_rows(model(rows));
        Ok(())
    }

    fn render_account(
        &mut self,
        window: &AccountWindow,
        today: NaiveDate,
        at: Timestamp,
    ) -> Result<(), String> {
        window.set_week_metrics(model(Vec::new()));
        window.set_month_metrics(model(Vec::new()));
        let selected = self.selected;
        let week = self.report(
            Period::iso_week(selected).map_err(account_error)?,
            selected,
            today,
            at,
        );
        let month = self.report(
            Period::month(selected).map_err(account_error)?,
            selected,
            today,
            at,
        );
        let week = optional_report(week)?;
        let month = match month {
            Ok(report) => report,
            Err(StoreError::Calculation(
                error @ (AccountError::MissingOpening | AccountError::MissingSchedule(_)),
            )) => {
                window.set_notice(account_error(error).into());
                if let Some(week) = week {
                    window.set_week_metrics(model(period_metrics(&week.period)));
                }
                return Ok(());
            }
            Err(error) => return Err(store_error(error)),
        };
        if let Some(week) = week {
            window.set_week_metrics(model(period_metrics(&week.period)));
        }
        let mut metrics = period_metrics(&month.period);
        metrics.push(optional_metric(
            "Übertrag aus dem Vormonat",
            month.period.opening_balance,
        ));
        metrics.push(metric(
            "Manuelle Saldoanpassungen",
            month.period.finalized_totals.adjustments,
            true,
        ));
        metrics.push(optional_metric("Gesamtsaldo", month.balance_as_of));
        window.set_month_metrics(model(metrics));
        if month.period.excludes_open_time || month.balance_excludes_open_time {
            window.set_notice("Die offene Erfassung wartet noch auf Bestätigung.".into());
        }
        window.set_has_report(true);
        self.displayed_report = Some((selected.min(today), month));
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
        window.set_opening_summary(opening.as_str().into());
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
        let adjustments = settings
            .adjustments
            .iter()
            .map(|entry| AdjustmentRow {
                id: entry.id.to_string().into(),
                date: display_date(entry.adjustment.effective_date).into(),
                reason: entry.reason.clone().into(),
                amount: duration(entry.adjustment.signed_seconds, true).into(),
                voided: entry.voided,
            })
            .collect();
        window.set_adjustments(model(adjustments));
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
            if (self.page == Page::Account)
                != matches!(event.entity.as_str(), "opening" | "adjustment")
            {
                continue;
            }
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
        let preview = account::preview_on(&mut self.connection, change, today, today, observed_at)
            .map_err(store_error)?;
        window.set_preview_text(format!("{description}\nGültig ab / am {}\n\nAbgeschlossener Gesamtsaldo am {}\nVorher: {}\nNachher: {}\n\nTagessoll am Gültigkeitsdatum: {} → {}",
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

fn greeting(hour: u32) -> &'static str {
    match hour {
        5..=11 => "Guten Morgen",
        12..=17 => "Guten Tag",
        _ => "Guten Abend",
    }
}

fn optional_report(result: Result<Reports, StoreError>) -> Result<Option<Reports>, String> {
    match result {
        Ok(report) => Ok(Some(report)),
        Err(StoreError::Calculation(
            AccountError::MissingOpening | AccountError::MissingSchedule(_),
        )) => Ok(None),
        Err(error) => Err(store_error(error)),
    }
}

fn period_metrics(report: &projektzeit_core::worktime_account::PeriodReport) -> Vec<Metric> {
    vec![
        metric("Ist netto bis Stichtag", report.totals.actual, false),
        metric("Fälliges Soll", report.finalized_totals.target, false),
        metric("Volles Plansoll", report.full_planned_target, false),
        metric("Abgeschlossener Saldo", report.finalized_totals.delta, true),
    ]
}

fn preference_error(error: rusqlite::Error) -> String {
    format!("Einstellungen konnten nicht geladen oder gespeichert werden ({error}).")
}

fn window_error(error: slint::PlatformError) -> String {
    format!("Das Mini-Timer-Fenster konnte nicht geändert werden ({error}).")
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
            "Bitte zuerst den Startsaldo im Arbeitszeitkonto erfassen.".into()
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
