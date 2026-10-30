//! Justice Clips inside the launcher: a Medal-like replay buffer (ffmpeg ddagrab -> NVENC HEVC, kept in
//! RAM) that runs while a Minecraft instance started from the launcher is running. A hotkey saves the
//! last N seconds as an MP4. Windows: own ffmpeg pipeline (Desktop Duplication, WASAPI).
//! Linux: gpu-screen-recorder in replay mode. Library, thumbnails and trimming are shared.

#[cfg(windows)]
mod audio;
mod ffmpeg;
#[cfg(not(windows))]
mod gsr;
mod library;
#[cfg(windows)]
mod recorder;
mod settings;
mod sys;
#[cfg(windows)]
mod win;

#[cfg(windows)]
type Rec = recorder::Recorder;
#[cfg(not(windows))]
type Rec = gsr::Gsr;
#[cfg(not(windows))]
use gsr::MonitorInfo;
#[cfg(windows)]
use win::MonitorInfo;

use std::path::PathBuf;
#[cfg(windows)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
#[cfg(windows)]
use tauri::image::Image;
#[cfg(windows)]
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
#[cfg(windows)]
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};
#[cfg(windows)]
use tauri::{WebviewUrl, WebviewWindowBuilder, Wry};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::i18n::tr;
use library::ClipInfo;
pub use settings::Settings;

pub struct ClipsState {
    settings: Arc<RwLock<Settings>>,
    rec: Arc<Rec>,
    thumbs: library::Thumbs,
    last_save: Mutex<Option<Instant>>,
    hotkey_error: Mutex<Option<String>>,
    /// Running Minecraft instances; the buffer runs while this is > 0.
    games: AtomicUsize,
    /// Name of the last started instance, used for clip names and folders.
    game_name: Mutex<String>,
}

#[cfg(windows)]
struct TrayItems {
    save: MenuItem<Wry>,
    open: MenuItem<Wry>,
    folder: MenuItem<Wry>,
    quit: MenuItem<Wry>,
}

type Res<T> = Result<T, String>;

fn err<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn clips_dir(st: &ClipsState) -> PathBuf {
    PathBuf::from(&st.settings.read().unwrap().clips_dir)
}

fn no_ffmpeg() -> String {
    tr("ffmpeg nenalezen", "ffmpeg not found").into()
}

/// Only files inside the clips folder may be modified through commands.
fn inside_clips(st: &ClipsState, path: &str) -> Res<PathBuf> {
    let dir = clips_dir(st);
    let p = PathBuf::from(path);
    let (Ok(d), Ok(c)) = (dir.canonicalize(), p.canonicalize()) else {
        return Err(tr("klip nenalezen", "clip not found").into());
    };
    if c.starts_with(&d) {
        Ok(p)
    } else {
        Err(tr("soubor není ve složce s klipy", "the file is not in the clips folder").into())
    }
}

// ---------------- setup & game lifecycle ----------------

pub fn init(app: &AppHandle) -> tauri::Result<()> {
    let s = settings::load();
    let _ = settings::save(&s);
    let _ = std::fs::create_dir_all(&s.clips_dir);
    let _ = app.asset_protocol_scope().allow_directory(&s.clips_dir, true);
    let settings = Arc::new(RwLock::new(s.clone()));
    #[cfg(windows)]
    let rec = recorder::Recorder::start(app.clone(), settings.clone(), true);
    #[cfg(not(windows))]
    let rec = gsr::Gsr::start(app.clone(), settings.clone(), true);
    let s2 = settings.clone();
    let thumbs = library::Thumbs::new(app.clone(), Arc::new(move || ffmpeg::find(&s2.read().unwrap().ffmpeg_path)));
    app.manage(ClipsState {
        settings,
        rec,
        thumbs,
        last_save: Mutex::new(None),
        hotkey_error: Mutex::new(None),
        games: AtomicUsize::new(0),
        game_name: Mutex::new("Minecraft".into()),
    });
    #[cfg(windows)]
    build_tray(app, &s.hotkey)?;
    let _ = register_hotkey(app, &s.hotkey);
    #[cfg(windows)]
    create_overlay(app)?;
    linfo!("clips ready: dir {}, hotkey {}, enabled {}", s.clips_dir, s.hotkey, s.enabled);
    if std::env::var_os("JC_CLIPS_TEST").is_some() {
        self_test(app.clone());
    }
    Ok(())
}

