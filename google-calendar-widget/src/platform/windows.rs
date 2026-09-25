//! Windows-specific window management.
//!
//! Goal: make the iced window behave like a desktop widget:
//!   * Always behind normal applications but above the wallpaper.
//!   * Removed from the taskbar and Alt+Tab.
//!   * Clickable (receives input) but never steals focus unexpectedly.
//!
//! Key tricks:
//!   - WM_WINDOWPOSCHANGING hook: when the OS tries to minimize the window to
//!     (-32000, -32000) (the classic "minimize to tray" position), intercept
//!     it and instead keep the window in place at the bottom of the z-order.
//!     This makes Win+D / Show Desktop not hide the widget.
//!   - is_window_foreground check in the KeepAtBottom loop: while the user is
//!     interacting with the widget we don't touch z-order or visibility, to
//!     avoid flicker.
//!   - WS_EX_TOOLWINDOW: removes the window from taskbar and Alt+Tab.
//!   - DWM cloak check: after a Show Desktop, Windows may "cloak" the window
//!     without hiding it; we uncloak explicitly.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;
use windows::core::{IUnknown, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmFlush, DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, FindWindowW, GetClassNameW, GetForegroundWindow,
    GetWindowLongW, IsIconic, SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowLongW,
    SetWindowPos, ShowWindow,
    GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST,
    LWA_ALPHA, SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_RESTORE, WNDPROC,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
};

// Cached once the HWND is found (find_hwnd is called frequently).
static CACHED_HWND: OnceLock<isize> = OnceLock::new();
static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();
static HOOK_INSTALLED: OnceLock<bool> = OnceLock::new();

const WM_WINDOWPOSCHANGING: u32 = 0x0046;

/// Win32 WINDOWPOS structure (defined manually because the bindings expose it
/// behind a feature we don't need elsewhere).
#[repr(C)]
struct WindowPos {
    hwnd: HWND,
    hwnd_insert_after: HWND,
    x: i32,
    y: i32,
    cx: i32,
    cy: i32,
    flags: u32,
}

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

/// Finds (and caches) the HWND of the widget window by title. Returns None
/// until the window is created by iced.
pub fn find_hwnd(title: &str) -> Option<isize> {
    if let Some(v) = CACHED_HWND.get() {
        return Some(*v);
    }
    let wide: Vec<u16> = OsStr::new(title)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        match FindWindowW(PCWSTR::null(), PCWSTR(wide.as_ptr())) {
            Ok(hwnd) => {
                if hwnd.0.is_null() {
                    None
                } else {
                    let raw = hwnd.0 as isize;
                    let _ = CACHED_HWND.set(raw);
                    Some(raw)
                }
            }
            Err(_) => None,
        }
    }
}

/// Returns true if the current foreground window is the desktop (Program
/// Manager or the WorkerW wallpaper host).
pub fn is_desktop_foreground() -> bool {
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground.0.is_null() {
            return false;
        }
        let mut class_buf = [0u16; 128];
        let len = GetClassNameW(foreground, &mut class_buf);
        if len <= 0 {
            return false;
        }
        let class = String::from_utf16_lossy(&class_buf[..len as usize]);
        class == "WorkerW" || class == "Progman"
    }
}

/// Returns true if the widget itself is the foreground window. Used to skip
/// z-order manipulation while the user is actively interacting with it.
pub fn is_window_foreground(hwnd_raw: isize) -> bool {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let fg = GetForegroundWindow();
        fg == hwnd
    }
}

/// Hook that intercepts minimize-to (-32000, -32000) attempts (Win+D / Show
/// Desktop) and reinterprets them as "keep in place at HWND_BOTTOM".
/// Everything else is forwarded to the original WndProc.
unsafe extern "system" fn window_proc_hook(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if msg == WM_WINDOWPOSCHANGING {
        let pos = lparam.0 as *mut WindowPos;
        if !pos.is_null() {
            let x = (*pos).x;
            let y = (*pos).y;
            if x == -32000 && y == -32000 {
                // Rewrite the request: no move, no size change, keep at bottom.
                let mut f = SET_WINDOW_POS_FLAGS((*pos).flags);
                f |= SWP_NOMOVE | SWP_NOSIZE;
                (*pos).flags = f.0;
                (*pos).hwnd_insert_after = HWND_BOTTOM;
                return LRESULT(0);
            }
        }
    }

    if let Some(original) = ORIGINAL_WNDPROC.get() {
        let proc: WNDPROC = std::mem::transmute(*original);
        return CallWindowProcW(proc, hwnd, msg, wparam, lparam);
    }

    LRESULT(0)
}

/// Installs the WndProc hook. Idempotent.
pub fn install_window_proc_hook(hwnd_raw: isize) -> bool {
    if let Some(v) = HOOK_INSTALLED.get() {
        return *v;
    }
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let hook_ptr = window_proc_hook as *const () as isize;
        let original = SetWindowLongPtrW(hwnd, GWLP_WNDPROC, hook_ptr);
        if original == 0 {
            crate::log::write("install_window_proc_hook: failed");
            let _ = HOOK_INSTALLED.set(false);
            return false;
        }
        let _ = ORIGINAL_WNDPROC.set(original);
        let _ = HOOK_INSTALLED.set(true);
        crate::log::write("install_window_proc_hook: OK");
        true
    }
}

