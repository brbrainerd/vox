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

/// Resolved paths to Sherpa-ONNX NeMo transducer model artifacts (e.g. Parakeet-TDT).
pub struct SherpaTransducerModelPaths {
    /// Path to the ONNX encoder model.
    pub encoder: PathBuf,
    /// Path to the ONNX decoder model.
    pub decoder: PathBuf,
    /// Path to the ONNX joiner model (transducer-specific; Whisper has no joiner).
    pub joiner: PathBuf,
    /// Path to the BPE/token vocabulary file.
    pub tokens: PathBuf,
}

/// Default HF model ID for the Parakeet-TDT transducer download.
///
/// VERIFY BEFORE RELYING ON THIS IN PRODUCTION: this repo ID is the best candidate
/// identified during research (see
/// docs/src/architecture/vox-axis-stt-accuracy-design-2026-08-01.md,
/// "External research" section, and
/// https://k2-fsa.github.io/sherpa/onnx/pretrained_models/offline-transducer/nemo-transducer-models.html)
/// but was not confirmed to exist byte-for-byte at design time. Step 4's
/// manual download check is the actual verification gate.
///
/// KNOWN RISK (minor, disclosed not silently accepted): this mirrors the
/// pre-existing `resolve_sherpa_model_paths`' pattern of fetching from an
/// env-configurable HF repo id with no checksum/signature verification of
/// the downloaded files. That was an opt-in, never-shipped surface before
/// this plan; a later task (Task 6) makes it the GUI's default, tried-first
/// backend path, which raises the stakes of an unverified download without
/// adding any new integrity control. A real fix (pinned content hash +
/// verification before `OfflineRecognizer::create` loads the files) is out
/// of scope for this plan.
pub const DEFAULT_SHERPA_TRANSDUCER_HF_MODEL: &str =
    "csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8";

/// Resolve transducer model paths: env-set local dir OR HF Hub download.
/// Mirrors [`resolve_sherpa_model_paths`] but also resolves a `joiner` file.
pub fn resolve_sherpa_transducer_model_paths() -> Result<SherpaTransducerModelPaths> {
    if let Ok(dir) = std::env::var("VOX_ORATIO_SHERPA_MODEL_DIR") {
        let dir = PathBuf::from(dir.trim());
        return Ok(SherpaTransducerModelPaths {
            encoder: dir.join("encoder.onnx"),
            decoder: dir.join("decoder.onnx"),
            joiner: dir.join("joiner.onnx"),
            tokens: dir.join("tokens.txt"),
        });
    }

    let model_id = std::env::var("VOX_ORATIO_SHERPA_HF_MODEL")
        .unwrap_or_else(|_| DEFAULT_SHERPA_TRANSDUCER_HF_MODEL.to_string());
    let revision = "main";
    let api = hf_hub::api::sync::Api::new().context("HF API init")?;
    let repo = api.repo(hf_hub::Repo::with_revision(
        model_id.clone(),
        hf_hub::RepoType::Model,
        revision.to_string(),
    ));

    let encoder = repo
        .get("encoder.int8.onnx")
        .or_else(|_| repo.get("encoder.onnx"))
        .with_context(|| format!("fetch encoder from {model_id}"))?;
    let decoder = repo
        .get("decoder.int8.onnx")
        .or_else(|_| repo.get("decoder.onnx"))
        .with_context(|| format!("fetch decoder from {model_id}"))?;
    let joiner = repo
        .get("joiner.int8.onnx")
        .or_else(|_| repo.get("joiner.onnx"))
        .with_context(|| format!("fetch joiner from {model_id}"))?;
    let tokens = repo
        .get("tokens.txt")
        .with_context(|| format!("fetch tokens.txt from {model_id}"))?;
    Ok(SherpaTransducerModelPaths {
        encoder,
        decoder,
        joiner,
        tokens,
    })
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transducer_paths_resolve_from_local_dir() {
        // Held for the duration of the test: see `crate::env_test_lock` doc
        // comment — this env var is also mutated by backend_dispatch's test.
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("encoder.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("decoder.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("joiner.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("tokens.txt"), b"stub").unwrap();

        // SAFETY: test-only env mutation; serialized against other tests
        // touching the same var by the lock above.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("VOX_ORATIO_SHERPA_MODEL_DIR", dir.path());
        }
        let paths = resolve_sherpa_transducer_model_paths().expect("resolve");
        assert_eq!(paths.encoder, dir.path().join("encoder.onnx"));
        assert_eq!(paths.decoder, dir.path().join("decoder.onnx"));
        assert_eq!(paths.joiner, dir.path().join("joiner.onnx"));
        assert_eq!(paths.tokens, dir.path().join("tokens.txt"));
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }
    }
}
