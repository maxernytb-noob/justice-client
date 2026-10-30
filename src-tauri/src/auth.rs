//! Offline accounts and Microsoft (device code) -> Xbox -> Minecraft login.

use crate::net::{self, CLIENT};
use crate::settings;
use anyhow::{anyhow, bail, Result};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Account {
    pub id: String,
    /// "msa" | "offline"
    pub kind: String,
    pub username: String,
    pub uuid: String,
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub xuid: String,
    pub skin_url: String,
    pub skin_variant: String,
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct AccountStore {
    pub active: String,
    pub accounts: Vec<Account>,
}

/// Account as sent to the UI: never includes tokens.
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub id: String,
    pub kind: String,
    pub username: String,
    pub uuid: String,
    pub skin_url: String,
    pub skin_variant: String,
    pub active: bool,
}

fn store_path() -> PathBuf {
    settings::data_dir().join("accounts.json")
}

pub fn load_store() -> AccountStore {
    net::read_json(&store_path())
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

pub fn save_store(s: &AccountStore) -> Result<()> {
    net::write_json(&store_path(), &serde_json::to_value(s)?)
}

pub fn public_list() -> Vec<PublicAccount> {
    let s = load_store();
    s.accounts
        .iter()
        .map(|a| PublicAccount {
            id: a.id.clone(),
            kind: a.kind.clone(),
            username: a.username.clone(),
            uuid: a.uuid.clone(),
            skin_url: a.skin_url.clone(),
            skin_variant: a.skin_variant.clone(),
            active: a.id == s.active,
        })
        .collect()
}

fn upsert(acc: Account) -> Result<()> {
    let mut s = load_store();
    s.accounts.retain(|a| a.id != acc.id);
    s.active = acc.id.clone();
    s.accounts.push(acc);
    save_store(&s)
}

pub fn active_account() -> Result<Account> {
    let s = load_store();
    s.accounts.into_iter().find(|a| a.id == s.active).ok_or_else(|| anyhow!(l!("Nejdřív přidej účet (vpravo nahoře)", "Add an account first (top right)")))
}

pub fn set_active(id: &str) -> Result<()> {
    let mut s = load_store();
    if !s.accounts.iter().any(|a| a.id == id) {
        bail!(l!("Účet neexistuje", "That account doesn't exist"));
    }
    s.active = id.into();
    save_store(&s)
}

pub fn remove(id: &str) -> Result<()> {
    let mut s = load_store();
    s.accounts.retain(|a| a.id != id);
    if s.active == id {
        s.active = s.accounts.first().map(|a| a.id.clone()).unwrap_or_default();
    }
    save_store(&s)
}

pub fn offline_uuid(name: &str) -> String {
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    hex::encode(h)
}

pub fn add_offline(name: &str) -> Result<()> {
    let name = name.trim();
    let valid = (3..=16).contains(&name.len())
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
    if !valid {
        bail!(l!("Jméno musí mít 3–16 znaků (písmena, čísla, _)", "The name must be 3–16 characters (letters, numbers, _)"));
    }
    let uuid = offline_uuid(name);
    upsert(Account {
        id: format!("offline-{uuid}"),
        kind: "offline".into(),
        username: name.into(),
        uuid,
        access_token: "0".into(),
        ..Default::default()
    })
}

// ---------------- Microsoft ----------------

static CANCEL: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCode {
    pub user_code: String,
    pub verification_uri: String,
    pub device_code: String,
    pub interval: u64,
    pub expires_in: u64,
}

fn client_id() -> String {
    let s = settings::get();
    if s.ms_client_id.trim().is_empty() {
        settings::DEFAULT_MS_CLIENT_ID.into()
    } else {
        s.ms_client_id.trim().into()
    }
}

pub async fn start_device_code() -> Result<DeviceCode> {
    CANCEL.store(false, Ordering::SeqCst);
    let r: Value = CLIENT
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode")
        .form(&[("client_id", client_id().as_str()), ("scope", "XboxLive.signin offline_access")])
        .send()
        .await?
        .json()
        .await?;
    if let Some(err) = r["error_description"].as_str() {
        bail!("Microsoft: {err}");
    }
    Ok(DeviceCode {
        user_code: r["user_code"].as_str().unwrap_or_default().into(),
        verification_uri: r["verification_uri"].as_str().unwrap_or("https://microsoft.com/link").into(),
        device_code: r["device_code"].as_str().unwrap_or_default().into(),
        interval: r["interval"].as_u64().unwrap_or(5),
        expires_in: r["expires_in"].as_u64().unwrap_or(900),
    })
}

pub fn cancel_login() {
    CANCEL.store(true, Ordering::SeqCst);
}

pub async fn poll_device_code(device_code: &str, interval: u64) -> Result<PublicAccount> {
    let mut wait = interval.max(1);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(900);
    let tokens = loop {
        tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
        if CANCEL.load(Ordering::SeqCst) {
            bail!(l!("Přihlášení zrušeno", "Sign-in cancelled"));
        }
        if std::time::Instant::now() > deadline {
            bail!(l!("Kód vypršel, zkus to znovu", "The code expired, try again"));
        }
        let r: Value = CLIENT
            .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
                ("client_id", client_id().as_str()),
                ("device_code", device_code),
            ])
            .send()
            .await?
            .json()
            .await?;
        match r["error"].as_str() {
            None => break r,
            Some("authorization_pending") => continue,
            Some("slow_down") => wait += 5,
            Some("authorization_declined") => bail!(l!("Přihlášení bylo odmítnuto", "Sign-in was declined")),
            Some("expired_token") => bail!(l!("Kód vypršel, zkus to znovu", "The code expired, try again")),
            Some(e) => bail!("Microsoft: {}", r["error_description"].as_str().unwrap_or(e)),
        }
    };
    let acc = finish_login(&tokens).await?;
    upsert(acc.clone())?;
    Ok(public_list().into_iter().find(|a| a.id == acc.id).unwrap())
}

