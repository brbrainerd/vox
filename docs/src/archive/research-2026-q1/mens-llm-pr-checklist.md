---
title: "Mens / HF fine-tune — LLM PR checklist"
description: "Official documentation for Mens / HF fine-tune — LLM PR checklist for the Vox language. Detailed technical reference, architecture guid"
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
# Mens / HF fine-tune — LLM PR checklist

Use this when agents or humans touch **`vox-populi`** **Mens** training (`mens-train`), **merge commands**, **LoRA/QLoRA**, or **parity tests**. Goal: avoid typical **context-blind** mistakes (wrong crate, wrong layout, doc drift).

## Duplication and ownership

- [ ] **Two `lora.rs` trees:** `crates/vox-tensor/src/lora.rs` (primitives) vs `crates/vox-populi/src/mens/tensor/lora.rs` (transformer + merge). Fixes to **linear LoRA math** may need **both** or a deliberate consolidation. Canonical split: [`mens-lora-ownership.md`](../reference/mens-lora-ownership.md).
- [ ] **CLI / operator strings:** user-facing merge errors should stay aligned with `MERGE_QLORA_REJECTS_BURN_BIN` in `tensor/artifact_bridge.rs`; grep SSOT markdown when changing wording. Planner / QLoRA preflight gates share `tensor/operator_messages.rs` — update there when changing tokenizer or weight-path errors.

## Feature flags and API

- [ ] **`cfg(feature = "mens-train")`** on `vox-populi` exports (e.g. `MERGE_QLORA_REJECTS_BURN_BIN`): every binary that needs them must enable **`vox-populi/mens-train`** (see `vox-cli` `gpu` feature wiring).
- [ ] **Format strings:** wrapping `anyhow!` / `bail!` messages that contain `{` — escape as `{{` / `}}` where needed.

## Tensor layout (Burn vs Candle)

- [ ] **Matmul orientation:** state explicitly e.g. `x [batch, in] @ W [in, out]`; qlora-rs stores base weight as `[out_features, in_features]` and uses `input.matmul(&weight.t())`.
- [ ] **Bias broadcast:** Burn often needs `bias.reshape([1, out])`; Candle uses `broadcast_add` — confirm ranks.
- [ ] **Tolerances:** tight for shared **f32** primitives; loose / statistical for end-to-end training — never one global epsilon for everything.

## Tests and CI

- [ ] **CI job names vs runbook:** `.github/workflows/ci.yml` Mens steps should stay aligned with [`mens-finetune-acceptance-runbook.md`](mens-finetune-acceptance-runbook.md) (same `cargo test` filters, e.g. `execution_planner` not multiple filters on one line).
- [ ] **Strict QLoRA proxy stack:** regression `preflight_strict_rejects_missing_o_proj` must stay green when changing `qlora_preflight` / planner middle-key inventory.
- [ ] **CI job vs test binary:** `.github/workflows/ci.yml` `--test <name>` must match `crates/vox-populi/tests/<name>.rs` (or `src/…` integration tests as wired).
- [ ] **GPU-only tests {** must not be the **only** coverage for logic that also runs on CPU / NdArray.
- [ ] **Path edge cases:** e.g. `merge-qlora` `*.bin` detection — consider double extensions and Windows paths when adding guards.

## Documentation

- [ ] **Same change, two docs:** behavior visible to users should match **`AGENTS.md`** (Mens subsection) and **`docs/src/reference/mens-training.md`** where applicable.
- [ ] **NF4 wording {** Burn path is **f32 LoRA**; Candle **`--backend qlora`** is **qlora-rs NF4** — do not conflate in CLI blurbs.

## Vox web / training corpus

- [ ] **Express / `server.ts`:** treat **`VOX_EMIT_EXPRESS_SERVER=1`** as **legacy / opt-in** in training text; default story is **Axum + `api.ts`** (see [`vox-fullstack-artifacts.md`](../reference/vox-fullstack-artifacts.md)).
- [ ] **Examples:** prefer **golden** `examples/*.vox` from [`examples/README.md`](../adr/index.md); avoid ingesting `examples/archive/**` unless the pipeline explicitly opts in.

## Merge / attention

- [ ] **RoPE:** no silent merge to static `MultiHeadAttention`; `use_rope` stacks need explicit **unmerged serve** or documented limitation (see `LoraAttention::merge` rustdoc).

## Parity strategy (reminder)

| Tier | What it proves |
|------|----------------|
| **A** | Shared f32 ops: matmul, biased linear, CE (`candle_burn_*_parity` tests). |
| **B** | NF4 round-trip → **same** f32 tensor → Burn vs Candle matmul (`candle_burn_nf4_dequant_lm_reference_parity`). |
| **C** | Avoid: single tight tolerance on **full** NF4 proxy vs **full** Burn LM without identical graph and reference path. |

## Related

- [HF fine-tune gap matrix](../reference/hf-finetune-gap-matrix.md)
- [Mens training SSOT](../reference/mens-training.md)
- [HF fine-tune capability matrix](hf-finetune-capability-matrix.md)


