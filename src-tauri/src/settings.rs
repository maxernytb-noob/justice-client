use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::RwLock;

/// Azure app registered for Justice Client (personal MS accounts, public client flows).
pub const DEFAULT_MS_CLIENT_ID: &str = "1d78c41d-dee8-40da-8516-194ccd654143";
/// Discord app registered for Justice Client, shown as "Playing Justice Client".
pub const DEFAULT_DISCORD_APP_ID: &str = "1553031868969128057";

/// The Discord app for Rich Presence: the user's own if they set one, otherwise Justice Client's.
pub fn discord_app_id() -> String {
    let id = get().discord_app_id.trim().to_string();
    if id.is_empty() { DEFAULT_DISCORD_APP_ID.into() } else { id }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub data_dir: String,
    pub ram_mb: u32,
    pub jvm_args: String,
    /// "hide" | "close" | "keep"
    pub on_launch: String,
    pub language: String,
    pub curseforge_key: String,
    pub ms_client_id: String,
    pub discord_rpc: bool,
    pub discord_app_id: String,
    pub streamer_mode: bool,
    pub backup_on_exit: bool,
    pub width: u32,
    pub height: u32,
    pub high_priority: bool,
    /// Garbage collector: "auto" (ZGC on Java 21+, else G1) | "zgc" | "g1"
    pub gc: String,
    pub obs_enabled: bool,
    pub obs_url: String,
    pub obs_password: String,
    pub obs_auto_record: bool,
    pub obs_scene: String,
    pub onboarded: bool,
    /// Accent colour as `#rrggbb`.
    pub accent_color: String,
    /// "oled" | "dark" | "graphite" or a theme preset background ("void", "midnight"…)
    pub bg_style: String,
    /// Quick-join button id ("justicesmp", "ertenix") -> instance id to launch it with.
    pub quick_servers: std::collections::BTreeMap<String, String>,
    /// Justice Mod (in-game menu) in supported Fabric instances; each instance can still opt out.
    pub justice_mod: bool,
    /// Pop a standalone live-log window when a game launches (Lunar-style). On by default.
    pub log_window_on_launch: bool,
    /// Look & layout preferences owned by the UI (background, radius, sidebar order, shortcuts…).
    pub ui: serde_json::Value,
}

impl Default for Settings {
    fn default() -> Self {
        let data_dir = default_data_dir();
        Self {
            data_dir: data_dir.to_string_lossy().into_owned(),
            ram_mb: recommended_ram_mb(),
            jvm_args: String::new(),
            on_launch: "hide".into(),
            language: "cs".into(),
            curseforge_key: String::new(),
            ms_client_id: DEFAULT_MS_CLIENT_ID.into(),
            discord_rpc: true,
            discord_app_id: String::new(),
            streamer_mode: false,
            backup_on_exit: false,
            width: 1280,
            height: 720,
            high_priority: true,
            gc: "auto".into(),
            obs_enabled: false,
            obs_url: "ws://127.0.0.1:4455".into(),
            obs_password: String::new(),
            obs_auto_record: false,
            obs_scene: String::new(),
            onboarded: false,
            accent_color: "#e11d38".into(),
            bg_style: "void".into(),
            quick_servers: Default::default(),
            justice_mod: true,
            log_window_on_launch: true,
            ui: serde_json::Value::Null,
        }
    }
}

pub fn total_ram_mb() -> u64 {
    crate::platform::total_ram_mb()
}

/// Heap for the game. Leaves plenty for Windows, OBS and Discord: on a 16 GB PC giving the game
/// more than ~6 GB pushes the rest into the page file, which stutters far worse than a smaller heap.
pub fn recommended_ram_mb() -> u32 {
    match total_ram_mb() {
        0..=6144 => 2048,
        6145..=8192 => 3072,
        8193..=12288 => 4096,
        12289..=16384 => 6144,
        _ => 8192,
    }
}

pub fn cpu_threads() -> usize {
    std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)
}

fn settings_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("JusticeClient")
        .join("settings.json")
}

static SETTINGS: Lazy<RwLock<Settings>> = Lazy::new(|| {
    let s = std::fs::read_to_string(settings_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    RwLock::new(s)
});

pub fn get() -> Settings {
    SETTINGS.read().unwrap().clone()
}

pub fn save(s: Settings) -> anyhow::Result<()> {
    let p = settings_path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, serde_json::to_string_pretty(&s)?)?;
    *SETTINGS.write().unwrap() = s;
    Ok(())
}

pub fn data_dir() -> PathBuf {
    PathBuf::from(get().data_dir)
}
pub fn meta_dir() -> PathBuf {
    data_dir().join("meta")
}
pub fn instances_dir() -> PathBuf {
    data_dir().join("instances")
}
pub fn java_dir() -> PathBuf {
    data_dir().join("java")
}
pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn free_space(path: &std::path::Path) -> Option<u64> {
    crate::platform::free_space(path)
}

/// Default data folder: AppData, unless that drive is nearly full and another fixed drive has room.
fn default_data_dir() -> PathBuf {
    let appdata = dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("JusticeClient");
    const GB: u64 = 1 << 30;
    // Only Windows has several drive letters worth probing; on Linux ~/.local/share is the place.
    if !cfg!(windows) || appdata.exists() || free_space(&appdata).unwrap_or(u64::MAX) >= 20 * GB {
        return appdata;
    }
    ('D'..='H')
        .map(|d| PathBuf::from(format!("{d}:\\")))
        .filter(|r| r.exists())
        .filter_map(|r| free_space(&r).map(|f| (r, f)))
        .filter(|(_, f)| *f >= 20 * GB)
        .max_by_key(|(_, f)| *f)
        .map(|(r, _)| r.join("JusticeClient"))
        .unwrap_or(appdata)
}
