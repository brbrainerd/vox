//! Native QLoRA training: NF4-quantized frozen base linears + trainable LoRA.
//!
//! Ported from `vox-populi/src/mens/tensor/candle_qlora_train/mod.rs` (SP3 sub-batch C).
//! Path rewrites:
//!   - `crate::mens::tensor::*` → `crate::*`
//!   - `crate::mens::tensor::candle_model_qwen::*` → `crate::model::*`
//!   - `super::device::DeviceKind` → `crate::device::DeviceKind`

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context, Result};
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use peft_rs::training::{AdapterTrainingConfig, LrSchedule};
use qlora_rs::QLoraConfig;
use qlora_rs::qlora::QuantizedLinear;
use qlora_rs::training::{QLoraTrainer, QLoraTrainingConfig};
use rand::SeedableRng;
use rand::seq::SliceRandom;
use tokenizers::Tokenizer;

use crate::config::LoraTrainingConfig;
use crate::device::DeviceKind;
use crate::hf_layout::HfArchitecture;
use crate::qlora_preflight::preflight_native_qlora;
use crate::train_jsonl_preflight::preflight_train_jsonl;
use crate::train_log;

/// EMA alpha for ETA calculation.
pub(super) const QLORA_ETA_EMA_ALPHA: f64 = 0.2;

/// Global flag for graceful interruption (Ctrl+C).
pub(super) static PAUSE_FLAG: AtomicBool = AtomicBool::new(false);

pub(super) enum TrainGraphModel {
    Qwen35(crate::model::Qwen35Model),
}

impl TrainGraphModel {
    pub fn forward(&self, input_ids: &Tensor) -> anyhow::Result<Tensor> {
        match self {
            Self::Qwen35(m) => Ok(m.forward(input_ids)?),
        }
    }
}

// ── DB message bus ────────────────────────────────────────────────────────────

pub(super) enum TrainingDbEvent {
    Start {
        run_id: String,
        adapter_tag: Option<String>,
        model_name: Option<String>,
        output_dir: String,
        data_dir: String,
        planned_steps: Option<u32>,
    },
    Checkpoint {
        run_id: String,
        epoch: u32,
        global_step: u32,
        last_loss: Option<f32>,
        adapter_path: String,
    },
    EpochSummary {
        run_id: String,
        epoch: u32,
        global_step: u32,
        avg_loss: f64,
        avg_val_loss: f64,
        val_steps: u32,
    },
    Complete {
        run_id: String,
        global_step: u32,
        adapter_path: String,
    },
    Failed {
        run_id: String,
        global_step: u32,
    },
    GrpoStep {
        run_id: String,
        step: u32,
        mean_reward: f32,
        policy_loss: f32,
        clip_fraction: f32,
        parse_rate: f32,
    },
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct TrainingLoopStats {
    pub skip_no_supervised_positions: u64,
    pub skip_short_seq: u64,
    pub skip_curriculum: u64,
    pub skip_token_id_oob: u64,
}

/// Load LoRA adapter weights into a trainer's varmap (warm-start).
fn load_adapter_into_trainer(trainer: &mut QLoraTrainer, path: &Path) -> Result<()> {
    if !path.exists() {
        anyhow::bail!("checkpoint adapter not found: {}", path.display());
    }
    trainer
        .load_lora_weights(path)
        .context("warm-start LoRA weights")?;
    train_log::info(&format!(
        "Warm-started LoRA weights from {}",
        path.display()
    ));
    Ok(())
}

/// Calculate cosine learning rate with linear warmup.
fn compute_cosine_lr(step: u32, warmup: usize, total: u32, base_lr: f64) -> f64 {
    if (step as usize) < warmup {
        base_lr * (step as f64 + 1.0) / warmup.max(1) as f64
    } else {
        let progress = (step as usize - warmup) as f64 / (total as usize - warmup).max(1) as f64;
        let progress = progress.clamp(0.0, 1.0);
        base_lr * 0.5 * (1.0 + (std::f64::consts::PI * progress).cos())
    }
}

fn synthesize_rope_inv_freq(
    head_dim: usize,
    rope_theta: Option<f64>,
    device: &Device,
) -> Result<Tensor> {
    let half = head_dim / 2;
    if half == 0 {
        anyhow::bail!("invalid head_dim={} for RoPE synthesis", head_dim);
    }
    let theta = rope_theta.unwrap_or(10_000.0) as f32;
    let hd = head_dim as f32;
    let mut vals = Vec::with_capacity(half);
    for i in 0..half {
        let exponent = (2.0_f32 * i as f32) / hd;
        vals.push(1.0_f32 / theta.powf(exponent));
    }
    Ok(Tensor::from_vec(vals, (half,), device)?)
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// JSON wire format for `run_candle_qlora_train` — all parameters needed by the plugin.
/// The caller (vox-populi or CLI) serializes this; the plugin deserializes it.
#[derive(Debug, serde::Deserialize)]
pub struct TrainRequest {
    pub config: LoraTrainingConfig,
    #[serde(default)]
    pub data_dir: Option<String>,
    #[serde(default)]
    pub output_dir: Option<String>,
    #[serde(default = "default_device_kind")]
    pub device_kind: DeviceKind,
    #[serde(default)]
    pub system_prompt: String,
}

fn default_device_kind() -> DeviceKind {
    DeviceKind::Best
}

/// Main entry point — called from `crate::training::run_full_training`.
pub fn run_candle_qlora_train(
    data_dir: &Path,
    output_dir: Option<&Path>,
    config: &LoraTrainingConfig,
    device_kind: DeviceKind,
    system_prompt: &str,
) -> Result<crate::training_summary::TrainingSummary> {
    let out_buf = output_dir.map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("mens/runs").join(config.run_id.as_deref().unwrap_or("v1"))
    });
    std::fs::create_dir_all(&out_buf).context("create output dir")?;
    let out: &Path = out_buf.as_path();

