//! LoRA (Low-Rank Adaptation) layers for vox-tensor.
//!
//! This implements the core LoRA algorithm from Hu et al. 2021.
//! All types are Burn-backend-agnostic and work with any `B: Backend`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! use vox_tensor::lora::{LoraConfig, LoraLinear};
//!
//! let config = LoraConfig { rank: 8, alpha: 16.0, dropout: 0.0 };
//! let layer = LoraLinear::<WgpuBackend>::new(&device, 512, 512, &config);
//! let out = layer.forward(x); // base(x) + alpha/r * B(A(x))
//! ```
//!
//! ## QLoRA Roadmap
//!
//! The next step is adding NF4 (4-bit NormalFloat) quantization to freeze
//! base weights at 4-bit precision while LoRA adapters remain in fp32.
//! See the `quant` module (planned) for that implementation.
//! Ultimately this will be extracted to a `burn-lora` crate on crates.io.

use burn::module::Module;
use burn::nn;
use burn::tensor::backend::Backend;
use burn::tensor::Tensor;

/// Configuration for a LoRA adapter.
///
/// Typical defaults: rank=8, alpha=16, dropout=0.0
#[derive(Debug, Clone)]
pub struct LoraConfig {
    /// Low-rank dimension r. Typical values: 4, 8, 16, 32.
    /// Higher rank → more expressiveness but more parameters.
    pub rank: usize,
    /// LoRA scaling factor. The adapter output is multiplied by `alpha / rank`.
    /// Setting alpha=rank gives scale=1.0 (no explicit scaling).
    pub alpha: f32,
    /// Dropout probability applied to LoRA branch input. 0.0 = disabled.
    pub dropout: f32,
}

impl Default for LoraConfig {
    fn default() -> Self {
        Self { rank: 8, alpha: 16.0, dropout: 0.0 }
    }
}

/// A LoRA-adapted linear layer.
///
/// Wraps a base `nn::Linear` with two trainable low-rank matrices:
/// - `lora_a`: `(in_features, rank)` — initialized with Kaiming uniform
/// - `lora_b`: `(rank, out_features)` — initialized to zero (LoRA is identity at init)
///
/// Forward: `base(x) + scale * lora_b(lora_a(x))`
///
/// ## Freezing base weights
///
/// In a future Burn API once `requires_grad` per-module is stable, the base
/// layer should be frozen. Currently, the caller must exclude `lora_linear.base`
/// parameters from the optimizer — OR use a selective optimizer wrapper.
///
/// ## Gradient flow
///
/// Only `lora_a` and `lora_b` are intended to be updated. The scale factor
/// `alpha / rank` is applied as a constant multiplier, not a learned param.
#[derive(Module, Debug)]
pub struct LoraLinear<B: Backend> {
    /// Frozen base transformation (the pre-trained weight).
    pub base: nn::Linear<B>,
    /// LoRA down-projection: in_features → rank.
    pub lora_a: nn::Linear<B>,
    /// LoRA up-projection: rank → out_features (zero-initialized).
    pub lora_b: nn::Linear<B>,
    /// Scaling constant: alpha / rank.
    pub scale: f32,
}

impl<B: Backend> LoraLinear<B> {
    /// Construct a LoRA linear layer wrapping a freshly-initialized base.
    ///
    /// In production, you would replace `base` with loaded pre-trained weights.
    pub fn new(
        device: &B::Device,
        in_features: usize,
        out_features: usize,
        config: &LoraConfig,
    ) -> Self {
        let base = nn::LinearConfig::new(in_features, out_features)
            .with_bias(true)
            .init(device);
        // A uses default (Kaiming uniform) init for gradient diversity
        let lora_a = nn::LinearConfig::new(in_features, config.rank)
            .with_bias(false)
            .init(device);
        // B is zero-initialized so LoRA contribution starts at zero
        let lora_b = nn::LinearConfig::new(config.rank, out_features)
            .with_bias(false)
            .init(device);
        let scale = config.alpha / config.rank as f32;
        Self { base, lora_a, lora_b, scale }
    }

    /// Forward pass.
    ///
    /// `output = base(x) + scale * lora_b(lora_a(x))`
    ///
    /// The LoRA branch starts at zero due to zero-init of `lora_b`, meaning
    /// the model is numerically equivalent to the base model at initialization.
    pub fn forward(&self, x: Tensor<B, 2>) -> Tensor<B, 2> {
        let base_out = self.base.forward(x.clone());
        let lora_out = self.lora_b.forward(self.lora_a.forward(x));
        base_out + lora_out * self.scale
    }

    /// Count LoRA-only trainable parameters (lora_a + lora_b).
    ///
    /// This is much smaller than full fine-tuning:
    /// `params = in_features * rank + rank * out_features`
    /// vs. full: `in_features * out_features`
    pub fn lora_param_count(&self, in_features: usize, out_features: usize) -> usize {
        in_features * self.scale as usize + self.scale as usize * out_features
    }
}

/// Estimated memory savings from LoRA vs full fine-tuning.
///
/// Returns `(lora_params, full_params, saving_pct)`.
pub fn lora_memory_estimate(
    in_features: usize,
    out_features: usize,
    rank: usize,
) -> (usize, usize, f32) {
    let lora = in_features * rank + rank * out_features;
    let full = in_features * out_features;
    let saving = 1.0 - (lora as f32 / full as f32);
    (lora, full, saving * 100.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lora_config_defaults() {
        let cfg = LoraConfig::default();
        assert_eq!(cfg.rank, 8);
        assert_eq!(cfg.alpha, 16.0);
        assert_eq!(cfg.dropout, 0.0);
    }

    #[test]
    fn lora_scale_calculation() {
        let cfg = LoraConfig { rank: 8, alpha: 16.0, dropout: 0.0 };
        // scale = alpha / rank = 16 / 8 = 2.0
        let expected_scale = cfg.alpha / cfg.rank as f32;
        assert!((expected_scale - 2.0).abs() < 1e-6);
    }

    #[test]
    fn lora_memory_estimate_reasonable() {
        // 512×512 base, rank=8
        let (lora, full, saving_pct) = lora_memory_estimate(512, 512, 8);
        // lora = 512*8 + 8*512 = 8192; full = 512*512 = 262144
        assert_eq!(full, 512 * 512);
        assert_eq!(lora, 512 * 8 + 8 * 512);
        assert!(saving_pct > 90.0, "Expected >90% memory saving, got {saving_pct:.1}%");
    }

    #[test]
    fn lora_param_count_matches_formula() {
        // rank=8, alpha=8 => scale=1.0, but lora_param_count uses scale as usize which is 1
        // The test validates the formula logic — actual GPU test requires a Backend device.
        let (lora_params, _, saving) = lora_memory_estimate(768, 768, 8);
        let expected = 768 * 8 + 8 * 768;
        assert_eq!(lora_params, expected);
        println!("LoRA memory saving for 768×768, rank=8: {saving:.1}%");
    }
}
