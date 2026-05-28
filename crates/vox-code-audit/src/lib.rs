//! # vox-code-audit
//!
//! **T**odo, **O**mitted wiring, **E**mpty bodies, **S**tub functions,
//! **T**oo-early victory, **U**nresolved references, **B**roken DRY — detector.
//!
//! TOESTUB mechanically detects AI coding anti-patterns that are banned by
//! AGENTS.md but otherwise only caught during manual review.
//!
//! Public modules and re-exports are intentionally thin; each detector/rule is documented in its
//! own file where non-obvious heuristics exist.

/// Stable diagnostic ID catalog (`vox/<category>/<name>` scheme) and explain infrastructure.
pub mod diagnostics;
pub(crate) mod embedded_rules;
pub(crate) mod rule_pack_bridge;
pub(crate) mod rule_pack_detector;

/// Optional LLM-backed triage: wraps provider-specific clients behind a small `AiAnalyzer` API.
pub mod ai_analyze;
/// Token maps, optional `syn` AST, and other shared analysis for detectors.
pub mod analysis;
/// Concrete TOESTUB rules (stubs, empty bodies, secrets, DRY, …) registered by [`detectors::all_rules`].
pub mod detectors;
/// Per-run canary / rollout flags for detectors (set by [`engine::ToestubEngine`]).
pub mod run_context;

/// Structured suppression store (`contracts/toestub/suppression.v1.schema.json`).
pub mod suppression;

/// CR-L6 retirement-guard parity check — cross-references
/// `contracts/retirement/retired-surfaces.v1.yaml` against the registered
/// detectors and diagnostic IDs. Library home of the planned
/// `vox ci retirement-audit` CLI command.
pub mod retirement_parity;

/// Stdlib-coverage parity check (non-CR-L tooling gate). Three-way diff
/// between binary registrations in `crates/vox-compiler/src/eval/builtins.rs`,
/// doc claims in `docs/src/reference/ref-builtins-stdlib.md`, and corpus
/// call sites under `scripts/`. Library home of the
/// `vox audit stdlib-coverage` subcommand. See
/// `docs/src/architecture/vox-stdlib-gap-audit-2026-05-23.md` §10 / §12.D.
pub mod stdlib_parity;

/// Runs configured detectors over a [`scanner::Scanner`] snapshot and aggregates [`rules::Finding`]s.
pub mod engine;
/// Renders findings to the terminal, JSON, or Markdown for CI and local CLI output.
pub mod report;
/// End-to-end **code review** flow: prompts, provider adapters (OpenAI, Ollama, …), SARIF/MD emit.
pub mod review;
/// Shared model for a single finding, severity/language enums, and the [`rules::DetectionRule`] trait.
pub mod rules;
/// Collects `SourceFile` entries from a repo path with language detection from extensions.
pub mod scanner;
/// In-memory bounded work queue used to cap parallel file/review tasks.
pub mod task_queue;

pub use ai_analyze::{AiAnalyzer, AiProvider};
pub use analysis::{NonCodeKind, RustFileContext, TokenMap};
pub use detectors::import_cycles::{detect_import_cycles_in_batch, extract_vox_imports};
pub use engine::{ToestubConfig, ToestubEngine, ToestubRunMode};
pub use report::{OutputFormat, Reporter, RunSnapshot, ToestubJsonReportV1};
pub use review::{
    ReviewCategory, ReviewClient, ReviewConfig, ReviewFinding, ReviewOutputFormat, ReviewProvider,
    ReviewResult, auto_discover_providers, build_diff_review_prompt, build_review_prompt,
    format_markdown, format_sarif, format_terminal, parse_review_response, review_system_prompt,
};
pub use rules::{DetectionRule, Finding, FindingConfidence, Language, Severity};
pub use run_context::ToestubTestsMode;
pub use scanner::Scanner;
pub use task_queue::TaskQueue;
