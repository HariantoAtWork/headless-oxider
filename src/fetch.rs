//! Page fetch with stealth profile, blocklist, humanised settle.

use std::time::Duration;

use chaser_oxide::cdp::browser_protocol::network::{BlockPattern, SetBlockedUrLsParams};
use chaser_oxide::{ChaserPage, ChaserProfile, Page};
use rand::{thread_rng, Rng};
use tracing::debug;

use crate::blocklist::Blocklist;
use crate::stealth::{build_profile, ProfileKind};

pub struct FetchResult {
    pub title: String,
    pub html: String,
    pub url: String,
    pub profile: String,
    pub stealth: bool,
}

pub async fn fetch_with_page(
    page: Page,
    blocklist: &Blocklist,
    profile_kind: &ProfileKind,
    stealth: bool,
    url: &str,
    timeout_ms: u64,
) -> Result<FetchResult, String> {
    let chaser = ChaserPage::new(page);
    let profile_id = profile_kind.id().to_string();

    if stealth {
        apply_stealth(&chaser, profile_kind).await?;
        apply_blocklist_cdp(&chaser, blocklist).await;
    }

    tokio::time::timeout(Duration::from_millis(timeout_ms), chaser.goto(url))
        .await
        .map_err(|_| "timeout navigating".to_string())?
        .map_err(|e| e.to_string())?;

    if stealth {
        humanise(&chaser).await;
    }

    let title = title_of(&chaser, timeout_ms).await;
    let html = tokio::time::timeout(Duration::from_millis(timeout_ms), chaser.content())
        .await
        .map_err(|_| "timeout reading HTML".to_string())?
        .map_err(|e| e.to_string())?;

    if html.trim().is_empty() {
        return Err("empty HTML".to_string());
    }

    let final_url = chaser
        .url()
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| url.to_string());

    let _ = chaser.raw_page().clone().close().await;

    Ok(FetchResult {
        title: if title.is_empty() {
            final_url.clone()
        } else {
            title
        },
        html,
        url: final_url,
        profile: profile_id,
        stealth,
    })
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
    match chaser.raw_page().execute(params).await {
        Ok(_) => debug!(count = blocklist.len().min(80), "applied Network.setBlockedURLs"),
        Err(err) => debug!(%err, "Network.setBlockedURLs failed"),
    }
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

async fn humanise(chaser: &ChaserPage) {
    let mut rng = thread_rng();
    let x = rng.gen_range(120.0..900.0);
    let y = rng.gen_range(80.0..700.0);
    let _ = chaser.move_mouse_human(x, y).await;
    let scroll = rng.gen_range(80..420);
    let _ = chaser.scroll_human(scroll).await;
    let pause = rng.gen_range(250..700);
    tokio::time::sleep(Duration::from_millis(pause)).await;
}

async fn title_of(chaser: &ChaserPage, timeout_ms: u64) -> String {
    let eval = tokio::time::timeout(
        Duration::from_millis(timeout_ms.min(15_000)),
        chaser.evaluate("document.title"),
    )
    .await;
    match eval {
        Ok(Ok(Some(value))) => value
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| value.to_string().trim_matches('"').to_string()),
        _ => String::new(),
    }
}
