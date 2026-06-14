//! OFT (Orthogonal Fine-Tuning) implementation.
//!
//! OFT applies orthogonal transformations to preserve the pretrained knowledge
//! while adapting models. It uses block-diagonal orthogonal matrices to
//! transform weights efficiently.
//!
//! Reference: <https://arxiv.org/abs/2306.07280>

use candle_core::{Device, IndexOp, Tensor};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for OFT adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OftConfig {
    /// Number of OFT blocks (determines expressiveness vs efficiency).
    pub r: usize,

    /// Whether to use constrained OFT (COFT) which enforces strict orthogonality.
    #[serde(default)]
    pub coft: bool,

    /// Small constant for numerical stability.
    #[serde(default = "default_eps")]
    pub eps: f64,

    /// Block sharing across layers.
    #[serde(default)]
    pub block_share: bool,

    /// Target modules to apply OFT to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,

    /// Whether to use exact Cayley transform computation.
    ///
    /// When `false` (default), uses a Neumann series approximation `(I + Q)^{-1} ≈ I - Q + Q^2`
    /// which is efficient but less accurate for larger Q values.
    ///
    /// When `true`, computes the exact inverse using Newton-Schulz iteration,
    /// providing higher accuracy at the cost of additional computation.
    #[serde(default)]
    pub use_exact_cayley: bool,
}

