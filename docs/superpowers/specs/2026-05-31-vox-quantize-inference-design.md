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

## SP-2 §9 — Extraction map (Task 0)

Investigation of the working Qwen3.5/2.5 forward pass to decide how `vox-inference` (L3) can run quantized inference when the forward lives in `vox-plugin-mens-candle-cuda` (L4). READ-only task. All cites are to `crates/vox-plugin-mens-candle-cuda/src/`.

### VERDICT (up front)

**Option B — duplicate a quantized-only forward inside `vox-inference` — built on candle `quantized::QMatMul`.** Difficulty: **Medium**.

Option A (extract the existing structs into an L2 `vox-model-qwen`) is **not viable as a copy** because every linear layer in the forward is a qlora-rs `QuantizedLinear`, and that type is *intrinsically* a training/LoRA primitive: it NF4-quantizes the weight, always allocates a LoRA adapter, and its `forward` dequantizes NF4 to f32 then does a dense `matmul`. That is neither inference-pure nor `QMatMul`-based. Extracting the structs verbatim would drag qlora-rs (and its NF4 + LoRA machinery) into L2 — exactly the coupling we are trying to escape. The *math* (attention, RoPE, the Qwen3.5 linear-attention recurrence, MLP, RMSNorm) is, however, cleanly portable and weight-map driven. So the right move is a **re-implementation**, not a relocation: rewrite the linears over candle `QMatMul` and copy the pure-math helpers. Whether that re-implemented forward lands in a new L2 `vox-model-qwen` or directly in `vox-inference` is a layering choice — both are "Option B" in substance (a fresh quantized-only forward). Recommend putting it in `vox-inference` first (Option B proper); promote to L2 `vox-model-qwen` only once a second consumer appears.

### 1. Type inventory

| Struct | Fields | Linear repr | Inference-relevant? |
|---|---|---|---|
| `Qwen2Attention` (`model.rs:60-68`) | `q_proj,k_proj,v_proj,o_proj: QuantizedLinear`; `n_heads,n_kv_heads,head_dim: usize` | `QuantizedLinear` x4 | Yes — full forward at `model.rs:70-156` |
| `Qwen2MLP` (`model.rs:201-205`) | `gate_proj,up_proj,down_proj: QuantizedLinear` | `QuantizedLinear` x3 | Yes — `model.rs:207-223` |
| `Qwen35LinearAttention` (`model.rs:227-241`) | `qkv_proj,z_proj,b_proj,a_proj,out_proj: QuantizedLinear`; `conv_weight,dt_bias,a_log: Tensor`; `norm: RmsNorm`; head dims | `QuantizedLinear` x5 + raw `Tensor` x3 | Yes — gated-delta recurrence `model.rs:284-414` |
| `Qwen35AttentionBlock` (`model.rs:417-421`) | enum `Full(Qwen2Attention)` / `Linear(Qwen35LinearAttention)` | — | Yes |
| `Qwen35Layer` (`model.rs:423-429`) | `input_layernorm,post_attention_layernorm: RmsNorm`; `attention`; `mlp`; `inv_freq: Option<Tensor>` | — | Yes — `model.rs:431-475` |
| `Qwen35Model` (`model.rs:477-482`) | `embed_tokens: Tensor`; `layers: Vec<Qwen35Layer>`; `norm: RmsNorm`; `lm_head: QuantizedLinear` | embed = raw `Tensor` (index_select), lm_head = `QuantizedLinear` | Yes — `model.rs:484-503` |
| `Qwen35LayerCache` (`model.rs:505-508`) | enum `Full((Tensor,Tensor))` / `Linear(Tensor)` | — | Yes (cache state) |
| `CandleModel` (`model.rs:516-520`) | `_inner: Qwen35Model`; `model_path: String` | — | **Training/plumbing only** — `load_from_path` is a stub that bails (`model.rs:538-544`) |

No struct is training-only in its *shape*; the training entanglement is entirely inside the `QuantizedLinear` field type, not the model structs. `embed_tokens` is a plain `Tensor` used via `index_select` (`model.rs:489-492`) — trivially portable.

### 2. Forward shape

