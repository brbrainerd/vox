use crate::mens::hardware::pipeline::ProbePipeline;
use crate::mens::hardware::probe::ProbeReport;
use crate::mens::hardware::types::HardwareSummary;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

/// TTL-aware replacement for the bare `OnceCell` cache.
///
/// `HardwareRegistryV2` re-probes at most once per `cache_ttl` interval. The
/// first call always probes immediately (cold cache). Subsequent calls within the
/// TTL window return the cached [`Arc<HardwareSummary>`] without re-probing.
///
/// The lock is async-aware (`tokio::sync::Mutex`) so concurrent callers queue
/// rather than deadlock.
pub struct HardwareRegistryV2 {
    cache_ttl: Duration,
    state: Mutex<CacheState>,
}

struct CacheState {
    entry: Option<(Instant, Arc<HardwareSummary>)>,
}

impl HardwareRegistryV2 {
    /// Creates a registry with the given TTL. Use `Duration::MAX` to cache forever.
    pub fn new(cache_ttl: Duration) -> Self {
        Self {
            cache_ttl,
            state: Mutex::new(CacheState { entry: None }),
        }
    }

    /// Returns a shared reference to the hardware summary, re-probing if the cache
    /// has expired.
    pub async fn probe(&self) -> Arc<HardwareSummary> {
        let mut state = self.state.lock().await;
        if let Some((cached_at, ref summary)) = state.entry
            && cached_at.elapsed() < self.cache_ttl
        {
            return Arc::clone(summary);
        }
        let report = run_probe_internal().await;
        let arc = Arc::new(report.summary);
        state.entry = Some((Instant::now(), Arc::clone(&arc)));
        arc
    }

    /// Probe with the full attempt log, bypassing the cache.
    pub async fn probe_with_report(&self) -> ProbeReport {
        run_probe_internal().await
    }

    /// Invalidate the cache, forcing the next [`Self::probe`] call to re-probe.
    pub fn invalidate(&self) {
        if let Ok(mut state) = self.state.try_lock() {
            state.entry = None;
        }
        // If the lock is held (probe in progress), it will re-fill the cache
        // on completion. Callers that want a guaranteed fresh result should
        // call probe() after invalidate() and await its completion.
    }
}

async fn run_probe_internal() -> ProbeReport {
    // Clavis override: if the operator has set VoxGpuModel + VoxGpuVramMb, skip probing.
    use crate::mens::hardware::types::{ComputeBackend, vendor_from_model};
    if let (Some(model), Some(vram_s)) = (
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGpuModel).expose(),
        vox_secrets::resolve_secret(vox_secrets::SecretId::VoxGpuVramMb).expose(),
    ) && let Ok(vram_mb) = vram_s.parse::<u64>()
    {
        let summary = HardwareSummary {
            vendor: vendor_from_model(model),
            model_name: model.to_string(),
            vram_mb,
            gpu_count: 1,
            backend: ComputeBackend::Unknown,
            driver_version: None,
            pci_bus_id: None,
            probe_failures: None,
        };
        return ProbeReport {
            summary,
            attempts: Vec::new(),
        };
    }

    ProbePipeline::default_for_platform().run().await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn fresh_registry_probes_on_first_call() {
        let registry = HardwareRegistryV2::new(vox_config::timeouts::D_300S);
        let summary = registry.probe().await;
        // On CI without GPU hardware, the pipeline falls back to "Host CPU".
        assert!(!summary.model_name.is_empty());
    }

    #[tokio::test]
    async fn cached_result_returned_within_ttl() {
        let registry = HardwareRegistryV2::new(vox_config::timeouts::D_300S);
        let first = registry.probe().await;
        let second = registry.probe().await;
        assert!(Arc::ptr_eq(&first, &second), "expected same Arc within TTL");
    }

    #[tokio::test]
    async fn expired_cache_re_probes() {
        let registry = HardwareRegistryV2::new(vox_config::timeouts::D_1MS);
        let first = registry.probe().await;
        // Sleep past the 1ms TTL
        tokio::time::sleep(vox_config::timeouts::D_10MS).await;
        let second = registry.probe().await;
        // A fresh probe may return the same model name, but it must be a new Arc.
        assert!(
            !Arc::ptr_eq(&first, &second),
            "expected new Arc after TTL expiry"
        );
    }

    #[tokio::test]
    async fn invalidate_forces_re_probe() {
        let registry = HardwareRegistryV2::new(vox_config::timeouts::D_300S);
        let first = registry.probe().await;
        registry.invalidate();
        let second = registry.probe().await;
        assert!(
            !Arc::ptr_eq(&first, &second),
            "expected new Arc after invalidate"
        );
    }
}
