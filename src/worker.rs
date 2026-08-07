//! Dedicated browser worker thread — `chaser_oxide::Browser` is not `Send`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chaser_oxide::{Browser, BrowserConfig, Page};
use futures::StreamExt;
use tokio::sync::{mpsc, oneshot};
use tracing::{info, warn};

use crate::blocklist::Blocklist;
use crate::config::Config;
use crate::fetch::{self, FetchResult};
use crate::stealth::ProfileKind;

pub struct FetchJob {
    pub url: String,
    pub timeout_ms: u64,
    pub session_key: Option<String>,
    pub proxy: Option<String>,
    pub stealth: bool,
    pub reply: oneshot::Sender<Result<FetchResult, String>>,
}

pub enum Job {
    Warm {
        reply: oneshot::Sender<Result<(), String>>,
    },
    Fetch(FetchJob),
}

#[derive(Clone)]
pub struct BrowserHandle {
    tx: mpsc::Sender<Job>,
}

impl BrowserHandle {
    pub fn start(config: Config, blocklist: Blocklist, profile_kind: ProfileKind) -> Self {
        let (tx, rx) = mpsc::channel::<Job>(32);
        std::thread::Builder::new()
            .name("headless-oxider-browser".into())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("browser worker runtime");
                rt.block_on(async move {
                    let local = tokio::task::LocalSet::new();
                    local
                        .run_until(worker_loop(config, blocklist, profile_kind, rx))
                        .await;
                });
            })
            .expect("spawn browser worker thread");
        Self { tx }
    }

    pub async fn warm(&self) -> Result<(), String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Job::Warm { reply })
            .await
            .map_err(|_| "browser worker gone".to_string())?;
        rx.await.map_err(|_| "browser worker dropped".to_string())?
    }

    pub async fn fetch(
        &self,
        url: String,
        timeout_ms: u64,
        session_key: Option<String>,
        proxy: Option<String>,
        stealth: bool,
    ) -> Result<FetchResult, String> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(Job::Fetch(FetchJob {
                url,
                timeout_ms,
                session_key,
                proxy,
                stealth,
                reply,
            }))
            .await
            .map_err(|_| "browser worker gone".to_string())?;
        rx.await.map_err(|_| "browser worker dropped".to_string())?
    }
}

async fn worker_loop(
    config: Config,
    blocklist: Blocklist,
    profile_kind: ProfileKind,
    mut rx: mpsc::Receiver<Job>,
) {
    let mut browsers: HashMap<String, Browser> = HashMap::new();
    while let Some(job) = rx.recv().await {
        match job {
            Job::Warm { reply } => {
                let result = ensure_browser(&mut browsers, &config, None, None).await;
                let _ = reply.send(result.map(|_| ()));
            }
            Job::Fetch(job) => {
                let result = run_fetch(
                    &mut browsers,
                    &config,
                    &blocklist,
                    &profile_kind,
                    job.url,
                    job.timeout_ms,
                    job.session_key.as_deref(),
                    job.proxy.as_deref(),
                    job.stealth,
                )
                .await;
                let _ = job.reply.send(result);
            }
        }
    }
    info!("browser worker stopped");
}

fn cache_key(session: Option<&str>, proxy: Option<&str>) -> String {
    format!(
        "s:{}|p:{}",
        session.unwrap_or("_default"),
        proxy.unwrap_or("_env")
    )
}

async fn ensure_browser(
    browsers: &mut HashMap<String, Browser>,
    config: &Config,
    session_key: Option<&str>,
    proxy_override: Option<&str>,
) -> Result<(), String> {
    let proxy = proxy_override
        .map(str::to_string)
        .or_else(|| config.proxy_url.clone());
    let key = cache_key(session_key, proxy.as_deref());
    if browsers.contains_key(&key) {
        return Ok(());
    }
    let user_data = session_key.map(|s| sanitize_session_dir(&config.sessions_dir, s));
    if let Some(ref dir) = user_data {
        if let Err(err) = std::fs::create_dir_all(dir) {
            warn!(%err, path = %dir.display(), "could not create session dir");
        }
    }
    let browser = launch_browser(config, proxy.as_deref(), user_data.as_deref()).await?;
    browsers.insert(key, browser);
    Ok(())
}

async fn open_page(
    browsers: &mut HashMap<String, Browser>,
    config: &Config,
    session_key: Option<&str>,
    proxy_override: Option<&str>,
) -> Result<Page, String> {
    ensure_browser(browsers, config, session_key, proxy_override).await?;
    let proxy = proxy_override
        .map(str::to_string)
        .or_else(|| config.proxy_url.clone());
    let key = cache_key(session_key, proxy.as_deref());
    let browser = browsers
        .get(&key)
        .ok_or_else(|| "browser missing".to_string())?;
    browser
        .new_page("about:blank")
        .await
        .map_err(|e| e.to_string())
}

async fn run_fetch(
    browsers: &mut HashMap<String, Browser>,
    config: &Config,
    blocklist: &Blocklist,
    profile_kind: &ProfileKind,
    url: String,
    timeout_ms: u64,
    session_key: Option<&str>,
    proxy: Option<&str>,
    stealth: bool,
) -> Result<FetchResult, String> {
    let page = open_page(browsers, config, session_key, proxy).await?;
    fetch::fetch_with_page(page, blocklist, profile_kind, stealth, &url, timeout_ms).await
}

fn sanitize_session_dir(root: &Path, key: &str) -> PathBuf {
    let safe: String = key
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    let safe = if safe.is_empty() {
        "session".into()
    } else {
        safe
    };
    root.join(safe)
}

async fn launch_browser(
    config: &Config,
    proxy: Option<&str>,
    user_data_dir: Option<&Path>,
) -> Result<Browser, String> {
    let mut builder = BrowserConfig::builder();

    if config.headful {
        builder = builder.with_head();
        info!("launching headed Chromium (Xvfb recommended)");
    } else {
        builder = builder.new_headless_mode();
    }

    // Dedicated API — `.arg("--no-sandbox")` alone is not enough when sandbox=true.
    builder = builder
        .no_sandbox()
        .arg("--disable-dev-shm-usage")
        .arg("--disable-blink-features=AutomationControlled");

    if let Some(path) = &config.chrome_path {
        builder = builder.chrome_executable(path.clone());
    }
    if let Some(dir) = user_data_dir {
        builder = builder.arg(format!("--user-data-dir={}", dir.display()));
    }
    if let Some(proxy) = proxy {
        builder = builder.arg(format!("--proxy-server={proxy}"));
        info!(%proxy, "launching with proxy");
    }

    let cfg = builder.build().map_err(|e| e.to_string())?;
    let (browser, mut handler) = Browser::launch(cfg)
        .await
        .map_err(|e| e.to_string())?;

    tokio::task::spawn_local(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    Ok(browser)
}
