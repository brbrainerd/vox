use serde::{Deserialize, Serialize};

use crate::attestation::Attestation;

/// Primitives for the contribution reward system.
/// Collapses compute donation and code contribution into one system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewardPrimitive {
    /// 1ms of GPU compute (adjusted by VRAM weight).
    GpuComputeMs,
    /// 1ms of CPU compute.
    CpuComputeMs,
    /// One successful result attestation.
    ResultAttestation,
    /// One peer-reviewed code contribution.
    CodeContribution,
    /// One peer-reviewed bug fix.
    BugFix,
    /// One peer-reviewed documentation improvement.
    DocsContribution,
}

impl RewardPrimitive {
    /// Return the human-readable slug for database storage.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::GpuComputeMs => "gpu_compute_ms",
            Self::CpuComputeMs => "cpu_compute_ms",
            Self::ResultAttestation => "result_attestation",
            Self::CodeContribution => "code_contribution",
            Self::BugFix => "bug_fix",
            Self::DocsContribution => "docs_contribution",
        }
    }
}

/// Convert an `Attestation`'s `gpu_seconds` into integer milliseconds for the
/// `GpuComputeMs` kudos primitive.
///
/// The conversion is `(gpu_seconds * 1000.0) as u64`, saturating at `u64::MAX`.
pub fn gpu_compute_ms_from_attestation(a: &Attestation) -> u64 {
    (a.gpu_seconds * 1000.0).min(u64::MAX as f64) as u64
}

/// Request to credit a user for a contribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreditJobRequest {
    pub vox_user_id: String,
    pub node_id: String,
    pub primitive: RewardPrimitive,
    pub amount: u64,
    pub task_id: Option<String>,
    pub metadata_json: Option<String>,
}

#[cfg(test)]
mod semcov_wave8_tests {
    #![allow(unused_imports, dead_code)]
    use super::*;
    use crate::attestation::Attestation;

    fn make_attestation(gpu_seconds: f64) -> Attestation {
        Attestation {
            task_id: "t1".into(),
            input_hash_blake3_hex: "aa".into(),
            output_hash_blake3_hex: "bb".into(),
            gpu_seconds,
            trace_blake3_hex: None,
            ephemeral_pubkey_hex: "cc".into(),
            signature_b64: "dd".into(),
            signed_at_unix_ms: 0,
            tee_quote: None,
            replay_proof_blake3_hex: None,
            kudos_signature_b64: None,
        }
    }

    // Catches: gpu_compute_ms_from_attestation returning 0 for a non-zero gpu_seconds (e.g., wrong unit).
    #[test]
    fn one_second_is_1000_ms() {
        let a = make_attestation(1.0);
        assert_eq!(gpu_compute_ms_from_attestation(&a), 1000);
    }

    // Catches: fractional seconds being truncated to 0 instead of rounded.
    #[test]
    fn fractional_second_rounds_to_positive_ms() {
        let a = make_attestation(0.001);
        assert_eq!(gpu_compute_ms_from_attestation(&a), 1, "0.001s must be 1ms");
    }

    // Catches: zero gpu_seconds producing non-zero ms (e.g., off-by-one bias).
    #[test]
    fn zero_gpu_seconds_is_zero_ms() {
        let a = make_attestation(0.0);
        assert_eq!(gpu_compute_ms_from_attestation(&a), 0);
    }

    // Catches: f64 Inf from very large gpu_seconds overflowing the u64 cast and panicking.
    #[test]
    fn very_large_gpu_seconds_does_not_panic() {
        let a = make_attestation(f64::MAX);
        let ms = gpu_compute_ms_from_attestation(&a);
        // Should saturate at u64::MAX, not panic.
        assert_eq!(ms, u64::MAX);
    }

    // Catches: RewardPrimitive::as_str returning wrong slug (e.g., swapping two variants).
    #[test]
    fn reward_primitive_as_str_slug_correctness() {
        assert_eq!(RewardPrimitive::GpuComputeMs.as_str(), "gpu_compute_ms");
        assert_eq!(RewardPrimitive::CpuComputeMs.as_str(), "cpu_compute_ms");
        assert_eq!(
            RewardPrimitive::ResultAttestation.as_str(),
            "result_attestation"
        );
        assert_eq!(
            RewardPrimitive::CodeContribution.as_str(),
            "code_contribution"
        );
        assert_eq!(RewardPrimitive::BugFix.as_str(), "bug_fix");
        assert_eq!(
            RewardPrimitive::DocsContribution.as_str(),
            "docs_contribution"
        );
    }

    // Catches: RewardPrimitive round-tripping serde with wrong variant name.
    #[test]
    fn reward_primitive_serde_round_trip() {
        for prim in [
            RewardPrimitive::GpuComputeMs,
            RewardPrimitive::BugFix,
            RewardPrimitive::DocsContribution,
        ] {
            let json = serde_json::to_string(&prim).unwrap();
            let back: RewardPrimitive = serde_json::from_str(&json).unwrap();
            assert_eq!(back, prim);
        }
    }
}