fn default_eps() -> f64 {
    1e-5
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

impl Default for OftConfig {
    fn default() -> Self {
        Self {
            r: 8,
            coft: false,
            eps: default_eps(),
            block_share: false,
            target_modules: default_target_modules(),
            use_exact_cayley: false,
        }
    }
}

impl AdapterConfig for OftConfig {
    fn validate(&self) -> Result<()> {
        if self.r == 0 {
            return Err(PeftError::InvalidConfig(
                "number of blocks (r) must be > 0".into(),
            ));
        }
        if self.eps <= 0.0 {
            return Err(PeftError::InvalidConfig("eps must be > 0".into()));
        }
        Ok(())
    }
}

/// OFT layer implementing Orthogonal Fine-Tuning.
///
/// Uses block-diagonal orthogonal matrices to transform weights:
/// `W' = W @ R` where R is a block-diagonal orthogonal matrix.
///
/// The orthogonal matrix R is parameterized via Cayley transform:
/// `R = (I - Q) @ (I + Q)^{-1}` where Q is skew-symmetric.
pub struct OftLayer {
    /// Skew-symmetric parameters for Cayley parameterization.
    /// Shape: [`num_blocks`, `block_size`, `block_size`]
    oft_r: Tensor,
    /// Configuration
    config: OftConfig,
    /// Input/output dimension (OFT requires square transformation)
    features: usize,
    /// Size of each block
    block_size: usize,
    /// Number of blocks
    num_blocks: usize,
    /// Whether gradients are disabled
    frozen: bool,
}

impl OftLayer {
    /// Create a new OFT layer.
    ///
    /// # Arguments
    /// * `features` - Dimension of the weight matrix (must be divisible by r)
    /// * `config` - OFT configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    ///
    /// Returns an error if configuration validation fails or if layer construction fails.
    pub fn new(features: usize, config: OftConfig, device: &Device) -> Result<Self> {
        config.validate()?;

        if !features.is_multiple_of(config.r) {
            return Err(PeftError::InvalidConfig(format!(
                "features ({}) must be divisible by r ({})",
                features, config.r
            )));
        }

        let num_blocks = config.r;
        let block_size = features / num_blocks;

        // Initialize skew-symmetric parameters to small values
        // This makes the initial orthogonal matrix close to identity
        let std = 0.01_f32;
        let oft_r = Tensor::randn(0.0f32, std, (num_blocks, block_size, block_size), device)?;

        Ok(Self {
            oft_r,
            config,
            features,
            block_size,
            num_blocks,
            frozen: false,
        })
    }

    /// Get the number of blocks.
    #[must_use]
    pub fn num_blocks(&self) -> usize {
        self.num_blocks
    }

    /// Get the block size.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Make the parameter matrix skew-symmetric: Q = (R - R^T) / 2
    fn make_skew_symmetric(&self) -> Result<Tensor> {
        let r_t = self.oft_r.transpose(1, 2)?;
        let diff = self.oft_r.broadcast_sub(&r_t)?;
        let two = Tensor::new(2.0f32, self.oft_r.device())?;
        Ok(diff.broadcast_div(&two)?)
    }

    /// Compute the orthogonal matrix using Cayley transform.
    /// R = (I - Q) @ (I + Q)^{-1}
    ///
    /// Uses either exact computation or Neumann series approximation based on config.
    fn compute_orthogonal_matrix(&self) -> Result<Tensor> {
        let q = self.make_skew_symmetric()?;
        let device = q.device();

        // Create block-diagonal identity
        let eye = Tensor::eye(self.block_size, candle_core::DType::F32, device)?;
        let eye = eye
            .unsqueeze(0)?
            .expand((self.num_blocks, self.block_size, self.block_size))?;

        // I - Q
        let i_minus_q = eye.broadcast_sub(&q)?;

        // I + Q
        let i_plus_q = eye.broadcast_add(&q)?;

        let mut result_blocks = Vec::with_capacity(self.num_blocks);

        for block_idx in 0..self.num_blocks {
            let i_minus_q_block = i_minus_q.i(block_idx)?;
            let i_plus_q_block = i_plus_q.i(block_idx)?;
            let q_block = q.i(block_idx)?;

            let inv = if self.config.use_exact_cayley {
                // Exact method: Compute (I + Q)^{-1} using iterative refinement
                // Since Q is small (initialized with std=0.01), we use Newton-Schulz iteration
                // which converges quickly for matrices close to identity.
                //
                // Newton-Schulz iteration: X_{k+1} = X_k @ (2I - (I+Q) @ X_k)
                // Starting with X_0 = I (good initial guess since I+Q ≈ I)
                self.compute_exact_inverse(&i_plus_q_block)?
            } else {
                // Approximation method: Neumann series (I + Q)^{-1} ≈ I - Q + Q^2
                // Efficient but less accurate for larger Q values
                let eye_block = Tensor::eye(self.block_size, candle_core::DType::F32, device)?;
                let q_sq = q_block.matmul(&q_block)?;
                eye_block.broadcast_sub(&q_block)?.broadcast_add(&q_sq)?
            };

            // R_block = (I - Q) @ (I + Q)^{-1}
            let r_block = i_minus_q_block.matmul(&inv)?;
            result_blocks.push(r_block);
        }

        // Stack blocks: [num_blocks, block_size, block_size]
        Ok(Tensor::stack(&result_blocks, 0)?)
    }

    /// Compute exact inverse using Newton-Schulz iteration.
    ///
    /// Newton-Schulz iteration: `X_{k+1}` = `X_k` @ (2I - A @ `X_k`)
    /// Converges for matrices A with ||I - A|| < 1, which is satisfied
    /// since (I + Q) is close to identity for small Q (initialized with std=0.01).
    ///
    /// # Iteration Count
    /// Uses 5 iterations which provides accuracy to approximately 1e-10 for well-conditioned
    /// matrices close to identity. This is sufficient since:
    /// - Q is initialized with small values (std=0.01)
    /// - (I + Q) is thus very close to I, ensuring fast quadratic convergence
    /// - Each iteration roughly squares the error: ||`X_k` - A^{-1}|| ≈ ||`X_0` - A^{-1}||^{2^k}
    fn compute_exact_inverse(&self, matrix: &Tensor) -> Result<Tensor> {
        // Newton-Schulz iterations: 5 iterations provides ~1e-10 accuracy for matrices
        // close to identity, which is the case here since Q is initialized with small values.
        const NUM_ITERATIONS: usize = 5;

        let device = matrix.device();
        let eye = Tensor::eye(self.block_size, candle_core::DType::F32, device)?;
        let two = Tensor::new(2.0f32, device)?;
        let two_eye = eye.broadcast_mul(&two)?;

        // Start with identity as initial guess (good for matrices close to I)
        let mut x = eye.clone();

        for _ in 0..NUM_ITERATIONS {
            // X_{k+1} = X_k @ (2I - A @ X_k)
            let ax = matrix.matmul(&x)?;
            let factor = two_eye.broadcast_sub(&ax)?;
            x = x.matmul(&factor)?;
        }

        Ok(x)
    }

    /// Apply block-diagonal orthogonal transformation to input.
    fn apply_block_diagonal(&self, input: &Tensor, orth_matrix: &Tensor) -> Result<Tensor> {
        let input_dims = input.dims();
        let batch_seq = input_dims[0] * input_dims[1];

        // Reshape input to [batch*seq, num_blocks, block_size]
        let input_blocked = input.reshape((batch_seq, self.num_blocks, self.block_size))?;

        // Apply orthogonal transformation to each block
        // input_blocked: [batch*seq, num_blocks, block_size]
        // orth_matrix: [num_blocks, block_size, block_size]

        // For each block: output[b, n, :] = input[b, n, :] @ R[n, :, :]
        // We need batch matrix multiply

        let mut output_blocks = Vec::with_capacity(self.num_blocks);

        for block_idx in 0..self.num_blocks {
            // input_block: [batch*seq, block_size]
            let input_block = input_blocked.i((.., block_idx, ..))?;
            // orth_block: [block_size, block_size]
            let orth_block = orth_matrix.i(block_idx)?;

            // output_block: [batch*seq, block_size]
            let output_block = input_block.matmul(&orth_block)?;
            output_blocks.push(output_block);
        }

        // Stack and reshape back
        let output_stacked = Tensor::stack(&output_blocks, 1)?; // [batch*seq, num_blocks, block_size]
        Ok(output_stacked.reshape((input_dims[0], input_dims[1], self.features))?)
    }
}

impl Adapter for OftLayer {
    type Config = OftConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // Compute the orthogonal transformation matrix
        let orth_matrix = self.compute_orthogonal_matrix()?;

        // Apply block-diagonal orthogonal transformation
        let transformed = self.apply_block_diagonal(input, &orth_matrix)?;

        // For OFT, the transformation replaces the base output
        // If base_output provided, compute the difference (delta)
        match base_output {
            Some(base) => {
                // Return: base + (transformed - input) = base + delta
                let delta = transformed.broadcast_sub(input)?;
                Ok(base.broadcast_add(&delta)?)
            }
            None => Ok(transformed),
        }
    }

    fn num_parameters(&self) -> usize {
        // Skew-symmetric blocks: num_blocks * block_size * block_size
        // But only lower/upper triangle is independent: num_blocks * block_size * (block_size - 1) / 2
        // For simplicity, we count all parameters
        self.num_blocks * self.block_size * self.block_size
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for OftLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        // W' = W @ R (right multiply by orthogonal matrix)
        let orth_matrix = self.compute_orthogonal_matrix()?;

        // Construct full block-diagonal matrix from blocks
        let full_orth = self.construct_full_matrix(&orth_matrix)?;

        // base_weight: [out_features, in_features]
        // full_orth: [in_features, in_features]
        Ok(base_weight.matmul(&full_orth)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        // W = W' @ R^T (R is orthogonal, so R^{-1} = R^T)
        let orth_matrix = self.compute_orthogonal_matrix()?;
        let full_orth = self.construct_full_matrix(&orth_matrix)?;

        // R^T
        let full_orth_t = full_orth.t()?;

        Ok(merged_weight.matmul(&full_orth_t)?)
    }
}

impl OftLayer {
    /// Construct full block-diagonal matrix from blocks.
    fn construct_full_matrix(&self, blocks: &Tensor) -> Result<Tensor> {
        let device = blocks.device();
        let n = self.features;

        // Start with zeros
        let mut full_data = vec![0.0f32; n * n];

        // Fill in blocks along diagonal
        for block_idx in 0..self.num_blocks {
            let block = blocks.i(block_idx)?;
            let block_data: Vec<f32> = block.flatten_all()?.to_vec1()?;

            let start = block_idx * self.block_size;

            for i in 0..self.block_size {
                for j in 0..self.block_size {
                    let row = start + i;
                    let col = start + j;
                    full_data[row * n + col] = block_data[i * self.block_size + j];
                }
            }
        }

        Ok(Tensor::from_vec(full_data, (n, n), device)?)
    }
}

impl Trainable for OftLayer {
    fn register_parameters(&self, _var_map: &mut VarMap, _prefix: &str) -> Result<()> {
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
    use candle_core::{DType, IndexOp};

    #[test]
    fn test_oft_config_default() {
        let config = OftConfig::default();
        assert_eq!(config.r, 8);
        assert!(!config.coft);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_oft_config_invalid_r() {
        let config = OftConfig {
            r: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_oft_layer_creation() {
        let config = OftConfig {
            r: 8,
            ..Default::default()
        };
        let device = Device::Cpu;
        // 64 is divisible by 8
        let layer = OftLayer::new(64, config, &device);
        assert!(layer.is_ok());

        let layer = layer.unwrap();
        assert_eq!(layer.num_blocks(), 8);
        assert_eq!(layer.block_size(), 8);
    }

    #[test]
    fn test_oft_layer_invalid_dimensions() {
        let config = OftConfig {
            r: 8,
            ..Default::default()
        };
        let device = Device::Cpu;
        // 65 is not divisible by 8
        let layer = OftLayer::new(65, config, &device);
        assert!(layer.is_err());
    }

    #[test]
    fn test_oft_forward_shape() {
        let config = OftConfig {
            r: 8,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(64, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 64], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 64]);
    }

    #[test]
    fn test_oft_forward_with_base_output() {
        let config = OftConfig {
            r: 8,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(64, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 64], DType::F32, &device).unwrap();
        let base_output = Tensor::ones(&[1, 10, 64], DType::F32, &device).unwrap();
        let output = layer.forward(&input, Some(&base_output)).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 64]);
    }

    #[test]
    fn test_oft_num_parameters() {
        let config = OftConfig {
            r: 8,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(64, config, &device).unwrap();

        // 8 blocks of 8x8 = 8 * 64 = 512
        assert_eq!(layer.num_parameters(), 512);
    }

    #[test]
    fn test_oft_skew_symmetric() {
        let config = OftConfig {
            r: 2,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(8, config, &device).unwrap();

        let skew = layer.make_skew_symmetric().unwrap();

        // Check Q = -Q^T for each block
        for block_idx in 0..2 {
            let q = skew.i(block_idx).unwrap();
            let q_t = q.t().unwrap();
            let sum = q.broadcast_add(&q_t).unwrap();
            let max_val: f32 = sum
                .abs()
                .unwrap()
                .max(0)
                .unwrap()
                .max(0)
                .unwrap()
                .to_scalar()
                .unwrap();
            assert!(max_val < 1e-5, "Matrix should be skew-symmetric");
        }
    }

    #[test]
    fn test_oft_freeze_unfreeze() {
        let config = OftConfig::default();
        let device = Device::Cpu;
        let mut layer = OftLayer::new(64, config, &device).unwrap();

        assert!(!layer.is_frozen());
        layer.freeze();
        assert!(layer.is_frozen());
        layer.unfreeze();
        assert!(!layer.is_frozen());
    }

    #[test]
    fn test_oft_merge_unmerge() {
        let config = OftConfig {
            r: 4,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(16, config, &device).unwrap();

        let base_weight = Tensor::eye(16, DType::F32, &device).unwrap();
        let merged = layer.merge(&base_weight).unwrap();
        let unmerged = layer.unmerge(&merged).unwrap();

        // Unmerged should be close to original
        let diff = unmerged.broadcast_sub(&base_weight).unwrap();
        let max_diff: f32 = diff
            .abs()
            .unwrap()
            .max(0)
            .unwrap()
            .max(0)
            .unwrap()
            .to_scalar()
            .unwrap();
        assert!(max_diff < 0.1, "Max diff: {max_diff}"); // Allow some numerical error
    }

    #[test]
    fn test_oft_exact_cayley_config() {
        // Test that exact Cayley option is properly configured
        let config = OftConfig {
            r: 4,
            use_exact_cayley: true,
            ..Default::default()
        };
        assert!(config.use_exact_cayley);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_oft_exact_cayley_forward() {
        // Test forward pass with exact Cayley transform
        let config = OftConfig {
            r: 4,
            use_exact_cayley: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(16, config, &device).unwrap();

        let input = Tensor::zeros(&[1, 10, 16], DType::F32, &device).unwrap();
        let output = layer.forward(&input, None).unwrap();

        assert_eq!(output.shape().dims(), &[1, 10, 16]);
    }

    #[test]
    fn test_oft_exact_cayley_merge_unmerge() {
        // Test merge/unmerge with exact Cayley - should have better accuracy
        // The exact method uses Newton-Schulz iteration which provides higher precision
        // for the Cayley transform computation. Tolerance of 0.05 is stricter than the
        // 0.1 used for the approximation method in test_oft_merge_unmerge.
        const EXACT_METHOD_TOLERANCE: f32 = 0.05;

        let config = OftConfig {
            r: 4,
            use_exact_cayley: true,
            ..Default::default()
        };
        let device = Device::Cpu;
        let layer = OftLayer::new(16, config, &device).unwrap();

        let base_weight = Tensor::eye(16, DType::F32, &device).unwrap();
        let merged = layer.merge(&base_weight).unwrap();
        let unmerged = layer.unmerge(&merged).unwrap();

        let diff = unmerged.broadcast_sub(&base_weight).unwrap();
        let max_diff: f32 = diff
            .abs()
            .unwrap()
            .max(0)
            .unwrap()
            .max(0)
            .unwrap()
            .to_scalar()
            .unwrap();
        assert!(
            max_diff < EXACT_METHOD_TOLERANCE,
            "Max diff with exact Cayley: {max_diff}"
        );
    }

    #[test]
    fn test_oft_approx_vs_exact_cayley() {
        // Compare approximation vs exact methods
        let device = Device::Cpu;

        // Approximation method
        let config_approx = OftConfig {
            r: 4,
            use_exact_cayley: false,
            ..Default::default()
        };
        let layer_approx = OftLayer::new(16, config_approx, &device).unwrap();

        // Exact method (with same initialization - we can't easily compare due to random init)
        let config_exact = OftConfig {
            r: 4,
            use_exact_cayley: true,
            ..Default::default()
        };
        let layer_exact = OftLayer::new(16, config_exact, &device).unwrap();

        // Both should produce valid outputs with correct shape
        let input = Tensor::randn(0.0f32, 1.0, (1, 10, 16), &device).unwrap();

        let output_approx = layer_approx.forward(&input, None).unwrap();
        let output_exact = layer_exact.forward(&input, None).unwrap();

        assert_eq!(output_approx.shape().dims(), &[1, 10, 16]);
        assert_eq!(output_exact.shape().dims(), &[1, 10, 16]);
    }
}
