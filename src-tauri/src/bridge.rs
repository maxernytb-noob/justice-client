//! Local bridge for Justice Mod: a tiny HTTP server on 127.0.0.1 so the in-game chat and world
//! sharing go through the launcher (which holds the friends session). The port and a random secret
//! are written into each instance's `config/justicemod-bridge.json` at launch; every request must
//! carry the secret, so other local programs can't use it.

use crate::{social, world};
use anyhow::Result;
use once_cell::sync::OnceCell;
use serde_json::{json, Value};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

static ADDR: OnceCell<(u16, String)> = OnceCell::new();

/// (port, secret) once the bridge is running.
pub fn info() -> Option<(u16, String)> {
    ADDR.get().cloned()
}

pub fn start() {
    tauri::async_runtime::spawn(async {
        let listener = match TcpListener::bind(("127.0.0.1", 0)).await {
            Ok(l) => l,
            Err(e) => return lwarn!("bridge: cannot listen: {e}"),
        };
        let port = listener.local_addr().map(|a| a.port()).unwrap_or(0);
        let secret = uuid::Uuid::new_v4().simple().to_string();
        let _ = ADDR.set((port, secret.clone()));
        linfo!("bridge: listening on 127.0.0.1:{port}");
        loop {
            let Ok((stream, _)) = listener.accept().await else { continue };
            let secret = secret.clone();
            tauri::async_runtime::spawn(async move {
                if let Err(e) = handle(stream, &secret).await {
                    ldebug!("bridge: request failed: {e:#}");
                }
            });
        }
    });
}

async fn handle(mut s: TcpStream, secret: &str) -> Result<()> {
    // Read the head (small requests only).
    let mut buf = Vec::with_capacity(2048);
    let mut chunk = [0u8; 2048];
    let head_end = loop {
        let n = tokio::time::timeout(Duration::from_secs(10), s.read(&mut chunk)).await??;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break i;
        }
        if buf.len() > 64 * 1024 {
            return Ok(());
        }
    };
    let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
    let mut lines = head.lines();
    let mut first = lines.next().unwrap_or("").split_whitespace();
    let (method, target) = (first.next().unwrap_or("").to_string(), first.next().unwrap_or("").to_string());
    let mut len = 0usize;
    let mut auth = String::new();
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            match k.trim().to_ascii_lowercase().as_str() {
                "content-length" => len = v.trim().parse().unwrap_or(0).min(256 * 1024),
                "x-justice-secret" => auth = v.trim().to_string(),
                _ => {}
            }
        }
    }
    let mut body = buf[head_end + 4..].to_vec();
    while body.len() < len {
        let n = s.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&chunk[..n]);
    }
    let (status, reply) = if auth != secret {
        (401, json!({ "error": "bad secret" }))
    } else {
        let body: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        match route(&method, &target, body).await {
            Ok(v) => (200, v),
            Err(e) => (400, json!({ "error": format!("{e:#}") })),
        }
    };
    let text = reply.to_string();
    let resp = format!(
        "HTTP/1.1 {status} {}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
        if status == 200 { "OK" } else { "Error" },
        text.len()
    );
    s.write_all(resp.as_bytes()).await?;
    Ok(())
}

