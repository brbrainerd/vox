//! SCIENTIA nanopublication builder: TriG emission, Ed25519 signing, and Trusty URI derivation.
//!
//! Converts atomic claims (T2) into signed nanopublications ready for the Nanopublication Network.

pub mod network;
pub mod signing;
pub mod trig;

pub use network::{NanopubNetworkConfig, PublishResult, publish_stub};
pub use signing::{SignedNanopub, sign_nanopub, verify_nanopub};
pub use trig::{NanopubDocument, NanopubGraphs, build_nanopub};
