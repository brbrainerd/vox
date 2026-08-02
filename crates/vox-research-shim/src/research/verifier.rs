//! Claim verification against retrieved research evidence.

use std::fmt;

use serde::{Deserialize, Serialize};

use super::claims::Claim;
use super::provider::ProviderRegistry;
use super::types::ResearchHit;

/// Verifier configuration. Phase 0a — fields are placeholders; Phase 1
/// adds calibration parameters (`abstain_threshold`, `temperature`,
/// `escalation_endpoint`, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifierConfig {
    pub abstain_threshold: Option<f32>,
    pub model: Option<String>,
    /// NLI model ID used for claim classification.
    /// Defaults to the registry FALLBACK_NLI_MODEL_ID constant; overridden
    /// in `verifier_config_for_research_run` when registry resolves a better model.
    ///
    /// **Phase-0a default is empty string.** Phase 1 must set this to a real
    /// model ID before calling the verifier, or behavior is undefined.
    pub nli_model_id: String,
}

/// Per-claim verification outcome.
///
/// **Taxonomy note:** the SCIENTIA plan (§3.2, citing
/// [SciFact (arXiv 2210.13777)](https://arxiv.org/abs/2210.13777)) specifies
/// the canonical SciFact labels: `Support`, `Contradict`, `NotEnoughInfo`,
/// `Abstain`. The variants here (`Supported`, `Contradicted`, `Contested`,
/// `Unverified`) match the pre-existing consumer at
/// `dei_shim::research::orchestrator::stages` to keep Phase 0a compile-correct
/// without rewriting unrelated code. Phase 1's `vox-claim-extractor`
/// integration is the right point to reconcile to the SciFact taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Supported,
    Contradicted,
    Contested,
    Unverified,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supported => write!(f, "supported"),
            Self::Contradicted => write!(f, "contradicted"),
            Self::Contested => write!(f, "contested"),
            Self::Unverified => write!(f, "unverified"),
        }
    }
}

/// Type of evidence span linkage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    Supporting,
    Contradicting,
    Background,
}

impl fmt::Display for SpanType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supporting => write!(f, "supporting"),
            Self::Contradicting => write!(f, "contradicting"),
            Self::Background => write!(f, "background"),
        }
    }
}

/// One evidence span linking a claim to a source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSpan {
    pub source_id: i64,
    pub span_start: usize,
    pub span_end: usize,
    pub text: String,
    pub span_type: SpanType,
}

/// Per-claim verification verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimVerdict {
    pub claim: Claim,
    pub verdict: Verdict,
    pub confidence: f64,
    pub supporting_count: usize,
    pub contradicting_count: usize,
    pub evidence_spans: Vec<EvidenceSpan>,
    /// Fraction of `RESAMPLE_COUNT` repeated verification calls that agreed
    /// with the final `verdict`. 1.0 = fully consistent, lower = the LLM
    /// gave different answers across resamples for the same claim/evidence.
    pub resample_stability: f64,
    // NOTE: renamed from `self_consistency` — do not confuse with the
    // unrelated `self_consistency` signal in vox-orchestrator's
    // confidence_fusion.rs (a different, pre-existing concept).
}