async fn finish_login(ms: &Value) -> Result<Account> {
    let ms_token = ms["access_token"].as_str().ok_or_else(|| anyhow!(l!("chybí MS token", "missing MS token")))?;
    let refresh = ms["refresh_token"].as_str().unwrap_or_default().to_string();

    let xbl: Value = CLIENT
        .post("https://user.auth.xboxlive.com/user/authenticate")
        .json(&json!({
            "Properties": {"AuthMethod": "RPS", "SiteName": "user.auth.xboxlive.com", "RpsTicket": format!("d={ms_token}")},
            "RelyingParty": "http://auth.xboxlive.com",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json()
        .await?;
    let xbl_token = xbl["Token"].as_str().ok_or_else(|| anyhow!(l!("Xbox Live přihlášení selhalo", "Xbox Live sign-in failed")))?;

    let xsts: Value = CLIENT
        .post("https://xsts.auth.xboxlive.com/xsts/authorize")
        .json(&json!({
            "Properties": {"SandboxId": "RETAIL", "UserTokens": [xbl_token]},
            "RelyingParty": "rp://api.minecraftservices.com/",
            "TokenType": "JWT"
        }))
        .send()
        .await?
        .json()
        .await?;
    if let Some(code) = xsts["XErr"].as_u64() {
        bail!(match code {
            2148916233 => l!(
                "Tento Microsoft účet nemá Xbox profil. Přihlas se jednou na minecraft.net a zkus to znovu.",
                "This Microsoft account has no Xbox profile. Sign in once at minecraft.net and try again."
            ),
            2148916235 => l!("Xbox Live není ve tvé zemi dostupný.", "Xbox Live isn't available in your country."),
            2148916236 | 2148916237 => l!("Účet potřebuje ověření dospělosti na xbox.com.", "The account needs adult verification on xbox.com."),
            2148916238 => l!(
                "Dětský účet — rodič ho musí přidat do Microsoft Family.",
                "Child account — a parent has to add it to a Microsoft Family."
            ),
            _ => l!("Xbox (XSTS) přihlášení selhalo.", "Xbox (XSTS) sign-in failed."),
        });
    }
    let xsts_token = xsts["Token"].as_str().ok_or_else(|| anyhow!(l!("XSTS selhalo", "XSTS failed")))?;
    let uhs = xsts["DisplayClaims"]["xui"][0]["uhs"].as_str().unwrap_or_default();
    let xuid = xsts["DisplayClaims"]["xui"][0]["xid"].as_str().unwrap_or_default().to_string();

    let identity = format!("XBL3.0 x={uhs};{xsts_token}");
    let resp = CLIENT
        .post("https://api.minecraftservices.com/authentication/login_with_xbox")
        .json(&json!({"identityToken": identity}))
        .send()
        .await?;
    let status = resp.status();
    let mut mc: Value = resp.json().await.unwrap_or(Value::Null);
    // Mojang has handed out Bedrock/PlayFab tokens here (no Java profile inside): the profile API
    // accepts them, but every online server says "Invalid session". Ask the official launcher's
    // endpoint instead, and never keep such a token.
    if status.is_success() && !is_java_token(mc["access_token"].as_str().unwrap_or_default()) {
        lwarn!("auth: login_with_xbox returned a token without a Java profile, trying launcher/login");
        let alt: Value = CLIENT
            .post("https://api.minecraftservices.com/launcher/login")
            .json(&json!({"xtoken": identity, "platform": "PC_LAUNCHER"}))
            .send()
            .await?
            .json()
            .await
            .unwrap_or(Value::Null);
        if !is_java_token(alt["access_token"].as_str().unwrap_or_default()) {
            bail!(l!(
                "Mojang teď vydává vadné přihlášení (servery by hlásily Invalid session). Zkus to za pár minut znovu.",
                "Mojang is handing out broken sign-ins right now (servers would say Invalid session). Try again in a few minutes."
            ));
        }
        mc = alt;
    }
    if !status.is_success() {
        let msg = mc["errorMessage"].as_str().or(mc["error"].as_str()).unwrap_or_default();
        if status.as_u16() == 403 || msg.to_lowercase().contains("invalid app registration") {
            bail!(l!(
                "Microsoft ještě neschválil Justice Client pro Minecraft (AppID review). Do té doby použij offline účet.",
                "Microsoft hasn't approved Justice Client for Minecraft yet (AppID review). Use an offline account until then."
            ));
        }
        bail!(l!("Minecraft přihlášení selhalo ({status}) {msg}", "Minecraft sign-in failed ({status}) {msg}"));
    }
    let mc_token = mc["access_token"].as_str().ok_or_else(|| anyhow!(l!("chybí MC token", "missing MC token")))?.to_string();
    let expires = chrono::Utc::now().timestamp() + mc["expires_in"].as_i64().unwrap_or(86400) - 300;

    let profile = get_profile(&mc_token).await?;
    let (skin_url, skin_variant) = active_skin(&profile);
    let uuid = profile["id"].as_str().unwrap_or_default().to_string();
    Ok(Account {
        id: format!("msa-{uuid}"),
        kind: "msa".into(),
        username: profile["name"].as_str().unwrap_or_default().into(),
        uuid,
        access_token: mc_token,
        refresh_token: refresh,
        expires_at: expires,
        xuid,
        skin_url,
        skin_variant,
    })
}

/// A Minecraft access token is a JWT; a Java one names the profile in `profiles.mc`.
fn is_java_token(token: &str) -> bool {
    use base64::Engine;
    token
        .split('.')
        .nth(1)
        .and_then(|p| base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(p.trim_end_matches('=')).ok())
        .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
        .map(|c| c["profiles"]["mc"].is_string())
        .unwrap_or(false)
}

fn active_skin(profile: &Value) -> (String, String) {
    profile["skins"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["state"] == "ACTIVE"))
        .map(|s| {
            (
                s["url"].as_str().unwrap_or_default().to_string(),
                s["variant"].as_str().unwrap_or("CLASSIC").to_string(),
            )
        })
        .unwrap_or_default()
}

pub async fn get_profile(token: &str) -> Result<Value> {
    let r = CLIENT
        .get("https://api.minecraftservices.com/minecraft/profile")
        .bearer_auth(token)
        .send()
        .await?;
    if r.status().as_u16() == 404 {
        bail!(l!("Tento účet nevlastní Minecraft Java Edition.", "This account doesn't own Minecraft Java Edition."));
    }
    if !r.status().is_success() {
        bail!(l!("Nepodařilo se načíst profil ({})", "Couldn't load the profile ({})", r.status()));
    }
    Ok(r.json().await?)
}

/// Returns a launch-ready account, refreshing Microsoft tokens when expired.
pub async fn ready_account(id: Option<&str>) -> Result<Account> {
    let s = load_store();
    let id = id.map(String::from).unwrap_or(s.active.clone());
    let acc = s
        .accounts
        .iter()
        .find(|a| a.id == id)
        .cloned()
        .ok_or_else(|| anyhow!(l!("Nejdřív přidej účet (vpravo nahoře)", "Add an account first (top right)")))?;
    // A token without a Java profile (see finish_login) can't join servers, so it's replaced even
    // before it expires.
    if acc.kind != "msa" || (chrono::Utc::now().timestamp() < acc.expires_at && is_java_token(&acc.access_token)) {
        return Ok(acc);
    }
    let r: Value = CLIENT
        .post("https://login.microsoftonline.com/consumers/oauth2/v2.0/token")
        .form(&[
            ("grant_type", "refresh_token"),
            ("client_id", client_id().as_str()),
            ("refresh_token", acc.refresh_token.as_str()),
            ("scope", "XboxLive.signin offline_access"),
        ])
        .send()
        .await?
        .json()
        .await?;
    if r["error"].is_string() {
        bail!(l!("Přihlášení vypršelo — přihlas se znovu k účtu {}", "Your sign-in expired — sign in to {} again", acc.username));
    }
    let fresh = finish_login(&r).await?;
    upsert(fresh.clone())?;
    Ok(fresh)
}

// ---------------- Skins & capes ----------------

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Cape {
    pub id: String,
    pub alias: String,
    pub url: String,
    pub active: bool,
}

pub async fn capes(account_id: &str) -> Result<Vec<Cape>> {
    let acc = ready_account(Some(account_id)).await?;
    if acc.kind != "msa" {
        return Ok(vec![]);
    }
    let p = get_profile(&acc.access_token).await?;
    Ok(p["capes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| Cape {
            id: c["id"].as_str().unwrap_or_default().into(),
            alias: c["alias"].as_str().unwrap_or_default().into(),
            url: c["url"].as_str().unwrap_or_default().into(),
            active: c["state"] == "ACTIVE",
        })
        .collect())
}

pub async fn set_cape(account_id: &str, cape_id: Option<&str>) -> Result<()> {
    let acc = ready_account(Some(account_id)).await?;
    if acc.kind != "msa" {
        bail!(l!("Minecraft capes má jen Microsoft účet — offline účet si vybere Justice plášť v profilu u přátel", "Only Microsoft accounts have Minecraft capes — offline accounts pick a Justice cape in their friends profile"));
    }
    let url = "https://api.minecraftservices.com/minecraft/profile/capes/active";
    let r = match cape_id {
        Some(id) => CLIENT.put(url).bearer_auth(&acc.access_token).json(&json!({"capeId": id})).send().await?,
        None => CLIENT.delete(url).bearer_auth(&acc.access_token).send().await?,
    };
    if !r.status().is_success() {
        bail!(l!("Změna capes selhala ({})", "Changing the cape failed ({})", r.status()));
    }
    Ok(())
}

/// Skins of offline accounts: served by Justice Social under their MD5, and kept here too so the
/// launcher preview and your own player in-game work before (or without) the upload.
pub const JUSTICE_SKINS: &str = "https://justiceclient.online/u/skins/";

pub fn skins_dir() -> PathBuf {
    settings::data_dir().join("skins")
}

/// The local copy of a Justice skin URL, if we have it.
pub fn skin_file(url: &str) -> Option<PathBuf> {
    let name = url.strip_prefix(JUSTICE_SKINS)?;
    if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.') || name.starts_with('.') {
        return None;
    }
    let p = skins_dir().join(name);
    p.is_file().then_some(p)
}

