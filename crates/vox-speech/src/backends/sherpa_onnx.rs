//! Sherpa-ONNX offline ASR backend — high-accuracy Whisper via C ONNX runtime.

#![cfg(feature = "stt-sherpa")]

use super::asr_backend::{AsrBackend, AsrOutput};
use super::sherpa_model_config::{ensure_sherpa_model_paths, ensure_sherpa_transducer_model_paths};
use anyhow::Result;
use sherpa_onnx::{
    OfflineRecognizer, OfflineRecognizerConfig, OfflineTransducerModelConfig,
    OfflineWhisperModelConfig,
};
use std::sync::Mutex;

const SHERPA_DEFAULT_THREADS: u32 = 4;
const ASR_CONFIDENCE_UNAVAILABLE: f32 = 0.85; // per AsrOutput docs

/// Oratio Sherpa-ONNX backend. Thread-safe via Mutex; one session per process.
pub struct SherpaOnnxBackend {
    inner: Mutex<OfflineRecognizer>,
}

impl SherpaOnnxBackend {
    /// Initialize the backend (downloads model if needed).
    ///
    /// Tries the NeMo transducer (Parakeet) path first — it is the default,
    /// faster, more accurate engine (see the STT accuracy design doc). Falls
    /// back to the Whisper-shaped config only when `VOX_ORATIO_SHERPA_KIND=whisper`
    /// is explicitly set, so existing local Whisper-model setups keep working.
    ///
    /// `VOX_ORATIO_SHERPA_KIND` is a distinct axis from `VOX_ORATIO_BACKEND`
    /// (which picks Sherpa vs. Candle Whisper entirely, at a higher layer,
    /// in `backend_dispatch.rs`): this one only selects the model *family*
    /// once Sherpa has already been chosen. Same literal value ("whisper"),
    /// different env var, different meaning — don't conflate the two.
    ///
    /// **Known gotcha**: `ensure_sherpa_model_paths` (Whisper) and
    /// `ensure_sherpa_transducer_model_paths` (this default path) both key
    /// off the same `VOX_ORATIO_SHERPA_MODEL_DIR` override, but expect
    /// different files in that directory (Whisper needs no `joiner.onnx`;
    /// transducer does). A local dir set up for one shape and read under the
    /// other fails at `OfflineRecognizer::create` with only a generic
    /// "check model paths" error — if you're pointing
    /// `VOX_ORATIO_SHERPA_MODEL_DIR` at a local model dir, also set
    /// `VOX_ORATIO_SHERPA_KIND` to match its shape.
    pub fn new() -> Result<Self> {
        let kind_env = std::env::var("VOX_ORATIO_SHERPA_KIND").unwrap_or_default();
        let is_whisper = kind_env.eq_ignore_ascii_case("whisper");
        // Canonical label for logs/errors — never the raw env value, so a
        // typo'd `VOX_ORATIO_SHERPA_KIND` (e.g. "whispr") can't misleadingly
        // echo back as if it were recognized; it silently falls through to
        // the transducer default and the log honestly says so.
        let kind_label = if is_whisper { "whisper" } else { "transducer" };
        let mut config = OfflineRecognizerConfig::default();

        if is_whisper {
            let paths = ensure_sherpa_model_paths()?;
            config.model_config.whisper = OfflineWhisperModelConfig {
                encoder: Some(paths.encoder.to_string_lossy().to_string()),
                decoder: Some(paths.decoder.to_string_lossy().to_string()),
                ..Default::default()
            };
            config.model_config.tokens = Some(paths.tokens.to_string_lossy().to_string());
        } else {
            let paths = ensure_sherpa_transducer_model_paths()?;
            config.model_config.transducer = OfflineTransducerModelConfig {
                encoder: Some(paths.encoder.to_string_lossy().to_string()),
                decoder: Some(paths.decoder.to_string_lossy().to_string()),
                joiner: Some(paths.joiner.to_string_lossy().to_string()),
            };
            config.model_config.tokens = Some(paths.tokens.to_string_lossy().to_string());
        }
        config.model_config.num_threads = SHERPA_DEFAULT_THREADS as i32;
        config.model_config.debug = false;

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            anyhow::anyhow!("Sherpa-ONNX init failed (kind={kind_label}, check model paths)")
        })?;

        tracing::info!(
            target: "vox_oratio_sherpa",
            event = "sherpa_backend_init",
            kind = kind_label,
            "Sherpa-ONNX backend initialized"
        );
        Ok(Self {
            inner: Mutex::new(recognizer),
        })
    }
}

impl AsrBackend for SherpaOnnxBackend {
    fn name(&self) -> &'static str {
        "sherpa-onnx"
    }

    fn transcribe_pcm(
        &self,
        pcm: &[f32],
        sample_rate: u32,
        _language: Option<&str>,
    ) -> Result<AsrOutput> {
        let rec = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("SherpaOnnxBackend mutex poisoned"))?;

        let stream = rec.create_stream();
        stream.accept_waveform(sample_rate as i32, pcm);
        rec.decode(&stream);
        let result = stream.get_result();

        Ok(AsrOutput {
            raw_text: result
                .map(|r| r.text.trim().to_string())
                .unwrap_or_default(),
            confidence: ASR_CONFIDENCE_UNAVAILABLE,
            n_best: Vec::new(),
            segments: Vec::new(), // TODO: map timestamps to segments
        })
    }
}

#[allow(dead_code)]
fn resample_to_16k(pcm: &[f32], from_hz: u32) -> Result<Vec<f32>> {
    use rubato::{FftFixedInOut, Resampler};
    let ratio = 16_000.0 / from_hz as f64;
    let chunk = 1024usize;
    let mut resampler = FftFixedInOut::<f32>::new(from_hz as usize, 16_000, chunk, 1)
        .map_err(|e| anyhow::anyhow!("rubato init: {e}"))?;
    let mut out = Vec::with_capacity((pcm.len() as f64 * ratio) as usize + chunk);
    let mut pos = 0usize;
    let in_chunk = resampler.input_frames_next();
    while pos + in_chunk <= pcm.len() {
        let frames = resampler
            .process(&[&pcm[pos..pos + in_chunk]], None)
            .map_err(|e| anyhow::anyhow!("rubato process: {e}"))?;
        out.extend_from_slice(&frames[0]);
        pos += in_chunk;
    }
    // Handle tail
    if pos < pcm.len() {
        let mut tail = pcm[pos..].to_vec();
        tail.resize(in_chunk, 0.0);
        let frames = resampler
            .process(&[&tail], None)
            .map_err(|e| anyhow::anyhow!("rubato tail: {e}"))?;
        let useful = ((pcm.len() - pos) as f64 * ratio) as usize;
        out.extend_from_slice(&frames[0][..useful.min(frames[0].len())]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transducer_config_variant_builds_recognizer_config() {
        // Construction-only test (no real ONNX files) — asserts the config
        // struct wiring is correct, not that a real model loads.
        let mut config = OfflineRecognizerConfig::default();
        config.model_config.transducer = sherpa_onnx::OfflineTransducerModelConfig {
            encoder: Some("encoder.onnx".to_string()),
            decoder: Some("decoder.onnx".to_string()),
            joiner: Some("joiner.onnx".to_string()),
        };
        config.model_config.tokens = Some("tokens.txt".to_string());
        assert_eq!(
            config.model_config.transducer.encoder.as_deref(),
            Some("encoder.onnx")
        );
        assert_eq!(
            config.model_config.transducer.joiner.as_deref(),
            Some("joiner.onnx")
        );
    }
}
