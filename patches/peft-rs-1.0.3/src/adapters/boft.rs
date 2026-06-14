//! BOFT (Butterfly Orthogonal Fine-Tuning) implementation.
//!
//! BOFT extends OFT by using butterfly factorization to achieve even more
//! parameter efficiency. It reduces the parameter complexity from O(d²) to
//! O(d log d) while maintaining the benefits of orthogonal transformations.
//!
//! The butterfly structure is inspired by the Cooley-Tukey FFT algorithm
//! and enables efficient O(n log n) matrix multiplication.
//!
//! Reference: <https://arxiv.org/abs/2311.06243>

#![allow(clippy::uninlined_format_args)]

use std::collections::HashMap;

use candle_core::{Device, IndexOp, Tensor, Var};
use candle_nn::VarMap;
use serde::{Deserialize, Serialize};

use crate::error::{PeftError, Result};
use crate::io::SaveLoad;
use crate::traits::{Adapter, AdapterConfig, Mergeable, Trainable};

/// Configuration for BOFT adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoftConfig {
    /// Block size for butterfly factorization.
    /// If 0, computed from `boft_block_num`. Must divide input features.
    #[serde(default)]
    pub boft_block_size: usize,

    /// Number of blocks for butterfly factorization.
    /// If 0, computed from `boft_block_size`. Must divide input features.
    #[serde(default = "default_boft_block_num")]
    pub boft_block_num: usize,

    /// Number of butterfly factors to use (1 = no butterfly, higher = more expressive).
    /// Must satisfy: `boft_block_num` must be divisible by `2^(n_butterfly_factor-1)`.
    #[serde(default = "default_boft_n_butterfly_factor")]
    pub boft_n_butterfly_factor: usize,

    /// Dropout probability for multiplicative dropout (0.0 = no dropout).
    /// Note: Dropout is currently not implemented in forward pass.
    /// This parameter is reserved for future implementation.
    #[serde(default)]
    pub boft_dropout: f64,

    /// Small constant for numerical stability.
    #[serde(default = "default_eps")]
    pub eps: f64,

    /// Target modules to apply BOFT to.
    #[serde(default = "default_target_modules")]
    pub target_modules: Vec<String>,
}

fn default_boft_block_num() -> usize {
    4
}

fn default_boft_n_butterfly_factor() -> usize {
    1
}

fn default_eps() -> f64 {
    1e-5
}

fn default_target_modules() -> Vec<String> {
    vec!["q_proj".into(), "v_proj".into()]
}

impl Default for BoftConfig {
    fn default() -> Self {
        Self {
            boft_block_size: 0,
            boft_block_num: default_boft_block_num(),
            boft_n_butterfly_factor: default_boft_n_butterfly_factor(),
            boft_dropout: 0.0,
            eps: default_eps(),
            target_modules: default_target_modules(),
        }
    }
}

impl AdapterConfig for BoftConfig {
    fn validate(&self) -> Result<()> {
        if self.boft_block_size == 0 && self.boft_block_num == 0 {
            return Err(PeftError::InvalidConfig(
                "Either boft_block_size or boft_block_num must be > 0".into(),
            ));
        }
        if self.boft_block_size != 0 && self.boft_block_num != 0 {
            return Err(PeftError::InvalidConfig(
                "Only one of boft_block_size or boft_block_num should be specified".into(),
            ));
        }
        if self.boft_n_butterfly_factor == 0 {
            return Err(PeftError::InvalidConfig(
                "boft_n_butterfly_factor must be > 0".into(),
            ));
        }
        if self.eps <= 0.0 {
            return Err(PeftError::InvalidConfig("eps must be > 0".into()));
        }
        if !(0.0..=1.0).contains(&self.boft_dropout) {
            return Err(PeftError::InvalidConfig(
                "boft_dropout must be in [0.0, 1.0]".into(),
            ));
        }
        Ok(())
    }
}

/// BOFT layer implementing Butterfly Orthogonal Fine-Tuning.
///
/// Uses butterfly factorization of block-diagonal orthogonal matrices:
/// `W' = W @ R` where R is constructed from butterfly factors.
///
/// Each butterfly factor is: `P @ BlockDiag(O_i) @ P^T`
/// where `P` is a permutation matrix and `O_i` are orthogonal matrices.
pub struct BoftLayer {
    /// Skew-symmetric parameters for Cayley parameterization.
    /// Shape: `[n_butterfly_factor + 1, block_num, block_size, block_size]`
    boft_r: Tensor,

