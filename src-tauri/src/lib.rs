#[macro_use]
mod logger;
#[macro_use]
mod i18n;
mod auth;
mod backups;
mod clips;
mod compat;
mod content;
mod crash;
mod downloads;
mod events;
mod instances;
mod integrations;
mod java;
mod justice_mod;
mod launch;
mod loaders;
mod minecraft;
mod modbot;
mod net;
mod packs;
mod platform;
mod pvp;
mod recording;
mod servers;
mod settings;
mod skinlib;
mod social;
mod world;
mod bridge;
mod media;
mod art;
mod optimizer;
mod sonolith;

use base64::Engine;
use content::Kind;
use instances::Instance;
use serde::Serialize;

type Res<T> = Result<T, String>;

fn e<T>(r: anyhow::Result<T>) -> Res<T> {
    r.map_err(|e| {
        let msg = format!("{e:#}");
        lerror!("{msg}");
        msg
    })
}

// ---------------- Logging & self-update ----------------

/// Frontend errors and notes end up in the same terminal/log file as the backend's.
#[tauri::command]
fn log_frontend(level: String, message: String) {
    logger::write(&level.to_uppercase(), "ui", &message);
}

#[tauri::command]
fn log_path() -> String {
    logger::path().to_string_lossy().into_owned()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ManualUpdate {
    version: String,
    notes: String,
    url: String,
    /// How this build was installed: "appimage", "deb", "arch" or "other" — the dialog shows the matching steps.
    kind: &'static str,
}

fn linux_package_kind() -> &'static str {
    if std::env::var_os("APPIMAGE").is_some() {
        "appimage"
    } else if std::path::Path::new("/var/lib/dpkg/info/justice-client.list").exists() {
        "deb"
    } else if std::fs::read_dir("/var/lib/pacman/local")
        .map(|d| d.flatten().any(|e| e.file_name().to_string_lossy().starts_with("justice-client-")))
        .unwrap_or(false)
    {
        "arch"
    } else {
        "other"
    }
}

fn version_parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split(['.', '-', '+']).map_while(|p| p.parse().ok()).collect()
}

/// For builds the in-app updater can't replace (Linux packages and the AppImage): read the same
/// feed and just tell the UI a newer version exists and where to get it.
#[tauri::command]
async fn check_update_manual() -> Res<Option<ManualUpdate>> {
    let feed = "https://justiceclient.online/latest.json";
    ldebug!("update check (manual) -> {feed}");
    let v = e(net::get_json(feed).await)?;
    let latest = v["version"].as_str().unwrap_or_default().to_string();
    let current = env!("CARGO_PKG_VERSION");
    linfo!("update check: current {current}, latest {latest}");
    if version_parts(&latest) > version_parts(current) {
        Ok(Some(ManualUpdate { version: latest, notes: v["notes"].as_str().unwrap_or_default().into(), url: "https://justiceclient.online/".into(), kind: linux_package_kind() }))
    } else {
        Ok(None)
    }
}

// ---------------- Settings ----------------

#[tauri::command]
fn instance_last_place(id: String) -> Option<instances::LastPlace> {
    instances::last_place(&id)
}

