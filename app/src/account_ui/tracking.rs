//! Shared desktop commands for the dashboard, mini-timer and recorded-time editor.

use chrono::{NaiveTime, TimeDelta, TimeZone, Timelike};
use chrono_tz::Tz;
use projektzeit_core::Timestamp;
use projektzeit_storage::tracker::{self, Command, EntryInput, Error, Kind, Project, Recovery};
use slint::{Color, ComponentHandle, ModelRc, SharedString, Timer, TimerMode};
use std::{cell::RefCell, rc::Rc, time::Duration};

use super::{
    Controller, Page, Range, action, dash, dashboard, display_date, duration, local_date, model,
    now, parse_date,
    view::{AccountWindow, EntryForm, MiniTimer, ProjectChoice, ProjectForm, TrackingView},
};

pub(super) struct Tracking {
    trusted: Option<i64>,
    selection: Option<i64>,
    pending_activity: Option<i64>,
    pending_count: usize,
    projects: Vec<Project>,
    selector_names: ModelRc<SharedString>,
    filter: Option<String>,
    pub editor: Option<EntryEditor>,
    timer: Timer,
    last_pulse: Timestamp,
}

impl Tracking {
    pub fn load(db: &rusqlite::Connection, now: Timestamp) -> Result<Self, String> {
        let projects = tracker::projects(db).map_err(tracking_error)?;
        let selector_names = project_names(&projects);
        let pending = tracker::pending_descriptions(db).map_err(tracking_error)?;
        Ok(Self {
            trusted: None,
            selection: tracker::last_project(db).map_err(tracking_error)?,
            pending_activity: None,
            pending_count: pending.len(),
            projects,
            selector_names,
            filter: None,
            editor: None,
            timer: Timer::default(),
            last_pulse: now,
        })
    }

    fn expire_trust(&mut self, at: Timestamp) -> bool {
        let gap = at - self.last_pulse;
        if self.trusted.is_some() && !(TimeDelta::zero()..=TimeDelta::minutes(2)).contains(&gap) {
            self.trusted = None;
            return true;
        }
        false
    }

    pub fn trusted(&self) -> Option<i64> {
        self.trusted
    }
    pub fn filtered(&self) -> bool {
        self.filter.is_some()
    }
}

pub(super) struct EntryEditor {
    id: Option<i64>,
    day: Option<i64>,
    zone: Tz,
    original: Option<(Timestamp, Timestamp)>,
}

