use crate::net;
use crate::settings;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Instance {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub group: String,
    pub mc_version: String,
    pub loader: String,
    pub loader_version: String,
    /// Version id actually launched (e.g. `fabric-loader-0.16.10-1.21.4`), set after install.
    pub installed_id: String,
    pub ram_mb: Option<u32>,
    pub java_path: String,
    pub jvm_args: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub created: i64,
    pub last_played: i64,
    pub playtime_secs: u64,
    pub template: String,
    /// Active mod profile name; each profile is a set of enabled mod files.
    pub mod_profile: String,
    pub mod_profiles: std::collections::BTreeMap<String, Vec<String>>,
    pub recording_mode: bool,
    /// Disable heavy visual mods (Distant Horizons, dynamic lights…) at launch.
    pub light_mode: bool,
    /// Opt-out of the bundled Justice Mod (in-game menu) for this instance.
    pub no_justice_mod: bool,
}

pub fn dir(id: &str) -> PathBuf {
    settings::instances_dir().join(id)
}
pub fn game_dir(id: &str) -> PathBuf {
    dir(id).join("minecraft")
}
fn json_path(id: &str) -> PathBuf {
    dir(id).join("instance.json")
}

pub fn load(id: &str) -> Result<Instance> {
    let v = net::read_json(&json_path(id)).map_err(|_| anyhow!(l!("Instance {id} neexistuje", "Instance {id} doesn't exist")))?;
    let mut i: Instance = serde_json::from_value(v)?;
    i.id = id.to_string();
    Ok(i)
}

pub fn save(i: &Instance) -> Result<()> {
    std::fs::create_dir_all(game_dir(&i.id))?;
    net::write_json(&json_path(&i.id), &serde_json::to_value(i)?)
}

pub fn list() -> Vec<Instance> {
    let mut out: Vec<Instance> = std::fs::read_dir(settings::instances_dir())
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| load(&e.file_name().to_string_lossy()).ok())
        .collect();
    out.sort_by(|a, b| b.last_played.max(b.created).cmp(&a.last_played.max(a.created)));
    out
}

fn slug(name: &str) -> String {
    let s: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    if s.is_empty() { "instance".into() } else { s }
}

pub fn unique_id(name: &str) -> String {
    let base = slug(name);
    let mut id = base.clone();
    let mut n = 2;
    while dir(&id).exists() {
        id = format!("{base}-{n}");
        n += 1;
    }
    id
}

pub fn create(mut i: Instance) -> Result<Instance> {
    i.id = unique_id(&i.name);
    i.created = chrono::Utc::now().timestamp();
    if i.loader.is_empty() {
        i.loader = "vanilla".into();
    }
    for sub in ["mods", "resourcepacks", "shaderpacks", "saves", "config", "screenshots"] {
        std::fs::create_dir_all(game_dir(&i.id).join(sub))?;
    }
    save(&i)?;
    Ok(i)
}

pub fn delete(id: &str) -> Result<()> {
    let d = dir(id);
    if d.exists() {
        std::fs::remove_dir_all(d)?;
    }
    Ok(())
}

pub fn copy_dir(from: &std::path::Path, to: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(to)?;
    for e in std::fs::read_dir(from)?.flatten() {
        let t = to.join(e.file_name());
        if e.file_type()?.is_dir() {
            copy_dir(&e.path(), &t)?;
        } else {
            std::fs::copy(e.path(), t)?;
        }
    }
    Ok(())
}

pub fn duplicate(id: &str) -> Result<Instance> {
    let mut i = load(id)?;
    i.name = format!("{} (kopie)", i.name);
    let new_id = unique_id(&i.name);
    copy_dir(&dir(id), &dir(&new_id))?;
    i.id = new_id;
    i.created = chrono::Utc::now().timestamp();
    i.playtime_secs = 0;
    i.last_played = 0;
    save(&i)?;
    Ok(i)
}

/// Where the player was last in this instance: a server or a singleplayer world, with a picture.
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LastPlace {
    /// "server" | "world"
    pub kind: String,
    /// Server address or world name.
    pub name: String,
    /// The newest screenshot, else the world's icon (absolute path).
    pub image: Option<String>,
}

fn newest(dir: PathBuf, keep: impl Fn(&std::path::Path) -> bool) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| keep(p))
        .max_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok())
}

pub fn last_place(id: &str) -> Option<LastPlace> {
    let gd = game_dir(id);
    // The last session's log says whether it last connected to a server or started a singleplayer
    // world ("Starting integrated minecraft server"); a world is then the most recently saved one,
    // whose folder name is also what quick play needs.
    static RE: once_cell::sync::Lazy<regex::Regex> = once_cell::sync::Lazy::new(|| {
        regex::Regex::new(r"\]: (?:Connecting to ([^,\s]+), (\d+)$|Starting integrated minecraft server)").unwrap()
    });
    let log = std::fs::read(gd.join("logs").join("latest.log")).ok().map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
    let newest_world = || newest(gd.join("saves"), |p| p.join("level.dat").is_file()).and_then(|p| Some(("world", p.file_name()?.to_string_lossy().into_owned())));
    let place = match log.lines().filter_map(|l| RE.captures(l.trim_end())).last() {
        Some(c) => match (c.get(1), c.get(2)) {
            (Some(host), Some(port)) => Some(("server", if port.as_str() == "25565" { host.as_str().to_string() } else { format!("{}:{}", host.as_str(), port.as_str()) })),
            _ => newest_world(),
        },
        // No session log: the most recently saved world, if any.
        None => newest_world(),
    };
    let (kind, name) = place?;
    let shot = newest(gd.join("screenshots"), |p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")));
    let icon = (kind == "world").then(|| gd.join("saves").join(&name).join("icon.png")).filter(|p| p.is_file());
    Some(LastPlace { kind: kind.into(), name, image: shot.or(icon).map(|p| p.to_string_lossy().into_owned()) })
}
