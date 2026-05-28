//! Model-autonomic system (L1/L2/L3) — discovery + classification + promotion.
//!
//! Council-ratified 2026-05-15. SSOT:
//! [`docs/src/architecture/model-autonomic-system-2026.md`](../../../../docs/src/architecture/model-autonomic-system-2026.md).
//!
//! This module is the runtime side of the autonomic-system contract; it
//! provides the data types and entry points the CLI (`vox models discover`,
//! `vox models classify`, `vox models shadow`, `vox models council-report`)
//! and the nightly cron job consume.
//!
//! Each layer emits a corresponding telemetry event:
//!
//! | Layer | Event                       | When emitted                              |
//! |-------|-----------------------------|-------------------------------------------|
//! | L0    | [`SelectionDecisionEvent`]  | every `select()` call (see [`super::select`]) |
//! | L1    | [`DiscoveryEvent`]          | new model id seen in upstream catalog     |
//! | L2    | [`ClassificationEvent`]     | classifier LLM emits tier+strengths       |
//! | L2/L3 | [`ConfidencePromotionEvent`]| confidence-state boundary crossed         |
//!
//! [`SelectionDecisionEvent`]: vox_telemetry::SelectionDecisionEvent
//! [`DiscoveryEvent`]: vox_telemetry::DiscoveryEvent
//! [`ClassificationEvent`]: vox_telemetry::ClassificationEvent
//! [`ConfidencePromotionEvent`]: vox_telemetry::ConfidencePromotionEvent

use crate::models::{ModelRegistry, ModelTier, StrengthTag};
use std::collections::HashSet;
use vox_telemetry::{
    ClassificationEvent, ConfidencePromotionEvent, DiscoveryEvent, TelemetryEvent,
};

// ─── Confidence state machine ──────────────────────────────────────────────

/// Lifecycle of a model id inside the registry.
///
/// `Provisional` (discovered, classifier-tagged, no scoreboard data) →
/// `Shadowed` (running on eval panel, not yet routed to production) →
/// `Confirmed` (scoreboard passes thresholds, eligible for production
/// routing). `Deprecated` is a sink state for council-retired models.
///
/// The current registry implicitly treats every loaded model as `Confirmed`;
/// this enum is the explicit state that the autonomic system layers on top.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelConfidence {
    Provisional,
    Shadowed,
    Confirmed,
    Deprecated,
}

impl ModelConfidence {
    /// Snake-case wire-format string matching telemetry contracts.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisional => "provisional",
            Self::Shadowed => "shadowed",
            Self::Confirmed => "confirmed",
            Self::Deprecated => "deprecated",
        }
    }

    /// True iff a model in this state should be eligible for production
    /// routing by `select()`. Provisional/Shadowed are *not* eligible.
    #[must_use]
    pub fn eligible_for_routing(self) -> bool {
        matches!(self, Self::Confirmed)
    }
}

// ─── L1: Discovery ─────────────────────────────────────────────────────────

/// Source of a discovery event. Mirrors `DiscoveryEvent.source` wire format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiscoverySource {
    OpenRouter,
    LiteLlm,
    AnthropicDirect,
    PopuliMesh,
}

impl DiscoverySource {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::LiteLlm => "litellm",
            Self::AnthropicDirect => "anthropic_direct",
            Self::PopuliMesh => "populi_mesh",
        }
    }
}

/// Diff a new set of model ids from an upstream catalog against the prior
/// snapshot. Emits a [`DiscoveryEvent`] for each new id and returns the new ids.
///
/// Caller is responsible for persisting `new_ids` (e.g. to `vox-db`) so the
/// next refresh cycle has an updated baseline.
pub fn diff_and_emit_discovery(
    source: DiscoverySource,
    prior: &HashSet<String>,
    fresh: impl IntoIterator<Item = DiscoveredModel>,
) -> Vec<String> {
    let mut new_ids = Vec::new();
    let pins = vox_config::load_model_pins_config().unwrap_or_default();
    let retired: HashSet<&str> = pins.retired_ids.iter().map(String::as_str).collect();
    for m in fresh {
        if prior.contains(&m.id) {
            continue;
        }
        if retired.contains(m.id.as_str()) {
            // Council-retired ids must not trigger discovery; otherwise an
            // OpenRouter alias resurrection would re-introduce them silently.
            continue;
        }
        let event = DiscoveryEvent {
            source: source.as_str().to_string(),
            model_id: m.id.clone(),
            description: m.description,
            max_context_tokens: m.max_context_tokens,
        };
        vox_telemetry::record_event!(&TelemetryEvent::ModelDiscovery(event));
        new_ids.push(m.id);
    }
    new_ids
}

