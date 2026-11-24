//! Window lifecycle and placement. The timer only displays shared account data.

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;

use std::cell::Cell;

use slint::{
    PhysicalPosition, PlatformError, Window,
    winit_030::{EventResult, WinitWindowAccessor, winit::event::WindowEvent},
};

use super::preferences::Point;

thread_local! {
    static CREATING_MINI: Cell<bool> = const { Cell::new(false) };
}

pub(super) fn initialize() -> Result<(), PlatformError> {
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(|attributes| {
            if CREATING_MINI.get() {
                attributes.with_active(false)
            } else {
                attributes
            }
        })
        .select()
}

pub(super) fn create_mini<T>(create: impl FnOnce() -> T) -> T {
    CREATING_MINI.set(true);
    let result = create();
    CREATING_MINI.set(false);
    result
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum Visibility {
    #[default]
    Hidden,
    Automatic,
    Manual,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum WindowAction {
    ShowAutomatically,
    Hide,
    Keep,
}

#[derive(Default)]
pub(super) struct WindowState {
    pub visibility: Visibility,
    minimized: bool,
}

impl WindowState {
    pub fn minimize_changed(&mut self, minimized: bool, auto_show: bool) -> WindowAction {
        if self.minimized == minimized {
            return WindowAction::Keep;
        }
        self.minimized = minimized;
        self.preference_changed(auto_show)
    }

    pub fn preference_changed(&self, auto_show: bool) -> WindowAction {
        match (self.minimized && auto_show, self.visibility) {
            (true, Visibility::Hidden) => WindowAction::ShowAutomatically,
            (false, Visibility::Automatic) => WindowAction::Hide,
            _ => WindowAction::Keep,
        }
    }
}

pub(super) fn observe_main(
    window: &Window,
    notify: impl Fn(bool) + Clone + Send + 'static,
    clock_interrupted: impl Fn() + Clone + Send + 'static,
) {
    #[cfg(not(windows))]
    let _ = clock_interrupted;
    #[cfg(windows)]
    let mut hooked_window = None;
    window.on_winit_window_event(move |window, event| {
        #[cfg(windows)]
        windows::observe_window(
            window,
            &mut hooked_window,
            {
                let notify = notify.clone();
                move |minimized| {
                    let notify = notify.clone();
                    let _ = slint::invoke_from_event_loop(move || notify(minimized));
                }
            },
            {
                let clock_interrupted = clock_interrupted.clone();
                move || {
                    let _ = slint::invoke_from_event_loop(clock_interrupted.clone());
                }
            },
        );
        if matches!(
            event,
            WindowEvent::Focused(_) | WindowEvent::Occluded(_) | WindowEvent::Resized(_)
        ) && let Some(Some(minimized)) = window.with_winit_window(|window| window.is_minimized())
        {
            let notify = notify.clone();
            let _ = slint::invoke_from_event_loop(move || notify(minimized));
        }
        EventResult::Propagate
    });
}

pub(super) fn observe_mini(window: &Window, notify: impl Fn() + Clone + Send + 'static) {
    window.on_winit_window_event(move |_, event| {
        if matches!(event, WindowEvent::Moved(_) | WindowEvent::Resized(_)) {
            let notify = notify.clone();
            let _ = slint::invoke_from_event_loop(notify);
        }
        EventResult::Propagate
    });
}

pub(super) fn show_without_activation(
    window: &Window,
    show: impl FnOnce() -> Result<(), PlatformError>,
) -> Result<(), PlatformError> {
    #[cfg(windows)]
    return windows::show_without_activation(window, show);
    #[cfg(not(windows))]
    {
        let _ = window;
        show()
    }
}

#[derive(Clone, Copy)]
struct Area {
    origin: Point,
    width: i32,
    height: i32,
}

impl Area {
    fn overlaps(self, point: Point, size: (i32, i32)) -> bool {
        point.x.saturating_add(size.0) > self.origin.x
            && point.y.saturating_add(size.1) > self.origin.y
            && point.x < self.origin.x.saturating_add(self.width)
            && point.y < self.origin.y.saturating_add(self.height)
    }

    fn position(self, saved: Option<Point>, size: (i32, i32), scale: f64, margin: u32) -> Point {
        let max_x = self
            .origin
            .x
            .saturating_add(self.width.saturating_sub(size.0).max(0));
        let max_y = self
            .origin
            .y
            .saturating_add(self.height.saturating_sub(size.1).max(0));
        let margin = (f64::from(margin) * scale).round() as i32;
        let point = saved.unwrap_or(Point {
            x: max_x.saturating_sub(margin),
            y: max_y.saturating_sub(margin),
        });
        Point {
            x: point.x.clamp(self.origin.x, max_x),
            y: point.y.clamp(self.origin.y, max_y),
        }
    }
}

pub(super) fn place(
    main: &Window,
    mini: &Window,
    saved: Option<Point>,
    margin: u32,
) -> Option<Point> {
    let (size, source_scale) = mini.with_winit_window(|window| {
        let size = window.outer_size();
        (
            (size.width as i32, size.height as i32),
            window.scale_factor(),
        )
    })?;
    #[cfg(windows)]
    let (area, saved, scale) = windows::work_area(main, saved, size)?;
    #[cfg(not(windows))]
    let (area, saved, scale) = main
        .with_winit_window(|window| {
            let area = |monitor: &slint::winit_030::winit::monitor::MonitorHandle| Area {
                origin: Point {
                    x: monitor.position().x,
                    y: monitor.position().y,
                },
                width: monitor.size().width as i32,
                height: monitor.size().height as i32,
            };
            if let Some(saved) = saved
                && let Some(monitor) = window
                    .available_monitors()
                    .find(|monitor| area(monitor).overlaps(saved, size))
            {
                return Some((area(&monitor), Some(saved), monitor.scale_factor()));
            }
            window
                .current_monitor()
                .or_else(|| window.primary_monitor())
                .map(|monitor| (area(&monitor), None, monitor.scale_factor()))
        })
        .flatten()?;

    let ratio = scale / source_scale;
    let size = (
        (f64::from(size.0) * ratio).ceil() as i32,
        (f64::from(size.1) * ratio).ceil() as i32,
    );
    let point = area.position(saved, size, scale, margin);
    mini.set_position(PhysicalPosition::new(point.x, point.y));
    Some(point)
}

pub(super) fn position(window: &Window) -> Point {
    let pos = window.position();
    Point { x: pos.x, y: pos.y }
}

pub(super) fn restore_main(window: &Window) {
    if window
        .with_winit_window(|native| {
            native.set_minimized(false);
            native.focus_window();
        })
        .is_none()
    {
        window.set_minimized(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_minimize_and_restore_respect_automatic_and_manual_windows() {
        let mut state = WindowState::default();
        assert_eq!(
            state.minimize_changed(true, true),
            WindowAction::ShowAutomatically
        );
        state.visibility = Visibility::Automatic;
        assert_eq!(state.minimize_changed(true, true), WindowAction::Keep);
        assert_eq!(state.minimize_changed(false, true), WindowAction::Hide);
        state.visibility = Visibility::Hidden;
        assert_eq!(state.minimize_changed(true, false), WindowAction::Keep);
        assert_eq!(
            state.preference_changed(true),
            WindowAction::ShowAutomatically
        );
        state.visibility = Visibility::Manual;
        assert_eq!(state.minimize_changed(false, true), WindowAction::Keep);
        assert_eq!(state.minimize_changed(true, true), WindowAction::Keep);
    }

    #[test]
    fn hidden_automatic_timer_stays_closed_until_the_next_minimize() {
        let mut state = WindowState::default();
        assert_eq!(
            state.minimize_changed(true, true),
            WindowAction::ShowAutomatically
        );
        state.visibility = Visibility::Hidden;
        assert_eq!(state.minimize_changed(true, true), WindowAction::Keep);
        assert_eq!(state.minimize_changed(false, true), WindowAction::Keep);
        assert_eq!(
            state.minimize_changed(true, true),
            WindowAction::ShowAutomatically
        );
    }

    #[test]
    fn positions_use_the_work_area_and_logical_margin_on_negative_monitors() {
        let area = Area {
            origin: Point { x: -1920, y: 40 },
            width: 1920,
            height: 1000,
        };
        let size = (528, 310);
        assert_eq!(
            area.position(None, size, 1.5, 24),
            Point { x: -564, y: 694 }
        );
        let custom = Point { x: -800, y: 82 };
        assert!(area.overlaps(custom, size));
        assert_eq!(area.position(Some(custom), size, 1.5, 24), custom);
        assert_eq!(
            area.position(Some(Point { x: -40, y: 900 }), size, 1.5, 24),
            Point { x: -528, y: 730 }
        );
        assert!(!area.overlaps(Point { x: 0, y: 100 }, size));
    }
}
