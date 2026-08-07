//! Headless Chromium fetch service — same REST shape as headless-playwright.
//! GET /health · POST /fetch { url, timeoutMs? } · GET /

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use chromiumoxide::browser::{Browser, BrowserConfig};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tracing::{error, info};

const DEFAULT_PORT: u16 = 9381;
const DEFAULT_TIMEOUT_MS: u64 = 45_000;
const MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone)]
struct AppState {
    browser: Arc<Mutex<Option<Browser>>>,
}

#[derive(Serialize)]
struct Blurb {
    service: &'static str,
    version: &'static str,
    endpoints: Endpoints,
}

#[derive(Serialize)]
struct Endpoints {
    health: &'static str,
    fetch: &'static str,
}

#[derive(Serialize)]
struct HealthOk {
    ok: bool,
    browser: &'static str,
}

#[derive(Serialize)]
struct HealthErr {
    ok: bool,
    error: String,
}

#[derive(Deserialize)]
struct FetchBody {
    url: String,
    #[serde(default, rename = "timeoutMs")]
    timeout_ms: Option<u64>,
}

#[derive(Serialize)]
struct FetchOk {
    ok: bool,
    title: String,
    html: String,
    url: String,
    #[serde(rename = "latencyMs")]
    latency_ms: u64,
}

#[derive(Serialize)]
struct ErrBody {
    ok: bool,
    error: String,
}

fn clamp_timeout(ms: Option<u64>) -> u64 {
    let n = ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    n.clamp(1_000, MAX_TIMEOUT_MS)
}

fn chrome_path() -> Option<PathBuf> {
    let from_env = std::env::var("CHROME_PATH")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    if from_env.is_some() {
        return from_env;
    }
    for candidate in [
        "/usr/bin/chromium",
        "/usr/bin/chromium-browser",
        "/usr/bin/google-chrome",
        "/usr/bin/google-chrome-stable",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() {
            return Some(p);
        }
    }
    None
}

async fn ensure_browser(state: &AppState) -> Result<(), String> {
    let mut guard = state.browser.lock().await;
    if guard.is_some() {
        return Ok(());
    }

    let mut builder = BrowserConfig::builder()
        .arg("--no-sandbox")
        .arg("--disable-dev-shm-usage")
        .arg("--disable-gpu");

    if let Some(path) = chrome_path() {
        builder = builder.chrome_executable(path);
    }

    let config = builder.build().map_err(|e| e.to_string())?;
    let (browser, mut handler) = Browser::launch(config)
        .await
        .map_err(|e| e.to_string())?;

    tokio::spawn(async move {
        while let Some(h) = handler.next().await {
            if h.is_err() {
                break;
            }
        }
    });

    *guard = Some(browser);
    Ok(())
}

async fn get_browser(state: &AppState) -> Result<Browser, String> {
    ensure_browser(state).await?;
    let guard = state.browser.lock().await;
    guard
        .as_ref()
        .cloned()
        .ok_or_else(|| "browser not launched".to_string())
}

async fn fetch_page(state: &AppState, url: &str, timeout_ms: u64) -> Result<(String, String, String), String> {
    let browser = get_browser(state).await?;

    let page = tokio::time::timeout(Duration::from_millis(timeout_ms), browser.new_page(url))
        .await
        .map_err(|_| "timeout opening page".to_string())?
        .map_err(|e| e.to_string())?;

    // new_page(url) already navigates; read DOM after load.
    let title = tokio::time::timeout(Duration::from_millis(timeout_ms), page.get_title())
        .await
        .map_err(|_| "timeout reading title".to_string())?
        .map_err(|e| e.to_string())?
        .unwrap_or_default();

    let html = tokio::time::timeout(Duration::from_millis(timeout_ms), page.content())
        .await
        .map_err(|_| "timeout reading HTML".to_string())?
        .map_err(|e| e.to_string())?;

    if html.trim().is_empty() {
        return Err("empty HTML".to_string());
    }

    let final_url = page.url().await.ok().flatten().unwrap_or_else(|| url.to_string());
    let _ = page.close().await;
    Ok((
        if title.is_empty() {
            final_url.clone()
        } else {
            title
        },
        html,
        final_url,
    ))
}

async fn root() -> Json<Blurb> {
    Json(Blurb {
        service: "headless-rust",
        version: env!("CARGO_PKG_VERSION"),
        endpoints: Endpoints {
            health: "GET /health",
            fetch: "POST /fetch { url, timeoutMs? }",
        },
    })
}

async fn health(State(state): State<AppState>) -> Result<Json<HealthOk>, (StatusCode, Json<HealthErr>)> {
    match ensure_browser(&state).await {
        Ok(()) => Ok(Json(HealthOk {
            ok: true,
            browser: "chromium",
        })),
        Err(error) => {
            error!(%error, "health failed");
            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(HealthErr { ok: false, error }),
            ))
        }
    }
}

async fn fetch(
    State(state): State<AppState>,
    Json(body): Json<FetchBody>,
) -> Result<Json<FetchOk>, (StatusCode, Json<ErrBody>)> {
    let url = body.url.trim().to_string();
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrBody {
                ok: false,
                error: "Body must include url starting with http(s)://".into(),
            }),
        ));
    }
    let timeout_ms = clamp_timeout(body.timeout_ms);
    let started = Instant::now();
    match fetch_page(&state, &url, timeout_ms).await {
        Ok((title, html, final_url)) => Ok(Json(FetchOk {
            ok: true,
            title,
            html,
            url: final_url,
            latency_ms: started.elapsed().as_millis() as u64,
        })),
        Err(error) => {
            error!(%error, %url, "fetch failed");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrBody { ok: false, error }),
            ))
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "headless_rust=info,tower_http=info".into()),
        )
        .init();

    let port: u16 = std::env::var("RUST_FETCH_PORT")
        .or_else(|_| std::env::var("PORT"))
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let host = std::env::var("RUST_FETCH_HOST").unwrap_or_else(|_| "0.0.0.0".into());

    let state = AppState {
        browser: Arc::new(Mutex::new(None)),
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/fetch", post(fetch))
        .with_state(state);

    let addr = format!("{host}:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    info!("listening on http://{addr} (GET /health, POST /fetch)");
    axum::serve(listener, app).await.expect("server");
}
