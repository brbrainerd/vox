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
