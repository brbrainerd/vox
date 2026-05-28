//! Optional per-deployment overrides merged onto [`super::policy_types::ConfidencePolicy`].

use serde::{Deserialize, Serialize};

/// Optional per-deployment overrides (TOML / env) merged onto [`super::policy_types::ConfidencePolicy::workspace_default`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConfidencePolicyOverride {
    /// Overrides [`super::policy_types::ConfidencePolicy::min_review_finding_confidence`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_review_finding_confidence: Option<u8>,
    /// Overrides [`super::policy_types::ConfidencePolicy::min_prompt_report_confidence`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_prompt_report_confidence: Option<u8>,
    /// Overrides [`super::policy_types::ConfidencePolicy::abstain_threshold`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abstain_threshold: Option<f64>,
    /// Overrides [`super::policy_types::ConfidencePolicy::ask_for_help_threshold`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ask_for_help_threshold: Option<f64>,
    /// Overrides [`super::policy_types::ConfidencePolicy::max_contradiction_ratio_for_answer`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_contradiction_ratio_for_answer: Option<f64>,
    /// Overrides [`super::policy_types::ConfidencePolicy::min_persist_confidence`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_persist_confidence: Option<f64>,
    /// Overrides [`super::policy_types::ConfidencePolicy::min_training_pair_confidence`] when set.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_training_pair_confidence: Option<f64>,
}
