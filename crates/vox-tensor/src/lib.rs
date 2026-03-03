//! Native ML Tensor operations for Vox.
//!
//! Wraps the `burn` framework to provide PyTorch-like `Tensor` ergonomics
//! using native Rust cross-platform GPU capabilities (NdArray/WGPU) and autograd.
//!
//! Enable the `gpu` feature to compile the burn-backed tensor and nn modules.
//!
//! The `data` module (tokenizer + JSONL dataloader) is always available
//! so that callers can prepare training data on CPU without a GPU dependency.

/// Pure-Rust tokenizer and JSONL DataLoader — always compiled, no GPU required.
pub mod data;

#[cfg(feature = "gpu")]
pub mod nn;
#[cfg(feature = "gpu")]
pub mod tensor;
#[cfg(feature = "gpu")]
pub mod optim;
#[cfg(feature = "gpu")]
pub mod train;
/// LoRA (Low-Rank Adaptation) — parameter-efficient fine-tuning.
/// Phase 1 of the burn-lora strategy. See `lora::LoraLinear` for usage.
#[cfg(feature = "gpu")]
pub mod lora;

#[cfg(feature = "gpu")]
pub extern crate burn;

#[cfg(feature = "gpu")]
pub use tensor::{ElementType, Tensor, TensorShape};
#[cfg(feature = "gpu")]
pub use nn::{Module, Sequential, cross_entropy_loss};
#[cfg(feature = "gpu")]
pub use lora::{LoraConfig, LoraLinear};
