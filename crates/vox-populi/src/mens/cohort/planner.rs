//! The cohort planner: VRAM exclusion gate + throughput-gain estimate.

use crate::mens::tensor::memory_budget;

/// A candidate mesh node, distilled to just what the cohort decision needs.
///
/// Built from a `NodeRecord` by the caller (the planner stays free of registry /
/// wire types so it can be unit-tested in isolation).
#[derive(Debug, Clone, PartialEq)]
pub struct CohortNode {
    /// Stable node id.
    pub id: String,
    /// Total device VRAM in GiB.
    pub vram_gib: f64,
    /// Normalized GPU model name, if known (used for TFLOPS-weighted throughput).
    pub gpu_name: Option<String>,
    /// Node has opted in to training workloads (`donation_policy.accepts_training_workloads`).
    pub accepts_training: bool,
    /// Node is quarantined (excluded regardless of VRAM).
    pub quarantined: bool,
    /// Node is in maintenance (excluded regardless of VRAM).
    pub maintenance: bool,
}

impl CohortNode {
    /// Convenience constructor for a healthy, training-opted-in node.
    #[must_use]
    pub fn new(id: impl Into<String>, vram_gib: f64, gpu_name: Option<String>) -> Self {
        Self {
            id: id.into(),
            vram_gib,
            gpu_name,
            accepts_training: true,
            quarantined: false,
            maintenance: false,
        }
    }
}

/// Why a node was excluded from the cohort.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExclusionReason {
    /// The target model does not fit this node's VRAM (`memory_budget::plan` over budget).
    OverVramBudget,
    /// The node has not opted in to training workloads.
    NotAcceptingTraining,
    /// The node is quarantined.
    Quarantined,
    /// The node is in maintenance.
    Maintenance,
}

impl ExclusionReason {
    /// Short human-readable label for logs / UI.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::OverVramBudget => "over VRAM budget for target model",
            Self::NotAcceptingTraining => "not accepting training workloads",
            Self::Quarantined => "quarantined",
            Self::Maintenance => "in maintenance",
        }
    }
}

/// A node that was excluded, with the reason.
#[derive(Debug, Clone, PartialEq)]
pub struct ExcludedNode {
    pub node: CohortNode,
    pub reason: ExclusionReason,
}

/// The cohort decision: who is in, who is out, and whether pooling is worth it.
#[derive(Debug, Clone, PartialEq)]
pub struct CohortPlan {
    /// Target model id (echoed for logging / downstream dispatch).
    pub target_model: String,
    /// Target model size in billions of parameters.
    pub target_params_b: f64,
    /// Nodes that can usefully participate.
    pub included: Vec<CohortNode>,
    /// Nodes that cannot participate, with the reason.
    pub excluded: Vec<ExcludedNode>,
    /// Estimated wall-clock speedup vs the single fastest included node
    /// (1.0 = no gain). `(sum of included throughput) / (max single throughput)`.
    pub estimated_speedup: f64,
    /// True when pooling is not worth it: 0–1 usable nodes, or speedup below the
    /// [`MIN_USEFUL_SPEEDUP`] threshold. The caller should train on one machine.
    pub recommend_single_machine: bool,
    /// Human-readable one-line rationale for logs.
    pub rationale: String,
}

/// Minimum speedup over single-machine that justifies the coordination overhead of a
/// federated cohort. Below this, the plan recommends a single machine.
///
/// Note: the cohort-speedup estimate (`sum(weights) / max(weights)`) is optimistic —
/// it assumes perfect parallelism and ignores synchronization/communication overhead,
/// so real-world speedups will be somewhat lower than the estimate compared here.
pub const MIN_USEFUL_SPEEDUP: f64 = 1.1;

/// Plan a training cohort for `target_model` (`target_params_b` billions of params),
/// using **uniform** per-node throughput weights.
///
/// A node is excluded if the target model does not fit its VRAM, or it is not
/// accepting training, or it is quarantined / in maintenance. With uniform weights
/// the estimated speedup is simply the included-node count (each node trains a shard
/// in parallel), so the gain reflects parallelism without GPU-class weighting.
///
/// For TFLOPS-weighted throughput (heterogeneous GPUs), use
/// [`plan_cohort_with_estimator`].
#[must_use]
pub fn plan_cohort(nodes: &[CohortNode], target_model: &str, target_params_b: f64) -> CohortPlan {
    plan_cohort_weighted(nodes, target_model, target_params_b, |_| 1.0)
}

