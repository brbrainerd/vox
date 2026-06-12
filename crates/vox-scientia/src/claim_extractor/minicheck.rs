use crate::claim_extractor::atomic::fnv1a_hash;
use crate::claim_extractor::types::VerifierOutput;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub enum MiniCheckBackend {
    Mock,
    Http { endpoint: String },
}

pub struct MiniCheckVerifier {
    pub backend: MiniCheckBackend,
    pub abstain_threshold: f64,
}

/// Negation tokens used by the Mock backend's lexical-negation asymmetry heuristic.
/// If one side (claim or context sentence) contains any of these tokens and the other
/// does not, the pair is treated as a potential contradiction.  This is a surface-form
/// heuristic — it is NOT a full NLI model.
const NEGATORS: &[&str] = &[
    "not", "no", "never", "fails", "cannot", "doesn't", "does not", "won't", "isn't", "aren't",
];

// Serialization types for the MiniCheck HTTP API.
#[derive(Serialize)]
struct MiniCheckRequest<'a> {
    claim: &'a str,
    context: &'a str,
}

#[derive(Deserialize)]
struct MiniCheckResponse {
    support_score: f64,
    /// Optional contradiction score returned by the endpoint.
    /// Absent in responses from endpoints that do not implement it (defaults to 0.0).
    #[serde(default)]
    contradiction_score: f64,
}

impl MiniCheckVerifier {
    pub fn mock() -> Self {
        Self {
            backend: MiniCheckBackend::Mock,
            abstain_threshold: 0.3,
        }
    }

    pub fn http(endpoint: impl Into<String>) -> Self {
        Self {
            backend: MiniCheckBackend::Http {
                endpoint: endpoint.into(),
            },
            abstain_threshold: 0.3,
        }
    }

    pub fn from_env() -> Self {
        if let Ok(url) = std::env::var("VOX_MINICHECK_ENDPOINT") {
            Self::http(url)
        } else {
            Self::mock()
        }
    }

