//! SCIENTIA Phase 5: UK AISI Inspect task adapter, atomic-NEI novelty scoring,
//! ChronoFact timestamp filtering, and EvidenceConflict detection.

pub mod chronofact;
pub mod conflict;
pub mod inspect_task;
pub mod novelty;

pub use chronofact::ChronoFilter;
pub use conflict::{ClaimPolarity, EvidenceConflict, EvidenceConflictDetector, PolarizedHit};
pub use inspect_task::{InspectSample, InspectTaskDescriptor, vox_probe_to_inspect_sample};
pub use novelty::{AtomicNoveltyScorer, NoveltyConfig, NoveltyVerdict};
