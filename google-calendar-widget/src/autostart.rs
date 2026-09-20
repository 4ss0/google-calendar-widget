use auto_launch::AutoLaunch;

fn autostart_instance() -> AutoLaunch {
    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_str = exe_path.to_string_lossy().to_string();
    AutoLaunch::new("GoogleCalendarWidget", &exe_str, &["--minimized"])
}

pub fn is_autostart_enabled() -> bool {
    autostart_instance().is_enabled().unwrap_or(false)
}

pub fn enable_autostart() -> Result<(), String> {
    autostart_instance().enable().map_err(|e| e.to_string())
}

pub fn disable_autostart() -> Result<(), String> {
    autostart_instance()
        .disable()
        .map_err(|e| e.to_string())
}