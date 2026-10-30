//! Now playing + controls for any music app: Windows' media controls (what the media keys and the
//! volume flyout use) or MPRIS on Linux. Sonolith (formerly ytdesk), Spotify, YouTube in a browser…
//! Title, artist, cover, position, play/pause/next/previous/seek. Playlists and the queue aren't exposed there.

use anyhow::{anyhow, Result};
use serde::Serialize;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    /// "Sonolith", "Spotify", "Firefox"… from the app id.
    pub source: String,
    pub title: String,
    pub playing: bool,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub id: String,
    pub source: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    /// data: URL of the cover; left out (None) when the caller already has the cover for `thumb_key`.
    pub thumb: Option<String>,
    /// Identifies the cover (app + title + artist), so it's only sent when it changes.
    pub thumb_key: String,
    pub position: f64,
    pub duration: f64,
    pub playing: bool,
    pub can_next: bool,
    pub can_prev: bool,
    pub can_seek: bool,
    pub sessions: Vec<Session>,
}

/// Friendly name for a Windows app id. Sonolith (formerly ytdesk) is a webview app, so it shows up as msedgewebview2.
fn source_name(aumid: &str) -> String {
    let a = aumid.to_lowercase();
    let known = [
        ("msedgewebview2", "Sonolith"),
        ("sonolith", "Sonolith"),
        ("ytdesk", "Sonolith"),
        ("spotify", "Spotify"),
        ("308046b0af4a39cb", "Firefox"),
        ("firefox", "Firefox"),
        ("chrome", "Chrome"),
        ("msedge", "Edge"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("zunemusic", "Media Player"),
        ("vlc", "VLC"),
    ];
    known.iter().find(|(k, _)| a.contains(k)).map(|(_, v)| v.to_string()).unwrap_or_else(|| aumid.trim_end_matches(".exe").to_string())
}

#[cfg(windows)]
mod imp {
    use super::*;
    use base64::Engine;
    use once_cell::sync::Lazy;
    use std::sync::Mutex;
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSession as Sess, GlobalSystemMediaTransportControlsSessionManager as Mgr,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

    /// (session id, title, artist) -> cover data URL, so the cover isn't re-read every second.
    static THUMB: Lazy<Mutex<Option<((String, String, String), Option<String>, std::time::Instant)>>> = Lazy::new(|| Mutex::new(None));

    fn manager() -> Result<Mgr> {
        Ok(Mgr::RequestAsync()?.get()?)
    }

    fn playing(s: &Sess) -> bool {
        s.GetPlaybackInfo().and_then(|p| p.PlaybackStatus()).map(|st| st == Status::Playing).unwrap_or(false)
    }

    fn read_thumb(r: &IRandomAccessStreamReference) -> Result<String> {
        let stream = r.OpenReadAsync()?.get()?;
        let size = stream.Size()? as u32;
        let ctype = stream.ContentType().map(|c| c.to_string()).unwrap_or_default();
        let reader = DataReader::CreateDataReader(&stream)?;
        reader.LoadAsync(size)?.get()?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf)?;
        let ctype = if ctype.is_empty() { "image/png".to_string() } else { ctype };
        Ok(format!("data:{ctype};base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
    }

    fn pick(mgr: &Mgr, prefer: Option<&str>) -> Result<Option<Sess>> {
        let all: Vec<Sess> = mgr.GetSessions()?.into_iter().collect();
        if let Some(id) = prefer {
            if let Some(s) = all.iter().find(|s| s.SourceAppUserModelId().map(|x| x.to_string() == id).unwrap_or(false)) {
                return Ok(Some(s.clone()));
            }
        }
        // Playing beats paused; Windows' "current" session breaks the tie; Sonolith first otherwise.
        if let Some(s) = all.iter().find(|s| playing(s)) {
            return Ok(Some(s.clone()));
        }
        if let Ok(s) = mgr.GetCurrentSession() {
            return Ok(Some(s));
        }
        Ok(all.iter().find(|s| s.SourceAppUserModelId().map(|x| source_name(&x.to_string()) == "Sonolith").unwrap_or(false)).or(all.first()).cloned())
    }

    pub fn state(prefer: Option<&str>, known_thumb: Option<&str>) -> Result<Option<MediaState>> {
        let mgr = manager()?;
        let sessions = mgr
            .GetSessions()?
            .into_iter()
            .map(|s| {
                let id = s.SourceAppUserModelId().map(|x| x.to_string()).unwrap_or_default();
                let title = s.TryGetMediaPropertiesAsync().and_then(|o| o.get()).and_then(|p| p.Title()).map(|t| t.to_string()).unwrap_or_default();
                Session { source: source_name(&id), id, title, playing: playing(&s) }
            })
            .collect::<Vec<_>>();
        let Some(s) = pick(&mgr, prefer)? else { return Ok(None) };
        let id = s.SourceAppUserModelId()?.to_string();
        let props = s.TryGetMediaPropertiesAsync()?.get()?;
        let title = props.Title()?.to_string();
        let artist = props.Artist()?.to_string();
        let album = props.AlbumTitle().map(|a| a.to_string()).unwrap_or_default();
        let key = (id.clone(), title.clone(), artist.clone());
        let thumb_key = format!("{id}|{title}|{artist}");
        // known_thumb "*" = the caller doesn't want the cover at all (the in-game HUD): reading it
        // can be slow or fail, and that must never hold up the title.
        let thumb = if known_thumb == Some("*") {
            None
        } else {
            let mut cache = THUMB.lock().unwrap();
            match cache.as_ref() {
                Some((k, t, _)) if *k == key && t.is_some() => t.clone(),
                // Couldn't read it a moment ago: don't hammer the media session every poll.
                Some((k, None, at)) if *k == key && at.elapsed() < std::time::Duration::from_secs(4) => None,
                _ => {
                    let t = props.Thumbnail().ok().and_then(|r| read_thumb(&r).ok());
                    *cache = Some((key, t.clone(), std::time::Instant::now()));
                    t
                }
            }
        };
        let tl = s.GetTimelineProperties()?;
        let secs = |d: windows::Foundation::TimeSpan| d.Duration as f64 / 10_000_000.0;
        let mut position = secs(tl.Position()?);
        let duration = secs(tl.EndTime()?) - secs(tl.StartTime()?);
        let info = s.GetPlaybackInfo()?;
        let playing = info.PlaybackStatus()? == Status::Playing;
        // The timeline is only updated now and then; move it on by the time since its last update.
        if playing {
            let updated = tl.LastUpdatedTime()?.UniversalTime; // 100 ns ticks since 1601
            let now = (std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos() / 100) as i64 + 116_444_736_000_000_000;
            if updated > 0 && now > updated {
                position += (now - updated) as f64 / 10_000_000.0;
            }
        }
        let ctl = info.Controls()?;
        Ok(Some(MediaState {
            source: source_name(&id),
            id,
            title,
            artist,
            album,
            thumb: if known_thumb == Some(thumb_key.as_str()) { None } else { thumb },
            thumb_key,
            position: if duration > 0.0 { position.min(duration) } else { position },
            duration: duration.max(0.0),
            playing,
            can_next: ctl.IsNextEnabled().unwrap_or(true),
            can_prev: ctl.IsPreviousEnabled().unwrap_or(true),
            can_seek: ctl.IsPlaybackPositionEnabled().unwrap_or(false),
            sessions,
        }))
    }

    pub fn command(cmd: &str, value: Option<f64>, prefer: Option<&str>) -> Result<()> {
        let mgr = manager()?;
        let s = pick(&mgr, prefer)?.ok_or_else(|| anyhow!(l!("Nic nehraje", "Nothing is playing")))?;
        let ok = match cmd {
            "playPause" => s.TryTogglePlayPauseAsync()?.get()?,
            "next" => s.TrySkipNextAsync()?.get()?,
            "previous" => s.TrySkipPreviousAsync()?.get()?,
            "seek" => s.TryChangePlaybackPositionAsync((value.unwrap_or(0.0) * 10_000_000.0) as i64)?.get()?,
            _ => return Err(anyhow!("unknown media command {cmd}")),
        };
        if !ok {
            ldebug!("media: {cmd} refused by {}", s.SourceAppUserModelId().map(|x| x.to_string()).unwrap_or_default());
        }
        Ok(())
    }
}

#[cfg(windows)]
pub use imp::{command, state};

/// Linux: MPRIS players on the session D-Bus (Sonolith, Spotify, browsers, VLC…).
#[cfg(target_os = "linux")]
mod mpris {
    use super::*;
    use base64::Engine;
    use once_cell::sync::Lazy;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use zbus::blocking::{fdo::DBusProxy, Connection, Proxy};
    use zbus::zvariant::{ObjectPath, OwnedValue};

    const PATH: &str = "/org/mpris/MediaPlayer2";
    const PLAYER: &str = "org.mpris.MediaPlayer2.Player";

    static CONN: Lazy<Mutex<Option<Connection>>> = Lazy::new(|| Mutex::new(None));
    /// (bus name, title, artist) -> cover, so a local file isn't re-read every second.
    static THUMB: Lazy<Mutex<Option<((String, String, String), Option<String>)>>> = Lazy::new(|| Mutex::new(None));

    fn conn() -> Result<Connection> {
        let mut c = CONN.lock().unwrap();
        if c.is_none() {
            *c = Some(Connection::session()?);
        }
        Ok(c.clone().unwrap())
    }

    fn players(c: &Connection) -> Result<Vec<String>> {
        Ok(DBusProxy::new(c)?.list_names()?.into_iter().map(|n| n.to_string()).filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).collect())
    }

    fn proxy<'a>(c: &'a Connection, name: &str, iface: &'static str) -> Result<Proxy<'a>> {
        Ok(Proxy::new(c, name.to_string(), PATH, iface)?)
    }

    fn identity(c: &Connection, name: &str) -> String {
        let fallback = name.trim_start_matches("org.mpris.MediaPlayer2.").split('.').next().unwrap_or(name).to_string();
        let id = proxy(c, name, "org.mpris.MediaPlayer2").ok().and_then(|p| p.get_property::<String>("Identity").ok()).unwrap_or(fallback);
        if id.to_lowercase().contains("ytdesk") { "Sonolith".into() } else { id }
    }

    fn playing(c: &Connection, name: &str) -> bool {
        proxy(c, name, PLAYER).ok().and_then(|p| p.get_property::<String>("PlaybackStatus").ok()).is_some_and(|s| s == "Playing")
    }

    fn meta(c: &Connection, name: &str) -> HashMap<String, OwnedValue> {
        proxy(c, name, PLAYER).ok().and_then(|p| p.get_property::<HashMap<String, OwnedValue>>("Metadata").ok()).unwrap_or_default()
    }

    fn s(m: &HashMap<String, OwnedValue>, k: &str) -> String {
        m.get(k).and_then(|v| v.try_clone().ok()).and_then(|v| String::try_from(v).ok()).unwrap_or_default()
    }

    fn artists(m: &HashMap<String, OwnedValue>) -> String {
        m.get("xesam:artist").and_then(|v| v.try_clone().ok()).and_then(|v| Vec::<String>::try_from(v).ok()).map(|a| a.join(", ")).unwrap_or_default()
    }

    fn micros(v: Option<&OwnedValue>) -> f64 {
        let Some(v) = v.and_then(|v| v.try_clone().ok()) else { return 0.0 };
        if let Ok(x) = i64::try_from(v.try_clone().unwrap_or(v.clone())) {
            return x as f64 / 1e6;
        }
        u64::try_from(v).map(|x| x as f64 / 1e6).unwrap_or(0.0)
    }

    fn cover(url: &str) -> Option<String> {
        if url.starts_with("http://") || url.starts_with("https://") || url.starts_with("data:") {
            return Some(url.to_string());
        }
        let path = url.strip_prefix("file://")?;
        let path = percent_decode(path);
        let bytes = std::fs::read(&path).ok()?;
        let mime = if path.ends_with(".png") { "image/png" } else { "image/jpeg" };
        Some(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
    }

    fn percent_decode(s: &str) -> String {
        let b = s.as_bytes();
        let mut out = Vec::with_capacity(b.len());
        let mut i = 0;
        while i < b.len() {
            if b[i] == b'%' && i + 2 < b.len() {
                if let Ok(x) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                    out.push(x);
                    i += 3;
                    continue;
                }
            }
            out.push(b[i]);
            i += 1;
        }
        String::from_utf8_lossy(&out).into_owned()
    }

    fn pick(c: &Connection, prefer: Option<&str>) -> Result<Option<String>> {
        let all = players(c)?;
        if let Some(p) = prefer.filter(|p| all.iter().any(|n| n == p)) {
            return Ok(Some(p.to_string()));
        }
        if let Some(p) = all.iter().find(|n| playing(c, n)) {
            return Ok(Some(p.clone()));
        }
        Ok(all.iter().find(|n| identity(c, n) == "Sonolith").or(all.first()).cloned())
    }

    pub fn state(prefer: Option<&str>, known_thumb: Option<&str>) -> Result<Option<MediaState>> {
        let c = conn()?;
        let sessions = players(&c)?
            .into_iter()
            .map(|n| Session { source: identity(&c, &n), title: s(&meta(&c, &n), "xesam:title"), playing: playing(&c, &n), id: n })
            .collect::<Vec<_>>();
        let Some(name) = pick(&c, prefer)? else { return Ok(None) };
        let p = proxy(&c, &name, PLAYER)?;
        let m = meta(&c, &name);
        let title = s(&m, "xesam:title");
        let artist = artists(&m);
        let key = (name.clone(), title.clone(), artist.clone());
        let thumb_key = format!("{name}|{title}|{artist}");
        let thumb = if known_thumb == Some("*") {
            None
        } else {
            let mut cache = THUMB.lock().unwrap();
            match cache.as_ref() {
                Some((k, t)) if *k == key => t.clone(),
                _ => {
                    let t = cover(&s(&m, "mpris:artUrl"));
                    *cache = Some((key, t.clone()));
                    t
                }
            }
        };
        let flag = |prop: &str| p.get_property::<bool>(prop).unwrap_or(false);
        let duration = micros(m.get("mpris:length"));
        let position = p.get_property::<i64>("Position").map(|x| x as f64 / 1e6).unwrap_or(0.0);
        Ok(Some(MediaState {
            source: identity(&c, &name),
            title,
            artist,
            album: s(&m, "xesam:album"),
            thumb: if known_thumb == Some(thumb_key.as_str()) { None } else { thumb },
            thumb_key,
            position: if duration > 0.0 { position.min(duration) } else { position },
            duration,
            playing: playing(&c, &name),
            can_next: flag("CanGoNext"),
            can_prev: flag("CanGoPrevious"),
            can_seek: flag("CanSeek"),
            sessions,
            id: name,
        }))
    }

    pub fn command(cmd: &str, value: Option<f64>, prefer: Option<&str>) -> Result<()> {
        let c = conn()?;
        let name = pick(&c, prefer)?.ok_or_else(|| anyhow!(l!("Nic nehraje", "Nothing is playing")))?;
        let p = proxy(&c, &name, PLAYER)?;
        match cmd {
            "playPause" => p.call_method("PlayPause", &())?,
            "next" => p.call_method("Next", &())?,
            "previous" => p.call_method("Previous", &())?,
            "seek" => {
                let m = meta(&c, &name);
                let track = m.get("mpris:trackid").and_then(|v| v.try_clone().ok()).and_then(|v| ObjectPath::try_from(v).ok().map(|p| p.into_owned()));
                let pos = (value.unwrap_or(0.0) * 1e6) as i64;
                match track {
                    Some(t) => p.call_method("SetPosition", &(t, pos))?,
                    // No track id: seek relative to where it is.
                    None => {
                        let now = p.get_property::<i64>("Position").unwrap_or(0);
                        p.call_method("Seek", &(pos - now))?
                    }
                }
            }
            _ => return Err(anyhow!("unknown media command {cmd}")),
        };
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub use mpris::{command, state};

#[cfg(not(any(windows, target_os = "linux")))]
pub fn state(_prefer: Option<&str>, _known_thumb: Option<&str>) -> Result<Option<MediaState>> {
    Ok(None)
}
#[cfg(not(any(windows, target_os = "linux")))]
pub fn command(_cmd: &str, _value: Option<f64>, _prefer: Option<&str>) -> Result<()> {
    Err(anyhow!(l!("Ovládání hudby na tomhle systému zatím nefunguje", "Music control doesn't work on this system yet")))
}

/// Starts the music app (Sonolith, formerly ytdesk) or brings it up, so a song can be picked there.
pub fn open_music_app() -> Result<()> {
    #[cfg(windows)]
    {
        let local = dirs::data_local_dir().ok_or_else(|| anyhow!("no local app data"))?;
        for (dir, exe) in [("Sonolith", "Sonolith.exe"), ("sonolith", "sonolith.exe"), ("ytdesk", "ytdesk.exe")] {
            let p = local.join(dir).join(exe);
            if p.is_file() {
                std::process::Command::new(p).spawn()?;
                return Ok(());
            }
        }
    }
    #[cfg(not(windows))]
    {
        for bin in ["sonolith", "Sonolith", "ytdesk"] {
            if std::process::Command::new(bin).spawn().is_ok() {
                return Ok(());
            }
        }
    }
    Err(anyhow!(l!("Sonolith není nainstalovaný", "Sonolith isn't installed")))
}
