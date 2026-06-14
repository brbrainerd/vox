//! VRAM auto-detection and training preset selection.
//!
//! Uses the HardwareRegistry SSOT to identify available video memory.

/// Query available GPU VRAM in GiB.
pub fn get_system_vram_gb() -> Option<f32> {
    // Priority 1: env override
    if let Some(v) = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxVramOverrideGb).expose()
        && let Ok(gb) = v.parse::<f32>()
        && gb > 0.0
    {
        return Some(gb);
    }

    // Priority 2: hardware SSOT
    let hardware = futures::executor::block_on(crate::mens::hardware::probe());
    if hardware.vram_mb > 0 {
        return Some(hardware.vram_mb as f32 / crate::mens::hardware::types::MB_PER_GB as f32);
    }

    // Priority 3: nvidia-smi fallback. The hardware probe is a stub on some
    // builds (returns 0); query the driver directly so VRAM-aware budgeting works
    // out of the box on any machine with an NVIDIA driver.
    if let Some(gb) = nvidia_smi_total_vram_gb() {
        return Some(gb);
    }

    None
}

/// Query total VRAM (GiB) of the first GPU via `nvidia-smi`. Returns `None` when
/// nvidia-smi is absent or unparseable.
fn nvidia_smi_total_vram_gb() -> Option<f32> {
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    // First line is the first GPU's total memory in MiB (nounits).
    let first = text.lines().next()?.trim();
    let mib: f32 = first.parse().ok()?;
    if mib > 0.0 { Some(mib / 1024.0) } else { None }
}

/// Select the best training preset for the detected hardware.
///
/// Returns a preset name matching `preset_schema.rs` known aliases.
/// Returns `None` when CUDA is not in use or VRAM is too low.
pub fn auto_preset(device_is_cuda: bool, vram_gb: Option<f32>) -> Option<&'static str> {
    if !device_is_cuda {
        return None;
    }
    match vram_gb {
        Some(v) if v < 6.0 => None, // Too small for QLoRA
        Some(v) if v < 10.0 => Some("safe"),
        Some(v) if v <= 16.0 => Some("qwen_4080_16g"),
        Some(v) if v <= 24.0 => Some("4080"),
        Some(_) => Some("a100"),
        None => None,
    }
}

