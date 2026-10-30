//! Discord Rich Presence (local IPC pipe) and OBS WebSocket v5.

use crate::backups;
use crate::instances::Instance;
use crate::platform::{discord_connect_raw, IpcStream};
use crate::settings;
use anyhow::{anyhow, bail, Result};
use base64::Engine;
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

pub async fn game_started(inst: &Instance, server: Option<&str>) {
    let s = settings::get();
    if s.discord_rpc {
        let name = &inst.name;
        let details = if inst.recording_mode { l!("Natáčí · {name}", "Recording · {name}") } else { l!("Hraje {name}", "Playing {name}") };
        let state = match (server, s.streamer_mode) {
            (Some(_), false) => l!("Minecraft {} · na serveru", "Minecraft {} · on a server", inst.mc_version),
            _ => format!("Minecraft {} · {}", inst.mc_version, loader_label(&inst.loader)),
        };
        let _ = tokio::task::spawn_blocking(move || discord_set(Some((details, state)))).await;
    }
    if s.obs_enabled {
        let mut reqs = vec![];
        if !s.obs_scene.trim().is_empty() {
            reqs.push(("SetCurrentProgramScene", json!({"sceneName": s.obs_scene.trim()})));
        }
        if s.obs_auto_record {
            reqs.push(("StartRecord", json!({})));
        }
        if !reqs.is_empty() {
            let _ = obs_requests(&reqs).await;
        }
    }
}

pub async fn game_stopped(inst: &Instance) {
    let s = settings::get();
    let _ = tokio::task::spawn_blocking(|| discord_set(None)).await;
    if s.obs_enabled && s.obs_auto_record {
        let _ = obs_requests(&[("StopRecord", json!({}))]).await;
    }
    if s.backup_on_exit {
        let id = inst.id.clone();
        let _ = tokio::task::spawn_blocking(move || backups::backup_all_worlds(&id)).await;
    }
}

fn loader_label(l: &str) -> &str {
    match l {
        "fabric" => "Fabric",
        "quilt" => "Quilt",
        "forge" => "Forge",
        "neoforge" => "NeoForge",
        _ => "Vanilla",
    }
}

// ---------------- Discord ----------------

static DISCORD: Lazy<Mutex<Option<Box<dyn IpcStream>>>> = Lazy::new(|| Mutex::new(None));

fn frame(op: u32, payload: &Value) -> Vec<u8> {
    let body = payload.to_string().into_bytes();
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

fn read_frame(f: &mut dyn IpcStream) -> Result<Value> {
    let mut hdr = [0u8; 8];
    f.read_exact(&mut hdr)?;
    let len = u32::from_le_bytes(hdr[4..8].try_into().unwrap()) as usize;
    let mut body = vec![0u8; len.min(1 << 20)];
    f.read_exact(&mut body)?;
    Ok(serde_json::from_slice(&body).unwrap_or(Value::Null))
}

fn discord_connect() -> Result<Box<dyn IpcStream>> {
    let app_id = settings::discord_app_id();
    let Some(mut f) = discord_connect_raw() else { bail!(l!("Discord neběží", "Discord isn't running")) };
    f.write_all(&frame(0, &json!({"v": 1, "client_id": app_id})))?;
    read_frame(f.as_mut())?;
    Ok(f)
}

fn discord_set(activity: Option<(String, String)>) -> Result<()> {
    let mut guard = DISCORD.lock().unwrap();
    if activity.is_none() && guard.is_none() {
        return Ok(());
    }
    if guard.is_none() {
        *guard = Some(discord_connect()?);
    }
    let f = guard.as_mut().unwrap();
    let act = activity.map(|(details, state)| {
        json!({
            "details": details,
            "state": state,
            "timestamps": {"start": chrono::Utc::now().timestamp()},
            "assets": {"large_image": "https://justiceclient.online/icon.png", "large_text": "Justice Client"}
        })
    });
    let clear = act.is_none();
    let msg = json!({
        "cmd": "SET_ACTIVITY",
        "args": {"pid": std::process::id(), "activity": act},
        "nonce": uuid::Uuid::new_v4().to_string()
    });
    let res = f.write_all(&frame(1, &msg)).and_then(|_| read_frame(f.as_mut()).map(|_| ()).map_err(std::io::Error::other));
    if res.is_err() || clear {
        *guard = None;
    }
    res.map_err(|e| anyhow!(e))
}

// ---------------- OBS ----------------

fn obs_auth(password: &str, salt: &str, challenge: &str) -> String {
    let b64 = base64::engine::general_purpose::STANDARD;
    let secret = b64.encode(Sha256::digest(format!("{password}{salt}").as_bytes()));
    b64.encode(Sha256::digest(format!("{secret}{challenge}").as_bytes()))
}

pub async fn obs_requests(reqs: &[(&str, Value)]) -> Result<Vec<Value>> {
    let s = settings::get();
    let (mut ws, _) = tokio::time::timeout(std::time::Duration::from_secs(4), tokio_tungstenite::connect_async(s.obs_url.trim()))
        .await
        .map_err(|_| anyhow!(l!("OBS neodpovídá — je spuštěný a má zapnutý WebSocket server?", "OBS isn't responding — is it running with the WebSocket server enabled?")))??;

    let hello: Value = loop {
        match ws.next().await {
            Some(Ok(Message::Text(t))) => break serde_json::from_str(&t)?,
            Some(Ok(_)) => continue,
            Some(Err(e)) => bail!(e),
            None => bail!(l!("OBS ukončil spojení", "OBS closed the connection")),
        }
    };
    let mut identify = json!({"op": 1, "d": {"rpcVersion": 1}});
    if let Some(a) = hello["d"]["authentication"].as_object() {
        let auth = obs_auth(
            &s.obs_password,
            a.get("salt").and_then(|v| v.as_str()).unwrap_or_default(),
            a.get("challenge").and_then(|v| v.as_str()).unwrap_or_default(),
        );
        identify["d"]["authentication"] = json!(auth);
    }
    ws.send(Message::Text(identify.to_string())).await?;

    let mut out = vec![];
    let mut identified = false;
    let mut sent = false;
    while out.len() < reqs.len() {
        if identified && !sent {
            for (i, (ty, data)) in reqs.iter().enumerate() {
                let msg = json!({"op": 6, "d": {"requestType": ty, "requestId": i.to_string(), "requestData": data}});
                ws.send(Message::Text(msg.to_string())).await?;
            }
            sent = true;
        }
        let msg = match tokio::time::timeout(std::time::Duration::from_secs(5), ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => serde_json::from_str::<Value>(&t)?,
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => bail!(l!("OBS odmítl připojení — zkontroluj heslo.", "OBS refused the connection — check the password.")),
            Ok(Some(Ok(_))) => continue,
            Ok(Some(Err(e))) => bail!(e),
            Err(_) => bail!(l!("OBS neodpověděl", "OBS didn't respond")),
        };
        match msg["op"].as_u64() {
            Some(2) => identified = true,
            Some(7) => out.push(msg["d"].clone()),
            _ => {}
        }
    }
    let _ = ws.close(None).await;
    Ok(out)
}

pub async fn obs_test() -> Result<Vec<String>> {
    let r = obs_requests(&[("GetSceneList", json!({}))]).await?;
    Ok(r.first()
        .and_then(|d| d["responseData"]["scenes"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .rev()
        .filter_map(|s| s["sceneName"].as_str().map(String::from))
        .collect())
}