/// The local file behind an offline account's Justice skin, if it has one.
pub fn local_skin(acc: &Account) -> Option<PathBuf> {
    skin_file(&acc.skin_url)
}

pub(crate) fn check_skin(bytes: &[u8]) -> Result<()> {
    let png = bytes.len() >= 24 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" && &bytes[12..16] == b"IHDR";
    if !png {
        bail!(l!("Skin musí být PNG obrázek", "The skin must be a PNG image"));
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into()?);
    if w != 64 || (h != 64 && h != 32) {
        bail!(l!("Skin musí mít 64×64 (nebo starý 64×32) pixelů, tenhle má {}×{}", "The skin must be 64×64 (or legacy 64×32) pixels, this one is {}×{}", w, h));
    }
    if bytes.len() > 64 * 1024 {
        bail!(l!("Soubor skinu je moc velký", "The skin file is too large"));
    }
    Ok(())
}

pub async fn upload_skin(account_id: &str, path: &str, variant: &str) -> Result<()> {
    let bytes = std::fs::read(path)?;
    apply_skin(account_id, bytes, variant).await
}

/// Sets the skin from PNG bytes: Microsoft accounts at Mojang, offline accounts on Justice.
pub(crate) async fn apply_skin(account_id: &str, bytes: Vec<u8>, variant: &str) -> Result<()> {
    check_skin(&bytes)?;
    let acc = ready_account(Some(account_id)).await?;
    if acc.kind != "msa" {
        let hash = hex::encode(Md5::digest(&bytes));
        std::fs::create_dir_all(skins_dir())?;
        std::fs::write(skins_dir().join(format!("{hash}.png")), &bytes)?;
        let updated = set_skin_fields(&acc.id, format!("{JUSTICE_SKINS}{hash}.png"), if variant.eq_ignore_ascii_case("slim") { "SLIM" } else { "CLASSIC" })?;
        crate::social::push_skin(&updated).await;
        return Ok(());
    }
    let form = reqwest::multipart::Form::new().text("variant", variant.to_string()).part(
        "file",
        reqwest::multipart::Part::bytes(bytes).file_name("skin.png").mime_str("image/png")?,
    );
    let r = CLIENT
        .post("https://api.minecraftservices.com/minecraft/profile/skins")
        .bearer_auth(&acc.access_token)
        .multipart(form)
        .send()
        .await?;
    if !r.status().is_success() {
        bail!(l!("Nahrání skinu selhalo ({})", "Uploading the skin failed ({})", r.status()));
    }
    let p = get_profile(&acc.access_token).await?;
    let (skin_url, skin_variant) = active_skin(&p);
    set_skin_fields(&acc.id, skin_url, &skin_variant)?;
    Ok(())
}

