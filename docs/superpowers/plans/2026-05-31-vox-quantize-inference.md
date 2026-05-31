# Quantized Inference Wiring (SP-2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `vox-inference` actually load an SP-1 quantized Qwen3.5 artifact and generate tokens — **CUDA-first** on the RTX 4080, with CPU as the CI/fallback path — without violating the L3→L4 layering rule.

**Architecture:** Two slices. **SP-2a** extracts the Qwen3.5/2.5 forward pass out of the L4 plugin into a new L2 crate `vox-model-qwen` built on `QMatMul` (accepts quantized or f32 weights). **SP-2b** wires `vox-inference` backends to load SP-1 artifacts into `vox-model-qwen` and run a sampling loop.

**Tech Stack:** Rust, `candle-core`/`candle-nn` (`quantized::QMatMul`), `vox-quantize` (SP-1), `vox-hf-layout`, `tokenizers`.

**Spec:** `docs/superpowers/specs/2026-05-31-vox-quantize-inference-design.md`
**Depends on:** SP-1 (built, API stable). Heaviest sub-project — build SP-4 and SP-3 first.

> **Why some tasks reference source to be read at execution time:** the canonical Qwen3.5 forward lives in `vox-plugin-mens-candle-cuda/src/model.rs` (≈480 lines, hybrid full/linear attention). SP-2a *extracts and adapts* that existing, working code rather than reinventing it. The plan gives the exact extraction recipe, interfaces, and parity tests; the forward-pass body is moved (not hand-rewritten) so it cannot be inlined verbatim here without first reading the live file. Read `model.rs` fully as Task 0.

---

### Task 0: Read the existing model + confirm the extraction surface

**Files:** Read-only: `crates/vox-plugin-mens-candle-cuda/src/model.rs`, `inference.rs`, `hf_keymap.rs`.

- [ ] **Step 1: Map the forward pass**

Read `model.rs` end to end. Write a short note (`docs/superpowers/specs/2026-05-31-vox-quantize-inference-design.md`, append a "§9 extraction map" section) listing: the public types (`Qwen35Model`, `Qwen2Attention`, `Qwen35LinearAttention`, `Qwen2MLP`, `Qwen35Layer`), which use `QuantizedLinear` vs raw `Tensor`, what state the forward needs (KV cache?), and which parts are training-only (autograd, VarMap) vs inference-pure.

- [ ] **Step 2: Decide the split**

Confirm Option A from the spec: a new L2 `vox-model-qwen` holding the inference-pure forward (built on `QMatMul`), with the plugin refactored to depend on it for structure. Record any blockers (e.g. training graph tightly coupled to forward) in the note. If coupling is severe, fall back to spec Option B (forward duplicated in vox-inference) and document why.

- [ ] **Step 3: Commit the note**

```bash
git add docs/superpowers/specs/2026-05-31-vox-quantize-inference-design.md
git commit -m "docs(quantize): SP-2 extraction map for Qwen3.5 forward pass"
```

---

### Task 1: Scaffold `vox-model-qwen` (L2)

**Files:**
- Create: `crates/vox-model-qwen/Cargo.toml`, `crates/vox-model-qwen/src/lib.rs`
- Modify: root `Cargo.toml`, `docs/src/architecture/layers.toml`, `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Cargo.toml**

```toml
[package]
name = "vox-model-qwen"
version = "0.1.0"
edition = "2021"
description = "Inference-pure Qwen3.5/2.5 forward pass on candle QMatMul (quantized or f32 weights)."

[dependencies]
candle-core = { workspace = true }
candle-nn = { workspace = true }
vox-hf-layout = { workspace = true }
vox-quantize = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Register in arch docs**

`layers.toml`:
```toml
vox-model-qwen          = { layer = 2, max_loc = 6_000 }
```
Root `Cargo.toml` `[workspace.dependencies]`: `vox-model-qwen = { path = "crates/vox-model-qwen" }` (alphabetical).
`where-things-live.md` L2 row:
```markdown
| [`vox-model-qwen`](../../../crates/vox-model-qwen/) | Inference-pure Qwen3.5/2.5 forward pass (QMatMul; quantized or f32). |
```

- [ ] **Step 3: Verify + commit**

Run: `cargo build -p vox-model-qwen && cargo run -p vox-arch-check`
Expected: builds, arch-check PASS.
```bash
git add crates/vox-model-qwen Cargo.toml docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "feat(model-qwen): scaffold L2 inference model crate"
```