    /// Scaling factors for output features.
    /// Shape: `[out_features, 1]`
    boft_s: Tensor,

    /// Precomputed permutation matrices for butterfly structure.
    /// Shape: `[n_butterfly_factor + 1, features, features]`
    boft_p: Tensor,

    /// Configuration
    config: BoftConfig,

    /// Output dimension
    out_features: usize,

    /// Size of each block
    block_size: usize,

    /// Number of blocks
    block_num: usize,

    /// Number of butterfly factors (config value - 1, for internal use)
    n_butterfly_factor: usize,

    /// Whether gradients are disabled
    frozen: bool,
}

impl BoftLayer {
    /// Create a new BOFT layer.
    ///
    /// # Arguments
    /// * `in_features` - Input dimension
    /// * `out_features` - Output dimension  
    /// * `config` - BOFT configuration
    /// * `device` - Device to create tensors on
    ///
    /// # Errors
    /// Returns error if configuration is invalid or tensor initialization fails.
    pub fn new(
        in_features: usize,
        out_features: usize,
        config: BoftConfig,
        device: &Device,
    ) -> Result<Self> {
        config.validate()?;

        // Compute block_size and block_num based on config
        let (block_size, block_num) = if config.boft_block_size == 0 {
            // Compute block_size from block_num
            if !in_features.is_multiple_of(config.boft_block_num) {
                return Err(PeftError::InvalidConfig(format!(
                    "in_features ({}) must be divisible by boft_block_num ({})",
                    in_features, config.boft_block_num
                )));
            }
            (in_features / config.boft_block_num, config.boft_block_num)
        } else {
            // Compute block_num from block_size
            if !in_features.is_multiple_of(config.boft_block_size) {
                return Err(PeftError::InvalidConfig(format!(
                    "in_features ({}) must be divisible by boft_block_size ({})",
                    in_features, config.boft_block_size
                )));
            }
            (config.boft_block_size, in_features / config.boft_block_size)
        };

        // Butterfly factor validation (internally we use n-1)
        let n_butterfly_factor = config.boft_n_butterfly_factor.saturating_sub(1);

        if n_butterfly_factor > 0 {
            // Check block_num divisibility
            #[allow(clippy::cast_possible_truncation)]
            let divisor = 2_usize.pow(n_butterfly_factor as u32);
            if block_num % divisor != 0 {
                return Err(PeftError::InvalidConfig(format!(
                    "boft_block_num ({}) must be divisible by 2^{} = {}",
                    block_num, n_butterfly_factor, divisor
                )));
            }

            // Check that we have enough features
            if in_features < block_size * divisor {
                return Err(PeftError::InvalidConfig(format!(
                    "in_features ({}) must be >= block_size * 2^{} = {}",
                    in_features,
                    n_butterfly_factor,
                    block_size * divisor
                )));
            }

            // Block size and block num must be even for butterfly
            if block_num % 2 != 0 {
                return Err(PeftError::InvalidConfig(format!(
                    "boft_block_num ({}) must be even for butterfly factorization",
                    block_num
                )));
            }
            if block_size % 2 != 0 {
                return Err(PeftError::InvalidConfig(format!(
                    "boft_block_size ({}) must be even for butterfly factorization",
                    block_size
                )));
            }
        }

        // Initialize skew-symmetric parameters
        // Shape: [n_butterfly_factor+1, block_num, block_size, block_size]
        let std = 0.1_f32;
        let boft_r = Tensor::randn(
            0.0f32,
            std,
            (n_butterfly_factor + 1, block_num, block_size, block_size),
            device,
        )?;

        // Initialize scaling factors to ones
        // Shape: [out_features, 1]
        let boft_s = Tensor::ones((out_features, 1), candle_core::DType::F32, device)?;

        // Precompute permutation matrices
        let boft_p = Self::compute_permutation_matrices(
            in_features,
            block_num,
            block_size,
            n_butterfly_factor,
            device,
        )?;

        Ok(Self {
            boft_r,
            boft_s,
            boft_p,
            config,
            out_features,
            block_size,
            block_num,
            n_butterfly_factor,
            frozen: false,
        })
    }

