//! Fabric mod compatibility: reads `depends` / `breaks` from every enabled mod and, when two
//! tracked Modrinth mods conflict, swaps one of them for a version that satisfies the other.

use crate::content::{self, Kind};
use crate::instances;
use crate::net::{self, Job};
use crate::settings;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
struct ModInfo {
    id: String,
    version: String,
    provides: Vec<String>,
    depends: Vec<(String, Vec<String>)>,
    breaks: Vec<(String, Vec<String>)>,
}

fn preds(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(String::from)).collect(),
        _ => vec!["*".into()],
    }
}

fn read_fabric_json(path: &Path) -> Option<ModInfo> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let mut s = String::new();
    z.by_name("fabric.mod.json").ok()?.read_to_string(&mut s).ok()?;
    // Some mods ship JSON with raw newlines inside strings; fabric tolerates it, serde does not.
    let v: Value = serde_json::from_str(&s).or_else(|_| serde_json::from_str(&s.replace(['\n', '\r', '\t'], " "))).ok()?;
    let map = |k: &str| -> Vec<(String, Vec<String>)> {
        v[k].as_object().map(|o| o.iter().map(|(id, p)| (id.clone(), preds(p))).collect()).unwrap_or_default()
    };
    Some(ModInfo {
        id: v["id"].as_str()?.to_string(),
        version: v["version"].as_str().unwrap_or("0").to_string(),
        provides: v["provides"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()).unwrap_or_default(),
        depends: map("depends"),
        breaks: map("breaks"),
    })
}

// ---------------- Version predicates (subset of Fabric's semver syntax) ----------------

#[derive(PartialEq, PartialOrd, Debug, Clone)]
struct Ver {
    nums: Vec<u64>,
    pre: Option<String>,
}

fn parse_ver(s: &str) -> Ver {
    let s = s.trim().trim_start_matches('v');
    let core = s.split('+').next().unwrap_or(s);
    let (main, pre) = match core.split_once('-') {
        Some((m, p)) => (m, Some(p.to_string())),
        None => (core, None),
    };
    let nums = main
        .split('.')
        .map(|p| p.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(0))
        .collect();
    Ver { nums, pre }
}

fn cmp(a: &Ver, b: &Ver) -> std::cmp::Ordering {
    let n = a.nums.len().max(b.nums.len());
    for i in 0..n {
        let (x, y) = (a.nums.get(i).copied().unwrap_or(0), b.nums.get(i).copied().unwrap_or(0));
        if x != y {
            return x.cmp(&y);
        }
    }
    match (&a.pre, &b.pre) {
        (None, None) => std::cmp::Ordering::Equal,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (Some(_), None) => std::cmp::Ordering::Less,
        (Some(x), Some(y)) => x.cmp(y),
    }
}

fn match_one(ver: &Ver, pred: &str) -> bool {
    let p = pred.trim();
    if p.is_empty() || p == "*" {
        return true;
    }
    for (op, f) in [
        (">=", (|o: std::cmp::Ordering| o.is_ge()) as fn(std::cmp::Ordering) -> bool),
        ("<=", |o| o.is_le()),
        (">", |o| o.is_gt()),
        ("<", |o| o.is_lt()),
        ("=", |o| o.is_eq()),
    ] {
        if let Some(rest) = p.strip_prefix(op) {
            return f(cmp(ver, &parse_ver(rest)));
        }
    }
    if let Some(rest) = p.strip_prefix('^') {
        let base = parse_ver(rest);
        return cmp(ver, &base).is_ge() && ver.nums.first() == base.nums.first();
    }
    if let Some(rest) = p.strip_prefix('~') {
        let base = parse_ver(rest);
        return cmp(ver, &base).is_ge() && ver.nums.iter().take(2).eq(base.nums.iter().take(2));
    }
    if p.contains(".x") || p.contains(".X") || p.contains(".*") {
        let prefix: Vec<u64> = p.split('.').take_while(|s| !matches!(*s, "x" | "X" | "*")).filter_map(|s| s.parse().ok()).collect();
        return ver.nums.iter().take(prefix.len()).eq(prefix.iter());
    }
    cmp(ver, &parse_ver(p)).is_eq()
}

