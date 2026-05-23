---
title: "Vox source → compiler → Mens training (pipeline SSOT)"
description: "End-to-end map from .vox text through the lexer and compiler to golden examples, corpus mix, and Mens (HF) training—without conflating compile tokens with model tokens."
category: "Architecture SSOTs"
status: "current"
sort_order: 3
last_updated: "2026-04-12"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox source → compiler → Mens training (pipeline SSOT)

This page is the **persistent** crosswalk for contributors: where `.vox` files are enforced, how they relate to documentation, and how they reach **Mens** fine-tuning. It deliberately separates **compile-time lexing** from **training-time tokenization**.

## 1. Authoritative `.vox` layout

| Tree | Role | Enforcement |
| --- | --- | --- |
| `examples/golden/**/*.vox` | Canonical, training-eligible demos | `cargo test -p vox-compiler --test golden_vox_examples` (parse → HIR → WebIR validate → Syntax-K metrics) |
| `examples/parser-inventory/**/*.vox` | Negative / recovery fixtures | Must **not** be mixed into Mens goldens; excluded by SSOT |
| Policy file | Declares golden roots, negative roots, doc scan roots | [`examples/examples.ssot.v1.yaml`](../../../examples/examples.ssot.v1.yaml) |
| mdBook includes | Hash-include paths under `docs/src` must resolve to **existing** `.vox` under `examples/golden/` (see [Golden Examples corpus](../how-to/examples-corpus.md)) | `cargo test -p vox-compiler --test examples_ssot` |

Operator entry: [`examples/README.md`](../../../examples/README.md).

## 2. Lexer and parser (language surface)

- **Lexer:** `crates/vox-compiler/src/lexer/` — `logos`-derived [`Token`](../../../crates/vox-compiler/src/lexer/token.rs) stream; batch API [`lex`](../../../crates/vox-compiler/src/lexer/cursor.rs).
- **Parser / typechecker / lowering:** monolithic `vox-compiler` (see [Compiler IR pipeline](compiler-ir-pipeline.md), [IR emission SSOT](ir-emission-ssot.md)).

The lexer’s keyword inventory is the **source-of-truth for what characters become which tokens** before AST construction. It does **not** define Mens vocabulary.

**Lexing note:** [`lex`](../../../crates/vox-compiler/src/lexer/cursor.rs) currently **skips** spans that do not match a token (`logos` errors are dropped). Prefer adding explicit `#[token("@…")]` entries for documented decorators so source is not silently altered.

## 3. Documentation corpus

- Verified snippets: **pull from `examples/golden/`** via `{{#include}}` (see [Golden Examples book page](../examples/golden.md), [documentation governance](../contributors/documentation-governance.md)).
- `vox mens pipeline` may ingest `docs/src` into mix-side JSONL; default production mix may remain code-heavy—see [Mens native training](../reference/mens-training.md) § documentation corpus lane.

## 4. Mens training path (model input)

1. **Golden / codegen pairs:** `vox_corpus` walks `examples/golden/**/*.vox` (and other configured roots) to build instruction–response rows.
2. **Mix + validate:** `mens/config/mix.yaml`, `vox mens corpus validate`, etc.—see [Native ML pipeline](../explanation/expl-ml-pipeline.md) and [Mens native training](../reference/mens-training.md).
3. **QLoRA default:** **`vox mens train`** uses **Hugging Face tokenizer** for the chosen base model—not `VoxTokenizer` and not the compile lexer. Lab `VoxTokenizer` in `vox-tensor` is a **small Burn/dogfood** path only.

## 5. Gap checklist (goldens vs journeys)

Use this when adding files under `examples/golden/`:

| Journey / capability | Golden coverage (Apr 2026) | Suggested follow-up |
| --- | --- | --- |
| Script / CLI `vox run` | `mesh/noop.vox`, `hello.vox`, `std_http_wrappers.vox` | Optional: dedicated `golden/script_args.vox` if CLI argv story grows |
| Reactive UI | `reactive_counter.vox`, `dashboard_ui.vox`, `web_routing_fullstack.vox` | Expand when `layout_groups` grammar lands (see backlog docs) |
| Data + HTTP API | `crud_api.vox`, `blog_fullstack.vox` | — |
| Actors / workflows / MCP | `counter_actor.vox`, `checkout_workflow.vox`, `mcp_tools.vox` | — |
| `@scheduled` decorator | `scheduled_tick.vox` | `WebIrModule.scheduled_jobs` carries name + interval from HIR |
| `@pure` / `@require` / `@deprecated` | `ref_effects.vox` (regions wired in mdBook API pages) | HTTP `Result` / `Error` mapping: `http_error_mapping.vox` |
| Error / `Result` patterns | `http_error_mapping.vox`, `type_system.vox` (partial) | — |

## 6. Related links

- [Language surface SSOT](language-surface-ssot.md)
- [Populi data pipeline](populi-data-pipeline.md) (mesh / control-plane vs training data)
- [Mens training data contract](../reference/mens-training-data-contract.md)
- [Vox corpus lab (research 2026)](vox-corpus-lab-research-2026.md) — Tier B mass corpus, batch lanes, eval harness sketch
- [Mens vision and multimodal inputs (research 2026)](mens-vision-multimodal-research-2026.md)
- [Mens Qwen family migration (research 2026)](mens-qwen-family-migration-research-2026.md)


