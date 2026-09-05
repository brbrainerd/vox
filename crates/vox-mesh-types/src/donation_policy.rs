use crate::task::TaskKind;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DonationSlot {
    pub task_kind: TaskKind,
    pub max_concurrent: u8,
    pub weight_pct: u8,
}

/// `Default` is derived deliberately. Every field added since the original
/// shape carries `#[serde(default)]`, but a struct *literal* must still name
/// every field — so the Mn-T7 additions silently broke
/// `vox-ml-cli`'s `populi up` initializer, and `vox populi` stopped compiling
/// with no test covering it. Constructors should spread `..Default::default()`
/// so the next field is additive rather than breaking.
///
/// The derived defaults are the conservative ones: accept nothing, advertise
/// nothing. A node opts in explicitly.
///
/// **Why that matters, and why it is tested here rather than at the call site.**
/// This struct governs what a node donates to a *public* mesh.
/// `accepts_inference_workloads` currently has no consumer at all — `a2a.rs`
/// gates on `public_mesh_opt_in`, `min_priority`, `slots` and the user lists —
/// so the day a planner starts honouring it, a `true` default would silently
/// make every `vox populi up` node an inference donor.
///
/// The obvious guard is to construct the struct explicitly at the call site, so
/// a new field breaks the build and forces a decision. That guard is an
/// illusion here: `vox-ml-cli`'s `mod populi_lifecycle` is
/// `#[cfg(feature = "populi")]` and that crate's default is `["mens-base"]`, so
/// the call site is compiled by neither the default build nor the release
/// builder. It is exactly why the Mn-T7 break sat unnoticed. `vox-mesh-types`
/// has no `[features]` section, so the test below always compiles and always
/// runs — which is what a consent-relevant invariant needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct WorkerDonationPolicy {
    pub slots: Vec<DonationSlot>,
    pub nsfw_allowed: bool,
    pub max_job_duration_secs: u64,
    pub public_mesh_opt_in: bool,
    /// Minimum priority required to accept a job from the public mesh.
    pub min_priority: u8,
    /// Optional whitelist of scopes this node is willing to donate to.
    /// If None, and public_mesh_opt_in is true, it accepts from any scope.
    pub allowed_scopes: Option<Vec<String>>,
    /// Optional whitelist of user IDs allowed to run tasks on this node.
    pub allowed_users: Option<Vec<String>>,
    /// Optional blacklist of user IDs explicitly denied from running tasks.
    pub denied_users: Option<Vec<String>>,
    /// Optional list of federated mesh networks (scope IDs) to explicitly allow.
    pub allowed_mesh_networks: Option<Vec<String>>,
    /// If `true`, this node is willing to accept workloads marked as handling
    /// sensitive data (e.g. PII, health records). Defaults to `false` for
    /// backwards compatibility with serialized policies that lack this field.
    #[serde(default)]
    pub accept_sensitive_workloads: bool,
    /// Optional redundancy / replication policy for declared-deterministic tasks (P6-T4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redundancy: Option<crate::redundancy::RedundancyPolicy>,
    /// Whether this node accepts mesh inference workloads (Mn-T7).
    #[serde(default)]
    pub accepts_inference_workloads: bool,
    /// Whether this node accepts distributed training workloads (Mn-T7). CUDA training path only.
    #[serde(default)]
    pub accepts_training_workloads: bool,
    /// Advertised CUDA tier for planners (`0` = none / unknown).
    #[serde(default)]
    pub cuda_tier: u8,
    /// Advertised Metal tier for planners (`0` = none / unknown).
    #[serde(default)]
    pub metal_tier: u8,
    /// Minimum VRAM (GiB) this node claims for training/inference scheduling hints.
    #[serde(default)]
    pub vram_min_gb: u32,
    /// Distinct from [`Self::accept_sensitive_workloads`]: gates *training* data sensitivity (Mn-T7).
    #[serde(default)]
    pub accepts_sensitive_training_data: bool,
}

#[cfg(test)]
mod default_is_conservative {
    use super::*;

    /// Every consent-relevant field must default to "donate nothing".
    ///
    /// If you add a field to `WorkerDonationPolicy` and this test fails, the
    /// derived default is not conservative — fix the default, do not weaken the
    /// assertion. If you add a field and this test still passes, add a line for
    /// it: an untested field is one a future planner may act on.
    #[test]
    fn a_defaulted_policy_donates_nothing() {
        let p = WorkerDonationPolicy::default();
        assert!(
            !p.accepts_inference_workloads,
            "must not donate inference by default"
        );
        assert!(
            !p.accepts_training_workloads,
            "must not donate training by default"
        );
        assert!(
            !p.accepts_sensitive_training_data,
            "must never default to accepting sensitive training data"
        );
        assert!(
            !p.accept_sensitive_workloads,
            "must not accept sensitive workloads by default"
        );
        assert!(!p.nsfw_allowed, "must not allow nsfw by default");
        assert!(
            !p.public_mesh_opt_in,
            "a node joins the public mesh only by saying so"
        );
    }

    /// Advertise nothing: a node that has not described its hardware must not
    /// appear to have any.
    #[test]
    fn a_defaulted_policy_advertises_no_hardware() {
        let p = WorkerDonationPolicy::default();
        assert_eq!(p.cuda_tier, 0);
        assert_eq!(p.metal_tier, 0);
        assert_eq!(p.vram_min_gb, 0);
        assert!(p.slots.is_empty());
    }

    /// The `populi up` initializer spreads `..Default::default()`, so this is
    /// the value that call site actually ships for the Mn-T7 fields. Asserting
    /// it here keeps the guarantee in a crate that is always compiled, rather
    /// than behind `--features populi`.
    #[test]
    fn the_spread_used_by_populi_up_yields_the_conservative_values() {
        let p = WorkerDonationPolicy {
            nsfw_allowed: true,
            ..Default::default()
        };
        assert!(p.nsfw_allowed, "explicit fields still win");
        assert!(!p.accepts_inference_workloads);
        assert!(!p.accepts_training_workloads);
        assert!(!p.accepts_sensitive_training_data);
        assert_eq!((p.cuda_tier, p.metal_tier, p.vram_min_gb), (0, 0, 0));
    }
}
