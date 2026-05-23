---
title: "Syntax K complexity telemetry (WebIR + emit)"
description: "Kolmogorov-style syntax complexity telemetry for Vox compiler outputs, including schema, reproducibility protocol, and rollout gates."
category: "Language Reference"
last_updated: "2026-03-27"
training_eligible: true

schema_type: "TechArticle"
---

# Syntax K complexity telemetry (WebIR + emit)

This page defines the repository-wide method for tracking **syntax K complexity** of Vox output programs.

## Scope

- Measure complexity of **compiler outputs**, not Rust source complexity.
- Primary object: canonical **WebIR JSON**.
- Secondary object: canonicalized emitted output bundle (for current tests: TSX preview emit bundle).
- Collection points: compiler golden/parity tests and eval-matrix benchmark classes.

## Mathematics

`K` is uncomputable; Vox uses practical compression-based proxies:

- Absolute estimate:
  - `K_est(x) = min_z |z(x)|` over fixed compressors `z = {zstd,bzip2,gzip}` with pinned profiles.
- Relative drift:
  - `NCD_z(x,y) = (|z(xy)| - min(|z(x)|,|z(y)|)) / max(|z(x)|,|z(y)|)`.
- Support metrics:
  - structural counts from `WebIrLowerSummary` and `WebIrValidateMetrics`.

## Event contract

Events are written to `research_metrics` with:

- `session_id = syntaxk:<repository_id>`
- `metric_type = syntax_k_event`
- `metadata_json` payload conforming to:
  - `contracts/eval/syntax-k-event.schema.json`

Core payload fields:

- `schema_version`
- `fixture_id`
- `source_hash`
- `web_ir_hash`
- `target_kind`
- `raw_bytes`
- `compressor_results`
- `k_est_bytes`
- `ncd_vs_baseline` (optional)
- `support_metrics` (optional): may include `representability`, `llm_surface`, and `runtime_projection` summaries (canonical SHA-3 of runtime projection JSON, policy counts, host-probe flag when `VOX_RUNTIME_PROJECTION_INCLUDE_HOST_PROBE=1`, and whether module-level task hints were inferred from `db.*` `.using` / `.scope` metadata). Shape is forward-compatible (`additionalProperties` allowed in eval schema).
- `toolchain_fingerprint`

## Reproducibility protocol

- Canonicalize output bytes before compression.
- Keep compressor set/profile fixed.
- Use deterministic concatenation policy for NCD (`len(x)||x||len(y)||y`).
- Record toolchain/profile fingerprint in every event.
- Start with observe-only tracking; avoid immediate hard fail gates.

## Integration surfaces

- Compiler estimators: `crates/vox-compiler/src/syntax_k.rs`
- Compiler test artifacts:
  - `target/benchmarks/syntax-k/golden/*.json`
  - `target/benchmarks/syntax-k/parity/*.json`
- VoxDB API:
  - `VoxDb::record_syntax_k_event`
  - `VoxDb::list_syntax_k_events`
- Eval matrix classes:
  - `vox_compiler_syntax_k_webir`
  - `vox_compiler_syntax_k_emit`
  - `vox_compiler_syntax_k_regression_gate`
- MCP tools:
  - `vox_benchmark_list` / `vox_benchmark_record` with `metric_type = syntax_k_event`

## Rollout gates

- `VOX_SYNTAX_K_TELEMETRY=1|true`
  - Enables writing syntax-K telemetry rows from CLI benchmark paths.
  - If unset, falls back to `VOX_BENCHMARK_TELEMETRY`.
- `VOX_SYNTAX_K_GATE`
  - `observe` (default): track and emit artifacts only.
  - `enforce`: enables threshold assertion in the regression-gate benchmark test.
- `VOX_SYNTAX_K_MAX_BYTES`
  - Optional byte threshold used only when gate mode is `enforce`.


