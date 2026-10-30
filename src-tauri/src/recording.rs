//! Recording mode (cinematic options.txt tweaks), mod profiles and Flashback replay library.

use crate::content;
use crate::instances::{self, Instance};
use anyhow::{bail, Result};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;
use std::path::Path;

/// options.txt values applied when recording mode is on; the originals are saved and restored.
const RECORDING_OPTIONS: &[(&str, &str)] = &[
    ("bobView", "false"),
    ("fovEffectScale", "0.0"),
    ("screenEffectScale", "0.0"),
    ("darknessEffectScale", "0.0"),
    ("damageTiltStrength", "0.0"),
    ("showAutosaveIndicator", "false"),
    ("tutorialStep", "none"),
    ("onboardAccessibility", "false"),
    ("narrator", "0"),
];

fn read_options(p: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(p)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(':').map(|(k, v)| (k.to_string(), v.to_string())))
        .collect()
}

fn write_options(p: &Path, opts: &[(String, String)]) -> Result<()> {
    let s: String = opts.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    std::fs::write(p, s)?;
    Ok(())
}

fn set_opt(opts: &mut Vec<(String, String)>, k: &str, v: &str) {
    match opts.iter_mut().find(|(key, _)| key == k) {
        Some(e) => e.1 = v.to_string(),
        None => opts.push((k.to_string(), v.to_string())),
    }
}

pub fn apply_before_launch(inst: &Instance, game_dir: &Path) -> Result<()> {
    let opts_path = game_dir.join("options.txt");
    let backup = game_dir.join(".justice").join("options.before-recording.txt");
    let mut opts = read_options(&opts_path);
    if inst.recording_mode {
        if !backup.exists() {
            std::fs::create_dir_all(backup.parent().unwrap())?;
            let saved: Vec<(String, String)> = RECORDING_OPTIONS
                .iter()
                .filter_map(|(k, _)| opts.iter().find(|(key, _)| key == k).cloned())
                .collect();
            write_options(&backup, &saved)?;
        }
        for (k, v) in RECORDING_OPTIONS {
            set_opt(&mut opts, k, v);
        }
        write_options(&opts_path, &opts)?;
    } else if backup.exists() {
        let saved = read_options(&backup);
        opts.retain(|(k, _)| !RECORDING_OPTIONS.iter().any(|(r, _)| r == k) || saved.iter().any(|(s, _)| s == k));
        for (k, v) in saved {
            set_opt(&mut opts, &k, &v);
        }
        write_options(&opts_path, &opts)?;
        std::fs::remove_file(backup)?;
    }
    Ok(())
}

// ---------------- Mod profiles ----------------

pub fn save_profile(inst_id: &str, name: &str) -> Result<()> {
    let mut inst = instances::load(inst_id)?;
    let enabled: Vec<String> = content::list_installed(inst_id)
        .into_iter()
        .filter(|m| m.kind == "mod" && m.enabled)
        .map(|m| m.file)
        .collect();
    inst.mod_profiles.insert(name.to_string(), enabled);
    inst.mod_profile = name.to_string();
    instances::save(&inst)
}

pub fn apply_profile(inst_id: &str, name: &str) -> Result<()> {
    let mut inst = instances::load(inst_id)?;
    let Some(enabled) = inst.mod_profiles.get(name).cloned() else { bail!(l!("Profil {name} neexistuje", "Profile {name} doesn't exist")) };
    for m in content::list_installed(inst_id).into_iter().filter(|m| m.kind == "mod") {
        let base = m.file.trim_end_matches(".disabled").to_string();
        let want = enabled.iter().any(|e| e.trim_end_matches(".disabled") == base);
        if want != m.enabled {
            content::toggle(inst_id, "mod", &m.file, want)?;
        }
    }
    inst.mod_profile = name.to_string();
    instances::save(&inst)
}

pub fn delete_profile(inst_id: &str, name: &str) -> Result<()> {
    let mut inst = instances::load(inst_id)?;
    inst.mod_profiles.remove(name);
    if inst.mod_profile == name {
        inst.mod_profile.clear();
    }
    instances::save(&inst)
}

// ---------------- Flashback & media library ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaFile {
    pub instance: String,
    pub instance_name: String,
    pub path: String,
    pub name: String,
    pub kind: String,
    pub size: u64,
    pub modified: i64,
    pub duration_secs: Option<u64>,
    pub world: String,
}

fn modified(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn replay_meta(p: &Path) -> (Option<u64>, String, String) {
    let read = || -> Option<Value> {
        let mut z = zip::ZipArchive::new(std::fs::File::open(p).ok()?).ok()?;
        let mut s = String::new();
        z.by_name("metadata.json").ok()?.read_to_string(&mut s).ok()?;
        serde_json::from_str(&s).ok()
    };
    let Some(v) = read() else { return (None, String::new(), String::new()) };
    let ticks = v["total_ticks"].as_u64().or(v["totalTicks"].as_u64());
    (
        ticks.map(|t| t / 20),
        v["world_name"].as_str().or(v["worldName"].as_str()).unwrap_or_default().to_string(),
        v["name"].as_str().unwrap_or_default().to_string(),
    )
}

fn walk(dir: &Path, depth: u32, out: &mut Vec<std::path::PathBuf>) {
    if depth > 4 {
        return;
    }
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, depth + 1, out);
        } else {
            out.push(p);
        }
    }
}

/// Flashback replays, video exports and screenshots across all instances.
pub fn media(kind_filter: &str) -> Vec<MediaFile> {
    let mut out = vec![];
    for inst in instances::list() {
        let game = instances::game_dir(&inst.id);
        let mut files = vec![];
        if kind_filter != "screenshot" {
            walk(&game.join("flashback"), 0, &mut files);
            walk(&game.join("replay_recordings"), 0, &mut files);
            walk(&game.join("recordings"), 0, &mut files);
        }
        if kind_filter != "replay" && kind_filter != "video" {
            walk(&game.join("screenshots"), 0, &mut files);
        }
        for p in files {
            let ext = p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
            let under_flashback = p.components().any(|c| c.as_os_str() == "flashback");
            let kind = match ext.as_str() {
                "zip" if under_flashback => "replay",
                "mcpr" => "replay",
                "mp4" | "webm" | "mov" | "mkv" | "gif" => "video",
                "png" | "jpg" | "jpeg" => "screenshot",
                _ => continue,
            };
            if !kind_filter.is_empty() && kind_filter != kind {
                continue;
            }
            let (duration_secs, world, title) = if kind == "replay" && ext == "zip" { replay_meta(&p) } else { (None, String::new(), String::new()) };
            out.push(MediaFile {
                instance: inst.id.clone(),
                instance_name: inst.name.clone(),
                name: if title.is_empty() { p.file_stem().unwrap_or_default().to_string_lossy().into_owned() } else { title },
                path: p.to_string_lossy().into_owned(),
                kind: kind.into(),
                size: std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0),
                modified: modified(&p),
                duration_secs,
                world,
            });
        }
    }
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}
