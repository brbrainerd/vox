//! Stable diagnostic ID catalog and explain infrastructure.
//!
//! See the `catalog` submodule for the full list of stable IDs.

pub mod ai_fixture_explain;
pub mod catalog;

pub use ai_fixture_explain::explain_ai_fixture_diagnostic;
pub use catalog::{ALL_KNOWN_IDS, ALL_PHASE2_IDS, explain_url, is_known_id};
