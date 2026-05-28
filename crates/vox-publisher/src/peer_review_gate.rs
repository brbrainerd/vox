//! F1000-style peer review gate: ≥2 distinct signed approvals before indexing.

use sha3::{Digest, Sha3_256};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approve,
    Reject,
    RequestRevision,
}

/// A single signed peer review.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PeerReview {
    pub reviewer_key_hex: String,
    pub publication_digest: String,
    pub decision: ReviewDecision,
    pub rationale: Option<String>,
    pub signed_at: i64,
    /// SHA3-256 hex of `canonical_review_payload(...)` — tamper detection.
    pub signature_hex: String,
}

#[derive(Debug)]
pub enum PeerReviewGateError {
    InvalidSignature {
        reviewer_key_hex: String,
    },
    InsufficientApprovals {
        got: usize,
        need: usize,
    },
    DigestMismatch {
        reviewer_key_hex: String,
    },
    Rejected {
        reviewer_key_hex: String,
        rationale: Option<String>,
    },
}

impl std::fmt::Display for PeerReviewGateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSignature { reviewer_key_hex } => {
                write!(f, "invalid signature from reviewer {reviewer_key_hex}")
            }
            Self::InsufficientApprovals { got, need } => {
                write!(f, "need {need} approvals, got {got}")
            }
            Self::DigestMismatch { reviewer_key_hex } => {
                write!(f, "digest mismatch in review from {reviewer_key_hex}")
            }
            Self::Rejected {
                reviewer_key_hex,
                rationale,
            } => {
                write!(f, "rejected by {reviewer_key_hex}: {:?}", rationale)
            }
        }
    }
}

impl std::error::Error for PeerReviewGateError {}

/// Gate requiring at least `min_approvals` distinct signed reviews, no rejections.
pub struct PeerReviewGate {
    pub min_approvals: usize,
}

impl Default for PeerReviewGate {
    fn default() -> Self {
        Self { min_approvals: 2 }
    }
}

impl PeerReviewGate {
    pub fn new(min_approvals: usize) -> Self {
        Self { min_approvals }
    }

    /// Check that `reviews` contains sufficient approvals for `publication_digest`.
    ///
    /// Returns `Ok(())` if the gate passes. Returns the first blocking error otherwise.
    pub fn check(
        &self,
        publication_digest: &str,
        reviews: &[PeerReview],
    ) -> Result<(), PeerReviewGateError> {
        let mut approvals: Vec<&str> = Vec::new();
        for review in reviews {
            // 1. Digest must match.
            if review.publication_digest != publication_digest {
                return Err(PeerReviewGateError::DigestMismatch {
                    reviewer_key_hex: review.reviewer_key_hex.clone(),
                });
            }
            // 2. Signature must be valid.
            if !verify_review(review) {
                return Err(PeerReviewGateError::InvalidSignature {
                    reviewer_key_hex: review.reviewer_key_hex.clone(),
                });
            }
            // 3. Rejections block immediately.
            if review.decision == ReviewDecision::Reject {
                return Err(PeerReviewGateError::Rejected {
                    reviewer_key_hex: review.reviewer_key_hex.clone(),
                    rationale: review.rationale.clone(),
                });
            }
            if review.decision == ReviewDecision::Approve {
                approvals.push(&review.reviewer_key_hex);
            }
        }
        // 4. Sufficient distinct approvals.
        let distinct = count_distinct(&approvals);
        if distinct < self.min_approvals {
            return Err(PeerReviewGateError::InsufficientApprovals {
                got: distinct,
                need: self.min_approvals,
            });
        }
        Ok(())
    }
}

