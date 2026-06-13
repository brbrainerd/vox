# Semantic Behavior Map — `vox-inference`

Deterministically synthesized from 31 distinct proven-behavior claims (of 31 extracted) across 20 symbols. 3 symbols have an explicit error-path proof; **14 are proven only on the happy path** (no error/edge/invariant claim) — the semantic holes line coverage hides.

## Per-symbol proven behaviors


### `QwenForward::forward`  (happy, invariant; EXTRACTED)
- [happy] forward() with full attention returns logits with shape [batch_size, seq_len, vocab_size]  (crates/vox-inference/src/qwen_forward.rs)
- [invariant] forward() with full attention returns logits with all finite values  (crates/vox-inference/src/qwen_forward.rs)
- [happy] forward() with linear attention returns logits with shape [batch_size, seq_len, vocab_size]  (crates/vox-inference/src/qwen_forward.rs)
- [invariant] forward() with linear attention returns logits with all finite values  (crates/vox-inference/src/qwen_forward.rs)

### `QwenForward::forward()`  (happy; EXTRACTED)
- [happy] forward() returns logits with correct shape [batch, seq_len, vocab_size]  (crates/vox-inference/src/qwen_forward.rs)
- [happy] forward() produces finite logit values (no NaN or Inf)  (crates/vox-inference/src/qwen_forward.rs)
- [happy] forward() with linear attention returns logits with shape [batch, seq_len, vocab_size]  (crates/vox-inference/src/qwen_forward.rs)
- [happy] forward() with linear attention produces finite logit values  (crates/vox-inference/src/qwen_forward.rs)

### `CandleCpuBackend::predict`  (error, happy, invariant; EXTRACTED)
- [happy] predict() returns non-stub text output (does not contain 'stub')  (crates/vox-inference/src/backends/candle_cpu.rs)
- [invariant] predict() with temperature=0.0 is deterministic for same input  (crates/vox-inference/src/backends/candle_cpu.rs)
- [error] predict() with unknown label returns InferenceError::Internal containing 'not loaded'  (crates/vox-inference/src/backends/candle_cpu.rs)

### `generate()`  (happy, invariant; EXTRACTED)
- [happy] generate() output length does not exceed max_new_tokens parameter  (crates/vox-inference/src/generate.rs)
- [happy] generate() returns token IDs that are all within vocab range  (crates/vox-inference/src/generate.rs)
- [invariant] greedy generate with temperature=0.0 is deterministic across repeated calls  (crates/vox-inference/src/generate.rs)

### `QwenWeights::qmatmul()`  (happy; EXTRACTED)
- [happy] qmatmul() returns Some(QMatMul) for quantized matrix weights  (crates/vox-inference/src/qwen_weights.rs)
- [happy] qmatmul() returns None for F32 tensor weights  (crates/vox-inference/src/qwen_weights.rs)

### `CandleCpuBackend::load_from_dir`  (happy; EXTRACTED)
- [happy] load_from_dir() returns a LoadedModel with label prefixed with 'candle-cpu-dir-'  (crates/vox-inference/src/backends/candle_cpu.rs)

### `CandleCpuBackend::loaded`  (invariant; EXTRACTED)
- [invariant] loaded map contains key for the LoadedModel after load_from_dir succeeds  (crates/vox-inference/src/backends/candle_cpu.rs)

### `CandleCpuBackend::unload`  (happy; EXTRACTED)
- [happy] unload() removes LoadedModel from the loaded map  (crates/vox-inference/src/backends/candle_cpu.rs)

### `CandleCudaBackend::load_from_dir`  (happy; EXTRACTED)
- [happy] load_from_dir() returns LoadedModel with label prefixed with 'candle-cuda-dir-'  (crates/vox-inference/src/backends/candle_cuda.rs)

### `CandleCudaBackend::predict`  (happy; EXTRACTED)
- [happy] predict() returns non-stub text (falls back to CPU when CUDA unavailable)  (crates/vox-inference/src/backends/candle_cuda.rs)

