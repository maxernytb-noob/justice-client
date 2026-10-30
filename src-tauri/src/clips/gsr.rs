//! Linux replay buffer on top of `gpu-screen-recorder` (GPU capture + encode, like ShadowPlay).
//! It runs in replay mode (`-r`); SIGUSR1 makes it write the last N seconds to its output folder,
//! which we then move into the clips folder under the usual name. SIGINT stops it.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Context};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::library;
use super::settings::Settings;
use crate::i18n::tr;

#[derive(Serialize, Clone)]
pub struct Status {
    /// "starting" | "recording" | "paused" | "error"
    pub state: String,
    pub error: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct MonitorInfo {
    pub index: u32,
    pub name: String,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
}

pub fn binary() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).map(|d| d.join("gpu-screen-recorder")).find(|p| p.is_file())
}

struct Running {
    child: Child,
    started: Instant,
}

pub struct Gsr {
    app: AppHandle,
    settings: Arc<RwLock<Settings>>,
    running: Mutex<Option<Running>>,
    status: Mutex<Status>,
    log: Arc<Mutex<VecDeque<String>>>,
    /// Paths gpu-screen-recorder printed after writing a replay.
    saved: Arc<Mutex<VecDeque<PathBuf>>>,
    paused: AtomicBool,
    quit: AtomicBool,
}

fn push_log(log: &Mutex<VecDeque<String>>, line: String) {
    let mut l = log.lock().unwrap();
    l.push_back(format!("{} {line}", chrono::Local::now().format("%H:%M:%S")));
    while l.len() > 200 {
        l.pop_front();
    }
}

/// Where gpu-screen-recorder writes replays before we rename and move them.
fn spool(s: &Settings) -> PathBuf {
    PathBuf::from(&s.clips_dir).join(".replay")
}

impl Gsr {
    pub fn start(app: AppHandle, settings: Arc<RwLock<Settings>>, paused: bool) -> Arc<Gsr> {
        let g = Arc::new(Gsr {
            app,
            settings,
            running: Mutex::new(None),
            status: Mutex::new(Status { state: if paused { "paused" } else { "starting" }.into(), error: None }),
            log: Arc::new(Mutex::new(VecDeque::new())),
            saved: Arc::new(Mutex::new(VecDeque::new())),
            paused: AtomicBool::new(paused),
            quit: AtomicBool::new(false),
        });
        let w = g.clone();
        std::thread::Builder::new().name("gsr-supervisor".into()).spawn(move || w.supervise()).expect("spawn gsr supervisor");
        g
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().iter().cloned().collect()
    }

