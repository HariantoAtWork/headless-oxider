//! PDF render via CDP Page.printToPDF — same HTTP contract as headless-playwright.

use std::time::Duration;

use chaser_oxide::cdp::browser_protocol::network::{
    BlockPattern, Headers, SetBlockedUrLsParams, SetExtraHttpHeadersParams,
};
use chaser_oxide::cdp::browser_protocol::page::PrintToPdfParams;
use chaser_oxide::{ChaserPage, ChaserProfile, Page};
use tracing::debug;

use crate::blocklist::Blocklist;
use crate::stealth::{build_profile, ProfileKind};

const DEFAULT_WAIT_SELECTOR: &str = "[data-cv-print-ready=\"true\"]";

#[derive(Debug, Clone, Default)]
pub struct PdfOptions {
    pub format: Option<String>,
    pub scale: Option<f64>,
    pub print_background: Option<bool>,
    pub margin_top: Option<String>,
    pub margin_right: Option<String>,
    pub margin_bottom: Option<String>,
    pub margin_left: Option<String>,
}

pub struct PdfResult {
    pub bytes: Vec<u8>,
}

/// Navigate, wait for selector, print to PDF. Closes the page.
pub async fn pdf_with_page(
    page: Page,
    blocklist: &Blocklist,
    profile_kind: &ProfileKind,
    stealth: bool,
    url: &str,
    timeout_ms: u64,
    cookie: Option<&str>,
    wait_for_selector: Option<&str>,
    pdf_opts: &PdfOptions,
) -> Result<PdfResult, String> {
    let chaser = ChaserPage::new(page);

    if stealth {
        apply_stealth(&chaser, profile_kind).await?;
        apply_blocklist_cdp(&chaser, blocklist).await;
    }

    if let Some(cookie) = cookie.map(str::trim).filter(|s| !s.is_empty()) {
        let mut map = std::collections::HashMap::new();
        map.insert("cookie".to_string(), cookie.to_string());
        let headers = Headers::new(serde_json::to_value(map).map_err(|e| e.to_string())?);
        let params = SetExtraHttpHeadersParams::new(headers);
        if let Err(err) = chaser.raw_page().execute(params).await {
            debug!(%err, "Network.setExtraHTTPHeaders failed");
        }
    }

    tokio::time::timeout(Duration::from_millis(timeout_ms), chaser.goto(url))
        .await
        .map_err(|_| "timeout navigating".to_string())?
        .map_err(|e| e.to_string())?;

    let selector = wait_for_selector
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_WAIT_SELECTOR);

    wait_for_selector_poll(&chaser, selector, timeout_ms.min(30_000)).await?;
    tokio::time::sleep(Duration::from_millis(250)).await;

    let scale = pdf_opts.scale.unwrap_or(0.79);
    let print_background = pdf_opts.print_background.unwrap_or(true);
    let (paper_width, paper_height) = paper_inches(pdf_opts.format.as_deref());

    let params = PrintToPdfParams::builder()
        .print_background(print_background)
        .scale(scale)
        .paper_width(paper_width)
        .paper_height(paper_height)
        .margin_top(parse_margin_inches(pdf_opts.margin_top.as_deref()))
        .margin_right(parse_margin_inches(pdf_opts.margin_right.as_deref()))
        .margin_bottom(parse_margin_inches(pdf_opts.margin_bottom.as_deref()))
        .margin_left(parse_margin_inches(pdf_opts.margin_left.as_deref()))
        .build();

    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        chaser.raw_page().execute(params),
    )
    .await
    .map_err(|_| "timeout printing PDF".to_string())?
    .map_err(|e| e.to_string())?;

    let bytes = pdf_bytes_from_cdp_data(&result.data)?;
    if bytes.is_empty() || !bytes.starts_with(b"%PDF") {
        return Err("PrintToPDF returned empty or non-PDF data".to_string());
    }

    let _ = chaser.raw_page().clone().close().await;
    Ok(PdfResult { bytes })
}

fn pdf_bytes_from_cdp_data(data: &impl AsRef<str>) -> Result<Vec<u8>, String> {
    let s = data.as_ref().trim().trim_matches('"');
    if s.as_bytes().starts_with(b"%PDF") {
        return Ok(s.as_bytes().to_vec());
    }
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
        .or_else(|_| {
            base64::Engine::decode(&base64::engine::general_purpose::STANDARD_NO_PAD, s)
        })
        .map_err(|e| format!("PDF base64 decode failed: {e}"))
}

fn paper_inches(format: Option<&str>) -> (f64, f64) {
    match format.map(|s| s.to_ascii_lowercase()).as_deref() {
        Some("letter") => (8.5, 11.0),
        _ => (8.27, 11.69),
    }
}

fn parse_margin_inches(value: Option<&str>) -> f64 {
    let Some(raw) = value.map(str::trim).filter(|s| !s.is_empty()) else {
        return 0.0;
    };
    if let Some(mm) = raw.strip_suffix("mm") {
        return mm.parse::<f64>().unwrap_or(0.0) / 25.4;
    }
    if let Some(cm) = raw.strip_suffix("cm") {
        return cm.parse::<f64>().unwrap_or(0.0) / 2.54;
    }
    if let Some(inch) = raw.strip_suffix("in") {
        return inch.parse::<f64>().unwrap_or(0.0);
    }
    raw.parse::<f64>().unwrap_or(0.0)
}

async fn wait_for_selector_poll(
    chaser: &ChaserPage,
    selector: &str,
    timeout_ms: u64,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + Duration::from_millis(timeout_ms);
    let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
    let expr = format!("!!document.querySelector('{escaped}')");
    loop {
        let eval = tokio::time::timeout(Duration::from_millis(5_000), chaser.evaluate(&expr)).await;
        if let Ok(Ok(Some(value))) = eval {
            let ok = value.as_bool().unwrap_or(false)
                || value.as_str().map(|s| s == "true").unwrap_or(false);
            if ok {
                return Ok(());
            }
        }
        if std::time::Instant::now() >= deadline {
            return Err(format!("timeout waiting for selector {selector}"));
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

async fn apply_stealth(chaser: &ChaserPage, kind: &ProfileKind) -> Result<(), String> {
    match kind {
        ProfileKind::Native => chaser
            .apply_native_profile()
            .await
            .map_err(|e| e.to_string())?,
        other => {
            let profile: ChaserProfile = build_profile(other);
            chaser
                .apply_profile(&profile)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

async fn apply_blocklist_cdp(chaser: &ChaserPage, blocklist: &Blocklist) {
    if blocklist.len() == 0 {
        return;
    }
    let patterns: Vec<BlockPattern> = blocklist
        .patterns()
        .iter()
        .take(80)
        .map(|p| BlockPattern::new(to_url_pattern(p), true))
        .collect();
    let params = SetBlockedUrLsParams::builder()
        .url_patterns(patterns)
        .build();
    let _ = chaser.raw_page().execute(params).await;
}

fn to_url_pattern(raw: &str) -> String {
    let p = raw.trim();
    if p.contains("://") {
        return p.to_string();
    }
    let stripped = p.trim_matches('*');
    if stripped.is_empty() {
        return "*://*/*".to_string();
    }
    format!("*://*{stripped}*/*")
}
