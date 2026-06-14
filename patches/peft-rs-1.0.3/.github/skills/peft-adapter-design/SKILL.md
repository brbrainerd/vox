---
name: peft-adapter-design
description: Design and implement new PEFT adapter types in peft-rs, following established trait patterns
---

# PEFT Adapter Design Skill

## When to Use

Invoke when the user asks to:
- Add a new adapter type (IA³, AdaLoRA, etc.)
- Modify existing adapter implementations
- Design adapter composition patterns
- Implement adapter serialization/deserialization

## Adapter Architecture

All adapters must implement these traits:

```rust
// Required
pub trait Adapter {
    type Config: AdapterConfig;
    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor>;
    fn num_parameters(&self) -> usize;
    fn config(&self) -> &Self::Config;
}

// For adapters that can merge into base weights
pub trait Mergeable: Adapter {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor>;
    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor>;
}

// For trainable adapters
pub trait Trainable: Adapter {
    fn register_parameters(&self, var_map: &mut VarMap, prefix: &str) -> Result<()>;
    fn freeze(&mut self);
    fn unfreeze(&mut self);
    fn is_frozen(&self) -> bool;
}
```

## Adding a New Adapter

### Step 1: Create Config

```rust
// src/adapters/new_adapter.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewAdapterConfig {
    // Hyperparameters with serde defaults
}

impl AdapterConfig for NewAdapterConfig {
    fn validate(&self) -> Result<()> { /* validation logic */ }
}
```

### Step 2: Implement Adapter

```rust
pub struct NewAdapterLayer {
    // Trainable parameters (Tensor)
    // Config
}

impl NewAdapterLayer {
    pub fn new(config: NewAdapterConfig, device: &Device) -> Result<Self> { }
}

impl Adapter for NewAdapterLayer { /* ... */ }
```

### Step 3: Add Tests

- Unit tests in same file
- Property-based tests for numerical operations
- Shape preservation tests

### Step 4: Export from lib.rs

```rust
pub use adapters::new_adapter::{NewAdapterConfig, NewAdapterLayer};
```

## Key Files

- `src/traits.rs` - Core trait definitions
- `src/adapters/mod.rs` - Adapter module exports
- `src/adapters/lora.rs` - Reference implementation
