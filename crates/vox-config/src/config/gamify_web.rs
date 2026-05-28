//! Gamification and web run-mode enums for `VoxConfig`.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// Gamification UX / reward tuning (consumed by `vox-gamify`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum GamifyMode {
    #[default]
    Balanced,
    Serious,
    Learning,
}

impl GamifyMode {
    /// Lower-case slug persisted in `Vox.toml` / `~/.vox/config.toml` (`[vox].gamify_mode`).
    #[must_use]
    pub fn as_config_str(self) -> &'static str {
        match self {
            Self::Balanced => "balanced",
            Self::Serious => "serious",
            Self::Learning => "learning",
        }
    }

    /// Multiplier applied to XP / crystal grants where ludus uses mode-aware scaling.
    #[must_use]
    pub fn reward_multiplier(self) -> f64 {
        match self {
            Self::Balanced => 1.0,
            Self::Serious => 0.5,
            Self::Learning => 1.25,
        }
    }

    /// Relative hint frequency in `[0.0, 1.0]` for coaching surfaces.
    #[must_use]
    pub fn hint_frequency(self) -> f64 {
        match self {
            Self::Balanced => 0.5,
            Self::Serious => 0.0,
            Self::Learning => 1.0,
        }
    }

    /// Whether level-up / quest-complete style overlays should render.
    #[must_use]
    pub fn show_overlays(self) -> bool {
        !matches!(self, Self::Serious)
    }
}

/// Build target for `vox build` / `vox dev`: controls which codegen paths are enabled.
///
/// Override order (highest to lowest): CLI `--target` flag > `VOX_BUILD_TARGET` env var >
/// `[build] target` in `Vox.toml` > implicit default (`Fullstack`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BuildTarget {
    /// Emit TypeScript/React frontend files **and** Axum Rust backend (default mode).
    #[default]
    Fullstack,
    /// Emit only the Axum Rust backend; skip all TypeScript codegen and Vite scaffolding.
    Server,
    /// Emit a zero-runtime TypeScript SDK package only; skip Rust codegen.
    Client,
    /// Emit React Native + Expo (Expo Router) TypeScript for mobile, plus the standard
    /// Vox-app-contract and OpenAPI artifacts. The Rust runtime is not emitted at this
    /// stage; mobile apps that need on-device Rust pull it in via the uniffi-bridged
    /// `@vox/runtime-rn` package (see `docs/src/architecture/mobile-rn-expo-implementation-spec-2026.md`).
    Mobile,
}

impl FromStr for BuildTarget {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "fullstack" => Ok(Self::Fullstack),
            "server" => Ok(Self::Server),
            "client" => Ok(Self::Client),
            "mobile" | "rn" | "expo" => Ok(Self::Mobile),
            _ => Err(()),
        }
    }
}

impl BuildTarget {
    /// Lower-case slug persisted in `Vox.toml` `[build] target`.
    #[must_use]
    pub fn as_config_str(self) -> &'static str {
        match self {
            Self::Fullstack => "fullstack",
            Self::Server => "server",
            Self::Client => "client",
            Self::Mobile => "mobile",
        }
    }
}

/// How `vox run` (and compilerd) choose the **script** lane vs **web app** lane when the CLI mode is `auto`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum WebRunMode {
    #[default]
    Auto,
    App,
    Script,
}

impl WebRunMode {
    /// Lower-case slug in `Vox.toml` `[web] run_mode` and `VOX_WEB_RUN_MODE`.
    #[must_use]
    pub fn as_config_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::App => "app",
            Self::Script => "script",
        }
    }
}