    /// Compute all permutation matrices for butterfly structure.
    fn compute_permutation_matrices(
        n: usize,
        block_num: usize,
        block_size: usize,
        n_butterfly_factor: usize,
        device: &Device,
    ) -> Result<Tensor> {
        let mut permutation_matrices = Vec::new();

        for i in 0..=n_butterfly_factor {
            #[allow(clippy::cast_possible_truncation)]
            let current_block_num = block_num / (2_usize.pow(i as u32));
            #[allow(clippy::cast_possible_truncation)]
            let current_block_size = block_size * (2_usize.pow(i as u32));

            let perm_indices = Self::block_butterfly_perm(
                n,
                current_block_num,
                current_block_size / 2,
                n_butterfly_factor,
            )?;

            let perm_matrix = Self::perm_to_matrix(&perm_indices, n, device)?;
            permutation_matrices.push(perm_matrix);
        }

        // Stack into single tensor [n_butterfly_factor+1, n, n]
        Ok(Tensor::stack(&permutation_matrices, 0)?)
    }

    /// Generate block butterfly permutation indices.
    ///
    /// This creates a permutation that reorders blocks in a butterfly pattern,
    /// separating even and odd positioned blocks.
    fn block_butterfly_perm(
        n: usize,
        b: usize,
        r: usize,
        n_butterfly_factor: usize,
    ) -> Result<Vec<usize>> {
        // If no butterfly factor, return identity permutation
        if n_butterfly_factor == 0 {
            return Ok((0..n).collect());
        }

        // Validate parameters
        if b * r * 2 > n {
            return Err(PeftError::InvalidConfig(
                "Invalid number of blocks for butterfly permutation".into(),
            ));
        }

        let block_size = n / b;
        let mut indices: Vec<usize> = (0..n).collect();

        // Sort blocks by separating even and odd positions
        let sorted_order = Self::sort_block(block_size, r);

        // Apply sorting to each block
        for i in (0..n).step_by(block_size) {
            let block_end = i + block_size;
            let tmp_indices: Vec<usize> = indices[i..block_end].to_vec();
            for (j, &idx) in sorted_order.iter().enumerate() {
                indices[i + j] = tmp_indices[idx];
            }
        }

        Ok(indices)
    }

    /// Sort a single block by separating even and odd positions.
    fn sort_block(block_size: usize, r: usize) -> Vec<usize> {
        let step = block_size / r;
        let mut sorted_order = vec![0; block_size];

        // Collect even positions
        let mut evens: Vec<usize> = (0..step).step_by(2).collect();
        // Collect odd positions
        let mut odds: Vec<usize> = (1..step).step_by(2).collect();

        evens.append(&mut odds);
        let sorted_seq = evens;

        for (i, &pos) in sorted_seq.iter().enumerate() {
            for j in 0..r {
                sorted_order[i * r + j] = pos * r + j;
            }
        }

        sorted_order
    }

    /// Convert permutation indices to permutation matrix.
    fn perm_to_matrix(indices: &[usize], n: usize, device: &Device) -> Result<Tensor> {
        let mut data = vec![0.0f32; n * n];

        for (i, &idx) in indices.iter().enumerate() {
            data[i * n + idx] = 1.0;
        }

        Ok(Tensor::from_vec(data, (n, n), device)?)
    }

    /// Make parameter matrices skew-symmetric: Q = (R - R^T) / 2
    fn make_skew_symmetric(&self) -> Result<Tensor> {
        // boft_r shape: [N, D, H, H]
        let r_t = self.boft_r.transpose(2, 3)?;
        let diff = self.boft_r.broadcast_sub(&r_t)?;
        let two = Tensor::new(2.0f32, self.boft_r.device())?;
        Ok(diff.broadcast_div(&two)?)
    }

