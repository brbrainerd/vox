//! Adapter method and quant mode enums for on-disk artifact descriptors.
//!
//! Ported from `vox-populi/src/mens/tensor/finetune_contract.rs` (SP3 sub-batch C).
//! Only the enums used by adapter_schema_v3 are needed here.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdapterMethod {
    Lora,
    Qlora,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaseQuantMode {
    None,
    Nf4,
}
