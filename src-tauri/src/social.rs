//! Friends & chat (Justice Social on justiceclient.online). Sign-in proves the Minecraft account the same
//! way joining a server does: Mojang `join` with our access token, the server checks `hasJoined`.
//! The server never sees the access token. A WebSocket delivers messages/presence as
//! `social-event` events to the UI.
//! Offline accounts can't prove anything to Mojang, so they get a Justice password instead; the
//! session token is kept per account in social-sessions.json so they only type it once.

use crate::{auth, events, net};
use anyhow::{anyhow, bail, Context, Result};
use futures::{SinkExt, StreamExt};
use once_cell::sync::Lazy;
use serde_json::{json, Value};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;
use tokio::sync::mpsc;

const BASE: &str = "https://justiceclient.online/social";
const WS: &str = "wss://justiceclient.online/social/ws";
pub const RELAY: &str = "wss://justiceclient.online/social/relay";

/// Plays a clip a friend shared. Only our own upload URLs, so neither chat can open arbitrary links.
/// On Linux the file is downloaded and handed to the system player: WebKitGTK and Linux browsers
/// often can't decode HEVC (clips from Windows) and the in-app <video> crashed the whole window.
pub async fn open_shared_clip(url: &str) -> Result<()> {
    // Uploads are served on both domains (older clips still link the old one).
    if !url.starts_with("https://justiceclient.online/u/") && !url.starts_with("https://ertenix.net/justice/u/") {
        bail!("only shared clips can be opened");
    }
    use tauri_plugin_opener::OpenerExt;
    let app = events::app().ok_or_else(|| anyhow!("launcher not ready"))?;
    #[cfg(target_os = "linux")]
    {
        let name: String = url.rsplit('/').next().unwrap_or("clip").chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.')).collect();
        let name = if name.ends_with(".mp4") { name } else { format!("{name}.mp4") };
        let path = crate::settings::cache_dir().join("shared-clips").join(name);
        net::download(&net::Job::new(url, path.clone())).await.context("downloading the clip")?;
        app.opener().open_path(path.to_string_lossy(), None::<&str>)?;
    }
    #[cfg(not(target_os = "linux"))]
    app.opener().open_url(url, None::<&str>)?;
    Ok(())
}

#[derive(Clone)]
struct Session {
    token: String,
    uuid: String,
    name: String,
}

/// Social events with a sequence number, for the in-game chat (Justice Mod long-polls them).
static EVENTS: Lazy<Mutex<(u64, std::collections::VecDeque<(u64, Value)>)>> = Lazy::new(|| Mutex::new((0, Default::default())));
static EVENT_NOTIFY: Lazy<tokio::sync::Notify> = Lazy::new(tokio::sync::Notify::new);

/// Sends a social event to the launcher UI and keeps it for the in-game chat.
pub(crate) fn emit(v: Value) {
    {
        let mut e = EVENTS.lock().unwrap();
        e.0 += 1;
        let seq = e.0;
        e.1.push_back((seq, v.clone()));
        while e.1.len() > 200 {
            e.1.pop_front();
        }
    }
    EVENT_NOTIFY.notify_waiters();
    events::emit("social-event", v);
}

/// Events newer than `after`, waiting up to `wait` for the first one.
pub async fn events_after(after: u64, wait: Duration) -> (u64, Vec<Value>) {
    let collect = || {
        let e = EVENTS.lock().unwrap();
        let list: Vec<Value> = e.1.iter().filter(|(s, _)| *s > after).map(|(_, v)| v.clone()).collect();
        (e.0, list)
    };
    let (seq, list) = collect();
    if !list.is_empty() || after > seq {
        return (seq, list);
    }
    let _ = tokio::time::timeout(wait, EVENT_NOTIFY.notified()).await;
    collect()
}

pub fn token() -> Option<String> {
    SESSION.lock().unwrap().as_ref().map(|s| s.token.clone())
}

pub fn me() -> Option<(String, String)> {
    SESSION.lock().unwrap().as_ref().map(|s| (s.uuid.clone(), s.name.clone()))
}

static SESSION: Lazy<Mutex<Option<Session>>> = Lazy::new(|| Mutex::new(None));
/// Outgoing WebSocket frames (presence, typing).
static OUT: Lazy<Mutex<Option<mpsc::UnboundedSender<String>>>> = Lazy::new(|| Mutex::new(None));
/// Bumped on every (re)connect so an old socket task stops.
static GEN: AtomicU64 = AtomicU64::new(0);
static PRESENCE: Lazy<Mutex<Value>> = Lazy::new(|| Mutex::new(json!({ "state": "online" })));

