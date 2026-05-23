---
title: "Qwen3.5 Multimodal Phase 2 Backlog"
description: "Post–text-only Qwen3.5 work: vision/video tokens, config and TrainingPair/corpus contracts, candle QLoRA train and inference serve, merge metadata, CI fixtures, and phase-2 exit criteria."
category: "Architecture SSOTs"

schema_type: "TechArticle"
training_eligible: false
archived_date: 2026-04-18
---

# Qwen3.5 Multimodal Phase 2 Backlog

This backlog starts only after native text Qwen3.5 support is green in CI/dogfood.

## Scope boundary

- Phase 1 (current): native text-only Qwen3.5 (`0.8B/2B/4B/9B`) in train/merge/serve/gates.
- Phase 2 (this backlog): add multimodal (vision/video token path) for training and inference.

## Work items

1. Config and model layout extension
   - Extend multimodal config parsing in `crates/vox-populi/src/mens/tensor/hf_load.rs` for `vision_config` and token ids (`vision_start_token_id`, `vision_end_token_id`, `image_token_id`, `video_token_id`).
   - Add explicit architecture guard in preflight for text-only vs multimodal checkpoints.

2. Data contract and corpus pipeline
   - Extend `vox_tensor::data::TrainingPair` contract to include multimodal payload references and modality tags.
   - Add corpus extract/mix validation for multimodal source rows (required files, max media size, decode status).
   - Add deterministic JSONL schema checks in `vox-cli` corpus commands to reject malformed multimodal rows early.

3. Trainer graph integration
   - Add multimodal embedding ingestion in `crates/vox-populi/src/mens/tensor/candle_qlora_train/mod.rs` with strict feature gating.
   - Thread modality-aware masking and sequence assembly through training loop and validation.
   - Update manifest fields to include modality counters and multimodal preflight status.

4. Inference serve path
   - Extend `crates/vox-populi/src/mens/tensor/candle_inference_serve.rs` to accept multimodal prompt payloads.
   - Add modality-aware tokenization/packing and guardrails when requested modality is unsupported by loaded checkpoint.

5. Merge and artifact compatibility
   - Extend adapter metadata schema for multimodal capability flags.
   - Add merge validation for multimodal-sensitive keys and reject incomplete merges for multimodal checkpoints.

6. CI and regression coverage
   - Add synthetic multimodal fixture tests in `crates/vox-populi/tests`.
   - Add CI contract checks for multimodal schema + parser + preflight gates (without requiring large media artifacts).
   - Add optional nightly multimodal smoke for short-run finite-loss and artifact checks on GPU runners.

## Exit criteria for Phase 2

- Multimodal preflight rejects bad checkpoints/data with actionable diagnostics.
- Multimodal train path runs with finite loss and checkpoints in nightly smoke.
- Serve path can load multimodal-enabled artifacts and run basic generation.
- CI includes deterministic multimodal contract tests and no regressions in text-only Qwen3.5 paths.

