use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use once_cell::sync::Lazy;
use serde_json::Value;
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

/// Download control flag shared with the download manager.
pub const DL_RUN: u8 = 0;
pub const DL_PAUSE: u8 = 1;
pub const DL_CANCEL: u8 = 2;

pub static CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .user_agent("JusticeClient/0.1.0 (github.com/justice-client; youtube.com/@maxernMC)")
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(300))
        .build()
        .expect("http client")
});

pub async fn get_json(url: &str) -> Result<Value> {
    let r = CLIENT.get(url).send().await.with_context(|| format!("GET {url}"));
    let r = match r {
        Ok(r) => r,
        Err(err) => {
            lwarn!("{err:#}");
            return Err(err);
        }
    };
    let status = r.status();
    ldebug!("GET {url} -> {status}");
    if !status.is_success() {
        bail!("GET {url} -> {status}");
    }
    Ok(r.json().await?)
}

/// Fetch JSON and cache it on disk; fall back to the cached copy when offline.
pub async fn get_json_cached(url: &str, cache: &Path, max_age_secs: u64) -> Result<Value> {
    if let Ok(meta) = std::fs::metadata(cache) {
        let fresh = meta
            .modified()
            .ok()
            .and_then(|m| m.elapsed().ok())
            .map(|a| a.as_secs() < max_age_secs)
            .unwrap_or(false);
        if fresh {
            if let Ok(v) = read_json(cache) {
                return Ok(v);
            }
        }
    }
    match get_json(url).await {
        Ok(v) => {
            write_json(cache, &v)?;
            Ok(v)
        }
        Err(e) => read_json(cache).map_err(|_| e),
    }
}

pub fn read_json(p: &Path) -> Result<Value> {
    let t = std::fs::read_to_string(p).with_context(|| format!("read {}", p.display()))?;
    Ok(serde_json::from_str(&t)?)
}

pub fn write_json(p: &Path, v: &Value) -> Result<()> {
    if let Some(d) = p.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(p, serde_json::to_vec_pretty(v)?)?;
    Ok(())
}

pub fn sha1_file(p: &Path) -> Result<String> {
    let bytes = std::fs::read(p)?;
    Ok(hex::encode(Sha1::digest(&bytes)))
}

#[derive(Clone, Debug)]
pub struct Job {
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Mirrors tried in order when `url` fails (old Forge libraries moved between Mavens).
    pub alt_urls: Vec<String>,
}

impl Job {
    pub fn new(url: impl Into<String>, path: PathBuf) -> Self {
        Self { url: url.into(), path, sha1: None, size: None, alt_urls: vec![] }
    }
    fn is_done(&self) -> bool {
        match std::fs::metadata(&self.path) {
            Ok(m) => self.size.map(|s| s == m.len()).unwrap_or(m.len() > 0),
            Err(_) => false,
        }
    }
}

pub async fn download(job: &Job) -> Result<()> {
    if job.is_done() {
        return Ok(());
    }
    let mut last_err = anyhow!("no attempt");
    for url in std::iter::once(&job.url).chain(job.alt_urls.iter()) {
        for attempt in 0..3u64 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
            }
            match download_once(job, url).await {
                Ok(()) => return Ok(()),
                // A 404 won't fix itself — move on to the next mirror.
                Err(e) if e.to_string().contains("404") => {
                    last_err = e;
                    break;
                }
                Err(e) => last_err = e,
            }
        }
    }
    lwarn!("download failed {}: {last_err:#}", job.url);
    Err(last_err.context(format!("download {}", job.url)))
}

async fn download_once(job: &Job, url: &str) -> Result<()> {
    let r = CLIENT.get(url).send().await?;
    if !r.status().is_success() {
        bail!("HTTP {}", r.status());
    }
    let bytes = r.bytes().await?;
    if let Some(expected) = &job.sha1 {
        let got = hex::encode(Sha1::digest(&bytes));
        if !got.eq_ignore_ascii_case(expected) {
            bail!("sha1 mismatch ({got} != {expected})");
        }
    }
    if let Some(d) = job.path.parent() {
        tokio::fs::create_dir_all(d).await?;
    }
    let tmp = job.path.with_extension("part");
    tokio::fs::write(&tmp, &bytes).await?;
    tokio::fs::rename(&tmp, &job.path).await?;
    Ok(())
}

