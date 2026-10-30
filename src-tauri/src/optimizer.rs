//! The Optimize button. Looks at an instance and at the PC, proposes changes (missing performance
//! mods, mods that fight them, video settings too heavy for the hardware, memory), applies the ones
//! the user keeps, and saves what it changed so one click puts everything back.

use crate::content::{self, Kind};
use crate::{instances, net, settings};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Change {
    pub id: String,
    /// "mods" | "conflicts" | "video" | "sodium" | "memory"
    pub group: String,
    pub title: String,
    pub detail: String,
    pub from: Option<String>,
    pub to: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    /// "low" | "mid" | "high"
    pub tier: String,
    pub hardware: String,
    pub changes: Vec<Change>,
    pub undo_available: bool,
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub applied: Vec<String>,
    pub installed: Vec<String>,
    pub unavailable: Vec<String>,
}

/// What a backup remembers, next to the copied files.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Backup {
    ram_mb: Option<u32>,
    had_options: bool,
    had_sodium: bool,
    installed_files: Vec<String>,
    disabled_files: Vec<String>,
}

fn tier() -> (&'static str, String) {
    let threads = settings::cpu_threads();
    let ram = settings::total_ram_mb();
    let t = if threads <= 4 || ram <= 8192 {
        "low"
    } else if threads <= 8 || ram <= 16384 {
        "mid"
    } else {
        "high"
    };
    (t, l!("{} vláken CPU, {} GB RAM", "{} CPU threads, {} GB RAM", threads, (ram + 512) / 1024))
}

fn backups_dir(id: &str) -> PathBuf {
    instances::dir(id).join("optimize-backups")
}

fn latest_backup(id: &str) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(backups_dir(id)).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v.pop()
}

fn mod_files(mods: &Path) -> Vec<String> {
    std::fs::read_dir(mods)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".jar"))
        .collect()
}

/// options.txt as ordered key/value pairs (unknown lines kept as they are).
fn read_options(path: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .map(|l| match l.split_once(':') {
            Some((k, v)) => (k.to_string(), v.to_string()),
            None => (l.to_string(), String::new()),
        })
        .collect()
}

fn write_options(path: &Path, opts: &[(String, String)]) -> Result<()> {
    let text: String = opts.iter().map(|(k, v)| if v.is_empty() && !k.contains(':') { format!("{k}\n") } else { format!("{k}:{v}\n") }).collect();
    std::fs::write(path, text)?;
    Ok(())
}

/// Video settings that cost a lot for little, per hardware tier: key, label, too-heavy test, value to set.
fn video_rules(tier: &str) -> Vec<(&'static str, String, Box<dyn Fn(&str) -> bool>, String)> {
    let (rd, sim) = match tier {
        "low" => (8, 6),
        "mid" => (12, 8),
        _ => (16, 10),
    };
    let num = |v: &str| v.trim().parse::<i64>().unwrap_or(0);
    let mut r: Vec<(&'static str, String, Box<dyn Fn(&str) -> bool>, String)> = vec![
        ("renderDistance", l!("Dohled", "Render distance"), Box::new(move |v| num(v) > rd), rd.to_string()),
        ("simulationDistance", l!("Simulační vzdálenost", "Simulation distance"), Box::new(move |v| num(v) > sim), sim.to_string()),
        // Fabulous is a second full-screen pass for translucency; most players can't tell it apart.
        ("graphicsMode", l!("Grafika", "Graphics"), Box::new(|v| v.trim() == "2" || v.contains("fabulous")), "1".into()),
        ("biomeBlendRadius", l!("Prolínání biomů", "Biome blend"), Box::new(move |v| num(v) > 2), "1".into()),
    ];
    if tier == "low" {
        r.push(("entityShadows", l!("Stíny entit", "Entity shadows"), Box::new(|v| v.trim() == "true"), "false".into()));
        r.push(("particles", l!("Částice", "Particles"), Box::new(|v| v.trim() == "0"), "1".into()));
        r.push(("renderClouds", l!("Mraky", "Clouds"), Box::new(|v| v.contains("true")), "\"fast\"".into()));
        r.push(("graphicsMode", l!("Grafika", "Graphics"), Box::new(|v| v.trim() == "1" || v.contains("fancy")), "0".into()));
    }
    r
}

