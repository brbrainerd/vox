//! Dysarthric and typical speaker profiles for STT routing and refinement.

use serde::{Deserialize, Serialize};

/// Custom speaker profile routing, enabling integration with Voiceitt or dysarthric tuning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerProfile {
    /// Standard typical speech backend.
    #[default]
    Standard,
    /// Dysarthric speaker profile identifier to route through customized models.
    Dysarthric(String),
}
