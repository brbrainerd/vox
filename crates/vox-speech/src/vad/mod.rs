//! Voice Activity Detection for pre-segmenting audio before STT.

pub mod energy_vad;
pub mod passthrough;

/// One contiguous voiced segment (sample indices, inclusive start / exclusive end).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VadSegment {
    /// Start index in the audio samples array.
    pub start: usize,
    /// End index in the audio samples array.
    pub end: usize,
}

impl VadSegment {
    /// Length in samples.
    #[must_use]
    pub fn len(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
    /// Whether the segment is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Synchronous VAD backend trait.
pub trait VadBackend: Send {
    /// Detect speech segments in the provided PCM audio buffer.
    fn detect_segments(&mut self, pcm: &[f32], sample_rate: u32) -> Vec<VadSegment>;
}

/// Create the VAD backend selected by env `VOX_ORATIO_VAD`.
/// Values: `"none"` or `""` → passthrough, `"energy"` (default) → energy VAD.
pub fn create_vad() -> Box<dyn VadBackend> {
    match std::env::var("VOX_ORATIO_VAD")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "none" | "passthrough" => Box::new(passthrough::PassthroughVad),
        _ => Box::new(energy_vad::EnergyVad::default()),
    }
}
