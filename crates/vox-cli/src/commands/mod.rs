//! Subcommand implementations for the **`vox`** binary.
//!
//! Each submodule corresponds to one clap variant in [`crate::Cli`] (`src/lib.rs`). Shared behavior (lex/parse/typecheck)
//! is available in [`crate::pipeline`] for a single frontend; `build` and `check` here still use the
//! legacy inline path. New work should route through `pipeline` for consistent diagnostics.

mod command_registry_handler_needles;

pub mod add;

/// Quality-gate umbrella command (`vox audit`); reads `contracts/ci/check-targets.v1.yaml`.
pub mod audit;

#[cfg(feature = "dei")]
pub mod attention;
/// Identity and master key integration (`vox auth`).
pub mod auth;
/// Building and codegen orchestration endpoints.
pub mod build;
/// Packaging tools for bundling Vox web apps (e.g., TanStack/Vite wrapper).
pub mod bundle;
/// Catalog explicit management commands (`vox catalog`).
pub mod catalog;
/// Validation and static checking (`vox check`).
pub mod check;
/// CI / SSOT guard commands (`vox ci`).
pub mod ci;
/// Codex integration logic for `vox db` subcommands.
pub mod codex;
/// Native / workspace compile umbrella (`vox compile`).
pub mod compile;
/// `vox config` CLI endpoint logic.
pub mod config;
/// Training data extraction / mixing pipelines (`vox corpus`).
/// Codex research ingest / reliability helpers (`vox db` research subcommands).
mod db_research;
/// Canonical login for vault / Secrets (`vox login`, `vox auth connect`, `vox secrets login`).
pub mod login_shared;
pub mod remove;
/// Centralized secret lifecycle commands (`vox secrets`).
pub mod secrets;
// `db.rs` re-exports this tree; keep a same-file reference for tooling / unwired-module checks.
#[allow(unused_imports)]
use self::db_research as _;
/// Local VoxDB / Codex diagnostics (`vox db`).
pub mod db;
/// Clap entrypoints for `vox db`.
pub mod db_cli;
pub(crate) mod db_retention;
/// DEI decision engine commands (requires `--features dei`).
#[cfg(feature = "dei")]
pub mod dei;
/// `vox deploy` — `Vox.toml` `[deploy]` execution (`vox-container`).
pub mod deploy;
/// Auto-reloading compilation daemon runner (`vox dev`).
pub mod dev;
/// Submodules for `architect`, `doctor`, `clean`, etc.
pub mod diagnostics;
/// API documentation generator wrapper (`vox doc`).
pub mod doc;
/// Narrow codegen (`vox emit client`, …).
pub mod emit;
/// Extension lane: unified entry for legacy/ML subcommands (ars, ludus, oratio, schola).
pub mod ext;
/// Supplemental subcommands (snippet, share, ars).
pub mod extras;
/// Hybrid retrieval CLI (`vox memory search`).
pub mod memory_cli;
/// Socrates / evidence fusion for scientia worthiness (`metadata_json.scientia_evidence`).
pub mod scientia_worthiness_enrich;
/// ARS `vox skill` implementation (`extras::ars`); re-exported for internal call sites and any out-of-tree dispatch shims.
#[cfg(feature = "ars")]
pub use extras::ars;
/// AST formatting and canonicalization (`vox fmt`).
pub mod fmt;
/// `vox info` — package metadata from registry / local Arca store (`vox-package`).
pub mod info;
/// `vox init` — scaffold `Vox.toml` / `src/main.vox` / skill markdown.
pub mod init;
/// Interactive telemetry-enabled execution orchestrator (`vox live`).
#[cfg(feature = "live")]
pub mod live;
pub mod lock;
/// Launch Language Server Protocol wrapper (`vox lsp`).
pub mod lsp;
/// Start the Vox MCP server wrapper (`vox mcp`).
pub mod mcp;
#[cfg(feature = "mcp-server")]
pub mod mcp_server;
/// React interop / web stack migrations (`vox migrate web`, …).
pub mod migrate;
pub mod model;
pub mod new;
/// `vox openclaw` tools for orchestrator testing.
#[cfg(feature = "ars")]
pub mod openclaw;
pub mod play;
pub mod pm;
pub mod pm_lifecycle;
pub mod repair;
#[cfg(feature = "dei")]
pub mod safety;

/// Explicit multi-repo catalog and read-only polyrepo queries (`vox repo`).
pub mod repo;
pub mod repo_init;
pub(crate) mod repo_upgrade;
/// TOESTUB structural testing guard logic.
#[cfg(feature = "stub-check")]
pub mod stub_check;
pub mod sync;
pub(crate) mod toolchain_upgrade;
pub mod upgrade;
/// Ludus gamification systems logic wrapper.
#[cfg(feature = "extras-ludus")]
pub use extras::ludus;
/// AI-powered CodeRabbit review adapter (`vox review`).
#[cfg(feature = "coderabbit")]
pub mod review;
/// Native execution via local runtime execution (`vox run`).
pub mod run;
/// Extended runtime subtree (`dev`, `info`, `run` script path, shell) — see submodules.
pub mod runtime;
/// Vox Scientia research facade (`vox scientia` → `vox db` research tools).
pub mod scientia;
pub(crate) mod scientia_ledger_contract;
/// `vox scientia scout` — Phase F single-command surface over the Phase A
/// signal producers in `vox-scientia-producers`.
pub mod scout;
/// Thin CLI handlers wrapping the pure-library SCIENTIA Phase B / C / D / E
/// / G / H crates (`vox-replay-runner`, `vox-manuscript-scaffold`,
/// `vox-critic-gate`, `vox-class-routing`, `vox-findings-site`,
/// `vox-scientia-dashboard`).
pub mod scientia_phase_handlers;
/// Public-URL tunnel for Vox apps (`vox share`). S1: LAN backend only.
pub mod share;
/// Optional telemetry upload queue (`vox telemetry`).
pub mod telemetry;
/// Test suite integration wrapper (`vox test`).
pub mod test;
pub mod update;

pub mod grammar;

/// Unified research operations: infrastructure and evaluation.
pub mod research;

/// Manual plan bridging via PlanningOrchestrator
pub mod plan;

/// `vox plugin` — install, remove, list, and inspect Vox plugins.
pub mod plugin;

/// `vox bundle` — list, build, and apply plugin distribution bundles.
pub mod plugin_bundle;

/// LLM-native context and prompt generation tools
pub mod llm;

/// Generate Vox code from a prompt using the MENS inference model (`vox generate`).
pub mod generate;

/// Vox Visus: Voice of Vision. Agentic GUI visual intelligence and bug detection.
#[cfg(feature = "dei")]
pub mod visus;


/// Native Tauri 2 GUI (`vox gui`).
#[cfg(feature = "gui")]
pub mod gui;

/// Workspace drift and pattern-repetition linter (`vox drift-check`).
pub mod drift_check;

/// Insta snapshot helpers: orphan detection and cleanup (`vox snapshot`).
pub mod snapshot;

/// Rollback
pub mod rollback;

/// Workflow introspection: dry-run schedule preview (`vox workflow preview`) (P1-T8).
pub mod workflow;

/// Dispatch-time routing preview — `vox dispatch preview` (P2-T6).
pub mod dispatch;
