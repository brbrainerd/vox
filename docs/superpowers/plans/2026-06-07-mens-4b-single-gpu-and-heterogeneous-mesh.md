# MENS 4B-on-16GB + Heterogeneous Mesh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Run in a dedicated worktree.

**Goal:** Make `Qwen/Qwen3.5-4B-Base` QLoRA-trainable end-to-end on a single 16 GB RTX 4080 SUPER (validated, not assumed), make crashed runs self-heal, and add a VRAM-heterogeneity-aware mesh that excludes nodes too small to help — without the single-machine 4B target depending on the mesh.

**Architecture:** Three independently shippable phases. Phase 0 (auto-recovery) is a `.vox` wrapper, zero trainer changes. Phase 1 (VRAM bundle) lowers the trainer's resident + activation footprint inside `vox-plugin-mens-candle-cuda` / `patches/qlora-rs-1.0.5` and recalibrates `vox-populi`'s budget so the auto-scaler stops retreating 4B→2B. Phase 2 validates the 4B run to victory with a fallback ladder. Phase 3 (mesh) adds per-node VRAM advertisement + a cohort planner that reuses the existing budget math as its exclusion gate and federated LoRA-delta sync over the existing A2A transport.

**Tech Stack:** Rust (Candle 0.9 CUDA, patched `qlora-rs`), VoxScript (`.vox`) for automation, axum control-plane + Ed25519-signed A2A envelopes for the mesh, `cargo test` / `cargo nextest`.

**Target & fallback ladder (decided 2026-06-07):**
- Headline 4B: `Qwen/Qwen3.5-4B-Base` (3.5 family, resident coeff 3.5/B; needs download). There is **no Qwen3.5-Coder** and no 4B coder model — coding in 3.5 is folded into the base model.
- Coder deliverable + fallback (all already in `~/.cache/huggingface/hub`): `Qwen/Qwen2.5-Coder-3B-Instruct` → `…-1.5B-Instruct` → `…-0.5B-Instruct` (coder family, resident coeff 5.0/B).
- Not feasible on 16 GB even after the bundle (state explicitly, do not attempt): `Qwen2.5-Coder-7B/14B/32B`, any Qwen3-Coder (MoE 30B+).

**Hard VRAM facts (from `crates/vox-populi/src/mens/tensor/memory_budget.rs`):** budget = `16.0 × 0.88 = 14.08 GiB`. Today resident(4B, 3.5/B) = `4×3.5 + 1.6 = 15.6 GiB` > 14.08 → `over_budget`, retreats 4B→2B (test `ladder_retreats_4b_to_2b_on_16gb` :432-445). Victory requires driving the *effective* resident+activation peak for 4B under 14.08 and proving it with a real run.

---

## Phase 0 — Auto-recovery wrapper (Track A)

**Why first:** zero trainer risk, immediately stops multi-hour runs dying unrecoverably. All resume machinery already exists (`training_loop/checkpoint.rs:14-91`, `--resume`, atomic temp-rename, last-good preserved). The only gap is an outer relaunch loop. Project policy: automation is `.vox`, not `.ps1`/`.sh`.

### Task 0.1: Resilient training launcher

**Files:**
- Create: `scripts/mens/train_resilient.vox`
- Reference (no change): `scripts/mens/run_4080_cycles.vox` (process.run idiom), `scripts/mens/train_dogfood.vox`

- [ ] **Step 1: Write the launcher**

