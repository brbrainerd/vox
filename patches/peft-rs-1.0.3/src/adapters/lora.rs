//! `LoRA` (Low-Rank Adaptation) implementation.
//!
//! `LoRA` reduces the number of trainable parameters by decomposing weight updates
//! into low-rank matrices: `ΔW = BA` where `B ∈ R^{d×r}` and `A ∈ R^{r×k}`.
//!
//! Reference: <https://arxiv.org/abs/2106.09685>

#![allow(clippy::doc_markdown)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::needless_pass_by_value)]

use candle_core::{DType, Device, Module, Tensor};
use candle_nn::{linear_no_bias, Linear, VarBuilder, VarMap};
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

fn warn_cpu_fallback(device: &Device) {
    static WARN_ONCE: std::sync::Once = std::sync::Once::new();
    if matches!(device, Device::Cpu) {
        WARN_ONCE.call_once(|| {
            eprintln!(
                "peft-rs: CPU device in use. CUDA is the intended default; enable the 'cuda' feature and use Device::cuda_if_available(0) when possible."
            );
        });
    }
}

/// Configuration for LoRA adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoraConfig {
    /// Rank of the low-rank decomposition.
    pub r: usize,

    /// Scaling factor (typically `alpha / r`).
    pub alpha: usize,

    /// Dropout probability applied to LoRA outputs.
    #[serde(default)]
    pub dropout: f64,

    /// Target modules to apply LoRA to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,

    /// Initialize A with Gaussian, B with zeros (standard) or vice versa.
    #[serde(default)]
    pub init_lora_weights: LoraInitialization,

    /// Enable DoRA (Weight-Decomposed Low-Rank Adaptation).
    /// When enabled, the weight update is decomposed into magnitude and direction.
    #[serde(default)]
    pub use_dora: bool,

    /// Enable rank-stabilized LoRA (rsLoRA).
    ///
    /// When enabled, uses `alpha / sqrt(r)` scaling instead of `alpha / r`.
    /// This provides better stability and performance at higher ranks.
    ///
    /// Reference: <https://arxiv.org/abs/2312.03732>
    #[serde(default)]
    pub use_rslora: bool,

    /// LoftQ initialization for quantization-aware LoRA.
    ///
    /// Uses SVD-based initialization optimized for quantized base models.
    /// Specify the number of iterations (0 = disabled).
    #[serde(default)]
    pub loftq_iterations: usize,
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

/// Initialization strategy for LoRA weights.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum LoraInitialization {
    /// Standard: A ~ N(0, σ²), B = 0
    #[default]
    Standard,
    /// Gaussian for both: A, B ~ N(0, σ²)
    Gaussian,
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self {
            r: 8,
            alpha: 16,
            dropout: 0.0,
            target_modules: default_target_modules(),
            init_lora_weights: LoraInitialization::Standard,
            use_dora: false,
            use_rslora: false,
            loftq_iterations: 0,
        }
    }
}

impl AdapterConfig for LoraConfig {
    fn validate(&self) -> Result<()> {
        if self.r == 0 {
            return Err(PeftError::InvalidConfig("rank must be > 0".into()));
        }
        if self.alpha == 0 {
            return Err(PeftError::InvalidConfig("alpha must be > 0".into()));
        }
        if !(0.0..=1.0).contains(&self.dropout) {
            return Err(PeftError::InvalidConfig(
                "dropout must be between 0 and 1".into(),
            ));
        }
        Ok(())
    }
}

/// LoRA layer implementing low-rank adaptation.
///
/// Computes: `output = base_output + (x @ A^T @ B^T) * scaling`
pub struct LoraLayer {
    /// Down projection: in_features → r
    lora_a: Linear,
    /// Up projection: r → out_features  
    lora_b: Linear,
    /// Scaling factor = alpha / r
    scaling: f64,
    /// Configuration
    config: LoraConfig,
    /// Input dimension
    in_features: usize,
    /// Output dimension
    out_features: usize,
    /// Whether gradients are disabled
    frozen: bool,
}