/// Developer check (JC_CLIPS_TEST=1): record ~12 s, save a clip, log the result, quit.
fn self_test(app: AppHandle) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(2));
        game_started("Clips selftest");
        for _ in 0..30 {
            std::thread::sleep(Duration::from_secs(1));
            if app.state::<ClipsState>().rec.status().state == "recording" {
                break;
            }
        }
        std::thread::sleep(Duration::from_secs(10));
        let st = app.state::<ClipsState>();
        linfo!("clips selftest: status {:?}, buffer {:?}", st.rec.status().state, buffer_stats(&st.rec));
        trigger_save(&app, false);
        std::thread::sleep(Duration::from_secs(8));
        for l in st.rec.log().iter().rev().take(8) {
            linfo!("clips selftest log: {l}");
        }
        game_stopped();
        std::thread::sleep(Duration::from_secs(1));
        linfo!("clips selftest: done, status {}", st.rec.status().state);
        shutdown(&app);
        app.exit(0);
    });
}

pub fn enabled() -> bool {
    crate::events::app()
        .and_then(|a| a.try_state::<ClipsState>().map(|st| st.settings.read().unwrap().enabled))
        .unwrap_or(false)
}

/// A Minecraft instance started: start the replay buffer (when Clips are on).
pub fn game_started(instance_name: &str) {
    let Some(app) = crate::events::app() else { return };
    let Some(st) = app.try_state::<ClipsState>() else { return };
    *st.game_name.lock().unwrap() = instance_name.to_string();
    let n = st.games.fetch_add(1, Ordering::SeqCst) + 1;
    if n == 1 && st.settings.read().unwrap().enabled {
        linfo!("clips: game started ({instance_name}), buffer on");
        st.rec.set_paused(false);
    }
}

/// A Minecraft instance exited: stop the buffer once no game is left.
pub fn game_stopped() {
    let Some(app) = crate::events::app() else { return };
    let Some(st) = app.try_state::<ClipsState>() else { return };
    let prev = st.games.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| Some(n.saturating_sub(1))).unwrap_or(0);
    if prev <= 1 {
        linfo!("clips: no game running, buffer off");
        st.rec.set_paused(true);
    }
}

pub fn shutdown(app: &AppHandle) {
    if let Some(st) = app.try_state::<ClipsState>() {
        st.rec.shutdown();
    }
}

// ---------------- commands ----------------

#[tauri::command]
pub fn clips_get_settings(st: State<ClipsState>) -> Settings {
    st.settings.read().unwrap().clone()
}

