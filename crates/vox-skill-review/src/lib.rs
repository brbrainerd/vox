//! Local advisory pre-publish review gate for VoxSkills. Deterministic floor only
//! (no network, no execution); the LLM review pass is an optional follow-up.

pub mod checks;
pub mod model;
pub mod review;

pub use model::{ReviewItem, ReviewReport, Severity, Verdict};
pub use review::review_skill;
