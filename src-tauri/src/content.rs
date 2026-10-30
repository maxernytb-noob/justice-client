//! Mods, resource packs and shaders from Modrinth and CurseForge.

use crate::events;
use crate::instances::{self, Instance};
use crate::net::{self, Job, CLIENT};
use crate::settings;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Read;
use std::path::{Path, PathBuf};

const MR: &str = "https://api.modrinth.com/v2";
const CF: &str = "https://api.curseforge.com/v1";
const CF_GAME: u32 = 432;

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Mod,
    ResourcePack,
    Shader,
    /// Whole modpacks: only searched here, installed as a new instance (packs::install_modpack).
    Modpack,
}

impl Kind {
    pub fn parse(s: &str) -> Self {
        match s {
            "resourcepack" => Kind::ResourcePack,
            "shader" => Kind::Shader,
            "modpack" => Kind::Modpack,
            _ => Kind::Mod,
        }
    }
    fn folder(self) -> &'static str {
        match self {
            Kind::Mod => "mods",
            Kind::ResourcePack => "resourcepacks",
            Kind::Shader => "shaderpacks",
            Kind::Modpack => "modpacks",
        }
    }
    fn mr_type(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::ResourcePack => "resourcepack",
            Kind::Shader => "shader",
            Kind::Modpack => "modpack",
        }
    }
    fn cf_class(self) -> u32 {
        match self {
            Kind::Mod => 6,
            Kind::ResourcePack => 12,
            Kind::Shader => 6552,
            Kind::Modpack => 4471,
        }
    }
    fn key(self) -> &'static str {
        match self {
            Kind::Mod => "mod",
            Kind::ResourcePack => "resourcepack",
            Kind::Shader => "shader",
            Kind::Modpack => "modpack",
        }
    }
}

fn cf_loader(loader: &str) -> Option<u32> {
    match loader {
        "forge" => Some(1),
        "fabric" => Some(4),
        "quilt" => Some(5),
        "neoforge" => Some(6),
        _ => None,
    }
}

/// Loaders whose mods run on this instance (Quilt runs Fabric mods).
fn compatible_loaders(loader: &str) -> Vec<&'static str> {
    match loader {
        "fabric" => vec!["fabric"],
        "quilt" => vec!["quilt", "fabric"],
        "forge" => vec!["forge"],
        "neoforge" => vec!["neoforge"],
        _ => vec![],
    }
}

// ---------------- Search ----------------

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub source: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub icon_url: String,
    pub downloads: u64,
    pub updated: String,
    pub categories: Vec<String>,
    pub url: String,
}

#[derive(Serialize)]
pub struct SearchResult {
    pub hits: Vec<Hit>,
    pub total: u64,
}

pub struct SearchQuery<'a> {
    pub source: &'a str,
    pub query: &'a str,
    pub kind: Kind,
    pub game_version: &'a str,
    pub loader: &'a str,
    pub sort: &'a str,
    pub offset: u32,
    pub category: &'a str,
}

pub async fn search(q: SearchQuery<'_>) -> Result<SearchResult> {
    match q.source {
        "curseforge" => search_cf(q).await,
        _ => search_mr(q).await,
    }
}

async fn search_mr(q: SearchQuery<'_>) -> Result<SearchResult> {
    let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", q.kind.mr_type())]];
    if !q.game_version.is_empty() {
        facets.push(vec![format!("versions:{}", q.game_version)]);
    }
    if q.kind == Kind::Mod {
        let loaders = compatible_loaders(q.loader);
        if !loaders.is_empty() {
            facets.push(loaders.iter().map(|l| format!("categories:{l}")).collect());
        }
    }
    if !q.category.is_empty() {
        facets.push(vec![format!("categories:{}", q.category)]);
    }
    let index = match q.sort {
        "downloads" => "downloads",
        "updated" => "updated",
        "newest" => "newest",
        _ => "relevance",
    };
    let url = format!(
        "{MR}/search?query={}&facets={}&index={index}&offset={}&limit=20",
        urlencoding::encode(q.query),
        urlencoding::encode(&serde_json::to_string(&facets)?),
        q.offset
    );
    let v = net::get_json(&url).await?;
    let hits = v["hits"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|h| Hit {
            source: "modrinth".into(),
            id: h["project_id"].as_str().unwrap_or_default().into(),
            slug: h["slug"].as_str().unwrap_or_default().into(),
            title: h["title"].as_str().unwrap_or_default().into(),
            description: h["description"].as_str().unwrap_or_default().into(),
            author: h["author"].as_str().unwrap_or_default().into(),
            icon_url: h["icon_url"].as_str().unwrap_or_default().into(),
            downloads: h["downloads"].as_u64().unwrap_or(0),
            updated: h["date_modified"].as_str().unwrap_or_default().into(),
            categories: h["display_categories"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| c.as_str().map(String::from))
                .collect(),
            url: format!(
                "https://modrinth.com/{}/{}",
                h["project_type"].as_str().unwrap_or("mod"),
                h["slug"].as_str().unwrap_or_default()
            ),
        })
        .collect();
    Ok(SearchResult { hits, total: v["total_hits"].as_u64().unwrap_or(0) })
}

/// Key baked in at build time (src-tauri/cf_key.txt); a key set in Settings overrides it.
pub const BUILTIN_CF_KEY: &str = env!("JC_CF_KEY");

pub fn has_cf_key() -> bool {
    !settings::get().curseforge_key.trim().is_empty() || !BUILTIN_CF_KEY.is_empty()
}

fn cf_key() -> Result<String> {
    let k = settings::get().curseforge_key;
    let k = if k.trim().is_empty() { BUILTIN_CF_KEY.to_string() } else { k.trim().to_string() };
    if k.is_empty() {
        bail!(l!("CurseForge potřebuje API klíč — vlož ho v Nastavení.", "CurseForge needs an API key — add it in Settings."));
    }
    Ok(k)
}

