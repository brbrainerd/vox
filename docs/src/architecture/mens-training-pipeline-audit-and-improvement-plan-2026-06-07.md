---
title: "MENS training pipeline — audit + improvement scoping (2026-06-07)"
description: "Read-only audit of the MENS QLoRA trainer: checkpoint/resume reality, the crash-at-checkpoint diagnosis, a ranked single-GPU VRAM/throughput backlog, and the verdict on mesh/Populi LAN-distributed training — with scoped plans for auto-recovery, a VRAM bundle, and federated-LoRA-over-LAN."
category: "Architecture SSOTs"
---

# MENS training pipeline — audit + improvement scoping (2026-06-07)

Companion to [Qwen3.7 profile + MENS 4B feasibility](./qwen-3.7-profile-and-mens-4b-feasibility-2026-06-07.md). This is an **audit + scoping** document — no trainer code has been changed. Three work tracks are scoped at spec level (goal / approach / files / effort / risks / acceptance / open questions); each can be expanded into a full bite-sized TDD plan in a worktree on request.

Live trainer: `crates/vox-plugin-mens-candle-cuda` (the `mens-training.md` file map is stale — the trainer is **not** in `vox-populi`). Budget/planner: `crates/vox-populi/src/mens/tensor/`. Quant lib: patched `patches/qlora-rs-1.0.5`.

---

## Part 1 — Audit findings

### 1.1 "Checkpoint" means two different things
The earlier advice conflated them. Corrected:

| Concept | Status |
|---|---|
| **Model checkpointing** (save state + resume) | **Exists, solid.** `--resume <dir>`, auto-detect `CheckpointState::load`, atomic temp-rename, last-good preserved, exact shuffle/step/epoch restore + index sanitization. `training_loop/checkpoint.rs:14-91`; state at `checkpoint_state.rs`. |
| **Gradient checkpointing** (activation recompute) | **Absent.** Named by the budget code itself as the missing piece for 3B+ (`memory_budget.rs:108-111`). |
| **Crash auto-recovery** (relaunch after crash) | **Absent.** No retry/relaunch loop; a save/step error propagates via `?` and kills the process. |

### 1.2 The "crash at every/every few checkpoints" is NOT a save-time spike
`save_adapter` writes **only the ~137 MB LoRA adapter** — no dequantize, no merge, no `to_dtype` (`patches/qlora-rs-1.0.5/src/training.rs:600-611`). The classic "save merges to BF16 and spikes VRAM" failure does not apply. Real causes, in likelihood order:

1. **Zero-margin 4B-on-16GB.** Over-budget by design (`memory_budget.rs:27-31`); any transient fragmentation is fatal and coincidentally lands near a save.
2. **Disk exhaustion.** A real recorded incident: "~70 files / ~13 GB → 'no space'" (`checkpoint_mid.rs:52-53`). Pre-prune builds fill the disk with 137 MB adapters every checkpoint. Retention default is now 3 (`VOX_MENS_KEEP_CHECKPOINTS`).

Resume restarts AdamW momentum (optimizer state is not checkpointed — `CheckpointState` carries only step/epoch/offset/shuffle/loss) — a minor convergence blip, not a correctness issue.

### 1.3 Single-GPU improvement backlog (ranked impact ÷ effort)

| # | Improvement | Status | Impact | Effort | Evidence |
|---|---|---|---|---|---|
| 1 | **BF16 activations** | absent (F32) | ~½ activation VRAM → +50-80% seq/batch. Biggest lever. | L | `qlora.rs:442-446` casts base matmul back to F32; all attn/MLP/RMSNorm F32 |
| 2 | **Quantize/BF16 embedding table** | absent (F32 resident) | ~3.3 GiB for 4B (vocab 248k × 3584 × 4B) — most of the "mystery" resident; save ~2-3 GiB | M | `logic.rs:460`, `model.rs:502` |
| 3 | **Wire gradient clipping** | **BUG (no-op)** | Stability (loss-spike protection) | S | `training.rs:642-643, 733-735` `let _ = max_norm; // Placeholder` |
| 4 | **Retune calibration constants** | empirical fudge factors | Convert freed VRAM into bigger budgeted configs | S | `memory_budget.rs:24-46` |
| 5 | **Gradient checkpointing** | absent | The real 4B-on-16GB unlock | L | Candle 0.9 has no autograd-checkpoint API → manual re-forward |
| 6 | **Flash/SDPA attention** | absent (naive O(seq²)) | Cuts the seq² activation term → longer context | L | no `candle-flash-attn` dep |

Supporting notes: `batch_size` is fiction (forward hard-codes batch=1, `forward.rs:24`); the paged-optimizer "doesn't write back" rationale is **stale** (fixed in patched qlora-rs `training.rs:660,890`) — paging is correctly off only because LoRA state is tiny; contradictory weight-cache comments at `mod.rs:469-479` (code: cache **on** by default); plugin-trait stubs (`run_train_step`/`run_eval_step`/`load_from_path`) bypass the real path and are debt under the no-stubs policy.