```vox
// vox:caps fs subprocess env
// Resilient MENS training: relaunch from last checkpoint on crash, with a cap
// and an escalation step (shrink seq-len, then drop a model rung) before giving up.

fn run_train(model: str, out: str, seq: int) -> int {
    let mut args = ["mens", "train", "--resume", out]
    if model != "" { args.push("--model"); args.push(model) }
    if seq > 0 { args.push("--seq-len"); args.push(str(seq)) }
    print("[resilient] vox " + args.join(" "))
    match process.run("vox", args) {
        Some(res) => res.code,
        None => 1,
    }
}

fn main() {
    let out = "mens/runs/latest"
    // Escalation ladder: (model, seq). "" = let the auto-scaler choose.
    let plan = [
        ["", 0],                                // 1: as-configured (auto-scaler)
        ["", 256],                              // 2: shrink seq
        ["Qwen/Qwen2.5-Coder-3B-Instruct", 384],// 3: drop to coder-3B (local)
        ["Qwen/Qwen2.5-Coder-1.5B-Instruct", 384],
    ]
    let mut i = 0
    while i < plan.len() {
        let model = plan[i][0]
        let seq = plan[i][1]
        let code = run_train(model, out, seq)
        if code == 0 { print("[resilient] training complete"); return null }
        print("[resilient] attempt " + str(i + 1) + " failed (code " + str(code) + "); escalating")
        i = i + 1
    }
    print("[resilient] exhausted all attempts")
}
```

- [ ] **Step 2: Lint/compile the script**

Run: `vox check scripts/mens/train_resilient.vox`
Expected: no diagnostics (compiles).

- [ ] **Step 3: Dry-run smoke test (no GPU work)**

Run: `VOX_VRAM_OVERRIDE_GB=2 vox run scripts/mens/train_resilient.vox` against a tiny/empty dataset so the trainer exits non-zero fast.
Expected: log shows escalation through the ladder and a final "exhausted all attempts" — proves the loop + cap + escalation work without infinite looping.

- [ ] **Step 4: Document**

Add a "Resilient training" subsection to `docs/src/reference/mens-training.md` (note: file needs YAML frontmatter already present; just add a section) describing `train_resilient.vox`, the escalation ladder, and that optimizer momentum resets on resume (minor convergence blip).

- [ ] **Step 5: Commit**

```bash
git add scripts/mens/train_resilient.vox docs/src/reference/mens-training.md
git commit -m "feat(mens): resilient training launcher with crash auto-resume + escalation"
```

**Acceptance:** killing a real run mid-epoch and re-invoking `train_resilient.vox` resumes from the last checkpoint and completes; a deterministic failure exhausts the ladder and exits cleanly (no infinite loop).

---

## Phase 1 — VRAM bundle (Track B, full)

Order is dependency-driven: cheapest/lowest-risk first; each task is independently committable and testable. Tasks 1.2–1.4 touch the autodiff graph — **read the current function before editing** (exact tensor code is not reproduced here to avoid drift). Every task adds a measured VRAM assertion where possible.

### Task 1.1: Wire the no-op gradient clip (stability bug fix)

**Files:**
- Modify: `patches/qlora-rs-1.0.5/src/training.rs:641-643` and `:733-735` (currently `let _ = max_norm; // Placeholder`)
- Test: `patches/qlora-rs-1.0.5/src/training.rs` (`#[cfg(test)]` module) or a new `patches/qlora-rs-1.0.5/tests/grad_clip.rs`

- [ ] **Step 1: Write the failing test** — construct two trainable vars with known large grads, call the clip helper with `max_norm = 1.0`, assert the resulting global grad-norm ≤ 1.0 + eps.

```rust
#[test]
fn grad_clip_caps_global_norm() {
    // Build a tiny VarMap with grads of known norm > 1.0, apply clip(1.0),
    // assert sqrt(sum(sq(g))) <= 1.0 + 1e-5.
    // (Use candle_core::Tensor on Device::Cpu so the test needs no GPU.)
}
```

- [ ] **Step 2: Run it, verify it fails** — `cargo test -p qlora-rs grad_clip_caps_global_norm` → FAIL (clip is a no-op today).
- [ ] **Step 3: Read the two placeholder sites** and the surrounding optimizer step (`training.rs:630-660, 720-740`) to see how grads are accessed, then implement global-norm clipping: compute total norm across trainable grads, scale each grad by `min(1, max_norm / (total_norm + eps))` before the optimizer step.
- [ ] **Step 4: Run the test, verify it passes.**
- [ ] **Step 5: Commit** — `git commit -m "fix(qlora): implement gradient clipping (was a silent no-op)"`

