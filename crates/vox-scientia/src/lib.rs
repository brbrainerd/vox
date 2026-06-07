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
//! Now-present sub-modules (formerly tracked as planned crates in layers.toml):
//! `claim_extractor`, `inspect_bridge`, `nanopub`, `ro_crate`, `ingest`.
//! Still planned (not yet in this crate): `prereg`.

// ── Pre-existing modules ──────────────────────────────────────────────────────
pub mod claim_extractor;
pub mod ingest;
pub mod inspect_bridge;
pub mod nanopub;
pub mod ro_crate;

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

// ── P2: human-gated discovery review (pure logic) ────────────────────────────
pub mod review;

// ── P3: shared review-flow SSOT (DB + vault I/O; CLI + GUI both call this) ────
pub mod review_flow;
