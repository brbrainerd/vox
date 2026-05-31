# SP-1 — `vox-quantize` Core Engine Design

**Date:** 2026-05-31
**Status:** Approved (decomposition + method); SP-1 spec under review
**Author:** brainstorming session
**Scope:** First sub-project of the model-quantization initiative — the shared, CPU-only quantization engine that every later surface (CLI, merge-quant, inference) builds on.

---

## 1. Context & motivation

The user wants the ability to quantize Qwen 3.5 / 2.5 models within the existing Candle-based ML stack. An audit established:

- **Training-time quantization already ships** — `vox-plugin-mens-candle-cuda` does QLoRA with NF4 base quant via the patched `qlora-rs 1.0.5`.
- **Inference-time quantization is declared but hollow** — `vox-inference` advertises a `Quantization { Fp16, Bf16, Q8Zero, Q4K }` enum and backend capabilities, but `load`/`predict` are stubs.
- There is **no general post-training quantization (PTQ) engine** that turns a full/merged SafeTensors model into a smaller quantized artifact.

### Charter constraints (binding)

The MENS plan and ADR 003 pin hard rules this design must respect:

- **§0.2.1** Training is Candle-on-CUDA only; **Python / Unsloth is historical context only** — explicitly out of scope. The user confirmed "stay in-charter": no Python, no Unsloth.
- **§0.2.3** SafeTensors is the only on-disk weight format; GGUF/ONNX are import/export-edge only.
- **§0.2.5** No new quantization frameworks without a phase plan.

The in-charter answer is **native Rust quantization via Candle's `quantized` module**, using **data-free k-quants** (round-to-nearest into GGML block types). This was confirmed feasible against the actual code (see §9).

### Device policy (GPU-first)

The engine is **device-selectable**, defaulting to **GPU when available** (`auto` → CUDA/Metal, else CPU). Source tensors are loaded onto the selected device and quantized there via `QTensor::quantize_onto(src, dtype, dev)`; the serialized block bytes return to host for the SafeTensors artifact. GPU keeps large models off host RAM and is the path for the RTX 4080 dev box. CPU remains a first-class fallback (and the CI test path, since runners have no GPU). The `cuda`/`metal` Candle backends are optional crate features, mirroring `vox-plugin-mens-candle-cuda`'s `device_select.rs`.

> Note: candle's k-quant `from_float` kernels run host-side; `quantize_onto` lands the result on the GPU. The dominant GPU win in this initiative is **SP-2 inference** (running the quantized forward on CUDA), not the one-time offline quantize. SP-1 exposes device selection so it is never CPU-locked.

---

## 2. Goals & non-goals

### In scope (SP-1)

1. A new **`vox-quantize`** crate (L2, library) that:
   - Reads f32/f16/bf16 weights from a SafeTensors model (single-file **and** sharded via `*.index.json`).
   - Quantizes per-tensor into Candle GGML k-quant types (`Q4_K`, `Q5_K`, `Q6_K`, `Q8_0`) under a configurable **tensor-type policy**.
   - Emits a quantized artifact on disk as **SafeTensors-canonical** (quantized blocks stored as `u8` tensors + a metadata map; see §6).
   - Runs a **round-trip self-check** (quantize → dequantize → per-tensor error) as a built-in quality gate.
   - Is **device-selectable** (`auto`/`cuda`/`metal`/`cpu`, default GPU-when-available); compiles & tests on CPU with no GPU feature, and accelerates on GPU when the `cuda`/`metal` feature is enabled.
2. Named k-quant **mixtures** mirroring llama.cpp conventions: `Q4_K_M`, `Q5_K_M`, `Q6_K`, `Q8_0`, plus a manual per-tensor-type override.
3. A clean public API for the three downstream surfaces (CLI, merge-quant, inference) to call.

### Out of scope (SP-1 — deferred to later sub-projects / phases)

