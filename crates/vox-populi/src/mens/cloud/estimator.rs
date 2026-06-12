//! GPU training time estimator.
//!
//! Three-tier estimation with explicit provenance tracking:
//!
//! 1. **Measured** — exact `ms_per_step` from a real run on this GPU in Arca
//! 2. **TFLOPS-scaled** — measured profile from another GPU × TFLOPS ratio from `gpu-specs.yaml`
//! 3. **Conservative** — [`CONSERVATIVE_MS_PER_STEP`] floor when no data exists at all
//!
//! `gpu-specs.yaml` is loaded at runtime (not compiled in) so new GPU models
//! can be added without recompiling the binary. The same YAML also contains a
//! `presets:` section used by both local and cloud training (SSOT for hardware configs).

use std::collections::HashMap;
use std::path::Path;

use super::{CONSERVATIVE_MS_PER_STEP, normalize_gpu_name};

// Re-exporting from preset_schema for back-compat/organization
use crate::mens::tensor::preset_schema::{GpuSpec, GpuSpecsFile, TrainingPreset};

// ── Estimate source (provenance) ──────────────────────────────────────────────

/// Documents which estimation tier produced a time estimate.
#[derive(Debug, Clone)]
pub enum EstimateSource {
    /// From a real measured training run on this exact GPU in Arca.
    Measured { gpu: String },
    /// Derived by scaling a measured profile from `from_gpu` by a TFLOPS ratio.
    TflopsScaled { from_gpu: String, ratio: f64 },
    /// Conservative fallback: [`CONSERVATIVE_MS_PER_STEP`] ms/step.
    Conservative,
}

impl std::fmt::Display for EstimateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Measured { gpu } => write!(f, "measured ({gpu})"),
            Self::TflopsScaled { from_gpu, ratio } => {
                write!(f, "TFLOPS×{ratio:.2} from {from_gpu}")
            }
            Self::Conservative => write!(f, "conservative ({CONSERVATIVE_MS_PER_STEP}ms/step)"),
        }
    }
}

// ── TimeEstimator ─────────────────────────────────────────────────────────────

/// Estimates training time + returns provenance.
pub struct TimeEstimator {
    /// Canonical GPU name → physical spec.
    specs: HashMap<String, GpuSpec>,
    /// (canonical_gpu_name, seq_len, batch_size) → ms_per_step from Arca DB.
    profiles: HashMap<(String, usize, usize), f64>,
    /// VRAM presets for auto-selection.
    pub presets: HashMap<String, TrainingPreset>,
}

impl TimeEstimator {
    /// Load specs from `gpu-specs.yaml` and measured profiles from Arca.
    pub fn new(
        gpu_specs_path: &Path,
        profiles: Vec<(String, usize, usize, f64)>,
    ) -> anyhow::Result<Self> {
        let yaml_str = std::fs::read_to_string(gpu_specs_path).map_err(|e| {
            anyhow::anyhow!(
                "Could not read {}: {e}\n\
                 Expected at mens/config/gpu-specs.yaml",
                gpu_specs_path.display()
            )
        })?;
        let parsed: GpuSpecsFile = serde_yaml::from_str(&yaml_str)
            .map_err(|e| anyhow::anyhow!("gpu-specs.yaml parse error: {e}"))?;

        // Normalize all YAML keys to canonical form
        let specs: HashMap<String, GpuSpec> = parsed
            .gpus
            .into_iter()
            .map(|(k, v): (String, GpuSpec)| (normalize_gpu_name(&k), v))
            .collect();

        let profile_map = profiles
            .into_iter()
            .map(|(gpu, seq_len, batch_size, ms)| {
                ((normalize_gpu_name(&gpu), seq_len, batch_size), ms)
            })
            .collect();

        Ok(Self {
            specs,
            profiles: profile_map,
            presets: parsed.presets,
        })
    }