    pub async fn verify_claim(
        &self,
        claim: &str,
        context: &str,
    ) -> Result<VerifierOutput, Box<dyn std::error::Error + Send + Sync>> {
        let claim_id = fnv1a_hash(claim);
        match &self.backend {
            MiniCheckBackend::Mock => {
                let claim_words: Vec<&str> = claim.split_whitespace().collect();

                // Split context into sentences for best-overlap detection.
                // Naive split on '.', '!', '?' — sufficient for the heuristic.
                let context_sentences: Vec<&str> = context
                    .split(['.', '!', '?'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .collect();

                // Helper: word-overlap fraction for one context sentence.
                // Tokens are compared punctuation-stripped and inflection-tolerant
                // ("reduces" matches "reduce"): exact equality, or a shared prefix
                // when both tokens are ≥4 chars.
                let word_matches = |a: &str, b: &str| -> bool {
                    a == b
                        || (a.len() >= 4 && b.len() >= 4 && (a.starts_with(b) || b.starts_with(a)))
                };
                let norm = |w: &str| -> String {
                    w.trim_matches(|c: char| !c.is_alphanumeric())
                        .to_ascii_lowercase()
                };
                let overlap_with = |sentence: &str| -> f64 {
                    if claim_words.is_empty() {
                        return 0.5;
                    }
                    let sentence_words: Vec<String> = sentence
                        .split_whitespace()
                        .map(norm)
                        .filter(|w| !w.is_empty())
                        .collect();
                    let n = claim_words
                        .iter()
                        .map(|w| norm(w))
                        .filter(|w| !w.is_empty())
                        .filter(|w| sentence_words.iter().any(|s| word_matches(w, s)))
                        .count();
                    n as f64 / claim_words.len() as f64
                };

                // Find the context sentence with the highest overlap fraction.
                let best_sentence = context_sentences.iter().copied().max_by(|a, b| {
                    overlap_with(a)
                        .partial_cmp(&overlap_with(b))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });

                let (overlap_score, best_sentence) = if let Some(s) = best_sentence {
                    (overlap_with(s), s)
                } else {
                    // No context at all: fall back to full-context overlap (original behaviour).
                    let n = if claim_words.is_empty() {
                        return Ok(VerifierOutput {
                            claim_id,
                            support_score: 0.5,
                            contradiction_score: 0.0,
                            abstained: 0.5_f64 < self.abstain_threshold,
                            verifier_model: "mock".to_string(),
                        });
                    } else {
                        claim_words
                            .iter()
                            .filter(|w| {
                                context
                                    .to_ascii_lowercase()
                                    .contains(&w.to_ascii_lowercase())
                            })
                            .count()
                    };
                    (n as f64 / claim_words.len() as f64, context)
                };

                // Lexical-negation asymmetry heuristic (NOT an NLI model):
                // if one of the two texts contains a negator and the other does not,
                // treat the pair as potentially contradictory.
                let contains_negator = |text: &str| -> bool {
                    let lower = text.to_ascii_lowercase();
                    NEGATORS.iter().any(|n| {
                        if n.contains(' ') {
                            // Multi-word negators ("does not") match as phrases.
                            lower.contains(n)
                        } else {
                            // Single-word negators match whole tokens only —
                            // "no" must not fire inside "know" or "nominal".
                            lower
                                .split_whitespace()
                                .map(|w| {
                                    w.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
                                })
                                .any(|w| w == *n)
                        }
                    })
                };
                let claim_negated = contains_negator(claim);
                let context_negated = contains_negator(best_sentence);
                let negation_asymmetry = claim_negated != context_negated;

                let (support_score, contradiction_score) = if negation_asymmetry {
                    // One side negates the other: high overlap with flipped polarity
                    // → contradiction.  Reduce support to signal inversion.
                    let reduced_support = (1.0 - overlap_score).min(overlap_score);
                    (reduced_support, overlap_score)
                } else {
                    let raw = 0.5 + 0.5 * overlap_score;
                    (raw, 0.0_f64)
                };

                Ok(VerifierOutput {
                    claim_id,
                    support_score,
                    contradiction_score,
                    abstained: support_score < self.abstain_threshold,
                    verifier_model: "mock".to_string(),
                })
            }
            MiniCheckBackend::Http { endpoint } => {
                let client = vox_http_client::client_builder()
                    .timeout(vox_config::timeouts::D_10S)
                    .build()?;
                let payload = MiniCheckRequest { claim, context };
                let resp = client
                    .post(endpoint)
                    .json(&payload)
                    .send()
                    .await?
                    .error_for_status()?;
                let body: MiniCheckResponse = resp.json().await?;
                let score = body.support_score.clamp(0.0, 1.0);
                let contradiction_score = body.contradiction_score.clamp(0.0, 1.0);
                Ok(VerifierOutput {
                    claim_id,
                    support_score: score,
                    contradiction_score,
                    abstained: score < self.abstain_threshold,
                    verifier_model: endpoint.clone(),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_verifier_returns_result() {
        let verifier = MiniCheckVerifier::mock();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt
            .block_on(verifier.verify_claim(
                "latency increased",
                "The provider's p95 latency rose by 12ms in April 2026.",
            ))
            .unwrap();
        assert!(result.support_score >= 0.0 && result.support_score <= 1.0);
    }

    /// Mock lexical-negation asymmetry: claim asserts a fact, context denies it.
    ///
    /// Scoring walkthrough (punctuation-stripped, prefix-tolerant matching):
    ///   claim = "The cache reduces latency."  →  4 tokens [the, cache, reduces, latency]
    ///   context sentence = "The cache does not reduce latency"
    ///   matches: the ✓, cache ✓, reduces→reduce (prefix) ✓, latency ✓ → 4/4
    ///   overlap_score = 1.0
    ///   claim has no negator; context sentence contains "does not" → asymmetry = true
    ///   contradiction_score = 1.0  (= overlap_score)
    ///   support_score = min(1.0−1.0, 1.0) = 0.0
    #[test]
    fn negation_mismatch_yields_contradiction_score() {
        let verifier = MiniCheckVerifier::mock();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let result = rt
            .block_on(verifier.verify_claim(
                "The cache reduces latency.",
                "The cache does not reduce latency.",
            ))
            .unwrap();
        assert!(
            result.contradiction_score >= 0.5,
            "expected contradiction_score >= 0.5, got {}",
            result.contradiction_score
        );
        assert!(
            result.support_score < result.contradiction_score,
            "support_score {} should be less than contradiction_score {} when context negates claim",
            result.support_score,
            result.contradiction_score
        );
    }

    /// When claim and context are the same affirmative sentence, no negation
    /// asymmetry exists and contradiction_score must be 0.0.
    #[test]
    fn agreeing_text_has_zero_contradiction() {
        let verifier = MiniCheckVerifier::mock();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let text = "The cache reduces latency significantly.";
        let result = rt.block_on(verifier.verify_claim(text, text)).unwrap();
        assert_eq!(
            result.contradiction_score, 0.0,
            "identical affirmative text should have zero contradiction_score"
        );
    }
}
