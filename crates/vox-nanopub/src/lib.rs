//! SCIENTIA nanopublication builder: TriG emission, Ed25519 signing, and Trusty-URI derivation.
//!
//! Converts atomic claims into signed nanopublications ready for the Nanopublication Network.
//!
//! This is a **leaf crate** — it depends only on `vox-crypto`, `sha2`, and `hex`, with no
//! connection to the Vox compiler / CLI / database spine. It can therefore be consumed
//! independently of `vox-scientia` (e.g. dropped into an external publication system that
//! only wants nanopublication output).
//!
//! Network publishing (HTTP POST to the Nanopub Network) deliberately lives in the consumer
//! (`vox-scientia::nanopub::network`), keeping this crate I/O-free.

pub mod signing;
pub mod trig;

pub use signing::{SignedNanopub, SigningKey, VerifyingKey, sign_nanopub, verify_nanopub};
pub use trig::{NanopubDocument, NanopubGraphs, build_nanopub};