fn session() -> Result<Session> {
    SESSION.lock().unwrap().clone().ok_or_else(|| anyhow!(l!("Nejsi přihlášený do přátel", "You're not signed in to friends")))
}

/// Error text the UI recognises: an offline account has to sign in with its Justice password.
pub const NEEDS_PASSWORD: &str = "JUSTICE_NEEDS_PASSWORD";

fn sessions_path() -> std::path::PathBuf {
    crate::settings::data_dir().join("social-sessions.json")
}

fn stored_token(account: &str) -> Option<String> {
    let v = net::read_json(&sessions_path()).ok()?;
    v[account].as_str().map(String::from)
}

fn store_token(account: &str, token: Option<&str>) {
    let mut v = net::read_json(&sessions_path()).unwrap_or(json!({}));
    if !v.is_object() {
        v = json!({});
    }
    match token {
        Some(t) => v[account] = json!(t),
        None => {
            v.as_object_mut().unwrap().remove(account);
        }
    }
    if let Err(e) = net::write_json(&sessions_path(), &v) {
        lwarn!("social: couldn't save the session: {e:#}");
    }
}

/// Starts the session from a /login-style reply ({token, uuid, name}).
fn start(v: &Value) -> Session {
    let s = Session {
        token: v["token"].as_str().unwrap_or_default().to_string(),
        uuid: v["uuid"].as_str().unwrap_or_default().to_string(),
        name: v["name"].as_str().unwrap_or_default().to_string(),
    };
    linfo!("social: signed in as {} ({})", s.name, s.uuid);
    *SESSION.lock().unwrap() = Some(s.clone());
    connect(s.clone());
    emit(json!({ "type": "signed-in" }));
    s
}

/// Offline account with a saved Justice session: check it still works.
async fn login_offline_saved(acc: &auth::Account) -> Result<Value> {
    let Some(token) = stored_token(&acc.id) else { bail!(NEEDS_PASSWORD) };
    let r = net::CLIENT.get(format!("{BASE}/me")).bearer_auth(&token).send().await?;
    if r.status() == reqwest::StatusCode::UNAUTHORIZED {
        store_token(&acc.id, None);
        bail!(NEEDS_PASSWORD);
    }
    let me = parse(r).await?;
    if !me["name"].as_str().unwrap_or_default().eq_ignore_ascii_case(&acc.username) {
        store_token(&acc.id, None);
        bail!(NEEDS_PASSWORD);
    }
    let v = json!({ "token": token, "uuid": me["uuid"], "name": me["name"] });
    start(&v);
    sync_skin(acc.clone());
    Ok(v)
}

/// Offline accounts: create the Justice account (`register`) or sign in to it with the password.
pub async fn login_password(password: &str, register: bool) -> Result<Value> {
    let acc = auth::ready_account(None).await?;
    if acc.kind == "msa" {
        bail!(l!("Microsoft účet se přihlašuje sám, heslo nepotřebuje", "Microsoft accounts sign in by themselves, no password needed"));
    }
    let path = if register { "/register" } else { "/login/password" };
    let r = net::CLIENT.post(format!("{BASE}{path}")).json(&json!({ "name": acc.username, "password": password })).send().await?;
    let v = parse(r).await?;
    store_token(&acc.id, v["token"].as_str());
    start(&v);
    sync_skin(acc);
    Ok(v)
}

/// Signs out for good: an offline account forgets its saved session too.
pub fn sign_out() {
    if let Ok(s) = auth::active_account() {
        if s.kind != "msa" {
            store_token(&s.id, None);
        }
    }
    logout();
    emit(json!({ "type": "signed-out" }));
}

/// After switching accounts: drop the old session and sign in as the new one in the background.
pub fn account_changed() {
    logout();
    emit(json!({ "type": "signed-out" }));
    tauri::async_runtime::spawn(async {
        if let Err(e) = login().await {
            ldebug!("social: sign-in after account switch skipped: {e:#}");
        }
    });
}

