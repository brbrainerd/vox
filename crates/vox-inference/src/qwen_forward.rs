//! Quantized Qwen3.5 / Qwen2.5 forward pass over candle [`QMatMul`] (SP-2, Option B).
//!
//! This is a faithful PORT of the working forward in
//! `vox-plugin-mens-candle-cuda/src/model.rs`, with the only change being that the
//! linear projections run through candle's `quantized::QMatMul` (bias/LoRA already
//! merged away in SP-1) instead of `qlora_rs::QuantizedLinear`. The pure-math helpers
//! (`rotate_half`, `apply_rotary_emb`, causal depthwise conv + SiLU, L2-norm, the
//! gated delta-net recurrence) are copied line-for-line so the result is intended to
//! be numerically equivalent to the plugin forward (the parity reference for the later
//! GPU validation step).
//!
//! Status: FULL-ATTENTION path implemented and tested (shapes + finiteness on CPU).
//! LINEAR-ATTENTION (delta-net) path implemented as a faithful port; tested for
//! shapes + finiteness only. No numerical-parity claim is made here — that is a later
//! GPU step against the plugin reference.

use candle_core::quantized::QMatMul;
use candle_core::{D, DType, Device, Module, Tensor};
use candle_nn::RmsNorm;
use vox_hf_layout::HfTransformerLayout;

use crate::qwen_weights::{QwenWeights, WeightsError};

