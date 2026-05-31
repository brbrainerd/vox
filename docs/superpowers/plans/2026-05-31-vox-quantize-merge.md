# Quantize Merged QLoRA (SP-3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After `merge-qlora` produces a merged f32 SafeTensors *subset*, recombine it over the base model into a complete model, then quantize it with `vox-quantize` to produce a small deployable artifact.

**Architecture:** A glue module that (1) materializes a complete f32 model into a temp dir by copying base shards and overwriting adapted keys with the merged subset, (2) calls `vox_quantize::quantize` on that temp dir, (3) cleans up. No new quantization logic.

**Tech Stack:** Rust, `vox-quantize` (SP-1), `candle-core` safetensors I/O, existing merge path.

**Spec:** `docs/superpowers/specs/2026-05-31-vox-quantize-merge-design.md`
**Depends on:** SP-1. Reuses the existing `merge-qlora` output (verified at `vox-plugin-mens-candle-cuda/src/merge.rs:102-191`, f32 subset + `external_serving_handoff_v1.json`).

---

### Task 1: Recombine subset-over-base into a complete model

**Files:**
- Create: `crates/vox-quantize/src/recombine.rs`
- Modify: `crates/vox-quantize/src/lib.rs` (add `pub mod recombine;`)
- Test: inline `#[cfg(test)]`