    if config.force_restart
        && let Err(e) = device_select::purge_fresh_start_artifacts(out)
    {
        train_log::warn(&format!(
            "Could not remove previous run artifacts in {}: {e}",
            out.display()
        ));
    }

    let (device, device_label) =
        device_select::select_candle_device(device_kind, config.allow_cpu_fallback)?;
    if config.require_gpu && matches!(device, Device::Cpu) {
        anyhow::bail!(
            "GPU execution was required, but Candle selected CPU device '{}'. \
             Re-run with --device Metal (or metal on macOS) after fixing local accelerator setup.",
            device_label
        );
    }

    // ── Graceful pause (Ctrl+C) ──────────────────────────────────────────────
    PAUSE_FLAG.store(false, Ordering::SeqCst);
    let _ = ctrlc::set_handler(move || {
        if PAUSE_FLAG.load(Ordering::SeqCst) {
            std::process::exit(1);
        }
        train_log::warn("Ctrl+C detected — saving checkpoint and pausing after current step...");
        PAUSE_FLAG.store(true, Ordering::SeqCst);
    });

    let bundle = preflight_native_qlora(config).map_err(|e| {
        anyhow::anyhow!("Model preflight failed: {e}. Ensure you have run 'vox mens download --model <name>' and that tokenizer.json + safetensors are present.")
    })?;

    let n_layer = bundle.layout.num_hidden_layers;
    if config.qlora_lm_head_only {
        anyhow::bail!(
            "Candle QLoRA: `--qlora-lm-head-only` (partial adapter stack) is not implemented; \
             omit the flag to train the full graph (depth {} layers).",
            n_layer
        );
    }
    if let Some(0) = config.qlora_proxy_max_layers {
        anyhow::bail!(
            "Candle QLoRA: `--qlora-proxy-max-layers 0` selects LM-head-only training, which is not implemented; \
             omit the flag for a full stack."
        );
    }
    if let Some(cap) = config.qlora_proxy_max_layers
        && cap < n_layer
    {
        anyhow::bail!(
            "Candle QLoRA: `--qlora-proxy-max-layers={cap}` is less than model depth ({n_layer}); \
                 partial-stack training is not implemented. Omit the flag or set the cap to at least {n_layer}."
        );
    }

