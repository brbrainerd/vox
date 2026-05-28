use crate::mens::hardware::probe::{HardwareProbe, ProbeAttempt, ProbeOutcome, ProbeReport};
use crate::mens::hardware::types::{ComputeBackend, GpuVendor, HardwareSummary};
use std::time::Instant;
use tracing::debug;

/// Runs a sequence of [`HardwareProbe`]s in order, collecting an attempt log.
///
/// Call [`ProbePipeline::empty`] + [`ProbePipeline::with_probe`] to build a custom pipeline
/// (useful in tests and operator-override scenarios).
pub struct ProbePipeline {
    pub(crate) probes: Vec<Box<dyn HardwareProbe>>,
}

impl ProbePipeline {
    /// An empty pipeline with no probes. Add probes with [`Self::with_probe`].
    pub fn empty() -> Self {
        Self { probes: Vec::new() }
    }

    /// Appends a probe to the end of the pipeline order (builder style).
    pub fn with_probe(mut self, probe: Box<dyn HardwareProbe>) -> Self {
        self.probes.push(probe);
        self
    }

    /// Run all probes in order.
    ///
    /// Returns a [`ProbeReport`] whose `summary` is the first successful result,
    /// or a CPU-only fallback if every probe returned `NoDevice`, `NotApplicable`,
    /// or `Failed`. Failed probe names are collected into `summary.probe_failures`.
    pub async fn run(&self) -> ProbeReport {
        let mut attempts = Vec::new();
        let mut summary: Option<HardwareSummary> = None;
        let mut failures: Vec<String> = Vec::new();

        for probe in &self.probes {
            let name = probe.name();
            if !probe.applicable() {
                debug!(
                    "vox.mesh.probe.name" = name,
                    "vox.mesh.probe.outcome" = "not_applicable",
                    "vox.mesh.probe.duration_ms" = 0u64,
                );
                attempts.push(ProbeAttempt {
                    probe_name: name,
                    outcome: ProbeOutcome::NotApplicable,
                    duration_ms: 0,
                });
                continue;
            }
            let start = Instant::now();
            let res = probe.probe().await;
            let duration_ms = start.elapsed().as_millis() as u64;
            match res {
                Ok(Some(s)) => {
                    debug!(
                        "vox.mesh.probe.name" = name,
                        "vox.mesh.probe.outcome" = "found",
                        "vox.mesh.probe.duration_ms" = duration_ms,
                    );
                    let s_clone = s.clone();
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::Found(Box::new(s)),
                        duration_ms,
                    });
                    if summary.is_none() {
                        summary = Some(s_clone);
                    }
                }
                Ok(None) => {
                    debug!(
                        "vox.mesh.probe.name" = name,
                        "vox.mesh.probe.outcome" = "no_device",
                        "vox.mesh.probe.duration_ms" = duration_ms,
                    );
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::NoDevice,
                        duration_ms,
                    });
                }
                Err(e) => {
                    debug!(
                        "vox.mesh.probe.name" = name,
                        "vox.mesh.probe.outcome" = "failed",
                        "vox.mesh.probe.duration_ms" = duration_ms,
                        "vox.mesh.probe.error" = %e,
                    );
                    failures.push(name.to_string());
                    attempts.push(ProbeAttempt {
                        probe_name: name,
                        outcome: ProbeOutcome::Failed(e.to_string()),
                        duration_ms,
                    });
                }
            }
        }

        let mut summary = summary.unwrap_or_else(cpu_fallback);
        if !failures.is_empty() {
            summary.probe_failures = Some(failures);
        }
        ProbeReport { summary, attempts }
    }
}

impl ProbePipeline {
    /// Returns the names of all probes in this pipeline, in order.
    pub fn probe_names(&self) -> Vec<&str> {
        self.probes.iter().map(|p| p.name()).collect()
    }

    /// Validates that every name in `order` refers to an existing probe in this pipeline.
    ///
    /// Returns `Ok(())` if all names are known, or `Err(unknown_names)` listing the
    /// unrecognised entries. Called by `vox populi config check` before applying
    /// `[mesh.probe.order]` from the workspace config.
    pub fn validate_probe_names(&self, order: &[&str]) -> Result<(), Vec<String>> {
        let known: std::collections::HashSet<&str> = self.probes.iter().map(|p| p.name()).collect();
        let unknown: Vec<String> = order
            .iter()
            .filter(|&&name| !known.contains(name))
            .map(|&name| name.to_string())
            .collect();
        if unknown.is_empty() {
            Ok(())
        } else {
            Err(unknown)
        }
    }

    /// Returns a new pipeline with probes reordered according to `order`.
    ///
    /// Probes named in `order` appear first in the given sequence; any probes
    /// not listed are appended in their original relative order. Unknown names
    /// in `order` are silently ignored (use [`Self::validate_probe_names`] before
    /// calling this if operator-supplied input should be rejected on unknown names).
    pub fn reorder(self, order: &[&str]) -> Self {
        let mut remaining: Vec<Box<dyn HardwareProbe>> = self.probes;
        let mut reordered: Vec<Box<dyn HardwareProbe>> = Vec::with_capacity(remaining.len());

        for &name in order {
            if let Some(pos) = remaining.iter().position(|p| p.name() == name) {
                reordered.push(remaining.remove(pos));
            }
        }
        reordered.extend(remaining);
        Self { probes: reordered }
    }

    /// Returns the platform-default probe order.
    ///
    /// On Windows: DXGI → wgpu → NVML (feature-gated).
    /// On Linux: DRM probe.
    /// On macOS: Metal probe.
    pub fn default_for_platform() -> Self {
        #[allow(unused_mut)]
        let mut pipeline = Self::empty();

        #[cfg(target_os = "linux")]
        {
            pipeline =
                pipeline.with_probe(Box::new(crate::mens::hardware::linux_drm::LinuxDrmProbe));
        }
        #[cfg(target_os = "macos")]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::macos_metal::MacosMetalProbe,
            ));
        }
        // nvml.rs deleted: NVML probing is owned by vox-plugin-nvml-probe.
        // wgpu_probe deleted: Burn/wgpu removed.
        #[cfg(target_os = "windows")]
        {
            pipeline = pipeline.with_probe(Box::new(
                crate::mens::hardware::windows_fallback::WindowsFallbackProbe,
            ));
        }

        pipeline
    }
}

fn cpu_fallback() -> HardwareSummary {
    HardwareSummary {
        model_name: "Host CPU".into(),
        vram_mb: 0,
        gpu_count: 0,
        vendor: GpuVendor::Cpu,
        backend: ComputeBackend::Cpu,
        driver_version: None,
        pci_bus_id: None,
        probe_failures: None,
    }
}
