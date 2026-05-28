//! Minimal Hugging Face `config.json` parsing for architecture diagnostics and layout validation.
//! SSOT for `vox-populi` MENS tensor code and `vox-plugin-mens-candle-cuda`.

use std::path::Path;

use anyhow::Context;

use serde::Deserialize;
use serde_json::Value;
use vox_bounded_fs::read_utf8_path_capped;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HfArchitecture {
    Gpt2,
    Qwen35,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDims {
    pub n_embd: usize,
    pub n_head: usize,
    pub n_layer: usize,
    pub vocab_size: usize,
}

/// Structured transformer layout from HF `config.json` (GPT-2, Qwen2-style, qwen3_5-style).
#[derive(Debug, Clone, PartialEq)]
pub struct HfTransformerLayout {
    /// GPT-2 vs stacked causal LM family.
    pub architecture: HfArchitecture,
    pub model_type: String,
    pub architectures: Vec<String>,
    /// Layer namespace prefix used for weight key construction.
    pub namespace_prefix: String,
    pub hidden_size: usize,
    pub num_attention_heads: usize,
    /// Key/value head count for GQA. Falls back to `num_attention_heads` when absent.
    pub num_key_value_heads: usize,
    pub num_hidden_layers: usize,
    pub vocab_size: usize,
    pub intermediate_size: Option<usize>,
    pub max_position_embeddings: Option<usize>,
    pub rope_theta: Option<f64>,
    pub rope_partial_rotary_factor: Option<f64>,
    /// qwen3_5 hybrid stack metadata (`linear_attention` / `full_attention`).
    pub layer_types: Vec<String>,
    pub linear_attention_heads: Option<usize>,
    pub full_attention_heads: Option<usize>,
    /// Optional explicit per-head dim for attention projections.
    pub head_dim: Option<usize>,
    /// qwen3_5 linear-attention key/value and value projection geometry.
    pub linear_num_key_heads: Option<usize>,
    pub linear_num_value_heads: Option<usize>,
    pub linear_key_head_dim: Option<usize>,
    pub linear_value_head_dim: Option<usize>,
    pub linear_conv_kernel_dim: Option<usize>,
    /// Same numbers as the HF fields above, in [`ConfigDims`] shape (legacy / graph code).
    pub dims: ConfigDims,
}

impl HfTransformerLayout {
    /// Parse `config.json` at `path` into a structured layout.
    pub fn from_config_path(path: &Path) -> anyhow::Result<Self> {
        let raw =
            read_utf8_path_capped(path).with_context(|| format!("read {}", path.display()))?;
        Self::from_config_json_str(&raw).with_context(|| format!("parse {}", path.display()))
    }

    /// Parse a `config.json` string (tests + tooling).
    pub fn from_config_json_str(raw: &str) -> anyhow::Result<Self> {
        let v: Value = serde_json::from_str(raw)?;
        Self::from_value(&v)
    }

    fn from_value(v: &Value) -> anyhow::Result<Self> {
        let architecture = classify_hf_architecture(v);
        let model_type = v
            .get("model_type")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let model_l = model_type.to_lowercase();

        if (model_l.contains("qwen2") || model_l.contains("qwen2.5")) && !model_l.contains("qwen3")
        {
            tracing::warn!(
                "Qwen 2.5 / Qwen2 detected: This architecture is DEPRECATED in Vox. \
                 Qwen 3.5 is the new production default. Please migrate your base weights."
            );
        }
        let architectures: Vec<String> = v
            .get("architectures")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        let cfg_source = qwen35_text_config(v, architecture).unwrap_or(v);

        // Llama / Mistral / Qwen2 / Qwen3.5 and many causal LMs.
        if let (Some(h), Some(nh), Some(nl), Some(vs)) = (
            json_usize(cfg_source, "hidden_size"),
            json_usize(cfg_source, "num_attention_heads"),
            json_usize(cfg_source, "num_hidden_layers"),
            json_usize(cfg_source, "vocab_size"),
        ) {
            let dims = ConfigDims {
                n_embd: h,
                n_head: nh,
                n_layer: nl,
                vocab_size: vs,
            };
            let n_kv = json_usize(cfg_source, "num_key_value_heads").unwrap_or(nh);
            let mut layer_types = json_string_vec(cfg_source, "layer_types").unwrap_or_default();
            if layer_types.is_empty() {
                layer_types = vec!["full_attention".to_string(); nl];
            }
            let namespace_prefix = if model_l.contains("qwen3") {
                "model.language_model.layers".to_string()
            } else {
                "model.layers".to_string()
            };
            return Ok(Self {
                architecture,
                model_type,
                architectures,
                namespace_prefix,
                hidden_size: h,
                num_attention_heads: nh,
                num_key_value_heads: n_kv,
                num_hidden_layers: nl,
                vocab_size: vs,
                intermediate_size: json_usize(cfg_source, "intermediate_size"),
                max_position_embeddings: json_usize(cfg_source, "max_position_embeddings"),
                rope_theta: qwen35_rope_theta(cfg_source)
                    .or_else(|| json_f64(cfg_source, "rope_theta")),
                rope_partial_rotary_factor: qwen35_partial_rotary_factor(cfg_source),
                layer_types,
                linear_attention_heads: json_usize(cfg_source, "num_linear_heads"),
                full_attention_heads: json_usize(cfg_source, "num_full_heads"),
                head_dim: json_usize(cfg_source, "head_dim"),
                linear_num_key_heads: json_usize(cfg_source, "linear_num_key_heads"),
                linear_num_value_heads: json_usize(cfg_source, "linear_num_value_heads"),
                linear_key_head_dim: json_usize(cfg_source, "linear_key_head_dim"),
                linear_value_head_dim: json_usize(cfg_source, "linear_value_head_dim"),
                linear_conv_kernel_dim: json_usize(cfg_source, "linear_conv_kernel_dim"),
                dims,
            });
        }

        // GPT-2 style
        if let (Some(n_embd), Some(n_head), Some(n_layer), Some(vs)) = (
            json_usize(v, "n_embd"),
            json_usize(v, "n_head"),
            json_usize(v, "n_layer"),
            json_usize(v, "vocab_size"),
        ) {
            let dims = ConfigDims {
                n_embd,
                n_head,
                n_layer,
                vocab_size: vs,
            };
            return Ok(Self {
                architecture,
                model_type,
                architectures,
                namespace_prefix: "h".to_string(),
                hidden_size: n_embd,
                num_attention_heads: n_head,
                num_key_value_heads: n_head, // GPT-2 is full MHA
                num_hidden_layers: n_layer,
                vocab_size: vs,
                intermediate_size: None, // GPT-2 uses linear 4x expansion in MLP
                max_position_embeddings: json_usize(v, "n_positions"),
                rope_theta: None,
                rope_partial_rotary_factor: None,
                layer_types: vec!["full_attention".to_string(); n_layer],
                linear_attention_heads: None,
                full_attention_heads: Some(n_head),
                head_dim: None,
                linear_num_key_heads: None,
                linear_num_value_heads: None,
                linear_key_head_dim: None,
                linear_value_head_dim: None,
                linear_conv_kernel_dim: None,
                dims,
            });
        }

        anyhow::bail!(
            "config.json: need either (hidden_size, num_attention_heads, num_hidden_layers, vocab_size) \
             or (n_embd, n_head, n_layer, vocab_size)"
        )
    }
}

fn qwen35_text_config(v: &Value, architecture: HfArchitecture) -> Option<&Value> {
    if architecture == HfArchitecture::Qwen35 {
        return v.get("text_config");
    }
    None
}

fn qwen35_rope_parameters(v: &Value) -> Option<&Value> {
    v.get("rope_parameters")
}

fn qwen35_rope_theta(v: &Value) -> Option<f64> {
    qwen35_rope_parameters(v)
        .and_then(|rp| rp.get("rope_theta"))
        .and_then(|x| x.as_f64())
}

fn qwen35_partial_rotary_factor(v: &Value) -> Option<f64> {
    qwen35_rope_parameters(v)
        .and_then(|rp| rp.get("partial_rotary_factor"))
        .and_then(|x| x.as_f64())
}

fn json_usize(v: &Value, key: &str) -> Option<usize> {
    v.get(key).and_then(|x| x.as_u64()).map(|u| u as usize)
}

fn json_f64(v: &Value, key: &str) -> Option<f64> {
    v.get(key).and_then(|x| x.as_f64())
}

fn json_string_vec(v: &Value, key: &str) -> Option<Vec<String>> {
    v.get(key).and_then(|a| a.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect()
    })
}