/// Sign in with the active account and open the realtime connection. Microsoft accounts prove
/// themselves through Mojang; offline accounts use their saved Justice session.
pub async fn login() -> Result<Value> {
    let acc = auth::ready_account(None).await?;
    if acc.kind != "msa" {
        return login_offline_saved(&acc).await;
    }
    let ch = net::get_json(&format!("{BASE}/challenge")).await?;
    let server_id = ch["serverId"].as_str().ok_or_else(|| anyhow!("bad challenge"))?.to_string();
    let join = net::CLIENT
        .post("https://sessionserver.mojang.com/session/minecraft/join")
        .json(&json!({ "accessToken": acc.access_token, "selectedProfile": acc.uuid.replace('-', ""), "serverId": server_id }))
        .send()
        .await
        .context("Mojang join")?;
    if !join.status().is_success() {
        bail!(l!("Mojang nepotvrdil účet ({})", "Mojang didn't confirm the account ({})", join.status()));
    }
    let r = net::CLIENT.post(format!("{BASE}/login")).json(&json!({ "name": acc.username, "serverId": server_id })).send().await?;
    let v: Value = parse(r).await?;
    start(&v);
    Ok(v)
}

type Cached = Lazy<Mutex<Option<(std::time::Instant, Value)>>>;
static CAPES: Cached = Lazy::new(|| Mutex::new(None));
static PETS: Cached = Lazy::new(|| Mutex::new(None));
static SKINS: Cached = Lazy::new(|| Mutex::new(None));

/// A uuid -> value map from the server, refreshed at most once a minute (Justice Mod polls it).
async fn cached_map(cache: &Cached, path: &str) -> Value {
    if let Some((t, v)) = cache.lock().unwrap().as_ref() {
        if t.elapsed() < Duration::from_secs(60) {
            return v.clone();
        }
    }
    if session().is_err() {
        return json!({});
    }
    match get(path).await {
        Ok(v) => {
            *cache.lock().unwrap() = Some((std::time::Instant::now(), v.clone()));
            v
        }
        Err(e) => {
            ldebug!("social: {path}: {e:#}");
            cache.lock().unwrap().as_ref().map(|(_, v)| v.clone()).unwrap_or(json!({}))
        }
    }
}

/// uuid -> Justice cape.
pub async fn capes() -> Value {
    cached_map(&CAPES, "/capes").await
}

pub async fn pets() -> Value {
    cached_map(&PETS, "/pets").await
}

/// Forget the cached capes (after picking a new one, so it shows in-game right away).
pub fn capes_changed() {
    *CAPES.lock().unwrap() = None;
}

/// uuid -> {url, slim} skins of offline players. Your own offline skin is always in it, with the
/// local `file`, so it shows in-game even without the friends sign-in.
pub async fn skins() -> Value {
    let mut v = cached_map(&SKINS, "/skins").await;
    if !v.is_object() {
        v = json!({});
    }
    if let Ok(acc) = auth::active_account() {
        if acc.kind != "msa" {
            let uuid = acc.uuid.replace('-', "");
            match auth::local_skin(&acc) {
                Some(file) => v[uuid] = json!({ "url": acc.skin_url, "slim": acc.skin_variant == "SLIM", "file": file }),
                None => {
                    v.as_object_mut().unwrap().remove(&uuid);
                }
            }
        }
    }
    v
}

/// Sends an offline account's skin (or its reset) to Justice Social so other players see it.
/// Best effort: without the friends sign-in it stays local until the next sign-in.
pub async fn push_skin(acc: &auth::Account) {
    if acc.kind == "msa" || me().map(|m| m.0.replace('-', "")) != Some(acc.uuid.replace('-', "")) {
        return;
    }
    let r: Result<Value> = async {
        match auth::local_skin(acc) {
            Some(file) => {
                let bytes = tokio::fs::read(&file).await?;
                let slim = acc.skin_variant == "SLIM";
                let r = net::CLIENT.put(format!("{BASE}/skin?slim={slim}")).bearer_auth(&session()?.token).body(bytes).send().await?;
                parse(r).await
            }
            None => delete("/skin").await,
        }
    }
    .await;
    match r {
        Ok(_) => *SKINS.lock().unwrap() = None,
        Err(e) => lwarn!("social: skin upload failed: {e:#}"),
    }
}

/// After an offline sign-in: make sure the server has the skin picked in the launcher.
fn sync_skin(acc: auth::Account) {
    tauri::async_runtime::spawn(async move { push_skin(&acc).await });
}

