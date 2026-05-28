//! SCIENTIA RO-Crate 1.2 JSON-LD metadata builder.
//!
//! Produces CFF, CodeMeta, TOP-Level-2, and ACM badge compliance surfaces for research artifacts.

pub mod ai_disclosure;
pub mod cff;
pub mod compliance;
pub mod metadata;

pub use ai_disclosure::{AiDisclosureBlock, AiToolUsage};
pub use cff::{CffAuthor, CffMetadata, build_cff_json};
pub use compliance::{AcmBadge, TopComplianceReport, TopLevel, acm_artifacts_available_badge};
pub use metadata::{FigureProvenance, MainEntity, RoCrateMetadata, build_ro_crate_json};
