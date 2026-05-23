---
title: "Vox Language Focused Training SSOT"
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-17"
training_eligible: false
archived_date: 2026-04-18
---

# Vox Language Focused Training SSOT

This document defines the single source of truth (SSOT) for the `vox-lang` domain training pipeline. It consolidates the corpus ingestion strategy, QLoRA training parameters, and K-complexity constraints.

## 1. Corpus Strategy

The Vox Language corpus is managed exclusively via the `mens/config/mix-vox-lang.yaml` pipeline. 

### Canonical Sources
All valid JSONL files must live either in `target/dogfood/` (generated) or `mens/data/mix_sources/` (curated).

- **High-Leverage Source:** `organic_vox.jsonl`
  * Organic Vox programs written during real user sessions are the most valuable data.
  * Extracted via: `vox mens corpus extract --source organic`

### Decommissioned Sources
Empty or redundant legacy files have been deleted, including `golden_pairs.jsonl`, `golden_validated.jsonl`, `attention_dogfood.jsonl`, and various stale snapshots.

## 2. Training Pipeline (QLoRA)

The binding constraint for Vox-specific model quality is **data volume**, not tokenizer fertility. Thus, Continual Pretraining (CPT) and custom BPE extensions are strictly gated behind data thresholds.

### CPT Decision Gate
Do not attempt Continual Pretraining until all the following are true:
1. `organic_vox.jsonl` contains >100,000 high-quality, verified examples.
2. The eval loss plateau has been reached via standard QLoRA despite increasing data volume.
3. Context window saturation (fertility inefficiency) is actively degrading target workflows.

### LoRA Configuration
For `vox-lang` adaptation, we target **all linear layers** to maximize knowledge absorption (as opposed to just `q_proj` and `v_proj`).
- Target modules: `q, k, v, o, gate_proj, up_proj, down_proj`
- Configuration via `QLoraConfig::preset_all_bf16(rank, alpha)` or equivalent in `candle_qlora_train.rs`.

### Eval Gate
The `mens/data/golden_extracted.jsonl` file contains a curated set of golden Vox programs. This must be used via `vox mens corpus eval` before and after every training run to track the validation loss curve.

## 3. Language K-Complexity Standardization

To maximize tokenizer efficiency and eliminate split-brain ambiguities in the training data, the Vox language syntax adheres to the following constraints:

1. **Brace Syntax Only**: Colon-based block syntax (`:`) is deprecated (v0.2 legacy). All blocks must use brace syntax `{}` (v0.4+ standard).
2. **Canonical Keywords**: The `ret` keyword is deprecated. `return` is the sole canonical keyword.
3. **Canonical Decorators**: Legacy decorators like `@component` have been removed from the language surface and training data. The `@table`, `@query`, and `@mutation` decorators remain unchanged due to their high value.

