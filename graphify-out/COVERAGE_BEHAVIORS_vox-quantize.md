# Semantic Behavior Map — `vox-quantize`

Deterministically synthesized from 25 distinct proven-behavior claims (of 25 extracted) across 12 symbols. 1 symbols have an explicit error-path proof; **8 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `ArtifactWriter`  (happy; EXTRACTED)
- [happy] ArtifactWriter.finish() writes metadata that correctly records GGML dtype (Q4K) for quantized tensors  (crates/vox-quantize/src/write.rs)
- [happy] ArtifactWriter.finish() writes metadata that correctly sets quantized flag to true for quantized tensors and false for F32 tensors  (crates/vox-quantize/src/write.rs)
- [happy] ArtifactWriter.finish() writes metadata that correctly records the mixture name (Q4_K_M) in quant-metadata.json  (crates/vox-quantize/src/write.rs)
- [happy] ArtifactWriter.finish() creates model.safetensors file in the output directory  (crates/vox-quantize/src/write.rs)

### `quantize()`  (edge, happy; EXTRACTED)
- [happy] quantize() with Q4KM mixture returns a Report with expected tensor target_dtype assignments (Q6K for down_proj/v_proj, Q4K for matrix weights, F32 for norms)  (crates/vox-quantize/src/engine.rs)
- [happy] quantize() successfully processes sharded SafeTensors models split across multiple files with index.json  (crates/vox-quantize/src/engine.rs)
- [edge] quantize() applies alignment fallback: last_dim 96 falls back to Q8_0, last_dim 100 falls back to F32, norms stay F32 by role  (crates/vox-quantize/src/engine.rs)

### `QuantMixture::Q4KM target_for()`  (happy; EXTRACTED)
- [happy] Q4KM.target_for(DownProj) and target_for(VProj) return Some(GgmlDType::Q6K)  (crates/vox-quantize/src/policy.rs)
- [happy] Q4KM.target_for(Matrix) returns Some(GgmlDType::Q4K) and target_for(KeepF32) returns None  (crates/vox-quantize/src/policy.rs)

### `QuantReport`  (happy; EXTRACTED)
- [happy] QuantReport.compression_ratio is > 1.5 and worst_mse is finite for successful quantization  (crates/vox-quantize/src/engine.rs)
- [happy] QuantReport from sharded model contains all tensors with correct target_dtype (Q4K for all when using Q4KM)  (crates/vox-quantize/src/engine.rs)

### `SafeTensorsSource load_f32()`  (happy; EXTRACTED)
- [happy] load_f32() reads a tensor by name from single-file model and returns correct shape  (crates/vox-quantize/src/read.rs)
- [happy] load_f32() reads tensors from sharded files correctly and returns correct shape  (crates/vox-quantize/src/read.rs)

### `SafeTensorsSource::open()`  (happy; EXTRACTED)
- [happy] open() successfully opens a directory with single model.safetensors file and returns list of tensor names  (crates/vox-quantize/src/read.rs)
- [happy] open() successfully parses model.safetensors.index.json weight_map and maps tensors across sharded files  (crates/vox-quantize/src/read.rs)

### `TensorRole::from_key()`  (happy; EXTRACTED)
- [happy] from_key() classifies layernorm.weight, A_log, dt_bias as TensorRole::KeepF32  (crates/vox-quantize/src/policy.rs)
- [happy] from_key() classifies down_proj.weight, v_proj.weight, embed_tokens, lm.head with their respective roles (DownProj, VProj, Embedding, Output)  (crates/vox-quantize/src/policy.rs)

### `recombine()`  (error; EXTRACTED)
- [error] recombine() returns Err when a merged key is absent from the base model, catching adapter/base mismatches  (crates/vox-quantize/src/recombine.rs)
- [error] recombine() returns Err when merged tensor shape does not match the corresponding base tensor shape  (crates/vox-quantize/src/recombine.rs)

### `round_trip_mse()`  (happy, invariant; EXTRACTED)
- [happy] round_trip_mse() computes smaller mean-squared error for Q8_0 quantization than for Q4K quantization on the same tensor  (crates/vox-quantize/src/verify.rs)
- [invariant] round_trip_mse() returns finite values when computing error metrics for both Q8_0 and Q4K quantized tensors  (crates/vox-quantize/src/verify.rs)

### `select()`  (happy; EXTRACTED)
- [happy] select(DevicePref::Cpu) returns a Device in Ok state that reports is_cpu() == true  (crates/vox-quantize/src/device.rs)
- [happy] select(DevicePref::Auto) returns Ok(Device) without error even on CPU-only systems  (crates/vox-quantize/src/device.rs)

### `QuantizeError Display`  (happy; EXTRACTED)
- [happy] QuantizeError::ShardIndex displays with 'shard index' in the error message  (crates/vox-quantize/src/error.rs)

### `resolve_dtype()`  (edge; EXTRACTED)
- [edge] resolve_dtype(Q4K, 512) returns Q4K; resolve_dtype(Q4K, 96) returns Q8_0; resolve_dtype(Q4K, 100) returns F32  (crates/vox-quantize/src/policy.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`ArtifactWriter`** — only: _ArtifactWriter.finish() writes metadata that correctly records GGML dtype (Q4K) for quantized tensors_
- **`QuantMixture::Q4KM target_for()`** — only: _Q4KM.target_for(DownProj) and target_for(VProj) return Some(GgmlDType::Q6K)_
- **`QuantReport`** — only: _QuantReport.compression_ratio is > 1.5 and worst_mse is finite for successful quantization_
- **`QuantizeError Display`** — only: _QuantizeError::ShardIndex displays with 'shard index' in the error message_
- **`SafeTensorsSource load_f32()`** — only: _load_f32() reads a tensor by name from single-file model and returns correct shape_
- **`SafeTensorsSource::open()`** — only: _open() successfully opens a directory with single model.safetensors file and returns list of tensor names_
- **`TensorRole::from_key()`** — only: _from_key() classifies layernorm.weight, A_log, dt_bias as TensorRole::KeepF32_
- **`select()`** — only: _select(DevicePref::Cpu) returns a Device in Ok state that reports is_cpu() == true_
