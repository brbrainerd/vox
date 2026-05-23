---
title: "Contributing — Mens native training"
description: "Onboarding for vox-populi tensor, vox mens train paths, and the golden corpus flywheel."
category: "How-To Guides"
last_updated: "2026-04-17"

schema_type: "HowTo"
---

# Contributing — Mens training (native)

This guide covers how to contribute to the `vox mens train` pipeline and how to ensure your code changes feed the MENS training loop correctly.

## Read first

- [Mens native training SSOT](../reference/mens-training.md)
- [HF finetune capability matrix](../archive/research-2026-q1/hf-finetune-capability-matrix.md)
- [The Vox contribution loop](../contributors/contribution-loop.md)

## Entrypoints

| Surface | Location |
|---------|----------|
| CLI | `vox mens train` → `crates/vox-cli/src/commands/schola/train/` |
| Library | `vox_populi::mens::tensor::run_mens_training` (`lora_train.rs`) |
| Contract | `FineTuneContract`, `ExecutionPlanner`, `preflight_train` |

## The Corpus Validate Flow

MENS training relies on high-quality `.vox` examples. When you add or modify code in `examples/golden/*.vox`, it must pass validation before being ingested.

The flow is:
1. `examples/golden/*.vox` (human or agent written)
2. `vox corpus validate-batch` (CI pipeline check)
3. Output: `golden_validated.jsonl` (ready for training)

## What Training-Eligible Code Looks Like

To ensure your code becomes a positive training example:
- **Parse Rate:** The code must pass the parser 100% cleanly. `vox corpus eval --mode ast` must succeed.
- **Test Blocks:** Use `@test` blocks to validate logic. The AST coverage and test pass rates will be used in future GRPO reward shaping.
- **No Stubs:** Zero `todo!()`, `unimplemented!()`, or empty function bodies.
- **Rich Constructs:** Use idiomatically correct Vox patterns.

## Commands

**Run the training planner tests:**
```bash
cargo check -p vox-populi --features mens-train
cargo test -p vox-populi --features mens-train execution_planner
```

**Validate the golden corpus locally:**
```bash
cargo run -p vox-cli -- corpus eval --mode ast examples/golden/
```

## Definition of Done

A PR contributing to the MENS pipeline is "done" when:
- `cargo test -p vox-populi --features mens-train` is green.
- No new parse failures are introduced to the golden corpus.
- TOESTUB reports zero stubs or god-object violations in the touched code.
- Any new CLI flags are documented in the SSOT.

## SSOT rule

**Candle QLoRA** is the active `vox mens train` backend; keep docs and error messages aligned (`lora_train.rs` is authoritative when in doubt).

