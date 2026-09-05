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
/// Verified via the HF Hub API (`/api/models/{repo}/tree/main`): this repo
/// contains exactly `encoder.int8.onnx`, `decoder.int8.onnx`,
/// `joiner.int8.onnx`, and `tokens.txt` — matching the filenames
/// `resolve_sherpa_transducer_model_paths` requests below.
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
    let client = hf_hub::HFClientSync::new().context("HF API init")?;
    let (owner, name) = hf_hub::split_id(&model_id);
    let repo = client.model(owner, name);
    let fetch = |filename: &str| {
        repo.download_file()
            .filename(filename)
            .revision(revision)
            .send()
    };

    let encoder = fetch("encoder.int8.onnx")
        .or_else(|_| fetch("encoder.onnx"))
        .with_context(|| format!("fetch encoder from {model_id}"))?;
    let decoder = fetch("decoder.int8.onnx")
        .or_else(|_| fetch("decoder.onnx"))
        .with_context(|| format!("fetch decoder from {model_id}"))?;
    let joiner = fetch("joiner.int8.onnx")
        .or_else(|_| fetch("joiner.onnx"))
        .with_context(|| format!("fetch joiner from {model_id}"))?;
    let tokens =
        fetch("tokens.txt").with_context(|| format!("fetch tokens.txt from {model_id}"))?;
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
    let client = hf_hub::HFClientSync::new().context("HF API init")?;
    let (owner, name) = hf_hub::split_id(&model_id);
    let repo = client.model(owner, name);
    let fetch = |filename: &str| {
        repo.download_file()
            .filename(filename)
            .revision(revision)
            .send()
    };

    let encoder = fetch("tiny.en-encoder.int8.onnx")
        .or_else(|_| fetch("encoder.onnx"))
        .or_else(|_| fetch("model.onnx"))
        .with_context(|| format!("fetch encoder from {model_id}"))?;
    let decoder = fetch("tiny.en-decoder.int8.onnx")
        .or_else(|_| fetch("decoder.onnx"))
        .unwrap_or_default(); // Might be optional for some models?
    let tokens = fetch("tiny.en-tokens.txt")
        .or_else(|_| fetch("tokens.txt"))
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
        // comment — a planned backend_dispatch test (Task 6) will also
        // mutate this env var and must take this same lock.
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
