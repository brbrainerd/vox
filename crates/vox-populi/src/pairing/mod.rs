//! GitHub-attested pairing (SSOT Phase 5 P5-T2).
#![allow(missing_docs)]

#[cfg(feature = "transport")]
pub mod device_flow;
pub mod github_attestation;
pub mod revocation;

pub use github_attestation::{AttestationManifest, ManifestVerifyError};
