//! Clip files: saving buffer snapshots, listing, thumbnails, trimming, renaming, deleting.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fs::File;
use std::hash::{Hash, Hasher};
#[cfg_attr(not(windows), allow(unused_imports))]
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::time::UNIX_EPOCH;

use anyhow::{anyhow, bail, Context};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use super::ffmpeg;
use crate::i18n::tr;
#[cfg(windows)]
use super::recorder::Snapshot;

pub const EXTS: [&str; 3] = ["mp4", "mkv", "mov"];

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ClipInfo {
    pub path: String,
    pub name: String,
    pub game: String,
    pub size: u64,
    pub modified: i64,
    pub duration: f64,
    pub audio_tracks: u32,
    pub thumb: Option<String>,
}

pub fn thumbs_dir(clips: &Path) -> PathBuf {
    clips.join(".thumbs")
}

/// Thumbnail key survives renames: it depends on size + mtime only.
fn key(meta: &std::fs::Metadata) -> String {
    let mut h = DefaultHasher::new();
    meta.len().hash(&mut h);
    meta.modified().ok().and_then(|m| m.duration_since(UNIX_EPOCH).ok()).map(|d| d.as_nanos()).hash(&mut h);
    format!("{:016x}", h.finish())
}

fn thumb_path(clips: &Path, meta: &std::fs::Metadata) -> PathBuf {
    thumbs_dir(clips).join(format!("{}.jpg", key(meta)))
}

fn game_of(stem: &str, folder: Option<&str>) -> String {
    // "<Game> YYYY-MM-DD HH-MM-SS"
    let b = stem.as_bytes();
    if b.len() >= 20 {
        let tail = &stem[stem.len() - 19..];
        let looks_dated = tail.chars().enumerate().all(|(i, c)| match i {
            4 | 7 | 13 | 16 => c == '-',
            10 => c == ' ',
            _ => c.is_ascii_digit(),
        });
        if looks_dated {
            let g = stem[..stem.len() - 19].trim();
            if !g.is_empty() {
                return g.to_string();
            }
        }
    }
    folder.unwrap_or("").to_string()
}

pub fn info(clips: &Path, path: &Path) -> anyhow::Result<ClipInfo> {
    let meta = std::fs::metadata(path)?;
    let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let folder = path
        .parent()
        .filter(|p| *p != clips)
        .and_then(|p| p.file_name())
        .map(|s| s.to_string_lossy().into_owned());
    let mp4 = mp4_info_cached(path, &meta);
    let thumb = thumb_path(clips, &meta);
    Ok(ClipInfo {
        path: path.to_string_lossy().into_owned(),
        game: game_of(&stem, folder.as_deref()),
        name: stem,
        size: meta.len(),
        modified: meta
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
        duration: mp4.duration,
        audio_tracks: mp4.audio_tracks,
        thumb: thumb.is_file().then(|| thumb.to_string_lossy().into_owned()),
    })
}

pub fn list(clips: &Path) -> Vec<ClipInfo> {
    let mut out = Vec::new();
    let mut visit = |dir: &Path| {
        let Ok(rd) = std::fs::read_dir(dir) else { return Vec::new() };
        let mut subdirs = Vec::new();
        for e in rd.flatten() {
            let p = e.path();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
                if !e.file_name().to_string_lossy().starts_with('.') {
                    subdirs.push(p);
                }
            } else if p
                .extension()
                .map(|x| EXTS.contains(&x.to_string_lossy().to_lowercase().as_str()))
                .unwrap_or(false)
                && !p.file_name().unwrap().to_string_lossy().starts_with(".~")
            {
                if let Ok(i) = info(clips, &p) {
                    out.push(i);
                }
            }
        }
        subdirs
    };
    for d in visit(clips) {
        visit(&d);
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}

// ---------- MP4 header parsing (duration + audio track count, no ffprobe needed) ----------

#[derive(Default, Clone, Copy)]
struct Mp4Info {
    duration: f64,
    audio_tracks: u32,
}

/// Header parse results keyed by path + size/mtime, so re-listing the library reads nothing.
fn mp4_info_cached(path: &Path, meta: &std::fs::Metadata) -> Mp4Info {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, String), Mp4Info>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let k = (path.to_path_buf(), key(meta));
    if let Some(i) = cache.lock().unwrap().get(&k) {
        return *i;
    }
    let i = mp4_info(path).unwrap_or_default();
    let mut c = cache.lock().unwrap();
    if c.len() > 5000 {
        c.clear();
    }
    c.insert(k, i);
    i
}

