//! Justice Mod: the in-game menu (PvP HUD, zoom, recording tools) shipped inside the launcher and
//! dropped into matching Fabric, Quilt, NeoForge and Forge instances at launch.

use crate::events;
use crate::instances::Instance;
use crate::settings;
use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};
use tauri::Manager;

/// Bundled jar for each supported Minecraft version (patch releases share their line's build).
const BUILDS: &[(&[&str], &str)] = &[
    (&["1.21.9", "1.21.10"], "justice-mod-1.21.10.jar"),
    (&["1.21.11"], "justice-mod-1.21.11.jar"),
    (&["26.1", "26.1.1", "26.1.2"], "justice-mod-26.1.jar"),
    (&["26.2", "26.2.1", "26.2.2"], "justice-mod-26.2.jar"),
    (&["26.3", "26.3.1", "26.3.2"], "justice-mod-26.3.jar"),
];

fn jar_name(mc: &str) -> Option<&'static str> {
    BUILDS.iter().find(|(versions, _)| versions.contains(&mc)).map(|(_, f)| *f)
}
/// File name inside the instance's mods folder; fixed so updates replace it in place.
const INSTALLED_NAME: &str = "justice-mod.jar";

/// NeoForge and Forge get their own build of the same version ("justice-mod-26.3-neoforge.jar").
fn loader_jar(inst: &Instance) -> Option<String> {
    let base = jar_name(&inst.mc_version)?;
    Some(match inst.loader.as_str() {
        "neoforge" | "forge" => base.replace(".jar", &format!("-{}.jar", inst.loader)),
        _ => base.to_string(),
    })
}

fn bundled_jar(inst: &Instance) -> Option<PathBuf> {
    let file = loader_jar(inst)?;
    let file = file.as_str();
    if let Some(res) = events::app().and_then(|a| a.path().resource_dir().ok()).map(|d| d.join("resources").join(file)) {
        if res.exists() {
            return Some(res);
        }
    }
    // Arch package installs it here (see packaging/arch/PKGBUILD).
    #[cfg(target_os = "linux")]
    {
        let sys = PathBuf::from("/usr/lib/justice-client/resources").join(file);
        if sys.exists() {
            return Some(sys);
        }
    }
    // `tauri dev` runs from the source tree.
    let dev = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources").join(file);
    dev.exists().then_some(dev)
}

pub fn supported(inst: &Instance) -> bool {
    matches!(inst.loader.as_str(), "fabric" | "quilt" | "neoforge" | "forge") && jar_name(&inst.mc_version).is_some()
}

fn has_fabric_api(mods: &Path) -> bool {
    std::fs::read_dir(mods).into_iter().flatten().flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_lowercase();
        n.ends_with(".jar") && (n.starts_with("fabric-api") || n.starts_with("quilted-fabric-api") || n.starts_with("qfapi"))
    })
}

/// The mod needs Fabric API; download it when an instance doesn't have it yet.
/// Returns false when it is still missing, in which case the mod is left out rather than crash the game.
async fn ensure_fabric_api(inst: &Instance, mods: &Path) -> bool {
    // The NeoForge and Forge builds don't use Fabric API.
    if !matches!(inst.loader.as_str(), "fabric" | "quilt") || has_fabric_api(mods) {
        return true;
    }
    let slug = if inst.loader == "quilt" { "qsl" } else { "fabric-api" };
    let _ = crate::content::install_many(&inst.id, &[slug], crate::content::Kind::Mod).await;
    has_fabric_api(mods)
}

/// Install, update or remove the mod for this launch, and pass the launcher's look to it.
pub async fn sync(inst: &Instance, game_dir: &Path) -> Result<()> {
    let target = game_dir.join("mods").join(INSTALLED_NAME);
    let wanted = supported(inst) && !inst.no_justice_mod && settings::get().justice_mod && ensure_fabric_api(inst, &game_dir.join("mods")).await;
    if !wanted {
        if target.exists() {
            std::fs::remove_file(&target)?;
        }
        return Ok(());
    }
    let Some(src) = bundled_jar(inst) else { return Ok(()) };
    let same = match (std::fs::read(&src), std::fs::read(&target)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        std::fs::create_dir_all(target.parent().unwrap())?;
        std::fs::copy(&src, &target)?;
    }
    let s = settings::get();
    let cfg = json!({
        "accent": s.accent_color,
        "language": s.language,
        "streamer": s.streamer_mode,
        "launcherVersion": env!("CARGO_PKG_VERSION"),
    });
    // In-game chat and world sharing talk to the launcher through the local bridge.
    if let Some((port, secret)) = crate::bridge::info() {
        let b = json!({ "port": port, "secret": secret });
        let p = game_dir.join("config").join("justicemod-bridge.json");
        std::fs::create_dir_all(p.parent().unwrap())?;
        std::fs::write(p, serde_json::to_vec_pretty(&b)?)?;
    }
    let path = game_dir.join("config").join("justicemod-launcher.json");
    std::fs::create_dir_all(path.parent().unwrap())?;
    std::fs::write(path, serde_json::to_vec_pretty(&cfg)?)?;
    Ok(())
}