### Task 1.2: BF16 activations through forward/backward

**Files:**
- Modify: `patches/qlora-rs-1.0.5/src/qlora.rs:442-446` (the cast back to F32 after base matmul) and the activation dtype in `crates/vox-plugin-mens-candle-cuda/src/model.rs` (attention/MLP/RMSNorm). Keep RMSNorm/softmax *accumulations* in F32, store activations in BF16.
- Test: new `crates/vox-plugin-mens-candle-cuda/tests/activation_dtype.rs`

- [ ] **Step 1: Write the failing test** — run a single forward+backward on a 2-layer tiny config on CUDA (gated `#[cfg(feature = "cuda")]`, skip if no GPU), assert the intermediate activation tensors report `DType::BF16` and the loss is finite.
- [ ] **Step 2: Run it, verify it fails** (activations are F32 today).
- [ ] **Step 3: Read `model.rs` forward + `qlora.rs:430-460`**, then thread a `compute_dtype = BF16` through the block forward; cast inputs to BF16 at block entry, do reductions (RMSNorm variance, softmax) in F32 then back to BF16. Remove the unconditional F32 cast at `qlora.rs:442-446`.
- [ ] **Step 4: Run the test, verify it passes** (BF16 activations, finite loss).
- [ ] **Step 5: Add a VRAM-peak regression test** — train 50 steps of 1.5B at seq 512 on CUDA, capture peak via `device::device_mem_used_total_mb()` (`device.rs:81-96`), assert peak dropped vs a recorded F32 baseline (store baseline as a const with a comment).
- [ ] **Step 6: Commit** — `git commit -m "perf(mens): BF16 activations in QLoRA forward/backward (halve activation VRAM)"`

### Task 1.3: BF16 (or NF4) the embedding table + tied lm_head

**Files:**
- Modify: embedding load `crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/training_loop/logic.rs:460` (`wte` loaded F32) and the model field `model.rs:502`; lm_head derivation `logic.rs:914-922`.
- Test: `crates/vox-plugin-mens-candle-cuda/tests/embedding_dtype.rs`

Decision (judgment): **BF16 first** (simple, ~½ saving, ~lossless). For Qwen3.5-4B (vocab ~248k × 3584) F32 embeddings ≈ 3.3 GiB → BF16 ≈ 1.65 GiB, a ~1.6 GiB resident win. Only escalate to NF4 if 4B still doesn't fit after Task 1.4.

- [ ] **Step 1: Write the failing test** — load a tiny model, assert `model.embed.dtype() == BF16` and that `index_select` for embeddings + the tied lm_head matmul produce finite logits.
- [ ] **Step 2: Run it, verify it fails** (F32 today).
- [ ] **Step 3: Read `logic.rs:455-470, 905-925`**, then load `wte` as BF16 (or cast post-load) and ensure `index_select` (`model.rs:515`) and lm_head reuse the BF16 tensor; keep the final logits→F32 cast before cross-entropy for numerical stability.
- [ ] **Step 4: Run the test, verify it passes.**
- [ ] **Step 5: Commit** — `git commit -m "perf(mens): BF16 embedding table + tied lm_head (~1.6 GiB resident win at 4B)"`

### Task 1.4: Gradient checkpointing (activation recompute)

**Files:**
- Modify: per-layer forward in `crates/vox-plugin-mens-candle-cuda/src/model.rs` (the `Qwen35Layer`/block forward) + the training forward `training_loop/forward.rs`; add a `gradient_checkpointing: bool` to `crates/vox-plugin-mens-candle-cuda/src/config.rs` (next to existing `LoraTrainingConfig` fields) and a CLI flag `--gradient-checkpointing` in `crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs`.
- Test: `crates/vox-plugin-mens-candle-cuda/tests/grad_checkpointing.rs`

Note: Candle 0.9 has **no built-in autograd checkpoint API**, so this is a manual rematerialization — split the layer stack into segments; in the backward, re-run each segment's forward under a fresh graph to regenerate activations instead of retaining them. This is the highest-effort task; isolate it behind the flag (default off) so nothing regresses if it's imperfect.

