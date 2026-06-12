---
title: "MENS Distributed Training & Execution Plan (2026-05-09)"
description: "Distributed-AI track that supplements the seven-phase Mesh & Language SSOT. Audits MENS current state (training stubbed), surveys distributed training prior art, defines the inference-anywhere / training-only-on-CUDA split, content-addresses SafeTensors model bundles, and lays out 15 Mn-T tasks integrating with SSOT P0–P6 (especially P0-T7 SkillRuntime, P2-T1 CAS, P4-T12 model registry, P5-T8 mesh inventory, P6-T4 redundant execution). Covers MENS corpus gaps for the Vox spine primitives so emissions stay on-distribution."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Defines the MENS / distributed-AI track for the mesh; agents and contributors should orient from this before changes that cross MENS, vox-populi mens, vox-distributed-training, model dispatch, or HF safetensors paths."
sourced_at: "2026-05-09"
---

# MENS Distributed Training & Execution Plan (2026-05-09)

## What this is / what it isn't

This document is the **MENS / distributed-AI track** that runs alongside
the [Mesh & Language Distribution SSOT](mesh-and-language-distribution-ssot-2026.md).
The SSOT covers the *transport, durability, identity, op-log, dashboard,
and language-spine* layers of the Vox mesh. This document covers the
*model-loading, model-execution, model-training, and MENS-corpus* layers
that ride on top of that mesh.

It is a **plan**, not an implementation. It defines task IDs (`Mn-T1`
through `Mn-T15`), file boundaries, struct/trait sketches, failing-test
ideas, and acceptance gates. It does not write the code.

It is **not** a replacement for the SSOT. Where they overlap (CAS,
op-log, donation policy, dashboard model registry, redundant execution),
this document points at the SSOT task IDs and stays in their lane.

It is **not** a corpus-collection harness, an eval harness, or a
training-script repository. It schedules the work that creates those
artifacts and points at the schema docs that govern them.

- Hopper integration: MENS is the natural future host for the optional
  `vox-priority-policy` learning crate (`Hp-T9` in SSOT §3.5); reserved
  as a Wave-2 follow-up after override telemetry is real.

## §0 Charter

### 0.1 Scope

The MENS track is responsible for everything that turns a Vox mesh
into an *AI-aware* compute fabric:

- Loading model weights from local disk and the network.
- Executing inference on the heterogeneous hardware Vox actually runs on
  (CUDA desktops, Apple Silicon laptops, CPU-only servers, llama.cpp RPC
  fan-outs, Ollama subprocesses).
- Executing **training** on the narrow slice of Vox hosts that can
  actually train (CUDA desktops with sufficient VRAM, today). Treating
  training as the *exception* and inference as the *default*.
- Content-addressing model bundles (weights + tokenizer + config) so
  that bundles ship across the mesh by hash, not by URL.
- Wiring `@inference`, `@training_step`, and `@distributed_train`
  annotations into the Vox compiler with effect-row enforcement and
  capability-token gating.
- Building the MENS corpus so MENS-trained models *emit Vox programs
  that compile and pass effect-checks* — not Rust, not generic Python,
  not "JavaScript with Vox keywords".

### 0.2 Pinned constraints

These constraints are non-negotiable in this track. A task whose design
violates any of them is rejected at review.

1. **Training: Candle on CUDA only.** The training-side runtime is the
   `candle` crate family on NVIDIA CUDA. Apple Silicon training, CPU
   training, ROCm training, and PyTorch-via-Python training are all
   explicitly out of scope for v0.6 / v0.7 / v1.0. Their absence is the
   feature: it lets us bound the test matrix to one backend and ship.
2. **Inference: GPU/CPU agnostic via probes.** Inference must run on
   anything the [hardware probe](populi-mesh-probe-correctness-spec-2026.md)
   says is capable (CUDA, Metal, CPU, llama.cpp RPC peer, Ollama
   subprocess). Inference does not assume CUDA. The dispatcher routes
   based on the probe report plus the SSOT P5-T8 mesh inventory.