fn boxes(mut b: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut v = Vec::new();
    while b.len() >= 8 {
        let mut size = u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as usize;
        let typ = &b[4..8];
        let mut hdr = 8;
        if size == 1 && b.len() >= 16 {
            size = u64::from_be_bytes(b[8..16].try_into().unwrap()) as usize;
            hdr = 16;
        } else if size == 0 {
            size = b.len();
        }
        if size < hdr || size > b.len() {
            break;
        }
        v.push((typ, &b[hdr..size]));
        b = &b[size..];
    }
    v
}

fn mp4_info(path: &Path) -> Option<Mp4Info> {
    let mut f = File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let mut pos = 0u64;
    while pos + 8 <= len {
        f.seek(SeekFrom::Start(pos)).ok()?;
        let mut h = [0u8; 16];
        f.read_exact(&mut h[..8]).ok()?;
        let mut size = u32::from_be_bytes(h[..4].try_into().unwrap()) as u64;
        let mut hdr = 8u64;
        if size == 1 {
            f.read_exact(&mut h[8..16]).ok()?;
            size = u64::from_be_bytes(h[8..16].try_into().unwrap());
            hdr = 16;
        } else if size == 0 {
            size = len - pos;
        }
        if size < hdr {
            return None;
        }
        if &h[4..8] == b"moov" {
            let n = (size - hdr).min(64 << 20) as usize;
            let mut moov = vec![0u8; n];
            f.read_exact(&mut moov).ok()?;
            let mut info = Mp4Info::default();
            for (t, b) in boxes(&moov) {
                if t == b"mvhd" && b.len() >= 32 {
                    let (ts, dur) = if b[0] == 1 {
                        (u32::from_be_bytes(b[20..24].try_into().unwrap()) as f64, u64::from_be_bytes(b[24..32].try_into().unwrap()) as f64)
                    } else {
                        (u32::from_be_bytes(b[12..16].try_into().unwrap()) as f64, u32::from_be_bytes(b[16..20].try_into().unwrap()) as f64)
                    };
                    if ts > 0.0 {
                        info.duration = dur / ts;
                    }
                } else if t == b"trak" {
                    let is_audio = boxes(b)
                        .into_iter()
                        .filter(|(t, _)| *t == b"mdia")
                        .flat_map(|(_, m)| boxes(m))
                        .any(|(t, h)| t == b"hdlr" && h.len() >= 12 && &h[8..12] == b"soun");
                    if is_audio {
                        info.audio_tracks += 1;
                    }
                }
            }
            return Some(info);
        }
        pos += size;
    }
    None
}

// ---------- saving ----------

#[cfg(windows)]
pub struct SaveReq {
    pub ffmpeg: PathBuf,
    pub clips_dir: PathBuf,
    pub game: String,
    pub game_folders: bool,
    pub mic_volume: f32,
}

pub fn unique(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let mut p = dir.join(format!("{stem}.{ext}"));
    let mut i = 2;
    while p.exists() {
        p = dir.join(format!("{stem} ({i}).{ext}"));
        i += 1;
    }
    p
}

/// Remuxes a buffer snapshot to MP4 without touching the video. With game + mic it writes
/// [mix (default), game, mic] so Discord/phones play everything while editors keep separate tracks.
#[cfg(windows)]
pub fn save(req: &SaveReq, snap: Snapshot) -> anyhow::Result<PathBuf> {
    let dir = if req.game_folders { req.clips_dir.join(&req.game) } else { req.clips_dir.clone() };
    std::fs::create_dir_all(&dir)
        .with_context(|| l!("nelze vytvořit složku {}", "cannot create folder {}", dir.display()))?;
    let (t_mix, t_game, t_mic) = (
        format!("title={}", tr("Hra + mikrofon", "Game + mic")),
        format!("title={}", tr("Hra", "Game")),
        format!("title={}", tr("Mikrofon", "Mic")),
    );
    let stem = format!("{} {}", req.game, chrono::Local::now().format("%Y-%m-%d %H-%M-%S"));
    let out = unique(&dir, &stem, "mp4");
    let tmp = dir.join(format!(".~{}.mp4", out.file_stem().unwrap().to_string_lossy()));

    let mut a: Vec<String> = ["-fflags", "+genpts", "-f", "mpegts", "-i", "pipe:0"].map(String::from).to_vec();
    let t = snap.tracks;
    if t.game && t.mic {
        a.extend(
            [
                "-filter_complex",
                &format!(
                    "[0:a:1]volume={:.2},aformat=channel_layouts=stereo[m];[0:a:0][m]amix=inputs=2:normalize=0:duration=longest[mix]",
                    req.mic_volume
                ),
                "-map", "0:v", "-map", "[mix]", "-map", "0:a:0", "-map", "0:a:1",
                "-c:v", "copy", "-c:a:0", "aac", "-b:a:0", "192k", "-c:a:1", "copy", "-c:a:2", "copy",
                "-metadata:s:a:0", &t_mix, "-metadata:s:a:1", &t_game, "-metadata:s:a:2", &t_mic,
                "-disposition:a:0", "default", "-disposition:a:1", "0", "-disposition:a:2", "0",
            ]
            .map(String::from),
        );
    } else {
        a.extend(["-map", "0", "-c", "copy"].map(String::from));
        if t.game {
            a.extend(["-metadata:s:a:0".into(), t_game]);
        } else if t.mic {
            a.extend(["-metadata:s:a:0".into(), t_mic]);
        }
    }
    a.extend(["-tag:v", "hvc1", "-movflags", "+faststart", "-f", "mp4"].map(String::from));
    a.push(tmp.to_string_lossy().into_owned());

    let mut child = ffmpeg::command(&req.ffmpeg, 0)
        .args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"])
        .args(&a)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stdin = child.stdin.take().unwrap();
    let writer = std::thread::spawn(move || {
        for p in snap.parts {
            if stdin.write_all(&p).is_err() {
                break;
            }
        }
    });
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    let st = child.wait()?;
    let _ = writer.join();
    if !st.success() || !tmp.is_file() {
        let _ = std::fs::remove_file(&tmp);
        bail!("ffmpeg: {}", ffmpeg::tail(&err));
    }
    std::fs::rename(&tmp, &out)?;
    Ok(out)
}