/// A predicate string may hold several space-separated comparisons that must all hold.
fn matches(version: &str, preds: &[String]) -> bool {
    let v = parse_ver(version);
    preds.iter().any(|p| p.split_whitespace().all(|part| match_one(&v, part)) || p.trim().is_empty())
}

// ---------------- Conflict detection ----------------

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conflict {
    pub declarer: String,
    pub target: String,
    pub kind: String,
    pub text: String,
}

struct Scan {
    mods: HashMap<String, (ModInfo, String)>, // id -> (info, file name)
}

fn scan(inst_id: &str) -> Scan {
    let dir = instances::game_dir(inst_id).join("mods");
    let mut mods = HashMap::new();
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".jar") {
            continue;
        }
        if let Some(info) = read_fabric_json(&e.path()) {
            for p in &info.provides {
                mods.entry(p.clone()).or_insert((info.clone(), name.clone()));
            }
            mods.insert(info.id.clone(), (info, name));
        }
    }
    Scan { mods }
}

fn conflicts_in(scan: &Scan) -> Vec<Conflict> {
    let mut out = vec![];
    let mut seen = std::collections::HashSet::new();
    for (id, (info, _)) in &scan.mods {
        if id != &info.id || !seen.insert(id.clone()) {
            continue; // skip `provides` aliases
        }
        for (target, p) in &info.breaks {
            if let Some((t, _)) = scan.mods.get(target) {
                if matches(&t.version, p) {
                    out.push(Conflict {
                        declarer: info.id.clone(),
                        target: target.clone(),
                        kind: "breaks".into(),
                        text: l!("{} {} nejde s {} {}", "{} {} doesn't work with {} {}", info.id, info.version, target, t.version),
                    });
                }
            }
        }
        for (target, p) in &info.depends {
            if let Some((t, _)) = scan.mods.get(target) {
                if !matches(&t.version, p) {
                    out.push(Conflict {
                        declarer: info.id.clone(),
                        target: target.clone(),
                        kind: "depends".into(),
                        text: l!("{} {} potřebuje {} {}, je tam {}", "{} {} needs {} {}, found {}", info.id, info.version, target, p.join(" / "), t.version),
                    });
                }
            }
        }
    }
    out
}

pub fn check(inst_id: &str) -> Vec<Conflict> {
    conflicts_in(&scan(inst_id))
}

/// Mods that cost the most FPS/CPU; light mode turns them off for normal play.
pub const HEAVY_MODS: &[&str] = &[
    "distanthorizons",
    "lambdynlights",
    "continuity",
    "entity_texture_features",
    "entity_model_features",
    "sound_physics_remastered",
    "physicsmod",
];

/// Enable/disable heavy mods. Only files disabled by light mode itself are re-enabled later,
/// so mods the player switched off by hand stay off.
pub fn apply_light_mode(inst_id: &str, on: bool) -> Result<Vec<String>> {
    let mods = instances::game_dir(inst_id).join("mods");
    let state = instances::game_dir(inst_id).join(".justice").join("light-mode.json");
    let mut disabled: Vec<String> = net::read_json(&state).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    let mut changed = vec![];
    if on {
        let s = scan(inst_id);
        for id in HEAVY_MODS {
            if let Some((info, file)) = s.mods.get(*id) {
                if info.id == *id && mods.join(file).exists() {
                    std::fs::rename(mods.join(file), mods.join(format!("{file}.disabled")))?;
                    disabled.push(file.clone());
                    changed.push(info.id.clone());
                }
            }
        }
        net::write_json(&state, &serde_json::to_value(&disabled)?)?;
    } else if !disabled.is_empty() {
        for file in &disabled {
            let off = mods.join(format!("{file}.disabled"));
            if off.exists() {
                std::fs::rename(&off, mods.join(file))?;
                changed.push(file.clone());
            }
        }
        let _ = std::fs::remove_file(&state);
    }
    Ok(changed)
}

