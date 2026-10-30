//! Downloads Eclipse Temurin runtimes on demand, one per Java major version.

use crate::events;
use crate::net;
use crate::platform::{ADOPTIUM_OS, JAVA_CONSOLE, JAVA_GUI};
use crate::settings;
use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use std::io::Write;
use std::path::{Path, PathBuf};

fn find_java(dir: &Path) -> Option<PathBuf> {
    let direct = dir.join("bin").join(JAVA_GUI);
    if direct.exists() {
        return Some(direct);
    }
    for e in std::fs::read_dir(dir).ok()?.flatten() {
        let p = e.path().join("bin").join(JAVA_GUI);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

/// Minecraft 1.17 asks for Java 16, which Temurin no longer ships — 17 runs it fine.
pub fn normalize(major: u32) -> u32 {
    match major {
        0..=8 => 8,
        9..=17 => 17,
        m => m,
    }
}

pub async fn ensure(major: u32, task: &str) -> Result<PathBuf> {
    let major = normalize(major);
    let dir = settings::java_dir().join(major.to_string());
    if let Some(p) = find_java(&dir) {
        return Ok(p);
    }
    let dl_label = l!("Stahuji Javu {major}", "Downloading Java {major}");
    events::progress(task, &dl_label, 0, 1);
    let url = format!(
        "https://api.adoptium.net/v3/binary/latest/{major}/ga/{ADOPTIUM_OS}/x64/jre/hotspot/normal/eclipse"
    );
    let resp = net::CLIENT.get(&url).send().await?;
    if !resp.status().is_success() {
        bail!(l!("Java {major} se nepodařilo stáhnout ({})", "Couldn't download Java {major} ({})", resp.status()));
    }
    let total = resp.content_length().unwrap_or(0) as usize;
    // Windows builds are .zip, Linux builds are .tar.gz.
    let ext = if cfg!(windows) { "zip" } else { "tar.gz" };
    let archive = settings::cache_dir().join(format!("java-{major}.{ext}"));
    std::fs::create_dir_all(archive.parent().unwrap())?;
    let mut file = std::fs::File::create(&archive)?;
    let mut stream = resp.bytes_stream();
    let mut got = 0usize;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk)?;
        got += chunk.len();
        events::progress(task, &dl_label, got, total.max(got));
    }
    drop(file);

    events::progress(task, &l!("Rozbaluji Javu {major}", "Unpacking Java {major}"), 0, 1);
    let archive2 = archive.clone();
    let dir2 = dir.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        std::fs::create_dir_all(&dir2)?;
        let f = std::fs::File::open(&archive2)?;
        if cfg!(windows) {
            zip::ZipArchive::new(f)?.extract(&dir2).context("extract java")?;
        } else {
            // tar keeps the executable bits on bin/java.
            tar::Archive::new(flate2::read::GzDecoder::new(f)).unpack(&dir2).context("extract java")?;
        }
        Ok(())
    })
    .await??;
    let _ = std::fs::remove_file(&archive);
    find_java(&dir).ok_or_else(|| anyhow!(l!("Java {major} se po rozbalení nenašla", "Java {major} wasn't found after unpacking")))
}

/// `java.exe` next to a `javaw.exe` — used for installers where we want exit codes and output.
pub fn console_java(java: &Path) -> PathBuf {
    let j = java.with_file_name(JAVA_CONSOLE);
    if j.exists() {
        j
    } else {
        java.to_path_buf()
    }
}