> Rationale: recombination is a reusable model operation, so it lives in `vox-quantize` (L2), not the CLI. SP-3's CLI wiring (Task 2) just calls it.

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{Device, Tensor};
    use std::collections::HashMap;

    #[test]
    fn merged_subset_overrides_base_keys() {
        let dev = Device::Cpu;
        let base = tempfile::tempdir().unwrap();
        let merged = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();

        // base: two tensors
        let mut b: HashMap<String, Tensor> = HashMap::new();
        b.insert("w_adapted".into(), Tensor::zeros((256, 256), candle_core::DType::F32, &dev).unwrap());
        b.insert("w_frozen".into(), Tensor::ones((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
        std::fs::write(base.path().join("config.json"), r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();

        // merged subset: only w_adapted, with a distinguishable value (2.0)
        let mut m: HashMap<String, Tensor> = HashMap::new();
        m.insert("w_adapted".into(), Tensor::full(2.0f32, (256, 256), &dev).unwrap());
        candle_core::safetensors::save(&m, merged.path().join("merged.safetensors")).unwrap();

        recombine(base.path(), &merged.path().join("merged.safetensors"), out.path()).unwrap();

        let result = candle_core::safetensors::load(out.path().join("model.safetensors"), &dev).unwrap();
        // adapted key takes merged value
        assert_eq!(result["w_adapted"].mean_all().unwrap().to_scalar::<f32>().unwrap(), 2.0);
        // frozen key retains base value
        assert_eq!(result["w_frozen"].mean_all().unwrap().to_scalar::<f32>().unwrap(), 1.0);
        // config copied
        assert!(out.path().join("config.json").exists());
    }

    #[test]
    fn merged_key_absent_from_base_errors() {
        let dev = Device::Cpu;
        let base = tempfile::tempdir().unwrap();
        let merged = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        let mut b: HashMap<String, Tensor> = HashMap::new();
        b.insert("w_frozen".into(), Tensor::ones((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
        let mut m: HashMap<String, Tensor> = HashMap::new();
        m.insert("not_in_base".into(), Tensor::zeros((256, 256), candle_core::DType::F32, &dev).unwrap());
        candle_core::safetensors::save(&m, merged.path().join("merged.safetensors")).unwrap();
        assert!(recombine(base.path(), &merged.path().join("merged.safetensors"), out.path()).is_err());
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-quantize recombine::`
Expected: FAIL (compile error — `recombine` undefined).

- [ ] **Step 3: Implement**

```rust
use crate::error::QuantizeError;
use crate::read::SafeTensorsSource;
use candle_core::Device;
use std::collections::HashMap;
use std::path::Path;

/// Build a complete f32 model in `out_dir` by taking every base tensor and
/// overwriting the keys present in the merged subset. Errors if the subset
/// contains a key absent from the base (a sign of an adapter/base mismatch).
pub fn recombine(base_dir: &Path, merged_subset: &Path, out_dir: &Path) -> Result<(), QuantizeError> {
    let base = SafeTensorsSource::open(base_dir)?;
    let merged = candle_core::safetensors::load(merged_subset, &Device::Cpu)?;

    // validate every merged key exists in base
    let base_names: std::collections::HashSet<&str> =
        base.tensor_names().iter().map(|s| s.as_str()).collect();
    for k in merged.keys() {
        if !base_names.contains(k.as_str()) {
            return Err(QuantizeError::ReadModel(format!(
                "merged key `{k}` not present in base model — adapter/base mismatch"
            )));
        }
    }

    let mut complete: HashMap<String, candle_core::Tensor> = HashMap::new();
    for name in base.tensor_names() {
        let t = match merged.get(name) {
            Some(m) => m.to_dtype(candle_core::DType::F32)?,
            None => base.load_f32(name)?,
        };
        complete.insert(name.clone(), t);
    }

    std::fs::create_dir_all(out_dir)?;
    candle_core::safetensors::save(&complete, out_dir.join("model.safetensors"))?;
    let cfg = base_dir.join("config.json");
    if cfg.exists() {
        std::fs::copy(&cfg, out_dir.join("config.json"))?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-quantize recombine::`
Expected: PASS (2 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-quantize/src/recombine.rs crates/vox-quantize/src/lib.rs
git commit -m "feat(quantize): recombine merged-subset over base into complete model"
```

---

### Task 2: `--quantize` option on the merge CLI path

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/schola/merge_qlora.rs`
- Test: `crates/vox-ml-cli/tests/merge_quantize.rs`

- [ ] **Step 1: Add the flag**

In the merge-qlora args struct in `merge_qlora.rs:40-45`, add:
```rust
    /// If set, after merging, recombine over base and quantize to this mixture
    /// (q4_k_m|q5_k_m|q6_k|q8_0). Writes a quantized artifact next to --output.
    #[arg(long)]
    pub quantize: Option<String>,
```

- [ ] **Step 2: Write the integration test**

```rust
#[test]
fn merge_then_quantize_produces_quantized_artifact() {
    // Arrange: tiny base + a merged subset already on disk (skip real training).
    use candle_core::{Device, DType, Tensor};
    use std::collections::HashMap;
    let dev = Device::Cpu;
    let base = tempfile::tempdir().unwrap();
    let merged_out = tempfile::tempdir().unwrap();
    let q_out = tempfile::tempdir().unwrap();

    let mut b: HashMap<String, Tensor> = HashMap::new();
    b.insert("model.language_model.layers.0.mlp.gate_proj.weight".into(),
        Tensor::randn(0f32, 1f32, (256, 256), &dev).unwrap());
    b.insert("model.language_model.norm.weight".into(),
        Tensor::ones((256,), DType::F32, &dev).unwrap());
    candle_core::safetensors::save(&b, base.path().join("model.safetensors")).unwrap();
    std::fs::write(base.path().join("config.json"),
        r#"{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],"hidden_size":256,"num_attention_heads":8,"num_hidden_layers":1,"vocab_size":512}"#).unwrap();

    let mut m: HashMap<String, Tensor> = HashMap::new();
    m.insert("model.language_model.layers.0.mlp.gate_proj.weight".into(),
        Tensor::full(0.5f32, (256, 256), &dev).unwrap());
    candle_core::safetensors::save(&m, merged_out.path().join("merged.safetensors")).unwrap();

    // Act: drive the recombine+quantize helper directly (the CLI calls this).
    vox_quantize::recombine::recombine(
        base.path(), &merged_out.path().join("merged.safetensors"), q_out.path()).unwrap();
    let report = vox_quantize::quantize(&vox_quantize::engine::QuantizeRequest {
        input_dir: q_out.path().to_path_buf(),
        output_dir: q_out.path().join("quantized"),
        mixture: vox_quantize::QuantMixture::Q4KM,
        verify: true,
    }).unwrap();

    assert!(q_out.path().join("quantized/quant-metadata.json").exists());
    assert!(report.compression_ratio > 1.5);
}
```

- [ ] **Step 3: Implement the post-merge branch in `run`**

After the existing merge writes the subset to `--output`, add:
```rust
    if let Some(mixture_str) = args.quantize.as_deref() {
        let mixture = crate::commands::quantize::parse_mixture(mixture_str)?;
        let recombined = args.output.with_extension("recombined");
        // base_dir = directory of the first --base-shard; merged subset = args.output
        let base_dir = args.base_shard.first()
            .and_then(|p| p.parent())
            .ok_or_else(|| anyhow::anyhow!("need at least one --base-shard to recombine"))?;
        vox_quantize::recombine::recombine(base_dir, &args.output, &recombined)?;
        let q_out = args.output.with_extension("quantized");
        let report = vox_quantize::quantize(&vox_quantize::engine::QuantizeRequest {
            input_dir: recombined.clone(),
            output_dir: q_out.clone(),
            mixture,
            verify: true,
        })?;
        println!("Quantized merged model -> {} ({:.2}x)", q_out.display(), report.compression_ratio);
        let _ = std::fs::remove_dir_all(&recombined);
    }
```

> Confirm the merge args field name for base shards (`base_shard: Vec<PathBuf>` per spec; verify against `merge_qlora.rs`). Adjust `base_dir` derivation if shards live in subfolders.

- [ ] **Step 4: Run + commit**

Run: `cargo test -p vox-ml-cli --test merge_quantize`
Expected: PASS
Run: `cargo test -p vox-quantize`
Expected: PASS (recombine still green)
```bash
git add crates/vox-ml-cli/src/commands/schola/merge_qlora.rs crates/vox-ml-cli/tests/merge_quantize.rs
git commit -m "feat(ml-cli): --quantize on merge-qlora (recombine over base + quantize)"
```

---

## Self-Review

- **Spec coverage:** recombine subset-over-base ✓ T1; key-mismatch error ✓ T1; merge→quantize glue ✓ T2; reuse SP-1, no new quant logic ✓ (only `recombine` + glue added).
- **Placeholder scan:** none. The `base_shard` field-name confirmation is an explicit verify-then-adjust note.
- **Type consistency:** `recombine` signature consistent T1↔T2; `QuantizeRequest`/`QuantMixture` match SP-1; `parse_mixture` reused from SP-4 (Task 2 depends on SP-4 Task 1 being present — note this ordering: build SP-4 before SP-3, matching the recommended SP-1→SP-4→SP-3→SP-2 order).
