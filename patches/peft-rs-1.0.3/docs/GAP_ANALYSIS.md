# PEFT-RS Gap Analysis

This document analyzes the gaps between the Rust `peft-rs` implementation and the official HuggingFace Python PEFT library.

> **See also:** [TASK_TRACKER.md](TASK_TRACKER.md) for implementation status and roadmap.

## Current State of peft-rs

### Implemented Adapters
| Adapter | Status | Notes |
|---------|--------|-------|
| LoRA | ✅ Complete | Core functionality with VarBuilder support |
| DoRA | ✅ Complete | Weight-Decomposed LoRA |
| AdaLoRA | ✅ Complete | SVD-based adaptive rank allocation |
| IA³ | ✅ Complete | Learned rescaling vectors |
| LoHa | ✅ Complete | Low-Rank Hadamard Product |
| LoKr | ✅ Complete | Low-Rank Kronecker Product |
| OFT | ✅ Complete | Orthogonal Fine-Tuning |
| BOFT | ✅ Complete | Butterfly Orthogonal Fine-Tuning |
| VeRA | ✅ Complete | Vector-based Random Matrix Adaptation |
| Prefix Tuning | ✅ Complete | Trainable prefix vectors |
| Prompt Tuning | ✅ Complete | Soft prompt embeddings |

### Core Infrastructure
| Component | Status | Notes |
|-----------|--------|-------|
| `Adapter` trait | ✅ | Forward pass and parameter counting |
| `Mergeable` trait | ✅ | Weight merging/unmerging |
| `Trainable` trait | ✅ | Parameter registration, freeze/unfreeze |
| `AdapterConfig` trait | ✅ | Configuration validation |
| Error handling | ✅ | PeftError enum with variants |

## Gaps Identified

### ~~Priority 1: Missing Core Adapters~~ ✅ COMPLETED

All priority 1 adapters have been implemented:
- ✅ **IA³** - Implemented in `src/adapters/ia3.rs`
- ✅ **AdaLoRA** - Implemented in `src/adapters/adalora.rs`

### ~~Priority 2: LoRA Variants~~ ✅ COMPLETED

- ✅ **DoRA** - Implemented in `src/adapters/lora.rs` (DoraLayer)

#### QLoRA Support
**Python PEFT Reference:** `src/peft/tuners/lora/bnb.py`

Quantized LoRA for 4-bit/8-bit training.

**Key features to implement:**
- Integration with quantization backend
- `Linear4bit` and `Linear8bitLt` layer support
- Dequantization during forward pass

### Priority 3: Additional Adapter Types

#### LoHa (Low-Rank Hadamard Product)
**Python PEFT Reference:** `src/peft/tuners/loha/`

Uses Hadamard product of two low-rank matrices.

**Key features:**
- `ΔW = (A1 ⊗ B1) ⊙ (A2 ⊗ B2)` where ⊙ is Hadamard product
- More expressive than standard LoRA

#### LoKr (Low-Rank Kronecker Product)
**Python PEFT Reference:** `src/peft/tuners/lokr/`

Uses Kronecker product for weight decomposition.

#### OFT (Orthogonal Fine-Tuning)
**Python PEFT Reference:** `src/peft/tuners/oft/`

Applies orthogonal transformations to preserve pretrained knowledge.

#### BOFT (Butterfly Orthogonal Fine-Tuning)
**Python PEFT Reference:** `src/peft/tuners/boft/`

Uses butterfly factorization for efficient orthogonal transformations.

#### VeRA (Vector-based Random Matrix Adaptation)
**Python PEFT Reference:** `src/peft/tuners/vera/`

Uses frozen random matrices with trainable scaling vectors.

### ~~Priority 4: Infrastructure Improvements~~ ✅ COMPLETED

#### ~~Weight Loading/Saving~~ ✅ COMPLETED
Implemented in `src/io.rs`:
- `save_pretrained()` / `load_pretrained()` - HuggingFace PEFT compatible
- `save_adapter_weights()` / `load_adapter_weights()` - safetensors format
- `save_adapter_config()` / `load_adapter_config()` - JSON serialization
- `SaveLoad` trait for adapters

#### ~~Model Integration~~ ✅ COMPLETED
Implemented in `src/model.rs`:
- `get_peft_model()` convenience function
- `PeftModel<A>` wrapper struct
- `ModulePattern` with glob-style matching (`*.attention`, `layer.*`, `*`)
- Per-module adapter management

#### ~~Multi-Adapter Support~~ ✅ COMPLETED
Implemented in `src/registry.rs`:
- `AdapterRegistry<A>` for named adapter management
- `set_active_adapter()` / `get_adapter()` methods
- Adapter switching at runtime

#### ~~Training Utilities~~ ✅ COMPLETED
Implemented in `src/training.rs`:
- `LrSchedule` enum (Constant, LinearWarmup, CosineAnnealing, LinearDecay)
- `AdapterTrainingConfig` / `AdapterTrainingState`
- Gradient accumulation support
- `count_trainable_parameters()` / `format_parameter_count()`

### Priority 5: Advanced Features

#### Quantization Support
- 4-bit quantization (bitsandbytes equivalent)
- 8-bit quantization
- GPTQ/AWQ integration

#### ~~Training Utilities~~ ✅ COMPLETED
- ~~Gradient checkpointing integration~~ (basic support)
- ~~Mixed precision training support~~ (via candle)
- ~~Learning rate scheduling for adapters~~ ✅

#### Evaluation/Inference
- Batch adapter switching
- Merged inference mode
- Export to standard formats

## Recommended Implementation Order

### ~~Phase 1: Core Adapters (High Impact)~~ ✅ COMPLETED
1. ~~**IA³** - Simple, highly efficient, good for benchmarking~~ ✅
2. ~~**AdaLoRA** - Advanced LoRA variant with practical benefits~~ ✅

### ~~Phase 2: LoRA Enhancements~~ ✅ COMPLETED
3. ~~**DoRA** - Popular enhancement to LoRA~~ ✅
4. **Quantization infrastructure** - Essential for large models (PENDING)

### Phase 3: Model Integration (NEXT)
5. **Weight loading/saving** - Required for practical use
6. **Model injection** - User-friendly API

### Phase 4: Additional Methods
7. **LoHa/LoKr** - LyCORIS family adapters
8. **OFT/BOFT** - Orthogonal methods
9. **VeRA** - Ultra-efficient adaptation

## API Design Recommendations

### Consistent Trait Pattern
All adapters should implement:
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

pub trait Trainable: Adapter {
    fn register_parameters(&self, var_map: &mut VarMap, prefix: &str) -> Result<()>;
    fn freeze(&mut self);
    fn unfreeze(&mut self);
    fn is_frozen(&self) -> bool;
}
```

### Configuration Pattern
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterConfig {
    pub target_modules: Vec<String>,
    pub modules_to_save: Vec<String>,
    // ... adapter-specific fields
}
```

### Model Integration Pattern
```rust
pub fn get_peft_model<M: Model>(model: M, config: impl AdapterConfig) -> Result<PeftModel<M>> {
    // Inject adapters into target modules
}
```

## References

- [HuggingFace PEFT Repository](https://github.com/huggingface/peft)
- [LoRA Paper](https://arxiv.org/abs/2106.09685)
- [IA³ Paper](https://arxiv.org/abs/2205.05638)
- [AdaLoRA Paper](https://arxiv.org/abs/2303.10512)
- [DoRA Paper](https://arxiv.org/abs/2402.09353)
- [Prefix Tuning Paper](https://arxiv.org/abs/2101.00190)
- [Prompt Tuning Paper](https://arxiv.org/abs/2104.08691)