// ---------- thumbnails ----------

#[derive(Serialize, Clone)]
struct ThumbReady {
    path: String,
    thumb: String,
}

pub struct Thumbs {
    tx: Mutex<Sender<(PathBuf, PathBuf, PathBuf)>>,
    pending: Arc<Mutex<HashSet<PathBuf>>>,
}

impl Thumbs {
    pub fn new(app: AppHandle, ffmpeg: Arc<dyn Fn() -> Option<PathBuf> + Send + Sync>) -> Thumbs {
        let (tx, rx) = mpsc::channel::<(PathBuf, PathBuf, PathBuf)>();
        let pending: Arc<Mutex<HashSet<PathBuf>>> = Default::default();
        let p2 = pending.clone();
        std::thread::Builder::new()
            .name("thumbs".into())
            .spawn(move || {
                for (clips, clip, thumb) in rx {
                    if let Some(ff) = ffmpeg() {
                        if make_thumb(&ff, &clips, &clip, &thumb).is_ok() {
                            let _ = app.emit(
                                "clips-thumb",
                                ThumbReady { path: clip.to_string_lossy().into(), thumb: thumb.to_string_lossy().into() },
                            );
                        }
                    }
                    p2.lock().unwrap().remove(&clip);
                }
            })
            .expect("thumbs thread");
        Thumbs { tx: Mutex::new(tx), pending }
    }

    /// Queues thumbnails for clips that have none.
    pub fn ensure(&self, clips: &Path, list: &[ClipInfo]) {
        for c in list.iter().filter(|c| c.thumb.is_none()) {
            let p = PathBuf::from(&c.path);
            let Ok(meta) = std::fs::metadata(&p) else { continue };
            if self.pending.lock().unwrap().insert(p.clone()) {
                let _ = self.tx.lock().unwrap().send((clips.to_path_buf(), p, thumb_path(clips, &meta)));
            }
        }
    }
}

fn make_thumb(ff: &Path, clips: &Path, clip: &Path, thumb: &Path) -> anyhow::Result<()> {
    let td = thumbs_dir(clips);
    if !td.exists() {
        std::fs::create_dir_all(&td)?;
        super::sys::set_hidden(&td);
    }
    // Highlights are usually at the end of a replay clip.
    let dur = mp4_info(clip).map(|i| i.duration).unwrap_or(0.0);
    let at = if dur > 4.0 { dur - 3.0 } else { dur / 2.0 };
    ffmpeg::run(
        ff,
        &[
            "-ss".into(), format!("{at:.2}"), "-i".into(), clip.to_string_lossy().into(),
            "-frames:v".into(), "1".into(), "-vf".into(), "scale=480:-2".into(), "-q:v".into(), "4".into(),
            "-threads".into(), "2".into(), thumb.to_string_lossy().into(),
        ],
        ffmpeg::BELOW_NORMAL,
    )
}

