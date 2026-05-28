use crate::mens::hardware::types::HardwareSummary;
use async_trait::async_trait;

/// Errors returned by a [`HardwareProbe`] implementation.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum ProbeError {
    /// The underlying native library (NVML, wgpu, DRM, etc.) could not be loaded.
    #[error("library unavailable: {0}")]
    LibraryUnavailable(String),
    /// The process lacks the OS permissions required to query this device class.
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    /// The device was found but returned an error when queried.
    #[error("device-side error: {0}")]
    DeviceError(String),
    #[error("other: {0}")]
    Other(String),
}

/// A single hardware-detection backend (NVML, wgpu, DRM, Metal, DXGI, …).
///
/// Implementations are stateless; all state lives in the [`ProbePipeline`](crate::mens::hardware::pipeline::ProbePipeline).
#[async_trait]
pub trait HardwareProbe: Send + Sync {
    /// Short identifier used in logs and [`ProbeAttempt::probe_name`].
    fn name(&self) -> &'static str;
    /// Returns `true` if this probe is worth attempting on the current platform/build.
    fn applicable(&self) -> bool;
    /// Run the probe.
    ///
    /// - `Ok(Some(summary))` — device found and queried successfully.
    /// - `Ok(None)` — probe is applicable but no matching device is present on this machine.
    /// - `Err(_)` — probe is applicable and a device may exist, but the probe failed
    ///   (library missing, permission denied, device error). The pipeline records this as
    ///   a [`ProbeOutcome::Failed`] attempt and continues to the next probe.
    async fn probe(&self) -> Result<Option<HardwareSummary>, ProbeError>;
}

/// Outcome of a single [`HardwareProbe::probe`] call, as recorded in [`ProbeAttempt`].
#[derive(Debug, Clone)]
pub enum ProbeOutcome {
    /// [`HardwareProbe::applicable`] returned `false`; probe was skipped entirely.
    NotApplicable,
    /// Probe ran but found no matching device (`Ok(None)`).
    NoDevice,
    /// Probe ran and returned a hardware summary.
    Found(Box<HardwareSummary>),
    /// Probe returned an [`Err`]; the error message is preserved for diagnostics.
    Failed(String),
}

/// Record of one probe attempt within a [`ProbeReport`].
#[derive(Debug, Clone)]
pub struct ProbeAttempt {
    /// Value of [`HardwareProbe::name`] for the probe that was attempted.
    pub probe_name: &'static str,
    pub outcome: ProbeOutcome,
    /// Wall-clock duration of the probe call in milliseconds.
    pub duration_ms: u64,
}

/// Result of running the full [`ProbePipeline`](crate::mens::hardware::pipeline::ProbePipeline).
#[derive(Debug, Clone)]
pub struct ProbeReport {
    /// Best-available hardware summary: the first `Found` result, or the CPU-only fallback
    /// if every hardware probe returned `NoDevice`, `NotApplicable`, or `Failed`.
    pub summary: HardwareSummary,
    /// Ordered record of every probe attempt, including skipped and failed ones.
    pub attempts: Vec<ProbeAttempt>,
}
