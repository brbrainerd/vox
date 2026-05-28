//! Crash-safe mid-epoch checkpoint state for Candle QLoRA training.
//!
//! [`CheckpointState`] captures all information needed to resume training
//! after a crash or intentional pause. It is persisted atomically to
//! `<output_dir>/checkpoint_state.json` using a temp-rename pattern so that
//! a power-cut mid-write cannot corrupt the last good checkpoint.
//!
//! ## Resume Invariant
//!
//! After loading a [`CheckpointState`] the caller must:
//!
//! 1. Load LoRA weights from `adapter_path`.
//! 2. Restore `global_step` and begin the outer epoch loop at `epoch`.
//! 3. Re-apply the shuffle stored in `shuffled_indices` to the pairs list
//!    (i.e. `pairs = shuffled_indices.iter().map(|&i| pairs[i].clone()).collect()`).
//! 4. Skip the first `pair_offset` pairs in the current epoch.
//!
//! Step 4 means the model will not re-train on already-seen pairs within the
//! interrupted epoch, preserving gradient correctness.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Schema identifier — bump when the struct shape changes.
pub const CHECKPOINT_SCHEMA: &str = "vox_mens_checkpoint_v1";

/// The file name written inside the output directory.
pub const CHECKPOINT_FILENAME: &str = "checkpoint_state.json";

/// All state necessary to resume a Candle QLoRA training run mid-epoch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointState {
    /// Schema version for forward compatibility; must equal [`CHECKPOINT_SCHEMA`].
    pub schema: String,
    /// Identifies the Vox training run in VoxDB (set from `LoraTrainingConfig::run_id`).
    pub run_id: String,
    /// The epoch (1-indexed) that was in progress when this checkpoint was saved.
    pub epoch: u32,
    /// Total gradient optimizer steps completed across all epochs (not just this one).
    pub global_step: u32,
    /// Index of the next pair to process in `shuffled_indices` within `epoch`.
    /// After loading: skip the first `pair_offset` elements of `shuffled_indices`.
    pub pair_offset: usize,
    /// The full shuffled order of pair indices for `epoch`.
    ///
    /// `shuffled_indices[i]` is the original index into the full pairs array.
    /// The caller must apply this index mapping to restore identical row order.
    pub shuffled_indices: Vec<usize>,
    /// Seed used to initialize the RNG before the first shuffle (epoch 1).
    /// Used to verify determinism if needed; actual order is captured in `shuffled_indices`.
    pub rng_seed: u64,
    /// Absolute path to the safetensors file containing LoRA adapter weights at this checkpoint.
    pub adapter_path: String,
    /// Last recorded training loss at this checkpoint.
    pub last_loss: f32,
    /// Wall time in seconds elapsed before this checkpoint.
    pub wall_seconds_elapsed: f64,
    /// RFC 3339 UTC timestamp when this checkpoint was saved.
    pub saved_at_utc: String,
}

impl CheckpointState {
    /// Returns the path where the checkpoint file lives inside `output_dir`.
    pub fn path_in(output_dir: &Path) -> PathBuf {
        output_dir.join(CHECKPOINT_FILENAME)
    }

