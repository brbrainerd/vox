---
title: "Vox corpus lab: mass examples, metrics, and eval harness (research 2026)"
description: "Tier A/B/C corpus policy, compiler lanes vs golden parity, Syntax-K and WebIR aggregates, optional UI and vision rubrics, integration with Mens validate-batch and research metrics."
category: "Architecture SSOTs"
status: "research"
sort_order: 18
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Unifies compiler evidence, optional UI pixels-to-JSON, and model eval without duplicating SSOT already in golden tests and Mens contracts."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox corpus lab: mass examples, metrics, and eval harness (research 2026)

## Executive summary

The **corpus lab** is an evidence pipeline, not a single script:

- **Tier A** — Checked-in `examples/golden/**/*.vox`: CI gate `all_golden_vox_examples_parse_and_lower` (parse, HIR, WebIR validate, Syntax-K, runtime projection). See [Golden examples corpus](../how-to/examples-corpus.md) and [examples README](../../../examples/README.md).
- **Tier B** — Ephemeral, gitignored mass corpus under operator control: seeds, mutations, LLM outputs after `validate_generated_vox` / full frontend; must not be mdBook-included until promoted to Tier A ([AGENTS.md](../../../AGENTS.md) documentation hygiene).
- **Tier C** — `examples/parser-inventory/`: negative fixtures; never mixed into Mens goldens.

**Lanes:** Any batch tool should expose at least **`diagnostics_only`** (cheap, parse/typecheck payloads) and **`golden_compatible`** (matches golden test expectations including WebIR validate). Optional: `emit_ir`, `vox build` matrix, screenshot + [vision rubric research](mens-vision-multimodal-research-2026.md).

## Strategic pillars (tie-back)

| Pillar | Corpus lab contribution |
| --- | --- |
| Language evidence | Token histograms, diagnostic taxonomies, WebIR lowering summaries, `legacy_ast_nodes` rate (must stay zero on success path). |
| Behavioral evidence | Optional Vite build, Playwright, screenshot digest + rubric JSON. |
| Model evidence | Same JSONL slice: compiler pass + Mens-served model quality ([Mens training reference](../reference/mens-training.md), Schola serve SSOT). |
| Operational evidence | Cost, wall time, artifact size; align with [telemetry trust](telemetry-trust-ssot.md) if persisted. |

## Existing machinery (do not duplicate silently)

| Capability | Pointer |
| --- | --- |
| Full frontend | `vox-compiler` `pipeline.rs` — lex, parse, lower, typecheck, HIR validate. |
| MCP check | `vox-mcp` `code_validator` — `check_file` diagnostics JSON. |
| Golden gate | `vox-compiler` `tests/golden_vox_examples.rs`. |
| IR emission | [IR emission SSOT](ir-emission-ssot.md) — `vox check --emit-ir` vs `vox build --emit-ir` shapes differ. |
| Mens batch gate | [Mens training data contract](../reference/mens-training-data-contract.md) — `validate-batch`, quarantine. |
| WebIR backlog | [Internal Web IR implementation blueprint](internal-web-ir-implementation-blueprint.md). |

## Generation strategies (research priorities)

1. **Template expansion** from Tier A seeds — lowest garbage rate for WebIR stress.
2. **AST-aware mutation** after successful parse — use `canonicalize_vox` for stable diffs.
3. **Parser no-panic corpus** expansion — `parser_corpus_no_panic.rs` style strings; separate metrics bucket from “valid Vox”.
4. **Synthetic JSONL** — `vox-corpus` `synthetic_gen`; optional emission of `.vox` files for compiler stats, not only Mens rows.
5. **LLM round-trip** — normalize fences (`generated_vox.rs`), then compiler gate; failures feed trajectory repair lanes when enabled.

## Eval harness (corpus × model)

Sketch for a future **`eval_report.json`** (schema to be versioned under `contracts/eval/` when implemented):

- **Inputs:** `corpus_manifest.json` (fixture ids, generator, compiler git SHA), optional `screenshot_sha256`, optional `vision_rubric.json`.
- **Compiler metrics:** pass/fail per lane, WebIR hash, Syntax-K event id or digest if emitted.
- **Model metrics:** same prompts run against baseline remote model and Mens-served adapter; record edit distance to canonical surface, parse pass after model edit (oracle loop), token cost if available.
- **Regression:** compare Qwen2-loaded vs Qwen3.5-loaded adapters on identical slice ([Qwen family research](mens-qwen-family-migration-research-2026.md)).

## Artifact layout (proposal)

Operator-local, gitignored root e.g. `.vox/corpus-lab/` (exact name subject to `vox ci artifact-audit` alignment):

- `runs/<run_id>/manifest.json`
- `runs/<run_id>/per-fixture/<id>.diagnostics.json`
- `runs/<run_id>/per-fixture/<id>.web_ir.sha256` (full JSON optional)
- `runs/<run_id>/vision/<id>.rubric.json` (optional)

## CI posture

- **Default CI:** keep golden Tier A; optional nightly Tier B sampling without network.
- **Browser / vision jobs:** `[self-hosted, linux, x64, browser]` per runner contract; behind env flags; no raw image bytes in uploaded CI artifacts without redaction policy.

## See also

- [GUI, v0/islands, vision, and Mens Qwen — virtuous-cycle implementation plan (2026)](vox-gui-vision-virtuous-cycle-implementation-plan-2026.md)
- [Mens vision and multimodal inputs (research 2026)](mens-vision-multimodal-research-2026.md)
- [Mens Qwen family migration (research 2026)](mens-qwen-family-migration-research-2026.md)
- [Compiler IR pipeline](compiler-ir-pipeline.md)
- [Vox source → Mens pipeline SSOT](vox-source-to-mens-pipeline-ssot.md)

## Open questions

1. Single CLI owner (`vox ci corpus-lab` vs `vox mens corpus` extension) to avoid duplicate batch drivers.
2. Whether to reuse `syntax_k_event` schema only or define `corpus_lab_event` sibling in `contracts/eval/`.
3. Windows `target/` lock contention policy for parallel batch runs ([build environment](../../../.cursor/rules/build-environment.mdc) guidance).


