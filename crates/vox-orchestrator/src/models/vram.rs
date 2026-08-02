//! Advisory VRAM-fit estimation for local model scoring (Task 2.6).
//!
//! No surveyed coding tool does hardware/VRAM capability gating before
//! recommending a local model. Vox has a real NVML probe
//! (`vox-plugin-nvml-probe`) that was previously wired to nothing. This
//! module turns it into a **soft ranking signal only**:
//!
//! - A model estimated not to fit is deprioritized in scoring, never removed
//!   from candidates or hard-blocked. The estimate can be wrong — unusual
//!   setups, other GPU-using processes, quantizations Vox doesn't model.
//! - Non-NVIDIA hardware (Apple Silicon, AMD, integrated/no GPU) and any NVML
//!   failure (`ProbeError::LibraryUnavailable`, no device, etc.) must degrade
//!   to [`VramFit::Unknown`], which contributes **zero** score effect — never
//!   treated as an error, never defaulted to "fits" or "doesn't fit".
//!
//! Known limitation (by design, not a gap): NVIDIA/NVML only. AMD and Apple
//! Silicon VRAM probing are out of scope for this task — on that hardware the
//! signal is simply absent, which is the correct graceful-degradation
//! behavior, not a bug.

use std::sync::RwLock;

use super::ModelSpec;

/// Advisory VRAM-fit classification for a candidate model against the
/// current free-VRAM figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VramFit {
    /// Estimated footprint comfortably fits with headroom to spare.
    Comfortable,
    /// Estimated footprint fits, but with little margin.
    Tight,
    /// Estimated footprint exceeds the observed free VRAM.
    Exceeds,
    /// No signal available (no NVML/GPU, or no parameter-count data for this
    /// model). Must be treated as a true no-op by every caller.
    Unknown,
}

/// Assumed bytes-per-parameter when the model's quantization is unknown.
/// `0.5` approximates a common Q4-class local-model quantization (4 bits/param
/// plus rounding/overhead). This is a coarse default, not a measurement — Vox
/// does not currently track per-model quantization, so real footprints for
/// Q8/F16 models will be undercounted and heavily-quantized (Q2/Q3) models
/// will be overcounted. Advisory only; see module docs.
const BYTES_PER_PARAM_DEFAULT_Q4: f64 = 0.5;

/// Crude KV-cache overhead, expressed as a fraction of estimated weight size,
/// applied on top of the weights estimate. Real KV-cache size depends on
/// layer count, head dimensions, and how full the context window actually is
/// at request time — none of which Vox tracks per-model — so this is a flat,
/// deliberately conservative fudge factor rather than a derived quantity.
const KV_CACHE_OVERHEAD_FRACTION: f64 = 0.15;

/// Headroom multiplier above the estimated requirement to call a fit
/// "Comfortable" rather than merely "Tight".
const COMFORTABLE_HEADROOM: f64 = 1.25;

const MIB: f64 = 1024.0 * 1024.0;

/// Estimate whether `m` fits in `free_vram_mb`.
///
/// Formula: `required_mb = params_b * 1e9 * BYTES_PER_PARAM_DEFAULT_Q4 / MiB * (1 + KV_CACHE_OVERHEAD_FRACTION)`.
/// If `free_vram_mb >= required_mb * COMFORTABLE_HEADROOM`, the fit is `Comfortable`;
/// else if `free_vram_mb >= required_mb`, the fit is `Tight`; otherwise `Exceeds`.
///
/// Returns [`VramFit::Unknown`] — never a guess — when either input is
/// missing: no parameter-count data for `m` (`capabilities.param_count_b`),
/// or no free-VRAM reading (`free_vram_mb` is `None`, e.g. no NVIDIA GPU).
#[must_use]
pub fn estimate_vram_fit(m: &ModelSpec, free_vram_mb: Option<u64>) -> VramFit {
    let (Some(params_b), Some(free_mb)) = (m.capabilities.param_count_b, free_vram_mb) else {
        return VramFit::Unknown;
    };
    if params_b <= 0.0 {
        return VramFit::Unknown;
    }

    let params = f64::from(params_b) * 1e9;
    let weights_mb = (params * BYTES_PER_PARAM_DEFAULT_Q4) / MIB;
    let required_mb = weights_mb * (1.0 + KV_CACHE_OVERHEAD_FRACTION);
    let free_mb = free_mb as f64;

    if free_mb >= required_mb * COMFORTABLE_HEADROOM {
        VramFit::Comfortable
    } else if free_mb >= required_mb {
        VramFit::Tight
    } else {
        VramFit::Exceeds
    }
}

/// Process-global cache of the most recently observed free-VRAM figure
/// (megabytes), refreshed once per catalog-refresh cycle rather than probed
/// per scoring call. `None` means "no signal" (no NVML/GPU, or probe hasn't
/// run yet) — scoring must treat that identically to a probe failure.
static FREE_VRAM_MB_HINT: RwLock<Option<u64>> = RwLock::new(None);

/// Install (or clear, with `None`) the cached free-VRAM hint. Called once per
/// discovery/refresh pass after probing NVML — never per scoring call.
pub fn set_free_vram_mb_hint(v: Option<u64>) {
    if let Ok(mut g) = FREE_VRAM_MB_HINT.write() {
        *g = v;
    }
}