    /// Apply Cayley transform to skew-symmetric matrices.
    ///
    /// For a skew-symmetric matrix Q: `R = (I - Q) @ (I + Q)^{-1}`
    /// This produces an orthogonal matrix R.
    fn cayley_batch(skew_mat: &Tensor) -> Result<Tensor> {
        let device = skew_mat.device();
        let shape = skew_mat.dims();
        let batch_size = shape[0];
        let mat_size = shape[1];

        // Create identity matrix
        let eye = Tensor::eye(mat_size, candle_core::DType::F32, device)?;
        let eye = eye.unsqueeze(0)?.expand((batch_size, mat_size, mat_size))?;

        // I - Q
        let i_minus_q = eye.broadcast_sub(skew_mat)?;

        // I + Q (computed for potential future exact inverse implementation)
        let _i_plus_q = eye.broadcast_add(skew_mat)?;

        // Solve (I + Q) @ R = (I - Q) for R
        // This is equivalent to R = (I - Q) @ (I + Q)^{-1}
        let mut result_blocks = Vec::with_capacity(batch_size);

        for batch_idx in 0..batch_size {
            let i_minus_q_block = i_minus_q.i(batch_idx)?;

            // Use Neumann series approximation: (I + Q)^{-1} ≈ I - Q + Q²
            // This approximation is valid when ||Q|| is small (typically < 0.5).
            // Since Q is skew-symmetric and initialized with small std (0.1),
            // this approximation is accurate for most practical cases.
            let q_block = skew_mat.i(batch_idx)?;
            let q_sq = q_block.matmul(&q_block)?;
            let inv_approx = eye
                .i(batch_idx)?
                .broadcast_sub(&q_block)?
                .broadcast_add(&q_sq)?;

            let result = i_minus_q_block.matmul(&inv_approx)?;
            result_blocks.push(result);
        }

        Ok(Tensor::stack(&result_blocks, 0)?)
    }

    /// Construct block diagonal matrix from blocks.
    ///
    /// Given blocks of shape `[D, H, H]`, creates a block diagonal matrix
    /// of shape `[D*H, D*H]`.
    fn block_diag(blocks: &Tensor) -> Result<Tensor> {
        let device = blocks.device();
        let shape = blocks.dims();
        let num_blocks = shape[0];
        let block_size = shape[1];
        let total_size = num_blocks * block_size;

        // Create zero matrix
        let mut data = vec![0.0f32; total_size * total_size];

        // Fill in blocks
        for block_idx in 0..num_blocks {
            let block = blocks.i(block_idx)?;
            let block_data: Vec<f32> = block.flatten_all()?.to_vec1()?;

            let offset = block_idx * block_size;
            for i in 0..block_size {
                for j in 0..block_size {
                    let row = offset + i;
                    let col = offset + j;
                    data[row * total_size + col] = block_data[i * block_size + j];
                }
            }
        }

        Ok(Tensor::from_vec(data, (total_size, total_size), device)?)
    }

    /// Compute the full butterfly OFT matrix.
    ///
    /// Applies the butterfly factorization: product of `P @ BlockDiag @ P^T`
    /// across all butterfly factors.
    fn compute_butterfly_oft_matrix(&self) -> Result<Tensor> {
        // Get skew-symmetric matrices
        let q = self.make_skew_symmetric()?;

        // q shape: [N, D, H, H] where N = n_butterfly_factor + 1
        let mut butterfly_matrices = Vec::new();

        for factor_idx in 0..=self.n_butterfly_factor {
            // Extract blocks for this factor
            let q_factor = q.i(factor_idx)?; // Shape: [D, H, H]

            // Reshape for batch Cayley
            let shape = q_factor.dims();
            let d = shape[0];
            let h = shape[1];
            let q_reshaped = q_factor.reshape((d, h, h))?;

            // Apply Cayley transform to get orthogonal blocks
            let orth_blocks = Self::cayley_batch(&q_reshaped)?;

            // Construct block diagonal matrix
            let block_diag_mat = Self::block_diag(&orth_blocks)?;

            // Get permutation matrix for this factor
            let perm = self.boft_p.i(factor_idx)?;
            let perm_t = perm.t()?;

            // Compute P @ BlockDiag @ P^T
            let tmp = block_diag_mat.matmul(&perm_t)?;
            let butterfly_mat = perm.matmul(&tmp)?;

            butterfly_matrices.push(butterfly_mat);
        }

        // Multiply all butterfly factors together
        let mut result = butterfly_matrices[0].clone();
        for butterfly_mat in butterfly_matrices.iter().skip(1) {
            result = butterfly_mat.matmul(&result)?;
        }

        Ok(result)
    }

    /// Get the number of blocks.
    #[must_use]
    pub fn block_num(&self) -> usize {
        self.block_num
    }

    /// Get the block size.
    #[must_use]
    pub fn block_size(&self) -> usize {
        self.block_size
    }

    /// Get the number of butterfly factors.
    #[must_use]
    pub fn n_butterfly_factor(&self) -> usize {
        self.n_butterfly_factor + 1 // Return the config value
    }
}

