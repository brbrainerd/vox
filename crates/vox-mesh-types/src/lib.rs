//! Pure-types L0 leaf for the populi/mesh subsystem (a2a envelopes, donation policy, federation, kudos).

pub mod a2a;
/// Signed result attestation envelope (P5-T4).
pub mod attestation;
/// Public attestation manifest + cache (P6-T2).
pub mod attestation_manifest;
/// A2A wire types for content-addressed bundle requests/responses (P2-T4).
pub mod bundle;
pub mod donation_policy;
pub mod federation;
pub mod kudos;
/// Mesh-wide model inventory snapshot (P5-T8).
pub mod model_inventory;
pub mod model_registry;
/// Signed federation op-fragment envelope (P6-T1).
pub mod op_fragment;
pub mod quorum;
/// Redundancy policy and voting helpers (P6-T4).
pub mod redundancy;
pub mod secret_sync;
pub mod task;
/// TEE attestation envelope (P6-T5).
pub mod tee_attestation;
pub mod trace;

pub use a2a::A2ADeliverRequest;
pub use attestation::Attestation;
pub use attestation_manifest::{
    AttestationCache, ManifestVerifyError, PublicAttestationManifest, SupportedTask,
};
pub use donation_policy::*;
pub use federation::*;
pub use kudos::*;
pub use model_inventory::{MeshModelInventory, ModelInventoryEntry};
pub use model_registry::*;
pub use op_fragment::{
    FederationEnvelope, FederationEnvelopeKind, FederationSignature, OpFragmentEnvelope,
    OpFragmentKind,
};
pub use quorum::*;
pub use redundancy::{RedundancyMode, RedundancyPolicy, TrustTier, VoteOutcome};
pub use secret_sync::*;
pub use task::*;
pub use tee_attestation::{StubTeeVerifier, TeeQuote, TeeQuoteKind, TeeVerifier, TeeVerifyError};
pub use trace::{MeshTraceContext, ParseTraceparentError, SpanId, TraceId};