    /// (bytes, seconds) in the buffer; gpu-screen-recorder keeps it internally, so seconds only.
    pub fn stats(&self) -> (usize, f64) {
        let r = self.running.lock().unwrap();
        let secs = r.as_ref().map(|r| r.started.elapsed().as_secs_f64()).unwrap_or(0.0);
        (0, secs.min(self.settings.read().unwrap().buffer_secs as f64))
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::SeqCst);
        if p {
            self.stop();
            self.set_status("paused", None);
        }
    }

    pub fn restart(&self) {
        self.stop();
    }

    pub fn set_buffer(&self, _secs: u32, _post: f32) {
        // The replay length is a command-line option; the supervisor restarts with the new one.
        self.stop();
    }

    pub fn shutdown(&self) {
        self.quit.store(true, Ordering::SeqCst);
        self.stop();
    }

    fn set_status(&self, state: &str, error: Option<String>) {
        let s = Status { state: state.into(), error };
        *self.status.lock().unwrap() = s.clone();
        let _ = self.app.emit("clips-status", s);
    }

    fn stop(&self) {
        if let Some(mut r) = self.running.lock().unwrap().take() {
            unsafe {
                libc::kill(r.child.id() as i32, libc::SIGINT);
            }
            for _ in 0..30 {
                if matches!(r.child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = r.child.kill();
            let _ = r.child.wait();
        }
    }

    fn supervise(self: Arc<Self>) {
        let mut fails = 0u32;
        loop {
            if self.quit.load(Ordering::SeqCst) {
                return;
            }
            std::thread::sleep(Duration::from_millis(500));
            if self.paused.load(Ordering::SeqCst) {
                fails = 0;
                continue;
            }
            // What the running process is doing: None = not started, Some(None) = alive, Some(Some(st)) = exited.
            let state = {
                let mut r = self.running.lock().unwrap();
                match r.as_mut() {
                    None => None,
                    Some(run) => {
                        let exited = run.child.try_wait().ok().flatten();
                        let up_for = run.started.elapsed();
                        if exited.is_some() {
                            *r = None;
                        }
                        Some((exited, up_for))
                    }
                }
            };
            match state {
                None => {
                    if fails > 0 {
                        // Back off after failures (5 s, 10 s, ... up to 60 s).
                        std::thread::sleep(Duration::from_secs((5 * fails as u64).min(60)));
                        if self.paused.load(Ordering::SeqCst) || self.quit.load(Ordering::SeqCst) {
                            continue;
                        }
                    }
                    if let Err(e) = self.spawn() {
                        fails += 1;
                        self.set_status("error", Some(format!("{e:#}")));
                    }
                }
                Some((None, up_for)) => {
                    if up_for > Duration::from_millis(1500) && self.status().state != "recording" {
                        fails = 0;
                        self.set_status("recording", None);
                    }
                }
                Some((Some(st), _)) => {
                    if self.paused.load(Ordering::SeqCst) || self.quit.load(Ordering::SeqCst) {
                        continue;
                    }
                    fails += 1;
                    let last = self.log.lock().unwrap().iter().rev().take(3).cloned().collect::<Vec<_>>().join(" | ");
                    self.set_status("error", Some(l!("gpu-screen-recorder skončil ({}): {}", "gpu-screen-recorder exited ({}): {}", st, last)));
                }
            }
        }
    }

    fn spawn(&self) -> anyhow::Result<()> {
        let bin = binary().ok_or_else(|| {
            anyhow!(tr(
                "gpu-screen-recorder není nainstalovaný (Arch: yay -S gpu-screen-recorder)",
                "gpu-screen-recorder isn't installed (Arch: yay -S gpu-screen-recorder)"
            ))
        })?;
        let s = self.settings.read().unwrap().clone();
        let out = spool(&s);
        std::fs::create_dir_all(&out).context("replay folder")?;

        let monitors = monitors();
        let target = monitors.get(s.monitor as usize).map(|m| m.name.clone()).unwrap_or_else(|| "screen".into());
        let quality = match s.quality.as_str() {
            "small" => "high",
            "high" => "ultra",
            _ => "very_high",
        };
        let out_dev = if s.game_device.is_empty() { "default_output".to_string() } else { s.game_device.clone() };
        let in_dev = if s.mic_device.is_empty() { "default_input".to_string() } else { s.mic_device.clone() };

        let mut cmd = Command::new(bin);
        cmd.args(["-w", &target, "-f", &s.fps.to_string(), "-c", "mp4", "-q", quality, "-r", &s.buffer_secs.to_string()])
            .args(["-cursor", if s.draw_mouse { "yes" } else { "no" }]);
        // Same track layout as the Windows recorder: [game + mic mix, game, mic].
        match (s.game_audio, s.mic) {
            (true, true) => {
                cmd.args(["-a", &format!("{out_dev}|{in_dev}"), "-a", &out_dev, "-a", &in_dev]);
            }
            (true, false) => {
                cmd.args(["-a", &out_dev]);
            }
            (false, true) => {
                cmd.args(["-a", &in_dev]);
            }
            _ => {}
        }
        cmd.arg("-o").arg(&out).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        push_log(&self.log, format!("start: {cmd:?}"));
        self.set_status("starting", None);
        let mut child = cmd.spawn().context("gpu-screen-recorder")?;

        // stdout: one path per saved replay. stderr: diagnostics for the log.
        if let Some(o) = child.stdout.take() {
            let saved = self.saved.clone();
            let log = self.log.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(o).lines().map_while(Result::ok) {
                    let t = line.trim();
                    if t.ends_with(".mp4") {
                        saved.lock().unwrap().push_back(PathBuf::from(t));
                    }
                    push_log(&log, t.to_string());
                }
            });
        }
        if let Some(e) = child.stderr.take() {
            let log = self.log.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(e).lines().map_while(Result::ok) {
                    push_log(&log, line);
                }
            });
        }
        *self.running.lock().unwrap() = Some(Running { child, started: Instant::now() });
        Ok(())
    }

    /// Asks for the replay and moves it into the clips folder as "<game> <date>.mp4".
    pub fn save(&self, game: &str, s: &Settings) -> anyhow::Result<PathBuf> {
        let since = SystemTime::now();
        {
            let r = self.running.lock().unwrap();
            let run = r.as_ref().ok_or_else(|| anyhow!(tr("nahrávání neběží", "recording is not running")))?;
            self.saved.lock().unwrap().clear();
            unsafe {
                libc::kill(run.child.id() as i32, libc::SIGUSR1);
            }
        }
        let spool = spool(s);
        let mut file = None;
        for _ in 0..150 {
            std::thread::sleep(Duration::from_millis(100));
            if let Some(p) = self.saved.lock().unwrap().pop_front() {
                file = Some(p);
                break;
            }
            // Older versions don't print the path: take the newest file written since the signal.
            if let Some(p) = newest_since(&spool, since) {
                std::thread::sleep(Duration::from_millis(400));
                file = Some(p);
                break;
            }
        }
        let src = file.ok_or_else(|| anyhow!(tr("gpu-screen-recorder replay neuložil", "gpu-screen-recorder didn't write the replay")))?;
        let clips = PathBuf::from(&s.clips_dir);
        let dir = if s.game_folders { clips.join(game) } else { clips };
        std::fs::create_dir_all(&dir)?;
        let stem = format!("{} {}", game, chrono::Local::now().format("%Y-%m-%d %H-%M-%S"));
        let dst = library::unique(&dir, &stem, "mp4");
        if std::fs::rename(&src, &dst).is_err() {
            std::fs::copy(&src, &dst)?;
            let _ = std::fs::remove_file(&src);
        }
        Ok(dst)
    }
}

