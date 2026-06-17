//! NVML probe logic: hardware summary and per-device telemetry.
//!
//! Both functions return JSON strings — the plugin trait doesn't know about
//! serde types so we serialize here and hand back raw strings to the host.

use serde::Serialize;
use thiserror::Error;

const BYTES_TO_MB: u64 = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("NVML library unavailable: {0}")]
    LibraryUnavailable(String),
    #[error("NVML device error: {0}")]
    DeviceError(String),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

// ── Summary types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct DeviceSummary {
    index: u32,
    name: String,
    vram_total_mb: u64,
    vram_free_mb: u64,
    vram_used_mb: u64,
    compute_capability: Option<String>,
}

#[derive(Serialize)]
struct ProbeSummary {
    device_count: u32,
    driver_version: Option<String>,
    devices: Vec<DeviceSummary>,
}

/// Probe all NVIDIA GPUs and return a JSON summary.
///
/// Schema (subject to plugin versioning):
/// ```json
/// {
///   "device_count": 2,
///   "driver_version": "535.54.03",
///   "devices": [
///     { "index": 0, "name": "NVIDIA RTX 4090", "vram_total_mb": 24576,
///       "vram_free_mb": 20480, "vram_used_mb": 4096 }
///   ]
/// }
/// ```
pub fn probe_summary() -> Result<String, ProbeError> {
    use nvml_wrapper::Nvml;

    let nvml = Nvml::init().map_err(|e| ProbeError::LibraryUnavailable(e.to_string()))?;
    let count = nvml
        .device_count()
        .map_err(|e| ProbeError::DeviceError(e.to_string()))?;
    let driver_version = nvml.sys_driver_version().ok();

    let mut devices = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let device = match nvml.device_by_index(idx) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(idx, error = %e, "NVML: failed to open device");
                continue;
            }
        };
        let name = device
            .name()
            .map_err(|e| ProbeError::DeviceError(e.to_string()))?;
        let (total_mb, free_mb, used_mb) = match device.memory_info() {
            Ok(m) => (
                m.total / BYTES_TO_MB,
                m.free / BYTES_TO_MB,
                m.used / BYTES_TO_MB,
            ),
            Err(_) => (0, 0, 0),
        };
        let compute_capability = device
            .cuda_compute_capability()
            .ok()
            .map(|cc| format!("{}.{}", cc.major, cc.minor));
        devices.push(DeviceSummary {
            index: idx,
            name,
            vram_total_mb: total_mb,
            vram_free_mb: free_mb,
            vram_used_mb: used_mb,
            compute_capability,
        });
    }

    let summary = ProbeSummary {
        device_count: count,
        driver_version,
        devices,
    };
    Ok(serde_json::to_string(&summary)?)
}

// ── Telemetry types ──────────────────────────────────────────────────────────

#[derive(Serialize)]
struct DeviceMetrics {
    index: u32,
    utilization_pct: f32,
    memory_used_mb: u64,
    memory_free_mb: u64,
    temperature_c: f32,
    power_usage_w: f32,
    fan_speed_pct: Option<f32>,
}

#[derive(Serialize)]
struct DeviceMetricsReport {
    metrics: Vec<DeviceMetrics>,
}

/// Probe per-device real-time metrics and return a JSON report.
///
/// Schema:
/// ```json
/// {
///   "metrics": [
///     { "index": 0, "utilization_pct": 72.5, "memory_used_mb": 4096,
///       "memory_free_mb": 20480, "temperature_c": 68.0,
///       "power_usage_w": 250.0, "fan_speed_pct": 55.0 }
///   ]
/// }
/// ```
pub fn device_metrics() -> Result<String, ProbeError> {
    use nvml_wrapper::Nvml;

    let nvml = Nvml::init().map_err(|e| ProbeError::LibraryUnavailable(e.to_string()))?;
    let count = nvml
        .device_count()
        .map_err(|e| ProbeError::DeviceError(e.to_string()))?;

    let mut metrics = Vec::with_capacity(count as usize);
    for idx in 0..count {
        let device = match nvml.device_by_index(idx) {
            Ok(d) => d,
            Err(e) => {
                tracing::warn!(idx, error = %e, "NVML: failed to open device for metrics");
                continue;
            }
        };

        let utilization_pct = device
            .utilization_rates()
            .ok()
            .map(|u| u.gpu as f32)
            .unwrap_or(0.0);
        let (memory_used_mb, memory_free_mb) = match device.memory_info() {
            Ok(m) => (m.used / BYTES_TO_MB, m.free / BYTES_TO_MB),
            Err(_) => (0, 0),
        };
        let temperature_c = device
            .temperature(nvml_wrapper::enum_wrappers::device::TemperatureSensor::Gpu)
            .unwrap_or(0) as f32;
        let power_usage_w = device.power_usage().unwrap_or(0) as f32 / 1000.0; // mW → W
        let fan_speed_pct = device.fan_speed(0).ok().map(|f| f as f32);

        metrics.push(DeviceMetrics {
            index: idx,
            utilization_pct,
            memory_used_mb,
            memory_free_mb,
            temperature_c,
            power_usage_w,
            fan_speed_pct,
        });
    }

    let report = DeviceMetricsReport { metrics };
    Ok(serde_json::to_string(&report)?)
}

#[cfg(test)]
mod tests_cc {
    use super::*;

    #[test]
    fn test_summary_has_compute_capability() {
        let s = probe_summary().unwrap_or_default();
        if !s.is_empty() {
            assert!(s.contains("compute_capability"));
        }
    }
}