fn video_label(key: &str, value: &str) -> String {
    match (key, value.trim().trim_matches('"')) {
        ("graphicsMode", "0") | ("graphicsMode", "fast") => l!("Rychlá", "Fast"),
        ("graphicsMode", "1") | ("graphicsMode", "fancy") => l!("Pěkná", "Fancy"),
        ("graphicsMode", "2") | ("graphicsMode", "fabulous") => l!("Úžasná", "Fabulous"),
        ("particles", "0") => l!("Všechny", "All"),
        ("particles", "1") => l!("Omezené", "Decreased"),
        ("entityShadows", "true") | ("renderClouds", "true") => l!("Zapnuto", "On"),
        ("entityShadows", "false") | ("renderClouds", "false") => l!("Vypnuto", "Off"),
        ("renderClouds", "fast") => l!("Rychlé", "Fast"),
        ("renderDistance", v) | ("simulationDistance", v) => l!("{} chunků", "{} chunks", v),
        (_, v) => v.to_string(),
    }
}

/// Performance mods from the pack that this instance doesn't have and that exist for its version.
async fn missing_mods(inst: &instances::Instance) -> (Vec<String>, Vec<String>) {
    let slugs = content::performance_slugs(&inst.loader);
    if slugs.is_empty() {
        return (vec![], vec![]);
    }
    let have = content::installed_projects(&inst.id);
    let all_files = mod_files(&instances::game_dir(&inst.id).join("mods"));
    let files: Vec<String> = all_files.iter().map(|f| f.to_lowercase().replace(['-', '_'], "")).collect();
    // Big modpacks often bring their own world generation; these reach deep into it or into block
    // entity rendering, so they're only offered to lighter instances.
    let heavy = all_files.len() > 40;
    let slugs: Vec<&str> = slugs.into_iter().filter(|s| !heavy || !matches!(*s, "c2me-fabric" | "zfastnoise" | "vmp-fabric" | "obe")).collect();
    let ids = serde_json::to_string(&slugs).unwrap_or_default();
    let projects = net::get_json(&format!("https://api.modrinth.com/v2/projects?ids={}", urlencode(&ids))).await.unwrap_or(Value::Null);
    let mut id_of: BTreeMap<String, (String, String)> = BTreeMap::new();
    for p in projects.as_array().into_iter().flatten() {
        if let (Some(slug), Some(id)) = (p["slug"].as_str(), p["id"].as_str()) {
            id_of.insert(slug.to_string(), (id.to_string(), short_title(slug, p["title"].as_str().unwrap_or(slug))));
        }
    }
    let loader = if inst.loader == "quilt" { "[\"fabric\",\"quilt\"]".to_string() } else { format!("[\"{}\"]", inst.loader) };
    let checks = slugs.iter().filter_map(|s| {
        let (id, title) = id_of.get(*s)?.clone();
        let token = s.replace(['-', '_'], "");
        if have.contains(&id) || files.iter().any(|f| f.starts_with(&token)) {
            return None;
        }
        let url = format!(
            "https://api.modrinth.com/v2/project/{id}/version?loaders={}&game_versions={}",
            urlencode(&loader),
            urlencode(&format!("[\"{}\"]", inst.mc_version))
        );
        Some(async move {
            let ok = net::get_json(&url).await.ok().and_then(|v| v.as_array().map(|a| !a.is_empty())).unwrap_or(false);
            (s.to_string(), title, ok)
        })
    });
    let mut add = vec![];
    let mut titles = vec![];
    for (slug, title, ok) in futures::future::join_all(checks).await {
        if ok {
            add.push(slug);
            titles.push(title);
        }
    }
    (add, titles)
}

/// Modrinth titles as a short list entry.
fn short_title(slug: &str, title: &str) -> String {
    match slug {
        "c2me-fabric" => "C2ME".into(),
        "vmp-fabric" => "VMP".into(),
        "forcecloseworldloadingscreen" => "Force Close Loading Screen".into(),
        "obe" => "OBE".into(),
        _ => title.trim_end_matches(" (Fabric)").to_string(),
    }
}