#[tauri::command]
pub fn clips_set_settings(app: AppHandle, st: State<ClipsState>, mut s: Settings) -> Res<Settings> {
    s.sanitize();
    let old = st.settings.read().unwrap().clone();
    settings::save(&s).map_err(err)?;
    *st.settings.write().unwrap() = s.clone();
    if old.clips_dir != s.clips_dir {
        let _ = std::fs::create_dir_all(&s.clips_dir);
        let _ = app.asset_protocol_scope().allow_directory(&s.clips_dir, true);
    }
    if old.buffer_secs != s.buffer_secs || old.post_secs != s.post_secs {
        st.rec.set_buffer(s.buffer_secs, s.post_secs);
    }
    if old.enabled != s.enabled {
        let playing = st.games.load(Ordering::SeqCst) > 0;
        st.rec.set_paused(!(s.enabled && playing));
    } else if s.capture_changed(&old) {
        st.rec.restart();
    }
    if old.hotkey != s.hotkey {
        register_hotkey(&app, &s.hotkey)?;
        update_tray(&app, &st.rec.status().state);
    }
    Ok(s)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusInfo {
    state: String,
    error: Option<String>,
    buffer_bytes: usize,
    buffer_secs: f64,
    ffmpeg: Option<String>,
    hotkey_error: Option<String>,
    game_running: bool,
    platform: &'static str,
    /// Linux: gpu-screen-recorder isn't installed.
    recorder_missing: bool,
}

#[tauri::command]
pub fn clips_status(st: State<ClipsState>) -> StatusInfo {
    let s = st.rec.status();
    let (bytes, secs) = buffer_stats(&st.rec);
    StatusInfo {
        state: s.state,
        error: s.error,
        buffer_bytes: bytes,
        buffer_secs: secs,
        ffmpeg: ffmpeg::find(&st.settings.read().unwrap().ffmpeg_path).map(|p| p.to_string_lossy().into_owned()),
        hotkey_error: st.hotkey_error.lock().unwrap().clone(),
        game_running: st.games.load(Ordering::SeqCst) > 0,
        platform: if cfg!(windows) { "windows" } else { "linux" },
        #[cfg(windows)]
        recorder_missing: false,
        #[cfg(not(windows))]
        recorder_missing: gsr::binary().is_none(),
    }
}

#[cfg(windows)]
fn buffer_stats(rec: &Rec) -> (usize, f64) {
    rec.ring.lock().unwrap().stats()
}
#[cfg(not(windows))]
fn buffer_stats(rec: &Rec) -> (usize, f64) {
    rec.stats()
}

#[tauri::command]
pub fn clips_save(app: AppHandle) {
    trigger_save(&app, false);
}

/// Manual override: record now (e.g. for another game) or stop.
#[tauri::command]
pub fn clips_set_recording(app: AppHandle, st: State<ClipsState>, on: bool) {
    st.rec.set_paused(!on);
    update_tray(&app, if on { "starting" } else { "paused" });
}

#[tauri::command]
pub fn clips_list(st: State<ClipsState>) -> Vec<ClipInfo> {
    let dir = clips_dir(&st);
    let l = library::list(&dir);
    st.thumbs.ensure(&dir, &l);
    l
}

#[tauri::command]
pub async fn clips_strip(st: State<'_, ClipsState>, path: String) -> Res<String> {
    let p = inside_clips(&st, &path)?;
    let ff = ffmpeg::find(&st.settings.read().unwrap().ffmpeg_path).ok_or_else(no_ffmpeg)?;
    let dir = clips_dir(&st);
    tauri::async_runtime::spawn_blocking(move || library::strip(&ff, &dir, &p))
        .await
        .map_err(err)?
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(err)
}

#[tauri::command]
pub fn clips_delete(st: State<ClipsState>, path: String) -> Res<()> {
    let p = inside_clips(&st, &path)?;
    sys::recycle(&p).map_err(err)
}

#[tauri::command]
pub fn clips_rename(st: State<ClipsState>, path: String, name: String) -> Res<ClipInfo> {
    inside_clips(&st, &path)?;
    let p = library::rename(&path, &name).map_err(err)?;
    library::info(&clips_dir(&st), &p).map_err(err)
}

#[tauri::command]
pub fn clips_reveal(path: String) {
    sys::reveal(&path);
}

#[tauri::command]
pub fn clips_open_dir(st: State<ClipsState>) {
    let dir = clips_dir(&st);
    let _ = std::fs::create_dir_all(&dir);
    sys::open_dir(&dir);
}

#[tauri::command]
pub async fn clips_trim(app: AppHandle, st: State<'_, ClipsState>, req: library::TrimReq) -> Res<ClipInfo> {
    inside_clips(&st, &req.path)?;
    let ff = ffmpeg::find(&st.settings.read().unwrap().ffmpeg_path).ok_or_else(no_ffmpeg)?;
    let dir = clips_dir(&st);
    let a2 = app.clone();
    let info = tauri::async_runtime::spawn_blocking(move || library::trim(&a2, &ff, &dir, &req))
        .await
        .map_err(err)?
        .map_err(err)?;
    st.thumbs.ensure(&clips_dir(&st), std::slice::from_ref(&info));
    Ok(info)
}

/// Transcodes a clip to browser-friendly H.264 720p (capped at 60 s) plus a JPG thumbnail, in temp files.
fn transcode_for_feed(ff: &std::path::Path, src: &std::path::Path) -> anyhow::Result<(PathBuf, PathBuf)> {
    let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let base = std::env::temp_dir().join(format!("justice-feed-{stamp}"));
    let mp4 = base.with_extension("mp4");
    let thumb = base.with_extension("jpg");
    let s = |x: &str| x.to_string();
    ffmpeg::run(ff, &[
        s("-i"), src.to_string_lossy().into_owned(), s("-t"), s("60"),
        s("-vf"), s("scale=-2:720"),
        s("-c:v"), s("libx264"), s("-preset"), s("veryfast"), s("-crf"), s("23"),
        s("-pix_fmt"), s("yuv420p"), s("-c:a"), s("aac"), s("-b:a"), s("128k"),
        s("-movflags"), s("+faststart"), mp4.to_string_lossy().into_owned(),
    ], 0)?;
    let _ = ffmpeg::run(ff, &[
        s("-ss"), s("1"), s("-i"), mp4.to_string_lossy().into_owned(),
        s("-frames:v"), s("1"), s("-vf"), s("scale=-2:360"), thumb.to_string_lossy().into_owned(),
    ], 0);
    Ok((mp4, thumb))
}

/// Publishes a clip to the Justice feed: transcodes to 720p, uploads it and a thumbnail, then posts metadata.
#[tauri::command]
pub async fn clips_publish(st: State<'_, ClipsState>, path: String, title: String, description: String, tags: Vec<String>, hashtags: Vec<String>, duration_ms: i64, captured: i64) -> Res<serde_json::Value> {
    let p = inside_clips(&st, &path)?;
    let ff = ffmpeg::find(&st.settings.read().unwrap().ffmpeg_path).ok_or_else(no_ffmpeg)?;
    let (mp4, thumb) = tauri::async_runtime::spawn_blocking(move || transcode_for_feed(&ff, &p)).await.map_err(err)?.map_err(err)?;
    let vid = crate::social::upload(&mp4.to_string_lossy()).await.map_err(err)?;
    let th = crate::social::upload(&thumb.to_string_lossy()).await.ok();
    let _ = std::fs::remove_file(&mp4);
    let _ = std::fs::remove_file(&thumb);
    let url = vid["url"].as_str().ok_or_else(|| err("upload returned no url"))?;
    let thumb_url = th.as_ref().and_then(|t| t["url"].as_str()).unwrap_or("");
    let body = serde_json::json!({
        "url": url, "thumb": thumb_url, "title": title, "description": description,
        "tags": tags, "hashtags": hashtags, "durationMs": duration_ms, "height": 720, "captured": captured,
    });
    crate::social::post("/clips/publish", body).await.map_err(err)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Devices {
    outputs: Vec<String>,
    inputs: Vec<String>,
    default_output: Option<String>,
    default_input: Option<String>,
}

#[tauri::command]
pub async fn clips_devices() -> Devices {
    tauri::async_runtime::spawn_blocking(|| {
        #[cfg(windows)]
        let d = {
            let (outputs, inputs) = audio::device_names();
            Devices {
                outputs,
                inputs,
                default_output: audio::default_name(audio::Kind::Loopback),
                default_input: audio::default_name(audio::Kind::Mic),
            }
        };
        #[cfg(not(windows))]
        let d = {
            let (outputs, inputs) = gsr::audio_devices();
            Devices { outputs, inputs, default_output: None, default_input: None }
        };
        d
    })
    .await
    .unwrap_or(Devices { outputs: vec![], inputs: vec![], default_output: None, default_input: None })
}

#[tauri::command]
pub fn clips_monitors() -> Vec<MonitorInfo> {
    #[cfg(windows)]
    return win::monitors();
    #[cfg(not(windows))]
    return gsr::monitors();
}

#[tauri::command]
pub fn clips_log(st: State<ClipsState>) -> Vec<String> {
    st.rec.log()
}

#[tauri::command]
pub fn clips_test_notification(app: AppHandle) {
    let s = app.state::<ClipsState>().settings.read().unwrap().clone();
    let fg = sys::foreground();
    overlay_show(
        &app,
        Overlay {
            kind: "saved".into(),
            title: tr("Klip uložen", "Clip saved").into(),
            sub: tr("Takhle vypadá notifikace ve hře", "This is how the in-game notification looks").into(),
        },
        fg.work,
    );
    if s.sound {
        sys::play_blip();
    }
}

#[tauri::command]
pub fn clips_overlay_hide(app: AppHandle) {
    #[cfg(not(windows))]
    let _ = app;
    #[cfg(windows)]
    if let Some(w) = app.get_webview_window("overlay") {
        if let Ok(h) = w.hwnd() {
            win::hide(h.0);
        }
    }
}

#[tauri::command]
pub fn clips_detect_ffmpeg(path: String) -> Option<String> {
    ffmpeg::find(&path).map(|p| p.to_string_lossy().into_owned())
}

// ---------------- clip saving ----------------

/// Clip name: the Minecraft instance while one runs, otherwise the foreground app.
fn clip_game(st: &ClipsState, fg_game: &str) -> String {
    let playing = st.games.load(Ordering::SeqCst) > 0;
    let looks_like_mc = fg_game.starts_with("Minecraft") || fg_game.eq_ignore_ascii_case("javaw") || fg_game.eq_ignore_ascii_case("java");
    if playing && looks_like_mc {
        let n: String = st.game_name.lock().unwrap().chars().filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control()).collect();
        let n = n.trim().to_string();
        if !n.is_empty() {
            return n;
        }
    }
    fg_game.to_string()
}

fn trigger_save(app: &AppHandle, from_hotkey: bool) {
    let st = app.state::<ClipsState>();
    {
        let mut last = st.last_save.lock().unwrap();
        if last.map(|t| t.elapsed() < Duration::from_millis(1200)).unwrap_or(false) {
            return;
        }
        *last = Some(Instant::now());
    }
    let s = st.settings.read().unwrap().clone();
    let fg = sys::foreground();
    let game = clip_game(&st, &fg.game);
    let status = st.rec.status();
    if status.state != "recording" {
        let msg = match status.state.as_str() {
            "paused" => tr("Nahrávání neběží — zapne se se hrou", "Recording is off — it starts with the game"),
            "starting" => tr("Nahrávání se teprve spouští", "Recording is still starting"),
            _ => tr("Nahrávání neběží", "Recording is not running"),
        };
        if s.overlay && from_hotkey {
            overlay_show(
                app,
                Overlay { kind: "error".into(), title: tr("Klip neuložen", "Clip not saved").into(), sub: msg.into() },
                fg.work,
            );
        }
        let _ = app.emit("clips-toast", Toast { kind: "error".into(), text: msg.into() });
        return;
    }
    linfo!("clips: saving clip of {game}");
    if s.overlay {
        overlay_show(
            app,
            Overlay {
                kind: "saving".into(),
                title: tr("Ukládám klip…", "Saving clip…").into(),
                sub: l!("{} · posledních {} s", "{} · last {} s", game, s.buffer_secs),
            },
            fg.work,
        );
    }
    let _ = app.emit("clips-saving", ());
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs_f32(s.post_secs));
        let st = app.state::<ClipsState>();
        let clips = PathBuf::from(&s.clips_dir);
        #[cfg(not(windows))]
        let res = st.rec.save(&game, &s).and_then(|path| library::info(&clips, &path));
        #[cfg(windows)]
        let res = (|| -> anyhow::Result<ClipInfo> {
            let ff = ffmpeg::find(&s.ffmpeg_path).ok_or_else(|| anyhow::anyhow!(no_ffmpeg()))?;
            let snap = st
                .rec
                .ring
                .lock()
                .unwrap()
                .snapshot(Duration::from_secs_f32(s.buffer_secs as f32 + s.post_secs))
                .ok_or_else(|| anyhow::anyhow!(tr("buffer je zatím prázdný", "the buffer is still empty")))?;
            let path = library::save(
                &library::SaveReq {
                    ffmpeg: ff,
                    clips_dir: clips.clone(),
                    game: game.clone(),
                    game_folders: s.game_folders,
                    mic_volume: s.mic_volume,
                },
                snap,
            )?;
            library::info(&clips, &path)
        })();
        match res {
            Ok(info) => {
                if s.sound {
                    sys::play_blip();
                }
                if s.overlay {
                    overlay_show(
                        &app,
                        Overlay {
                            kind: "saved".into(),
                            title: tr("Klip uložen", "Clip saved").into(),
                            sub: format!("{} · {:.0} s · {:.0} MB", info.game, info.duration, info.size as f64 / 1_048_576.0),
                        },
                        fg.work,
                    );
                }
                linfo!("clips: saved {} ({:.1} s, {} B)", info.path, info.duration, info.size);
                st.thumbs.ensure(&clips, std::slice::from_ref(&info));
                let _ = app.emit("clips-saved", info);
            }
            Err(e) => {
                let text = format!("{e:#}");
                lwarn!("clips: saving failed: {text}");
                if s.overlay {
                    overlay_show(
                        &app,
                        Overlay { kind: "error".into(), title: tr("Uložení selhalo", "Saving failed").into(), sub: text.clone() },
                        fg.work,
                    );
                }
                let _ = app.emit("clips-toast", Toast { kind: "error".into(), text: l!("Uložení klipu selhalo: {}", "Saving the clip failed: {}", text) });
            }
        }
    });
}

