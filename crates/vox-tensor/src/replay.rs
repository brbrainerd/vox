//! Experience replay buffer for catastrophic forgetting mitigation.
//!
//! Research (Continual Learning §catastrophic-forgetting) proves that QLoRA
//! fine-tuning without replay causes up to 12% degradation on held-out benchmarks.
//! This module implements a `ReplayBuffer` that mixes a configurable percentage
//! of base pre-training data into each fine-tuning batch.
//!
//! ## Mix-CD strategy
//!
//! The buffer prioritises samples the model is *about to forget* using a simple
//! density estimator: samples whose loss increased most between consecutive
//! evaluation checkpoints are replayed more frequently.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::data::TrainingPair;

// ── Configuration ────────────────────────────────────────────────────────────

/// Replay buffer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayConfig {
    /// Fraction of each training batch that should be replay samples (default 0.10).
    pub replay_ratio: f64,
    /// Maximum number of samples to hold in the buffer (default 10_000).
    pub max_buffer_size: usize,
    /// When true, prioritise samples whose loss increased between checkpoints.
    pub mix_cd_enabled: bool,
    /// Loss increase threshold above which a sample is considered "at risk" (default 0.1).
    pub loss_increase_threshold: f64,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            replay_ratio: 0.10,
            max_buffer_size: 10_000,
            mix_cd_enabled: true,
            loss_increase_threshold: 0.1,
        }
    }
}

// ── Replay sample ────────────────────────────────────────────────────────────

/// A training pair with optional loss tracking for mix-CD prioritisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySample {
    pub pair: TrainingPair,
    /// Loss at the previous evaluation checkpoint.
    pub prev_loss: Option<f64>,
    /// Loss at the current evaluation checkpoint.
    pub curr_loss: Option<f64>,
    /// Number of times this sample has been replayed.
    pub replay_count: u32,
}

impl ReplaySample {
    /// Loss delta (positive means the model is forgetting this sample).
    pub fn loss_delta(&self) -> f64 {
        match (self.prev_loss, self.curr_loss) {
            (Some(prev), Some(curr)) => curr - prev,
            _ => 0.0,
        }
    }

    /// Whether this sample is "at risk" of being forgotten.
    pub fn is_at_risk(&self, threshold: f64) -> bool {
        self.loss_delta() > threshold
    }
}

// ── Replay buffer ────────────────────────────────────────────────────────────

/// Experience replay buffer that mixes base pre-training data into fine-tuning batches.
pub struct ReplayBuffer {
    pub config: ReplayConfig,
    /// Pool of replay candidates.
    samples: Vec<ReplaySample>,
    /// Sampling index (round-robin with priority boost for at-risk samples).
    next_idx: usize,
}

impl ReplayBuffer {
    pub fn new(config: ReplayConfig) -> Self {
        Self {
            config,
            samples: Vec::new(),
            next_idx: 0,
        }
    }

