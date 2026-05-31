# vox-quantize Engine (SP-1) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `vox-quantize`, a CPU-only L2 crate that turns a full SafeTensors model (Qwen3.5/2.5) into a smaller quantized SafeTensors-canonical artifact using Candle's data-free k-quants.

**Architecture:** Read f32/f16 tensors (single + sharded SafeTensors) → resolve a per-tensor target `GgmlDType` via a mixture policy (with QK_K=256 alignment fallback) → `QTensor::quantize` → round-trip verify → write quantized `u8` block tensors + `quant-metadata.json` sidecar. No GPU feature required.

**Tech Stack:** Rust, `candle-core 0.9` (`quantized` module, CPU), `safetensors 0.7`, `serde`/`serde_json`, `anyhow`/`thiserror`, `tracing`.

**Spec:** `docs/superpowers/specs/2026-05-31-vox-quantize-engine-design.md`

---

### Task 0: Spike — confirm candle quantized-bytes accessor

**Files:**
- Scratch: a throwaway test in any existing crate, or `crates/vox-quantize/src/spike.rs` (deleted after).

- [ ] **Step 1: Confirm the raw-bytes path**

In the candle 0.9.2 source at `~/.cargo/registry/src/*/candle-core-0.9.2/src/quantized/`, confirm the method used by the GGUF writer to obtain a quantized tensor's raw block bytes (look in `gguf_file.rs` / `mod.rs` for how `QTensor` is serialized — likely `QTensor::data() -> Result<Cow<[u8]>>` or via `QStorage`). Note the exact signature and whether it works for a CPU `QStorage`.

- [ ] **Step 2: Record the finding**

Write the confirmed accessor signature into the SP-1 spec §6 as a one-line note (e.g. `// quantized bytes: QTensor::data()? -> Cow<'_,[u8]>`). This unblocks Task 6 (`write`). Do not proceed to Task 6's serialization until confirmed.

- [ ] **Step 3: Commit the note**

```bash
git add docs/superpowers/specs/2026-05-31-vox-quantize-engine-design.md
git commit -m "docs(quantize): confirm candle QTensor raw-bytes accessor for SP-1 write"
```

---

### Task 1: Scaffold the crate

**Files:**
- Create: `crates/vox-quantize/Cargo.toml`
- Create: `crates/vox-quantize/src/lib.rs`
- Modify: `Cargo.toml` (workspace root, `[workspace.dependencies]`)
- Modify: `docs/src/architecture/layers.toml`
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 1: Create `crates/vox-quantize/Cargo.toml`**

```toml
[package]
name = "vox-quantize"
version = "0.1.0"
edition = "2021"
description = "Data-free k-quant PTQ engine: SafeTensors -> Candle GGML quantized SafeTensors (CPU-only)."

[dependencies]
candle-core = { workspace = true }
safetensors = { workspace = true }
vox-hf-layout = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
```

- [ ] **Step 2: Create `crates/vox-quantize/src/lib.rs` (skeleton)**

```rust
//! Data-free k-quant post-training quantization engine.
//!
//! SafeTensors model in -> quantized SafeTensors-canonical artifact out. CPU-only.
pub mod error;
pub mod policy;
pub mod read;
pub mod engine;
pub mod verify;
pub mod write;

pub use engine::quantize;
pub use error::QuantizeError;
pub use policy::{QuantMixture, TensorRole};
pub use verify::{QuantReport, TensorQuantStat};
```

- [ ] **Step 3: Register in workspace + arch docs**

In root `Cargo.toml` `[workspace.dependencies]`, add alphabetically:
```toml
vox-quantize              = { path = "crates/vox-quantize" }
```
In `docs/src/architecture/layers.toml`, add to the L2 block:
```toml
vox-quantize            = { layer = 2, max_loc = 4_000 }
```
In `docs/src/architecture/where-things-live.md`, add to the L2 section table:
```markdown
| [`vox-quantize`](../../../crates/vox-quantize/) | Data-free k-quant PTQ engine (SafeTensors → Candle GGML quantized SafeTensors, CPU-only). |
```

- [ ] **Step 4: Verify it compiles and arch-check passes**