/// Saves the account's skin and returns the updated account.
fn set_skin_fields(account_id: &str, url: String, variant: &str) -> Result<Account> {
    let mut s = load_store();
    let a = s.accounts.iter_mut().find(|a| a.id == account_id).ok_or_else(|| anyhow!(l!("Účet neexistuje", "That account doesn't exist")))?;
    a.skin_url = url;
    a.skin_variant = variant.to_string();
    let a = a.clone();
    save_store(&s)?;
    Ok(a)
}

/// Copies the skin (and arm width) of any premium player by name.
pub async fn skin_from_player(account_id: &str, name: &str) -> Result<()> {
    let (bytes, variant) = fetch_player_skin(name).await?;
    apply_skin(account_id, bytes, &variant).await
}

/// Downloads a premium player's skin PNG + arm variant by name (for the skin library and copy-skin).
pub async fn fetch_player_skin(name: &str) -> Result<(Vec<u8>, String)> {
    let name = name.trim();
    if !(3..=16).contains(&name.len()) || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!(l!("Zadej herní jméno (3–16 znaků)", "Enter a player name (3–16 characters)"));
    }
    let r = CLIENT.get(format!("https://api.mojang.com/users/profiles/minecraft/{name}")).send().await?;
    if r.status().as_u16() == 404 || r.status().as_u16() == 204 {
        bail!(l!("Hráč {} neexistuje (nebo nemá koupený Minecraft)", "Player {} doesn't exist (or doesn't own Minecraft)", name));
    }
    if !r.status().is_success() {
        bail!(l!("Mojang neodpověděl ({})", "Mojang didn't answer ({})", r.status()));
    }
    let id = r.json::<Value>().await?["id"].as_str().unwrap_or_default().to_string();
    let profile = net::get_json(&format!("https://sessionserver.mojang.com/session/minecraft/profile/{id}")).await?;
    use base64::Engine;
    let textures: Value = profile["properties"]
        .as_array()
        .and_then(|a| a.iter().find(|p| p["name"] == "textures"))
        .and_then(|p| p["value"].as_str())
        .and_then(|v| base64::engine::general_purpose::STANDARD.decode(v).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or(Value::Null);
    let skin = &textures["textures"]["SKIN"];
    let url = skin["url"].as_str().ok_or_else(|| anyhow!(l!("{} má výchozí skin", "{} uses a default skin", name)))?;
    let variant = if skin["metadata"]["model"] == "slim" { "slim" } else { "classic" };
    let bytes = CLIENT.get(url).send().await?.error_for_status()?.bytes().await?.to_vec();
    Ok((bytes, variant.to_string()))
}

