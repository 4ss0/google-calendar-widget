//! Platform-specific integrations. Currently Windows-only, gated behind cfg
//! so the crate still compiles on other targets (albeit without the desktop
//! overlay behavior).

#[cfg(target_os = "windows")]
pub mod windows;