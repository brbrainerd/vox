//! Typed SCIENTIA research event types, `PreregistrationV1`, and `ResearchEventEmitter` trait.
//!
//! This is an L1 crate (pure data — no async runtime, no DB). All SCIENTIA pipeline
//! components communicate through these types to avoid circular crate dependencies.

// `schema_types.generated.rs` (typify) references `::regress::Regex`; keep `regress` in the crate
// root prelude so `cargo test` graphs that unify features differently still compile.
#[allow(unused_imports)]
use regress as _;

pub mod emitter;
pub mod events;
pub mod observation;
pub mod preregistration;
pub mod publication_format;
pub mod schema_types;

pub use emitter::{NoopEmitter, ResearchEventEmitter};
pub use events::{ResearchEvent, ResearchEventKind};
pub use observation::{
    KeywordObservationClassifier, LearnedProfileRow, ObservationClass, PenaltyRecord,
    ScientiaObservationClassifier, ScientiaWeightExtension,
};
pub use preregistration::{
    DecisionRule, MetricSpec, PreregistrationV1, StatisticalTest, StopRule, SubstrateRef, TestSpec,
};
pub use publication_format::{
    FigurePolicy, PublicationPlatform, ShortFormVariant, adapt_claim_to_platform,
    validate_short_form,
};
pub use schema_types::{
    DiscoverySignal, DiscoverySignalFamily, DiscoverySignalStrength, EvidencePackV1,
    FindingCandidateClass, FindingCandidateV1, NoveltyEvidenceBundle, SignalProvenance,
    WorthinessSignalsV2,
};