/// The account's current skin PNG bytes + variant ("slim"/"classic"), for saving into the library.
pub async fn account_skin_bytes(account_id: &str) -> Result<(Vec<u8>, String)> {
    let acc = ready_account(Some(account_id)).await?;
    if acc.skin_url.is_empty() {
        bail!(l!("Tenhle účet má výchozí skin", "This account uses the default skin"));
    }
    let variant = if acc.skin_variant.eq_ignore_ascii_case("slim") { "slim" } else { "classic" };
    let bytes = if let Some(file) = skin_file(&acc.skin_url) {
        std::fs::read(file)?
    } else {
        CLIENT.get(&acc.skin_url).send().await?.error_for_status()?.bytes().await?.to_vec()
    };
    Ok((bytes, variant.to_string()))
}

/// Back to the default Steve/Alex skin.
pub async fn reset_skin(account_id: &str) -> Result<()> {
    let acc = ready_account(Some(account_id)).await?;
    if acc.kind == "msa" {
        let r = CLIENT.delete("https://api.minecraftservices.com/minecraft/profile/skins/active").bearer_auth(&acc.access_token).send().await?;
        if !r.status().is_success() {
            bail!(l!("Reset skinu selhal ({})", "Resetting the skin failed ({})", r.status()));
        }
        let (url, variant) = active_skin(&get_profile(&acc.access_token).await?);
        set_skin_fields(&acc.id, url, &variant)?;
        return Ok(());
    }
    let acc = set_skin_fields(&acc.id, String::new(), "")?;
    crate::social::push_skin(&acc).await;
    Ok(())
}