3. **SafeTensors only on disk.** The only on-disk weight format Vox
   reads or writes is HuggingFace
   [SafeTensors](https://github.com/huggingface/safetensors). Pickle
   files, GGUF, ONNX, and TorchScript are *importable* through external
   tooling but are converted to SafeTensors on entry. The CAS and
   bundle-by-hash machinery assume a single canonical format.
4. **Crypto via vox-crypto only.** All signatures, hashes, and KDFs go
   through [`vox-crypto`](../../../crates/vox-crypto/). Ed25519 for
   signatures, SHA3-512 / BLAKE3 for content hashing. No new
   dependencies on `ring`, `openssl`, `sodiumoxide`, or `dalek` outside
   the `vox-crypto` re-export surface.
5. **No new ML frameworks.** Adding a second tensor framework (a
   second-tier alternative to Candle) is explicitly out of scope. We
   either have one supported framework or zero. We have one: Candle.
6. **No `.ps1` / `.sh` / `.py` scripts.** All automation is `.vox`
   per [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md).
   Corpus harvesters, training launchers, eval harnesses, and CI shims
   are `.vox` files invoked via `vox run scripts/foo.vox`.
7. **No Bittensor-style token economy.** Per
   [SSOT §0](mesh-and-language-distribution-ssot-2026.md), incentive
   layers built on cryptocurrency / staking / market-clearing are
   anti-goals. The mesh uses *kudos* (a non-fungible accounting
   primitive in [`vox-mesh-types::kudos`](../../../crates/vox-mesh-types/src/kudos.rs))
   and *reputation* (peer-local EMA), neither of which trade for value.

### 0.3 What this charter buys us

A bounded blast radius. By forbidding training on non-CUDA and
forbidding new frameworks, we get: one tested code path for gradient
sync, one weight format on disk, one set of dependency upgrades to
chase, and a corpus that doesn't have to teach MENS three different
ways to spell "load a model".

A model-as-data invariant. Because all weights are SafeTensors and all
bundles are content-addressed, the mesh treats a 70B-parameter model
exactly the same way it treats a 4-byte function bundle: by hash. This
means the same op-log, the same gossip, the same lease semantics, the
same dashboard view.

**Phase 1 dependency.** Mn-T4 (`@inference`) and Mn-T5 (`@training_step`, `@distributed_train`)
require Phase 1 P1-T6 to extend the effect-row enum with `GpuCompute` (and `Mutate` for
training-step). The SSOT §1.2 has been updated to list these variants. MENS cannot land Mn-T4
or Mn-T5 ahead of P1-T6.

---

## §1 Audited current state of MENS

The MENS code lives in two places:

- [`crates/vox-populi/src/mens/`](../../../crates/vox-populi/src/mens/) —
  the in-tree mens primitives (hardware probe, cloud routing, tensor
  helpers, training stubs).
- [`crates/vox-plugin-mens-candle-cuda/`](../../../crates/vox-plugin-mens-candle-cuda/) —
  the plugin that actually wraps Candle on CUDA for QLoRA fine-tunes.

What follows is what these crates *actually* do today, file by file.

### 1.1 The distributed training stub

**File:** [`crates/vox-populi/src/mens/tensor/populi_train.rs`](../../../crates/vox-populi/src/mens/tensor/mod.rs)
(~30 LoC).

The entire distributed-mens-training surface in-tree is:

```rust
pub struct MeshTrainConfig {
    pub world_size: usize,
    pub rank: usize,
    pub gradient_reduce: bool,
}

pub fn is_mesh_mode() -> bool { /* env-var: VoxMeshTrain */ }
pub fn get_mesh_rank() -> usize { /* env-var: VoxMeshRank */ }
```

That is **the entirety** of the in-tree distributed-training surface.
No collective-communication library, no all-reduce, no shard
exchange, no rank discovery, no NCCL binding, no PCCL binding. The
struct is shaped *as if* it would gate a future implementation, but
the implementation is two env-var lookups.

**Verdict.** This is a *stub* in the
[`vox-code-audit`](../../../crates/vox-code-audit/) sense. It satisfies
the type-check, gives downstream code something to import, and does
nothing. Mn-T1 deletes it and replaces it with a real crate.

### 1.2 What MENS does have working

| Concept | File | Status |
|---|---|---|
| Hardware probe (per-device tier) | [`mens/hardware/probe.rs`](../../../crates/vox-populi/src/mens/hardware/probe.rs) | **Working** (Linux DRM, Apple Metal, mock); see [populi-mesh-probe-correctness-spec-2026.md](populi-mesh-probe-correctness-spec-2026.md) |
| Hardware probe registry | [`mens/hardware/registry.rs`](../../../crates/vox-populi/src/mens/hardware/registry.rs) | Working |
| Cloud-burst budget + provider catalog | [`mens/cloud/`](../../../crates/vox-populi/src/mens/cloud/) | Working (RunPod, Vast, local) |
| QLoRA fine-tune (single-GPU CUDA) | [`vox-plugin-mens-candle-cuda/src/candle_qlora_train/`](../../../crates/vox-plugin-mens-candle-cuda/src/candle_qlora_train/) | **Working** for a single device; no rank coordination |
| Training preflight (memory / VRAM check) | [`mens/tensor/preflight_train.rs`](../../../crates/vox-populi/src/mens/tensor/preflight_train.rs) | Working |
| HF SafeTensors loader | [`mens/tensor/hf_load.rs`](../../../crates/vox-populi/src/mens/tensor/hf_load.rs) | Working (single file at a time) |
| HF key-map (param-name remapping) | [`mens/tensor/hf_keymap.rs`](../../../crates/vox-populi/src/mens/tensor/hf_keymap.rs) | Working |
| Manifest + checkpoint state | [`mens/tensor/manifest/`](../../../crates/vox-populi/src/mens/tensor/manifest/), [`mens/tensor/checkpoint_state.rs`](../../../crates/vox-populi/src/mens/tensor/checkpoint_state.rs) | Working (single-host scope) |
| Domain router (which preset for which task) | [`mens/tensor/domain_router.rs`](../../../crates/vox-populi/src/mens/tensor/domain_router.rs) | Working |
| Execution planner (inference path selection) | [`mens/tensor/execution_planner.rs`](../../../crates/vox-populi/src/mens/tensor/execution_planner.rs) | Partial — selects backend but does not consume mesh-wide inventory |
| Operator messages (event types) | [`mens/tensor/operator_messages.rs`](../../../crates/vox-populi/src/mens/tensor/operator_messages.rs) | Working |
| Telemetry schema | [`mens/tensor/telemetry_schema.rs`](../../../crates/vox-populi/src/mens/tensor/telemetry_schema.rs) | Working — namespace `vox.mens.*` |
| Finetune contract + registry | [`mens/tensor/finetune_contract.rs`](../../../crates/vox-populi/src/mens/tensor/finetune_contract.rs), [`mens/tensor/finetune_registry.rs`](../../../crates/vox-populi/src/mens/tensor/finetune_registry.rs) | Working (local registry only) |
| External-serving handoff (Ollama / vLLM) | [`mens/tensor/external_serving_handoff.rs`](../../../crates/vox-populi/src/mens/tensor/external_serving_handoff.rs) | Working |
| Healing (resume after partial failure) | [`mens/healing.rs`](../../../crates/vox-populi/src/mens/healing.rs) | Working (single-host) |
| Hub (per-domain orchestration) | [`mens/hub.rs`](../../../crates/vox-populi/src/mens/hub.rs) | Working |

### 1.3 What MENS is missing

| Concept | Today | Gap |
|---|---|---|
| Distributed training | env-var stub at [`populi_train.rs`](../../../crates/vox-populi/src/mens/tensor/mod.rs) | No gradient sync, no rank coordination, no signed checkpoint exchange — Mn-T1 |
| Inference backend trait | Ad-hoc dispatch in `execution_planner.rs` | No common trait; each call site re-shapes the dispatch — Mn-T2 |
| Content-addressed model bundles | None | Models are loaded by file path; no bundle-by-hash, no mesh-CAS — Mn-T3 |
| `@inference` / `@training_step` / `@distributed_train` annotations | None | Compiler does not know about model effects; routing is runtime-only — Mn-T4, Mn-T5 |
| Op-log integration for training checkpoints | None | Checkpoints are local files; no signed CAS bundle as op-log entry — Mn-T6 |
| Donation-policy training fields | None | `WorkerDonationPolicy` does not encode `cuda_tier` / `vram_min_gb` / `accepts_training_workloads` — Mn-T7 |
| Model-bundle CLI | None | No `vox model ls` / `push` / `pull` — Mn-T8 |
| Dashboard inference-router | Empty | No model-availability overlay on the mesh canvas — Mn-T9 |
| Distributed training observability | Single-host telemetry only | Per-shard spans, signed checkpoint events, `vox.train.*` namespace — Mn-T10 |
| MENS corpus harvester | Manual | No `.vox` script that walks docs + sources + diagnostic emitter into HF-Datasets jsonl — Mn-T11 |
| Eval harness | Manual notebooks | No automated emit-then-compile loop — Mn-T12 |
| Apple Silicon inference | Probe only | Probe says Metal works; backend trait does not yet wrap Candle-Metal — Mn-T13 |
| Petals-style swarm inference | None | Stretch goal for huge models — Mn-T14 |
| `where-things-live.md` rows | Out of date | Pending crates (vox-distributed-training, vox-inference, vox-mens-corpus) not registered — Mn-T15 |

### 1.4 What's in plugin-mens-candle-cuda

The plugin crate
[`vox-plugin-mens-candle-cuda`](../../../crates/vox-plugin-mens-candle-cuda/)
is well-formed but single-host. Its surface:

- `model.rs` — model construction (Llama-family, Mistral-family).
- `inference.rs` — single-device inference loop.
- `qlora_weights.rs`, `qlora_preflight.rs`, `merge.rs` — QLoRA load/save/merge.
- `candle_qlora_train/` — training loop, device select, epoch boundary,
  checkpoint mid-epoch, finalize.
- `checkpoint.rs`, `checkpoint_state.rs` — checkpoint marshalling.
- `operator_messages.rs` — event types.

This crate stays. Mn-T2 *consumes* it (via the inference-backend trait)
rather than rewriting it. Mn-T1 *consumes* it (via the
TrainingSession trait) the same way.

---

## §2 Distributed training prior art

A distributed-training plan that does not survey prior art is a Bittensor
white-paper. Here is a focused survey, one verdict per system.

| System | One-line synthesis | Verdict |
|---|---|---|
| **PyTorch DDP** ([source](https://pytorch.org/tutorials/intermediate/ddp_tutorial.html)) | Synchronous data-parallel; each rank holds a full model copy and runs all-reduce on gradients per step. | **ADAPT** — the gradient-sync envelope shape is what we copy; PyTorch itself is out of scope per charter. |
| **PyTorch FSDP** ([source](https://pytorch.org/docs/stable/fsdp.html)) | Sharded data parallel: parameters, gradients, optimizer state are sharded across ranks; gather on demand. | **ADAPT** — Phase 2+ once we want models that exceed single-GPU VRAM. The shard-on-demand pattern is the right one. |
| **DeepSpeed ZeRO 1/2/3** ([source](https://www.deepspeed.ai/tutorials/zero/)) | Three stages of optimizer-state / gradient / parameter partitioning; ZeRO-3 ≈ FSDP. | **ADAPT** — same as FSDP. We borrow the staging concept (start with ZeRO-1-equivalent, escalate when needed). |
| **Megatron-LM tensor + pipeline parallel** ([source](https://github.com/NVIDIA/Megatron-LM)) | Splits *within* a layer (tensor) and *across* layers (pipeline) for very large models. | **REJECT (for v1.0)** — we don't have the model sizes that need this. Revisit in Mn-T14 / P6. |
| **Petals** ([paper](https://arxiv.org/abs/2209.01188)) | Decentralized inference of BLOOM-176B by partitioning transformer layers across volunteer GPUs over public internet. | **KEEP (as Mn-T14 / Phase 6)** — exact pattern we want for huge-model inference on the volunteer mesh. |
| **Hivemind / Learning@Home** ([paper](https://arxiv.org/abs/2002.04013), [project](https://github.com/learning-at-home/hivemind)) | Asynchronous all-reduce over volunteer compute with DHT-based peer discovery; tolerates churn. | **ADAPT** — the *churn-tolerance* posture (don't block on a slow rank) is the right default for a volunteer mesh. |
| **Bittensor** ([overview](https://bittensor.com/)) | Decentralized AI with token-economic incentive for miners and validators. | **REJECT** — incentive layer is anti-goal per SSOT §0. We use kudos. |
| **SWARM Parallelism** ([paper](https://arxiv.org/abs/2301.11913)) | Pipeline-parallel training over heterogeneous, slow, unreliable workers. | **KEEP for research** — directly addresses the "training on volunteer mesh" problem; not for v1.0. |
| **FedAvg / Federated Learning** ([paper](https://arxiv.org/abs/1602.05629)) | Train locally on private data; server averages weights periodically. Privacy-preserving. | **ADAPT (privacy mode)** — the right shape for `accepts_sensitive_training_data: false` peers contributing data updates without exposing inputs. |
| **vLLM tensor parallel** ([source](https://docs.vllm.ai/)) | Production tensor-parallel inference; CUDA-only; mature batching. | **REJECT (as a dependency)** — but we *handoff* to vLLM via the existing `external_serving_handoff.rs` for users who already run it. |
| **llama.cpp RPC** ([source](https://github.com/ggml-org/llama.cpp/tree/master/tools/rpc)) | Splits a GGUF model across machines using a custom RPC protocol; CPU + GPU. | **KEEP (as Mn-T2 backend)** — exactly the pattern for "use my friend's MacBook + my desktop together for inference". |
| **mlx-distributed** ([source](https://ml-explore.github.io/mlx/build/html/usage/distributed.html)) | Apple's MLX has a small distributed primitive; macOS-only. | **REJECT (for v0.6/v0.7)** — Apple Silicon training is out of scope; revisit if we ever lift the charter. |
| **Candle distributed (current)** ([source](https://github.com/huggingface/candle)) | Single-device today; multi-device is sketched but not first-class. | **ADAPT** — we build on Candle's `Device` and `VarMap` and provide our own all-reduce envelope on top. |
| **BOINC adaptive replication** ([project overview](https://boinc.berkeley.edu/) — legacy Trac wiki page on adaptive replication is retired) | Volunteer compute network; redundant execution of jobs with majority voting; "trusted host" downgrade after consistent agreement. | **KEEP** — directly informs Mn-T14 / SSOT P6-T4. |
| **SafeTensors sharding** ([source](https://huggingface.co/docs/safetensors/index)) | Memory-mapped, zero-copy, no-arbitrary-code-execution tensor format; supports sharded models via index json. | **KEEP** — this is the only on-disk weight format we touch. |

The synthesis: we copy the *shape* of DDP gradient sync, the
*churn-tolerance* of Hivemind, the *bundle-by-hash* of SafeTensors,
the *redundant execution* of BOINC, and the *layer-split* of Petals
(stretch). We do not adopt any of these as a dependency.

---

## §3 Gap analysis — execution side

### 3.1 What inference needs

Inference is the common case. Most Vox users will *use* models far more
often than they will *train* them. The execution side must:

1. Enumerate every backend the host can run (CUDA, Metal, CPU, llama.cpp
   RPC peer, Ollama subprocess, external-serving handoff to vLLM).
2. Map `(model_id, backend) → peer` via the SSOT P5-T8 mesh inventory.
3. Cold-start fetch a model bundle from the SSOT P2-T1 CAS by hash if
   the local host doesn't have it.
4. Wrap each backend in a uniform trait so the dispatcher's code path is
   *one* code path, not five.
5. Surface backend-tier status (which backends are available right now,
   are they busy, is the model loaded?) to the dashboard via Mn-T9.

### 3.2 The InferenceBackend trait shape

```rust
pub trait InferenceBackend: Send + Sync {
    fn id(&self) -> BackendId;
    fn capabilities(&self) -> BackendCapabilities;
    fn can_serve(&self, bundle: &ModelBundle) -> Verdict;
    fn load(&self, bundle: &ModelBundle) -> Result<LoadedModel>;
    async fn predict(
        &self,
        loaded: &LoadedModel,
        input: PromptInput,
        params: SamplingParams,
    ) -> Result<PredictStream>;
    fn unload(&self, loaded: LoadedModel) -> Result<()>;
}
```

`BackendCapabilities` carries a `cuda_tier`, `metal_tier`, `vram_gb`,
`max_context_len`, `quantization_set`, `streaming: bool` so the
dispatcher can plan against the probe + inventory without instantiating
the backend.

### 3.3 Concrete impls (all under `vox-inference`, see Mn-T2)

- `CandleCuda` — wraps the existing
  [`vox-plugin-mens-candle-cuda::inference`](../../../crates/vox-plugin-mens-candle-cuda/src/inference.rs).
- `CandleMetal` — Mn-T13. Wraps `candle-core` with `Device::Metal`.
- `CandleCpu` — wraps `candle-core` with `Device::Cpu`. Slow but always
  available; used as the smoke-test backend in CI.
- `LlamaCppRpc` — speaks the [llama.cpp RPC protocol](https://github.com/ggml-org/llama.cpp/tree/master/tools/rpc)
  to a remote host running `rpc-server`. The remote host doesn't need
  Vox installed — this is one of two interop paths for "I have an old
  GGUF model on a NAS".
- `OllamaSubprocess` — shells out to a local `ollama` binary via its
  HTTP API; conversion goes through the existing `external_serving_handoff.rs`.
  This is the second interop path; it's the on-ramp for the millions of
  users with Ollama already installed.

### 3.4 Cold-start CAS fetch

The dispatcher's pre-flight:

1. Resolve `model_id` to a `ModelBundle.weights_hash` via the SSOT P4-T12
   model registry.
2. Check `vox-package` artifact cache for the bundle by hash (SSOT P2-T1).
3. If miss, find a peer that has it (SSOT P5-T8 inventory) and fetch
   over the existing A2A envelope path.
4. Verify SHA3-512 + Ed25519 signature on receipt (vox-crypto).
5. Hand the bundle to the chosen InferenceBackend's `load` method.

### 3.5 SSOT integration points (execution side)

| Mn-T | SSOT task | Use |
|---|---|---|
| Mn-T2 | P0-T7 (SkillRuntime trait) | InferenceBackend is a sibling trait; both ride the same plugin-loading path |
| Mn-T3 | P2-T1 (CAS bundles) | ModelBundle extends `vox-package` artifact cache types |
| Mn-T4 | P0-T7, P5-T8 | `@inference` annotation routes through SkillRuntime against mesh inventory |
| Mn-T9 | P4-T4, P4-T12 | Dashboard mesh canvas + model-registry view get a backend-tier overlay |

---

## §4 Gap analysis — training side

### 4.1 The CUDA-only constraint, restated

Per charter §0.2: training is Candle on CUDA. Period. This means:

- No Apple Silicon training. The
  [`mens/hardware/macos_metal.rs`](../../../crates/vox-populi/src/mens/hardware/macos_metal.rs)
  probe will *report* Metal but the `@training_step` annotation's
  compile-time check refuses to admit a Metal-only host into a training
  cohort.
- No CPU training. CPU training "works" in Candle for tiny models;
  forbidding it lets us drop the test path entirely.
- No ROCm yet. AMD-GPU users go through the cloud-burst path
  (`mens/cloud/`) on a CUDA provider until upstream Candle's ROCm story
  matures.
- Cloud-burst training (RunPod, Vast) is in scope — those are CUDA
  providers; they just happen to be remote.

### 4.2 Data-parallel via signed envelope

The MVP distributed-training pattern is **data parallel** (each rank
holds a full model copy; gradients all-reduce per step). Specifically:

1. Each rank journals its gradient shard to the op-log as a signed
   `GradientShard` envelope.
2. The lock-leader (SSOT P0-T2) sums shards across ranks and emits the
   reduced gradient as a fresh op-log entry.
3. Each rank reads the reduced gradient, applies it, and proceeds.

This is slower than NCCL all-reduce by orders of magnitude. It is also
*audit-trail-correct*: every gradient exchange is a signed op-log entry
that survives daemon restart, gets checkpointed into the SSOT P3-T9
projection architecture, and can be replayed for incident review. We
take the speed hit at v0.7 and revisit native-NCCL in v1.x.

### 4.3 Checkpoints into op-log CAS

Per Mn-T6: a checkpoint = a signed CAS bundle (SSOT P2-T1) referenced
from an op-log entry (SSOT P3-T1). Resume is *fetch-by-hash* + Candle
restore.

This means:

- Checkpoints are content-addressed; collision-free, versionable, gossipable.
- Resuming on a *different* host requires only that the new host has
  CUDA + the bundle hash; the bundle ships across the mesh.
- The "what was the loss curve at step N?" question is answered by
  scrubbing the op-log (SSOT P4-T5).

### 4.4 Annotations

```vox
// vox:skip
@training_step
fn step(model: Llama, batch: Batch) -> StepResult {
    // standard forward/backward, returns loss + grads
}

@distributed_train(strategy = "data_parallel", peers = 4)
workflow train_run(cfg: TrainConfig) -> ModelBundle {
    // emits GradientShard envelopes; expects 4 ranks
}
```

The compiler:

1. Checks `@training_step` carries the `GpuCompute + Random + Mutate`
   effect row.
2. Checks `@distributed_train` is on a `workflow` (durable, replayable)
   not a plain `fn`.
3. Checks the call-site host's `cuda_tier` (from the probe) is
   ≥ the configured threshold (default 70 — so RTX 30-series and up).
4. Refuses to lower if any rank in the cohort lacks CUDA. The diagnostic
   is `vox/train/cuda-required` and names the offending peer.

### 4.5 Admission via WorkerDonationPolicy

Per Mn-T7, the `WorkerDonationPolicy` type in
[`vox-mesh-types`](../../../crates/vox-mesh-types/) gains:

```rust
pub struct WorkerDonationPolicy {
    // existing fields ...
    pub accepts_training_workloads: bool,
    pub accepts_inference_workloads: bool,
    pub cuda_tier: u8,             // 0..100; gate matches probe tier
    pub metal_tier: u8,
    pub vram_min_gb: u32,
    pub accepts_sensitive_training_data: bool,
}
```

This is the single SSOT for "can this peer train? can it serve
inference? at what tier? does it accept private training inputs?". The
mesh planner reads it on every dispatch.

### 4.6 SSOT integration points (training side)

| Mn-T | SSOT task | Use |
|---|---|---|
| Mn-T1 | P3-T1 (oplog) | GradientShard / CheckpointBundle land as signed op-log entries |
| Mn-T5 | P5-T9 (privacy class) | `accepts_sensitive_training_data` extends the privacy taxonomy |
| Mn-T6 | P2-T1, P3-T1 | Checkpoint = CAS bundle referenced from oplog |
| Mn-T7 | P5-T9 (donation policy) | New training fields land alongside the existing privacy field |
| Mn-T10 | P4-T9 (run-row drawer) | Per-shard spans render in the run-row event tree |

---

## §5 MENS language-corpus gaps

The MENS model exists to *emit Vox programs*. If MENS doesn't know about
the durability spine, it will emit non-durable code; if it doesn't know
about the effect rows, it will emit code that fails `vox check`; if it
doesn't know the diagnostic IDs, its error-recovery suggestions will be
gibberish.

This section enumerates what is missing from the corpus today.

### 5.1 The five spine primitives

Every Vox author — human or model — must know:

1. **`DurablePromise[T]`** (SSOT P1-T1). The single awaitable. Subsumes
   Future, Promise, activity-result, signal-await, awakeable. The
   corpus needs hundreds of examples that demonstrate
   `await durable_promise` semantics, replay-after-crash behavior, and
   the relationship to `@activity` results.
2. **Effect rows.** Every function in Vox carries an effect row
   (`{Net, FS, Spawn, Random, GpuCompute, Mutate, ...}`) inferred
   bottom-up (SSOT P1-T6). The corpus needs effect-annotated examples
   so that MENS emissions don't drop effects on the floor.
3. **CAS (content-addressed storage).** Every Vox bundle is hashed
   (SSOT P2-T1). The corpus needs examples of `@generated-hash` stamps,
   bundle-fetch-by-hash idioms, and `vox-package` artifact-cache
   integration.
4. **Signed op-fragments.** The op-log is a stream of signed Ed25519
   envelopes (SSOT P3-T2). The corpus needs examples of
   `OpFragmentEnvelope` construction, signature verification flow, and
   how to journal a side-effect into the log.
5. **Capability tokens.** VCS capability mints (SSOT P3-T6) gate
   destructive operations. The corpus needs examples of
   `WorkingTreeWrite`, `BranchCreate`, etc., used as type-level proofs.

### 5.2 Annotations

The corpus must teach the *full* annotation set, not a subset:

| Annotation | Meaning | Corpus needs |
|---|---|---|
| `@remote` | Function may run on another peer | Effect-row consequences, serializability constraint |
| `@uses(secret)` | Function reads a vox-secrets secret | Injection flow, denial diagnostic |
| `@with_id(expr)` | Override auto-derived activity_id | When to use: business-key idempotency |
| `@activity(dedup="7d")` | Cache result for window | The dedup ledger row, replay semantics |
| `@training_step` | Function is one step of a training loop | Effect row `{GpuCompute, Random, Mutate}`; CUDA gate |
| `@inference` | Function performs model inference | Effect row `{GpuCompute, Random, Net?}`; routing via probe |
| `@distributed_train(...)` | Workflow runs distributed training | Strategy params, peer count, fail-stop semantics |

### 5.3 Keywords

`workflow`, `activity`, `actor`, `side_effect { ... }` — these are not
mere sugar. The corpus must demonstrate:

- A `workflow` body that calls `activity` calls and `await`s
  `DurablePromise`s.
- An `actor` with a mailbox, supervised by the actor runtime.
- A `side_effect { ... }` block as the *only* sanctioned source of
  non-determinism inside a `workflow` (SSOT P1-T7).

Today's MENS will happily emit `async fn` Rust because that's what
upstream training data looks like. The corpus must outweigh that prior.

### 5.4 The VoxScript-First Glue Code rule

Per [AGENTS.md §VoxScript-First Glue Code](../../../AGENTS.md): any
glue script in the project is a `.vox` file, not a `.ps1` / `.sh` /
`.py`. The corpus must include hundreds of `.vox` glue examples
(corpus harvesters, CI shims, devloop helpers) so MENS *defaults* to
emitting `.vox` when asked to "write a script".

### 5.5 Diagnostic IDs

Every Vox diagnostic has a stable kebab ID in the `vox/<category>/<kebab>`
namespace (SSOT P1-T9, [vox-language-rules-and-enforcement-plan-2026.md](vox-language-rules-and-enforcement-plan-2026.md)).
The corpus needs:

- A `diagnostics/` directory of synthetic "this snippet emits
  `vox/workflow/non-deterministic-builtin`" examples.
- A "what does this diagnostic mean? how do I fix it?" QA pair set.
- A "show me a diff that resolves this diagnostic" rewrite set.

### 5.6 Auto-derived activity_id semantics

Per SSOT P1-T4: `activity_id = blake3(workflow_id ‖ call_site_span ‖
structural_arg_hash ‖ replay_counter)`. The corpus must teach the model
that:

- You almost never write `@with_id` by hand.
- The compiler emits the hash inputs at the call site.
- Time-dependent args (`time.now()`) inside the call expression cause a
  warning; the fix is `side_effect { ... }`.

### 5.7 Mesh-control vocabulary

The mesh has a small but specific vocabulary:

- *Lock-leader* (SSOT P0-T2) — the daemon that arbitrates lock writes.
- *Op-fragment* (SSOT P3-T1) — a signed unit of op-log mutation.
- *Sealed mint* (SSOT P3-T6) — a capability minted via the sealed
  trait, not via the deprecated `pub fn mint`.
- *Signed attestation* (SSOT P5-T4) — the result-attestation envelope.
- *Kudos primitive* (SSOT P5-T7) — non-fungible accounting credit.

MENS should recognize these terms, not paraphrase them.

### 5.8 Vox.toml `[mesh.*]` keys

The corpus must include real `Vox.toml` examples with:

- `[mesh.transport]` (SSOT P0-T5) — TLS / WireGuard config.
- `[mesh.policy]` (Mn-T7) — donation policy fields.
- `[mesh.budget]` (P4-T6) — spend gauges.
- `[mesh.privacy]` (P4-T10) — privacy-class default.

### 5.9 Corpus collection sources

Per Mn-T11, the corpus collector harvests:

1. **Doc prose** — every `docs/src/architecture/*.md` with
   `training_eligible: true` in frontmatter (this doc included).
2. **Real `.vox` sources** — every `.vox` file in the workspace and the
   bundled examples.
3. **Diagnostic-emitter synthetic data** — programs *designed* to emit
   each diagnostic, paired with the diagnostic's expected output.
4. **Test-corpus golden files** — `cargo test`'s expectation files
   carry inputs and expected outputs; both are fair game.
5. **Op-log replay traces** — anonymized op-log slices show real
   workflow progression patterns.

### 5.10 Curriculum staging

Per Mn-T11, training proceeds in stages, each with a held-out eval
(Mn-T12):

| Stage | Focus | Eval gate |
|---|---|---|
| 1. Foundations | syntax, types, imports, `workflow`/`activity`/`actor` keywords | program compiles |
| 2. Effects | effect-row inference, `@uses(secret)`, `side_effect { }` | passes `vox check` |
| 3. Durability | `DurablePromise[T]`, `@with_id`, `@activity(dedup=...)`, replay | survives forced kill-9 + restart |
| 4. Mesh primitives | `@remote`, capability tokens, op-fragment | round-trips through mesh fixture |
| 5. Multi-agent VCS | lock-leader, op-fragment gossip, capability mints | passes 5-agent forced-conflict golden |

---

## §6 The Mn-T plan (concrete tasks)

Each task lists files (Create / Modify), substeps in TDD shape (failing
test → make it pass), an acceptance gate, and SSOT dependencies.

### Mn-T1 — `vox-distributed-training` (L2 crate)

**Goal.** Replace
[`crates/vox-populi/src/mens/tensor/populi_train.rs`](../../../crates/vox-populi/src/mens/tensor/mod.rs)
with a real distributed-training crate.

**Files Create.**

- `crates/vox-distributed-training/Cargo.toml` (L2; deps:
  `vox-mesh-types`, `vox-crypto`, `vox-populi` for hardware probe,
  `vox-orchestrator-queue` for op-log access, `candle-core`).
- `crates/vox-distributed-training/src/lib.rs`.
- `crates/vox-distributed-training/src/session.rs` —
  `TrainingSession` trait.
- `crates/vox-distributed-training/src/gradient.rs` — `GradientShard`.
- `crates/vox-distributed-training/src/checkpoint.rs` — `CheckpointBundle`.
- `crates/vox-distributed-training/src/strategy/mod.rs` — strategy enum.
- `crates/vox-distributed-training/src/strategy/data_parallel.rs`.
- `crates/vox-distributed-training/tests/single_host_smoke.rs`.

**Files Modify.**

- Delete `crates/vox-populi/src/mens/tensor/populi_train.rs`; redirect
  its callers (audit reveals only `mens/tensor/mod.rs` re-exports it).
- `crates/vox-populi/src/mens/tensor/mod.rs` — drop `populi_train` mod.
- `docs/src/architecture/where-things-live.md` — Mn-T15 adds the row.
- `docs/src/architecture/layers.toml` — register new L2 crate.

**Sketch.**

```rust
pub trait TrainingSession: Send + Sync {
    fn rank(&self) -> u32;
    fn world_size(&self) -> u32;
    async fn step(&mut self, batch: Batch) -> StepResult;
    async fn all_reduce(&mut self, shard: GradientShard) -> Result<GradientShard>;
    async fn checkpoint(&mut self) -> Result<CheckpointBundle>;
    async fn resume(&mut self, bundle: CheckpointBundle) -> Result<()>;
}

pub struct GradientShard {
    pub session_id: SessionId,
    pub step: u64,
    pub rank: u32,
    pub tensor_blob_hash: Sha3_512,
    pub signature: Ed25519Signature,
}

pub struct CheckpointBundle {
    pub session_id: SessionId,
    pub step: u64,
    pub bundle_hash: Sha3_512,        // CAS hash of the SafeTensors bundle
    pub optimizer_state_hash: Sha3_512,
    pub signature: Ed25519Signature,
}
```

**Substeps.**

- [ ] Failing test: `cargo test -p vox-distributed-training
  single_host_smoke` — instantiate a 1-rank `DataParallelSession`,
  step + all-reduce + checkpoint + resume; assert checkpoint round-trip
  matches.
- [ ] Implement `TrainingSession` trait + `DataParallelSession` struct.
- [ ] Implement `GradientShard` + Ed25519 signing via `vox-crypto`.
- [ ] Implement `CheckpointBundle` referencing a CAS bundle hash.
- [ ] Wire to `vox-populi` hardware probe for rank-local CUDA tier.
- [ ] Delete `populi_train.rs`; update re-exports.

**Acceptance.** Single-rank end-to-end training run produces a signed
checkpoint, restartable from the bundle hash. `cargo run -p
vox-arch-check` clean.

**Dependencies.** SSOT P0-T6 (probe), P2-T1 (CAS), P3-T1 (oplog),
P3-T2 (signing). Mn-T3 (ModelBundle) lands first or in the same series.

**Commit suffix.** `(Mn-T1)`.

---

### Mn-T2 — `vox-inference` (L2 crate)

**Goal.** A single trait + multiple impls for inference dispatch.

**Files Create.**

- `crates/vox-populi/src/inference/Cargo.toml` (L2; deps:
  `vox-mesh-types`, `vox-populi` for probe, `vox-package` for CAS,
  `vox-plugin-mens-candle-cuda` *behind a feature flag* — see Mn-T13).
- `crates/vox-populi/src/inference/src/lib.rs`.
- `crates/vox-populi/src/inference/src/backend.rs` — `InferenceBackend` trait,
  `BackendCapabilities`, `BackendId`, `Verdict`.
- `crates/vox-populi/src/inference/src/backends/candle_cuda.rs`.
- `crates/vox-populi/src/inference/src/backends/candle_metal.rs` (Mn-T13 fills in).
- `crates/vox-populi/src/inference/src/backends/candle_cpu.rs`.
- `crates/vox-populi/src/inference/src/backends/llama_cpp_rpc.rs`.
- `crates/vox-populi/src/inference/src/backends/ollama_subprocess.rs`.
- `crates/vox-populi/src/inference/src/dispatcher.rs` — selection logic.
- `crates/vox-populi/src/inference/tests/dispatcher_routing.rs`.

**Sketch.**

```rust
pub trait InferenceBackend: Send + Sync {
    fn id(&self) -> BackendId;
    fn capabilities(&self) -> BackendCapabilities;
    fn can_serve(&self, bundle: &ModelBundle) -> Verdict;
    fn load(&self, bundle: &ModelBundle) -> Result<LoadedModel>;
    async fn predict(&self, m: &LoadedModel, p: PromptInput, s: SamplingParams) -> Result<PredictStream>;
    fn unload(&self, m: LoadedModel) -> Result<()>;
}

pub enum BackendId { CandleCuda, CandleMetal, CandleCpu, LlamaCppRpc, OllamaSubprocess, External }

pub struct BackendCapabilities {
    pub cuda_tier: u8, pub metal_tier: u8, pub vram_gb: u32,
    pub max_context_len: u32, pub streaming: bool,
    pub quantizations: Vec<Quantization>,
}
```

**Substeps.**

- [ ] Failing test: `dispatcher_routing` — given a fixture probe
  (CUDA tier 80, Metal tier 0, 24 GB VRAM) and a 7B Q4 bundle,
  dispatcher chooses `CandleCuda`. Given Metal-only fixture, chooses
  `CandleMetal`. Given neither, chooses `CandleCpu`.
- [ ] Implement trait + dispatcher.
- [ ] Wire `CandleCuda` impl by delegating to the existing
  `vox-plugin-mens-candle-cuda::inference`.
- [ ] Add `CandleCpu` impl (smoke-test backend).
- [ ] Stub `CandleMetal` (Mn-T13), `LlamaCppRpc`, `OllamaSubprocess`.

**Acceptance.** All backend impls return correct
`BackendCapabilities`; dispatcher chooses correctly across the three
fixture probes. CPU smoke test runs in CI.

**Dependencies.** SSOT P0-T6 (probe), P0-T7 (SkillRuntime trait
shape — InferenceBackend mirrors). Mn-T3 (ModelBundle).

**Commit suffix.** `(Mn-T2)`.

---

### Mn-T3 — Content-addressed `ModelBundle`

**Goal.** Models are addressed by hash. `vox-package` artifact cache
is the on-disk substrate.

**Files Create.**

- `crates/vox-package/src/model_bundle.rs` — `ModelBundle` type and
  CAS lookup helpers.

**Files Modify.**

- `crates/vox-package/src/artifact_cache.rs` — add
  `lookup_model(model_hash) -> Option<ModelBundle>` and
  `store_model(bundle) -> Result<Sha3_512>`.
- `crates/vox-package-types/src/lib.rs` — add `ModelBundleRef`
  pure-data type.

**Sketch.**

```rust
pub struct ModelBundle {
    /// For single-file models, the SHA3-512 of the weights file.
    /// For multi-shard models, the Merkle root over per-shard SHA3-512 leaves
    /// (so partial-fetch verification works without downloading every shard).
    pub weights_hash: Sha3_512,
    pub weights_merkle_leaves: Option<Vec<Sha3_512>>,  // None for single-file
    pub tokenizer_hash: Sha3_512,
    pub config_hash: Sha3_512,
    pub bundle_hash: Sha3_512,         // SHA3-512 over the three above, sorted
    pub format: WeightFormat,          // SafeTensors only for now
    pub provenance: BundleProvenance,  // who signed it, when
}

pub enum WeightFormat { SafeTensorsSingle, SafeTensorsSharded { index_hash: Sha3_512 } }
```

**Sharded weight verification.** For multi-shard SafeTensors models (e.g., 70B+ split into 8+
files), `weights_hash` is a Merkle root and `weights_merkle_leaves` carries the per-shard
SHA3-512 leaves. A worker that fetches only shards 3–5 verifies them against the root by
recomputing the path through the tree from the leaves it received. Single-file models leave
`weights_merkle_leaves = None`. Mn-T3 substeps include the Merkle construction helper in the
`vox-package` extension.

**Substeps.**

- [ ] Failing test: `cargo test -p vox-package model_bundle_roundtrip`
  — store a 3-file bundle (weights/tokenizer/config), look up by
  bundle hash, get the same bundle back; verify all four hashes match.
- [ ] Implement `ModelBundle` + serialization (CBOR via existing
  vox-package surface).
- [ ] Add CAS store + lookup.
- [ ] Integrate sharded-SafeTensors index file (HF convention).
- [ ] Add Merkle-root construction helper for sharded weights; failing
  test verifies a single-shard partial fetch validates against the root
  using only its leaves' sibling path.
- [ ] Implement `BundleMeta` from P2-T1g for `ModelBundle`:

  ```rust
  impl vox_package_types::bundle_meta_sealed::Sealed for ModelBundle {}
  impl vox_package_types::BundleMeta for ModelBundle {
      fn content_hash(&self) -> [u8; 64] { self.bundle_hash }
      fn kind_label(&self) -> &'static str { "model" }
  }
  ```

  Run: `cargo check -p vox-package 2>&1 | tail -10` Expected: clean.

**Acceptance.** A 7B model imports → CAS stores 3 files + 1 manifest →
lookup by bundle hash returns the right files. `ModelBundle` implements
`BundleMeta`. `vox-arch-check` clean.

**Dependencies.** SSOT P2-T1 (artifact CAS + `BundleMeta` sealed trait from P2-T1g — must land first).

**Commit suffix.** `(Mn-T3)`.

---

### Mn-T4 — `@inference` annotation

**Goal.** A function annotated `@inference` carries a known effect row
and is routed through the InferenceBackend dispatcher.

**Files Create.**

- `crates/vox-compiler/src/annotations/inference.rs`.

**Files Modify.**

- `crates/vox-compiler/src/parser/mod.rs` — recognize `@inference`.
- `crates/vox-compiler/src/typeck/effect_check.rs` — add effect row
  `{GpuCompute, Random, Net?}` for `@inference` functions.
- `crates/vox-codegen/src/lower.rs` — lower `@inference` calls to a
  `vox-inference::dispatch(...)` runtime call.
- `crates/vox-actor-runtime/src/inference.rs` (or wherever the runtime hooks
  live) — bridge to `vox-inference`.

**Sketch (Vox source).**

```vox
@inference(model = "llama-3.1-8b-q4")
fn predict(input: PromptInput) -> DurablePromise[Output] {
    // body is implicit: dispatch to backend
}
```

**Substeps.**

- [ ] Failing test: parser recognizes `@inference(model = "...")`.
- [ ] Failing test: type-check assigns the right effect row.
- [ ] Failing test: codegen emits a `vox_inference::dispatch` call.
- [ ] Failing test: routing decision goes through SSOT P5-T8 inventory
  fixture.
- [ ] Implement parser, typeck, codegen.

**Acceptance.** A `@inference` function compiles, type-checks with the
expected effect row, and routes correctly in an integration fixture.

**Dependencies.** Mn-T2, Mn-T3. SSOT P0-T7 (SkillRuntime), P1-T6 (effect-row enum extension to add `GpuCompute` and `Mutate` variants — see SSOT §1.2 update), P5-T8 (mesh inventory).

**Commit suffix.** `(Mn-T4)`.

---

### Mn-T5 — `@training_step` + `@distributed_train`

**Goal.** Training annotations with compile-time CUDA gating and
strategy params.

**Files Create.**

- `crates/vox-compiler/src/annotations/training.rs`.

**Files Modify.**

- `crates/vox-compiler/src/parser/mod.rs` — `@training_step`,
  `@distributed_train(strategy = ..., peers = ...)`.
- `crates/vox-compiler/src/typeck/effect_check.rs` — effect rows
  `{GpuCompute, Random, Mutate}` for `@training_step`,
  `{Spawn, Net, GpuCompute, Random, Mutate}` for `@distributed_train`.
- `crates/vox-compiler/src/typeck/cuda_gate.rs` (new) — refuses to
  lower if call-site host's `cuda_tier < N` (configurable; default 70).
- `crates/vox-codegen/src/lower.rs` — lower to
  `vox-distributed-training` runtime calls.

**Substeps.**

- [ ] Failing test: parser recognizes both annotations with params.
- [ ] Failing test: `vox check` rejects `@training_step` on a host
  with no CUDA, with diagnostic `vox/train/cuda-required`.
- [ ] Failing test: `@distributed_train` workflow with `peers = 4`
  emits 4 `GradientShard` envelopes per step in fixture.
- [ ] Implement parser, typeck, cuda_gate, codegen.

**Acceptance.** A `@distributed_train` workflow lowers to
`vox-distributed-training::DataParallelSession`; CUDA-less host fails
`vox check` with a clear diagnostic.

**Dependencies.** Mn-T1, Mn-T3. SSOT P0-T6 (probe), P1-T6 (effect-row enum extension to add `GpuCompute` and `Mutate` variants — see SSOT §1.2 update; Mn-T5 cannot land ahead of P1-T6).

**Commit suffix.** `(Mn-T5)`.

---

### Mn-T6 — Training checkpoint as signed CAS bundle in op-log

**Goal.** Each `CheckpointBundle` becomes a signed op-log entry whose
payload is a CAS bundle reference. Resume = fetch-by-hash + Candle
restore.

**Files Modify.**

- `crates/vox-distributed-training/src/checkpoint.rs` — wire to op-log.
- `crates/vox-orchestrator-queue/src/oplog/store.rs` — accept
  `OpFragmentKind::TrainingCheckpoint { session_id, bundle_hash }`.
- `crates/vox-distributed-training/src/session.rs` —
  `TrainingSession::resume(bundle: CheckpointBundle)` fetches by hash
  via `vox-package` CAS.

**Substeps.**

- [ ] Failing test: a step emits a checkpoint; the op-log contains a
  `TrainingCheckpoint` entry; the CAS contains the bundle.
- [ ] Failing test: `resume` on a fresh process fetches by hash,
  restores state, takes the next step from the right offset.
- [ ] Implement op-log entry kind, CAS store, restore path.

**Acceptance.** Kill the training process mid-step; restart; the
session resumes from the last checkpoint without redoing completed
steps. Op-log replay reconstructs the training history.

**Dependencies.** Mn-T1, Mn-T3. SSOT P3-T1 (oplog persist), P3-T2
(signing), P3-T9 (projections).

**Commit suffix.** `(Mn-T6)`.

---

### Mn-T7 — `WorkerDonationPolicy` extensions

**Goal.** The donation policy carries enough info to route training and
inference correctly.

**Files Modify.**

- `crates/vox-mesh-types/src/donation_policy.rs` (or wherever
  `WorkerDonationPolicy` lives) — add the new fields.
- `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs` — admission
  consults the new fields.
- `crates/vox-mesh-policy/...` (created in SSOT P4-T3) — schema
  update for `donations.vox`.

**Sketch.**

```rust
pub struct WorkerDonationPolicy {
    pub accepts_inference_workloads: bool,
    pub accepts_training_workloads: bool,
    pub cuda_tier: u8,                 // 0..100 (matches probe)
    pub metal_tier: u8,
    pub vram_min_gb: u32,
    pub accepts_sensitive_training_data: bool,
    // ... existing fields preserved ...
}
```

**Distinct from P5-T9.** `WorkerDonationPolicy.accepts_sensitive_training_data` (Mn-T7) gates
**training-data privacy** — whether the worker may receive batches containing sensitive
upstream data. This is *complementary*, not redundant, with P5-T9's `accept_sensitive_workloads`,
which gates **plaintext workloads the worker observes** in non-training contexts. Both flags must
hold for a sensitive training admission. Both ship on the same `WorkerDonationPolicy` struct;
neither subsumes the other.

**Substeps.**

- [ ] Failing test: a peer with `accepts_training_workloads = false`
  refuses a `@distributed_train` admission.
- [ ] Failing test: a peer with `cuda_tier = 50` refuses a workload
  configured for tier ≥ 70.
- [ ] Failing test: `accepts_sensitive_training_data = false` peer
  refuses a workload tagged `privacy_class = "sensitive"`.
- [ ] Implement new fields + admission checks + `donations.vox` schema.

**Acceptance.** All three admission tests pass. Existing donation
policy tests still pass. `donations.vox` round-trips through Vox
parser.

**Dependencies.** SSOT P4-T3 (donation editor), P5-T9 (privacy class).

**Commit suffix.** `(Mn-T7)`.

---

### Mn-T8 — `vox model` CLI

**Goal.** First-class CLI for managing CAS-addressed model bundles.

**Files Create.**

- `crates/vox-cli/src/commands/model/mod.rs`.
- `crates/vox-cli/src/commands/model/ls.rs`.
- `crates/vox-cli/src/commands/model/push.rs`.
- `crates/vox-cli/src/commands/model/pull.rs`.
- `crates/vox-cli/src/commands/model/import_safetensors.rs`.

**CLI shape.**

```text
vox model ls                  # list bundles in local CAS
vox model push <bundle-hash>  # gossip availability
vox model pull <bundle-hash>  # fetch from a peer that has it
vox model import <path>       # convert / verify SafeTensors → CAS bundle
```

**Substeps.**

- [ ] Failing test: `vox model ls` prints the bundles created by
  Mn-T3's roundtrip test.
- [ ] Failing test: `vox model import some.safetensors` produces a
  bundle whose `weights_hash` matches the file's SHA3-512.
- [ ] Implement subcommands.

**Acceptance.** All four subcommands work end-to-end against a local
CAS; `pull` works against a fixture peer.

**Dependencies.** Mn-T3. SSOT P5-T8 (inventory).

**Commit suffix.** `(Mn-T8)`.

---

### Mn-T9 — Dashboard inference-router viz

**Goal.** The mesh canvas (SSOT P4-T4) gains a model-availability
overlay; the model-registry view (SSOT P4-T12) gains backend tier
pills.

**Files Modify.**

- `crates/vox-dashboard/src/api/mesh.rs` — extend the live state with
  per-node `BackendCapabilities` + bundle inventory snapshot.
- `crates/vox-dashboard/app/src/generated/NetworkTab.tsx` — overlay
  layer rendering small "model-X is here" badges on nodes.
- `crates/vox-dashboard/app/src/generated/ModelsTab.tsx` (new) —
  per-model rows with backend tier pills.

**Substeps.**

- [ ] Failing UI fixture: a node hosting `llama-3.1-8b` shows the
  model badge.
- [ ] Failing UI fixture: clicking "predict" routes to the right node.
- [ ] Implement API + UI.

**Acceptance.** "Who can run this model?" is a single-glance answer in
the dashboard. Inventory updates within ≤ 30 s of model push/pull.

**Dependencies.** Mn-T2, Mn-T3, Mn-T8. SSOT P4-T4, P4-T12, P5-T8.

**Commit suffix.** `(Mn-T9)`.

---

### Mn-T10 — Distributed training observability

**Goal.** Per-shard spans, signed checkpoint events, and
`vox.train.*` telemetry namespace surface in the run-row drawer.

**Files Create.**

- `crates/vox-distributed-training/src/telemetry.rs` — emit
  `vox.train.step`, `vox.train.gradient_shard`,
  `vox.train.checkpoint`, `vox.train.all_reduce` spans.

**Files Modify.**

- `crates/vox-dashboard/src/api/runs.rs` — surface the new event
  kinds in the event tree.
- `crates/vox-dashboard/app/src/generated/RunRowDrawer.tsx` —
  per-shard sub-row rendering.

**Substeps.**

- [ ] Failing test: a 4-rank training step emits 4 `gradient_shard`
  events in the event tree.
- [ ] Failing test: a checkpoint emits a signed `checkpoint` event
  with the bundle hash.
- [ ] Implement telemetry + dashboard wiring.

**Acceptance.** A real training run shows per-shard timing in the
drawer; signed checkpoint events are visible and clickable.

**Dependencies.** Mn-T1, Mn-T6. SSOT P4-T9 (run-row drawer).

**Commit suffix.** `(Mn-T10)`.

---

### Mn-T11 — Corpus-collection `.vox` script

**Goal.** Harvest docs + sources + diagnostics into HuggingFace-Datasets
JSONL. **No Python.**

**Files Create.**

- `scripts/mens-corpus/harvest.vox` — root entry point.
- `scripts/mens-corpus/walk_docs.vox` — frontmatter filter for
  `training_eligible: true`.
- `scripts/mens-corpus/walk_sources.vox` — `.vox` file walker.
- `scripts/mens-corpus/emit_diagnostics.vox` — synthetic
  diagnostic-paired snippets.
- `scripts/mens-corpus/jsonl_writer.vox` — schema-conformant emitter.

**Files Reference (do not create).**

- `docs/src/architecture/vox-mens-corpus-schema.md` — schema doc
  referenced by the script. *Authored separately; this plan only
  points at it.*

**Substeps.**

- [ ] Failing test: `vox run scripts/mens-corpus/harvest.vox --dry-run`
  reports a non-zero number of harvested records from the workspace.
- [ ] Failing test: schema validation of emitted JSONL rejects a
  malformed record.
- [ ] Implement walkers, filters, schema validator, JSONL emitter.

**Acceptance.** `vox run scripts/mens-corpus/harvest.vox --out
/tmp/mens-corpus.jsonl` produces a valid corpus snapshot. No `.py` in
the tree. No `.sh`. No `.ps1`.

**Dependencies.** None outside the language itself.

**Commit suffix.** `(Mn-T11)`.

---

### Mn-T12 — Eval harness

**Goal.** Held-out prompts; emissions must compile + pass effect check
+ be durability-correct. Failures land as `vox/eval/<diagnostic>`.

**Files Create.**

- `crates/vox-mens-eval/Cargo.toml` (L3).
- `crates/vox-mens-eval/src/lib.rs`.
- `crates/vox-mens-eval/src/prompts.rs` — held-out set.
- `crates/vox-mens-eval/src/runner.rs` — emit → compile → effect-check
  → durability-replay.
- `crates/vox-mens-eval/tests/eval_harness.rs`.

**Sketch.**

```rust
pub struct EvalReport {
    pub prompt: String,
    pub emission: String,
    pub compiled: bool,
    pub effect_check: bool,
    pub durability_replay: Option<DurabilityVerdict>,
    pub diagnostic_id: Option<String>,
}
```

**Substeps.**

- [ ] Failing test: a synthetic emission containing `time.now()` inside
  a `workflow` body is flagged with
  `vox/workflow/non-deterministic-builtin`.
- [ ] Failing test: an emission missing an effect declaration is
  flagged.
- [ ] Implement runner; integrate with `vox-compiler`.

**Acceptance.** Full eval suite runs as a `cargo test` integration;
each failure produces a stable `vox/eval/<diagnostic>` ID.

**Dependencies.** SSOT P1 (DurablePromise, effect rows, diagnostic IDs).

**Commit suffix.** `(Mn-T12)`.

---

### Mn-T13 — Apple Silicon inference path

**Goal.** Fill in `CandleMetal` impl from Mn-T2; provide an opt-in
`mlx` shim.

**Files Modify.**

- `crates/vox-populi/src/inference/src/backends/candle_metal.rs` — full impl.
- `crates/vox-populi/src/inference/Cargo.toml` — `metal` feature.

**Files Create (opt-in).**

- `crates/vox-plugin-mens-mlx/Cargo.toml` (L3 plugin, opt-in feature).
- `crates/vox-plugin-mens-mlx/src/inference.rs` — mlx FFI bridge.

**Substeps.**

- [ ] Failing test: on a Metal-capable host, `CandleMetal` loads a
  small model and returns a prediction.
- [ ] Failing test: `cargo test -p vox-inference --features metal`
  runs in CI on macos-latest only.
- [ ] Implement; gate mlx behind a feature flag (license/binding
  maturity).

**Acceptance.** Apple Silicon laptop can serve inference on the mesh.
mlx feature flag is off by default.

**Dependencies.** Mn-T2.

**Commit suffix.** `(Mn-T13)`.

---

### Mn-T14 — Petals-style swarm inference (stretch)

**Goal.** Layer-partition a huge model across volunteer GPUs; pipeline
forward pass over the mesh. Tied to v1.x grand network (SSOT P6).

**Files Create.**

- `crates/vox-inference-swarm/Cargo.toml` (L3, opt-in `swarm` feature).
- `crates/vox-inference-swarm/src/layer_split.rs`.
- `crates/vox-inference-swarm/src/pipeline.rs`.
- `crates/vox-inference-swarm/src/coordinator.rs`.

**Sketch (high level).**

```rust
pub struct SwarmPlan {
    pub model_hash: Sha3_512,
    pub layer_assignments: Vec<(LayerRange, NodeId)>,
    pub coordinator: NodeId,
}
```

**Substeps.**

- [ ] Failing test: a 2-node fixture splits a 12-layer model into 6+6
  and produces an identical output to the single-host baseline.
- [ ] Failing test: a node drop mid-inference triggers re-routing.
- [ ] Implement layer-split, pipeline forward, coordinator.

**Acceptance.** Two-node toy run produces matching outputs to the
baseline. Real "BLOOM-176B on 30 volunteer GPUs" is explicitly *not*
required for v0.6/v0.7/v1.0; document the gap and ship the seam.

**Dependencies.** Mn-T2, Mn-T3, Mn-T9. SSOT P6-T1 (federation
envelope), P6-T4 (redundant execution).

**Commit suffix.** `(Mn-T14)`.

---

### Mn-T15 — Update `where-things-live.md`

**Goal.** Three new crates appear in the lookup table.

**Files Modify.**

- `docs/src/architecture/where-things-live.md` — three new rows under
  L2/L3 sections.

**Rows to add.**

```text
| vox-distributed-training | Distributed training session, gradient shards, checkpoint bundles. CUDA-only. |
| vox-inference            | InferenceBackend trait + impls (CandleCuda, CandleMetal, CandleCpu, LlamaCppRpc, OllamaSubprocess). |
| vox-mens-corpus          | MENS corpus collection schemas and harvester reference. |
| vox-mens-eval            | MENS eval harness: emit → compile → effect-check → durability-replay. |
| vox-inference-swarm      | (Stretch) Petals-style layer-partition swarm inference. |
| vox-plugin-mens-mlx      | (Opt-in) mlx FFI bridge for Apple Silicon inference. |
```

**Files Modify.**

- `docs/src/architecture/layers.toml` — register the same crates with
  layer + LoC budgets.

**Substeps.**

- [ ] Add rows in alphabetical position.
- [ ] Add layers.toml entries.
- [ ] Run `cargo run -p vox-arch-check` — clean.

**Acceptance.** `vox-arch-check` clean. The next agent can find these
crates by name.

**Dependencies.** Mn-T1, Mn-T2, Mn-T11, Mn-T12, Mn-T13, Mn-T14.

**Commit suffix.** `(Mn-T15)`.

---

## §7 Release acceptance contracts (MENS slice per SSOT release)

### 7.1 v0.6 (foundations + language spine)

Aligned with SSOT Phases 0–1.

- Mn-T15 register placeholder crates (empty shells acceptable).
- Mn-T3 `ModelBundle` lands as a pure data type in
  `vox-package-types`; CAS plumbing follows in v0.7.
- Mn-T4 `@inference` annotation parses; codegen lowers to an
  unimplemented stub that returns a clear "not yet wired" diagnostic.
- Hardware probe + cloud routing already shipped; no MENS regressions.

**Acceptance.** Existing single-host QLoRA still works.
`@inference` parses but explicitly errors at runtime with
`vox/inference/not-yet-wired`. No new dependencies.

### 7.2 v0.7 (code mobility + multi-agent VCS)

Aligned with SSOT Phases 2–3.

- Mn-T1 `vox-distributed-training` crate ships; single-rank smoke test
  green; multi-rank gated behind a feature flag.
- Mn-T2 `vox-inference` crate ships; CandleCuda + CandleCpu impls
  green; CandleMetal stub.
- Mn-T3 CAS plumbing wired end-to-end through `vox-package`.
- Mn-T6 training checkpoints as signed CAS bundles in op-log.
- Mn-T8 `vox model` CLI shipping.
- Mn-T11 corpus harvester `.vox` script lands and is run in CI.
- Mn-T12 eval harness integrates into `cargo test`.

**Acceptance.** End-to-end: import a SafeTensors file, push to mesh,
pull on a peer, run inference. Train a single-rank QLoRA, kill the
process, restart, resume from checkpoint. MENS corpus snapshot in CI
for every PR that touches the corpus-relevant files.

### 7.3 v1.0 (dashboard mesh control + public-internet trust)

Aligned with SSOT Phases 4–5.

- Mn-T5 `@training_step` + `@distributed_train` fully wired (data
  parallel only).
- Mn-T7 `WorkerDonationPolicy` extensions with all training fields.
- Mn-T9 dashboard inference-router viz.
- Mn-T10 distributed training observability.
- Mn-T13 Apple Silicon inference path.

**Acceptance.** Two-laptop mesh: one CUDA desktop, one Apple Silicon
laptop. The desktop trains a QLoRA in a `@distributed_train` workflow
across two CUDA GPUs (or simulates rank ≥ 1); the laptop serves
inference of the resulting model via Mn-T13. Dashboard shows
per-shard spans + per-model availability overlay. Donation policy
gates all of the above.

### 7.4 v1.x (grand network)

Aligned with SSOT Phase 6.

- Mn-T14 Petals-style swarm inference, opt-in.
- Federation-envelope-compatible `GradientShard` and `CheckpointBundle`
  shapes (P6-T1).
- Redundant-execution voting on deterministic inference jobs (P6-T4).
- TEE attestation interface stubbed (P6-T5).

**Acceptance.** Two GitHub-attested strangers pair their meshes and
share a model bundle. Swarm-inference toy demo runs on two volunteer
nodes without falling over when one drops.

---

## §8 Top-5 MENS-specific risks

### 8.1 Candle's distributed story is a moving target

**Risk.** Upstream Candle does not yet ship a stable, well-documented
multi-device collective. Our `DataParallelSession` builds on whatever
the current API surface is.

**Mitigation.** Wrap the Candle API behind `TrainingSession` /
`InferenceBackend` traits we own. When upstream churns, we update one
file. The traits' shapes (driven by op-log and signed envelopes) are
ours and don't depend on Candle internals.

### 8.2 SafeTensors-only is a hard constraint that conflicts with reality

**Risk.** Half the open-source models ship as GGUF (llama.cpp), some
as PyTorch pickles, some as ONNX. "SafeTensors only on disk" sounds
good but breaks the moment a user wants to use an existing GGUF.

**Mitigation.** `vox model import` (Mn-T8) is the conversion seam. We
*read* GGUF and pickles via existing tooling and *write* SafeTensors
on the way into the CAS. Users never see GGUF inside Vox; the import
diagnostic explains the conversion. For inference of GGUF specifically,
the `LlamaCppRpc` backend is the escape hatch — we don't store GGUF
in CAS but we can hand off to a GGUF-native server.

### 8.3 Gradient sync via signed op-log envelope is slow

**Risk.** All-reduce over a signed Ed25519 envelope through the op-log
is orders of magnitude slower than NCCL all-reduce. Big training jobs
won't fit in this budget.

**Mitigation.** Honest scoping: v0.7/v1.0 distributed training is for
QLoRA-scale jobs (millions, not billions, of trainable params), and
mostly for *correctness demonstrations* that the mesh can train at
all. Native NCCL bypass with op-log *summaries* (every N steps, not
every step) is a v1.x conversation. Document this clearly so users
don't try to pretrain a 70B model on the mesh and report a "bug".

### 8.4 The MENS corpus is the bottleneck

**Risk.** Even with all the engineering, MENS will emit non-Vox code
unless its training data is overwhelmingly Vox-shaped. The default
internet corpus is Python + JS + Rust; Vox is a rounding error in
that mix.

**Mitigation.** Mn-T11 (`harvest.vox`) is necessary but not
sufficient. The longer-term answer is *synthetic* data: programs
generated by exhaustive enumeration from the diagnostic emitter and
the grammar (round-tripped through `vox check`). The corpus schema
doc (referenced in Mn-T11, authored separately) must specify the
synthetic-vs-organic ratio target (we hypothesize 5:1 synthetic to
real, subject to ablation).

### 8.5 Donation-policy / privacy semantics drift

**Risk.** Mn-T7 adds five new fields to `WorkerDonationPolicy`. Each
field has subtle semantics ("does `accepts_sensitive_training_data:
false` mean refuse, or accept and ignore?"). Drift between intent and
enforcement is a privacy-policy bug.

**Mitigation.** Every field has a *fail-closed default* (refusal on
ambiguity). Every admission decision lands in the op-log with the
policy state at decision time, so the audit trail is recoverable. The
SSOT P5-T9 privacy-class taxonomy is the single semantic SSOT for the
boolean flags; this document points at it rather than redefining it.

### 8.6 Cross-developer signal pooling

**Risk.** Pooling override telemetry across users into a shared MENS
fine-tune (e.g. for a future `vox-priority-policy` learning crate) would
profile developers in aggregate.

**Mitigation.** Per-user local-only training by default; explicit opt-in
for any cross-user pooling; documented in the eval harness (Mn-T12) as a
test-fixture gate. The unified-task-hopper SSOT non-goal #10 (never
override developer-set priorities) is the upstream constraint; this
section is the MENS-side mitigation.

---

## §9 Open questions for Wave-3

These are *not* blockers for v0.6 or v0.7 but should be answered before
v1.0 ships.

1. **NCCL bypass policy.** Should we ship a CUDA-NCCL all-reduce path
   in v1.x as a *fast path* (skipping op-log signing for steps within
   a single trust domain)? If yes, what's the trust-domain definition?
2. **Federated-learning mode.** Mn-T7's
   `accepts_sensitive_training_data: false` plus a future `FedAvg`
   strategy in `vox-distributed-training/src/strategy/` would let
   privacy-conscious peers contribute *gradient updates* without
   exposing raw inputs. Is this a v1.0 commitment or v1.x?
3. **Quantization standardization.** SafeTensors itself doesn't
   specify quantization (the field is just `dtype`). We need a
   canonical Vox quantization tag (Q4_0, Q4_K_M, Q8_0, FP16, BF16) in
   `ModelBundle` so the dispatcher can match against backend
   capabilities. Where does that taxonomy live?
4. **Cloud-burst training and the CUDA-only constraint.** Does
   `mens/cloud/` (RunPod / Vast) count as "the same mesh" for
   gradient-sync purposes, or is it a *handoff* (we ship the bundle
   to the cloud and pull the trained bundle back)? The latter is
   simpler; the former composes better.
5. **Tokenizer mobility.** The `tokenizer_hash` in `ModelBundle` is
   load-bearing — but tokenizers are versioned independently and a
   1-byte difference in the tokenizer file breaks compatibility. Do
   we sign the tokenizer separately, or always treat
   `(weights, tokenizer)` as an atomic pair?
6. **Op-log retention for training.** Training generates a *lot* of
   op-log entries (one `GradientShard` per rank per step). The SSOT
   P3-T1 tiered retention (hot 10K / warm db / cold checkpoint) was
   designed for code-edit op-fragments. Is the budget right for
   training, or do training entries need their own retention class?
7. **Should `vox-priority-policy` (Hp-T9) be hosted under MENS or as an
   independent crate?** Hosting under MENS gives it the corpus-harvester
   (Mn-T11) + eval-harness (Mn-T12) + Candle trainer (Mn-T1) for free,
   but couples a developer-priority feature to the MENS release cycle.
   A separate L2 crate keeps the surfaces independent at the cost of
   duplicating eval/test scaffolding. Recommend deciding when override
   telemetry from Hp-T2 first lands (~v0.6+).

---

## §10 Crosslinks

- [Mesh & Language Distribution SSOT (2026-05-09)](mesh-and-language-distribution-ssot-2026.md) — the umbrella plan-of-record.
- [Mesh Phase 0 — Foundations Plan (2026)](mesh-phase0-foundations-plan-2026.md)
- [Mesh Phase 1 — Language Spine Plan (2026)](mesh-phase1-language-spine-plan-2026.md)
- [Mesh Phase 2 — Code Mobility Plan (2026)](mesh-phase2-code-mobility-plan-2026.md)
- [Mesh Phase 3 — VCS Gossip Plan (2026)](mesh-phase3-vcs-gossip-plan-2026.md)
- [Mesh Phase 4 — Dashboard Control Plan (2026)](mesh-phase4-dashboard-control-plan-2026.md)
- [Mesh Phase 5 — Public Internet Plan (2026)](mesh-phase5-public-internet-plan-2026.md)
- [Mesh Phase 6 — Grand Network Plan (2026)](mesh-phase6-grand-network-plan-2026.md)
- [Populi Mesh Probe Correctness Spec (2026)](populi-mesh-probe-correctness-spec-2026.md)
- [Populi Mesh Probe Correctness Plan (2026)](populi-mesh-probe-correctness-plan-2026.md)
- [Mesh, Dashboard & Distributed Compute Research (2026)](mesh-dashboard-and-distributed-compute-research-2026.md)
- [Vox Language Rules & Enforcement Plan (2026)](vox-language-rules-and-enforcement-plan-2026.md)
- [unified-task-hopper-research-2026.md](unified-task-hopper-research-2026.md) — defines the
  optional `vox-priority-policy` learning crate (`Hp-T9`); a future MENS application surface
  for inferring priority suggestions from override history. Out of scope for the current plan
  but a natural Mn-T16+ candidate when the hopper accumulates real override telemetry.
- [LANGUAGE_DESIGN_PRIORITIES.md](../../../LANGUAGE_DESIGN_PRIORITIES.md)
- [Where Things Live](where-things-live.md)
- [layers.toml](layers.toml)

---

## Appendix A — Glossary

- **MENS** — the umbrella name for Vox's ML / AI subsystem; lives in
  [`crates/vox-populi/src/mens/`](../../../crates/vox-populi/src/mens/)
  and the `vox-plugin-mens-*` plugin family.
- **CAS** — Content-Addressed Storage. SSOT P2-T1 substrate.
- **Op-log** — the single signed event stream that backs every mesh
  state mutation. SSOT P3-T1.
- **Op-fragment** — one entry in the op-log; signed, immutable, gossipable.
- **DurablePromise[T]** — the single awaitable primitive in Vox. SSOT P1-T1.
- **Effect row** — the compile-time effect set on every Vox function.
  SSOT P1-T6.
- **Lock-leader** — the daemon arbitrating lock writes for a given
  resource. SSOT P0-T2.
- **Kudos** — non-fungible accounting credit; the Vox replacement for
  a token economy.
- **Donation policy** — the per-peer config for what work this peer
  accepts.

## Appendix B — Why each prior-art verdict matters

This appendix expands the §2 verdict column with one paragraph per
KEEP / ADAPT system, so the next reviewer knows *exactly* what we
borrowed and what we left.

### B.1 PyTorch DDP — ADAPT

PyTorch DDP's contribution is the *envelope shape*: each rank holds a
full model copy; per step, gradients all-reduce across ranks; the
optimizer step is local. We copy this for our v0.7 baseline because it
has the smallest semantic surface (no parameter sharding, no pipeline
splits) and gives us the cleanest mapping to "one signed envelope per
rank per step → one summed envelope per step". We do *not* use the
PyTorch implementation; the only ML framework in the tree is Candle.

### B.2 PyTorch FSDP / DeepSpeed ZeRO — ADAPT (later)

Once a single-GPU model copy doesn't fit in VRAM, we want sharded
data parallelism: parameters and optimizer state are partitioned
across ranks, gathered on demand. This is a Phase 2+ MENS task, not in
v0.7's scope. We ADAPT the *staging concept* (start with a smaller
shard set, escalate when needed) so users don't pay sharding overhead
on small models.

### B.3 Petals — KEEP (Mn-T14)

Petals is the existence proof that volunteer-mesh inference of a
massive model is possible: BLOOM-176B served across volunteer GPUs
over the public internet by partitioning transformer *layers*. The
shape — assign layer ranges to peers, coordinator routes the forward
pass — is exactly Mn-T14. We don't import Petals (it's PyTorch
+ Hivemind + bespoke routing); we re-implement against
`vox-inference` + the SSOT P5-T8 inventory + P6-T1 federation
envelope.

### B.4 Hivemind / Learning@Home — ADAPT

Hivemind's contribution is the *churn-tolerance* default: a slow or
absent rank does not block the cohort. For a volunteer mesh where
peers come and go, this is the right default. We adopt the *posture*
(prefer eventual consistency over synchronous gates) without adopting
the codebase.

### B.5 SafeTensors — KEEP

SafeTensors is the only on-disk weight format in Vox per charter §0.2.
It is memory-mapped, zero-copy, and disallows arbitrary code
execution at load time (unlike pickle). HuggingFace publishes both
single-file and sharded variants; both are recognized by `ModelBundle`
(Mn-T3).

### B.6 BOINC adaptive replication — KEEP (P6-T4)

BOINC's adaptive replication is the prior art for redundant execution
on volunteer compute: send the same job to N untrusted peers, accept
the majority answer; once a peer demonstrates consistent agreement
across many jobs, downgrade its replication count toward 1. Our
Mn-T14 stretch and SSOT P6-T4 are direct descendants. We ADAPT the
*policy* (adaptive, not blanket); we do not adopt the *substrate*
(BOINC server-client topology is wrong for our gossip mesh).

### B.7 llama.cpp RPC — KEEP (Mn-T2 backend)

llama.cpp's RPC mode lets a server host a model while clients send
inference requests over a custom protocol; it's a tiny, well-tested
surface that the GGUF community already runs. Mn-T2's `LlamaCppRpc`
backend speaks this protocol so a Vox host can use a friend's
NAS-hosted llama.cpp server transparently. We KEEP the wire protocol;
we do not vendor llama.cpp itself.

### B.8 FedAvg — ADAPT (privacy mode)

FedAvg's contribution is the *privacy posture*: train locally on
private data; only weight deltas leave the host; a central server
averages and broadcasts. Our future `accepts_sensitive_training_data:
false` peers (Mn-T7) want exactly this shape — they keep their data,
contribute deltas. The "central server" in FedAvg becomes the
lock-leader (SSOT P0-T2) in our model. This is a Wave-3 question (§9.2).

## Appendix C — File-by-file change index

To make execution easy, this appendix lists every file the Mn-T plan
creates or modifies, grouped by task. A single PR per Mn-T is the
default; bigger tasks (Mn-T2, Mn-T9) may split.

### Mn-T1 files

Create: `crates/vox-distributed-training/Cargo.toml`,
`src/lib.rs`, `src/session.rs`, `src/gradient.rs`,
`src/checkpoint.rs`, `src/strategy/mod.rs`,
`src/strategy/data_parallel.rs`, `tests/single_host_smoke.rs`.

Delete: `crates/vox-populi/src/mens/tensor/populi_train.rs`.

Modify: `crates/vox-populi/src/mens/tensor/mod.rs`,
`docs/src/architecture/where-things-live.md` (Mn-T15),
`docs/src/architecture/layers.toml`.

### Mn-T2 files

Create: `crates/vox-populi/src/inference/Cargo.toml`, `src/lib.rs`,
`src/backend.rs`, `src/dispatcher.rs`,
`src/backends/{candle_cuda,candle_metal,candle_cpu,llama_cpp_rpc,ollama_subprocess}.rs`,
`tests/dispatcher_routing.rs`.

Modify: `crates/vox-plugin-mens-candle-cuda/src/inference.rs` to
expose its inference function through the new trait. (No behavior
change; pure wiring.)

### Mn-T3 files

Create: `crates/vox-package/src/model_bundle.rs`.

Modify: `crates/vox-package/src/artifact_cache.rs`,
`crates/vox-package-types/src/lib.rs`.

### Mn-T4 files

Create: `crates/vox-compiler/src/annotations/inference.rs`.

Modify: `crates/vox-compiler/src/parser/mod.rs`,
`crates/vox-compiler/src/typeck/effect_check.rs`,
`crates/vox-codegen/src/lower.rs`,
`crates/vox-actor-runtime/src/inference.rs` (or equivalent).

### Mn-T5 files

Create: `crates/vox-compiler/src/annotations/training.rs`,
`crates/vox-compiler/src/typeck/cuda_gate.rs`.

Modify: `crates/vox-compiler/src/parser/mod.rs`,
`crates/vox-compiler/src/typeck/effect_check.rs`,
`crates/vox-codegen/src/lower.rs`.

### Mn-T6 files

Modify: `crates/vox-distributed-training/src/checkpoint.rs`,
`crates/vox-orchestrator-queue/src/oplog/store.rs`,
`crates/vox-distributed-training/src/session.rs`.

### Mn-T7 files

Modify: `crates/vox-mesh-types/src/donation_policy.rs` (or
equivalent; verify path during implementation),
`crates/vox-orchestrator/src/a2a/dispatch/mesh.rs`,
SSOT-P4-T3 `vox-mesh-policy` schema.

### Mn-T8 files

Create: `crates/vox-cli/src/commands/model/mod.rs`, `ls.rs`,
`push.rs`, `pull.rs`, `import_safetensors.rs`.

Modify: `crates/vox-cli/src/commands/mod.rs` to register the new
command group.

### Mn-T9 files

Modify: `crates/vox-dashboard/src/api/mesh.rs`,
`crates/vox-dashboard/app/src/generated/NetworkTab.tsx`.

Create: `crates/vox-dashboard/app/src/generated/ModelsTab.tsx`.

### Mn-T10 files

Create: `crates/vox-distributed-training/src/telemetry.rs`.

Modify: `crates/vox-dashboard/src/api/runs.rs`,
`crates/vox-dashboard/app/src/generated/RunRowDrawer.tsx`.

### Mn-T11 files

Create: `scripts/mens-corpus/harvest.vox`,
`scripts/mens-corpus/walk_docs.vox`,
`scripts/mens-corpus/walk_sources.vox`,
`scripts/mens-corpus/emit_diagnostics.vox`,
`scripts/mens-corpus/jsonl_writer.vox`.

Reference (do not create here): `docs/src/architecture/vox-mens-corpus-schema.md`.

### Mn-T12 files

Create: `crates/vox-mens-eval/Cargo.toml`, `src/lib.rs`,
`src/prompts.rs`, `src/runner.rs`, `tests/eval_harness.rs`.

### Mn-T13 files

Modify: `crates/vox-populi/src/inference/src/backends/candle_metal.rs`,
`crates/vox-populi/src/inference/Cargo.toml`.

Create (opt-in): `crates/vox-plugin-mens-mlx/Cargo.toml`,
`src/inference.rs`.

### Mn-T14 files

Create: `crates/vox-inference-swarm/Cargo.toml`,
`src/layer_split.rs`, `src/pipeline.rs`, `src/coordinator.rs`.

### Mn-T15 files

Modify: `docs/src/architecture/where-things-live.md`,
`docs/src/architecture/layers.toml`.

---

## Appendix D — Why Candle and not Burn / tch / dfdx

The charter pins Candle-on-CUDA for training. A natural objection is
"why this Rust ML framework and not the others?" Brief answers:

- **`tch`** (PyTorch FFI) — pulls libtorch (~1 GB), ABI-fragile across
  PyTorch versions, depends on the C++ runtime. Disqualified by the
  no-foreign-runtime aesthetic of the rest of the workspace
  (no libgit2, no openssl, etc.).
- **`burn`** — pure Rust, ergonomic, but the ecosystem is younger;
  fewer pre-trained model implementations land per quarter than Candle.
  We will revisit if Burn's HF compatibility matures.
- **`dfdx`** — strong type-safety story but small contributor base;
  weak HF interop story.
- **Candle** — pure Rust, made by HuggingFace, ships with
  reference implementations of popular HF models (Llama, Mistral,
  Whisper, Stable Diffusion), uses CUDA via cudarc, has a lively
  contributor base, and is already the framework backing the
  existing `vox-plugin-mens-candle-cuda` plugin and
  `vox-plugin-oratio` (Whisper).

The decision is documented here and not relitigated in PRs. New ML
frameworks may be added only via an ADR.

## Appendix E — How to read this document if you're a model

If you are a Claude / GPT / Vox-trained MENS that just opened this
file: this is your entrypoint to the distributed-AI track. The five
things you need to know:

1. Training is **CUDA only**. Inference is **anything the probe says**.
2. Weights on disk are **SafeTensors**. Bundles travel **by hash**.
3. Crypto is **vox-crypto** (Ed25519, SHA3-512, BLAKE3). Don't import
   `ring` or `openssl`.
4. The 15 tasks in §6 are stable IDs; cite `Mn-T<n>` in commits.
5. Scripts are **`.vox`**. Don't generate `.py`/`.sh`/`.ps1`.

When in doubt, point at this document and ask the human reviewer.

---

*End of MENS Distributed Training & Execution Plan (2026-05-09).*
