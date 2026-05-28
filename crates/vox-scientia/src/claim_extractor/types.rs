use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiabilityClass {
    Numeric,
    Structured,
    Semantic,
    EventBased,
    Unverifiable,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpanBound {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SciClaimTuple {
    pub variable_a: String,
    pub relation: String,
    pub variable_b: String,
    pub qualifier: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AtomicClaim {
    pub id: u64,
    pub text: String,
    pub tuple: Option<SciClaimTuple>,
    pub span: SpanBound,
    pub verifiability: VerifiabilityClass,
    pub verifiability_score: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerifierOutput {
    pub claim_id: u64,
    pub support_score: f64,
    pub abstained: bool,
    pub verifier_model: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum ClaimVerdict {
    Supported { confidence: f64 },
    Contradicted { confidence: f64 },
    Contested { confidence: f64 },
    Abstain { reason: String },
}

impl ClaimVerdict {
    pub fn is_supported(&self) -> bool {
        matches!(self, Self::Supported { .. })
    }
    pub fn is_promotable(&self) -> bool {
        matches!(self, Self::Supported { confidence } if *confidence >= 0.7)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractionResult {
    pub source_text: String,
    pub claims: Vec<AtomicClaim>,
    pub verdicts: Vec<ClaimVerdict>,
    pub promotable_claim_ids: Vec<u64>,
    pub abstained_sentence_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_claim_round_trips() {
        let claim = AtomicClaim {
            id: 12345678,
            text: "p95 latency increased by 10ms".to_string(),
            tuple: Some(SciClaimTuple {
                variable_a: "p95_latency_ms".to_string(),
                relation: "increased_by".to_string(),
                variable_b: "10.0 ms".to_string(),
                qualifier: Some("after model update".to_string()),
            }),
            span: SpanBound { start: 0, end: 30 },
            verifiability: VerifiabilityClass::Numeric,
            verifiability_score: 0.91,
        };
        let json = serde_json::to_string(&claim).unwrap();
        let back: AtomicClaim = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, 12345678);
        assert_eq!(back.verifiability, VerifiabilityClass::Numeric);
    }

    #[test]
    fn claim_verdict_abstain_is_not_supported() {
        let v = ClaimVerdict::Abstain {
            reason: "below tau".to_string(),
        };
        assert!(!v.is_supported());
    }
}
