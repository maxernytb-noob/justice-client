//! WASAPI capture (loopback + microphone) fed to ffmpeg through named pipes as raw f32 PCM.
//!
//! Timing: ffmpeg aligns every input to t=0, so audio sample 0 must correspond to the moment the
//! first video frame was grabbed. The writer measures "wanted samples" against that instant and
//! pads silence when the device delivers nothing (loopback is silent-by-absence) or drops samples
//! when it runs ahead, which also absorbs the audio-clock vs. system-clock drift over long sessions.

use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use super::win;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Loopback,
    Mic,
}

pub struct Capture {
    pub rate: u32,
    pub channels: u16,
    pub pipe: String,
    stop: Arc<AtomicBool>,
    connected: Arc<AtomicBool>,
    pub failed: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

pub type VideoStart = Arc<Mutex<Option<Instant>>>;

pub fn device_names() -> (Vec<String>, Vec<String>) {
    let host = cpal::default_host();
    let outs = host
        .output_devices()
        .map(|d| d.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    let ins = host
        .input_devices()
        .map(|d| d.filter_map(|d| d.name().ok()).collect())
        .unwrap_or_default();
    (outs, ins)
}

pub fn default_name(kind: Kind) -> Option<String> {
    let host = cpal::default_host();
    match kind {
        Kind::Loopback => host.default_output_device(),
        Kind::Mic => host.default_input_device(),
    }
    .and_then(|d| d.name().ok())
}

fn pick(kind: Kind, name: &str) -> Option<cpal::Device> {
    let host = cpal::default_host();
    if !name.is_empty() {
        let found = match kind {
            Kind::Loopback => host.output_devices().ok().and_then(|mut i| i.find(|d| d.name().ok().as_deref() == Some(name))),
            Kind::Mic => host.input_devices().ok().and_then(|mut i| i.find(|d| d.name().ok().as_deref() == Some(name))),
        };
        if found.is_some() {
            return found;
        }
    }
    match kind {
        Kind::Loopback => host.default_output_device(),
        Kind::Mic => host.default_input_device(),
    }
}

impl Capture {
    pub fn start(kind: Kind, device: &str, pipe: String, video_start: VideoStart) -> anyhow::Result<Capture> {
        let stop = Arc::new(AtomicBool::new(false));
        let failed = Arc::new(AtomicBool::new(false));
        let connected = Arc::new(AtomicBool::new(false));
        let buf: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
        let (tx, rx) = mpsc::channel::<anyhow::Result<(u32, u16)>>();

        // cpal streams are not Send; the stream lives on its own parked thread.
        let stream_thread = {
            let (stop, failed, buf, device) = (stop.clone(), failed.clone(), buf.clone(), device.to_string());
            std::thread::Builder::new().name("audio-stream".into()).spawn(move || {
                let res = (|| -> anyhow::Result<(cpal::Stream, u32, u16)> {
                    let dev = pick(kind, &device).ok_or_else(|| anyhow!("audio device not found"))?;
                    let cfg = match kind {
                        Kind::Loopback => dev.default_output_config(),
                        Kind::Mic => dev.default_input_config(),
                    }
                    .context("audio config")?;
                    let (rate, ch) = (cfg.sample_rate().0, cfg.channels());
                    let cap = rate as usize * ch as usize * 3;
                    let sc: cpal::StreamConfig = cfg.config();
                    let f2 = failed.clone();
                    let err = move |_e: cpal::StreamError| f2.store(true, Ordering::Relaxed);
                    macro_rules! build {
                        ($t:ty, $conv:expr) => {{
                            let b = buf.clone();
                            dev.build_input_stream(
                                &sc,
                                move |data: &[$t], _: &cpal::InputCallbackInfo| {
                                    let mut v = b.lock().unwrap();
                                    v.extend(data.iter().map($conv));
                                    if v.len() > cap {
                                        let excess = v.len() - cap;
                                        v.drain(..excess);
                                    }
                                },
                                err,
                                None,
                            )?
                        }};
                    }
                    let stream = match cfg.sample_format() {
                        cpal::SampleFormat::F32 => build!(f32, |s: &f32| *s),
                        cpal::SampleFormat::I16 => build!(i16, |s: &i16| *s as f32 / 32768.0),
                        cpal::SampleFormat::I32 => build!(i32, |s: &i32| *s as f32 / 2147483648.0),
                        cpal::SampleFormat::U16 => build!(u16, |s: &u16| (*s as f32 - 32768.0) / 32768.0),
                        f => anyhow::bail!("unsupported sample format {f:?}"),
                    };
                    stream.play()?;
                    Ok((stream, rate, ch))
                })();
                match res {
                    Ok((stream, rate, ch)) => {
                        let _ = tx.send(Ok((rate, ch)));
                        while !stop.load(Ordering::Relaxed) {
                            std::thread::park_timeout(Duration::from_millis(250));
                        }
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                    }
                }
            })?
        };

        let (rate, channels) = rx
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| anyhow!("audio device timed out"))??;

        // The pipe must exist before ffmpeg is spawned.
        let file = win::create_pipe(&pipe, 1 << 20).context("create pipe")?;

        let writer = {
            let (stop, connected, buf) = (stop.clone(), connected.clone(), buf.clone());
            std::thread::Builder::new().name("audio-pipe".into()).spawn(move || {
                if win::connect_pipe(&file).is_err() || stop.load(Ordering::Relaxed) {
                    return;
                }
                connected.store(true, Ordering::Relaxed);
                // ddagrab opens before the audio pipes, so the first video frame time is normally known.
                let deadline = Instant::now() + Duration::from_secs(2);
                let start = loop {
                    if let Some(t) = *video_start.lock().unwrap() {
                        break t;
                    }
                    if Instant::now() > deadline {
                        break Instant::now();
                    }
                    std::thread::sleep(Duration::from_millis(5));
                };
                buf.lock().unwrap().clear();
                pump(file, buf, &stop, start, rate, channels as usize);
            })?
        };

        Ok(Capture { rate, channels, pipe, stop, connected, failed, threads: vec![stream_thread, writer] })
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::Relaxed);
        if !self.connected.load(Ordering::Relaxed) {
            // Unblock ConnectNamedPipe by connecting to it ourselves.
            let _ = std::fs::OpenOptions::new().read(true).open(&self.pipe);
        }
        for t in self.threads {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

fn pump(mut out: File, buf: Arc<Mutex<Vec<f32>>>, stop: &AtomicBool, start: Instant, rate: u32, ch: usize) {
    let rate = rate as i64;
    let zeros = vec![0f32; rate as usize * ch / 10];
    let mut local: Vec<f32> = Vec::with_capacity(rate as usize * ch / 10);
    let mut written: i64 = 0;
    let bytes = |s: &[f32]| unsafe { std::slice::from_raw_parts(s.as_ptr() as *const u8, s.len() * 4) };

    while !stop.load(Ordering::Relaxed) {
        std::thread::sleep(Duration::from_millis(10));
        std::mem::swap(&mut *buf.lock().unwrap(), &mut local);
        let target = (start.elapsed().as_secs_f64() * rate as f64) as i64;
        let avail = (local.len() / ch) as i64;

        // Running ahead of the wall clock by >100 ms: drop the oldest samples.
        let skip = if written + avail > target + rate / 10 { (written + avail - target).min(avail) } else { 0 };
        if skip < avail && out.write_all(bytes(&local[skip as usize * ch..avail as usize * ch])).is_err() {
            return;
        }
        written += avail - skip;
        local.clear();

        // Behind by >50 ms (silence / device hiccup): fill with silence up to ~10 ms behind.
        if written < target - rate / 20 {
            let mut pad = target - rate / 100 - written;
            written += pad;
            while pad > 0 {
                let n = pad.min((zeros.len() / ch) as i64);
                if out.write_all(bytes(&zeros[..n as usize * ch])).is_err() {
                    return;
                }
                pad -= n;
            }
        }
    }
}