/// Fraction of `verdicts` matching the most common verdict among them.
/// Used as a self-consistency signal: low agreement across repeated
/// samples of the same claim/evidence pair suggests the LLM's verdict is
/// unreliable, independent of its own stated confidence.
fn agreement_rate(verdicts: &[Verdict]) -> f64 {
    if verdicts.is_empty() {
        return 0.0;
    }
    use std::collections::HashMap;
    let mut counts: HashMap<Verdict, usize> = HashMap::new();
    for v in verdicts {
        *counts.entry(*v).or_insert(0) += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    max_count as f64 / verdicts.len() as f64
}

/// Picks the verdict with the most support among `verdicts`. When every
/// verdict is distinct (no genuine majority — e.g. a 3-way split), returns
/// `Verdict::Contested` rather than arbitrarily picking one disagreeing
/// sample, since `agreement_rate` alone can't distinguish "one dissenter"
/// from "no consensus at all" without this.
///
/// Note: this "max_count == 1 implies full disagreement" check is only
/// exhaustive for `RESAMPLE_COUNT == 3` (the only tie shape possible with 3
/// samples is 3 distinct verdicts). If `RESAMPLE_COUNT` changes, revisit
/// this logic for other tie shapes (e.g. two 2-way ties among 4 samples).
fn majority_verdict(verdicts: &[Verdict]) -> Verdict {
    use std::collections::HashMap;
    let mut counts: HashMap<Verdict, usize> = HashMap::new();
    for v in verdicts {
        *counts.entry(*v).or_insert(0) += 1;
    }
    let max_count = counts.values().copied().max().unwrap_or(0);
    let distinct_leaders = counts.values().filter(|&&c| c == max_count).count();
    if max_count == 1 && distinct_leaders > 1 {
        // Full disagreement — every sample got exactly one vote.
        return Verdict::Contested;
    }
    *verdicts
        .iter()
        .max_by_key(|v| counts[*v])
        .expect("verdicts is non-empty")
}

// For each claim, sample the verification cascade `RESAMPLE_COUNT` times
// and keep the majority verdict, recording the agreement rate as the new
// `resample_stability` field on `ClaimVerdict`.
const RESAMPLE_COUNT: usize = 3;

/// Merges `primary` (from `primary_candidate_for_intent`, if any key-gated
/// candidate cleared selection) ahead of `fallback` (the cascade), and forces
/// verification's per-candidate overrides (`max_tokens`, JSON response
/// format, and a nonzero resample temperature) onto every candidate —
/// including `primary`, which `llm_config_for_spec` always builds with
/// `temperature: None` since it never runs through `apply_stage_defaults`.
/// Without this, whichever candidate is tried first when a key is configured
/// resamples at whatever the provider's own default temperature is, and
/// `resample_stability` no longer reflects genuine sample variation.
#[cfg(feature = "runtime")]
fn resample_candidates(
    primary: Option<vox_actor_runtime::llm::LlmConfig>,
    fallback: Vec<vox_actor_runtime::llm::LlmConfig>,
) -> Vec<vox_actor_runtime::llm::LlmConfig> {
    let mut candidates: Vec<vox_actor_runtime::llm::LlmConfig> = primary.into_iter().collect();
    candidates.extend(fallback);
    for candidate in &mut candidates {
        candidate.max_tokens = Some(500);
        candidate.response_format = Some(serde_json::json!({"type": "json_object"}));
        candidate.temperature = Some(0.3);
    }
    candidates
}

/// Verify a batch of claims against retrieved evidence.
///
/// Verifies claims against evidence via an LLM cascade (behind the `runtime`
/// feature; without it, degrades to blanket `Unverified`). Each claim is
/// verified `RESAMPLE_COUNT` times (SelfCheckGPT-style resampling) and the
/// majority verdict is kept, with `ClaimVerdict::resample_stability` recording
/// the agreement rate — see Task 7 of
/// `docs/superpowers/plans/2026-08-01-deep-research-trust-novelty-core.md`.
/// This file's module-level SciFact-taxonomy note above still has the
/// open Verdict naming reconciliation, which resampling does not address.
pub async fn verify_claims_with_config(
    claims: &[Claim],
    query: &str,
    evidence_hits: &[ResearchHit],
    _registry: &ProviderRegistry,
    config: &VerifierConfig,
    endpoint: Option<&str>,
    api_key: Option<&str>,
) -> Vec<ClaimVerdict> {
    if claims.is_empty() || evidence_hits.is_empty() {
        return Vec::new();
    }

    #[cfg(feature = "runtime")]
    {
        use vox_actor_runtime::ActivityOptions;
        use vox_actor_runtime::llm::LlmChatMessage;
        use vox_actor_runtime::llm::cascade::{
            ResearchStage, cascade_with_optional_manual, chat_with_cascade,
        };
        use vox_actor_runtime::model_resolution::RouteResolutionInput;

        let mut input = RouteResolutionInput::default();
        if !config.nli_model_id.trim().is_empty() {
            input.openrouter_model = config.nli_model_id.clone();
        } else if let Some(model) = config.model.as_deref().filter(|m| !m.trim().is_empty()) {
            input.openrouter_model = model.to_string();
        }
        let abstain_threshold = config.abstain_threshold.unwrap_or(0.5);
        let evidence = evidence_context(evidence_hits, 8);
        let opts = ActivityOptions::new().with_timeout_secs(30);
        let mut verdicts = Vec::new();

        for claim in claims {
            // Resample the same claim/evidence pair RESAMPLE_COUNT times,
            // relying on the cascade's nonzero verification temperature
            // (see `apply_stage_defaults` in vox-actor-runtime's
            // `llm::cascade` module) to produce genuine variation across
            // samples. The samples are independent (no shared mutable
            // state), so run them concurrently rather than sequentially.
            let sample_futures = (0..RESAMPLE_COUNT).map(|_| {
                let opts = &opts;
                let input = &input;
                let evidence = &evidence;
                async move {
                    let primary = crate::research::orchestrator::model_dispatch::primary_candidate_for_intent(
                        vox_orchestrator::models::SelectionIntent::nli_classifier(),
                    );
                    let fallback = cascade_with_optional_manual(
                        ResearchStage::Verification,
                        input,
                        endpoint,
                        api_key,
                        Some(input.openrouter_model.as_str()),
                    );
                    let candidates = resample_candidates(primary, fallback);
                    let messages = vec![
                        LlmChatMessage {
                            role: "system".to_string(),
                            content: "Classify whether retrieved evidence supports the claim. \
                            Output only JSON: {\"verdict\":\"Supported|Contradicted|Contested|Unverified\",\
                            \"confidence\":0.0,\"supporting_indices\":[0],\"contradicting_indices\":[1]}."
                                .to_string(), ..Default::default()
                        },
                        LlmChatMessage {
                            role: "user".to_string(),
                            content: format!(
                                "Original question: {query}\n\nClaim: {}\n\nEvidence:\n{evidence}",
                                claim.text
                            ), ..Default::default()
                        },
                    ];
                    match chat_with_cascade(opts, messages, candidates, None).await {
                        Ok(response) => {
                            match parse_verifier_response(
                                &response.content,
                                claim.clone(),
                                evidence_hits,
                                abstain_threshold,
                            ) {
                                Ok(verdict) => verdict,
                                Err(e) => {
                                    tracing::warn!(claim_id = claim.claim_id, error = %e, "verifier response invalid");
                                    unverified(claim.clone())
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!(claim_id = claim.claim_id, error = %e, "verifier cascade failed");
                            unverified(claim.clone())
                        }
                    }
                }
            });
            let sampled: Vec<ClaimVerdict> = futures::future::join_all(sample_futures).await;
            verdicts.push(assemble_resampled_verdict(sampled));
        }

        verdicts
    }

    #[cfg(not(feature = "runtime"))]
    {
        let _ = (query, endpoint, api_key, config);
        claims.iter().cloned().map(unverified).collect()
    }
}

#[derive(Deserialize)]
struct VerifierPayload {
    verdict: String,
    confidence: f64,
    #[serde(default)]
    supporting_indices: Vec<usize>,
    #[serde(default)]
    contradicting_indices: Vec<usize>,
}

fn parse_verifier_response(
    response: &str,
    claim: Claim,
    evidence_hits: &[ResearchHit],
    abstain_threshold: f32,
) -> anyhow::Result<ClaimVerdict> {
    let payload: VerifierPayload = super::json_parse::parse_json_response(response)?;
    let confidence = payload.confidence.clamp(0.0, 1.0);
    let verdict = parse_verdict_label(&payload.verdict)?;
    if confidence < f64::from(abstain_threshold) {
        return Ok(unverified(claim));
    }

    let mut evidence_spans = Vec::new();
    for idx in payload.supporting_indices {
        if let Some(hit) = evidence_hits.get(idx) {
            let text = hit.snippet.clone();
            evidence_spans.push(EvidenceSpan {
                source_id: idx as i64,
                span_start: 0,
                span_end: text.len(),
                text,
                span_type: SpanType::Supporting,
            });
        }
    }
    for idx in payload.contradicting_indices {
        if let Some(hit) = evidence_hits.get(idx) {
            let text = hit.snippet.clone();
            evidence_spans.push(EvidenceSpan {
                source_id: idx as i64,
                span_start: 0,
                span_end: text.len(),
                text,
                span_type: SpanType::Contradicting,
            });
        }
    }
    let supporting_count = evidence_spans
        .iter()
        .filter(|span| span.span_type == SpanType::Supporting)
        .count();
    let contradicting_count = evidence_spans
        .iter()
        .filter(|span| span.span_type == SpanType::Contradicting)
        .count();

    Ok(ClaimVerdict {
        claim,
        verdict,
        confidence,
        supporting_count,
        contradicting_count,
        evidence_spans,
        resample_stability: 1.0,
    })
}

fn parse_verdict_label(raw: &str) -> anyhow::Result<Verdict> {
    match raw.trim().to_ascii_lowercase().replace('-', "_").as_str() {
        "supported" | "support" => Ok(Verdict::Supported),
        "contradicted" | "contradict" => Ok(Verdict::Contradicted),
        "contested" | "mixed" => Ok(Verdict::Contested),
        "unverified" | "not_enough_info" | "abstain" | "unknown" => Ok(Verdict::Unverified),
        other => anyhow::bail!("unknown verifier verdict `{other}`"),
    }
}

/// Combine `RESAMPLE_COUNT` independent verification samples for the same
/// claim into a single `ClaimVerdict`. The authoritative verdict is always
/// `majority_verdict` over the samples (forcing `Contested` on full
/// disagreement); the other fields (confidence/supporting/contradicting/
/// evidence_spans) are taken from a representative sample — one matching the
/// final verdict if any exists, preferring the highest self-reported
/// confidence, otherwise the highest-confidence sample overall.
///
/// Invariant: the returned `verdict` and `resample_stability` are never
/// contradictory — a low `resample_stability` from full disagreement always
/// implies `verdict == Verdict::Contested`, never a single sample's
/// confident individual answer.
fn assemble_resampled_verdict(sampled: Vec<ClaimVerdict>) -> ClaimVerdict {
    let sampled_verdicts: Vec<Verdict> = sampled.iter().map(|v| v.verdict).collect();
    let final_verdict = majority_verdict(&sampled_verdicts);
    let resample_stability = agreement_rate(&sampled_verdicts);
    // Among the samples matching the final verdict, keep the one with the
    // highest self-reported confidence for the other fields
    // (confidence/supporting/contradicting/evidence_spans). When
    // `majority_verdict` forced `Contested` on full disagreement, no sample
    // truly represents the group; prefer a sample that itself said
    // `Contested` if one exists, otherwise fall back to the
    // highest-confidence sample overall.
    let has_matching_sample = sampled.iter().any(|v| v.verdict == final_verdict);
    let mut chosen = sampled
        .into_iter()
        .filter(|v| !has_matching_sample || v.verdict == final_verdict)
        .fold(None::<ClaimVerdict>, |best, cand| match &best {
            Some(b) if b.confidence >= cand.confidence => best,
            _ => Some(cand),
        })
        .expect("sampled is non-empty");
    chosen.verdict = final_verdict;
    chosen.resample_stability = resample_stability;
    chosen
}

fn unverified(claim: Claim) -> ClaimVerdict {
    ClaimVerdict {
        claim,
        verdict: Verdict::Unverified,
        confidence: 0.0,
        supporting_count: 0,
        contradicting_count: 0,
        evidence_spans: Vec::new(),
        resample_stability: 1.0,
    }
}

fn evidence_context(hits: &[ResearchHit], limit: usize) -> String {
    hits.iter()
        .take(limit)
        .enumerate()
        .map(|(i, hit)| {
            format!(
                "[{i}] {}\nURL: {}\n{}",
                hit.title,
                hit.url,
                hit.snippet.replace('\n', " ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::research::types::ResearchHit;

    fn claim() -> Claim {
        Claim {
            text: "CRAG continues retrieval when evidence quality is below target.".to_string(),
            claim_id: 42,
            is_numeric: false,
            is_recent: false,
            is_named_event: true,
        }
    }

    fn hits() -> Vec<ResearchHit> {
        vec![
            ResearchHit {
                url: "https://example.com/a".to_string(),
                title: "A".to_string(),
                snippet: "CRAG checks evidence quality and may continue retrieval.".to_string(),
                score: 0.9,
                http_status: 200,
                trust_score: 1.0,
                raw_content: String::new(),
            },
            ResearchHit {
                url: "https://example.com/b".to_string(),
                title: "B".to_string(),
                snippet: "A contradictory source says retrieval always stops immediately."
                    .to_string(),
                score: 0.7,
                http_status: 200,
                trust_score: 1.0,
                raw_content: String::new(),
            },
        ]
    }

    #[cfg(feature = "runtime")]
    #[test]
    fn resample_candidates_forces_nonzero_temperature_on_primary_too() {
        let primary = Some(vox_actor_runtime::llm::LlmConfig::openrouter("primary-model"));
        let fallback = vec![vox_actor_runtime::llm::LlmConfig::openrouter("fallback-model")];

        let candidates = resample_candidates(primary, fallback);

        assert_eq!(candidates.len(), 2);
        assert_eq!(
            candidates[0].model, "primary-model",
            "primary candidate must be tried first"
        );
        assert!(
            candidates.iter().all(|c| c.temperature == Some(0.3)),
            "every resample candidate — including primary, which llm_config_for_spec \
             always builds with temperature: None — must resample at a nonzero \
             temperature or resample_stability stops reflecting genuine variation"
        );
    }

    #[test]
    fn parse_verifier_response_maps_verdict_indices_and_confidence() {
        let response = r#"```json
        {
          "verdict": "Supported",
          "confidence": 0.82,
          "supporting_indices": [0],
          "contradicting_indices": [1]
        }
        ```"#;

        let verdict =
            parse_verifier_response(response, claim(), &hits(), 0.5).expect("verdict parses");

        assert_eq!(verdict.verdict, Verdict::Supported);
        assert_eq!(verdict.supporting_count, 1);
        assert_eq!(verdict.contradicting_count, 1);
        assert_eq!(verdict.evidence_spans.len(), 2);
        assert_eq!(verdict.evidence_spans[0].source_id, 0);
        assert_eq!(verdict.evidence_spans[0].span_type, SpanType::Supporting);
        assert_eq!(verdict.evidence_spans[1].span_type, SpanType::Contradicting);
    }

    #[test]
    fn parse_verifier_response_abstains_below_threshold() {
        let verdict = parse_verifier_response(
            r#"{"verdict":"Supported","confidence":0.49,"supporting_indices":[0],"contradicting_indices":[]}"#,
            claim(),
            &hits(),
            0.5,
        )
        .expect("verdict parses");

        assert_eq!(verdict.verdict, Verdict::Unverified);
        assert_eq!(verdict.supporting_count, 0);
    }

    #[test]
    fn agreement_rate_computes_fraction_matching_majority_verdict() {
        let verdicts = vec![
            Verdict::Supported,
            Verdict::Supported,
            Verdict::Contradicted,
        ];
        assert_eq!(agreement_rate(&verdicts), 2.0 / 3.0);
    }

    #[test]
    fn agreement_rate_is_one_for_unanimous_verdicts() {
        let verdicts = vec![Verdict::Supported, Verdict::Supported, Verdict::Supported];
        assert_eq!(agreement_rate(&verdicts), 1.0);
    }

    #[test]
    fn agreement_rate_is_zero_for_empty_input() {
        let verdicts: Vec<Verdict> = vec![];
        assert_eq!(agreement_rate(&verdicts), 0.0);
    }

    fn verdict_with(v: Verdict, confidence: f64) -> ClaimVerdict {
        let mut cv = unverified(claim());
        cv.verdict = v;
        cv.confidence = confidence;
        cv
    }

    #[test]
    fn assemble_resampled_verdict_writes_back_contested_on_full_disagreement() {
        // Three samples fully disagree: Supported, Contradicted, Unverified.
        // majority_verdict forces Contested, and the final ClaimVerdict must
        // reflect that -- not the highest-confidence individual sample's
        // own (Supported) verdict.
        let sampled = vec![
            verdict_with(Verdict::Supported, 0.95),
            verdict_with(Verdict::Contradicted, 0.4),
            verdict_with(Verdict::Unverified, 0.2),
        ];

        let chosen = assemble_resampled_verdict(sampled);

        assert_eq!(chosen.verdict, Verdict::Contested);
        assert!(chosen.resample_stability < 1.0);
        // Regression invariant: verdict and resample_stability must never be
        // contradictory -- full disagreement (low resample_stability) implies
        // Contested, never a single confident sample's answer.
        if chosen.resample_stability < (2.0 / 3.0) {
            assert_eq!(chosen.verdict, Verdict::Contested);
        }
    }

    #[test]
    fn assemble_resampled_verdict_keeps_genuine_majority() {
        let sampled = vec![
            verdict_with(Verdict::Supported, 0.6),
            verdict_with(Verdict::Supported, 0.9),
            verdict_with(Verdict::Contradicted, 0.5),
        ];

        let chosen = assemble_resampled_verdict(sampled);

        assert_eq!(chosen.verdict, Verdict::Supported);
        // Representative sample among the matching ones is the
        // highest-confidence Supported sample.
        assert_eq!(chosen.confidence, 0.9);
        assert_eq!(chosen.resample_stability, 2.0 / 3.0);
    }

    #[test]
    fn majority_verdict_forces_contested_on_full_disagreement() {
        let verdicts = vec![
            Verdict::Supported,
            Verdict::Contradicted,
            Verdict::Contested,
        ];
        assert_eq!(majority_verdict(&verdicts), Verdict::Contested);
    }

    #[test]
    fn majority_verdict_picks_genuine_majority() {
        let verdicts = vec![
            Verdict::Supported,
            Verdict::Supported,
            Verdict::Contradicted,
        ];
        assert_eq!(majority_verdict(&verdicts), Verdict::Supported);
    }
}