// ---------------- Automatic fix ----------------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FixReport {
    pub fixed: Vec<String>,
    pub remaining: Vec<Conflict>,
}

/// Modrinth versions of a project for this instance, newest first.
async fn candidates(project: &str, inst: &instances::Instance) -> Vec<Value> {
    let loaders = if inst.loader == "quilt" { r#"["quilt","fabric"]"# } else { r#"["fabric"]"# };
    let url = format!(
        "https://api.modrinth.com/v2/project/{project}/version?game_versions={}&loaders={}",
        urlencoding::encode(&format!("[\"{}\"]", inst.mc_version)),
        urlencoding::encode(loaders)
    );
    let mut v = net::get_json(&url).await.ok().and_then(|v| v.as_array().cloned()).unwrap_or_default();
    v.sort_by(|a, b| b["date_published"].as_str().cmp(&a["date_published"].as_str()));
    v
}

async fn fetch_info(version: &Value) -> Option<ModInfo> {
    let files = version["files"].as_array()?;
    let f = files.iter().find(|f| f["primary"] == true).or(files.first())?;
    let path = settings::cache_dir().join("compat").join(f["filename"].as_str()?);
    let mut job = Job::new(f["url"].as_str()?, path.clone());
    job.sha1 = f["hashes"]["sha1"].as_str().map(String::from);
    net::download(&job).await.ok()?;
    read_fabric_json(&path)
}

fn satisfied(kind: &str, declarer: &ModInfo, target_id: &str, target_version: &str) -> bool {
    let list = if kind == "breaks" { &declarer.breaks } else { &declarer.depends };
    match list.iter().find(|(id, _)| id == target_id) {
        None => true,
        Some((_, p)) => {
            let m = matches(target_version, p);
            if kind == "breaks" { !m } else { m }
        }
    }
}

async fn published(version_id: &str) -> Option<String> {
    let v = net::get_json(&format!("https://api.modrinth.com/v2/version/{version_id}")).await.ok()?;
    v["date_published"].as_str().map(String::from)
}

/// Candidates reordered so versions released no later than `before` come first (newest first,
/// releases before betas). A mod is built against whatever its dependency looked like on its
/// release day, so that version is the one most likely to actually work.
fn aligned(cands: Vec<Value>, before: Option<&str>) -> Vec<Value> {
    let Some(before) = before else { return cands };
    let (mut fit, rest): (Vec<Value>, Vec<Value>) = cands.into_iter().partition(|v| v["date_published"].as_str().map(|d| d <= before).unwrap_or(false));
    fit.sort_by(|a, b| {
        let key = |v: &Value| (v["version_type"] == "release", v["date_published"].as_str().unwrap_or_default().to_string());
        key(b).cmp(&key(a))
    });
    fit.extend(rest);
    fit
}

fn tracked_of(inst_id: &str, file: &str) -> Option<content::Tracked> {
    content::load_meta(inst_id).get(&format!("mods/{file}")).filter(|t| t.source == "modrinth").cloned()
}

async fn install_pinned(inst_id: &str, project: &str, version_id: &str) -> Option<()> {
    content::install(inst_id, "modrinth", project, Kind::Mod, Some(version_id)).await.ok()?;
    content::set_pinned_project(inst_id, project, true).ok()
}

/// Try to replace one side of a conflict with a Modrinth version that resolves it.
async fn fix_one(inst: &instances::Instance, c: &Conflict, s: &Scan) -> Option<String> {
    let (decl, decl_file) = s.mods.get(&c.declarer)?.clone();
    let (target, target_file) = s.mods.get(&c.target)?.clone();
    let target_date = match tracked_of(&inst.id, &target_file) {
        Some(t) => published(&t.version_id).await,
        None => None,
    };

    // 1) Swap the mod that declares the constraint (e.g. a Sodium that doesn't break this Iris).
    if let Some(project) = tracked_of(&inst.id, &decl_file).map(|t| t.project_id) {
        for v in aligned(candidates(&project, inst).await, target_date.as_deref()).into_iter().take(12) {
            let Some(info) = fetch_info(&v).await else { continue };
            if info.version == decl.version {
                continue;
            }
            if satisfied(&c.kind, &info, &c.target, &target.version) {
                install_pinned(&inst.id, &project, v["id"].as_str()?).await?;
                return Some(format!("{}: {} → {}", decl.id, decl.version, info.version));
            }
        }
    }
    // 2) Otherwise swap the target (e.g. a newer Iris that this Sodium accepts).
    if let Some(project) = tracked_of(&inst.id, &target_file).map(|t| t.project_id) {
        for v in candidates(&project, inst).await.into_iter().take(12) {
            let Some(info) = fetch_info(&v).await else { continue };
            if info.version == target.version {
                continue;
            }
            if satisfied(&c.kind, &decl, &c.target, &info.version) {
                install_pinned(&inst.id, &project, v["id"].as_str()?).await?;
                return Some(format!("{}: {} → {}", target.id, target.version, info.version));
            }
        }
    }
    None
}

/// Put `target` on the version that was current when `lagging` was released
/// (e.g. the Sodium an Iris build was compiled against).
pub async fn align(inst_id: &str, lagging: &str, target: &str) -> Result<Option<String>> {
    let inst = instances::load(inst_id)?;
    let s = scan(inst_id);
    let (Some((lag, lag_file)), Some((tgt, tgt_file))) = (s.mods.get(lagging).cloned(), s.mods.get(target).cloned()) else {
        return Ok(None);
    };
    let (Some(lag_t), Some(tgt_t)) = (tracked_of(inst_id, &lag_file), tracked_of(inst_id, &tgt_file)) else {
        return Ok(None);
    };
    let Some(lag_date) = published(&lag_t.version_id).await else { return Ok(None) };
    let cands = aligned(candidates(&tgt_t.project_id, &inst).await, Some(&lag_date));
    let Some(pick) = cands.into_iter().find(|v| v["date_published"].as_str().map(|d| d <= lag_date.as_str()).unwrap_or(false)) else {
        return Ok(None);
    };
    let Some(id) = pick["id"].as_str() else { return Ok(None) };
    if id == tgt_t.version_id {
        return Ok(None);
    }
    if install_pinned(inst_id, &tgt_t.project_id, id).await.is_none() {
        return Ok(None);
    }
    let new_version = scan(inst_id).mods.get(target).map(|(i, _)| i.version.clone()).unwrap_or_default();
    Ok(Some(l!("{}: {} → {} (sladěno s {} {})", "{}: {} → {} (matched to {} {})", tgt.id, tgt.version, new_version, lag.id, lag.version)))
}

/// Find the jar that contains `class` (e.g. `net.caffeinemc.mods.sodium.client.X`) and return its mod id.
fn mod_owning_class(inst_id: &str, class: &str) -> Option<String> {
    let entry = format!("{}.class", class.split('$').next()?.replace('.', "/"));
    let dir = instances::game_dir(inst_id).join("mods");
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let Ok(mut z) = zip::ZipArchive::new(match std::fs::File::open(e.path()) { Ok(f) => f, Err(_) => continue }) else { continue };
        if z.by_name(&entry).is_ok() {
            return read_fabric_json(&e.path()).map(|i| i.id);
        }
    }
    None
}