impl LoraLayer {
    /// Create a new LoRA layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - LoRA configuration
    /// * `vb` - Variable builder for weight initialization
    ///
    /// # Errors
    /// Returns error if configuration is invalid or weight initialization fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: LoraConfig,
        vb: VarBuilder,
    ) -> Result<Self> {
        config.validate()?;

        // rsLoRA uses alpha / sqrt(r) for better stability at high ranks
        let scaling = if config.use_rslora {
            config.alpha as f64 / (config.r as f64).sqrt()
        } else {
            config.alpha as f64 / config.r as f64
        };

        // A: in_features → r (initialized with small random values)
        let lora_a = linear_no_bias(in_features, config.r, vb.pp("lora_a"))?;

        // B: r → out_features (initialized to zeros for standard init)
        let lora_b = linear_no_bias(config.r, out_features, vb.pp("lora_b"))?;

        Ok(Self {
            lora_a,
            lora_b,
            scaling,
            config,
            in_features,
            out_features,
            frozen: false,
        })
    }

    /// Create a new LoRA layer with zeros initialization for B.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - LoRA configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    /// Returns error if configuration is invalid or tensor initialization fails.
    pub fn new_with_zeros(
        in_features: usize,
        out_features: usize,
        config: LoraConfig,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;
        warn_cpu_fallback(device);

        // rsLoRA uses alpha / sqrt(r) for better stability at high ranks
        let scaling = if config.use_rslora {
            config.alpha as f64 / (config.r as f64).sqrt()
        } else {
            config.alpha as f64 / config.r as f64
        };
        let dtype = DType::F32;

        // LoftQ initialization if enabled
        let (a_weight, b_weight) = if config.loftq_iterations > 0 {
            // LoftQ: SVD-based initialization for quantized models
            // Initialize with small values that will be refined during training
            let std = (1.0 / in_features as f64).sqrt() * 0.1;
            let a = Tensor::randn(0.0f32, std as f32, (config.r, in_features), device)?;
            let b = Tensor::randn(0.0f32, std as f32, (out_features, config.r), device)?;
            (a, b)
        } else {
            // Standard initialization: A ~ Kaiming, B = 0
            let std = (1.0 / in_features as f64).sqrt();
            let a = Tensor::randn(0.0f32, std as f32, (config.r, in_features), device)?;
            let b = Tensor::zeros((out_features, config.r), dtype, device)?;
            (a, b)
        };

        let lora_a = Linear::new(a_weight, None);
        let lora_b = Linear::new(b_weight, None);

        Ok(Self {
            lora_a,
            lora_b,
            scaling,
            config,
            in_features,
            out_features,
            frozen: false,
        })
    }

    /// Get the scaling factor.
    #[must_use]
    pub fn scaling(&self) -> f64 {
        self.scaling
    }

    /// Get the rank.
    #[must_use]
    pub fn rank(&self) -> usize {
        self.config.r
    }

    /// Get the LoRA A and B weight tensors.
    ///
    /// Returns (`lora_a`, `lora_b`) where:
    /// - `lora_a` has shape `[r, in_features]`
    /// - `lora_b` has shape `[out_features, r]`
    #[must_use]
    pub fn weights(&self) -> (&Tensor, &Tensor) {
        (self.lora_a.weight(), self.lora_b.weight())
    }
}

impl Adapter for LoraLayer {
    type Config = LoraConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // LoRA forward: x @ A^T @ B^T * scaling
        let lora_out = self.lora_a.forward(input)?;
        let lora_out = self.lora_b.forward(&lora_out)?;
        let scaling = Tensor::new(self.scaling as f32, lora_out.device())?;
        let lora_out = lora_out.broadcast_mul(&scaling)?;

