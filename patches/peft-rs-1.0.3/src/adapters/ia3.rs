//! IA³ (Infused Adapter by Inhibiting and Amplifying Inner Activations) implementation.
//!
//! IA³ is an extremely parameter-efficient fine-tuning method that learns
//! rescaling vectors for keys, values, and feedforward layers.
//!
//! Reference: <https://arxiv.org/abs/2205.05638>

#![allow(clippy::doc_markdown)]
#![allow(clippy::uninlined_format_args)]

use candle_core::{DType, Device, Tensor};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for IA³ adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ia3Config {
    /// Target modules to apply IA³ to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,

    /// Modules treated as feedforward (scaling applied to input).
    /// Must be a subset of `target_modules`.
    #[serde(default)]
    pub feedforward_modules: Vec<String>,

    /// Whether to initialize the vectors in IA³ layers to ones.
    /// Setting this to false is discouraged.
    #[serde(default = "default_true")]
    pub init_ia3_weights: bool,

    /// Set to true if the layer stores weight like (fan_in, fan_out).
    #[serde(default)]
    pub fan_in_fan_out: bool,
}

fn default_target_modules() -> Vec<String> {
    vec!["k_proj".into(), "v_proj".into(), "down_proj".into()]
}

fn default_true() -> bool {
    true
}

impl Default for Ia3Config {
    fn default() -> Self {
        Self {
            target_modules: default_target_modules(),
            feedforward_modules: vec!["down_proj".into()],
            init_ia3_weights: true,
            fan_in_fan_out: false,
        }
    }
}

impl AdapterConfig for Ia3Config {
    fn validate(&self) -> Result<()> {
        if self.target_modules.is_empty() {
            return Err(PeftError::InvalidConfig(
                "target_modules cannot be empty".into(),
            ));
        }
        // Check that feedforward_modules is a subset of target_modules
        for ff_module in &self.feedforward_modules {
            if !self.target_modules.contains(ff_module) {
                return Err(PeftError::InvalidConfig(format!(
                    "feedforward_module '{}' must be in target_modules",
                    ff_module
                )));
            }
        }
        Ok(())
    }
}

/// IA³ layer implementing learned rescaling vectors.
///
/// For non-feedforward modules: `output = base_output * ia3_vector`
/// For feedforward modules: `output = base_layer(input * ia3_vector)`
pub struct Ia3Layer {
    /// The learned scaling vector.
    /// Shape: [out_features, 1] for non-feedforward, [1, in_features] for feedforward.
    ia3_l: Tensor,
    /// Configuration
    config: Ia3Config,
    /// Input dimension
    in_features: usize,
    /// Output dimension
    out_features: usize,
    /// Whether this is a feedforward layer
    is_feedforward: bool,
    /// Whether gradients are disabled
    frozen: bool,
}

impl Ia3Layer {
    /// Create a new IA³ layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `is_feedforward` - Whether this is a feedforward layer (scales input vs output)
    /// * `config` - IA³ configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    /// Returns error if configuration is invalid or tensor initialization fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        is_feedforward: bool,
        config: Ia3Config,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;

        // Initialize the scaling vector
        let ia3_l = if config.init_ia3_weights {
            // Initialize to ones (identity transform)
            if is_feedforward {
                Tensor::ones((1, in_features), DType::F32, device)?
            } else {
                Tensor::ones((out_features, 1), DType::F32, device)?
            }
        } else {
            // Random initialization
            if is_feedforward {
                Tensor::randn(0.0f32, 0.02, (1, in_features), device)?
            } else {
                Tensor::randn(0.0f32, 0.02, (out_features, 1), device)?
            }
        };

        Ok(Self {
            ia3_l,
            config,
            in_features,
            out_features,
            is_feedforward,
            frozen: false,
        })
    }

    /// Get the scaling vector.
    #[must_use]
    pub fn scaling_vector(&self) -> &Tensor {
        &self.ia3_l
    }

    /// Check if this is a feedforward layer.
    #[must_use]
    pub fn is_feedforward(&self) -> bool {
        self.is_feedforward
    }

    /// Apply IA³ scaling to input (for feedforward layers).
    ///
    /// # Arguments
    /// * `input` - Input tensor [batch, seq_len, in_features]
    ///
    /// # Returns
    /// Scaled input tensor
    ///
    /// # Errors
    /// Returns error if called on non-feedforward layer or tensor operations fail.
    pub fn scale_input(&self, input: &Tensor) -> Result<Tensor> {
        if !self.is_feedforward {
            return Err(PeftError::InvalidConfig(
                "scale_input called on non-feedforward IA³ layer".into(),
            ));
        }
        // ia3_l shape: [1, in_features], input shape: [batch, seq_len, in_features]
        // Need to reshape for broadcasting
        let scaling = self.ia3_l.reshape((1, 1, self.in_features))?;
        Ok(input.broadcast_mul(&scaling)?)
    }

    /// Apply IA³ scaling to output (for non-feedforward layers).
    ///
    /// # Arguments
    /// * `output` - Output tensor from base layer [batch, seq_len, out_features]
    ///
    /// # Returns
    /// Scaled output tensor
    ///
    /// # Errors
    /// Returns error if called on feedforward layer or tensor operations fail.
    pub fn scale_output(&self, output: &Tensor) -> Result<Tensor> {
        if self.is_feedforward {
            return Err(PeftError::InvalidConfig(
                "scale_output called on feedforward IA³ layer".into(),
            ));
        }
        // ia3_l shape: [out_features, 1], output shape: [batch, seq_len, out_features]
        // Reshape to [1, 1, out_features] for broadcasting
        let scaling = self.ia3_l.reshape((1, 1, self.out_features))?;
        Ok(output.broadcast_mul(&scaling)?)
    }
}

