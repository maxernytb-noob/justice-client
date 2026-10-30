//! Thin Win32 helpers (named pipes, foreground window, overlay window styles, recycle bin, sound).

use std::ffi::c_void;
use std::fs::File;
use std::io;
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use std::path::Path;
use std::sync::OnceLock;

use windows_sys::Win32::Foundation::*;
use windows_sys::Win32::Graphics::Gdi::*;
use windows_sys::Win32::Media::Audio::*;
use windows_sys::Win32::Storage::FileSystem::*;
use windows_sys::Win32::System::Pipes::*;
use windows_sys::Win32::System::Threading::*;
use windows_sys::Win32::UI::Shell::*;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Creates an outbound byte-mode named pipe server; ffmpeg opens it as a plain input file.
pub fn create_pipe(name: &str, buffer: u32) -> io::Result<File> {
    let n = wide(name);
    let h = unsafe {
        CreateNamedPipeW(
            n.as_ptr(),
            PIPE_ACCESS_OUTBOUND,
            PIPE_TYPE_BYTE | PIPE_WAIT,
            1,
            buffer,
            0,
            0,
            std::ptr::null(),
        )
    };
    if h == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(h as _) })
}

/// Blocks until a client connects to the pipe.
pub fn connect_pipe(f: &File) -> io::Result<()> {
    let ok = unsafe { ConnectNamedPipe(f.as_raw_handle() as HANDLE, std::ptr::null_mut()) };
    if ok != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

pub struct Foreground {
    pub game: String,
    /// Work area (left, top, right, bottom) of the foreground window's monitor.
    pub work: (i32, i32, i32, i32),
}

/// Name of the app in the foreground (used to name clips) and its monitor work area.
pub fn foreground() -> Foreground {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut work = (0, 0, 1920, 1080);
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTOPRIMARY);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if GetMonitorInfoW(mon, &mut mi) != 0 {
            let r = mi.rcWork;
            work = (r.left, r.top, r.right, r.bottom);
        }

        let mut title = [0u16; 256];
        let tl = GetWindowTextW(hwnd, title.as_mut_ptr(), title.len() as i32).max(0) as usize;
        let title = String::from_utf16_lossy(&title[..tl]);

        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, &mut pid);
        let mut exe = String::new();
        let ph = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !ph.is_null() {
            let mut buf = [0u16; 1024];
            let mut len = buf.len() as u32;
            if QueryFullProcessImageNameW(ph, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut len) != 0 {
                let full = String::from_utf16_lossy(&buf[..len as usize]);
                exe = Path::new(&full)
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default();
            }
            CloseHandle(ph);
        }
        Foreground { game: game_name(&exe, &title), work }
    }
}

fn game_name(exe: &str, title: &str) -> String {
    let lower = exe.to_lowercase();
    if lower.is_empty() || ["explorer", "justice-clips", "searchhost", "shellexperiencehost"].contains(&lower.as_str()) {
        return "Desktop".into();
    }
    // Window titles are usually the nicest name ("Counter-Strike 2", "Minecraft 1.21.4 - Multiplayer").
    let t = title.split(" - ").next().unwrap_or("").split(" — ").next().unwrap_or("");
    let words: Vec<&str> = t
        .split_whitespace()
        .take_while(|w| !w.starts_with(|c: char| c.is_ascii_digit()) || w.len() > 6)
        .collect();
    let from_title = words.join(" ");
    let name = if !from_title.is_empty() && from_title.chars().count() <= 32 {
        from_title
    } else {
        let mut n = exe.to_string();
        for suf in ["-Win64-Shipping", "-WinGDK-Shipping", "_x64", "-x64", "_BE", "_EAC"] {
            if let Some(i) = n.find(suf) {
                n.truncate(i);
            }
        }
        n
    };
    let clean: String = name
        .chars()
        .filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control())
        .collect();
    let clean = clean.trim().trim_end_matches('.').to_string();
    if clean.is_empty() { "Clip".into() } else { clean }
}

/// Makes the overlay a non-activating tool window so showing it never steals focus from a game.
pub fn overlay_styles(hwnd: *mut c_void) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetWindowLongPtrW(
            hwnd,
            GWL_EXSTYLE,
            ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW | WS_EX_TOPMOST | WS_EX_TRANSPARENT) as isize,
        );
    }
}

pub fn show_no_activate(hwnd: *mut c_void, x: i32, y: i32) {
    unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        );
    }
}

pub fn hide(hwnd: *mut c_void) {
    unsafe {
        ShowWindow(hwnd, SW_HIDE);
    }
}

/// Moves a file to the Recycle Bin.
pub fn recycle(path: &Path) -> io::Result<()> {
    let mut from: Vec<u16> = path.as_os_str().to_string_lossy().encode_utf16().collect();
    from.extend([0, 0]);
    let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
    op.wFunc = FO_DELETE as _;
    op.pFrom = from.as_ptr();
    op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_SILENT | FOF_NOERRORUI) as _;
    let r = unsafe { SHFileOperationW(&mut op) };
    if r == 0 && op.fAnyOperationsAborted == 0 {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::Other, format!("SHFileOperation failed ({r})")))
    }
}

