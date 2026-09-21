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
    BringWindowToTop, CallWindowProcW, FindWindowW, GetClassNameW, GetForegroundWindow, 
    GetWindowLongW, IsIconic, SetForegroundWindow,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowLongW, SetWindowPos, ShowWindow,
    GWL_EXSTYLE, GWL_STYLE, GWLP_WNDPROC, HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST,
    LWA_ALPHA, SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW, SW_HIDE, SW_RESTORE, SW_SHOW, WNDPROC,
    WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
};

static CACHED_HWND: OnceLock<isize> = OnceLock::new();
static ORIGINAL_WNDPROC: OnceLock<isize> = OnceLock::new();
static HOOK_INSTALLED: OnceLock<bool> = OnceLock::new();

const WM_WINDOWPOSCHANGING: u32 = 0x0046;

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

pub fn force_show_on_desktop(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);

        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = ShowWindow(hwnd, SW_SHOW);

        let _ = SetWindowPos(
            hwnd,
            HWND_TOP,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );

        let _ = SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE | SWP_FRAMECHANGED,
        );

        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);
        let _ = DwmFlush();
    }
}

pub fn ensure_visible_bottom(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);

        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }

        uncloak_if_needed(hwnd_raw);

        if is_desktop_foreground() {
            force_show_on_desktop(hwnd_raw);
        } else {
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
}

pub fn hide_from_taskbar(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            (ex_style & !(WS_EX_TOOLWINDOW.0 as i32)) | (WS_EX_LAYERED.0 as i32),
        );
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