Run: `cargo build -p vox-quantize`
Expected: builds (empty modules will fail — create empty `error.rs`, `policy.rs`, `read.rs`, `engine.rs`, `verify.rs`, `write.rs` with `// placeholder` to compile, replaced in later tasks).
Run: `cargo run -p vox-arch-check`
Expected: PASS (vox-quantize recognized as L2, no orphan/LoC violation).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize Cargo.toml docs/src/architecture/layers.toml docs/src/architecture/where-things-live.md
git commit -m "feat(quantize): scaffold vox-quantize L2 crate"
```

---

### Task 2: Error type

**Files:**
- Create: `crates/vox-quantize/src/error.rs`
- Test: inline `#[cfg(test)]` in `error.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_is_human_readable() {
        let e = QuantizeError::ShardIndex("model.safetensors.index.json missing".into());
        assert!(format!("{e}").contains("shard index"));
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize error::tests::display_is_human_readable`
Expected: FAIL (compile error — `QuantizeError` undefined).

- [ ] **Step 3: Implement**

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum QuantizeError {
    #[error("failed to read model: {0}")]
    ReadModel(String),
    #[error("unsupported source dtype for tensor `{0}`")]
    UnsupportedDtype(String),
    #[error("shard index error: {0}")]
    ShardIndex(String),
    #[error("candle quantize error: {0}")]
    Quantize(#[from] candle_core::Error),
    #[error("write error: {0}")]
    Write(String),
    #[error("verification failed for tensor `{tensor}`: non-finite error (mse={mse})")]
    VerifyFailed { tensor: String, mse: f64 },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize error::tests::display_is_human_readable`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize/src/error.rs
git commit -m "feat(quantize): typed QuantizeError"
```

---

### Task 3: Policy — roles, mixtures, and the QK_K=256 fallback

**Files:**
- Create: `crates/vox-quantize/src/policy.rs`
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::quantized::GgmlDType;

    #[test]
    fn role_classification_keeps_norms_f32() {
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.input_layernorm.weight"), TensorRole::KeepF32);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.linear_attn.A_log"), TensorRole::KeepF32);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.linear_attn.dt_bias"), TensorRole::KeepF32);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.mlp.down_proj.weight"), TensorRole::DownProj);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.self_attn.v_proj.weight"), TensorRole::VProj);
        assert_eq!(TensorRole::from_key("lm.head.weight"), TensorRole::Output);
        assert_eq!(TensorRole::from_key("model.language_model.embed_tokens.weight"), TensorRole::Embedding);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.mlp.gate_proj.weight"), TensorRole::Matrix);
    }

    #[test]
    fn q4km_bumps_downproj_and_vproj_to_q6k() {
        let m = QuantMixture::Q4KM;
        assert_eq!(m.target_for(TensorRole::Matrix), Some(GgmlDType::Q4K));
        assert_eq!(m.target_for(TensorRole::DownProj), Some(GgmlDType::Q6K));
        assert_eq!(m.target_for(TensorRole::VProj), Some(GgmlDType::Q6K));
        assert_eq!(m.target_for(TensorRole::Embedding), Some(GgmlDType::Q6K));
        assert_eq!(m.target_for(TensorRole::KeepF32), None);
    }

    #[test]
    fn alignment_falls_back_below_256() {
        // 256-divisible -> keep k-quant
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 512), GgmlDType::Q4K);
        // not 256 but 32-divisible -> Q8_0
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 96), GgmlDType::Q8_0);
        // neither -> F32
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 100), GgmlDType::F32);
        // Q8_0 target with 32-divisible stays Q8_0
        assert_eq!(resolve_dtype(GgmlDType::Q8_0, 64), GgmlDType::Q8_0);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize policy::`
Expected: FAIL (compile error — `TensorRole`, `QuantMixture`, `resolve_dtype` undefined).

- [ ] **Step 3: Implement**