- Top entry: `Qwen35Model::forward(&self, input_ids: &Tensor) -> Result<Tensor>` (`model.rs:485`). Takes `[b, seq]` token ids, embeds via `index_select`, runs layers, final `RmsNorm`, clamps to +/-64 (`model.rs:498`), projects through `lm_head` to logits `[b, seq, vocab]`.
- **It threads NO cache and hardcodes `pos = 0`**: the loop calls `layer.forward(&x, 0, None)` (`model.rs:495`). So today's top-level forward is full-context, prefill-only. The KV-cache / RoPE-position plumbing exists one level down but is unused.
- `Qwen35Layer::forward(&self, x, pos, kv_cache: Option<&mut Qwen35LayerCache>)` (`model.rs:432`) — pre-norm residual: `input_layernorm` -> attention -> residual add -> `post_attention_layernorm` -> MLP -> residual add.
- `Qwen2Attention::forward(&self, x, pos, inv_freq: Option<&Tensor>, kv_cache: Option<&mut (Tensor,Tensor)>)` (`model.rs:71-77`) — supports a real KV cache (concat at `model.rs:110-118`) and RoPE (`apply_rotary_emb`, `model.rs:158`), with numeric guards (att clamp +/-120, v clamp +/-256). Causal mask only applied when `seq_len>1` (`model.rs:129`).
- `Qwen35LinearAttention::forward(&self, x, pos, inv_freq, state_cache: Option<&mut Tensor>)` (`model.rs:284`) — gated delta-net recurrence: depthwise causal conv+SiLU (`model.rs:263`), L2-norm q/k, per-timestep state update `[b, v_heads, k_dim, v_dim]` (`model.rs:366-395`), output gating by `silu(z)` and RMSNorm. `inv_freq`/`pos` are accepted but unused here (`model.rs:402-404`).
- RoPE is computed on the fly from `inv_freq` (per-layer `Option<Tensor>`); no separate sin/cos cache struct. Cache state is per-layer in `Qwen35LayerCache` (KV tuple for full layers, single recurrent state Tensor for linear layers).

Inference today (`inference.rs`): `generate` (`inference.rs:386`) re-runs the **full context** every step (`InferenceEngine::load` builds the model, `generate` calls `model.forward(&input)` per token, no cache — see comment `inference.rs:396-399`), takes the last position's logits, greedy-argmax. So the plugin's "inference" path deliberately ignores the cache-capable lower forwards.

### 3. QMatMul re-expressibility verdict

**Re-expressible: YES, and cleanly — but as a rewrite, not a wrap.** Every linear op in the forward is a single right-multiply `x @ W^T` (optionally + bias). candle `quantized::QMatMul` covers exactly this:
- `QMatMul::from_qtensor(QTensor)` for GGUF/quantized weights, or a `QMatMul` over an f32 `Tensor` for passthrough — `QMatMul::forward(x)` computes `x . W^T` for 2D/3D `x`, matching `QuantizedLinear::forward`'s base path (`qlora.rs:366-398`, which is dequantize -> `weight.t()` -> `matmul`).

What qlora-rs `QuantizedLinear` does that `QMatMul` does **not**:
- **NF4 quantization** (`quantize_nf4_with_config`, `qlora.rs:269`) — a bitsandbytes-style 4-bit format, *not* GGUF. `QMatMul` only consumes candle `QTensor` (GGUF k-quants / f32). So you cannot hand `QuantizedLinear`'s internal `QuantizedTensor` to `QMatMul`. For inference we want to feed `QMatMul` either the original f32 safetensors weight (what `inference.rs:119-138` already produces via `get_tensor`) or a candle-quantized `QTensor` — both supported.
- **LoRA adapter** (`lora.forward`, `qlora.rs:391`) — always present, added to the base output. For frozen-base inference it is zero-init (`LoraLayer::new_with_zeros`, `qlora.rs:279`) and contributes nothing; a merged-adapter inference would fold A.B into the base weight before quantizing. `QMatMul` has no LoRA — correct for inference; the adapter must be merged upstream (or omitted) rather than re-expressed.

Bias: `QuantizedLinear` supports optional bias (`qlora.rs:394`); the Qwen projections are constructed with `bias: None` everywhere (`inference.rs:185-328`), so bias handling can be dropped for the quantized-only forward (or kept as an optional `broadcast_add`).

Everything else in the forward is plain candle tensor ops (matmul, softmax, silu, sigmoid, cat, narrow, RmsNorm) — directly portable with zero qlora-rs dependency.

### 4. Coupling assessment — difficulty **Medium**

- **Autograd / VarMap / VarBuilder**: the inference forward does **not** touch them. `from_weight` (`qlora.rs:255`) builds inference-mode layers with no `VarBuilder`; `from_weight_with_varbuilder` (`qlora.rs:305`) is the training constructor and is **not** used in `inference.rs`. The plugin's inference load path (`inference.rs:45-348`) reads f32 safetensors via `SafeTensors`/`from_raw_buffer` into plain `Tensor`s — **a plain weight map, no VarMap** — then wraps each in `QuantizedLinear::from_weight`. So construction is already weight-map-driven; only the *layer type* is training-flavored.
- **The single coupling point** is the `QuantizedLinear` field type on six structs. Because it is a struct field (not a trait object), you cannot swap it without editing the structs — hence a copy of the structs as-is forces qlora-rs into L2. The math methods (`apply_rotary_emb`, `causal_depthwise_conv_silu`, `l2norm_last`, `repeat_kv`, the recurrence) are free functions / `impl` methods over `Tensor` with no qlora-rs references — pure copy.
- **CandleModel / training plumbing** (`model.rs:516-545`) is irrelevant to the forward and is itself a stub.