// Polled every second by the player, so failures aren't logged as errors.
#[tauri::command]
async fn media_state(prefer: Option<String>, known_thumb: Option<String>) -> Res<Option<media::MediaState>> {
    tauri::async_runtime::spawn_blocking(move || media::state(prefer.as_deref(), known_thumb.as_deref())).await.map_err(|e| e.to_string())?.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
async fn media_command(command: String, value: Option<f64>, prefer: Option<String>) -> Res<()> {
    e(tauri::async_runtime::spawn_blocking(move || media::command(&command, value, prefer.as_deref())).await.map_err(|e| anyhow::anyhow!(e.to_string())).and_then(|r| r))
}

#[tauri::command]
async fn version_art() -> Res<std::collections::HashMap<String, art::Art>> {
    art::all().await.map_err(|e| format!("{e:#}"))
}

#[tauri::command]
fn open_music_app() -> Res<()> {
    e(media::open_music_app())
}

#[tauri::command]
fn open_logs_window(app: tauri::AppHandle) {
    open_logs(&app);
}

/// Opens, or focuses, the standalone live-log window (its own OS window, like Lunar Client's).
/// Window creation must run on the main thread, so dispatch there from wherever this is called.
pub fn open_logs(app: &tauri::AppHandle) {
    let app = app.clone();
    let _ = app.clone().run_on_main_thread(move || {
        use tauri::Manager;
        if let Some(w) = app.get_webview_window("logs") {
            let _ = w.show();
            let _ = w.unminimize();
            let _ = w.set_focus();
            return;
        }
        let r = tauri::WebviewWindowBuilder::new(&app, "logs", tauri::WebviewUrl::App("index.html?window=logs".into()))
            .title("Justice — Logs")
            .inner_size(920.0, 580.0)
            .min_inner_size(440.0, 320.0)
            .decorations(false)
            .build();
        if let Err(err) = r {
            eprintln!("logs window failed to open: {err}");
        }
    });
}

#[tauri::command]
fn get_settings() -> settings::Settings {
    settings::get()
}

#[tauri::command]
fn save_settings(settings: settings::Settings) -> Res<()> {
    e(settings::save(settings))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Hardware {
    cpu: String,
    threads: usize,
    gpus: Vec<String>,
    total_ram_mb: u64,
}

#[tauri::command]
async fn hardware_info() -> Hardware {
    let (cpu, gpus) = tokio::task::spawn_blocking(platform::hardware_names).await.unwrap_or_default();
    Hardware { cpu, threads: settings::cpu_threads(), gpus, total_ram_mb: settings::total_ram_mb() }
}

#[tauri::command]
fn free_space(path: String) -> Option<u64> {
    settings::free_space(std::path::Path::new(&path))
}

/// Moves all launcher data (instances, Java, libraries…) to a new folder, then switches to it.
#[tauri::command]
async fn move_data_dir(dest: String) -> Res<()> {
    if !launch::running_ids().is_empty() {
        return Err(l!("Nejdřív zavři hru.", "Close the game first."));
    }
    let old = settings::data_dir();
    let new = std::path::PathBuf::from(&dest);
    if new.starts_with(&old) {
        return Err(l!("Nová složka nemůže být uvnitř té staré.", "The new folder can't be inside the old one."));
    }
    let old2 = old.clone();
    let new2 = new.clone();
    e(tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        if old2.exists() {
            events::progress("move-data", &l!("Přesouvám data", "Moving data"), 0, 1);
            for entry in std::fs::read_dir(&old2)?.flatten() {
                // settings.json may live in the same folder as the data; it stays where it is.
                if entry.file_name() == "settings.json" {
                    continue;
                }
                let target = new2.join(entry.file_name());
                if entry.path().is_dir() {
                    instances::copy_dir(&entry.path(), &target)?;
                } else {
                    std::fs::create_dir_all(&new2)?;
                    std::fs::copy(entry.path(), &target)?;
                }
            }
        }
        Ok(())
    })
    .await
    .map_err(anyhow::Error::from)
    .and_then(|r| r))?;
    let mut s = settings::get();
    s.data_dir = dest;
    e(settings::save(s))?;
    // Only delete the old copy after the new location is saved as active.
    for entry in std::fs::read_dir(&old).into_iter().flatten().flatten() {
        if entry.file_name() == "settings.json" {
            continue;
        }
        let _ = if entry.path().is_dir() { std::fs::remove_dir_all(entry.path()) } else { std::fs::remove_file(entry.path()) };
    }
    events::progress_done("move-data");
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SystemInfo {
    total_ram_mb: u64,
    recommended_ram_mb: u32,
    version: String,
    has_cf_key: bool,
}

#[tauri::command]
fn system_info() -> SystemInfo {
    SystemInfo {
        total_ram_mb: settings::total_ram_mb(),
        recommended_ram_mb: settings::recommended_ram_mb(),
        version: env!("CARGO_PKG_VERSION").into(),
        has_cf_key: content::has_cf_key(),
    }
}

// ---------------- Versions ----------------

#[tauri::command]
async fn game_versions() -> Res<Vec<minecraft::GameVersion>> {
    e(minecraft::game_versions().await)
}

#[tauri::command]
async fn loader_versions(loader: String, game: String) -> Res<Vec<loaders::LoaderVersion>> {
    e(loaders::list(&loader, &game).await)
}

// ---------------- Instances ----------------

#[tauri::command]
fn list_instances() -> Vec<Instance> {
    instances::list()
}

#[tauri::command]
fn get_instance(id: String) -> Res<Instance> {
    e(instances::load(&id))
}

#[tauri::command]
async fn create_instance(instance: Instance, packs: Vec<String>, optimize: bool) -> Res<Instance> {
    let mut inst = e(instances::create(instance))?;
    let slugs = content::template_slugs(&packs, &inst.loader, optimize);
    if !slugs.is_empty() {
        inst.template = packs.first().cloned().unwrap_or_else(|| "fps".into());
        inst.recording_mode = packs.iter().any(|p| p == "cinematic");
        e(instances::save(&inst))?;
        content::install_many(&inst.id, &slugs, Kind::Mod).await;
        if packs.iter().any(|p| p == "cinematic" || p == "smp") && matches!(inst.loader.as_str(), "fabric" | "quilt") {
            let _ = content::install(&inst.id, "modrinth", "complementary-reimagined", Kind::Shader, None).await;
        }
        let _ = compat::fix(&inst.id).await;
        events::progress_done(&inst.id);
    }
    Ok(inst)
}

// ---------------- Friends & chat ----------------

#[tauri::command]
async fn social_login() -> Res<serde_json::Value> {
    e(social::login().await)
}
#[tauri::command]
fn social_logout() {
    social::sign_out();
}
/// Offline accounts: sign in to (or create, `register`) the Justice account with a password.
#[tauri::command]
async fn social_login_password(password: String, register: bool) -> Res<serde_json::Value> {
    e(social::login_password(&password, register).await)
}
#[tauri::command]
async fn party_warp_here() -> Res<serde_json::Value> {
    e(social::warp_here().await)
}
#[tauri::command]
fn pvp_stats() -> serde_json::Value {
    pvp::get()
}
#[tauri::command]
fn social_status() -> serde_json::Value {
    serde_json::json!({ "signedIn": social::signed_in().is_some(), "uuid": social::signed_in() })
}
#[tauri::command]
async fn social_get(path: String) -> Res<serde_json::Value> {
    e(social::get(&path).await)
}
#[tauri::command]
async fn social_post(path: String, body: serde_json::Value) -> Res<serde_json::Value> {
    e(social::post(&path, body).await)
}
#[tauri::command]
async fn social_put(path: String, body: serde_json::Value) -> Res<serde_json::Value> {
    let r = e(social::put(&path, body).await)?;
    if path == "/me" {
        social::capes_changed();
    }
    Ok(r)
}
#[tauri::command]
async fn social_upload(path: String) -> Res<serde_json::Value> {
    e(social::upload(&path).await)
}
#[tauri::command]
async fn social_open_clip(url: String) -> Res<()> {
    e(social::open_shared_clip(&url).await)
}
#[tauri::command]
fn social_typing(to: String) {
    social::typing(&to);
}
/// Guest side of a shared world: local port for Minecraft to connect to.
#[tauri::command]
async fn world_join(world: String) -> Res<u16> {
    e(world::join(&world).await)
}

/// Custom instance icon: copies the picture next to the instance and points `icon` at it.
#[tauri::command]
fn set_instance_icon(id: String, path: String) -> Res<Instance> {
    let src = std::path::PathBuf::from(&path);
    let ext = src.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if !["png", "jpg", "jpeg", "webp", "gif"].contains(&ext.as_str()) {
        return Err(l!("Ikona musí být obrázek (png, jpg, webp, gif)", "The icon must be a picture (png, jpg, webp, gif)"));
    }
    let size = std::fs::metadata(&src).map(|m| m.len()).unwrap_or(0);
    if size > 8 * 1024 * 1024 {
        return Err(l!("Obrázek je moc velký (max 8 MB)", "The picture is too big (max 8 MB)"));
    }
    let mut inst = e(instances::load(&id))?;
    let dir = instances::dir(&id);
    // A new name each time, so the UI doesn't show a cached old picture.
    for old in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        if old.file_name().to_string_lossy().starts_with("icon-") {
            let _ = std::fs::remove_file(old.path());
        }
    }
    let dst = dir.join(format!("icon-{}.{ext}", chrono::Utc::now().timestamp_millis()));
    std::fs::copy(&src, &dst).map_err(|e| e.to_string())?;
    inst.icon = format!("img:{}", dst.display());
    e(instances::save(&inst))?;
    Ok(inst)
}