```rust
use candle_core::quantized::GgmlDType;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TensorRole {
    Embedding,
    Output,     // lm_head
    DownProj,
    VProj,
    Matrix,     // all other quantizable 2-D weights
    KeepF32,    // norms, biases, A_log, dt_bias, 1-D scales
}

impl TensorRole {
    pub fn from_key(key: &str) -> Self {
        let k = key.to_ascii_lowercase();
        // 1-D / log-domain params always kept
        if k.ends_with("layernorm.weight")
            || k.ends_with(".norm.weight")
            || k == "model.language_model.norm.weight"
            || k.ends_with(".a_log")
            || k.ends_with(".dt_bias")
            || k.ends_with(".bias")
            || k.contains("inv_freq")
        {
            return TensorRole::KeepF32;
        }
        if k.contains("embed_tokens") { return TensorRole::Embedding; }
        if k.contains("lm.head") || k.contains("lm_head") { return TensorRole::Output; }
        if k.ends_with("down_proj.weight") { return TensorRole::DownProj; }
        if k.ends_with("v_proj.weight") { return TensorRole::VProj; }
        TensorRole::Matrix
    }
}

#[derive(Debug, Clone)]
pub enum QuantMixture {
    Q4KM,
    Q5KM,
    Q6K,
    Q8_0,
    Manual(BTreeMap<TensorRole, GgmlDType>),
}

impl QuantMixture {
    /// Desired dtype for a role before alignment is considered. None = keep F32.
    pub fn target_for(&self, role: TensorRole) -> Option<GgmlDType> {
        if role == TensorRole::KeepF32 { return None; }
        match self {
            QuantMixture::Q4KM => Some(match role {
                TensorRole::DownProj | TensorRole::VProj | TensorRole::Embedding | TensorRole::Output => GgmlDType::Q6K,
                _ => GgmlDType::Q4K,
            }),
            QuantMixture::Q5KM => Some(match role {
                TensorRole::DownProj | TensorRole::VProj | TensorRole::Embedding | TensorRole::Output => GgmlDType::Q6K,
                _ => GgmlDType::Q5K,
            }),
            QuantMixture::Q6K => Some(GgmlDType::Q6K),
            QuantMixture::Q8_0 => Some(GgmlDType::Q8_0),
            QuantMixture::Manual(m) => m.get(&role).copied(),
        }
    }
}

/// Enforce GGML block-size alignment against the tensor's last dimension.
/// k-quants need last_dim % 256 == 0; Q8_0/legacy need % 32; else keep F32.
pub fn resolve_dtype(target: GgmlDType, last_dim: usize) -> GgmlDType {
    let is_kquant = matches!(
        target,
        GgmlDType::Q2K | GgmlDType::Q3K | GgmlDType::Q4K | GgmlDType::Q5K | GgmlDType::Q6K | GgmlDType::Q8K
    );
    if is_kquant {
        if last_dim % 256 == 0 { return target; }
        if last_dim % 32 == 0 { return GgmlDType::Q8_0; }
        return GgmlDType::F32;
    }
    // legacy / Q8_0 targets
    if last_dim % 32 == 0 { target } else { GgmlDType::F32 }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize policy::`
