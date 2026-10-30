//! Fabric / Quilt / Forge / NeoForge: listing versions and installing them into the meta dir.

use crate::events;
use crate::java;
use crate::minecraft;
use crate::net::{self, Job};
use crate::settings;
use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::io::Read;


#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
}

pub async fn list(loader: &str, game: &str) -> Result<Vec<LoaderVersion>> {
    match loader {
        "fabric" => {
            let v = net::get_json(&format!("https://meta.fabricmc.net/v2/versions/loader/{game}")).await?;
            Ok(v.as_array()
                .into_iter()
                .flatten()
                .map(|e| LoaderVersion {
                    version: e["loader"]["version"].as_str().unwrap_or_default().into(),
                    stable: e["loader"]["stable"].as_bool().unwrap_or(false),
                })
                .collect())
        }
        "quilt" => {
            let v = net::get_json(&format!("https://meta.quiltmc.org/v3/versions/loader/{game}")).await?;
            Ok(v.as_array()
                .into_iter()
                .flatten()
                .map(|e| {
                    let ver = e["loader"]["version"].as_str().unwrap_or_default().to_string();
                    LoaderVersion { stable: !ver.contains('-'), version: ver }
                })
                .collect())
        }
        "forge" => {
            let cache = settings::cache_dir().join("forge-maven-metadata.json");
            let meta = net::get_json_cached(
                "https://files.minecraftforge.net/net/minecraftforge/forge/maven-metadata.json",
                &cache,
                3600,
            )
            .await?;
            let promos = net::get_json(
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
            )
            .await
            .unwrap_or(Value::Null);
            let rec = promos["promos"][format!("{game}-recommended")].as_str().unwrap_or_default();
            let mut out: Vec<LoaderVersion> = meta[game]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .map(|full| {
                    let short = full.strip_prefix(&format!("{game}-")).unwrap_or(full);
                    LoaderVersion { version: full.into(), stable: short.starts_with(rec) && !rec.is_empty() }
                })
                .collect();
            out.reverse();
            Ok(out)
        }
        "neoforge" => {
            let v = net::get_json(
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
            )
            .await?;
            let prefix = neoforge_prefix(game);
            let mut out: Vec<LoaderVersion> = v["versions"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|v| v.as_str())
                .filter(|v| v.starts_with(&prefix))
                .map(|v| LoaderVersion { version: v.into(), stable: !v.contains("beta") })
                .collect();
            out.reverse();
            Ok(out)
        }
        _ => Ok(vec![]),
    }
}

/// MC `1.21.1` -> NeoForge `21.1.`, `1.21` -> `21.0.`, year-based `26.1` -> `26.1.`
fn neoforge_prefix(game: &str) -> String {
    match game.strip_prefix("1.") {
        Some(rest) => {
            let parts: Vec<&str> = rest.split('.').collect();
            format!("{}.{}.", parts[0], parts.get(1).unwrap_or(&"0"))
        }
        None => format!("{game}."),
    }
}

/// Installs the loader (if needed) and returns the version id to launch.
pub async fn install(loader: &str, game: &str, loader_version: Option<&str>, task: &str) -> Result<String> {
    minecraft::ensure_vanilla_json(game).await?;
    let pick = |list: Vec<LoaderVersion>| -> Result<String> {
        list.iter()
            .find(|v| v.stable)
            .or(list.first())
            .map(|v| v.version.clone())
            .ok_or_else(|| anyhow!(l!("{loader} pro {game} neexistuje", "{loader} doesn't exist for {game}")))
    };
    match loader {
        "vanilla" | "" => Ok(game.to_string()),
        "fabric" | "quilt" => {
            let lv = match loader_version {
                Some(v) if !v.is_empty() => v.to_string(),
                _ => pick(list(loader, game).await?)?,
            };
            let url = if loader == "fabric" {
                format!("https://meta.fabricmc.net/v2/versions/loader/{game}/{lv}/profile/json")
            } else {
                format!("https://meta.quiltmc.org/v3/versions/loader/{game}/{lv}/profile/json")
            };
            let profile = net::get_json(&url).await?;
            let id = profile["id"].as_str().ok_or_else(|| anyhow!("bad profile"))?.to_string();
            net::write_json(&minecraft::version_json_path(&id), &profile)?;
            Ok(id)
        }
        "forge" | "neoforge" => {
            let lv = match loader_version {
                Some(v) if !v.is_empty() => v.to_string(),
                // For 1.8.9 everyone plays on the last build (2318); "recommended" is far older.
                _ if loader == "forge" && game == "1.8.9" => list(loader, game)
                    .await?
                    .first()
                    .map(|v| v.version.clone())
                    .ok_or_else(|| anyhow!(l!("Forge pro 1.8.9 nenalezen", "Forge for 1.8.9 not found")))?,
                _ => pick(list(loader, game).await?)?,
            };
            install_forge_like(loader, game, &lv, task).await
        }
        other => bail!(l!("Neznámý loader {other}", "Unknown loader {other}")),
    }
}

