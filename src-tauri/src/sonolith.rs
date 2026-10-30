//! Sonolith local API (JSON-RPC 2.0, one message per line) over a Windows named pipe or a Linux
//! Unix socket. Registration only after the user clicks Connect; the token is kept in a user-only
//! file. Events are forwarded to the UI as "sonolith"; the UI calls a fixed set of methods.
//! Spec: https://github.com/opecko/sonolith/blob/master/api/SPEC.md

use crate::events;
use anyhow::{anyhow, bail, Context, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, oneshot};

const APP_ID: &str = "online.justiceclient.launcher";
const API_VERSION: &str = "1.0";
const TOPICS: [&str; 5] = ["player.state", "player.track", "player.position", "queue", "lyrics.line"];

/// Methods the UI may call through `call`.
const ALLOWED: [&str; 24] = [
    "player.getState", "player.play", "player.pause", "player.toggle", "player.next", "player.previous", "player.seek",
    "player.setVolume", "player.setShuffle", "player.setRepeat", "player.playItem", "queue.get", "queue.add",
    "queue.playNext", "queue.remove", "queue.move", "search.query", "search.suggestions", "library.playlists",
    "library.likedSongs", "playlist.get", "rating.set", "lyrics.get", "app.info",
];

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    /// Sonolith's API endpoint exists (Sonolith runs with the API on).
    pub available: bool,
    /// We have a token (the user connected Justice Client once).
    pub registered: bool,
    /// A session is open right now.
    pub connected: bool,
    /// Waiting for the user to approve us in Sonolith.
    pub pending: bool,
    pub granted_scopes: Vec<String>,
    pub signed_in: bool,
    pub app_version: String,
    /// Last problem worth showing ("denied", "blocked", "expired", or a message).
    pub error: Option<String>,
    pub retry_after_ms: Option<u64>,
}

static STATUS: Lazy<Mutex<Status>> = Lazy::new(|| Mutex::new(Status::default()));
static OUT: Lazy<Mutex<Option<mpsc::UnboundedSender<String>>>> = Lazy::new(|| Mutex::new(None));
static WAIT: Lazy<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static RUNNING: AtomicBool = AtomicBool::new(false);

fn set_status(f: impl FnOnce(&mut Status)) {
    let s = {
        let mut st = STATUS.lock().unwrap();
        f(&mut st);
        st.clone()
    };
    events::emit("sonolith-status", s);
}

pub fn status() -> Status {
    let mut s = STATUS.lock().unwrap().clone();
    s.available = discovery().is_some_and(|(_, pid)| pid_alive(pid));
    s.registered = token().is_some();
    s
}

// ---------------- Discovery and transport ----------------

fn discovery_path() -> Option<PathBuf> {
    #[cfg(windows)]
    return dirs::data_local_dir().map(|d| d.join("Sonolith").join("api").join("discovery.json"));
    #[cfg(not(windows))]
    {
        let run = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from);
        run.map(|r| r.join("sonolith").join("discovery.json"))
    }
}

/// (endpoint path, server pid) from discovery.json.
fn discovery() -> Option<(String, u32)> {
    let text = std::fs::read_to_string(discovery_path()?).ok()?;
    let v: Value = serde_json::from_str(&text).ok()?;
    Some((v["path"].as_str()?.to_string(), v["pid"].as_u64().unwrap_or(0) as u32))
}

/// discovery.json can outlive Sonolith (a crash): only trust it while its process runs.
fn pid_alive(pid: u32) -> bool {
    if pid == 0 {
        return true;
    }
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::Foundation::CloseHandle;
        use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return false;
        }
        CloseHandle(h);
        true
    }
    #[cfg(not(windows))]
    {
        std::path::Path::new(&format!("/proc/{pid}")).exists()
    }
}

trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