pub fn logout() {
    *SESSION.lock().unwrap() = None;
    GEN.fetch_add(1, Ordering::SeqCst);
    *OUT.lock().unwrap() = None;
}

pub fn signed_in() -> Option<String> {
    SESSION.lock().unwrap().as_ref().map(|s| s.uuid.clone())
}

async fn parse(r: reqwest::Response) -> Result<Value> {
    let status = r.status();
    let v: Value = r.json().await.unwrap_or(Value::Null);
    if !status.is_success() {
        let msg = v["error"].as_str().unwrap_or("error").to_string();
        bail!("{}", translate(&msg));
    }
    Ok(v)
}

/// Server errors are English; show the common ones in Czech too.
fn translate(msg: &str) -> String {
    match msg {
        "player doesn't use Justice Client yet" => l!("Tenhle hráč ještě nepoužívá Justice Client", "That player doesn't use Justice Client yet"),
        "already friends" => l!("Už jste přátelé", "You're already friends"),
        "that's you" => l!("To jsi ty :)", "That's you :)"),
        "you can only message friends" => l!("Psát můžeš jen přátelům", "You can only message friends"),
        "session expired" | "not signed in" => l!("Přihlášení vypršelo, přihlas se znovu", "Your session expired, sign in again"),
        "wrong name or password" => l!("Špatné heslo", "Wrong password"),
        "that name is already taken" => l!("Tohle jméno už má někdo jiný — přihlas se heslem, nebo si udělej offline účet s jiným jménem", "That name is already taken — sign in with the password, or make an offline account with another name"),
        "that name belongs to a premium account" => l!(
            "Tohle jméno patří prémiovému (koupenému) účtu, offline ho použít nejde. Přidej si offline účet s jiným jménem.",
            "That name belongs to a premium (bought) account, so it can't be used offline. Add an offline account with another name."
        ),
        "the password must have at least 6 characters" => l!("Heslo musí mít aspoň 6 znaků", "The password needs at least 6 characters"),
        "the name must be 3–16 letters, digits or _" => l!("Jméno musí mít 3–16 znaků (písmena, číslice, _)", "The name must be 3–16 letters, digits or _"),
        "too many attempts, try again in 10 minutes" => l!("Moc pokusů, zkus to za 10 minut", "Too many attempts, try again in 10 minutes"),
        "you can only invite friends" => l!("Do party můžeš pozvat jen přátele", "You can only invite friends to a party"),
        "the party is full" => l!("Party je plná (max 8)", "The party is full (max 8)"),
        "already in your party" => l!("Už je v tvojí party", "Already in your party"),
        "the invite expired" => l!("Pozvánka už neplatí", "The invite expired"),
        "only the party leader can do that" => l!("Tohle může jen vedoucí party", "Only the party leader can do that"),
        "file is over 300 MB" => l!("Soubor je větší než 300 MB", "The file is over 300 MB"),
        other => other.to_string(),
    }
}

async fn call(method: reqwest::Method, path: &str, body: Option<Value>) -> Result<Value> {
    let s = session()?;
    let mut req = net::CLIENT.request(method, format!("{BASE}{path}")).bearer_auth(&s.token);
    if let Some(b) = body {
        req = req.json(&b);
    }
    let r = req.send().await?;
    if r.status() == reqwest::StatusCode::UNAUTHORIZED {
        logout();
        emit(json!({ "type": "signed-out" }));
    }
    parse(r).await
}

pub async fn get(path: &str) -> Result<Value> {
    call(reqwest::Method::GET, path, None).await
}

pub async fn post(path: &str, body: Value) -> Result<Value> {
    call(reqwest::Method::POST, path, Some(body)).await
}

pub async fn delete(path: &str) -> Result<Value> {
    call(reqwest::Method::DELETE, path, None).await
}

pub async fn put(path: &str, body: Value) -> Result<Value> {
    call(reqwest::Method::PUT, path, Some(body)).await
}

