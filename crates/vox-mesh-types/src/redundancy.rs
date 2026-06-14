//! Redundancy policy and BOINC-style adaptive replication (P6-T4).
//!
//! `RedundancyPolicy` controls how many independent replicas of a
//! declared-deterministic task are dispatched, and how results are
//! reconciled by majority vote.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Trust tier assigned to a peer node.
///
/// Higher tiers mean more trust; the policy can skip redundant execution for
/// sufficiently trusted peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrustTier {
    /// Unknown / unauthenticated peer.
    Unknown = 0,
    /// Peer has a valid GitHub-attested manifest but no track record.
    Attested = 1,
    /// Peer has a positive reputation score (>= 10 successes, < 5% failure rate).
    Reputable = 2,
    /// Vetted peer (known operator with signed identity and long track record).
    Vetted = 3,
    /// Internal / same-mesh peer.
    Internal = 4,
}

/// Redundancy dispatch mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RedundancyMode {
    /// Dispatch a single replica (no redundancy).
    None,
    /// Dispatch N replicas and take the first successful result.
    Race,
    /// Dispatch N replicas and return only when a majority agree.
    Majority,
    /// Adaptive: start at `min_replicas`, increase on mismatch (BOINC-style).
    Adaptive,
}

// ---------------------------------------------------------------------------
// Policy
// ---------------------------------------------------------------------------

/// Redundancy policy attached to a `WorkerDonationPolicy` or a task spec.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RedundancyPolicy {
    /// Dispatch mode.
    pub mode: RedundancyMode,
    /// Minimum number of replicas to dispatch.
    pub min_replicas: u8,
    /// Maximum number of replicas allowed (caps adaptive growth).
    pub max_replicas: u8,
    /// Trust tier at or above which redundancy is skipped entirely.
    /// A peer at `skip_above` or higher is trusted to run without a duplicate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_above: Option<TrustTier>,
    /// BLAKE3 hex digest of the task determinism proof (set for declared-deterministic tasks).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub determinism_proof_blake3_hex: Option<String>,
}

impl Default for RedundancyPolicy {
    fn default() -> Self {
        Self {
            mode: RedundancyMode::None,
            min_replicas: 1,
            max_replicas: 1,
            skip_above: None,
            determinism_proof_blake3_hex: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Voting
// ---------------------------------------------------------------------------

/// Outcome of a majority vote over replica outputs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoteOutcome {
    /// All replicas agreed — carries the winning output hash.
    Consensus { output_blake3_hex: String },
    /// A majority agreed — carries the winning output hash and minority count.
    Majority {
        output_blake3_hex: String,
        minority_count: usize,
    },
    /// No majority reached — carries the most common hash and split counts.
    Split {
        most_common_blake3_hex: String,
        counts: Vec<(String, usize)>,
    },
    /// No outputs provided.
    NoVotes,
}

/// Decide how many replicas to dispatch given a policy and the peer's trust tier.
pub fn decide_replicas(policy: &RedundancyPolicy, peer_tier: TrustTier) -> u8 {
    if let Some(skip) = policy.skip_above
        && peer_tier >= skip
    {
        return 1;
    }
    policy.min_replicas.max(1)
}

/// Seeded variant for deterministic testing.
///
/// `_seed` is reserved for future adaptive logic that randomises peer selection
/// in the N-replica set to avoid correlated failures.
pub fn decide_replicas_with_seed(
    policy: &RedundancyPolicy,
    peer_tier: TrustTier,
    _seed: u64,
) -> u8 {
    decide_replicas(policy, peer_tier)
}

/// Vote on a set of output BLAKE3 digests and return the outcome.
///
/// `outputs` is a slice of `(node_id, output_blake3_hex)` pairs. The vote
/// picks the most common hash; ties are reported as `Split`.
pub fn vote_majority(outputs: &[(String, String)]) -> VoteOutcome {
    if outputs.is_empty() {
        return VoteOutcome::NoVotes;
    }

    // Count occurrences of each unique hash.
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for (_, hash) in outputs {
        *counts.entry(hash.as_str()).or_insert(0) += 1;
    }

    let total = outputs.len();
    let majority_threshold = total / 2 + 1;

    // Find the most common.
    let mut sorted: Vec<(&str, usize)> = counts.into_iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));

    let (winner_hash, winner_count) = sorted[0];

    if winner_count == total {
        VoteOutcome::Consensus {
            output_blake3_hex: winner_hash.to_string(),
        }
    } else if winner_count >= majority_threshold {
        VoteOutcome::Majority {
            output_blake3_hex: winner_hash.to_string(),
            minority_count: total - winner_count,
        }
    } else {
        VoteOutcome::Split {
            most_common_blake3_hex: winner_hash.to_string(),
            counts: sorted
                .into_iter()
                .map(|(h, c)| (h.to_string(), c))
                .collect(),
        }
    }
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;

