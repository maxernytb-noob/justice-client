//! Share codes, Justice pack export, and importing .mrpack / CurseForge / .dawnpack / .justice packs.

use crate::backups;
use crate::content::{self, Kind};
use crate::downloads;
use crate::events;
use crate::instances::{self, Instance};
use crate::net::{self, Job};
use crate::settings;
use anyhow::{anyhow, bail, Context, Result};
use base64::Engine;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const CODE_PREFIX: &str = "JUSTICE1:";

fn b64() -> base64::engine::GeneralPurpose {
    base64::engine::general_purpose::URL_SAFE_NO_PAD
}

/// Compact description of an instance: versions + tracked content (no files).
fn describe(inst: &Instance) -> Value {
    let items: Vec<Value> = content::load_meta(&inst.id)
        .values()
        .map(|t| json!([if t.source == "curseforge" { 1 } else { 0 }, t.project_id, t.version_id, t.kind]))
        .collect();
    json!({"n": inst.name, "v": inst.mc_version, "l": inst.loader, "lv": inst.loader_version, "r": inst.recording_mode, "c": items})
}

pub fn share_code(inst_id: &str) -> Result<String> {
    let inst = instances::load(inst_id)?;
    let mut enc = DeflateEncoder::new(vec![], flate2::Compression::best());
    enc.write_all(describe(&inst).to_string().as_bytes())?;
    Ok(format!("{CODE_PREFIX}{}", b64().encode(enc.finish()?)))
}

fn decode_code(code: &str) -> Result<Value> {
    let data = code.trim().strip_prefix(CODE_PREFIX).ok_or_else(|| anyhow!(l!("Tohle není Justice kód", "That isn't a Justice code")))?;
    let bytes = b64().decode(data.trim()).map_err(|_| anyhow!(l!("Kód je poškozený", "The code is corrupted")))?;
    let mut s = String::new();
    DeflateDecoder::new(&bytes[..]).read_to_string(&mut s)?;
    Ok(serde_json::from_str(&s)?)
}

async fn create_from_description(d: &Value, name_override: Option<&str>) -> Result<Instance> {
    let inst = instances::create(Instance {
        name: name_override.unwrap_or(d["n"].as_str().unwrap_or("Shared instance")).to_string(),
        mc_version: d["v"].as_str().ok_or_else(|| anyhow!(l!("chybí verze", "missing version")))?.to_string(),
        loader: d["l"].as_str().unwrap_or("vanilla").to_string(),
        loader_version: d["lv"].as_str().unwrap_or_default().to_string(),
        recording_mode: d["r"].as_bool().unwrap_or(false),
        icon: "share".into(),
        ..Default::default()
    })?;
    let items = d["c"].as_array().cloned().unwrap_or_default();
    let kind = |it: &Value| Kind::parse(it[3].as_str().unwrap_or("mod"));
    // Modrinth items with an exact version go in bulk (fast); the rest one by one.
    let (bulk, rest): (Vec<&Value>, Vec<&Value>) = items.iter().partition(|it| it[0] != 1 && it[2].as_str().is_some_and(|s| !s.is_empty()));
    let pinned: Vec<(String, Kind)> = bulk.iter().map(|it| (it[2].as_str().unwrap_or_default().to_string(), kind(it))).collect();
    let mut slow: Vec<(&str, String, Option<String>, Kind)> = rest
        .iter()
        .map(|it| (if it[0] == 1 { "curseforge" } else { "modrinth" }, it[1].as_str().unwrap_or_default().to_string(), it[2].as_str().filter(|s| !s.is_empty()).map(String::from), kind(it)))
        .collect();
    match content::install_mr_versions(&inst.id, &pinned).await {
        Ok((_, deps, leftover)) => {
            // A pinned version that failed in bulk is retried normally, by its project.
            let by_version: std::collections::HashMap<&str, &str> = bulk.iter().filter_map(|it| Some((it[2].as_str()?, it[1].as_str()?))).collect();
            slow.extend(leftover.into_iter().filter_map(|(v, k)| Some(("modrinth", by_version.get(v.as_str())?.to_string(), Some(v), k))));
            slow.extend(deps.into_iter().map(|p| ("modrinth", p, None, Kind::Mod)));
        }
        Err(e) => {
            lwarn!("modpack: bulk install failed, going one by one: {e:#}");
            slow.extend(bulk.iter().map(|it| ("modrinth", it[1].as_str().unwrap_or_default().to_string(), it[2].as_str().map(String::from), kind(it))));
        }
    }
    let total = slow.len();
    for (i, (source, pid, vid, k)) in slow.iter().enumerate() {
        events::progress(&inst.id, &l!("Instaluji obsah", "Installing content"), i, total);
        let _ = content::install(&inst.id, source, pid, *k, vid.as_deref()).await;
    }
    events::progress_done(&inst.id);
    Ok(inst)
}