/// Keyframe film strip for the trim timeline (decodes only keyframes, so it is fast).
pub fn strip(ff: &Path, clips: &Path, clip: &Path) -> anyhow::Result<PathBuf> {
    let meta = std::fs::metadata(clip)?;
    let out = thumbs_dir(clips).join(format!("{}_strip.jpg", key(&meta)));
    if out.is_file() {
        return Ok(out);
    }
    std::fs::create_dir_all(thumbs_dir(clips))?;
    let dur = mp4_info(clip).map(|i| i.duration).unwrap_or(60.0).max(1.0);
    let n = 16;
    let fps = n as f64 / dur;
    ffmpeg::run(
        ff,
        &[
            "-skip_frame".into(), "nokey".into(), "-i".into(), clip.to_string_lossy().into(),
            "-an".into(), "-vf".into(), format!("fps={fps:.5}:round=down,scale=192:-2,tile={n}x1"),
            "-frames:v".into(), "1".into(), "-q:v".into(), "5".into(), out.to_string_lossy().into(),
        ],
        ffmpeg::BELOW_NORMAL,
    )?;
    Ok(out)
}

// ---------- editing ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimReq {
    pub path: String,
    pub start: f64,
    pub end: f64,
    /// "all" | "mix" | "game"
    pub audio: String,
    pub precise: bool,
    pub replace: bool,
    /// Playback speed, 1.0 = unchanged. Anything else forces a re-encode.
    #[serde(default = "one")]
    pub speed: f64,
    /// Optional caption burned into the bottom of the clip.
    #[serde(default)]
    pub text: String,
}

fn one() -> f64 {
    1.0
}

/// Audio pitch-preserving speed; atempo only spans 0.5–2.0, so chain it for extremes.
fn atempo_chain(mut s: f64) -> String {
    let mut parts: Vec<String> = vec![];
    while s > 2.0 {
        parts.push("atempo=2.0".into());
        s /= 2.0;
    }
    while s < 0.5 {
        parts.push("atempo=0.5".into());
        s *= 2.0;
    }
    parts.push(format!("atempo={s:.4}"));
    parts.join(",")
}

/// A bold caption at the bottom of the frame. `font_name` is the font file's bare name — ffmpeg runs
/// with its folder as the working directory, which sidesteps the hopeless Windows-path colon escaping.
fn drawtext_filter(text: &str, font_name: Option<&str>) -> String {
    let safe = text.replace('\\', "\\\\").replace('\'', "\u{2019}").replace(':', "\\:").replace('%', "\\%");
    let font = font_name.map(|f| format!("fontfile={f}:")).unwrap_or_default();
    format!("drawtext={font}text='{safe}':fontcolor=white:fontsize=h/13:borderw=4:bordercolor=black@0.85:x=(w-text_w)/2:y=h-(h/7)")
}

fn system_font() -> Option<PathBuf> {
    #[cfg(windows)]
    let candidates = ["C:/Windows/Fonts/arialbd.ttf", "C:/Windows/Fonts/arial.ttf", "C:/Windows/Fonts/segoeui.ttf"];
    #[cfg(not(windows))]
    let candidates = [
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf",
        "/usr/share/fonts/dejavu/DejaVuSans.ttf",
        "/System/Library/Fonts/Supplemental/Arial.ttf",
    ];
    candidates.iter().map(PathBuf::from).find(|f| f.is_file())
}

#[derive(Serialize, Clone)]
struct Progress {
    path: String,
    pct: f64,
}

