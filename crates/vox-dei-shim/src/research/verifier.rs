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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// Verify a batch of claims against retrieved evidence.
///
/// **PHASE_0a_STUB**: returns `Vec::new()`. Phase 1 wires this to
/// `vox-claim-extractor`'s MiniCheck-backed verifier.
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
            let mut candidates = cascade_with_optional_manual(
                ResearchStage::Verification,
                &input,
                endpoint,
                api_key,
                Some(input.openrouter_model.as_str()),
            );
            for candidate in &mut candidates {
                candidate.temperature = Some(0.0);
                candidate.max_tokens = Some(500);
                candidate.response_format = Some(serde_json::json!({"type": "json_object"}));
            }
            let messages = vec![
                LlmChatMessage {
                    role: "system".to_string(),
                    content: "Classify whether retrieved evidence supports the claim. \
                        Output only JSON: {\"verdict\":\"Supported|Contradicted|Contested|Unverified\",\
                        \"confidence\":0.0,\"supporting_indices\":[0],\"contradicting_indices\":[1]}."
                        .to_string(),
                },
                LlmChatMessage {
                    role: "user".to_string(),
                    content: format!(
                        "Original question: {query}\n\nClaim: {}\n\nEvidence:\n{evidence}",
                        claim.text
                    ),
                },
            ];
            match chat_with_cascade(&opts, messages, candidates, None).await {
                Ok(response) => {
                    match parse_verifier_response(
                        &response.content,
                        claim.clone(),
                        evidence_hits,
                        abstain_threshold,
                    ) {
                        Ok(verdict) => verdicts.push(verdict),
                        Err(e) => {
                            tracing::warn!(claim_id = claim.claim_id, error = %e, "verifier response invalid");
                            verdicts.push(unverified(claim.clone()));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(claim_id = claim.claim_id, error = %e, "verifier cascade failed");
                    verdicts.push(unverified(claim.clone()));
                }
            }
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

fn unverified(claim: Claim) -> ClaimVerdict {
    ClaimVerdict {
        claim,
        verdict: Verdict::Unverified,
        confidence: 0.0,
        supporting_count: 0,
        contradicting_count: 0,
        evidence_spans: Vec::new(),
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
    use crate::dei_shim::research::types::ResearchHit;

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
}