fn classify_hf_architecture(v: &Value) -> HfArchitecture {
    let arch = v
        .get("architectures")
        .and_then(|a| a.as_array())
        .and_then(|a| a.first())
        .and_then(|x| x.as_str())
        .unwrap_or("");
    let model_type = v.get("model_type").and_then(|x| x.as_str()).unwrap_or("");
    let arch_l = arch.to_lowercase();
    let model_l = model_type.to_lowercase();
    if model_l == "qwen3_5" || model_l == "qwen35" || model_l.contains("qwen3") {
        return HfArchitecture::Qwen35;
    }
    if arch_l.contains("qwen3") {
        return HfArchitecture::Qwen35;
    }
    if arch_l.contains("qwen") || model_l.contains("qwen") || model_l == "qwen2" {
        return HfArchitecture::Qwen35;
    }
    if arch_l.contains("llama") || model_l.contains("llama") || model_l == "llama" {
        return HfArchitecture::Qwen35;
    }
    if arch_l.contains("mistral") || model_l.contains("mistral") {
        return HfArchitecture::Qwen35;
    }
    HfArchitecture::Gpt2
}

impl From<&HfTransformerLayout> for ConfigDims {
    fn from(h: &HfTransformerLayout) -> Self {
        h.dims.clone()
    }
}

