//! Wire format and identifiers for **`vox-tauri-stt`** — the speech-to-text plugin surface for
//! **desktop** Tauri 2 apps.
//!
//! With the **`tauri-plugin`** feature, `plugin::init` registers a Tauri 2 plugin whose id matches
//! [`PLUGIN_ID`] and whose `transcribe` command matches [`TRANSCRIBE_COMMAND`] (guest `invoke` string).
//! Without that feature, this crate is **serde-only** for contracts and tests.
//!
//! Tauri is desktop-only (ADR: scope-tauri-desktop-only); the former Android Kotlin / iOS Swift
//! sources were removed with that decision. Mobile transcription belongs to the React Native
//! target (`vox build --target=mobile`) via `@vox/runtime-rn`. See `README.md`.

use serde::{Deserialize, Serialize};

#[cfg(feature = "tauri-plugin")]
pub mod plugin;

/// Tauri plugin identifier (must match guest JS `plugin:vox-stt|…`).
pub const PLUGIN_ID: &str = "vox-stt";

/// Invoke command name registered on the plugin.
pub const TRANSCRIBE_COMMAND: &str = "transcribe";

/// Guest ↔ Rust contract: same shape as the legacy Capacitor plugin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TranscribeResult {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn transcribe_result_json_contract() {
        let t = TranscribeResult {
            text: "hello".into(),
            confidence: Some(0.91),
        };
        let v = serde_json::to_value(&t).unwrap();
        assert_eq!(v["text"], "hello");
        assert_eq!(v["confidence"], serde_json::json!(0.91));
    }

    #[test]
    fn constants_are_stable_for_guest_js() {
        assert_eq!(PLUGIN_ID, "vox-stt");
        assert_eq!(TRANSCRIBE_COMMAND, "transcribe");
    }

    #[test]
    fn guest_facade_uses_stable_plugin_invoke_string() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let guest = std::fs::read_to_string(root.join("guest-js/index.ts")).expect("read guest-js");
        let expected = format!("plugin:{}|{}", PLUGIN_ID.trim(), TRANSCRIBE_COMMAND.trim());
        assert!(
            guest.contains(&format!("invoke<TranscribeResult>(\"{expected}\"")),
            "guest-js must invoke `{expected}`"
        );
    }
}
