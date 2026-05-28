---
title: "Burn Framework Necessity Audit (2026-05-08)"
description: "Does the Vox codebase need Burn at all? Production fine-tuning uses Candle; Burn is legacy NdArray dogfood + 4,526 LOC of LoRA scaffolding behind a feature flag. Recommendation: delete unless cross-vendor GPU is a roadmap commitment."
category: "Architecture SSOTs"
status: "deprecated"
training_eligible: true
training_rationale: "Snapshot of ML framework usage; clarifies Burn vs Candle roles and proposes a deletion path with an explicit fork point for cross-vendor GPU."
---

# Burn Framework Necessity Audit

> **Status: EXECUTED 2026-05-08.** Burn deleted from the codebase per Option A. See commit history on `claude/infallible-lalande-baf300` for the deletion sequence. Deleted: `vox-plugin-tensor-burn-wgpu`, Burn parity tests, `vox-populi` Burn LoRA + `burn_stack`, `vox-mens` native trainer + `merge-weights` CLI, all Burn modules in `vox-tensor` (~4,550 LOC removed). The `vox-tensor` crate now contains only pure-CPU data loaders. Workspace deps `burn` and `wgpu` removed.

> User question: *"Audit if we even need burn for anything really in this code base. We started out with fine tuning QWEN 3.5 and I think that mostly just uses Candle. Do we need anything for burn now or in the future?"*

**TL;DR**: The user's recollection is correct. Production Qwen 3.5 QLoRA fine-tuning runs on **Candle**, not Burn. Burn is **already labeled "legacy"** in the codebase's own doc-comments and is gated behind a `mens-dei` feature flag. Total Burn-touching surface: **~4,526 LOC** across 11+ files. Recommendation: **delete Burn entirely** unless cross-vendor GPU training (AMD / Intel / Apple Silicon) is an explicit roadmap commitment for the next 6 months. If yes, delete from CORE and keep only inside the `vox-plugin-tensor-burn-wgpu` plugin (currently empty scaffold).

---

## 1. Where Burn lives today

| Location | Role | LOC |
|---|---|---:|
| `crates/vox-tensor/src/{tensor,vox_nn,train,optim,lora,lora_config,grpo}.rs` | Burn-based tensor library, GPU-feature-gated | ~2,500 |
| `crates/vox-populi/src/mens/tensor/burn_stack.rs` | `TransformerBlock<B: Backend>`, merged-weight VoxTransformer | ~600 |
| `crates/vox-populi/src/mens/tensor/lora/` | LoRA layers in Burn (attention, block, vox transformer, GPU tests) | ~700 |
| `crates/vox-mens/src/training/native.rs` | Burn NdArray training loop (CPU dogfood) | ~400 |
| `crates/vox-mens/src/commands/mens/merge_weights.rs` | Merge Burn LoRA checkpoints into a Burn `VoxTransformer` | ~120 |
| `crates/vox-populi/tests/candle_burn_*_parity.rs` (×4) | Burn↔Candle op equivalence tests | ~200 |
| `crates/vox-plugin-tensor-burn-wgpu/` | Empty plugin scaffold; SP7 deferral | ~30 |
| **Total** | | **~4,550 LOC** |

Heavy workspace deps gated behind these features: `burn` (autodiff + wgpu + train), `vox-tensor/gpu`, `wgpu` (when `mens-gpu` is on). All transitively pulled when `mens-train` is enabled.

---

## 2. Which Vox training path actually fine-tunes Qwen 3.5?

### Production: Candle (NOT Burn)

`vox mens train --backend qlora` → `vox-plugin-mens-candle-cuda/src/candle_qlora_train/` is the canonical Qwen 3.5 fine-tuning path. Reads HF-format models, runs QLoRA via `qlora-rs`, writes adapter manifests as v3. Has Qwen35 attention block (full + linear), partial-rotary RoPE, NF4 dequant, the whole stack.

Source: `crates/vox-mens/src/commands/ai/train.rs` shells out to `vox mens train --backend qlora --tokenizer hf`.

### Legacy: Burn (explicitly labeled so by the codebase)

From the doc-comment of `crates/vox-mens/src/commands/ai/train.rs`:

> *"**`--native`** (legacy Burn scratch trainer behind `mens-dei`)"*

