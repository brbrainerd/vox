//! SCIENTIA knowledge platform integration components.
//!
//! Modules that correspond to architecture plan phases:
//!   - Phase A producers: `producers`
//!   - Phase B replay runner: `replay`
//!   - Phase C–4 manuscript pipeline: `manuscript`
//!   - Phase D critic gate: `critic_gate`
//!   - Phase E class routing: `class_routing`
//!   - Phase G findings site: `findings_site`
//!   - Phase H dashboard JSON: `dashboard`
//!
//! Planned phases (not yet in this crate, see [planned] in layers.toml):
//! claim_extractor, inspect_bridge, nanopub, prereg, ro_crate, scientia_ingest.

// ── Pre-existing modules ──────────────────────────────────────────────────────
pub mod nanopub;
pub mod ro_crate;
pub mod inspect_bridge;
pub mod claim_extractor;
pub mod ingest;

// ── Phase A: self-observation signal producers ────────────────────────────────
pub mod producers;

// ── Phase B: replay runner ────────────────────────────────────────────────────
pub mod replay;

// ── Phase C + 3+4: manuscript pipeline (scaffold + LaTeX) ────────────────────
pub mod manuscript;

// ── Phase D: solo-author critic gate ─────────────────────────────────────────
pub mod critic_gate;

// ── Phase E: per-class venue routing ─────────────────────────────────────────
pub mod class_routing;

// ── Phase G: findings page renderer ──────────────────────────────────────────
pub mod findings_site;

// ── Phase H: dashboard JSON builders ─────────────────────────────────────────
pub mod dashboard;
