//! HF file checks before native Candle QLoRA (`vox mens train --backend qlora`).
//!
//! Copied verbatim from `vox-populi/src/mens/tensor/qlora_preflight.rs` (SP3 sub-batch B).
//!
//! Import path changes made:
//! - `super::hf_load::{HfArchitecture, HfTransformerLayout}` → `crate::hf_layout::{…}`
//! - `super::operator_messages` → `crate::operator_messages`
//! - `super::training_config::{LoraTrainingConfig, MensTokenizerMode}` → `crate::config::{…}`
//! - `super::candle_qlora_weights::*` → `crate::qlora_weights::*`
//! - `super::train_log::*` inlined as local `train_log_*` wrappers (3-line module).
//!
//! **Tokenizer contract:** `--backend qlora` requires [`MensTokenizerMode::Hf`] and a real
//! Hugging Face `tokenizer.json` (download via `--model <repo>` or explicit path). The Burn LoRA
//! path (`--backend lora`) uses the Vox tokenizer stack — do not mix modes; CLI enforces this before
//! training dispatches.
//!
//! **Weights:** requires HF `config.json` plus safetensors shards listing a supported **embedding**
//! matrix (`wte.weight` or `model.embed_tokens.weight`) for vocab / `d_model` discovery.

use std::io::Read;
use std::path::PathBuf;

use anyhow::Context;
use safetensors::SafeTensors;

use crate::config::{LoraTrainingConfig, MensTokenizerMode};
use crate::hf_layout::{HfArchitecture, HfTransformerLayout};
use crate::operator_messages::{
    self, QLORA_NEEDS_HF_WEIGHTS, QLORA_NEEDS_TOKENIZER_PATH, QLORA_REQUIRES_HF_TOKENIZER,
};

// ---------------------------------------------------------------------------
// Inlined train_log helpers (vox-populi/src/mens/tensor/train_log.rs — 3 fns)
// ---------------------------------------------------------------------------

fn train_log_info(msg: &str) {
    tracing::info!(target: "vox_mens_train", "{}", msg);
}

fn train_log_warn(msg: &str) {
    tracing::warn!(target: "vox_mens_train", "{}", msg);
}

fn train_log_debug(msg: &str) {
    tracing::debug!(target: "vox_mens_train", "{}", msg);
}

// ---------------------------------------------------------------------------

/// Prefer `wte` (GPT-2) before `embed_tokens` when both exist anywhere in the shard list.
const EMBED_KEYS: &[&str] = &[
    "wte.weight",
    "model.embed_tokens.weight",
    "model.language_model.embed_tokens.weight",
];

/// Read only the SafeTensors header JSON (without tensor data) and return a fake-padded
/// buffer that satisfies `SafeTensors::deserialize`'s offset-range validation.
///
/// The safetensors format is: `[8 bytes: N][N bytes: JSON][tensor data]`.
/// `SafeTensors::deserialize` checks that every tensor's `data_offsets.1` lies within the
/// buffer — it fails with `MetadataIncompleteBuffer` on a header-only slice. We pad the
/// buffer with zero bytes up to the last declared offset so the validation passes without
/// loading multi-GB weight data.
fn read_safetensors_header(path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    use std::collections::HashMap;
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open weight shard {}", path.display()))?;
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)
        .with_context(|| format!("read header length from {}", path.display()))?;
    let header_len = u64::from_le_bytes(len_buf) as usize;
    let mut json_buf = vec![0u8; header_len];
    file.read_exact(&mut json_buf)
        .with_context(|| format!("read header JSON from {}", path.display()))?;

    // Parse the header JSON to find the maximum tensor data end-offset.
    // Format: `{"key": {"dtype":"F32","shape":[…],"data_offsets":[start,end]}, …}`.
    let max_end: usize = {
        #[derive(serde::Deserialize)]
        struct TensorMeta {
            data_offsets: Option<[usize; 2]>,
        }
        let map: HashMap<String, TensorMeta> =
            serde_json::from_slice(&json_buf).unwrap_or_default();
        map.values()
            .filter_map(|m| m.data_offsets)
            .map(|[_s, e]| e)
            .max()
            .unwrap_or(0)
    };

    // Build the padded buffer: [8-byte prefix][header JSON][zero-filled tensor region].
    let total = 8 + header_len + max_end;
    let mut buf = vec![0u8; total];
    buf[..8].copy_from_slice(&len_buf);
    buf[8..8 + header_len].copy_from_slice(&json_buf);
    Ok(buf)
}