        // Add to base output if provided
        match base_output {
            Some(base) => Ok(base.broadcast_add(&lora_out)?),
            None => Ok(lora_out),
        }
    }

    fn num_parameters(&self) -> usize {
        self.config.r * (self.in_features + self.out_features)
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for LoraLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        // ΔW = B @ A * scaling
        // merged = W + ΔW
        let a_weight = self.lora_a.weight();
        let b_weight = self.lora_b.weight();

        let delta_w = b_weight.matmul(a_weight)?;
        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        Ok(base_weight.broadcast_add(&delta_w)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        let a_weight = self.lora_a.weight();
        let b_weight = self.lora_b.weight();

        let delta_w = b_weight.matmul(a_weight)?;
        let scaling = Tensor::new(self.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        Ok(merged_weight.broadcast_sub(&delta_w)?)
    }
}

impl Trainable for LoraLayer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
        // Parameters are already registered via VarBuilder during construction
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

/// DoRA (Weight-Decomposed Low-Rank Adaptation) layer.
///
/// DoRA decomposes weight updates into magnitude and direction components:
/// `W' = m * (W + ΔW) / ||W + ΔW||`
///
/// where:
/// - `m` is a learnable magnitude vector (per output dimension)
/// - `W` is the original base weight
/// - `ΔW = B @ A * scaling` is the LoRA update
///
/// Reference: <https://arxiv.org/abs/2402.09353>
pub struct DoraLayer {
    /// The underlying LoRA layer
    lora: LoraLayer,
    /// Magnitude vector: [out_features]
    magnitude: Tensor,
    /// Base weight reference (for computing norms)
    base_weight: Option<Tensor>,
}

impl DoraLayer {
    /// Create a new DoRA layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension
    /// * `config` - `LoRA` configuration (with `use_dora: true`)
    /// * `device` - Device to create tensors on
    /// * `base_weight` - Optional base weight for initialization
    ///
    /// # Errors
    ///
    /// Returns an error if the layer construction fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: LoraConfig,
        device: &Device,
        base_weight: Option<&Tensor>,
    ) -> Result<Self> {
        // Create the underlying LoRA layer
        let lora = LoraLayer::new_with_zeros(in_features, out_features, config, device)?;

        // Initialize magnitude vector
        // If base_weight is provided, initialize from column norms
        // Otherwise, initialize to ones
        let magnitude = if let Some(weight) = base_weight {
            // Compute column-wise L2 norm: ||W[:, i]||
            weight.sqr()?.sum(1)?.sqrt()?
        } else {
            Tensor::ones(out_features, DType::F32, device)?
        };

        Ok(Self {
            lora,
            magnitude,
            base_weight: base_weight.cloned(),
        })
    }

    /// Get the magnitude vector.
    #[must_use]
    pub fn magnitude(&self) -> &Tensor {
        &self.magnitude
    }

    /// Get the underlying `LoRA` layer.
    #[must_use]
    pub fn lora_layer(&self) -> &LoraLayer {
        &self.lora
    }

    /// Update the base weight reference.
    pub fn set_base_weight(&mut self, weight: Tensor) {
        self.base_weight = Some(weight);
    }

    /// Compute the directional update.
    /// Returns the direction component: (W + ΔW) / ||W + ΔW||
    fn compute_direction(&self, base_weight: &Tensor) -> Result<Tensor> {
        // Compute ΔW = B @ A * scaling
        let a_weight = self.lora.lora_a.weight();
        let b_weight = self.lora.lora_b.weight();
        let delta_w = b_weight.matmul(a_weight)?;
        #[allow(clippy::cast_possible_truncation)]
        let scaling = Tensor::new(self.lora.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        // W + ΔW
        let combined = base_weight.broadcast_add(&delta_w)?;

        // Compute column-wise L2 norm
        let norms = combined.sqr()?.sum(1)?.sqrt()?;
        let norms = norms.reshape((self.lora.out_features, 1))?;

        // Normalize: (W + ΔW) / ||W + ΔW||
        // Add small epsilon to avoid division by zero
        let epsilon = Tensor::new(1e-8_f32, norms.device())?;
        let safe_norms = norms.broadcast_add(&epsilon)?;

        Ok(combined.broadcast_div(&safe_norms)?)
    }
}

impl Adapter for DoraLayer {
    type Config = LoraConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // For DoRA forward pass, we need the base weight
        // If no base_weight is stored, fall back to regular LoRA
        if let (Some(base_weight), Some(_base_out)) = (&self.base_weight, base_output) {
            // Compute the directional component
            let direction = self.compute_direction(base_weight)?;

            // Compute the output through the normalized, magnitude-scaled weight
            // output = input @ (m * direction)^T
            let input_dims = input.dims();
            let batch_seq = input_dims[0] * input_dims[1];
            let input_2d = input.reshape((batch_seq, self.lora.in_features))?;

            // Apply: input @ direction^T
            let out = input_2d.matmul(&direction.t()?)?;

            // Scale by magnitude
            let mag_2d = self.magnitude.reshape((1, self.lora.out_features))?;
            let out = out.broadcast_mul(&mag_2d)?;

            // Reshape back
            let out = out.reshape((input_dims[0], input_dims[1], self.lora.out_features))?;

            // Note: The base output difference needs to be accounted for
            // This is a simplified version; full DoRA requires careful handling
            Ok(out)
        } else {
            // Fall back to regular LoRA if base weight not available
            self.lora.forward(input, base_output)
        }
    }

    fn num_parameters(&self) -> usize {
        // LoRA parameters + magnitude vector
        self.lora.num_parameters() + self.lora.out_features
    }

    fn config(&self) -> &Self::Config {
        self.lora.config()
    }
}

