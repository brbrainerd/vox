//! Prefix Tuning implementation.
//!
//! Prefix tuning prepends trainable "prefix" vectors to the keys and values
//! in attention layers, without modifying the original model weights.
//!
//! Reference: <https://arxiv.org/abs/2101.00190>

use candle_core::{Device, IndexOp, Tensor};
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig};

/// Configuration for prefix tuning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixTuningConfig {
    /// Number of prefix tokens to prepend.
    pub num_prefix_tokens: usize,

    /// Hidden dimension of prefix vectors.
    pub prefix_dim: usize,

    /// Number of attention heads.
    pub num_heads: usize,

    /// Number of layers to apply prefix to.
    pub num_layers: usize,

    /// Whether to use a reparameterization MLP.
    #[serde(default = "default_true")]
    pub use_reparameterization: bool,
}

fn default_true() -> bool {
    true
}

impl Default for PrefixTuningConfig {
    fn default() -> Self {
        Self {
            num_prefix_tokens: 20,
            prefix_dim: 512,
            num_heads: 12,
            num_layers: 12,
            use_reparameterization: true,
        }
    }
}

impl AdapterConfig for PrefixTuningConfig {
    fn validate(&self) -> Result<()> {
        if self.num_prefix_tokens == 0 {
            return Err(PeftError::InvalidConfig(
                "num_prefix_tokens must be > 0".into(),
            ));
        }
        if self.prefix_dim == 0 {
            return Err(PeftError::InvalidConfig("prefix_dim must be > 0".into()));
        }
        Ok(())
    }
}

/// Prefix tuning layer.
///
/// Stores trainable prefix embeddings for keys and values.
pub struct PrefixTuningLayer {
    /// Prefix embeddings for keys: [`num_layers`, `num_prefix_tokens`, `num_heads`, `head_dim`]
    prefix_keys: Tensor,
    /// Prefix embeddings for values: [`num_layers`, `num_prefix_tokens`, `num_heads`, `head_dim`]
    prefix_values: Tensor,
    /// Configuration
    config: PrefixTuningConfig,
}

impl PrefixTuningLayer {
    /// Create a new prefix tuning layer.
    ///
    /// # Arguments
    /// * `config` - Prefix tuning configuration
    /// * `head_dim` - Dimension per attention head
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails or layer construction fails.
    pub fn new(config: PrefixTuningConfig, head_dim: usize, device: &Device) -> Result<Self> {
        config.validate()?;

        let shape = (
            config.num_layers,
            config.num_prefix_tokens,
            config.num_heads,
            head_dim,
        );

        // Initialize with small random values
        let prefix_keys = Tensor::randn(0.0f32, 0.02, shape, device)?;
        let prefix_values = Tensor::randn(0.0f32, 0.02, shape, device)?;

        Ok(Self {
            prefix_keys,
            prefix_values,
            config,
        })
    }

    /// Get prefix keys for a specific layer.
    ///
    /// # Errors
    ///
    /// Returns an error if the layer index is out of bounds.
    pub fn get_prefix_keys(&self, layer_idx: usize) -> Result<Tensor> {
        Ok(self.prefix_keys.i(layer_idx)?)
    }

    /// Get prefix values for a specific layer.
    ///
    /// # Errors
    ///
    /// Returns an error if the layer index is out of bounds.
    pub fn get_prefix_values(&self, layer_idx: usize) -> Result<Tensor> {
        Ok(self.prefix_values.i(layer_idx)?)
    }
}

impl Adapter for PrefixTuningLayer {
    type Config = PrefixTuningConfig;

    fn forward(&self, input: &Tensor, _base_output: Option<&Tensor>) -> Result<Tensor> {
        // Prefix tuning doesn't modify the input directly;
        // it provides prefixes to be concatenated in attention
        Ok(input.clone())
    }

    fn num_parameters(&self) -> usize {
        self.prefix_keys.elem_count() + self.prefix_values.elem_count()
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_prefix_tuning_creation() {
        let config = PrefixTuningConfig::default();
        let device = Device::Cpu;
        let layer = PrefixTuningLayer::new(config, 64, &device);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_prefix_shapes() {
        let config = PrefixTuningConfig {
            num_prefix_tokens: 10,
            num_heads: 8,
            num_layers: 6,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = PrefixTuningLayer::new(config, 64, &device).unwrap();

        let keys = layer.get_prefix_keys(0).unwrap();
        assert_eq!(keys.shape().dims(), &[10, 8, 64]);
    }
}