/// After a crash: find "Mixin apply for mod X failed ... -> some.Class", align X's target mod, then fix metadata conflicts.
pub async fn repair_from_log(inst_id: &str, log: &str) -> Result<FixReport> {
    let re = regex::Regex::new(r"Mixin apply for mod ([a-z0-9_\-]+) failed .*? -> ([A-Za-z0-9_.$]+)").unwrap();
    let mut fixed = vec![];
    let mut seen = std::collections::HashSet::new();
    for c in re.captures_iter(log) {
        let (lagging, class) = (c[1].to_string(), c[2].to_string());
        let Some(target) = mod_owning_class(inst_id, &class) else { continue };
        if target == lagging || !seen.insert((lagging.clone(), target.clone())) {
            continue;
        }
        if let Some(msg) = align(inst_id, &lagging, &target).await? {
            fixed.push(msg);
        }
    }
    let mut r = fix(inst_id).await?;
    fixed.extend(r.fixed.drain(..));
    r.fixed = fixed;
    Ok(r)
}

pub async fn fix(inst_id: &str) -> Result<FixReport> {
    let inst = instances::load(inst_id)?;
    let mut report = FixReport::default();
    if !matches!(inst.loader.as_str(), "fabric" | "quilt") {
        return Ok(report);
    }
    let mut unfixable: Vec<Conflict> = vec![];
    for _ in 0..6 {
        let s = scan(inst_id);
        let pending: Vec<Conflict> = conflicts_in(&s)
            .into_iter()
            .filter(|c| !unfixable.iter().any(|u| u.declarer == c.declarer && u.target == c.target))
            .collect();
        let Some(c) = pending.first().cloned() else { break };
        crate::events::progress(inst_id, &l!("Opravuji kompatibilitu modů", "Fixing mod compatibility"), 0, 1);
        match fix_one(&inst, &c, &s).await {
            Some(msg) => report.fixed.push(msg),
            None => unfixable.push(c),
        }
    }
    report.remaining = check(inst_id);
    Ok(report)
}