- [ ] **Step 1: Write the failing test** — with `gradient_checkpointing = true`, run forward+backward on a small multi-layer config on CUDA; assert (a) loss matches the non-checkpointed loss within tolerance (correctness) and (b) measured peak activation VRAM is lower than non-checkpointed (the point).
- [ ] **Step 2: Run it, verify it fails** (flag/feature absent).
- [ ] **Step 3: Implement** the segmented recompute behind the flag; wire `config.gradient_checkpointing` → forward; default the `qwen_4080_16g` preset and the 4B branch to `true`.
- [ ] **Step 4: Run the test, verify it passes** (matching loss, lower peak).
- [ ] **Step 5: Commit** — `git commit -m "feat(mens): optional gradient checkpointing (activation recompute) for 4B-on-16GB"`

### Task 1.5: Recalibrate the budget + lift the 4B retreat

**Files:**
- Modify: `crates/vox-populi/src/mens/tensor/memory_budget.rs:31,46,112` (resident coeffs, activation coeff) + the retreat tests `:432-453, :491-501`.
- Modify (doc-drift fix the audit found): `crates/vox-populi/src/mens/mod.rs:31-33` (claims 7B-coder→3B on 16 GiB; code does 1.5B).

- [ ] **Step 1: Measure** real resident + per-seq activation peaks for 4B (Qwen3.5) and 3B (coder) *after* Tasks 1.2–1.4, using `device_mem_used_total_mb()` at steady state and at peak. Record numbers in the commit message.
- [ ] **Step 2: Write the new retreat test** — `ladder_keeps_4b_on_16gb_after_bundle` asserting `plan_qwen35(16.0, 4.0)` returns `Qwen/Qwen3.5-4B` with `over_budget == false` and a sane seq_len (≥256), given the recalibrated coeffs.
- [ ] **Step 3: Run it, verify it fails** with current coeffs.
- [ ] **Step 4: Update** `RESIDENT_GIB_PER_B_PARAMS` (3.5 family) and `QWEN2_RESIDENT_GIB_PER_B` (coder) to the measured values; update `ACT_GIB_PER_KTOK_PER_SQRTB`; fix the stale `mod.rs` doc-comment to match.
- [ ] **Step 5: Run the full budget test module, verify all pass** — `cargo test -p vox-populi memory_budget` (existing 24 GiB / floor tests must still hold).
- [ ] **Step 6: Commit** — `git commit -m "perf(mens): recalibrate VRAM budget after bundle; 4B no longer retreats on 16 GiB"`

---

## Phase 2 — Validate to victory (single machine, no mesh)

No "done" claim until a real 4B run completes and resumes cleanly on the 4080. Follows superpowers:verification-before-completion (evidence before assertion).

### Task 2.1: Acquire the 4B base

- [ ] **Step 1:** Trigger the in-train downloader or pre-fetch: `vox mens train --model Qwen/Qwen3.5-4B-Base --dry-run` (or first real launch) pulls via `mens::hub::download_model_blocking` into `~/.cache/huggingface/hub`. Confirm the **text LM** weights load through the Qwen35 graph (the repo is multimodal; the trainer is text-only — if the loader rejects vision tensors, add a key-filter in `hf_keymap.rs`/`logic.rs` load path as a follow-up task and fall back to Task 2.3 meanwhile).
- [ ] **Step 2:** Verify free disk ≥ 30 GB before download (the audit recorded a prior disk-fill OOM at checkpoint time).

### Task 2.2: 4B training run + live monitoring