async fn open() -> Result<Box<dyn Io>> {
    let (path, pid) = discovery().ok_or_else(|| anyhow!(l!("Sonolith neběží nebo má vypnuté API", "Sonolith isn't running or its API is off")))?;
    #[cfg(windows)]
    {
        use tokio::net::windows::named_pipe::ClientOptions;
        let pipe = ClientOptions::new().open(&path).with_context(|| format!("open {path}"))?;
        // Only send the token to the real Sonolith: the pipe's server must be the process from discovery.json.
        if pid != 0 {
            use std::os::windows::io::AsRawHandle;
            let mut server = 0u32;
            let ok = unsafe { windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId(pipe.as_raw_handle() as _, &mut server) };
            if ok != 0 && server != pid {
                bail!("Sonolith pipe server pid mismatch");
            }
        }
        Ok(Box::new(pipe))
    }
    #[cfg(unix)]
    {
        let _ = pid;
        let s = tokio::net::UnixStream::connect(&path).await.with_context(|| format!("connect {path}"))?;
        Ok(Box::new(s))
    }
}

// ---------------- Token ----------------

fn token_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("JusticeClient").join("sonolith.token")
}

fn token() -> Option<String> {
    std::fs::read_to_string(token_path()).ok().map(|s| s.trim().to_string()).filter(|s| !s.is_empty())
}

fn save_token(t: &str) -> Result<()> {
    let p = token_path();
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(&p, t)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

fn forget_token() {
    let _ = std::fs::remove_file(token_path());
}

// ---------------- Session ----------------

fn icon_data_uri() -> String {
    use base64::Engine;
    let png = include_bytes!("../icons/64x64.png");
    format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(png))
}

fn line(method: &str, id: Option<u64>, params: Value) -> String {
    let mut m = json!({ "jsonrpc": "2.0", "method": method, "params": params });
    if let Some(id) = id {
        m["id"] = id.into();
    }
    format!("{m}\n")
}

/// Asks the user to let Justice Client connect. Keeps the connection open until Sonolith answers.
pub async fn register() -> Result<()> {
    if STATUS.lock().unwrap().pending {
        return Ok(());
    }
    let io = open().await?;
    let (r, mut w) = tokio::io::split(io);
    let mut lines = BufReader::new(r).lines();
    let reason = |cs: &str, en: &str| if crate::i18n::is_en() { en.to_string() } else { cs.to_string() };
    let params = json!({
        "appId": APP_ID,
        "name": "Justice Client",
        "description": reason("Minecraft launcher: ukazuje a ovládá tvoji hudbu v launcheru.", "Minecraft launcher: shows and controls your music in the launcher."),
        "publisher": "Justice Client",
        "homepage": "https://justiceclient.online",
        "icon": icon_data_uri(),
        "apiVersion": API_VERSION,
        "scopes": [
            { "scope": "player:read", "required": true, "reason": reason("Ukázat, co hraje, v přehrávači launcheru.", "Show what's playing in the launcher's player.") },
            { "scope": "player:control", "required": true, "reason": reason("Pauza, další a předchozí písnička a posun z launcheru.", "Pause, skip and seek from the launcher.") },
            { "scope": "queue:read", "required": false, "reason": reason("Ukázat frontu písniček.", "Show your queue.") },
            { "scope": "queue:write", "required": false, "reason": reason("Přidávat písničky do fronty z launcheru.", "Add songs to your queue from the launcher.") },
            { "scope": "search", "required": false, "reason": reason("Hledat písničky přímo v launcheru.", "Search songs right in the launcher.") },
            { "scope": "library:read", "required": false, "reason": reason("Ukázat tvoje playlisty a oblíbené.", "Show your playlists and liked songs.") },
            { "scope": "lyrics:read", "required": false, "reason": reason("Ukázat text písničky v launcheru.", "Show song lyrics in the launcher.") },
        ],
    });
    w.write_all(line("app.register", Some(1), params).as_bytes()).await?;
    set_status(|s| {
        s.pending = true;
        s.error = None;
        s.retry_after_ms = None;
    });
    let outcome = tokio::time::timeout(Duration::from_secs(330), async {
        while let Some(l) = lines.next_line().await? {
            let v: Value = serde_json::from_str(&l).unwrap_or(Value::Null);
            if v["id"] == 1 {
                if let Some(e) = v.get("error") {
                    return Ok::<Value, anyhow::Error>(json!({ "status": "error", "error": e }));
                }
                continue;
            }
            if v["method"] == "registration.resolved" {
                return Ok(v["params"].clone());
            }
        }
        Ok(json!({ "status": "error", "error": { "message": "connection closed" } }))
    })
    .await;
    let res = match outcome {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => json!({ "status": "error", "error": { "message": e.to_string() } }),
        Err(_) => json!({ "status": "expired" }),
    };
    let status = res["status"].as_str().unwrap_or("error").to_string();
    match status.as_str() {
        "approved" => {
            let tok = res["token"].as_str().ok_or_else(|| anyhow!("no token"))?;
            save_token(tok)?;
            set_status(|s| s.pending = false);
            start();
            Ok(())
        }
        other => {
            let msg = match other {
                "error" => {
                    let e = &res["error"];
                    let name = e["data"]["name"].as_str().unwrap_or_default();
                    if name == "Blocked" { "blocked".to_string() } else if name == "RegistrationDenied" { "denied".to_string() } else { e["message"].as_str().unwrap_or("error").to_string() }
                }
                s => s.to_string(),
            };
            let retry = res["retryAfterMs"].as_u64().or_else(|| res["error"]["data"]["retryAfterMs"].as_u64());
            set_status(|s| {
                s.pending = false;
                s.error = Some(msg.clone());
                s.retry_after_ms = retry;
            });
            bail!(msg)
        }
    }
}

