//! Favourite servers and Server List Ping (player count, MOTD, icon, latency).

use crate::net::{self, CLIENT};
use crate::settings;
use anyhow::{anyhow, bail, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Server {
    pub id: String,
    pub name: String,
    pub address: String,
    pub instance: String,
    pub pinned: bool,
}

fn path() -> PathBuf {
    settings::data_dir().join("servers.json")
}

pub fn list() -> Vec<Server> {
    let mut v: Vec<Server> = net::read_json(&path()).ok().and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
    // Justice SMP moved to its own domain.
    let mut moved = false;
    for s in v.iter_mut().filter(|s| s.address.trim().eq_ignore_ascii_case("justicesmp.chleb.top")) {
        s.address = "justicesmp.online".into();
        moved = true;
    }
    if moved {
        let _ = save_all(&v);
    }
    v.sort_by_key(|s| !s.pinned);
    v
}

pub fn save_all(list: &[Server]) -> Result<()> {
    net::write_json(&path(), &serde_json::to_value(list)?)
}

pub fn upsert(mut s: Server) -> Result<Server> {
    if s.address.trim().is_empty() {
        bail!(l!("Zadej adresu serveru", "Enter a server address"));
    }
    let mut all = list();
    if s.id.is_empty() {
        s.id = uuid::Uuid::new_v4().to_string();
    }
    all.retain(|x| x.id != s.id);
    all.push(s.clone());
    save_all(&all)?;
    Ok(s)
}

pub fn remove(id: &str) -> Result<()> {
    let mut all = list();
    all.retain(|x| x.id != id);
    save_all(&all)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub online: u64,
    pub max: u64,
    pub motd: String,
    pub version: String,
    pub icon: Option<String>,
    pub ping_ms: u64,
}

fn write_varint(buf: &mut Vec<u8>, mut v: i32) {
    loop {
        let mut b = (v & 0x7f) as u8;
        v = ((v as u32) >> 7) as i32;
        if v != 0 {
            b |= 0x80;
        }
        buf.push(b);
        if v == 0 {
            break;
        }
    }
}

async fn read_varint(s: &mut TcpStream) -> Result<i32> {
    let mut out = 0i32;
    for i in 0..5 {
        let b = s.read_u8().await?;
        out |= ((b & 0x7f) as i32) << (7 * i);
        if b & 0x80 == 0 {
            return Ok(out);
        }
    }
    bail!("varint too long")
}

fn packet(id: i32, body: &[u8]) -> Vec<u8> {
    let mut inner = vec![];
    write_varint(&mut inner, id);
    inner.extend_from_slice(body);
    let mut out = vec![];
    write_varint(&mut out, inner.len() as i32);
    out.extend(inner);
    out
}

/// Resolve `_minecraft._tcp` SRV records over DNS-over-HTTPS (std has no SRV lookup).
async fn resolve_srv(host: &str) -> Option<(String, u16)> {
    let url = format!("https://cloudflare-dns.com/dns-query?name=_minecraft._tcp.{host}&type=SRV");
    let v: Value = CLIENT.get(url).header("accept", "application/dns-json").send().await.ok()?.json().await.ok()?;
    let data = v["Answer"].as_array()?.iter().find(|a| a["type"] == 33)?["data"].as_str()?.to_string();
    let parts: Vec<&str> = data.split_whitespace().collect();
    let port = parts.get(2)?.parse().ok()?;
    Some((parts.get(3)?.trim_end_matches('.').to_string(), port))
}

fn flatten_motd(v: &Value) -> String {
    fn code(color: &str) -> Option<char> {
        Some(match color {
            "black" => '0', "dark_blue" => '1', "dark_green" => '2', "dark_aqua" => '3', "dark_red" => '4',
            "dark_purple" => '5', "gold" => '6', "gray" => '7', "dark_gray" => '8', "blue" => '9',
            "green" => 'a', "aqua" => 'b', "red" => 'c', "light_purple" => 'd', "yellow" => 'e', "white" => 'f',
            _ => return None,
        })
    }
    match v {
        Value::String(s) => s.clone(),
        Value::Array(a) => a.iter().map(flatten_motd).collect(),
        Value::Object(o) => {
            let mut s = String::new();
            if let Some(c) = o.get("color").and_then(|c| c.as_str()).and_then(code) {
                s.push('§');
                s.push(c);
            }
            if o.get("bold") == Some(&Value::Bool(true)) {
                s.push_str("§l");
            }
            s.push_str(o.get("text").and_then(|t| t.as_str()).unwrap_or_default());
            if let Some(extra) = o.get("extra") {
                s.push_str(&flatten_motd(extra));
            }
            s
        }
        _ => String::new(),
    }
}

pub async fn ping(address: &str) -> Result<Status> {
    let address = address.trim();
    let (host, port) = match address.rsplit_once(':') {
        Some((h, p)) if p.parse::<u16>().is_ok() => (h.to_string(), p.parse().unwrap()),
        _ => match resolve_srv(address).await {
            Some(hp) => hp,
            None => (address.to_string(), 25565),
        },
    };
    let fut = async {
        let start = Instant::now();
        let mut s = TcpStream::connect((host.as_str(), port)).await?;
        let mut hs = vec![];
        write_varint(&mut hs, -1);
        write_varint(&mut hs, host.len() as i32);
        hs.extend_from_slice(host.as_bytes());
        hs.extend_from_slice(&port.to_be_bytes());
        write_varint(&mut hs, 1);
        s.write_all(&packet(0, &hs)).await?;
        s.write_all(&packet(0, &[])).await?;
        let _len = read_varint(&mut s).await?;
        let _id = read_varint(&mut s).await?;
        let n = read_varint(&mut s).await? as usize;
        if n > 1 << 21 {
            bail!(l!("odpověď je moc velká", "the response is too large"));
        }
        let mut buf = vec![0u8; n];
        s.read_exact(&mut buf).await?;
        let ping_ms = start.elapsed().as_millis() as u64;
        let v: Value = serde_json::from_slice(&buf)?;
        Ok::<_, anyhow::Error>(Status {
            online: v["players"]["online"].as_u64().unwrap_or(0),
            max: v["players"]["max"].as_u64().unwrap_or(0),
            motd: flatten_motd(&v["description"]),
            version: v["version"]["name"].as_str().unwrap_or_default().into(),
            icon: v["favicon"].as_str().map(String::from),
            ping_ms,
        })
    };
    tokio::time::timeout(Duration::from_secs(5), fut).await.map_err(|_| anyhow!(l!("Server neodpovídá", "The server isn't responding")))?
}