fn urlencode(s: &str) -> String {
    s.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// Mods that fight Sodium: OptiFine and the bridges that load it on Fabric.
fn conflicts(mods: &Path, sodium_coming: bool) -> Vec<String> {
    let files = mod_files(mods);
    let has_sodium = sodium_coming || files.iter().any(|f| f.to_lowercase().starts_with("sodium-fabric") || f.to_lowercase().starts_with("sodium-neoforge"));
    if !has_sodium {
        return vec![];
    }
    files.into_iter().filter(|f| {
        let l = f.to_lowercase();
        l.starts_with("optifine") || l.starts_with("optifabric")
    }).collect()
}

pub async fn plan(id: &str) -> Result<Plan> {
    let inst = instances::load(id)?;
    let game = instances::game_dir(id);
    let (tier, hardware) = tier();
    let mut changes = vec![];

    let (add, titles) = missing_mods(&inst).await;
    if !add.is_empty() {
        changes.push(Change {
            id: "mods".into(),
            group: "mods".into(),
            title: l!("Doinstalovat {} výkonových modů", "Install {} performance mods", add.len()),
            detail: titles.join(", "),
            from: None,
            to: None,
        });
    }

    for f in conflicts(&game.join("mods"), add.iter().any(|s| s == "sodium")) {
        changes.push(Change {
            id: format!("disable:{f}"),
            group: "conflicts".into(),
            title: l!("Vypnout {}", "Turn off {}", f),
            detail: l!("OptiFine se Sodiem padá nebo zpomaluje; Sodium dělá totéž lépe.", "OptiFine crashes or slows down next to Sodium, which does the same job better."),
            from: None,
            to: None,
        });
    }

    let opts = read_options(&game.join("options.txt"));
    let mut seen = HashSet::new();
    for (key, label, too_heavy, to) in video_rules(tier).into_iter().rev() {
        if !seen.insert(key) {
            continue; // a stricter rule for the same key came later in the list and won
        }
        if let Some((_, v)) = opts.iter().find(|(k, _)| k == key) {
            if too_heavy(v) {
                changes.push(Change {
                    id: format!("opt:{key}={to}"),
                    group: "video".into(),
                    title: label,
                    detail: String::new(),
                    from: Some(video_label(key, v)),
                    to: Some(video_label(key, &to)),
                });
            }
        }
    }

    let sodium_path = game.join("config").join("sodium-options.json");
    if let Ok(text) = std::fs::read_to_string(&sodium_path) {
        let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
        let threads = v["performance"]["chunk_builder_threads"].as_i64().unwrap_or(0);
        let staging = v["advanced"]["use_advanced_staging_buffers"].as_bool().unwrap_or(true);
        if tier != "high" && (threads == 0 || staging) {
            changes.push(Change {
                id: "sodium".into(),
                group: "sodium".into(),
                title: l!("Nastavení Sodia pro tvůj procesor", "Sodium settings for your CPU"),
                detail: l!("Méně vláken na stavbu chunků a bez pokročilých staging bufferů: při načítání světa nebere procesor hře.", "Fewer chunk builder threads and no advanced staging buffers, so loading the world doesn't take the CPU from the game."),
                from: None,
                to: None,
            });
        }
    }

    let s = settings::get();
    let current = inst.ram_mb.unwrap_or(s.ram_mb);
    let mods = mod_files(&game.join("mods")).len() + add.len();
    let total = settings::total_ram_mb() as u32;
    let want = (settings::recommended_ram_mb()).max(if mods > 120 { 4096 } else if mods > 60 { 3072 } else { 2048 }).min(total * 6 / 10);
    if current + 512 < want || current > total * 6 / 10 {
        changes.push(Change {
            id: format!("ram:{want}"),
            group: "memory".into(),
            title: l!("Paměť pro hru", "Memory for the game"),
            detail: if current > want {
                l!("Víc paměti než hra potřebuje bere RAM systému a hra se pak seká.", "More memory than the game needs takes RAM from the system and the game stutters.")
            } else {
                l!("Na {} modů je to málo, hra by čistila paměť příliš často.", "Too little for {} mods; the game would collect garbage too often.", mods)
            },
            from: Some(format!("{} GB", fmt_gb(current))),
            to: Some(format!("{} GB", fmt_gb(want))),
        });
    }

    Ok(Plan { tier: tier.into(), hardware, changes, undo_available: latest_backup(id).is_some() })
}

fn fmt_gb(mb: u32) -> String {
    let gb = mb as f64 / 1024.0;
    if (gb - gb.round()).abs() < 0.05 { format!("{}", gb.round()) } else { format!("{gb:.1}") }
}

pub async fn apply(id: &str, ids: Vec<String>) -> Result<ApplyReport> {
    let mut inst = instances::load(id)?;
    let game = instances::game_dir(id);
    let mods_dir = game.join("mods");
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let bdir = backups_dir(id).join(&stamp);
    std::fs::create_dir_all(&bdir)?;
    let options = game.join("options.txt");
    let sodium = game.join("config").join("sodium-options.json");
    let mut backup = Backup { ram_mb: inst.ram_mb, had_options: options.exists(), had_sodium: sodium.exists(), ..Default::default() };
    if backup.had_options {
        std::fs::copy(&options, bdir.join("options.txt"))?;
    }
    if backup.had_sodium {
        std::fs::copy(&sodium, bdir.join("sodium-options.json"))?;
    }
    let mut report = ApplyReport::default();

    if ids.iter().any(|i| i == "mods") {
        let before: HashSet<String> = mod_files(&mods_dir).into_iter().collect();
        let (add, _) = missing_mods(&inst).await;
        let refs: Vec<&str> = add.iter().map(String::as_str).collect();
        let r = content::install_many(id, &refs, Kind::Mod).await;
        report.installed = r.installed;
        report.unavailable = r.skipped;
        backup.installed_files = mod_files(&mods_dir).into_iter().filter(|f| !before.contains(f)).collect();
        report.applied.push("mods".into());
    }

    for i in &ids {
        if let Some(file) = i.strip_prefix("disable:") {
            let from = mods_dir.join(file);
            if from.exists() {
                std::fs::rename(&from, mods_dir.join(format!("{file}.disabled")))?;
                backup.disabled_files.push(file.to_string());
                report.applied.push(i.clone());
            }
        }
    }

    let opt_changes: Vec<(String, String)> = ids
        .iter()
        .filter_map(|i| i.strip_prefix("opt:").and_then(|kv| kv.split_once('=')).map(|(k, v)| (k.to_string(), v.to_string())))
        .collect();
    if !opt_changes.is_empty() && backup.had_options {
        let mut opts = read_options(&options);
        for (k, v) in &opt_changes {
            if let Some(entry) = opts.iter_mut().find(|(key, _)| key == k) {
                entry.1 = v.clone();
                report.applied.push(format!("opt:{k}"));
            }
        }
        write_options(&options, &opts)?;
    }

    if ids.iter().any(|i| i == "sodium") && backup.had_sodium {
        let mut v: Value = serde_json::from_str(&std::fs::read_to_string(&sodium)?).unwrap_or(Value::Null);
        if v.is_object() {
            let builders = match settings::cpu_threads() {
                0..=4 => 1,
                5..=8 => 2,
                _ => 3,
            };
            v["performance"]["chunk_builder_threads"] = builders.into();
            v["advanced"]["use_advanced_staging_buffers"] = false.into();
            std::fs::write(&sodium, serde_json::to_vec_pretty(&v)?)?;
            report.applied.push("sodium".into());
        }
    }

    if let Some(mb) = ids.iter().find_map(|i| i.strip_prefix("ram:").and_then(|v| v.parse::<u32>().ok())) {
        inst.ram_mb = Some(mb);
        instances::save(&inst)?;
        report.applied.push("ram".into());
    }

    std::fs::write(bdir.join("backup.json"), serde_json::to_vec_pretty(&backup)?)?;
    Ok(report)
}

/// Puts back what the last optimization changed, then forgets that backup.
pub fn undo(id: &str) -> Result<()> {
    let Some(bdir) = latest_backup(id) else { anyhow::bail!(l!("Není co vrátit", "Nothing to undo")) };
    let b: Backup = serde_json::from_slice(&std::fs::read(bdir.join("backup.json"))?)?;
    let game = instances::game_dir(id);
    let mods = game.join("mods");
    for f in &b.installed_files {
        let _ = std::fs::remove_file(mods.join(f));
    }
    content::forget_missing(id);
    for f in &b.disabled_files {
        let off = mods.join(format!("{f}.disabled"));
        if off.exists() {
            std::fs::rename(off, mods.join(f))?;
        }
    }
    if b.had_options {
        std::fs::copy(bdir.join("options.txt"), game.join("options.txt"))?;
    }
    if b.had_sodium {
        std::fs::copy(bdir.join("sodium-options.json"), game.join("config").join("sodium-options.json"))?;
    }
    let mut inst = instances::load(id)?;
    inst.ram_mb = b.ram_mb;
    instances::save(&inst)?;
    std::fs::remove_dir_all(bdir)?;
    Ok(())
}
