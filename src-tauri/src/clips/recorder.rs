//! Replay buffer: ffmpeg (ddagrab -> NVENC HEVC, zero-copy on the GPU) writes MPEG-TS to stdout;
//! we keep the last N seconds in RAM split at keyframes, so saving a clip is a pure remux.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use super::audio::{self, Capture, Kind, VideoStart};
use super::ffmpeg;
use super::win;
use super::settings::Settings;

const TS: usize = 188;
const PMT_PID: u16 = 0x1000;
const VIDEO_PID: u16 = 0x100;

struct Gop {
    t: Instant,
    data: Arc<Vec<u8>>,
}

/// Which audio tracks the buffered stream carries, in stream order after video.
#[derive(Clone, Copy, Default, Debug)]
pub struct Tracks {
    pub game: bool,
    pub mic: bool,
}

#[derive(Default)]
pub struct Ring {
    pat: Vec<u8>,
    pmt: Vec<u8>,
    gops: VecDeque<Gop>,
    cur: Option<(Instant, Vec<u8>)>,
    keep: Duration,
    bytes: usize,
    last_gop: usize,
    pub tracks: Tracks,
}

pub struct Snapshot {
    pub parts: Vec<Arc<Vec<u8>>>,
    pub tracks: Tracks,
}

impl Ring {
    fn clear(&mut self) {
        let keep = self.keep;
        *self = Ring { keep, ..Default::default() };
    }

    fn push(&mut self, p: &[u8]) {
        if p[0] != 0x47 {
            return;
        }
        let pid = (((p[1] & 0x1f) as u16) << 8) | p[2] as u16;
        match pid {
            0 => {
                self.pat = p.to_vec();
                return;
            }
            PMT_PID => {
                self.pmt = p.to_vec();
                return;
            }
            0x11 => return,
            _ => {}
        }
        let pusi = p[1] & 0x40 != 0;
        let rai = p[3] & 0x20 != 0 && p[4] > 0 && p[5] & 0x40 != 0;
        if pid == VIDEO_PID && pusi && rai {
            let now = Instant::now();
            if let Some((t, d)) = self.cur.take() {
                self.last_gop = d.len();
                self.gops.push_back(Gop { t, data: Arc::new(d) });
            }
            self.cur = Some((now, Vec::with_capacity(self.last_gop + self.last_gop / 4 + 65536)));
            // Drop GOPs that end before the buffer window.
            while self.gops.len() > 1 && now.duration_since(self.gops[1].t) >= self.keep {
                let g = self.gops.pop_front().unwrap();
                self.bytes -= g.data.len();
            }
            if self.gops.len() == 1 && now.duration_since(self.gops[0].t) > self.keep + Duration::from_secs(5) {
                let g = self.gops.pop_front().unwrap();
                self.bytes -= g.data.len();
            }
        }
        if let Some((_, d)) = &mut self.cur {
            d.extend_from_slice(p);
            self.bytes += TS;
        }
    }

    pub fn snapshot(&self, dur: Duration) -> Option<Snapshot> {
        let (_, cur) = self.cur.as_ref()?;
        if self.pat.is_empty() || self.pmt.is_empty() {
            return None;
        }
        let now = Instant::now();
        // Latest GOP that starts at or before `now - dur` (or the oldest we have).
        let mut first = 0;
        for (i, g) in self.gops.iter().enumerate() {
            if now.duration_since(g.t) >= dur {
                first = i;
            }
        }
        let mut parts = vec![Arc::new([self.pat.clone(), self.pmt.clone()].concat())];
        parts.extend(self.gops.iter().skip(first).map(|g| g.data.clone()));
        parts.push(Arc::new(cur.clone()));
        Some(Snapshot { parts, tracks: self.tracks })
    }