pub fn set_hidden(path: &Path) {
    let w = wide(&path.to_string_lossy());
    unsafe {
        SetFileAttributesW(w.as_ptr(), FILE_ATTRIBUTE_HIDDEN);
    }
}

/// Short two-tone "clip saved" blip, synthesized once.
pub fn play_blip() {
    static WAV: OnceLock<Vec<u8>> = OnceLock::new();
    let wav = WAV.get_or_init(|| {
        let rate = 44100u32;
        let mut pcm: Vec<i16> = Vec::new();
        for (freq, dur) in [(987.77f32, 0.075f32), (1318.51, 0.14)] {
            let n = (rate as f32 * dur) as usize;
            for i in 0..n {
                let t = i as f32 / rate as f32;
                let env = (t / 0.004).min(1.0) * (-t * 28.0).exp();
                let s = (t * freq * std::f32::consts::TAU).sin() * 0.7
                    + (t * freq * 2.0 * std::f32::consts::TAU).sin() * 0.12;
                pcm.push((s * env * 0.32 * i16::MAX as f32) as i16);
            }
        }
        let data_len = (pcm.len() * 2) as u32;
        let mut w = Vec::with_capacity(44 + data_len as usize);
        w.extend(b"RIFF");
        w.extend((36 + data_len).to_le_bytes());
        w.extend(b"WAVEfmt ");
        w.extend(16u32.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(1u16.to_le_bytes());
        w.extend(rate.to_le_bytes());
        w.extend((rate * 2).to_le_bytes());
        w.extend(2u16.to_le_bytes());
        w.extend(16u16.to_le_bytes());
        w.extend(b"data");
        w.extend(data_len.to_le_bytes());
        for s in pcm {
            w.extend(s.to_le_bytes());
        }
        w
    });
    unsafe {
        PlaySoundW(wav.as_ptr() as *const u16, std::ptr::null_mut(), SND_MEMORY | SND_ASYNC | SND_NODEFAULT);
    }
}

#[link(name = "gdi32")]
extern "system" {
    fn D3DKMTSetProcessSchedulingPriorityClass(process: HANDLE, priority: i32) -> i32;
}

/// Lets the game's GPU work go before the recorder's (capture copies, encoder input). Without it the
/// recorder competes as an equal on a GPU the game keeps at 100 %, which shows up as hitches in-game.
pub fn low_gpu_priority(child: &std::process::Child) {
    const D3DKMT_SCHEDULINGPRIORITYCLASS_BELOW_NORMAL: i32 = 1;
    unsafe {
        D3DKMTSetProcessSchedulingPriorityClass(child.as_raw_handle() as _, D3DKMT_SCHEDULINGPRIORITYCLASS_BELOW_NORMAL);
    }
}

/// Puts this process in a kill-on-close job so ffmpeg children never outlive the app.
/// Puts a child (ffmpeg) into a job that dies with the launcher, so a crash never leaves the
/// recorder running. Only ffmpeg: Minecraft must survive the launcher closing.
pub fn tie_to_launcher(child: &std::process::Child) {
    use std::os::windows::io::AsRawHandle;
    use std::sync::OnceLock;
    use windows_sys::Win32::System::JobObjects::*;
    static JOB: OnceLock<usize> = OnceLock::new();
    let job = *JOB.get_or_init(|| unsafe {
        let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
        if job.is_null() {
            return 0;
        }
        let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
        info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        SetInformationJobObject(
            job,
            JobObjectExtendedLimitInformation,
            &info as *const _ as *const c_void,
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        );
        // Leaked on purpose: closing it at process exit kills the recorder.
        job as usize
    });
    if job != 0 {
        unsafe {
            AssignProcessToJobObject(job as _, child.as_raw_handle() as _);
        }
    }
}

#[derive(serde::Serialize, Clone)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

/// Outputs of the default DXGI adapter, in the same order ffmpeg's ddagrab `output_idx` uses.
pub fn monitors() -> Vec<MonitorInfo> {
    use windows::Win32::Graphics::Dxgi::*;
    let mut out = Vec::new();
    unsafe {
        let Ok(factory) = CreateDXGIFactory1::<IDXGIFactory1>() else { return out };
        let Ok(adapter) = factory.EnumAdapters1(0) else { return out };
        let mut i = 0;
        while let Ok(o) = adapter.EnumOutputs(i) {
            if let Ok(d) = o.GetDesc() {
                let r = d.DesktopCoordinates;
                let name = String::from_utf16_lossy(&d.DeviceName);
                out.push(MonitorInfo {
                    index: i,
                    name: name.trim_end_matches('\0').trim_start_matches("\\\\.\\").to_string(),
                    width: r.right - r.left,
                    height: r.bottom - r.top,
                    primary: r.left == 0 && r.top == 0,
                });
            }
            i += 1;
        }
    }
    out
}
