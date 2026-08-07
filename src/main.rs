//! Headless Chromium fetch service — stealth by default (chaser-oxide).
//! GET /health · POST /fetch · GET /

mod blocklist;
mod config;
mod fetch;
mod stealth;
mod worker;

use std::time::Instant;

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use blocklist::Blocklist;
use config::Config;
use stealth::{resolve_profile_kind, ProfileKind};
use worker::BrowserHandle;

const DEFAULT_TIMEOUT_MS: u64 = 45_000;
const MAX_TIMEOUT_MS: u64 = 120_000;

#[derive(Clone)]
struct AppState {
    browser: BrowserHandle,
    config: Config,
    profile_kind: ProfileKind,
    blocklist_len: usize,
}

#[derive(Serialize)]
struct Blurb {
    service: &'static str,
    version: &'static str,
    stealth_default: bool,
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
    stealth: bool,
    profile: String,
    blocklist: usize,
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
    #[serde(default, rename = "sessionKey")]
    session_key: Option<String>,
    #[serde(default)]
    proxy: Option<String>,
    #[serde(default)]
    stealth: Option<bool>,
}

#[derive(Serialize)]
struct FetchOk {
    ok: bool,
    title: String,
    html: String,
    url: String,
    #[serde(rename = "latencyMs")]
    latency_ms: u64,
    stealth: bool,
    profile: String,
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

async fn root(State(state): State<AppState>) -> Json<Blurb> {
    Json(Blurb {
        service: "headless-oxider",
        version: env!("CARGO_PKG_VERSION"),
        stealth_default: state.config.stealth,
        endpoints: Endpoints {
            health: "GET /health",
            fetch: "POST /fetch { url, timeoutMs?, sessionKey?, proxy?, stealth? }",
        },
    })
}

async fn health(
    State(state): State<AppState>,
) -> Result<Json<HealthOk>, (StatusCode, Json<HealthErr>)> {
    match state.browser.warm().await {
        Ok(()) => Ok(Json(HealthOk {
            ok: true,
            browser: "chromium",
            stealth: state.config.stealth,
            profile: state.profile_kind.id().to_string(),
            blocklist: state.blocklist_len,
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

async fn fetch_handler(
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
    let stealth = body.stealth.unwrap_or(state.config.stealth);
    let session = body
        .session_key
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let proxy = body
        .proxy
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let started = Instant::now();
    match state
        .browser
        .fetch(url.clone(), timeout_ms, session, proxy, stealth)
        .await
    {
        Ok(result) => Ok(Json(FetchOk {
            ok: true,
            title: result.title,
            html: result.html,
            url: result.url,
            latency_ms: started.elapsed().as_millis() as u64,
            stealth: result.stealth,
            profile: result.profile,
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

    let config = Config::from_env();
    let profile_kind = resolve_profile_kind(&config.fingerprint_profile);
    let blocklist = Blocklist::load(&config.blocklist_path);
    let blocklist_len = blocklist.len();
    let _ = std::fs::create_dir_all(&config.sessions_dir);

    info!(
        stealth = config.stealth,
        headful = config.headful,
        profile = profile_kind.id(),
        blocklist = blocklist_len,
        "headless-oxider starting"
    );

    let browser = BrowserHandle::start(config.clone(), blocklist, profile_kind.clone());

    let state = AppState {
        browser,
        config: config.clone(),
        profile_kind,
        blocklist_len,
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health))
        .route("/fetch", post(fetch_handler))
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    info!("listening on http://{addr} (GET /health, POST /fetch)");
    axum::serve(listener, app).await.expect("server");
}