    pub fn stats(&self) -> (usize, f64) {
        let secs = self.gops.front().map(|g| g.t.elapsed().as_secs_f64()).unwrap_or(0.0);
        (self.bytes, secs)
    }
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// "starting" | "recording" | "paused" | "error"
    pub state: String,
    pub error: Option<String>,
}

enum Cmd {
    Restart,
    Pause(bool),
    Quit,
}

pub struct Recorder {
    pub ring: Arc<Mutex<Ring>>,
    status: Arc<Mutex<Status>>,
    log: Arc<Mutex<VecDeque<String>>>,
    tx: Mutex<Sender<Cmd>>,
    paused: AtomicBool,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl Recorder {
    /// `paused`: start idle (the launcher runs the buffer only while Minecraft is running).
    pub fn start(app: AppHandle, settings: Arc<RwLock<Settings>>, paused: bool) -> Arc<Recorder> {
        let (tx, rx) = mpsc::channel();
        let keep = { let s = settings.read().unwrap(); keep_for(s.buffer_secs, s.post_secs) };
        let rec = Arc::new(Recorder {
            ring: Arc::new(Mutex::new(Ring { keep, ..Default::default() })),
            status: Arc::new(Mutex::new(Status { state: if paused { "paused" } else { "starting" }.into(), error: None })),
            log: Arc::new(Mutex::new(VecDeque::new())),
            tx: Mutex::new(tx),
            paused: AtomicBool::new(paused),
            worker: Mutex::new(None),
        });
        let r = rec.clone();
        let h = std::thread::Builder::new()
            .name("recorder".into())
            .spawn(move || r.supervise(app, settings, rx))
            .expect("spawn recorder");
        *rec.worker.lock().unwrap() = Some(h);
        rec
    }

    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn log(&self) -> Vec<String> {
        self.log.lock().unwrap().iter().cloned().collect()
    }


    pub fn restart(&self) {
        let _ = self.tx.lock().unwrap().send(Cmd::Restart);
    }

    pub fn set_paused(&self, p: bool) {
        self.paused.store(p, Ordering::Relaxed);
        let _ = self.tx.lock().unwrap().send(Cmd::Pause(p));
    }

    pub fn set_buffer(&self, secs: u32, post: f32) {
        self.ring.lock().unwrap().keep = keep_for(secs, post);
    }

    pub fn shutdown(&self) {
        let _ = self.tx.lock().unwrap().send(Cmd::Quit);
        if let Some(h) = self.worker.lock().unwrap().take() {
            let _ = h.join();
        }
    }

    fn set_status(&self, app: &AppHandle, state: &str, error: Option<String>) {
        let s = Status { state: state.into(), error };
        *self.status.lock().unwrap() = s.clone();
        let _ = app.emit("clips-status", s);
        super::update_tray(app, state);
    }

    fn push_log(&self, line: String) {
        push_log(&self.log, line);
    }

    fn supervise(&self, app: AppHandle, settings: Arc<RwLock<Settings>>, rx: Receiver<Cmd>) {
        let mut fails = 0u32;
        loop {
            if self.paused.load(Ordering::Relaxed) {
                self.set_status(&app, "paused", None);
                match rx.recv() {
                    Ok(Cmd::Quit) | Err(_) => return,
                    Ok(Cmd::Pause(true)) | Ok(Cmd::Restart) => continue,
                    Ok(Cmd::Pause(false)) => fails = 0,
                }
                continue;
            }

            let s = settings.read().unwrap().clone();
            self.set_status(&app, "starting", None);
            self.ring.lock().unwrap().clear();
            let started = Instant::now();
            let mut err: Option<String> = None;
            match Session::start(&s, self.ring.clone(), self.log.clone()) {
                Ok(mut sess) => {
                    self.push_log(format!("capture started: {}", sess.describe));
                    let mut last_dev_check = Instant::now();
                    let dev_game = audio::default_name(Kind::Loopback);
                    let dev_mic = audio::default_name(Kind::Mic);
                    let mut shown_recording = false;
                    loop {
                        match rx.recv_timeout(Duration::from_millis(500)) {
                            Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => {
                                sess.stop();
                                return;
                            }
                            Ok(Cmd::Restart) => {
                                sess.stop();
                                fails = 0;
                                break;
                            }
                            Ok(Cmd::Pause(p)) => {
                                if p {
                                    sess.stop();
                                    self.ring.lock().unwrap().clear();
                                    break;
                                }
                            }
                            Err(RecvTimeoutError::Timeout) => {}
                        }
                        if !shown_recording && sess.got_data() {
                            shown_recording = true;
                            self.set_status(&app, "recording", None);
                        }
                        if let Some(e) = sess.dead() {
                            err = Some(e);
                            sess.stop();
                            break;
                        }
                        // Follow Windows default-device changes (headset plugged in etc.).
                        if last_dev_check.elapsed() > Duration::from_secs(3) {
                            last_dev_check = Instant::now();
                            let changed = (s.game_audio && s.game_device.is_empty() && audio::default_name(Kind::Loopback) != dev_game)
                                || (s.mic && s.mic_device.is_empty() && audio::default_name(Kind::Mic) != dev_mic);
                            if changed {
                                self.push_log("default audio device changed, restarting capture".into());
                                sess.stop();
                                break;
                            }
                        }
                    }
                }
                Err(e) => err = Some(format!("{e:#}")),
            }

            if let Some(e) = err {
                self.push_log(format!("capture error: {e}"));
                if started.elapsed() > Duration::from_secs(30) {
                    fails = 0;
                }
                fails += 1;
                self.set_status(&app, "error", Some(e));
                // Back off, but stay responsive to commands.
                let wait = Duration::from_secs((fails as u64 * 2).min(15));
                match rx.recv_timeout(wait) {
                    Ok(Cmd::Quit) | Err(RecvTimeoutError::Disconnected) => return,
                    Ok(Cmd::Restart) => fails = 0,
                    _ => {}
                }
            }
        }
    }
}

/// Buffer window: clip length + post-hotkey time + one GOP of slack.
fn keep_for(secs: u32, post: f32) -> Duration {
    Duration::from_secs(secs as u64 + post.ceil() as u64 + 3)
}

fn push_log(log: &Mutex<VecDeque<String>>, line: String) {
    let mut l = log.lock().unwrap();
    l.push_back(format!("{} {}", chrono::Local::now().format("%H:%M:%S"), line));
    while l.len() > 300 {
        l.pop_front();
    }
}

struct Session {
    child: Child,
    caps: Vec<Capture>,
    threads: Vec<JoinHandle<()>>,
    last_data: Arc<AtomicU64>,
    epoch: Instant,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    describe: String,
}

static SESSION_ID: AtomicU64 = AtomicU64::new(0);

impl Session {
    fn start(s: &Settings, ring: Arc<Mutex<Ring>>, log: Arc<Mutex<VecDeque<String>>>) -> anyhow::Result<Session> {
        let ffmpeg = ffmpeg::find(&s.ffmpeg_path).ok_or_else(|| anyhow::anyhow!(crate::i18n::tr(
            "ffmpeg.exe nenalezen – nastav cestu v Nastavení",
            "ffmpeg.exe not found – set its path in Settings",
        )))?;
        let id = format!("{}_{}", std::process::id(), SESSION_ID.fetch_add(1, Ordering::Relaxed));
        let video_start: VideoStart = Arc::new(Mutex::new(None));

        let mut caps = Vec::new();
        let mut tracks = Tracks::default();
        let stop_caps = |caps: Vec<Capture>| caps.into_iter().for_each(Capture::stop);
        if s.game_audio {
            match Capture::start(Kind::Loopback, &s.game_device, format!(r"\\.\pipe\justiceclips_{id}_game"), video_start.clone()) {
                Ok(c) => {
                    caps.push(c);
                    tracks.game = true;
                }
                Err(e) => push_log(&log, format!("game audio unavailable: {e:#}")),
            }
        }
        if s.mic {
            match Capture::start(Kind::Mic, &s.mic_device, format!(r"\\.\pipe\justiceclips_{id}_mic"), video_start.clone()) {
                Ok(c) => {
                    caps.push(c);
                    tracks.mic = true;
                }
                Err(e) => push_log(&log, format!("microphone unavailable: {e:#}")),
            }
        }

        let q = ffmpeg::quality(&s.quality);
        let fps = s.fps;
        let mut a: Vec<String> = vec![];
        let mut arg = |x: &str| a.push(x.to_string());
        for x in ["-hide_banner", "-nostats", "-nostdin", "-loglevel", "verbose"] {
            arg(x);
        }
        // Video first: ddagrab grabs its first frame while opening, which dates audio sample 0.
        for x in ["-thread_queue_size", "64", "-f", "lavfi", "-i"] {
            arg(x);
        }
        arg(&format!("ddagrab=output_idx={}:framerate={}:draw_mouse={}", s.monitor, fps, s.draw_mouse as u8));
        for c in &caps {
            if s.audio_offset_ms != 0 {
                arg("-itsoffset");
                arg(&format!("{:.3}", s.audio_offset_ms as f64 / 1000.0));
            }
            for x in ["-thread_queue_size", "4096", "-f", "f32le"] {
                arg(x);
            }
            arg("-ar");
            arg(&c.rate.to_string());
            arg("-ac");
            arg(&c.channels.to_string());
            arg("-i");
            arg(&c.pipe);
        }
        arg("-map");
        arg("0:v");
        for i in 0..caps.len() {
            arg("-map");
            arg(&format!("{}:a", i + 1));
        }
        for x in [
            "-fps_mode", "cfr", "-r", &fps.to_string(),
            "-c:v", "hevc_nvenc", "-preset", q.preset, "-tune", "hq",
            "-rc", "vbr", "-cq", &q.cq.to_string(), "-b:v", "0",
            "-maxrate", &format!("{}M", q.maxrate_mbps), "-bufsize", &format!("{}M", q.maxrate_mbps * 2),
            "-g", &(fps * 2).to_string(), "-bf", "3", "-b_ref_mode", "middle",
            // No temporal AQ / lookahead / multipass: they run on the CUDA cores the game renders with.
            "-spatial-aq", "1",
        ] {
            arg(x);
        }
        if !caps.is_empty() {
            for x in ["-c:a", "aac", "-ar", "48000"] {
                arg(x);
            }
            let mut ai = 0;
            if tracks.game {
                for x in [format!("-b:a:{ai}"), "192k".into(), format!("-ac:a:{ai}"), "2".into()] {
                    arg(&x);
                }
                ai += 1;
            }
            if tracks.mic {
                for x in [format!("-b:a:{ai}"), "96k".into(), format!("-ac:a:{ai}"), "1".into()] {
                    arg(&x);
                }
            }
        }
        for x in [
            "-f", "mpegts", "-mpegts_pmt_start_pid", "0x1000", "-mpegts_start_pid", "0x100",
            "-muxdelay", "0", "-muxpreload", "0", "-flush_packets", "1", "pipe:1",
        ] {
            arg(x);
        }

        // Below the game on both CPU and GPU: a hitch in the game is worse than a duplicated clip frame.
        let mut child = match ffmpeg::command(&ffmpeg, ffmpeg::BELOW_NORMAL)
            .args(&a)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => {
                win::tie_to_launcher(&c);
                win::low_gpu_priority(&c);
                c
            }
            Err(e) => {
                stop_caps(caps);
                return Err(e.into());
            }
        };
        ring.lock().unwrap().tracks = tracks;

