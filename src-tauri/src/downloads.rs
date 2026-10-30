//! Download manager: tracks background downloads (modpacks for now) so they show in the top bar,
//! the launcher stays usable, and each can be paused, resumed, cancelled, with live speed and ETA.
//! The actual bytes are moved by `net::download_all_tracked`; this module owns the UI-facing state.

use crate::events;
use crate::net;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::sync::atomic::{AtomicU64, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);
static ENTRIES: Lazy<Mutex<Vec<Entry>>> = Lazy::new(Default::default);
static TICKER: std::sync::Once = std::sync::Once::new();

struct Entry {
    id: u64,
    name: String,
    kind: String, // "modpack" | "content"
    done: Arc<AtomicU64>,
    total: Arc<AtomicU64>,
    control: Arc<AtomicU8>,
    status: String, // "running" | "paused" | "done" | "error" | "canceled"
    error: Option<String>,
    // For the speed estimate: bytes and time at the last tick.
    last_bytes: u64,
    last_tick: Instant,
    speed: u64, // bytes/s, smoothed
}

/// What the install task uses to report bytes and read pause/cancel.
pub struct Handle {
    pub id: u64,
    pub done: Arc<AtomicU64>,
    pub total: Arc<AtomicU64>,
    pub control: Arc<AtomicU8>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DownloadView {
    pub id: u64,
    pub name: String,
    pub kind: String,
    pub done: u64,
    pub total: u64,
    pub speed: u64,
    pub eta_secs: u64,
    pub status: String,
    pub error: Option<String>,
}

fn view(e: &Entry) -> DownloadView {
    let (done, total) = (e.done.load(Ordering::Relaxed), e.total.load(Ordering::Relaxed));
    let eta_secs = if e.status == "running" && e.speed > 0 && total > done { (total - done) / e.speed } else { 0 };
    DownloadView {
        id: e.id,
        name: e.name.clone(),
        kind: e.kind.clone(),
        done,
        total,
        speed: if e.status == "running" { e.speed } else { 0 },
        eta_secs,
        status: e.status.clone(),
        error: e.error.clone(),
    }
}

pub fn list() -> Vec<DownloadView> {
    ENTRIES.lock().unwrap().iter().map(view).collect()
}

fn emit() {
    events::emit("downloads", list());
}

/// Register a new download and get the handle the worker reports through. Starts the UI ticker.
pub fn start(name: &str, kind: &str) -> Handle {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let done = Arc::new(AtomicU64::new(0));
    let total = Arc::new(AtomicU64::new(0));
    let control = Arc::new(AtomicU8::new(net::DL_RUN));
    ENTRIES.lock().unwrap().push(Entry {
        id,
        name: name.to_string(),
        kind: kind.to_string(),
        done: done.clone(),
        total: total.clone(),
        control: control.clone(),
        status: "running".into(),
        error: None,
        last_bytes: 0,
        last_tick: Instant::now(),
        speed: 0,
    });
    start_ticker();
    emit();
    Handle { id, done, total, control }
}

/// Mark a download finished: ok, cancelled (if that's why it stopped), or errored.
pub fn finish(id: u64, result: Result<(), String>) {
    let mut es = ENTRIES.lock().unwrap();
    if let Some(e) = es.iter_mut().find(|e| e.id == id) {
        let canceled = e.control.load(Ordering::Relaxed) == net::DL_CANCEL;
        match result {
            _ if canceled => e.status = "canceled".into(),
            Ok(()) => { e.status = "done".into(); e.done.store(e.total.load(Ordering::Relaxed).max(e.done.load(Ordering::Relaxed)), Ordering::Relaxed); }
            Err(msg) => { e.status = "error".into(); e.error = Some(msg); }
        }
        e.speed = 0;
    }
    drop(es);
    emit();
}

fn set_control(id: u64, v: u8, status: &str) {
    let mut es = ENTRIES.lock().unwrap();
    if let Some(e) = es.iter_mut().find(|e| e.id == id) {
        if e.status == "running" || e.status == "paused" {
            e.control.store(v, Ordering::Relaxed);
            if e.status != status {
                e.status = status.to_string();
            }
        }
    }
    drop(es);
    emit();
}

pub fn pause(id: u64) {
    set_control(id, net::DL_PAUSE, "paused");
}
pub fn resume(id: u64) {
    set_control(id, net::DL_RUN, "running");
}
pub fn cancel(id: u64) {
    set_control(id, net::DL_CANCEL, "canceled");
}

/// Drop finished/cancelled/errored rows from the list (the "clear" button).
pub fn clear_done() {
    ENTRIES.lock().unwrap().retain(|e| e.status == "running" || e.status == "paused");
    emit();
}

/// Background ticker: once a second, recompute each running download's speed and push an update.
fn start_ticker() {
    TICKER.call_once(|| {
        tauri::async_runtime::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(1000)).await;
                let mut changed = false;
                {
                    let mut es = ENTRIES.lock().unwrap();
                    for e in es.iter_mut() {
                        if e.status != "running" {
                            continue;
                        }
                        let now = Instant::now();
                        let bytes = e.done.load(Ordering::Relaxed);
                        let dt = now.duration_since(e.last_tick).as_secs_f64().max(0.001);
                        let inst = ((bytes.saturating_sub(e.last_bytes)) as f64 / dt) as u64;
                        // Smooth so the number doesn't jump around.
                        e.speed = (e.speed * 2 + inst) / 3;
                        e.last_bytes = bytes;
                        e.last_tick = now;
                        changed = true;
                    }
                    // Stop ticking once nothing is active (the Once won't re-fire, so re-arm on next start()).
                    if es.iter().all(|e| e.status != "running") {
                        // leave entries as-is; just stop emitting speed churn
                    }
                }
                if changed {
                    emit();
                }
            }
        });
    });
}
