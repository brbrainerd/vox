//! Emergency **bootstrap** model identifiers when dynamic catalogs are unavailable.
//!
//! Prefer OpenRouter virtual routes and registry resolution at runtime; these strings are the
//! single workspace fallback surface; orchestrator lives in `vox-orchestrator` (see AGENTS.md retired surfaces).
//!
//! 2026-Q2 refresh (2026-05-15): retired stale `gpt-4o` / `gpt-4o-mini` constants in favor of
//! current GA models. Rationale + benchmarks: docs/src/architecture/model-selection-2026-q2.md.

/// OpenRouter dynamic auto-selection route.
pub const OPENROUTER_AUTO: &str = "openrouter/auto";

/// OpenRouter free-tier preference route (VIRTUAL registry id — NOT dispatchable to
/// the OpenRouter API directly; resolve to concrete `:free` slugs before egress).
#[allow(dead_code)]
pub const OPENROUTER_FREE: &str = "openrouter/free";

/// Concrete, **dispatchable** OpenRouter free-tier model slugs (all end in `:free`,
/// guaranteeing $0), ordered most-capable-first. Use these for the research free-tier
/// fallback floor: unlike the virtual [`OPENROUTER_FREE`] route, these are real model
/// ids the OpenRouter API accepts directly.
///
/// SSOT note: mirrors `vox-gamify`'s `OPENROUTER_FREE_MODELS`; the two should converge
/// onto this constant (follow-up — see `docs/superpowers/antigravity-handoff-ledger.md` AGH-0006).
pub const OPENROUTER_FREE_FALLBACK_MODELS: &[&str] = &[
    "google/gemma-3-27b-it:free",
    "meta-llama/llama-3.3-70b-instruct:free",
    "qwen/qwen3-235b-a22b:free",
    "mistralai/mistral-7b-instruct:free",
    "microsoft/phi-3-mini-128k-instruct:free",
];

/// Research / planner / claim stages when no registry candidate exists.
/// 2026-Q2: Gemini 3 Flash — cheap multimodal, fast, 1M context. Was `gpt-4o-mini` (retired).
pub const RESEARCH_FLASH_FALLBACK: &str = "google/gemini-3-flash";

/// Review / judge premium fallback when no registry candidate exists.
/// 2026-Q2: Sonnet 4.6 — best price/quality for code review at $3/$15. Was `gpt-4o` (retired).
pub const REVIEW_PREMIUM_FALLBACK: &str = "anthropic/claude-sonnet-4.6";

/// NLI / verifier default before research run replaces it with a resolved model.
/// 2026-Q2: Gemini 3.1 Flash-Lite — cheapest classifier-grade with structured output.
/// Was `gpt-4o-mini` (retired).
pub const NLI_FALLBACK: &str = "google/gemini-3.1-flash-lite";

/// Preferred model for `vox repair` LLM loop. Sonnet 4.6 with prompt caching is the
/// price/quality optimum for the 3-attempt source-resend pattern (cached input drops to
/// $0.30/MTok, cutting per-session cost ~60% vs uncached).
/// Per `docs/src/architecture/model-selection-2026-q2.md` §3.2.
pub const REPAIR_LOOP_PREFERRED: &str = "anthropic/claude-sonnet-4.6";
