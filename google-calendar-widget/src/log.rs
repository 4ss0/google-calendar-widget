//! Minimal file logger with rotation.
//!
//! There's no external logging crate: every call appends a timestamped line to
//! `debug.log` under the config dir and echoes to stderr (useful in a console
//! build). Files larger than 5 MB are renamed to `debug.log.1` before the next
//! write.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

const MAX_LOG_BYTES: u64 = 5 * 1024 * 1024;

fn log_path() -> PathBuf {
    crate::config::AppConfig::config_dir().join("debug.log")
}

fn rotated_path() -> PathBuf {
    crate::config::AppConfig::config_dir().join("debug.log.1")
}

pub fn path() -> PathBuf {
    log_path()
}

fn rotate_if_needed(p: &PathBuf) {
    let too_big = std::fs::metadata(p).map(|m| m.len() > MAX_LOG_BYTES).unwrap_or(false);
    if !too_big {
        return;
    }
    // Single-generation rotation: the previous .1 is dropped.
    let old = rotated_path();
    let _ = std::fs::remove_file(&old);
    let _ = std::fs::rename(p, &old);
}

pub fn write(msg: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] {}", now, msg);

    eprintln!("{}", line);

    let p = log_path();
    if let Some(parent) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("[log] create_dir_all({:?}) failed: {}", parent, e);
            return;
        }
    }

    rotate_if_needed(&p);

    // Open-append-close per call: cheap enough at our rates and avoids keeping
    // a file handle alive across the process lifetime.
    match OpenOptions::new().create(true).append(true).open(&p) {
        Ok(mut f) => {
            if let Err(e) = writeln!(f, "{}", line) {
                eprintln!("[log] write({:?}) failed: {}", p, e);
            }
        }
        Err(e) => {
            eprintln!("[log] open({:?}) failed: {}", p, e);
        }
    }
}

/// Writes a visual separator line, useful between app sessions.
pub fn line() {
    write("------------------------------------------------------------");
}