#[derive(Debug, thiserror::Error)]
pub enum ForwardError {
    #[error("candle: {0}")]
    Candle(#[from] candle_core::Error),
    #[error("weights: {0}")]
    Weights(#[from] WeightsError),
    #[error("missing weight `{0}`")]
    MissingWeight(String),
    #[error("unsupported architecture / config: {0}")]
    Unsupported(String),
}

// ── Pure-math helpers (ported verbatim from plugin model.rs) ───────────────────

fn causal_mask(seq_len: usize, device: &Device) -> Result<Tensor, ForwardError> {
    let mut data = vec![0.0f32; seq_len * seq_len];
    for row in 0..seq_len {
        for col in (row + 1)..seq_len {
            data[row * seq_len + col] = f32::NEG_INFINITY;
        }
    }
    Ok(Tensor::from_vec(data, (1, 1, seq_len, seq_len), device)?)
}

fn repeat_kv(x: &Tensor, n_rep: usize) -> Result<Tensor, ForwardError> {
    if n_rep == 1 {
        return Ok(x.clone());
    }
    let (b, n_kv, seq, hd) = x.dims4()?;
    Ok(x.unsqueeze(2)?
        .expand((b, n_kv, n_rep, seq, hd))?
        .reshape((b, n_kv * n_rep, seq, hd))?)
}

fn rotate_half(x: &Tensor) -> Result<Tensor, ForwardError> {
    let last_dim = x.dim(D::Minus1)?;
    let x1 = x.narrow(D::Minus1, 0, last_dim / 2)?;
    let x2 = x.narrow(D::Minus1, last_dim / 2, last_dim / 2)?;
    Ok(Tensor::cat(&[&x2.neg()?, &x1], D::Minus1)?)
}

/// Dequantize a [`QMatMul`] weight back to its raw `[out, in]` F32 matrix.
///
/// Used for `embed_tokens`, which SP-1 quantizes (role `Embedding` -> Q6K) but the
/// forward consumes via `index_select`, not as a matmul.
fn dequantize_qmatmul(qmm: &QMatMul, dev: &Device) -> Result<Tensor, ForwardError> {
    let t = match qmm {
        QMatMul::QTensor(qt) => qt.dequantize(dev)?,
        QMatMul::Tensor(t) | QMatMul::TensorF16(t) => t.to_dtype(DType::F32)?,
    };
    Ok(t)
}

// ── Linear application over loaded weights ─────────────────────────────────────

/// A loaded linear layer: either a quantized `QMatMul` (`y = x @ Wᵀ`) or a raw F32
/// weight applied the same way. No bias, no LoRA (merged in SP-1).
enum Linear {
    Q(QMatMul),
    F(Tensor),
}

impl Linear {
    fn forward(&self, x: &Tensor) -> Result<Tensor, ForwardError> {
        match self {
            // candle Module::forward on QMatMul computes x @ Wᵀ (dequantize + matmul).
            Linear::Q(qmm) => Ok(Module::forward(qmm, x)?),
            // Raw f32 weight is stored [out, in]; a linear is x @ Wᵀ.
            Linear::F(w) => Ok(x.broadcast_matmul(&w.t()?)?),
        }
    }
}

fn load_linear(w: &QwenWeights, name: &str) -> Result<Linear, ForwardError> {
    if let Some(q) = w.qmatmul(name) {
        Ok(Linear::Q(q.clone()))
    } else if let Some(t) = w.tensor(name) {
        Ok(Linear::F(t.clone()))
    } else {
        Err(ForwardError::MissingWeight(name.to_string()))
    }
}

fn load_tensor(w: &QwenWeights, name: &str) -> Result<Tensor, ForwardError> {
    w.tensor(name)
        .cloned()
        .ok_or_else(|| ForwardError::MissingWeight(name.to_string()))
}

fn load_rmsnorm(w: &QwenWeights, name: &str, eps: f64) -> Result<RmsNorm, ForwardError> {
    let weight = load_tensor(w, name)?;
    Ok(RmsNorm::new(weight, eps))
}

// ── Full attention (GQA + RoPE) ────────────────────────────────────────────────

struct FullAttention {
    q_proj: Linear,
    k_proj: Linear,
    v_proj: Linear,
    o_proj: Linear,
    n_heads: usize,
    n_kv_heads: usize,
    head_dim: usize,
}

impl FullAttention {
    fn forward(&self, x: &Tensor, pos: usize, inv_freq: &Tensor) -> Result<Tensor, ForwardError> {
        let (b, seq_len, _d_model) = x.dims3()?;
        let device = x.device();

        let q = self.q_proj.forward(x)?;
        let k = self.k_proj.forward(x)?;
        let v = self.v_proj.forward(x)?;

        let q = q
            .reshape((b, seq_len, self.n_heads, self.head_dim))?
            .transpose(1, 2)?;
        let k = k
            .reshape((b, seq_len, self.n_kv_heads, self.head_dim))?
            .transpose(1, 2)?;
        let v = v
            .reshape((b, seq_len, self.n_kv_heads, self.head_dim))?
            .transpose(1, 2)?;

        let (q, k) = self.apply_rotary_emb(&q, &k, inv_freq, pos)?;

        let n_rep = self.n_heads / self.n_kv_heads;
        let k = repeat_kv(&k, n_rep)?;
        let v = repeat_kv(&v, n_rep)?;
        let v = v.clamp(-256f64, 256f64)?;

        let scale = 1.0 / (self.head_dim as f64).sqrt();
        let mut att = (q.contiguous()?.matmul(&k.transpose(2, 3)?.contiguous()?)? * scale)?;
        att = att.clamp(-120f64, 120f64)?;

        let y = if seq_len > 1 {
            let att_max = att.max_keepdim(D::Minus1)?;
            att = att.broadcast_sub(&att_max)?;
            let mask = causal_mask(seq_len, device)?;
            let att = att.broadcast_add(&mask)?;
            let att = candle_nn::ops::softmax(&att, D::Minus1)?;
            att.matmul(&v.contiguous()?)?
        } else {
            let att = candle_nn::ops::softmax(&att, D::Minus1)?;
            att.matmul(&v.contiguous()?)?
        };
        let y =
            y.transpose(1, 2)?
                .contiguous()?
                .reshape((b, seq_len, self.n_heads * self.head_dim))?;
        self.o_proj.forward(&y)
    }

    fn apply_rotary_emb(
        &self,
        q: &Tensor,
        k: &Tensor,
        inv_freq: &Tensor,
        pos: usize,
    ) -> Result<(Tensor, Tensor), ForwardError> {
        let (_b, _n_heads, seq_len, head_dim) = q.dims4()?;
        let rope_dim = inv_freq.elem_count().saturating_mul(2);
        if rope_dim == 0 || rope_dim > head_dim {
            return Err(ForwardError::Unsupported(format!(
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
        let cos = freqs.cos()?.reshape((1, 1, seq_len, rope_dim))?;
        let sin = freqs.sin()?.reshape((1, 1, seq_len, rope_dim))?;
        if rope_dim == head_dim {
            let q_embed = (q.broadcast_mul(&cos)? + rotate_half(q)?.broadcast_mul(&sin)?)?;
            let k_embed = (k.broadcast_mul(&cos)? + rotate_half(k)?.broadcast_mul(&sin)?)?;
            Ok((q_embed, k_embed))
        } else {
            let q_rot = q.narrow(D::Minus1, 0, rope_dim)?;
            let q_pass = q.narrow(D::Minus1, rope_dim, head_dim - rope_dim)?;
            let k_rot = k.narrow(D::Minus1, 0, rope_dim)?;
            let k_pass = k.narrow(D::Minus1, rope_dim, head_dim - rope_dim)?;
            let q_r = (q_rot.broadcast_mul(&cos)? + rotate_half(&q_rot)?.broadcast_mul(&sin)?)?;
            let k_r = (k_rot.broadcast_mul(&cos)? + rotate_half(&k_rot)?.broadcast_mul(&sin)?)?;
            let q_embed = Tensor::cat(&[&q_r, &q_pass], D::Minus1)?;
            let k_embed = Tensor::cat(&[&k_r, &k_pass], D::Minus1)?;
            Ok((q_embed, k_embed))
        }
    }
}

// ── Linear (gated delta-net) attention ─────────────────────────────────────────

struct LinearAttention {
    qkv_proj: Linear,
    z_proj: Linear,
    b_proj: Linear,
    a_proj: Linear,
    out_proj: Linear,
    conv_weight: Tensor,
    dt_bias: Tensor,
    a_log: Tensor,
    norm: RmsNorm,
    num_k_heads: usize,
    num_v_heads: usize,
    head_k_dim: usize,
    head_v_dim: usize,
}

impl LinearAttention {
    fn repeat_heads_bshd(x: &Tensor, n_rep: usize) -> Result<Tensor, ForwardError> {
        if n_rep == 1 {
            return Ok(x.clone());
        }
        let (b, s, h, d) = x.dims4()?;
        Ok(x.unsqueeze(3)?
            .expand((b, s, h, n_rep, d))?
            .reshape((b, s, h * n_rep, d))?)
    }

    fn l2norm_last(x: &Tensor, eps: f64) -> Result<Tensor, ForwardError> {
        let d = x.dim(D::Minus1)?;
        let sq = x.broadcast_mul(x)?;
        let sq = sq.sum_keepdim(D::Minus1)?;
        let inv = (sq / (d as f64))?.broadcast_add(&Tensor::new(eps as f32, x.device())?)?;
        let inv = inv.sqrt()?.recip()?;
        Ok(x.broadcast_mul(&inv)?)
    }

    fn causal_depthwise_conv_silu(
        x: &Tensor,
        conv_weight: &Tensor,
    ) -> Result<Tensor, ForwardError> {
        let (b, s, c) = x.dims3()?;
        let k = conv_weight.dim(1)?;
        let dev = x.device();
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
        Ok(Tensor::stack(&steps, 1)?)
    }

    fn forward(&self, x: &Tensor) -> Result<Tensor, ForwardError> {
        let (b, seq_len, _d_model) = x.dims3()?;
        let device = x.device();
        let qkv = self.qkv_proj.forward(x)?;
        let mixed_qkv = Self::causal_depthwise_conv_silu(&qkv, &self.conv_weight)?;

        let key_dim = self.num_k_heads * self.head_k_dim;
        let value_dim = self.num_v_heads * self.head_v_dim;
        let expected_total = key_dim + key_dim + value_dim;
        let got_total = mixed_qkv.dim(D::Minus1)?;
        if got_total != expected_total {
            return Err(ForwardError::Unsupported(format!(
                "qwen3_5 linear_attention qkv dim mismatch: expected {expected_total}, got {got_total}",
            )));
        }

        let query = mixed_qkv.narrow(D::Minus1, 0, key_dim)?.reshape((
            b,
            seq_len,
            self.num_k_heads,
            self.head_k_dim,
        ))?;
        let key = mixed_qkv.narrow(D::Minus1, key_dim, key_dim)?.reshape((
            b,
            seq_len,
            self.num_k_heads,
            self.head_k_dim,
        ))?;
        let value = mixed_qkv
            .narrow(D::Minus1, key_dim + key_dim, value_dim)?
            .reshape((b, seq_len, self.num_v_heads, self.head_v_dim))?;

        let z = self
            .z_proj
            .forward(x)?
            .reshape((b, seq_len, self.num_v_heads, self.head_v_dim))?;
        let beta = candle_nn::ops::sigmoid(&self.b_proj.forward(x)?)?.reshape((
            b,
            seq_len,
            self.num_v_heads,
        ))?;
        let a = self
            .a_proj
            .forward(x)?
            .reshape((b, seq_len, self.num_v_heads))?;

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

        let mut state = Tensor::zeros(
            (b, self.num_v_heads, self.head_k_dim, self.head_v_dim),
            DType::F32,
            device,
        )?;
        let mut outs = Vec::with_capacity(seq_len);
        for t in 0..seq_len {
            let q_t = query.narrow(1, t, 1)?.squeeze(1)?;
            let k_t = key.narrow(1, t, 1)?.squeeze(1)?;
            let v_t = value.narrow(1, t, 1)?.squeeze(1)?;
            let g_t = g.narrow(1, t, 1)?.squeeze(1)?;
            let beta_t = beta.narrow(1, t, 1)?.squeeze(1)?;

            let g_scale = g_t.exp()?.reshape((b, self.num_v_heads, 1, 1))?;
            state = state.broadcast_mul(&g_scale)?;

            let k_col = k_t.unsqueeze(D::Minus1)?;
            let kv_mem = state
                .transpose(2, 3)?
                .contiguous()?
                .matmul(&k_col)?
                .squeeze(D::Minus1)?;
            let delta = v_t
                .broadcast_sub(&kv_mem)?
                .broadcast_mul(&beta_t.unsqueeze(D::Minus1)?)?;
            let delta_row = delta.unsqueeze(2)?;
            let upd = k_col.matmul(&delta_row)?;
            state = (state + upd)?;

            let out_t = state
                .transpose(2, 3)?
                .contiguous()?
                .matmul(&q_t.unsqueeze(D::Minus1)?)?
                .squeeze(D::Minus1)?;
            outs.push(out_t);
        }

        let mut y = Tensor::stack(&outs, 1)?;
        let y_flat = y.reshape((b * seq_len * self.num_v_heads, self.head_v_dim))?;
        let z_flat = z.reshape((b * seq_len * self.num_v_heads, self.head_v_dim))?;
        let y_norm = candle_nn::Module::forward(&self.norm, &y_flat)?;
        let y_gate = y_norm.broadcast_mul(&candle_nn::ops::silu(&z_flat)?)?;
        y = y_gate.reshape((b, seq_len, value_dim))?;

        self.out_proj.forward(&y)
    }
}

enum AttentionBlock {
    Full(FullAttention),
    Linear(LinearAttention),
}

struct Mlp {
    gate_proj: Linear,
    up_proj: Linear,
    down_proj: Linear,
}

impl Mlp {
    fn forward(&self, x: &Tensor) -> Result<Tensor, ForwardError> {
        let lhs = candle_nn::ops::silu(&self.gate_proj.forward(x)?)?;
        let rhs = self.up_proj.forward(x)?;
        self.down_proj.forward(&(lhs * rhs)?)
    }
}

struct Layer {
    input_layernorm: RmsNorm,
    attention: AttentionBlock,
    post_attention_layernorm: RmsNorm,
    mlp: Mlp,
    inv_freq: Option<Tensor>,
}

impl Layer {
    fn forward(&self, x: &Tensor, pos: usize) -> Result<Tensor, ForwardError> {
        let residual = x;
        let h = candle_nn::Module::forward(&self.input_layernorm, x)?;
        let h = match &self.attention {
            AttentionBlock::Full(a) => {
                let inv_freq = self.inv_freq.as_ref().ok_or_else(|| {
                    ForwardError::Unsupported(
                        "full-attention layer missing synthesized RoPE inv_freq".to_string(),
                    )
                })?;
                a.forward(&h, pos, inv_freq)?
            }
            AttentionBlock::Linear(a) => a.forward(&h)?,
        };
        let x = (residual + h)?;

        let residual = &x;
        let h = candle_nn::Module::forward(&self.post_attention_layernorm, &x)?;
        let h = self.mlp.forward(&h)?;
        Ok((residual + h)?)
    }
}

/// A quantized Qwen3.5 / Qwen2.5 model ready for a forward pass.
pub struct QwenForward {
    embed_tokens: Tensor,
    layers: Vec<Layer>,
    norm: RmsNorm,
    lm_head: Linear,
    hidden_size: usize,
}

impl QwenForward {
    /// Build the forward modules from a parsed layout and loaded quantized weights.
    pub fn new(
        layout: &HfTransformerLayout,
        weights: QwenWeights,
        dev: &Device,
    ) -> Result<Self, ForwardError> {
        let w = &weights;
        let prefix = &layout.namespace_prefix;
        let eps = 1e-6;

        // RMSNorm eps: HF Qwen uses rms_norm_eps (default 1e-6). Layout does not expose
        // it, so we use the Qwen default; flagged as a numerical-parity consideration.

        // embed_tokens is quantized by SP-1 (role Embedding); dequantize for index_select.
        let embed_key = format!("{}.embed_tokens.weight", prefix.trim_end_matches(".layers"));
        let embed_tokens = if let Some(q) = w.qmatmul(&embed_key) {
            dequantize_qmatmul(q, dev)?
        } else if let Some(t) = w.tensor(&embed_key) {
            t.clone()
        } else {
            return Err(ForwardError::MissingWeight(embed_key));
        };

        let head_dim = layout
            .head_dim
            .unwrap_or(layout.hidden_size / layout.num_attention_heads.max(1));
        let rope_theta = layout.rope_theta.unwrap_or(10_000.0);
        let partial = layout.rope_partial_rotary_factor.unwrap_or(1.0);
        // rope_dim = head_dim * partial_rotary_factor, rounded to even.
        let mut rope_dim = ((head_dim as f64) * partial).round() as usize;
        if rope_dim == 0 {
            rope_dim = head_dim;
        }
        rope_dim &= !1; // make even
        let inv_freq = synth_inv_freq(rope_dim, rope_theta, dev)?;

        let n_layers = layout.num_hidden_layers;
        let mut layers = Vec::with_capacity(n_layers);
        for i in 0..n_layers {
            let lp = format!("{prefix}.{i}");
            let layer_type = layout
                .layer_types
                .get(i)
                .map(String::as_str)
                .unwrap_or("full_attention");

            let input_layernorm = load_rmsnorm(w, &format!("{lp}.input_layernorm.weight"), eps)?;
            let post_attention_layernorm =
                load_rmsnorm(w, &format!("{lp}.post_attention_layernorm.weight"), eps)?;

            let (attention, layer_inv_freq) = if layer_type == "linear_attention" {
                let num_k_heads = layout.linear_num_key_heads.ok_or_else(|| {
                    ForwardError::Unsupported("missing linear_num_key_heads".to_string())
                })?;
                let num_v_heads = layout.linear_num_value_heads.ok_or_else(|| {
                    ForwardError::Unsupported("missing linear_num_value_heads".to_string())
                })?;
                let head_k_dim = layout.linear_key_head_dim.ok_or_else(|| {
                    ForwardError::Unsupported("missing linear_key_head_dim".to_string())
                })?;
                let head_v_dim = layout.linear_value_head_dim.ok_or_else(|| {
                    ForwardError::Unsupported("missing linear_value_head_dim".to_string())
                })?;
                let attn = LinearAttention {
                    qkv_proj: load_linear(w, &format!("{lp}.linear_attn.in_proj_qkv.weight"))?,
                    z_proj: load_linear(w, &format!("{lp}.linear_attn.in_proj_z.weight"))?,
                    b_proj: load_linear(w, &format!("{lp}.linear_attn.in_proj_b.weight"))?,
                    a_proj: load_linear(w, &format!("{lp}.linear_attn.in_proj_a.weight"))?,
                    out_proj: load_linear(w, &format!("{lp}.linear_attn.out_proj.weight"))?,
                    conv_weight: load_tensor(w, &format!("{lp}.linear_attn.conv1d.weight"))?,
                    dt_bias: load_tensor(w, &format!("{lp}.linear_attn.dt_bias"))?,
                    a_log: load_tensor(w, &format!("{lp}.linear_attn.A_log"))?,
                    norm: load_rmsnorm(w, &format!("{lp}.linear_attn.norm.weight"), eps)?,
                    num_k_heads,
                    num_v_heads,
                    head_k_dim,
                    head_v_dim,
                };
                (AttentionBlock::Linear(attn), None)
            } else {
                let attn = FullAttention {
                    q_proj: load_linear(w, &format!("{lp}.self_attn.q_proj.weight"))?,
                    k_proj: load_linear(w, &format!("{lp}.self_attn.k_proj.weight"))?,
                    v_proj: load_linear(w, &format!("{lp}.self_attn.v_proj.weight"))?,
                    o_proj: load_linear(w, &format!("{lp}.self_attn.o_proj.weight"))?,
                    n_heads: layout.num_attention_heads,
                    n_kv_heads: layout.num_key_value_heads,
                    head_dim,
                };
                (AttentionBlock::Full(attn), Some(inv_freq.clone()))
            };

            let mlp = Mlp {
                gate_proj: load_linear(w, &format!("{lp}.mlp.gate_proj.weight"))?,
                up_proj: load_linear(w, &format!("{lp}.mlp.up_proj.weight"))?,
                down_proj: load_linear(w, &format!("{lp}.mlp.down_proj.weight"))?,
            };

            layers.push(Layer {
                input_layernorm,
                attention,
                post_attention_layernorm,
                mlp,
                inv_freq: layer_inv_freq,
            });
        }

        let norm_key = format!("{}.norm.weight", prefix.trim_end_matches(".layers"));
        let norm = load_rmsnorm(w, &norm_key, eps)?;

        // lm_head: prefer an explicit lm_head.weight, else tie to embed_tokens.
        let lm_head = if w.contains("lm_head.weight") {
            load_linear(w, "lm_head.weight")?
        } else {
            Linear::F(embed_tokens.clone())
        };

        Ok(Self {
            embed_tokens,
            layers,
            norm,
            lm_head,
            hidden_size: layout.hidden_size,
        })
    }

    /// Run a forward pass over `input_ids` (`[1, seq]`, u32). Returns logits
    /// `[1, seq, vocab]`; `pos` is the starting absolute position for RoPE.
    pub fn forward(&mut self, input_ids: &Tensor, pos: usize) -> Result<Tensor, ForwardError> {
        let (b, seq_len) = input_ids.dims2()?;
        let ids = input_ids.flatten_all()?;
        let mut x =
            self.embed_tokens
                .index_select(&ids, 0)?
                .reshape((b, seq_len, self.hidden_size))?;

        for layer in &self.layers {
            x = layer.forward(&x, pos)?;
        }
        let x = candle_nn::Module::forward(&self.norm, &x)?;
        let x = x.clamp(-64f64, 64f64)?;
        self.lm_head.forward(&x)
    }
}

/// Standard RoPE inverse-frequency table: `inv_freq[i] = theta^(-2i/rope_dim)` for
/// `i in 0..rope_dim/2`. Synthesized from config when HF omits `*.rotary_emb.inv_freq`
/// (the usual case for Qwen3.5).
fn synth_inv_freq(rope_dim: usize, theta: f64, dev: &Device) -> Result<Tensor, ForwardError> {
    let half = rope_dim / 2;
    let mut data = Vec::with_capacity(half);
    for i in 0..half {
        let exponent = (2 * i) as f64 / rope_dim as f64;
        data.push((1.0 / theta.powf(exponent)) as f32);
    }
    Ok(Tensor::from_vec(data, (half,), dev)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    /// Build a synthetic SP-1 quantized artifact whose tensors cover exactly the keys
    /// the forward looks up, with 256-aligned dims, then load it through `QwenWeights`.
    fn build_artifact(
        config_json: &str,
        tensors: &std::collections::HashMap<String, Tensor>,
        dev: &Device,
    ) -> tempfile::TempDir {
        let indir = tempfile::tempdir().unwrap();
        let outdir = tempfile::tempdir().unwrap();
        candle_core::safetensors::save(tensors, indir.path().join("model.safetensors")).unwrap();
        std::fs::write(indir.path().join("config.json"), config_json).unwrap();
        vox_quantize::quantize(&vox_quantize::QuantizeRequest {
            input_dir: indir.path().to_path_buf(),
            output_dir: outdir.path().to_path_buf(),
            mixture: vox_quantize::QuantMixture::Q4KM,
            verify: false,
            device: vox_quantize::DevicePref::Cpu,
        })
        .unwrap();
        let _ = dev;
        outdir
    }

    fn rand2(out: usize, inp: usize, dev: &Device) -> Tensor {
        // small scale keeps quantized roundtrip + recurrence numerically tame.
        (Tensor::randn(0f32, 1f32, (out, inp), dev).unwrap() * 0.02).unwrap()
    }
    fn ones1(n: usize, dev: &Device) -> Tensor {
        Tensor::ones((n,), DType::F32, dev).unwrap()
    }

    #[test]
    fn full_attention_forward_shapes_and_finite() {
        let dev = Device::Cpu;
        let hidden = 256usize;
        let heads = 8usize;
        let head_dim = hidden / heads; // 32
        let inter = 256usize;
        let vocab = 512usize;
        let p = "model.language_model.layers";

        let cfg = format!(
            r#"{{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],
                "text_config":{{"hidden_size":{hidden},"num_attention_heads":{heads},
                "num_key_value_heads":{heads},"num_hidden_layers":1,"vocab_size":{vocab},
                "intermediate_size":{inter},"head_dim":{head_dim},
                "rope_parameters":{{"rope_theta":10000,"partial_rotary_factor":1.0}},
                "layer_types":["full_attention"]}}}}"#
        );

        let mut t = std::collections::HashMap::new();
        t.insert(
            "model.language_model.embed_tokens.weight".into(),
            rand2(vocab, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.q_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.k_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.v_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.self_attn.o_proj.weight"),
            rand2(hidden, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.gate_proj.weight"),
            rand2(inter, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.up_proj.weight"),
            rand2(inter, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.down_proj.weight"),
            rand2(hidden, inter, &dev),
        );
        t.insert(format!("{p}.0.input_layernorm.weight"), ones1(hidden, &dev));
        t.insert(
            format!("{p}.0.post_attention_layernorm.weight"),
            ones1(hidden, &dev),
        );
        t.insert(
            "model.language_model.norm.weight".into(),
            ones1(hidden, &dev),
        );
        t.insert("lm_head.weight".into(), rand2(vocab, hidden, &dev));

        let outdir = build_artifact(&cfg, &t, &dev);
        let layout = HfTransformerLayout::from_config_json_str(&cfg).unwrap();
        let weights = QwenWeights::load(outdir.path(), &dev).unwrap();
        let mut model = QwenForward::new(&layout, weights, &dev).unwrap();

        let ids = Tensor::from_vec(vec![1u32, 2, 3, 4], (1, 4), &dev).unwrap();
        let logits = model.forward(&ids, 0).unwrap();
        assert_eq!(logits.dims(), &[1, 4, vocab]);
        let flat = logits.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        assert!(flat.iter().all(|v| v.is_finite()), "logits must be finite");
    }

    #[test]
    fn linear_attention_forward_shapes_and_finite() {
        let dev = Device::Cpu;
        let hidden = 256usize;
        let heads = 8usize;
        let head_dim = hidden / heads;
        let inter = 256usize;
        let vocab = 512usize;
        let p = "model.language_model.layers";

        // linear-attn geometry (256-aligned projection widths).
        let num_k_heads = 4usize;
        let num_v_heads = 8usize;
        let head_k_dim = 32usize; // key_dim = 128
        let head_v_dim = 32usize; // value_dim = 256
        let key_dim = num_k_heads * head_k_dim; // 128
        let value_dim = num_v_heads * head_v_dim; // 256
        let qkv_out = key_dim + key_dim + value_dim; // 512 (256-aligned)
        let conv_k = 4usize;

        let cfg = format!(
            r#"{{"model_type":"qwen3_5","architectures":["Qwen35ForCausalLM"],
                "text_config":{{"hidden_size":{hidden},"num_attention_heads":{heads},
                "num_key_value_heads":{heads},"num_hidden_layers":1,"vocab_size":{vocab},
                "intermediate_size":{inter},"head_dim":{head_dim},
                "linear_num_key_heads":{num_k_heads},"linear_num_value_heads":{num_v_heads},
                "linear_key_head_dim":{head_k_dim},"linear_value_head_dim":{head_v_dim},
                "linear_conv_kernel_dim":{conv_k},
                "rope_parameters":{{"rope_theta":10000,"partial_rotary_factor":1.0}},
                "layer_types":["linear_attention"]}}}}"#
        );

        let mut t = std::collections::HashMap::new();
        t.insert(
            "model.language_model.embed_tokens.weight".into(),
            rand2(vocab, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.linear_attn.in_proj_qkv.weight"),
            rand2(qkv_out, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.linear_attn.in_proj_z.weight"),
            rand2(value_dim, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.linear_attn.in_proj_a.weight"),
            rand2(num_v_heads, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.linear_attn.in_proj_b.weight"),
            rand2(num_v_heads, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.linear_attn.out_proj.weight"),
            rand2(hidden, value_dim, &dev),
        );
        // conv1d weight [channels=qkv_out, kernel]; KEEP-F32 (not a *.weight matmul role? it is Matrix-> but 2D kernel small). Keep as f32 by naming convention: conv1d.weight is Matrix role, but last dim = conv_k (4) -> not 256-aligned -> resolve_dtype falls back to F32. Good: loader puts it in f32 map.
        t.insert(
            format!("{p}.0.linear_attn.conv1d.weight"),
            (Tensor::randn(0f32, 1f32, (qkv_out, conv_k), &dev).unwrap() * 0.1).unwrap(),
        );
        t.insert(
            format!("{p}.0.linear_attn.dt_bias"),
            (Tensor::randn(0f32, 1f32, (num_v_heads,), &dev).unwrap() * 0.1).unwrap(),
        );
        t.insert(
            format!("{p}.0.linear_attn.A_log"),
            Tensor::zeros((num_v_heads,), DType::F32, &dev).unwrap(),
        );
        t.insert(
            format!("{p}.0.linear_attn.norm.weight"),
            ones1(head_v_dim, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.gate_proj.weight"),
            rand2(inter, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.up_proj.weight"),
            rand2(inter, hidden, &dev),
        );
        t.insert(
            format!("{p}.0.mlp.down_proj.weight"),
            rand2(hidden, inter, &dev),
        );
        t.insert(format!("{p}.0.input_layernorm.weight"), ones1(hidden, &dev));
        t.insert(
            format!("{p}.0.post_attention_layernorm.weight"),
            ones1(hidden, &dev),
        );
        t.insert(
            "model.language_model.norm.weight".into(),
            ones1(hidden, &dev),
        );
        t.insert("lm_head.weight".into(), rand2(vocab, hidden, &dev));

        let outdir = build_artifact(&cfg, &t, &dev);
        let layout = HfTransformerLayout::from_config_json_str(&cfg).unwrap();
        let weights = QwenWeights::load(outdir.path(), &dev).unwrap();
        let mut model = QwenForward::new(&layout, weights, &dev).unwrap();

        let ids = Tensor::from_vec(vec![1u32, 2, 3, 4], (1, 4), &dev).unwrap();
        let logits = model.forward(&ids, 0).unwrap();
        assert_eq!(logits.dims(), &[1, 4, vocab]);
        let flat = logits.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        assert!(flat.iter().all(|v| v.is_finite()), "logits must be finite");
    }
}