pub async fn import_code(code: &str) -> Result<Instance> {
    let d = decode_code(code)?;
    create_from_description(&d, None).await
}

/// Zip with justice.json + configs + untracked files so friends get exactly the same setup.
pub fn export_pack(inst_id: &str, dest: &str) -> Result<()> {
    let inst = instances::load(inst_id)?;
    let g = instances::game_dir(inst_id);
    let tmp = settings::cache_dir().join(format!("export-{inst_id}"));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp)?;
    std::fs::write(tmp.join("justice.json"), describe(&inst).to_string())?;

    let tracked: std::collections::HashSet<String> = content::load_meta(inst_id).keys().cloned().collect();
    let mut entries: Vec<(PathBuf, String)> = vec![(tmp.join("justice.json"), "justice.json".into())];
    for (p, name) in [
        (g.join("config"), "overrides/config"),
        (g.join("options.txt"), "overrides/options.txt"),
        (g.join("servers.dat"), "overrides/servers.dat"),
    ] {
        entries.push((p, name.into()));
    }
    for folder in ["mods", "resourcepacks", "shaderpacks"] {
        for e in std::fs::read_dir(g.join(folder)).into_iter().flatten().flatten() {
            let f = e.file_name().to_string_lossy().into_owned();
            if !tracked.contains(&format!("{folder}/{}", f.trim_end_matches(".disabled"))) {
                entries.push((e.path(), format!("overrides/{folder}/{f}")));
            }
        }
    }
    backups::zip_paths(&entries, Path::new(dest))?;
    let _ = std::fs::remove_dir_all(&tmp);
    Ok(())
}

/// A modpack from Modrinth or CurseForge becomes a new instance with the pack's own version and loader.
pub async fn install_modpack(source: &str, project: &str, version: Option<&str>) -> Result<Instance> {
    let (url, name) = content::modpack_file(source, project, version).await?;
    let name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let path = settings::cache_dir().join("modpack-download").join(name.trim_start_matches('.'));
    net::download(&Job::new(url, path.clone())).await.context(l!("Stažení modpacku selhalo", "Downloading the modpack failed"))?;
    let r = import_file(&path.to_string_lossy()).await;
    let _ = std::fs::remove_file(&path);
    r
}

/// Same as `install_modpack`, but the file downloads report to the download manager (byte progress,
/// pause/cancel) via `h`. The pack zip itself is small, so it's fetched untracked.
pub async fn install_modpack_tracked(source: &str, project: &str, version: Option<&str>, h: &downloads::Handle) -> Result<Instance> {
    let (url, name) = content::modpack_file(source, project, version).await?;
    let name: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') { c } else { '_' }).collect();
    let path = settings::cache_dir().join("modpack-download").join(name.trim_start_matches('.'));
    net::download(&Job::new(url, path.clone())).await.context(l!("Stažení modpacku selhalo", "Downloading the modpack failed"))?;
    let r = import_file_h(&path.to_string_lossy(), Some(h)).await;
    let _ = std::fs::remove_file(&path);
    r
}

fn copy_overrides(from: &Path, inst_id: &str) -> Result<()> {
    if from.is_dir() {
        instances::copy_dir(from, &instances::game_dir(inst_id))?;
    }
    Ok(())
}

/// Run a batch of file downloads, either tracked by the download manager (byte progress, pause/cancel)
/// or, with no handle, reported on the instance's own progress bar as before.
async fn run_jobs(jobs: Vec<Job>, inst_id: &str, h: Option<&downloads::Handle>) -> Result<()> {
    match h {
        Some(h) => net::download_all_tracked(jobs, 16, h.done.clone(), h.total.clone(), h.control.clone()).await,
        None => {
            let id = inst_id.to_string();
            let label = l!("Stahuji modpack", "Downloading modpack");
            net::download_all(jobs, 16, move |d, n| events::progress(&id, &label, d, n)).await
        }
    }
}

pub async fn import_file(path: &str) -> Result<Instance> {
    import_file_h(path, None).await
}

