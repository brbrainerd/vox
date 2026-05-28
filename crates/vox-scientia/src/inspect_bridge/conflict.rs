//! EvidenceConflict detection for opposing-polarity high-similarity hits.
//!
//! A conflict is flagged when the filtered hit set contains BOTH supporting
//! and contradicting hits (hits with similarity >= `similarity_threshold`).

use serde::{Deserialize, Serialize};

/// Polarity of a retrieved piece of evidence relative to the claim direction.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ClaimPolarity {
    Positive,
    Negative,
    Neutral,
}

/// A retrieved hit annotated with its claim polarity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolarizedHit {
    pub work_uri: String,
    pub similarity: f64,
    pub polarity: ClaimPolarity,
    pub excerpt: Option<String>,
}

/// A detected conflict between supporting and contradicting evidence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceConflict {
    pub claim_text: String,
    pub supporting_hits: Vec<PolarizedHit>,
    pub contradicting_hits: Vec<PolarizedHit>,
    /// Severity: `min(supporting, contradicting) / total_high_similarity_hits` (0.0–1.0).
    pub conflict_score: f64,
}

/// Detects `EvidenceConflict`s among a set of `PolarizedHit`s.
pub struct EvidenceConflictDetector {
    /// Only hits with `similarity >= similarity_threshold` are considered.
    pub similarity_threshold: f64,
}

impl Default for EvidenceConflictDetector {
    fn default() -> Self {
        Self {
            similarity_threshold: 0.8,
        }
    }
}

impl EvidenceConflictDetector {
    pub fn new(similarity_threshold: f64) -> Self {
        Self {
            similarity_threshold,
        }
    }

    /// Examine `hits` for an opposing-polarity conflict.
    ///
    /// Returns `Some(EvidenceConflict)` if and only if the filtered set contains
    /// at least one `Positive` and at least one `Negative` hit.
    /// `Neutral` hits are included in neither bucket.
    pub fn detect(&self, claim_text: &str, hits: &[PolarizedHit]) -> Option<EvidenceConflict> {
        let high_sim: Vec<&PolarizedHit> = hits
            .iter()
            .filter(|h| h.similarity >= self.similarity_threshold)
            .collect();

        let supporting: Vec<PolarizedHit> = high_sim
            .iter()
            .filter(|h| h.polarity == ClaimPolarity::Positive)
            .map(|h| (*h).clone())
            .collect();

        let contradicting: Vec<PolarizedHit> = high_sim
            .iter()
            .filter(|h| h.polarity == ClaimPolarity::Negative)
            .map(|h| (*h).clone())
            .collect();

        if supporting.is_empty() || contradicting.is_empty() {
            return None;
        }

        let total = high_sim.len() as f64;
        let conflict_score = supporting.len().min(contradicting.len()) as f64 / total;

        Some(EvidenceConflict {
            claim_text: claim_text.to_string(),
            supporting_hits: supporting,
            contradicting_hits: contradicting,
            conflict_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(uri: &str, similarity: f64, polarity: ClaimPolarity) -> PolarizedHit {
        PolarizedHit {
            work_uri: uri.to_string(),
            similarity,
            polarity,
            excerpt: None,
        }
    }

    #[test]
    fn no_conflict_when_all_supporting() {
        let hits = vec![
            hit("doi:10.1", 0.9, ClaimPolarity::Positive),
            hit("doi:10.2", 0.85, ClaimPolarity::Positive),
        ];
        let detector = EvidenceConflictDetector::default();
        assert!(detector.detect("some claim", &hits).is_none());
    }

    #[test]
    fn conflict_detected_when_opposing_polarity() {
        let hits = vec![
            hit("doi:10.1", 0.9, ClaimPolarity::Positive),
            hit("doi:10.2", 0.85, ClaimPolarity::Negative),
        ];
        let detector = EvidenceConflictDetector::default();
        let conflict = detector.detect("some claim", &hits);
        assert!(conflict.is_some());
        let c = conflict.unwrap();
        assert_eq!(c.supporting_hits.len(), 1);
        assert_eq!(c.contradicting_hits.len(), 1);
        // conflict_score = min(1,1)/2 = 0.5
        assert!((c.conflict_score - 0.5).abs() < 1e-9);
    }

    #[test]
    fn low_similarity_hits_ignored() {
        let hits = vec![
            hit("doi:10.1", 0.9, ClaimPolarity::Positive),
            // Below threshold — should not be counted as contradicting.
            hit("doi:10.2", 0.5, ClaimPolarity::Negative),
        ];
        let detector = EvidenceConflictDetector::default(); // threshold = 0.8
        assert!(detector.detect("some claim", &hits).is_none());
    }
}
