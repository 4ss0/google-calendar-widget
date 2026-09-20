use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

fn log_path() -> PathBuf {
    crate::config::AppConfig::config_dir().join("debug.log")
}

pub fn path() -> PathBuf {
    log_path()
}

pub fn write(msg: &str) {
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
    let line = format!("[{}] {}", now, msg);

    // Always mirror to stderr so messages are visible when running from a terminal,
    // even if writing to the log file fails.
    eprintln!("{}", line);

    let p = log_path();
    if let Some(parent) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            eprintln!("[log] create_dir_all({:?}) failed: {}", parent, e);
            return;
        }
    }

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