        let epoch = Instant::now();
        let last_data = Arc::new(AtomicU64::new(0));
        let stderr_tail: Arc<Mutex<VecDeque<String>>> = Arc::new(Mutex::new(VecDeque::new()));
        let mut threads = Vec::new();

        let mut stdout = child.stdout.take().unwrap();
        {
            let last_data = last_data.clone();
            threads.push(std::thread::Builder::new().name("ts-reader".into()).spawn(move || {
                let mut buf = vec![0u8; TS * 512];
                let mut have = 0usize;
                loop {
                    let n = match stdout.read(&mut buf[have..]) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => n,
                    };
                    have += n;
                    // Resync if we ever lose packet alignment.
                    let mut off = 0;
                    while off < have && buf[off] != 0x47 {
                        off += 1;
                    }
                    let whole = (have - off) / TS * TS;
                    if whole > 0 {
                        let mut r = ring.lock().unwrap();
                        for p in buf[off..off + whole].chunks_exact(TS) {
                            r.push(p);
                        }
                    }
                    let used = off + whole;
                    buf.copy_within(used..have, 0);
                    have -= used;
                    last_data.store(epoch.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
                }
            })?);
        }

        let stderr = child.stderr.take().unwrap();
        {
            let (tail, vs) = (stderr_tail.clone(), video_start.clone());
            threads.push(std::thread::Builder::new().name("ffmpeg-log".into()).spawn(move || {
                for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                    if line.contains("Probed") || line.contains("Opened dxgi output") {
                        let mut v = vs.lock().unwrap();
                        if v.is_none() || line.contains("Probed") {
                            *v = Some(Instant::now());
                        }
                    }
                    let important = line.contains("rror")
                        || line.contains("ailed")
                        || line.contains("arning")
                        || line.contains("Opened dxgi output");
                    if important {
                        push_log(&log, format!("ffmpeg: {line}"));
                    }
                    let mut t = tail.lock().unwrap();
                    t.push_back(line);
                    if t.len() > 40 {
                        t.pop_front();
                    }
                }
            })?);
        }

