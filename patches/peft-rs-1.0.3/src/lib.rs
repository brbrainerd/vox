//! # peft-rs
//!
//! Comprehensive PEFT (Parameter-Efficient Fine-Tuning) adapter library for Rust.
//!
//! This crate provides modular implementations of various PEFT methods:
//! - **`LoRA`** (Low-Rank Adaptation)
//! - **`DoRA`** (Weight-Decomposed Low-Rank Adaptation)
//! - **`AdaLoRA`** (Adaptive Low-Rank Adaptation)
//! - **IA³** (Infused Adapter by Inhibiting and Amplifying Inner Activations)
//! - **`LoHa`** (Low-Rank Hadamard Product)
//! - **`LoKr`** (Low-Rank Kronecker Product)
//! - **OFT** (Orthogonal Fine-Tuning)
//! - **BOFT** (Butterfly Orthogonal Fine-Tuning)
//! - **`VeRA`** (Vector-based Random Matrix Adaptation)
//! - **Prefix Tuning**
//! - **Prompt Tuning**
//!
//! ## Quick Start
//!
//! ```rust,ignore
//! use peft_rs::{LoraConfig, LoraLayer};
//! use candle_core::{Device, Tensor};
//!
//! // Create a LoRA layer
//! let config = LoraConfig {
//!     r: 8,
//!     alpha: 16,
//!     dropout: 0.0,
//! };
//! let layer = LoraLayer::new(768, 768, config, &Device::Cpu)?;
//!
//! // Apply to input
//! let input = Tensor::zeros(&[1, 10, 768], candle_core::DType::F32, &Device::Cpu)?;
//! let output = layer.forward(&input)?;
//! ```
//!
//! ## Architecture
//!
//! All adapters implement the [`Adapter`] trait, which provides a common interface
//! for forward passes and weight merging.

#![warn(missing_docs)]
#![warn(clippy::pedantic)]

pub mod adapters;
pub mod config;
pub mod error;
pub mod io;
#[cfg(feature = "cuda")]
pub mod kernels;
pub mod model;
pub mod registry;
pub mod training;
pub mod traits;

pub use adapters::adalora::{AdaLoraConfig, AdaLoraLayer};
pub use adapters::boft::{BoftConfig, BoftLayer};
pub use adapters::ia3::{Ia3Config, Ia3Layer};
pub use adapters::loha::{LoHaConfig, LoHaLayer};
pub use adapters::lokr::{LoKrConfig, LoKrLayer};
pub use adapters::lora::{DoraLayer, LoraConfig, LoraLayer};
pub use adapters::oft::{OftConfig, OftLayer};
pub use adapters::prefix_tuning::{PrefixTuningConfig, PrefixTuningLayer};
pub use adapters::prompt_tuning::{PromptTuningConfig, PromptTuningLayer};
pub use adapters::vera::{VeraConfig, VeraLayer};
pub use error::{PeftError, Result};
pub use io::{
    load_adapter_config, load_adapter_weights, load_pretrained, save_adapter_config,
    save_adapter_weights, save_pretrained, SaveLoad, ADAPTER_CONFIG_FILENAME,
    ADAPTER_WEIGHTS_FILENAME,
};
pub use model::{get_peft_model, ModulePattern, PeftModel};
pub use registry::AdapterRegistry;
pub use training::{
    count_trainable_parameters, format_parameter_count, AdapterTrainingConfig,
    AdapterTrainingState, LrSchedule,
};
pub use traits::{Adapter, AdapterConfig, Mergeable, Trainable};
