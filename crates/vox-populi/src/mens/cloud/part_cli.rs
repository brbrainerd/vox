// ── CLI cloud target ──────────────────────────────────────────────────────────

/// `--cloud` CLI argument variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudTarget {
    /// Use local GPU — current default behavior, unchanged.
    Local,
    /// Query all configured providers and pick the cheapest.
    Auto,
    /// Force Vast.ai specifically.
    Vast,
    /// Force RunPod specifically.
    RunPod,
}

impl std::str::FromStr for CloudTarget {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "auto" => Ok(Self::Auto),
            "vast" => Ok(Self::Vast),
            "runpod" | "run-pod" | "run_pod" => Ok(Self::RunPod),
            other => {
                anyhow::bail!("Unknown cloud target '{other}'. Valid: local, auto, vast, runpod")
            }
        }
    }
}

// ── GPU name normalization ────────────────────────────────────────────────────

/// Normalize a raw GPU name from any provider into a comparable canonical form.
///
/// Used by both clients so [`estimator::TimeEstimator`] can look up TFLOPS
/// profiles consistently regardless of source.
///
/// # Examples
///
/// ```
/// # use vox_populi::mens::cloud::normalize_gpu_name;
/// assert_eq!(normalize_gpu_name("NVIDIA GeForce RTX 4090"), "rtx 4090");
/// assert_eq!(normalize_gpu_name("NVIDIA A100-SXM4-80GB"), "a100-sxm4-80gb");
/// assert_eq!(normalize_gpu_name("Tesla V100-SXM2-16GB"), "v100-sxm2-16gb");
/// ```
pub fn normalize_gpu_name(raw: &str) -> String {
    raw.to_lowercase()
        .replace("nvidia", "")
        .replace("geforce", "")
        .replace(" ada lovelace", "")
        .replace("tesla ", "")
        .replace("quadro ", "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}
