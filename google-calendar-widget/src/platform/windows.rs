use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{COLORREF, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetWindowLongW, SetLayeredWindowAttributes, SetWindowLongW, SetWindowPos,
    ShowWindow, GWL_EXSTYLE, GWL_STYLE, HWND_BOTTOM, LWA_ALPHA, SWP_FRAMECHANGED, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_RESTORE, WS_EX_LAYERED, WS_EX_TOOLWINDOW,
    WS_MAXIMIZEBOX, WS_MINIMIZEBOX,
};

pub fn find_hwnd(title: &str) -> Option<isize> {
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
                    Some(hwnd.0 as isize)
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