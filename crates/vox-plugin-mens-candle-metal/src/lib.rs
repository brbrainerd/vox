//! vox-plugin-mens-candle-metal — Candle + Metal ML backend plugin.
//!
//! Implements `MlBackend` from vox-plugin-api. The model handle stores
//! a `Box<CandleModel>`; methods recover it via the opaque pointer field.
//!
//! # SP3 sub-batch C — QLoRA training fully ported from vox-populi
//!
//! `candle_qlora_train/` contains the full training loop extracted from
//! `vox-populi/src/mens/tensor/candle_qlora_train/`.  `training.rs` now wires
//! through to `candle_qlora_train::run_candle_qlora_train` via a `TrainRequest`
//! JSON envelope.
//!
//! See: docs/src/architecture/plugin-system-redesign-2026.md

pub mod adapter_schema_v3;
mod backend;
pub mod candle_qlora_train;
mod checkpoint;
pub mod checkpoint_state;
pub mod config;
pub mod device;
pub mod external_serving_handoff;
pub mod finetune_contract;
pub mod hf_keymap;
pub mod hf_layout;
pub mod inference;
pub mod manifest;
pub mod merge;
mod model;
pub mod model_card;
pub mod operator_messages;
pub mod qlora_preflight;
pub(crate) mod qlora_weights;
pub mod telemetry;
pub mod telemetry_schema;
pub mod train_jsonl_preflight;
pub mod train_log;
mod training;
pub mod training_summary;
pub mod training_text;

use abi_stable::{
    erased_types::TD_Opaque, export_root_module, prefix_type::PrefixTypeTrait, sabi_extern_fn,
    std_types::*,
};
use vox_plugin_api::VOX_PLUGIN_ABI_VERSION;
use vox_plugin_api::abi::{VoxPlugin_TO, VoxPluginRef, VoxPluginRoot, VoxPluginRootRef};
use vox_plugin_api::host::VoxHost_TO;

#[export_root_module]
fn root_module() -> VoxPluginRootRef {
    VoxPluginRoot {
        abi_version: VOX_PLUGIN_ABI_VERSION,
        manifest_json,
        init,
    }
    .leak_into_prefix()
}

#[sabi_extern_fn]
fn manifest_json() -> RString {
    RString::from(r#"{"id":"mens-candle-metal","version":"0.1.0"}"#)
}

#[sabi_extern_fn]
fn init(_host: VoxHost_TO<'static, RBox<()>>) -> RResult<VoxPluginRef, RBoxError> {
    let plugin = backend::CandleMetalPlugin::new();
    let to = VoxPlugin_TO::from_value(plugin, TD_Opaque);
    RResult::ROk(to)
}
