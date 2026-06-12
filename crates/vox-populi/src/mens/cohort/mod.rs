//! Heterogeneity-aware federated-LoRA training cohort planning.
//!
//! Given a set of mesh nodes with heterogeneous VRAM, the planner decides which
//! nodes can usefully participate in a federated-LoRA training cohort for a target
//! model, EXCLUDES nodes too small to hold the model, and estimates the throughput
//! gain from pooling them. When adding peers yields no real speedup it recommends
//! running on a single machine instead.
//!
//! This module is intentionally **pure**: no network, no disk, no A2A wire messages,
//! no federated sync loop. It is the decision layer that Task 3.3 (the actual sync
//! protocol) will build on top of. Throughput weighting reuses the per-GPU FP16
//! TFLOPS table via [`crate::mens::cloud::TimeEstimator::tflops_for`] in the
//! estimator-backed variant; the default variant uses uniform weights so the core
//! logic stays unit-testable without loading `gpu-specs.yaml`.

pub mod planner;

pub use planner::{CohortNode, CohortPlan, plan_cohort};

#[cfg(feature = "mens-cloud")]
pub use planner::plan_cohort_with_estimator;