/// Scan all shards: first **valid** rank-2 table in key order (`wte` then `embed_tokens`).
/// If a preferred key exists but is not rank-2, fail (do not fall back silently).
fn resolve_embedding_table(
    weight_paths: &[std::path::PathBuf],
) -> anyhow::Result<(String, usize, usize)> {
    for &key in EMBED_KEYS {
        let mut bad_for_key: Vec<String> = Vec::new();
        for wp in weight_paths {
            let header = read_safetensors_header(wp)?;
            let st = SafeTensors::deserialize(&header)
                .with_context(|| format!("parse handle {}", wp.display()))?;
            let Ok(t) = st.tensor(key) else {
                continue;
            };
            let shape = t.shape();
            if shape.len() == 2 {
                return Ok((key.into(), shape[0], shape[1]));
            }
            bad_for_key.push(format!("{} shape {:?}", wp.display(), shape));
        }
        if !bad_for_key.is_empty() {
            anyhow::bail!(
                "Embedding tensor `{key}` is present but not a 2D matrix [vocab_size, hidden_size]: {}. \
                 Next: use HF-compatible `wte.weight` or `model.embed_tokens.weight` checkpoints only.",
                bad_for_key.join("; ")
            );
        }
    }

    let mut keys_preview = Vec::new();
    if let Some(wp) = weight_paths.first()
        && let Ok(bytes) = std::fs::read(wp)
        && let Ok(st) = SafeTensors::deserialize(&bytes)
    {
        keys_preview = st
            .tensors()
            .iter()
            .map(|(k, _)| k.clone())
            .take(24)
            .collect();
    }
    anyhow::bail!(
        "No supported embedding tensor in safetensors (need one of {:?}). First-shard key sample: {:?}. \
         Next: confirm shards are from the base model and include the token embedding table.",
        EMBED_KEYS,
        keys_preview
    )
}

/// Resolved embedding table metadata for the Candle trainer.
#[derive(Debug, Clone)]
pub struct QloraEmbedBundle {
    pub weight_paths: Vec<PathBuf>,
    /// HF `config.json` path (used for layout parse and tracing; keep for operator debugging).
    pub config_path: PathBuf,
    pub tokenizer_path: PathBuf,
    pub embed_key: String,
    pub vocab: usize,
    pub d_model: usize,
    /// HF `config.json` layout (layers, heads, model family).
    pub layout: HfTransformerLayout,
}

fn qwen35_rope_candidates(layout: &HfTransformerLayout, layer_idx: usize) -> Vec<String> {
    let prefix = format!("{}.{}", layout.namespace_prefix, layer_idx);
    vec![
        format!("{prefix}.self_attn.rotary_emb.inv_freq"),
        format!("{prefix}.linear_attn.rotary_emb.inv_freq"),
    ]
}

fn warn_on_missing_qwen35_rope_keys(
    layout: &HfTransformerLayout,
    present: &std::collections::HashSet<String>,
) {
    if layout.architecture != HfArchitecture::Qwen35 {
        return;
    }
    let mut missing: Vec<String> = Vec::new();
    for layer_idx in 0..layout.num_hidden_layers {
        let candidates = qwen35_rope_candidates(layout, layer_idx);
        if !candidates.iter().any(|k| present.contains(k)) {
            missing.push(format!("layer {layer_idx}: one of {:?}", candidates));
        }
    }
    if !missing.is_empty() {
        train_log_debug(&format!(
            "qwen3_5 RoPE tensors (`inv_freq`) are missing for {} layer(s) in shards; trainer will synthesize rotary frequencies from config `rope_theta` when available. Missing (up to 8): {:?}",
            missing.len(),
            missing.into_iter().take(8).collect::<Vec<_>>()
        ));
    }
}

