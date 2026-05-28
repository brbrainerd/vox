//! Unified adapter artifact schema v3 (method + quant + base key map + layer order).
//!
//! Ported verbatim from `vox-populi/src/mens/tensor/adapter_schema_v3.rs` (SP3 sub-batch C).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::finetune_contract::{AdapterMethod, BaseQuantMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopuliAdapterManifestV3 {
    pub format: String,
    pub version: u32,
    #[serde(flatten)]
    pub method: AdapterMethodFields,
    pub quant: QuantFields,
    pub base_key_map: HashMap<String, String>,
    pub layer_order: Vec<String>,
    pub vocab: usize,
    pub d_model: usize,
    pub rank: usize,
    pub alpha: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<AdapterProvenanceFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterMethodFields {
    pub adapter_method: AdapterMethod,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantFields {
    pub base_quant: BaseQuantMode,
    #[serde(default = "default_true")]
    pub double_quant: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdapterProvenanceFields {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_family: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream_model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_class: Option<String>,
    #[serde(default)]
    pub attribution_required: bool,
}

fn default_true() -> bool {
    true
}

impl PopuliAdapterManifestV3 {
    pub const FORMAT: &'static str = "vox_mens_adapter";
    pub const VERSION: u32 = 3;

    #[allow(clippy::too_many_arguments)]
    pub fn new(
        method: AdapterMethod,
        base_quant: BaseQuantMode,
        double_quant: bool,
        base_key_map: HashMap<String, String>,
        layer_order: Vec<String>,
        vocab: usize,
        d_model: usize,
        rank: usize,
        alpha: usize,
        base_model: Option<String>,
        provenance: Option<AdapterProvenanceFields>,
    ) -> Self {
        Self {
            format: Self::FORMAT.to_string(),
            version: Self::VERSION,
            method: AdapterMethodFields {
                adapter_method: method,
            },
            quant: QuantFields {
                base_quant,
                double_quant,
            },
            base_key_map,
            layer_order,
            vocab,
            d_model,
            rank,
            alpha,
            base_model,
            provenance,
        }
    }
}