/// Minimal upstream-catalog row shape the L1 discovery diff consumes. Each
/// upstream adapter (OpenRouter, LiteLLM, AnthropicDirect, Mesh) lowers its
/// native rows into this normalized shape before calling [`diff_and_emit_discovery`].
#[derive(Debug, Clone)]
pub struct DiscoveredModel {
    pub id: String,
    pub description: Option<String>,
    pub max_context_tokens: Option<u32>,
}

// ─── L2: Classification ────────────────────────────────────────────────────

/// Structured output the classifier LLM returns. JSON-schema-pinned so the
/// classifier prompt can use structured-output mode (Anthropic / Gemini /
/// OpenAI all support this in 2026-Q2).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct ClassificationJudgement {
    pub tier: ModelTier,
    pub strengths: Vec<StrengthTag>,
    /// 0.0–1.0 — classifier's self-reported confidence.
    pub confidence: f32,
    /// Free-form rationale (max ~200 words); kept for council-report rendering.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rationale: Option<String>,
}

/// Build the classifier prompt for a single model. The prompt is intentionally
/// LLM-agnostic and emits structured-output JSON matching
/// [`ClassificationJudgement`]. Returns the full prompt body the caller hands
/// to the classifier provider.
///
/// The caller is responsible for the actual HTTP call — this module stays
/// adapter-free so it can be tested without network.
#[must_use]
pub fn build_classifier_prompt(
    target_id: &str,
    description: Option<&str>,
    supported_parameters: &[String],
    sample_input_cost_per_1k: Option<f64>,
) -> String {
    let mut s = String::new();
    s.push_str("You are the Vox model-autonomic classifier. Given the model below, ");
    s.push_str("emit a single JSON object matching the ClassificationJudgement schema.\n\n");
    s.push_str("Required JSON keys:\n");
    s.push_str("  tier        : one of [Unknown, Local, Light, Pro, Elite]\n");
    s.push_str("  strengths   : array of {codegen, debugging, logic, research, parsing, ");
    s.push_str("review, planning, inter_agent, visus, security, vision, ui_codegen, frontend, ");
    s.push_str("generalist, long_context}\n");
    s.push_str("  confidence  : float 0.0–1.0 (your self-reported confidence)\n");
    s.push_str("  rationale   : <= 200 word explanation\n\n");
    s.push_str("Tier guidance:\n");
    s.push_str("  Local  : runs on consumer hardware (<=32B params) or labeled `mesh/`\n");
    s.push_str("  Light  : cheap cloud frontier (Haiku/Flash-Lite/Mini tier)\n");
    s.push_str("  Pro    : mid-frontier (Sonnet/Flash/non-pro GPT)\n");
    s.push_str("  Elite  : top frontier (Opus/Pro/GPT-5-Pro tier; >= $5/MTok input)\n\n");
    s.push_str(&format!("Model id: {target_id}\n"));
    if let Some(d) = description {
        s.push_str(&format!("Upstream description: {d}\n"));
    }
    if !supported_parameters.is_empty() {
        s.push_str("Supported parameters: ");
        s.push_str(&supported_parameters.join(", "));
        s.push('\n');
    }
    if let Some(cost) = sample_input_cost_per_1k {
        s.push_str(&format!("Sample input cost (USD per 1k tokens): {cost:.4}\n"));
    }
    s.push_str("\nEmit only the JSON object. No prose preamble.\n");
    s
}

/// Persist a classifier judgement: emit telemetry + (when wired up) write to
/// `vox-db model_classification` table.
///
/// **Status:** telemetry-only in this scaffold. The DB write happens in a
/// follow-up phase once the schema lands in `vox-db`.
pub fn record_classification(
    model_id: &str,
    classifier_model: &str,
    judgement: &ClassificationJudgement,
) {
    let event = ClassificationEvent {
        model_id: model_id.to_string(),
        classifier_model: classifier_model.to_string(),
        tier: format!("{:?}", judgement.tier).to_ascii_lowercase(),
        strengths: judgement
            .strengths
            .iter()
            .map(|s| format!("{s:?}").to_ascii_lowercase())
            .collect(),
        confidence: judgement.confidence,
    };
    vox_telemetry::record_event!(&TelemetryEvent::ModelClassification(event));
}

// ─── L2/L3: Promotion ──────────────────────────────────────────────────────

