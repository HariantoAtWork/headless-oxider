//! Fingerprint profile selection (consistent, not random noise).

use chaser_oxide::{ChaserProfile, Gpu};
use rand::seq::SliceRandom;
use rand::thread_rng;
use tracing::info;

#[derive(Clone, Debug)]
pub enum ProfileKind {
    Native,
    Linux,
    Windows,
    Macos,
}

impl ProfileKind {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "linux" => Self::Linux,
            "windows" | "win" => Self::Windows,
            "macos" | "mac" | "macos_arm" | "macos-arm" => Self::Macos,
            "rotate" => {
                // Pick one consistent preset for this process lifetime via caller.
                Self::Linux
            }
            _ => Self::Native,
        }
    }

    pub fn id(&self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Linux => "linux",
            Self::Windows => "windows",
            Self::Macos => "macos",
        }
    }
}

/// Resolve profile kind from config string; `rotate` picks once at startup.
pub fn resolve_profile_kind(config_value: &str) -> ProfileKind {
    let lower = config_value.trim().to_ascii_lowercase();
    if lower == "rotate" {
        let choices = [
            ProfileKind::Linux,
            ProfileKind::Windows,
            ProfileKind::Macos,
        ];
        let pick = choices
            .choose(&mut thread_rng())
            .cloned()
            .unwrap_or(ProfileKind::Linux);
        info!(profile = pick.id(), "rotated fingerprint profile for process");
        return pick;
    }
    ProfileKind::parse(&lower)
}

pub fn build_profile(kind: &ProfileKind) -> ChaserProfile {
    match kind {
        ProfileKind::Native => ChaserProfile::native().build(),
        ProfileKind::Linux => ChaserProfile::linux()
            .chrome_version(131)
            .gpu(Gpu::IntelIrisXe)
            .memory_gb(16)
            .cpu_cores(8)
            .locale("en-GB")
            .timezone("Europe/Amsterdam")
            .screen(1920, 1080)
            .build(),
        ProfileKind::Windows => ChaserProfile::windows()
            .chrome_version(131)
            .gpu(Gpu::NvidiaRTX3080)
            .memory_gb(16)
            .cpu_cores(8)
            .locale("en-US")
            .timezone("America/New_York")
            .screen(1920, 1080)
            .build(),
        ProfileKind::Macos => ChaserProfile::macos_arm()
            .chrome_version(131)
            .gpu(Gpu::AppleM2Max)
            .memory_gb(16)
            .cpu_cores(8)
            .locale("en-US")
            .timezone("America/Los_Angeles")
            .screen(2560, 1600)
            .build(),
    }
}
