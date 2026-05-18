//! Historical stub-runner tombstone (empty as of 2026-05-18).
//!
//! All 9 CR-L runners are now real implementations:
//! - CR-L0 → `spec_to_app::SpecToAppRunner`
//! - CR-L1 → `humaneval::HumanEvalRunner`
//! - CR-L2 → `mens_on_distribution::MensOnDistributionRunner`
//! - CR-L3 → `repair_corpus::RepairCorpusRunner`
//! - CR-L4 → `plan_fidelity::PlanFidelityRunner`
//! - CR-L5 → `aci_default::AciDefaultSubcommand`
//! - CR-L6 → `retirement::RetirementSubcommand`
//! - CR-L7 → `deploy::DeployRunner`
//! - CR-L8 → `corpus_feedback::CorpusFeedbackSubcommand`
//!
//! Module retained as a doc-anchor; no live code. The
//! `corpus_stub_outcome` helper that previously routed infra-error
//! returns on behalf of stubs lived here and is no longer called from
//! anywhere in the registry. Removed entirely in this revision to
//! make accidental re-introduction of a stub a build-time failure
//! rather than a silent regression.
