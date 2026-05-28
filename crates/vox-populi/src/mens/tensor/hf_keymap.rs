//! Shared Hugging Face weight **key naming** for transformer blocks (Burn + Candle).
//!
//! Candle QLoRA also uses `super::candle_qlora_weights` for safetensors I/O (key union, logging).

use std::collections::HashSet;

use super::hf_load::{HfArchitecture, HfTransformerLayout};

/// HF key for attention output projection per layer (shape `[d_model, d_model]`).
#[must_use]
pub fn middle_block_projection_key(arch: HfArchitecture, layer: usize) -> String {
    match arch {
        HfArchitecture::Gpt2 => format!("h.{layer}.attn.c_proj.weight"),

        HfArchitecture::Qwen35 => {
            format!("model.language_model.layers.{layer}.self_attn.o_proj.weight")
        }
    }
}

/// Self-attention linear weight keys for one block (inventory / diagnostics).
#[must_use]
pub fn layer_attention_weight_keys(arch: HfArchitecture, layer: usize) -> Vec<String> {
    match arch {
        HfArchitecture::Gpt2 => vec![
            format!("h.{layer}.attn.c_attn.weight"),
            middle_block_projection_key(arch, layer),
        ],

        HfArchitecture::Qwen35 => vec![
            format!("model.language_model.layers.{layer}.self_attn.q_proj.weight"),
            format!("model.language_model.layers.{layer}.self_attn.k_proj.weight"),
            format!("model.language_model.layers.{layer}.self_attn.v_proj.weight"),
            format!("model.language_model.layers.{layer}.self_attn.o_proj.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.in_proj_qkv.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.in_proj_z.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.in_proj_a.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.in_proj_b.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.out_proj.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.conv1d.weight"),
            format!("model.language_model.layers.{layer}.linear_attn.dt_bias"),
            format!("model.language_model.layers.{layer}.linear_attn.A_log"),
            format!("model.language_model.layers.{layer}.linear_attn.norm.weight"),
        ],
    }
}

/// MLP / FFN linear weight keys for one block.
#[must_use]
pub fn layer_mlp_weight_keys(arch: HfArchitecture, layer: usize) -> Vec<String> {
    match arch {
        HfArchitecture::Gpt2 => vec![
            format!("h.{layer}.mlp.c_fc.weight"),
            format!("h.{layer}.mlp.c_proj.weight"),
        ],

        HfArchitecture::Qwen35 => vec![
            format!("model.language_model.layers.{layer}.mlp.gate_proj.weight"),
            format!("model.language_model.layers.{layer}.mlp.up_proj.weight"),
            format!("model.language_model.layers.{layer}.mlp.down_proj.weight"),
        ],
    }
}

/// RoPE (Rotary Position Embedding) keys for one block.
#[must_use]
pub fn layer_rope_weight_keys(arch: HfArchitecture, layer: usize) -> Vec<String> {
    match arch {
        HfArchitecture::Gpt2 => vec![],

        HfArchitecture::Qwen35 => vec![
            format!("model.language_model.layers.{layer}.self_attn.rotary_emb.inv_freq"),
            format!("model.language_model.layers.{layer}.linear_attn.rotary_emb.inv_freq"),
        ],
    }
}

/// Norm keys (RMSNorm or LayerNorm) for one block.
#[must_use]
pub fn layer_norm_weight_keys(arch: HfArchitecture, layer: usize) -> Vec<String> {
    match arch {
        HfArchitecture::Gpt2 => vec![
            format!("h.{layer}.ln_1.weight"),
            format!("h.{layer}.ln_1.bias"),
            format!("h.{layer}.ln_2.weight"),
            format!("h.{layer}.ln_2.bias"),
        ],

        HfArchitecture::Qwen35 => vec![
            format!("model.language_model.layers.{layer}.input_layernorm.weight"),
            format!("model.language_model.layers.{layer}.post_attention_layernorm.weight"),
        ],
    }
}