Expected: PASS (3 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize/src/policy.rs
git commit -m "feat(quantize): tensor-role classification + mixtures + QK_K alignment fallback"
```

---

### Task 4: Read — single + sharded SafeTensors source

**Files:**
- Create: `crates/vox-quantize/src/read.rs`
- Test: inline `#[cfg(test)]` (uses a fixture builder helper)

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use std::collections::HashMap;

    fn write_st(dir: &std::path::Path, name: &str, tensors: &[(&str, Tensor)]) {
        let map: HashMap<String, Tensor> = tensors.iter().map(|(k, t)| (k.to_string(), t.clone())).collect();
        candle_core::safetensors::save(&map, dir.join(name)).unwrap();
    }

    #[test]
    fn reads_single_file_model() {
        let dir = tempfile::tempdir().unwrap();
        let t = Tensor::zeros((4, 256), candle_core::DType::F32, &Device::Cpu).unwrap();
        write_st(dir.path(), "model.safetensors", &[("w", t)]);
        let src = SafeTensorsSource::open(dir.path()).unwrap();
        let names: Vec<_> = src.tensor_names().to_vec();
        assert_eq!(names, vec!["w".to_string()]);
        let loaded = src.load_f32("w").unwrap();
        assert_eq!(loaded.dims(), &[4, 256]);
    }

    #[test]
    fn reads_sharded_model_via_index() {
        let dir = tempfile::tempdir().unwrap();
        let a = Tensor::zeros((2, 256), candle_core::DType::F32, &Device::Cpu).unwrap();
        let b = Tensor::zeros((2, 256), candle_core::DType::F32, &Device::Cpu).unwrap();
        write_st(dir.path(), "model-00001-of-00002.safetensors", &[("a", a)]);
        write_st(dir.path(), "model-00002-of-00002.safetensors", &[("b", b)]);
        std::fs::write(dir.path().join("model.safetensors.index.json"),
            r#"{"weight_map":{"a":"model-00001-of-00002.safetensors","b":"model-00002-of-00002.safetensors"}}"#).unwrap();
        let src = SafeTensorsSource::open(dir.path()).unwrap();
        let mut names = src.tensor_names().to_vec();
        names.sort();
        assert_eq!(names, vec!["a".to_string(), "b".to_string()]);
        assert_eq!(src.load_f32("b").unwrap().dims(), &[2, 256]);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize read::`
Expected: FAIL (compile error — `SafeTensorsSource` undefined).

- [ ] **Step 3: Implement**

```rust
use crate::error::QuantizeError;
use candle_core::{Device, Tensor};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A SafeTensors model source: single `model.safetensors` or sharded via
/// `model.safetensors.index.json` (HF `weight_map`).
pub struct SafeTensorsSource {
    /// tensor name -> shard file path
    map: HashMap<String, PathBuf>,
    names: Vec<String>,
}

#[derive(serde::Deserialize)]
struct ShardIndex {
    weight_map: HashMap<String, String>,
}

impl SafeTensorsSource {
    pub fn open(dir: &Path) -> Result<Self, QuantizeError> {
        let index = dir.join("model.safetensors.index.json");
        let single = dir.join("model.safetensors");
        let mut map = HashMap::new();
        if index.exists() {
            let raw = std::fs::read_to_string(&index)?;
            let idx: ShardIndex = serde_json::from_str(&raw)
                .map_err(|e| QuantizeError::ShardIndex(e.to_string()))?;
            for (name, file) in idx.weight_map {
                map.insert(name, dir.join(file));
            }
        } else if single.exists() {
            // enumerate every tensor in the single file
            let st = candle_core::safetensors::load(&single, &Device::Cpu)?;
            for name in st.keys() {
                map.insert(name.clone(), single.clone());
            }
        } else {
            return Err(QuantizeError::ReadModel(format!(
                "no model.safetensors or model.safetensors.index.json in {}",
                dir.display()
            )));
        }
        let names: Vec<String> = map.keys().cloned().collect();
        Ok(Self { map, names })
    }

    pub fn tensor_names(&self) -> &[String] {
        &self.names
    }

    /// Load a tensor and cast to f32 on CPU.
    pub fn load_f32(&self, name: &str) -> Result<Tensor, QuantizeError> {
        let path = self
            .map
            .get(name)
            .ok_or_else(|| QuantizeError::ReadModel(format!("tensor `{name}` not found")))?;
        let st = candle_core::safetensors::load(path, &Device::Cpu)?;
        let t = st
            .get(name)
            .ok_or_else(|| QuantizeError::ReadModel(format!("tensor `{name}` missing from shard")))?;
        Ok(t.to_dtype(candle_core::DType::F32)?)
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize read::`
Expected: PASS (2 tests)

> Note: `load_f32` re-reads the shard per tensor for simplicity. If profiling later shows this is slow on large sharded models, cache the loaded shard map; YAGNI for now.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize/src/read.rs
git commit -m "feat(quantize): SafeTensors source reader (single + sharded)"
```

---

### Task 5: Verify — round-trip error metric

**Files:**
- Create: `crates/vox-quantize/src/verify.rs`
- Test: inline `#[cfg(test)]`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use candle_core::quantized::{GgmlDType, QTensor};

    #[test]
    fn q8_0_error_smaller_than_q4k() {
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (16, 256), &dev).unwrap();
        let q8 = QTensor::quantize(&t, GgmlDType::Q8_0).unwrap();
        let q4 = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let e8 = round_trip_mse(&t, &q8).unwrap();
        let e4 = round_trip_mse(&t, &q4).unwrap();
        assert!(e8 < e4, "Q8_0 mse {e8} should be < Q4K mse {e4}");
        assert!(e8.is_finite() && e4.is_finite());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize verify::`
Expected: FAIL (compile error — `round_trip_mse` undefined).

- [ ] **Step 3: Implement**

```rust
use crate::error::QuantizeError;
use candle_core::quantized::QTensor;
use candle_core::Tensor;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct TensorQuantStat {
    pub name: String,
    pub src_dtype: String,
    pub target_dtype: String,
    pub params: usize,
    pub mse: f64,
    pub max_abs: f64,
    pub fallback: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct QuantReport {
    pub tensors: Vec<TensorQuantStat>,
    pub total_src_bytes: u64,
    pub total_quant_bytes: u64,
    pub compression_ratio: f64,
    pub worst_mse: f64,
}

/// Dequantize `q` and compute mean-squared error against the f32 source `src`.
pub fn round_trip_mse(src: &Tensor, q: &QTensor) -> Result<f64, QuantizeError> {
    let deq = q.dequantize(src.device())?;
    let diff = (src - &deq)?;
    let sq = diff.sqr()?;
    let mse = sq.mean_all()?.to_scalar::<f32>()? as f64;
    Ok(mse)
}

/// Max absolute error against the f32 source.
pub fn round_trip_max_abs(src: &Tensor, q: &QTensor) -> Result<f64, QuantizeError> {
    let deq = q.dequantize(src.device())?;
    let diff = (src - &deq)?.abs()?;
    Ok(diff.max_all()?.to_scalar::<f32>()? as f64)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize verify::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize/src/verify.rs
git commit -m "feat(quantize): round-trip MSE/max-abs + report types"
```

---

### Task 6: Write — quantized SafeTensors-canonical artifact

**Files:**
- Create: `crates/vox-quantize/src/write.rs`
- Test: inline `#[cfg(test)]`

> Prerequisite: Task 0 confirmed the `QTensor` raw-bytes accessor. The code below assumes `qtensor.data()? -> std::borrow::Cow<[u8]>`. If Task 0 found a different signature, adjust `quantized_bytes` accordingly.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use candle_core::quantized::{GgmlDType, QTensor};

    #[test]
    fn writes_blocks_and_metadata_that_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let dev = Device::Cpu;
        let t = Tensor::randn(0f32, 1f32, (8, 256), &dev).unwrap();
        let q = QTensor::quantize(&t, GgmlDType::Q4K).unwrap();
        let mut artifact = ArtifactWriter::new();
        artifact.add_quantized("w", &q, &[8, 256]).unwrap();
        artifact.add_f32("norm", &Tensor::ones((8,), candle_core::DType::F32, &dev).unwrap()).unwrap();
        artifact.finish(dir.path(), "Q4_K_M").unwrap();

        // metadata sidecar present and describes both tensors
        let meta_raw = std::fs::read_to_string(dir.path().join("quant-metadata.json")).unwrap();
        assert!(meta_raw.contains("\"ggml_dtype\":\"Q4K\""));
        assert!(meta_raw.contains("\"mixture\":\"Q4_K_M\""));
        // safetensors file present
        assert!(dir.path().join("model.safetensors").exists());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize write::`
Expected: FAIL (compile error — `ArtifactWriter` undefined).

- [ ] **Step 3: Implement**

```rust
use crate::error::QuantizeError;
use candle_core::quantized::QTensor;
use candle_core::Tensor;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Serialize)]
pub struct TensorMeta {
    pub ggml_dtype: String,   // "Q4K", "Q6K", "Q8_0", or "F32"
    pub orig_shape: Vec<usize>,
    pub orig_dtype: String,   // "F32" for kept tensors
    pub quantized: bool,
}

#[derive(Debug, Serialize)]
struct QuantMetadata {
    mixture: String,
    writer_version: String,
    tensors: HashMap<String, TensorMeta>,
}

pub struct ArtifactWriter {
    // safetensors payload: name -> (raw bytes, dtype tag, shape)
    raw: HashMap<String, (Vec<u8>, candle_core::DType, Vec<usize>)>,
    meta: HashMap<String, TensorMeta>,
}

fn quantized_bytes(q: &QTensor) -> Result<Vec<u8>, QuantizeError> {
    // Confirmed in Task 0. data() returns the CPU block bytes.
    Ok(q.data()?.into_owned())
}

impl ArtifactWriter {
    pub fn new() -> Self {
        Self { raw: HashMap::new(), meta: HashMap::new() }
    }

    /// Store a quantized tensor as a 1-D u8 block tensor + metadata.
    pub fn add_quantized(&mut self, name: &str, q: &QTensor, orig_shape: &[usize]) -> Result<(), QuantizeError> {
        let bytes = quantized_bytes(q)?;
        self.meta.insert(name.to_string(), TensorMeta {
            ggml_dtype: format!("{:?}", q.dtype()),
            orig_shape: orig_shape.to_vec(),
            orig_dtype: "F32".into(),
            quantized: true,
        });
        let len = bytes.len();
        self.raw.insert(name.to_string(), (bytes, candle_core::DType::U8, vec![len]));
        Ok(())
    }

    /// Store an unquantized f32 tensor unchanged.
    pub fn add_f32(&mut self, name: &str, t: &Tensor) -> Result<(), QuantizeError> {
        let shape = t.dims().to_vec();
        let flat = t.flatten_all()?.to_vec1::<f32>()?;
        let mut bytes = Vec::with_capacity(flat.len() * 4);
        for v in &flat { bytes.extend_from_slice(&v.to_le_bytes()); }
        self.meta.insert(name.to_string(), TensorMeta {
            ggml_dtype: "F32".into(),
            orig_shape: shape.clone(),
            orig_dtype: "F32".into(),
            quantized: false,
        });
        self.raw.insert(name.to_string(), (bytes, candle_core::DType::F32, shape));
        Ok(())
    }

    pub fn finish(self, out_dir: &Path, mixture: &str) -> Result<(), QuantizeError> {
        std::fs::create_dir_all(out_dir)?;
        // Build safetensors tensors from raw bytes.
        use candle_core::{Device, Tensor};
        let mut tensors: HashMap<String, Tensor> = HashMap::new();
        for (name, (bytes, dtype, shape)) in self.raw {
            let t = match dtype {
                candle_core::DType::U8 => Tensor::from_vec(bytes, shape.clone(), &Device::Cpu)?,
                candle_core::DType::F32 => {
                    let floats: Vec<f32> = bytes.chunks_exact(4)
                        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect();
                    Tensor::from_vec(floats, shape.clone(), &Device::Cpu)?
                }
                _ => return Err(QuantizeError::Write(format!("unexpected dtype for `{name}`"))),
            };
            tensors.insert(name, t);
        }
        candle_core::safetensors::save(&tensors, out_dir.join("model.safetensors"))?;

        let meta = QuantMetadata {
            mixture: mixture.to_string(),
            writer_version: env!("CARGO_PKG_VERSION").to_string(),
            tensors: self.meta,
        };
        let json = serde_json::to_string_pretty(&meta)
            .map_err(|e| QuantizeError::Write(e.to_string()))?;
        std::fs::write(out_dir.join("quant-metadata.json"), json)?;
        Ok(())
    }
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize write::`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize/src/write.rs
git commit -m "feat(quantize): write quantized SafeTensors-canonical artifact + metadata sidecar"
```

---

### Task 7: Engine — orchestrate the full pipeline

**Files:**
- Create: `crates/vox-quantize/src/engine.rs`
- Test: inline `#[cfg(test)]` (end-to-end tiny-model fixture)

- [ ] **Step 1: Write the failing integration test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::QuantMixture;
    use candle_core::{Device, DType, Tensor};
    use std::collections::HashMap;

    // Build a 1-layer Qwen3.5-shaped model with dims divisible by 256.
    fn tiny_model(dir: &std::path::Path) {
        let dev = Device::Cpu;
        let d = 256usize;
        let mut t: HashMap<String, Tensor> = HashMap::new();
        let w = |r, c| Tensor::randn(0f32, 1f32, (r, c), &dev).unwrap();
        let v = |n| Tensor::ones((n,), DType::F32, &dev).unwrap();
        t.insert("model.language_model.embed_tokens.weight".into(), w(512, d));
        t.insert("model.language_model.layers.0.self_attn.q_proj.weight".into(), w(d, d));
        t.insert("model.language_model.layers.0.self_attn.v_proj.weight".into(), w(d, d));
        t.insert("model.language_model.layers.0.mlp.down_proj.weight".into(), w(d, d));
        t.insert("model.language_model.layers.0.input_layernorm.weight".into(), v(d));
        t.insert("model.language_model.layers.0.linear_attn.A_log".into(), v(d));
        t.insert("model.language_model.norm.weight".into(), v(d));
        candle_core::safetensors::save(&t, dir.join("model.safetensors")).unwrap();
        std::fs::write(dir.join("config.json"),
            r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();
    }

    #[test]
    fn quantizes_end_to_end_q4km() {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        tiny_model(indir.path());
        let req = QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: QuantMixture::Q4KM,
            verify: true,
        };
        let report = quantize(&req).unwrap();
        // norms / A_log kept F32, matrices quantized
        let by_name: std::collections::HashMap<_,_> =
            report.tensors.iter().map(|s| (s.name.as_str(), s)).collect();
        assert_eq!(by_name["model.language_model.layers.0.input_layernorm.weight"].target_dtype, "F32");
        assert_eq!(by_name["model.language_model.layers.0.linear_attn.A_log"].target_dtype, "F32");
        assert_eq!(by_name["model.language_model.layers.0.self_attn.q_proj.weight"].target_dtype, "Q4K");
        // down_proj & v_proj bumped to Q6K under _M
        assert_eq!(by_name["model.language_model.layers.0.mlp.down_proj.weight"].target_dtype, "Q6K");
        assert_eq!(by_name["model.language_model.layers.0.self_attn.v_proj.weight"].target_dtype, "Q6K");
        // compression achieved, all errors finite
        assert!(report.compression_ratio > 1.5, "ratio {}", report.compression_ratio);
        assert!(report.worst_mse.is_finite());
        assert!(outdir.path().join("quant-metadata.json").exists());
        assert!(outdir.path().join("config.json").exists());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize engine::tests::quantizes_end_to_end_q4km`
Expected: FAIL (compile error — `QuantizeRequest`, `quantize` undefined).

- [ ] **Step 3: Implement**

```rust
use crate::error::QuantizeError;
use crate::policy::{resolve_dtype, QuantMixture, TensorRole};
use crate::read::SafeTensorsSource;
use crate::verify::{round_trip_max_abs, round_trip_mse, QuantReport, TensorQuantStat};
use crate::write::ArtifactWriter;
use candle_core::quantized::{GgmlDType, QTensor};
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct QuantizeRequest {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub mixture: QuantMixture,
    pub verify: bool,
}

pub fn quantize(req: &QuantizeRequest) -> Result<QuantReport, QuantizeError> {
    let src = SafeTensorsSource::open(&req.input_dir)?;
    let mut writer = ArtifactWriter::new();
    let mut stats = Vec::new();
    let mut total_src: u64 = 0;
    let mut total_quant: u64 = 0;

    for name in src.tensor_names() {
        let t = src.load_f32(name)?;
        let shape = t.dims().to_vec();
        let params: usize = shape.iter().product();
        let src_bytes = (params * 4) as u64;
        total_src += src_bytes;

        let role = TensorRole::from_key(name);
        let desired = req.mixture.target_for(role);

        match desired {
            None => {
                // keep F32
                writer.add_f32(name, &t)?;
                total_quant += src_bytes;
                stats.push(TensorQuantStat {
                    name: name.clone(), src_dtype: "F32".into(), target_dtype: "F32".into(),
                    params, mse: 0.0, max_abs: 0.0, fallback: false,
                });
            }
            Some(target) => {
                let last_dim = *shape.last().unwrap_or(&0);
                let resolved = resolve_dtype(target, last_dim);
                let fallback = resolved != target;
                if matches!(resolved, GgmlDType::F32) {
                    writer.add_f32(name, &t)?;
                    total_quant += src_bytes;
                    stats.push(TensorQuantStat {
                        name: name.clone(), src_dtype: "F32".into(), target_dtype: "F32".into(),
                        params, mse: 0.0, max_abs: 0.0, fallback,
                    });
                } else {
                    let q = QTensor::quantize(&t, resolved)?;
                    let (mse, max_abs) = if req.verify {
                        let mse = round_trip_mse(&t, &q)?;
                        if !mse.is_finite() {
                            return Err(QuantizeError::VerifyFailed { tensor: name.clone(), mse });
                        }
                        (mse, round_trip_max_abs(&t, &q)?)
                    } else { (0.0, 0.0) };
                    total_quant += q.storage_size_in_bytes() as u64;
                    let dtype_str = format!("{:?}", resolved);
                    writer.add_quantized(name, &q, &shape)?;
                    stats.push(TensorQuantStat {
                        name: name.clone(), src_dtype: "F32".into(), target_dtype: dtype_str,
                        params, mse, max_abs, fallback,
                    });
                }
            }
        }
    }

    // Copy config.json alongside the artifact for downstream loaders.
    let cfg = req.input_dir.join("config.json");
    if cfg.exists() {
        std::fs::create_dir_all(&req.output_dir)?;
        std::fs::copy(&cfg, req.output_dir.join("config.json"))?;
    }

    let mixture_name = match &req.mixture {
        QuantMixture::Q4KM => "Q4_K_M",
        QuantMixture::Q5KM => "Q5_K_M",
        QuantMixture::Q6K => "Q6_K",
        QuantMixture::Q8_0 => "Q8_0",
        QuantMixture::Manual(_) => "manual",
    };
    writer.finish(&req.output_dir, mixture_name)?;

    let worst_mse = stats.iter().map(|s| s.mse).fold(0.0_f64, f64::max);
    let compression_ratio = if total_quant == 0 { 0.0 } else { total_src as f64 / total_quant as f64 };
    Ok(QuantReport { tensors: stats, total_src_bytes: total_src, total_quant_bytes: total_quant, compression_ratio, worst_mse })
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize engine::tests::quantizes_end_to_end_q4km`
Expected: PASS

- [ ] **Step 5: Run the full suite + arch-check + clippy**

Run: `cargo test -p vox-quantize`
Expected: all PASS
Run: `cargo clippy -p vox-quantize -- -D warnings`
Expected: clean
Run: `cargo run -p vox-arch-check`
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-quantize/src/engine.rs
git commit -m "feat(quantize): end-to-end quantize pipeline + QuantReport"
```

---

### Task 8: ADR-043 + sharded integration test

**Files:**
- Create: `docs/src/architecture/adr-043-quantized-safetensors-ondisk-format.md`
- Test: add to `crates/vox-quantize/src/engine.rs` tests

- [ ] **Step 1: Write the sharded end-to-end test**

```rust
    #[test]
    fn quantizes_sharded_model() {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        let dev = candle_core::Device::Cpu;
        let w = |r, c| candle_core::Tensor::randn(0f32, 1f32, (r, c), &dev).unwrap();
        let mut s1 = std::collections::HashMap::new();
        s1.insert("model.language_model.layers.0.mlp.gate_proj.weight".to_string(), w(256, 256));
        let mut s2 = std::collections::HashMap::new();
        s2.insert("model.language_model.layers.0.mlp.up_proj.weight".to_string(), w(256, 256));
        candle_core::safetensors::save(&s1, indir.path().join("model-00001-of-00002.safetensors")).unwrap();
        candle_core::safetensors::save(&s2, indir.path().join("model-00002-of-00002.safetensors")).unwrap();
        std::fs::write(indir.path().join("model.safetensors.index.json"),
            r#"{"weight_map":{"model.language_model.layers.0.mlp.gate_proj.weight":"model-00001-of-00002.safetensors","model.language_model.layers.0.mlp.up_proj.weight":"model-00002-of-00002.safetensors"}}"#).unwrap();
        let req = QuantizeRequest {
            input_dir: indir.path().to_path_buf(), output_dir: outdir.path().to_path_buf(),
            mixture: QuantMixture::Q4KM, verify: true,
        };
        let report = quantize(&req).unwrap();
        assert_eq!(report.tensors.len(), 2);
        assert!(report.tensors.iter().all(|s| s.target_dtype == "Q4K"));
    }
```

- [ ] **Step 2: Run to verify it passes**

Run: `cargo test -p vox-quantize engine::tests::quantizes_sharded_model`
Expected: PASS

- [ ] **Step 3: Write ADR-043**

```markdown
---
title: "ADR-043: Quantized SafeTensors On-Disk Format"
description: "Quantized models are stored as SafeTensors-canonical u8 block tensors plus a quant-metadata.json sidecar."
category: architecture
---

# ADR-043: Quantized SafeTensors On-Disk Format

## Status
Accepted (2026-05-31)

## Context
Charter §0.2.3 mandates SafeTensors as the only on-disk weight format. GGML k-quant
blocks are not a native SafeTensors dtype.

## Decision
`vox-quantize` writes quantized tensors as 1-D `u8` SafeTensors tensors carrying the raw
GGML block bytes, accompanied by a `quant-metadata.json` sidecar mapping each tensor to
`{ ggml_dtype, orig_shape, orig_dtype, quantized }`, plus the mixture name and writer
version. KEEP-F32 tensors are stored unchanged. The source `config.json` is copied
alongside. The sidecar is part of the artifact contract — the `u8` blobs are
uninterpretable without it.

## Consequences
- Stays SafeTensors-canonical; no GGUF on disk (charter-compliant).
- Downstream loaders (SP-2 inference) reconstruct `QTensor`s from the `u8` bytes + metadata.
- Artifacts remain hash-addressable for the CAS/bundle machinery.
```

- [ ] **Step 4: Commit**

```bash
git add crates/vox-quantize/src/engine.rs docs/src/architecture/adr-043-quantized-safetensors-ondisk-format.md
git commit -m "feat(quantize): sharded e2e test + ADR-043 on-disk format"
```

---

## Self-Review

- **Spec coverage:** read (single+sharded) ✓ Task 4; policy + mixtures + alignment ✓ Task 3; engine ✓ Task 7; verify ✓ Task 5; write + on-disk format ✓ Task 6 + ADR Task 8; CPU-only (no GPU feature in Cargo.toml) ✓ Task 1; public API matches spec §5 ✓ Task 7.
- **Placeholder scan:** none — every code step is complete. Task 0 spike confirms the one candle API detail before it's used in Task 6.
- **Type consistency:** `QuantizeRequest`/`QuantMixture`/`QuantReport`/`TensorQuantStat`/`TensorRole`/`SafeTensorsSource`/`ArtifactWriter` names consistent across Tasks 3–8. `round_trip_mse`/`round_trip_max_abs` defined in Task 5, used in Task 7.
- **Known risk:** `QTensor::data()` accessor (Task 6) is gated behind Task 0; `storage_size_in_bytes()` used in Task 7 for quant-byte accounting — confirm both exist in Task 0.