async fn cf_get(path: &str) -> Result<Value> {
    let r = CLIENT.get(format!("{CF}{path}")).header("x-api-key", cf_key()?).send().await?;
    if r.status().as_u16() == 403 && path.starts_with("/mods/search") {
        bail!(l!(
            "CurseForge zatím nepovolil vyhledávání pro tenhle API klíč. Instalace a aktualizace z CurseForge fungují — mody zatím hledej na Modrinthu.",
            "CurseForge hasn't enabled search for this API key yet. Installs and updates from CurseForge work — search on Modrinth for now."
        ));
    }
    if r.status().as_u16() == 403 {
        bail!(l!("CurseForge odmítl API klíč — zkontroluj ho v Nastavení.", "CurseForge rejected the API key — check it in Settings."));
    }
    if !r.status().is_success() {
        bail!("CurseForge {} ({})", path, r.status());
    }
    Ok(r.json().await?)
}

fn cf_hit(m: &Value) -> Hit {
    Hit {
        source: "curseforge".into(),
        id: m["id"].as_u64().unwrap_or(0).to_string(),
        slug: m["slug"].as_str().unwrap_or_default().into(),
        title: m["name"].as_str().unwrap_or_default().into(),
        description: m["summary"].as_str().unwrap_or_default().into(),
        author: m["authors"][0]["name"].as_str().unwrap_or_default().into(),
        icon_url: m["logo"]["thumbnailUrl"].as_str().unwrap_or_default().into(),
        downloads: m["downloadCount"].as_f64().unwrap_or(0.0) as u64,
        updated: m["dateModified"].as_str().unwrap_or_default().into(),
        categories: m["categories"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|c| c["name"].as_str().map(String::from))
            .take(3)
            .collect(),
        url: m["links"]["websiteUrl"].as_str().unwrap_or_default().into(),
    }
}

async fn search_cf(q: SearchQuery<'_>) -> Result<SearchResult> {
    // sortField: 1 featured, 2 popularity, 3 last updated, 6 downloads
    let sort = match q.sort {
        "downloads" => 6,
        "updated" | "newest" => 3,
        _ => 2,
    };
    let mut path = format!(
        "/mods/search?gameId={CF_GAME}&classId={}&searchFilter={}&sortField={sort}&sortOrder=desc&pageSize=20&index={}",
        q.kind.cf_class(),
        urlencoding::encode(q.query),
        q.offset
    );
    if !q.game_version.is_empty() {
        path += &format!("&gameVersion={}", q.game_version);
    }
    if q.kind == Kind::Mod {
        if let Some(l) = cf_loader(q.loader) {
            path += &format!("&modLoaderType={l}");
        }
    }
    let v = cf_get(&path).await?;
    Ok(SearchResult {
        hits: v["data"].as_array().into_iter().flatten().map(cf_hit).collect(),
        total: v["pagination"]["totalCount"].as_u64().unwrap_or(0).min(10_000),
    })
}

// ---------------- Project details ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVersion {
    pub id: String,
    pub name: String,
    pub version_number: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub date: String,
    pub kind: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectDetails {
    pub hit: Hit,
    pub body: String,
    pub gallery: Vec<String>,
    pub versions: Vec<ProjectVersion>,
}

