---
title: "Vox playground architecture (research)"
description: "Target shape for a browser-local Vox playground vs today's REPL-oriented tooling; cites compiler pipeline, bounded execution, and Phase 4 monitor hooks."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Aligns future playground work with compiler tiers, telemetry trust, and deterministic eval hooks."
sort_order: 10
---

# Vox playground architecture (research)

**Scope:** Architecture-only — **no playground runtime ships** with this note. It folds Phase 4 monitor ideas ([`vox-language-rules-phase4-runtime-monitors-2026.md`](vox-language-rules-phase4-runtime-monitors-2026.md)) into a concrete product surface.

## Problem statement

Authors today rely on:

| Surface | Strength | Gap |
| --- | --- | --- |
| `vox check` / `vox fmt` | Full compiler pipeline | Not browser-local; no embeddable UX. |
| `vox shell repl` | Fast feedback loop | Not a structured teaching surface; limited linkage to diagnostics / fixes. |
| IDEs via `vox lsp` | Rich editing | Requires install; blocks zero-install tutorials. |

A **playground** closes the zero-install loop while staying inside telemetry + sandbox policy ([`telemetry-trust-ssot.md`](telemetry-trust-ssot.md)).

## Architectural slices

1. **Syntax & diagnostics tier** — Wasm bundle of lexer/parser/typecheck **read-only** stages (mirrors [`crates/vox-compiler`](../../../crates/vox-compiler) algorithms). Emits the same structured payloads as [`VoxCompilerDiagnosticPayload`](../../../crates/vox-compiler/src/typeck/diagnostics.rs) / [`vox check --for-llm`](../../../crates/vox-cli/src/pipeline.rs).
2. **Formatter tier** — Optional second Wasm module wrapping [`vox_compiler::fmt::format`](../../../crates/vox-compiler/src/fmt/mod.rs); must respect idempotency tests ([`format_round_trip.rs`](../../../crates/vox-compiler/tests/format_round_trip.rs)).
3. **Execution tier (optional)** — Behind explicit **Run** consent: `vox-actor-runtime` wasm/isolation lane (`vox run --isolation wasm` semantics), fuel + allocation caps per Phase 4 plan. No ambient filesystem writes — [`vox-bounded-fs`](../../../crates/vox-bounded-fs) only.
4. **Telemetry tier** — Default **local-only** mirrors CLI trust boundaries; remote upload remains opt-in per ADR-023. Planned `vox.idiom.*` families are specified in [`contracts/telemetry/idiom-events.v1.yaml`](../../../contracts/telemetry/idiom-events.v1.yaml).

## UX contours

- **Share URL** encodes compressed source + `syntax_version` (never secrets).
- **Diff pane** shows structured fixes coming from LSP-shaped JSON (parity with IDE quick-fixes).
- **Deterministic mode** exposes `--seed` contracts from Phase 4 (`vox playground --deterministic`) once wired.

## Non-goals (near term)

- Hosting arbitrary network calls from the playground Wasm tier (would violate effect policy until Phase 5 UX exists).
- Replacing [`vox shell repl`](../reference/cli.md) — playground optimizes **documentation + teaching**, not power-user shell ergonomics.

## Verification hooks

When implementation lands, require:

1. Snapshot parity tests mirroring [`diagnostic_snapshots`](../../../crates/vox-compiler/tests/diagnostic_snapshots.rs) / [`vox-lsp/tests/diagnostic_snapshots.rs`](../../../crates/vox-lsp/tests/diagnostic_snapshots.rs).
2. `vox ci parse-status` alignment for golden corpus examples ([`examples/PARSE_STATUS.md`](../../../examples/PARSE_STATUS.md)).

## See also

- [`editor-integrations.md`](../how-to/editor-integrations.md)
- [`vox-compiler-architecture-research-2026.md`](vox-compiler-architecture-research-2026.md)
