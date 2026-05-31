# SP-2 — Quantized Inference Wiring Design

**Date:** 2026-05-31
**Status:** Approved (decomposition); spec under review
**Depends on:** SP-1 (`vox-quantize` engine) + resolution of the L3→L4 model-forward layering
**Scope:** Fill the hollow `vox-inference` backends so they actually load a quantized Qwen3.5 artifact and generate tokens. Heaviest sub-project; sequenced last.

---

## 1. Motivation

`vox-inference` advertises `Quantization { Fp16, Bf16, Q8Zero, Q4K }` and a full `InferenceBackend` trait, but every backend's `load`/`predict` is a stub returning `"[…stub]"`. SP-2 makes quantized inference real on top of the SP-1 artifact format.

## 2. The blocking architectural problem (verified)

The working Qwen3.5 forward pass — `Qwen35Model` — lives in **`vox-plugin-mens-candle-cuda` (L4 plugin)** and uses `QuantizedLinear` modules. `vox-inference` is **L3** and **cannot** take a compile-time dependency on an L4 plugin (`layers.toml` lower→higher inversion rule; confirmed no `[[known_inversions]]` entry, and `vox-inference/Cargo.toml` has no plugin dep).

**This must be resolved before any predict logic.** Options:

| Option | Description | Trade-off |
|---|---|---|
| **A (recommended)** | Extract a **shared inference model** into a new L2 crate `vox-model-qwen` (forward pass + QMatMul-based layers, no training). Both the plugin and `vox-inference` depend on it. | Clean layering; one canonical forward; some extraction effort. The plugin keeps its *training* graph, depends on `vox-model-qwen` for the base structure. |
| **B** | Put a quantized-only forward directly inside `vox-inference`. | No new crate, but duplicates model code already in the plugin; drift risk. |
| **C** | Load the plugin at runtime via `vox-plugin-host` (libloading/abi_stable) and call inference through the plugin ABI. | Matches plugin-host design, but inference-through-cdylib is heavier and the plugin is training-oriented. |

**Default: Option A.** A dedicated spec section in the SP-2 plan will detail the extraction. This is itself a meaningful slice — the plan may split SP-2 into **SP-2a (extract `vox-model-qwen`)** and **SP-2b (wire backends)**.

## 3. Goals / non-goals

**In scope:**
- Load an SP-1 quantized artifact (SafeTensors `u8` blocks + `quant-metadata.json` + `config.json`) into `QTensor`/`QMatMul` weights.
- Real `predict`: tokenize (existing `tokenizers` dep) → forward (quantized) → sample → detokenize, for `CandleCpu` first (CI-testable), then `CandleCuda`/`CandleMetal`.
- Map SP-1 GgmlDTypes to the `Quantization` enum capability advertisement (extend enum if needed: add `Q5K`, `Q6K`).

**Out of scope:**
- Streaming token output (capability already flagged false for Candle backends; later).
- `LlamaCppRpc` / `Ollama` backends (external, separate).
- Batch/server serving.

## 4. Architecture

1. **SP-2a:** `vox-model-qwen` (L2) — Qwen3.5/2.5 forward pass built on `QMatMul` (accepts quantized or f32 weights), RoPE, hybrid full/linear attention. Plugin refactored to consume it for the base structure.
2. **SP-2b:** in `vox-inference`:
   - `load`: read SP-1 artifact via `vox-quantize` reader + `quant-metadata.json`, build `vox-model-qwen` weights as `QMatMul::from_qtensor`.
   - `predict`: greedy + temperature/top-p sampling loop using `SamplingParams`.
   - `capabilities`: advertise the artifact's actual quantization set.

## 5. Error handling

- Artifact/metadata mismatch, missing `config.json`, unsupported arch → typed `InferenceError::Load`.
- Device unavailable (CUDA/Metal feature off) → fall back to CPU with a warning, mirroring `device_select.rs`.

## 6. Testing

- SP-2a: forward-pass parity test — `vox-model-qwen` vs the plugin's existing forward on a tiny fixture (same logits within tolerance).
- SP-2b CPU: load SP-1 tiny quantized fixture, run `predict` on a fixed prompt+seed, assert deterministic non-stub output and shape.
- `vox-arch-check` must pass (no L3→L4 inversion introduced).

## 7. Verified facts

- `InferenceBackend` trait signatures + `Quantization` enum: `vox-inference/src/backend.rs:8-15,76-94`.
- All 5 backends are stubs returning `"[…stub]"`: `backends/*.rs:50`.
- `Qwen35Model` defined in L4 plugin `model.rs:1-24,201-482`; **not** referenced by `vox-inference`; no plugin dep in its `Cargo.toml`.
- Layer inversion rule + L4 "never compile-time deps for L0..L3": `layers.toml:21-22,183-196`.

## 8. Why last

Largest surface, depends on SP-1 *and* a non-trivial extraction (SP-2a), and is the only sub-project that touches the layering rules. Building SP-4/SP-3 first proves the engine and de-risks the artifact format before the heavy inference work.