pub(super) fn connect(window: &AccountWindow, controller: &Rc<RefCell<Controller>>) {
    window.on_tracking_action(action(window, controller, |state, window, event| {
        state.tracking_action(window, event)
    }));
    window.on_tracking_project(action(window, controller, |state, window, index| {
        let id = state
            .tracking
            .projects
            .iter()
            .filter(|project| !project.archived)
            .nth(usize::try_from(index - 1).unwrap_or(usize::MAX))
            .map(|project| project.id);
        tracker::remember_project(&state.connection, id).map_err(tracking_error)?;
        state.tracking.selection = id;
        state.sync_tracking(window, now())
    }));
    window.on_quick_project(action(
        window,
        controller,
        |state, window, name: SharedString| {
            let id = tracker::save_project(&state.connection, None, &name, "", "#0E5A47", now())
                .map_err(tracking_error)?;
            tracker::remember_project(&state.connection, Some(id)).map_err(tracking_error)?;
            state.tracking.selection = Some(id);
            state.refresh_projects(window)
        },
    ));
    window.on_describe_work(action(
        window,
        controller,
        |state, window, text: SharedString| {
            let id = state
                .tracking
                .pending_activity
                .ok_or_else(|| "Kein Abschnitt ausgewählt.".to_owned())?;
            tracker::describe(&state.connection, id, &text, now()).map_err(tracking_error)?;
            state.tracking.pending_activity = None;
            state.render(window)
        },
    ));
    window.on_edit_record(
        action(
            window,
            controller,
            |state, window, (id, bounds): (SharedString, bool)| {
                state.open_record(window, &id, bounds)
            },
        )
        .tuple_callback(),
    );
    window.on_new_record(
        action(window, controller, |state, window, ()| {
            state.open_record(window, "", false)
        })
        .unit_callback(),
    );
    window.on_save_record(action(
        window,
        controller,
        |state, window, form: EntryForm| state.save_record(window, form),
    ));
    window.on_delete_record(
        action(window, controller, |state, window, ()| {
            let id = state
                .tracking
                .editor
                .as_ref()
                .and_then(|editor| editor.id)
                .ok_or_else(|| "Kein Eintrag ausgewählt.".to_owned())?;
            tracker::delete_entry(&mut state.connection, id).map_err(tracking_error)?;
            state.tracking.editor = None;
            state.render(window)
        })
        .unit_callback(),
    );
    window.on_cancel_record(
        action(window, controller, |state, window, ()| {
            state.tracking.editor = None;
            window.set_show_entry_form(false);
            Ok(())
        })
        .unit_callback(),
    );
    window.on_filter_project(action(
        window,
        controller,
        |state, window, filter: SharedString| {
            state.tracking.filter = (!filter.is_empty()).then(|| filter.to_string());
            state.render(window)
        },
    ));
    window.on_edit_project(action(
        window,
        controller,
        |state, window, id: SharedString| {
            let form = if id.is_empty() {
                ProjectForm {
                    color: "#0E5A47".into(),
                    ..Default::default()
                }
            } else {
                let project = state
                    .tracking
                    .projects
                    .iter()
                    .find(|project| project.id.to_string() == id.as_str())
                    .ok_or_else(|| "Projekt nicht gefunden.".to_owned())?;
                ProjectForm {
                    id,
                    name: project.name.as_str().into(),
                    description: project.description.as_str().into(),
                    color: project.color.as_str().into(),
                    archived: project.archived,
                    swatch: swatch(Some(&project.color)),
                }
            };
            window.set_project_form(form);
            window.set_show_project_form(true);

            Ok(())
        },
    ));
    window.on_save_project(action(
        window,
        controller,
        |state, window, form: ProjectForm| {
            tracker::save_project(
                &state.connection,
                parse_id(&form.id)?,
                &form.name,
                &form.description,
                &form.color,
                now(),
            )
            .map_err(tracking_error)?;
            window.set_show_project_form(false);
            state.refresh_projects(window)
        },
    ));
    window.on_archive_project(action(
        window,
        controller,
        |state, window, id: SharedString| {
            let id = parse_id(&id)?.ok_or_else(|| "Projekt nicht gefunden.".to_owned())?;
            let project = state
                .tracking
                .projects
                .iter()
                .find(|project| project.id == id)
                .ok_or_else(|| "Projekt nicht gefunden.".to_owned())?;
            tracker::archive_project(&state.connection, id, !project.archived, now())
                .map_err(tracking_error)?;
            if !project.archived && state.tracking.selection == Some(id) {
                tracker::remember_project(&state.connection, None).map_err(tracking_error)?;
                state.tracking.selection = None;
            }
            state.refresh_projects(window)
        },
    ));
    window.on_cancel_project(
        action(window, controller, |_, window, ()| {
            window.set_show_project_form(false);
            Ok(())
        })
        .unit_callback(),
    );

    let weak_state = Rc::downgrade(controller);
    let weak_window = window.as_weak();
    controller.borrow().tracking.timer.start(
        TimerMode::Repeated,
        Duration::from_secs(1),
        move || {
            if let (Some(state), Some(window)) = (weak_state.upgrade(), weak_window.upgrade()) {
                let mut state = state.borrow_mut();
                let at = now();
                if local_date(at, state.zone) != local_date(state.tracking.last_pulse, state.zone) {
                    if let Err(error) = state.render(&window) {
                        window.set_feedback(error.into());
                    }
                } else if state.tracking.trusted.is_some()
                    && let Err(error) = state.sync_tracking(&window, at)
                {
                    window.set_feedback(error.clone().into());
                    if let Some(mini) = &state.mini {
                        mini.set_feedback(error.into());
                    }
                }
                state.tracking.last_pulse = at;
            }
        },
    );
}

// Slint uses separate arguments for some callbacks. Keep this adaptation at the UI boundary.
trait Callback<A>: Fn(A) + Sized + 'static {}
impl<A, F: Fn(A) + 'static> Callback<A> for F {}
trait UnitCallback: Callback<()> {
    fn unit_callback(self) -> impl Fn() + 'static {
        move || self(())
    }
}
impl<F: Callback<()>> UnitCallback for F {}
trait TupleCallback<A, B>: Callback<(A, B)> {
    fn tuple_callback(self) -> impl Fn(A, B) + 'static {
        move |a, b| self((a, b))
    }
}
impl<A, B, F: Callback<(A, B)>> TupleCallback<A, B> for F {}

