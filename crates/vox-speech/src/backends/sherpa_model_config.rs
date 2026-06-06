//! Model path resolution for the Sherpa-ONNX backend.
//!
//! Priority: `VOX_ORATIO_SHERPA_MODEL_DIR` env (local dir) → HF Hub download.

#![cfg(feature = "stt-sherpa")]

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Resolved paths to Sherpa-ONNX model artifacts.
pub struct SherpaModelPaths {
    /// Path to the ONNX encoder model.
    pub encoder: PathBuf,
    /// Path to the ONNX decoder model (can be empty if not required).
    pub decoder: PathBuf,
    /// Path to the BPE tokens file.
    pub tokens: PathBuf,
}

/// Default HF model ID for Sherpa download.
pub const DEFAULT_SHERPA_HF_MODEL: &str = "k2-fsa/sherpa-onnx-whisper-tiny.en";

/// Resolve model paths: env-set local dir OR HF Hub download.
pub fn resolve_sherpa_model_paths() -> Result<SherpaModelPaths> {
    if let Ok(dir) = std::env::var("VOX_ORATIO_SHERPA_MODEL_DIR") {
        let dir = PathBuf::from(dir.trim());
        let encoder = dir.join("encoder.onnx");
        let decoder = dir.join("decoder.onnx");
        let tokens = dir.join("tokens.txt");
        return Ok(SherpaModelPaths {
            encoder,
            decoder,
            tokens,
        });
    }

    // HF Hub download
    let model_id = std::env::var("VOX_ORATIO_SHERPA_HF_MODEL").unwrap_or_else(|_| {
        std::env::var("VOX_ORATIO_SHERPA_MODEL")
            .unwrap_or_else(|_| DEFAULT_SHERPA_HF_MODEL.to_string())
    });
    let revision = "main";
    let api = hf_hub::api::sync::Api::new().context("HF API init")?;
    let repo = api.repo(hf_hub::Repo::with_revision(
        model_id.clone(),
        hf_hub::RepoType::Model,
        revision.to_string(),
    ));

    let encoder = repo
        .get("tiny.en-encoder.int8.onnx")
        .or_else(|_| repo.get("encoder.onnx"))
        .or_else(|_| repo.get("model.onnx"))
        .with_context(|| format!("fetch encoder from {model_id}"))?;
    let decoder = repo
        .get("tiny.en-decoder.int8.onnx")
        .or_else(|_| repo.get("decoder.onnx"))
        .unwrap_or_default(); // Might be optional for some models?
    let tokens = repo
        .get("tiny.en-tokens.txt")
        .or_else(|_| repo.get("tokens.txt"))
        .with_context(|| format!("fetch tokens.txt from {model_id}"))?;
    Ok(SherpaModelPaths {
        encoder,
        decoder,
        tokens,
    })
}