- The standalone CLI surface (**SP-4**).
- Quantizing merged QLoRA adapters (**SP-3**).
- Wiring quantized inference `load`/`predict` (**SP-2**, includes resolving the L3→L4 model-forward layering).
- **Calibration-based PTQ** (GPTQ/AWQ). The engine reserves an extension seam (`QTensor::quantize_imatrix` exists in Candle 0.9.2) but SP-1 ships data-free only.
- GGUF on-disk output. (Candle-native canonical only, per user choice.)

---

## 3. Architecture

```
                 ┌────────────────────────────────────────────┐
                 │  vox-quantize  (L2 library, GPU-first/CPU)   │
                 │                                              │
  model dir ───► │  read::SafeTensorsSource  (single+sharded)   │
                 │        │ f32/f16/bf16 Tensor stream           │
                 │        ▼                                      │
                 │  policy::QuantPolicy  (mixture + 256-align)   │
                 │        │ per-tensor target GgmlDType / skip    │
                 │        ▼                                      │
                 │  engine::quantize_model                       │
                 │        │  QTensor::quantize(&t, dtype)         │
                 │        ▼                                      │
                 │  verify::round_trip  (MSE / max-abs gate)     │
                 │        │                                      │
                 │        ▼                                      │
                 │  write::QuantizedArtifact (SafeTensors+meta)  │
                 └────────────────────────────────────────────┘
                          ▲                ▲              ▲
                   SP-4 CLI         SP-3 merge-quant   SP-2 inference
```

### Dependencies

- `candle-core = "0.9"` (`default-features = false`) — `quantized` module is **not** GPU-gated; CPU k-quant kernels are pure Rust (rayon-parallel).
- `safetensors = "0.7"` — read source tensors, write quantized artifact.
- `memmap2 = "0.9"` — mmap shard files (follow `vox-plugin-mens-candle-cuda/src/merge.rs` pattern).
- `vox-hf-layout` (L1) — parse `config.json` for architecture + dims (used to validate/aid policy; **does not** read tensors itself).
- `serde` / `serde_json` — the metadata sidecar.
- `anyhow` / `thiserror`, `tracing`.

### Module layout

| Module | Responsibility |
|---|---|
| `read` | `SafeTensorsSource`: enumerate tensors from a model dir; handle single `model.safetensors` and sharded `model.safetensors.index.json`; yield `(name, shape, dtype, f32 view)`. |
| `policy` | `QuantPolicy` + `QuantMixture`; decide per-tensor target `GgmlDType` or `Skip(F32)`; enforce QK_K=256 alignment fallback. |
| `engine` | `quantize_model`: drive read → policy → `QTensor::quantize` → collect blocks; orchestration + progress events. |
| `verify` | `round_trip`: dequantize each `QTensor`, compute MSE + max-abs vs source; produce `QuantReport`. |
| `write` | `QuantizedArtifact`: serialize quantized `u8` block tensors + `quant-metadata.json` sidecar into SafeTensors-canonical form. |
| `lib` | Public façade: `quantize(input, mixture, out) -> QuantReport`, plus the types above. |

---

## 4. Quantization policy (tensor-type mixtures)

Built on the verified Qwen3.5 tensor inventory. The split is the foundation:

**QUANTIZABLE-2D** (weight matrices): `embed_tokens`, attention `q/k/v/o_proj`, linear-attn `in_proj_qkv/z/a/b`, `out_proj`, `conv1d`, MLP `gate/up/down_proj`, `lm_head`.

**KEEP-F32** (1-D scales / log-domain params — never quantize): all `*_layernorm.weight`, final `norm.weight`, linear-attn `norm.weight`, `dt_bias`, `A_log`, and any bias vectors. (`rotary_emb.inv_freq` is omitted in official HF checkpoints; if present, keep F32.)

### Named mixtures (presets)

| Mixture | attn/mlp matrices | embeddings / `lm_head` | norms / scales |
|---|---|---|---|
| `Q4_K_M` | `Q4_K` (down_proj & v_proj → `Q6_K`) | `Q6_K` | F32 |
| `Q5_K_M` | `Q5_K` (down_proj & v_proj → `Q6_K`) | `Q6_K` | F32 |
| `Q6_K` | `Q6_K` | `Q6_K` | F32 |
| `Q8_0` | `Q8_0` | `Q8_0` | F32 |