/// Forge ≤1.12 installers: the version JSON sits inside install_profile.json ("versionInfo") and
/// the universal jar is shipped in the installer — no processors to run, so we unpack it ourselves.
fn install_legacy_forge(installer: &std::path::Path, game: &str) -> Result<String> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(installer)?)?;
    let mut s = String::new();
    z.by_name("install_profile.json")
        .map_err(|_| anyhow!(l!("Tenhle Forge installer neznám (chybí install_profile.json)", "Unknown Forge installer (install_profile.json is missing)")))?
        .read_to_string(&mut s)?;
    let profile: Value = serde_json::from_str(&s)?;
    let mut version = profile["versionInfo"].clone();
    let id = version["id"].as_str().ok_or_else(|| anyhow!("install_profile bez id"))?.to_string();
    if version["inheritsFrom"].is_null() {
        version["inheritsFrom"] = Value::String(game.to_string());
    }
    if version["jar"].is_null() {
        version["jar"] = Value::String(game.to_string());
    }

    let coord = profile["install"]["path"].as_str().ok_or_else(|| anyhow!("install_profile bez cesty k Forge"))?;
    let file = profile["install"]["filePath"].as_str().ok_or_else(|| anyhow!("install_profile bez souboru Forge"))?;
    let dest = settings::meta_dir().join("libraries").join(minecraft::maven_path(coord));
    if !dest.exists() {
        std::fs::create_dir_all(dest.parent().unwrap())?;
        let mut buf = vec![];
        z.by_name(file)?.read_to_end(&mut buf)?;
        std::fs::write(&dest, buf)?;
    }
    net::write_json(&minecraft::version_json_path(&id), &version)?;
    Ok(id)
}

async fn install_forge_like(loader: &str, game: &str, lv: &str, task: &str) -> Result<String> {
    let url = if loader == "forge" {
        format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{lv}/forge-{lv}-installer.jar")
    } else {
        format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{lv}/neoforge-{lv}-installer.jar")
    };
    let installer = settings::cache_dir().join("installers").join(format!("{loader}-{lv}-installer.jar"));
    events::progress(task, &l!("Stahuji {loader} installer", "Downloading the {loader} installer"), 0, 1);
    net::download(&Job::new(url, installer.clone())).await?;

    let id = {
        let f = std::fs::File::open(&installer)?;
        let mut z = zip::ZipArchive::new(f).context(l!("installer není platný jar", "the installer isn't a valid jar"))?;
        if z.by_name("version.json").is_err() {
            drop(z);
            return install_legacy_forge(&installer, game);
        }
        let mut s = String::new();
        z.by_name("version.json")?.read_to_string(&mut s)?;
        let v: Value = serde_json::from_str(&s)?;
        v["id"].as_str().ok_or_else(|| anyhow!("version.json bez id"))?.to_string()
    };
    if minecraft::version_json_path(&id).exists() {
        return Ok(id);
    }

    let meta = settings::meta_dir();
    std::fs::create_dir_all(&meta)?;
    let profiles = meta.join("launcher_profiles.json");
    if !profiles.exists() {
        std::fs::write(&profiles, r#"{"profiles":{}}"#)?;
    }
    // The installer processors need the vanilla jar and a JVM matching the game.
    let vanilla = minecraft::load_merged(game).await?;
    minecraft::install_files(game, &vanilla, task).await?;
    let javaw = java::ensure(minecraft::java_major(&vanilla), task).await?;
    let java = java::console_java(&javaw);

    events::progress(task, &l!("Instaluji {loader} {lv}", "Installing {loader} {lv}"), 0, 1);
    let mut cmd = tokio::process::Command::new(java);
    cmd.arg("-jar")
        .arg(&installer)
        .arg("--installClient")
        .arg(&meta)
        .current_dir(installer.parent().unwrap());
    crate::platform::hide_window(&mut cmd);
    let out = cmd.output().await?;
    if !out.status.success() || !minecraft::version_json_path(&id).exists() {
        let log = String::from_utf8_lossy(&out.stdout);
        let tail: Vec<&str> = log.lines().rev().take(15).collect();
        let tail = tail.into_iter().rev().collect::<Vec<_>>().join("\n");
        bail!(l!("Instalace {loader} selhala:\n{tail}", "Installing {loader} failed:\n{tail}"));
    }
    Ok(id)
}