    let tokenizer = Tokenizer::from_file(&bundle.tokenizer_path)
        .map_err(|e| anyhow::anyhow!("load tokenizer: {e}"))?;

    let train_path: PathBuf = config
        .train_file
        .clone()
        .unwrap_or_else(|| data_dir.join("train.jsonl"));
    let _ = preflight_train_jsonl(&train_path, 1_000_000)?;
    let jsonl_strict_resolved =
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxMensTrainJsonlStrict);
    let jsonl_policy = if jsonl_strict_resolved.expose().is_some_and(|s| s == "1") {
        vox_tensor::data::MalformedJsonlPolicy::FailFast
    } else {
        vox_tensor::data::MalformedJsonlPolicy::Skip
    };
    train_log::info(&format!(
        "Loading training data from {} (min_rating={})...",
        train_path.display(),
        config.min_rating
    ));
    let mut pairs =
        vox_tensor::data::load_all_with_policy(&train_path, config.min_rating, jsonl_policy)
            .with_context(|| format!("load training data from {}", train_path.display()))?;
    train_log::info(&format!("Loaded {} pairs.", pairs.len()));
    let mut computed_contamination = None;
    if let Some(filter) = config.context_filter.as_ref() {
        let before = pairs.len();
        let is_vox_pure = filter
            .categories
            .as_ref()
            .is_some_and(|cats| cats.iter().any(|c| c.eq_ignore_ascii_case("vox_pure")));
        let mut total_react_tokens = 0usize;
        let mut total_code_tokens = 0usize;

        pairs.retain(|p| {
            if let Some(r_min) = filter.rating_min
                && p.rating.unwrap_or(0) < r_min
            {
                return false;
            }
            if let Some(d) = p.difficulty {
                if let Some(d_min) = filter.difficulty_min
                    && d < d_min
                {
                    return false;
                }
                if let Some(d_max) = filter.difficulty_max
                    && d > d_max
                {
                    return false;
                }
            }
            if let Some(cats) = &filter.categories {
                if cats.is_empty() {
                    return true;
                }
                let mut matches = is_vox_pure;
                if !matches && let Some(c) = &p.category {
                    let text = c.to_ascii_lowercase();
                    if cats
                        .iter()
                        .any(|cat| text.contains(&cat.to_ascii_lowercase()))
                    {
                        matches = true;
                    }
                }
                if !matches && let Some(l) = &p.lane {
                    let text = l.to_ascii_lowercase();
                    if cats
                        .iter()
                        .any(|cat| text.contains(&cat.to_ascii_lowercase()))
                    {
                        matches = true;
                    }
                }
                if !matches && let Some(tf) = &p.task_family {
                    let text = tf.to_ascii_lowercase();
                    if cats
                        .iter()
                        .any(|cat| text.contains(&cat.to_ascii_lowercase()))
                    {
                        matches = true;
                    }
                }
                if !matches {
                    return false;
                }
            }

            if is_vox_pure && let Some(resp) = p.effective_response() {
                let hits = resp.matches("className=").count()
                    + resp.matches("import React").count()
                    + resp.matches("useEffect(").count()
                    + resp.matches("useState(").count()
                    + resp.matches("<div").count()
                    + resp.matches("onClick=").count();

                total_react_tokens += hits;
                total_code_tokens += resp.len().max(1) / 4;

                if hits > 3 {
                    return false;
                }
            }

            true
        });

        if is_vox_pure && total_code_tokens > 0 {
            computed_contamination = Some(total_react_tokens as f32 / total_code_tokens as f32);
        }

        train_log::info(&format!(
            "Applied context_filter: {} -> {} rows (contamination={:?})",
            before,
            pairs.len(),
            computed_contamination
        ));
    }
    if pairs.is_empty() {
        anyhow::bail!("No training pairs found in {}", train_path.display());
    }

    let val_count = {
        let pct_count =
            (pairs.len() as f64 * config.validation_split_ratio.unwrap_or(0.05)) as usize;
        if pairs.len() > 20 {
            pct_count.max(10).min(pairs.len() / 2)
        } else {
            pct_count
        }
    };
    let eval_pairs = if val_count > 0 && pairs.len() > val_count {
        let mut rng = rand::rngs::StdRng::seed_from_u64(config.seed ^ 0xA1B2_C3D4_E5F6_1122);
        pairs.shuffle(&mut rng);
        pairs.split_off(pairs.len() - val_count)
    } else {
        Vec::new()
    };

    // ── GQA-aware dimensions ─────────────────────────────────────────────────
    let n_heads = bundle.layout.num_attention_heads;
    let n_kv_heads = bundle.layout.num_key_value_heads;
    let head_dim = bundle
        .layout
        .head_dim
        .unwrap_or_else(|| bundle.d_model / n_heads.max(1));
    let kv_dim = n_kv_heads * head_dim;

    // ── mmap base weights ─────────────────────────────────────────────────────
    train_log::info(&format!(
        "Mmapping base weights from {} files...",
        bundle.weight_paths.len()
    ));
    #[allow(unsafe_code)]
    let vb_mmap =
        unsafe { VarBuilder::from_mmaped_safetensors(&bundle.weight_paths, DType::F32, &device)? };
    train_log::info(&format!(
        "Loading embeddings ('{}') to device...",
        bundle.embed_key
    ));
    let wte = vb_mmap.get((bundle.vocab, bundle.d_model), &bundle.embed_key)?;
    train_log::info("Embeddings loaded.");

    // ── qlora-rs config ───────────────────────────────────────────────────────
    let rank = config.rank.max(1);
    let alpha_u = config.alpha.round() as usize;
    let qlora_cfg = QLoraConfig::preset_all_bf16(rank, alpha_u);

    let total_steps_planned = (pairs.len() * config.epochs) as u32;
    let grad_accum = config.grad_accum.max(1) as u32;
    let total_optimizer_steps_planned = total_steps_planned.div_ceil(grad_accum);
    let warmup_steps = config
        .warmup_steps
        .min((total_optimizer_steps_planned / 10).max(1) as usize);

    let train_cfg = QLoraTrainingConfig {
        adapter_config: AdapterTrainingConfig {
            learning_rate: config.learning_rate,
            lr_schedule: LrSchedule::LinearWarmup { warmup_steps },
            weight_decay: 0.01,
            gradient_accumulation_steps: config.grad_accum.max(1),
            max_grad_norm: Some(1.0),
        },
        num_epochs: config.epochs,
        ..Default::default()
    };

    let mut trainer = QLoraTrainer::new(train_cfg, device.clone());

    // ── Build transformer graph ───────────────────────────────────────────────
    let mut model_layers_qwen35 = Vec::with_capacity(bundle.layout.num_hidden_layers);
    let mut adapter_layer_order: Vec<String> = Vec::new();
    let mut base_key_map: HashMap<String, String> = HashMap::new();
    let synthesized_rope_inv_freq =
        synthesize_rope_inv_freq(head_dim, bundle.layout.rope_theta, &device).ok();
    if synthesized_rope_inv_freq.is_some() {
        train_log::info(
            "RoPE inv_freq tensors are optional in some HF shards; using synthesized inv_freq from config rope_theta when per-layer tensors are absent.",
        );
    }

    let (final_norm, lm_head) = {
        let vb = trainer.var_builder();
        train_log::info(&format!(
            "Candle QLoRA: building full graph ({} layers, n_heads={}, n_kv_heads={}, head_dim={})",
            bundle.layout.num_hidden_layers, n_heads, n_kv_heads, head_dim
        ));

        for i in 0..bundle.layout.num_hidden_layers {
            if i % 8 == 0 {
                train_log::info(&format!(
                    "  [graph] building layer {i}/{}...",
                    bundle.layout.num_hidden_layers
                ));
            }
            let layer_prefix = format!("{}.{}", bundle.layout.namespace_prefix, i);
            let layer_type = bundle
                .layout
                .layer_types
                .get(i)
                .map(String::as_str)
                .unwrap_or("full_attention");
            let is_qwen35 = bundle.layout.architecture == HfArchitecture::Qwen35;
            if is_qwen35 && layer_type != "full_attention" && layer_type != "linear_attention" {
                anyhow::bail!(
                    "qwen3_5 layer_types[{i}]={layer_type:?} is unsupported at runtime; expected `full_attention` or `linear_attention`."
                );
            }
            // ── Layer norms ───────────────────────────────────────────────────
            let ln1_key = format!("{layer_prefix}.input_layernorm.weight");
            let ln2_key = format!("{layer_prefix}.post_attention_layernorm.weight");
            let w_ln1 = vb_mmap
                .get(bundle.d_model, &ln1_key)?
                .to_dtype(DType::F32)?;
            let w_ln2 = vb_mmap
                .get(bundle.d_model, &ln2_key)?
                .to_dtype(DType::F32)?;
            let ln1 = candle_nn::RmsNorm::new(w_ln1, 1e-6);
            let ln2 = candle_nn::RmsNorm::new(w_ln2, 1e-6);

            // ── Attention projections ────────────────────────────────────────
            let qwen35_attn = if is_qwen35 && layer_type == "linear_attention" {
                let qkv_key = format!("{layer_prefix}.linear_attn.in_proj_qkv.weight");
                let z_key = format!("{layer_prefix}.linear_attn.in_proj_z.weight");
                let b_key = format!("{layer_prefix}.linear_attn.in_proj_b.weight");
                let a_key = format!("{layer_prefix}.linear_attn.in_proj_a.weight");
                let o_key = format!("{layer_prefix}.linear_attn.out_proj.weight");
                let conv_key = format!("{layer_prefix}.linear_attn.conv1d.weight");
                let dt_key = format!("{layer_prefix}.linear_attn.dt_bias");
                let alog_key = format!("{layer_prefix}.linear_attn.A_log");
                let norm_key = format!("{layer_prefix}.linear_attn.norm.weight");

                let linear_key_heads = bundle.layout.linear_num_key_heads.unwrap_or(n_heads);
                let linear_value_heads = bundle.layout.linear_num_value_heads.unwrap_or(n_heads);
                let linear_key_dim = bundle.layout.linear_key_head_dim.unwrap_or(head_dim);
                let linear_value_dim = bundle.layout.linear_value_head_dim.unwrap_or(head_dim);
                let q_dim = linear_key_heads * linear_key_dim;
                let k_dim = linear_key_heads * linear_key_dim;
                let v_dim = linear_value_heads * linear_value_dim;
                let qkv_rows = q_dim + k_dim + v_dim;
                let w_qkv = vb_mmap
                    .get((qkv_rows, bundle.d_model), &qkv_key)?
                    .to_dtype(DType::F32)?;
                let w_z = vb_mmap
                    .get((v_dim, bundle.d_model), &z_key)?
                    .to_dtype(DType::F32)?;
                let w_b = vb_mmap
                    .get((linear_value_heads, bundle.d_model), &b_key)?
                    .to_dtype(DType::F32)?;
                let w_a = vb_mmap
                    .get((linear_value_heads, bundle.d_model), &a_key)?
                    .to_dtype(DType::F32)?;
                let w_o = vb_mmap
                    .get((bundle.d_model, v_dim), &o_key)?
                    .to_dtype(DType::F32)?;
                let w_conv = vb_mmap
                    .get(
                        (
                            qkv_rows,
                            1,
                            bundle.layout.linear_conv_kernel_dim.unwrap_or(4),
                        ),
                        &conv_key,
                    )
                    .or_else(|_| {
                        vb_mmap.get(
                            (qkv_rows, bundle.layout.linear_conv_kernel_dim.unwrap_or(4)),
                            &conv_key,
                        )
                    })?
                    .to_dtype(DType::F32)?;
                let conv_weight = if w_conv.rank() == 3 {
                    w_conv.squeeze(1)?
                } else {
                    w_conv
                };
                let dt_bias = vb_mmap
                    .get(linear_value_heads, &dt_key)?
                    .to_dtype(DType::F32)?;
                let a_log = vb_mmap
                    .get(linear_value_heads, &alog_key)?
                    .to_dtype(DType::F32)?;
                let norm_w = vb_mmap
                    .get(linear_value_dim, &norm_key)?
                    .to_dtype(DType::F32)?;

                let qkv_label = format!("l{i}.lin_qkv");
                let z_label = format!("l{i}.lin_z");
                let b_label = format!("l{i}.lin_b");
                let a_label = format!("l{i}.lin_a");
                let o_label = format!("l{i}.lin_o");

                let qkv_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_qkv,
                    None,
                    &qlora_cfg,
                    vb.pp(&qkv_label),
                )?;
                let z_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_z,
                    None,
                    &qlora_cfg,
                    vb.pp(&z_label),
                )?;
                let b_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_b,
                    None,
                    &qlora_cfg,
                    vb.pp(&b_label),
                )?;
                let a_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_a,
                    None,
                    &qlora_cfg,
                    vb.pp(&a_label),
                )?;
                let out_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_o,
                    None,
                    &qlora_cfg,
                    vb.pp(&o_label),
                )?;

                adapter_layer_order.push(qkv_label.clone());
                base_key_map.insert(qkv_label, qkv_key);
                adapter_layer_order.push(z_label.clone());
                base_key_map.insert(z_label, z_key);
                adapter_layer_order.push(b_label.clone());
                base_key_map.insert(b_label, b_key);
                adapter_layer_order.push(a_label.clone());
                base_key_map.insert(a_label, a_key);
                adapter_layer_order.push(o_label.clone());
                base_key_map.insert(o_label, o_key);

                Some(crate::model::Qwen35AttentionBlock::Linear(
                    crate::model::Qwen35LinearAttention {
                        qkv_proj,
                        z_proj,
                        b_proj,
                        a_proj,
                        out_proj,
                        conv_weight,
                        dt_bias,
                        a_log,
                        norm: candle_nn::RmsNorm::new(norm_w, 1e-6),
                        num_k_heads: linear_key_heads,
                        num_v_heads: linear_value_heads,
                        head_k_dim: linear_key_dim,
                        head_v_dim: linear_value_dim,
                    },
                ))
            } else if is_qwen35 {
                let q_key = format!("{layer_prefix}.self_attn.q_proj.weight");
                let k_key = format!("{layer_prefix}.self_attn.k_proj.weight");
                let v_key = format!("{layer_prefix}.self_attn.v_proj.weight");
                let o_key = format!("{layer_prefix}.self_attn.o_proj.weight");

                let q_rows = n_heads * head_dim;
                let q_fallback_rows = q_rows.saturating_mul(2);
                let mut w_q = vb_mmap
                    .get((q_rows, bundle.d_model), &q_key)
                    .or_else(|_| vb_mmap.get((q_fallback_rows, bundle.d_model), &q_key))?
                    .to_dtype(DType::F32)?;
                if w_q.dim(0)? > q_rows {
                    w_q = w_q.narrow(0, 0, q_rows)?;
                }
                let w_k = vb_mmap
                    .get((kv_dim, bundle.d_model), &k_key)?
                    .to_dtype(DType::F32)?;
                let w_v = vb_mmap
                    .get((kv_dim, bundle.d_model), &v_key)?
                    .to_dtype(DType::F32)?;
                let w_o = vb_mmap
                    .get((bundle.d_model, q_rows), &o_key)?
                    .to_dtype(DType::F32)?;

                let q_label = format!("l{i}.q");
                let k_label = format!("l{i}.k");
                let v_label = format!("l{i}.v");
                let o_label = format!("l{i}.o");

                let q_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_q,
                    None,
                    &qlora_cfg,
                    vb.pp(&q_label),
                )?;
                let k_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_k,
                    None,
                    &qlora_cfg,
                    vb.pp(&k_label),
                )?;
                let v_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_v,
                    None,
                    &qlora_cfg,
                    vb.pp(&v_label),
                )?;
                let o_proj = QuantizedLinear::from_weight_with_varbuilder(
                    &w_o,
                    None,
                    &qlora_cfg,
                    vb.pp(&o_label),
                )?;

                for (lbl, bk) in [
                    (&q_label, &q_key),
                    (&k_label, &k_key),
                    (&v_label, &v_key),
                    (&o_label, &o_key),
                ] {
                    adapter_layer_order.push(lbl.clone());
                    base_key_map.insert(lbl.clone(), bk.clone());
                }
                let attn = crate::model::Qwen2Attention {
                    q_proj,
                    k_proj,
                    v_proj,
                    o_proj,
                    n_heads,
                    n_kv_heads,
                    head_dim,
                };
                Some(crate::model::Qwen35AttentionBlock::Full(attn))
            } else {
                anyhow::bail!(
                    "Unsupported architecture for training: {:?}",
                    bundle.layout.architecture
                );
            };

            // ── MLP projections ───────────────────────────────────────────────
            let inter_sz = bundle
                .layout
                .intermediate_size
                .unwrap_or(bundle.d_model * 4);
            let gate_key = format!("{layer_prefix}.mlp.gate_proj.weight");
            let up_key = format!("{layer_prefix}.mlp.up_proj.weight");
            let down_key = format!("{layer_prefix}.mlp.down_proj.weight");

            let w_gate = vb_mmap
                .get((inter_sz, bundle.d_model), &gate_key)?
                .to_dtype(DType::F32)?;
            let w_up = vb_mmap
                .get((inter_sz, bundle.d_model), &up_key)?
                .to_dtype(DType::F32)?;
            let w_down = vb_mmap
                .get((bundle.d_model, inter_sz), &down_key)?
                .to_dtype(DType::F32)?;

            let gate_label = format!("l{i}.gate");
            let up_label = format!("l{i}.up");
            let down_label = format!("l{i}.down");

            let gate_proj = QuantizedLinear::from_weight_with_varbuilder(
                &w_gate,
                None,
                &qlora_cfg,
                vb.pp(&gate_label),
            )?;
            let up_proj = QuantizedLinear::from_weight_with_varbuilder(
                &w_up,
                None,
                &qlora_cfg,
                vb.pp(&up_label),
            )?;
            let down_proj = QuantizedLinear::from_weight_with_varbuilder(
                &w_down,
                None,
                &qlora_cfg,
                vb.pp(&down_label),
            )?;

            for (lbl, bk) in [
                (&gate_label, &gate_key),
                (&up_label, &up_key),
                (&down_label, &down_key),
            ] {
                adapter_layer_order.push(lbl.clone());
                base_key_map.insert(lbl.clone(), bk.clone());
            }

            // ── RoPE frequency table ─────────────────────────────────────────
            let rope_dim_base = if is_qwen35 && layer_type == "linear_attention" {
                bundle.layout.linear_key_head_dim.unwrap_or(head_dim)
            } else {
                head_dim
            };
            let rope_dim = if let Some(frac) = bundle.layout.rope_partial_rotary_factor {
                let d = ((rope_dim_base as f64) * frac).round() as usize;
                d.max(2).min(rope_dim_base)
            } else {
                rope_dim_base
            };
            let rope_half = (rope_dim / 2).max(1);
            let inv_candidates = if is_qwen35 && layer_type == "linear_attention" {
                vec![format!("{layer_prefix}.linear_attn.rotary_emb.inv_freq")]
            } else {
                vec![
                    format!("{layer_prefix}.self_attn.rotary_emb.inv_freq"),
                    format!("{layer_prefix}.linear_attn.rotary_emb.inv_freq"),
                ]
            };
            let inv_freq = inv_candidates
                .iter()
                .find_map(|inv_key| {
                    vb_mmap
                        .get((rope_half,), inv_key)
                        .ok()
                        .and_then(|t| t.to_dtype(DType::F32).ok())
                })
                .or_else(|| {
                    synthesize_rope_inv_freq(rope_dim, bundle.layout.rope_theta, &device).ok()
                })
                .or_else(|| synthesized_rope_inv_freq.clone());

            let mlp = crate::model::Qwen2MLP {
                gate_proj,
                up_proj,
                down_proj,
            };
            if let Some(attn) = qwen35_attn {
                model_layers_qwen35.push(crate::model::Qwen35Layer {
                    input_layernorm: ln1,
                    attention: attn,
                    post_attention_layernorm: ln2,
                    mlp,
                    inv_freq,
                });
            } else {
                anyhow::bail!("Internal error: failed to build attention for layer {i}");
            }
        }

        // ── Final norm + LM head ─────────────────────────────────────────────
        let fnorm_w = if bundle.layout.architecture == HfArchitecture::Qwen35 {
            vb_mmap
                .get(bundle.d_model, "model.language_model.norm.weight")
                .or_else(|_| vb_mmap.get(bundle.d_model, "model.norm.weight"))?
                .to_dtype(DType::F32)?
        } else {
            vb_mmap
                .get(bundle.d_model, "model.norm.weight")?
                .to_dtype(DType::F32)?
        };
        let final_norm = candle_nn::RmsNorm::new(fnorm_w, 1e-6);
        let w_lm = wte.to_dtype(DType::F32)?;
        let lm_label = "lm_head".to_string();
        let lm_base = bundle.embed_key.clone();
        let lm_head = QuantizedLinear::from_weight_with_varbuilder(
            &w_lm,
            None,
            &qlora_cfg,
            vb.pp(&lm_label),
        )?;
        adapter_layer_order.push(lm_label.clone());
        base_key_map.insert(lm_label, lm_base);

        (final_norm, lm_head)
    };

    trainer
        .init_optimizer(&[])
        .context("init qlora optimizer")?;

    let model = TrainGraphModel::Qwen35(crate::model::Qwen35Model {
        embed_tokens: wte,
        layers: model_layers_qwen35,
        norm: final_norm,
        lm_head,
    });

    // ── Async VoxDB writer ────────────────────────────────────────────────────
    let run_id = config.run_id.clone().unwrap_or_else(|| {
        format!(
            "qlora_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        )
    });

    let db_tx = db_thread::spawn_training_db_writer(run_id.clone());

    let _ = db_tx.send(TrainingDbEvent::Start {
        run_id: run_id.clone(),
        adapter_tag: config.adapter_tag.clone(),
        model_name: config.base_model.clone(),
        output_dir: out.display().to_string(),
        data_dir: data_dir.display().to_string(),
        planned_steps: Some(total_steps_planned),
    });

    let resume = training_loop::apply_checkpoint_resume(&mut trainer, config, out, pairs.len())
        .context("QLoRA checkpoint resume")?;

    training_loop::preflight_masked_ce_finite(
        &model,
        &bundle,
        &pairs,
        &tokenizer,
        &device,
        config,
        system_prompt,
        resume.start_epoch,
    )
    .context("QLoRA masked CE numeric preflight")?;

    let result = training_loop::run_training_loop(
        &mut trainer,
        model,
        &bundle,
        resume,
        out,
        config,
        pairs,
        eval_pairs,
        &tokenizer,
        &device,
        &db_tx,
        &run_id,
        &device_label,
        &train_path,
        system_prompt,
        &adapter_layer_order,
        &base_key_map,
        total_steps_planned,
        total_optimizer_steps_planned,
        warmup_steps,
        computed_contamination,
    );

    if result.is_err() {
        let _ = db_tx.send(TrainingDbEvent::Failed {
            run_id: run_id.clone(),
            global_step: 0,
        });
    }

    result
}

mod ce_mask_align;
mod checkpoint_mid;
mod db_thread;
mod device_select;
mod epoch_boundary;
mod finalize;
mod training_loop;
mod validation;