#[derive(Serialize, Clone)]
struct Toast {
    kind: String,
    text: String,
}

#[derive(Serialize, Clone)]
struct Overlay {
    kind: String,
    title: String,
    sub: String,
}

#[cfg(windows)]
static OVERLAY_GEN: AtomicU64 = AtomicU64::new(0);

/// Linux: a desktop notification (there's no overlay window); the "saving" step is skipped.
#[cfg(not(windows))]
fn overlay_show(_app: &AppHandle, msg: Overlay, _work: (i32, i32, i32, i32)) {
    if msg.kind == "saving" {
        return;
    }
    // The app's own icon (installed by the package); generic theme icons are missing in many themes.
    let icon = ["/usr/share/icons/hicolor/128x128/apps/justice-client.png", "/usr/share/icons/hicolor/256x256/apps/justice-client.png"]
        .into_iter()
        .find(|p| std::path::Path::new(p).exists())
        .unwrap_or("justice-client");
    std::thread::spawn(move || {
        use std::sync::atomic::{AtomicU32, Ordering};
        // Replace the previous notification instead of stacking a new one each time.
        static LAST: AtomicU32 = AtomicU32::new(0);
        let mut cmd = std::process::Command::new("notify-send");
        cmd.args(["-a", "Justice Clips", "-i", icon, "-t", "3500", "-p"])
            .args(["-h", "string:x-canonical-private-synchronous:justice-clips", "-h", "string:x-dunst-stack-tag:justice-clips"]);
        let last = LAST.load(Ordering::SeqCst);
        if last != 0 {
            cmd.args(["-r", &last.to_string()]);
        }
        match cmd.args([&msg.title, &msg.sub]).output() {
            Ok(out) if out.status.success() => {
                if let Ok(id) = String::from_utf8_lossy(&out.stdout).trim().parse::<u32>() {
                    LAST.store(id, Ordering::SeqCst);
                }
            }
            // Old libnotify without -p/-r: plain notification.
            _ => {
                let _ = std::process::Command::new("notify-send").args(["-a", "Justice Clips", "-i", icon, "-t", "3500", &msg.title, &msg.sub]).status();
            }
        }
    });
}

