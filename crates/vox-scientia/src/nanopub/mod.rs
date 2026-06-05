//! SCIENTIA nanopublication surface.
//!
//! The builder (TriG emission, Ed25519 signing, Trusty-URI derivation) now lives in the
//! standalone leaf crate [`vox_nanopub`] so it can be consumed independently of SCIENTIA.
//! This module re-exports that surface and adds the SCIENTIA-only network-publishing layer.

pub mod network;

// Re-export the leaf crate's modules so existing `crate::nanopub::{trig,signing}::*` and
// `vox_scientia::nanopub::*` consumers keep resolving unchanged.
pub use vox_nanopub::{signing, trig};
pub use vox_nanopub::{
    NanopubDocument, NanopubGraphs, SignedNanopub, build_nanopub, sign_nanopub, verify_nanopub,
};

pub use network::{NanopubNetworkConfig, PublishResult, publish_stub};
