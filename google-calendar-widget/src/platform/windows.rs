//! Windows-specific window management.
//!
//! Goal: make the iced window behave like a desktop widget:
//!   * Always behind normal applications but above the wallpaper.
//!   * Removed from the taskbar and Alt+Tab.
//!   * Never hidden by Win+D / Show Desktop.
//!
//! Strategy:
//!   1. Custom WndProc that blocks minimize/hide attempts (WM_SYSCOMMAND
//!      SC_MINIMIZE, WM_SIZE SIZE_MINIMIZED, WM_SHOWWINDOW wparam=0) and
//!      rewrites the Win+D WM_WINDOWPOSCHANGING (-32000,-32000) into a
//!      "keep in place, stay at HWND_BOTTOM" request.
//!   2. Soft reparenting to Progman via GWLP_HWNDPARENT (NOT SetParent,
//!      which fails silently on Win11 24H2). Executed on the window's own
//!      thread by posting a custom WM_APP message.
//!   3. A SetWinEventHook on the foreground event that re-asserts the
//!      widget at HWND_BOTTOM whenever Progman/WorkerW becomes foreground.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;
use windows::core::{IUnknown, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::{
     DwmGetWindowAttribute, DwmSetWindowAttribute, DWMWA_CLOAK, DWMWA_CLOAKED,
    DWMWA_EXCLUDED_FROM_PEEK,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, FindWindowW, GetClassNameW, GetWindowLongPtrW,
    GetWindowLongW, IsIconic, SendMessageTimeoutW, SetLayeredWindowAttributes,
    SetWindowLongPtrW, SetWindowLongW, SetWindowPos, ShowWindow,
    GWL_EXSTYLE, GWL_STYLE, GWLP_HWNDPARENT, GWLP_WNDPROC, HWND_BOTTOM, HWND_NOTOPMOST, 
    LWA_ALPHA, SMTO_ABORTIFHUNG,
    SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SW_RESTORE, WNDPROC, WM_APP, WS_CHILD, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX, WS_POPUP, WS_VISIBLE,
};

static CACHED_HWND: OnceLock<isize> = OnceLock::new();
static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();
static HOOK_INSTALLED: OnceLock<bool> = OnceLock::new();
// HWINEVENTHOOK wraps a raw pointer and isn't Send/Sync, so we store the
// raw pointer value as isize instead.
static WIN_EVENT_HOOK: OnceLock<isize> = OnceLock::new();

const WM_APP_REPARENT: u32 = WM_APP + 1;
const WM_WINDOWPOSCHANGING: u32 = 0x0046;
const WM_SYSCOMMAND: u32 = 0x0112;
const WM_SIZE: u32 = 0x0005;
const WM_SHOWWINDOW: u32 = 0x0018;
const SC_MINIMIZE_MASK: usize = 0xFFF0;
const SC_MINIMIZE_VAL: usize = 0xF020;
const SWP_HIDEWINDOW_RAW: u32 = 0x0080;
const EVENT_SYSTEM_FOREGROUND_VAL: u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZEEND_VAL: u32 = 0x0017;
const WINEVENT_OUTOFCONTEXT_VAL: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS_VAL: u32 = 0x0002;

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

pub fn find_hwnd(title: &str) -> Option<isize> {
    if let Some(v) = CACHED_HWND.get() {
        return Some(*v);
    }
    let wide = to_wide(title);
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

fn find_progman() -> HWND {
    unsafe {
        let class = to_wide("Progman");
        if let Ok(h) = FindWindowW(PCWSTR(class.as_ptr()), PCWSTR::null()) {
            if !h.0.is_null() {
                return h;
            }
        }
        let workerw = to_wide("WorkerW");
        if let Ok(h) = FindWindowW(PCWSTR(workerw.as_ptr()), PCWSTR::null()) {
            if !h.0.is_null() {
                return h;
            }
        }
        HWND(std::ptr::null_mut())
    }
}

/// Soft reparent: set WS_CHILD and attach owner/parent to Progman via
/// GWLP_HWNDPARENT. Must run on the window's own thread.
unsafe fn do_soft_reparent(hwnd: HWND) {
    let progman = find_progman();
    if progman.0.is_null() {
        crate::log::write("soft_reparent: Progman/WorkerW not found");
        return;
    }

    let style = GetWindowLongPtrW(hwnd, GWL_STYLE) as u32;
    let new_style = (style & !WS_POPUP.0) | WS_CHILD.0 | WS_VISIBLE.0;
    SetWindowLongPtrW(hwnd, GWL_STYLE, new_style as isize);

    SetWindowLongPtrW(hwnd, GWLP_HWNDPARENT, progman.0 as isize);

    let _ = SetWindowPos(
        hwnd,
        HWND(std::ptr::null_mut()),
        0, 0, 0, 0,
        SWP_FRAMECHANGED | SWP_SHOWWINDOW | SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
    );
    crate::log::write("soft_reparent: OK");
}

/// WinEvent callback: whenever Progman/WorkerW becomes foreground, re-assert
/// the widget at the bottom of the z-order so Show Desktop doesn't hide it.
unsafe extern "system" fn win_event_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _thread: u32,
    _time: u32,
) {
    if id_object != 0 || id_child != 0 || event != EVENT_SYSTEM_FOREGROUND_VAL {
        return;
    }

    let mut class_buf = [0u16; 128];
    let len = GetClassNameW(hwnd, &mut class_buf);
    if len <= 0 {
        return;
    }
    let class = String::from_utf16_lossy(&class_buf[..len as usize]);
    if class != "Progman" && class != "WorkerW" {
        return;
    }

    if let Some(&our_raw) = CACHED_HWND.get() {
        let our_hwnd = HWND(our_raw as *mut _);
        let _ = SetWindowPos(
            our_hwnd,
            HWND_BOTTOM,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

fn install_win_event_hook() {
    if WIN_EVENT_HOOK.get().is_some() {
        return;
    }
    unsafe {
        let hook = SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND_VAL,
            EVENT_SYSTEM_MINIMIZEEND_VAL,
            None,
            Some(win_event_proc),
            0,
            0,
            WINEVENT_OUTOFCONTEXT_VAL | WINEVENT_SKIPOWNPROCESS_VAL,
        );
        if !hook.0.is_null() {
            let _ = WIN_EVENT_HOOK.set(hook.0 as isize);
            crate::log::write("install_win_event_hook: OK");
        } else {
            crate::log::write("install_win_event_hook: FAILED");
        }
    }
}

unsafe extern "system" fn window_proc_hook(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Soft reparent + install WinEvent hook, executed on the window's thread.
    if msg == WM_APP_REPARENT {
        do_soft_reparent(hwnd);
        install_win_event_hook();
        return LRESULT(0);
    }

    if msg == WM_SYSCOMMAND
        && (wparam.0 & SC_MINIMIZE_MASK) == SC_MINIMIZE_VAL {
            return LRESULT(0);
        }

    if msg == WM_SIZE && wparam.0 == 0 {
        return LRESULT(0);
    }

    if msg == WM_SHOWWINDOW && wparam.0 == 0 {
        return LRESULT(0);
    }

    if msg == WM_WINDOWPOSCHANGING {
        let pos = lparam.0 as *mut WindowPos;
        if !pos.is_null() {
            let x = (*pos).x;
            let y = (*pos).y;
            if x == -32000 && y == -32000 {
                let mut flags = (*pos).flags;
                flags &= !SWP_HIDEWINDOW_RAW;
                flags |= SWP_NOMOVE.0 | SWP_NOSIZE.0 | SWP_SHOWWINDOW.0;
                (*pos).flags = flags;
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
    }

    // Trigger soft reparent (and WinEvent hook install) on the window thread.
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let _ = SendMessageTimeoutW(
            hwnd,
            WM_APP_REPARENT,
            WPARAM(0),
            LPARAM(0),
            SMTO_ABORTIFHUNG,
            5000,
            None,
        );
    }

    // Safety net: exclude the widget from Aero Peek.
    exclude_from_peek(hwnd_raw);

    true
}


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

pub fn exclude_from_peek(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let val: BOOL = BOOL(0);
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_EXCLUDED_FROM_PEEK,
            &val as *const BOOL as *const _,
            std::mem::size_of::<BOOL>() as u32,
        );
    }
}



pub fn ensure_visible_bottom(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        uncloak_if_needed(hwnd_raw);
        let _ = SetWindowPos(
            hwnd,
            HWND_NOTOPMOST,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0, 0, 0, 0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn hide_from_taskbar(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE) as u32;
        // Remove WS_EX_TOOLWINDOW: it blocks WM_WINDOWPOSCHANGING on some
        // Windows builds, which would defeat the Win+D interception.
        let new_ex = (ex_style & !WS_EX_APPWINDOW.0 & !WS_EX_TOOLWINDOW.0)
            | WS_EX_LAYERED.0
            | WS_EX_NOACTIVATE.0;
        SetWindowLongW(hwnd, GWL_EXSTYLE, new_ex as i32);
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            0, 0, 0, 0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

pub fn remove_minimize_box(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let style = GetWindowLongW(hwnd, GWL_STYLE) as u32;
        // Keep WS_MINIMIZEBOX: needed to receive certain system messages.
        let new_style = (style & !WS_MAXIMIZEBOX.0) | WS_MINIMIZEBOX.0;
        SetWindowLongW(hwnd, GWL_STYLE, new_style as i32);
        let _ = SetWindowPos(
            hwnd,
            HWND(std::ptr::null_mut()),
            0, 0, 0, 0,
            SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

pub fn restore_window(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let _ = ShowWindow(hwnd, SW_RESTORE);
    }
}

pub fn set_window_alpha(hwnd_raw: isize, alpha: f32) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let b_alpha = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
        let _ = SetLayeredWindowAttributes(hwnd, COLORREF(0), b_alpha, LWA_ALPHA);
    }
}

pub fn delete_taskbar_tab(hwnd_raw: isize) {
    unsafe {
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
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        if res.0 != 0 {
            return false;
        }
        data == 0
    }
}