/// Plan a cohort with TFLOPS-weighted throughput from a loaded [`TimeEstimator`].
///
/// Each included node is weighted by `estimator.tflops_for(gpu_name)`; unknown GPUs
/// (or nodes without a name) fall back to a weight of `1.0`. The estimated speedup is
/// `(sum of included weights) / (max single included weight)`, so a fast card pooled
/// with a slow one yields a smaller-than-count speedup, and a lone fast card pooled
/// with much slower peers may fall below [`MIN_USEFUL_SPEEDUP`].
#[cfg(feature = "mens-cloud")]
#[must_use]
pub fn plan_cohort_with_estimator(
    nodes: &[CohortNode],
    target_model: &str,
    target_params_b: f64,
    estimator: &crate::mens::cloud::TimeEstimator,
) -> CohortPlan {
    plan_cohort_weighted(nodes, target_model, target_params_b, |node| {
        node.gpu_name
            .as_deref()
            .and_then(|g| estimator.tflops_for(g))
            .filter(|t| *t > 0.0)
            .unwrap_or(1.0)
    })
}

/// Core planner shared by the uniform and estimator-backed entry points.
///
/// `weight_of` returns each included node's throughput weight (TFLOPS or 1.0).
fn plan_cohort_weighted<F>(
    nodes: &[CohortNode],
    target_model: &str,
    target_params_b: f64,
    weight_of: F,
) -> CohortPlan
where
    F: Fn(&CohortNode) -> f64,
{
    let mut included: Vec<CohortNode> = Vec::new();
    let mut excluded: Vec<ExcludedNode> = Vec::new();

    for node in nodes {
        let reason = exclusion_reason(node, target_params_b);
        match reason {
            Some(reason) => excluded.push(ExcludedNode {
                node: node.clone(),
                reason,
            }),
            None => included.push(node.clone()),
        }
    }

    // Throughput weights for the included nodes (positive, finite).
    let weights: Vec<f64> = included
        .iter()
        .map(|n| {
            let w = weight_of(n);
            if w.is_finite() && w > 0.0 { w } else { 1.0 }
        })
        .collect();

    let total: f64 = weights.iter().sum();
    let max_single = weights.iter().cloned().fold(0.0_f64, f64::max);

    // Speedup vs the single fastest node. With <2 nodes there is no parallelism.
    let estimated_speedup = if included.len() <= 1 || max_single <= 0.0 {
        1.0
    } else {
        total / max_single
    };

    let recommend_single_machine = included.len() <= 1 || estimated_speedup < MIN_USEFUL_SPEEDUP;

    let rationale = if included.is_empty() {
        format!(
            "no node can fit / host {target_model} (~{target_params_b:.1}B): {} excluded",
            excluded.len()
        )
    } else if recommend_single_machine {
        format!(
            "{} usable node(s) for {target_model} (~{target_params_b:.1}B); estimated speedup \
             {estimated_speedup:.2}× < {MIN_USEFUL_SPEEDUP:.2}× — recommend single machine",
            included.len()
        )
    } else {
        format!(
            "{} usable node(s) for {target_model} (~{target_params_b:.1}B); estimated speedup \
             {estimated_speedup:.2}× over the fastest single node ({} excluded)",
            included.len(),
            excluded.len()
        )
    };

    CohortPlan {
        target_model: target_model.to_string(),
        target_params_b,
        included,
        excluded,
        estimated_speedup,
        recommend_single_machine,
        rationale,
    }
}