fn first_tensor_shape(weight_paths: &[PathBuf], key: &str) -> anyhow::Result<Option<Vec<usize>>> {
    for wp in weight_paths {
        let header = read_safetensors_header(wp)?;
        let st =
            SafeTensors::deserialize(&header).with_context(|| format!("parse {}", wp.display()))?;
        if let Ok(t) = st.tensor(key) {
            return Ok(Some(t.shape().to_vec()));
        }
    }
    Ok(None)
}

fn expect_shape_exact(
    weight_paths: &[PathBuf],
    key: &str,
    expected: &[usize],
) -> anyhow::Result<()> {
    if let Some(found) = first_tensor_shape(weight_paths, key)?
        && found != expected
    {
        anyhow::bail!(
            "qwen3_5 shape mismatch for `{}`: expected {:?}, found {:?}",
            key,
            expected,
            found
        );
    }
    Ok(())
}

fn expect_shape_one_of(
    weight_paths: &[PathBuf],
    key: &str,
    expected: &[Vec<usize>],
) -> anyhow::Result<()> {
    if let Some(found) = first_tensor_shape(weight_paths, key)?
        && !expected.iter().any(|e| e == &found)
    {
        anyhow::bail!(
            "qwen3_5 shape mismatch for `{}`: expected one of {:?}, found {:?}",
            key,
            expected,
            found
        );
    }
    Ok(())
}

fn validate_qwen35_linear_shapes(bundle: &QloraEmbedBundle) -> anyhow::Result<()> {
    if bundle.layout.architecture != HfArchitecture::Qwen35 {
        return Ok(());
    }
    let n_heads = bundle.layout.num_attention_heads.max(1);
    let head_dim = bundle
        .layout
        .head_dim
        .unwrap_or(bundle.layout.hidden_size / n_heads);
    let key_heads = bundle.layout.linear_num_key_heads.unwrap_or(n_heads);
    let value_heads = bundle.layout.linear_num_value_heads.unwrap_or(n_heads);
    let key_dim = bundle.layout.linear_key_head_dim.unwrap_or(head_dim);
    let value_dim = bundle.layout.linear_value_head_dim.unwrap_or(head_dim);
    let value_total = value_heads * value_dim;
    let qkv_rows = (key_heads * key_dim * 2) + value_total;
    let conv_k = bundle.layout.linear_conv_kernel_dim.unwrap_or(4);

    for i in 0..bundle.layout.num_hidden_layers {
        let p = format!("{}.{}", bundle.layout.namespace_prefix, i);
        let ty = bundle
            .layout
            .layer_types
            .get(i)
            .map(String::as_str)
            .unwrap_or("full_attention");
        if ty != "linear_attention" {
            continue;
        }
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.in_proj_qkv.weight"),
            &[qkv_rows, bundle.layout.hidden_size],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.in_proj_z.weight"),
            &[value_total, bundle.layout.hidden_size],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.in_proj_a.weight"),
            &[value_heads, bundle.layout.hidden_size],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.in_proj_b.weight"),
            &[value_heads, bundle.layout.hidden_size],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.out_proj.weight"),
            &[bundle.layout.hidden_size, value_total],
        )?;
        expect_shape_one_of(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.conv1d.weight"),
            &[vec![qkv_rows, 1, conv_k], vec![qkv_rows, conv_k]],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.dt_bias"),
            &[value_heads],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.A_log"),
            &[value_heads],
        )?;
        expect_shape_exact(
            &bundle.weight_paths,
            &format!("{p}.linear_attn.norm.weight"),
            &[value_dim],
        )?;
    }
    Ok(())
}

