//! Skin library: save several skins in the launcher and switch any account between them. Skins live
//! as PNGs in `<data>/skin-library/` with a small JSON index; applying one reuses the normal skin path.

use crate::auth;
use crate::settings;
use anyhow::{bail, Result};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SkinEntry {
    pub id: String, // = md5 of the PNG
    pub name: String,
    pub variant: String, // "slim" | "classic"
    #[serde(default)]
    pub added: i64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SkinView {
    #[serde(flatten)]
    pub entry: SkinEntry,
    /// Absolute path to the PNG, for the UI to preview via convertFileSrc.
    pub file: String,
}

fn dir() -> PathBuf {
    settings::data_dir().join("skin-library")
}

fn index_path() -> PathBuf {
    dir().join("index.json")
}

fn load_index() -> Vec<SkinEntry> {
    std::fs::read(index_path()).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
}

fn save_index(v: &[SkinEntry]) -> Result<()> {
    std::fs::create_dir_all(dir())?;
    std::fs::write(index_path(), serde_json::to_vec_pretty(v)?)?;
    Ok(())
}

pub fn list() -> Vec<SkinView> {
    load_index()
        .into_iter()
        .filter(|e| dir().join(format!("{}.png", e.id)).exists())
        .map(|entry| {
            let file = dir().join(format!("{}.png", entry.id)).to_string_lossy().into_owned();
            SkinView { entry, file }
        })
        .collect()
}

/// Store PNG bytes under their hash and add (or rename) the index entry. Returns the entry id.
fn store(bytes: Vec<u8>, name: &str, variant: &str) -> Result<String> {
    auth::check_skin(&bytes)?;
    let id = hex::encode(Md5::digest(&bytes));
    std::fs::create_dir_all(dir())?;
    std::fs::write(dir().join(format!("{id}.png")), &bytes)?;
    let mut idx = load_index();
    let variant = if variant.eq_ignore_ascii_case("slim") { "slim" } else { "classic" };
    let nm = if name.trim().is_empty() { "Skin".to_string() } else { name.trim().to_string() };
    if let Some(e) = idx.iter_mut().find(|e| e.id == id) {
        e.name = nm;
        e.variant = variant.to_string();
    } else {
        idx.push(SkinEntry { id: id.clone(), name: nm, variant: variant.to_string(), added: chrono::Utc::now().timestamp() });
    }
    save_index(&idx)?;
    Ok(id)
}

pub fn add_from_file(path: &str, name: &str, variant: &str) -> Result<String> {
    store(std::fs::read(path)?, name, variant)
}

pub async fn add_from_player(player: &str, name: &str) -> Result<String> {
    let (bytes, variant) = auth::fetch_player_skin(player).await?;
    let nm = if name.trim().is_empty() { player.trim() } else { name.trim() };
    store(bytes, nm, &variant)
}

pub async fn add_current(account_id: &str, name: &str) -> Result<String> {
    let (bytes, variant) = auth::account_skin_bytes(account_id).await?;
    store(bytes, name, &variant)
}

pub async fn apply(account_id: &str, entry_id: &str) -> Result<()> {
    let entry = load_index().into_iter().find(|e| e.id == entry_id).ok_or_else(|| anyhow::anyhow!("skin not in library"))?;
    let bytes = std::fs::read(dir().join(format!("{entry_id}.png")))?;
    auth::apply_skin(account_id, bytes, &entry.variant).await
}

pub fn rename(entry_id: &str, name: &str) -> Result<()> {
    let mut idx = load_index();
    let e = idx.iter_mut().find(|e| e.id == entry_id).ok_or_else(|| anyhow::anyhow!("skin not in library"))?;
    if name.trim().is_empty() {
        bail!(l!("Jméno nemůže být prázdné", "The name can't be empty"));
    }
    e.name = name.trim().to_string();
    save_index(&idx)
}

pub fn remove(entry_id: &str) -> Result<()> {
    let mut idx = load_index();
    idx.retain(|e| e.id != entry_id);
    let _ = std::fs::remove_file(dir().join(format!("{entry_id}.png")));
    save_index(&idx)
}