async fn route(method: &str, target: &str, body: Value) -> Result<Value> {
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let q = |k: &str| query.split('&').find_map(|p| p.strip_prefix(&format!("{k}="))).unwrap_or("").to_string();
    Ok(match (method, path) {
        ("GET", "/state") => {
            let me = social::me();
            json!({
                "signedIn": me.is_some(),
                "uuid": me.as_ref().map(|m| m.0.clone()),
                "name": me.as_ref().map(|m| m.1.clone()),
                "world": world::hosted().map(|h| json!({ "id": h.id, "name": h.name })),
            })
        }
        ("GET", "/friends") => social::get("/friends").await?,
        // Now playing (Windows media controls) for the mod's Now Playing HUD; the cover stays in the launcher.
        ("GET", "/media") => {
            let st = tauri::async_runtime::spawn_blocking(|| crate::media::state(None, Some("*"))).await??;
            match st {
                Some(mut m) => {
                    m.thumb = None;
                    serde_json::to_value(m)?
                }
                None => json!({ "playing": false, "title": "" }),
            }
        }
        ("POST", "/media/command") => {
            let cmd = body["command"].as_str().unwrap_or_default().to_string();
            let value = body["value"].as_f64();
            tauri::async_runtime::spawn_blocking(move || crate::media::command(&cmd, value, None)).await??;
            json!({ "ok": true })
        }
        // uuid -> Justice cape for everyone who picked one (the mod draws them).
        ("GET", "/capes") => social::capes().await,
        // uuid -> Justice pet for everyone who picked one (the mod draws them).
        ("GET", "/pets") => social::pets().await,
        // uuid -> {url, slim, file?} Justice skins of offline players (the mod draws them).
        ("GET", "/skins") => social::skins().await,
        ("GET", "/party") => social::get("/party").await?,
        ("POST", "/party/accept") => social::post("/party/accept", json!({ "id": body["id"] })).await?,
        ("POST", "/party/decline") => social::post("/party/decline", json!({ "id": body["id"] })).await?,
        ("POST", "/party/leave") => social::post("/party/leave", json!({})).await?,
        ("POST", "/party/invite") => social::post("/party/invite", json!({ "uuid": body["uuid"] })).await?,
        ("POST", "/party/warp") => social::warp_here().await?,
        ("POST", "/presence/server") => {
            social::set_current_server(body["server"].as_str().filter(|s| !s.is_empty()));
            json!({ "ok": true })
        }
        // PvP since the mod's last report; kept as totals for the profile and leaderboard.
        ("POST", "/stats") => crate::pvp::add(&body)?,
        ("GET", p) if p.starts_with("/messages/") => social::get(p).await?,
        ("POST", "/send") => {
            social::post("/messages", json!({ "to": body["to"], "kind": "text", "body": body["body"] })).await?
        }
        ("GET", "/events") => {
            let after: u64 = q("after").parse().unwrap_or(0);
            let (seq, events) = social::events_after(after, Duration::from_secs(20)).await;
            json!({ "seq": seq, "events": events })
        }
        ("POST", "/world/host") => {
            let port = body["port"].as_u64().unwrap_or(0) as u16;
            if port == 0 {
                anyhow::bail!("missing port");
            }
            world::host(port, body["name"].as_str().unwrap_or("World"), body["mc"].as_str().unwrap_or(""), body["loader"].as_str().unwrap_or("")).await?
        }
        ("POST", "/world/invite") => world::invite(body["uuid"].as_str().unwrap_or_default()).await?,
        // The game asks while a player logs into our shared world: offline Justice accounts skip Mojang.
        ("GET", "/world/offline-guest") => json!({ "offline": world::is_offline_guest(&q("name")) }),
        ("POST", "/world/stop") => {
            world::stop().await;
            json!({ "ok": true })
        }
        ("POST", "/world/join") => json!({ "port": world::join(body["world"].as_str().unwrap_or_default()).await? }),
        ("POST", "/open") => {
            social::open_shared_clip(body["url"].as_str().unwrap_or_default()).await?;
            json!({ "ok": true })
        }
        // Installs a modpack sent in chat. Downloading takes a while, so it runs in the background and
        // reports back as a `pack-installed` social event.
        ("POST", "/pack/install") => {
            let code = body["code"].as_str().unwrap_or_default().to_string();
            let name = body["name"].as_str().unwrap_or_default().to_string();
            if code.is_empty() {
                anyhow::bail!("missing code");
            }
            tauri::async_runtime::spawn(async move {
                let r = crate::packs::import_code(&code).await;
                if let Err(e) = &r {
                    lwarn!("bridge: modpack install failed: {e:#}");
                }
                crate::events::emit("instances-changed", ());
                social::emit(json!({
                    "type": "pack-installed",
                    "request": name,
                    "name": r.as_ref().map(|i| i.name.clone()).unwrap_or_else(|_| name.clone()),
                    "ok": r.is_ok(),
                    "error": r.err().map(|e| format!("{e:#}")),
                }));
            });
            json!({ "ok": true })
        }
        _ => anyhow::bail!("unknown endpoint {method} {path}"),
    })
}