fn newest_since(dir: &Path, since: SystemTime) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            let t = m.modified().ok()?;
            (m.is_file() && t >= since && e.path().extension().is_some_and(|x| x == "mp4")).then(|| (t, e.path()))
        })
        .max_by_key(|(t, _)| *t)
        .map(|(_, p)| p)
}

/// Monitors from `gpu-screen-recorder --list-capture-options` ("DP-1|1920x1080" lines).
pub fn monitors() -> Vec<MonitorInfo> {
    let Some(bin) = binary() else { return vec![] };
    let Ok(out) = Command::new(bin).arg("--list-capture-options").output() else { return vec![] };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let (name, res) = l.split_once('|')?;
            let (w, h) = res.split_once('x')?;
            Some((name.to_string(), w.trim().parse().ok()?, h.trim().parse().ok()?))
        })
        .enumerate()
        .map(|(i, (name, width, height))| MonitorInfo { index: i as u32, name, width, height, primary: i == 0 })
        .collect()
}

/// (outputs, inputs) from `gpu-screen-recorder --list-audio-devices` ("name|Description" lines).
pub fn audio_devices() -> (Vec<String>, Vec<String>) {
    let Some(bin) = binary() else { return (vec![], vec![]) };
    let Ok(out) = Command::new(bin).arg("--list-audio-devices").output() else { return (vec![], vec![]) };
    let (mut outputs, mut inputs) = (vec![], vec![]);
    for l in String::from_utf8_lossy(&out.stdout).lines() {
        let name = l.split('|').next().unwrap_or("").trim().to_string();
        if name.is_empty() || name.starts_with("default_") {
            continue;
        }
        // PulseAudio/PipeWire monitors of an output are what "PC audio" records.
        if name.ends_with(".monitor") {
            outputs.push(name);
        } else {
            inputs.push(name);
        }
    }
    (outputs, inputs)
}
