//! Builds the Java command line for an instance and supervises the running game.

use crate::auth::{self, Account};
use crate::events;
use crate::instances::{self, Instance};
use crate::java;
use crate::loaders;
use crate::minecraft;
use crate::settings;
use crate::{compat, crash, integrations, justice_mod, platform, recording};
use anyhow::{bail, Result};
use once_cell::sync::Lazy;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Mutex;
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, BufReader};

const MAX_LOG_LINES: usize = 3000;

static RUNNING: Lazy<Mutex<HashMap<String, u32>>> = Lazy::new(Default::default);
static LOGS: Lazy<Mutex<HashMap<String, VecDeque<String>>>> = Lazy::new(Default::default);
/// Instances stopped from the launcher — their non-zero exit code is not a crash.
static KILLED: Lazy<Mutex<std::collections::HashSet<String>>> = Lazy::new(Default::default);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct LogLine {
    instance: String,
    line: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameExit {
    pub instance: String,
    pub code: Option<i32>,
    pub seconds: u64,
    pub crash: Option<crash::CrashReport>,
}

pub fn running_ids() -> Vec<String> {
    RUNNING.lock().unwrap().keys().cloned().collect()
}

pub fn logs(id: &str) -> Vec<String> {
    LOGS.lock().unwrap().get(id).map(|l| l.iter().cloned().collect()).unwrap_or_default()
}

pub fn kill(id: &str) -> Result<()> {
    let pid = RUNNING.lock().unwrap().get(id).copied();
    if let Some(pid) = pid {
        KILLED.lock().unwrap().insert(id.to_string());
        platform::kill_pid(pid);
    }
    Ok(())
}

/// Installs the loader version + all files for an instance. Returns the version id.
pub async fn prepare(inst: &mut Instance) -> Result<(String, Value)> {
    let task = inst.id.clone();
    let needs_install = inst.installed_id.is_empty()
        || !minecraft::version_json_path(&inst.installed_id).exists();
    if needs_install {
        events::progress(&task, &l!("Připravuji verzi", "Preparing version"), 0, 1);
        let lv = (!inst.loader_version.is_empty()).then_some(inst.loader_version.as_str());
        let id = loaders::install(&inst.loader, &inst.mc_version, lv, &task).await?;
        inst.installed_id = id;
        instances::save(inst)?;
    }
    let version = minecraft::load_merged(&inst.installed_id).await?;
    Ok((inst.installed_id.clone(), version))
}

fn substitute(s: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = s.to_string();
    if out.contains("${") {
        for (k, v) in vars {
            out = out.replace(&format!("${{{k}}}"), v);
        }
    }
    out
}

fn collect_args(list: Option<&Value>, features: &HashMap<&str, bool>, out: &mut Vec<String>) {
    for a in list.and_then(|l| l.as_array()).into_iter().flatten() {
        if let Some(s) = a.as_str() {
            out.push(s.to_string());
            continue;
        }
        if !minecraft::rules_allow(a.get("rules"), features) {
            continue;
        }
        match &a["value"] {
            Value::String(s) => out.push(s.clone()),
            Value::Array(v) => out.extend(v.iter().filter_map(|x| x.as_str().map(String::from))),
            _ => {}
        }
    }
}

/// Mojang's launcher copies the parent jar next to a child version; Forge's
/// `ignoreList` relies on the jar being named after the launched version.
fn launch_jar(version_id: &str) -> Result<PathBuf> {
    let jar_id = minecraft::jar_id(version_id);
    let src = minecraft::client_jar_path(&jar_id);
    if jar_id == version_id {
        return Ok(src);
    }
    let dst = minecraft::client_jar_path(version_id);
    let stale = match (std::fs::metadata(&src), std::fs::metadata(&dst)) {
        (Ok(a), Ok(b)) => a.len() != b.len(),
        _ => true,
    };
    if stale {
        std::fs::create_dir_all(dst.parent().unwrap())?;
        std::fs::copy(&src, &dst)?;
    }
    Ok(dst)
}

/// JVM flags tuned for smooth frame times. `java_major` is the runtime we launch with,
/// or None for a user-supplied Java whose version we don't know (then only safe G1 flags).
fn perf_jvm_flags(ram: u32, java_major: Option<u32>, gc: &str) -> Vec<String> {
    let use_zgc = match (gc, java_major) {
        ("g1", _) | (_, None) => false,
        ("zgc", Some(m)) => m >= 21,
        (_, Some(m)) => m >= 21,
    };
    let mut f: Vec<String> = vec![format!("-Xmx{ram}M")];
    if use_zgc {
        // Generational ZGC: sub-millisecond pauses, so no GC stutter. Java 21/22 need the flag,
        // later versions make it the default and warn (or refuse) if it's passed.
        // Small start heap: half of -Xmx up front (3.8 GB at 7.5 GB) crashed the game with "out of
        // native memory" when other programs already used the RAM. ZGC grows it as needed.
        f.push(format!("-Xms{}M", ram.min(1024)));
        f.push("-XX:+UseZGC".into());
        // ZGC lets the heap grow to -Xmx even when the game needs a fraction of it. On a 16 GB PC
        // that squeezes the OS and the game starts paging, which feels like random hitches. The soft
        // max makes ZGC collect earlier to stay smaller, and it may still grow when it really must.
        f.push(format!("-XX:SoftMaxHeapSize={}M", (ram * 6 / 10).clamp(2048.min(ram), ram)));
        if matches!(java_major, Some(21 | 22)) {
            f.push("-XX:+ZGenerational".into());
        }
    } else {
        f.push(format!("-Xms{}M", ram.min(2048)));
        f.extend(
            [
                "-XX:+UnlockExperimentalVMOptions",
                "-XX:+UseG1GC",
                "-XX:G1NewSizePercent=20",
                "-XX:G1ReservePercent=20",
                "-XX:MaxGCPauseMillis=50",
                "-XX:G1HeapRegionSize=32M",
                "-XX:+ParallelRefProcEnabled",
            ]
            .map(String::from),
        );
    }
    f.extend(
        [
            "-XX:+DisableExplicitGC",
            // Big modpacks fill the default 240 MB code cache; when it is full the JIT switches off
            // and the game drops to a fraction of its speed until restart.
            "-XX:ReservedCodeCacheSize=512m",
            // hsperfdata is written to disk every tick otherwise; shows up as micro-stutter on slow drives.
            "-XX:+PerfDisableSharedMem",
            "-Dfml.ignoreInvalidMinecraftCertificates=true",
            "-Dfml.ignorePatchDiscrepancies=true",
        ]
        .map(String::from),
    );
    // macOS: LWJGL/GLFW must open its window on the first thread, or the game crashes on launch.
    #[cfg(target_os = "macos")]
    f.push("-XstartOnFirstThread".into());
    f
}

/// Sensible first-run video settings, written only when the instance has no options.txt yet.
fn write_default_options(game_dir: &Path) -> Result<()> {
    let p = game_dir.join("options.txt");
    if p.exists() {
        return Ok(());
    }
    let threads = settings::cpu_threads();
    let (render, sim) = match threads {
        0..=4 => (8, 5),
        5..=8 => (10, 6),
        9..=12 => (12, 8),
        _ => (16, 10),
    };
    let opts = format!(
        "renderDistance:{render}\nsimulationDistance:{sim}\nbiomeBlendRadius:1\nentityShadows:false\nparticles:1\nenableVsync:false\nmaxFps:260\nentityDistanceScaling:0.75\n"
    );
    std::fs::write(p, opts)?;
    Ok(())
}

/// Sodium's own settings for an instance that has it and hasn't saved any yet. Measured on a 4-core
/// CPU while walking through new terrain: its default chunk builders (one per spare core) took the
/// CPU from the render thread and the average FPS fell a quarter below vanilla; two builders and no
/// advanced staging buffers (which sync with the GPU through fences) won most of that back, with the
/// 1% lows level with vanilla and still far fewer hitches. Big CPUs keep Sodium's own choice.
fn write_sodium_defaults(game_dir: &Path) -> Result<()> {
    let p = game_dir.join("config").join("sodium-options.json");
    if p.exists() {
        return Ok(());
    }
    let has_sodium = std::fs::read_dir(game_dir.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("sodium-fabric"));
    if !has_sodium {
        return Ok(());
    }
    let builders = match settings::cpu_threads() {
        0..=4 => 1,
        5..=8 => 2,
        9..=12 => 3,
        _ => 0, // 0 = Sodium decides
    };
    let opts = serde_json::json!({
        "performance": { "chunk_builder_threads": builders },
        "advanced": { "use_advanced_staging_buffers": false },
    });
    std::fs::create_dir_all(p.parent().unwrap())?;
    std::fs::write(p, serde_json::to_vec_pretty(&opts)?)?;
    Ok(())
}

fn split_user_args(s: &str) -> Vec<String> {
    s.split_whitespace().map(String::from).collect()
}

pub async fn launch(instance_id: &str, server: Option<String>) -> Result<()> {
    if RUNNING.lock().unwrap().contains_key(instance_id) {
        bail!(l!("Tahle instance už běží", "This instance is already running"));
    }
    let task = instance_id.to_string();
    linfo!("launch {instance_id} (server: {server:?})");
    let result = launch_inner(instance_id, server).await;
    if let Err(err) = &result {
        lerror!("launch {instance_id} failed: {err:#}");
    }
    events::progress_done(&task);
    result
}

async fn launch_inner(instance_id: &str, server: Option<String>) -> Result<()> {
    let task = instance_id.to_string();
    let s = settings::get();
    if let Some(free) = settings::free_space(&settings::data_dir()) {
        const GB: u64 = 1 << 30;
        if free < GB {
            bail!(l!(
                "Na disku s daty launcheru zbývá jen {} MB. Uvolni místo, nebo data přesuň v Nastavení → Přesunout data.",
                "Only {} MB left on the launcher's data drive. Free up space, or move your data in Settings → Move data.",
                free >> 20
            ));
        }
        if free < 5 * GB {
            events::emit("low-disk", free);
        }
    }
    let account = auth::ready_account(None).await?;
    let mut inst = instances::load(instance_id)?;
    let game_dir = instances::game_dir(instance_id);
    std::fs::create_dir_all(&game_dir)?;

    let (version_id, version) = prepare(&mut inst).await?;

    if matches!(inst.loader.as_str(), "fabric" | "quilt") {
        events::progress(&task, &l!("Kontroluji kompatibilitu modů", "Checking mod compatibility"), 0, 1);
        if let Ok(r) = compat::fix(instance_id).await {
            if !r.fixed.is_empty() {
                events::emit("compat-fixed", serde_json::json!({"instance": instance_id, "fixed": r.fixed}));
            }
        }
    }
    let libs = minecraft::install_files(&version_id, &version, &task).await?;
    let assets_dir = minecraft::install_assets(&version, &task, &game_dir).await?;

    events::progress(&task, &l!("Připravuji spuštění", "Preparing to launch"), 0, 1);
    let natives_dir = settings::meta_dir().join("natives").join(&version_id);
    minecraft::extract_natives(&libs.natives, &natives_dir)?;

    let (javaw, java_major) = if !inst.java_path.is_empty() && Path::new(&inst.java_path).exists() {
        (PathBuf::from(&inst.java_path), None)
    } else {
        let major = java::normalize(minecraft::java_major(&version));
        (java::ensure(major, &task).await?, Some(major))
    };

    let mut cp: Vec<String> = libs.classpath.iter().map(|p| p.to_string_lossy().into_owned()).collect();
    cp.push(launch_jar(&version_id)?.to_string_lossy().into_owned());

    write_default_options(&game_dir)?;
    write_sodium_defaults(&game_dir)?;
    recording::apply_before_launch(&inst, &game_dir)?;
    if matches!(inst.loader.as_str(), "fabric" | "quilt") {
        compat::apply_light_mode(instance_id, inst.light_mode)?;
    }
    justice_mod::sync(&inst, &game_dir).await?;

    let width = inst.width.unwrap_or(s.width);
    let height = inst.height.unwrap_or(s.height);
    let mut vars = build_vars(&account, &version, &version_id, &game_dir, &assets_dir, &natives_dir, &cp, width, height, server.as_deref());

    let modern_quickplay = version["arguments"]["game"]
        .as_array()
        .map(|a| a.iter().any(|x| x.to_string().contains("quickPlayMultiplayer")))
        .unwrap_or(false);
    // `server` is either "host[:port]" or "world:<save folder>" for singleplayer quick play.
    let world = server.as_deref().and_then(|s| s.strip_prefix("world:")).map(String::from);
    let server = if world.is_some() { None } else { server };
    let mut features: HashMap<&str, bool> = HashMap::new();
    features.insert("has_custom_resolution", true);
    features.insert("is_quick_play_multiplayer", server.is_some() && modern_quickplay);
    features.insert("is_quick_play_singleplayer", world.is_some() && modern_quickplay);

    if let Some(w) = &world {
        vars.insert("quickPlaySingleplayer", w.clone());
        vars.insert("quickPlayMultiplayer", String::new());
    }

    let mut jvm: Vec<String> = perf_jvm_flags(inst.ram_mb.unwrap_or(s.ram_mb), java_major, &s.gc);
    if let Some(arg) = version["logging"]["client"]["argument"].as_str() {
        let id = version["logging"]["client"]["file"]["id"].as_str().unwrap_or("client.xml");
        let cfg = settings::meta_dir().join("assets").join("log_configs").join(id);
        jvm.push(arg.replace("${path}", &cfg.to_string_lossy()));
    }
    if version["arguments"]["jvm"].is_array() {
        collect_args(version.get("arguments").and_then(|a| a.get("jvm")), &features, &mut jvm);
    } else {
        jvm.push("-Djava.library.path=${natives_directory}".into());
        jvm.push("-cp".into());
        jvm.push("${classpath}".into());
    }
    jvm.extend(split_user_args(&s.jvm_args));
    jvm.extend(split_user_args(&inst.jvm_args));

    let mut game: Vec<String> = vec![];
    if let Some(legacy) = version["minecraftArguments"].as_str() {
        game.extend(legacy.split_whitespace().map(String::from));
        game.extend(["--width".into(), "${resolution_width}".into(), "--height".into(), "${resolution_height}".into()]);
    } else {
        collect_args(version.get("arguments").and_then(|a| a.get("game")), &features, &mut game);
    }
    if let (Some(srv), false) = (&server, modern_quickplay) {
        let (host, port) = srv.split_once(':').unwrap_or((srv.as_str(), "25565"));
        game.extend(["--server".into(), host.into(), "--port".into(), port.into()]);
    }
    // Some versions lack the singleplayer rule in their JSON even though the game supports it.
    if let Some(w) = &world {
        if !game.iter().any(|a| a == "--quickPlaySingleplayer") {
            game.extend(["--quickPlaySingleplayer".into(), w.clone()]);
        }
    }

    let main_class = version["mainClass"].as_str().unwrap_or("net.minecraft.client.main.Main");
    let mut args: Vec<String> = jvm.iter().map(|a| substitute(a, &vars)).collect();
    args.push(main_class.to_string());
    args.extend(game.iter().map(|a| substitute(a, &vars)));

    linfo!("java: {} | main class: {main_class} | game dir: {}", javaw.display(), game_dir.display());
    {
        // Never print the session token.
        let mut shown = args.clone();
        for i in 1..shown.len() {
            if shown[i - 1] == "--accessToken" {
                shown[i] = "***".into();
            }
        }
        ldebug!("command: {}", shown.join(" "));
    }
    let mut cmd = tokio::process::Command::new(&javaw);
    cmd.args(&args)
        .current_dir(&game_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());
    platform::hide_window(&mut cmd);
    let mut child = cmd.spawn()?;
    let pid = child.id().unwrap_or(0);
    linfo!("game started, pid {pid}");

    if s.high_priority {
        platform::raise_priority(&child);
    }

    RUNNING.lock().unwrap().insert(instance_id.to_string(), pid);
    LOGS.lock().unwrap().insert(instance_id.to_string(), VecDeque::new());
    events::emit("game-start", instance_id.to_string());
    // Pop the live-log window (Lunar-style), unless the user turned it off.
    if settings::get().log_window_on_launch {
        if let Some(app) = events::app() {
            crate::open_logs(app);
        }
    }
    events::progress_done(&task);

    inst.last_played = chrono::Utc::now().timestamp();
    instances::save(&inst)?;

    if let Some(app) = events::app() {
        if let Some(w) = app.get_webview_window("main") {
            match s.on_launch.as_str() {
                "hide" => {
                    let _ = w.hide();
                }
                // Clips record from the launcher process, so with Clips on it hides to the tray instead.
                "close" if crate::clips::enabled() || crate::social::signed_in().is_some() => {
                    let _ = w.hide();
                }
                "close" => app.exit(0),
                _ => {}
            }
        }
    }
    integrations::game_started(&inst, server.as_deref()).await;
    crate::clips::game_started(&inst.name);
    crate::social::set_presence(serde_json::json!({
        "state": "playing",
        "instance": inst.name,
        "mc": inst.mc_version,
        "loader": inst.loader,
        // "world:<name>" means singleplayer; never share world names.
        "server": server.as_deref().filter(|s| !s.starts_with("world:")),
    }));

    if let Some(out) = child.stdout.take() {
        tokio::spawn(pump(BufReader::new(out).lines(), instance_id.to_string()));
    }
    if let Some(err) = child.stderr.take() {
        tokio::spawn(pump(BufReader::new(err).lines(), instance_id.to_string()));
    }

    let started = std::time::Instant::now();
    let id = instance_id.to_string();
    tokio::spawn(async move {
        let status = child.wait().await.ok();
        let seconds = started.elapsed().as_secs();
        RUNNING.lock().unwrap().remove(&id);
        if let Ok(mut i) = instances::load(&id) {
            i.playtime_secs += seconds;
            let _ = instances::save(&i);
            integrations::game_stopped(&i).await;
        }
        crate::clips::game_stopped();
        if running_ids().is_empty() {
            crate::social::set_presence(serde_json::json!({ "state": "online" }));
        }
        let code = status.and_then(|s| s.code());
        let killed = KILLED.lock().unwrap().remove(&id);
        linfo!("game {id} exited: code {code:?}, {seconds}s, killed by launcher: {killed}");
        let crash = if code.unwrap_or(0) != 0 && !killed {
            Some(crash::analyze(&logs(&id), &instances::game_dir(&id)))
        } else {
            None
        };
        if let Some(w) = events::app().and_then(|a| a.get_webview_window("main")) {
            let _ = w.show();
            let _ = w.set_focus();
        }
        events::emit("game-exit", GameExit { instance: id, code, seconds, crash });
    });
    Ok(())
}

fn push_log(id: &str, line: String) {
    {
        let mut logs = LOGS.lock().unwrap();
        let buf = logs.entry(id.to_string()).or_default();
        if buf.len() >= MAX_LOG_LINES {
            buf.pop_front();
        }
        buf.push_back(line.clone());
    }
    events::emit("game-log", LogLine { instance: id.to_string(), line });
}

fn xml_attr(tag: &str, name: &str) -> String {
    let key = format!("{name}=\"");
    tag.find(&key)
        .and_then(|i| tag[i + key.len()..].split('"').next())
        .unwrap_or_default()
        .to_string()
}

/// Mojang's log config prints log4j XML events; turn them back into readable lines.
async fn pump<R: tokio::io::AsyncBufRead + Unpin>(mut lines: tokio::io::Lines<R>, id: String) {
    let mut header: Option<String> = None;
    let mut body: Vec<String> = vec![];
    while let Ok(Some(line)) = lines.next_line().await {
        let t = line.trim_start();
        if t.starts_with("<log4j:Event") {
            let ts = xml_attr(t, "timestamp").parse::<i64>().ok();
            let time = ts
                .and_then(chrono::DateTime::from_timestamp_millis)
                .map(|d| d.with_timezone(&chrono::Local).format("%H:%M:%S").to_string())
                .unwrap_or_default();
            header = Some(format!("[{time}] [{}/{}]", xml_attr(t, "thread"), xml_attr(t, "level")));
            body.clear();
            continue;
        }
        if let Some(h) = &header {
            if t.starts_with("</log4j:Event>") {
                let mut it = body.drain(..);
                push_log(&id, format!("{h} {}", it.next().unwrap_or_default()));
                for rest in it {
                    push_log(&id, rest);
                }
                header = None;
                continue;
            }
            let cleaned = line
                .replace("<log4j:Message><![CDATA[", "")
                .replace("]]></log4j:Message>", "")
                .replace("<log4j:Throwable><![CDATA[", "")
                .replace("]]></log4j:Throwable>", "");
            let cleaned = if body.is_empty() { cleaned.trim().to_string() } else { cleaned };
            if !cleaned.trim().is_empty() {
                body.push(cleaned);
            }
            continue;
        }
        push_log(&id, line);
    }
}


#[allow(clippy::too_many_arguments)]
fn build_vars<'a>(
    acc: &Account,
    version: &Value,
    version_id: &str,
    game_dir: &Path,
    assets_dir: &Path,
    natives_dir: &Path,
    cp: &[String],
    width: u32,
    height: u32,
    server: Option<&str>,
) -> HashMap<&'a str, String> {
    let mut v = HashMap::new();
    let token = if acc.access_token.is_empty() { "0".to_string() } else { acc.access_token.clone() };
    v.insert("auth_player_name", acc.username.clone());
    v.insert("version_name", version_id.to_string());
    v.insert("game_directory", game_dir.to_string_lossy().into_owned());
    v.insert("assets_root", settings::meta_dir().join("assets").to_string_lossy().into_owned());
    v.insert("game_assets", assets_dir.to_string_lossy().into_owned());
    v.insert("assets_index_name", version["assetIndex"]["id"].as_str().unwrap_or_default().to_string());
    v.insert("auth_uuid", acc.uuid.clone());
    v.insert("auth_access_token", token.clone());
    v.insert("auth_session", format!("token:{token}:{}", acc.uuid));
    v.insert("auth_xuid", if acc.xuid.is_empty() { "0".into() } else { acc.xuid.clone() });
    v.insert("clientid", settings::get().ms_client_id);
    v.insert("user_type", if acc.kind == "msa" { "msa".into() } else { "legacy".into() });
    v.insert("user_properties", "{}".into());
    v.insert("version_type", "Justice Client".into());
    v.insert("natives_directory", natives_dir.to_string_lossy().into_owned());
    v.insert("launcher_name", "justice-client".into());
    v.insert("launcher_version", env!("CARGO_PKG_VERSION").into());
    v.insert("classpath", cp.join(platform::CLASSPATH_SEP));
    v.insert("classpath_separator", platform::CLASSPATH_SEP.into());
    v.insert("library_directory", settings::meta_dir().join("libraries").to_string_lossy().into_owned());
    v.insert("resolution_width", width.to_string());
    v.insert("resolution_height", height.to_string());
    v.insert("quickPlayMultiplayer", server.unwrap_or_default().to_string());
    v.insert("quickPlayPath", "quickplay.json".into());
    v
}