// ---------------- Repair wizard ----------------
// One guided pass over an instance: duplicates, missing dependencies, metadata conflicts and the
// last crash log. It shows a preview (RepairPlan) the user ticks, applies the chosen fixes, and
// keeps a full snapshot of the mods folder so the whole thing can be undone in one click.

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RepairAction {
    pub id: String,
    /// "duplicate" | "missing" | "conflict" | "crash"
    pub group: String,
    pub title: String,
    pub detail: String,
    /// "error" | "warn"
    pub severity: String,
    pub checked: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RepairPlan {
    pub actions: Vec<RepairAction>,
    pub crash: Option<crate::crash::CrashReport>,
    pub undo_available: bool,
    /// Repair only touches Fabric/Quilt instances.
    pub supported: bool,
}

/// Mod ids provided by the loader/game themselves — never "missing".
const BUILTIN_IDS: &[&str] = &["minecraft", "java", "fabricloader", "fabric-loader", "quilt_loader", "quilted_fabric_api", "mixinextras"];

/// Common dependency id → Modrinth slug, so "install the missing dep" lands the right project.
fn dep_slug(id: &str) -> &str {
    match id {
        "fabric" | "fabric-api" | "fabric_api" => "fabric-api",
        "cloth-config" | "cloth-config2" | "cloth_config" => "cloth-config",
        "architectury" => "architectury-api",
        "fabric-language-kotlin" | "fabric_language_kotlin" => "fabric-language-kotlin",
        "yet_another_config_lib_v3" | "yacl" => "yacl",
        "midnightlib" => "midnightlib",
        "owo" => "owo-lib",
        "geckolib" => "geckolib",
        "trinkets" => "trinkets",
        other => other,
    }
}

fn repair_backups_dir(id: &str) -> PathBuf {
    instances::dir(id).join("repair-backups")
}

fn latest_repair_backup(id: &str) -> Option<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(repair_backups_dir(id)).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    v.sort();
    v.pop()
}

/// Every mod jar with its metadata (duplicates included, unlike `scan`).
fn all_mods(inst_id: &str) -> Vec<(ModInfo, String)> {
    let dir = instances::game_dir(inst_id).join("mods");
    let mut out = vec![];
    for e in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.ends_with(".jar") {
            if let Some(info) = read_fabric_json(&e.path()) {
                out.push((info, name));
            }
        }
    }
    out
}

