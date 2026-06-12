//! Cached local CPU/RAM snapshot for scaling decisions.
//!
//! sysinfo refreshes are not free; the scaling tick runs frequently, so the
//! snapshot is cached for `CACHE_TTL` and refreshed lazily. Feature-gated behind
//! `system-metrics`; without it `snapshot()` returns `None` and scaling falls
//! back to its pre-existing (resource-unaware) behavior.

#[cfg(feature = "system-metrics")]
use std::sync::Mutex;
#[cfg(feature = "system-metrics")]
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalResourceSnapshot {
    /// Global CPU utilization percent (0–100).
    pub cpu_usage_pct: f32,
    /// Free (available) memory in MiB.
    pub memory_free_mb: u64,
}

#[cfg(feature = "system-metrics")]
mod probe {
    use super::*;
    use sysinfo::System;

    const CACHE_TTL: Duration = Duration::from_secs(5);

    struct ProbeState {
        system: System,
        last: Option<(Instant, LocalResourceSnapshot)>,
    }

    static PROBE: Mutex<Option<ProbeState>> = Mutex::new(None);

    /// Best-effort snapshot; `None` only if the probe lock is poisoned.
    pub fn snapshot() -> Option<LocalResourceSnapshot> {
        let mut guard = PROBE.lock().ok()?;
        let state = guard.get_or_insert_with(|| ProbeState {
            system: System::new_all(),
            last: None,
        });
        if let Some((at, snap)) = state.last {
            if at.elapsed() < CACHE_TTL {
                return Some(snap);
            }
        }
        // Repo idiom (orchestrator/scaling.rs, vox-ml-cli populi_cli.rs).
        state.system.refresh_cpu_all();
        state.system.refresh_memory();
        let snap = LocalResourceSnapshot {
            cpu_usage_pct: state.system.global_cpu_usage(),
            memory_free_mb: state.system.available_memory() / (1024 * 1024),
        };
        state.last = Some((Instant::now(), snap));
        Some(snap)
    }
}

#[cfg(feature = "system-metrics")]
pub use probe::snapshot;

/// Without the `system-metrics` feature there is no probe; scaling keeps its
/// pre-existing behavior (no local resource guard).
#[cfg(not(feature = "system-metrics"))]
pub fn snapshot() -> Option<LocalResourceSnapshot> {
    None
}

#[cfg(all(test, feature = "system-metrics"))]
mod tests {
    use super::*;

    #[test]
    fn snapshot_returns_plausible_values() {
        let s = snapshot().expect("probe");
        assert!(s.cpu_usage_pct >= 0.0);
        assert!(s.memory_free_mb > 0);
    }
}
