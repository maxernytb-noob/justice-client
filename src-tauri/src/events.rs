use once_cell::sync::OnceCell;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

static APP: OnceCell<AppHandle> = OnceCell::new();

pub fn init(app: AppHandle) {
    let _ = APP.set(app);
}

pub fn app() -> Option<&'static AppHandle> {
    APP.get()
}

pub fn emit<T: Serialize + Clone>(event: &str, payload: T) {
    if let Some(app) = APP.get() {
        let _ = app.emit(event, payload);
    }
}

#[derive(Clone, Serialize)]
struct ToastEvent {
    text: String,
    kind: &'static str,
}

/// A short message the launcher window shows as a toast (ignored when no window is open).
pub fn toast(text: &str) {
    emit("toast", ToastEvent { text: text.to_string(), kind: "info" });
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// Key the UI groups progress by, e.g. an instance id.
    pub task: String,
    pub label: String,
    pub done: usize,
    pub total: usize,
}

pub fn progress(task: &str, label: &str, done: usize, total: usize) {
    if APP.get().is_none() && (done == 0 || done == total) {
        println!("[{task}] {label} {done}/{total}");
    }
    emit(
        "progress",
        Progress { task: task.into(), label: label.into(), done, total },
    );
}

pub fn progress_done(task: &str) {
    progress(task, "", 1, 0);
}
