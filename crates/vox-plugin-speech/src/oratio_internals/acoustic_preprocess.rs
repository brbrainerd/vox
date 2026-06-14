//! Lightweight preprocessing before STT: peak normalization with a strict latency budget.
//!
//! Set **`VOX_ORATIO_ACOUSTIC_PREPROCESS`** to `none` (default), `peak_normalize`, or `1` (alias for peak).

use std::time::Instant;

use serde::Serialize;

/// What ran in [`preprocess_audio_pcm_f32_reported`].
#[derive(Debug, Clone, Serialize)]
pub struct AcousticPreprocessDiagnostics {
    /// Effective mode (`none`, `peak_normalize`, or `skipped_budget`).
    pub mode: String,
    /// Wall time spent in preprocessing (milliseconds).
    pub wall_ms: f64,
    /// Absolute peak before processing.
    pub peak_before: f32,
    /// Absolute peak after processing.
    pub peak_after: f32,
    /// RMS level before any processing (only valid for rms_normalize mode).
    pub rms_before: f32,
    /// RMS level after processing.
    pub rms_after: f32,
    /// True when work exceeded `max_extra_ms` and the original buffer was returned.
    pub skipped_due_to_budget: bool,
}

fn peak_abs(samples: &[f32]) -> f32 {
    samples.iter().map(|x| x.abs()).fold(0.0f32, f32::max)
}

fn effective_mode() -> &'static str {
    match vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioAcousticPreprocess)
        .expose()
        .map(|s| s.to_string())
    {
        Some(s) => {
            let t = s.trim();
            if t.is_empty() || t == "0" || t.eq_ignore_ascii_case("none") {
                return "none";
            }
            if t == "1"
                || t.eq_ignore_ascii_case("peak")
                || t.eq_ignore_ascii_case("peak_normalize")
            {
                return "peak_normalize";
            }
            if t.eq_ignore_ascii_case("rms_normalize") || t.eq_ignore_ascii_case("rms") {
                return "rms_normalize";
            }
            "none"
        }
        None => "none",
    }
}