async fn import_file_h(path: &str, h: Option<&downloads::Handle>) -> Result<Instance> {
    let p = PathBuf::from(path);
    let tmp = settings::cache_dir().join("import");
    let _ = std::fs::remove_dir_all(&tmp);
    backups::unzip_into(&p, &tmp).context(l!("Soubor není platný modpack (zip)", "The file isn't a valid modpack (zip)"))?;
    let stem = p.file_stem().unwrap_or_default().to_string_lossy().into_owned();

    let result = if tmp.join("justice.json").exists() {
        let d = net::read_json(&tmp.join("justice.json"))?;
        let inst = create_from_description(&d, None).await?;
        copy_overrides(&tmp.join("overrides"), &inst.id)?;
        Ok(inst)
    } else if tmp.join("modrinth.index.json").exists() {
        import_mrpack(&tmp, &stem, h).await
    } else if tmp.join("dawnpack.json").exists() {
        import_dawnpack(&tmp, &stem, h).await
    } else if tmp.join("manifest.json").exists() {
        import_curseforge(&tmp, &stem).await
    } else {
        Err(anyhow!(l!("Neznámý formát — podporuji .mrpack, CurseForge .zip, Dawn .dawnpack a Justice export", "Unknown format — supported are .mrpack, CurseForge .zip, Dawn .dawnpack and Justice exports")))
    };
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

async fn import_mrpack(dir: &Path, fallback_name: &str, h: Option<&downloads::Handle>) -> Result<Instance> {
    let idx = net::read_json(&dir.join("modrinth.index.json"))?;
    let deps = &idx["dependencies"];
    let (loader, lv) = [("fabric-loader", "fabric"), ("quilt-loader", "quilt"), ("neoforge", "neoforge"), ("forge", "forge")]
        .iter()
        .find_map(|(k, l)| deps[*k].as_str().map(|v| (l.to_string(), v.to_string())))
        .unwrap_or(("vanilla".into(), String::new()));
    let mc = deps["minecraft"].as_str().ok_or_else(|| anyhow!("mrpack bez verze Minecraftu"))?;
    let lv = if loader == "forge" && !lv.starts_with(mc) { format!("{mc}-{lv}") } else { lv };
    let inst = instances::create(Instance {
        name: idx["name"].as_str().unwrap_or(fallback_name).to_string(),
        mc_version: mc.into(),
        loader,
        loader_version: lv,
        icon: "modpack".into(),
        ..Default::default()
    })?;
    let game = instances::game_dir(&inst.id);
    let tracked: Vec<(String, String)> = idx["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|f| Some((f["path"].as_str()?.to_string(), f["hashes"]["sha1"].as_str()?.to_string())))
        .collect();
    let jobs: Vec<Job> = idx["files"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|f| f["env"]["client"] != "unsupported")
        .filter_map(|f| {
            let rel = f["path"].as_str()?;
            if rel.contains("..") || Path::new(rel).is_absolute() {
                return None;
            }
            let mut j = Job::new(f["downloads"][0].as_str()?, game.join(rel));
            j.sha1 = f["hashes"]["sha1"].as_str().map(String::from);
            j.size = f["fileSize"].as_u64();
            Some(j)
        })
        .collect();
    run_jobs(jobs, &inst.id, h).await?;
    if let Err(e) = content::track_by_hashes(&inst.id, &tracked).await {
        lwarn!("mrpack: couldn't look the files up on Modrinth: {e:#}");
    }
    copy_overrides(&dir.join("overrides"), &inst.id)?;
    copy_overrides(&dir.join("client-overrides"), &inst.id)?;
    events::progress_done(&inst.id);
    Ok(inst)
}

/// Dawn Client `.dawnpack`: zip with `dawnpack.json` (profile + Modrinth-referenced content) and `overrides/`.
async fn import_dawnpack(dir: &Path, fallback_name: &str, h: Option<&downloads::Handle>) -> Result<Instance> {
    let d = net::read_json(&dir.join("dawnpack.json"))?;
    let p = &d["profile"];
    let mc = p["minecraftVersion"].as_str().ok_or_else(|| anyhow!(l!("dawnpack bez verze Minecraftu", "dawnpack without a Minecraft version")))?;
    let loader = p["loader"]["kind"].as_str().unwrap_or("Vanilla").to_lowercase();
    let lv = p["loader"]["version"].as_str().unwrap_or("").to_string();
    let lv = if loader == "forge" && !lv.is_empty() && !lv.starts_with(mc) { format!("{mc}-{lv}") } else { lv };
    let jvm = p["settings"]["jvmOptions"]["extraArguments"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect::<Vec<_>>().join(" "))
        .unwrap_or_default();
    let inst = instances::create(Instance {
        name: p["name"].as_str().unwrap_or(fallback_name).to_string(),
        mc_version: mc.into(),
        loader,
        loader_version: lv,
        jvm_args: jvm,
        icon: "modpack".into(),
        ..Default::default()
    })?;
    let game = instances::game_dir(&inst.id);

    // Every reference names a Modrinth version; resolve them in batches to download URLs.
    let refs: Vec<Value> = d["contentReferences"].as_array().cloned().unwrap_or_default();
    let version_of = |r: &Value| -> Option<String> {
        r["providerCandidates"].as_array()?.iter().find(|c| c["provider"] == "Modrinth")?["versionId"].as_str().map(String::from)
    };
    let ids: Vec<String> = refs.iter().filter_map(version_of).collect();
    let mut versions = std::collections::HashMap::new();
    for chunk in ids.chunks(80) {
        let list = chunk.iter().map(|i| format!("%22{i}%22")).collect::<Vec<_>>().join("%2C");
        let url = format!("https://api.modrinth.com/v2/versions?ids=%5B{list}%5D");
        if let Ok(Value::Array(vs)) = net::get_json(&url).await {
            for v in vs {
                if let Some(id) = v["id"].as_str() {
                    versions.insert(id.to_string(), v);
                }
            }
        }
    }

    let mut jobs = Vec::new();
    let mut missing = Vec::new();
    for r in &refs {
        let folder = match r["type"].as_str().unwrap_or("Mod") {
            "Mod" => "mods",
            "ResourcePack" => "resourcepacks",
            "ShaderPack" | "Shader" | "Shaders" => "shaderpacks",
            _ => continue,
        };
        let name = r["fileName"].as_str().unwrap_or_default();
        if name.is_empty() || name.contains("..") || name.contains('/') || name.contains('\\') {
            continue;
        }
        let sha1 = r["hashes"]["sha1"].as_str();
        let file = version_of(r).and_then(|vid| versions.get(&vid)).and_then(|v| {
            let files = v["files"].as_array()?;
            files
                .iter()
                .find(|f| sha1.is_some() && f["hashes"]["sha1"].as_str() == sha1)
                .or_else(|| files.iter().find(|f| f["filename"] == name))
                .or_else(|| files.iter().find(|f| f["primary"] == true))
                .cloned()
        });
        let Some(file) = file else {
            missing.push(name.to_string());
            continue;
        };
        let Some(url) = file["url"].as_str() else { continue };
        let disabled = r["enabled"] == false;
        let target = game.join(folder).join(if disabled { format!("{name}.disabled") } else { name.to_string() });
        let mut j = Job::new(url, target);
        j.sha1 = file["hashes"]["sha1"].as_str().map(String::from);
        j.size = file["size"].as_u64();
        jobs.push(j);
    }
    run_jobs(jobs, &inst.id, h).await?;
    copy_overrides(&dir.join("overrides"), &inst.id)?;
    events::progress_done(&inst.id);
    if !missing.is_empty() {
        eprintln!("dawnpack: not found on Modrinth: {}", missing.join(", "));
    }
    Ok(inst)
}

async fn import_curseforge(dir: &Path, fallback_name: &str) -> Result<Instance> {
    let m = net::read_json(&dir.join("manifest.json"))?;
    let mc = m["minecraft"]["version"].as_str().ok_or_else(|| anyhow!("manifest bez verze"))?;
    let primary = m["minecraft"]["modLoaders"]
        .as_array()
        .and_then(|a| a.iter().find(|l| l["primary"] == true).or(a.first()))
        .and_then(|l| l["id"].as_str())
        .unwrap_or("");
    let (loader, lv) = match primary.split_once('-') {
        Some(("forge", v)) => ("forge".to_string(), format!("{mc}-{v}")),
        Some((l, v)) => (l.to_string(), v.to_string()),
        None => ("vanilla".to_string(), String::new()),
    };
    if !content::has_cf_key() {
        bail!(l!("Na import CurseForge modpacku potřebuješ CurseForge API klíč (Nastavení).", "Importing a CurseForge modpack needs a CurseForge API key (Settings)."));
    }
    let inst = instances::create(Instance {
        name: m["name"].as_str().unwrap_or(fallback_name).to_string(),
        mc_version: mc.into(),
        loader,
        loader_version: lv,
        icon: "modpack".into(),
        ..Default::default()
    })?;
    let files = m["files"].as_array().cloned().unwrap_or_default();
    let total = files.len();
    for (i, f) in files.iter().enumerate() {
        events::progress(&inst.id, &l!("Stahuji modpack", "Downloading modpack"), i, total);
        let (Some(pid), Some(fid)) = (f["projectID"].as_u64(), f["fileID"].as_u64()) else { continue };
        let _ = content::install(&inst.id, "curseforge", &pid.to_string(), Kind::Mod, Some(&fid.to_string())).await;
    }
    let ov = m["overrides"].as_str().unwrap_or("overrides");
    copy_overrides(&dir.join(ov), &inst.id)?;
    events::progress_done(&inst.id);
    Ok(inst)
}