#[tauri::command]
fn update_instance(instance: Instance) -> Res<Instance> {
    let mut cur = e(instances::load(&instance.id))?;
    let version_changed = cur.mc_version != instance.mc_version
        || cur.loader != instance.loader
        || cur.loader_version != instance.loader_version;
    cur.name = instance.name;
    cur.icon = instance.icon;
    cur.group = instance.group;
    cur.mc_version = instance.mc_version;
    cur.loader = instance.loader;
    cur.loader_version = instance.loader_version;
    cur.ram_mb = instance.ram_mb;
    cur.java_path = instance.java_path;
    cur.jvm_args = instance.jvm_args;
    cur.width = instance.width;
    cur.height = instance.height;
    cur.recording_mode = instance.recording_mode;
    cur.light_mode = instance.light_mode;
    cur.no_justice_mod = instance.no_justice_mod;
    if version_changed {
        cur.installed_id.clear();
    }
    e(instances::save(&cur))?;
    Ok(cur)
}

#[tauri::command]
fn delete_instance(id: String) -> Res<()> {
    e(instances::delete(&id))
}

#[tauri::command]
fn duplicate_instance(id: String) -> Res<Instance> {
    e(instances::duplicate(&id))
}

#[tauri::command]
fn instance_path(id: String, sub: Option<String>) -> String {
    let mut p = instances::game_dir(&id);
    if let Some(s) = sub.filter(|s| !s.is_empty()) {
        p = p.join(s);
    }
    let _ = std::fs::create_dir_all(&p);
    p.to_string_lossy().into_owned()
}

// ---------------- Launch ----------------

#[tauri::command]
async fn launch_instance(id: String, server: Option<String>) -> Res<()> {
    e(launch::launch(&id, server).await)
}

#[tauri::command]
fn kill_game(id: String) -> Res<()> {
    e(launch::kill(&id))
}

#[tauri::command]
fn running_games() -> Vec<String> {
    launch::running_ids()
}

#[tauri::command]
fn game_logs(id: String) -> Vec<String> {
    launch::logs(&id)
}

#[tauri::command]
fn analyze_crash(id: String) -> crash::CrashReport {
    crash::analyze(&launch::logs(&id), &instances::game_dir(&id))
}

// ---------------- Accounts ----------------

#[tauri::command]
fn list_accounts() -> Vec<auth::PublicAccount> {
    auth::public_list()
}

#[tauri::command]
fn add_offline_account(name: String) -> Res<()> {
    e(with_account_change(|| auth::add_offline(&name)))
}

/// Runs an account change and re-signs friends in when the active account is now a different one.
fn with_account_change<T>(f: impl FnOnce() -> anyhow::Result<T>) -> anyhow::Result<T> {
    let before = auth::load_store().active;
    let r = f()?;
    if auth::load_store().active != before {
        social::account_changed();
    }
    Ok(r)
}

#[tauri::command]
async fn ms_login_start() -> Res<auth::DeviceCode> {
    e(auth::start_device_code().await)
}

#[tauri::command]
async fn ms_login_poll(device_code: String, interval: u64) -> Res<auth::PublicAccount> {
    let r = e(auth::poll_device_code(&device_code, interval).await)?;
    social::account_changed();
    Ok(r)
}

#[tauri::command]
fn ms_login_cancel() {
    auth::cancel_login()
}

#[tauri::command]
fn set_active_account(id: String) -> Res<()> {
    e(with_account_change(|| auth::set_active(&id)))
}

#[tauri::command]
fn remove_account(id: String) -> Res<()> {
    e(with_account_change(|| auth::remove(&id)))
}

#[tauri::command]
async fn get_capes(id: String) -> Res<Vec<auth::Cape>> {
    e(auth::capes(&id).await)
}

#[tauri::command]
async fn set_cape(id: String, cape: Option<String>) -> Res<()> {
    e(auth::set_cape(&id, cape.as_deref()).await)
}

#[tauri::command]
async fn upload_skin(id: String, path: String, variant: String) -> Res<()> {
    e(auth::upload_skin(&id, &path, &variant).await)
}

/// Copies a premium player's skin (handy for offline accounts).
#[tauri::command]
async fn skin_from_player(id: String, name: String) -> Res<()> {
    e(auth::skin_from_player(&id, &name).await)
}

#[tauri::command]
async fn reset_skin(id: String) -> Res<()> {
    e(auth::reset_skin(&id).await)
}

// ---------------- Skin library ----------------

#[tauri::command]
fn skinlib_list() -> Vec<skinlib::SkinView> {
    skinlib::list()
}

#[tauri::command]
fn skinlib_add_file(path: String, name: String, variant: String) -> Res<String> {
    e(skinlib::add_from_file(&path, &name, &variant))
}

#[tauri::command]
async fn skinlib_add_player(player: String, name: String) -> Res<String> {
    e(skinlib::add_from_player(&player, &name).await)
}

#[tauri::command]
async fn skinlib_add_current(account: String, name: String) -> Res<String> {
    e(skinlib::add_current(&account, &name).await)
}

#[tauri::command]
async fn skinlib_apply(account: String, id: String) -> Res<()> {
    e(skinlib::apply(&account, &id).await)
}

#[tauri::command]
fn skinlib_rename(id: String, name: String) -> Res<()> {
    e(skinlib::rename(&id, &name))
}

