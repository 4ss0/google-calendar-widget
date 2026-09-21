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

pub fn line() {
    write("------------------------------------------------------------");
}