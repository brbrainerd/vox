//! Failure categories for speech-to-code observability (matches `contracts/speech-to-code`).

use serde::{Deserialize, Serialize};

/// High-level failure bucket for ASR → LLM → compiler pipelines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpeechFailureCategory {
    /// Mic, noise, segmentation, clipping.
    Acoustic,
    /// Word / identifier recognition, homophones.
    Lexical,
    /// Invalid Vox syntax after intent mapping.
    Syntactic,
    /// Type or HIR invariant failures; wrong intent.
    Semantic,
    /// Tool routing, timeouts, policy denials.
    Orchestration,
    /// Unclassified.
    Unknown,
}

impl SpeechFailureCategory {
    /// Stable string label for logs and KPI JSON.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Acoustic => "acoustic",
            Self::Lexical => "lexical",
            Self::Syntactic => "syntactic",
            Self::Semantic => "semantic",
            Self::Orchestration => "orchestration",
            Self::Unknown => "unknown",
        }
    }
}
