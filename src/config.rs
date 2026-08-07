//! Runtime configuration from environment.

use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct Config {
    pub host: String,
    pub port: u16,
    /// Stealth on by default; set STEALTH=0 to disable.
    pub stealth: bool,
    /// Headful window (use with Xvfb in Docker).
    pub headful: bool,
    /// Global proxy, e.g. http://user:pass@host:port
    pub proxy_url: Option<String>,
    /// native | linux | windows | macos | rotate
    pub fingerprint_profile: String,
    pub chrome_path: Option<PathBuf>,
    pub sessions_dir: PathBuf,
    pub blocklist_path: PathBuf,
}

impl Config {
    pub fn from_env() -> Self {
        let stealth = !matches!(
            std::env::var("STEALTH")
                .unwrap_or_else(|_| "1".into())
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "0" | "false" | "off" | "no"
        );
        let headful = matches!(
            std::env::var("HEADFUL")
                .unwrap_or_else(|_| "0".into())
                .trim()
                .to_ascii_lowercase()
                .as_str(),
            "1" | "true" | "on" | "yes"
        );
        let proxy_url = std::env::var("PROXY_URL")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let fingerprint_profile = std::env::var("FINGERPRINT_PROFILE")
            .unwrap_or_else(|_| "native".into())
            .trim()
            .to_ascii_lowercase();
        let chrome_path = std::env::var("CHROME_PATH")
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(detect_chrome);
        let sessions_dir = PathBuf::from(
            std::env::var("SESSIONS_DIR")
                .unwrap_or_else(|_| "/var/lib/headless-rust/sessions".into()),
        );
        let blocklist_path = PathBuf::from(
            std::env::var("BLOCKLIST_PATH")
                .unwrap_or_else(|_| "/etc/headless-rust/blocklist.txt".into()),
        );
        let port: u16 = std::env::var("RUST_FETCH_PORT")
            .or_else(|_| std::env::var("PORT"))
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(9381);
        let host = std::env::var("RUST_FETCH_HOST").unwrap_or_else(|_| "0.0.0.0".into());

        Self {
            host,
            port,
            stealth,
            headful,
            proxy_url,
            fingerprint_profile,
            chrome_path,
            sessions_dir,
            blocklist_path,
        }
    }
}

fn detect_chrome() -> Option<PathBuf> {
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
