//! Campaign gate — the orchestrator enforcement point for pre-registration.
//!
//! Per SCIENTIA plan §5.1: "The orchestrator **refuses to run a campaign without
//! a signed prereg**."
//!
//! [`PreregGate::check_campaign`] is a synchronous call the orchestrator makes
//! in its campaign-dispatch path before allocating any compute budget.

use crate::preregistration::signing::verify_prereg;
use vox_research_events::preregistration::PreregistrationV1;

/// Result of [`PreregGate::check_campaign`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateResult {
    /// The campaign is approved to proceed.
    Approved,
    /// The campaign is refused; `reason` is a human-readable explanation.
    Refused { reason: String },
}

/// Enforces pre-registration requirements before a campaign may start.
#[derive(Debug, Default, Clone)]
pub struct PreregGate;

impl PreregGate {
    pub fn new() -> Self {
        Self
    }

    /// Check whether a campaign may proceed.
    ///
    /// # Refusal conditions
    /// - `prereg` is `None` → refused with "no preregistration provided"
    /// - `signature_hex` is `None` → refused with "no signature provided"
    /// - signature verification fails → refused with the verification error
    pub fn check_campaign(
        &self,
        prereg: Option<&PreregistrationV1>,
        signature_hex: Option<&str>,
    ) -> GateResult {
        let prereg = match prereg {
            Some(p) => p,
            None => {
                return GateResult::Refused {
                    reason: "no preregistration provided; campaigns require a signed prereg before data collection".to_string(),
                }
            }
        };

        let sig = match signature_hex {
            Some(s) => s,
            None => return GateResult::Refused {
                reason:
                    "no signature provided; the preregistration must be signed with an Ed25519 key"
                        .to_string(),
            },
        };

        match verify_prereg(prereg, sig) {
            Ok(()) => GateResult::Approved,
            Err(e) => GateResult::Refused {
                reason: format!("invalid signature: {e}"),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preregistration::signing::sign_prereg;
    use vox_crypto::facades::generate_signing_keypair;
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    fn draft_prereg() -> PreregistrationV1 {
        PreregistrationV1 {
            id: String::new(),
            hypothesis: "JSON-mode violation rate rose after provider update".to_string(),
            eval_substrate: SubstrateRef {
                repo_swhid: "swh:1:rev:aabbcc".to_string(),
                eval_set_swhid: "swh:1:dir:ddeeff".to_string(),
                inspect_task_id: None,
            },
            metric: MetricSpec {
                name: "json_violation_rate_pct".to_string(),
                aggregation: "mean".to_string(),
                units: "percent".to_string(),
            },
            statistical_test: TestSpec {
                kind: StatisticalTest::Bayesian,
                prior: Some("Beta(1,1)".to_string()),
                threshold: Some(0.95),
                alpha: None,
            },
            stopping_rule: StopRule {
                max_n: 300,
                alpha: None,
                threshold: Some(0.95),
            },
            decision_rule: DecisionRule {
                description: "if posterior P(increase) > 0.95, flag provider".to_string(),
            },
            cost_cap_usd: 15.0,
            signed_at: 0,
            signing_key: String::new(),
            supersedes: None,
            analysis_tree_commit: None,
        }
    }

    #[test]
    fn approved_with_valid_signed_prereg() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        let sig = sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let result = gate.check_campaign(Some(&prereg), Some(&sig.0));
        assert_eq!(
            result,
            GateResult::Approved,
            "valid signed prereg must be approved"
        );
    }

    #[test]
    fn refused_without_prereg() {
        let gate = PreregGate::new();
        let result = gate.check_campaign(None, None);
        assert!(
            matches!(result, GateResult::Refused { .. }),
            "missing prereg must be refused"
        );
        if let GateResult::Refused { reason } = result {
            assert!(
                reason.contains("preregistration"),
                "reason must mention preregistration"
            );
        }
    }

    #[test]
    fn refused_without_signature() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        // Pass prereg but no signature
        let result = gate.check_campaign(Some(&prereg), None);
        assert!(
            matches!(result, GateResult::Refused { .. }),
            "missing signature must be refused"
        );
        if let GateResult::Refused { reason } = result {
            assert!(
                reason.contains("signature"),
                "reason must mention signature"
            );
        }
    }

    #[test]
    fn refused_with_bad_signature() {
        let gate = PreregGate::new();
        let (sk, _vk) = generate_signing_keypair();
        let mut prereg = draft_prereg();
        sign_prereg(&mut prereg, &sk).expect("signing must succeed");
        let bad_sig = "00".repeat(64);
        let result = gate.check_campaign(Some(&prereg), Some(&bad_sig));
        assert!(
            matches!(result, GateResult::Refused { .. }),
            "bad signature must be refused"
        );
        if let GateResult::Refused { reason } = result {
            assert!(
                reason.contains("signature"),
                "reason must mention signature"
            );
        }
    }
}