Medium (not Low) because: the Qwen3.5 linear-attention recurrence (`model.rs:284-414`) is intricate and numerically delicate (clamps, eps, gating) and must be ported faithfully and re-validated; and the top-level forward must gain real KV-cache + position threading (currently `pos=0`, no cache, `model.rs:495`) to be a usable decode loop. Not High because there is zero autograd/VarMap entanglement to unwind.

### 5. Recommendation detail

Choose **Option B**: re-implement a quantized-only forward in `vox-inference`, parameterizing linears over candle `QMatMul`. Sketch of the minimal surface (whether kept in `vox-inference` or later promoted to L2 `vox-model-qwen`):

```rust
// vox:skip — design sketch, not a compilable excerpt
pub struct QwenConfig { /* hidden_size, n_layers, heads, kv_heads, head_dim,
                           layer_types, rope_theta, linear-attn dims, vocab */ }
impl QwenConfig { pub fn from_hf_layout(layout: &HfTransformerLayout) -> Self; }

pub struct QwenForward { /* embed_tokens: Tensor, layers (QMatMul linears + raw Tensors
                            + RmsNorm), norm, lm_head: QMatMul, caches */ }
impl QwenForward {
    // weights: a name->QTensor/Tensor map (the get_tensor closure from inference.rs:119)
    pub fn new(cfg: &QwenConfig, weights: &dyn WeightSource, dev: &Device) -> Result<Self>;
    pub fn forward(&mut self, tokens: &Tensor, pos: usize) -> Result<Tensor>; // logits
}
```

- **Reuse from the plugin**: the pure-math helpers verbatim (`rotate_half`, `repeat_kv`, `causal_mask`, `apply_rotary_emb`, `causal_depthwise_conv_silu`, `l2norm_last`, `repeat_heads_bshd`, the delta recurrence, MLP/attention/layer body) and the HF key naming (`hf_keymap.rs`) and layout parsing (`hf_layout`). These are already qlora-rs-free.
- **Replace**: every `QuantizedLinear` field/`.forward(...)` call -> `QMatMul` field/`.forward(...)`. Construction switches from `QuantizedLinear::from_weight(weight, None, &qlora_cfg, dev)` (`inference.rs:185`...) to `QMatMul::from_qtensor` (quantized) or an f32 `QMatMul` (passthrough). LoRA must be pre-merged into the base weights upstream (SP-1/loader), since `QMatMul` carries no adapter.
- **Plugin keeps**: its training path and `QuantizedLinear`-based model unchanged (it still trains LoRA). Long-term the plugin's *inference* path could call the new forward, but that rewire is out of scope for SP-2.

### 6. Risks / unknowns blocking Task 3

1. **LoRA merge**: does SP-1's loader produce a base-merged f32/quantized weight, or are adapters still separate at inference time? `QMatMul` cannot apply LoRA; if adapters must stay separate, the new forward needs an explicit merge step or a residual LoRA path (re-introducing some qlora-rs-shaped logic). Confirm the artifact format first.
2. **Quantization format mismatch**: existing checkpoints quantize via NF4 inside `QuantizedLinear`; `QMatMul` expects candle GGUF `QTensor`. Decide whether SP-2 inference consumes f32 safetensors (simplest, matches `inference.rs` today) or a candle-quantized artifact — and who does the candle quantization.
3. **Numerical parity**: the Qwen3.5 delta-net recurrence and the clamps/eps (`model.rs:123,127,131,342,346,498`) must reproduce the plugin's outputs; needs a golden-logits test vs the plugin before Task 3 is "done."
4. **KV-cache + position correctness**: the top-level forward currently passes `pos=0`/`None` (`model.rs:495`); a real decode loop must thread per-layer caches and RoPE positions, including the mixed full/linear layer cache enum. Unvalidated path today.
5. **L2 placement vs arch rules**: if promoting to `vox-model-qwen` (L2), confirm `candle-core`/`candle-nn` are L2-permissible deps and the LoC budget fits; `vox-arch-check` must pass. Defer until a 2nd consumer justifies it.
6. **tokenizer/EOS**: hardcoded EOS `151643` (`inference.rs:428`) and tokenizer loading are plugin-side; `vox-inference` needs its own tokenizer wiring (likely SP-1 scope).
