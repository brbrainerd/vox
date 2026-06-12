//! CandleModel: wrapper holding a loaded Qwen3.5 model for use by the MlBackend trait.
//!
//! The transformer block implementation (Qwen2Attention, Qwen35LinearAttention, etc.) is
//! copied verbatim from `vox-populi`'s `candle_model_qwen` module. In a future cleanup
//! pass (SP6+) this should be extracted into a shared `vox-candle-models` crate so there
//! is a single canonical copy.
//!
//! # SP3 extraction status
//!
//! The forward/backward training code in `vox-populi` is deeply tangled with vox-populi
//! types (`LoraTrainingConfig`, `QloraEmbedBundle`, `TrainingPair`, `CheckpointState`,
//! `vox_tensor`, `vox_secrets`, VoxDB async channel, etc.). Untangling into the plugin's
//! `training.rs` / `checkpoint.rs` is deferred to a follow-up commit; those modules
//! currently contain stubs that return `Err("not yet wired")`.
//!
//! `load_from_path` is also stubbed: the real implementation requires `QloraEmbedBundle`
//! (preflight logic that reads HF `config.json` and locates safetensors shards), which
//! lives in vox-populi. Batch 3 wires vox-populi to consume this plugin through the host;
//! at that point, the plugin can receive the already-loaded `Qwen35Model` via a serialized
//! handle rather than needing to replicate the full preflight.
//!
//! # TODO (batch 4): add `unload_model` verb to `MlBackend` trait to free the boxed
//! `CandleModel` and avoid the current memory leak on plugin unload.

use candle_core::{DType, Device, Result, Tensor};
use candle_nn::RmsNorm;
use qlora_rs::qlora::QuantizedLinear;

// ── Helpers ──────────────────────────────────────────────────────────────────

fn causal_mask(seq_len: usize, device: &Device) -> Result<Tensor> {
    let mut data = vec![0.0f32; seq_len * seq_len];
    for row in 0..seq_len {
        for col in (row + 1)..seq_len {
            data[row * seq_len + col] = f32::NEG_INFINITY;
        }
    }
    Tensor::from_vec(data, (1, 1, seq_len, seq_len), device)
}

/// Differentiable RMSNorm with an F32-stable reduction that preserves the activation
/// dtype. candle's `RmsNorm::forward_diff` upcasts BF16/F16 to F32 for the variance, but
/// casts the normalized result back to the *input* dtype before multiplying by the (F32)
/// norm weight — which would mix dtypes on the BF16-activation path. We sidestep that by
/// running `forward_diff` in F32 (norm weights are F32) and casting the result back to the
/// input's activation dtype. On the all-F32 path both casts are no-ops.
fn rms_norm_f32(norm: &RmsNorm, x: &Tensor) -> Result<Tensor> {
    let in_dtype = x.dtype();
    if in_dtype == DType::F32 {
        return norm.forward_diff(x);
    }
    norm.forward_diff(&x.to_dtype(DType::F32)?)?
        .to_dtype(in_dtype)
}

fn repeat_kv(x: &Tensor, n_rep: usize) -> Result<Tensor> {
    if n_rep == 1 {
        return Ok(x.clone());
    }
    let (b, n_kv, seq, hd) = x.dims4()?;
    x.unsqueeze(2)?
        .expand((b, n_kv, n_rep, seq, hd))?
        .reshape((b, n_kv * n_rep, seq, hd))
}

fn rotate_half(x: &Tensor) -> Result<Tensor> {
    let last_dim = x.dim(candle_core::D::Minus1)?;
    let x1 = x.narrow(candle_core::D::Minus1, 0, last_dim / 2)?;
    let x2 = x.narrow(candle_core::D::Minus1, last_dim / 2, last_dim / 2)?;
    Tensor::cat(&[&x2.neg()?, &x1], candle_core::D::Minus1)
}

// ── Attention ─────────────────────────────────────────────────────────────────

pub struct Qwen2Attention {
    pub q_proj: QuantizedLinear,
    pub k_proj: QuantizedLinear,
    pub v_proj: QuantizedLinear,
    pub o_proj: QuantizedLinear,
    /// Qwen2/Qwen2.5 use additive biases on the q/k/v projections. Omitting them
    /// makes the forward subtly wrong (the model — and any adapter trained against
    /// it — only matches a bias-less engine, not standard Qwen2). `None` for
    /// architectures without qkv bias.
    pub q_bias: Option<Tensor>,
    pub k_bias: Option<Tensor>,
    pub v_bias: Option<Tensor>,
    pub n_heads: usize,
    pub n_kv_heads: usize,
    pub head_dim: usize,
}

