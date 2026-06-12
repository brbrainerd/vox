//! # qlora-rs
//!
//! 4-bit quantized `LoRA` (`QLoRA`) implementation for Rust.
//!
//! This crate provides:
//! - NF4 (4-bit `NormalFloat`) quantization
//! - Double quantization for memory efficiency
//! - `QLoRA` training with frozen quantized base weights
//! - GGUF model export for inference deployment
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use qlora_rs::{QLoraConfig, QuantizedLinear, quantize_nf4};
//! use candle_core::Device;
//!
//! // Quantize a weight tensor to 4-bit
//! let quantized = quantize_nf4(&weights, 64)?;
//!
//! // Create QLoRA layer
//! let config = QLoraConfig::default();
//! let layer = QuantizedLinear::new(768, 768, config, &Device::Cpu)?;
//! ```
//!
//! ## Architecture
//!
//! `QLoRA` keeps base model weights frozen in 4-bit precision while training
//! `LoRA` adapters in full precision. This enables fine-tuning large models
//! on consumer hardware.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]

pub mod error;
pub mod export;
pub mod formats;
#[cfg(feature = "cuda")]
pub mod kernels;
pub mod native;
pub mod qlora;
pub mod quantization;
pub mod training;

pub use error::{QLoraError, Result};
pub use formats::{export_model, export_native_format, ExportConfig, ExportFormat};
pub use qlora::{QLoraConfig, QLoraLayer, QuantizedLinear};
pub use quantization::{
    dequantize_nf4, dequantize_nf4_with_dtype, pad_for_quantization,
    pad_for_quantization_with_info, quantize_nf4, unpad_tensor, ComputeDType, PaddingInfo,
    QuantizationConfig, QuantizationStrategy, QuantizedTensor,
};
pub use training::{
    accumulate_grads_for_vars, backward_from_cotangent, cross_entropy_loss, PagedAdamW,
    PagedAdamWState, QLoraTrainer, QLoraTrainingConfig, TrainingMetrics,
};
