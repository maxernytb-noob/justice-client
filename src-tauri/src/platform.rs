//! Everything that differs between Windows and Linux lives here.

use std::path::Path;

/// Name Mojang uses in library/argument rules.
pub const OS_NAME: &str = if cfg!(windows) { "windows" } else if cfg!(target_os = "macos") { "osx" } else { "linux" };
pub const CLASSPATH_SEP: &str = if cfg!(windows) { ";" } else { ":" };
/// Java binary without a console window (javaw on Windows).
pub const JAVA_GUI: &str = if cfg!(windows) { "javaw.exe" } else { "java" };
pub const JAVA_CONSOLE: &str = if cfg!(windows) { "java.exe" } else { "java" };
/// Adoptium API os name.
pub const ADOPTIUM_OS: &str = if cfg!(windows) { "windows" } else { "linux" };

/// Don't flash a console window for child processes on Windows.
pub fn hide_window(cmd: &mut tokio::process::Command) {
    #[cfg(windows)]
    {
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn hide_window_std(cmd: &mut std::process::Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

pub fn kill_pid(pid: u32) {
    #[cfg(windows)]
    {
        let mut c = std::process::Command::new("taskkill");
        c.args(["/PID", &pid.to_string(), "/T", "/F"]);
        hide_window_std(&mut c);
        let _ = c.output();
    }
    #[cfg(unix)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

pub fn raise_priority(child: &tokio::process::Child) {
    #[cfg(windows)]
    {
        extern "system" {
            fn SetPriorityClass(h: *mut std::ffi::c_void, class: u32) -> i32;
        }
        const ABOVE_NORMAL_PRIORITY_CLASS: u32 = 0x8000;
        if let Some(h) = child.raw_handle() {
            unsafe {
                SetPriorityClass(h as *mut _, ABOVE_NORMAL_PRIORITY_CLASS);
            }
        }
    }
    // Raising priority on Linux needs root (negative nice), so we leave it alone.
    #[cfg(not(windows))]
    let _ = child;
}

pub fn total_ram_mb() -> u64 {
    #[cfg(windows)]
    unsafe {
        #[repr(C)]
        struct MemoryStatusEx {
            len: u32,
            load: u32,
            total_phys: u64,
            avail_phys: u64,
            total_page: u64,
            avail_page: u64,
            total_virt: u64,
            avail_virt: u64,
            avail_ext: u64,
        }
        extern "system" {
            fn GlobalMemoryStatusEx(buf: *mut MemoryStatusEx) -> i32;
        }
        let mut s: MemoryStatusEx = std::mem::zeroed();
        s.len = std::mem::size_of::<MemoryStatusEx>() as u32;
        if GlobalMemoryStatusEx(&mut s) != 0 {
            return s.total_phys / 1024 / 1024;
        }
    }
    #[cfg(unix)]
    if let Ok(t) = std::fs::read_to_string("/proc/meminfo") {
        if let Some(kb) = t
            .lines()
            .find(|l| l.starts_with("MemTotal:"))
            .and_then(|l| l.split_whitespace().nth(1))
            .and_then(|v| v.parse::<u64>().ok())
        {
            return kb / 1024;
        }
    }
    8192
}

/// Free bytes on the drive holding `path` (walks up to an existing ancestor).
pub fn free_space(path: &Path) -> Option<u64> {
    let mut p = path.to_path_buf();
    while !p.exists() {
        p = p.parent()?.to_path_buf();
    }
    #[cfg(windows)]
    {
        let wide: Vec<u16> = std::os::windows::ffi::OsStrExt::encode_wide(p.as_os_str()).chain(Some(0)).collect();
        extern "system" {
            fn GetDiskFreeSpaceExW(dir: *const u16, avail: *mut u64, total: *mut u64, free: *mut u64) -> i32;
        }
        let mut avail = 0u64;
        let ok = unsafe { GetDiskFreeSpaceExW(wide.as_ptr(), &mut avail, std::ptr::null_mut(), std::ptr::null_mut()) };
        (ok != 0).then_some(avail)
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let c = std::ffi::CString::new(p.as_os_str().as_bytes()).ok()?;
        let mut s: libc::statvfs = unsafe { std::mem::zeroed() };
        let ok = unsafe { libc::statvfs(c.as_ptr(), &mut s) } == 0;
        ok.then(|| s.f_bavail as u64 * s.f_frsize as u64)
    }
}

/// CPU model name and GPU names for the Settings "your PC" card.
pub fn hardware_names() -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        let mut c = std::process::Command::new("powershell");
        c.args([
            "-NoProfile",
            "-Command",
            "(Get-CimInstance Win32_Processor).Name; '---'; (Get-CimInstance Win32_VideoController).Name",
        ]);
        hide_window_std(&mut c);
        let out = c.output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned()).unwrap_or_default();
        let (cpu, gpus) = out.split_once("---").unwrap_or((&out, ""));
        (
            cpu.trim().to_string(),
            gpus.lines().map(str::trim).filter(|l| !l.is_empty()).map(String::from).collect(),
        )
    }
    #[cfg(not(windows))]
    {
        let cpu = std::fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|t| t.lines().find(|l| l.starts_with("model name")).and_then(|l| l.split_once(':')).map(|(_, v)| v.trim().to_string()))
            .unwrap_or_default();
        let gpus = std::process::Command::new("lspci")
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
            .unwrap_or_default()
            .lines()
            .filter(|l| l.contains("VGA compatible controller") || l.contains("3D controller") || l.contains("Display controller"))
            .filter_map(|l| l.splitn(3, ':').nth(2).map(|s| s.trim().to_string()))
            .collect();
        (cpu, gpus)
    }
}

/// Discord's local IPC endpoints: named pipes on Windows, unix sockets elsewhere.
pub trait IpcStream: std::io::Read + std::io::Write + Send {}
impl<T: std::io::Read + std::io::Write + Send> IpcStream for T {}

pub fn discord_connect_raw() -> Option<Box<dyn IpcStream>> {
    #[cfg(windows)]
    {
        for i in 0..10 {
            if let Ok(f) = std::fs::OpenOptions::new().read(true).write(true).open(format!(r"\\.\pipe\discord-ipc-{i}")) {
                return Some(Box::new(f));
            }
        }
        None
    }
    #[cfg(unix)]
    {
        let mut dirs: Vec<std::path::PathBuf> = vec![];
        for var in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            if let Ok(v) = std::env::var(var) {
                dirs.push(v.into());
            }
        }
        dirs.push("/tmp".into());
        // Flatpak / Snap Discord put the socket in a sub-folder.
        let subs = ["", "app/com.discordapp.Discord", "snap.discord"];
        for d in &dirs {
            for sub in subs {
                for i in 0..10 {
                    let p = d.join(sub).join(format!("discord-ipc-{i}"));
                    if let Ok(s) = std::os::unix::net::UnixStream::connect(&p) {
                        return Some(Box::new(s));
                    }
                }
            }
        }
        None
    }
}