pub async fn details(source: &str, id: &str, kind: Kind, inst: &Instance) -> Result<ProjectDetails> {
    if source == "curseforge" {
        let m = cf_get(&format!("/mods/{id}")).await?["data"].clone();
        let body = cf_get(&format!("/mods/{id}/description")).await.map(|v| v["data"].as_str().unwrap_or_default().to_string()).unwrap_or_default();
        let files = cf_files(id, kind, inst).await.unwrap_or_default();
        return Ok(ProjectDetails {
            hit: cf_hit(&m),
            body,
            gallery: m["screenshots"].as_array().into_iter().flatten().filter_map(|s| s["url"].as_str().map(String::from)).take(8).collect(),
            versions: files
                .iter()
                .map(|f| ProjectVersion {
                    id: f["id"].as_u64().unwrap_or(0).to_string(),
                    name: f["displayName"].as_str().unwrap_or_default().into(),
                    version_number: f["fileName"].as_str().unwrap_or_default().into(),
                    game_versions: f["gameVersions"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect(),
                    loaders: vec![],
                    date: f["fileDate"].as_str().unwrap_or_default().into(),
                    kind: match f["releaseType"].as_u64() { Some(2) => "beta", Some(3) => "alpha", _ => "release" }.into(),
                })
                .collect(),
        });
    }
    let p = net::get_json(&format!("{MR}/project/{id}")).await?;
    let versions = mr_versions(id, kind, inst).await.unwrap_or_default();
    let team = net::get_json(&format!("{MR}/project/{id}/members")).await.unwrap_or(Value::Null);
    let author = team.as_array().and_then(|a| a.iter().find(|m| m["role"] == "Owner").or(a.first())).map(|m| m["user"]["username"].as_str().unwrap_or_default().to_string()).unwrap_or_default();
    Ok(ProjectDetails {
        hit: Hit {
            source: "modrinth".into(),
            id: p["id"].as_str().unwrap_or_default().into(),
            slug: p["slug"].as_str().unwrap_or_default().into(),
            title: p["title"].as_str().unwrap_or_default().into(),
            description: p["description"].as_str().unwrap_or_default().into(),
            author,
            icon_url: p["icon_url"].as_str().unwrap_or_default().into(),
            downloads: p["downloads"].as_u64().unwrap_or(0),
            updated: p["updated"].as_str().unwrap_or_default().into(),
            categories: p["categories"].as_array().into_iter().flatten().filter_map(|c| c.as_str().map(String::from)).collect(),
            url: format!("https://modrinth.com/{}/{}", p["project_type"].as_str().unwrap_or("mod"), p["slug"].as_str().unwrap_or_default()),
        },
        body: p["body"].as_str().unwrap_or_default().into(),
        gallery: p["gallery"].as_array().into_iter().flatten().filter_map(|g| g["url"].as_str().map(String::from)).take(8).collect(),
        versions: versions
            .iter()
            .map(|v| ProjectVersion {
                id: v["id"].as_str().unwrap_or_default().into(),
                name: v["name"].as_str().unwrap_or_default().into(),
                version_number: v["version_number"].as_str().unwrap_or_default().into(),
                game_versions: v["game_versions"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect(),
                loaders: v["loaders"].as_array().into_iter().flatten().filter_map(|x| x.as_str().map(String::from)).collect(),
                date: v["date_published"].as_str().unwrap_or_default().into(),
                kind: v["version_type"].as_str().unwrap_or("release").into(),
            })
            .collect(),
    })
}

async fn mr_versions(id: &str, kind: Kind, inst: &Instance) -> Result<Vec<Value>> {
    // A modpack brings its own game version and loader, so every version is offered.
    if kind == Kind::Modpack {
        return Ok(net::get_json(&format!("{MR}/project/{id}/version")).await?.as_array().cloned().unwrap_or_default());
    }
    let mut url = format!(
        "{MR}/project/{id}/version?game_versions={}",
        urlencoding::encode(&json!([inst.mc_version]).to_string())
    );
    if kind == Kind::Mod {
        let loaders = compatible_loaders(&inst.loader);
        if !loaders.is_empty() {
            url += &format!("&loaders={}", urlencoding::encode(&json!(loaders).to_string()));
        }
    }
    let v = net::get_json(&url).await?;
    Ok(v.as_array().cloned().unwrap_or_default())
}

/// Download URL and file name of a modpack version (the newest release when none is picked).
pub async fn modpack_file(source: &str, project: &str, version: Option<&str>) -> Result<(String, String)> {
    if source == "curseforge" {
        let file = match version {
            Some(f) => cf_get(&format!("/mods/{project}/files/{f}")).await?["data"].clone(),
            None => {
                let mut files = cf_get(&format!("/mods/{project}/files?pageSize=50")).await?["data"].as_array().cloned().unwrap_or_default();
                files.sort_by_key(|f| (f["releaseType"].as_u64().unwrap_or(1), std::cmp::Reverse(f["fileDate"].as_str().unwrap_or_default().to_string())));
                files.into_iter().next().ok_or_else(|| anyhow!(l!("Modpack nemá žádnou verzi", "The modpack has no version")))?
            }
        };
        let url = file["downloadUrl"].as_str().filter(|u| !u.is_empty()).ok_or_else(|| {
            anyhow!(l!("Autor tohohle modpacku zakázal stahování mimo CurseForge — stáhni zip v prohlížeči a importuj ho", "This modpack's author disallows downloads outside CurseForge — download the zip in your browser and import it"))
        })?;
        return Ok((url.to_string(), file["fileName"].as_str().unwrap_or("modpack.zip").to_string()));
    }
    let v = match version {
        Some(v) => net::get_json(&format!("{MR}/version/{v}")).await?,
        None => net::get_json(&format!("{MR}/project/{project}/version"))
            .await?
            .as_array()
            .into_iter()
            .flatten()
            .max_by_key(|v| (v["version_type"] == "release", v["date_published"].as_str().unwrap_or_default().to_string()))
            .cloned()
            .ok_or_else(|| anyhow!(l!("Modpack nemá žádnou verzi", "The modpack has no version")))?,
    };
    let files = v["files"].as_array().cloned().unwrap_or_default();
    let f = files.iter().find(|f| f["primary"] == true).or(files.first()).ok_or_else(|| anyhow!(l!("verze bez souboru", "version has no file")))?;
    Ok((f["url"].as_str().unwrap_or_default().to_string(), f["filename"].as_str().unwrap_or("modpack.mrpack").to_string()))
}

async fn cf_files(id: &str, kind: Kind, inst: &Instance) -> Result<Vec<Value>> {
    if kind == Kind::Modpack {
        let mut files = cf_get(&format!("/mods/{id}/files?pageSize=50")).await?["data"].as_array().cloned().unwrap_or_default();
        files.sort_by(|a, b| b["fileDate"].as_str().cmp(&a["fileDate"].as_str()));
        return Ok(files);
    }
    let mut path = format!("/mods/{id}/files?gameVersion={}&pageSize=50", inst.mc_version);
    if kind == Kind::Mod {
        if let Some(l) = cf_loader(&inst.loader) {
            path += &format!("&modLoaderType={l}");
        }
    }
    let v = cf_get(&path).await?;
    let mut files = v["data"].as_array().cloned().unwrap_or_default();
    files.sort_by(|a, b| b["fileDate"].as_str().cmp(&a["fileDate"].as_str()));
    Ok(files)
}

// ---------------- Installed content metadata ----------------

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Tracked {
    pub source: String,
    pub project_id: String,
    pub version_id: String,
    pub title: String,
    pub icon_url: String,
    pub kind: String,
    pub version_number: String,
    /// Version locked by the compatibility fixer; skipped by update checks.
    pub pinned: bool,
}

fn meta_path(inst_id: &str) -> PathBuf {
    instances::game_dir(inst_id).join(".justice").join("content.json")
}

pub fn load_meta(inst_id: &str) -> BTreeMap<String, Tracked> {
    net::read_json(&meta_path(inst_id)).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default()
}

pub fn set_pinned_project(inst_id: &str, project_id: &str, pinned: bool) -> Result<()> {
    let mut m = load_meta(inst_id);
    for t in m.values_mut().filter(|t| t.project_id == project_id) {
        t.pinned = pinned;
    }
    save_meta(inst_id, &m)
}

pub fn set_pinned_file(inst_id: &str, kind: &str, file: &str, pinned: bool) -> Result<()> {
    let mut m = load_meta(inst_id);
    if let Some(t) = m.get_mut(&meta_key(Kind::parse(kind).folder(), file)) {
        t.pinned = pinned;
    }
    save_meta(inst_id, &m)
}

/// Drops metadata of files that are no longer on disk (e.g. after the optimizer's undo).
pub fn forget_missing(inst_id: &str) {
    let game = instances::game_dir(inst_id);
    let mut m = load_meta(inst_id);
    let before = m.len();
    m.retain(|key, _| {
        let p = game.join(key);
        p.exists() || p.with_file_name(format!("{}.disabled", p.file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default())).exists()
    });
    if m.len() != before {
        let _ = save_meta(inst_id, &m);
    }
}

fn save_meta(inst_id: &str, m: &BTreeMap<String, Tracked>) -> Result<()> {
    net::write_json(&meta_path(inst_id), &serde_json::to_value(m)?)
}

/// Key used in metadata: `<folder>/<file name without .disabled>`
fn meta_key(folder: &str, file: &str) -> String {
    format!("{folder}/{}", file.trim_end_matches(".disabled"))
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub file: String,
    pub kind: String,
    pub enabled: bool,
    pub title: String,
    pub version: String,
    pub icon_url: String,
    pub source: String,
    pub project_id: String,
    pub size: u64,
    pub pinned: bool,
}

fn read_jar_info(path: &Path) -> Option<(String, String)> {
    let f = std::fs::File::open(path).ok()?;
    let mut z = zip::ZipArchive::new(f).ok()?;
    for name in ["fabric.mod.json", "quilt.mod.json"] {
        if let Ok(mut e) = z.by_name(name) {
            let mut s = String::new();
            e.read_to_string(&mut s).ok()?;
            let v: Value = serde_json::from_str(&s).ok()?;
            let root = if name == "quilt.mod.json" { &v["quilt_loader"] } else { &v };
            let title = root["name"].as_str().or(root["metadata"]["name"].as_str()).or(root["id"].as_str())?;
            return Some((title.to_string(), root["version"].as_str().unwrap_or_default().to_string()));
        }
    }
    let toml = ["META-INF/mods.toml", "META-INF/neoforge.mods.toml"].into_iter().find(|n| z.by_name(n).is_ok());
    if let Some(Ok(mut e)) = toml.map(|n| z.by_name(n)) {
        let mut s = String::new();
        e.read_to_string(&mut s).ok()?;
        let grab = |key: &str| {
            s.lines().find_map(|l| {
                let l = l.trim();
                l.strip_prefix(key).and_then(|r| r.trim().strip_prefix('=')).map(|r| r.trim().trim_matches('"').to_string())
            })
        };
        let title = grab("displayName")?;
        let ver = grab("version").unwrap_or_default();
        return Some((title, if ver.contains("${") { String::new() } else { ver }));
    }
    None
}

pub fn list_installed(inst_id: &str) -> Vec<Installed> {
    let meta = load_meta(inst_id);
    let game = instances::game_dir(inst_id);
    let mut out = vec![];
    for kind in [Kind::Mod, Kind::ResourcePack, Kind::Shader] {
        let folder = kind.folder();
        for e in std::fs::read_dir(game.join(folder)).into_iter().flatten().flatten() {
            let file = e.file_name().to_string_lossy().into_owned();
            let base = file.trim_end_matches(".disabled");
            let ok_ext = base.ends_with(".jar") || base.ends_with(".zip") || (kind != Kind::Mod && e.path().is_dir());
            if !ok_ext {
                continue;
            }
            let t = meta.get(&meta_key(folder, &file)).cloned().unwrap_or_default();
            let (title, version) = if !t.title.is_empty() {
                (t.title.clone(), t.version_number.clone())
            } else if kind == Kind::Mod {
                read_jar_info(&e.path()).unwrap_or_else(|| (base.trim_end_matches(".jar").to_string(), String::new()))
            } else {
                (base.trim_end_matches(".zip").to_string(), String::new())
            };
            out.push(Installed {
                enabled: !file.ends_with(".disabled"),
                kind: kind.key().into(),
                title,
                version,
                icon_url: t.icon_url,
                source: t.source,
                project_id: t.project_id,
                pinned: t.pinned,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
                file,
            });
        }
    }
    out.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    out
}

pub fn toggle(inst_id: &str, kind: &str, file: &str, enabled: bool) -> Result<()> {
    let dir = instances::game_dir(inst_id).join(Kind::parse(kind).folder());
    let base = file.trim_end_matches(".disabled");
    let (from, to) = if enabled {
        (format!("{base}.disabled"), base.to_string())
    } else {
        (base.to_string(), format!("{base}.disabled"))
    };
    if dir.join(&from).exists() {
        std::fs::rename(dir.join(from), dir.join(to))?;
    }
    Ok(())
}

pub fn remove(inst_id: &str, kind: &str, file: &str) -> Result<()> {
    let folder = Kind::parse(kind).folder();
    let p = instances::game_dir(inst_id).join(folder).join(file);
    if p.is_dir() {
        std::fs::remove_dir_all(&p)?;
    } else if p.exists() {
        std::fs::remove_file(&p)?;
    }
    let mut m = load_meta(inst_id);
    m.remove(&meta_key(folder, file));
    save_meta(inst_id, &m)
}

/// Copies dropped files into the right folder: .jar -> mods, .zip -> shaderpacks or resourcepacks.
pub fn add_local_files(inst_id: &str, paths: &[String]) -> Result<usize> {
    let game = instances::game_dir(inst_id);
    let mut n = 0;
    for p in paths {
        let src = Path::new(p);
        let name = src.file_name().ok_or_else(|| anyhow!("bad path"))?;
        let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let folder = match ext.as_str() {
            "jar" => "mods",
            "zip" => {
                let is_shader = std::fs::File::open(src)
                    .ok()
                    .and_then(|f| zip::ZipArchive::new(f).ok())
                    .map(|z| z.file_names().any(|f| f.starts_with("shaders/")))
                    .unwrap_or(false);
                if is_shader { "shaderpacks" } else { "resourcepacks" }
            }
            _ => continue,
        };
        std::fs::create_dir_all(game.join(folder))?;
        std::fs::copy(src, game.join(folder).join(name))?;
        n += 1;
    }
    if n == 0 {
        bail!(l!("Podporované jsou jen soubory .jar a .zip", "Only .jar and .zip files are supported"));
    }
    Ok(n)
}

// ---------------- Install ----------------

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub skipped: Vec<String>,
    /// CurseForge files whose authors block third-party downloads: (title, url).
    pub manual: Vec<(String, String)>,
}

pub fn installed_projects(inst_id: &str) -> HashSet<String> {
    load_meta(inst_id).values().map(|t| t.project_id.clone()).collect()
}

pub async fn install(
    inst_id: &str,
    source: &str,
    project_id: &str,
    kind: Kind,
    version_id: Option<&str>,
) -> Result<InstallReport> {
    let inst = instances::load(inst_id)?;
    let mut report = InstallReport::default();
    let mut queue = vec![(source.to_string(), project_id.to_string(), version_id.map(String::from), kind)];
    let mut done = installed_projects(inst_id);
    // A project explicitly asked for (e.g. a new version) is reinstalled even if present.
    done.remove(project_id);
    while let Some((src, pid, vid, k)) = queue.pop() {
        if !done.insert(pid.clone()) {
            continue;
        }
        events::progress(inst_id, &l!("Instaluji obsah", "Installing content"), report.installed.len(), report.installed.len() + queue.len() + 1);
        let deps = if src == "curseforge" {
            install_cf(&inst, &pid, k, vid.as_deref(), &mut report).await
        } else {
            install_mr(&inst, &pid, k, vid.as_deref(), &mut report).await
        };
        match deps {
            Ok(deps) => queue.extend(deps.into_iter().map(|d| (src.clone(), d, None, Kind::Mod))),
            Err(e) if pid != project_id => report.skipped.push(format!("{pid}: {e:#}")),
            Err(e) => {
                events::progress_done(inst_id);
                return Err(e);
            }
        }
    }
    events::progress_done(inst_id);
    Ok(report)
}

fn remove_previous(inst_id: &str, folder: &str, project_id: &str) -> Result<()> {
    let mut m = load_meta(inst_id);
    let old: Vec<String> = m.iter().filter(|(_, t)| t.project_id == project_id).map(|(k, _)| k.clone()).collect();
    for key in old {
        let file = key.trim_start_matches(&format!("{folder}/")).to_string();
        let dir = instances::game_dir(inst_id).join(folder);
        let _ = std::fs::remove_file(dir.join(&file));
        let _ = std::fs::remove_file(dir.join(format!("{file}.disabled")));
        m.remove(&key);
    }
    save_meta(inst_id, &m)
}

async fn install_mr(inst: &Instance, pid: &str, kind: Kind, vid: Option<&str>, report: &mut InstallReport) -> Result<Vec<String>> {
    let version = match vid {
        Some(v) => net::get_json(&format!("{MR}/version/{v}")).await?,
        None => mr_versions(pid, kind, inst)
            .await?
            .into_iter()
            .max_by_key(|v| (v["version_type"] == "release", v["date_published"].as_str().unwrap_or_default().to_string()))
            .ok_or_else(|| anyhow!(l!("Žádná verze pro {} {}", "No version for {} {}", inst.loader, inst.mc_version)))?,
    };
    let files = version["files"].as_array().cloned().unwrap_or_default();
    let file = files.iter().find(|f| f["primary"] == true).or(files.first()).ok_or_else(|| anyhow!(l!("verze bez souboru", "version has no file")))?;
    let name = file["filename"].as_str().unwrap_or("mod.jar").to_string();
    let project_id = version["project_id"].as_str().unwrap_or(pid).to_string();
    let project = net::get_json(&format!("{MR}/project/{project_id}")).await.unwrap_or(Value::Null);
    let folder = kind.folder();

    remove_previous(&inst.id, folder, &project_id)?;
    let mut job = Job::new(file["url"].as_str().unwrap_or_default(), instances::game_dir(&inst.id).join(folder).join(&name));
    job.sha1 = file["hashes"]["sha1"].as_str().map(String::from);
    net::download(&job).await?;

    let title = project["title"].as_str().unwrap_or(&name).to_string();
    let mut m = load_meta(&inst.id);
    m.insert(
        meta_key(folder, &name),
        Tracked {
            source: "modrinth".into(),
            project_id: project_id.clone(),
            version_id: version["id"].as_str().unwrap_or_default().into(),
            title: title.clone(),
            icon_url: project["icon_url"].as_str().unwrap_or_default().into(),
            kind: kind.key().into(),
            version_number: version["version_number"].as_str().unwrap_or_default().into(), pinned: false,
        },
    );
    save_meta(&inst.id, &m)?;
    report.installed.push(title);

    let mut deps = vec![];
    for d in version["dependencies"].as_array().into_iter().flatten() {
        if d["dependency_type"] != "required" {
            continue;
        }
        if let Some(p) = d["project_id"].as_str() {
            deps.push(p.to_string());
        } else if let Some(v) = d["version_id"].as_str() {
            if let Ok(dv) = net::get_json(&format!("{MR}/version/{v}")).await {
                if let Some(p) = dv["project_id"].as_str() {
                    deps.push(p.to_string());
                }
            }
        }
    }
    Ok(deps)
}

async fn install_cf(inst: &Instance, pid: &str, kind: Kind, fid: Option<&str>, report: &mut InstallReport) -> Result<Vec<String>> {
    let file = match fid {
        Some(f) => cf_get(&format!("/mods/{pid}/files/{f}")).await?["data"].clone(),
        None => cf_files(pid, kind, inst)
            .await?
            .into_iter()
            .min_by_key(|f| (f["releaseType"].as_u64().unwrap_or(1), std::cmp::Reverse(f["fileDate"].as_str().unwrap_or_default().to_string())))
            .ok_or_else(|| anyhow!(l!("Žádná verze pro {} {}", "No version for {} {}", inst.loader, inst.mc_version)))?,
    };
    let m = cf_get(&format!("/mods/{pid}")).await?["data"].clone();
    let kind = match m["classId"].as_u64() {
        Some(12) => Kind::ResourcePack,
        Some(6552) => Kind::Shader,
        _ => kind,
    };
    let title = m["name"].as_str().unwrap_or(pid).to_string();
    let name = file["fileName"].as_str().unwrap_or("mod.jar").to_string();
    let Some(url) = file["downloadUrl"].as_str() else {
        let page = format!("{}/files/{}", m["links"]["websiteUrl"].as_str().unwrap_or_default(), file["id"]);
        report.manual.push((title, page));
        return Ok(vec![]);
    };
    let folder = kind.folder();
    remove_previous(&inst.id, folder, pid)?;
    let mut job = Job::new(url, instances::game_dir(&inst.id).join(folder).join(&name));
    job.sha1 = file["hashes"]
        .as_array()
        .and_then(|h| h.iter().find(|x| x["algo"] == 1))
        .and_then(|x| x["value"].as_str().map(String::from));
    net::download(&job).await?;

    let mut meta = load_meta(&inst.id);
    meta.insert(
        meta_key(folder, &name),
        Tracked {
            source: "curseforge".into(),
            project_id: pid.into(),
            version_id: file["id"].as_u64().unwrap_or(0).to_string(),
            title: title.clone(),
            icon_url: m["logo"]["thumbnailUrl"].as_str().unwrap_or_default().into(),
            kind: kind.key().into(),
            version_number: file["displayName"].as_str().unwrap_or_default().into(), pinned: false,
        },
    );
    save_meta(&inst.id, &meta)?;
    report.installed.push(title);

    Ok(file["dependencies"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|d| d["relationType"] == 3)
        .filter_map(|d| d["modId"].as_u64().map(|x| x.to_string()))
        .collect())
}

/// Install many Modrinth projects by slug, ignoring ones without a compatible version.
/// Modpacks: exact Modrinth versions, fetched with two bulk API calls and downloaded 16 at a time,
/// instead of a few requests and one download per mod in a row. Returns the required dependencies
/// that aren't part of the pack, and the versions that couldn't be installed this way (the caller
/// falls back to `install` for both).
pub async fn install_mr_versions(inst_id: &str, items: &[(String, Kind)]) -> Result<(InstallReport, Vec<String>, Vec<(String, Kind)>)> {
    let mut report = InstallReport::default();
    if items.is_empty() {
        return Ok((report, vec![], vec![]));
    }
    let bulk = |path: &str, ids: Vec<String>| {
        let url = format!("{MR}/{path}?ids={}", urlencoding::encode(&serde_json::to_string(&ids).unwrap_or_default()));
        async move { net::get_json(&url).await }
    };
    let mut versions: HashMap<String, Value> = HashMap::new();
    for chunk in items.chunks(100) {
        let list = bulk("versions", chunk.iter().map(|(v, _)| v.clone()).collect()).await?;
        for v in list.as_array().into_iter().flatten() {
            if let Some(id) = v["id"].as_str() {
                versions.insert(id.to_string(), v.clone());
            }
        }
    }
    let projects = mr_projects(versions.values()).await;

    let game = instances::game_dir(inst_id);
    let mut jobs = vec![];
    let mut planned = vec![];
    let mut leftover = vec![];
    for (vid, kind) in items {
        let Some(v) = versions.get(vid) else {
            leftover.push((vid.clone(), *kind));
            continue;
        };
        let files = v["files"].as_array().cloned().unwrap_or_default();
        let Some(file) = files.iter().find(|f| f["primary"] == true).or(files.first()) else {
            leftover.push((vid.clone(), *kind));
            continue;
        };
        let name = file["filename"].as_str().unwrap_or("mod.jar").to_string();
        if name.contains(['/', '\\']) || name.starts_with('.') {
            leftover.push((vid.clone(), *kind));
            continue;
        }
        let path = game.join(kind.folder()).join(&name);
        let mut job = Job::new(file["url"].as_str().unwrap_or_default(), path.clone());
        job.sha1 = file["hashes"]["sha1"].as_str().map(String::from);
        job.size = file["size"].as_u64();
        jobs.push(job);
        planned.push((vid.clone(), *kind, name, path));
    }
    let id = inst_id.to_string();
    let label = l!("Stahuji modpack", "Downloading modpack");
    if let Err(e) = net::download_all(jobs, 16, move |d, n| events::progress(&id, &label, d, n)).await {
        lwarn!("modpack: some downloads failed: {e:#}");
    }

    // Metadata written once for the whole pack.
    let mut m = load_meta(inst_id);
    let mut have: HashSet<String> = m.values().map(|t| t.project_id.clone()).collect();
    let mut deps = vec![];
    for (vid, kind, name, path) in planned {
        if !path.is_file() {
            leftover.push((vid, kind));
            continue;
        }
        let v = &versions[&vid];
        let pid = v["project_id"].as_str().unwrap_or_default().to_string();
        let p = projects.get(&pid).cloned().unwrap_or(Value::Null);
        let title = p["title"].as_str().unwrap_or(&name).to_string();
        m.insert(
            meta_key(kind.folder(), &name),
            Tracked {
                source: "modrinth".into(),
                project_id: pid.clone(),
                version_id: vid.clone(),
                title: title.clone(),
                icon_url: p["icon_url"].as_str().unwrap_or_default().into(),
                kind: kind.key().into(),
                version_number: v["version_number"].as_str().unwrap_or_default().into(),
                pinned: false,
            },
        );
        have.insert(pid);
        report.installed.push(title);
        for d in v["dependencies"].as_array().into_iter().flatten() {
            if d["dependency_type"] == "required" {
                if let Some(p) = d["project_id"].as_str() {
                    deps.push(p.to_string());
                }
            }
        }
    }
    save_meta(inst_id, &m)?;
    deps.retain(|d| !have.contains(d));
    deps.sort();
    deps.dedup();
    Ok((report, deps, leftover))
}

/// Projects (title, icon) of these Modrinth versions, in bulk.
async fn mr_projects<'a>(versions: impl Iterator<Item = &'a Value>) -> HashMap<String, Value> {
    let ids: Vec<String> = versions.filter_map(|v| v["project_id"].as_str().map(String::from)).collect::<HashSet<_>>().into_iter().collect();
    let mut projects = HashMap::new();
    for chunk in ids.chunks(100) {
        let url = format!("{MR}/projects?ids={}", urlencoding::encode(&serde_json::to_string(chunk).unwrap_or_default()));
        if let Ok(list) = net::get_json(&url).await {
            for p in list.as_array().into_iter().flatten() {
                if let Some(id) = p["id"].as_str() {
                    projects.insert(id.to_string(), p.clone());
                }
            }
        }
    }
    projects
}