    fn pair(node: &str, hash: &str) -> (String, String) {
        (node.to_string(), hash.to_string())
    }

    // Catches: vote_majority returning Consensus for a single replica instead of Consensus (trivially
    // correct but should be Consensus, not Majority with minority=0 which would be confusing).
    #[test]
    fn single_replica_is_consensus() {
        let out = vote_majority(&[pair("n1", "aabbcc")]);
        assert!(
            matches!(out, VoteOutcome::Consensus { .. }),
            "single replica must be Consensus"
        );
    }

    // Catches: empty outputs returning something other than NoVotes (e.g., panicking on sorted[0]).
    #[test]
    fn empty_outputs_returns_no_votes() {
        assert!(matches!(vote_majority(&[]), VoteOutcome::NoVotes));
    }

    // Catches: exact tie (2 distinct hashes, 1 vote each) being reported as Majority instead of Split.
    #[test]
    fn two_way_tie_is_split() {
        let out = vote_majority(&[pair("n1", "aaa"), pair("n2", "bbb")]);
        assert!(
            matches!(out, VoteOutcome::Split { .. }),
            "exact tie must be Split, not Majority"
        );
    }

    // Catches: 3 nodes, 2 agreeing, 1 not — being reported as Consensus or Split instead of Majority.
    #[test]
    fn two_of_three_is_majority() {
        let out = vote_majority(&[pair("n1", "aaa"), pair("n2", "aaa"), pair("n3", "bbb")]);
        match out {
            VoteOutcome::Majority {
                output_blake3_hex,
                minority_count,
            } => {
                assert_eq!(output_blake3_hex, "aaa");
                assert_eq!(minority_count, 1);
            }
            other => panic!("expected Majority, got {other:?}"),
        }
    }

    // Catches: all-agreeing replicas producing Majority (minority_count=0) instead of Consensus.
    #[test]
    fn all_agree_is_consensus_not_majority() {
        let out = vote_majority(&[pair("n1", "aaa"), pair("n2", "aaa"), pair("n3", "aaa")]);
        assert!(matches!(out, VoteOutcome::Consensus { output_blake3_hex } if output_blake3_hex == "aaa"));
    }

    // Catches: decide_replicas ignoring min_replicas=0 and returning 0 instead of clamping to 1.
    #[test]
    fn decide_replicas_clamps_min_to_one() {
        let policy = RedundancyPolicy {
            mode: RedundancyMode::None,
            min_replicas: 0,
            max_replicas: 1,
            skip_above: None,
            determinism_proof_blake3_hex: None,
        };
        assert_eq!(decide_replicas(&policy, TrustTier::Unknown), 1);
    }

    // Catches: skip_above threshold applying when peer_tier is BELOW skip_above (should not skip).
    #[test]
    fn skip_above_not_triggered_below_threshold() {
        let policy = RedundancyPolicy {
            mode: RedundancyMode::Majority,
            min_replicas: 3,
            max_replicas: 5,
            skip_above: Some(TrustTier::Vetted),
            determinism_proof_blake3_hex: None,
        };
        // Reputable < Vetted, so skip should NOT trigger — min_replicas should apply.
        assert_eq!(decide_replicas(&policy, TrustTier::Reputable), 3);
    }

    // Catches: skip_above exactly at threshold not triggering skip (should skip AT or ABOVE).
    #[test]
    fn skip_above_exact_match_triggers_skip() {
        let policy = RedundancyPolicy {
            mode: RedundancyMode::Majority,
            min_replicas: 3,
            max_replicas: 5,
            skip_above: Some(TrustTier::Vetted),
            determinism_proof_blake3_hex: None,
        };
        assert_eq!(
            decide_replicas(&policy, TrustTier::Vetted),
            1,
            "peer at exactly skip_above threshold must skip to 1 replica"
        );
    }

    // Catches: TrustTier ordering being wrong (e.g., Internal < Vetted when it should be higher).
    #[test]
    fn trust_tier_ordering_is_monotone() {
        assert!(TrustTier::Unknown < TrustTier::Attested);
        assert!(TrustTier::Attested < TrustTier::Reputable);
        assert!(TrustTier::Reputable < TrustTier::Vetted);
        assert!(TrustTier::Vetted < TrustTier::Internal);
    }

    // Catches: RedundancyPolicy serde round-trip dropping skip_above when it's Some.
    #[test]
    fn redundancy_policy_round_trips_with_skip_above() {
        let policy = RedundancyPolicy {
            mode: RedundancyMode::Adaptive,
            min_replicas: 2,
            max_replicas: 8,
            skip_above: Some(TrustTier::Reputable),
            determinism_proof_blake3_hex: Some("deadbeef".to_string()),
        };
        let json = serde_json::to_string(&policy).unwrap();
        let back: RedundancyPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(back.skip_above, Some(TrustTier::Reputable));
        assert_eq!(back.min_replicas, 2);
        assert_eq!(back.max_replicas, 8);
    }
}
