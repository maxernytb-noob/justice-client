//! World backups and instance snapshots (mods + configs) with restore.

use crate::instances;
use crate::settings;
use anyhow::{bail, Result};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

fn backups_dir(inst: &str) -> PathBuf {
    settings::data_dir().join("backups").join(inst)
}

pub fn zip_paths(entries: &[(PathBuf, String)], dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest.parent().unwrap())?;
    let tmp = dest.with_extension("part");
    let mut z = zip::ZipWriter::new(std::io::BufWriter::new(std::fs::File::create(&tmp)?));
    let opts = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated).compression_level(Some(3));
    fn add(z: &mut zip::ZipWriter<std::io::BufWriter<std::fs::File>>, p: &Path, name: &str, opts: SimpleFileOptions) -> Result<()> {
        if p.is_dir() {
            for e in std::fs::read_dir(p)?.flatten() {
                let child = format!("{name}/{}", e.file_name().to_string_lossy());
                add(z, &e.path(), &child, opts)?;
            }
        } else if p.exists() {
            // session.lock is held open by a running game; it's useless in a backup anyway.
            if p.file_name().map(|n| n == "session.lock").unwrap_or(false) {
                return Ok(());
            }
            // Jars, zips and pictures are compressed already: storing them is much faster and barely bigger.
            let ext = p.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            let opts = if matches!(ext.as_str(), "jar" | "zip" | "png" | "jpg" | "ogg" | "mp4" | "gz" | "mca") {
                opts.compression_method(zip::CompressionMethod::Stored).compression_level(None)
            } else {
                opts
            };
            z.start_file(name, opts)?;
            std::io::copy(&mut std::fs::File::open(p)?, z)?;
        }
        Ok(())
    }
    for (p, name) in entries {
        add(&mut z, p, name, opts)?;
    }
    z.finish()?.flush()?;
    std::fs::rename(tmp, dest)?;
    Ok(())
}

pub fn unzip_into(zip_path: &Path, dest: &Path) -> Result<()> {
    let mut z = zip::ZipArchive::new(std::fs::File::open(zip_path)?)?;
    z.extract(dest)?;
    Ok(())
}

fn stamp() -> String {
    chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string()
}

pub fn backup_world(inst: &str, world: &str) -> Result<String> {
    let src = instances::game_dir(inst).join("saves").join(world);
    if !src.is_dir() {
        bail!(l!("Svět {world} neexistuje", "World {world} doesn't exist"));
    }
    let dest = backups_dir(inst).join("worlds").join(format!("{world}__{}.zip", stamp()));
    zip_paths(&[(src, world.to_string())], &dest)?;
    Ok(dest.to_string_lossy().into_owned())
}

pub fn backup_all_worlds(inst: &str) -> Result<()> {
    for w in worlds(inst) {
        backup_world(inst, &w.name)?;
    }
    Ok(())
}

pub fn snapshot(inst: &str, label: &str) -> Result<String> {
    let g = instances::game_dir(inst);
    let dest = backups_dir(inst).join("snapshots").join(format!("{label}__{}.zip", stamp()));
    zip_paths(
        &[
            (g.join("mods"), "mods".into()),
            (g.join("config"), "config".into()),
            (g.join(".justice"), ".justice".into()),
            (g.join("options.txt"), "options.txt".into()),
            (instances::dir(inst).join("instance.json"), "instance.json".into()),
        ],
        &dest,
    )?;
    Ok(dest.to_string_lossy().into_owned())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Backup {
    pub file: String,
    pub kind: String,
    pub label: String,
    pub created: String,
    pub size: u64,
}

pub fn list(inst: &str) -> Vec<Backup> {
    let mut out = vec![];
    for kind in ["worlds", "snapshots"] {
        for e in std::fs::read_dir(backups_dir(inst).join(kind)).into_iter().flatten().flatten() {
            let name = e.file_name().to_string_lossy().trim_end_matches(".zip").to_string();
            let (label, created) = name.split_once("__").map(|(a, b)| (a.to_string(), b.to_string())).unwrap_or((name.clone(), String::new()));
            out.push(Backup {
                file: e.path().to_string_lossy().into_owned(),
                kind: if kind == "worlds" { "world".into() } else { "snapshot".into() },
                label,
                created,
                size: e.metadata().map(|m| m.len()).unwrap_or(0),
            });
        }
    }
    out.sort_by(|a, b| b.created.cmp(&a.created));
    out
}

fn inside_backups(inst: &str, file: &str) -> Result<PathBuf> {
    let p = PathBuf::from(file);
    if !p.starts_with(backups_dir(inst)) || !p.exists() {
        bail!(l!("Záloha neexistuje", "That backup doesn't exist"));
    }
    Ok(p)
}

pub fn restore(inst: &str, file: &str) -> Result<()> {
    let p = inside_backups(inst, file)?;
    let g = instances::game_dir(inst);
    if p.parent().map(|d| d.ends_with("worlds")).unwrap_or(false) {
        let world = p.file_stem().unwrap().to_string_lossy().split("__").next().unwrap_or_default().to_string();
        let target = g.join("saves").join(&world);
        if target.exists() {
            // Keep the current world as a backup instead of deleting it.
            backup_world(inst, &world)?;
            std::fs::remove_dir_all(&target)?;
        }
        unzip_into(&p, &g.join("saves"))
    } else {
        snapshot(inst, "pred-obnovou")?;
        for d in ["mods", "config", ".justice"] {
            let _ = std::fs::remove_dir_all(g.join(d));
        }
        let tmp = settings::cache_dir().join(format!("restore-{inst}"));
        let _ = std::fs::remove_dir_all(&tmp);
        unzip_into(&p, &tmp)?;
        for e in std::fs::read_dir(&tmp)?.flatten() {
            let name = e.file_name();
            let dst = if name == "instance.json" { instances::dir(inst).join(&name) } else { g.join(&name) };
            if e.path().is_dir() {
                instances::copy_dir(&e.path(), &dst)?;
            } else {
                std::fs::copy(e.path(), dst)?;
            }
        }
        let _ = std::fs::remove_dir_all(&tmp);
        Ok(())
    }
}

pub fn delete(inst: &str, file: &str) -> Result<()> {
    std::fs::remove_file(inside_backups(inst, file)?)?;
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct World {
    pub name: String,
    pub path: String,
    pub modified: i64,
    pub size: u64,
    pub icon: Option<String>,
}

fn dir_size(p: &Path) -> u64 {
    std::fs::read_dir(p)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| if e.path().is_dir() { dir_size(&e.path()) } else { e.metadata().map(|m| m.len()).unwrap_or(0) })
        .sum()
}

pub fn worlds(inst: &str) -> Vec<World> {
    let mut out: Vec<World> = std::fs::read_dir(instances::game_dir(inst).join("saves"))
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().join("level.dat").exists())
        .map(|e| {
            let icon = e.path().join("icon.png");
            World {
                name: e.file_name().to_string_lossy().into_owned(),
                path: e.path().to_string_lossy().into_owned(),
                modified: e
                    .path()
                    .join("level.dat")
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
                size: dir_size(&e.path()),
                icon: icon.exists().then(|| icon.to_string_lossy().into_owned()),
            }
        })
        .collect();
    out.sort_by(|a, b| b.modified.cmp(&a.modified));
    out
}