/// Files a .mrpack put in place, given as (path in the game folder, sha1): found on Modrinth by
/// hash in one request, so they get a title, icon and update checks like content from the browser.
pub async fn track_by_hashes(inst_id: &str, files: &[(String, String)]) -> Result<()> {
    if files.is_empty() {
        return Ok(());
    }
    let hashes: Vec<&str> = files.iter().map(|(_, h)| h.as_str()).collect();
    let found: Value = CLIENT
        .post(format!("{MR}/version_files"))
        .json(&json!({ "hashes": hashes, "algorithm": "sha1" }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    let Some(found) = found.as_object() else { return Ok(()) };
    let projects = mr_projects(found.values()).await;
    let mut m = load_meta(inst_id);
    for (rel, sha1) in files {
        let Some(v) = found.get(sha1) else { continue };
        let Some((folder, name)) = rel.split_once('/') else { continue };
        let kind = match folder {
            "mods" => Kind::Mod,
            "resourcepacks" => Kind::ResourcePack,
            "shaderpacks" => Kind::Shader,
            _ => continue,
        };
        if name.contains('/') {
            continue;
        }
        let pid = v["project_id"].as_str().unwrap_or_default().to_string();
        let p = projects.get(&pid).cloned().unwrap_or(Value::Null);
        m.insert(
            meta_key(kind.folder(), name),
            Tracked {
                source: "modrinth".into(),
                project_id: pid,
                version_id: v["id"].as_str().unwrap_or_default().into(),
                title: p["title"].as_str().unwrap_or(name).into(),
                icon_url: p["icon_url"].as_str().unwrap_or_default().into(),
                kind: kind.key().into(),
                version_number: v["version_number"].as_str().unwrap_or_default().into(),
                pinned: false,
            },
        );
    }
    save_meta(inst_id, &m)
}

pub async fn install_many(inst_id: &str, slugs: &[&str], kind: Kind) -> InstallReport {
    let mut total = InstallReport::default();
    for s in slugs {
        // "a|b": alternatives for different game versions — take the first that installs.
        let mut ok = false;
        for alt in s.split('|') {
            if let Ok(r) = install(inst_id, "modrinth", alt, kind, None).await {
                total.installed.extend(r.installed);
                total.skipped.extend(r.skipped);
                total.manual.extend(r.manual);
                ok = true;
                break;
            }
        }
        if !ok {
            total.skipped.push((*s).to_string());
        }
    }
    total
}

// ---------------- Updates ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Update {
    pub file: String,
    pub kind: String,
    pub title: String,
    pub source: String,
    pub project_id: String,
    pub new_version_id: String,
    pub new_version: String,
}

pub async fn check_updates(inst_id: &str) -> Result<Vec<Update>> {
    let inst = instances::load(inst_id)?;
    let meta = load_meta(inst_id);
    let mut out = vec![];
    for kind in [Kind::Mod, Kind::ResourcePack, Kind::Shader] {
        let folder = kind.folder();
        let mut by_hash: BTreeMap<String, (String, Tracked)> = BTreeMap::new();
        for (key, t) in meta.iter().filter(|(_, t)| t.kind == kind.key() && t.source == "modrinth" && !t.pinned) {
            let file = key.trim_start_matches(&format!("{folder}/")).to_string();
            let dir = instances::game_dir(inst_id).join(folder);
            let path = if dir.join(&file).exists() { dir.join(&file) } else { dir.join(format!("{file}.disabled")) };
            if let Ok(h) = net::sha1_file(&path) {
                by_hash.insert(h, (file, t.clone()));
            }
        }
        if by_hash.is_empty() {
            continue;
        }
        let mut body = json!({
            "hashes": by_hash.keys().collect::<Vec<_>>(),
            "algorithm": "sha1",
            "game_versions": [inst.mc_version],
        });
        if kind == Kind::Mod {
            body["loaders"] = json!(compatible_loaders(&inst.loader));
        }
        let r: Value = CLIENT.post(format!("{MR}/version_files/update")).json(&body).send().await?.json().await?;
        for (hash, v) in r.as_object().into_iter().flatten() {
            let Some((file, t)) = by_hash.get(hash) else { continue };
            let new_id = v["id"].as_str().unwrap_or_default();
            if !new_id.is_empty() && new_id != t.version_id {
                out.push(Update {
                    file: file.clone(),
                    kind: kind.key().into(),
                    title: t.title.clone(),
                    source: "modrinth".into(),
                    project_id: t.project_id.clone(),
                    new_version_id: new_id.into(),
                    new_version: v["version_number"].as_str().unwrap_or_default().into(),
                });
            }
        }
    }
    if has_cf_key() {
        for (key, t) in meta.iter().filter(|(_, t)| t.source == "curseforge" && !t.pinned) {
            let kind = Kind::parse(&t.kind);
            if let Ok(files) = cf_files(&t.project_id, kind, &inst).await {
                if let Some(f) = files.first() {
                    let fid = f["id"].as_u64().unwrap_or(0).to_string();
                    if fid != t.version_id {
                        out.push(Update {
                            file: key.split_once('/').map(|x| x.1).unwrap_or(key).to_string(),
                            kind: t.kind.clone(),
                            title: t.title.clone(),
                            source: "curseforge".into(),
                            project_id: t.project_id.clone(),
                            new_version_id: fid,
                            new_version: f["displayName"].as_str().unwrap_or_default().into(),
                        });
                    }
                }
            }
        }
    }
    Ok(out)
}

// ---------------- Presets ----------------

pub fn performance_slugs(loader: &str) -> Vec<&'static str> {
    match loader {
        "fabric" | "quilt" => {
            let mut v = vec![
                "sodium", "lithium", "ferrite-core", "immediatelyfast", "entityculling", "modernfix",
                "sodium-extra", "reeses-sodium-options", "modmenu",
                // No Dynamic FPS: the game limits FPS when minimized or AFK by itself since 1.21.2,
                // and on tiling Wayland compositors it can take a focused window for an unfocused one.
                // Measured 2026-10-29 (walking through new terrain, 1.21.11): with these the 1% lows
                // went up by half and hitches over 50 ms dropped five times, level with OneClient,
                // whose performance bundle they come from. Ones without a build for the game version
                // are skipped.
                "c2me-fabric", "scalablelux", "ixeris", "gnetum", "vmp-fabric", "asynclogger",
                "particle-core", "cull-fewer-leaves", "structure-layout-optimizer", "zfastnoise",
                "fastquit", "forcecloseworldloadingscreen", "better-mipmaps", "optipainting-reloaded",
                "sciophobia", "obe",
            ];
            if cfg!(windows) {
                v.push("windows-priority-boost");
            }
            if cfg!(target_os = "linux") {
                v.push("fix-keyboard-on-linux");
            }
            v
        }
        "neoforge" => vec!["sodium", "ferrite-core", "immediatelyfast", "entityculling", "modernfix"],
        "forge" => vec!["embeddium", "ferrite-core", "immediatelyfast", "entityculling", "modernfix"],
        _ => vec![],
    }
}

