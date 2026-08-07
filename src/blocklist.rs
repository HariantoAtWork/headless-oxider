//! Tracker / analytics URL blocklist (Obscura-class).

use std::fs;
use std::path::Path;
use std::sync::Arc;

use tracing::{info, warn};

/// Default curated patterns (Chrome Network.setBlockedURLs style globs).
const BUILTIN: &[&str] = &[
    "*google-analytics.com*",
    "*googletagmanager.com*",
    "*googlesyndication.com*",
    "*doubleclick.net*",
    "*googleadservices.com*",
    "*facebook.net/*fbevents*",
    "*connect.facebook.net*",
    "*analytics.tiktok.com*",
    "*hotjar.com*",
    "*fullstory.com*",
    "*segment.io*",
    "*segment.com*",
    "*mixpanel.com*",
    "*amplitude.com*",
    "*sentry.io*",
    "*newrelic.com*",
    "*nr-data.net*",
    "*clarity.ms*",
    "*mouseflow.com*",
    "*heap-api.com*",
    "*heapanalytics.com*",
    "*scorecardresearch.com*",
    "*quantserve.com*",
    "*adservice.google.*",
    "*pagead2.googlesyndication.com*",
    "*amazon-adsystem.com*",
    "*adnxs.com*",
    "*adsrvr.org*",
    "*taboola.com*",
    "*outbrain.com*",
    "*criteo.com*",
    "*criteo.net*",
    "*pubmatic.com*",
    "*openx.net*",
    "*rubiconproject.com*",
    "*moatads.com*",
    "*chartbeat.com*",
    "*parsely.com*",
    "*pingdom.net*",
    "*crazyegg.com*",
    "*optimizely.com*",
    "*crazyegg.com*",
    "*yandex.ru/metrika*",
    "*mc.yandex.ru*",
    "*bat.bing.com*",
    "*static.ads-twitter.com*",
    "*analytics.twitter.com*",
    "*linkedin.com/px*",
    "*snap.licdn.com*",
    "*pixel.linkedin.com*",
];

#[derive(Clone)]
pub struct Blocklist {
    patterns: Arc<Vec<String>>,
}

impl Blocklist {
    pub fn load(path: &Path) -> Self {
        let mut patterns: Vec<String> = BUILTIN.iter().map(|s| (*s).to_string()).collect();
        if path.is_file() {
            match fs::read_to_string(path) {
                Ok(text) => {
                    let mut extra = 0usize;
                    for line in text.lines() {
                        let line = line.trim();
                        if line.is_empty() || line.starts_with('#') {
                            continue;
                        }
                        patterns.push(line.to_string());
                        extra += 1;
                    }
                    info!(extra, path = %path.display(), "loaded blocklist file");
                }
                Err(err) => warn!(%err, path = %path.display(), "blocklist file unreadable"),
            }
        } else {
            info!(
                builtin = BUILTIN.len(),
                "using builtin tracker blocklist (no file at {})",
                path.display()
            );
        }
        patterns.sort();
        patterns.dedup();
        Self {
            patterns: Arc::new(patterns),
        }
    }

    pub fn patterns(&self) -> &[String] {
        &self.patterns
    }

    pub fn len(&self) -> usize {
        self.patterns.len()
    }
}