/// Layout + coarse [`HfArchitecture`] bucket (GPT-2 vs stacked causal LM field layout).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTransformerLayout {
    pub architecture: HfArchitecture,
    pub dims: ConfigDims,
    /// First entry in `architectures[]`, or `model_type` when empty.
    pub primary_architecture: String,
}

/// Parse `config.json` and classify architecture for legacy CLI / tests.
pub fn parse_transformer_layout(path: &Path) -> anyhow::Result<ParsedTransformerLayout> {
    let layout = HfTransformerLayout::from_config_path(path)?;
    let architecture = layout.architecture;
    let primary_architecture = layout
        .architectures
        .first()
        .cloned()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| layout.model_type.clone());
    let dims = layout.dims.clone();
    Ok(ParsedTransformerLayout {
        architecture,
        dims,
        primary_architecture,
    })
}

#[derive(Debug, Deserialize)]
struct Gpt2Cfg {
    n_embd: usize,
    n_head: usize,
    n_layer: usize,
    vocab_size: usize,
}

#[derive(Debug, Deserialize)]
struct StackedCausalCfg {
    hidden_size: usize,
    num_attention_heads: usize,
    num_hidden_layers: usize,
    vocab_size: usize,
}

pub fn detect_hf_architecture(config_path: &Path) -> anyhow::Result<HfArchitecture> {
    let raw = read_utf8_path_capped(config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    let v: Value = serde_json::from_str(&raw)?;
    Ok(classify_hf_architecture(&v))
}

fn gpt2_config_from_path(config_path: &Path) -> anyhow::Result<Gpt2Cfg> {
    let raw = read_utf8_path_capped(config_path)?;
    Ok(serde_json::from_str(&raw)?)
}

fn stacked_causal_config_from_path(config_path: &Path) -> anyhow::Result<StackedCausalCfg> {
    let raw = read_utf8_path_capped(config_path)?;
    if let Ok(parsed) = serde_json::from_str::<StackedCausalCfg>(&raw) {
        return Ok(parsed);
    }
    let v: Value = serde_json::from_str(&raw)?;
    if let Some(text) = v.get("text_config") {
        return Ok(serde_json::from_value(text.clone())?);
    }
    anyhow::bail!(
        "stacked-causal config requires hidden_size/num_attention_heads/num_hidden_layers/vocab_size"
    )
}

/// Load [`ConfigDims`] from a Hugging Face `config.json` for `arch`.
///
/// Prefer [`HfTransformerLayout::from_config_path`] for new code; this keeps older call sites stable.
pub fn config_dims_for_architecture(
    config_path: &Path,
    arch: HfArchitecture,
) -> anyhow::Result<ConfigDims> {
    match arch {
        HfArchitecture::Gpt2 => Ok(gpt2_config_from_path(config_path)?.into()),
        HfArchitecture::Qwen35 => Ok(stacked_causal_config_from_path(config_path)?.into()),
    }
}

impl From<Gpt2Cfg> for ConfigDims {
    fn from(c: Gpt2Cfg) -> Self {
        Self {
            n_embd: c.n_embd,
            n_head: c.n_head,
            n_layer: c.n_layer,
            vocab_size: c.vocab_size,
        }
    }
}

impl From<StackedCausalCfg> for ConfigDims {
    fn from(c: StackedCausalCfg) -> Self {
        Self {
            n_embd: c.hidden_size,
            n_head: c.num_attention_heads,
            n_layer: c.num_hidden_layers,
            vocab_size: c.vocab_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{HfArchitecture, HfTransformerLayout};

    #[test]
    fn parses_qwen35_nested_text_config_layout() {
        let raw = r#"{
            "model_type":"qwen3_5",
            "architectures":["Qwen3_5ForCausalLM"],
            "text_config":{
                "hidden_size":1024,
                "num_attention_heads":16,
                "num_key_value_heads":8,
                "num_hidden_layers":4,
                "vocab_size":151936,
                "intermediate_size":2816,
                "max_position_embeddings":262144,
                "layer_types":["linear_attention","full_attention","linear_attention","full_attention"]
            }
        }"#;
        let layout = HfTransformerLayout::from_config_json_str(raw).expect("qwen3_5 parse");
        assert_eq!(layout.architecture, HfArchitecture::Qwen35);
        assert_eq!(layout.namespace_prefix, "model.language_model.layers");
        assert_eq!(layout.num_hidden_layers, 4);
        assert_eq!(layout.layer_types.len(), 4);
        assert_eq!(layout.layer_types[0], "linear_attention");
    }

    #[test]
    fn qwen35_defaults_layer_types_to_full_attention() {
        let raw = r#"{
            "model_type":"qwen3_5",
            "text_config":{
                "hidden_size":512,
                "num_attention_heads":8,
                "num_hidden_layers":2,
                "vocab_size":32000
            }
        }"#;
        let layout = HfTransformerLayout::from_config_json_str(raw).expect("qwen3_5 parse");
        assert_eq!(layout.layer_types, vec!["full_attention", "full_attention"]);
    }
}
