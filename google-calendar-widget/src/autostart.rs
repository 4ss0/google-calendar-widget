use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::Foundation::WIN32_ERROR;
use windows::Win32::System::Registry::{
    RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
    RegSetValueExW, HKEY, HKEY_CURRENT_USER, KEY_READ, KEY_WRITE,
    REG_OPTION_NON_VOLATILE, REG_SZ,
};

const RUN_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Run";
const VALUE_NAME: &str = "GoogleCalendarWidget";

fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(std::iter::once(0)).collect()
}

fn exe_command() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let exe_str = exe.to_string_lossy().to_string();
    Ok(format!("\"{}\" --minimized", exe_str))
}

fn win32_ok(err: WIN32_ERROR) -> Result<(), String> {
    if err.0 == 0 {
        Ok(())
    } else {
        Err(format!("WIN32_ERROR: {}", err.0))
    }
}

pub fn is_autostart_enabled() -> bool {
    let subkey = to_wide(RUN_KEY);
    let value = to_wide(VALUE_NAME);

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

        let mut buf = [0u16; 1024];
        let mut size = (buf.len() * 2) as u32;
        let result = RegQueryValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            None,
            None,
            Some(buf.as_mut_ptr() as *mut u8),
            Some(&mut size),
        );
        let _ = RegCloseKey(hkey);
        result.0 == 0 && size > 2
    }
}

pub fn enable_autostart() -> Result<(), String> {
    let cmd = exe_command()?;
    let subkey = to_wide(RUN_KEY);
    let value = to_wide(VALUE_NAME);
    let data = to_wide(&cmd);

    unsafe {
        let mut hkey = HKEY::default();
        win32_ok(RegCreateKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            PCWSTR::null(),
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        ))
        .map_err(|e| format!("RegCreateKeyExW: {}", e))?;

        let bytes = std::slice::from_raw_parts(data.as_ptr() as *const u8, data.len() * 2);
        let set = RegSetValueExW(
            hkey,
            PCWSTR(value.as_ptr()),
            0,
            REG_SZ,
            Some(bytes),
        );
        let _ = RegCloseKey(hkey);
        win32_ok(set).map_err(|e| format!("RegSetValueExW: {}", e))?;
    }
    Ok(())
}

pub fn disable_autostart() -> Result<(), String> {
    let subkey = to_wide(RUN_KEY);
    let value = to_wide(VALUE_NAME);

    unsafe {
        let mut hkey = HKEY::default();
        let open = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            PCWSTR(subkey.as_ptr()),
            0,
            KEY_WRITE,
            &mut hkey,
        );
        if open.0 != 0 {
            return Ok(());
        }

        let del = RegDeleteValueW(hkey, PCWSTR(value.as_ptr()));
        let _ = RegCloseKey(hkey);
        win32_ok(del).map_err(|e| format!("RegDeleteValueW: {}", e))?;
    }
    Ok(())
}

pub fn heal_autostart() {
    if is_autostart_enabled() {
        let _ = enable_autostart();
    }
}