/// Decide whether a node must be excluded, and why. `None` means it can participate.
fn exclusion_reason(node: &CohortNode, target_params_b: f64) -> Option<ExclusionReason> {
    if node.quarantined {
        return Some(ExclusionReason::Quarantined);
    }
    if node.maintenance {
        return Some(ExclusionReason::Maintenance);
    }
    if !node.accepts_training {
        return Some(ExclusionReason::NotAcceptingTraining);
    }
    if memory_budget::plan(node.vram_gib, target_params_b).over_budget {
        return Some(ExclusionReason::OverVramBudget);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(id: &str, vram: f64) -> CohortNode {
        CohortNode::new(id, vram, None)
    }

    // NOTE on `target_params_b` choices: under the current `memory_budget::plan`
    // calibration (3.5 GiB/B resident), a 16 GiB card fits a ~2B target but NOT 3B
    // (3B leaves only ~2.0 GiB for activations, below the seq-128 floor → over_budget).
    // The plan's illustrative "3B fits 16 GiB" predates that recalibration, so these
    // tests use 2.0B as the target — the exclusion-vs-inclusion intent is unchanged:
    // the 16 GiB nodes are usable and the 2 GiB node is not.

    #[test]
    fn excludes_subthreshold_and_estimates_gain() {
        // 16 + 16 GiB fit a ~2B coder; a 2 GiB node cannot.
        let nodes = vec![node("a", 16.0), node("b", 16.0), node("c", 2.0)];
        let plan = plan_cohort(&nodes, "Qwen/Qwen2.5-Coder-3B-Instruct", 2.0);

        assert_eq!(
            plan.included.len(),
            2,
            "two 16 GiB nodes should be included"
        );
        assert_eq!(plan.excluded.len(), 1, "the 2 GiB node should be excluded");
        assert_eq!(plan.excluded[0].node.id, "c");
        assert_eq!(plan.excluded[0].reason, ExclusionReason::OverVramBudget);
        assert!(
            plan.estimated_speedup > 1.0,
            "two usable nodes should estimate a real gain, got {}",
            plan.estimated_speedup
        );
        assert!(!plan.recommend_single_machine);
    }

    #[test]
    fn recommends_single_machine_when_no_gain() {
        // Only one node can host the model → no pooling benefit.
        let nodes = vec![node("a", 16.0), node("b", 2.0)];
        let plan = plan_cohort(&nodes, "Qwen/Qwen2.5-Coder-3B-Instruct", 2.0);

        assert_eq!(plan.included.len(), 1);
        assert!(plan.recommend_single_machine);
        assert_eq!(plan.estimated_speedup, 1.0);
    }

    #[test]
    fn excludes_quarantined_maintenance_and_opt_out() {
        let mut quarantined = node("q", 24.0);
        quarantined.quarantined = true;
        let mut maint = node("m", 24.0);
        maint.maintenance = true;
        let mut opt_out = node("o", 24.0);
        opt_out.accepts_training = false;
        let healthy = node("h", 24.0);

        let nodes = vec![quarantined, maint, opt_out, healthy];
        let plan = plan_cohort(&nodes, "Qwen/Qwen2.5-Coder-3B-Instruct", 3.0);

        assert_eq!(plan.included.len(), 1);
        assert_eq!(plan.included[0].id, "h");
        assert_eq!(plan.excluded.len(), 3);
        let reasons: Vec<_> = plan.excluded.iter().map(|e| e.reason).collect();
        assert!(reasons.contains(&ExclusionReason::Quarantined));
        assert!(reasons.contains(&ExclusionReason::Maintenance));
        assert!(reasons.contains(&ExclusionReason::NotAcceptingTraining));
    }

    #[test]
    fn empty_when_nothing_fits() {
        let nodes = vec![node("a", 2.0), node("b", 4.0)];
        let plan = plan_cohort(&nodes, "Qwen/Qwen2.5-Coder-3B-Instruct", 3.0);
        assert!(plan.included.is_empty());
        assert_eq!(plan.excluded.len(), 2);
        assert!(plan.recommend_single_machine);
        assert_eq!(plan.estimated_speedup, 1.0);
    }

    #[test]
    fn three_usable_nodes_scale() {
        let nodes = vec![node("a", 24.0), node("b", 24.0), node("c", 24.0)];
        let plan = plan_cohort(&nodes, "Qwen/Qwen2.5-Coder-3B-Instruct", 3.0);
        assert_eq!(plan.included.len(), 3);
        // Uniform weights → speedup == node count.
        assert!((plan.estimated_speedup - 3.0).abs() < 1e-9);
        assert!(!plan.recommend_single_machine);
    }
}