    /// Estimate training time in seconds plus provenance.
    ///
    /// # Arguments
    /// - `gpu_name` — raw GPU name from provider API (normalized internally)
    /// - `seq_len` — training sequence length in tokens
    /// - `batch_size` — micro-batch size
    /// - `num_samples` — number of training pairs
    /// - `epochs` — training epochs
    pub fn estimate(
        &self,
        gpu_name: &str,
        seq_len: usize,
        batch_size: usize,
        num_samples: usize,
        epochs: usize,
    ) -> (f64, EstimateSource) {
        let norm = normalize_gpu_name(gpu_name);
        let total_steps = ((num_samples.max(1) + batch_size - 1) / batch_size) * epochs;

        // ── Tier 1: exact measured profile ───────────────────────────────────
        if let Some(&ms) = self.profiles.get(&(norm.clone(), seq_len, batch_size)) {
            return (
                total_steps as f64 * ms / 1000.0,
                EstimateSource::Measured { gpu: norm },
            );
        }

        // ── Tier 2: TFLOPS-scaled from any measured profile ───────────────────
        let target_tflops = self.specs.get(&norm).map(|s| s.fp16_tflops).unwrap_or(0.0);
        if target_tflops > 0.0 {
            // Prefer exact seq+batch match
            if let Some(((base_gpu, _, _), &base_ms)) = self
                .profiles
                .iter()
                .find(|((_, s, b), _)| *s == seq_len && *b == batch_size)
            {
                let base_tflops = self
                    .specs
                    .get(base_gpu)
                    .map(|s| s.fp16_tflops)
                    .unwrap_or(0.0);
                if base_tflops > 0.0 {
                    let ratio = base_tflops / target_tflops;
                    return (
                        total_steps as f64 * base_ms * ratio / 1000.0,
                        EstimateSource::TflopsScaled {
                            from_gpu: base_gpu.clone(),
                            ratio,
                        },
                    );
                }
            }

            // Fuzzy fallback: closest by edit distance + param distance
            if let Some(((base_gpu, _, _), &base_ms)) =
                self.profiles.iter().min_by_key(|((g, s, b), _)| {
                    edit_distance(g, &norm) * 1000 + s.abs_diff(seq_len) + b.abs_diff(batch_size)
                })
            {
                let base_tflops = self
                    .specs
                    .get(base_gpu)
                    .map(|s| s.fp16_tflops)
                    .unwrap_or(0.0);
                if base_tflops > 0.0 {
                    let ratio = base_tflops / target_tflops;
                    return (
                        total_steps as f64 * base_ms * ratio / 1000.0,
                        EstimateSource::TflopsScaled {
                            from_gpu: format!("{base_gpu} (approx)"),
                            ratio,
                        },
                    );
                }
            }
        }

        // ── Tier 3: conservative floor ────────────────────────────────────────
        (
            total_steps as f64 * CONSERVATIVE_MS_PER_STEP / 1000.0,
            EstimateSource::Conservative,
        )
    }

    /// Look up FP16 TFLOPS for a normalized GPU name.
    pub fn tflops_for(&self, norm_name: &str) -> Option<f64> {
        self.specs.get(norm_name).map(|s| s.fp16_tflops)
    }

    /// Look up VRAM in MB for a normalized GPU name.
    pub fn vram_mb_for(&self, norm_name: &str) -> Option<u64> {
        self.specs.get(norm_name).map(|s| s.vram_mb)
    }

    /// Select the best preset for the given VRAM — used by both local and cloud training.
    pub fn preset_for_vram(&self, vram_mb: u64) -> Option<(&str, &TrainingPreset)> {
        TrainingPreset::best_for_vram(&self.presets, vram_mb)
    }
}

// ── Inline Levenshtein ────────────────────────────────────────────────────────

/// O(n×m) Levenshtein edit distance. Used for fuzzy GPU name matching.
fn edit_distance(a: &str, b: &str) -> usize {
    strsim::levenshtein(a, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_same() {
        assert_eq!(edit_distance("abc", "abc"), 0);
    }

    #[test]
    fn edit_distance_one_insert() {
        assert_eq!(edit_distance("abc", "abcd"), 1);
    }

    #[test]
    fn estimator_conservative_with_no_data() {
        let est = TimeEstimator {
            specs: HashMap::new(),
            profiles: HashMap::new(),
            presets: HashMap::new(),
        };
        let (secs, source) = est.estimate("rtx 4080 super", 512, 1, 1000, 1);
        // 1000 steps × 200ms = 200s
        assert!((secs - 200.0).abs() < 1.0, "unexpected: {secs}");
        assert!(matches!(source, EstimateSource::Conservative));
    }

    #[test]
    fn estimator_exact_profile() {
        let mut profiles = HashMap::new();
        profiles.insert(("rtx 4080 super".to_string(), 512, 1), 50.0); // 50 ms/step
        let est = TimeEstimator {
            specs: HashMap::new(),
            profiles,
            presets: HashMap::new(),
        };
        let (secs, source) = est.estimate("NVIDIA GeForce RTX 4080 SUPER", 512, 1, 100, 1);
        // 100 steps × 50ms = 5s
        assert!((secs - 5.0).abs() < 0.1, "{secs}");
        assert!(matches!(source, EstimateSource::Measured { .. }));
    }

    #[test]
    fn conservative_source_displays_ms_value() {
        let src = EstimateSource::Conservative;
        let s = src.to_string();
        assert!(s.contains("200"), "{s}");
    }
}
