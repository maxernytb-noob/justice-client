//! Vanilla version metadata: manifest, version JSON merging, libraries, assets.

use crate::events;
use crate::net::{self, Job};
use crate::settings;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

const MANIFEST_URL: &str = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json";
const LIBRARIES_URL: &str = "https://libraries.minecraft.net/";
const RESOURCES_URL: &str = "https://resources.download.minecraft.net/";
const FORGE_MAVEN: &str = "https://maven.minecraftforge.net/";
const MAVEN_CENTRAL: &str = "https://repo1.maven.org/maven2/";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct GameVersion {
    pub id: String,
    pub kind: String,
    pub release_time: String,
}

pub async fn manifest() -> Result<Value> {
    let cache = settings::cache_dir().join("version_manifest_v2.json");
    net::get_json_cached(MANIFEST_URL, &cache, 60 * 30).await
}

pub async fn game_versions() -> Result<Vec<GameVersion>> {
    let m = manifest().await?;
    Ok(m["versions"]
        .as_array()
        .ok_or_else(|| anyhow!("bad manifest"))?
        .iter()
        .map(|v| GameVersion {
            id: v["id"].as_str().unwrap_or_default().into(),
            kind: v["type"].as_str().unwrap_or_default().into(),
            release_time: v["releaseTime"].as_str().unwrap_or_default().into(),
        })
        .collect())
}

pub fn version_json_path(id: &str) -> PathBuf {
    settings::meta_dir().join("versions").join(id).join(format!("{id}.json"))
}

/// Make sure the vanilla version JSON for `id` is on disk.
pub async fn ensure_vanilla_json(id: &str) -> Result<()> {
    let path = version_json_path(id);
    if path.exists() {
        return Ok(());
    }
    let m = manifest().await?;
    let entry = m["versions"]
        .as_array()
        .and_then(|a| a.iter().find(|v| v["id"] == id))
        .ok_or_else(|| anyhow!(l!("Verze {id} neexistuje", "Version {id} doesn't exist")))?;
    let mut job = Job::new(entry["url"].as_str().unwrap_or_default(), path);
    job.sha1 = entry["sha1"].as_str().map(String::from);
    net::download(&job).await
}

/// Load a version JSON and merge it with its `inheritsFrom` parents.
pub async fn load_merged(id: &str) -> Result<Value> {
    let mut chain = vec![];
    let mut cur = id.to_string();
    loop {
        let p = version_json_path(&cur);
        if !p.exists() {
            ensure_vanilla_json(&cur).await?;
        }
        let v = net::read_json(&p)?;
        let parent = v["inheritsFrom"].as_str().map(String::from);
        chain.push(v);
        match parent {
            Some(p) if chain.len() < 8 => cur = p,
            _ => break,
        }
    }
    let mut merged = chain.pop().unwrap();
    while let Some(child) = chain.pop() {
        merged = merge(merged, child);
    }
    Ok(merged)
}

fn lib_key(name: &str) -> String {
    // group:artifact[:classifier] — the version is what a child is allowed to override.
    let parts: Vec<&str> = name.split(':').collect();
    match parts.len() {
        0..=2 => name.to_string(),
        3 => format!("{}:{}", parts[0], parts[1]),
        _ => format!("{}:{}:{}", parts[0], parts[1], parts[3]),
    }
}

fn merge(parent: Value, child: Value) -> Value {
    let mut out = parent.as_object().cloned().unwrap_or_default();
    let child = child.as_object().cloned().unwrap_or_default();

    let mut libs: Vec<Value> = vec![];
    let mut seen = HashSet::new();
    for l in child.get("libraries").and_then(|v| v.as_array()).into_iter().flatten() {
        seen.insert(lib_key(l["name"].as_str().unwrap_or_default()));
        libs.push(l.clone());
    }
    for l in out.get("libraries").and_then(|v| v.as_array()).into_iter().flatten() {
        if seen.insert(lib_key(l["name"].as_str().unwrap_or_default())) {
            libs.push(l.clone());
        }
    }

    let mut args = out.get("arguments").cloned().unwrap_or(json!({}));
    if let Some(ca) = child.get("arguments") {
        for k in ["game", "jvm"] {
            if let Some(extra) = ca[k].as_array() {
                let base = args[k].as_array().cloned().unwrap_or_default();
                args[k] = Value::Array(base.into_iter().chain(extra.iter().cloned()).collect());
            }
        }
    }

    for (k, v) in child {
        match k.as_str() {
            "libraries" | "arguments" | "inheritsFrom" => {}
            _ => {
                out.insert(k, v);
            }
        }
    }
    out.insert("libraries".into(), Value::Array(libs));
    if args.as_object().map(|o| !o.is_empty()).unwrap_or(false) {
        out.insert("arguments".into(), args);
    }
    Value::Object(out)
}