The `_M` "medium" mixtures bump `down_proj` and `v_proj` one level (llama.cpp convention — these layers are quality-sensitive). A `QuantMixture::Manual(HashMap<TensorRole, GgmlDType>)` variant allows full override.

### QK_K=256 alignment rule (verified constraint)

k-quant types require the quantized (last) dimension divisible by **256**. The policy enforces, per tensor:

1. If target is a k-quant and `last_dim % 256 == 0` → use it.
2. Else fall back to `Q8_0` (block size 32) if `last_dim % 32 == 0`.
3. Else **keep F32** and record a `policy_fallback` note in the report.

This guarantees the engine never panics on odd-shaped tensors (small Qwen3.5 variants, fused projections).

---

## 5. Public API (sketch)

```rust
pub struct QuantizeRequest {
    pub input_dir: PathBuf,        // dir containing config.json + *.safetensors
    pub output_dir: PathBuf,
    pub mixture: QuantMixture,     // Q4KM | Q5KM | Q6K | Q8_0 | Manual(..)
    pub verify: bool,              // run round-trip self-check (default true)
    pub device: DevicePref,        // Auto | Cuda(usize) | Metal | Cpu (default Auto)
}

pub enum DevicePref { Auto, Cuda(usize), Metal, Cpu }

pub enum QuantMixture { Q4KM, Q5KM, Q6K, Q8_0, Manual(BTreeMap<TensorRole, GgmlDType>) }

pub struct QuantReport {
    pub tensors: Vec<TensorQuantStat>,   // name, src_dtype, target_dtype, params, mse, max_abs, fallback
    pub total_src_bytes: u64,
    pub total_quant_bytes: u64,
    pub compression_ratio: f64,
    pub worst_mse: f64,
}

pub fn quantize(req: &QuantizeRequest) -> Result<QuantReport, QuantizeError>;
```

`QuantizeError` (thiserror): `ReadModel`, `UnsupportedDtype`, `ShardIndex`, `Quantize(candle)`, `Write`, `VerifyFailed { tensor, mse }`.

---

## 6. On-disk format (ADR-043)

**Decision:** quantized models are written as **SafeTensors-canonical** to honor charter §0.2.3.

- Each quantized tensor's GGML block bytes are stored as a **1-D `u8` SafeTensors tensor** under the original key.
- KEEP-F32 tensors are stored unchanged (their native dtype).
- A `quant-metadata.json` sidecar maps `tensor_name → { ggml_dtype, orig_shape, orig_dtype }`, plus the mixture name and `vox-quantize` version. Without it the `u8` blobs are uninterpretable — it is part of the artifact contract.
- A copy of the source `config.json` is written alongside so downstream loaders (SP-2) have architecture/dims.

This keeps GGUF off-disk while still producing portable, hash-addressable SafeTensors. **ADR-043** records this choice and its rationale (a new ADR file `adr-043-quantized-safetensors-ondisk-format.md`).

---

## 7. Error handling & quality gate

- **Round-trip verification** is on by default. After quantizing each 2-D tensor the engine dequantizes and computes MSE + max-abs error vs the f32 source.
- A configurable `max_mse` threshold (default loose, e.g. surfaced not enforced in SP-1) flags suspicious tensors in the report. Hard-fail (`VerifyFailed`) only when MSE is non-finite (NaN/Inf) — a sign of a real bug, not just lossy quant.
- All shape/dtype/shard-index problems are typed errors, never panics.

---

## 8. Testing strategy (TDD)

Tests pin `DevicePref::Cpu` for determinism and run in CI without a GPU; the GPU path (`Auto`/`Cuda`) is exercised manually on the RTX 4080:

1. **Unit — policy:** mixture resolution per `TensorRole`; QK_K=256 fallback ladder (256-divisible → k-quant; 32-divisible → Q8_0; else F32).
2. **Unit — round-trip:** quantize→dequantize a known tensor; assert MSE within the expected band for each GgmlDType (Q8_0 ≪ Q6_K < Q4_K).
3. **Integration — tiny model fixture:** a synthetic 2-layer Qwen3.5-shaped SafeTensors model (dims divisible by 256) + `config.json`. Quantize end-to-end; assert: norms/`A_log`/`dt_bias` untouched (F32), 2-D matrices quantized, `quant-metadata.json` round-trips, compression ratio in expected range.
4. **Integration — sharded read:** two-shard fixture + `index.json`; assert all tensors discovered and quantized.
5. **Property:** odd-shaped tensor (last dim not ÷256) → falls back, never panics.

Coverage floor: match repo norm (≥70%; effort-audit S1 precedent hit 92%).

---

## 9. Verified facts this design rests on

| Claim | Verdict | Evidence |
|---|---|---|
| Candle 0.9.2 `quantized` is CPU-only, no GPU gate | confirmed | `candle-core-0.9.2/src/lib.rs:77` (no `#[cfg]`); pure-Rust `k_quants.rs` |
| `QTensor::quantize(&Tensor, GgmlDType)` + GgmlDType `Q4K/Q5K/Q6K/Q8_0` | confirmed | `quantized/mod.rs:254-270`, `:482-500` |
| QK_K = 256 block size for k-quants | confirmed | `quantized/k_quants.rs` (QK_K=256) |
| `vox-hf-layout` is clean L1 (no candle/safetensors) and parses Qwen3.5 layout | confirmed | `vox-hf-layout/Cargo.toml`; `lib.rs:27-70` |
| Qwen3.5 tensor inventory; F32-keep set (norms, `A_log`, `dt_bias`, biases) | confirmed | `vox-plugin-mens-candle-cuda/src/model.rs:201-482`, `hf_keymap.rs:32-93` |
| `vox-hf-layout` does **not** read SafeTensors / shards → SP-1 owns shard reading | confirmed (adaptation) | `vox-hf-layout/Cargo.toml` (no safetensors dep) |
| New L2 crate mechanics; next ADR = 043 | confirmed | `layers.toml:71-124`; `adr-042-*` is current max |

---

## 10. Crate scaffolding mechanics (follow verbatim)

1. `crates/vox-quantize/` with `Cargo.toml` + `src/lib.rs` (auto-included by `members = ["crates/*"]`).
2. `Cargo.toml` (root) → add `vox-quantize = { path = "crates/vox-quantize" }` alphabetically in `[workspace.dependencies]`.
3. `docs/src/architecture/layers.toml` → `vox-quantize = { layer = 2, max_loc = 4_000 }`.
4. `docs/src/architecture/where-things-live.md` → L2 section row: `| `[vox-quantize](../../../crates/vox-quantize/)` | Data-free k-quant PTQ engine (SafeTensors → Candle GGML quantized SafeTensors). |`
5. `docs/src/architecture/adr-043-quantized-safetensors-ondisk-format.md` (with required frontmatter).
6. Run `cargo run -p vox-arch-check` — must pass layer/orphan/LoC checks.

---

## 11. Downstream roadmap (separate spec→plan cycles)

| # | Sub-project | Depends on | Note |
|---|---|---|---|
| **SP-4** | Standalone `vox quantize` CLI (`vox-ml-cli`) | SP-1 | Fast follow; surfaces `QuantReport`. |
| **SP-3** | Quantize merged QLoRA output | SP-1 | ⚠️ `merge-qlora` emits a **subset**; must recombine with base before quantizing. |
| **SP-2** | Quantized inference `load`/`predict` (`vox-inference`) | SP-1 | ⚠️ L3→L4 layering: Qwen3.5 forward lives in the L4 plugin; must be shared into an L2/L3 home first. Heaviest; sequenced last. |

Each gets its own `docs/superpowers/specs/` design + `docs/superpowers/plans/` plan.