---

### Task 2: Artifact loader — reconstruct QMatMul weights from SP-1 output

**Files:**
- Create: `crates/vox-model-qwen/src/load.rs`
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};

    #[test]
    fn loads_quantized_weight_as_qmatmul() {
        // Produce an SP-1 artifact for a single matrix, then load it back.
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        let dev = Device::Cpu;
        let mut m = std::collections::HashMap::new();
        m.insert("model.language_model.layers.0.mlp.gate_proj.weight".to_string(),
            Tensor::randn(0f32, 1f32, (256, 256), &dev).unwrap());
        candle_core::safetensors::save(&m, indir.path().join("model.safetensors")).unwrap();
        std::fs::write(indir.path().join("config.json"),
            r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();
        vox_quantize::quantize(&vox_quantize::QuantizeRequest{
            input_dir: indir.path().to_path_buf(), output_dir: outdir.path().to_path_buf(),
            mixture: vox_quantize::QuantMixture::Q4KM, verify: false,
            device: vox_quantize::DevicePref::Cpu,
        }).unwrap();

        let weights = QuantizedWeights::load(outdir.path(), &dev).unwrap();
        let qmm = weights.qmatmul("model.language_model.layers.0.mlp.gate_proj.weight").unwrap();
        // a (1,256) input forwards to (1,256)
        let x = Tensor::zeros((1, 256), candle_core::DType::F32, &dev).unwrap();
        let y = candle_core::Module::forward(qmm, &x).unwrap();
        assert_eq!(y.dims(), &[1, 256]);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-model-qwen load::`
Expected: FAIL (compile error — `QuantizedWeights` undefined).

- [ ] **Step 3: Implement**

```rust
use anyhow::{anyhow, Result};
use candle_core::quantized::{GgmlDType, QStorage, QTensor, QMatMul};
use candle_core::{Device, Tensor};
use std::collections::HashMap;
use std::path::Path;

#[derive(serde::Deserialize)]
struct TensorMeta { ggml_dtype: String, orig_shape: Vec<usize>, quantized: bool }
#[derive(serde::Deserialize)]
struct QuantMetadata { tensors: HashMap<String, TensorMeta> }

fn parse_ggml(s: &str) -> Result<GgmlDType> {
    Ok(match s {
        "Q4K" => GgmlDType::Q4K, "Q5K" => GgmlDType::Q5K, "Q6K" => GgmlDType::Q6K,
        "Q8_0" => GgmlDType::Q8_0, "F32" => GgmlDType::F32,
        other => return Err(anyhow!("unsupported ggml dtype `{other}`")),
    })
}

pub struct QuantizedWeights {
    qmm: HashMap<String, QMatMul>,
    f32: HashMap<String, Tensor>,
}

impl QuantizedWeights {
    pub fn load(artifact_dir: &Path, dev: &Device) -> Result<Self> {
        let meta_raw = std::fs::read_to_string(artifact_dir.join("quant-metadata.json"))?;
        let meta: QuantMetadata = serde_json::from_str(&meta_raw)?;
        let st = candle_core::safetensors::load(artifact_dir.join("model.safetensors"), dev)?;

        let mut qmm = HashMap::new();
        let mut f32 = HashMap::new();
        for (name, tm) in &meta.tensors {
            let raw = st.get(name).ok_or_else(|| anyhow!("tensor `{name}` missing"))?;
            if !tm.quantized {
                f32.insert(name.clone(), raw.to_dtype(candle_core::DType::F32)?);
                continue;
            }
            let dtype = parse_ggml(&tm.ggml_dtype)?;
            let bytes = raw.to_dtype(candle_core::DType::U8)?.flatten_all()?.to_vec1::<u8>()?;
            // Reconstruct a QTensor from raw block bytes + original 2-D shape.
            let qstorage = QStorage::from_raw_bytes(dev, &bytes, dtype)?; // see note
            let qt = QTensor::new(qstorage, (tm.orig_shape[0], tm.orig_shape[1]))?;
            qmm.insert(name.clone(), QMatMul::from_qtensor(qt)?);
        }
        Ok(Self { qmm, f32 })
    }

    pub fn qmatmul(&self, name: &str) -> Option<&QMatMul> { self.qmm.get(name) }
    pub fn tensor(&self, name: &str) -> Option<&Tensor> { self.f32.get(name) }
}
```

> **Spike inside this task:** the exact constructor for a `QTensor` from raw bytes (`QStorage::from_raw_bytes` above is illustrative) must be confirmed against candle 0.9.2 — it is the read-side counterpart of SP-1 Task 0's `QTensor::data()`. Candle's GGUF *reader* (`quantized::gguf_file`) does exactly this; copy its reconstruction path. Confirm and adjust before Step 4.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-model-qwen load::loads_quantized_weight_as_qmatmul`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-model-qwen/src/load.rs crates/vox-model-qwen/src/lib.rs
git commit -m "feat(model-qwen): load SP-1 quantized artifact into QMatMul weights"
```

---

### Task 3: Extract the forward pass into `vox-model-qwen`

**Files:**
- Create: `crates/vox-model-qwen/src/model.rs` (moved/adapted from plugin)
- Modify: `crates/vox-plugin-mens-candle-cuda/src/model.rs` (re-export from `vox-model-qwen` for the base structure; keep training graph local)
- Test: `crates/vox-model-qwen/tests/forward_parity.rs`

- [ ] **Step 1: Move the inference-pure forward**

Per the Task 0 extraction map, move `Qwen2Attention`, `Qwen35LinearAttention`, `Qwen2MLP`, `Qwen35Layer`, and the inference path of `Qwen35Model::forward` into `vox-model-qwen/src/model.rs`, parameterizing weight access over `QuantizedWeights` (from Task 2) instead of the plugin's training `VarMap`. Construct each linear as a `QMatMul` (quantized) or `QMatMul::Tensor` (f32) so the same forward serves both.

- [ ] **Step 2: Write the parity test**

```rust
// Build a tiny quantized model, run vox-model-qwen forward, assert finite logits
// of shape [seq, vocab]. (Full numeric parity vs the plugin's f32 forward is a
// follow-up; this gates shape + finiteness + no-panic on the hybrid attention path.)
#[test]
fn forward_produces_finite_logits() {
    // ... build tiny artifact as in load.rs test, load QuantizedWeights, build model,
    // forward a [1,4] token tensor, assert logits dims == [4, vocab] and all finite.
}
```

Fill the test body using the same tiny-artifact construction as Task 2's test, then:
```rust
    let weights = vox_model_qwen::load::QuantizedWeights::load(art.path(), &dev).unwrap();
    let cfg = vox_hf_layout::HfTransformerLayout::from_config_path(&art.path().join("config.json")).unwrap();
    let model = vox_model_qwen::model::QwenForward::new(&cfg, weights, &dev).unwrap();
    let tokens = candle_core::Tensor::new(&[[1u32, 2, 3, 4]], &dev).unwrap();
    let logits = model.forward(&tokens, 0).unwrap();
    assert_eq!(logits.dims().last().copied(), Some(cfg.vocab_size));
    assert!(logits.flatten_all().unwrap().to_vec1::<f32>().unwrap().iter().all(|v| v.is_finite()));
```

- [ ] **Step 3: Refactor the plugin to depend on `vox-model-qwen`**

In `vox-plugin-mens-candle-cuda`, replace the moved type definitions with `pub use vox_model_qwen::model::{...}` for the base structure, keeping only training-specific wrappers local. Add `vox-model-qwen = { workspace = true }` to the plugin's Cargo.toml.

- [ ] **Step 4: Verify nothing regressed**

Run: `cargo build -p vox-plugin-mens-candle-cuda`
Expected: builds.
Run: `cargo test -p vox-model-qwen`
Expected: PASS.
Run: `cargo run -p vox-arch-check`
Expected: PASS (plugin→L2 is downward; no inversion).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-model-qwen/src/model.rs crates/vox-plugin-mens-candle-cuda/src/model.rs crates/vox-plugin-mens-candle-cuda/Cargo.toml
git commit -m "refactor(model-qwen): extract inference-pure Qwen forward from plugin (L2)"
```

---

### Task 4: Wire `vox-inference` CandleCpu backend

**Files:**
- Modify: `crates/vox-inference/Cargo.toml`, `crates/vox-inference/src/backends/candle_cpu.rs`, `crates/vox-inference/src/backend.rs`
- Test: `crates/vox-inference/tests/cpu_predict.rs`

- [ ] **Step 1: Add deps + extend the Quantization enum**

`vox-inference/Cargo.toml`: add `vox-model-qwen = { workspace = true }`, `vox-quantize = { workspace = true }`, `vox-hf-layout = { workspace = true }`, `tokenizers = { workspace = true }`, `candle-core = { workspace = true }`.
In `backend.rs:8-15`, extend the enum to cover SP-1's mixtures:
```rust
pub enum Quantization { Fp16, Bf16, Q8Zero, Q4K, Q5K, Q6K }
```

- [ ] **Step 2: Write the failing test**

```rust
#[test]
fn cpu_backend_predicts_non_stub() {
    // Build a tiny quantized artifact (as in earlier tests) and a minimal tokenizer.json,
    // package a ModelBundle pointing at the artifact dir, then:
    let backend = vox_inference::backends::candle_cpu::CandleCpuBackend::default();
    let loaded = backend.load(&bundle).unwrap();
    let out = futures::executor::block_on(
        backend.predict(&loaded, prompt, sampling)).unwrap();
    assert!(!out.contains("stub"));
    assert!(!out.is_empty());
}
```

> Fill `bundle`, `prompt`, `sampling` per the `ModelBundle`/`PromptInput`/`SamplingParams` shapes in `backend.rs`. Use a 2-3 token greedy generation (`SamplingParams` with temperature 0) for determinism.

- [ ] **Step 3: Implement `load` + `predict`**

```rust
// load: read bundle.path -> QuantizedWeights::load -> build QwenForward + load tokenizer.json
// predict: tokenize prompt -> loop { forward -> argmax (greedy) or sample -> append } for max_new_tokens -> detokenize
```
Replace the `"[candle-cpu stub]"` return with a real greedy loop calling `QwenForward::forward`, using `tokenizers::Tokenizer` for encode/decode and `SamplingParams` for `max_new_tokens`/temperature. Keep streaming off (capability already false).

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-inference --test cpu_predict`
Expected: PASS
Run: `cargo run -p vox-arch-check`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-inference
git commit -m "feat(inference): real CandleCpu quantized load + greedy predict"
```

---

### Task 5: CandleCuda / CandleMetal backends + capability advertisement

**Files:**
- Modify: `crates/vox-inference/src/backends/candle_cuda.rs`, `candle_metal.rs`, `src/backend.rs`

- [ ] **Step 1: Generalize the load/predict over device**

Factor the Task 4 CPU `load`/`predict` into a shared helper parameterized by `candle_core::Device`, mirroring the plugin's `device_select.rs` fallback (CUDA/Metal feature off → CPU + warning). CandleCuda/CandleMetal call the helper with their device; behind disabled features they fall back to CPU.

- [ ] **Step 2: Advertise real quantization sets**

In each backend's `capabilities()`, set `quantizations` to the dtypes actually supported (`[Q4K, Q5K, Q6K, Q8Zero]`) and read `vram_gb` from the existing hardware probe rather than the hardcoded `0`.

- [ ] **Step 3: Test (feature-gated)**

Add a CPU-fallback test asserting CandleCuda/CandleMetal `predict` returns non-stub output when their GPU feature is disabled (CI has no GPU). GPU-on tests are `#[ignore]` for manual runs on the RTX 4080.

- [ ] **Step 4: Run + commit**

Run: `cargo test -p vox-inference`
Expected: PASS (GPU tests ignored).
```bash
git add crates/vox-inference
git commit -m "feat(inference): CandleCuda/Metal quantized backends + capability advertisement"
```

---

## Self-Review

- **Spec coverage:** SP-2a extraction ✓ T0/T1/T3 (resolves L3→L4 layering); artifact load ✓ T2; CPU predict ✓ T4; CUDA/Metal + capabilities ✓ T5; Quantization enum extended ✓ T4.
- **Placeholder scan:** Tasks 3/4/5 deliberately move/adapt existing plugin code and reference live types (`ModelBundle`, `SamplingParams`, the forward body) that must be read at execution time — flagged explicitly in the task preamble and Task 0. Two spikes (raw-bytes read in T2; device helper in T5) are gated, real-API confirmations, not vague TODOs. This is the irreducible uncertainty of wiring a real forward pass; it is isolated and called out, not hidden.
- **Type consistency:** `QuantizedWeights`/`QwenForward`/`Quantization` consistent across T2–T5; `QuantizeRequest`/`QuantMixture` match SP-1.
- **Note:** SP-2 is the largest and least mechanical plan; recommend executing T0 first and re-confirming T3/T4 code against the live `model.rs` before implementing, per subagent-driven-development's review checkpoints.
