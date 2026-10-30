//! MaxPack helper bot (launcher side). Sends the chat to the VPS proxy (which talks to a free LLM,
//! no key in the launcher), then resolves the bot's picks against Modrinth for the target version and
//! loader so every suggested mod is real and installable. Building a pack reuses the normal installer.

use crate::content::{self, Kind, SearchQuery};
use crate::instances::{self, Instance};
use crate::net;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const PROXY: &str = "https://justiceclient.online/social/modbot";

#[derive(Deserialize)]
pub struct Msg {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModCard {
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub icon_url: String,
    pub downloads: u64,
    pub reason: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatReply {
    pub reply: String,
    pub limited: bool,
    pub mods: Vec<ModCard>,
}

/// Ask MaxPack: relays the conversation to the proxy, then turns its picks into verified mod cards.
pub async fn chat(mc: &str, loader: &str, messages: Vec<Msg>) -> Result<ChatReply> {
    let body = json!({
        "mc": mc,
        "loader": loader,
        "lang": if crate::i18n::is_en() { "en" } else { "cs" },
        "messages": messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
    });
    let r = net::CLIENT.post(PROXY).json(&body).send().await?;
    if !r.status().is_success() {
        anyhow::bail!("MaxPack proxy HTTP {}", r.status());
    }
    let v: Value = r.json().await?;
    let reply = v["reply"].as_str().unwrap_or_default().to_string();
    let limited = v["limited"].as_bool().unwrap_or(false);

    let loader = if loader.is_empty() { "fabric" } else { loader };
    let mut mods: Vec<ModCard> = vec![];
    let mut seen = std::collections::HashSet::new();
    for pick in v["picks"].as_array().cloned().unwrap_or_default() {
        let Some(query) = pick["query"].as_str() else { continue };
        let reason = pick["reason"].as_str().unwrap_or_default().to_string();
        let res = content::search(SearchQuery {
            source: "modrinth",
            query,
            kind: Kind::Mod,
            game_version: mc,
            loader,
            sort: "relevance",
            offset: 0,
            category: "",
        })
        .await;
        if let Ok(res) = res {
            if let Some(h) = res.hits.into_iter().find(|h| seen.insert(h.id.clone())) {
                mods.push(ModCard {
                    id: h.id,
                    slug: h.slug,
                    title: h.title,
                    description: h.description,
                    icon_url: h.icon_url,
                    downloads: h.downloads,
                    reason,
                });
            }
        }
    }
    Ok(ChatReply { reply, limited, mods })
}

/// Build a brand-new instance from chosen mods.
pub async fn build(mc: &str, loader: &str, name: &str, slugs: Vec<String>) -> Result<Instance> {
    let loader = if loader.is_empty() { "fabric".into() } else { loader.to_string() };
    let inst = instances::create(Instance {
        name: if name.trim().is_empty() { format!("MaxPack {mc}") } else { name.to_string() },
        mc_version: mc.to_string(),
        loader,
        icon: "modpack".into(),
        ..Default::default()
    })?;
    let refs: Vec<&str> = slugs.iter().map(String::as_str).collect();
    let _ = content::install_many(&inst.id, &refs, Kind::Mod).await;
    Ok(inst)
}

/// Add chosen mods to an existing instance.
pub async fn add(inst_id: &str, slugs: Vec<String>) -> Result<content::InstallReport> {
    let refs: Vec<&str> = slugs.iter().map(String::as_str).collect();
    Ok(content::install_many(inst_id, &refs, Kind::Mod).await)
}
