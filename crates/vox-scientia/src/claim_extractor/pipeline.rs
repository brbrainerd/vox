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
    /// Minimum `contradiction_score` from the verifier that promotes a verdict to
    /// `Contradicted` instead of `Abstain` or `Contested`.  Checked before the
    /// abstain/support/contest ladder so a high-confidence contradiction always wins.
    pub contradiction_threshold: f64,
}

impl Default for ExtractionConfig {
    fn default() -> Self {
        Self {
            veriscore: VeriScoreConfig::default(),
            atomic: AtomicConfig::default(),
            abstain_threshold: 0.3,
            promotion_threshold: 0.7,
            contradiction_threshold: 0.6,
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
            // Contradiction is checked first: a high contradiction_score overrides
            // the abstain/support/contest ladder regardless of support_score.
            let verdict = if output.contradiction_score >= self.config.contradiction_threshold {
                ClaimVerdict::Contradicted {
                    confidence: output.contradiction_score,
                }
            } else if output.abstained {
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
    /// Trailing tokens after which a `.` does not end a sentence ("e.g.", "vs.", …).
    const NON_TERMINAL_SUFFIXES: [&str; 4] = ["e.g", "i.e", "etc", "vs"];
    let chars: Vec<char> = text.chars().collect();
    let mut sentences = Vec::new();
    let mut current = String::new();
    for i in 0..chars.len() {
        let ch = chars[i];
        current.push(ch);
        let terminal = match ch {
            '!' | '?' => true,
            '.' => {
                // "12.5ms", "v0.6.2": a dot between digits is decimal/version punctuation.
                let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
                let next_digit = chars.get(i + 1).is_some_and(|c| c.is_ascii_digit());
                let mid_number = prev_digit && next_digit;
                let trimmed = current.trim_end_matches('.');
                let abbrev = NON_TERMINAL_SUFFIXES
                    .iter()
                    .any(|s| trimmed.to_lowercase().ends_with(s));
                let next_starts_sentence = match chars[i + 1..].iter().find(|c| !c.is_whitespace())
                {
                    None => true,
                    Some(c) => c.is_uppercase(),
                };
                !mid_number && !abbrev && next_starts_sentence
            }
            _ => false,
        };
        if terminal {
            let t = current.trim().to_string();
            if !t.is_empty() {
                sentences.push(t);
            }
            current.clear();
        }
    }
    let t = current.trim().to_string();
    if !t.is_empty() {
        sentences.push(t);
    }
    sentences
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_does_not_break_decimal_numbers() {
        let s = split_sentences("Latency fell to 12.5ms. Throughput rose 3.4x! Done?");
        assert_eq!(
            s,
            vec!["Latency fell to 12.5ms.", "Throughput rose 3.4x!", "Done?"]
        );
    }

    #[test]
    fn split_does_not_break_common_abbreviations_or_versions() {
        let s = split_sentences("Vox v0.6.2 ships today. See e.g. the docs.");
        assert_eq!(s.len(), 2, "got: {s:?}");
    }

    #[test]
    fn split_handles_trailing_unterminated_text() {
        let s = split_sentences("First sentence. Trailing fragment without period");
        assert_eq!(s.len(), 2);
    }

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

    /// When the context sentence explicitly negates the claim, the pipeline must
    /// emit at least one `Contradicted` verdict.
    ///
    /// Mock scoring for this pair:
    ///   claim = "The cache reduces latency."  (4 words)
    ///   context sentence = "The cache does not reduce latency."
    ///   overlap = 3/4 = 0.75  →  contradiction_score = 0.75
    ///   default contradiction_threshold = 0.6  →  0.75 ≥ 0.6  →  Contradicted ✓
    #[tokio::test]
    async fn negated_claim_against_contradicting_context_is_contradicted() {
        let pipeline = ExtractionPipeline::new(ExtractionConfig::default());
        let result = pipeline
            .extract(
                "The cache reduces latency.",
                &["The cache does not reduce latency."],
            )
            .await
            .unwrap();
        let has_contradicted = result
            .verdicts
            .iter()
            .any(|v| matches!(v, ClaimVerdict::Contradicted { .. }));
        assert!(
            has_contradicted,
            "expected at least one Contradicted verdict, got: {:?}",
            result.verdicts
        );
    }
}
