//! Debug log. Every line goes to stderr (visible when the launcher is started from a terminal) and to
//! `<config>/JusticeClient/launcher.log`, which holds the current session so users can send it in.

use once_cell::sync::Lazy;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

pub fn path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("JusticeClient").join("launcher.log")
}

static FILE: Lazy<Mutex<Option<std::fs::File>>> = Lazy::new(|| {
    let p = path();
    let _ = std::fs::create_dir_all(p.parent().unwrap());
    Mutex::new(std::fs::File::create(p).ok())
});

pub fn write(level: &str, module: &str, msg: &str) {
    let module = module.trim_start_matches("justice_client_lib::").trim_start_matches("justice_client::");
    let line = format!("[{} {level:<5} {module}] {msg}", chrono::Local::now().format("%H:%M:%S%.3f"));
    eprintln!("{line}");
    if let Ok(mut f) = FILE.lock() {
        if let Some(f) = f.as_mut() {
            let _ = writeln!(f, "{line}");
        }
    }
}

macro_rules! ldebug {
    ($($t:tt)*) => { $crate::logger::write("DEBUG", module_path!(), &format!($($t)*)) };
}
macro_rules! linfo {
    ($($t:tt)*) => { $crate::logger::write("INFO", module_path!(), &format!($($t)*)) };
}
macro_rules! lwarn {
    ($($t:tt)*) => { $crate::logger::write("WARN", module_path!(), &format!($($t)*)) };
}
macro_rules! lerror {
    ($($t:tt)*) => { $crate::logger::write("ERROR", module_path!(), &format!($($t)*)) };
}
