//! Persistent share session state (`~/.config/vox/share-state.json`).
//!
//! Override for testing: `VOX_SHARE_STATE_PATH` env var.

use crate::error::{ShareError, ShareResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Current version of the consent text. Bump when the warning changes substantially.
pub const CONSENT_TEXT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ShareState {
    /// Has the user accepted the Cloudflare ToS / public-exposure notice?
    pub cloudflare_consent_v1: bool,
    /// Version of the consent text the user accepted.
    #[serde(default)]
    pub consent_text_version: u32,
}

impl ShareState {
    pub fn load() -> ShareResult<Self> {
        let path = state_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)?;
        serde_json::from_str(&text)
            .map_err(|e| ShareError::Config(format!("parse share state: {}", e)))
    }

    pub fn save(&self) -> ShareResult<()> {
        let path = state_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self)
            .map_err(|e| ShareError::Config(format!("serialize share state: {}", e)))?;
        std::fs::write(&path, text)?;
        Ok(())
    }
}

fn state_path() -> ShareResult<PathBuf> {
    if let Ok(p) = std::env::var("VOX_SHARE_STATE_PATH") {
        return Ok(PathBuf::from(p));
    }
    let base = dirs::config_dir()
        .ok_or_else(|| ShareError::Config("could not determine config directory".into()))?;
    Ok(base.join("vox").join("share-state.json"))
}
