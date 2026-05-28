//! Candle device selection and fresh-run artifact cleanup.
//!
//! Ported from vox-populi (SP3 sub-batch C).
//! `probe_gpu` is provided by crate::device (stub).

use std::path::Path;

use anyhow::Result;
use candle_core::Device;

use crate::device::DeviceKind;
use crate::train_log;

pub(super) fn select_candle_device(
    kind: DeviceKind,
    allow_cpu_fallback: bool,
) -> Result<(Device, String)> {
    let device_resolved = vox_secrets::resolve_secret(vox_secrets::SecretId::VoxCandleDevice);
    if device_resolved
        .expose()
        .is_some_and(|v| v.trim().to_lowercase() == "cpu")
    {
        return Ok((Device::Cpu, "cpu(forced)".into()));
    }

    let (device, label) = match kind {
        DeviceKind::Cpu => (Device::Cpu, "cpu".into()),
        DeviceKind::Cuda => anyhow::bail!("Plugin built for Metal. CUDA not supported in this build."),
        DeviceKind::Metal => {
            #[cfg(feature = "metal")]
            {
                (Device::new_metal(0)?, "metal:0".into())
            }
            #[cfg(not(feature = "metal"))]
            {
                anyhow::bail!(
                    "Plugin built without the `metal` feature — recompile with `--features metal`"
                );
            }
        }
        DeviceKind::Best => {
            let g = crate::device::probe_gpu();
            #[cfg(feature = "metal")]
            {
                if g.vendor.as_str() == "apple" {
                    let d = match Device::new_metal(0) {
                        Ok(device) => device,
                        Err(err) => {
                            if !allow_cpu_fallback {
                                anyhow::bail!("Metal unavailable and CPU fallback disabled: {err}");
                            }
                            train_log::warn("Metal unavailable — falling back to CPU");
                            Device::Cpu
                        }
                    };
                    let lbl = if matches!(d, Device::Cpu) {
                        "cpu(fallback)"
                    } else {
                        "metal:0"
                    };
                    (d, lbl.into())
                } else {
                    match Device::new_metal(0) {
                        Ok(device) => (device, "metal:0".into()),
                        Err(err) => {
                            if !allow_cpu_fallback {
                                anyhow::bail!("Metal unavailable and CPU fallback disabled: {err}");
                            }
                            train_log::warn(&format!(
                                "Metal unavailable ({err}) — falling back to CPU (GPU vendor probe='{}')",
                                g.vendor
                            ));
                            (Device::Cpu, "cpu(fallback)".into())
                        }
                    }
                }
            }
            #[cfg(not(feature = "metal"))]
            {
                let _ = g;
                if !allow_cpu_fallback {
                    anyhow::bail!("No GPU feature enabled and CPU fallback disabled");
                }
                train_log::warn("No GPU feature compiled — using CPU");
                (Device::Cpu, "cpu(no-gpu-build)".into())
            }
        }
    };
    Ok((device, label))
}

/// Remove prior QLoRA checkpoints in `out` for a `--force-restart` run.
pub(super) fn purge_fresh_start_artifacts(out: &Path) -> std::io::Result<()> {
    let Ok(rd) = std::fs::read_dir(out) else {
        return Ok(());
    };
    for ent in rd.flatten() {
        let name = ent.file_name();
        let s = name.to_string_lossy();
        let drop = s == crate::checkpoint_state::CHECKPOINT_FILENAME
            || s == "checkpoint_state.json.tmp"
            || s == "candle_qlora_adapter.safetensors"
            || (s.starts_with("pause_step_") && s.ends_with(".safetensors"))
            || (s.starts_with("checkpoint_step_") && s.ends_with(".safetensors"))
            || (s.starts_with("checkpoint_epoch_") && s.ends_with(".safetensors"));
        if drop {
            let _ = std::fs::remove_file(ent.path());
        }
    }
    Ok(())
}