pub fn trim(app: &AppHandle, ff: &Path, clips: &Path, r: &TrimReq) -> anyhow::Result<ClipInfo> {
    let src = PathBuf::from(&r.path);
    if !src.is_file() {
        bail!(tr("klip neexistuje", "the clip does not exist"));
    }
    let tracks = mp4_info(&src).map(|i| i.audio_tracks).unwrap_or(1);
    let len = (r.end - r.start).max(0.1);
    let dir = src.parent().ok_or_else(|| anyhow!("bad path"))?;
    let stem = src.file_stem().unwrap().to_string_lossy().into_owned();
    let out = if r.replace { dir.join(format!(".~{stem}.trim.mp4")) } else { unique(dir, &format!("{stem} ({})", tr("střih", "trim")), "mp4") };

    let mut a: Vec<String> = vec![
        "-ss".into(), format!("{:.3}", r.start.max(0.0)), "-i".into(), r.path.clone(), "-t".into(), format!("{len:.3}"),
        "-map".into(), "0:v:0".into(),
    ];
    match r.audio.as_str() {
        // Our clips are [mix, game, mic]; for anything else track 0 is the best guess.
        "game" if tracks >= 3 => a.extend(["-map".into(), "0:a:1".into()]),
        "mix" | "game" => a.extend(["-map".into(), "0:a:0?".into()]),
        _ => a.extend(["-map".into(), "0:a?".into()]),
    }
    // Speed and text are filters, so they force a re-encode; plain trims still stream-copy.
    let speed = if r.speed.is_finite() && r.speed > 0.0 { r.speed.clamp(0.25, 4.0) } else { 1.0 };
    let has_speed = (speed - 1.0).abs() > 0.001;
    let text = r.text.trim();
    let mut vf: Vec<String> = vec![];
    let mut af: Vec<String> = vec![];
    if has_speed {
        vf.push(format!("setpts={:.6}*PTS", 1.0 / speed));
        af.push(atempo_chain(speed));
    }
    let caption_font = if text.is_empty() { None } else { system_font() };
    if !text.is_empty() {
        let name = caption_font.as_ref().and_then(|p| p.file_name()).and_then(|n| n.to_str());
        vf.push(drawtext_filter(text, name));
    }
    let filtered = !vf.is_empty() || !af.is_empty();

    if r.precise {
        a.extend(
            [
                "-c:v", "hevc_nvenc", "-preset", "p7", "-tune", "hq", "-multipass", "fullres", "-rc", "vbr", "-cq", "19", "-b:v", "0",
                "-spatial-aq", "1", "-temporal-aq", "1", "-rc-lookahead", "32", "-bf", "3",
            ]
            .map(String::from),
        );
    } else if filtered {
        // Filtered but not "precise": a universal CPU encode (any GPU, and H.264 plays everywhere).
        a.extend(["-c:v", "libx264", "-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p"].map(String::from));
    } else {
        a.extend(["-c", "copy"].map(String::from));
    }
    if !vf.is_empty() {
        a.push("-vf".into());
        a.push(vf.join(","));
    }
    if !af.is_empty() {
        a.push("-af".into());
        a.push(af.join(","));
        a.extend(["-c:a", "aac", "-b:a", "160k"].map(String::from));
    } else if r.precise {
        a.extend(["-c:a", "copy"].map(String::from));
    }
    a.extend(["-disposition:a:0", "default"].map(String::from));
    // HEVC output keeps the hvc1 tag; the libx264 (filtered, non-precise) output must not.
    if !(filtered && !r.precise) {
        a.extend(["-tag:v", "hvc1"].map(String::from));
    }
    a.extend(["-movflags", "+faststart", "-progress", "pipe:1", "-f", "mp4"].map(String::from));
    a.push(out.to_string_lossy().into_owned());

    let mut cmd = ffmpeg::command(ff, 0);
    // Run from the font's folder so drawtext can use its bare filename (no Windows-path colon to escape).
    if let Some(dir) = caption_font.as_ref().and_then(|p| p.parent()) {
        cmd.current_dir(dir);
    }
    let mut child = cmd
        .args(["-hide_banner", "-nostats", "-loglevel", "error", "-y"])
        .args(&a)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child.stdout.take().unwrap();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(v) = line.strip_prefix("out_time_us=").and_then(|v| v.trim().parse::<f64>().ok()) {
            let pct = (v / 1e6 / (len / speed) * 100.0).clamp(0.0, 100.0);
            let _ = app.emit("clips-trim-progress", Progress { path: r.path.clone(), pct });
        }
    }
    let mut err = String::new();
    if let Some(mut e) = child.stderr.take() {
        let _ = e.read_to_string(&mut err);
    }
    if !child.wait()?.success() || !out.is_file() {
        let _ = std::fs::remove_file(&out);
        bail!("ffmpeg: {}", ffmpeg::tail(&err));
    }
    let final_path = if r.replace {
        super::sys::recycle(&src).or_else(|_| std::fs::remove_file(&src)).context(tr("nelze nahradit originál", "cannot replace the original"))?;
        std::fs::rename(&out, &src)?;
        src
    } else {
        out
    };
    info(clips, &final_path)
}

pub fn rename(path: &str, new_name: &str) -> anyhow::Result<PathBuf> {
    let src = PathBuf::from(path);
    let clean: String = new_name
        .chars()
        .filter(|c| !"<>:\"/\\|?*".contains(*c) && !c.is_control())
        .collect();
    let clean = clean.trim().trim_end_matches('.');
    if clean.is_empty() {
        bail!(tr("neplatný název", "invalid name"));
    }
    let ext = src.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "mp4".into());
    let dst = src.with_file_name(format!("{clean}.{ext}"));
    if dst == src {
        return Ok(dst);
    }
    if dst.exists() {
        bail!(tr("soubor s tímto názvem už existuje", "a file with this name already exists"));
    }
    std::fs::rename(&src, &dst)?;
    Ok(dst)
}
