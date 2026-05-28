---
title: "ADR 035 — SWC parser vs alternatives (evaluation only)"
description: "No migration without explicit ADR sign-off: compare swc_ecma_* with oxc, biome_js_parser, tsgo for vox-cli validation paths."
category: "Architecture Decisions (ADRs)"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "ADR captures parser evaluation criteria and no-migration policy for vox-cli JS/TS validation; useful for agent answers about tooling choices."

schema_type: "TechArticle"
---

# ADR 035 — SWC parser vs alternatives (evaluation only)

## Context

- `vox-cli` / `vox-drift-check` use **`swc_ecma_parser`** / **`swc_ecma_ast`** / **`swc_common`** for JS/TS structural validation.
- Alternative parsers (oxc, biome_js_parser, future tsgo) may offer speed or simpler deps but imply **different AST shapes**, error semantics, and MSRV/feature tradeoffs.

## Decision

- **Status quo: SWC** remains the supported JS parse surface until this ADR is moved to **Accepted** with:
  - Benchmarks on representative corpora (cold cache, incremental).
  - Parity matrix for diagnostics consumers rely on today.
  - MSRV and `workspace-hack` impact notes.

## Non-goals

- Drive-by parser swaps in dependency-cleanup PRs.

## Status

**Proposed** — evaluation-only; link future spike PRs here.
