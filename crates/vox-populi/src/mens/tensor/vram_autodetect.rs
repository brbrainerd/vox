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