impl Adapter for Ia3Layer {
    type Config = Ia3Config;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        if self.is_feedforward {
            // For feedforward: scale the input before passing to base layer
            // The base layer computation should happen externally
            self.scale_input(input)
        } else {
            // For non-feedforward: scale the output from base layer
            match base_output {
                Some(output) => self.scale_output(output),
                None => Err(PeftError::InvalidConfig(
                    "Non-feedforward IA³ requires base_output".into(),
                )),
            }
        }
    }

    fn num_parameters(&self) -> usize {
        if self.is_feedforward {
            self.in_features
        } else {
            self.out_features
        }
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for Ia3Layer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        // For IA³, merging means multiplying base weights by the scaling vector
        // Weight shape: [out_features, in_features]
        // For feedforward: scale along in_features (column-wise)
        // For non-feedforward: scale along out_features (row-wise)

        if self.is_feedforward {
            // ia3_l shape: [1, in_features]
            // Broadcast multiply: each column scaled by corresponding element
            Ok(base_weight.broadcast_mul(&self.ia3_l)?)
        } else {
            // ia3_l shape: [out_features, 1]
            // Broadcast multiply: each row scaled by corresponding element
            Ok(base_weight.broadcast_mul(&self.ia3_l)?)
        }
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        // Unmerging IA³ can be inaccurate due to potential division by values close to zero
        // Add tolerance to avoid division by zero
        let tolerance = 1e-8_f32;
        let tolerance_tensor = Tensor::new(tolerance, self.ia3_l.device())?;
        let safe_divisor = self.ia3_l.broadcast_add(&tolerance_tensor)?;

        Ok(merged_weight.broadcast_div(&safe_divisor)?)
    }
}

impl Trainable for Ia3Layer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
        // Note: In the current design, tensors are created directly.
        // For full training support, tensors should be created via VarBuilder
        // during construction, which automatically registers them.
        // This is a simplified implementation suitable for inference.
        Ok(())
    }

    fn freeze(&mut self) {
        self.frozen = true;
    }

    fn unfreeze(&mut self) {
        self.frozen = false;
    }

    fn is_frozen(&self) -> bool {
        self.frozen
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ia3_config_default() {
        let config = Ia3Config::default();
        assert!(!config.target_modules.is_empty());
        assert!(config.init_ia3_weights);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_ia3_config_invalid_feedforward() {
        let config = Ia3Config {
            target_modules: vec!["q_proj".into()],
            feedforward_modules: vec!["not_in_targets".into()],
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_ia3_layer_creation_non_feedforward() {
        let config = Ia3Config::default();
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 768, false, config, &device);
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        assert!(!layer.is_feedforward());
        // Non-feedforward: scaling vector has shape [out_features, 1]
        assert_eq!(layer.scaling_vector().dims(), &[768, 1]);
    }

    #[test]
    fn test_ia3_layer_creation_feedforward() {
        let config = Ia3Config::default();
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 3072, true, config, &device);
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        assert!(layer.is_feedforward());
        // Feedforward: scaling vector has shape [1, in_features]
        assert_eq!(layer.scaling_vector().dims(), &[1, 768]);
    }

    #[test]
    fn test_ia3_num_parameters_non_feedforward() {
        let config = Ia3Config::default();
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 512, false, config, &device).unwrap();
        // Non-feedforward uses out_features
        assert_eq!(layer.num_parameters(), 512);
    }

    #[test]
    fn test_ia3_num_parameters_feedforward() {
        let config = Ia3Config::default();
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 3072, true, config, &device).unwrap();
        // Feedforward uses in_features
        assert_eq!(layer.num_parameters(), 768);
    }

    #[test]
    fn test_ia3_forward_non_feedforward() {
        let config = Ia3Config::default();
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 768, false, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let base_output = Tensor::ones(&[1, 10, 768], DType::F32, &device).unwrap();

        let output = layer.forward(&input, Some(&base_output)).unwrap();
        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_ia3_forward_feedforward() {
        let config = Ia3Config::default();
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 3072, true, config, &device).unwrap();

        let input = Tensor::ones(&[1, 10, 768], DType::F32, &device).unwrap();

        let output = layer.forward(&input, None).unwrap();
        // For feedforward, output has same shape as input
        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_ia3_initialized_to_ones() {
        let config = Ia3Config {
            init_ia3_weights: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = Ia3Layer::new(768, 768, false, config, &device).unwrap();

        // With init_ia3_weights=true, scaling should be all ones
        // So forward pass should return output unchanged
        let base_output = Tensor::full(2.0f32, &[1, 10, 768], &device).unwrap();
        let output = layer
            .forward(
                &Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap(),
                Some(&base_output),
            )
            .unwrap();

        // Output should equal base_output (scaled by 1)
        let output_sum: f32 = output.sum_all().unwrap().to_scalar().unwrap();
        let expected_sum = 2.0f32 * 1.0 * 10.0 * 768.0;
        assert!((output_sum - expected_sum).abs() < 1e-3);
    }
}