/// The cached free-VRAM hint (megabytes), or `None` if unavailable.
#[must_use]
pub fn free_vram_mb_hint() -> Option<u64> {
    FREE_VRAM_MB_HINT.read().ok().and_then(|g| *g)
}

/// Probes NVML once (blocking FFI call — run via `spawn_blocking` from async
/// contexts) and caches the minimum per-device free VRAM across all detected
/// GPUs via [`set_free_vram_mb_hint`]. The minimum (not sum/max) is used so
/// the advisory signal is conservative on multi-GPU boxes where a model must
/// fit on a single device.
///
/// `Err(ProbeError::LibraryUnavailable(_))` (no NVML library — Apple Silicon,
/// AMD, no discrete GPU, or the library simply isn't installed) and any other
/// probe error are both treated identically: clear the hint to `None`. This
/// is never logged as an error — it's the expected path on non-NVIDIA
/// hardware.
pub fn refresh_free_vram_hint_from_nvml() {
    match vox_plugin_nvml_probe::probe::device_metrics() {
        Ok(json) => {
            let free_mb = parse_min_free_mb(&json);
            set_free_vram_mb_hint(free_mb);
        }
        Err(e) => {
            tracing::debug!(
                target: "vox.orchestrator.vram",
                error = %e,
                "NVML unavailable; VRAM-fit advisory signal disabled this cycle"
            );
            set_free_vram_mb_hint(None);
        }
    }
}

/// Parses `device_metrics()`'s `{"metrics":[{"memory_free_mb": ..., ...}]}`
/// JSON and returns the minimum `memory_free_mb` across devices, or `None` if
/// the report is empty/unparseable.
fn parse_min_free_mb(json: &str) -> Option<u64> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let metrics = v.get("metrics")?.as_array()?;
    metrics
        .iter()
        .filter_map(|m| m.get("memory_free_mb")?.as_u64())
        .min()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{ModelCapabilities, ProviderType};

    fn spec_with_params(param_count_b: Option<f32>) -> ModelSpec {
        ModelSpec {
            id: "test/local".into(),
            canonical_slug: "ollama/test-local".into(),
            provider: "ollama".into(),
            provider_type: ProviderType::Ollama,
            max_tokens: 8192,
            cost_per_1k: 0.0,
            cost_per_1k_input: 0.0,
            cost_per_1k_output: 0.0,
            is_free: true,
            observed_cost_per_1k: None,
            strengths: vec![],
            capabilities: ModelCapabilities {
                param_count_b,
                ..Default::default()
            },
            cache_creation_cost_per_1k: 0.0,
            cache_read_cost_per_1k: 0.0,
            supports_prompt_caching: false,
            pricing_source: crate::models::spec::PricingSource::Bootstrap,
            supported_parameters: vec![],
        }
    }

    #[test]
    fn small_model_plenty_of_vram_is_comfortable() {
        // ~1B params @ Q4 ~= 465 MiB weights * 1.15 KV overhead ~= 535 MiB required.
        let m = spec_with_params(Some(1.0));
        assert_eq!(estimate_vram_fit(&m, Some(20_000)), VramFit::Comfortable);
    }

    #[test]
    fn large_model_little_vram_exceeds() {
        // ~70B params @ Q4 ~= huge weight footprint, far beyond 2 GiB free.
        let m = spec_with_params(Some(70.0));
        assert_eq!(estimate_vram_fit(&m, Some(2_000)), VramFit::Exceeds);
    }

    #[test]
    fn missing_param_data_is_unknown() {
        let m = spec_with_params(None);
        assert_eq!(estimate_vram_fit(&m, Some(20_000)), VramFit::Unknown);
    }

    #[test]
    fn missing_free_vram_is_unknown_even_with_param_data() {
        let m = spec_with_params(Some(7.0));
        assert_eq!(estimate_vram_fit(&m, None), VramFit::Unknown);
    }

    #[test]
    fn borderline_case_is_tight_not_exceeds_or_comfortable() {
        // ~7B params @ Q4: weights ~= 3255 MiB, required (with KV overhead)
        // ~= 3743 MiB. Comfortable needs >= 1.25x that (~4679 MiB).
        let m = spec_with_params(Some(7.0));
        assert_eq!(estimate_vram_fit(&m, Some(4_000)), VramFit::Tight);
    }

    #[test]
    fn parse_min_free_mb_picks_minimum_across_devices() {
        let json = r#"{"metrics":[
            {"index":0,"utilization_pct":10.0,"memory_used_mb":100,"memory_free_mb":20000,"temperature_c":50.0,"power_usage_w":100.0,"fan_speed_pct":null},
            {"index":1,"utilization_pct":10.0,"memory_used_mb":100,"memory_free_mb":6000,"temperature_c":50.0,"power_usage_w":100.0,"fan_speed_pct":null}
        ]}"#;
        assert_eq!(parse_min_free_mb(json), Some(6000));
    }

    #[test]
    fn parse_min_free_mb_empty_metrics_is_none() {
        let json = r#"{"metrics":[]}"#;
        assert_eq!(parse_min_free_mb(json), None);
    }

    #[test]
    fn free_vram_hint_defaults_to_none_and_round_trips() {
        set_free_vram_mb_hint(None);
        assert_eq!(free_vram_mb_hint(), None);
        set_free_vram_mb_hint(Some(12_345));
        assert_eq!(free_vram_mb_hint(), Some(12_345));
        // Reset for other tests sharing this process-global.
        set_free_vram_mb_hint(None);
    }
}
