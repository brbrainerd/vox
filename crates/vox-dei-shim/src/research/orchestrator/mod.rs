//! End-to-end research pipeline orchestrator.
//!
//! `run_research` is the single entry point for all research invocations.
//! It coordinates: session creation → query planning → provider search/extract →
//! Codex ingestion → hybrid retrieval fusion → claim detection → verification →
//! answer synthesis → source/claim persistence → session finalization.
//!
//! # Session and metric keys (cross-surface alignment)
//!
//! - **`ResearchMetadata.session_id`** — Opaque numeric session handle for this pipeline run (created
//!   when a Codex handle is present). Progress and completion metrics are written through the same
//!   Codex surface; the underlying `research_metrics.session_id` column is **TEXT** (see
//!   `vox_package::store` `append_research_metric`).
//! - **Agent memory bridge** — `vox_db::MemoryParams.session_id` is set to
//!   `format!("research_{}", session_id)` with `memory_type` `research_result` so MCP memory tools
//!   can recall past answers (matches content keyed in this module).
//! - **Chunk partitioning** — ingest paths may set `kb_id` to `research_session_{session_id}` for
//!   stable correlation with the run.
//!
//! **Benchmark telemetry (CLI, not this pipeline):** `vox_db::VoxDb::record_benchmark_event` writes
//! `research_metrics` under `session_id` **`bench:<repository_id>`** (`crates/vox-db/src/benchmark_telemetry.rs`).
//! That session namespace is separate from DeI research runs; align **`repository_id`** via repository
//! discovery / `VOX_REPOSITORY_ROOT` when comparing Codex rows from CLI subprocesses vs MCP.

mod config;
mod helpers;
mod pipeline;
mod pipeline_cache;
mod stages;
mod web_gather;

pub use config::{ProgressCallback, ResearchConfig};
pub use pipeline::{
    run_research, run_research_with_context, run_research_with_context_and_session,
};
