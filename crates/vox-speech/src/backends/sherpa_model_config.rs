//! Model path resolution for the Sherpa-ONNX backend.
//!
//! Priority: `VOX_ORATIO_SHERPA_MODEL_DIR` env (local dir) → HF Hub download.
//!
//! **Resolution never downloads.** `resolve_sherpa_model_paths` and
//! `resolve_sherpa_transducer_model_paths` are pure path computation: when
//! `VOX_ORATIO_SHERPA_MODEL_DIR` is unset they return an error naming what is
//! missing, its approximate size, and how to obtain it — they never touch the
//! network. The explicit `ensure_sherpa_model_paths` /
//! `ensure_sherpa_transducer_model_paths` functions perform the actual HF Hub
//! download (unless refused via `VOX_ORATIO_SHERPA_NO_DOWNLOAD`); they are the
//! only functions in this module allowed to call `hf_hub`.

#![cfg(feature = "stt-sherpa")]

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Resolved paths to Sherpa-ONNX model artifacts.
#[derive(Debug)]
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

/// Approximate download size for [`DEFAULT_SHERPA_HF_MODEL`] (and, when
/// overridden, whatever `VOX_ORATIO_SHERPA_HF_MODEL`/`VOX_ORATIO_SHERPA_MODEL`
/// points at instead).
///
/// Honesty note: the repo's actual file sizes are not recorded anywhere in
/// this codebase, and determining them requires a network call (HF Hub API or
/// a HEAD request), which acquisition-time and resolution-time code must not
/// make just to report a number. So this is deliberately "unknown" rather
/// than an invented figure — check the model card before downloading.
pub const SHERPA_WHISPER_SIZE_NOTE: &str = "unknown (not verifiable offline) — see https://huggingface.co/k2-fsa/sherpa-onnx-whisper-tiny.en \
     for the real repo file sizes before downloading";

/// Resolved paths to Sherpa-ONNX NeMo transducer model artifacts (e.g. Parakeet-TDT).
#[derive(Debug)]
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
/// `ensure_sherpa_transducer_model_paths` requests below.
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

/// Approximate download size for [`DEFAULT_SHERPA_TRANSDUCER_HF_MODEL`]. See
/// [`SHERPA_WHISPER_SIZE_NOTE`] for why this is "unknown" rather than a
/// number: real repo file sizes require a network call to determine, and
/// neither resolution nor acquisition should make one just to report a size.
pub const SHERPA_TRANSDUCER_SIZE_NOTE: &str = "unknown (not verifiable offline) — see \
     https://huggingface.co/csukuangfj/sherpa-onnx-nemo-parakeet-tdt-0.6b-v3-int8 \
     for the real repo file sizes before downloading";

/// Env var name: when set to a truthy value, [`ensure_sherpa_model_paths`] and
/// [`ensure_sherpa_transducer_model_paths`] refuse to download and return an
/// error instead, so a user or CI job can guarantee nothing large is fetched.
/// The error names `VOX_ORATIO_SHERPA_MODEL_DIR` as the offline alternative.
pub const NO_DOWNLOAD_ENV: &str = "VOX_ORATIO_SHERPA_NO_DOWNLOAD";

/// True iff `raw` (the raw `VOX_ORATIO_SHERPA_NO_DOWNLOAD` value, if any) asks
/// for downloads to be refused. Pure function of the env value so tests never
/// need to mutate real process env — see the module-level test-safety note.
/// Empty, `"0"`, and (case-insensitively) `"false"` are treated as "not set";
/// anything else truthy.
fn no_download_requested(raw: Option<&str>) -> bool {
    matches!(raw.map(str::trim), Some(v) if !v.is_empty() && v != "0" && !v.eq_ignore_ascii_case("false"))
}

/// Effective Whisper-shaped HF model id: `VOX_ORATIO_SHERPA_HF_MODEL`, else the
/// legacy `VOX_ORATIO_SHERPA_MODEL`, else [`DEFAULT_SHERPA_HF_MODEL`]. Reading
/// this env var is not itself a download — it only picks which repo a later
/// `ensure_*` call would fetch from.
fn whisper_model_id() -> String {
    std::env::var("VOX_ORATIO_SHERPA_HF_MODEL").unwrap_or_else(|_| {
        std::env::var("VOX_ORATIO_SHERPA_MODEL")
            .unwrap_or_else(|_| DEFAULT_SHERPA_HF_MODEL.to_string())
    })
}

/// Effective transducer HF model id: `VOX_ORATIO_SHERPA_HF_MODEL`, else
/// [`DEFAULT_SHERPA_TRANSDUCER_HF_MODEL`].
fn transducer_model_id() -> String {
    std::env::var("VOX_ORATIO_SHERPA_HF_MODEL")
        .unwrap_or_else(|_| DEFAULT_SHERPA_TRANSDUCER_HF_MODEL.to_string())
}