#[cfg(windows)]
fn overlay_show(app: &AppHandle, msg: Overlay, work: (i32, i32, i32, i32)) {
    let Some(w) = app.get_webview_window("overlay") else { return };
    let Ok(hwnd) = w.hwnd() else { return };
    let hwnd = hwnd.0 as isize;
    let kind = msg.kind.clone();
    let _ = app.emit_to("overlay", "overlay", msg);
    let scale = w.scale_factor().unwrap_or(1.0);
    let size = w.outer_size().map(|s| (s.width as i32, s.height as i32)).unwrap_or((380, 110));
    let margin = (18.0 * scale) as i32;
    win::show_no_activate(hwnd as _, work.2 - size.0 - margin, work.1 + margin);
    // Safety net: the overlay page hides itself after its exit animation.
    let gen = OVERLAY_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let delay = if kind == "saving" { 20 } else { 7 };
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(delay));
        if OVERLAY_GEN.load(Ordering::SeqCst) == gen {
            win::hide(hwnd as _);
        }
    });
}

// ---------------- hotkey / overlay window / tray ----------------

/// Called by the global-shortcut plugin (registered in lib.rs).
pub fn on_hotkey(app: &AppHandle) {
    trigger_save(app, true);
}

fn register_hotkey(app: &AppHandle, key: &str) -> Res<()> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let r = gs.register(key).map_err(|e| {
        l!(
            "Klávesu {} nejde zaregistrovat (nemáš zapnutý starý Justice Clips?): {}",
            "Cannot register the {} key (is the old Justice Clips app still running?): {}",
            key,
            e
        )
    });
    if let Err(e) = &r {
        lwarn!("{e}");
    }
    *app.state::<ClipsState>().hotkey_error.lock().unwrap() = r.as_ref().err().cloned();
    r
}