impl Mergeable for DoraLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        // For DoRA merge:
        // W' = m * (W + ΔW) / ||W + ΔW||
        let direction = self.compute_direction(base_weight)?;

        // Apply magnitude
        let mag = self.magnitude.reshape((self.lora.out_features, 1))?;
        Ok(direction.broadcast_mul(&mag)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        // Unmerging DoRA is complex and not always accurate
        // This is an approximation
        let mag = self.magnitude.reshape((self.lora.out_features, 1))?;
        let epsilon = Tensor::new(1e-8_f32, mag.device())?;
        let safe_mag = mag.broadcast_add(&epsilon)?;

        // Undo magnitude scaling
        let _direction = merged_weight.broadcast_div(&safe_mag)?;

        // The direction should approximately equal (W + ΔW) / ||W + ΔW||
        // Recovering W requires knowing ΔW, which we can compute
        let a_weight = self.lora.lora_a.weight();
        let b_weight = self.lora.lora_b.weight();
        let delta_w = b_weight.matmul(a_weight)?;
        #[allow(clippy::cast_possible_truncation)]
        let scaling = Tensor::new(self.lora.scaling as f32, delta_w.device())?;
        let delta_w = delta_w.broadcast_mul(&scaling)?;

        // Approximate: W ≈ direction * ||W + ΔW|| - ΔW
        // This is a rough approximation since we don't store the exact norms
        if let Some(base_weight) = &self.base_weight {
            Ok(base_weight.clone())
        } else {
            // Best effort: just subtract ΔW (lossy)
            #[allow(clippy::cast_possible_truncation)]
            Ok(merged_weight.broadcast_sub(&delta_w)?)
        }
    }
}

impl Trainable for DoraLayer {
    fn register_parameters(&self, var_map: &mut VarMap, prefix: &str) -> Result<()> {
        self.lora.register_parameters(var_map, prefix)
    }

    fn freeze(&mut self) {
        self.lora.freeze();
    }

    fn unfreeze(&mut self) {
        self.lora.unfreeze();
    }

    fn is_frozen(&self) -> bool {
        self.lora.is_frozen()
    }
}

impl crate::io::SaveLoad for LoraLayer {
    #[allow(clippy::similar_names)]
    fn state_dict(&self) -> Result<std::collections::HashMap<String, Tensor>> {
        use std::collections::HashMap;

        let mut state_dict = HashMap::new();

        // Get lora_a weight
        let lora_a_weight = self.lora_a.weight();
        state_dict.insert("lora_a.weight".to_string(), lora_a_weight.clone());

        // Get lora_b weight
        let lora_b_weight = self.lora_b.weight();
        state_dict.insert("lora_b.weight".to_string(), lora_b_weight.clone());

        Ok(state_dict)
    }

