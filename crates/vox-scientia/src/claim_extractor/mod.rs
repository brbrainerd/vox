//! SCIENTIA claim extraction: VeriScore → atomic decomposition → span integrity → MiniCheck.

pub mod atomic;
pub mod constrained;
pub mod minicheck;
pub mod pipeline;
pub mod span;
pub mod types;
pub mod veriscore;

pub use pipeline::{ExtractionConfig, ExtractionPipeline};
pub use types::{
    AtomicClaim, ClaimVerdict, ExtractionResult, SciClaimTuple, SpanBound, VerifiabilityClass,
    VerifierOutput,
};