fn qwen35_layer_type(layout: &HfTransformerLayout, layer: usize) -> &str {
    layout
        .layer_types
        .get(layer)
        .map(String::as_str)
        .unwrap_or("full_attention")
}

fn layer_prefix(layout: &HfTransformerLayout, layer: usize) -> String {
    format!("{}.{}", layout.namespace_prefix, layer)
}

fn middle_projection_key_for_layout(layout: &HfTransformerLayout, layer: usize) -> String {
    match layout.architecture {
        HfArchitecture::Qwen35 => {
            let p = layer_prefix(layout, layer);
            if qwen35_layer_type(layout, layer) == "linear_attention" {
                format!("{p}.linear_attn.out_proj.weight")
            } else {
                format!("{p}.self_attn.o_proj.weight")
            }
        }
        _ => middle_block_projection_key(layout.architecture, layer),
    }
}

fn attention_weight_keys_for_layout(layout: &HfTransformerLayout, layer: usize) -> Vec<String> {
    if layout.architecture != HfArchitecture::Qwen35 {
        return layer_attention_weight_keys(layout.architecture, layer);
    }
    let p = layer_prefix(layout, layer);
    if qwen35_layer_type(layout, layer) == "linear_attention" {
        vec![
            format!("{p}.linear_attn.in_proj_qkv.weight"),
            format!("{p}.linear_attn.in_proj_z.weight"),
            format!("{p}.linear_attn.in_proj_a.weight"),
            format!("{p}.linear_attn.in_proj_b.weight"),
            format!("{p}.linear_attn.out_proj.weight"),
            format!("{p}.linear_attn.conv1d.weight"),
            format!("{p}.linear_attn.dt_bias"),
            format!("{p}.linear_attn.A_log"),
            format!("{p}.linear_attn.norm.weight"),
        ]
    } else {
        vec![
            format!("{p}.self_attn.q_proj.weight"),
            format!("{p}.self_attn.k_proj.weight"),
            format!("{p}.self_attn.v_proj.weight"),
            format!("{p}.self_attn.o_proj.weight"),
        ]
    }
}

fn mlp_weight_keys_for_layout(layout: &HfTransformerLayout, layer: usize) -> Vec<String> {
    if layout.architecture != HfArchitecture::Qwen35 {
        return layer_mlp_weight_keys(layout.architecture, layer);
    }
    let p = layer_prefix(layout, layer);
    vec![
        format!("{p}.mlp.gate_proj.weight"),
        format!("{p}.mlp.up_proj.weight"),
        format!("{p}.mlp.down_proj.weight"),
    ]
}

fn rope_weight_keys_for_layout(layout: &HfTransformerLayout, layer: usize) -> Vec<String> {
    if layout.architecture != HfArchitecture::Qwen35 {
        return layer_rope_weight_keys(layout.architecture, layer);
    }
    let p = layer_prefix(layout, layer);
    if qwen35_layer_type(layout, layer) == "linear_attention" {
        vec![format!("{p}.linear_attn.rotary_emb.inv_freq")]
    } else {
        vec![format!("{p}.self_attn.rotary_emb.inv_freq")]
    }
}

fn norm_weight_keys_for_layout(layout: &HfTransformerLayout, layer: usize) -> Vec<String> {
    if layout.architecture != HfArchitecture::Qwen35 {
        return layer_norm_weight_keys(layout.architecture, layer);
    }
    let p = layer_prefix(layout, layer);
    vec![
        format!("{p}.input_layernorm.weight"),
        format!("{p}.post_attention_layernorm.weight"),
    ]
}

/// All non-embedding tensor name **candidates** for a full transformer (attention + MLP per layer).
#[must_use]
pub fn ordered_full_block_weight_keys(layout: &HfTransformerLayout) -> Vec<String> {
    let mut v = Vec::new();
    for i in 0..layout.dims.n_layer {
        v.extend(attention_weight_keys_for_layout(layout, i));
        v.extend(mlp_weight_keys_for_layout(layout, i));
        v.extend(rope_weight_keys_for_layout(layout, i));
        v.extend(norm_weight_keys_for_layout(layout, i));
    }
    v
}