/// Decide whether a model should be promoted from `from` → `to` based on
/// scoreboard evidence.
///
/// Inputs are *summary* statistics, not raw scoreboard rows, so this fn is
/// trivially unit-testable and the policy is auditable.
///
/// Thresholds come from `model-pins.v1.yaml` (`classifier.promotion_thresholds`).
#[must_use]
pub fn should_promote(
    from: ModelConfidence,
    successful_calls: u32,
    p50_latency_ms: f64,
    catalog_median_p50_ms: f64,
    classifier_confidence: f32,
) -> Option<ModelConfidence> {
    let pins = vox_config::load_model_pins_config().unwrap_or_default();
    let thresholds = pins.classifier.promotion_thresholds;

    match from {
        ModelConfidence::Provisional => {
            // Provisional → Shadowed once classifier confidence is high enough.
            if classifier_confidence >= thresholds.min_classifier_confidence {
                Some(ModelConfidence::Shadowed)
            } else {
                None
            }
        }
        ModelConfidence::Shadowed => {
            // Shadowed → Confirmed once scoreboard meets thresholds.
            let latency_ok = catalog_median_p50_ms == 0.0
                || p50_latency_ms <= catalog_median_p50_ms * thresholds.max_p50_latency_multiple;
            if successful_calls >= thresholds.min_successful_calls && latency_ok {
                Some(ModelConfidence::Confirmed)
            } else {
                None
            }
        }
        ModelConfidence::Confirmed | ModelConfidence::Deprecated => None,
    }
}

/// Record a confidence-state transition. Emits telemetry; the registry mutation
/// (if any) is the caller's responsibility — the autonomic module is a pure
/// observer of state changes, not the storage layer.
pub fn record_promotion(
    model_id: &str,
    from: ModelConfidence,
    to: ModelConfidence,
    evidence: PromotionEvidence,
) {
    let event = ConfidencePromotionEvent {
        model_id: model_id.to_string(),
        from: from.as_str().to_string(),
        to: to.as_str().to_string(),
        evidence: evidence.as_str().to_string(),
    };
    vox_telemetry::record_event!(&TelemetryEvent::ConfidencePromotion(event));
}

/// What evidence drove a confidence transition. Free-form on the wire but
/// constrained to known values inside this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionEvidence {
    ScoreboardThreshold,
    CouncilApproval,
    ShadowEval,
    FailureThreshold,
    /// Listed in `model-pins.v1.yaml` `retired_ids:` — explicit council retirement.
    CouncilRetirement,
}

impl PromotionEvidence {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ScoreboardThreshold => "scoreboard_threshold",
            Self::CouncilApproval => "council_approval",
            Self::ShadowEval => "shadow_eval",
            Self::FailureThreshold => "failure_threshold",
            Self::CouncilRetirement => "council_retirement",
        }
    }
}

// ─── L3: Council-report rendering ──────────────────────────────────────────

