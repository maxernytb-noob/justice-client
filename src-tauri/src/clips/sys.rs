//! The few OS calls Clips needs outside the recorder itself: bin, file manager, sounds, foreground app.

use std::path::Path;

pub struct Foreground {
    /// App in the foreground (used to name clips when no Minecraft instance is running).
    pub game: String,
    /// Work area (left, top, right, bottom) of its monitor; Windows overlay placement only.
    pub work: (i32, i32, i32, i32),
}

#[cfg(windows)]
mod imp {
    use super::*;

    pub fn recycle(path: &Path) -> std::io::Result<()> {
        super::super::win::recycle(path)
    }

    pub fn set_hidden(path: &Path) {
        super::super::win::set_hidden(path)
    }

    pub fn play_blip() {
        super::super::win::play_blip()
    }

    pub fn foreground() -> Foreground {
        let f = super::super::win::foreground();
        Foreground { game: f.game, work: f.work }
    }

    pub fn reveal(path: &str) {
        use std::os::windows::process::CommandExt;
        let _ = std::process::Command::new("explorer").raw_arg(format!("/select,\"{path}\"")).spawn();
    }

    pub fn open_dir(dir: &Path) {
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;

    /// Freedesktop trash via `gio trash` (GNOME/KDE/most setups); plain delete when that's missing.
    pub fn recycle(path: &Path) -> std::io::Result<()> {
        let ok = std::process::Command::new("gio").arg("trash").arg(path).status().map(|s| s.success()).unwrap_or(false);
        if ok { Ok(()) } else { std::fs::remove_file(path) }
    }

    /// Dot-folders are already hidden on Linux.
    pub fn set_hidden(_path: &Path) {}

    pub fn play_blip() {
        // Best effort: the freedesktop "complete" sound through libcanberra, if installed.
        let _ = std::process::Command::new("canberra-gtk-play").args(["-i", "complete"]).spawn();
    }

    /// No portable way to read the focused window on Wayland; clips get the instance name anyway.
    pub fn foreground() -> Foreground {
        Foreground { game: "Desktop".into(), work: (0, 0, 0, 0) }
    }

    pub fn reveal(path: &str) {
        let p = Path::new(path);
        let dir = p.parent().unwrap_or(p);
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }

    pub fn open_dir(dir: &Path) {
        let _ = std::process::Command::new("xdg-open").arg(dir).spawn();
    }
}

pub use imp::*;