        let describe = format!(
            "monitor {} @ {} fps, HEVC {} cq {}, audio: {}{}",
            s.monitor,
            fps,
            q.preset,
            q.cq,
            if tracks.game { "PC " } else { "" },
            if tracks.mic { "+ mic" } else { "" }
        );
        Ok(Session { child, caps, threads, last_data, epoch, stderr_tail, describe })
    }

    fn got_data(&self) -> bool {
        self.last_data.load(Ordering::Relaxed) > 0
    }

    /// Some(reason) when ffmpeg exited, audio failed or the stream stalled.
    fn dead(&mut self) -> Option<String> {
        if let Ok(Some(st)) = self.child.try_wait() {
            std::thread::sleep(Duration::from_millis(100));
            let tail = self.stderr_tail.lock().unwrap();
            let msg: Vec<&String> = tail
                .iter()
                .filter(|l| l.contains("rror") || l.contains("ailed") || l.contains("nvalid"))
                .collect();
            let last = msg.iter().rev().take(2).rev().map(|s| s.as_str()).collect::<Vec<_>>().join(" | ");
            let sep = if last.is_empty() { "" } else { ": " };
            return Some(l!("ffmpeg skončil ({}){}{}", "ffmpeg exited ({}){}{}", st, sep, last));
        }
        if self.caps.iter().any(|c| c.failed.load(Ordering::Relaxed)) {
            return Some(crate::i18n::tr("audio zařízení se odpojilo", "an audio device was disconnected").into());
        }
        let last = self.last_data.load(Ordering::Relaxed);
        let now = self.epoch.elapsed().as_millis() as u64;
        let stalled = if last == 0 { now > 15_000 } else { now.saturating_sub(last) > 6_000 };
        if stalled {
            return Some(crate::i18n::tr("nahrávání se zaseklo (žádná data)", "recording stalled (no data)").into());
        }
        None
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        for c in self.caps.drain(..) {
            c.stop();
        }
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