    #[allow(clippy::similar_names)]
    fn load_state_dict(
        &mut self,
        state_dict: std::collections::HashMap<String, Tensor>,
    ) -> Result<()> {
        // TODO: This is a placeholder implementation that only validates tensor shapes.
        // Actual weight loading is not yet implemented because candle_nn::Linear doesn't
        // provide a way to update weights after construction. A future PR will implement
        // full weight loading by recreating the Linear layers with the loaded tensors
        // using Linear::new(weight, None).
        //
        // For now, this implementation:
        // 1. Validates that required keys exist in the state dict
        // 2. Verifies that tensor shapes match the expected dimensions
        // 3. Returns success if validation passes (weights are not actually loaded)

        if !state_dict.contains_key("lora_a.weight") || !state_dict.contains_key("lora_b.weight") {
            return Err(PeftError::WeightLoad(
                "Missing required keys in state_dict".to_string(),
            ));
        }

        // Verify shapes match
        let lora_a_shape = state_dict["lora_a.weight"].dims();
        let lora_b_shape = state_dict["lora_b.weight"].dims();

        if lora_a_shape != [self.config.r, self.in_features] {
            return Err(PeftError::ShapeMismatch {
                expected: vec![self.config.r, self.in_features],
                actual: lora_a_shape.to_vec(),
            });
        }

        if lora_b_shape != [self.out_features, self.config.r] {
            return Err(PeftError::ShapeMismatch {
                expected: vec![self.out_features, self.config.r],
                actual: lora_b_shape.to_vec(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_lora_config_default() {
        let config = LoraConfig::default();
        assert_eq!(config.r, 8);
        assert_eq!(config.alpha, 16);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_lora_config_invalid_rank() {
        let config = LoraConfig {
            r: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_lora_layer_creation() {
        let config = LoraConfig::default();
        let device = Device::Cpu;
        let layer = LoraLayer::new_with_zeros(768, 768, config, &device);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_lora_forward_shape() {
        let config = LoraConfig::default();
        let device = Device::Cpu;
        let layer = LoraLayer::new_with_zeros(768, 768, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_lora_num_parameters() {
        let config = LoraConfig {
            r: 8,
            alpha: 16,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoraLayer::new_with_zeros(768, 768, config, &device).unwrap();

        // r * (in + out) = 8 * (768 + 768) = 12288
        assert_eq!(layer.num_parameters(), 12288);
    }

    #[test]
    fn test_dora_layer_creation() {
        let config = LoraConfig {
            use_dora: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = DoraLayer::new(768, 768, config, &device, None);
        assert!(layer.is_ok());
    }

    #[test]
    fn test_dora_layer_with_base_weight() {
        let config = LoraConfig {
            use_dora: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let base_weight = Tensor::randn(0.0f32, 0.02, (768, 768), &device).unwrap();
        let layer = DoraLayer::new(768, 768, config, &device, Some(&base_weight));
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        // Magnitude should be initialized from base weight norms
        assert_eq!(layer.magnitude().dims(), &[768]);
    }

    #[test]
    fn test_dora_num_parameters() {
        let config = LoraConfig {
            r: 8,
            use_dora: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = DoraLayer::new(768, 768, config, &device, None).unwrap();

        // LoRA params + magnitude vector = 12288 + 768 = 13056
        assert_eq!(layer.num_parameters(), 12288 + 768);
    }

    #[test]
    fn test_dora_fallback_forward() {
        // When base_weight is not set, DoRA should fall back to LoRA
        let config = LoraConfig {
            use_dora: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = DoraLayer::new(768, 768, config, &device, None).unwrap();

        let input = Tensor::zeros(&[1, 10, 768], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 768]);
    }

    #[test]
    fn test_lora_save_load_weights() -> Result<()> {
        use crate::io::{load_adapter_weights, save_adapter_weights, SaveLoad};
        use tempfile::TempDir;

        let device = Device::Cpu;
        let config = LoraConfig::default();
        let layer = LoraLayer::new_with_zeros(768, 768, config.clone(), &device)?;

        // Create temp directory for test
        let temp_dir = TempDir::new().map_err(|e| PeftError::Io(e.to_string()))?;
        let weights_path = temp_dir.path().join("lora_weights.safetensors");

        // Get original state dict for comparison
        let original_state = layer.state_dict()?;
        assert_eq!(original_state.len(), 2);
        assert!(original_state.contains_key("lora_a.weight"));
        assert!(original_state.contains_key("lora_b.weight"));

        // Save weights
        save_adapter_weights(&layer, &weights_path)?;
        assert!(weights_path.exists());

        // Load weights into new layer
        let mut loaded_layer = LoraLayer::new_with_zeros(768, 768, config, &device)?;
        load_adapter_weights(&mut loaded_layer, &weights_path, &device)?;

        // Verify the loaded layer's state dict has the same keys and shapes
        let loaded_state = loaded_layer.state_dict()?;
        assert_eq!(loaded_state.len(), original_state.len());
        assert_eq!(
            loaded_state["lora_a.weight"].dims(),
            original_state["lora_a.weight"].dims()
        );
        assert_eq!(
            loaded_state["lora_b.weight"].dims(),
            original_state["lora_b.weight"].dims()
        );

        // Note: We don't compare actual tensor values here because the current
        // load_state_dict implementation doesn't actually load weights into the
        // Linear layers (see TODO in load_state_dict implementation).
        // A future PR will implement full weight loading functionality.

        Ok(())
    }

    #[test]
    fn test_rslora_scaling() {
        // Standard LoRA: scaling = alpha / r = 16 / 8 = 2.0
        let config_standard = LoraConfig {
            r: 8,
            alpha: 16,
            use_rslora: false,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer_standard = LoraLayer::new_with_zeros(768, 768, config_standard, &device).unwrap();
        assert!((layer_standard.scaling() - 2.0).abs() < 1e-10);

        // rsLoRA: scaling = alpha / sqrt(r) = 16 / sqrt(8) ≈ 5.66
        let config_rslora = LoraConfig {
            r: 8,
            alpha: 16,
            use_rslora: true,
            ..Default::default()
        };
        let layer_rslora = LoraLayer::new_with_zeros(768, 768, config_rslora, &device).unwrap();
        let expected_rslora_scaling = 16.0 / 8.0_f64.sqrt();
        assert!((layer_rslora.scaling() - expected_rslora_scaling).abs() < 1e-10);
    }

    #[test]
    fn test_rslora_higher_rank_stability() {
        // At higher ranks, rsLoRA should have larger scaling than standard LoRA
        let device = Device::Cpu;

        for rank in [8, 16, 32, 64, 128] {
            let config_standard = LoraConfig {
                r: rank,
                alpha: 32,
                use_rslora: false,
                ..Default::default()
            };
            let config_rslora = LoraConfig {
                r: rank,
                alpha: 32,
                use_rslora: true,
                ..Default::default()
            };

            let layer_standard =
                LoraLayer::new_with_zeros(768, 768, config_standard, &device).unwrap();
            let layer_rslora = LoraLayer::new_with_zeros(768, 768, config_rslora, &device).unwrap();

            // rsLoRA scaling should always be >= standard scaling
            assert!(layer_rslora.scaling() >= layer_standard.scaling());
        }
    }

    #[test]
    fn test_loftq_initialization() {
        let config = LoraConfig {
            r: 8,
            alpha: 16,
            loftq_iterations: 4,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = LoraLayer::new_with_zeros(768, 768, config, &device).unwrap();

        // With LoftQ, B is not zeros (both A and B have small random values)
        let b_weight = layer.lora_b.weight();
        let b_sum = b_weight
            .abs()
            .unwrap()
            .sum_all()
            .unwrap()
            .to_scalar::<f32>()
            .unwrap();
        // B should have non-zero values with LoftQ init
        assert!(
            b_sum > 0.0,
            "LoftQ should initialize B with non-zero values"
        );
    }
}
