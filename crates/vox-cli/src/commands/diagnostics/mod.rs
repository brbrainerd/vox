//! Tools & Diagnostics domain: doctor, stub_check, tools (architect, audit, search, compact, clean), lock_report.
//!
//! **Local parity with CI:** `cargo fmt --all -- --check`, `cargo clippy --workspace -- -D warnings`,
//! and scoped TOESTUB via `vox ci toestub-scoped` (defaults to `crates/vox-repository`).

pub mod doctor;
#[cfg(any(
    feature = "script-execution",
    feature = "codex",
    feature = "stub-check"
))]
pub mod lock_report;
#[cfg(feature = "stub-check")]
pub mod stub_check;
#[cfg(any(feature = "codex", feature = "stub-check"))]
pub mod tools;
