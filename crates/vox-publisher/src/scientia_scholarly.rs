//! SCIENTIA scholarly output integration: claim extraction, novelty scoring,
//! nanopublication emission, and RO-Crate metadata packaging.
//!
//! All items in this module are re-exported from their respective crates and
//! are only available when the `scholarly-external-jobs` feature is enabled.

// --- Claim extraction ----------------------------------------------------------
#[cfg(feature = "scholarly-external-jobs")]
pub use vox_scientia::claim_extractor::{
    AtomicClaim, ClaimVerdict, ExtractionConfig, ExtractionPipeline, ExtractionResult,
};

// --- Novelty scoring & conflict detection --------------------------------------
#[cfg(feature = "scholarly-external-jobs")]
pub use vox_scientia::inspect_bridge::{
    AtomicNoveltyScorer, ChronoFilter, EvidenceConflict, EvidenceConflictDetector, NoveltyConfig,
    NoveltyVerdict,
};

// --- Nanopublication builder ---------------------------------------------------
#[cfg(feature = "scholarly-external-jobs")]
pub use vox_scientia::nanopub::{NanopubDocument, NanopubGraphs, SignedNanopub, build_nanopub, sign_nanopub};

// --- RO-Crate JSON-LD metadata ------------------------------------------------
#[cfg(feature = "scholarly-external-jobs")]
pub use vox_scientia::ro_crate::{
    AiDisclosureBlock, CffMetadata, RoCrateMetadata, build_cff_json, build_ro_crate_json,
};