/// Like [`ordered_full_block_weight_keys`], but for **strict** QLoRA preflight shard checks.
///
/// **Qwen3.5** Hugging Face checkpoints usually omit per-layer `*.rotary_emb.inv_freq`; the Candle
/// trainer synthesizes RoPE from `rope_theta` in `config.json`. Requiring those tensors here caused
/// false failures with `--qlora-require-full-proxy-stack` on official `Qwen/Qwen3.5-*` weights.
#[must_use]
pub fn ordered_full_block_weight_keys_strict_preflight(
    layout: &HfTransformerLayout,
) -> Vec<String> {
    let mut keys = ordered_full_block_weight_keys(layout);
    if layout.architecture == HfArchitecture::Qwen35 {
        keys.retain(|k| !k.ends_with("rotary_emb.inv_freq"));
    }
    keys
}

/// Ordered candidate keys from layer `0 .. n_layer-1` (Candle o_proj proxy stack).
#[must_use]
pub fn ordered_middle_projection_keys(layout: &HfTransformerLayout) -> Vec<String> {
    (0..layout.dims.n_layer)
        .map(|i| middle_projection_key_for_layout(layout, i))
        .collect()
}

/// Keep `keys` in order, only those present in `present`.
#[must_use]
pub fn filter_keys_in_shard(keys: &[String], present: &HashSet<String>) -> Vec<String> {
    keys.iter()
        .filter(|k| present.contains(k.as_str()))
        .cloned()
        .collect()
}

/// First `max_keys` sorted tensor names from an existing key union (no extra I/O).
#[must_use]
pub fn sample_present_keys_sorted_from_present(
    present: &HashSet<String>,
    max_keys: usize,
) -> Vec<String> {
    let mut v: Vec<_> = present.iter().cloned().collect();
    v.sort();
    v.truncate(max_keys);
    v
}