/// Fail fast unless HF tokenizer + safetensors shards contain a supported embedding matrix.
pub fn preflight_native_qlora(config: &LoraTrainingConfig) -> anyhow::Result<QloraEmbedBundle> {
    if !matches!(config.tokenizer_mode, MensTokenizerMode::Hf) {
        anyhow::bail!(QLORA_REQUIRES_HF_TOKENIZER);
    }
    let Some(tok_path) = config.tokenizer_path.as_ref() else {
        anyhow::bail!(QLORA_NEEDS_TOKENIZER_PATH);
    };
    if !tok_path.is_file() {
        anyhow::bail!(operator_messages::tokenizer_not_a_file(
            &tok_path.display().to_string()
        ));
    }
    let Some((weight_paths, config_path)) = config.base_model_paths.as_ref() else {
        anyhow::bail!(QLORA_NEEDS_HF_WEIGHTS);
    };
    if !config_path.is_file() {
        anyhow::bail!(operator_messages::hf_config_missing(
            &config_path.display().to_string()
        ));
    }
    if weight_paths.is_empty() {
        anyhow::bail!(operator_messages::no_safetensors_shards());
    }

    let (embed_key, vocab, d_model) = resolve_embedding_table(weight_paths)?;

    let layout = HfTransformerLayout::from_config_path(config_path).with_context(|| {
        format!(
            "parse HF transformer layout from {} (next: use the `config.json` shipped with these safetensors)",
            config_path.display()
        )
    })?;
    if layout.hidden_size != d_model {
        anyhow::bail!(
            "Embedding hidden size from tensor `{}` is {} (second dim) but config.json `hidden_size` / `n_embd` is {}. \
             Next: use the `config.json` from the same HF revision as the safetensors.",
            embed_key,
            d_model,
            layout.hidden_size
        );
    }
    if layout.vocab_size != vocab {
        anyhow::bail!(
            "Embedding vocab from tensor `{}` is {} but config.json `vocab_size` is {}. \
             Next: align tokenizer and base model revision, or fix mixed checkpoint paths.",
            embed_key,
            vocab,
            layout.vocab_size
        );
    }

    let mut bundle = QloraEmbedBundle {
        weight_paths: weight_paths.clone(),
        config_path: config_path.clone(),
        tokenizer_path: tok_path.clone(),
        embed_key,
        vocab,
        d_model,
        layout,
    };

    let present = crate::qlora_weights::tensor_keys_union(&bundle.weight_paths)
        .context("read safetensors key union for Candle QLoRA preflight")?;
    if bundle.layout.architecture == HfArchitecture::Qwen35 {
        let configured_probe = format!(
            "{}.0.input_layernorm.weight",
            bundle.layout.namespace_prefix
        );
        if !present.contains(&configured_probe)
            && present.contains("model.layers.0.input_layernorm.weight")
        {
            train_log_warn(
                "qwen3_5 layout namespace override: using `model.layers` keys from shards instead of `model.language_model.layers`.",
            );
            bundle.layout.namespace_prefix = "model.layers".to_string();
        }
    }
    let cov = crate::qlora_weights::middle_projection_coverage(&bundle.layout, &present);
    let n_mid = cov.expected;
    let matched_mid = cov.matched;
    let proxy_complete = n_mid == 0 || cov.complete;

    if config.qlora_require_full_proxy_stack && n_mid > 0 && !proxy_complete {
        let missing = crate::hf_keymap::missing_middle_keys_report(&bundle.layout, &present, 32);
        anyhow::bail!(
            "Candle QLoRA strict proxy stack: need all {} per-layer output-projection weights in shards; found {}. \
             Missing (up to 32): {:?}. \
             Next: pass complete HF `model*.safetensors` for full-stack training. \
             See {}.",
            n_mid,
            matched_mid,
            missing,
            operator_messages::POPULI_TRAINING_SSOT_RELPATH
        );
    }

    if bundle.layout.architecture == HfArchitecture::Qwen35 {
        let bad_layer_types: Vec<String> = bundle
            .layout
            .layer_types
            .iter()
            .enumerate()
            .filter(|(_, ty)| {
                let t = ty.as_str();
                t != "full_attention" && t != "linear_attention"
            })
            .map(|(idx, ty)| format!("layer {idx}: {ty}"))
            .collect();
        if !bad_layer_types.is_empty() {
            anyhow::bail!(
                "Unsupported qwen3_5 layer type(s): {:?}. Supported values are `full_attention` and `linear_attention`.",
                bad_layer_types
            );
        }
        if bundle.layout.layer_types.len() != bundle.layout.num_hidden_layers {
            anyhow::bail!(
                "qwen3_5 config mismatch: layer_types length {} does not match num_hidden_layers {}.",
                bundle.layout.layer_types.len(),
                bundle.layout.num_hidden_layers
            );
        }
        validate_qwen35_linear_shapes(&bundle)?;
        warn_on_missing_qwen35_rope_keys(&bundle.layout, &present);
    }

    let full = crate::hf_keymap::ordered_full_block_weight_keys_strict_preflight(&bundle.layout);
    let matched_full = full.iter().filter(|k| present.contains(k.as_str())).count();
    if config.qlora_require_full_proxy_stack && matched_full < full.len() {
        let missing_full: Vec<String> = full
            .iter()
            .filter(|k| !present.contains(k.as_str()))
            .take(32)
            .cloned()
            .collect();
        anyhow::bail!(
            "Candle QLoRA strict full-graph preflight: only {}/{} required block tensors were found. \
             Missing (up to 32): {:?}. \
             Next: pass a complete HF shard set from one revision (config + tokenizer + safetensors).",
            matched_full,
            full.len(),
            missing_full
        );
    }
    let sample = crate::hf_keymap::sample_present_keys_sorted_from_present(&present, 24);
    tracing::info!(
        target: "vox_populi::mens::qlora_preflight",
        hf_config_path = %bundle.config_path.display(),
        model_type = %bundle.layout.model_type,
        architecture = ?bundle.layout.architecture,
        hidden_size = bundle.layout.hidden_size,
        n_layer = bundle.layout.num_hidden_layers,
        weight_shards = bundle.weight_paths.len(),
        middle_projection_matched = matched_mid,
        middle_projection_expected = n_mid,
        full_block_key_matched = matched_full,
        full_block_key_candidates = full.len(),
        proxy_stack_complete = proxy_complete,
        embed_key = %bundle.embed_key,
        "qlora preflight coverage summary"
    );
    tracing::debug!(
        target: "vox_populi::mens::qlora_preflight",
        ?sample,
        "safetensors key sample (sorted, first 24)"
    );

    train_log_info(&format!(
        "QLoRA preflight OK — model_type={} hidden_size={} num_layers={} middle_projection_keys={}/{} \
         proxy_stack_complete={} embed_key={} weight_shards={}",
        bundle.layout.model_type,
        bundle.layout.hidden_size,
        bundle.layout.num_hidden_layers,
        matched_mid,
        n_mid,
        proxy_complete,
        bundle.embed_key,
        bundle.weight_paths.len(),
    ));

    Ok(bundle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use safetensors::serialize_to_file;
    use safetensors::tensor::{Dtype, TensorView};
    use tempfile::tempdir;

    #[test]
    fn preflight_rejects_vox_tokenizer_mode() {
        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Vox,
            ..Default::default()
        };
        let msg = preflight_native_qlora(&c).unwrap_err().to_string();
        assert!(
            msg.contains("tokenizer hf") || msg.contains("--tokenizer hf"),
            "{msg}"
        );
    }

    #[test]
    fn preflight_rejects_missing_tokenizer_file() {
        let mut c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            ..Default::default()
        };
        c.tokenizer_path = Some(PathBuf::from("/nonexistent/tokenizer.json"));
        let msg = preflight_native_qlora(&c).unwrap_err().to_string();
        assert!(
            msg.contains("not an existing file") || msg.contains("Tokenizer path"),
            "{msg}"
        );
    }

    fn write_minimal_safetensors(path: &std::path::Path, key: &str, vocab: usize, d_model: usize) {
        let n_bytes = vocab * d_model * 4;
        let raw: Vec<u8> = vec![0u8; n_bytes];
        let tv =
            TensorView::new(Dtype::F32, vec![vocab, d_model], raw.as_slice()).expect("tensor view");
        serialize_to_file([(key, tv)], None, path).expect("serialize safetensors");
    }

    fn write_tensor_view(path: &std::path::Path, key: &str, shape: Vec<usize>, raw: &[u8]) {
        let tv = TensorView::new(Dtype::F32, shape, raw).expect("tensor view");
        serialize_to_file([(key, tv)], None, path).expect("serialize safetensors");
    }

    #[test]
    fn preflight_ok_with_wte_weight() {
        let dir = tempdir().expect("tempdir");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&tok, "{}").expect("tokenizer placeholder");
        let cfg_path = dir.path().join("config.json");
        std::fs::write(
            &cfg_path,
            "{\"model_type\":\"gpt2\",\"n_embd\":19,\"n_head\":1,\"n_layer\":1,\"vocab_size\":17}",
        )
        .expect("config");
        let st = dir.path().join("shard.safetensors");
        write_minimal_safetensors(&st, "wte.weight", 17, 19);

        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            tokenizer_path: Some(tok),
            base_model_paths: Some((vec![st], cfg_path)),
            ..Default::default()
        };
        let b = preflight_native_qlora(&c).expect("preflight");
        assert_eq!(b.embed_key, "wte.weight");
        assert_eq!(b.vocab, 17);
        assert_eq!(b.d_model, 19);
    }

    #[test]
    fn preflight_prefers_wte_when_it_appears_only_on_second_shard() {
        let dir = tempdir().expect("tempdir");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&tok, "{}").expect("tokenizer");
        let cfg_path = dir.path().join("config.json");
        std::fs::write(
            &cfg_path,
            "{\"model_type\":\"gpt2\",\"n_embd\":19,\"n_head\":1,\"n_layer\":1,\"vocab_size\":17}",
        )
        .expect("config");
        let st_a = dir.path().join("a.safetensors");
        write_minimal_safetensors(&st_a, "model.embed_tokens.weight", 17, 19);
        let st_b = dir.path().join("b.safetensors");
        write_minimal_safetensors(&st_b, "wte.weight", 17, 19);

        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            tokenizer_path: Some(tok),
            base_model_paths: Some((vec![st_a, st_b], cfg_path)),
            ..Default::default()
        };
        let b = preflight_native_qlora(&c).expect("preflight");
        assert_eq!(b.embed_key, "wte.weight");
    }

    #[test]
    fn preflight_rejects_rank_mismatch_on_preferred_embed_key() {
        let dir = tempdir().expect("tempdir");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&tok, "{}").expect("tokenizer");
        let cfg_path = dir.path().join("config.json");
        std::fs::write(
            &cfg_path,
            "{\"model_type\":\"gpt2\",\"n_embd\":19,\"n_head\":1,\"n_layer\":1,\"vocab_size\":17}",
        )
        .expect("config");
        let st = dir.path().join("bad.safetensors");
        let shape = vec![17usize, 19, 2];
        let n = shape.iter().product::<usize>();
        let raw: Vec<u8> = vec![0u8; n * 4];
        write_tensor_view(&st, "wte.weight", shape, &raw);

        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            tokenizer_path: Some(tok),
            base_model_paths: Some((vec![st], cfg_path)),
            ..Default::default()
        };
        let msg = preflight_native_qlora(&c).unwrap_err().to_string();
        assert!(
            msg.contains("wte.weight") && msg.contains("2D matrix"),
            "{msg}"
        );
    }

    #[test]
    fn preflight_ok_with_embed_tokens_weight() {
        let dir = tempdir().expect("tempdir");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&tok, "{}").expect("tokenizer");
        let cfg_path = dir.path().join("config.json");
        std::fs::write(
            &cfg_path,
            "{\"model_type\":\"qwen2\",\"hidden_size\":7,\"num_attention_heads\":1,\"num_hidden_layers\":1,\"vocab_size\":5}",
        )
        .expect("config");
        let st = dir.path().join("m.safetensors");
        write_minimal_safetensors(&st, "model.embed_tokens.weight", 5, 7);

        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            tokenizer_path: Some(tok),
            base_model_paths: Some((vec![st], cfg_path)),
            ..Default::default()
        };
        let b = preflight_native_qlora(&c).expect("preflight");
        assert_eq!(b.embed_key, "model.embed_tokens.weight");
        assert_eq!(b.vocab, 5);
        assert_eq!(b.d_model, 7);
        assert_eq!(b.layout.hidden_size, 7);
        assert_eq!(b.layout.model_type, "qwen2");
    }

    #[test]
    fn preflight_ok_with_qwen35_language_model_embed_tokens_weight() {
        let dir = tempdir().expect("tempdir");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&tok, "{}").expect("tokenizer");
        let cfg_path = dir.path().join("config.json");
        std::fs::write(
            &cfg_path,
            r#"{"model_type":"qwen3_5","text_config":{"hidden_size":7,"num_attention_heads":1,"num_hidden_layers":1,"vocab_size":5,"layer_types":["full_attention"]}}"#,
        )
        .expect("config");
        let st_embed = dir.path().join("embed.safetensors");
        write_minimal_safetensors(&st_embed, "model.language_model.embed_tokens.weight", 5, 7);
        let st_rope = dir.path().join("rope.safetensors");
        let rope_raw: Vec<u8> = vec![0u8; 3 * 4];
        write_tensor_view(
            &st_rope,
            "model.language_model.layers.0.self_attn.rotary_emb.inv_freq",
            vec![3],
            &rope_raw,
        );

        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            tokenizer_path: Some(tok),
            base_model_paths: Some((vec![st_embed, st_rope], cfg_path)),
            ..Default::default()
        };
        let b = preflight_native_qlora(&c).expect("preflight");
        assert_eq!(b.embed_key, "model.language_model.embed_tokens.weight");
        assert_eq!(b.layout.architecture, HfArchitecture::Qwen35);
    }

    #[test]
    fn preflight_strict_rejects_missing_o_proj() {
        let dir = tempdir().expect("tempdir");
        let tok = dir.path().join("tokenizer.json");
        std::fs::write(&tok, "{}").expect("tokenizer");
        let cfg_path = dir.path().join("config.json");
        std::fs::write(
            &cfg_path,
            "{\"model_type\":\"qwen2\",\"hidden_size\":7,\"num_attention_heads\":1,\"num_hidden_layers\":2,\"vocab_size\":5}",
        )
        .expect("config");
        let st = dir.path().join("m.safetensors");
        write_minimal_safetensors(&st, "model.embed_tokens.weight", 5, 7);

        let c = LoraTrainingConfig {
            tokenizer_mode: MensTokenizerMode::Hf,
            tokenizer_path: Some(tok),
            base_model_paths: Some((vec![st], cfg_path)),
            qlora_require_full_proxy_stack: true,
            ..Default::default()
        };
        let msg = preflight_native_qlora(&c).unwrap_err().to_string();
        assert!(
            msg.contains("strict proxy stack") || msg.contains("o_proj"),
            "{msg}"
        );
    }
}