#[cfg(windows)]
fn create_overlay(app: &AppHandle) -> tauri::Result<()> {
    let w = WebviewWindowBuilder::new(app, "overlay", WebviewUrl::App("overlay.html".into()))
        .title("Justice Clips overlay")
        .inner_size(380.0, 110.0)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false)
        .build()?;
    w.set_ignore_cursor_events(true)?;
    if let Ok(h) = w.hwnd() {
        win::overlay_styles(h.0);
    }
    Ok(())
}

/// Brings the launcher window back (tray click, "Open").
pub fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

/// Tray icon: ring + dot, red while recording, grey when idle, amber on error.
#[cfg(windows)]
fn tray_image(state: &str) -> Image<'static> {
    let (r, g, b) = match state {
        "recording" => (235, 38, 52),
        "paused" => (140, 140, 150),
        "error" => (245, 170, 30),
        _ => (200, 90, 90),
    };
    let n = 32usize;
    let mut px = vec![0u8; n * n * 4];
    let c = (n as f32 - 1.0) / 2.0;
    for y in 0..n {
        for x in 0..n {
            let d = ((x as f32 - c).powi(2) + (y as f32 - c).powi(2)).sqrt();
            let ring = (1.0 - ((d - 13.0).abs() - 1.3).clamp(0.0, 1.0)) * 0.95;
            let dot = (8.2 - d).clamp(0.0, 1.0);
            let i = (y * n + x) * 4;
            let (cr, cg, cb, a) = if dot > 0.0 { (r, g, b, dot) } else { (235, 235, 240, ring) };
            px[i] = cr;
            px[i + 1] = cg;
            px[i + 2] = cb;
            px[i + 3] = (a * 255.0) as u8;
        }
    }
    Image::new_owned(px, n as u32, n as u32)
}

