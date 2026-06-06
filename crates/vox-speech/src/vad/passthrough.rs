//! Passthrough VAD — returns the whole buffer as one segment. Current default behavior.

use super::{VadBackend, VadSegment};

/// A VAD backend that performs no segmentation (returns the entire audio as one segment).
pub struct PassthroughVad;

impl VadBackend for PassthroughVad {
    fn detect_segments(&mut self, pcm: &[f32], _sample_rate: u32) -> Vec<VadSegment> {
        if pcm.is_empty() {
            vec![]
        } else {
            vec![VadSegment {
                start: 0,
                end: pcm.len(),
            }]
        }
    }
}