pub(super) fn connect_mini(mini: &MiniTimer, main: &AccountWindow) {
    let window = main.as_weak();
    mini.on_tracking_action(move |event| {
        if let Some(main) = window.upgrade() {
            main.invoke_tracking_action(event);
        }
    });
    let window = main.as_weak();
    mini.on_tracking_project(move |index| {
        if let Some(main) = window.upgrade() {
            main.invoke_tracking_project(index);
        }
    });
    let window = main.as_weak();
    mini.on_quick_project(move |name| {
        if let Some(main) = window.upgrade() {
            main.invoke_quick_project(name);
        }
    });
    let window = main.as_weak();
    mini.on_describe_work(move |text| {
        if let Some(main) = window.upgrade() {
            main.invoke_describe_work(text);
        }
    });
}

impl Controller {
    fn tracking_action(&mut self, window: &AccountWindow, event: i32) -> Result<(), String> {
        let at = now();
        if self.tracking.expire_trust(at) {
            self.render(window)?;
            return Err("Bitte die Zeit seit dem letzten Speicherpunkt prüfen.".into());
        }
        if event == 8 || event == 9 {
            self.tracking.pending_activity = if event == 8 {
                None
            } else {
                tracker::pending_descriptions(&self.connection)
                    .map_err(tracking_error)?
                    .first()
                    .map(|item| item.0)
            };
            return self.sync_tracking(window, at);
        }
        if event == 10 {
            tracker::finalize_manual_day(&mut self.connection, self.zone, at)
                .map_err(tracking_error)?;
            return self.render(window);
        }
        let active = tracker::active(&self.connection).map_err(tracking_error)?;
        let pending = if event == 0 {
            let started =
                tracker::start(&mut self.connection, self.zone, self.tracking.selection, at)
                    .map_err(tracking_error)?;
            self.tracking.trusted = Some(started.entry_id);
            None
        } else {
            let entry = active
                .as_ref()
                .ok_or_else(|| "Es läuft kein Arbeitstag.".to_owned())?
                .entry_id;
            if (5..=7).contains(&event) && self.tracking.trusted != Some(entry) {
                let recovery = match event {
                    5 => Recovery::Include,
                    6 => Recovery::BreakSinceCheckpoint,
                    _ => Recovery::FinishAtCheckpoint,
                };
                tracker::recover(&mut self.connection, entry, recovery, at)
                    .map_err(tracking_error)?
            } else {
                if self.tracking.trusted != Some(entry) {
                    return Err("Bitte die offene Erfassung zuerst wiederaufnehmen.".into());
                }
                let command = match event {
                    1 => Command::Switch(self.tracking.selection),
                    2 => Command::Pause,
                    3 => Command::Resume(self.tracking.selection),
                    4 => Command::Finish,
                    _ => return Err("Unbekannte Aktion.".into()),
                };
                tracker::transition(&mut self.connection, entry, command, at)
                    .map_err(tracking_error)?
            }
        };
        self.tracking.trusted = tracker::active(&self.connection)
            .map_err(tracking_error)?
            .map(|session| session.entry_id);
        self.tracking.last_pulse = at;
        if pending.is_some() {
            self.tracking.pending_activity = pending;
        }
        self.render(window)
    }

    pub(super) fn refresh_projects(&mut self, window: &AccountWindow) -> Result<(), String> {
        self.tracking.projects = tracker::projects(&self.connection).map_err(tracking_error)?;
        self.tracking.selector_names = project_names(&self.tracking.projects);
        self.render(window)
    }

    pub(super) fn refresh_pending(&mut self) -> Result<(), String> {
        let pending = tracker::pending_descriptions(&self.connection).map_err(tracking_error)?;
        self.tracking.pending_count = pending.len();
        if self
            .tracking
            .pending_activity
            .is_some_and(|id| !pending.iter().any(|(stored, _)| *stored == id))
        {
            self.tracking.pending_activity = None;
        }
        Ok(())
    }