/// Streams one file, adding each chunk's size to `done`, honouring a pause/cancel `control` flag.
async fn stream_once(job: &Job, url: &str, done: &AtomicU64, control: &AtomicU8) -> Result<()> {
    let r = CLIENT.get(url).send().await?;
    if !r.status().is_success() {
        bail!("HTTP {}", r.status());
    }
    let mut stream = r.bytes_stream();
    let mut buf: Vec<u8> = Vec::with_capacity(job.size.unwrap_or(0) as usize);
    loop {
        // Honour pause/cancel between chunks.
        loop {
            match control.load(Ordering::Relaxed) {
                DL_CANCEL => bail!("canceled"),
                DL_PAUSE => tokio::time::sleep(Duration::from_millis(200)).await,
                _ => break,
            }
        }
        match stream.next().await {
            Some(chunk) => {
                let chunk = chunk?;
                done.fetch_add(chunk.len() as u64, Ordering::Relaxed);
                buf.extend_from_slice(&chunk);
            }
            None => break,
        }
    }
    if let Some(expected) = &job.sha1 {
        let got = hex::encode(Sha1::digest(&buf));
        if !got.eq_ignore_ascii_case(expected) {
            bail!("sha1 mismatch ({got} != {expected})");
        }
    }
    if let Some(d) = job.path.parent() {
        tokio::fs::create_dir_all(d).await?;
    }
    let tmp = job.path.with_extension("part");
    tokio::fs::write(&tmp, &buf).await?;
    tokio::fs::rename(&tmp, &job.path).await?;
    Ok(())
}

/// Like `download`, but reports bytes as they arrive and can be paused/cancelled. On a size-known
/// file that is already present, counts it as instantly done.
pub async fn download_streaming(job: &Job, done: &AtomicU64, control: &AtomicU8) -> Result<()> {
    if job.is_done() {
        done.fetch_add(std::fs::metadata(&job.path).map(|m| m.len()).unwrap_or(0), Ordering::Relaxed);
        return Ok(());
    }
    let mut last_err = anyhow!("no attempt");
    for url in std::iter::once(&job.url).chain(job.alt_urls.iter()) {
        for attempt in 0..3u64 {
            if control.load(Ordering::Relaxed) == DL_CANCEL {
                bail!("canceled");
            }
            if attempt > 0 {
                tokio::time::sleep(Duration::from_millis(500 * attempt)).await;
            }
            match stream_once(job, url, done, control).await {
                Ok(()) => return Ok(()),
                Err(e) if e.to_string().contains("canceled") => return Err(e),
                Err(e) if e.to_string().contains("404") => { last_err = e; break; }
                Err(e) => last_err = e,
            }
        }
    }
    Err(last_err.context(format!("download {}", job.url)))
}

/// Parallel streaming download with a shared byte counter and pause/cancel control. Reports total
/// bytes up front (sum of known sizes). Used by the download manager so the UI can show speed/ETA.
pub async fn download_all_tracked(jobs: Vec<Job>, concurrency: usize, done: Arc<AtomicU64>, total: Arc<AtomicU64>, control: Arc<AtomicU8>) -> Result<()> {
    let pending: Vec<Job> = jobs.into_iter().filter(|j| !j.is_done()).collect();
    total.store(pending.iter().filter_map(|j| j.size).sum(), Ordering::Relaxed);
    if pending.is_empty() {
        return Ok(());
    }
    let results: Vec<Result<()>> = futures::stream::iter(pending)
        .map(|job| {
            let done = done.clone();
            let control = control.clone();
            async move { download_streaming(&job, &done, &control).await }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;
    if let Some(e) = results.into_iter().find_map(|r| r.err()) {
        return Err(e);
    }
    Ok(())
}

/// Download many files in parallel; `on_progress(done, total)` is called as files finish.
pub async fn download_all(
    jobs: Vec<Job>,
    concurrency: usize,
    on_progress: impl Fn(usize, usize) + Send + Sync + 'static,
) -> Result<()> {
    let pending: Vec<Job> = jobs.into_iter().filter(|j| !j.is_done()).collect();
    let total = pending.len();
    if total > 0 {
        ldebug!("downloading {total} files ({concurrency} at a time)");
    }
    if total == 0 {
        return Ok(());
    }
    let done = Arc::new(AtomicUsize::new(0));
    let on_progress = Arc::new(on_progress);
    on_progress(0, total);
    let results: Vec<Result<()>> = futures::stream::iter(pending)
        .map(|job| {
            let done = done.clone();
            let cb = on_progress.clone();
            async move {
                let r = download(&job).await;
                let n = done.fetch_add(1, Ordering::Relaxed) + 1;
                cb(n, total);
                r
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;
    let errors: Vec<_> = results.into_iter().filter_map(|r| r.err()).collect();
    if let Some(e) = errors.into_iter().next() {
        return Err(e);
    }
    Ok(())
}