#[tauri::command]
fn skinlib_remove(id: String) -> Res<()> {
    e(skinlib::remove(&id))
}

/// Remote image as a data URL, so the webview can draw it on a canvas without CORS issues.
/// Offline Justice skins come from the local copy (the upload may not have happened yet).
#[tauri::command]
async fn fetch_image(url: String) -> Res<String> {
    let bytes = match auth::skin_file(&url) {
        Some(p) => e(std::fs::read(p).map_err(Into::into))?,
        None => {
            let r = e(net::CLIENT.get(&url).send().await.map_err(Into::into))?;
            e(r.bytes().await.map_err(Into::into))?.to_vec()
        }
    };
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(bytes)))
}

// ---------------- Content ----------------

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn search_content(
    source: String,
    query: String,
    kind: String,
    game_version: String,
    loader: String,
    sort: String,
    offset: u32,
    category: Option<String>,
) -> Res<content::SearchResult> {
    let category = category.unwrap_or_default();
    e(content::search(content::SearchQuery {
        source: &source,
        query: &query,
        kind: Kind::parse(&kind),
        game_version: &game_version,
        loader: &loader,
        sort: &sort,
        offset,
        category: &category,
    })
    .await)
}

#[tauri::command]
async fn project_details(source: String, id: String, kind: String, instance: String) -> Res<content::ProjectDetails> {
    let inst = e(instances::load(&instance))?;
    e(content::details(&source, &id, Kind::parse(&kind), &inst).await)
}

#[tauri::command]
async fn install_content(
    instance: String,
    source: String,
    project: String,
    kind: String,
    version: Option<String>,
) -> Res<content::InstallReport> {
    e(content::install(&instance, &source, &project, Kind::parse(&kind), version.as_deref()).await)
}

#[tauri::command]
fn list_content(instance: String) -> Vec<content::Installed> {
    content::list_installed(&instance)
}

#[tauri::command]
fn toggle_content(instance: String, kind: String, file: String, enabled: bool) -> Res<()> {
    e(content::toggle(&instance, &kind, &file, enabled))
}

#[tauri::command]
fn remove_content(instance: String, kind: String, file: String) -> Res<()> {
    e(content::remove(&instance, &kind, &file))
}

#[tauri::command]
fn check_compat(instance: String) -> Vec<compat::Conflict> {
    compat::check(&instance)
}

#[tauri::command]
async fn fix_compat(instance: String) -> Res<compat::FixReport> {
    let r = compat::fix(&instance).await;
    events::progress_done(&instance);
    e(r)
}

/// Crash-dialog "fix automatically": reads the last game log + crash report and realigns mods.
#[tauri::command]
async fn repair_crash(instance: String) -> Res<compat::FixReport> {
    let mut log = launch::logs(&instance).join("\n");
    let latest = instances::game_dir(&instance).join("logs").join("latest.log");
    if let Ok(t) = std::fs::read_to_string(latest) {
        log.push('\n');
        log.push_str(&t);
    }
    let r = compat::repair_from_log(&instance, &log).await;
    events::progress_done(&instance);
    e(r)
}

#[tauri::command]
fn repair_plan(instance: String) -> Res<compat::RepairPlan> {
    e(compat::repair_plan(&instance))
}

#[tauri::command]
async fn repair_apply(instance: String, ids: Vec<String>) -> Res<compat::FixReport> {
    let r = compat::repair_apply(&instance, ids).await;
    events::progress_done(&instance);
    e(r)
}

#[tauri::command]
fn repair_undo(instance: String) -> Res<()> {
    e(compat::repair_undo(&instance))
}

#[tauri::command]
fn set_pinned(instance: String, kind: String, file: String, pinned: bool) -> Res<()> {
    e(content::set_pinned_file(&instance, &kind, &file, pinned))
}

#[tauri::command]
fn add_local_files(instance: String, paths: Vec<String>) -> Res<usize> {
    e(content::add_local_files(&instance, &paths))
}