    pub(super) fn sync_tracking(
        &mut self,
        window: &AccountWindow,
        at: Timestamp,
    ) -> Result<(), String> {
        let today = local_date(at, self.zone);
        let session = tracker::active(&self.connection).map_err(tracking_error)?;
        self.tracking.expire_trust(at);
        if self.tracking.trusted.is_some() {
            if session
                .as_ref()
                .is_none_or(|session| Some(session.entry_id) != self.tracking.trusted)
            {
                self.tracking.trusted = None;
            } else if let Some(session) = session
                .as_ref()
                .filter(|session| Some(session.entry_id) == self.tracking.trusted)
                && at - session.checkpoint >= TimeDelta::seconds(30)
            {
                tracker::checkpoint(&self.connection, session.entry_id, at)
                    .map_err(tracking_error)?;
            }
            self.tracking.last_pulse = at;
        }
        let trusted = session
            .as_ref()
            .is_some_and(|session| Some(session.entry_id) == self.tracking.trusted);
        let mode = match session.as_ref() {
            Some(_) if !trusted => 4,
            Some(session) if session.kind == Kind::Work => 1,
            Some(_) => 2,
            None if tracker::day_finished(&self.connection, today).map_err(tracking_error)? => 3,
            None => 0,
        };
        let day = dashboard::timeline(&self.connection, today, at, self.tracking.trusted)?;
        let target = super::account::daily_target(&self.connection, today)
            .map_err(super::store_error)?
            .map(i64::from);
        let remaining = target.map(|target| (target - day.work_seconds).max(0));
        let progress = target.filter(|&target| target > 0).map_or(0.0, |target| {
            (day.work_seconds as f32 / target as f32).min(1.0)
        });
        let live = session.as_ref().filter(|_| trusted);
        let elapsed = live.map_or(0, |session| (at - session.started).num_seconds().max(0));
        let project = session
            .as_ref()
            .filter(|session| session.kind == Kind::Work)
            .and_then(|session| session.project)
            .and_then(|id| {
                self.tracking
                    .projects
                    .iter()
                    .find(|project| project.id == id)
            });
        let finish = if mode == 1 || mode == 2 {
            remaining
                .and_then(|seconds| at.checked_add_signed(TimeDelta::seconds(seconds)))
                .map(|estimate| {
                    estimate
                        .with_timezone(&self.zone)
                        .format("%H:%M")
                        .to_string()
                })
        } else {
            None
        };
        let selection = self
            .tracking
            .projects
            .iter()
            .filter(|project| !project.archived)
            .position(|project| Some(project.id) == self.tracking.selection)
            .map_or(0, |index| index as i32 + 1);
        let view = TrackingView {
            mode,
            selection,
            projects: self.tracking.selector_names.clone(),
            worked: duration(day.work_seconds, false).into(),
            remaining: remaining
                .map_or_else(dash, |value| duration(value, false))
                .into(),
            target: target
                .map_or_else(dash, |value| duration(value, false))
                .into(),
            breaks: duration(day.break_seconds, false).into(),
            finish: finish.unwrap_or_else(dash).into(),
            progress,
            project: project
                .map_or("Ohne Projekt", |project| project.name.as_str())
                .into(),
            swatch: swatch(project.map(|project| project.color.as_str())),
            segment_time: duration(elapsed, false).into(),
            delta: target
                .map_or_else(dash, |target| duration(day.work_seconds - target, true))
                .into(),
            pending_id: self
                .tracking
                .pending_activity
                .map(|id| id.to_string())
                .unwrap_or_default()
                .into(),
            pending_count: self.tracking.pending_count as i32,
            can_finalize: mode == 0 && day.has_closed,
        };
        if self.page == Page::Today {
            window.set_segments(model(day.rows));
        }
        window.set_tracking(view.clone());
        if let Some(mini) = &self.mini {
            mini.set_tracking(view);
        }
        Ok(())
    }

    pub(super) fn checkpoint(&self) {
        let at = now();
        if let Some(entry) = self.tracking.trusted
            && (TimeDelta::zero()..=TimeDelta::minutes(2))
                .contains(&(at - self.tracking.last_pulse))
        {
            let _ = tracker::checkpoint(&self.connection, entry, at);
        }
    }

    pub(super) fn show_projects(&self, window: &AccountWindow) {
        window.set_project_list(model(
            self.tracking
                .projects
                .iter()
                .map(|project| ProjectForm {
                    id: project.id.to_string().into(),
                    name: project.name.as_str().into(),
                    description: project.description.as_str().into(),
                    color: project.color.as_str().into(),
                    archived: project.archived,
                    swatch: swatch(Some(&project.color)),
                })
                .collect(),
        ));
    }

