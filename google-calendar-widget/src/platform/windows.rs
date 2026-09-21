use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;
use windows::core::{IUnknown, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ,
};
use windows::Win32::UI::Shell::{ITaskbarList, TaskbarList};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos,
    ShowWindow, GWL_EXSTYLE, GWL_STYLE, HWND_BOTTOM, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
};

static CACHED_HWND: OnceLock<isize> = OnceLock::new();

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

pub fn set_bottom(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let _ = SetWindowPos(
            hwnd,
            HWND_BOTTOM,
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

pub fn hide_from_taskbar(hwnd_raw: isize) {
    unsafe {
        let hwnd = HWND(hwnd_raw as *mut _);
        let ex_style = GetWindowLongW(hwnd, GWL_EXSTYLE);
        SetWindowLongW(
            hwnd,
            GWL_EXSTYLE,
            ex_style | (WS_EX_TOOLWINDOW.0 as i32) | (WS_EX_LAYERED.0 as i32),
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
        let mask = (WS_MINIMIZEBOX.0 as i32) | (WS_MAXIMIZEBOX.0 as i32);
        let new_style = style & !mask;
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