//! Quantization policy: tensor-role classification, named k-quant mixtures,
//! and GGML block-size alignment fallback.

use candle_core::quantized::GgmlDType;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TensorRole {
    Embedding,
    Output,
    DownProj,
    VProj,
    Matrix,
    KeepF32,
}

impl TensorRole {
    pub fn from_key(key: &str) -> Self {
        let k = key.to_ascii_lowercase();
        if k.ends_with("layernorm.weight")
            || k.ends_with(".norm.weight")
            || k == "model.language_model.norm.weight"
            || k.ends_with(".a_log")
            || k.ends_with(".dt_bias")
            || k.ends_with(".bias")
            || k.contains("inv_freq")
        {
            return TensorRole::KeepF32;
        }
        if k.contains("embed_tokens") {
            return TensorRole::Embedding;
        }
        if k.contains("lm.head") || k.contains("lm_head") {
            return TensorRole::Output;
        }
        if k.ends_with("down_proj.weight") {
            return TensorRole::DownProj;
        }
        if k.ends_with("v_proj.weight") {
            return TensorRole::VProj;
        }
        TensorRole::Matrix
    }
}

#[derive(Debug, Clone)]
pub enum QuantMixture {
    Q4KM,
    Q5KM,
    Q6K,
    Q8_0,
    Manual(BTreeMap<TensorRole, GgmlDType>),
}

impl QuantMixture {
    pub fn target_for(&self, role: TensorRole) -> Option<GgmlDType> {
        if role == TensorRole::KeepF32 {
            return None;
        }
        match self {
            QuantMixture::Q4KM => Some(match role {
                TensorRole::DownProj
                | TensorRole::VProj
                | TensorRole::Embedding
                | TensorRole::Output => GgmlDType::Q6K,
                _ => GgmlDType::Q4K,
            }),
            QuantMixture::Q5KM => Some(match role {
                TensorRole::DownProj
                | TensorRole::VProj
                | TensorRole::Embedding
                | TensorRole::Output => GgmlDType::Q6K,
                _ => GgmlDType::Q5K,
            }),
            QuantMixture::Q6K => Some(GgmlDType::Q6K),
            QuantMixture::Q8_0 => Some(GgmlDType::Q8_0),
            QuantMixture::Manual(m) => m.get(&role).copied(),
        }
    }
}

/// Enforce GGML block-size alignment against the tensor's last dimension.
pub fn resolve_dtype(target: GgmlDType, last_dim: usize) -> GgmlDType {
    let is_kquant = matches!(
        target,
        GgmlDType::Q2K
            | GgmlDType::Q3K
            | GgmlDType::Q4K
            | GgmlDType::Q5K
            | GgmlDType::Q6K
            | GgmlDType::Q8K
    );
    if is_kquant {
        if last_dim.is_multiple_of(256) {
            return target;
        }
        if last_dim.is_multiple_of(32) {
            return GgmlDType::Q8_0;
        }
        return GgmlDType::F32;
    }
    if last_dim.is_multiple_of(32) {
        target
    } else {
        GgmlDType::F32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::quantized::GgmlDType;

    #[test]
    fn role_classification_keeps_norms_f32() {
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.input_layernorm.weight"), TensorRole::KeepF32);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.linear_attn.A_log"), TensorRole::KeepF32);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.linear_attn.dt_bias"), TensorRole::KeepF32);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.mlp.down_proj.weight"), TensorRole::DownProj);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.self_attn.v_proj.weight"), TensorRole::VProj);
        assert_eq!(TensorRole::from_key("lm.head.weight"), TensorRole::Output);
        assert_eq!(TensorRole::from_key("model.language_model.embed_tokens.weight"), TensorRole::Embedding);
        assert_eq!(TensorRole::from_key("model.language_model.layers.3.mlp.gate_proj.weight"), TensorRole::Matrix);
    }

    #[test]
    fn q4km_bumps_downproj_and_vproj_to_q6k() {
        let m = QuantMixture::Q4KM;
        assert_eq!(m.target_for(TensorRole::Matrix), Some(GgmlDType::Q4K));
        assert_eq!(m.target_for(TensorRole::DownProj), Some(GgmlDType::Q6K));
        assert_eq!(m.target_for(TensorRole::VProj), Some(GgmlDType::Q6K));
        assert_eq!(m.target_for(TensorRole::Embedding), Some(GgmlDType::Q6K));
        assert_eq!(m.target_for(TensorRole::KeepF32), None);
    }

    #[test]
    fn alignment_falls_back_below_256() {
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 512), GgmlDType::Q4K);
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 96), GgmlDType::Q8_0);
        assert_eq!(resolve_dtype(GgmlDType::Q4K, 100), GgmlDType::F32);
        assert_eq!(resolve_dtype(GgmlDType::Q8_0, 64), GgmlDType::Q8_0);
    }
}
