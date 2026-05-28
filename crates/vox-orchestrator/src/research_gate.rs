//! Pre-registration gate integration for the orchestrator.
//!
//! This module wires [`crate::preregistration`] into the orchestrator's campaign-dispatch
//! path. Per SCIENTIA §5.1, the orchestrator must refuse to launch any research
//! campaign that lacks a signed [`vox_research_events::preregistration::PreregistrationV1`]
//! record.
//!
//! # Usage
//!
//! ```rust,ignore
//! use vox_orchestrator::research_gate::{GateResult, check_campaign_prereg};
//! use vox_research_events::preregistration::PreregistrationV1;
//!
//! match check_campaign_prereg(Some(&prereg), Some(&signature_hex)) {
//!     GateResult::Approved => { /* proceed */ }
//!     GateResult::Refused { reason } => { /* abort + log */ }
//! }
//! ```

pub use crate::preregistration::{GateResult, PreregGate};

/// Check whether a campaign may proceed given an optional pre-registration
/// record and an optional hex-encoded Ed25519 signature.
///
/// This is a thin delegating call to [`PreregGate::check_campaign`] on a
/// default (stateless) gate instance. Call-sites inside the orchestrator can
/// import this single function without constructing a [`PreregGate`] directly.
///
/// # Refusal conditions
/// - `prereg` is `None` → refused ("no preregistration provided")
/// - `signature_hex` is `None` → refused ("no signature provided")
/// - Signature verification fails → refused with the verification error
pub fn check_campaign_prereg(
    prereg: Option<&vox_research_events::preregistration::PreregistrationV1>,
    signature_hex: Option<&str>,
) -> GateResult {
    PreregGate.check_campaign(prereg, signature_hex)
}