/// If DWM has cloaked the window (a common after-effect of Show Desktop),
/// ask it to uncloak. Cloaking hides a window without changing its visibility
/// state, so it must be handled separately from ShowWindow.
pub fn uncloak_if_needed(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let mut cloaked: i32 = 0;
        let hr = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut i32 as *mut _,
            std::mem::size_of::<i32>() as u32,
        );
        if hr.is_ok() && cloaked != 0 {
            let uncloak: BOOL = BOOL(0);
            let _ = DwmSetWindowAttribute(
                hwnd,
                DWMWA_CLOAK,
                &uncloak as *const BOOL as *const _,
                std::mem::size_of::<BOOL>() as u32,
            );
        }
    }
}

/// Brings the widget to the very top of the z-order (visible above the
/// wallpaper). Note: does NOT call SetForegroundWindow, which would steal
/// focus and cause flicker during the KeepAtBottom loop.
pub fn force_show_on_desktop(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);

        // If minimized, restore first so ShowWindow has an effect.
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        uncloak_if_needed(hwnd_raw);

        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE,
        );

        // HWND_TOPMOST here is transient: the KeepAtBottom loop immediately
        // demotes it back to HWND_NOTOPMOST/HWND_BOTTOM once the desktop is
        // no longer in the foreground, so it doesn't stay always-on-top.
        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );

        let _ = DwmFlush();
    }
}

/// Pushes the widget to the bottom of the z-order (behind normal apps, above
/// the wallpaper). Called on every KeepAtBottom tick when the desktop is NOT
/// the foreground window.
pub fn ensure_visible_bottom(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);

        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        uncloak_if_needed(hwnd_raw);

        // Drop topmost first, then push to bottom. Two calls because
        // SetWindowPos can't transition TOPMOST -> BOTTOM in one step.
        let _ = SetWindowPos(
            hwnd,
            HWND_NOTOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

/// Adds WS_EX_TOOLWINDOW so the window disappears from taskbar and Alt+Tab,
/// and WS_EX_LAYERED so SetLayeredWindowAttributes can control alpha.
pub fn hide_from_taskbar(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            (ex_style | (WS_EX_TOOLWINDOW.0 as i32)) | (WS_EX_LAYERED.0 as i32),
        );
        // SWP_FRAMECHANGED forces a non-client recompute so the style change
        // takes effect immediately.
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Removes WS_MAXIMIZEBOX from the window style (decorations are disabled
/// anyway, this guards against future toggles) and re-enables WS_MINIMIZEBOX.
pub fn remove_minimize_box(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let style = GetWindowLongW(hwnd, GWL_STYLE);
        let mask = WS_MAXIMIZEBOX.0 as i32;
        let new_style = (style & !mask) | (WS_MINIMIZEBOX.0 as i32);
        SetWindowLongW(hwnd, GWL_STYLE, new_style);
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

/// Restores a minimized window. Called when a zero-size Resized event is
/// observed (which happens when Windows minimizes to tray).
pub fn restore_window(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }
}

/// Sets the whole-window alpha (0.0 = invisible, 1.0 = opaque). Requires
/// WS_EX_LAYERED to have been set (see hide_from_taskbar).
pub fn set_window_alpha(hwnd_raw: isize, alpha: f32) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let b_alpha = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), b_alpha, LWA_ALPHA);
    }
}

/// Removes the taskbar tab via ITaskbarList. Complements WS_EX_TOOLWINDOW:
/// on some Windows builds the shell can recreate the tab (e.g. after toggling
/// visibility), so we delete it explicitly.
pub fn delete_taskbar_tab(hwnd_raw: isize) {
    unsafe {
        // CoInitializeEx is idempotent for the same threading model; safe to
        // call even if iced already initialized COM.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let created: windows::core::Result<ITaskbarList> = CoCreateInstance(
            &TaskbarList,
            None::<&IUnknown>,
            CLSCTX_INPROC_SERVER,
        );
        if let Ok(taskbar) = created {
            let _ = taskbar.HrInit();
            let hwnd = HWND(hwnd_raw as *mut _);
            let _ = taskbar.DeleteTab(hwnd);
        }
    }
}

/// Reads the "AppsUseLightTheme" registry value to detect dark mode.
/// Returns true when dark mode is active.
pub fn system_is_dark() -> bool {
    let subkey = to_wide("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    let value = to_wide("AppsUseLightTheme");
    unsafe {
        let mut hkey = HKEY::default();
        let open = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        );
        if open.0 != 0 {
            return false;
        }
        let mut data: u32 = 1;
        let mut size: u32 = 4;
        let res = RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            Some(&mut data as *mut u32 as *mut u8),
            Some(&mut size as *mut u32),
        );
        let _ = RegCloseKey(hkey);
        if res.0 != 0 {
            return false;
        }
        // AppsUseLightTheme = 0 means dark mode.
        data == 0
    }
}