#[tauri::command]
async fn check_updates(instance: String) -> Res<Vec<content::Update>> {
    e(content::check_updates(&instance).await)
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateReq {
    source: String,
    project_id: String,
    kind: String,
    new_version_id: String,
}

#[tauri::command]
async fn apply_updates(instance: String, updates: Vec<UpdateReq>) -> Res<Vec<String>> {
    let id = instance.clone();
    e(tokio::task::spawn_blocking(move || backups::snapshot(&id, "pred-aktualizaci")).await.map_err(anyhow::Error::from).and_then(|r| r))?;
    let mut failed = vec![];
    for u in updates {
        if let Err(err) = content::install(&instance, &u.source, &u.project_id, Kind::parse(&u.kind), Some(&u.new_version_id)).await {
            failed.push(format!("{}: {err:#}", u.project_id));
        }
    }
    if let Ok(r) = compat::fix(&instance).await {
        failed.extend(r.remaining.into_iter().map(|c| c.text));
    }
    events::progress_done(&instance);
    Ok(failed)
}

#[tauri::command]
fn sonolith_status() -> sonolith::Status {
    sonolith::status()
}

#[tauri::command]
async fn sonolith_connect() -> Res<()> {
    e(sonolith::register().await)
}

#[tauri::command]
async fn sonolith_disconnect() -> Res<()> {
    e(sonolith::disconnect().await)
}

#[tauri::command]
async fn sonolith_call(method: String, params: Option<serde_json::Value>) -> Res<serde_json::Value> {
    e(sonolith::call(&method, params.unwrap_or(serde_json::Value::Null)).await)
}

#[tauri::command]
async fn optimize_plan(instance: String) -> Res<optimizer::Plan> {
    e(optimizer::plan(&instance).await)
}

#[tauri::command]
async fn optimize_apply(instance: String, ids: Vec<String>) -> Res<optimizer::ApplyReport> {
    let r = e(optimizer::apply(&instance, ids).await);
    events::progress_done(&instance);
    r
}

#[tauri::command]
fn optimize_undo(instance: String) -> Res<()> {
    e(optimizer::undo(&instance))
}

#[tauri::command]
async fn optimize_instance(instance: String) -> Res<content::InstallReport> {
    let inst = e(instances::load(&instance))?;
    let slugs = content::performance_slugs(&inst.loader);
    if slugs.is_empty() {
        return Err(l!("Optimalizační mody potřebují Fabric, Quilt, Forge nebo NeoForge.", "Optimization mods need Fabric, Quilt, Forge or NeoForge."));
    }
    let r = content::install_many(&instance, &slugs, Kind::Mod).await;
    events::progress_done(&instance);
    Ok(r)
}

// ---------------- Recording / media ----------------

#[tauri::command]
fn save_mod_profile(instance: String, name: String) -> Res<()> {
    e(recording::save_profile(&instance, &name))
}

#[tauri::command]
fn apply_mod_profile(instance: String, name: String) -> Res<()> {
    e(recording::apply_profile(&instance, &name))
}

#[tauri::command]
fn delete_mod_profile(instance: String, name: String) -> Res<()> {
    e(recording::delete_profile(&instance, &name))
}

#[tauri::command]
async fn list_media(kind: String) -> Vec<recording::MediaFile> {
    tokio::task::spawn_blocking(move || recording::media(&kind)).await.unwrap_or_default()
}

#[tauri::command]
fn delete_media(path: String) -> Res<()> {
    let p = std::path::PathBuf::from(&path);
    if !p.starts_with(settings::instances_dir()) {
        return Err(l!("Soubor není v žádné instanci", "That file isn't inside any instance"));
    }
    std::fs::remove_file(p).map_err(|e| e.to_string())
}

#[tauri::command]
async fn obs_test() -> Res<Vec<String>> {
    e(integrations::obs_test().await)
}

// ---------------- Worlds & backups ----------------

#[tauri::command]
fn list_worlds(instance: String) -> Vec<backups::World> {
    backups::worlds(&instance)
}

#[tauri::command]
async fn backup_world(instance: String, world: String) -> Res<String> {
    e(tokio::task::spawn_blocking(move || backups::backup_world(&instance, &world)).await.map_err(anyhow::Error::from).and_then(|r| r))
}

#[tauri::command]
async fn snapshot_instance(instance: String) -> Res<String> {
    e(tokio::task::spawn_blocking(move || backups::snapshot(&instance, "rucni")).await.map_err(anyhow::Error::from).and_then(|r| r))
}

#[tauri::command]
fn list_backups(instance: String) -> Vec<backups::Backup> {
    backups::list(&instance)
}

#[tauri::command]
async fn restore_backup(instance: String, file: String) -> Res<()> {
    e(tokio::task::spawn_blocking(move || backups::restore(&instance, &file)).await.map_err(anyhow::Error::from).and_then(|r| r))
}

#[tauri::command]
fn delete_backup(instance: String, file: String) -> Res<()> {
    e(backups::delete(&instance, &file))
}

// ---------------- Sharing / import ----------------

#[tauri::command]
fn share_code(instance: String) -> Res<String> {
    e(packs::share_code(&instance))
}

#[tauri::command]
async fn import_code(code: String) -> Res<Instance> {
    e(packs::import_code(&code).await)
}

#[tauri::command]
async fn export_pack(instance: String, dest: String) -> Res<()> {
    e(tokio::task::spawn_blocking(move || packs::export_pack(&instance, &dest)).await.map_err(anyhow::Error::from).and_then(|r| r))
}

#[tauri::command]
async fn import_pack(path: String) -> Res<Instance> {
    e(packs::import_file(&path).await)
}

#[tauri::command]
async fn install_modpack(source: String, project: String, version: Option<String>) -> Res<Instance> {
    let r = packs::install_modpack(&source, &project, version.as_deref()).await;
    events::emit("instances-changed", ());
    e(r)
}

// ---------------- Download manager ----------------

/// Start a modpack download in the background and return its download-manager id. The launcher
/// stays usable; the top-bar queue shows progress, speed and ETA, and can pause/cancel it.
#[tauri::command]
fn download_modpack(source: String, project: String, version: Option<String>, name: String) -> u64 {
    let h = downloads::start(&name, "modpack");
    let id = h.id;
    tauri::async_runtime::spawn(async move {
        match packs::install_modpack_tracked(&source, &project, version.as_deref(), &h).await {
            Ok(_) => {
                events::emit("instances-changed", ());
                downloads::finish(id, Ok(()));
            }
            Err(err) => downloads::finish(id, Err(format!("{err:#}"))),
        }
    });
    id
}

#[tauri::command]
fn download_list() -> Vec<downloads::DownloadView> {
    downloads::list()
}

#[tauri::command]
fn download_pause(id: u64) {
    downloads::pause(id);
}

#[tauri::command]
fn download_resume(id: u64) {
    downloads::resume(id);
}

#[tauri::command]
fn download_cancel(id: u64) {
    downloads::cancel(id);
}

#[tauri::command]
fn download_clear() {
    downloads::clear_done();
}

// ---------------- MaxPack helper bot ----------------

#[tauri::command]
async fn modbot_chat(mc: String, loader: String, messages: Vec<modbot::Msg>) -> Res<modbot::ChatReply> {
    e(modbot::chat(&mc, &loader, messages).await)
}

#[tauri::command]
async fn modbot_build(mc: String, loader: String, name: String, slugs: Vec<String>) -> Res<Instance> {
    let r = modbot::build(&mc, &loader, &name, slugs).await;
    events::emit("instances-changed", ());
    e(r)
}

#[tauri::command]
async fn modbot_add(instance: String, slugs: Vec<String>) -> Res<content::InstallReport> {
    e(modbot::add(&instance, slugs).await)
}

// ---------------- Clips feed ----------------

#[tauri::command]
async fn clips_feed(offset: u32, hashtag: String, player: String, mine: bool) -> Res<serde_json::Value> {
    e(social::get(&format!(
        "/clips/feed?offset={offset}&hashtag={}&player={}&mine={mine}",
        urlencoding::encode(&hashtag),
        urlencoding::encode(&player)
    ))
    .await)
}

#[tauri::command]
async fn clips_like(id: String) -> Res<serde_json::Value> {
    e(social::post("/clips/like", serde_json::json!({ "id": id })).await)
}

#[tauri::command]
async fn clips_report(id: String, reason: String) -> Res<serde_json::Value> {
    e(social::post("/clips/report", serde_json::json!({ "id": id, "reason": reason })).await)
}

#[tauri::command]
async fn clips_feed_delete(id: String) -> Res<serde_json::Value> {
    e(social::post("/clips/delete", serde_json::json!({ "id": id })).await)
}

// ---------------- Servers ----------------

#[tauri::command]
fn list_servers() -> Vec<servers::Server> {
    servers::list()
}

#[tauri::command]
fn save_server(server: servers::Server) -> Res<servers::Server> {
    e(servers::upsert(server))
}

#[tauri::command]
fn remove_server(id: String) -> Res<()> {
    e(servers::remove(&id))
}

#[tauri::command]
async fn ping_server(address: String) -> Res<servers::Status> {
    e(servers::ping(&address).await)
}

/// `justice-client --selftest <mc> <loader> [template]`: install + launch headlessly, report, kill.
fn selftest(args: &[String]) -> anyhow::Result<()> {
    let mc = args.get(2).cloned().unwrap_or_else(|| "1.21.4".into());
    let loader = args.get(3).cloned().unwrap_or_else(|| "fabric".into());
    let template = args.get(4).cloned();
    tokio::runtime::Runtime::new()?.block_on(async move {
        let name = format!("selftest {loader} {mc}");
        let inst = match instances::list().into_iter().find(|i| i.name == name) {
            Some(i) => i,
            None => instances::create(Instance { name, mc_version: mc, loader: loader.clone(), ..Default::default() })?,
        };
        if let Some(t) = template {
            let r = content::install_many(&inst.id, &content::template_slugs(&[t], &loader, true), Kind::Mod).await;
            println!("mods installed: {:?}\nskipped: {:?}", r.installed, r.skipped);
        }
        let had_accounts = !auth::load_store().accounts.is_empty();
        if !had_accounts {
            auth::add_offline("JusticeTest")?;
        }
        let world = std::env::var("JC_WORLD").ok();
        // Friends bridge + sign-in, so the mod's chat and world sharing can be tested too.
        bridge::start();
        if let Err(err) = social::login().await {
            println!("social sign-in skipped: {err:#}");
        }
        for _ in 0..20 {
            if bridge::info().is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        let started = std::time::Instant::now();
        let launched_at = std::time::SystemTime::now();
        let res = launch::launch(&inst.id, world.as_ref().map(|w| format!("world:{w}"))).await;
        println!("launch returned after {:?}: {:?}", started.elapsed(), res.as_ref().err().map(|e| format!("{e:#}")));
        res?;
        let mut ok = false;
        for _ in 0..180 {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let mut logs = launch::logs(&inst.id);
            // Forge doesn't echo its log to stdout; read the game's own log file too.
            // Only this run's file (the last run's stays until the game opens a new one). Lossy: device
            // names in it come in the Windows code page, not UTF-8.
            let log = instances::game_dir(&inst.id).join("logs").join("latest.log");
            let fresh = std::fs::metadata(&log).and_then(|m| m.modified()).is_ok_and(|t| t >= launched_at);
            if let (true, Ok(t)) = (fresh, std::fs::read(&log)) {
                logs.extend(String::from_utf8_lossy(&t).lines().map(String::from));
            }
            let ready = match world {
                // In a world: joined, then survive 20 s of rendering without a mixin failure.
                Some(_) => logs.iter().any(|l| l.contains("logged in with entity id")),
                None => logs.iter().any(|l| l.contains("Sound engine started") || l.contains("Created: 1024x")),
            };
            if ready {
                if world.is_some() {
                    tokio::time::sleep(std::time::Duration::from_secs(20)).await;
                    let logs = launch::logs(&inst.id);
                    ok = launch::running_ids().contains(&inst.id) && !logs.iter().any(|l| l.contains("Mixin apply") || l.contains("Game crashed"));
                } else {
                    ok = true;
                }
                break;
            }
            if !launch::running_ids().contains(&inst.id) {
                break;
            }
        }
        // JC_HOLD=<seconds>: keep the game open longer (e.g. for the mod's UI screenshots on the title screen).
        let hold = std::env::var("JC_HOLD").ok().and_then(|h| h.parse().ok()).unwrap_or(3);
        tokio::time::sleep(std::time::Duration::from_secs(hold)).await;
        let logs = launch::logs(&inst.id);
        println!("---- last log lines ----");
        for l in logs.iter().rev().take(25).collect::<Vec<_>>().into_iter().rev() {
            println!("{l}");
        }
        let _ = launch::kill(&inst.id);
        if !had_accounts {
            let _ = auth::remove(&format!("offline-{}", auth::offline_uuid("JusticeTest")));
        }
        println!("SELFTEST {}", if ok { "OK" } else { "FAILED" });
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        // The test instance is throwaway: remove it so tests don't pile up in the user's instance list.
        // JC_KEEP=1 keeps it (e.g. to look at its logs or screenshots afterwards).
        if std::env::var_os("JC_KEEP").is_none() {
            let _ = instances::delete(&inst.id);
        }
        Ok(())
    })
}

pub fn run() {
    // WebKitGTK's DMA-BUF renderer shows a blank window on many NVIDIA + Wayland setups, so it's off
    // there. Elsewhere it stays on: without it every frame is drawn and copied on the CPU, and the
    // whole launcher lags. Laptops with an Intel/AMD chip next to the NVIDIA one (Optimus) draw the
    // desktop on that chip, so they keep it too.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() && std::path::Path::new("/proc/driver/nvidia/version").exists() && !has_integrated_gpu() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    // WebKitGTK plays <video> through GStreamer, and its hardware decoders (VA-API, NVDEC) crash the
    // web process on many drivers — the Clips player took the whole window down. Software decoding
    // (gst-libav) is plenty for a 1080p clip.
    #[cfg(target_os = "linux")]
    if std::env::var_os("GST_PLUGIN_FEATURE_RANK").is_none() {
        let hw = ["vaapih264dec", "vaapih265dec", "vah264dec", "vah265dec", "nvh264dec", "nvh265dec", "nvh264sldec", "nvh265sldec", "v4l2h264dec", "v4l2h265dec"];
        std::env::set_var("GST_PLUGIN_FEATURE_RANK", hw.map(|d| format!("{d}:NONE")).join(","));
    }
    let args: Vec<String> = std::env::args().collect();
    linfo!("Justice Client {} on {} {} - args {:?}", env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH, &args[1..]);
    linfo!("data dir: {} | log file: {}", settings::data_dir().display(), logger::path().display());
    if args.get(1).map(|a| a == "--install").unwrap_or(false) {
        // --install <instance> <modrinth|curseforge> <project>
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let r = rt.block_on(content::install(&args[2], &args[3], &args[4], Kind::Mod, None));
        match r {
            Ok(r) => println!("installed: {:?}\nmanual: {:?}\nskipped: {:?}", r.installed, r.manual, r.skipped),
            Err(err) => println!("error: {err:#}"),
        }
        return;
    }
    if args.get(1).map(|a| a == "--compat").unwrap_or(false) {
        let id = args.get(2).cloned().unwrap_or_default();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        println!("before: {:#?}", compat::check(&id));
        let log = args.get(3).and_then(|p| std::fs::read_to_string(p).ok());
        let res = match log {
            Some(l) => rt.block_on(compat::repair_from_log(&id, &l)),
            None => rt.block_on(compat::fix(&id)),
        };
        match res {
            Ok(r) => println!("fixed: {:?}\nremaining: {:#?}", r.fixed, r.remaining),
            Err(err) => println!("error: {err:#}"),
        }
        return;
    }
    if args.get(1).map(|a| a == "--import").unwrap_or(false) {
        // --import <modpack file>: same as the "Import modpack" button, for testing.
        let path = args.get(2).cloned().unwrap_or_default();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        match rt.block_on(packs::import_file(&path)) {
            Ok(i) => println!("imported: {} ({} {} {})", i.id, i.mc_version, i.loader, i.loader_version),
            Err(err) => println!("error: {err:#}"),
        }
        return;
    }
    if args.get(1).map(|a| a == "--world-test").unwrap_or(false) {
        // End-to-end relay check: a local echo server stands in for the LAN world, we host it
        // through the relay, join our own world and expect our bytes back.
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            use tokio::io::{AsyncReadExt, AsyncWriteExt};
            let echo = tokio::net::TcpListener::bind(("127.0.0.1", 0)).await.unwrap();
            let echo_port = echo.local_addr().unwrap().port();
            tokio::spawn(async move {
                while let Ok((mut s, _)) = echo.accept().await {
                    tokio::spawn(async move {
                        let mut b = vec![0u8; 65536];
                        while let Ok(n) = s.read(&mut b).await {
                            if n == 0 || s.write_all(&b[..n]).await.is_err() {
                                break;
                            }
                        }
                    });
                }
            });
            if let Err(err) = social::login().await {
                return println!("login failed: {err:#}");
            }
            let id = match world::host(echo_port, "Relay test", "1.21.11", "fabric").await {
                Ok(v) => v["id"].as_str().unwrap_or_default().to_string(),
                Err(err) => return println!("host failed: {err:#}"),
            };
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let port = match world::join(&id).await {
                Ok(p) => p,
                Err(err) => return println!("join failed: {err:#}"),
            };
            let mut c = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
            let payload: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
            let started = std::time::Instant::now();
            c.write_all(&payload).await.unwrap();
            let mut got = vec![0u8; payload.len()];
            let res = tokio::time::timeout(std::time::Duration::from_secs(20), c.read_exact(&mut got)).await;
            match res {
                Ok(Ok(_)) if got == payload => println!("WORLD RELAY OK: 200 KB round trip in {:?}", started.elapsed()),
                other => println!("WORLD RELAY FAILED: {other:?}"),
            }
            world::stop().await;
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        });
        return;
    }
    if args.get(1).map(|a| a == "--optimize-apply" || a == "--optimize-undo").unwrap_or(false) {
        // Test helpers: apply every proposed change, or undo the last optimization.
        let id = args.get(2).cloned().unwrap_or_default();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let r = rt.block_on(async {
            if args[1] == "--optimize-undo" {
                return optimizer::undo(&id).map(|_| "undone".to_string());
            }
            let p = optimizer::plan(&id).await?;
            let ids = p.changes.iter().map(|c| c.id.clone()).collect();
            let r = optimizer::apply(&id, ids).await?;
            Ok(serde_json::to_string_pretty(&r)?)
        });
        match r {
            Ok(t) => println!("{t}"),
            Err(err) => {
                eprintln!("OPTIMIZE ERROR: {err:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(|a| a == "--optimize-plan").unwrap_or(false) {
        // Read-only: prints what the Optimize button would propose for an instance.
        let id = args.get(2).cloned().unwrap_or_default();
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        match rt.block_on(optimizer::plan(&id)) {
            Ok(p) => println!("{}", serde_json::to_string_pretty(&p).unwrap_or_default()),
            Err(err) => {
                eprintln!("OPTIMIZE PLAN ERROR: {err:#}");
                std::process::exit(1);
            }
        }
        return;
    }
    if args.get(1).map(|a| a == "--modpack-test").unwrap_or(false) {
        // Timing check: Modrinth modpack -> instance, export, re-import of the export; deletes both after.
        let project = args.get(2).cloned().unwrap_or_else(|| "fabulously-optimized".into());
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let ok = rt.block_on(async {
            let t = std::time::Instant::now();
            let a = packs::install_modpack("modrinth", &project, None).await?;
            let mods = content::load_meta(&a.id).len();
            println!("install {project}: {:?}, {mods} tracked files", t.elapsed());
            let zip = settings::cache_dir().join("modpack-test.zip");
            let t = std::time::Instant::now();
            packs::export_pack(&a.id, &zip.to_string_lossy())?;
            println!("export: {:?}, {} KB", t.elapsed(), std::fs::metadata(&zip)?.len() / 1024);
            let t = std::time::Instant::now();
            let b = packs::import_file(&zip.to_string_lossy()).await?;
            println!("re-import: {:?}, {} tracked files", t.elapsed(), content::load_meta(&b.id).len());
            let _ = std::fs::remove_file(&zip);
            instances::delete(&a.id)?;
            instances::delete(&b.id)?;
            anyhow::Ok(())
        });
        if let Err(err) = ok {
            eprintln!("MODPACK TEST ERROR: {err:#}");
            std::process::exit(1);
        }
        println!("MODPACK TEST OK");
        return;
    }
    if args.get(1).map(|a| a == "--social-test").unwrap_or(false) {
        // Signs in to Justice Social with the active account and prints profile + friends.
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        rt.block_on(async {
            match social::login().await {
                Ok(v) => println!("login ok: {} {}", v["name"], v["uuid"]),
                Err(err) => return println!("login failed: {err:#}"),
            }
            println!("me: {:?}", social::get("/me").await.map(|v| v.to_string()));
            println!("friends: {:?}", social::get("/friends").await.map(|v| v.to_string()));
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        });
        return;
    }
    if args.get(1).map(|a| a == "--selftest").unwrap_or(false) {
        if let Err(err) = selftest(&args) {
            eprintln!("SELFTEST ERROR: {err:#}");
            std::process::exit(1);
        }
        return;
    }
    tauri::Builder::default()
        // Must be first. A second launch hands its arguments to the running launcher:
        // `justice-client --save-clip` saves a clip (for Wayland, where global hotkeys don't work).
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if args.iter().any(|a| a == "--save-clip") {
                clips::on_hotkey(app);
            } else {
                clips::show_main(app);
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, ev| {
                    if ev.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed {
                        clips::on_hotkey(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            events::init(app.handle().clone());
            ldebug!("window ready, resources in {:?}", tauri::Manager::path(app).resource_dir().ok());
            if let Err(err) = clips::init(app.handle()) {
                lwarn!("clips init failed: {err}");
            }
            bridge::start();
            sonolith::start();
            // Friends: sign in quietly with the active Microsoft account.
            tauri::async_runtime::spawn(async {
                if auth::load_store().accounts.iter().any(|a| a.kind == "msa") {
                    if let Err(err) = social::login().await {
                        lwarn!("social sign-in skipped: {err:#}");
                    }
                }
            });
            let _ = std::fs::create_dir_all(settings::instances_dir());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings, save_settings, system_info,
            instance_last_place, media_state, media_command, open_music_app, open_logs_window, version_art, hardware_info, free_space, move_data_dir,
            game_versions, loader_versions,
            list_instances, get_instance, create_instance, update_instance, delete_instance, duplicate_instance, instance_path,
            launch_instance, kill_game, running_games, game_logs, analyze_crash,
            list_accounts, add_offline_account, ms_login_start, ms_login_poll, ms_login_cancel, set_active_account, remove_account,
            get_capes, set_cape, upload_skin, skin_from_player, reset_skin, fetch_image,
            skinlib_list, skinlib_add_file, skinlib_add_player, skinlib_add_current, skinlib_apply, skinlib_rename, skinlib_remove,
            search_content, project_details, install_content, list_content, toggle_content, remove_content, add_local_files, check_compat, fix_compat, repair_crash, repair_plan, repair_apply, repair_undo, set_pinned,
            check_updates, apply_updates, optimize_instance,
            optimize_plan,
            sonolith_status,
            sonolith_connect,
            sonolith_disconnect,
            sonolith_call,
            optimize_apply,
            optimize_undo,
            save_mod_profile, apply_mod_profile, delete_mod_profile, list_media, delete_media, obs_test,
            list_worlds, backup_world, snapshot_instance, list_backups, restore_backup, delete_backup,
            share_code, import_code, export_pack, import_pack, install_modpack,
            download_modpack, download_list, download_pause, download_resume, download_cancel, download_clear,
            modbot_chat, modbot_build, modbot_add,
            list_servers, save_server, remove_server, ping_server,
            log_frontend, log_path, check_update_manual, set_instance_icon,
            social_login, social_logout, social_status, social_get, social_post, social_put, social_upload, social_open_clip, social_login_password, pvp_stats, party_warp_here, social_typing, world_join,
            clips::clips_get_settings, clips::clips_set_settings, clips::clips_status, clips::clips_save, clips::clips_set_recording,
            clips::clips_list, clips::clips_strip, clips::clips_delete, clips::clips_rename, clips::clips_reveal, clips::clips_open_dir,
            clips::clips_trim, clips::clips_devices, clips::clips_monitors, clips::clips_log, clips::clips_test_notification,
            clips::clips_publish, clips_feed, clips_like, clips_report, clips_feed_delete,
            clips::clips_overlay_hide, clips::clips_detect_ffmpeg,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Justice Client")
        .run(|app, ev| match ev {
            // The hidden Clips overlay window would keep the app alive: closing the launcher quits,
            // unless a game is being recorded, then it goes to the tray.
            tauri::RunEvent::WindowEvent { label, event: tauri::WindowEvent::CloseRequested { api, .. }, .. } if label == "main" => {
                if (clips::enabled() || social::signed_in().is_some()) && !launch::running_ids().is_empty() {
                    api.prevent_close();
                    if let Some(w) = tauri::Manager::get_webview_window(app, "main") {
                        let _ = w.hide();
                    }
                } else {
                    clips::shutdown(app);
                    app.exit(0);
                }
            }
            tauri::RunEvent::Exit => clips::shutdown(app),
            _ => {}
        });
}

/// An Intel (0x8086) or AMD (0x1002) GPU besides NVIDIA, as on Optimus laptops.
#[cfg(target_os = "linux")]
fn has_integrated_gpu() -> bool {
    std::fs::read_dir("/sys/class/drm")
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with("card") && !e.file_name().to_string_lossy().contains('-'))
        .filter_map(|e| std::fs::read_to_string(e.path().join("device/vendor")).ok())
        .any(|v| matches!(v.trim(), "0x8086" | "0x1002"))
}
