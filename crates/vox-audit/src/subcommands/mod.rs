//! Concrete `vox audit <thing>` subcommand implementations.
//!
//! Each subcommand impls [`crate::Subcommand`]. Two flavors:
//!
//! - **Real implementations** (`retirement`, `aci_default`, `humaneval`) — wrap
//!   shipped library logic and return measured outcomes.
//! - **Stubs** (`stubs::*`) — corpus-driven gates whose fixtures are stubs
//!   today. They return [`crate::report::ExitCode::InfrastructureError`] with
//!   a structurally complete [`crate::report::AuditReport`] carrying
//!   `incomplete: true`. Per contract §exit-code-2-semantics, this does NOT
//!   block CI.

pub mod aci_default;
/// CR-F1 (Foundation): behavioral goldens — `// EXPECT:` stdout matches
/// `vox run --mode interp`. First registered Foundation-tier gate.
pub mod behavioral_goldens;
pub mod corpus_feedback;
/// CR-L1: HumanEval-Vox static-check gate. Replaced its stub in P2.3 — see
/// `crate::subcommands::humaneval::HumanEvalSubcommand`.
pub mod humaneval;
/// CR-F6 (Foundation): regression budget — zero `todo!`/`unimplemented!`/
/// `// vox:skip`/`de-stub-pending` in foundation crates and golden corpus.
pub mod regression_budget;
pub mod retirement;
/// Non-CR-L tooling gate: stdlib-coverage parity check. See
/// `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §10 / §12.D.
pub mod stdlib_coverage;
pub mod stubs;
