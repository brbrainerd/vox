//! Sherpa-ONNX offline ASR backend — high-accuracy Whisper via C ONNX runtime.

#![cfg(feature = "stt-sherpa")]

use super::asr_backend::{AsrBackend, AsrOutput};
use super::sherpa_model_config::resolve_sherpa_model_paths;
use anyhow::Result;
use sherpa_onnx::{OfflineRecognizer, OfflineRecognizerConfig, OfflineWhisperModelConfig};
use std::sync::Mutex;

const SHERPA_DEFAULT_THREADS: u32 = 4;
const ASR_CONFIDENCE_UNAVAILABLE: f32 = 0.85; // per AsrOutput docs

/// Oratio Sherpa-ONNX backend. Thread-safe via Mutex; one session per process.
pub struct SherpaOnnxBackend {
    inner: Mutex<OfflineRecognizer>,
}

impl SherpaOnnxBackend {
    /// Initialize the backend (downloads model if needed).
    pub fn new() -> Result<Self> {
        let paths = resolve_sherpa_model_paths()?;

        let mut config = OfflineRecognizerConfig::default();
        config.model_config.whisper = OfflineWhisperModelConfig {
            encoder: Some(paths.encoder.to_string_lossy().to_string()),
            decoder: Some(paths.decoder.to_string_lossy().to_string()),
            ..Default::default()
        };
        config.model_config.tokens = Some(paths.tokens.to_string_lossy().to_string());
        config.model_config.num_threads = SHERPA_DEFAULT_THREADS as i32;
        config.model_config.debug = false;

        let recognizer = OfflineRecognizer::create(&config).ok_or_else(|| {
            anyhow::anyhow!("Sherpa-ONNX Whisper init failed (check model paths)")
        })?;

        tracing::info!(
            target: "vox_oratio_sherpa",
            event = "sherpa_backend_init",
            "Sherpa-ONNX (Whisper) backend initialized"
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
