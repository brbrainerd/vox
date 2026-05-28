//! HF safetensors key discovery for Candle QLoRA projection stacking.
//!
//! Copied verbatim from `vox-populi/src/mens/tensor/candle_qlora_weights.rs` (SP3 sub-batch B).
//! Import paths updated: `super::hf_keymap::*` → `crate::hf_keymap::*`,
//! `super::hf_load::*` → `crate::hf_layout::*`.

pub use crate::hf_keymap::ordered_middle_projection_keys;

use crate::hf_layout::HfTransformerLayout;
use anyhow::Context;
use safetensors::SafeTensors;
use std::collections::HashSet;
use std::io::Read;

/// Counts for Candle QLoRA middle (`o_proj` / `c_proj`) projection keys vs safetensors union.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MiddleProjectionCoverage {
    pub matched: usize,
    pub expected: usize,
    /// qwen3_5: matched linear-attention middle projections.
    pub linear_matched: usize,
    /// qwen3_5: expected linear-attention middle projections.
    pub linear_expected: usize,
    /// qwen3_5: matched full-attention middle projections.
    pub full_matched: usize,
    /// qwen3_5: expected full-attention middle projections.
    pub full_expected: usize,
    /// `true` when `expected == 0` or every expected key is present in `present`.
    pub complete: bool,
}

/// Shared inventory for planner, `qlora_preflight`, and `candle_qlora_train` (avoid drift).
pub fn middle_projection_coverage(
    layout: &HfTransformerLayout,
    present: &HashSet<String>,
) -> MiddleProjectionCoverage {
    let expected_keys = ordered_middle_projection_keys(layout);
    let n_exp = expected_keys.len();
    if n_exp == 0 {
        return MiddleProjectionCoverage {
            matched: 0,
            expected: 0,
            linear_matched: 0,
            linear_expected: 0,
            full_matched: 0,
            full_expected: 0,
            complete: true,
        };
    }
    let mut matched = 0usize;
    let mut linear_expected = 0usize;
    let mut linear_matched = 0usize;
    let mut full_expected = 0usize;
    let mut full_matched = 0usize;
    for (idx, key) in expected_keys.iter().enumerate() {
        let is_linear = layout
            .layer_types
            .get(idx)
            .map(|t| t == "linear_attention")
            .unwrap_or(false);
        let has = present.contains(key.as_str());
        if has {
            matched += 1;
        }
        if is_linear {
            linear_expected += 1;
            if has {
                linear_matched += 1;
            }
        } else {
            full_expected += 1;
            if has {
                full_matched += 1;
            }
        }
    }
    MiddleProjectionCoverage {
        matched,
        expected: n_exp,
        linear_matched,
        linear_expected,
        full_matched,
        full_expected,
        complete: matched == n_exp,
    }
}

/// Read only the SafeTensors header JSON (without tensor data) and return a fake-padded
/// buffer that satisfies `SafeTensors::deserialize`'s offset-range validation.
fn read_safetensors_header(path: &std::path::Path) -> anyhow::Result<Vec<u8>> {
    use std::collections::HashMap;
    let mut file = std::fs::File::open(path)
        .with_context(|| format!("open output shard {}", path.display()))?;
    let mut len_buf = [0u8; 8];
    file.read_exact(&mut len_buf)
        .with_context(|| format!("read header length from {}", path.display()))?;
    let header_len = u64::from_le_bytes(len_buf) as usize;
    let mut json_buf = vec![0u8; header_len];
    file.read_exact(&mut json_buf)
        .with_context(|| format!("read header from {}", path.display()))?;

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

    let total = 8 + header_len + max_end;
    let mut buf = vec![0u8; total];
    buf[..8].copy_from_slice(&len_buf);
    buf[8..8 + header_len].copy_from_slice(&json_buf);
    Ok(buf)
}

/// Collect all tensor names across weight shards (union).
pub fn tensor_keys_union(weight_paths: &[std::path::PathBuf]) -> anyhow::Result<HashSet<String>> {
    let mut keys = HashSet::new();
    for wp in weight_paths {
        let header = read_safetensors_header(wp)?;
        let st = SafeTensors::deserialize(&header)
            .with_context(|| format!("parse handle {}", wp.display()))?;
        for (k, _) in st.tensors() {
            keys.insert(k.clone());
        }
    }
    Ok(keys)
}

#[cfg(test)]
mod coverage_tests {
    use super::*;
    use crate::hf_layout::HfTransformerLayout;
    use std::collections::HashSet;

    #[test]
    fn middle_projection_coverage_counts_keys() {
        let layout = HfTransformerLayout::from_config_json_str(
            "{\"model_type\":\"qwen2\",\"hidden_size\":8,\"num_attention_heads\":2,\"num_hidden_layers\":2,\"vocab_size\":10}",
        )
        .expect("layout");
        let mut present = HashSet::new();
        present.insert("model.layers.0.self_attn.o_proj.weight".to_string());
        let cov = middle_projection_coverage(&layout, &present);
        assert_eq!(cov.expected, 2);
        assert_eq!(cov.matched, 1);
        assert!(!cov.complete);
    }
}