#[cfg(windows)]
fn save_label(hotkey: &str) -> String {
    l!("Uložit klip ({})", "Save clip ({})", hotkey)
}

/// No tray on Linux (it would need libappindicator); the launcher window is the UI there.
#[cfg(not(windows))]
pub(crate) fn update_tray(_app: &AppHandle, _state: &str) {}

#[cfg(windows)]
pub(crate) fn update_tray(app: &AppHandle, state: &str) {
    let Some(tray) = app.tray_by_id("main") else { return };
    let _ = tray.set_icon(Some(tray_image(state)));
    let label = match state {
        "recording" => tr("Clips nahrávají", "Clips recording"),
        "paused" => tr("Clips čekají na hru", "Clips waiting for a game"),
        "error" => tr("Clips: chyba", "Clips: error"),
        _ => tr("Clips se spouští", "Clips starting"),
    };
    let _ = tray.set_tooltip(Some(format!("Justice Client – {label}")));
    if let Some(items) = app.try_state::<TrayItems>() {
        let _ = items.open.set_text(tr("Otevřít Justice Client", "Open Justice Client"));
        let _ = items.folder.set_text(tr("Otevřít složku s klipy", "Open clips folder"));
        let _ = items.quit.set_text(tr("Ukončit", "Quit"));
        if let Some(st) = app.try_state::<ClipsState>() {
            let hk = st.settings.read().unwrap().hotkey.clone();
            let _ = items.save.set_text(save_label(&hk));
        }
    }
}

#[cfg(windows)]
fn build_tray(app: &AppHandle, hotkey: &str) -> tauri::Result<()> {
    let save = MenuItem::with_id(app, "save", save_label(hotkey), true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", tr("Otevřít Justice Client", "Open Justice Client"), true, None::<&str>)?;
    let folder = MenuItem::with_id(app, "folder", tr("Otevřít složku s klipy", "Open clips folder"), true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", tr("Ukončit", "Quit"), true, None::<&str>)?;
    let sep1 = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sep1, &save, &folder, &sep2, &quit])?;
    app.manage(TrayItems { save, open, folder, quit });
    TrayIconBuilder::with_id("main")
        .icon(tray_image("paused"))
        .tooltip("Justice Client")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, e| match e.id.as_ref() {
            "save" => trigger_save(app, true),
            "open" => show_main(app),
            "folder" => {
                let st = app.state::<ClipsState>();
                let dir = clips_dir(&st);
                let _ = std::fs::create_dir_all(&dir);
                let _ = std::process::Command::new("explorer").arg(dir).spawn();
            }
            "quit" => {
                shutdown(app);
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, e| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;
    update_tray(app, "paused");
    Ok(())
}
