---
title: "Acceptance runbook — Mens HF fine-tune convergence"
description: "Official documentation for Acceptance runbook — Mens HF fine-tune convergence for the Vox language."
category: "Language Reference"
last_updated: "2026-03-24"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---
# Acceptance runbook — Mens HF fine-tune convergence

## Preconditions

- GPU-capable build: `vox-cli` with `gpu` (**`vox-populi`** `mens-train`, includes Candle qlora-rs).
- Corpus: `train.jsonl` from `vox mens corpus pairs …` or `vox mens corpus mix …` (optional `record_format: tool_trace` for tool/command supervision rows).

## Command matrix (smoke)

| # | Command | Pass criteria |
|---|---------|----------------|
| 1a | `cargo test -p vox-populi --features mens-train execution_planner` | Planner + Candle proxy inventory gates |
| 1b | `cargo test -p vox-populi --features mens-train hf_keymap` | HF key naming / Qwen middle keys |
| 1c | `cargo test -p vox-populi --features mens-train training_text` | ChatML / text policy |
| 1d | `cargo test -p vox-populi --features mens-train preflight_strict_rejects_missing_o_proj` | Strict `--qlora-require-full-proxy-stack` path fails closed on missing middle keys |
| 2 | `cargo test -p vox-populi --features mens-train burn_full_graph_smoke` | Forward shape smoke OK |
| 3 | `cargo test -p vox-populi --features mens-train lora_vox_transformer_checkpoint_roundtrip` | Burn `Checkpoint` bin save/load preserves logits |
| 4 | `cargo test -p vox-populi --features mens-train merged_vox_transformer_matches_lora_full_forward` | `LoraVoxTransformer::merge` forward matches LoRA forward |
| 5 | `cargo test -p vox-populi --features mens-train --test candle_burn_f32_matmul_parity` | Candle CPU vs Burn NdArray f32 matmul aligned |
| 6 | `cargo test -p vox-populi --features mens-train --test candle_burn_f32_linear_lm_logits_parity` | Candle vs Burn f32 biased linear (LM-head-shaped logits) |
| 7 | `cargo test -p vox-populi --features mens-train --test candle_burn_cross_entropy_parity` | Candle vs Burn CE scalar on same logits |
| 8 | `cargo test -p vox-populi --features mens-train --test candle_burn_nf4_dequant_lm_reference_parity` | Tier B: NF4 round-trip then shared f32 LM-linear parity |
| 9 | `cargo test -p vox-tensor --features gpu --lib linear_warmup_sequence_matches` | LR warmup matches Burn linear scheduler |
| 10 | `cargo test -p vox-cli merge_` | merge guards + merge-qlora roundtrip + Burn `*.bin` rejection on merge-qlora |
| 11 | `vox mens train --backend lora --data-dir … --output-dir …` | Completes, `training_manifest.json` has `execution_kernel` = `burn_lora` |
| 12 | `vox mens train --backend qlora --tokenizer hf --model <hf> …` | Completes, `populi_adapter_manifest_v3.json` written |
| 13 | `vox ci mens-gate --profile m1m4` (or `cargo run -p vox-cli -- ci mens-gate --profile m1m4` in CI) | M1–M4 subset + corpus `tool_trace` mix tests pass |

## Sign-off

- Burn: GPT-2-shaped HF tokenizer path trains without planner error.
- Candle: NF4 path unchanged functionally; telemetry includes `candle_compat_mode: true`.
- Merge: `merge-qlora` accepts v2 or v3 adapter meta.