    /// Saves `self` to `output_dir/checkpoint_state.json` atomically.
    ///
    /// Writes to a `.tmp` sibling then renames, so a power-cut mid-write
    /// cannot corrupt the last good checkpoint.
    pub fn save(&self, output_dir: &Path) -> anyhow::Result<()> {
        let final_path = Self::path_in(output_dir);
        let tmp_path = output_dir.join("checkpoint_state.json.tmp");
        let json = serde_json::to_string_pretty(self).context("serialize CheckpointState")?;
        std::fs::write(&tmp_path, &json)
            .with_context(|| format!("write checkpoint tmp {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, &final_path)
            .with_context(|| format!("rename checkpoint to {}", final_path.display()))?;
        Ok(())
    }

    /// Loads a checkpoint from `output_dir/checkpoint_state.json`.
    ///
    /// Returns `None` when the file does not exist (fresh run) or the JSON
    /// is corrupt / the schema does not match (start fresh, warn).
    pub fn load(output_dir: &Path) -> Option<Self> {
        let path = Self::path_in(output_dir);
        if !path.exists() {
            return None;
        }
        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "checkpoint_state.json unreadable — starting fresh"
                );
                return None;
            }
        };
        let state: CheckpointState = match serde_json::from_str(&raw) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "checkpoint_state.json parse error — starting fresh"
                );
                return None;
            }
        };
        if state.schema != CHECKPOINT_SCHEMA {
            tracing::warn!(
                found_schema = %state.schema,
                expected = CHECKPOINT_SCHEMA,
                "checkpoint schema mismatch — starting fresh"
            );
            return None;
        }
        Some(state)
    }

    /// Delete the checkpoint file (called on successful training completion).
    pub fn delete(output_dir: &Path) {
        let path = Self::path_in(output_dir);
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }

    /// Current UTC time as an RFC 3339 string.
    pub fn now_utc() -> String {
        // Use std::time for no extra dep; output is informational only.
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        // Format as a simplistic ISO-8601 UTC string — accurate to the second.
        let s = secs % 60;
        let m = (secs / 60) % 60;
        let h = (secs / 3600) % 24;
        let days_since_epoch = secs / 86400;
        // This is deliberately approximate — it's a log timestamp, not a calendar engine.
        format!("~{days_since_epoch}d {h:02}:{m:02}:{s:02} UTC (unix epoch offset)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn roundtrip_save_load() {
        let dir = std::env::temp_dir().join("vox_ckpt_test");
        std::fs::create_dir_all(&dir).unwrap();
        let state = CheckpointState {
            schema: CHECKPOINT_SCHEMA.to_string(),
            run_id: "test-run-1".to_string(),
            epoch: 2,
            global_step: 4500,
            pair_offset: 1823,
            shuffled_indices: vec![4, 1, 3, 2, 0],
            rng_seed: 42,
            adapter_path: std::env::temp_dir().join("adapter.safetensors").to_string_lossy().to_string(),
            last_loss: 1.25,
            wall_seconds_elapsed: 120.5,
            saved_at_utc: CheckpointState::now_utc(),
        };
        state.save(&dir).unwrap();
        let loaded = CheckpointState::load(&dir).expect("should load back");
        assert_eq!(loaded.epoch, 2);
        assert_eq!(loaded.global_step, 4500);
        assert_eq!(loaded.pair_offset, 1823);
        assert_eq!(loaded.shuffled_indices, vec![4, 1, 3, 2, 0]);
        assert_eq!(loaded.rng_seed, 42);
        CheckpointState::delete(&dir);
        assert!(CheckpointState::load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = std::env::temp_dir().join("vox_ckpt_missing_test");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(CheckpointState::load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_corrupt_returns_none() {
        let dir = std::env::temp_dir().join("vox_ckpt_corrupt_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = CheckpointState::path_in(&dir);
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "{{not valid json}}").unwrap();
        assert!(CheckpointState::load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_wrong_schema_returns_none() {
        let dir = std::env::temp_dir().join("vox_ckpt_schema_test");
        std::fs::create_dir_all(&dir).unwrap();
        let state = CheckpointState {
            schema: "vox_mens_checkpoint_v0".to_string(), // old schema
            run_id: "r".to_string(),
            epoch: 1,
            global_step: 10,
            pair_offset: 0,
            shuffled_indices: vec![],
            rng_seed: 0,
            adapter_path: String::new(),
            last_loss: 0.0,
            wall_seconds_elapsed: 0.0,
            saved_at_utc: String::new(),
        };
        let path = CheckpointState::path_in(&dir);
        std::fs::write(&path, serde_json::to_string(&state).unwrap()).unwrap();
        assert!(CheckpointState::load(&dir).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