    pub(super) fn show_filters(&self, window: &AccountWindow) {
        let mut names = vec![SharedString::from("Alle Projekte"), "Ohne Projekt".into()];
        let mut choices = vec![ProjectChoice::default(), ProjectChoice { id: "0".into() }];
        let mut selection = i32::from(self.tracking.filter.as_deref() == Some("0"));
        for project in &self.tracking.projects {
            let id = project.id.to_string();
            if self.tracking.filter.as_deref() == Some(id.as_str()) {
                selection = names.len() as i32;
            }
            names.push(project_caption(project).into());
            choices.push(ProjectChoice { id: id.into() });
        }
        window.set_filter_names(model(names));
        window.set_filter_choices(model(choices));
        window.set_filter_index(selection);
    }

    pub(super) fn selected_slices(&self, at: Timestamp) -> Result<Vec<dashboard::Slice>, String> {
        let period = self
            .range
            .period(self.selected)
            .map_err(super::account_error)?;
        let mut rows = dashboard::slices(&self.connection, period, at, self.tracking.trusted)?;
        match self.tracking.filter.as_deref() {
            None => {}
            Some("0") => {
                rows.retain(|slice| slice.entry.project.is_none() && slice.entry.kind == Kind::Work)
            }
            Some(id) => {
                let project = parse_id(id)?;
                rows.retain(|slice| slice.entry.project == project);
            }
        }
        Ok(rows)
    }

    pub(super) fn record_rows(&self, window: &AccountWindow, at: Timestamp) -> Result<(), String> {
        window.set_segments(model(
            self.selected_slices(at)?
                .iter()
                .map(dashboard::Slice::row)
                .collect(),
        ));
        Ok(())
    }

    fn show_assignments(&self, window: &AccountWindow, current: &str) {
        let mut names = vec![SharedString::from("Ohne Projekt")];
        let mut choices = vec![ProjectChoice { id: "0".into() }];
        let mut selection = 0;
        for project in &self.tracking.projects {
            let id = project.id.to_string();
            if id == current {
                selection = names.len() as i32;
            }
            names.push(project_caption(project).into());
            choices.push(ProjectChoice { id: id.into() });
        }
        window.set_assignment_names(model(names));
        window.set_assignment_choices(model(choices));
        window.set_assignment_index(selection);
    }

    fn open_record(
        &mut self,
        window: &AccountWindow,
        id: &str,
        bounds: bool,
    ) -> Result<(), String> {
        let parsed = parse_id(id)?;
        let today = local_date(now(), self.zone);
        if self.page != Page::Times {
            self.page = Page::Times;
            self.selected = today;
            self.range = Range::Day;
            self.render(window)?;
        }
        let (editor, mut form) = if let Some(id) = parsed {
            let entry = tracker::entry(&self.connection, id).map_err(tracking_error)?;
            let (zone, start, end) = if bounds {
                tracker::day_bounds(&self.connection, entry.workday_id).map_err(tracking_error)?
            } else {
                (
                    entry.zone,
                    entry.start,
                    entry.end.ok_or_else(|| tracking_error(Error::OpenEntry))?,
                )
            };
            (
                EntryEditor {
                    id: (!bounds).then_some(id),
                    day: bounds.then_some(entry.workday_id),
                    zone,
                    original: Some((start, end)),
                },
                EntryForm {
                    kind: i32::from(entry.kind == Kind::Break),
                    bounds,
                    existing: !bounds,
                    project: entry
                        .project
                        .map_or_else(|| "0".into(), |id| id.to_string().into()),
                    activity: entry.activity.into(),
                    start_date: display_date(local_date(start, zone)).into(),
                    end_date: display_date(local_date(end, zone)).into(),
                    start_time: start
                        .with_timezone(&zone)
                        .format("%H:%M:%S")
                        .to_string()
                        .into(),
                    end_time: end
                        .with_timezone(&zone)
                        .format("%H:%M:%S")
                        .to_string()
                        .into(),
                },
            )
        } else {
            let date = display_date(self.selected.min(today));
            (
                EntryEditor {
                    id: None,
                    day: None,
                    zone: self.zone,
                    original: None,
                },
                EntryForm {
                    start_date: date.as_str().into(),
                    end_date: date.into(),
                    project: "0".into(),
                    ..Default::default()
                },
            )
        };
        if bounds {
            form.project = "0".into();
        }
        self.tracking.editor = Some(editor);
        self.show_assignments(window, &form.project);
        window.set_entry_form(form);
        window.set_show_entry_form(true);
        Ok(())
    }