/// Uploads a clip (or picture) and returns its public URL info.
pub async fn upload(path: &str) -> Result<Value> {
    let s = session()?;
    let p = std::path::Path::new(path);
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let file = tokio::fs::File::open(p).await?;
    let len = file.metadata().await?.len();
    if len > 300 * 1024 * 1024 {
        bail!(l!("Soubor je větší než 300 MB", "The file is over 300 MB"));
    }
    linfo!("social: uploading {name} ({len} B)");
    let r = net::CLIENT
        .post(format!("{BASE}/upload?name={}", urlencoding::encode(&name)))
        .bearer_auth(&s.token)
        .header("content-length", len)
        .body(reqwest::Body::from(file))
        .timeout(Duration::from_secs(1800))
        .send()
        .await?;
    parse(r).await
}

/// What the player is doing, shown to friends ("playing PvP 1.21.11 on justicesmp").
pub fn set_presence(p: Value) {
    *PRESENCE.lock().unwrap() = p.clone();
    send_frame(json!({ "type": "presence", "presence": p }));
}

/// Justice Mod reports the server the game is on now (null = singleplayer/menu), so friends and the
/// party follow the player when they switch servers in-game.
pub fn set_current_server(server: Option<&str>) {
    let mut p = PRESENCE.lock().unwrap().clone();
    if p["state"] != "playing" || p["server"].as_str() == server {
        return;
    }
    p["server"] = json!(server);
    set_presence(p);
}

/// Party leader: send everyone to the server we're on, or into the world we're sharing.
pub async fn warp_here() -> Result<Value> {
    let p = PRESENCE.lock().unwrap().clone();
    let target = if let Some(h) = crate::world::hosted() {
        json!({ "kind": "world", "world": h.id, "name": h.name, "mc": p["mc"], "loader": p["loader"] })
    } else if let Some(server) = p["server"].as_str().filter(|_| p["state"] == "playing") {
        json!({ "kind": "server", "address": server, "mc": p["mc"], "loader": p["loader"], "instance": p["instance"] })
    } else {
        bail!(l!(
            "Nejdřív se připoj na server nebo otevři svět přátelům — pak k sobě přesuneš celou partu",
            "Join a server or open your world to friends first — then you can bring the whole party"
        ));
    };
    post("/party/warp", json!({ "target": target })).await
}

pub fn typing(to: &str) {
    send_frame(json!({ "type": "typing", "to": to }));
}

fn send_frame(v: Value) {
    if let Some(tx) = OUT.lock().unwrap().as_ref() {
        let _ = tx.send(v.to_string());
    }
}

/// Keeps one WebSocket open while signed in, reconnecting with back-off.
fn connect(s: Session) {
    let gen = GEN.fetch_add(1, Ordering::SeqCst) + 1;
    tauri::async_runtime::spawn(async move {
        let mut delay = 2u64;
        while GEN.load(Ordering::SeqCst) == gen {
            match run_socket(&s, gen).await {
                Ok(()) => delay = 2,
                Err(e) => lwarn!("social: connection lost: {e:#}"),
            }
            if GEN.load(Ordering::SeqCst) != gen {
                break;
            }
            emit(json!({ "type": "offline" }));
            tokio::time::sleep(Duration::from_secs(delay)).await;
            delay = (delay * 2).min(60);
        }
    });
}

async fn run_socket(s: &Session, gen: u64) -> Result<()> {
    let (ws, _) = tokio_tungstenite::connect_async(format!("{WS}?token={}", s.token)).await?;
    let (mut sink, mut stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();
    *OUT.lock().unwrap() = Some(tx.clone());
    let _ = tx.send(json!({ "type": "presence", "presence": PRESENCE.lock().unwrap().clone() }).to_string());
    emit(json!({ "type": "connected" }));
    ldebug!("social: connected");
    loop {
        tokio::select! {
            out = rx.recv() => match out {
                Some(text) => sink.send(tokio_tungstenite::tungstenite::Message::Text(text)).await?,
                None => return Ok(()),
            },
            msg = stream.next() => match msg {
                Some(Ok(tokio_tungstenite::tungstenite::Message::Text(t))) => {
                    if let Ok(v) = serde_json::from_str::<Value>(&t) {
                        emit(v);
                    }
                }
                Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(p))) => sink.send(tokio_tungstenite::tungstenite::Message::Pong(p)).await?,
                Some(Ok(tokio_tungstenite::tungstenite::Message::Close(_))) | None => return Ok(()),
                Some(Err(e)) => return Err(e.into()),
                _ => {}
            },
            _ = tokio::time::sleep(Duration::from_secs(5)) => {
                if GEN.load(Ordering::SeqCst) != gen {
                    return Ok(());
                }
            }
        }
    }
}