/// Starts (once) the background session: hello, subscribe, forward events, reconnect with backoff.
pub fn start() {
    if token().is_none() || RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }
    tauri::async_runtime::spawn(async {
        let mut delay = 1_000u64;
        loop {
            if token().is_none() {
                break;
            }
            match session().await {
                Ok(()) => delay = 1_000,
                Err(e) => {
                    let msg = e.to_string();
                    if msg == "unauthorized" {
                        forget_token();
                        set_status(|s| {
                            s.connected = false;
                            s.error = Some("disconnected".into());
                        });
                        break;
                    }
                }
            }
            set_status(|s| s.connected = false);
            *OUT.lock().unwrap() = None;
            // Backoff 1 s doubling to 60 s with ±20 % jitter.
            let jitter = (delay as f64 * (0.8 + 0.4 * rand_unit())) as u64;
            tokio::time::sleep(Duration::from_millis(jitter)).await;
            delay = (delay * 2).min(60_000);
        }
        RUNNING.store(false, Ordering::SeqCst);
    });
}

fn rand_unit() -> f64 {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
    (n % 1000) as f64 / 1000.0
}

async fn session() -> Result<()> {
    let tok = token().ok_or_else(|| anyhow!("unauthorized"))?;
    let io = open().await?;
    let (r, mut w) = tokio::io::split(io);
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    // hello first; Sonolith finishes it before reading the next line, so subscribe can follow.
    w.write_all(line("session.hello", Some(0), json!({ "appId": APP_ID, "token": tok, "apiVersion": API_VERSION })).as_bytes()).await?;
    w.write_all(line("events.subscribe", Some(u64::MAX - 1), json!({ "topics": TOPICS, "positionHz": 1 })).as_bytes()).await?;
    *OUT.lock().unwrap() = Some(tx);
    let writer = tokio::spawn(async move {
        while let Some(l) = rx.recv().await {
            if w.write_all(l.as_bytes()).await.is_err() {
                break;
            }
        }
    });
    let mut lines = BufReader::new(r).lines();
    let result: Result<()> = async {
        while let Some(l) = lines.next_line().await? {
            let v: Value = match serde_json::from_str(&l) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if let Some(m) = v["method"].as_str() {
                match m {
                    "session.revoked" => {
                        forget_token();
                        set_status(|s| {
                            s.connected = false;
                            s.error = Some("disconnected".into());
                        });
                        bail!("unauthorized");
                    }
                    "session.scopesChanged" => {
                        let g = scopes(&v["params"]["grantedScopes"]);
                        set_status(|s| s.granted_scopes = g);
                    }
                    "account.changed" => {
                        let si = v["params"]["signedIn"].as_bool().unwrap_or(false);
                        set_status(|s| s.signed_in = si);
                    }
                    _ => {}
                }
                events::emit("sonolith", json!({ "method": m, "params": v["params"] }));
                continue;
            }
            let id = v["id"].as_u64();
            if id == Some(0) {
                if let Some(e) = v.get("error") {
                    let name = e["data"]["name"].as_str().unwrap_or_default();
                    if matches!(name, "Unauthorized" | "NotRegistered") {
                        bail!("unauthorized");
                    }
                    bail!(e["message"].as_str().unwrap_or("hello failed").to_string());
                }
                let r = &v["result"];
                let g = scopes(&r["grantedScopes"]);
                let si = r["signedIn"].as_bool().unwrap_or(false);
                let ver = r["appVersion"].as_str().unwrap_or_default().to_string();
                set_status(|s| {
                    s.connected = true;
                    s.error = None;
                    s.granted_scopes = g;
                    s.signed_in = si;
                    s.app_version = ver;
                });
                continue;
            }
            if let Some(id) = id {
                if let Some(tx) = WAIT.lock().unwrap().remove(&id) {
                    let _ = tx.send(match v.get("error") {
                        Some(e) => Err(anyhow!("{}", e["data"]["name"].as_str().or(e["message"].as_str()).unwrap_or("error"))),
                        None => Ok(v["result"].clone()),
                    });
                }
            }
        }
        Ok(())
    }
    .await;
    writer.abort();
    WAIT.lock().unwrap().clear();
    result
}

