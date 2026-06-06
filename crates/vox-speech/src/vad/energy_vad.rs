//! Frame-energy VAD. Pure Rust, no ONNX. Splits PCM into 32 ms frames,
//! marks voiced frames above an RMS threshold, merges adjacent voiced regions.
//!
//! Env: VOX_ORATIO_VAD_ENERGY_THRESHOLD (f32, default 0.003)
//!      VOX_ORATIO_VAD_MERGE_GAP_MS     (u32 ms, default 300)
//!      VOX_ORATIO_VAD_ENABLED          (0/1, default 1)

use super::{VadBackend, VadSegment};

const DEFAULT_THRESHOLD: f32 = 0.003;
const DEFAULT_GAP_MS: u32 = 300;
const FRAME_MS: u32 = 32;

/// Energy-based Voice Activity Detection backend.
pub struct EnergyVad {
    pub(crate) threshold: f32,
    pub(crate) merge_gap_ms: u32,
}

impl Default for EnergyVad {
    fn default() -> Self {
        Self {
            threshold: std::env::var("VOX_ORATIO_VAD_ENERGY_THRESHOLD")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_THRESHOLD),
            merge_gap_ms: std::env::var("VOX_ORATIO_VAD_MERGE_GAP_MS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(DEFAULT_GAP_MS),
        }
    }
}

impl VadBackend for EnergyVad {
    fn detect_segments(&mut self, pcm: &[f32], sample_rate: u32) -> Vec<VadSegment> {
        if pcm.is_empty() {
            return vec![];
        }
        let frame_samples = ((FRAME_MS as usize) * sample_rate as usize) / 1000;
        let frame_samples = frame_samples.max(1);
        let gap_samples = ((self.merge_gap_ms as usize) * sample_rate as usize) / 1000;

        // Mark each frame as voiced/silent
        let voiced: Vec<bool> = pcm
            .chunks(frame_samples)
            .map(|frame| {
                let rms = (frame.iter().map(|x| x * x).sum::<f32>() / frame.len() as f32).sqrt();
                rms >= self.threshold
            })
            .collect();

        // Build raw segments from voiced frames
        let mut raw: Vec<VadSegment> = Vec::new();
        let mut i = 0;
        while i < voiced.len() {
            if voiced[i] {
                let start = i * frame_samples;
                let mut j = i;
                while j < voiced.len() && voiced[j] {
                    j += 1;
                }
                let end = (j * frame_samples).min(pcm.len());
                raw.push(VadSegment { start, end });
                i = j;
            } else {
                i += 1;
            }
        }

        // Merge segments separated by less than gap_samples
        let mut merged: Vec<VadSegment> = Vec::new();
        for seg in raw {
            if let Some(last) = merged.last_mut()
                && seg.start.saturating_sub(last.end) <= gap_samples
            {
                last.end = seg.end;
                continue;
            }
            merged.push(seg);
        }
        merged
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_gives_no_segments() {
        let pcm = vec![0.001f32; 16_000]; // 1 second of very quiet audio
        let mut vad = EnergyVad {
            threshold: 0.003,
            merge_gap_ms: 300,
        };
        let segs = vad.detect_segments(&pcm, 16_000);
        assert!(
            segs.is_empty(),
            "expected no voiced segments on near-silence"
        );
    }

    #[test]
    fn loud_audio_gives_one_segment() {
        let pcm = vec![0.5f32; 16_000];
        let mut vad = EnergyVad {
            threshold: 0.003,
            merge_gap_ms: 300,
        };
        let segs = vad.detect_segments(&pcm, 16_000);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].start, 0);
        assert_eq!(segs[0].end, 16_000);
    }

    #[test]
    fn short_silence_between_voiced_merges() {
        let sr = 16_000usize;
        let voiced_segment = vec![0.5f32; sr / 5]; // 0.2 s loud
        let silence = vec![0.001f32; sr / 10]; // 0.1 s quiet
        let mut pcm = Vec::new();
        pcm.extend_from_slice(&voiced_segment);
        pcm.extend_from_slice(&silence);
        pcm.extend_from_slice(&voiced_segment);
        let mut vad = EnergyVad {
            threshold: 0.003,
            merge_gap_ms: 300,
        };
        let segs = vad.detect_segments(&pcm, sr as u32);
        assert_eq!(segs.len(), 1, "expected merge: {:?}", segs);
    }
}