/// Mods of one template pack (packs can be combined; performance mods are a separate toggle).
pub fn pack_slugs(pack: &str, loader: &str) -> Vec<&'static str> {
    // 1.8.9 PvP (BedWars, SkyWars, Duels…) runs on Forge 1.8.9. Mostly Polyfrost mods, which pull
    // in OneConfig themselves. OptiFine isn't downloadable via any API — the UI links to it.
    if pack == "pvp18" {
        return if loader == "forge" {
            vec![
                "patcher", "animatium-legacy", "polysprint", "hippo-keystrokes|keystrokes", "crosshair", "hitbox",
                "damagetint", "hytils", "levelhead", "chatting", "vanillahud", "freelook-oneconfig",
                "entityculling", "crashpatch", "replaymod",
            ]
        } else {
            vec![]
        };
    }
    if !matches!(loader, "fabric" | "quilt") {
        return vec![];
    }
    match pack {
        "cinematic" => vec![
            "iris", "flashback", "distanthorizons", "zoomify", "freecam", "no-chat-reports", "continuity", "lambdynamiclights",
            "simple-voice-chat",
        ],
        // Mods of uku's PvP modpack (modrinth.com/modpack/ukupvp) without its resource pack,
        // menu theme (FancyMenu), bundled configs (YOSBR) or CraftPresence, which would fight
        // the launcher's own Discord status. `a|b` = install the first one that exists.
        "pvp" => vec![
            "sodium", "sodium-extra", "reeses-sodium-options", "lithium", "ferrite-core", "immediatelyfast",
            "entityculling", "dynamic-fps", "modmenu",
            "appleskin", "betterhurtcam", "bettershields", "gamma-utils", "zoomify", "xaeros-minimap",
            "ukus-armor-hud", "potioncounter", "totemcounter", "tiertagger", "numeral-ping", "why-am-i-on-fire",
            "whoami", "nowheel", "shulkerboxtooltip", "no-chat-reports", "viafabricplus",
            "in-game-account-switcher", "particle-core", "scalablelux", "ixeris", "better-block-entities",
            "cull-fewer-leaves|moreculling", "cubes-without-borders", "simple-voice-chat",
        ],
        "smp" => vec!["iris", "zoomify", "appleskin", "no-chat-reports", "xaeros-minimap", "jade", "mouse-tweaks", "simple-voice-chat"],
        _ => vec![],
    }
}

/// Everything to install for a set of packs, without duplicates.
pub fn template_slugs(packs: &[String], loader: &str, optimize: bool) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = if optimize { performance_slugs(loader) } else { vec![] };
    for p in packs {
        for s in pack_slugs(p, loader) {
            if !v.contains(&s) {
                v.push(s);
            }
        }
    }
    v
}