`vox train --native` → `run_native()` → `vox_mens::training::native::run_training()` uses Burn's NdArray (CPU) backend by default. This is a from-scratch transformer trained on Vox's own corpus for dogfood / smoke testing, not for shipping fine-tuned Qwen 3.5 weights. Operates on a tiny `VOCAB_SIZE` from `vox-tensor`'s data module.

`vox mens populi merge-weights` calls `merge_weights::run_merge_weights` which merges Burn LoRA checkpoints into a Burn `VoxTransformer`. This is the output of the Burn-based native training, NOT the Candle QLoRA training (Candle has its own merge path in `vox-plugin-mens-candle-cuda/src/merge.rs` which writes v3 manifests).

So there are TWO parallel training stacks:
- Candle (production, Qwen 3.5)
- Burn (legacy, dogfood, NdArray CPU)

with their own merge tools, their own checkpoint formats, their own LoRA implementations.

---

## 3. Why Burn was originally there (best inference)

Looking at file history and naming, Burn was added when Vox planned a cross-vendor GPU story (`burn-wgpu` runs on Vulkan / Metal / DX12 → AMD, Intel, Apple Silicon, not just NVIDIA). The pure-Rust JIT GPU kernel story (CubeCL, which Burn uses) was also attractive vs. Candle's nvcc-dependent CUDA backend.

Then the Qwen 3.5 work happened, the Candle path matured, the Burn path didn't, and the Candle path became the production option. The Burn code stayed in tree behind feature flags.

The empty `vox-plugin-tensor-burn-wgpu` plugin was the next-step scaffolding for properly extracting Burn into a plugin — never finished.

---

## 4. Three questions for the recommendation

### Q1: Does anything that's NOT Burn require it?

No. Production training (Candle), inference (Candle), runtime (cudarc-via-Candle), tokenization (`tokenizers` crate, no Burn dep), data loading (`vox-tensor/data` module is pure CPU, no Burn deps active when GPU feature is off).

The only crate with a non-trivial Burn-gated public surface is `vox-tensor`. Its always-compiled side (the `data` module: `JsonlDataLoader`, `VOCAB_SIZE`, `TrainingPair`) is consumed by `vox-corpus`, `vox-mens`, and the candle-cuda plugin. **Those consumers don't import any Burn types** — only the data module's pure-CPU types. Burn could be removed without breaking them.

### Q2: Is the `vox train --native` path used by anyone?

It exists. Whether it's used in practice is a question only the user can answer. Signals that suggest no:

- Doc-comment self-labels it "legacy".
- Behind `mens-dei` feature flag (off by default in normal builds).
- NdArray CPU backend means it's slow and probably unsuitable for serious training.
- Ships next to the actively-maintained Candle path which does what users actually need.

If the answer is "we use it for daily smoke tests", that changes the recommendation. If the answer is "I don't think anyone has run it in months", that's the death sentence.

### Q3: Is cross-vendor GPU training a near-term roadmap goal?

This is the only **future** reason to keep Burn:
- Burn-wgpu trains on AMD / Intel / Apple Silicon via Vulkan / Metal / DX12.
- Candle's CUDA backend is NVIDIA-only. Candle has CPU + Metal-via-Accelerate, but no AMD/Intel.

**If** Vox's roadmap commits to "Vox should fine-tune on Apple Silicon Macs and AMD machines", Burn-wgpu (or CubeCL) is the only real option today. Candle Metal exists but is incomplete vs. Burn's wgpu.

**If not** — if NVIDIA-CUDA + CPU fallback is enough — Burn's reason to exist evaporates.

---

## 5. Recommendation tree

### Option A — DELETE Burn entirely (recommended unless A/I roadmap explicitly says otherwise)

Delete the following:

```
crates/vox-tensor/src/{tensor,vox_nn,train,optim,lora,lora_config,grpo}.rs
crates/vox-tensor/Cargo.toml — drop [features.gpu], [features.train], burn dep
crates/vox-populi/src/mens/tensor/burn_stack.rs
crates/vox-populi/src/mens/tensor/lora/   (entire dir)
crates/vox-populi/Cargo.toml — drop mens-gpu feature, burn dep, wgpu dep
crates/vox-mens/src/training/native.rs
crates/vox-mens/src/commands/ai/train.rs — drop --native flag + run_native()
crates/vox-mens/src/commands/mens/merge_weights.rs (delete; Candle has its own merge)
crates/vox-populi/tests/candle_burn_*_parity.rs (×4 files; lose regression net but regress against what?)
crates/vox-plugin-tensor-burn-wgpu/   (empty scaffold; delete entirely + remove from catalog)
```