### `CandleMetalBackend::load_from_dir`  (happy; EXTRACTED)
- [happy] load_from_dir() returns a LoadedModel with a label starting with 'candle-metal-dir-'  (crates/vox-inference/src/backends/candle_metal.rs)

### `CandleMetalBackend::predict`  (happy; EXTRACTED)
- [happy] predict() on a loaded model succeeds and returns non-stub inference text  (crates/vox-inference/src/backends/candle_metal.rs)

### `InferenceError`  (error; EXTRACTED)
- [error] InferenceError::Unsupported contains 'CAS' in its message field  (crates/vox-inference/src/dispatcher.rs)

### `InferenceError::Unsupported`  (error; EXTRACTED)
- [error] predict_auto returns InferenceError::Unsupported when ModelBundle load is not supported  (crates/vox-inference/src/dispatcher.rs)

### `QMatMul::forward`  (happy; EXTRACTED)
- [happy] forward() on a QMatMul computes output with correct shape  (crates/vox-inference/src/qwen_weights.rs)

### `QMatMul::forward()`  (happy; EXTRACTED)
- [happy] QMatMul forward produces output with correct shape  (crates/vox-inference/src/qwen_weights.rs)

### `QwenWeights::load`  (happy; EXTRACTED)
- [happy] load() returns QwenWeights that can be queried for quantized matrices via qmatmul()  (crates/vox-inference/src/qwen_weights.rs)

### `QwenWeights::qmatmul`  (happy; EXTRACTED)
- [happy] qmatmul() returns None for F32 weights that are stored as Tensor not QMatMul  (crates/vox-inference/src/qwen_weights.rs)

### `QwenWeights::tensor`  (happy; EXTRACTED)
- [happy] tensor() returns Some for F32 weights that were not quantized  (crates/vox-inference/src/qwen_weights.rs)

### `QwenWeights::tensor()`  (happy; EXTRACTED)
- [happy] tensor() returns Some for F32 weights that were not quantized  (crates/vox-inference/src/qwen_weights.rs)

## Semantic gaps (proven happy-path only)

These symbols have proven behavior but **no error, edge, or invariant proof** — failure/empty/boundary modes are unverified:

- **`CandleCpuBackend::load_from_dir`** — only: _load_from_dir() returns a LoadedModel with label prefixed with 'candle-cpu-dir-'_
- **`CandleCpuBackend::unload`** — only: _unload() removes LoadedModel from the loaded map_
- **`CandleCudaBackend::load_from_dir`** — only: _load_from_dir() returns LoadedModel with label prefixed with 'candle-cuda-dir-'_
- **`CandleCudaBackend::predict`** — only: _predict() returns non-stub text (falls back to CPU when CUDA unavailable)_
- **`CandleMetalBackend::load_from_dir`** — only: _load_from_dir() returns a LoadedModel with a label starting with 'candle-metal-dir-'_
- **`CandleMetalBackend::predict`** — only: _predict() on a loaded model succeeds and returns non-stub inference text_
- **`QMatMul::forward`** — only: _forward() on a QMatMul computes output with correct shape_
- **`QMatMul::forward()`** — only: _QMatMul forward produces output with correct shape_
- **`QwenForward::forward()`** — only: _forward() returns logits with correct shape [batch, seq_len, vocab_size]_
- **`QwenWeights::load`** — only: _load() returns QwenWeights that can be queried for quantized matrices via qmatmul()_
- **`QwenWeights::qmatmul`** — only: _qmatmul() returns None for F32 weights that are stored as Tensor not QMatMul_
- **`QwenWeights::qmatmul()`** — only: _qmatmul() returns Some(QMatMul) for quantized matrix weights_
- **`QwenWeights::tensor`** — only: _tensor() returns Some for F32 weights that were not quantized_
- **`QwenWeights::tensor()`** — only: _tensor() returns Some for F32 weights that were not quantized_