/// Build the "not available locally" error resolution returns instead of
/// downloading: names the model, its approximate size, and both ways to
/// obtain it.
fn model_not_local_error(model_id: &str, size_note: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "Sherpa-ONNX model `{model_id}` is not available locally. Resolution does \
         not download models. Approximate size: {size_note}. To obtain it: \
         (1) set VOX_ORATIO_SHERPA_MODEL_DIR to a local directory already containing \
         the model files, or (2) call the explicit `ensure_*` acquisition function \
         (e.g. `ensure_sherpa_model_paths` / `ensure_sherpa_transducer_model_paths`) \
         to fetch it from Hugging Face Hub."
    )
}

/// Refuse the download when `VOX_ORATIO_SHERPA_NO_DOWNLOAD` is set.
fn ensure_download_allowed(model_id: &str) -> Result<()> {
    if no_download_requested(std::env::var(NO_DOWNLOAD_ENV).ok().as_deref()) {
        anyhow::bail!(
            "{NO_DOWNLOAD_ENV} is set: refusing to download Sherpa-ONNX model `{model_id}`. \
             Point VOX_ORATIO_SHERPA_MODEL_DIR at a local copy instead."
        );
    }
    Ok(())
}

/// Print the model id, approximate size, and destination directory before any
/// network byte is fetched.
fn print_download_notice(model_id: &str, size_note: &str) {
    let dest = hf_hub::Cache::default().path().display().to_string();
    eprintln!(
        "vox-speech: downloading Sherpa-ONNX model `{model_id}` (approximate size: \
         {size_note}) to {dest}"
    );
}

/// Resolve transducer model paths from a local directory only. **Never
/// downloads.** Returns an error naming the configured model id, its
/// approximate size, and how to obtain it when
/// `VOX_ORATIO_SHERPA_MODEL_DIR` is unset. Call
/// [`ensure_sherpa_transducer_model_paths`] to download.
pub fn resolve_sherpa_transducer_model_paths() -> Result<SherpaTransducerModelPaths> {
    let dir = std::env::var("VOX_ORATIO_SHERPA_MODEL_DIR")
        .map_err(|_| model_not_local_error(&transducer_model_id(), SHERPA_TRANSDUCER_SIZE_NOTE))?;
    let dir = PathBuf::from(dir.trim());
    Ok(SherpaTransducerModelPaths {
        encoder: dir.join("encoder.onnx"),
        decoder: dir.join("decoder.onnx"),
        joiner: dir.join("joiner.onnx"),
        tokens: dir.join("tokens.txt"),
    })
}