/// Tail of the instance's latest log (for crash analysis without the game running).
fn recent_log(inst_id: &str) -> Vec<String> {
    let p = instances::game_dir(inst_id).join("logs").join("latest.log");
    std::fs::read_to_string(&p)
        .map(|t| { let lines: Vec<&str> = t.lines().collect(); lines.iter().rev().take(500).rev().map(|s| s.to_string()).collect() })
        .unwrap_or_default()
}

pub fn repair_plan(inst_id: &str) -> Result<RepairPlan> {
    let inst = instances::load(inst_id)?;
    let supported = matches!(inst.loader.as_str(), "fabric" | "quilt");
    let mut actions = vec![];

    if supported {
        let mods = all_mods(inst_id);

        // Duplicates: the same mod id in more than one jar. Keep the newest, offer to disable the rest.
        let mut by_id: HashMap<String, Vec<(String, String)>> = HashMap::new(); // id -> [(version, file)]
        for (info, file) in &mods {
            by_id.entry(info.id.clone()).or_default().push((info.version.clone(), file.clone()));
        }
        for (id, mut versions) in by_id {
            if versions.len() < 2 {
                continue;
            }
            versions.sort_by(|a, b| cmp(&parse_ver(&a.0), &parse_ver(&b.0)));
            let newest = versions.pop().unwrap();
            for (ver, file) in versions {
                actions.push(RepairAction {
                    id: format!("disable:{file}"),
                    group: "duplicate".into(),
                    title: l!("Vypnout starší {} {}", "Disable older {} {}", id, ver),
                    detail: l!("{} je tam dvakrát; nechávám {} a vypínám tuhle starší verzi.", "{} is installed twice; keeping {} and disabling this older copy.", id, newest.0),
                    severity: "error".into(),
                    checked: true,
                });
            }
        }

        // Missing dependencies: a depend on an id nothing provides.
        let mut present = std::collections::HashSet::new();
        for (info, _) in &mods {
            present.insert(info.id.clone());
            for p in &info.provides {
                present.insert(p.clone());
            }
        }
        let mut wanted = std::collections::HashSet::new();
        for (info, _) in &mods {
            for (dep, _) in &info.depends {
                if present.contains(dep) || BUILTIN_IDS.contains(&dep.as_str()) || !wanted.insert(dep.clone()) {
                    continue;
                }
                let slug = dep_slug(dep);
                actions.push(RepairAction {
                    id: format!("install:{slug}"),
                    group: "missing".into(),
                    title: l!("Doinstalovat {}", "Install {}", dep),
                    detail: l!("Potřebuje ho {} – stáhnu ho z Modrinthu pro tvou verzi.", "{} needs it – I'll grab it from Modrinth for your version.", info.id),
                    severity: "error".into(),
                    checked: true,
                });
            }
        }

        // Metadata conflicts (version mismatches between two installed mods).
        let conflicts = check(inst_id);
        if !conflicts.is_empty() {
            actions.push(RepairAction {
                id: "fix-conflicts".into(),
                group: "conflict".into(),
                title: l!("Vyřešit konflikty verzí ({})", "Resolve version conflicts ({})", conflicts.len()),
                detail: conflicts.iter().map(|c| c.text.clone()).collect::<Vec<_>>().join("\n"),
                severity: "warn".into(),
                checked: true,
            });
        }
    }

    // Last crash: show the explanation, and offer a mixin-conflict repair when the log points at one.
    let log = recent_log(inst_id);
    let game = instances::game_dir(inst_id);
    let text = log.join("\n");
    let crashed = game.join("crash-reports").is_dir() && std::fs::read_dir(game.join("crash-reports")).map(|mut d| d.next().is_some()).unwrap_or(false);
    let mixin_crash = text.contains("Mixin apply for mod");
    let crash = if crashed || mixin_crash || text.contains("Exception in thread \"main\"") {
        Some(crate::crash::analyze(&log, &game))
    } else {
        None
    };
    if supported && mixin_crash {
        actions.push(RepairAction {
            id: "repair-crash".into(),
            group: "crash".into(),
            title: l!("Opravit konflikt z posledního pádu", "Fix the conflict from the last crash"),
            detail: l!("Log ukazuje selhání mixinu; sladím verze modů, které se perou.", "The log shows a mixin failure; I'll match the versions of the mods that clash."),
            severity: "error".into(),
            checked: true,
        });
    }

    Ok(RepairPlan { actions, crash, undo_available: latest_repair_backup(inst_id).is_some(), supported })
}

