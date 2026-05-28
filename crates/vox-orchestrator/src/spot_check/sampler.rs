/// Deterministic sampling decision for spot-checking task results.
///
/// The decision is derived from BLAKE3(task_id), so the same task_id always
/// produces the same outcome for a given probability — useful for reproducible
/// testing and for ensuring the submitter re-uses the same check decision
/// across retries.
pub struct SpotCheckSampler {
    /// Probability in [0.0, 1.0] that a given task should be spot-checked.
    pub prob: f32,
}

impl SpotCheckSampler {
    /// Create a sampler with the given probability.
    pub fn new(prob: f32) -> Self {
        Self { prob }
    }

    /// Returns `true` when `task_id` should be spot-checked.
    ///
    /// Decision is deterministic: BLAKE3-hash the task_id, interpret the first
    /// 4 bytes as a little-endian `u32`, and check whether it is below
    /// `prob * u32::MAX as f32`.
    pub fn should_check(&self, task_id: &str) -> bool {
        let hash = blake3::hash(task_id.as_bytes());
        let bytes = hash.as_bytes();
        let sample = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let threshold = (self.prob * u32::MAX as f32) as u32;
        sample < threshold
    }
}