/// Id of the version whose client jar is used (root of the inheritance chain).
pub fn jar_id(root_id: &str) -> String {
    let mut cur = root_id.to_string();
    for _ in 0..8 {
        let Ok(v) = net::read_json(&version_json_path(&cur)) else { break };
        if let Some(j) = v["jar"].as_str() {
            return j.to_string();
        }
        match v["inheritsFrom"].as_str() {
            Some(p) => cur = p.to_string(),
            None => break,
        }
    }
    cur
}

pub fn rules_allow(rules: Option<&Value>, features: &HashMap<&str, bool>) -> bool {
    let Some(rules) = rules.and_then(|r| r.as_array()) else { return true };
    let mut allowed = false;
    for r in rules {
        let mut matches = true;
        if let Some(os) = r.get("os") {
            if let Some(name) = os["name"].as_str() {
                matches &= name == crate::platform::OS_NAME;
            }
            if let Some(arch) = os["arch"].as_str() {
                // Match the host CPU so Apple Silicon gets arm64 libraries/natives, not the x64 ones.
                let ok = if cfg!(target_arch = "aarch64") {
                    matches!(arch, "arm64" | "aarch64")
                } else {
                    matches!(arch, "x64" | "x86_64" | "amd64")
                };
                matches &= ok;
            }
        }
        if let Some(f) = r.get("features").and_then(|f| f.as_object()) {
            for (k, v) in f {
                let want = v.as_bool().unwrap_or(false);
                matches &= features.get(k.as_str()).copied().unwrap_or(false) == want;
            }
        }
        if matches {
            allowed = r["action"] == "allow";
        }
    }
    allowed
}

/// Maven coordinate -> relative path (`group/artifact/version/artifact-version[-classifier].ext`).
pub fn maven_path(name: &str) -> String {
    let (coord, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, "jar"),
    };
    let p: Vec<&str> = coord.split(':').collect();
    let group = p.first().copied().unwrap_or_default().replace('.', "/");
    let artifact = p.get(1).copied().unwrap_or_default();
    let version = p.get(2).copied().unwrap_or_default();
    let classifier = p.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    format!("{group}/{artifact}/{version}/{artifact}-{version}{classifier}.{ext}")
}

pub struct ResolvedLibs {
    pub classpath: Vec<PathBuf>,
    pub natives: Vec<PathBuf>,
    pub jobs: Vec<Job>,
}

pub fn resolve_libraries(version: &Value) -> ResolvedLibs {
    let lib_dir = settings::meta_dir().join("libraries");
    let features = HashMap::new();
    let mut out = ResolvedLibs { classpath: vec![], natives: vec![], jobs: vec![] };
    let mut seen = HashSet::new();

    for lib in version["libraries"].as_array().into_iter().flatten() {
        // Old Forge marks server-only libraries with "clientreq": false.
        if !rules_allow(lib.get("rules"), &features) || lib["clientreq"] == false {
            continue;
        }
        let name = lib["name"].as_str().unwrap_or_default();

        // Legacy natives: separate classifier jar that has to be extracted.
        if let Some(classifier) = lib["natives"][crate::platform::OS_NAME].as_str() {
            let classifier = classifier.replace("${arch}", "64");
            let art = &lib["downloads"]["classifiers"][&classifier];
            let rel = art["path"]
                .as_str()
                .map(String::from)
                .unwrap_or_else(|| maven_path(&format!("{name}:{classifier}")));
            let path = lib_dir.join(&rel);
            let url = art["url"].as_str().map(String::from).unwrap_or_else(|| {
                format!("{}{}", lib["url"].as_str().unwrap_or(LIBRARIES_URL), rel)
            });
            let mut j = Job::new(url, path.clone());
            j.sha1 = art["sha1"].as_str().map(String::from);
            j.size = art["size"].as_u64();
            out.jobs.push(j);
            out.natives.push(path);
            if lib["downloads"]["artifact"].is_null() {
                continue;
            }
        }

        let art = &lib["downloads"]["artifact"];
        let rel = art["path"].as_str().map(String::from).unwrap_or_else(|| maven_path(name));
        let path = lib_dir.join(&rel);
        if !seen.insert(path.clone()) {
            continue;
        }
        let mut alt_urls = vec![];
        let url = match art["url"].as_str() {
            Some("") => None, // produced locally by a Forge installer processor
            Some(u) => Some(u.to_string()),
            None => {
                // Old Forge (≤1.12) lists bare Maven coordinates with a dead http:// Forge URL.
                let base = lib["url"].as_str().unwrap_or(LIBRARIES_URL);
                let base = base.replace("http://files.minecraftforge.net/maven", FORGE_MAVEN.trim_end_matches('/'));
                let base = if base.ends_with('/') { base } else { format!("{base}/") };
                alt_urls = [FORGE_MAVEN, MAVEN_CENTRAL, LIBRARIES_URL]
                    .iter()
                    .filter(|m| **m != base)
                    .map(|m| format!("{m}{rel}"))
                    .collect();
                Some(format!("{base}{rel}"))
            }
        };
        if let Some(url) = url {
            let mut j = Job::new(url, path.clone());
            j.sha1 = art["sha1"].as_str().map(String::from);
            j.size = art["size"].as_u64();
            j.alt_urls = alt_urls;
            out.jobs.push(j);
        }
        out.classpath.push(path);
    }
    out
}