    fn save_record(&mut self, window: &AccountWindow, form: EntryForm) -> Result<(), String> {
        let editor = self
            .tracking
            .editor
            .as_ref()
            .ok_or_else(|| "Kein Eintrag ausgewählt.".to_owned())?;
        let start = parse_local(
            editor.zone,
            &form.start_date,
            &form.start_time,
            editor.original.map(|pair| pair.0),
        )?;
        let end = parse_local(
            editor.zone,
            &form.end_date,
            &form.end_time,
            editor.original.map(|pair| pair.1),
        )?;
        let at = now();
        if let Some(day) = editor.day {
            tracker::save_bounds(&mut self.connection, day, start, end, at)
                .map_err(tracking_error)?;
        } else {
            let input = EntryInput {
                kind: if form.kind == 1 {
                    Kind::Break
                } else {
                    Kind::Work
                },
                project: parse_id(&form.project)?,
                zone: editor.zone,
                start,
                end,
                activity: &form.activity,
            };
            tracker::save_entry(&mut self.connection, editor.id, input, at)
                .map_err(tracking_error)?;
        }
        self.selected = local_date(start, editor.zone);
        self.tracking.editor = None;
        window.set_show_entry_form(false);
        self.render(window)?;
        window.set_feedback("Zeit gespeichert.".into());
        Ok(())
    }
}

fn parse_id(text: &str) -> Result<Option<i64>, String> {
    if text.is_empty() || text == "0" {
        return Ok(None);
    }
    text.parse::<i64>()
        .ok()
        .filter(|id| *id > 0)
        .map(Some)
        .ok_or_else(|| "Ungültiger Eintrag.".into())
}

fn project_names(projects: &[Project]) -> ModelRc<SharedString> {
    model(
        std::iter::once("Ohne Projekt".into())
            .chain(
                projects
                    .iter()
                    .filter(|project| !project.archived)
                    .map(|project| SharedString::from(project.name.as_str())),
            )
            .collect(),
    )
}

fn project_caption(project: &Project) -> String {
    if project.archived {
        format!("{} (archiviert)", project.name)
    } else {
        project.name.clone()
    }
}

pub(super) fn swatch(color: Option<&str>) -> Color {
    let value = color
        .and_then(|text| text.strip_prefix('#'))
        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
        .unwrap_or(0x0E5A47);
    Color::from_rgb_u8((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

fn parse_local(
    zone: Tz,
    date: &str,
    time: &str,
    original: Option<Timestamp>,
) -> Result<Timestamp, String> {
    let date = parse_date(date)?;
    let time = NaiveTime::parse_from_str(time.trim(), "%H:%M:%S")
        .or_else(|_| NaiveTime::parse_from_str(time.trim(), "%H:%M"))
        .map_err(|_| "Uhrzeit bitte als HH:MM oder HH:MM:SS eingeben.".to_owned())?;
    let value = date.and_time(time);
    if let Some(original) = original
        && original
            .with_timezone(&zone)
            .naive_local()
            .with_nanosecond(0)
            == Some(value)
    {
        return Ok(original);
    }
    zone.from_local_datetime(&value)
        .single()
        .map(|time| time.to_utc())
        .ok_or_else(|| {
            "Die Uhrzeit ist wegen der Zeitumstellung mehrdeutig oder existiert nicht.".into()
        })
}

pub(super) fn tracking_error(error: Error) -> String {
    match error {
        Error::Overlap => "Die Zeit überschneidet sich mit einem vorhandenen Eintrag.".into(),
        Error::InvalidRange => {
            "Beginn, Ende und bestehende Einträge passen zeitlich nicht zusammen.".into()
        }
        Error::InvalidTransition => {
            "Die Erfassung hat sich bereits geändert. Bitte erneut versuchen.".into()
        }
        Error::ArchivedProject => {
            "Das Projekt ist archiviert. Bitte ein aktives Projekt auswählen.".into()
        }
        Error::InvalidProject => {
            "Bitte einen Projektnamen und eine Farbe im Format #RRGGBB eingeben.".into()
        }
        Error::ActivityRequired => "Bitte eine Tätigkeitsbeschreibung eingeben.".into(),
        Error::Missing => {
            "Der Eintrag ist nicht mehr verfügbar oder der Arbeitstag ist noch offen.".into()
        }
        Error::OpenEntry => "Offene Zeiten können nach dem Beenden bearbeitet werden.".into(),
        other => format!("Zeitdaten konnten nicht verarbeitet werden ({other})."),
    }
}
