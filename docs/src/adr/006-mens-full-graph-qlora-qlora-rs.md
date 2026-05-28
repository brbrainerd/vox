---
title: "ADR 006: Mens full-graph Candle QLoRA with qlora-rs"
description: "Official documentation for ADR 006: Mens full-graph Candle QLoRA with qlora-rs for the Vox language."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-24"
training_eligible: true

schema_type: "TechArticle"
---
# ADR 006: Mens full-graph Candle QLoRA with qlora-rs

## Status

Accepted (2026-03-21)

## Context

Mens ships native `--backend qlora` using **qlora-rs** 1.0.5 and Candle: a frozen mmap `f32` embedding table (`wte` / `model.embed_tokens.weight`) for context, plus one or more **NF4** [`QuantizedLinear`](https://docs.rs/qlora-rs) modules trained via [`QLoraTrainer::training_step_lm`](https://docs.rs/qlora-rs) (**sequential** stack when HF shards include every expected block output projection; otherwise **LM head only**).

Product goals (Phase 2c) require **deeper** use of base weights: per-layer attention output projections (and eventually broader coverage), **multi-tensor adapter export**, optional **merge** into base-shaped `f32` shards, and clarity on **double quantization**.

## Decision

1. **Training API (Approach A — in-tree, public qlora-rs only)**  
   qlora-rs [`training_step_lm`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/training/struct.QLoraTrainer.html#method.training_step_lm) accepts `layers: &[&QuantizedLinear]` and applies them **sequentially** (`for layer in layers { logits = layer.forward(&logits)? }`). The optimizer is initialized from the trainer’s **single** `VarMap`, so **multiple** `QuantizedLinear` layers created with distinct `VarBuilder` prefixes are supported without forking qlora-rs.

2. **Full-graph scope (incremental)**  
   We expand the trainer by stacking **optional** middle blocks loaded from HF safetensors when present:
   - **GPT-2**: `h.{i}.attn.c_proj.weight` — shape `[d_model, d_model]`.
   - **Qwen2 / LLaMA-style** (`model_type` / `architectures` containing `Llama`, `Qwen`, `Mistral`, etc.): `model.layers.{i}.self_attn.o_proj.weight` — shape `[d_model, d_model]`.  
   If no per-layer weights are found, behavior falls back to the **LM-only** path (backward compatible).

   This is **not** a full causal transformer forward (no MHA/FFN block yet); it is the **supported bounded proxy v1** (`candle_qlora_proxy_v1` in manifests / `training_objective_note`), including optional suffix LM via **`--qlora-ce-last-k`** (see [mens-training.md](../reference/mens-training.md)). Naming in telemetry: `trainable_projection_stack` / `candle_qlora_graph_id`.

3. **Double quantization**  
   [`QLoraConfig`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/qlora/struct.QLoraConfig.html) embeds [`QuantizationConfig`](https://docs.rs/qlora-rs/1.0.5/qlora_rs/quantization/struct.QuantizationConfig.html) with `double_quant: bool`. Presets (`preset_qv_bf16`, etc.) default `double_quant: true`. Mens exposes a CLI flag to **disable** double quant for debugging; default remains **on** (paper-style).

4. **Burn LoRA + HF tokenizer**  
   Burn training consumes **VoxTokenizer** JSONL via `vox_tensor::data::load_all`. Wiring Hugging Face tokenization into the Burn path would require a parallel data pipeline and is **deferred**. CLI continues to reject `--backend lora` + `--tokenizer hf` with a message pointing to `--backend qlora`.

5. **Adapter format v2 + merge**  
   Adapters export LoRA matrices per logical layer (`mid0`, …, `lm_head`) with sidecar JSON mapping adapter prefixes → base safetensors keys. `vox schola merge-qlora` merges LoRA deltas into **f32** base tensors for those keys (reload for inference outside this ADR).

## Consequences

- Root [`Cargo.toml`](../../../Cargo.toml) must keep `qlora-rs` workspace pin aligned with **`vox-populi`** optional deps (`mens-candle-qlora`).
- SSOT: [`mens-training.md`](../reference/mens-training.md) and [`ref-cli.md`](../reference/cli.md) must list `merge-qlora` and `--qlora-no-double-quant`.
- CI: `cargo test -p vox-populi --features mens-train` and targeted `vox-cli` tests cover export/merge smoke paths.

## References

- qlora-rs 1.0.5 `src/training.rs`, `src/qlora.rs` (local registry copy)
- QLoRA paper: <https://arxiv.org/abs/2305.14314>


