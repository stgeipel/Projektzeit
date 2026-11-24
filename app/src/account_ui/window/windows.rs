//! Win32 window events, no-activate showing, and monitor work-area lookup.

use std::mem::size_of;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use slint::{PlatformError, Window, winit_030::WinitWindowAccessor};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITOR_DEFAULTTONULL, MONITORINFO,
        MonitorFromRect, MonitorFromWindow,
    },
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, PBT_APMRESUMEAUTOMATIC, SIZE_MAXIMIZED, SIZE_MINIMIZED,
            SIZE_RESTORED, SWP_NOACTIVATE, SetWindowLongPtrW, WINDOWPOS, WM_NCDESTROY,
            WM_POWERBROADCAST, WM_SIZE, WM_TIMECHANGE, WM_WINDOWPOSCHANGING, WS_EX_NOACTIVATE,
        },
    },
};

use super::{Area, Point};

const MINIMIZE_SUBCLASS: usize = 1;
struct Notifications {
    minimize: Box<dyn Fn(bool) + Send>,
    clock_interrupted: Box<dyn Fn() + Send>,
}

fn hwnd(window: &Window) -> Option<HWND> {
    let handle = window.window_handle();
    let handle = handle.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(handle) => Some(handle.hwnd.get() as HWND),
        _ => None,
    }
}

pub(super) fn observe_window(
    window: &Window,
    installed: &mut Option<HWND>,
    minimize: impl Fn(bool) + Send + 'static,
    clock_interrupted: impl Fn() + Send + 'static,
) {
    let Some(hwnd) = hwnd(window) else { return };
    if *installed == Some(hwnd) {
        return;
    }
    let callback = Box::new(Notifications {
        minimize: Box::new(minimize),
        clock_interrupted: Box::new(clock_interrupted),
    });
    // The callback is delivered on the owning GUI thread. The subclass owns
    // the Box until WM_NCDESTROY, or until installation fails.
    let callback = Box::into_raw(callback);
    if unsafe {
        SetWindowSubclass(
            hwnd,
            Some(window_proc),
            MINIMIZE_SUBCLASS,
            callback as usize,
        )
    } != 0
    {
        *installed = Some(hwnd);
    } else {
        drop(unsafe { Box::from_raw(callback) });
    }
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    subclass: usize,
    data: usize,
) -> LRESULT {
    let callback = data as *mut Notifications;
    if message == WM_NCDESTROY {
        unsafe {
            RemoveWindowSubclass(hwnd, Some(window_proc), subclass);
            drop(Box::from_raw(callback));
        }
    } else if message == WM_SIZE
        && matches!(
            wparam as u32,
            SIZE_RESTORED | SIZE_MINIMIZED | SIZE_MAXIMIZED
        )
    {
        let minimized = wparam as u32 == SIZE_MINIMIZED;
        // The callback only posts to the Slint event loop, avoiding reentry
        // while Win32/Winit dispatches the message.
        unsafe {
            ((*callback).minimize)(minimized);
        }
    } else if message == WM_TIMECHANGE
        || (message == WM_POWERBROADCAST && wparam == PBT_APMRESUMEAUTOMATIC as WPARAM)
    {
        // Resume invalidates the in-process observation even after a brief sleep.
        unsafe {
            ((*callback).clock_interrupted)();
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn show_without_activation(
    window: &Window,
    show: impl FnOnce() -> Result<(), PlatformError>,
) -> Result<(), PlatformError> {
    // A newly created Winit window also has with_active(false). For subsequent
    // shows Winit uses SW_SHOW, so suppress activation temporarily on its HWND.
    let Some(hwnd) = hwnd(window) else {
        return show();
    };
    let original = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    unsafe {
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original | WS_EX_NOACTIVATE as isize);
        SetWindowSubclass(hwnd, Some(no_activate_proc), MINIMIZE_SUBCLASS, 0);
    }
    let result = show();
    unsafe {
        RemoveWindowSubclass(hwnd, Some(no_activate_proc), MINIMIZE_SUBCLASS);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, original);
    }
    result
}

unsafe extern "system" fn no_activate_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    if message == WM_WINDOWPOSCHANGING && lparam != 0 {
        // Win32 gives the subclass a writable WINDOWPOS for this message.
        unsafe {
            (*(lparam as *mut WINDOWPOS)).flags |= SWP_NOACTIVATE;
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

pub(super) fn work_area(
    main: &Window,
    saved: Option<Point>,
    size: (i32, i32),
) -> Option<(Area, Option<Point>, f64)> {
    use slint::winit_030::winit::platform::windows::MonitorHandleExtWindows;
    let hwnd = hwnd(main)?;
    let saved_monitor = saved
        .map(|point| unsafe {
            MonitorFromRect(
                &RECT {
                    left: point.x,
                    top: point.y,
                    right: point.x.saturating_add(size.0),
                    bottom: point.y.saturating_add(size.1),
                },
                MONITOR_DEFAULTTONULL,
            )
        })
        .filter(|monitor| !monitor.is_null());
    let monitor = saved_monitor
        .unwrap_or_else(|| unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) });
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    if monitor.is_null() || unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    let area = Area {
        origin: Point {
            x: info.rcWork.left,
            y: info.rcWork.top,
        },
        width: info.rcWork.right - info.rcWork.left,
        height: info.rcWork.bottom - info.rcWork.top,
    };
    let scale = main.with_winit_window(|window| {
        window
            .available_monitors()
            .find(|handle| handle.hmonitor() == monitor as isize)
            .map_or_else(|| window.scale_factor(), |monitor| monitor.scale_factor())
    })?;
    Some((
        area,
        saved.filter(|&point| area.overlaps(point, size)),
        scale,
    ))
}