/// Snapshot every mod jar so a repair can be undone whole, then run the chosen actions.
pub async fn repair_apply(inst_id: &str, ids: Vec<String>) -> Result<FixReport> {
    let game = instances::game_dir(inst_id);
    let mods = game.join("mods");
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let bdir = repair_backups_dir(inst_id).join(&stamp).join("mods");
    std::fs::create_dir_all(&bdir)?;
    for e in std::fs::read_dir(&mods).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if (name.ends_with(".jar") || name.ends_with(".jar.disabled")) && e.path().is_file() {
            std::fs::copy(e.path(), bdir.join(&name))?;
        }
    }

    let mut report = FixReport::default();

    for id in &ids {
        if let Some(file) = id.strip_prefix("disable:") {
            let from = mods.join(file);
            if from.exists() {
                std::fs::rename(&from, mods.join(format!("{file}.disabled")))?;
                report.fixed.push(l!("Vypnuto: {}", "Disabled: {}", file));
            }
        }
    }

    let slugs: Vec<&str> = ids.iter().filter_map(|i| i.strip_prefix("install:")).collect();
    if !slugs.is_empty() {
        crate::events::progress(inst_id, &l!("Stahuji chybějící mody", "Downloading missing mods"), 0, 1);
        let r = content::install_many(inst_id, &slugs, Kind::Mod).await;
        for s in r.installed {
            report.fixed.push(l!("Nainstalováno: {}", "Installed: {}", s));
        }
    }

    if ids.iter().any(|i| i == "fix-conflicts") {
        let mut r = fix(inst_id).await?;
        report.fixed.append(&mut r.fixed);
    }

    if ids.iter().any(|i| i == "repair-crash") {
        let log = recent_log(inst_id);
        let mut r = repair_from_log(inst_id, &log.join("\n")).await?;
        report.fixed.append(&mut r.fixed);
    }

    crate::events::progress_done(inst_id);
    report.remaining = check(inst_id);
    Ok(report)
}

/// Put the mods folder back to the snapshot the last repair took.
pub fn repair_undo(inst_id: &str) -> Result<()> {
    let Some(bdir) = latest_repair_backup(inst_id) else { anyhow::bail!(l!("Není co vrátit", "Nothing to undo")) };
    let snap = bdir.join("mods");
    let mods = instances::game_dir(inst_id).join("mods");
    for e in std::fs::read_dir(&mods).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.ends_with(".jar") || name.ends_with(".jar.disabled") {
            let _ = std::fs::remove_file(e.path());
        }
    }
    for e in std::fs::read_dir(&snap).into_iter().flatten().flatten() {
        std::fs::copy(e.path(), mods.join(e.file_name()))?;
    }
    content::forget_missing(inst_id);
    std::fs::remove_dir_all(&bdir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn predicates() {
        assert!(matches("1.10.7+mc1.21.11", &["<=1.10.7".into()]));
        assert!(!matches("1.10.8+mc1.21.11", &["<=1.10.7".into()]));
        assert!(matches("0.8.13+mc1.21.11", &["0.8.x".into()]));
        assert!(!matches("0.9.0", &["0.8.x".into()]));
        assert!(matches("0.6.2", &[">=0.6.0 <0.7".into()]));
        assert!(matches("1.2.5", &["~1.2".into()]));
        assert!(matches("2.0.0-beta.1", &["*".into()]));
        assert!(!matches("0.5", &[">=0.6.0".into()]));
    }
}