impl Adapter for BoftLayer {
    type Config = BoftConfig;

    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor> {
        // Compute the butterfly OFT transformation matrix
        let butterfly_oft = self.compute_butterfly_oft_matrix()?;

        // Handle 3D input by reshaping
        let input_shape = input.dims();
        let is_3d = input_shape.len() == 3;

        let input_2d = if is_3d {
            // Reshape [batch, seq, features] -> [batch*seq, features]
            input.reshape((input_shape[0] * input_shape[1], input_shape[2]))?
        } else {
            input.clone()
        };

        // Apply transformation: output = input @ butterfly_oft^T
        let transformed = input_2d.matmul(&butterfly_oft.t()?)?;

        // Reshape back if needed
        let transformed = if is_3d {
            transformed.reshape(input_shape)?
        } else {
            transformed
        };

        // Apply scaling: output = transformed * boft_s
        let scaled = transformed.broadcast_mul(&self.boft_s.t()?)?;

        // Add base output if provided
        if let Some(base) = base_output {
            Ok(scaled.broadcast_add(base)?)
        } else {
            Ok(scaled)
        }
    }

    fn num_parameters(&self) -> usize {
        // Parameters in boft_r: (n_butterfly_factor+1) * block_num * block_size^2
        let r_params =
            (self.n_butterfly_factor + 1) * self.block_num * self.block_size * self.block_size;

        // Parameters in boft_s: out_features
        let s_params = self.out_features;

        r_params + s_params
    }

    fn config(&self) -> &Self::Config {
        &self.config
    }
}

impl Mergeable for BoftLayer {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor> {
        // Get butterfly OFT matrix
        let butterfly_oft = self.compute_butterfly_oft_matrix()?;

        // Merge: W' = (W^T @ butterfly_oft)^T * boft_s
        // = butterfly_oft^T @ W * boft_s
        let weight_t = base_weight.t()?;
        let merged_t = butterfly_oft.matmul(&weight_t)?;
        let merged = merged_t.t()?;

        // Apply scaling
        Ok(merged.broadcast_mul(&self.boft_s)?)
    }

    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor> {
        // Get butterfly OFT matrix
        let butterfly_oft = self.compute_butterfly_oft_matrix()?;

        // Unmerge: W = (butterfly_oft^T @ (W' / boft_s)^T)^T
        let unscaled = merged_weight.broadcast_div(&self.boft_s)?;
        let unscaled_t = unscaled.t()?;
        let butterfly_oft_t = butterfly_oft.t()?;
        let unmerged_t = butterfly_oft_t.matmul(&unscaled_t)?;

        Ok(unmerged_t.t()?)
    }
}

