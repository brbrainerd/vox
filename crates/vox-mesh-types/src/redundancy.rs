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
