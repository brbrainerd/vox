use crate::mens::hardware::probe::{HardwareProbe, ProbeError};
use crate::mens::hardware::types::HardwareSummary;
use async_trait::async_trait;

/// Minimal Windows hardware probe: attempts a best-effort GPU name lookup via the
/// `DISPLAY` adapter name in registry, falling back to a CPU-only report.
///
/// This probe is always applicable on Windows and returns `Ok(None)` when no
/// hardware-accelerated adapter is found — letting the pipeline fall through to
/// the CPU fallback summary.
pub struct WindowsFallbackProbe;

#[async_trait]
impl HardwareProbe for WindowsFallbackProbe {
    fn name(&self) -> &'static str {
        "windows_fallback"
    }

    fn applicable(&self) -> bool {
        cfg!(target_os = "windows")
    }

    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError> {
        #[cfg(target_os = "windows")]
        {
            use crate::mens::hardware::types::{ComputeBackend, GpuVendor};

            let name = windows_adapter_name().unwrap_or_default();
            if name.is_empty() {
                return Ok(None);
            }
            let vendor = if name.to_ascii_lowercase().contains("nvidia") {
                GpuVendor::Nvidia
            } else if name.to_ascii_lowercase().contains("amd")
                || name.to_ascii_lowercase().contains("radeon")
            {
                GpuVendor::Amd
            } else if name.to_ascii_lowercase().contains("intel") {
                GpuVendor::Intel
            } else {
                GpuVendor::Unknown
            };
            return Ok(Some(HardwareSummary {
                model_name: name,
                vram_mb: 0,
                gpu_count: 1,
                vendor,
                backend: ComputeBackend::Cpu,
                driver_version: None,
                pci_bus_id: None,
                probe_failures: None,
            }));
        }
        #[cfg(not(target_os = "windows"))]
        Ok(None)
    }
}

/// Read the first display adapter description from the Windows registry.
#[cfg(target_os = "windows")]
fn windows_adapter_name() -> Option<String> {
    use std::process::Command;
    let out = Command::new("reg")
        .args([
            "query",
            r"HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0000",
            "/v",
            "DriverDesc",
        ])
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines() {
        let line = line.trim();
        if line.starts_with("DriverDesc") {
            let parts: Vec<&str> = line.splitn(3, "REG_SZ").collect();
            if let Some(name) = parts.get(1) {
                let name = name.trim().to_string();
                if !name.is_empty() {
                    return Some(name);
                }
            }
        }
    }
    None
}
