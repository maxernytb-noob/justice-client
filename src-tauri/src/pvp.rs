//! PvP totals reported by Justice Mod (player kills, deaths, best combo) for the profile and the
//! friends leaderboard. The mod sends what happened since its last report; we keep the sums.

use crate::{net, settings};
use serde_json::{json, Value};
use std::sync::Mutex;

static LOCK: Mutex<()> = Mutex::new(());

fn path() -> std::path::PathBuf {
    settings::data_dir().join("pvp.json")
}

pub fn get() -> Value {
    let v = net::read_json(&path()).unwrap_or(Value::Null);
    let n = |k: &str| v[k].as_u64().unwrap_or(0);
    json!({ "kills": n("kills"), "deaths": n("deaths"), "bestCombo": n("bestCombo"), "hits": n("hits") })
}

/// `delta` = {kills, deaths, hits, bestCombo} since the mod's last report.
pub fn add(delta: &Value) -> anyhow::Result<Value> {
    let _g = LOCK.lock().unwrap();
    // One report covers a minute at most; anything bigger is nonsense.
    let d = |k: &str, max: u64| delta[k].as_u64().unwrap_or(0).min(max);
    let mut t = get();
    for (k, max) in [("kills", 500), ("deaths", 500), ("hits", 20_000)] {
        t[k] = json!(t[k].as_u64().unwrap_or(0) + d(k, max));
    }
    t["bestCombo"] = json!(t["bestCombo"].as_u64().unwrap_or(0).max(d("bestCombo", 1000)));
    net::write_json(&path(), &t)?;
    Ok(t)
}
