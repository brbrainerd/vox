---
title: "ADR 003 — Native Training Over Python"
category: adr
constructs: []
last_updated: 2026-03-02
training_eligible: false
difficulty: advanced
---

# ADR 003 — Native Rust Training Over Python

**Status**: Accepted (Python retained as fallback for large-model QLoRA)
**Date**: 2026-03-02
**Author**: Vox Core Team

---

## Context

The original Populi training pipeline used `populi/training/train.py` (Python, Unsloth, QLoRA). This caused:

1. **Environment friction**: Python version conflicts, uv/pip version pinning, CUDA version mismatches
2. **Slow iteration**: Python-based tokenizer was ~10× slower than native Rust
3. **Philosophical mismatch**: Vox cannot "dogfood" its own training if the training loop is in another language
4. **CI complexity**: Separate Python setup, uv installation, heavy deps install in every CI run

---

## Decision

**Move 95% of the training pipeline to native Rust (Burn 0.19), retaining Python only for QLoRA large-model fine-tuning.**

| Component | Before | After |
|-----------|--------|-------|
| Tokenizer | Python (`tokenizer.py`) | Rust (`VoxTokenizer` in `vox-tensor`) |
| Data loading | Python JSONL loop | Rust `JsonlDataLoader` |
| Data generation | `scripts/datagen.py` | `vox generate-data` (Rust) |
| Training loop | Python (`dogfood_train.py`) | `vox training native` (Burn) |
| Large-model QLoRA | Python (Unsloth) | **Python retained** |
| Corpus extraction | Python | `vox corpus extract` (Rust) |
| Training validation | Python | `vox corpus eval` (Rust via vox-eval) |

---

## Implementation

The native pipeline lives entirely in `crates/`:

```
crates/vox-tensor/
  src/data.rs      — VoxTokenizer + JsonlDataLoader
  src/nn.rs        — VoxTransformer model
  src/optim.rs     — AdamW + LinearWarmupScheduler
  src/train.rs     — Checkpoint + gradient_clip_norm

crates/vox-cli/src/training/
  mod.rs           — Corpus utilities + system prompt generation
  native.rs        — Training loop
  datagen.rs       — Synthetic data generation
```

---

## Consequences

**Positive:**
- Zero-Python install for development: `cargo run -p vox-cli -- training native`
- Native tokenizer is ~10× faster than Python equivalent
- Single binary: `vox` handles data gen, training, eval, corpus management
- Better Windows support (no Python env issues)
- Training data schema is enforced by Rust type system (`TrainingPair` struct)

**Negative:**
- Large pre-trained model access (Llama, Qwen) requires Python/HuggingFace hub
- QLoRA fine-tuning still requires Python for quantized adapter training
- Burn ecosystem less mature than PyTorch (fewer optimizers, no flash-attention yet)

---

## References
- `crates/vox-cli/src/training/datagen.rs` — Synthetic data generation
- `crates/vox-tensor/src/data.rs` — Native tokenizer + dataloader
- [Burn ML framework](https://burn.dev)
- `docs/src/expl-ml-pipeline.md` — Full pipeline documentation