**Key insight:** the ~15.6 GiB "resident" for 4B is **not** 4-bit weights (those are ~2.2 GB). It is F32 embeddings (~3.3 GB) + BF16 weight cache (~8 GB) + overhead. Fixing #1 and #2 lowers the resident wall — arguably a more direct path to fitting 4B than #5 alone.

### 1.4 Mesh / Vox Populi distributed training — verdict
**No cross-machine (or even cross-GPU) training exists today.** `vox-distributed-training` (and its duplicate in `vox-populi/src/distributed_training/`) is a **contract-only stub**: signed Ed25519 gradient/checkpoint *envelopes* with no tensors; `all_reduce` returns `AllReduceUnsupported` (`strategy/data_parallel.rs:83`); `step()` returns loss 0.0; **no in-tree consumers**. The live trainer hardcodes `Device::new_cuda(0)` (`device_select.rs:28-40`) — single-device, zero NCCL/cudarc-multi hooks. The plan (Mn-T1..T15, `mesh-mens-distributed-training-and-execution-plan-2026.md`) admits its op-log all-reduce MVP would be "slower than NCCL by orders of magnitude."

LAN technique feasibility (consumer 1-10 GbE, 16 GB cards):

| Technique | Syncs/step | Verdict on LAN |
|---|---|---|
| Full-model DDP all-reduce | ~14-16 GB | Hopeless |
| FSDP / ZeRO | more than DDP | Hopeless (and it's the only thing that shrinks per-node VRAM) |
| Tensor parallelism | per-layer activations | Hopeless (needs NVLink) |
| Pipeline parallelism | stage activations | Marginal/complex; no Candle scheduler |
| **Federated / data-parallel LoRA-delta sync** | **single-digit MB** | **Easy — the realistic path** |

**Bottom line:** federated LoRA-delta averaging buys **throughput / more data per hour**, not a bigger model — each node still holds the full base resident in its own 16 GB. "Train a model too big for one 16 GB card" needs sharding (FSDP/TP/PP), which is bandwidth-hostile and impractical on consumer LAN; Candle is serving-first with no FSDP equivalent.

---

## Part 2 — Scoped plans

### Track A — Auto-recovery wrapper (cheapest, highest leverage)

**Goal:** A crashed run relaunches itself from the last checkpoint with no human in the loop, capped to avoid infinite loops.

**Approach:** All resume machinery already exists; the only gap is an **outer relaunch loop**. Project policy mandates automation as `.vox` (not `.ps1`/`.sh`). Add a new launcher `scripts/mens/train_resilient.vox` that wraps `vox mens train`, re-invoking with `--resume <out-dir>` on non-zero exit, with a max-attempts cap and a backoff that escalates VRAM pressure relief (e.g. trim retention, drop seq-len one rung) before giving up. Follows the existing `process.run` idiom in `scripts/mens/run_4080_cycles.vox` (returns `Option` with `.code`).

Illustrative sketch (scoping only):
```vox
// vox:skip — illustrative scoping sketch, not yet an in-tree script
fn train_once(out: str, extra: list[str]) -> int {
    let mut args = ["mens", "train", "--resume", out]
    for a in extra { args.push(a) }
    match process.run("vox", args) {
        Some(res) => res.code,
        None => 1,
    }
}

fn main() {
    let out = "mens/runs/latest"
    let max_attempts = 5
    let mut attempt = 0
    while attempt < max_attempts {
        let code = train_once(out, [])   // first pass --resume is a no-op if no state
        if code == 0 { print("training complete"); return null }
        attempt = attempt + 1
        print("run crashed (code " + str(code) + "); relaunch " + str(attempt) + "/" + str(max_attempts))
    }
    print("giving up after " + str(max_attempts) + " attempts")
}
```

**Files:**
- Create: `scripts/mens/train_resilient.vox`
- Reference (no change): `training_loop/checkpoint.rs:14-91` (resume), `checkpoint_mid.rs` (retention)
- Docs: add a row to `docs/src/reference/mens-training.md` ("resilient training").

**Effort:** S (days). No Rust/trainer changes.

**Risks:** (1) If the crash is deterministic (always OOMs at the same step), naive relaunch loops forever — mitigate with the attempt cap + an escalation step (lower seq-len / shrink model) before exhausting attempts. (2) Optimizer-momentum reset on each resume slightly perturbs convergence; acceptable, but note it.

**Acceptance:** Kill a run mid-epoch; `train_resilient.vox` relaunches and completes from the last checkpoint; a deterministic OOM exhausts attempts and exits non-zero with a clear message (no infinite loop).

**Open questions:** Should escalation auto-shrink the model (4B→2B) on repeated OOM, or only warn? Should we also checkpoint optimizer state (separate, larger change) to make resume seamless?

### Track B — VRAM bundle (make 4B / longer context fit)

**Goal:** Lower the resident + activation walls so 4B (or longer sequence at 2B) trains within 16 GB.

**Approach (in dependency order):** (B1) wire the no-op gradient clip (`training.rs:642-643, 733-735`) — independent, ships first as a stability fix; (B2) BF16 activations through forward/backward (`model.rs`, `qlora.rs:442-446`); (B3) quantize or BF16 the embedding table + tied lm_head (`logic.rs:460`, `model.rs:502`); (B4) retune `RESIDENT_GIB_PER_B_PARAMS` / activation coefficient in `memory_budget.rs:24-46` against measured peaks after B2/B3 land, and lift the 4B→2B auto-retreat threshold if it now fits.

**Files:** `patches/qlora-rs-1.0.5/src/training.rs`, `crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/{forward,logic}.rs` + `model.rs`/`qlora.rs` in qlora-rs, `crates/vox-populi/src/mens/tensor/memory_budget.rs`.

**Effort:** B1 = S; B2 = L; B3 = M; B4 = S (but gated on measured re-calibration).

**Risks:** BF16 activations can hurt numerical stability for some ops (RMSNorm/softmax) — keep those accumulations in F32. Candle 0.9 dtype handling is manual; mixed-precision bugs are subtle → strong before/after loss-curve + VRAM-peak tests required. Embedding quantization can degrade quality on rare tokens.

**Acceptance:** A measured VRAM-peak harness shows 4B trains at a usable seq-len (≥256) under 16 GB without OOM across a full epoch, with eval quality within an agreed delta of the 2B baseline; gradient-clip test proves norms are actually clipped.

**Open questions:** BF16 vs FP16 for activations on Ada (4080)? Quantize embeddings to NF4 vs just BF16 (quality/VRAM trade)? Is the marginal payoff worth L-effort vs simply using 2B?

### Track C — Federated LoRA over LAN (throughput across boxes)

**Goal:** Run the existing single-device trainer on N LAN boxes over different data shards, periodically averaging LoRA deltas, for ~N× tokens/hour. Explicitly **not** a bigger-model feature.

**Approach:** Keep `candle_qlora_train` untouched (single-device). Add a coordinator that: (1) shards the dataset across nodes; (2) every K steps, each node exports `candle_qlora_adapter.safetensors`; (3) aggregate — averaging the **product** `B·A` (not B and A independently, since `avg(B·A) ≠ avg(B)·avg(A)`), or adopt FedEx-LoRA exact aggregation; (4) broadcast the merged adapter; (5) each node loads it (`load_adapter_into_trainer` already exists) and continues. Transport: reuse the existing mesh A2A signed envelopes (`vox-populi/src/transport`) — no collective-comm library needed. This is the plan's `FedAvg = ADAPT` path, not its op-log per-step all-reduce.

**Files:** New module (likely under `vox-populi/src/mens/` or a thin new crate), reusing `transport/` + `load_adapter_into_trainer`; CLI surface `vox mens train --federated --peers ... --sync-every K`; do **not** build on the `vox-distributed-training` stub (wrong layer — audit-envelope contracts).

**Effort:** M (days-to-low-weeks). Reuses the trainer; new code is shard / average / broadcast / load.

**Risks:** Inexact LoRA averaging hurts quality if K is large or factors averaged independently — must aggregate the product or use FedEx-LoRA. Stragglers/churn (one slow box) stall sync — need timeout/async averaging. No VRAM benefit (each node needs full base resident) — set expectations.

**Acceptance:** 2-3 LAN boxes complete an epoch in ~1/N wall-clock vs single-box with eval quality within an agreed delta of single-box training; sync traffic measured in MB/round (confirming the cheap-comms premise); a downed peer doesn't hang the run.

**Open questions:** Sync cadence K (per-epoch vs per-K-steps)? Synchronous vs asynchronous averaging? Reuse signed envelopes (audit trail) vs a plain TCP fast-path for the MVP?

---

## Part 3 — Recommended sequencing

1. **Track A first** — directly answers the "recover and continue" need, days of work, zero trainer risk, and immediately stops multi-hour runs dying unrecoverably. Ships value before any deep trainer surgery.
2. **Track B (B1 gradient-clip now; B2/B3 as a focused effort)** — the legitimate path to 4B-on-16GB. B1 is a free stability fix regardless of the rest.
3. **Track C** — once single-box training is reliable (A) and right-sized (B), add LAN throughput. Build on the trainer + mesh transport, not the distributed-training stub.

Each track should become its own bite-sized TDD plan under `docs/superpowers/plans/` when scheduled, executed in a dedicated worktree.