    /// Load base pre-training data from a JSONL file into the buffer.
    pub fn load_from_jsonl(&mut self, path: &Path) -> anyhow::Result<usize> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut count = 0;

        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if self.samples.len() >= self.config.max_buffer_size {
                break;
            }
            if let Ok(pair) = serde_json::from_str::<TrainingPair>(trimmed) {
                self.samples.push(ReplaySample {
                    pair,
                    prev_loss: None,
                    curr_loss: None,
                    replay_count: 0,
                });
                count += 1;
            }
        }

        Ok(count)
    }

    /// Add a single training pair to the buffer.
    pub fn add_sample(&mut self, pair: TrainingPair) {
        if self.samples.len() >= self.config.max_buffer_size {
            // Evict the least-at-risk sample (lowest loss delta).
            if let Some(min_idx) = self
                .samples
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    a.loss_delta()
                        .partial_cmp(&b.loss_delta())
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .map(|(i, _)| i)
            {
                self.samples.swap_remove(min_idx);
            }
        }
        self.samples.push(ReplaySample {
            pair,
            prev_loss: None,
            curr_loss: None,
            replay_count: 0,
        });
    }

    /// Number of samples in the buffer.
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    /// Whether the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Update loss observations for a sample (identified by prompt hash).
    pub fn update_loss(&mut self, prompt_hash: u64, new_loss: f64) {
        for sample in &mut self.samples {
            let hash = simple_hash(sample.pair.prompt.as_deref().unwrap_or(""));
            if hash == prompt_hash {
                sample.prev_loss = sample.curr_loss;
                sample.curr_loss = Some(new_loss);
                break;
            }
        }
    }

    /// Select `count` replay sample indices for mixing into a fine-tuning batch.
    ///
    /// When mix-CD is enabled, at-risk samples (loss increased) are
    /// preferentially selected. Otherwise, round-robin selection is used.
    ///
    /// Returns indices into the internal sample buffer. Use [`Self::get_pair`] to
    /// retrieve the actual `TrainingPair` for each index.
    pub fn select_replay_indices(&mut self, count: usize) -> Vec<usize> {
        if self.samples.is_empty() || count == 0 {
            return vec![];
        }

        let mut selected = Vec::with_capacity(count);

        if self.config.mix_cd_enabled {
            // Prioritise at-risk samples first.
            let mut at_risk_indices: Vec<usize> = self
                .samples
                .iter()
                .enumerate()
                .filter(|(_, s)| s.is_at_risk(self.config.loss_increase_threshold))
                .map(|(i, _)| i)
                .collect();
            // Sort by loss delta descending (most at-risk first).
            at_risk_indices.sort_by(|&a, &b| {
                self.samples[b]
                    .loss_delta()
                    .partial_cmp(&self.samples[a].loss_delta())
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            for &idx in at_risk_indices.iter().take(count) {
                self.samples[idx].replay_count += 1;
                selected.push(idx);
            }
        }

        // Fill remaining slots with round-robin.
        let remaining = count.saturating_sub(selected.len());
        for _ in 0..remaining {
            if self.next_idx >= self.samples.len() {
                self.next_idx = 0;
            }
            self.samples[self.next_idx].replay_count += 1;
            selected.push(self.next_idx);
            self.next_idx += 1;
        }

        selected
    }

    /// Get the training pair at a given buffer index.
    pub fn get_pair(&self, idx: usize) -> Option<&TrainingPair> {
        self.samples.get(idx).map(|s| &s.pair)
    }

    /// Convenience: select and collect training pairs in one call.
    pub fn select_replay_batch(&mut self, count: usize) -> Vec<TrainingPair> {
        let indices = self.select_replay_indices(count);
        indices
            .into_iter()
            .filter_map(|i| self.samples.get(i).map(|s| s.pair.clone()))
            .collect()
    }

    /// Compute how many replay samples to mix into a batch of `batch_size`.
    pub fn replay_count_for_batch(&self, batch_size: usize) -> usize {
        (batch_size as f64 * self.config.replay_ratio).ceil() as usize
    }

    /// Summary statistics for telemetry.
    pub fn stats(&self) -> ReplayBufferStats {
        let at_risk = self
            .samples
            .iter()
            .filter(|s| s.is_at_risk(self.config.loss_increase_threshold))
            .count();
        let total_replays: u64 = self.samples.iter().map(|s| s.replay_count as u64).sum();
        ReplayBufferStats {
            buffer_size: self.samples.len(),
            at_risk_count: at_risk,
            total_replays,
        }
    }
}

/// Telemetry summary for the replay buffer.
#[derive(Debug, Clone, Serialize)]
pub struct ReplayBufferStats {
    pub buffer_size: usize,
    pub at_risk_count: usize,
    pub total_replays: u64,
}

/// Simple non-cryptographic hash for prompt lookup.
fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 5381;
    for b in s.bytes() {
        h = h.wrapping_mul(33).wrapping_add(b as u64);
    }
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_pair(prompt: &str) -> TrainingPair {
        TrainingPair {
            prompt: Some(prompt.to_string()),
            response: Some("fn hello() {}".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn buffer_add_and_select() {
        let mut buf = ReplayBuffer::new(ReplayConfig::default());
        for i in 0..5 {
            buf.add_sample(sample_pair(&format!("prompt {i}")));
        }
        assert_eq!(buf.len(), 5);
        let batch = buf.select_replay_batch(3);
        assert_eq!(batch.len(), 3);
    }

    #[test]
    fn replay_count_for_batch_10pct() {
        let buf = ReplayBuffer::new(ReplayConfig::default());
        assert_eq!(buf.replay_count_for_batch(100), 10);
        assert_eq!(buf.replay_count_for_batch(8), 1);
    }

    #[test]
    fn mix_cd_prioritises_at_risk_samples() {
        let mut buf = ReplayBuffer::new(ReplayConfig {
            mix_cd_enabled: true,
            loss_increase_threshold: 0.05,
            ..ReplayConfig::default()
        });
        for i in 0..10 {
            buf.add_sample(sample_pair(&format!("stable {i}")));
        }
        // Mark one sample as at-risk by updating its loss.
        buf.samples[3].prev_loss = Some(0.5);
        buf.samples[3].curr_loss = Some(0.8); // delta = 0.3 > threshold 0.05

        let indices = buf.select_replay_indices(1);
        assert_eq!(indices.len(), 1);
        // The at-risk sample should be selected first.
        let pair = buf.get_pair(indices[0]).expect("valid index");
        assert_eq!(
            pair.prompt.as_deref(),
            Some("stable 3"),
            "at-risk sample should be prioritised"
        );
    }

    #[test]
    fn eviction_removes_least_at_risk() {
        let mut buf = ReplayBuffer::new(ReplayConfig {
            max_buffer_size: 3,
            ..ReplayConfig::default()
        });
        buf.add_sample(sample_pair("a"));
        buf.add_sample(sample_pair("b"));
        buf.add_sample(sample_pair("c"));
        // Mark 'a' as at-risk.
        buf.samples[0].prev_loss = Some(0.5);
        buf.samples[0].curr_loss = Some(1.0);

        // Adding a 4th should evict the least at-risk (not 'a').
        buf.add_sample(sample_pair("d"));
        assert_eq!(buf.len(), 3);
        let prompts: Vec<_> = buf
            .samples
            .iter()
            .map(|s| s.pair.prompt.as_deref().unwrap_or(""))
            .collect();
        assert!(
            prompts.contains(&"a"),
            "at-risk sample 'a' should be preserved"
        );
        assert!(prompts.contains(&"d"), "new sample 'd' should be added");
    }

    #[test]
    fn stats_reports_at_risk_count() {
        let mut buf = ReplayBuffer::new(ReplayConfig::default());
        for i in 0..5 {
            buf.add_sample(sample_pair(&format!("p{i}")));
        }
        buf.samples[1].prev_loss = Some(0.3);
        buf.samples[1].curr_loss = Some(0.5);
        buf.samples[4].prev_loss = Some(0.2);
        buf.samples[4].curr_loss = Some(0.4);
        let s = buf.stats();
        assert_eq!(s.at_risk_count, 2);
    }

    #[test]
    fn config_defaults_sane() {
        let cfg = ReplayConfig::default();
        assert!((cfg.replay_ratio - 0.10).abs() < f64::EPSILON);
        assert_eq!(cfg.max_buffer_size, 10_000);
        assert!(cfg.mix_cd_enabled);
    }
}
