//! Integration tests for the `research_gate` module.
//!
//! Verifies that `vox-prereg` is correctly wired into `vox-orchestrator` and
//! that the convenience helper and re-exported types are reachable.

use vox_orchestrator::research_gate::{GateResult, PreregGate, check_campaign_prereg};

/// A campaign with no prereg and no signature must be refused.
#[test]
fn refused_when_no_prereg_provided() {
    let result = check_campaign_prereg(None, None);
    assert!(
        matches!(result, GateResult::Refused { .. }),
        "check_campaign_prereg(None, None) must return Refused"
    );
    if let GateResult::Refused { reason } = result {
        assert!(
            reason.contains("preregistration"),
            "refusal reason must mention 'preregistration', got: {reason}"
        );
    }
}

/// A campaign with a prereg but no signature must be refused.
#[test]
fn refused_when_no_signature_provided() {
    use vox_research_events::preregistration::{
        DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef,
        TestSpec,
    };

    let prereg = PreregistrationV1 {
        id: String::new(),
        hypothesis: "test hypothesis".to_string(),
        eval_substrate: SubstrateRef {
            repo_swhid: "swh:1:rev:aabbcc".to_string(),
            eval_set_swhid: "swh:1:dir:ddeeff".to_string(),
            inspect_task_id: None,
        },
        metric: MetricSpec {
            name: "error_rate".to_string(),
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
            max_n: 100,
            alpha: None,
            threshold: Some(0.95),
        },
        decision_rule: DecisionRule {
            description: "if posterior P(increase) > 0.95, flag".to_string(),
        },
        cost_cap_usd: 5.0,
        signed_at: 0,
        signing_key: String::new(),
        supersedes: None,
        analysis_tree_commit: None,
    };

    let result = check_campaign_prereg(Some(&prereg), None);
    assert!(
        matches!(result, GateResult::Refused { .. }),
        "missing signature must be refused"
    );
    if let GateResult::Refused { reason } = result {
        assert!(
            reason.contains("signature"),
            "refusal reason must mention 'signature', got: {reason}"
        );
    }
}

/// `PreregGate` is directly re-exported and usable.
#[test]
fn prerereg_gate_type_is_reachable() {
    let gate = PreregGate::new();
    let result = gate.check_campaign(None, None);
    assert!(
        matches!(result, GateResult::Refused { .. }),
        "PreregGate::check_campaign(None, None) must be Refused"
    );
}
