//! Core traits for PEFT adapters.

use candle_core::Tensor;
use candle_nn::VarMap;

use crate::Result;

/// Configuration trait for adapter hyperparameters.
pub trait AdapterConfig: Clone + Send + Sync {
    /// Validate the configuration parameters.
    ///
    /// # Errors
    ///
    /// Returns an error if the configuration is invalid.
    fn validate(&self) -> Result<()>;
}

/// Core adapter trait for parameter-efficient fine-tuning.
pub trait Adapter: Send + Sync {
    /// The configuration type for this adapter.
    type Config: AdapterConfig;

    /// Forward pass applying the adapter transformation.
    ///
    /// # Arguments
    /// * `input` - Input tensor
    /// * `base_output` - Optional output from the base layer (for residual adapters)
    ///
    /// # Returns
    /// Transformed tensor
    ///
    /// # Errors
    ///
    /// Returns an error if the forward pass fails.
    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor>;

    /// Get the number of trainable parameters.
    #[must_use]
    fn num_parameters(&self) -> usize;

    /// Get the adapter's configuration.
    fn config(&self) -> &Self::Config;
}

/// Trait for adapters that can be merged into base weights.
pub trait Mergeable: Adapter {
    /// Merge adapter weights into base model weights.
    ///
    /// # Arguments
    /// * `base_weight` - The original weight tensor to merge into
    ///
    /// # Returns
    /// New tensor with adapter weights merged
    ///
    /// # Errors
    ///
    /// Returns an error if merging fails.
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor>;

    /// Unmerge adapter weights from merged weights.
    ///
    /// # Arguments
    /// * `merged_weight` - Weight tensor with adapter already merged
    ///
    /// # Returns
    /// Original base weight tensor
    ///
    /// # Errors
    ///
    /// Returns an error if unmerging fails.
    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor>;
}

/// Trait for trainable adapters.
pub trait Trainable: Adapter {
    /// Register trainable parameters with the variable map.
    ///
    /// # Errors
    ///
    /// Returns an error if parameter registration fails.
    fn register_parameters(&self, var_map: &mut VarMap, prefix: &str) -> Result<()>;

    /// Freeze all adapter parameters (disable gradients).
    fn freeze(&mut self);

    /// Unfreeze all adapter parameters (enable gradients).
    fn unfreeze(&mut self);

    /// Check if the adapter is frozen.
    #[must_use]
    fn is_frozen(&self) -> bool;
}

#[cfg(test)]
mod tests {
    use super::*;

    // Trait object safety check
    fn _assert_adapter_object_safe(_: &dyn Adapter<Config = crate::LoraConfig>) {}
}
