//! Official key art for Minecraft versions, from Mojang's launcher content feed (the same one the
//! official launcher uses): a square picture per release, the drop's name ("Chaos Cubed"), and a wide
//! banner when the drop had a "Play …" news post. Hotfixes take their drop's art.
//! Those are small, so drops we know also get full-HD key art hosted on our site (`hd`); the list
//! lives in art.json there, so a new drop doesn't need a launcher update.

use crate::net;
use anyhow::Result;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::Mutex;

const BASE: &str = "https://launchercontent.mojang.com";
const HD_BASE: &str = "https://justiceclient.online/art/";
/// Used when art.json can't be fetched: drop name (lowercase) -> file in HD_BASE.
const HD_BUILTIN: &[(&str, &str)] = &[
    ("wilderness bound", "wilderness-bound.jpg"),
    ("chaos cubed", "chaos-cubed.jpg"),
    ("tiny takeover", "tiny-takeover.jpg"),
    ("mounts of mayhem", "mounts-of-mayhem.jpg"),
    ("the copper age", "copper-age.jpg"),
];

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Art {
    /// Drop name, e.g. "Chaos Cubed"; empty when the notes don't name one.
    pub name: String,
    /// Square picture (540×540).
    pub image: String,
    /// Wide banner (772×350) when there is one.
    pub wide: Option<String>,
    /// Full-HD key art (1920×1080) when we have one for this drop.
    pub hd: Option<String>,
}

/// Drop name (lowercase) -> HD picture URL; art.json values may be file names or full URLs.
async fn hd_map() -> HashMap<String, String> {
    let url = |f: &str| if f.starts_with("https://") { f.to_string() } else { format!("{HD_BASE}{f}") };
    let mut m: HashMap<String, String> = HD_BUILTIN.iter().map(|(n, f)| (n.to_string(), url(f))).collect();
    if let Ok(Value::Object(o)) = net::get_json(&format!("{HD_BASE}art.json")).await {
        for (k, v) in o {
            if let Some(f) = v.as_str() {
                m.insert(k.to_lowercase(), url(f));
            }
        }
    }
    m
}

static CACHE: Lazy<Mutex<Option<HashMap<String, Art>>>> = Lazy::new(|| Mutex::new(None));

/// "Wilderness Bound is out now…", "here comes the Tiny Takeover drop", "has entered The Copper Age!"
fn drop_name(text: &str) -> Option<String> {
    static RES: Lazy<Vec<Regex>> = Lazy::new(|| {
        [
            r"^(?:Today )?([A-Z][\w' ]+?) (?:is out now|has landed|charges into|is here|arrives)",
            r"has entered (The [A-Z][\w ]+?)[!.]",
            // Capitalised words and small joiners only: "the Chase the Skies drop", "the Tricky Trials Update".
            r"(?:the|our) ([A-Z][\w']*(?: (?:[A-Z][\w']*|of|the|to|and))*) (?:game )?(?:drop|[Uu]pdate)",
        ]
        .iter()
        .map(|r| Regex::new(r).unwrap())
        .collect()
    });
    RES.iter().find_map(|re| re.captures(text).map(|c| c[1].trim().to_string())).filter(|n| n.len() > 3 && !n.starts_with("Minecraft"))
}

async fn load() -> Result<HashMap<String, Art>> {
    let notes = net::get_json(&format!("{BASE}/v2/javaPatchNotes.json")).await?;
    let news = net::get_json(&format!("{BASE}/v2/news.json")).await.unwrap_or(Value::Null);
    let hd = hd_map().await;
    // Wide banners from "Play Chaos Cubed", "The Copper Age drop"… posts, matched by the drop name.
    let banners: Vec<(String, String)> = news["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| Some((n["title"].as_str()?.to_lowercase(), n["newsPageImage"]["url"].as_str()?.to_string())))
        .collect();
    let mut releases: Vec<(String, String, String)> = notes["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["type"] == "release")
        .filter_map(|e| Some((e["version"].as_str()?.to_string(), e["shortText"].as_str().unwrap_or_default().to_string(), e["image"]["url"].as_str()?.to_string())))
        .collect();
    // Oldest first, so a hotfix can take the art of the drop before it.
    releases.reverse();
    let mut out = HashMap::new();
    let mut last: Option<(String, Art)> = None;
    // "1.21.10" -> "1.21", "26.1.2" -> "26.1": hotfixes only inherit within their line.
    let line = |v: &str| v.split('.').take(2).collect::<Vec<_>>().join(".");
    for (version, text, image) in releases {
        let hotfix = text.to_lowercase().contains("hotfix");
        let name = if hotfix { None } else { drop_name(&text) };
        let art = match (name, &last) {
            (Some(name), _) => {
                let key = name.to_lowercase();
                let wide = banners.iter().find(|(t, _)| t.contains(&key)).map(|(_, u)| format!("{BASE}{u}"));
                let hd = hd.get(&key).cloned();
                Art { name, image: format!("{BASE}{image}"), wide, hd }
            }
            (None, Some((pv, prev))) if hotfix && line(pv) == line(&version) => prev.clone(),
            _ => Art { name: String::new(), image: format!("{BASE}{image}"), wide: None, hd: None },
        };
        if !art.name.is_empty() {
            last = Some((version.clone(), art.clone()));
        }
        out.insert(version, art);
    }
    Ok(out)
}

/// Version -> art, fetched once per run.
pub async fn all() -> Result<HashMap<String, Art>> {
    let mut c = CACHE.lock().await;
    if let Some(m) = c.as_ref() {
        return Ok(m.clone());
    }
    let m = load().await?;
    *c = Some(m.clone());
    Ok(m)
}