Plus catalog cleanup (remove `tensor-burn-wgpu` from `crates/vox-plugin-catalog/catalog.toml`).

What survives:
- `vox-tensor/src/{data,replay}.rs` (always-compiled CPU data + replay; no Burn deps)
- `vox-tensor/Cargo.toml` becomes a tiny shared crate
- The Candle stack stays exactly as is

**Wins**:
- ~4,500 LOC removed
- `burn`, `burn-train`, `burn-wgpu`, `wgpu`, `cubecl`, `autodiff` machinery gone from workspace dep tree
- One fewer ML framework to maintain version-pin against
- Faster `cargo check` / `cargo build` for users in workspace
- Eliminates the parallel "two trainers, two merge tools, two checkpoint formats" cognitive tax

**Risks**:
- Lose `vox train --native` (legacy path; if anyone uses it, they need Candle CUDA or remote)
- Lose `vox mens populi merge-weights` for Burn checkpoints (Candle has its own merge)
- Lose Burn↔Candle parity tests (they catch upstream Burn/Candle drift; without Burn there's nothing to drift against)
- Closes the door on cross-vendor GPU training **in core**. Plugin door stays open (Option B).

**Effort**: M (~half day, mostly mechanical deletion + import cleanups + feature flag removal). Most affected file count: small.

### Option B — Move Burn into the plugin only

If you want to keep cross-vendor GPU training as a *future* option without paying for it in CORE today:

1. Move all Burn code into `vox-plugin-tensor-burn-wgpu/` (currently empty scaffold).
2. Remove Burn deps from `vox-tensor`, `vox-populi`, `vox-mens`.
3. Delete the `--native` legacy CLI path (or move it into the plugin too).
4. Plugin becomes opt-in via `vox plugin install tensor-burn-wgpu`. Default builds don't see Burn.

**Wins**:
- All of Option A's wins for default builds
- Future-proofs cross-vendor GPU as an opt-in capability

**Risks**:
- Bigger refactor than deletion (need to extract LoRA into the plugin, port `merge_weights` callers to plugin dispatch or delete them)
- Plugin remains an SP7 scaffold; finishing it has been deferred multiple times

**Effort**: L (1+ day; the architectural blocker is `LoraVoxTransformer<B: Backend>` generic types crossing the plugin ABI boundary — either redesign the plugin's surface to be non-generic or accept that Burn callers must dep on the plugin's rlib).

### Option C — Keep Burn as-is

Status quo. The cost is the ~4,500 LOC and the dep tree weight; both are gated behind feature flags so they don't hurt default builds. The cognitive tax (two trainers, two merges, two checkpoint formats) is the real ongoing cost.

**Wins**: zero work today.
**Risks**: keeps growing over time; gates stay until someone deletes them.

---

## 6. My recommendation

**Option A — delete Burn entirely** unless the answer to "is cross-vendor GPU training a real near-term commitment?" is an unambiguous yes.

Reasons:
- Codebase already self-labels the Burn path "legacy"
- Production Qwen 3.5 fine-tuning is Candle, period
- Burn's value (cross-vendor GPU) requires the `tensor-burn-wgpu` plugin to be finished; the scaffold has been "deferred" through multiple SP rounds
- The parity tests are valuable only insofar as both frameworks coexist
- ~4,500 LOC of dead/legacy code is real maintenance debt

If the answer to cross-vendor GPU is yes, Option B is the pragmatic alternative — keep Burn, but ONLY in the plugin where its weight is opt-in.

Either way, the answer is **NOT** Option C (status quo). The current state is paying ongoing weight for something nobody invokes.

---

## 7. Open questions for the user

1. Has anyone run `vox train --native` (the Burn dogfood path) in the last 90 days?
2. Is fine-tuning on Apple Silicon / AMD GPUs a roadmap goal for the next 6 months? (Yes → Option B, finish the Burn plugin. No → Option A, delete.)
3. Is `vox mens populi merge-weights` for Burn checkpoints exercised by anyone? (The Candle path has its own merge that writes v3 manifests; the Burn one is independent.)
4. Are the 4 Burn↔Candle parity tests catching anything in CI today, or are they passing trivially because both frameworks are stable upstream?
