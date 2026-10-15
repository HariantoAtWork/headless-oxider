//! Headless Chromium fetch service — stealth by default (chaser-oxide).
//! GET /health · POST /fetch · POST /pdf · GET /

mod blocklist;
mod config;
mod fetch;
mod pdf;
mod stealth;
mod worker;

use std::time::Instant;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use tracing::{error, info};

use blocklist::Blocklist;
use config::Config;
use pdf::PdfOptions;
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
    pdf: &'static str,
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

#[derive(Deserialize, Default)]
struct PdfMarginBody {
    top: Option<String>,
    right: Option<String>,
    bottom: Option<String>,
    left: Option<String>,
}

#[derive(Deserialize, Default)]
struct PdfOptsBody {
    format: Option<String>,
    scale: Option<f64>,
    #[serde(default, rename = "printBackground")]
    print_background: Option<bool>,
    margin: Option<PdfMarginBody>,
}

#[derive(Deserialize)]
struct PdfBody {
    url: String,
    #[serde(default, rename = "timeoutMs")]
    timeout_ms: Option<u64>,
    #[serde(default, rename = "sessionKey")]
    session_key: Option<String>,
    #[serde(default)]
    proxy: Option<String>,
    #[serde(default)]
    stealth: Option<bool>,
    #[serde(default)]
    cookie: Option<String>,
    #[serde(default, rename = "waitForSelector")]
    wait_for_selector: Option<String>,
    #[serde(default)]
    pdf: Option<PdfOptsBody>,
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
            pdf: "POST /pdf { url, timeoutMs?, cookie?, waitForSelector?, pdf?, sessionKey?, proxy?, stealth? }",
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

async fn pdf_handler(
    State(state): State<AppState>,
    Json(body): Json<PdfBody>,
) -> Result<Response, (StatusCode, Json<ErrBody>)> {
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
    let cookie = body
        .cookie
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let wait_for_selector = body
        .wait_for_selector
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let margin = body.pdf.as_ref().and_then(|p| p.margin.as_ref());
    let pdf_opts = PdfOptions {
        format: body.pdf.as_ref().and_then(|p| p.format.clone()),
        scale: body.pdf.as_ref().and_then(|p| p.scale),
        print_background: body.pdf.as_ref().and_then(|p| p.print_background),
        margin_top: margin.and_then(|m| m.top.clone()),
        margin_right: margin.and_then(|m| m.right.clone()),
        margin_bottom: margin.and_then(|m| m.bottom.clone()),
        margin_left: margin.and_then(|m| m.left.clone()),
    };

    match state
        .browser
        .pdf(
            url.clone(),
            timeout_ms,
            session,
            proxy,
            stealth,
            cookie,
            wait_for_selector,
            pdf_opts,
        )
        .await
    {
        Ok(result) => {
            let response = Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "application/pdf")
                .header(header::CONTENT_LENGTH, result.bytes.len())
                .body(Body::from(result.bytes))
                .map_err(|e| {
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrBody {
                            ok: false,
                            error: e.to_string(),
                        }),
                    )
                })?;
            Ok(response)
        }
        Err(error) => {
            error!(%error, %url, "pdf failed");
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
        .route("/pdf", post(pdf_handler))
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));
    info!("listening on http://{addr} (GET /health, POST /fetch, POST /pdf)");
    axum::serve(listener, app).await.expect("server");
}
