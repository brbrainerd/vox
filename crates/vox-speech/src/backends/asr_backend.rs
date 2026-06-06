//! Common output type and trait for all Oratio STT backends.

use serde::{Deserialize, Serialize};

/// Output of one transcription call from any backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AsrOutput {
    /// Raw model text before any CURT post-processing.
    pub raw_text: String,
    /// Confidence in [0.0, 1.0]. Use `0.85` if backend does not expose log-prob.
    pub confidence: f32,
    /// Alternative hypotheses, best-first. May be empty.
    pub n_best: Vec<String>,
    /// Timed segments (from VAD or chunking)
    pub segments: Vec<TimedSegment>,
}

/// A segment of text with start and end timestamps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TimedSegment {
    /// Start time in milliseconds.
    pub start_ms: u64,
    /// End time in milliseconds.
    pub end_ms: u64,
    /// Raw text for this segment.
    pub text: String,
}

/// Synchronous STT backend. Designed to be called from `tokio::task::spawn_blocking`.
pub trait AsrBackend: Send + Sync {
    /// Short name for logs and status JSON (e.g. `"candle-whisper"`, `"sherpa-onnx"`).
    fn name(&self) -> &'static str;

    /// Transcribe mono PCM f32 samples at `sample_rate` Hz.
    ///
    /// `language` is an ISO 639-1 code (e.g. `"en"`) or `None` for auto-detect.
    fn transcribe_pcm(
        &self,
        pcm: &[f32],
        sample_rate: u32,
        language: Option<&str>,
    ) -> anyhow::Result<AsrOutput>;
}