impl Qwen2Attention {
    pub fn forward(
        &self,
        x: &Tensor,
        pos: usize,
        inv_freq: Option<&Tensor>,
        kv_cache: Option<&mut (Tensor, Tensor)>,
    ) -> Result<Tensor> {
        let (b, seq_len, _d_model) = x.dims3()?;
        let device = x.device();

        let q = self
            .q_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        let k = self
            .k_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        let v = self
            .v_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;

        // Qwen2/Qwen2.5 additive qkv biases (broadcast over [b, seq, out_features]).
        // The activation dtype follows the configured compute dtype (BF16 on CUDA,
        // F32 on CPU/tests). Biases are loaded F32; cast them to the activation dtype
        // at point-of-use so the broadcast_add never mixes dtypes (no-op on the F32 path).
        let act_dtype = q.dtype();
        let q = match &self.q_bias {
            Some(bias) => q.broadcast_add(&bias.to_dtype(act_dtype)?)?,
            None => q,
        };
        let k = match &self.k_bias {
            Some(bias) => k.broadcast_add(&bias.to_dtype(act_dtype)?)?,
            None => k,
        };
        let v = match &self.v_bias {
            Some(bias) => v.broadcast_add(&bias.to_dtype(act_dtype)?)?,
            None => v,
        };

        let q = q
            .reshape((b, seq_len, self.n_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((b, seq_len, self.n_kv_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((b, seq_len, self.n_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        let (q, k) = if let Some(inv_freq) = inv_freq {
            self.apply_rotary_emb(&q, &k, inv_freq, pos)?
        } else {
            (q, k)
        };

        let (k, v) = if let Some((k_prev, v_prev)) = kv_cache {
            let k = Tensor::cat(&[&*k_prev, &k], 2)?;
            let v = Tensor::cat(&[&*v_prev, &v], 2)?;
            *k_prev = k.clone();
            *v_prev = v.clone();
            (k, v)
        } else {
            (k, v)
        };

        let n_rep = self.n_heads / self.n_kv_heads;
        let k = repeat_kv(&k, n_rep)?;
        let v = repeat_kv(&v, n_rep)?;
        let v = v.clamp(-256f64, 256f64)?;

        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let mut att = (q.contiguous()?.matmul(&k.transpose(2, 3)?.contiguous()?)? * scale)?;
        att = att.clamp(-120f64, 120f64)?;

        // Softmax (and the surrounding max-subtraction / mask add) accumulate in F32
        // for numerical stability regardless of the activation dtype, then cast back to
        // the activation dtype for the att @ v matmul. On the all-F32 path these casts
        // are no-ops; on the BF16 path they keep the attention probabilities stable.
        let act_dtype = q.dtype();
        if seq_len > 1 {
            let att = att.to_dtype(DType::F32)?;
            let att_max = att.max_keepdim(candle_core::D::Minus1)?;
            let att = att.broadcast_sub(&att_max)?;
            let mask = causal_mask(seq_len, device)?;
            let att = att.broadcast_add(&mask)?;
            let att = candle_nn::ops::softmax(&att, candle_core::D::Minus1)?;
            let att = att.to_dtype(act_dtype)?;
            let y = att.matmul(&v.contiguous()?)?;
            let y = y.transpose(1, 2)?.contiguous()?.reshape((
                b,
                seq_len,
                self.n_heads * self.head_dim,
            ))?;
            self.o_proj
                .forward(&y)
                .map_err(|e| candle_core::Error::Msg(e.to_string()))
        } else {
            let att = candle_nn::ops::softmax(&att.to_dtype(DType::F32)?, candle_core::D::Minus1)?
                .to_dtype(act_dtype)?;
            let y = att.matmul(&v.contiguous()?)?;
            let y = y.transpose(1, 2)?.contiguous()?.reshape((
                b,
                seq_len,
                self.n_heads * self.head_dim,
            ))?;
            self.o_proj
                .forward(&y)
                .map_err(|e| candle_core::Error::Msg(e.to_string()))
        }
    }

    fn apply_rotary_emb(
        &self,
        q: &Tensor,
        k: &Tensor,
        inv_freq: &Tensor,
        pos: usize,
    ) -> Result<(Tensor, Tensor)> {
        let (_b, _n_heads, seq_len, head_dim) = q.dims4()?;
        let rope_dim = inv_freq.elem_count().saturating_mul(2);
        if rope_dim == 0 || rope_dim > head_dim {
            return Err(candle_core::Error::Msg(format!(
                "RoPE inv_freq length inconsistent with head_dim: inv_freq_elems={} head_dim={head_dim}",
                inv_freq.elem_count()
            )));
        }
        let device = q.device();
        let t = Tensor::arange(pos as u32, (pos + seq_len) as u32, device)?
            .to_dtype(DType::F32)?
            .reshape((seq_len, 1))?;
        let freqs = t.matmul(&inv_freq.reshape((1, inv_freq.elem_count()))?)?;
        let freqs = Tensor::cat(&[&freqs, &freqs], 1)?;
        // cos/sin are computed in F32 for precision, then cast to the activation dtype
        // (q/k) so the broadcast_mul below never mixes dtypes (no-op on the F32 path).
        let act_dtype = q.dtype();
        let cos = freqs
            .cos()?
            .reshape((1, 1, seq_len, rope_dim))?
            .to_dtype(act_dtype)?;
        let sin = freqs
            .sin()?
            .reshape((1, 1, seq_len, rope_dim))?
            .to_dtype(act_dtype)?;
        if rope_dim == head_dim {
            let q_embed = (q.broadcast_mul(&cos)? + rotate_half(q)?.broadcast_mul(&sin)?)?;
            let k_embed = (k.broadcast_mul(&cos)? + rotate_half(k)?.broadcast_mul(&sin)?)?;
            Ok((q_embed, k_embed))
        } else {
            let q_rot = q.narrow(candle_core::D::Minus1, 0, rope_dim)?;
            let q_pass = q.narrow(candle_core::D::Minus1, rope_dim, head_dim - rope_dim)?;
            let k_rot = k.narrow(candle_core::D::Minus1, 0, rope_dim)?;
            let k_pass = k.narrow(candle_core::D::Minus1, rope_dim, head_dim - rope_dim)?;
            let q_r = (q_rot.broadcast_mul(&cos)? + rotate_half(&q_rot)?.broadcast_mul(&sin)?)?;
            let k_r = (k_rot.broadcast_mul(&cos)? + rotate_half(&k_rot)?.broadcast_mul(&sin)?)?;
            let q_embed = Tensor::cat(&[&q_r, &q_pass], candle_core::D::Minus1)?;
            let k_embed = Tensor::cat(&[&k_r, &k_pass], candle_core::D::Minus1)?;
            Ok((q_embed, k_embed))
        }
    }
}

// ── MLP ───────────────────────────────────────────────────────────────────────

pub struct Qwen2MLP {
    pub gate_proj: QuantizedLinear,
    pub up_proj: QuantizedLinear,
    pub down_proj: QuantizedLinear,
}

impl Qwen2MLP {
    pub fn forward(&self, x: &Tensor) -> Result<Tensor> {
        let lhs = candle_nn::ops::silu(
            &self
                .gate_proj
                .forward(x)
                .map_err(|e| candle_core::Error::Msg(e.to_string()))?,
        )?;
        let rhs = self
            .up_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        self.down_proj
            .forward(&(lhs * rhs)?)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))
    }
}

// ── Qwen3.5 hybrid attention ──────────────────────────────────────────────────

pub struct Qwen35LinearAttention {
    pub qkv_proj: QuantizedLinear,
    pub z_proj: QuantizedLinear,
    pub b_proj: QuantizedLinear,
    pub a_proj: QuantizedLinear,
    pub out_proj: QuantizedLinear,
    pub conv_weight: Tensor,
    pub dt_bias: Tensor,
    pub a_log: Tensor,
    pub norm: RmsNorm,
    pub num_k_heads: usize,
    pub num_v_heads: usize,
    pub head_k_dim: usize,
    pub head_v_dim: usize,
}

impl Qwen35LinearAttention {
    fn repeat_heads_bshd(x: &Tensor, n_rep: usize) -> Result<Tensor> {
        if n_rep == 1 {
            return Ok(x.clone());
        }
        let (b, s, h, d) = x.dims4()?;
        x.unsqueeze(3)?
            .expand((b, s, h, n_rep, d))?
            .reshape((b, s, h * n_rep, d))
    }

    fn l2norm_last(x: &Tensor, eps: f64) -> Result<Tensor> {
        let d = x.dim(candle_core::D::Minus1)?;
        let sq = x.broadcast_mul(x)?;
        let sq = sq.sum_keepdim(candle_core::D::Minus1)?;
        let inv = (sq / (d as f64))?.broadcast_add(&Tensor::new(eps as f32, x.device())?)?;
        let inv = inv.sqrt()?.recip()?;
        x.broadcast_mul(&inv)
    }

    fn causal_depthwise_conv_silu(x: &Tensor, conv_weight: &Tensor) -> Result<Tensor> {
        let (b, s, c) = x.dims3()?;
        let k = conv_weight.dim(1)?;
        let dev = x.device();
        // The depthwise conv accumulates in F32 for stability and returns F32; the caller
        // (linear-attention) runs its recurrence in F32 anyway. Cast inputs to F32 so the
        // BF16-activation path doesn't mix dtypes (no-op on the all-F32 path).
        let x = x.to_dtype(DType::F32)?;
        let conv_weight = conv_weight.to_dtype(DType::F32)?;
        let mut steps = Vec::with_capacity(s);
        for t in 0..s {
            let mut acc = Tensor::zeros((b, c), DType::F32, dev)?;
            for j in 0..k {
                if t < j {
                    continue;
                }
                let x_t = x.narrow(1, t - j, 1)?.squeeze(1)?;
                let w = conv_weight.narrow(1, j, 1)?.squeeze(1)?;
                let prod = x_t.broadcast_mul(&w.unsqueeze(0)?)?;
                acc = (acc + prod)?;
            }
            steps.push(candle_nn::ops::silu(&acc)?);
        }
        Tensor::stack(&steps, 1)
    }

    pub fn forward(
        &self,
        x: &Tensor,
        pos: usize,
        inv_freq: Option<&Tensor>,
        state_cache: Option<&mut Tensor>,
    ) -> Result<Tensor> {
        let (b, seq_len, _d_model) = x.dims3()?;
        let device = x.device();
        let qkv = self
            .qkv_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        let mixed_qkv = Self::causal_depthwise_conv_silu(&qkv, &self.conv_weight)?;

        let key_dim = self.num_k_heads * self.head_k_dim;
        let value_dim = self.num_v_heads * self.head_v_dim;
        let expected_total = key_dim + key_dim + value_dim;
        let got_total = mixed_qkv.dim(candle_core::D::Minus1)?;
        if got_total != expected_total {
            return Err(candle_core::Error::Msg(format!(
                "qwen3_5 linear_attention qkv dim mismatch: expected {expected_total}, got {got_total}",
            )));
        }

        // The gated-delta-net recurrence below carries an F32 state and many F32
        // constants (g, dt_bias, a_log). Run the whole linear-attention block in F32 for
        // numerical stability regardless of the activation dtype, then cast the gated
        // output back to the activation dtype before out_proj. On the all-F32 path these
        // casts are no-ops; on the BF16 path they avoid mixing BF16 activations with the
        // F32 recurrence state (and keep the sequential state-decay numerically sound).
        let act_dtype = x.dtype();
        let query = mixed_qkv
            .narrow(candle_core::D::Minus1, 0, key_dim)?
            .reshape((b, seq_len, self.num_k_heads, self.head_k_dim))?
            .to_dtype(DType::F32)?;
        let key = mixed_qkv
            .narrow(candle_core::D::Minus1, key_dim, key_dim)?
            .reshape((b, seq_len, self.num_k_heads, self.head_k_dim))?
            .to_dtype(DType::F32)?;
        let value = mixed_qkv
            .narrow(candle_core::D::Minus1, key_dim + key_dim, value_dim)?
            .reshape((b, seq_len, self.num_v_heads, self.head_v_dim))?
            .to_dtype(DType::F32)?;

        let z = self
            .z_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?
            .reshape((b, seq_len, self.num_v_heads, self.head_v_dim))?
            .to_dtype(DType::F32)?;
        let beta = candle_nn::ops::sigmoid(
            &self
                .b_proj
                .forward(x)
                .map_err(|e| candle_core::Error::Msg(e.to_string()))?,
        )?
        .reshape((b, seq_len, self.num_v_heads))?
        .to_dtype(DType::F32)?;
        let a = self
            .a_proj
            .forward(x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?
            .reshape((b, seq_len, self.num_v_heads))?
            .to_dtype(DType::F32)?;

        let a_log = self.a_log.to_dtype(DType::F32)?;
        let dt_bias = self.dt_bias.to_dtype(DType::F32)?;
        let g_pre =
            (a.broadcast_add(&dt_bias.reshape((1, 1, self.num_v_heads))?)?).to_dtype(DType::F32)?;
        let g_soft = (g_pre.exp()?.broadcast_add(&Tensor::new(1f32, device)?)?).log()?;
        let a_log_scale = a_log.exp()?.clamp(1e-6f64, 1e4f64)?;
        let g = g_soft
            .broadcast_mul(&a_log_scale.reshape((1, 1, self.num_v_heads))?)?
            .neg()?;
        let g = g.clamp(-80f64, 20f64)?;

        let mut query = Self::l2norm_last(&query, 1e-6)?;
        let mut key = Self::l2norm_last(&key, 1e-6)?;
        if self.num_v_heads > self.num_k_heads {
            let rep = self.num_v_heads / self.num_k_heads;
            query = Self::repeat_heads_bshd(&query, rep)?;
            key = Self::repeat_heads_bshd(&key, rep)?;
        }

        let mut state = if let Some(state_prev) = state_cache.as_ref() {
            (**state_prev).clone()
        } else {
            Tensor::zeros(
                (b, self.num_v_heads, self.head_k_dim, self.head_v_dim),
                DType::F32,
                device,
            )?
        };
        let mut outs = Vec::with_capacity(seq_len);
        for t in 0..seq_len {
            let q_t = query.narrow(1, t, 1)?.squeeze(1)?;
            let k_t = key.narrow(1, t, 1)?.squeeze(1)?;
            let v_t = value.narrow(1, t, 1)?.squeeze(1)?;
            let g_t = g.narrow(1, t, 1)?.squeeze(1)?;
            let beta_t = beta.narrow(1, t, 1)?.squeeze(1)?;

            let g_scale = g_t.exp()?.reshape((b, self.num_v_heads, 1, 1))?;
            state = state.broadcast_mul(&g_scale)?;

            let k_col = k_t.unsqueeze(candle_core::D::Minus1)?;
            let kv_mem = state
                .transpose(2, 3)?
                .contiguous()?
                .matmul(&k_col)?
                .squeeze(candle_core::D::Minus1)?;
            let delta = v_t
                .broadcast_sub(&kv_mem)?
                .broadcast_mul(&beta_t.unsqueeze(candle_core::D::Minus1)?)?;
            let delta_row = delta.unsqueeze(2)?;
            let upd = k_col.matmul(&delta_row)?;
            state = (state + upd)?;

            let out_t = state
                .transpose(2, 3)?
                .contiguous()?
                .matmul(&q_t.unsqueeze(candle_core::D::Minus1)?)?
                .squeeze(candle_core::D::Minus1)?;
            outs.push(out_t);
        }

        if let Some(state_prev) = state_cache {
            *state_prev = state.clone();
        }

        let mut y = Tensor::stack(&outs, 1)?;
        if let Some(inv_freq) = inv_freq {
            let _ = (inv_freq, pos);
        }
        let y_flat = y.reshape((b * seq_len * self.num_v_heads, self.head_v_dim))?;
        let z_flat = z.reshape((b * seq_len * self.num_v_heads, self.head_v_dim))?;
        // forward_diff = differentiable (composed) RMSNorm; the Module `forward` uses a
        // fused apply_op2_no_bwd kernel with NO backward, which silently severs gradient
        // flow through the norm (only post-norm params would train). Same numerics.
        // y_flat is already F32 here; norm weights are F32, so this normalizes in F32.
        let y_norm = self.norm.forward_diff(&y_flat)?;
        let y_gate = y_norm.broadcast_mul(&candle_nn::ops::silu(&z_flat)?)?;
        y = y_gate
            .reshape((b, seq_len, value_dim))?
            .to_dtype(act_dtype)?;

        self.out_proj
            .forward(&y)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))
    }
}

#[allow(clippy::large_enum_variant)]
pub enum Qwen35AttentionBlock {
    Full(Qwen2Attention),
    Linear(Qwen35LinearAttention),
}

pub struct Qwen35Layer {
    pub input_layernorm: RmsNorm,
    pub attention: Qwen35AttentionBlock,
    pub post_attention_layernorm: RmsNorm,
    pub mlp: Qwen2MLP,
    pub inv_freq: Option<Tensor>,
}

impl Qwen35Layer {
    pub fn forward(
        &self,
        x: &Tensor,
        pos: usize,
        kv_cache: Option<&mut Qwen35LayerCache>,
    ) -> Result<Tensor> {
        let residual = x;
        let h = rms_norm_f32(&self.input_layernorm, x)?;
        let h = match &self.attention {
            Qwen35AttentionBlock::Full(a) => {
                let cache = match kv_cache {
                    Some(Qwen35LayerCache::Full(kv)) => Some(kv),
                    Some(Qwen35LayerCache::Linear(_)) => {
                        return Err(candle_core::Error::Msg(
                            "qwen3_5 cache mismatch: full-attention layer received linear cache"
                                .to_string(),
                        ));
                    }
                    None => None,
                };
                a.forward(&h, pos, self.inv_freq.as_ref(), cache)?
            }
            Qwen35AttentionBlock::Linear(a) => {
                let cache = match kv_cache {
                    Some(Qwen35LayerCache::Linear(state)) => Some(state),
                    Some(Qwen35LayerCache::Full(_)) => {
                        return Err(candle_core::Error::Msg(
                            "qwen3_5 cache mismatch: linear-attention layer received KV cache"
                                .to_string(),
                        ));
                    }
                    None => None,
                };
                a.forward(&h, pos, self.inv_freq.as_ref(), cache)?
            }
        };
        let x = (residual + h)?;

        let residual = &x;
        let h = rms_norm_f32(&self.post_attention_layernorm, &x)?;
        let h = self.mlp.forward(&h)?;
        residual + h
    }
}

pub struct Qwen35Model {
    pub embed_tokens: Tensor,
    pub layers: Vec<Qwen35Layer>,
    pub norm: RmsNorm,
    pub lm_head: QuantizedLinear,
}

/// One checkpoint segment: a contiguous range of transformer layers
/// `[start_layer, end_layer)` whose **input** activation was severed from the
/// autograd tape at the segment boundary so only one segment's activations live
/// on the tape at a time during the eventual backward.
///
/// The boundary is materialized as a fresh, storage-independent leaf
/// ([`Tensor::copy`]'s deep copy, then detached) — **not** a bare
/// [`Tensor::detach`], which shares storage with the still-live grad-tracked
/// producer and (candle 0.9 quirk, see `forward_checkpointed`) yields a *wrong*
/// input gradient at the boundary. Storage independence is load-bearing for
/// gradient correctness.
pub struct CheckpointSegment {
    /// Input activation `[b, seq, d_model]` at this segment's boundary, as an
    /// independent detached leaf. During the live forward this is the tensor the
    /// next segment consumed; during recompute it is re-wrapped in a `Var` so its
    /// gradient (the cotangent to thread back) can be read.
    pub input: Tensor,
    /// First layer index (inclusive) in this segment.
    pub start_layer: usize,
    /// One-past-last layer index (exclusive) in this segment.
    pub end_layer: usize,
}

/// Materialize `x`'s values into a fresh, storage-independent detached leaf.
///
/// `Tensor::detach` shares storage with its source; when the source is a live
/// grad-tracked tensor, feeding that shared-storage detach as a segment boundary
/// makes candle 0.9 compute the **wrong** input gradient at the boundary (the
/// gradient is silently attenuated — verified against the eager full-graph
/// backward). Deep-copying first (`make_var` allocates new storage) and then
/// detaching gives a clean, independent leaf whose boundary gradient matches the
/// eager reference exactly.
fn independent_boundary(x: &Tensor) -> Result<Tensor> {
    // Var::from_tensor deep-copies a non-variable tensor into fresh storage (a new
    // leaf whose gradient candle records correctly). We first detach to get a
    // plain (non-variable) source, then wrap as a Var — this both severs the tape
    // AND breaks storage aliasing with the live grad-tracked producer. A bare
    // detach (shared storage) silently produces a WRONG boundary gradient in
    // candle 0.9. The boundary being a Var is harmless: it is not in the trainer's
    // VarMap, so the optimizer never touches it; backward still records its grad.
    let detached = x.detach();
    let var = candle_core::Var::from_tensor(&detached)?;
    Ok(var.as_tensor().clone())
}

/// Result of a checkpointed forward pass. `logits`'s autograd tape spans **only
/// the final segment + head**; `segments` carries the detached boundary
/// activations needed to recompute-and-backward earlier segments in reverse.
pub struct CheckpointedForward {
    /// Logits `[b, seq, vocab]`, F32. Tape spans only the last segment + head.
    pub logits: Tensor,
    /// Segments in forward order. The last entry's range ends at `num_layers`.
    pub segments: Vec<CheckpointSegment>,
}

impl Qwen35Model {
    pub fn forward(&self, input_ids: &Tensor) -> Result<Tensor> {
        let (b, seq_len) = input_ids.dims2()?;
        let d_model = self.embed_tokens.dim(1)?;
        let ids = input_ids.flatten_all()?;
        let mut x = self
            .embed_tokens
            .index_select(&ids, 0)?
            .reshape((b, seq_len, d_model))?;

        for layer in &self.layers {
            x = layer.forward(&x, 0, None)?;
        }
        self.head_forward(&x)
    }

    /// Final norm + clamp + lm_head + F32 cast, shared by the eager and
    /// checkpointed forward so logits are byte-identical for the same input.
    fn head_forward(&self, x: &Tensor) -> Result<Tensor> {
        let x = rms_norm_f32(&self.norm, x)?;
        let x = x.clamp(-64f64, 64f64)?;
        let logits = self
            .lm_head
            .forward(&x)
            .map_err(|e| candle_core::Error::Msg(e.to_string()))?;
        // Cast logits to F32 before they leave the model so the downstream
        // cross-entropy (log_softmax + masked gather in `forward_masked_ce`) runs
        // entirely in F32 regardless of the activation dtype. No-op on the F32 path.
        logits.to_dtype(DType::F32)
    }

    /// Run the transformer layers in `[start, end)` over `x`. Identical op
    /// sequence to [`Self::forward`]'s layer loop, so recompute reproduces the
    /// original numerics exactly (training has no KV cache / `pos` is always 0).
    fn run_layers(&self, x: &Tensor, start: usize, end: usize) -> Result<Tensor> {
        let mut h = x.clone();
        for layer in &self.layers[start..end] {
            h = layer.forward(&h, 0, None)?;
        }
        Ok(h)
    }

    /// Checkpointed forward: split the `num_layers` transformer layers into
    /// `n_segments` roughly-equal contiguous segments, detaching the activation
    /// at each boundary so the autograd tape never spans more than one segment +
    /// the head. This bounds the single-backward VRAM peak that OOMs 3B on 16GB.
    ///
    /// `n_segments` is clamped to `[1, num_layers]`.
    pub fn forward_checkpointed(
        &self,
        input_ids: &Tensor,
        n_segments: usize,
    ) -> Result<CheckpointedForward> {
        let num_layers = self.layers.len();
        let n_segments = n_segments.clamp(1, num_layers.max(1));

        let (b, seq_len) = input_ids.dims2()?;
        let d_model = self.embed_tokens.dim(1)?;
        let ids = input_ids.flatten_all()?;
        let embed = self
            .embed_tokens
            .index_select(&ids, 0)?
            .reshape((b, seq_len, d_model))?;

        // Even split of layers across segments (remainder to the front segments).
        let base = num_layers / n_segments;
        let rem = num_layers % n_segments;
        let mut bounds = Vec::with_capacity(n_segments + 1);
        bounds.push(0usize);
        let mut acc = 0usize;
        for s in 0..n_segments {
            acc += base + usize::from(s < rem);
            bounds.push(acc);
        }

        let mut segments = Vec::with_capacity(n_segments);
        // First segment input is the embedding as an independent detached leaf
        // (embeddings are frozen — no grad needed through them).
        let mut seg_input = independent_boundary(&embed)?;
        let mut logits = None;
        for s in 0..n_segments {
            let start = bounds[s];
            let end = bounds[s + 1];
            let seg_out = self.run_layers(&seg_input, start, end)?;
            segments.push(CheckpointSegment {
                input: seg_input.clone(),
                start_layer: start,
                end_layer: end,
            });
            if s + 1 == n_segments {
                // Last segment flows into the head WITHOUT severing.
                logits = Some(self.head_forward(&seg_out)?);
            } else {
                // Sever the boundary into a fresh, storage-independent leaf so the
                // next segment's backward computes a correct boundary gradient.
                seg_input = independent_boundary(&seg_out)?;
            }
        }

        Ok(CheckpointedForward {
            logits: logits.expect("at least one segment"),
            segments,
        })
    }

    /// Recompute one checkpoint segment's forward from its detached boundary
    /// `input` with the tape live, returning `(input_var, seg_output)`. After
    /// backprop, `input_var`'s gradient is `dL/d(input)` — the cotangent to
    /// thread to the previous segment.
    pub fn recompute_segment(&self, seg: &CheckpointSegment) -> Result<(candle_core::Var, Tensor)> {
        let input_var = candle_core::Var::from_tensor(&seg.input)?;
        let out = self.run_layers(input_var.as_tensor(), seg.start_layer, seg.end_layer)?;
        Ok((input_var, out))
    }
}

pub enum Qwen35LayerCache {
    Full((Tensor, Tensor)),
    Linear(Tensor),
}

// ── CandleModel: the opaque handle stored across plugin calls ─────────────────

/// Opaque model handle stored by the plugin. In the current SP3 stub, `load_from_path`
/// returns an error — the actual construction requires `QloraEmbedBundle` from
/// vox-populi's preflight logic. Batch 3 wires vox-populi to construct the model and
/// pass it to the plugin via an alternative init path.
pub struct CandleModel {
    /// Eagerly-built model graph. Unused on the inference path — `run_inference`
    /// reloads a fresh `InferenceEngine` from `model_path` on every call — so it is
    /// left `None` for handles produced by [`Self::load_from_path`].
    pub _inner: Option<Qwen35Model>,
    /// Path to the model directory, stored so `run_inference` can reload the engine.
    pub model_path: String,
}

impl CandleModel {
    /// Open an inference-ready model directory.
    ///
    /// The handle only carries `model_path`; the actual model graph is rebuilt by
    /// [`crate::inference::run`] (via `InferenceEngine::load`) on each `run_inference`
    /// call, so no `QloraEmbedBundle` construction is needed here. We validate that the
    /// directory contains the artifacts `InferenceEngine::load` requires, failing early
    /// with an actionable message rather than deep inside the load path.
    pub fn load_from_path(model_path: &str) -> anyhow::Result<Self> {
        let dir = std::path::Path::new(model_path);
        if !dir.is_dir() {
            anyhow::bail!(
                "model path {model_path} is not a directory — pass the training run dir \
                 (containing candle_qlora_adapter.safetensors, adapter_manifest.json, \
                 tokenizer.json, config.json)"
            );
        }
        for required in [
            "candle_qlora_adapter.safetensors",
            "adapter_manifest.json",
            "tokenizer.json",
            "config.json",
        ] {
            if !dir.join(required).is_file() {
                anyhow::bail!(
                    "model dir {model_path} is missing {required} — run \
                     `vox mens merge-qlora` / finalize the run before inference"
                );
            }
        }
        Ok(Self {
            _inner: None,
            model_path: model_path.to_string(),
        })
    }
}

#[cfg(test)]
mod gradient_checkpoint_tests {
    //! End-to-end correctness for activation/gradient checkpointing on the real
    //! `Qwen35Model`: a checkpointed segmented backward must produce the **same**
    //! LoRA gradients as an eager full-graph backward. CPU/F32 so it runs in
    //! normal `cargo test` (no CUDA). A silently-wrong backward is the worst
    //! possible outcome, so this is the load-bearing safety gate.

    use super::*;
    use candle_core::Var;
    use candle_nn::{VarBuilder, VarMap};
    use qlora_rs::QLoraConfig;

    fn qcfg() -> QLoraConfig {
        let mut c = QLoraConfig::preset_all_bf16(4, 8);
        c.quantization.compute_dtype = qlora_rs::quantization::ComputeDType::F32;
        c.cache_dequantized = true;
        c
    }

    fn qlin(vb: VarBuilder, d_out: usize, d_in: usize, dev: &Device) -> QuantizedLinear {
        let w = Tensor::randn(0f32, 0.02f32, (d_out, d_in), dev).unwrap();
        QuantizedLinear::from_weight_with_varbuilder(&w, None, &qcfg(), vb).unwrap()
    }

    fn build_layer(vb: VarBuilder, d: usize, n_heads: usize, dev: &Device) -> Qwen35Layer {
        let head_dim = d / n_heads;
        let attn = Qwen2Attention {
            q_proj: qlin(vb.pp("q"), d, d, dev),
            k_proj: qlin(vb.pp("k"), d, d, dev),
            v_proj: qlin(vb.pp("v"), d, d, dev),
            o_proj: qlin(vb.pp("o"), d, d, dev),
            q_bias: None,
            k_bias: None,
            v_bias: None,
            n_heads,
            n_kv_heads: n_heads,
            head_dim,
        };
        let mlp = Qwen2MLP {
            gate_proj: qlin(vb.pp("g"), d * 2, d, dev),
            up_proj: qlin(vb.pp("u"), d * 2, d, dev),
            down_proj: qlin(vb.pp("dn"), d, d * 2, dev),
        };
        Qwen35Layer {
            input_layernorm: RmsNorm::new(Tensor::ones(d, DType::F32, dev).unwrap(), 1e-6),
            attention: Qwen35AttentionBlock::Full(attn),
            post_attention_layernorm: RmsNorm::new(Tensor::ones(d, DType::F32, dev).unwrap(), 1e-6),
            mlp,
            inv_freq: None,
        }
    }

    fn build_model(
        vb: VarBuilder,
        d: usize,
        n_layers: usize,
        vocab: usize,
        dev: &Device,
    ) -> Qwen35Model {
        let embed = Tensor::randn(0f32, 0.02f32, (vocab, d), dev).unwrap();
        let layers: Vec<Qwen35Layer> = (0..n_layers)
            .map(|i| build_layer(vb.pp(format!("layer{i}")), d, 2, dev))
            .collect();
        let lm_head = QuantizedLinear::from_weight_with_varbuilder(
            &embed.clone(),
            None,
            &qcfg(),
            vb.pp("lm_head"),
        )
        .unwrap();
        Qwen35Model {
            embed_tokens: embed,
            layers,
            norm: RmsNorm::new(Tensor::ones(d, DType::F32, dev).unwrap(), 1e-6),
            lm_head,
        }
    }

    fn ce_loss(logits: &Tensor, targets: &Tensor) -> Tensor {
        let logits = logits.flatten_to(1).unwrap();
        let log_sm = candle_nn::ops::log_softmax(&logits, 1).unwrap();
        let lp = log_sm
            .gather(&targets.flatten_all().unwrap().unsqueeze(1).unwrap(), 1)
            .unwrap();
        lp.neg().unwrap().mean_all().unwrap()
    }

    fn varmap_name(varmap: &VarMap, v: &Var) -> String {
        let data = varmap.data().lock().unwrap();
        for (k, vv) in data.iter() {
            if vv.as_tensor().id() == v.as_tensor().id() {
                return k.clone();
            }
        }
        "<unknown>".to_string()
    }

    #[test]
    fn checkpointed_model_backward_matches_eager_grads() {
        let dev = Device::Cpu;
        let (d, n_layers, vocab, seq) = (8usize, 4usize, 16usize, 5usize);
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &dev);
        let model = build_model(vb, d, n_layers, vocab, &dev);

        let input_ids = Tensor::from_vec(vec![1u32, 2, 3, 4, 5], (1, seq), &dev).unwrap();
        let targets = Tensor::from_vec(vec![2u32, 3, 4, 5, 6], (1, seq), &dev).unwrap();

        let lora_vars: Vec<Var> = varmap.all_vars();
        assert!(!lora_vars.is_empty(), "no LoRA vars registered");

        // ── Eager: full-graph backward ──────────────────────────────────────
        let logits_eager = model.forward(&input_ids).unwrap();
        let loss_eager = ce_loss(&logits_eager, &targets);
        let g_eager = loss_eager.backward().unwrap();

        // ── Checkpointed: 2 segments, segmented recompute backward ──────────
        let ckpt = model.forward_checkpointed(&input_ids, 2).unwrap();
        assert_eq!(ckpt.segments.len(), 2);

        // logits must match eager forward exactly (same ops, just severed tape).
        let le: Vec<f32> = logits_eager.flatten_all().unwrap().to_vec1().unwrap();
        let lc: Vec<f32> = ckpt.logits.flatten_all().unwrap().to_vec1().unwrap();
        for (a, b) in le.iter().zip(lc.iter()) {
            assert!(
                (a - b).abs() < 1e-4,
                "checkpointed logits diverged: {a} vs {b}"
            );
        }

        let loss_ck = ce_loss(&ckpt.logits, &targets);
        let mut grads = loss_ck.backward().unwrap();
        let last = ckpt.segments.last().unwrap();
        let mut upstream = grads.get(&last.input).cloned().unwrap();
        for seg in ckpt.segments.iter().rev().skip(1) {
            let (input_var, seg_out) = model.recompute_segment(seg).unwrap();
            let seg_grads = qlora_rs::backward_from_cotangent(&seg_out, &upstream).unwrap();
            qlora_rs::accumulate_grads_for_vars(&mut grads, &seg_grads, &lora_vars).unwrap();
            upstream = seg_grads.get(input_var.as_tensor()).cloned().unwrap();
        }

        // ── Compare per-Var grads ───────────────────────────────────────────
        let mut compared = 0usize;
        for v in &lora_vars {
            match (g_eager.get(v.as_tensor()), grads.get(v.as_tensor())) {
                (Some(a), Some(b)) => {
                    let name = varmap_name(&varmap, v);
                    let av: Vec<f32> = a.flatten_all().unwrap().to_vec1().unwrap();
                    let bv: Vec<f32> = b.flatten_all().unwrap().to_vec1().unwrap();
                    for (x, y) in av.iter().zip(bv.iter()) {
                        let denom = x.abs().max(1e-3);
                        assert!(
                            (x - y).abs() / denom < 1e-2,
                            "grad mismatch for {name}: eager={x} ckpt={y}"
                        );
                    }
                    compared += 1;
                }
                (None, None) => {}
                (a, b) => panic!(
                    "grad presence mismatch: eager={} ckpt={}",
                    a.is_some(),
                    b.is_some()
                ),
            }
        }
        assert!(
            compared >= n_layers,
            "expected grads for >= {n_layers} vars, got {compared}"
        );
    }
}

#[cfg(test)]
mod bf16_activation_tests {
    //! Tests for the "BF16 bundle": activations follow the configured compute dtype so
    //! the forward stack runs in BF16 on CUDA (halving activation VRAM) while staying F32
    //! on CPU (where BF16 matmul is unsupported in this Candle build). The key property is
    //! that the model is now *dtype-following*: a forward pass on a BF16 activation stream
    //! produces a BF16 output (no silent cast back to F32 by the qlora base matmul) AND
    //! stays numerically finite, while the F32 path is byte-for-byte preserved.

    use super::*;
    use qlora_rs::QLoraConfig;
    use qlora_rs::qlora::QuantizedLinear;
    use qlora_rs::quantization::ComputeDType;

    /// Build a tiny single-head `Qwen2Attention` whose base weights are quantized at the
    /// given compute dtype.
    fn tiny_attention(device: &Device, compute: ComputeDType) -> Qwen2Attention {
        let d = 8usize; // d_model == n_heads * head_dim
        let mut cfg = QLoraConfig::preset_all_bf16(4, 8);
        cfg.quantization.compute_dtype = compute;
        cfg.cache_dequantized = false;
        let w = Tensor::randn(0f32, 0.02f32, (d, d), device).unwrap();
        let mk = || QuantizedLinear::from_weight(&w, None, &cfg, device).unwrap();
        Qwen2Attention {
            q_proj: mk(),
            k_proj: mk(),
            v_proj: mk(),
            o_proj: mk(),
            // F32 biases on purpose: the forward must cast them to the activation dtype.
            q_bias: Some(Tensor::zeros(d, DType::F32, device).unwrap()),
            k_bias: Some(Tensor::zeros(d, DType::F32, device).unwrap()),
            v_bias: Some(Tensor::zeros(d, DType::F32, device).unwrap()),
            n_heads: 2,
            n_kv_heads: 2,
            head_dim: 4,
        }
    }

    /// CPU path: F32 activations in → F32 out, finite. This must NOT regress (BF16 matmul
    /// is unsupported on CPU, so the F32 path is what every CPU unit test exercises).
    #[test]
    fn f32_activations_preserved_on_cpu() {
        let device = Device::Cpu;
        let attn = tiny_attention(&device, ComputeDType::F32);
        // [batch=1, seq=3, d_model=8]
        let x = Tensor::randn(0f32, 1f32, (1, 3, 8), &device).unwrap();
        let out = attn.forward(&x, 0, None, None).unwrap();
        assert_eq!(out.dtype(), DType::F32, "F32 activation path must stay F32");
        let v = out.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        assert!(
            v.iter().all(|f| f.is_finite()),
            "F32 forward must be finite"
        );
    }

    /// `rms_norm_f32` is the numerically-stable RMSNorm wrapper: it preserves the input
    /// dtype but reduces in F32. On the F32 path it is exactly `forward_diff`.
    #[test]
    fn rms_norm_f32_preserves_dtype_and_matches_forward_diff_on_f32() {
        let device = Device::Cpu;
        let w = Tensor::ones(8, DType::F32, &device).unwrap();
        let norm = RmsNorm::new(w, 1e-6);
        let x = Tensor::randn(0f32, 1f32, (1, 3, 8), &device).unwrap();
        let a = rms_norm_f32(&norm, &x).unwrap();
        let b = norm.forward_diff(&x).unwrap();
        assert_eq!(a.dtype(), DType::F32);
        let (av, bv) = (
            a.flatten_all().unwrap().to_vec1::<f32>().unwrap(),
            b.flatten_all().unwrap().to_vec1::<f32>().unwrap(),
        );
        for (x, y) in av.iter().zip(bv.iter()) {
            assert!(
                (x - y).abs() < 1e-6,
                "rms_norm_f32 must match forward_diff on F32"
            );
        }
    }

    /// CUDA path: BF16 activations propagate through the attention block (the output is
    /// BF16, proving the qlora base matmul no longer casts back to F32) and stay finite.
    /// Skips gracefully when no CUDA device is available.
    #[cfg(feature = "cuda")]
    #[test]
    fn bf16_activations_propagate_and_finite_on_cuda() {
        let device = match Device::new_cuda(0) {
            Ok(d) => d,
            Err(_) => {
                eprintln!("skipping: no CUDA device");
                return;
            }
        };
        let attn = tiny_attention(&device, ComputeDType::BF16);
        let x = Tensor::randn(0f32, 1f32, (1, 3, 8), &device)
            .unwrap()
            .to_dtype(DType::BF16)
            .unwrap();
        let out = attn.forward(&x, 0, None, None).unwrap();
        assert_eq!(
            out.dtype(),
            DType::BF16,
            "BF16 activations must propagate (no cast back to F32)"
        );
        let v = out
            .to_dtype(DType::F32)
            .unwrap()
            .flatten_all()
            .unwrap()
            .to_vec1::<f32>()
            .unwrap();
        assert!(
            v.iter().all(|f| f.is_finite()),
            "BF16 forward must be finite"
        );
    }
}