/// Expected middle keys missing from shard union (diagnostics). Lists up to `max_missing` names.
#[must_use]
pub fn missing_middle_keys_report(
    layout: &HfTransformerLayout,
    present: &HashSet<String>,
    max_missing: usize,
) -> Vec<String> {
    ordered_middle_projection_keys(layout)
        .into_iter()
        .filter(|k| !present.contains(k.as_str()))
        .take(max_missing.max(1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mens::tensor::hf_load::{HfArchitecture, HfTransformerLayout};

    fn gpt2_layout_one_layer() -> HfTransformerLayout {
        HfTransformerLayout::from_config_json_str(
            "{\"model_type\":\"gpt2\",\"n_embd\":64,\"n_head\":2,\"n_layer\":1,\"vocab_size\":100}",
        )
        .expect("gpt2 config")
    }

    fn qwen_layout_one_layer() -> HfTransformerLayout {
        HfTransformerLayout::from_config_json_str(
            "{\"model_type\":\"qwen2\",\"hidden_size\":32,\"num_attention_heads\":2,\"num_hidden_layers\":1,\"vocab_size\":50}",
        )
        .expect("qwen config")
    }

    fn qwen_layout_two_layers() -> HfTransformerLayout {
        HfTransformerLayout::from_config_json_str(
            "{\"model_type\":\"qwen2\",\"hidden_size\":32,\"num_attention_heads\":2,\"num_hidden_layers\":2,\"vocab_size\":50}",
        )
        .expect("qwen2 two-layer config")
    }

    #[test]
    fn gpt2_middle_key_matches_hf_naming() {
        assert_eq!(
            middle_block_projection_key(HfArchitecture::Gpt2, 0),
            "h.0.attn.c_proj.weight"
        );
        let layout = gpt2_layout_one_layer();
        let keys = ordered_middle_projection_keys(&layout);
        assert_eq!(keys, vec!["h.0.attn.c_proj.weight"]);
    }

    #[test]
    fn qwen2_middle_key_matches_hf_naming() {
        let layout = qwen_layout_one_layer();
        // middle_projection_key_for_layout uses layout.namespace_prefix, which is
        // "model.layers" for qwen2 (no nested text_config) vs
        // "model.language_model.layers" for qwen3.5 (text_config-wrapped).
        assert_eq!(
            middle_projection_key_for_layout(&layout, 0),
            "model.layers.0.self_attn.o_proj.weight"
        );
    }

    #[test]
    fn qwen2_ordered_middle_keys_track_all_layers() {
        let layout = qwen_layout_two_layers();
        let keys = ordered_middle_projection_keys(&layout);
        assert_eq!(
            keys,
            vec![
                "model.layers.0.self_attn.o_proj.weight",
                "model.layers.1.self_attn.o_proj.weight",
            ]
        );
    }

    #[test]
    fn qwen2_missing_middle_report_respects_max_and_order() {
        use std::collections::HashSet;
        let layout = qwen_layout_two_layers();
        let mut present = HashSet::new();
        present.insert("model.layers.0.self_attn.o_proj.weight".to_string());
        let miss = missing_middle_keys_report(&layout, &present, 8);
        assert_eq!(
            miss,
            vec!["model.layers.1.self_attn.o_proj.weight".to_string()]
        );
    }

    #[test]
    fn qwen2_full_block_keys_include_mlp_projections() {
        let layout = qwen_layout_one_layer();
        let v = ordered_full_block_weight_keys(&layout);
        assert!(v.iter().any(|k| k.contains("mlp.gate_proj")));
        assert!(v.iter().any(|k| k.contains("mlp.up_proj")));
        assert!(v.iter().any(|k| k.contains("mlp.down_proj")));
        assert!(v.iter().any(|k| k.contains("self_attn.q_proj")));
    }

    #[test]
    fn ordered_full_block_includes_attn_and_mlp() {
        let layout = gpt2_layout_one_layer();
        let v = ordered_full_block_weight_keys(&layout);
        assert!(v.iter().any(|k| k.contains("attn.c_attn")));
        assert!(v.iter().any(|k| k.contains("mlp.c_fc")));
    }

    #[test]
    fn qwen35_middle_projection_uses_layer_type() {
        let layout = HfTransformerLayout::from_config_json_str(
            r#"{
              "model_type":"qwen3_5",
              "text_config":{
                "hidden_size":64,
                "num_attention_heads":4,
                "num_hidden_layers":2,
                "vocab_size":1000,
                "layer_types":["linear_attention","full_attention"]
              }
            }"#,
        )
        .expect("qwen3_5 config");
        let keys = ordered_middle_projection_keys(&layout);
        assert_eq!(
            keys,
            vec![
                "model.language_model.layers.0.linear_attn.out_proj.weight",
                "model.language_model.layers.1.self_attn.o_proj.weight",
            ]
        );
    }

    #[test]
    fn qwen35_strict_preflight_keys_omit_synthesizable_rope_inv_freq() {
        let layout = HfTransformerLayout::from_config_json_str(
            r#"{
              "model_type":"qwen3_5",
              "text_config":{
                "hidden_size":64,
                "num_attention_heads":4,
                "num_hidden_layers":2,
                "vocab_size":1000,
                "layer_types":["linear_attention","full_attention"]
              }
            }"#,
        )
        .expect("qwen3_5 config");
        let full = ordered_full_block_weight_keys(&layout);
        let strict = ordered_full_block_weight_keys_strict_preflight(&layout);
        assert!(
            full.iter().any(|k| k.ends_with("rotary_emb.inv_freq")),
            "full key list should still mention inv_freq for docs/diagnostics"
        );
        assert!(
            !strict.iter().any(|k| k.ends_with("rotary_emb.inv_freq")),
            "strict preflight must not require HF inv_freq shards for Qwen3.5"
        );
        assert_eq!(full.len() - strict.len(), 2);
    }
}