/// Preprocess PCM f32 mono (typ. -1..=1) with diagnostics; respects `max_extra_ms` wall budget.
#[must_use]
pub fn preprocess_audio_pcm_f32_reported(
    samples: &[f32],
    max_extra_ms: u64,
) -> (Vec<f32>, AcousticPreprocessDiagnostics) {
    let start = Instant::now();
    let mode = effective_mode();
    let peak_before = peak_abs(samples);
    if samples.is_empty() || mode == "none" {
        return (
            samples.to_vec(),
            AcousticPreprocessDiagnostics {
                mode: mode.to_string(),
                wall_ms: elapsed_ms(&start),
                peak_before,
                peak_after: peak_before,
                rms_before: 0.0,
                rms_after: 0.0,
                skipped_due_to_budget: false,
            },
        );
    }

    let mut rms_before = 0.0;
    let mut rms_after = 0.0;

    let out: Vec<f32> = if mode == "rms_normalize" {
        let target_rms_dbfs: f32 =
            vox_secrets::resolve_secret(vox_secrets::SecretId::VoxOratioRmsTargetDbfs)
                .expose()
                .and_then(|s| s.parse().ok())
                .unwrap_or(-18.0_f32);
        let target_rms_linear = 10.0_f32.powf(target_rms_dbfs / 20.0);
        rms_before = (samples.iter().map(|x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        let gain = if rms_before > 1e-6 {
            target_rms_linear / rms_before
        } else {
            1.0
        };
        let processed: Vec<f32> = samples
            .iter()
            .map(|x| (x * gain).clamp(-1.0, 1.0))
            .collect();
        rms_after = (processed.iter().map(|x| x * x).sum::<f32>() / processed.len() as f32).sqrt();
        processed
    } else if peak_before > 0.0 {
        let scale = 0.95 / peak_before;
        samples
            .iter()
            .map(|x| (x * scale).clamp(-1.0, 1.0))
            .collect()
    } else {
        samples.to_vec()
    };

    let wall_ms = elapsed_ms(&start);
    if wall_ms > max_extra_ms as f64 && max_extra_ms > 0 {
        return (
            samples.to_vec(),
            AcousticPreprocessDiagnostics {
                mode: "skipped_budget".to_string(),
                wall_ms,
                peak_before,
                peak_after: peak_before,
                rms_before,
                rms_after: rms_before,
                skipped_due_to_budget: true,
            },
        );
    }

    let peak_after = peak_abs(&out);
    (
        out,
        AcousticPreprocessDiagnostics {
            mode: mode.to_string(),
            wall_ms,
            peak_before,
            peak_after,
            rms_before,
            rms_after,
            skipped_due_to_budget: false,
        },
    )
}

fn elapsed_ms(start: &Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}

#[cfg(test)]
#[allow(unsafe_code)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn peak_normalize_scales_quiet_signal() {
        let _guard = TEST_LOCK.lock().unwrap();
        // SAFETY: tests are single-threaded here; env is restored after the assertion.
        unsafe {
            std::env::set_var("VOX_ORATIO_ACOUSTIC_PREPROCESS", "peak_normalize");
        }
        let quiet: Vec<f32> = vec![0.01, -0.01, 0.005];
        let (out, d) = preprocess_audio_pcm_f32_reported(&quiet, 100);
        assert!(d.peak_after > d.peak_before);
        assert!((out[0].abs() - 0.95).abs() < 0.01, "got {:?}", out);
        unsafe {
            std::env::remove_var("VOX_ORATIO_ACOUSTIC_PREPROCESS");
        }
    }

    #[test]
    fn rms_normalize_brings_quiet_to_target() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { std::env::set_var("VOX_ORATIO_ACOUSTIC_PREPROCESS", "rms_normalize") };
        let quiet: Vec<f32> = vec![0.001; 16000];
        let (_out, d) = preprocess_audio_pcm_f32_reported(&quiet, 1000);
        assert!(d.rms_after > d.rms_before);
        unsafe { std::env::remove_var("VOX_ORATIO_ACOUSTIC_PREPROCESS") };
    }

    // Catches: preprocess_audio_pcm_f32_reported returning a different Vec than the
    // input when mode=="none", causing unnecessary allocations or silent data mutation.
    #[test]
    fn none_mode_returns_identical_samples() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { std::env::set_var("VOX_ORATIO_ACOUSTIC_PREPROCESS", "none") };
        let samples: Vec<f32> = vec![0.5, -0.3, 0.1, 0.9];
        let (out, d) = preprocess_audio_pcm_f32_reported(&samples, 1000);
        assert_eq!(out, samples, "mode=none must preserve samples exactly");
        assert_eq!(d.mode, "none");
        assert!(!d.skipped_due_to_budget);
        unsafe { std::env::remove_var("VOX_ORATIO_ACOUSTIC_PREPROCESS") };
    }

    // Catches: peak_normalize clamping output to values above 1.0, which would
    // saturate downstream float audio and introduce distortion artifacts.
    #[test]
    fn peak_normalize_output_never_exceeds_one() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { std::env::set_var("VOX_ORATIO_ACOUSTIC_PREPROCESS", "peak_normalize") };
        // Samples spanning full range
        let samples: Vec<f32> = vec![1.0, -1.0, 0.5, -0.5];
        let (out, _d) = preprocess_audio_pcm_f32_reported(&samples, 1000);
        for &s in &out {
            assert!(
                s.abs() <= 1.0,
                "sample {s} exceeds ±1.0 after peak normalization"
            );
        }
        unsafe { std::env::remove_var("VOX_ORATIO_ACOUSTIC_PREPROCESS") };
    }

    // Catches: empty sample slice causing division-by-zero in rms_normalize when
    // computing mean-square (sum / len with len=0).
    #[test]
    fn empty_slice_does_not_panic_in_any_mode() {
        let _guard = TEST_LOCK.lock().unwrap();
        for mode in &["none", "peak_normalize", "rms_normalize"] {
            unsafe { std::env::set_var("VOX_ORATIO_ACOUSTIC_PREPROCESS", mode) };
            let empty: Vec<f32> = vec![];
            // Must not panic
            let (out, d) = preprocess_audio_pcm_f32_reported(&empty, 1000);
            assert!(out.is_empty(), "empty in → empty out for mode={mode}");
            assert!(!d.skipped_due_to_budget, "empty slice never triggers budget path");
            unsafe { std::env::remove_var("VOX_ORATIO_ACOUSTIC_PREPROCESS") };
        }
    }

    // Catches: peak_normalize using peak_before==0.0 path producing NaN or Inf
    // when dividing by zero (0.95 / 0.0), corrupting the output buffer.
    #[test]
    fn peak_normalize_all_zero_samples_produces_no_nan() {
        let _guard = TEST_LOCK.lock().unwrap();
        unsafe { std::env::set_var("VOX_ORATIO_ACOUSTIC_PREPROCESS", "peak_normalize") };
        let silent: Vec<f32> = vec![0.0; 100];
        let (out, d) = preprocess_audio_pcm_f32_reported(&silent, 1000);
        assert!(
            out.iter().all(|x| x.is_finite()),
            "all-zero input must not produce NaN/Inf, got: {out:?}"
        );
        assert_eq!(d.peak_before, 0.0);
        unsafe { std::env::remove_var("VOX_ORATIO_ACOUSTIC_PREPROCESS") };
    }
}
