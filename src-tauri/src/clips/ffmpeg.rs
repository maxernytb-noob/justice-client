#[cfg(windows)]
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg_attr(not(windows), allow(dead_code))]
pub const BELOW_NORMAL: u32 = 0x0000_4000;

#[cfg(windows)]
const EXE: &str = "ffmpeg.exe";
#[cfg(not(windows))]
const EXE: &str = "ffmpeg";

/// Locates ffmpeg: explicit setting, next to the app, PATH, then (Windows) the winget install dir.
pub fn find(custom: &str) -> Option<PathBuf> {
    if !custom.trim().is_empty() {
        let p = PathBuf::from(custom.trim());
        return p.is_file().then_some(p);
    }
    if let Ok(exe) = std::env::current_exe() {
        let p = exe.with_file_name(EXE);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let p = dir.join(EXE);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if !cfg!(windows) {
        return None;
    }
    let winget = dirs::data_local_dir()?.join("Microsoft\\WinGet\\Packages");
    for pkg in std::fs::read_dir(winget).ok()?.flatten() {
        if !pkg.file_name().to_string_lossy().to_lowercase().contains("ffmpeg") {
            continue;
        }
        for sub in std::fs::read_dir(pkg.path()).ok()?.flatten() {
            let p = sub.path().join("bin\\ffmpeg.exe");
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

/// A hidden-console ffmpeg command with the given priority class flag (0 = normal).
pub fn command(ffmpeg: &Path, priority: u32) -> Command {
    let mut c = Command::new(ffmpeg);
    #[cfg(windows)]
    c.creation_flags(CREATE_NO_WINDOW | priority);
    #[cfg(not(windows))]
    let _ = priority;
    c.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    c
}

/// Whether this machine's ffmpeg can actually encode with `hevc_nvenc` (NVIDIA GPU + driver).
/// Probed once with a tiny throwaway encode and cached. On anything without working NVENC — most
/// Linux boxes, AMD/Intel GPUs — this is false and callers fall back to a software encoder instead
/// of letting ffmpeg fail with "Invalid argument" and write nothing.
pub fn hevc_nvenc_ok(ffmpeg: &Path) -> bool {
    use std::sync::OnceLock;
    static OK: OnceLock<bool> = OnceLock::new();
    *OK.get_or_init(|| {
        command(ffmpeg, 0)
            .args([
                "-hide_banner", "-loglevel", "error", "-nostdin",
                "-f", "lavfi", "-i", "color=black:s=128x128:r=5:d=0.2",
                "-frames:v", "3", "-c:v", "hevc_nvenc", "-f", "null", "-",
            ])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
    })
}

/// Runs ffmpeg to completion, returning the tail of stderr as the error.
pub fn run(ffmpeg: &Path, args: &[String], priority: u32) -> anyhow::Result<()> {
    let out = command(ffmpeg, priority)
        .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-y"])
        .args(args)
        .stderr(Stdio::piped())
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        anyhow::bail!("{}", tail(&String::from_utf8_lossy(&out.stderr)))
    }
}

pub fn tail(s: &str) -> String {
    let lines: Vec<&str> = s.lines().filter(|l| !l.trim().is_empty()).collect();
    lines[lines.len().saturating_sub(3)..].join("\n")
}

#[cfg_attr(not(windows), allow(dead_code))]
pub struct Quality {
    pub cq: u32,
    pub maxrate_mbps: u32,
    pub preset: &'static str,
}

/// NVENC HEVC presets for live capture. Lookahead and temporal AQ are left out on purpose: they
/// run on CUDA and made the game stutter on a GTX 1660 SUPER already running at 100 %.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn quality(q: &str) -> Quality {
    match q {
        "small" => Quality { cq: 28, maxrate_mbps: 16, preset: "p5" },
        "high" => Quality { cq: 20, maxrate_mbps: 60, preset: "p6" },
        _ => Quality { cq: 24, maxrate_mbps: 30, preset: "p5" },
    }
}