fn scopes(v: &Value) -> Vec<String> {
    v.as_array().into_iter().flatten().filter_map(|s| s.as_str().map(String::from)).collect()
}

/// One request on the open session (only methods from ALLOWED).
pub async fn call(method: &str, params: Value) -> Result<Value> {
    if !ALLOWED.contains(&method) {
        bail!("method not allowed: {method}");
    }
    let tx = OUT.lock().unwrap().clone().ok_or_else(|| anyhow!(l!("Sonolith není připojený", "Sonolith isn't connected")))?;
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let (otx, orx) = oneshot::channel();
    WAIT.lock().unwrap().insert(id, otx);
    let params = if params.is_null() { json!({}) } else { params };
    tx.send(line(method, Some(id), params)).map_err(|_| anyhow!("Sonolith isn't connected"))?;
    match tokio::time::timeout(Duration::from_secs(30), orx).await {
        Ok(Ok(r)) => r,
        _ => {
            WAIT.lock().unwrap().remove(&id);
            bail!("Timeout")
        }
    }
}

/// "Disconnect from Sonolith": unregister (when it's up) and forget the token either way.
pub async fn disconnect() -> Result<()> {
    if OUT.lock().unwrap().is_some() {
        let _ = tokio::time::timeout(Duration::from_secs(3), call_raw("app.unregister")).await;
    }
    forget_token();
    *OUT.lock().unwrap() = None;
    set_status(|s| {
        s.connected = false;
        s.granted_scopes.clear();
        s.error = None;
    });
    Ok(())
}

async fn call_raw(method: &str) -> Result<Value> {
    let tx = OUT.lock().unwrap().clone().ok_or_else(|| anyhow!("not connected"))?;
    let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
    let (otx, orx) = oneshot::channel();
    WAIT.lock().unwrap().insert(id, otx);
    tx.send(line(method, Some(id), json!({})))?;
    orx.await?
}