/// Ensure transducer model paths are available, downloading from HF Hub when
/// they are not already local. The only function in this module allowed to
/// call `hf_hub` for the transducer model. Prints the model id, approximate
/// size, and destination directory before the first byte is fetched, and
/// refuses when `VOX_ORATIO_SHERPA_NO_DOWNLOAD` is set (see
/// [`ensure_download_allowed`]).
pub fn ensure_sherpa_transducer_model_paths() -> Result<SherpaTransducerModelPaths> {
    if let Ok(paths) = resolve_sherpa_transducer_model_paths() {
        return Ok(paths);
    }

    let model_id = transducer_model_id();
    ensure_download_allowed(&model_id)?;
    print_download_notice(&model_id, SHERPA_TRANSDUCER_SIZE_NOTE);

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

/// Resolve Whisper-shaped model paths from a local directory only. **Never
/// downloads.** Returns an error naming the configured model id, its
/// approximate size, and how to obtain it when
/// `VOX_ORATIO_SHERPA_MODEL_DIR` is unset. Call [`ensure_sherpa_model_paths`]
/// to download.
pub fn resolve_sherpa_model_paths() -> Result<SherpaModelPaths> {
    let dir = std::env::var("VOX_ORATIO_SHERPA_MODEL_DIR")
        .map_err(|_| model_not_local_error(&whisper_model_id(), SHERPA_WHISPER_SIZE_NOTE))?;
    let dir = PathBuf::from(dir.trim());
    Ok(SherpaModelPaths {
        encoder: dir.join("encoder.onnx"),
        decoder: dir.join("decoder.onnx"),
        tokens: dir.join("tokens.txt"),
    })
}

/// Ensure Whisper-shaped model paths are available, downloading from HF Hub
/// when they are not already local. The only function in this module allowed
/// to call `hf_hub` for the Whisper-shaped model. Prints the model id,
/// approximate size, and destination directory before the first byte is
/// fetched, and refuses when `VOX_ORATIO_SHERPA_NO_DOWNLOAD` is set (see
/// [`ensure_download_allowed`]).
pub fn ensure_sherpa_model_paths() -> Result<SherpaModelPaths> {
    if let Ok(paths) = resolve_sherpa_model_paths() {
        return Ok(paths);
    }

    let model_id = whisper_model_id();
    ensure_download_allowed(&model_id)?;
    print_download_notice(&model_id, SHERPA_WHISPER_SIZE_NOTE);

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

    #[test]
    fn resolve_whisper_paths_never_downloads_when_dir_unset() {
        // Held for the duration of the test: shared env var with the test
        // above and with `backend_dispatch`'s tests.
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        // SAFETY: test-only env mutation, serialized by the lock above.
        // Defensively clear in case a prior test in this binary leaked it.
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }

        let err = resolve_sherpa_model_paths()
            .expect_err("must error, not fall through to a network download");
        let msg = err.to_string();
        assert!(
            msg.contains(DEFAULT_SHERPA_HF_MODEL),
            "error must name the model id: {msg}"
        );
        assert!(
            msg.contains("VOX_ORATIO_SHERPA_MODEL_DIR"),
            "error must name the local-dir alternative: {msg}"
        );
        assert!(
            msg.to_lowercase().contains("size"),
            "error must mention size: {msg}"
        );
    }

    #[test]
    fn resolve_transducer_paths_never_downloads_when_dir_unset() {
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }

        let err = resolve_sherpa_transducer_model_paths()
            .expect_err("must error, not fall through to a network download");
        let msg = err.to_string();
        assert!(
            msg.contains(DEFAULT_SHERPA_TRANSDUCER_HF_MODEL),
            "error must name the model id: {msg}"
        );
        assert!(
            msg.contains("VOX_ORATIO_SHERPA_MODEL_DIR"),
            "error must name the local-dir alternative: {msg}"
        );
    }

    #[test]
    fn ensure_whisper_paths_resolves_locally_without_network_when_dir_set() {
        // A set VOX_ORATIO_SHERPA_MODEL_DIR must short-circuit `ensure_*`
        // through `resolve_*` — no HF Hub call, hence no network I/O — even
        // though the files under it are stubs.
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("encoder.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("decoder.onnx"), b"stub").unwrap();
        std::fs::write(dir.path().join("tokens.txt"), b"stub").unwrap();

        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("VOX_ORATIO_SHERPA_MODEL_DIR", dir.path());
        }
        let paths = ensure_sherpa_model_paths().expect("resolve via local dir, no network");
        assert_eq!(paths.encoder, dir.path().join("encoder.onnx"));
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
        }
    }

    #[test]
    fn ensure_whisper_paths_refuses_download_when_opted_out() {
        // No VOX_ORATIO_SHERPA_MODEL_DIR (so resolution fails) and
        // VOX_ORATIO_SHERPA_NO_DOWNLOAD set: must error before any network
        // call, never reach `hf_hub`.
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
            std::env::set_var(NO_DOWNLOAD_ENV, "1");
        }
        let err = ensure_sherpa_model_paths().expect_err("must refuse, not download");
        let msg = err.to_string();
        assert!(
            msg.contains(NO_DOWNLOAD_ENV),
            "error must name the opt-out: {msg}"
        );
        assert!(
            msg.contains("VOX_ORATIO_SHERPA_MODEL_DIR"),
            "error must name the local-dir alternative: {msg}"
        );
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var(NO_DOWNLOAD_ENV);
        }
    }

    #[test]
    fn ensure_transducer_paths_refuses_download_when_opted_out() {
        let _guard = crate::env_test_lock::SHERPA_MODEL_DIR_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());

        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var("VOX_ORATIO_SHERPA_MODEL_DIR");
            std::env::set_var(NO_DOWNLOAD_ENV, "true");
        }
        let err = ensure_sherpa_transducer_model_paths().expect_err("must refuse, not download");
        let msg = err.to_string();
        assert!(
            msg.contains(NO_DOWNLOAD_ENV),
            "error must name the opt-out: {msg}"
        );
        #[allow(unsafe_code)]
        unsafe {
            std::env::remove_var(NO_DOWNLOAD_ENV);
        }
    }

    #[test]
    fn no_download_requested_treats_blank_zero_and_false_as_unset() {
        assert!(!no_download_requested(None));
        assert!(!no_download_requested(Some("")));
        assert!(!no_download_requested(Some("   ")));
        assert!(!no_download_requested(Some("0")));
        assert!(!no_download_requested(Some("false")));
        assert!(!no_download_requested(Some("FALSE")));
        assert!(no_download_requested(Some("1")));
        assert!(no_download_requested(Some("true")));
        assert!(no_download_requested(Some("yes")));
    }
}
