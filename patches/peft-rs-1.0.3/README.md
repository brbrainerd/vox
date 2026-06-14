# peft-rs

Comprehensive PEFT (Parameter-Efficient Fine-Tuning) adapter library for Rust.

[![Crates.io](https://img.shields.io/crates/v/peft-rs.svg)](https://crates.io/crates/peft-rs)
[![Documentation](https://docs.rs/peft-rs/badge.svg)](https://docs.rs/peft-rs)
[![License](https://img.shields.io/crates/l/peft-rs.svg)](LICENSE-MIT)

## Overview

`peft-rs` provides modular implementations of various PEFT methods for fine-tuning large language models efficiently:

- **LoRA** (Low-Rank Adaptation) - Decomposes weight updates into low-rank matrices
- **DoRA** (Weight-Decomposed Low-Rank Adaptation) - Magnitude and direction decomposition
- **AdaLoRA** (Adaptive Low-Rank Adaptation) - Dynamic rank allocation with SVD parameterization
- **IA³** (Infused Adapter by Inhibiting and Amplifying) - Learned rescaling vectors
- **LoHa** (Low-Rank Hadamard Product) - Hadamard product of two low-rank matrices
- **LoKr** (Low-Rank Kronecker Product) - Kronecker product decomposition
- **OFT** (Orthogonal Fine-Tuning) - Block-diagonal orthogonal transformations
- **BOFT** (Butterfly Orthogonal Fine-Tuning) - Butterfly factorization for efficient orthogonal transforms
- **VeRA** (Vector-based Random Matrix Adaptation) - Ultra-efficient with frozen random matrices
- **Prefix Tuning** - Prepends trainable vectors to attention keys/values
- **Prompt Tuning** - Adds learnable soft prompt embeddings

## Features

- 🦀 Pure Rust implementation using [candle](https://github.com/huggingface/candle)
- 🔌 Modular adapter design with common traits
- 📦 Easy integration with existing models
- ⚡ Optional CUDA acceleration
- 📊 Minimal memory overhead

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
peft-rs = "0.1"
```

For CUDA support:

```toml
[dependencies]
peft-rs = { version = "0.1", features = ["cuda"] }
```

## Quick Start

### LoRA Example

```rust
use peft_rs::{LoraConfig, LoraLayer};
use candle_core::{Device, Tensor, DType};

fn main() -> anyhow::Result<()> {
    let device = Device::Cpu;
    
    // Configure LoRA
    let config = LoraConfig {
        r: 8,           // Rank
        alpha: 16,      // Scaling factor
        dropout: 0.0,
        ..Default::default()
    };
    
    // Create LoRA layer for a 768-dim linear layer
    let lora = LoraLayer::new_with_zeros(768, 768, config, &device)?;
    
    // Forward pass
    let input = Tensor::randn(0.0, 1.0, (1, 10, 768), &device)?;
    let base_output = Tensor::zeros(&[1, 10, 768], DType::F32, &device)?;
    let output = lora.forward(&input, Some(&base_output))?;
    
    println!("Output shape: {:?}", output.shape());
    println!("Trainable parameters: {}", lora.num_parameters());
    
    Ok(())
}
```

### Prompt Tuning Example

```rust
use peft_rs::{PromptTuningConfig, PromptTuningLayer};
use candle_core::{Device, Tensor, DType};

fn main() -> anyhow::Result<()> {
    let device = Device::Cpu;
    
    let config = PromptTuningConfig {
        num_virtual_tokens: 20,
        hidden_size: 768,
        ..Default::default()
    };
    
    let prompt_tuning = PromptTuningLayer::new(config, &device)?;
    
    // Prepend soft prompts to input embeddings
    let input_embeds = Tensor::zeros(&[2, 100, 768], DType::F32, &device)?;
    let output = prompt_tuning.prepend_to_input(&input_embeds)?;
    
    // Output: [2, 120, 768] (20 virtual tokens + 100 input tokens)
    println!("Output shape: {:?}", output.shape());
    
    Ok(())
}
```

## Saving and Loading Adapters

Adapters can be saved and loaded using safetensors format:

```rust
use peft_rs::{LoraLayer, save_adapter_weights, load_adapter_weights, save_adapter_config, load_adapter_config};

// Save adapter weights and config
save_adapter_weights(&lora_layer, "adapter_weights.safetensors")?;
save_adapter_config(&config, "adapter_config.json")?;

// Load adapter weights and config
let loaded_config = load_adapter_config("adapter_config.json")?;
let mut loaded_layer = LoraLayer::new_with_zeros(768, 768, loaded_config, &device)?;
load_adapter_weights(&mut loaded_layer, "adapter_weights.safetensors", &device)?;
```

## Multi-Adapter Support

Manage multiple adapters and switch between them at runtime:

```rust
use peft_rs::{AdapterRegistry, LoraLayer, LoraConfig};

// Create registry
let mut registry = AdapterRegistry::new();

// Register multiple adapters
let task1_adapter = LoraLayer::new_with_zeros(768, 768, config1, &device)?;
let task2_adapter = LoraLayer::new_with_zeros(768, 768, config2, &device)?;

registry.register_adapter("task1", task1_adapter)?;
registry.register_adapter("task2", task2_adapter)?;

// Switch between adapters
registry.set_active_adapter("task1")?;
let output1 = registry.forward(&input, None)?;

registry.set_active_adapter("task2")?;
let output2 = registry.forward(&input, None)?;

// Access specific adapters
let task1 = registry.get_adapter("task1")?;
```

## Architecture

All adapters implement common traits for consistent usage:

```rust
pub trait Adapter {
    type Config: AdapterConfig;
    
    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor>;
    fn num_parameters(&self) -> usize;
    fn config(&self) -> &Self::Config;
}

pub trait Mergeable: Adapter {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor>;
    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor>;
}
```

## Comparison with Python PEFT

| Feature | peft-rs | HuggingFace PEFT |
|---------|---------|------------------|
| LoRA | ✅ | ✅ |
| DoRA | ✅ | ✅ |
| AdaLoRA | ✅ | ✅ |
| IA³ | ✅ | ✅ |
| LoHa | ✅ | ✅ |
| LoKr | ✅ | ✅ |
| OFT | ✅ | ✅ |
| BOFT | ✅ | ✅ |
| VeRA | ✅ | ✅ |
| Prefix Tuning | ✅ | ✅ |
| Prompt Tuning | ✅ | ✅ |
| Weight merging | ✅ | ✅ |
| Weight saving/loading | ✅ | ✅ |
| Multi-adapter support | ✅ | ✅ |
| CUDA support | ✅ | ✅ |
| No Python runtime | ✅ | ❌ |

## Contributing

Contributions welcome! See [docs/GAP_ANALYSIS.md](docs/GAP_ANALYSIS.md) for planned features and [docs/TASK_TRACKER.md](docs/TASK_TRACKER.md) for implementation status.

## License

MIT Licensed - see [LICENSE-MIT](LICENSE-MIT) for details.
