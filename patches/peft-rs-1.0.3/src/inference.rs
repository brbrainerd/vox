//! Inference utilities for PEFT adapters.

use crate::error::{PeftError, Result};
use crate::registry::AdapterRegistry;
use crate::traits::Adapter;

/// Inference mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceMode {
    /// Use adapter
    Adapter,
    /// Use merged weights
    Merged,
    /// Base model only
    BaseOnly,
}

impl Default for InferenceMode {
    fn default() -> Self {
        Self::Adapter
    }
}

/// Batch adapter switcher.
pub struct BatchAdapterSwitcher<A: Adapter> {
    registry: AdapterRegistry<A>,
    mode: InferenceMode,
}

impl<A: Adapter> BatchAdapterSwitcher<A> {
    /// Create new switcher.
    #[must_use]
    pub fn new(registry: AdapterRegistry<A>) -> Self {
        Self { registry, mode: InferenceMode::default() }
    }

    /// Set mode.
    pub fn set_mode(&mut self, mode: InferenceMode) {
        self.mode = mode;
    }

    /// Get mode.
    #[must_use]
    pub fn mode(&self) -> InferenceMode {
        self.mode
    }

    /// Switch adapter.
    pub fn switch_adapter(&mut self, adapter_name: impl Into<String>) -> Result<()> {
        self.registry.set_active_adapter(adapter_name)
    }

    /// Get active adapter.
    #[must_use]
    pub fn active_adapter(&self) -> Option<&str> {
        self.registry.active_adapter_name()
    }

    /// Get registry.
    #[must_use]
    pub fn registry(&self) -> &AdapterRegistry<A> {
        &self.registry
    }

    /// Should apply adapter.
    #[must_use]
    pub fn should_apply_adapter(&self) -> bool {
        !matches!(self.mode, InferenceMode::BaseOnly)
    }
}

/// Inference metrics.
#[derive(Debug, Clone, Copy, Default)]
pub struct InferenceMetrics {
    /// Total calls
    pub total_calls: usize,
    /// Adapter switches
    pub adapter_switches: usize,
    /// Average time
    pub avg_time_ms: Option<f64>,
}

impl InferenceMetrics {
    /// Create new metrics.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record call.
    pub fn record_call(&mut self) {
        self.total_calls += 1;
    }

    /// Record switch.
    pub fn record_switch(&mut self) {
        self.adapter_switches += 1;
    }

    /// Set average time.
    pub fn set_avg_time(&mut self, time_ms: f64) {
        self.avg_time_ms = Some(time_ms);
    }

    /// Get switches per call.
    #[must_use]
    pub fn switches_per_call(&self) -> f64 {
        if self.total_calls == 0 {
            0.0
        } else {
            self.adapter_switches as f64 / self.total_calls as f64
        }
    }
}

/// Validate adapter compatibility.
pub fn validate_adapter_compatibility<A: Adapter>(adapters: &[&A]) -> Result<()> {
    if adapters.is_empty() {
        return Ok(());
    }

    let first_params = adapters[0].num_parameters();
    for (idx, adapter) in adapters.iter().enumerate().skip(1) {
        if adapter.num_parameters() != first_params {
            return Err(PeftError::InvalidConfig(format!(
                "Adapter {} has {} parameters, expected {}",
                idx, adapter.num_parameters(), first_params
            )));
        }
    }
    Ok(())
}