pub fn extract_natives(natives: &[PathBuf], dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    for jar in natives {
        let f = std::fs::File::open(jar).with_context(|| format!("open {}", jar.display()))?;
        let mut z = zip::ZipArchive::new(f)?;
        for i in 0..z.len() {
            let mut e = z.by_index(i)?;
            let name = e.name().to_string();
            if e.is_dir() || name.starts_with("META-INF") {
                continue;
            }
            let target = dir.join(Path::new(&name).file_name().unwrap_or_default());
            if target.exists() {
                continue;
            }
            let mut buf = vec![];
            e.read_to_end(&mut buf)?;
            std::fs::write(target, buf)?;
        }
    }
    Ok(())
}

/// Downloads the asset index + objects. Returns the directory to pass as `game_assets`.
pub async fn install_assets(version: &Value, task: &str, game_dir: &Path) -> Result<PathBuf> {
    let assets_dir = settings::meta_dir().join("assets");
    let idx = &version["assetIndex"];
    let id = idx["id"].as_str().ok_or_else(|| anyhow!("missing assetIndex"))?;
    let idx_path = assets_dir.join("indexes").join(format!("{id}.json"));
    let mut j = Job::new(idx["url"].as_str().unwrap_or_default(), idx_path.clone());
    j.sha1 = idx["sha1"].as_str().map(String::from);
    if !idx_path.exists() {
        net::download(&j).await?;
    }
    let index = net::read_json(&idx_path)?;
    let objects = index["objects"].as_object().cloned().unwrap_or_default();
    let jobs: Vec<Job> = objects
        .values()
        .filter_map(|o| {
            let hash = o["hash"].as_str()?;
            let mut j = Job::new(
                format!("{RESOURCES_URL}{}/{hash}", &hash[..2]),
                assets_dir.join("objects").join(&hash[..2]).join(hash),
            );
            j.sha1 = Some(hash.into());
            j.size = o["size"].as_u64();
            Some(j)
        })
        .collect();
    let t = task.to_string();
    let label = l!("Stahuji assety", "Downloading assets");
    net::download_all(jobs, 48, move |d, n| events::progress(&t, &label, d, n)).await?;

    // Pre-1.7 versions read loose files instead of the hashed object store.
    let legacy_target = if index["map_to_resources"].as_bool() == Some(true) {
        Some(game_dir.join("resources"))
    } else if index["virtual"].as_bool() == Some(true) {
        Some(assets_dir.join("virtual").join(id))
    } else {
        None
    };
    if let Some(target) = &legacy_target {
        for (name, o) in &objects {
            let hash = o["hash"].as_str().unwrap_or_default();
            let dst = target.join(name);
            if !dst.exists() && hash.len() > 2 {
                if let Some(d) = dst.parent() {
                    std::fs::create_dir_all(d)?;
                }
                let _ = std::fs::copy(assets_dir.join("objects").join(&hash[..2]).join(hash), &dst);
            }
        }
    }
    Ok(legacy_target.unwrap_or(assets_dir))
}

/// Downloads client jar, libraries and the log4j config for a (merged) version.
pub async fn install_files(root_id: &str, version: &Value, task: &str) -> Result<ResolvedLibs> {
    let libs = resolve_libraries(version);
    let mut jobs = libs.jobs.clone();

    let jar_id = jar_id(root_id);
    let jar_path = client_jar_path(&jar_id);
    let jar_version = if jar_id == root_id { version.clone() } else { load_merged(&jar_id).await? };
    let client = &jar_version["downloads"]["client"];
    match client["url"].as_str() {
        Some(url) => {
            let mut j = Job::new(url, jar_path);
            j.sha1 = client["sha1"].as_str().map(String::from);
            j.size = client["size"].as_u64();
            jobs.push(j);
        }
        None if !jar_path.exists() => bail!(l!("Chybí client jar pro {jar_id}", "Missing client jar for {jar_id}")),
        None => {}
    }

    if let Some(file) = version["logging"]["client"]["file"].as_object() {
        let id = file.get("id").and_then(|v| v.as_str()).unwrap_or("client.xml");
        let mut j = Job::new(
            file.get("url").and_then(|v| v.as_str()).unwrap_or_default(),
            settings::meta_dir().join("assets").join("log_configs").join(id),
        );
        j.sha1 = file.get("sha1").and_then(|v| v.as_str()).map(String::from);
        jobs.push(j);
    }

    let t = task.to_string();
    let label = l!("Stahuji knihovny", "Downloading libraries");
    net::download_all(jobs, 24, move |d, n| events::progress(&t, &label, d, n)).await?;
    Ok(libs)
}

pub fn client_jar_path(jar_id: &str) -> PathBuf {
    settings::meta_dir().join("versions").join(jar_id).join(format!("{jar_id}.jar"))
}

pub fn java_major(version: &Value) -> u32 {
    version["javaVersion"]["majorVersion"].as_u64().map(|v| v as u32).unwrap_or(8)
}
