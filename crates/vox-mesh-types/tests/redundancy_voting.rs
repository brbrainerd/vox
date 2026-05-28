//! Integration tests for RedundancyPolicy and BOINC-style adaptive voting (P6-T4).

use vox_mesh_types::redundancy::{
    RedundancyMode, RedundancyPolicy, TrustTier, VoteOutcome, decide_replicas,
    decide_replicas_with_seed, vote_majority,
};

fn make_policy(mode: RedundancyMode, min: u8, max: u8) -> RedundancyPolicy {
    RedundancyPolicy {
        mode,
        min_replicas: min,
        max_replicas: max,
        skip_above: None,
        determinism_proof_blake3_hex: None,
    }
}

// ---------------------------------------------------------------------------
// decide_replicas
// ---------------------------------------------------------------------------

#[test]
fn replicas_none_mode_returns_min() {
    let p = make_policy(RedundancyMode::None, 1, 1);
    assert_eq!(decide_replicas(&p, TrustTier::Unknown), 1);
}

#[test]
fn replicas_majority_mode_returns_min() {
    let p = make_policy(RedundancyMode::Majority, 3, 5);
    assert_eq!(decide_replicas(&p, TrustTier::Attested), 3);
}

#[test]
fn replicas_skipped_for_trusted_tier() {
    let mut p = make_policy(RedundancyMode::Majority, 3, 5);
    p.skip_above = Some(TrustTier::Vetted);

    // Below skip threshold: replicas = min.
    assert_eq!(decide_replicas(&p, TrustTier::Attested), 3);
    // At skip threshold: only 1 replica (trust bypasses redundancy).
    assert_eq!(decide_replicas(&p, TrustTier::Vetted), 1);
    // Above skip threshold: still only 1.
    assert_eq!(decide_replicas(&p, TrustTier::Internal), 1);
}

#[test]
fn replicas_min_clamped_to_one() {
    let p = make_policy(RedundancyMode::Race, 0, 3);
    // min_replicas = 0 → clamped to 1
    assert_eq!(decide_replicas(&p, TrustTier::Unknown), 1);
}

#[test]
fn replicas_with_seed_matches_replicas() {
    let p = make_policy(RedundancyMode::Adaptive, 2, 4);
    assert_eq!(
        decide_replicas_with_seed(&p, TrustTier::Reputable, 42),
        decide_replicas(&p, TrustTier::Reputable)
    );
}

// ---------------------------------------------------------------------------
// vote_majority
// ---------------------------------------------------------------------------

#[test]
fn vote_empty_returns_no_votes() {
    assert_eq!(vote_majority(&[]), VoteOutcome::NoVotes);
}

#[test]
fn vote_single_returns_consensus() {
    let outputs = vec![("n1".to_string(), "aabbcc".to_string())];
    let outcome = vote_majority(&outputs);
    assert_eq!(
        outcome,
        VoteOutcome::Consensus {
            output_blake3_hex: "aabbcc".to_string()
        }
    );
}

#[test]
fn vote_all_agree_returns_consensus() {
    let outputs = vec![
        ("n1".to_string(), "deadbeef".to_string()),
        ("n2".to_string(), "deadbeef".to_string()),
        ("n3".to_string(), "deadbeef".to_string()),
    ];
    assert_eq!(
        vote_majority(&outputs),
        VoteOutcome::Consensus {
            output_blake3_hex: "deadbeef".to_string()
        }
    );
}

#[test]
fn vote_majority_two_of_three() {
    let outputs = vec![
        ("n1".to_string(), "hash_a".to_string()),
        ("n2".to_string(), "hash_a".to_string()),
        ("n3".to_string(), "hash_b".to_string()),
    ];
    let outcome = vote_majority(&outputs);
    assert_eq!(
        outcome,
        VoteOutcome::Majority {
            output_blake3_hex: "hash_a".to_string(),
            minority_count: 1,
        }
    );
}

#[test]
fn vote_split_no_majority() {
    let outputs = vec![
        ("n1".to_string(), "hash_x".to_string()),
        ("n2".to_string(), "hash_y".to_string()),
    ];
    let outcome = vote_majority(&outputs);
    match outcome {
        VoteOutcome::Split { counts, .. } => {
            assert_eq!(counts.len(), 2);
        }
        other => panic!("expected Split, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Policy round-trip
// ---------------------------------------------------------------------------

#[test]
fn policy_round_trip_json() {
    let p = RedundancyPolicy {
        mode: RedundancyMode::Adaptive,
        min_replicas: 2,
        max_replicas: 5,
        skip_above: Some(TrustTier::Vetted),
        determinism_proof_blake3_hex: Some("cafebabe".to_string()),
    };
    let json = serde_json::to_string(&p).unwrap();
    let decoded: RedundancyPolicy = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded.mode, RedundancyMode::Adaptive);
    assert_eq!(decoded.min_replicas, 2);
    assert_eq!(decoded.skip_above, Some(TrustTier::Vetted));
}

#[test]
fn donation_policy_accepts_redundancy_field() {
    use vox_mesh_types::donation_policy::WorkerDonationPolicy;
    let json = r#"{
        "slots": [],
        "nsfw_allowed": false,
        "max_job_duration_secs": 300,
        "public_mesh_opt_in": false,
        "min_priority": 128,
        "redundancy": {
            "mode": "majority",
            "min_replicas": 3,
            "max_replicas": 5
        }
    }"#;
    let p: WorkerDonationPolicy = serde_json::from_str(json).unwrap();
    assert!(p.redundancy.is_some());
    assert_eq!(p.redundancy.unwrap().min_replicas, 3);
}

#[test]
fn donation_policy_redundancy_defaults_to_none() {
    use vox_mesh_types::donation_policy::WorkerDonationPolicy;
    let json = r#"{
        "slots": [],
        "nsfw_allowed": false,
        "max_job_duration_secs": 300,
        "public_mesh_opt_in": false,
        "min_priority": 128
    }"#;
    let p: WorkerDonationPolicy = serde_json::from_str(json).unwrap();
    assert!(p.redundancy.is_none());
}
