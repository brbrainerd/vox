//! Shared device-agnostic load helper for Candle-based inference backends.
//!
//! All three Candle backends (CPU / CUDA / Metal) load SP-1 quantized artifacts the
//! same way; the only difference is the [`Device`] they use. This module centralises
//! that logic so each backend just constructs the right device and delegates here.

use crate::backend::{BackendId, InferenceError, LoadedModel};
use crate::qwen_forward::QwenForward;
use crate::qwen_weights::QwenWeights;
use candle_core::Device;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use vox_hf_layout::HfTransformerLayout;

/// Default Qwen end-of-sequence token id (`<|endoftext|>`).
const QWEN_DEFAULT_EOS: u32 = 151_643;

/// Internal per-model state owned by all Candle backends. The trait's [`LoadedModel`]
/// is opaque (carries only a label), so the real model lives here keyed by that label.
pub(crate) struct LoadedState {
    pub(crate) forward: Mutex<QwenForward>,
    pub(crate) tokenizer: tokenizers::Tokenizer,
    pub(crate) eos: Option<u32>,
}

/// Load a local SP-1 quantized artifact directory into an in-memory model on `dev`,
/// register it in `loaded` under `label`, and return the opaque [`LoadedModel`].
///
/// If the requested `dev` fails to initialise (feature off or no device present) the
/// caller should fall back to [`Device::Cpu`] before calling this function.
pub(crate) fn load_from_dir_on_device(
    dir: &Path,
    dev: Device,
    backend_id: BackendId,
    label: String,
    loaded: &Mutex<HashMap<String, Arc<LoadedState>>>,
) -> Result<LoadedModel, InferenceError> {
    let layout = HfTransformerLayout::from_config_path(&dir.join("config.json"))
        .map_err(|e| InferenceError::Internal(format!("parse config.json: {e}")))?;
    let weights = QwenWeights::load(dir, &dev)
        .map_err(|e| InferenceError::Internal(format!("load weights: {e}")))?;
    let forward = QwenForward::new(&layout, weights, &dev)
        .map_err(|e| InferenceError::Internal(format!("build forward: {e}")))?;

    let tokenizer = tokenizers::Tokenizer::from_file(dir.join("tokenizer.json"))
        .map_err(|e| InferenceError::Internal(format!("load tokenizer.json: {e}")))?;
    let eos = tokenizer
        .token_to_id("<|endoftext|>")
        .or(Some(QWEN_DEFAULT_EOS));

    let state = Arc::new(LoadedState {
        forward: Mutex::new(forward),
        tokenizer,
        eos,
    });
    loaded
        .lock()
        .expect("loaded map poisoned")
        .insert(label.clone(), state);

    Ok(LoadedModel {
        backend: backend_id,
        label,
    })
}
