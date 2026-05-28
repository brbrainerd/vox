---
title: "ADR 007: qlora-rs multi-layer training API (Phase 2c architecture gate)"
description: "Official documentation for ADR 007: qlora-rs multi-layer training API (Phase 2c architecture gate) for the Vox language."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---

# ADR 007: qlora-rs multi-layer training API (Phase 2c architecture gate)

## Status

**Accepted** — 2026-03-21. In-tree native Candle QLoRA (`vox mens train --backend qlora`) may expand from the current **single `QuantizedLinear` (LM head)** path to **multiple quantized layers** without forking **qlora-rs** 1.0.5, subject to graph construction work in **`vox-populi`** (`mens::tensor`).

## Context

- Workspace pins **`qlora-rs = "1.0.5`** (`Cargo.toml` `[workspace.dependencies]`).
- Today, `candle_qlora_train.rs` builds **one** [`QuantizedLinear`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/qlora/struct.QuantizedLinear.html) for the LM head and calls [`QLoraTrainer::training_step_lm`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/training/struct.QLoraTrainer.html#method.training_step_lm) with `layers: &[&QuantizedLinear]` of length **1**.
- Phase 2c (full-graph QLoRA) needs a clear answer: does **qlora-rs** support **one shared trainer + optimizer** over **many** `QuantizedLinear` modules in one step?

## Decision

**Approach A (chosen): extend the in-tree trainer using only public qlora-rs APIs.**

### Multi-layer / shared optimizer

Source audit (`qlora-rs` 1.0.5 `src/training.rs`):

1. **`QLoraTrainer::init_optimizer(&mut self, layers: &[&QuantizedLinear]) -> Result<()>`**  
   - Initializes **paged or standard AdamW** from **all variables** in the trainer’s **`VarMap`** (`self.varmap.all_vars()` / `data().lock()`).  
   - The `layers` slice is **not** used to enumerate parameters for the paged path beyond a discarded `layers.len()`; trainable weights are whatever was registered when layers were built with **`trainer.var_builder()`**.

2. **`training_step` / `training_step_lm`**  
   - Signature: `layers: &[&QuantizedLinear]`, `input`, `targets` / `target_ids`.  
   - Forward: `let mut logits = input.clone(); for layer in layers { logits = layer.forward(&logits)?; }`  
   - So **multiple** `QuantizedLinear` refs are **first-class**: one backward pass over the **sequential** composition, then optimizer step on **all** LoRA params in the `VarMap`.

**Implication:** Vox can register **N** layers (each constructed with the **same** trainer’s `var_builder()` under distinct prefixes, e.g. `vb.pp("layers.0")`, …), pass `init_optimizer` a slice of references to those layers, and pass the **same slice** to `training_step_lm` each step — **no** qlora-rs fork required for multi-module training, as long as the **forward graph** matches that sequential contract (or is refactored into a single forward that internally applies the same layers in order).

**Not chosen (unless future evidence contradicts the above):**

- **B)** Hybrid Candle forward + manual adapter grads for extra layers — only if a future qlora-rs release removes multi-layer `training_step_lm` or breaks `VarMap` registration.
- **C)** Fork / replace qlora-rs — last resort; would require ADR revision and pin policy update.

### Double quantization

[`QLoraConfig`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/qlora/struct.QLoraConfig.html) embeds [`QuantizationConfig`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/quantization/struct.QuantizationConfig.html) with **`double_quant: bool`**.

- Defaults and presets in qlora-rs 1.0.5 set **`double_quant: true`** (e.g. `QLoraConfig::default()`, `preset_all_bf16`, `preset_qv_bf16`).
- Vox today uses **`QLoraConfig::preset_qv_bf16`** in `candle_qlora_train.rs`, so **double quant is already on** for the shipped LM-head path.  
- User-visible toggles or documentation gaps are **product** follow-ups, not an API blocker.

## Consequences

- **Milestones 3–4** (multi-layer forward + training loop) should prefer **one `QLoraTrainer`**, **N** `QuantizedLinear` layers from **`var_builder()`**, **`init_optimizer(&layers)`**, **`training_step_lm(&layers, …)`**.
- **Telemetry / manifest** must stop hard-coding `n_layers: 1` / `n_heads: 1` once real layout is threaded from HF `config.json` (see `HfTransformerLayout` in `vox_populi::mens::tensor::hf_load` and SSOT).
- If qlora-rs is upgraded, **re-verify** `training.rs` forward loop and `init_optimizer` behavior before relying on this ADR.

## References

- Crate: `qlora-rs` 1.0.5 (`training.rs`, `qlora.rs`).
- SSOT: [`mens-training.md`](../reference/mens-training.md) — § Full-graph QLoRA design.


