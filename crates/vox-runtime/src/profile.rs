//! Runtime profile — the single dispatch axis between desktop and mobile.
//!
//! The whole point of this enum is that no downstream code needs an
//! `#[cfg(target_os = "ios")]` or `if mobile { ... } else { ... }` branch.
//! Each policy (scheduler thread count, journal flush, model loading) gets a
//! typed method on the profile, and the call site reads the policy off the
//! profile without knowing which platform it represents.

use serde::{Deserialize, Serialize};

/// Where this Vox runtime is executing.
///
/// Drives every per-target choice in the runtime layer. The default is
/// [`RuntimeProfile::Desktop`] because that's the lower-friction value for
/// development hosts; a Vox app that ships to mobile constructs its
/// `VoxConfig` with [`RuntimeProfile::Mobile`] explicitly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeProfile {
    /// Desktop (Tauri 2 shell): multi-threaded Tokio scheduler, free-running
    /// actors, periodic journal flushes, eager ML model retention.
    #[default]
    Desktop,
    /// Mobile (React Native + Expo + uniffi): single-threaded Tokio scheduler
    /// by default, suspendable actors, journal-on-lifecycle, lazy ML model
    /// retention with memory-pressure unload.
    Mobile,
}

impl RuntimeProfile {
    /// Default Tokio scheduler thread count for this profile.
    ///
    /// On desktop we let Tokio pick (its default is `num_cpus`); on mobile we
    /// pin to a single thread to honor battery + memory budgets.
    pub fn default_scheduler_threads(self) -> SchedulerThreads {
        match self {
            Self::Desktop => SchedulerThreads::Auto,
            Self::Mobile => SchedulerThreads::Single,
        }
    }

    /// How the workflow journal should flush to disk.
    pub fn journal_flush_strategy(self) -> JournalFlushStrategy {
        match self {
            Self::Desktop => JournalFlushStrategy::Periodic {
                interval_ms: 5_000,
            },
            // Mobile uses lifecycle-triggered flushes because the OS only
            // gives us ~30 seconds after backgrounding before potential kill.
            Self::Mobile => JournalFlushStrategy::OnLifecycle,
        }
    }

    /// How MENS / Candle ML models are kept in memory.
    pub fn model_loading_strategy(self) -> ModelLoadingStrategy {
        match self {
            Self::Desktop => ModelLoadingStrategy::Eager,
            Self::Mobile => ModelLoadingStrategy::Lazy {
                unload_on_memory_pressure: true,
            },
        }
    }

    /// Whether subsystems on this profile should opt into the [`Suspendable`]
    /// lifecycle hooks. Desktop subsystems can ignore them; mobile must
    /// implement them or risk data loss when the OS suspends the app.
    ///
    /// [`Suspendable`]: crate::lifecycle::Suspendable
    pub fn requires_suspend_hooks(self) -> bool {
        matches!(self, Self::Mobile)
    }
}

/// Tokio scheduler thread-count policy.
///
/// Used by [`RuntimeProfile::default_scheduler_threads`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SchedulerThreads {
    /// Let Tokio pick (`tokio::runtime::Builder::new_multi_thread`) — usually
    /// `num_cpus::get()` on the host machine.
    Auto,
    /// Single-threaded runtime (`tokio::runtime::Builder::new_current_thread`).
    Single,
    /// Explicit thread count. Honored on desktop; ignored on mobile (will
    /// always collapse to [`SchedulerThreads::Single`] regardless).
    Fixed(u32),
}

/// Journal flush policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum JournalFlushStrategy {
    /// Flush every `interval_ms` milliseconds. Desktop default.
    Periodic {
        /// Interval between disk flushes, in milliseconds.
        interval_ms: u64,
    },
    /// Flush only when the runtime receives [`crate::lifecycle::Suspendable::suspend`]
    /// or an equivalent OS-lifecycle hook. Mobile default.
    OnLifecycle,
}

/// ML model loading + retention policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModelLoadingStrategy {
    /// Load models at startup, retain in RAM for the process lifetime.
    /// Desktop default.
    Eager,
    /// Load on first use, evict under memory pressure.
    Lazy {
        /// Whether the runtime should respond to OS memory-pressure
        /// notifications by unloading idle models.
        unload_on_memory_pressure: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_is_default() {
        let p: RuntimeProfile = RuntimeProfile::default();
        assert_eq!(p, RuntimeProfile::Desktop);
    }

    #[test]
    fn desktop_threading_is_auto() {
        assert_eq!(
            RuntimeProfile::Desktop.default_scheduler_threads(),
            SchedulerThreads::Auto
        );
    }

    #[test]
    fn mobile_threading_is_single() {
        assert_eq!(
            RuntimeProfile::Mobile.default_scheduler_threads(),
            SchedulerThreads::Single
        );
    }

    #[test]
    fn desktop_journal_is_periodic() {
        let s = RuntimeProfile::Desktop.journal_flush_strategy();
        assert!(matches!(s, JournalFlushStrategy::Periodic { interval_ms: 5_000 }));
    }

    #[test]
    fn mobile_journal_is_on_lifecycle() {
        let s = RuntimeProfile::Mobile.journal_flush_strategy();
        assert!(matches!(s, JournalFlushStrategy::OnLifecycle));
    }

    #[test]
    fn desktop_models_are_eager() {
        assert_eq!(
            RuntimeProfile::Desktop.model_loading_strategy(),
            ModelLoadingStrategy::Eager
        );
    }

    #[test]
    fn mobile_models_are_lazy_with_pressure_unload() {
        let s = RuntimeProfile::Mobile.model_loading_strategy();
        assert_eq!(
            s,
            ModelLoadingStrategy::Lazy {
                unload_on_memory_pressure: true
            }
        );
    }

    #[test]
    fn requires_suspend_hooks_only_on_mobile() {
        assert!(!RuntimeProfile::Desktop.requires_suspend_hooks());
        assert!(RuntimeProfile::Mobile.requires_suspend_hooks());
    }

    #[test]
    fn profile_serializes_as_lowercase() {
        assert_eq!(
            serde_json::to_string(&RuntimeProfile::Desktop).unwrap(),
            "\"desktop\""
        );
        assert_eq!(
            serde_json::to_string(&RuntimeProfile::Mobile).unwrap(),
            "\"mobile\""
        );
    }

    #[test]
    fn profile_round_trips_through_json() {
        for &p in &[RuntimeProfile::Desktop, RuntimeProfile::Mobile] {
            let s = serde_json::to_string(&p).unwrap();
            let decoded: RuntimeProfile = serde_json::from_str(&s).unwrap();
            assert_eq!(decoded, p);
        }
    }
}