/// Build a council-report markdown for the current registry state.
///
/// **Status:** stub render — emits a placeholder structure with section headers.
/// A follow-up phase will query the telemetry sink (`research_metrics` table)
/// to fill in: top-N selected models, per-model cost rollup, recently-discovered
/// provisional ids awaiting shadow-eval, and recently-promoted/-deprecated ids.
#[must_use]
pub fn render_council_report(registry: &ModelRegistry) -> String {
    let mut out = String::new();
    out.push_str("# Vox Model Council Report\n\n");
    out.push_str("> Auto-generated. SSOT: `docs/src/architecture/model-autonomic-system-2026.md`.\n\n");
    out.push_str("## Catalog snapshot\n\n");
    let models = registry.list_models();
    out.push_str(&format!("Total models in registry: **{}**\n\n", models.len()));
    out.push_str("## Top tiers\n\n");
    out.push_str("| Tier | Count |\n|------|-------|\n");
    let mut tier_counts = std::collections::BTreeMap::<String, usize>::new();
    for m in &models {
        let tier = format!("{:?}", m.capabilities.tier);
        *tier_counts.entry(tier).or_insert(0) += 1;
    }
    for (tier, count) in &tier_counts {
        out.push_str(&format!("| {tier} | {count} |\n"));
    }
    out.push_str("\n## TODO sections (telemetry-backed)\n\n");
    out.push_str("- [ ] Top-N most-selected models this quarter (from SelectionDecisionEvent)\n");
    out.push_str("- [ ] Cost rollup by model (from ModelCallEvent)\n");
    out.push_str("- [ ] Provisional models awaiting shadow-eval (from DiscoveryEvent + ClassificationEvent)\n");
    out.push_str("- [ ] Recently confirmed / deprecated (from ConfidencePromotionEvent)\n");
    out
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confidence_routing_eligibility() {
        assert!(!ModelConfidence::Provisional.eligible_for_routing());
        assert!(!ModelConfidence::Shadowed.eligible_for_routing());
        assert!(ModelConfidence::Confirmed.eligible_for_routing());
        assert!(!ModelConfidence::Deprecated.eligible_for_routing());
    }

    #[test]
    fn discovery_source_wire_strings() {
        assert_eq!(DiscoverySource::OpenRouter.as_str(), "openrouter");
        assert_eq!(DiscoverySource::LiteLlm.as_str(), "litellm");
        assert_eq!(DiscoverySource::AnthropicDirect.as_str(), "anthropic_direct");
        assert_eq!(DiscoverySource::PopuliMesh.as_str(), "populi_mesh");
    }

    #[test]
    fn classifier_prompt_includes_target_id() {
        let p = build_classifier_prompt(
            "anthropic/claude-test-1",
            Some("a test model"),
            &["tools".to_string(), "vision".to_string()],
            Some(0.001),
        );
        assert!(p.contains("anthropic/claude-test-1"));
        assert!(p.contains("a test model"));
        assert!(p.contains("tools"));
        assert!(p.contains("0.0010"));
    }

    #[test]
    fn promotion_provisional_needs_classifier_confidence() {
        // Below threshold → stays Provisional.
        assert_eq!(
            should_promote(ModelConfidence::Provisional, 0, 0.0, 0.0, 0.50),
            None
        );
        // At/above threshold → Shadowed.
        assert_eq!(
            should_promote(ModelConfidence::Provisional, 0, 0.0, 0.0, 0.80),
            Some(ModelConfidence::Shadowed)
        );
    }

    #[test]
    fn promotion_shadowed_needs_scoreboard_data() {
        // Not enough calls → stays Shadowed.
        assert_eq!(
            should_promote(ModelConfidence::Shadowed, 10, 500.0, 800.0, 0.99),
            None
        );
        // Enough calls + latency OK → Confirmed.
        assert_eq!(
            should_promote(ModelConfidence::Shadowed, 100, 500.0, 800.0, 0.99),
            Some(ModelConfidence::Confirmed)
        );
        // Enough calls but latency 3× catalog median → blocked.
        assert_eq!(
            should_promote(ModelConfidence::Shadowed, 100, 5_000.0, 800.0, 0.99),
            None
        );
    }

    #[test]
    fn promotion_confirmed_and_deprecated_are_sink_states() {
        assert_eq!(
            should_promote(ModelConfidence::Confirmed, 1000, 100.0, 100.0, 0.99),
            None
        );
        assert_eq!(
            should_promote(ModelConfidence::Deprecated, 1000, 100.0, 100.0, 0.99),
            None
        );
    }

    #[test]
    fn diff_skips_retired_ids() {
        let prior = HashSet::new();
        let fresh = vec![
            DiscoveredModel {
                id: "anthropic/claude-3.5-sonnet".to_string(), // retired
                description: None,
                max_context_tokens: None,
            },
            DiscoveredModel {
                id: "anthropic/claude-future-1".to_string(),
                description: None,
                max_context_tokens: None,
            },
        ];
        let new_ids = diff_and_emit_discovery(DiscoverySource::OpenRouter, &prior, fresh);
        // Retired ids must not appear.
        assert!(!new_ids.contains(&"anthropic/claude-3.5-sonnet".to_string()));
        assert!(new_ids.contains(&"anthropic/claude-future-1".to_string()));
    }

    #[test]
    fn diff_skips_already_known_ids() {
        let mut prior = HashSet::new();
        prior.insert("anthropic/claude-future-1".to_string());
        let fresh = vec![DiscoveredModel {
            id: "anthropic/claude-future-1".to_string(),
            description: None,
            max_context_tokens: None,
        }];
        let new_ids = diff_and_emit_discovery(DiscoverySource::OpenRouter, &prior, fresh);
        assert!(new_ids.is_empty());
    }

    #[test]
    fn council_report_renders_with_empty_registry() {
        let registry = ModelRegistry::new();
        let report = render_council_report(&registry);
        assert!(report.contains("Vox Model Council Report"));
        assert!(report.contains("Catalog snapshot"));
    }
}
