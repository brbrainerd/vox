//! SCIENTIA nanopublication surface.
//!
//! Trusty-URI signing is provided by [`spec`], which wraps the upstream `nanopub`
//! crate (the real signing SSOT, consumed by `vox-cli`'s `scientia nanopub` command).

pub mod spec;

/// Approval-gated nanopublication TEST-server publishing (#274). The `spec` module
/// signs the Trusty-URI; this module performs the network deposit to the public
/// nanopub test server once a human approval gate has passed (see `review_flow`).
pub mod network;
