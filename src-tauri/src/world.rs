//! Shared singleplayer worlds (like Essential). The host's world is opened to LAN by Justice Mod;
//! this module tunnels it through the Justice relay (WebSockets over HTTPS), so nobody opens ports.
//! Host: a control socket per world, one data socket per guest, piped to the local LAN port.
//! Guest: a local TCP port for Minecraft to connect to, piped to the relay.

use crate::social;
use anyhow::{anyhow, bail, Result};
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::tungstenite::Message;

#[derive(Clone)]
pub struct Hosted {
    pub id: String,
    pub name: String,
    lan_port: u16,
}

static HOSTED: Lazy<Mutex<Option<Hosted>>> = Lazy::new(|| Mutex::new(None));
/// Offline Justice accounts the relay just let into our world (lowercase name -> when). Justice Mod
/// asks about each login so these players skip Mojang's check, which offline accounts can't pass.
static OFFLINE_GUESTS: Lazy<Mutex<std::collections::HashMap<String, std::time::Instant>>> = Lazy::new(Default::default);

/// Did an offline Justice account with this name just join our shared world through the relay?
pub fn is_offline_guest(name: &str) -> bool {
    let mut g = OFFLINE_GUESTS.lock().unwrap();
    g.retain(|_, t| t.elapsed() < Duration::from_secs(120));
    hosted().is_some() && g.contains_key(&name.to_lowercase())
}
static HOST_GEN: AtomicU64 = AtomicU64::new(0);

pub fn hosted() -> Option<Hosted> {
    HOSTED.lock().unwrap().clone()
}

fn token() -> Result<String> {
    social::token().ok_or_else(|| anyhow!(l!("Nejsi přihlášený do přátel", "You're not signed in to friends")))
}

/// Opens the local LAN world to invited friends.
pub async fn host(lan_port: u16, name: &str, mc: &str, loader: &str) -> Result<Value> {
    stop().await;
    let v = social::post("/worlds", json!({ "name": name, "mc": mc, "loader": loader })).await?;
    let id = v["id"].as_str().ok_or_else(|| anyhow!("bad reply"))?.to_string();
    let h = Hosted { id: id.clone(), name: name.to_string(), lan_port };
    *HOSTED.lock().unwrap() = Some(h.clone());
    let gen = HOST_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    let token = token()?;
    linfo!("world: hosting '{name}' (LAN port {lan_port}) as {id}");
    tauri::async_runtime::spawn(async move {
        let mut delay = 2;
        while HOST_GEN.load(Ordering::SeqCst) == gen {
            if let Err(e) = control(&token, &h, gen).await {
                lwarn!("world: control connection lost: {e:#}");
            }
            if HOST_GEN.load(Ordering::SeqCst) != gen {
                break;
            }
            tokio::time::sleep(Duration::from_secs(delay)).await;
            delay = (delay * 2).min(30);
        }
    });
    Ok(json!({ "id": id }))
}

async fn control(token: &str, h: &Hosted, gen: u64) -> Result<()> {
    let url = format!("{}/host?token={token}&world={}", social::RELAY, h.id);
    let (ws, _) = tokio_tungstenite::connect_async(url).await?;
    let (mut sink, mut stream) = ws.split();
    loop {
        tokio::select! {
            m = stream.next() => match m {
                Some(Ok(Message::Text(t))) => {
                    let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                    if v["type"] == "open" {
                        let conn = v["conn"].as_str().unwrap_or_default().to_string();
                        if v["guest"]["offline"] == true {
                            if let Some(name) = v["guest"]["name"].as_str() {
                                OFFLINE_GUESTS.lock().unwrap().insert(name.to_lowercase(), std::time::Instant::now());
                                linfo!("world: offline player {name} is joining");
                            }
                        }
                        let (token, port) = (token.to_string(), h.lan_port);
                        tauri::async_runtime::spawn(async move {
                            if let Err(e) = serve_guest(&token, &conn, port).await {
                                lwarn!("world: guest connection failed: {e:#}");
                            }
                        });
                    }
                }
                Some(Ok(Message::Ping(p))) => sink.send(Message::Pong(p)).await?,
                Some(Ok(Message::Close(_))) | None => return Ok(()),
                Some(Err(e)) => return Err(e.into()),
                _ => {}
            },
            _ = tokio::time::sleep(Duration::from_secs(2)) => {
                if HOST_GEN.load(Ordering::SeqCst) != gen {
                    let _ = sink.close().await;
                    return Ok(());
                }
            }
        }
    }
}

async fn serve_guest(token: &str, conn: &str, lan_port: u16) -> Result<()> {
    let url = format!("{}/data?token={token}&conn={conn}", social::RELAY);
    let (ws, _) = tokio_tungstenite::connect_async(url).await?;
    let tcp = TcpStream::connect(("127.0.0.1", lan_port)).await?;
    ldebug!("world: guest {conn} connected");
    pipe(tcp, ws).await;
    ldebug!("world: guest {conn} left");
    Ok(())
}

pub async fn invite(uuid: &str) -> Result<Value> {
    let h = hosted().ok_or_else(|| anyhow!(l!("Nemáš otevřený svět", "You don't have a world open")))?;
    social::post(&format!("/worlds/{}/invite", h.id), json!({ "uuid": uuid })).await
}

pub async fn stop() {
    HOST_GEN.fetch_add(1, Ordering::SeqCst);
    if let Some(h) = HOSTED.lock().unwrap().take() {
        let id = h.id.clone();
        tauri::async_runtime::spawn(async move {
            let _ = social::delete(&format!("/worlds/{id}")).await;
        });
        linfo!("world: stopped hosting {}", h.id);
    }
}

/// Guest side: returns a local port for Minecraft to connect to.
pub async fn join(world: &str) -> Result<u16> {
    let info = social::get(&format!("/worlds/{world}")).await?;
    if info["online"] != true {
        bail!(l!("Hostitel teď svět nemá otevřený", "The host doesn't have the world open right now"));
    }
    let token = token()?;
    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let world = world.to_string();
    linfo!("world: joining {world} through 127.0.0.1:{port}");
    tauri::async_runtime::spawn(async move {
        // Minecraft pings the server and then connects; keep accepting for a while.
        loop {
            let accepted = tokio::time::timeout(Duration::from_secs(30 * 60), listener.accept()).await;
            let Ok(Ok((tcp, _))) = accepted else { break };
            let url = format!("{}/join?token={token}&world={world}", social::RELAY);
            tauri::async_runtime::spawn(async move {
                match tokio_tungstenite::connect_async(url).await {
                    Ok((ws, _)) => pipe(tcp, ws).await,
                    Err(e) => lwarn!("world: relay join failed: {e:#}"),
                }
            });
        }
    });
    Ok(port)
}

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>;

/// Minecraft bytes <-> binary WebSocket frames until either side closes.
async fn pipe(tcp: TcpStream, ws: Ws) {
    let _ = tcp.set_nodelay(true);
    let (mut tr, mut tw) = tcp.into_split();
    let (mut wsink, mut wstream) = ws.split();
    let up = async {
        let mut buf = vec![0u8; 32 * 1024];
        loop {
            match tr.read(&mut buf).await {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if wsink.send(Message::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
            }
        }
        let _ = wsink.close().await;
    };
    let down = async {
        while let Some(Ok(m)) = wstream.next().await {
            match m {
                Message::Binary(d) => {
                    if tw.write_all(&d).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        let _ = tw.shutdown().await;
    };
    tokio::select! {
        _ = up => {},
        _ = down => {},
    }
}