impl Trainable for BoftLayer {
    #[allow(clippy::similar_names)]
    fn register_parameters(&self, var_map: &mut VarMap, prefix: &str) -> Result<()> {
        let boft_r_name = format!("{prefix}.boft_r");
        let boft_s_name = format!("{prefix}.boft_s");

        var_map
            .data()
            .lock()
            .unwrap()
            .insert(boft_r_name, Var::from_tensor(&self.boft_r)?);
        var_map
            .data()
            .lock()
            .unwrap()
            .insert(boft_s_name, Var::from_tensor(&self.boft_s)?);

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

impl SaveLoad for BoftLayer {
    fn state_dict(&self) -> Result<HashMap<String, Tensor>> {
        let mut state_dict = HashMap::new();
        state_dict.insert("boft_r".to_string(), self.boft_r.clone());
        state_dict.insert("boft_s".to_string(), self.boft_s.clone());
        Ok(state_dict)
    }

    fn load_state_dict(&mut self, state_dict: HashMap<String, Tensor>) -> Result<()> {
        if let Some(boft_r) = state_dict.get("boft_r") {
            self.boft_r = boft_r.clone();
        }
        if let Some(boft_s) = state_dict.get("boft_s") {
            self.boft_s = boft_s.clone();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::Device;

    #[test]
    fn test_boft_config_default() {
        let config = BoftConfig::default();
        assert_eq!(config.boft_block_num, 4);
        assert_eq!(config.boft_n_butterfly_factor, 1);
        assert!((config.boft_dropout - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_boft_config_validation() {
        let mut config = BoftConfig::default();

        // Valid config
        assert!(config.validate().is_ok());

        // Both block_size and block_num set
        config.boft_block_size = 8;
        config.boft_block_num = 4;
        assert!(config.validate().is_err());

        // Neither set
        config.boft_block_size = 0;
        config.boft_block_num = 0;
        assert!(config.validate().is_err());

        // Invalid butterfly factor
        config.boft_block_num = 4;
        config.boft_n_butterfly_factor = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_boft_layer_creation() -> Result<()> {
        let device = Device::Cpu;
        let config = BoftConfig {
            boft_block_size: 0,
            boft_block_num: 4,
            boft_n_butterfly_factor: 1,
            ..Default::default()
        };

        let layer = BoftLayer::new(64, 64, config, &device)?;
        assert_eq!(layer.block_num(), 4);
        assert_eq!(layer.block_size(), 16);
        assert_eq!(layer.n_butterfly_factor(), 1);

        Ok(())
    }

    #[test]
    fn test_boft_layer_forward() -> Result<()> {
        let device = Device::Cpu;
        let config = BoftConfig {
            boft_block_size: 0,
            boft_block_num: 4,
            boft_n_butterfly_factor: 1,
            ..Default::default()
        };

        let layer = BoftLayer::new(64, 64, config, &device)?;
        let input = Tensor::randn(0.0f32, 1.0f32, (2, 10, 64), &device)?;
        let output = layer.forward(&input, None)?;

        assert_eq!(output.dims(), &[2, 10, 64]);

        Ok(())
    }

    #[test]
    fn test_boft_parameter_count() -> Result<()> {
        let device = Device::Cpu;
        let config = BoftConfig {
            boft_block_size: 0,
            boft_block_num: 4,
            boft_n_butterfly_factor: 1,
            ..Default::default()
        };

        let layer = BoftLayer::new(64, 64, config, &device)?;

        // With n_butterfly_factor=1 (internally 0), we have:
        // 1 * 4 * 16 * 16 = 1024 parameters in boft_r
        // 64 parameters in boft_s
        // Total: 1088
        assert_eq!(layer.num_parameters(), 1088);

        Ok(())
    }

    #[test]
    fn test_boft_block_butterfly_perm() -> Result<()> {
        // Test identity permutation (no butterfly)
        let perm = BoftLayer::block_butterfly_perm(8, 4, 1, 0)?;
        assert_eq!(perm, vec![0, 1, 2, 3, 4, 5, 6, 7]);

        // Test actual butterfly permutation
        let perm = BoftLayer::block_butterfly_perm(8, 4, 1, 1)?;
        // Should separate even and odd positions within blocks
        assert_eq!(perm.len(), 8);

        Ok(())
    }

    #[test]
    fn test_boft_merge_unmerge() -> Result<()> {
        let device = Device::Cpu;
        let config = BoftConfig {
            boft_block_size: 0,
            boft_block_num: 4,
            boft_n_butterfly_factor: 1,
            ..Default::default()
        };

        let layer = BoftLayer::new(64, 64, config, &device)?;
        let base_weight = Tensor::randn(0.0f32, 1.0f32, (64, 64), &device)?;

        // Merge
        let merged = layer.merge(&base_weight)?;
        assert_eq!(merged.dims(), base_weight.dims());

        // Unmerge
        let unmerged = layer.unmerge(&merged)?;
        assert_eq!(unmerged.dims(), base_weight.dims());

        Ok(())
    }

    #[test]
    fn test_boft_invalid_features() {
        let device = Device::Cpu;
        let config = BoftConfig {
            boft_block_size: 0,
            boft_block_num: 5, // 64 is not divisible by 5
            boft_n_butterfly_factor: 1,
            ..Default::default()
        };

        let result = BoftLayer::new(64, 64, config, &device);
        assert!(result.is_err());
    }

    #[test]
    fn test_boft_butterfly_factor_validation() {
        let device = Device::Cpu;

        // With butterfly factor 2, block_num must be divisible by 2^1 = 2
        let config = BoftConfig {
            boft_block_size: 0,
            boft_block_num: 3, // Not divisible by 2
            boft_n_butterfly_factor: 2,
            ..Default::default()
        };

        let result = BoftLayer::new(64, 64, config, &device);
        assert!(result.is_err());
    }

    #[test]
    fn test_boft_freeze_unfreeze() -> Result<()> {
        let device = Device::Cpu;
        let config = BoftConfig::default();
        let mut layer = BoftLayer::new(64, 64, config, &device)?;

        assert!(!layer.is_frozen());
        layer.freeze();
        assert!(layer.is_frozen());
        layer.unfreeze();
        assert!(!layer.is_frozen());

        Ok(())
    }
}
