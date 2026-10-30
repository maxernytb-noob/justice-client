use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub clips_dir: String,
    pub hotkey: String,
    /// Seconds kept in the replay buffer (= clip length).
    pub buffer_secs: u32,
    /// Extra seconds recorded after the hotkey before the clip is cut.
    pub post_secs: f32,
    pub fps: u32,
    /// "small" | "balanced" | "high"
    pub quality: String,
    /// DXGI output index of the captured monitor.
    pub monitor: u32,
    pub draw_mouse: bool,
    pub game_audio: bool,
    /// Empty = Windows default device.
    pub game_device: String,
    pub mic: bool,
    pub mic_device: String,
    pub mic_volume: f32,
    /// Positive = audio later.
    pub audio_offset_ms: i32,
    pub overlay: bool,
    pub sound: bool,
    pub game_folders: bool,
    /// Empty = auto-detect.
    pub ffmpeg_path: String,
    /// Replay buffer runs while a Minecraft instance from the launcher is running.
    pub enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            clips_dir: default_clips_dir().to_string_lossy().into_owned(),
            hotkey: "F8".into(),
            buffer_secs: 60,
            post_secs: 1.0,
            fps: 60,
            quality: "balanced".into(),
            monitor: 0,
            draw_mouse: true,
            game_audio: true,
            game_device: String::new(),
            mic: true,
            mic_device: String::new(),
            mic_volume: 1.0,
            audio_offset_ms: 0,
            overlay: true,
            sound: true,
            game_folders: true,
            ffmpeg_path: String::new(),
            enabled: true,
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.buffer_secs = self.buffer_secs.clamp(10, 600);
        self.post_secs = self.post_secs.clamp(0.0, 10.0);
        if ![30, 60, 120].contains(&self.fps) {
            self.fps = 60;
        }
        if !["small", "balanced", "high"].contains(&self.quality.as_str()) {
            self.quality = "balanced".into();
        }
        self.mic_volume = self.mic_volume.clamp(0.0, 3.0);
        self.audio_offset_ms = self.audio_offset_ms.clamp(-1000, 1000);
        if self.hotkey.trim().is_empty() {
            self.hotkey = "F8".into();
        }
        if self.clips_dir.trim().is_empty() {
            self.clips_dir = default_clips_dir().to_string_lossy().into_owned();
        }
    }

    /// True when a change requires restarting the capture pipeline.
    pub fn capture_changed(&self, o: &Settings) -> bool {
        self.fps != o.fps
            || self.quality != o.quality
            || self.monitor != o.monitor
            || self.draw_mouse != o.draw_mouse
            || self.game_audio != o.game_audio
            || self.game_device != o.game_device
            || self.mic != o.mic
            || self.mic_device != o.mic_device
            || self.audio_offset_ms != o.audio_offset_ms
            || self.ffmpeg_path != o.ffmpeg_path
    }
}

fn default_clips_dir() -> PathBuf {
    // C: is small on this machine; prefer D: when it exists.
    if PathBuf::from("D:\\").exists() {
        PathBuf::from("D:\\Justice Clips")
    } else {
        dirs::video_dir()
            .unwrap_or_else(|| dirs::home_dir().unwrap_or_default())
            .join("Justice Clips")
    }
}

fn path() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("JusticeClient").join("clips.json")
}

/// Settings of the old standalone Justice Clips app, taken over on first start.
fn legacy_path() -> PathBuf {
    dirs::config_dir().unwrap_or_default().join("JusticeClips").join("settings.json")
}

pub fn load() -> Settings {
    let read = |p: PathBuf| std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str::<Settings>(&t).ok());
    let mut s = read(path()).or_else(|| read(legacy_path())).unwrap_or_default();
    s.sanitize();
    s
}

pub fn save(s: &Settings) -> anyhow::Result<()> {
    let p = path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    let tmp = p.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(s)?)?;
    std::fs::rename(tmp, p)?;
    Ok(())
}
