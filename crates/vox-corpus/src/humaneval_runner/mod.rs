//! HumanEval-Vox runner: load fixtures, compose runnable programs, detect
//! oracle neutralization, and verify by compiler/test exit code.
//!
//! See `docs/superpowers/plans/2026-09-01-vox-efficacy-benchmark-v2.md` and
//! `docs/src/architecture/vox-efficacy-benchmark-adversarial-audit-2026-09-01.md`.

pub mod canary;
pub mod compose;
pub mod conditions;
pub mod manifest;
pub mod verify;

pub use canary::{canary_program, is_oracle_neutralized, rejects_at_ingest};
pub use compose::compose_program;
pub use manifest::{Fixture, eligible_after, held_out, load_corpus};
pub use verify::{VerifyOutcome, verify_program};