/// Canonical payload string for signing a review.
pub fn canonical_review_payload(
    reviewer_key_hex: &str,
    publication_digest: &str,
    decision: &ReviewDecision,
    signed_at: i64,
) -> String {
    let decision_str = match decision {
        ReviewDecision::Approve => "approve",
        ReviewDecision::Reject => "reject",
        ReviewDecision::RequestRevision => "request_revision",
    };
    format!(
        "reviewer={reviewer_key_hex}\npub_digest={publication_digest}\ndecision={decision_str}\nsigned_at={signed_at}"
    )
}

pub(crate) fn sha3_hex(s: &str) -> String {
    let mut h = Sha3_256::new();
    h.update(s.as_bytes());
    hex::encode(h.finalize())
}

fn verify_review(review: &PeerReview) -> bool {
    let expected = canonical_review_payload(
        &review.reviewer_key_hex,
        &review.publication_digest,
        &review.decision,
        review.signed_at,
    );
    let expected_sig = sha3_hex(&expected);
    review.signature_hex == expected_sig
}

fn count_distinct(keys: &[&str]) -> usize {
    let mut seen: Vec<&str> = Vec::new();
    for k in keys {
        if !seen.contains(k) {
            seen.push(k);
        }
    }
    seen.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_review(digest: &str, decision: ReviewDecision) -> PeerReview {
        let key = format!(
            "reviewer-key-{}",
            match decision {
                ReviewDecision::Approve => "a",
                ReviewDecision::Reject => "r",
                ReviewDecision::RequestRevision => "rev",
            }
        );
        let payload = canonical_review_payload(&key, digest, &decision, 0);
        let sig = sha3_hex(&payload);
        PeerReview {
            reviewer_key_hex: key,
            publication_digest: digest.to_string(),
            decision,
            rationale: None,
            signed_at: 0,
            signature_hex: sig,
        }
    }

    #[test]
    fn two_approvals_passes() {
        let gate = PeerReviewGate::default();
        let reviews = vec![make_review("digest-abc", ReviewDecision::Approve), {
            let key = "reviewer-key-b".to_string();
            let decision = ReviewDecision::Approve;
            let payload = canonical_review_payload(&key, "digest-abc", &decision, 0);
            let sig = sha3_hex(&payload);
            PeerReview {
                reviewer_key_hex: key,
                publication_digest: "digest-abc".to_string(),
                decision,
                rationale: None,
                signed_at: 0,
                signature_hex: sig,
            }
        }];
        assert!(gate.check("digest-abc", &reviews).is_ok());
    }

    #[test]
    fn one_approval_fails() {
        let gate = PeerReviewGate::default();
        let reviews = vec![make_review("digest-abc", ReviewDecision::Approve)];
        let err = gate.check("digest-abc", &reviews).unwrap_err();
        assert!(matches!(
            err,
            PeerReviewGateError::InsufficientApprovals { .. }
        ));
    }

    #[test]
    fn rejection_blocks_gate() {
        let gate = PeerReviewGate::default();
        let reviews = vec![
            make_review("digest-abc", ReviewDecision::Approve),
            make_review("digest-abc", ReviewDecision::Reject),
        ];
        let err = gate.check("digest-abc", &reviews).unwrap_err();
        assert!(matches!(err, PeerReviewGateError::Rejected { .. }));
    }

    #[test]
    fn digest_mismatch_is_rejected() {
        let gate = PeerReviewGate::default();
        let mut r = make_review("digest-abc", ReviewDecision::Approve);
        r.publication_digest = "digest-WRONG".to_string();
        let reviews = vec![r];
        let err = gate.check("digest-abc", &reviews).unwrap_err();
        assert!(matches!(err, PeerReviewGateError::DigestMismatch { .. }));
    }

    #[test]
    fn tampered_signature_fails() {
        let gate = PeerReviewGate::default();
        let mut r = make_review("digest-abc", ReviewDecision::Approve);
        r.signature_hex = "deadbeef".to_string();
        let err = gate.check("digest-abc", &[r]).unwrap_err();
        assert!(matches!(err, PeerReviewGateError::InvalidSignature { .. }));
    }
}
