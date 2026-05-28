use crate::claim_extractor::atomic::{AtomicConfig, AtomicDecomposer};
use crate::claim_extractor::constrained::validate_claim_envelope;
use crate::claim_extractor::minicheck::MiniCheckVerifier;
use crate::claim_extractor::span::SpanChecker;
use crate::claim_extractor::types::{AtomicClaim, ClaimVerdict, ExtractionResult};
use crate::claim_extractor::veriscore::{VeriScoreConfig, VeriScoreGate};

#[derive(Debug, Clone)]
pub struct ExtractionConfig {
    pub veriscore: VeriScoreConfig,
    pub atomic: AtomicConfig,
    pub abstain_threshold: f64,
    pub promotion_threshold: f64,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            veriscore: VeriScoreConfig::default(),
            atomic: AtomicConfig::default(),
            abstain_threshold: 0.3,
            promotion_threshold: 0.7,
        }
    }
}

pub struct ExtractionPipeline {
    config: ExtractionConfig,
    gate: VeriScoreGate,
    decomposer: AtomicDecomposer,
    span_checker: SpanChecker,
    verifier: MiniCheckVerifier,
}

impl ExtractionPipeline {
    pub fn new(config: ExtractionConfig) -> Self {
        let gate = VeriScoreGate::new(config.veriscore.clone());
        let decomposer = AtomicDecomposer::new(config.atomic.clone());
        let span_checker = SpanChecker::default();
        let verifier = MiniCheckVerifier::from_env();
        Self {
            config,
            gate,
            decomposer,
            span_checker,
            verifier,
        }
    }

    pub async fn extract(
        &self,
        source_text: &str,
        context_passages: &[&str],
    ) -> Result<ExtractionResult, Box<dyn std::error::Error + Send + Sync>> {
        let sentences = split_sentences(source_text);
        let verifiable = self.gate.filter_sentences(&sentences);
        let abstained = sentences.len() - verifiable.len();

        let mut all_claims: Vec<AtomicClaim> = Vec::new();
        for (sentence, _score) in &verifiable {
            let claims = self.decomposer.decompose(sentence);
            all_claims.extend(claims);
        }

        let valid_claims: Vec<AtomicClaim> = all_claims
            .into_iter()
            .filter(|c| self.span_checker.check(&c.text, &c.span, source_text))
            .collect();

        // Stage 6: Constrained envelope validation
        let valid_claims: Vec<AtomicClaim> = valid_claims.into_iter()
            .filter_map(|c| {
                match serde_json::to_value(&c) {
                    Ok(json) => {
                        if validate_claim_envelope(&json).is_ok() { Some(c) } else { None }
                    }
                    Err(e) => {
                        tracing::warn!(claim_id = c.id, error = %e, "claim serialization failed; dropping");
                        None
                    }
                }
            })
            .collect();

        let context = context_passages.join(" ");
        let mut verdicts: Vec<ClaimVerdict> = Vec::new();
        let mut promotable: Vec<u64> = Vec::new();

        for claim in &valid_claims {
            let output = self.verifier.verify_claim(&claim.text, &context).await?;
            let verdict = if output.abstained {
                ClaimVerdict::Abstain {
                    reason: format!(
                        "support_score={:.2} < τ={:.2}",
                        output.support_score, self.config.abstain_threshold
                    ),
                }
            } else if output.support_score >= self.config.promotion_threshold {
                promotable.push(claim.id);
                ClaimVerdict::Supported {
                    confidence: output.support_score,
                }
            } else {
                ClaimVerdict::Contested {
                    confidence: output.support_score,
                }
            };
            verdicts.push(verdict);
        }

        Ok(ExtractionResult {
            source_text: source_text.to_string(),
            claims: valid_claims,
            verdicts,
            promotable_claim_ids: promotable,
            abstained_sentence_count: abstained,
        })
    }
}

fn split_sentences(text: &str) -> Vec<String> {
    let mut sentences = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        current.push(ch);
        if ch == '.' || ch == '!' || ch == '?' {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                sentences.push(trimmed);
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        sentences.push(current.trim().to_string());
    }
    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn pipeline_extracts_from_verifiable_sentence() {
        let pipeline = ExtractionPipeline::new(ExtractionConfig::default());
        let result = pipeline
            .extract(
                "Provider X p95 latency increased by 12ms after the April 2026 model update.",
                &[],
            )
            .await
            .unwrap();
        assert!(!result.claims.is_empty());
    }

    #[tokio::test]
    async fn pipeline_abstains_on_hedge() {
        let pipeline = ExtractionPipeline::new(ExtractionConfig::default());
        let result = pipeline
            .extract("Future work may potentially explore improvements.", &[])
            .await
            .unwrap();
        assert!(result.promotable_claim_ids.is_empty());
        assert!(result.abstained_sentence_count > 0);
    }
}