- [ ] **Step 1:** Launch via the Phase 0 wrapper: `vox run scripts/mens/train_resilient.vox` with `--model Qwen/Qwen3.5-4B-Base` and `--checkpoint-every 200`.
- [ ] **Step 2: Monitor VRAM** in a second shell: `nvidia-smi --query-gpu=memory.used --format=csv -l 5` and tail `mens/runs/latest/telemetry.jsonl` (`gpu used/total MiB` rows). Confirm steady-state and checkpoint-time peaks both stay < ~15.5 GiB (under the 16376 MiB card with margin).
- [ ] **Step 3: Victory criteria** — one full epoch completes; ≥2 mid-epoch checkpoints written and pruned to retention; no OOM; loss strictly finite and trending down; `eval_results.json` produced.
- [ ] **Step 4: Prove resume** — `Ctrl-C` mid-epoch, relaunch `train_resilient.vox`, confirm it resumes from `checkpoint_state.json` (logs "resuming from epoch=… global_step=…") and finishes.
- [ ] **Step 5: Run the eval gate** — `vox run scripts/mens/run_4080_cycles.vox` (checkpoint-integrity + eval-local pass@k gates) green.

### Task 2.3: Fallback ladder (only if 2.2 fails after the full bundle)

- [ ] **Step 1:** If 4B still OOMs at seq ≥256, fall back to `Qwen/Qwen2.5-Coder-3B-Instruct` (local), then `-1.5B`, then `-0.5B` — the wrapper already escalates. Record at which rung a stable epoch first completes.
- [ ] **Step 2: Advise** in the run report which model is the realistic ceiling on this 4080 and why (cite measured peaks), per the user's request for fallback guidance.

---

## Phase 3 — Heterogeneity-aware mesh (Track C)

Independent of Phases 0–2. Realistic mode = **federated LoRA-delta averaging** over the existing A2A transport (NOT the dead `all_reduce` stub). The cohort planner's exclusion gate **reuses `memory_budget::plan_*`**: a node is useless iff `plan(node_vram_gib, target_params_b).over_budget` — so a 2 GB node is auto-excluded for any real 4B target.

### Task 3.1: Advertise real per-node VRAM on the wire

**Files:**
- Modify: `crates/vox-populi-types/src/node_record.rs` (add `gpu_vram_total_mb: Option<u64>`, `gpu_model_name: Option<String>` near `:56-69`); merge allowlist `crates/vox-populi/src/transport/handlers/nodes.rs:166-212`; record construction in `crates/vox-populi/src/http_lifecycle.rs:91,133,156`.
- Test: `crates/vox-populi/tests/node_vram_advertise.rs`