/// Human-readable summary of detected VRAM + auto-selected preset.
pub fn vram_summary(device_is_cuda: bool) -> String {
    let vram = get_system_vram_gb();
    let preset = auto_preset(device_is_cuda, vram);
    match (vram, preset) {
        (Some(v), Some(p)) => format!("{:.1} GiB VRAM detected → preset '{p}'", v),
        (Some(v), None) => format!(
            "{:.1} GiB VRAM detected (no matching preset; specify --preset manually)",
            v
        ),
        (None, _) => {
            "Could not detect VRAM (set VOX_VRAM_OVERRIDE_GB or pass --preset manually)".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_preset_maps_correctly() {
        assert_eq!(auto_preset(true, Some(16.0)), Some("qwen_4080_16g"));
        assert_eq!(auto_preset(true, Some(8.0)), Some("safe"));
        assert_eq!(auto_preset(true, Some(80.0)), Some("a100"));
        assert_eq!(auto_preset(false, Some(16.0)), None);
        assert_eq!(auto_preset(true, Some(4.0)), None);
    }

    #[test]
    #[allow(unsafe_code)]
    fn vram_override_env_is_respected() {
        // Set a fake value and confirm it returns correctly.
        unsafe {
            std::env::set_var("VOX_VRAM_OVERRIDE_GB", "20.0");
        }
        assert_eq!(get_system_vram_gb(), Some(20.0));
        unsafe {
            std::env::remove_var("VOX_VRAM_OVERRIDE_GB");
        }
    }
}

#[cfg(test)]
mod semcov_wave26_tests {
    use super::*;

    // ── auto_preset boundary probes ──────────────────────────────────────────

    #[test]
    fn auto_preset_none_when_no_cuda() {
        // Catches: auto_preset() ignoring device_is_cuda=false and returning a
        // preset for any VRAM value, triggering CUDA ops on a CPU-only host.
        assert_eq!(auto_preset(false, Some(24.0)), None);
        assert_eq!(auto_preset(false, Some(100.0)), None);
        assert_eq!(auto_preset(false, None), None);
    }

    #[test]
    fn auto_preset_none_when_vram_none() {
        // Catches: auto_preset returning Some("safe") when VRAM is unknown,
        // which would silently run training with the wrong budget preset.
        assert_eq!(auto_preset(true, None), None);
    }

    #[test]
    fn auto_preset_exact_boundary_6gb() {
        // Catches: off-by-one at the 6.0 GiB threshold: < 6 → None, >= 6 → Some.
        // A "<= 6.0" comparison would incorrectly make 6.0 GiB → None (too small).
        assert_eq!(
            auto_preset(true, Some(6.0)),
            Some("safe"),
            "exactly 6.0 GiB should select 'safe', not be rejected as too small"
        );
        assert_eq!(
            auto_preset(true, Some(5.99)),
            None,
            "5.99 GiB is below the 6 GiB floor"
        );
    }

    #[test]
    fn auto_preset_exact_boundary_10gb() {
        // Catches: off-by-one at 10 GiB: < 10 → "safe", >= 10 → next tier.
        // A "<= 10.0" would wrongly assign 10.0 GiB → "safe" not "qwen_4080_16g".
        assert_eq!(
            auto_preset(true, Some(9.99)),
            Some("safe"),
            "9.99 GiB should be 'safe'"
        );
        // 10 GiB is between 10 and 16 → should land in qwen_4080_16g tier
        assert_eq!(
            auto_preset(true, Some(10.0)),
            Some("qwen_4080_16g"),
            "10.0 GiB should select 'qwen_4080_16g', not 'safe'"
        );
    }

    #[test]
    fn auto_preset_exact_boundary_16gb() {
        // Catches: strict `< 16` instead of `<= 16` that would push a 16 GiB card
        // into the "4080" (24 GiB) preset, over-allocating and causing OOM.
        assert_eq!(
            auto_preset(true, Some(16.0)),
            Some("qwen_4080_16g"),
            "16.0 GiB should be 'qwen_4080_16g'"
        );
        assert_ne!(
            auto_preset(true, Some(16.0)),
            Some("4080"),
            "16 GiB must not fall through to the 24 GiB '4080' preset"
        );
    }

    #[test]
    fn auto_preset_exact_boundary_24gb() {
        // Catches: `< 24` vs `<= 24` confusion: a 24 GiB card should use "4080",
        // not "a100"; using strict-less would silently bump it to a100.
        assert_eq!(
            auto_preset(true, Some(24.0)),
            Some("4080"),
            "24.0 GiB should be '4080'"
        );
        assert_ne!(
            auto_preset(true, Some(24.0)),
            Some("a100"),
            "24 GiB must not be assigned the a100 preset"
        );
    }

    #[test]
    fn auto_preset_very_large_vram_is_a100() {
        // Catches: missing wildcard arm that panics or returns None for very
        // large VRAM values (e.g. 80 GiB A100, 96 GiB H100).
        assert_eq!(auto_preset(true, Some(80.0)), Some("a100"));
        assert_eq!(auto_preset(true, Some(96.0)), Some("a100"));
    }

    #[test]
    fn auto_preset_zero_vram_returns_none() {
        // Catches: zero VRAM (e.g., hardware probe returning 0 and caller
        // converting to Some(0.0)) passing the 6-GiB floor and selecting "safe".
        assert_eq!(
            auto_preset(true, Some(0.0)),
            None,
            "0 GiB VRAM should be too small for any preset"
        );
    }

    // ── vram_summary format ───────────────────────────────────────────────────

    #[test]
    fn vram_summary_no_cuda_reports_no_preset_when_vram_known() {
        // Catches: vram_summary() ignoring device_is_cuda=false and printing a
        // CUDA preset name in the summary for a CPU-only machine.
        // We can only test the CPU-only path safely without env side-effects.
        let summary = vram_summary(false);
        // Either VRAM was detected or not; in neither case should a preset appear.
        assert!(
            !summary.contains("qwen_4080_16g")
                && !summary.contains("4080")
                && !summary.contains("a100")
                && !summary.contains("safe"),
            "CPU summary must not mention a GPU preset; got: {summary}"
        );
    }
}
