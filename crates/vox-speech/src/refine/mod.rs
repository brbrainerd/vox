//! Post-processing for transcripts (ITN, cleanup). Heavier rules land here over time.

pub mod llm_correction_prompt;
pub mod rules;

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::runtime_config::RefineTunables;
use crate::speaker_profile::SpeakerProfile;

/// Specialization domain for STT rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DomainMode {
    /// General speech domain (default).
    #[default]
    General,
    /// Rust/code-oriented domain.
    Code,
}

/// Correction strictness profile for transcript refinement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OratioCorrectionProfile {
    /// Apply only conservative deterministic corrections.
    Conservative,
    /// Balanced profile for command-oriented speech recognition.
    #[default]
    Balanced,
    /// Apply broader normalization and correction rules.
    Aggressive,
}

/// Runtime context for refinement and spelling correction.
#[derive(Debug, Clone, Default)]
pub struct CorrectionContext {
    /// Correction strictness profile.
    pub profile: OratioCorrectionProfile,
    /// Refinement domain for rules (e.g., General, Code).
    pub domain: DomainMode,
    /// Confidence / penalty tunables (defaults match historical hard-coded refine).
    pub refine_tunables: RefineTunables,
    /// Optional domain lexicon to prefer during replacements.
    pub domain_lexicon: HashSet<String>,
    /// Tokens that should never be rewritten.
    pub protected_tokens: HashSet<String>,
    /// Emit debug logs with input payloads and replacement details.
    pub debug_payload: bool,
    /// Speaker Profile dictates rules logic (e.g., dysarthric exceptions).
    pub speaker_profile: SpeakerProfile,
}

impl CorrectionContext {
    /// Build context using resolved runtime refine tunables.
    #[must_use]
    pub fn from_runtime(
        rt: &crate::runtime_config::OratioRuntimeConfig,
        profile: OratioCorrectionProfile,
        debug_payload: bool,
    ) -> Self {
        Self {
            profile,
            domain: rt.asr.domain_mode,
            refine_tunables: rt.refine,
            debug_payload,
            speaker_profile: SpeakerProfile::Standard,
            ..Default::default()
        }
    }
}

/// Trace item for one correction decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CorrectionTrace {
    /// Rule identifier that produced this decision.
    pub rule: String,
    /// Text before the correction.
    pub before: String,
    /// Text after the correction.
    pub after: String,
    /// Why this correction fired.
    pub reason: String,
}

/// Output of transcript refinement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefineOutput {
    /// Final refined text.
    pub text: String,
    /// Estimated confidence score for the refined output (0.0..=1.0).
    pub confidence: f32,
    /// Detailed correction decisions.
    pub trace: Vec<CorrectionTrace>,
}

/// Run deterministic refinement and spelling correction.
#[must_use]
pub fn refine_transcript(raw: &str, ctx: &CorrectionContext) -> RefineOutput {
    rules::refine_transcript(raw, ctx)
}
