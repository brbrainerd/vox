use std::sync::Arc;

use vox_orchestrator_types::socrates_policy::ConfidencePolicy;

use vox_orchestrator::services::embeddings::EmbeddingService;

/// Progress reporting callback for research operations.
pub type ProgressCallback = dyn Fn(String, Option<f32>) + Send + Sync + 'static;

/// Anti-laziness rider for all research LLM prompts.
pub(super) const ANTI_LAZINESS_RIDER: &str = "
<anti_laziness_rider>
DO NOT summarize or skip steps. DO NOT provide stubs, placeholders, or 'TODO' blocks. Implement ALL requested logic in full detail.
If providing a plan, ensure it is exhaustive and execution-ready. Laziness will be penalized with a 0 quality score.
</anti_laziness_rider>";

/// Configuration for a single research run.
#[derive(Clone)]
pub struct ResearchConfig {
    /// LLM endpoint base URL (e.g. `https://api.openai.com`).
    pub llm_endpoint: Option<String>,
    /// Bearer API key for the LLM endpoint.
    pub api_key: Option<String>,
    /// Model used for query decomposition / planning.
    pub planner_model: String,
    /// Sampling temperature for the planner (lower = more deterministic).
    pub planner_temperature: f32,
    /// Maximum number of subqueries the planner may emit.
    pub planner_max_subqueries: usize,
    /// Model used for claim extraction.
    pub claim_model: String,
    /// Max tokens for a single claim-extraction response.
    pub claim_max_tokens: u32,
    /// Model used for answer synthesis.
    pub synthesis_model: String,
    /// Sampling temperature for synthesis.
    pub synthesis_temperature: f32,
    /// Max tokens for synthesis response.
    pub synthesis_max_tokens: u32,
    /// Model used for the LLM-as-judge quality scorer.
    pub judge_model: String,
    /// Sampling temperature for the judge.
    pub judge_temperature: f32,
    /// Max tokens for the judge response.
    pub judge_max_tokens: u32,
    /// Quality score returned when no LLM judge is available.
    pub fallback_quality_score: i32,
    /// Max chars for the synthesis LLM context (hits + verdict text).
    pub synthesis_context_max_chars: usize,
    /// Maximum characters per extracted chunk.
    pub chunk_max_chars: usize,
    /// Chars of overlap between consecutive chunks.
    pub chunk_overlap_chars: usize,
    /// Multiplier applied to the provider score for high-trust domains.
    pub trust_multiplier: f64,
    /// Minimum confidence before a doc is persisted to `docs/src/research/`.
    pub persist_min_confidence: f64,
    /// Confidence gate configuration.
    pub gate: super::super::config::GateConfig,
    /// Claim verifier configuration.
    pub verifier: super::super::config::VerifierConfig,
    /// Routing tier thresholds.
    pub routing_thresholds: super::super::config::RoutingThresholds,
    /// Fusion weights: (vector_weight, bm25_weight, kb_chunk_weight).
    pub fusion_weights: (f64, f64, f64),
    /// Minimum confidence to write a Mens training pair.
    pub training_pair_min_confidence: f64,
    /// Minimum citation count to write a Mens training pair.
    pub training_pair_min_citations: usize,
    /// Max age in seconds for a cached research result.
    pub cache_ttl_secs: u64,
    /// Provider configuration (high-trust domains, timeout, etc.).
    pub provider: super::super::config::ProviderConfig,
    /// Optional embedding service for indexing chunks.
    pub embedder: Option<Arc<EmbeddingService>>,
    /// Whether claim detection and verification is enabled for this run.
    ///
    /// Can be overridden at runtime by the `rollout.claim_detection` config key.
    pub claim_detection_enabled: bool,
    /// Optional callback for progress reporting.
    pub progress_callback: Option<Arc<ProgressCallback>>,
    /// Optional SCIENTIA event sink for mesh signal emission.
    pub event_emitter: Option<Arc<dyn vox_research_events::ResearchEventEmitter>>,
    /// Optional retrieval feedback (caller-supplied). When unset and a DB is attached, the pipeline
    /// loads a short rolling window from `research_metrics` before applying [`vox_search::SearchPolicy::with_scientia_feedback`].
    pub search_policy_feedback: Option<vox_search::SearchPolicyFeedback>,
    /// Optional snapshot of workspace inference policy for registry stage picks.
    ///
    /// Phase 0a STUB: uses `super::super::model_select::InferenceConfig` (static fallbacks).
    /// Phase 1 replaces with `vox_orchestrator::mode::InferenceConfig` when that module is activated.
    pub model_pick_inference: Option<super::super::model_select::InferenceConfig>,
}

impl std::fmt::Debug for ResearchConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResearchConfig")
            .field("llm_endpoint", &self.llm_endpoint)
            .field("planner_model", &self.planner_model)
            .field("claim_model", &self.claim_model)
            .field("synthesis_model", &self.synthesis_model)
            .field("judge_model", &self.judge_model)
            .field("chunk_max_chars", &self.chunk_max_chars)
            .field("fusion_weights", &self.fusion_weights)
            .field("event_emitter", &self.event_emitter.is_some())
            .field(
                "search_policy_feedback",
                &self.search_policy_feedback.is_some(),
            )
            .finish_non_exhaustive()
    }
}

impl Default for ResearchConfig {
    /// Phase 0a STUB: uses static fallback model IDs from vox-config constants.
    /// Phase 1 wires to live ModelRegistry resolution via InferenceConfig.
    fn default() -> Self {
        let reg = vox_orchestrator::models::ModelRegistry::new();
        let base = super::super::model_select::InferenceConfig::default();
        let r = super::super::model_select::resolve_research_models(&reg, &base);
        let verifier = super::super::config::VerifierConfig {
            nli_model_id: r.claim_model.clone(),
            ..super::super::config::VerifierConfig::default()
        };
        Self {
            llm_endpoint: None,
            api_key: None,
            planner_model: r.planner_model,
            planner_temperature: 0.3,
            planner_max_subqueries: 6,
            claim_model: r.claim_model,
            claim_max_tokens: 512,
            synthesis_model: r.synthesis_model,
            synthesis_temperature: 0.2,
            synthesis_max_tokens: 1200,
            judge_model: r.judge_model,
            judge_temperature: 0.0,
            judge_max_tokens: 16,
            fallback_quality_score: i32::from(
                ConfidencePolicy::DEFAULT_MIN_REVIEW_FINDING_CONFIDENCE,
            ),
            synthesis_context_max_chars: 8000,
            chunk_max_chars: 1200,
            chunk_overlap_chars: 150,
            trust_multiplier: 1.2,
            persist_min_confidence: ConfidencePolicy::DEFAULT_MIN_PERSIST_CONFIDENCE,
            gate: super::super::config::GateConfig::default(),
            verifier,
            routing_thresholds: super::super::config::RoutingThresholds::default(),
            fusion_weights: (0.65, 0.50, 0.80),
            training_pair_min_confidence: ConfidencePolicy::DEFAULT_MIN_TRAINING_PAIR_CONFIDENCE,
            training_pair_min_citations: 2,
            cache_ttl_secs: 3600,
            provider: super::super::config::ProviderConfig::default(),
            embedder: None,
            claim_detection_enabled: true,
            progress_callback: None,
            event_emitter: None,
            search_policy_feedback: None,
            model_pick_inference: None,
        }
    }
}