- [ ] **Step 1: Write the failing test** — build a `NodeRecord`, populate VRAM from `HardwareRegistry::probe()`/`get_system_vram_gb()`, round-trip through join+heartbeat merge, assert `gpu_vram_total_mb` survives the merge allowlist.
- [ ] **Step 2: Run it, verify it fails** (fields don't exist).
- [ ] **Step 3: Add** the two fields (serde-optional for back-compat), populate them at record construction from `HardwareSummary.vram_mb`/`model_name` (`crates/vox-populi/src/mens/hardware/types.rs:34-49`), and add them to `merge_optional_node_fields`.
- [ ] **Step 4: Run the test, verify it passes.**
- [ ] **Step 5: Commit** — `git commit -m "feat(mesh): advertise probed GPU VRAM on NodeRecord (join+heartbeat)"`

### Task 3.2: Cohort planner — exclude weak nodes, pick target model, estimate gain

**Files:**
- Create: `crates/vox-populi/src/mens/cohort/mod.rs` + `planner.rs` (model it on `crates/vox-populi/src/mens/cloud/resolver.rs:159-274`).
- Test: `crates/vox-populi/src/mens/cohort/planner.rs` `#[cfg(test)]`

- [ ] **Step 1: Write the failing test** — given nodes `[16 GiB, 16 GiB, 2 GiB]` and target `Qwen3.5-4B`, assert the planner returns a cohort of exactly the two 16 GiB nodes (the 2 GiB excluded via `plan(2.0, 4.0).over_budget`), and a `gain` estimate > single-node throughput.

```rust
#[test]
fn planner_excludes_subthreshold_nodes_and_estimates_gain() {
    let nodes = vec![node(16.0), node(16.0), node(2.0)];
    let plan = plan_cohort(&nodes, "Qwen/Qwen3.5-4B", /*params_b*/4.0);
    assert_eq!(plan.included.len(), 2);
    assert!(plan.excluded.iter().any(|n| n.vram_gib < 4.0));
    assert!(plan.estimated_speedup > 1.0);
}
```

- [ ] **Step 2: Run it, verify it fails** (module absent).
- [ ] **Step 3: Implement `plan_cohort`** — for each node call `memory_budget::plan(node_vram_gib, target_params_b)`; exclude `over_budget` nodes and any with `donation_policy.accepts_training_workloads == false || quarantined || maintenance`; estimate per-node throughput via `mens::cloud::estimator::TimeEstimator::tflops_for(gpu_name)` (`estimator.rs:180`); `estimated_speedup = Σ included throughput / max single throughput`. Return `{ target_model, included, excluded, estimated_speedup }`.
- [ ] **Step 4: Add the no-gain guard test** — nodes `[16 GiB, 2 GiB]`: assert planner reports `recommend_single_machine == true` when including peers yields `< 1.1×` (one usable node ⇒ no parallel gain) or when adding a node would force a model downgrade for all.
- [ ] **Step 5: Run both tests, verify they pass.**
- [ ] **Step 6: Commit** — `git commit -m "feat(mesh): heterogeneity-aware cohort planner (VRAM exclusion + gain metric)"`

### Task 3.3: Federated LoRA-delta sync over A2A

**Files:**
- Create: `crates/vox-populi/src/mens/cohort/federated.rs`; new A2A message types + routes in `crates/vox-populi/src/transport/router.rs:69-101` and handler re-export `transport/handlers/mod.rs:18-27`.
- Reuse: `load_adapter_into_trainer` (exists), signed-envelope pattern from `vox-distributed-training/src/{gradient,checkpoint}.rs` (copy the sign/verify pattern, ignore the `all_reduce` stub), `CheckpointBundle::to_operation_kind()` for op-log markers.
- Test: `crates/vox-populi/src/mens/cohort/federated.rs` `#[cfg(test)]`

Decisions (judgment): sync cadence default **per-epoch** (configurable `--sync-every`); **synchronous with timeout** for the MVP (a downed peer is dropped after timeout, not hung); **reuse signed envelopes** for the audit trail. Aggregate the **product `B·A`**, not `B` and `A` independently (since `avg(B·A) ≠ avg(B)·avg(A)`).

- [ ] **Step 1: Write the failing test** — two in-process "nodes" each produce a LoRA adapter; call `average_adapters(&[a, b])`; assert the merged adapter equals the elementwise mean of the reconstructed `B·A` products (within tolerance), and that the round-trip through the signed A2A envelope verifies.
- [ ] **Step 2: Run it, verify it fails.**
- [ ] **Step 3: Implement** `average_adapters` (product-space mean) + the A2A `training_cohort_delta` message (deliver via `deliver_a2a`, pull via `a2a_inbox`, Ed25519-signed), and a `--federated --peers … --sync-every K` CLI surface that: shards data, runs the existing single-device trainer untouched, exchanges + averages adapters every K, broadcasts, continues.
- [ ] **Step 4: Run the test, verify it passes.**
- [ ] **Step 5: Integration smoke** — two local processes (or two `VOX_VRAM_OVERRIDE_GB=16` instances) federate one epoch on a tiny dataset; assert both converge to the same averaged adapter and sync payloads are MB-scale (log bytes/round to prove the cheap-comms premise).
- [ ] **Step 6: Commit** — `git commit -m "feat(mesh): federated LoRA-delta sync over signed A2A (throughput, not bigger-model)"`

### Task 3.4: Document the mesh boundary

- [ ] **Step 1:** Update `docs/src/architecture/mens-training-pipeline-audit-and-improvement-plan-2026-06-07.md` Track C section: federated LoRA buys throughput/more-data, NOT bigger-than-16GB models (each node holds the full base resident); sharding (FSDP/TP/PP) remains LAN-impractical. Commit.

---

## Self-Review (run against the spec)

- **Spec coverage:** 4B-on-16GB → Phases 1–2; auto-recovery/"recover and continue" → Phase 0; heterogeneous mesh + exclude-useless-nodes + metrics → Phase 3.1-3.2; "no gain ⇒ don't" → Task 3.2 no-gain guard; LAN multi-GPU → Task 3.3; fallback advice if 4B fails → Task 2.3; extend the auto-scaler → Tasks 1.5 + 3.2 (reuses `plan_*`). Qwen3.7 → confirmed out (closed/undownloadable), not in plan.
- **Placeholder scan:** deep autodiff tasks (1.2-1.4) use explicit "read current function, then change so test passes" steps rather than fabricated tensor code — intentional, to avoid drift against unread `model.rs`/`qlora.rs` internals. All other tasks carry concrete code/commands.
- **Type consistency:** `plan_cohort`/`plan_*`/`average_adapters`/`gpu_vram_total_mb` used consistently across Phase 3; `gradient_checkpointing` flag named identically in config + CLI + preset.

## Execution Handoff

Phases are independently shippable; recommended order **0 → 1 → 2 → 3** (the user's destination is Phases 0-2; the mesh is additive). Phase 1 should run in a dedicated worktree because it edits the patched `qlora-rs` and the autodiff graph.

Two execution options:
1. **Subagent-Driven (recommended)** — fresh subagent per task, two-stage review between tasks.
2. **Inline Execution** — batch with checkpoints via superpowers:executing-plans.

---

## Post-verification revision (2026-06-07)

Four verification passes against the live code changed the plan. Summary:

- **TARGET CHANGE (premise-breaker):** `Qwen/Qwen3.5-4B-Base` is multimodal (`Qwen3_5ForConditionalGeneration` + `vision_config`) and is **hard-rejected at config-parse** by the text trainer (`crates/vox-hf-layout/src/lib.rs:101-124`). No text-only 4B Qwen exists that this trainer can load. **Practical destination re-targeted to `Qwen/Qwen2.5-Coder-3B-Instruct`** (text-only, already in `~/.cache/huggingface/hub`, closest to the 4B intent). Multimodal-trainer support ≈350 LOC + a vision tower the trainer lacks — out of scope.
  - **Phase 2 amended:** Task 2.1 no longer downloads Qwen3.5-4B. Validate on **Qwen2.5-Coder-3B** first; fallback ladder 3B → 1.5B → 0.5B (all local). Task 1.5 recalibrates for the **3B** fit, not 4B.
- **PRUNE — Task 1.4 (gradient checkpointing):** confirmed L-effort (Candle 0.9 has no checkpoint primitive; needs a trainer fork) and **likely unnecessary** for the 3B target once 1.2 (BF16 activations) + 1.3 (BF16 embeddings) + 1.5 (recalibrate) land. Make it **conditional**: only implement if measured 3B peak still exceeds ~14 GiB. Default-off.
- **CONFIRMED cheap (proceed):** Task 1.1 (grad-clip fix), 1.2 (BF16 activations — localized; Candle 0.9 ships `softmax_bf16`), 1.3 (BF16 embeddings — localized). All small.
- **NEW prerequisite before Task 1.5:** resolve the dual-sizing conflict — `crates/vox-ml-cli/src/commands/schola/train/gpu.rs:87-132` re-derives seq/batch/grad_accum from a preset and can override the per-model VRAM budget in `train_arm.rs:256-376` (budget only fills `None`). Recalibration isn't trustworthy until precedence is fixed (preset-vs-VRAM-budget).
- **ADAPT — resilient wrapper (Task 0.1, already shipped):** add `--checkpoint-every <N>` to the relaunched command so mid-epoch resume points exist (CLI default is `None` = epoch-boundary only). Minor follow-up.
- **Phase 3 (mesh): all assumptions confirmed**, no changes — most build-ready phase. One note: make `load_adapter_into_trainer` `pub`; reuse only `CheckpointBundle::{sign,verify,to_operation_kind}` + `SessionId` from the distributed-training stub.
- **Known limitation:** `.vox` automation scripts only run under `vox run --mode interp` (native codegen bugs) — tracked as a separate cleanup task.
