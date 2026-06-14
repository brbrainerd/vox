# Agent Guide for peft-rs Development

> **Quick Start for AI Agents**: This document provides a navigation map and quick reference for developing in the peft-rs repository.

## 📑 Documentation Index

### Primary Documentation
| Document | Purpose | Location |
|----------|---------|----------|
| **TASK_TRACKER.md** | Implementation status, roadmap, and task breakdown | [`docs/TASK_TRACKER.md`](TASK_TRACKER.md) |
| **GAP_ANALYSIS.md** | Gap analysis vs HuggingFace Python PEFT | [`docs/GAP_ANALYSIS.md`](GAP_ANALYSIS.md) |
| **AGENT_GUIDE.md** | This file - navigation and quick reference | [`docs/AGENT_GUIDE.md`](AGENT_GUIDE.md) |
| **README.md** | User-facing documentation and examples | [`README.md`](../README.md) |

### Skill Documentation
| Skill | Purpose | Location |
|-------|---------|----------|
| **peft-adapter-design** | Adding/modifying adapters | [`.github/skills/peft-adapter-design/SKILL.md`](../.github/skills/peft-adapter-design/SKILL.md) |

## 🏗️ Project Structure

```
peft-rs/
├── src/
│   ├── adapters/          # All adapter implementations
│   │   ├── lora.rs        # LoRA & DoRA (reference implementation)
│   │   ├── adalora.rs     # Adaptive LoRA with SVD
│   │   ├── ia3.rs         # Learned rescaling vectors
│   │   ├── loha.rs        # Low-Rank Hadamard Product
│   │   ├── lokr.rs        # Low-Rank Kronecker Product
│   │   ├── oft.rs         # Orthogonal Fine-Tuning
│   │   ├── boft.rs        # Butterfly Orthogonal Fine-Tuning
│   │   ├── vera.rs        # Vector-based Random Matrix Adaptation
│   │   ├── prefix_tuning.rs
│   │   ├── prompt_tuning.rs
│   │   └── mod.rs         # Adapter module exports
│   ├── traits.rs          # Core trait definitions (Adapter, Mergeable, Trainable)
│   ├── config.rs          # Common config utilities
│   ├── error.rs           # Error types (PeftError)
│   ├── io.rs              # Weight loading/saving (safetensors)
│   ├── registry.rs        # Multi-adapter registry
│   └── lib.rs             # Public API exports
├── docs/                  # Documentation
├── benches/               # Performance benchmarks
└── tests/                 # Integration tests (if any)
```

## 🎯 Current Status (as of latest dev merge)

### Completed Adapters (11 total)
- ✅ LoRA (5 tests)
- ✅ DoRA (5 tests) 
- ✅ AdaLoRA (7 tests)
- ✅ IA³ (8 tests)
- ✅ LoHa (9 tests)
- ✅ LoKr (10 tests)
- ✅ OFT (14 tests)
- ✅ BOFT (10 tests) - **Recently completed**
- ✅ VeRA (10 tests)
- ✅ Prefix Tuning (2 tests)
- ✅ Prompt Tuning (3 tests)

**Total: 100 tests passing**

### Next Priorities (from TASK_TRACKER.md)
1. **Weight Loading/Saving Enhancement** (Priority: High, Phase 2)
2. **Model Integration System** (Priority: High, Phase 2)
3. **Multi-Adapter Support** (Priority: Medium, Phase 2)
4. **Quantization Support** (Priority: Low, Phase 3)

## 🔧 Common Development Tasks

### Running Tests
```bash
# Run all tests
cargo test

# Run tests quietly (summary only)
cargo test --quiet

# Run specific adapter tests
cargo test loha

# Run with output
cargo test -- --nocapture
```

### Building
```bash
# Standard build
cargo build

# Release build
cargo build --release

# With CUDA support
cargo build --features cuda
```

### Linting
```bash
# Check code
cargo check

# Run clippy
cargo clippy

# Format code
cargo fmt

# Check formatting without changing
cargo fmt -- --check
```

### Adding a New Adapter

**See**: [`.github/skills/peft-adapter-design/SKILL.md`](../.github/skills/peft-adapter-design/SKILL.md)

**Quick Steps**:
1. Create `src/adapters/new_adapter.rs`
2. Implement `NewAdapterConfig` with `AdapterConfig` trait
3. Implement `NewAdapterLayer` with `Adapter` trait
4. Optionally implement `Mergeable` and `Trainable` traits
5. Add comprehensive tests (aim for 10+ tests)
6. Export from `src/adapters/mod.rs`
7. Add public exports to `src/lib.rs`
8. Update `README.md` with usage example
9. Update `docs/TASK_TRACKER.md` to mark as complete

### Test Categories for Adapters
Each adapter should have tests for:
1. **Configuration** - Defaults, validation, serialization
2. **Creation** - Layer instantiation with various configs
3. **Forward pass** - Output shape correctness
4. **Parameter counting** - Verify trainable parameter count
5. **Merge/unmerge** - For Mergeable adapters
6. **Edge cases** - Boundary conditions, error handling
7. **Numerical correctness** - Basic mathematical properties

### Reference Implementation
**Use `src/adapters/lora.rs` as the canonical example** for:
- Code structure and organization
- Documentation style
- Test coverage patterns
- Error handling

## 📚 Key Trait Patterns

### Adapter (Required)
```rust
pub trait Adapter {
    type Config: AdapterConfig;
    fn forward(&self, input: &Tensor, base_output: Option<&Tensor>) -> Result<Tensor>;
    fn num_parameters(&self) -> usize;
    fn config(&self) -> &Self::Config;
}
```

### Mergeable (For weight-mergeable adapters)
```rust
pub trait Mergeable: Adapter {
    fn merge(&self, base_weight: &Tensor) -> Result<Tensor>;
    fn unmerge(&self, merged_weight: &Tensor) -> Result<Tensor>;
}
```

### Trainable (For parameter registration)
```rust
pub trait Trainable: Adapter {
    fn register_parameters(&self, var_map: &mut VarMap, prefix: &str) -> Result<()>;
    fn freeze(&mut self);
    fn unfreeze(&mut self);
    fn is_frozen(&self) -> bool;
}
```

## 🔍 Finding Information

### "Where do I find...?"

| What | Where |
|------|-------|
| Implementation status | `docs/TASK_TRACKER.md` |
| Gaps vs Python PEFT | `docs/GAP_ANALYSIS.md` |
| Adapter examples | `src/adapters/lora.rs` (best reference) |
| Core traits | `src/traits.rs` |
| Error types | `src/error.rs` |
| Public API | `src/lib.rs` |
| Usage examples | `README.md` |
| Test patterns | Any `src/adapters/*.rs` file (bottom of file) |
| Benchmarks | `benches/` directory |

### "How do I...?"

| Task | Command/Location |
|------|------------------|
| Run all tests | `cargo test` |
| Check code compiles | `cargo check` |
| Format code | `cargo fmt` |
| Run linter | `cargo clippy` |
| Build with CUDA | `cargo build --features cuda` |
| Add a dependency | Edit `Cargo.toml` |
| See current version | Check `Cargo.toml` (currently 0.3.0) |

## 🚀 Workflow for New Features

1. **Update from dev**: `git fetch origin && git checkout dev && git pull origin dev`
2. **Create feature branch**: `git checkout -b copilot/descriptive-name`
3. **Make changes**: Implement feature with tests
4. **Run tests**: `cargo test` (all must pass)
5. **Format**: `cargo fmt`
6. **Lint**: `cargo clippy` (fix warnings)
7. **Commit & push**: Use `report_progress` tool
8. **Create PR**: Target `dev` branch
9. **Mark ready**: Set PR as "ready for review"

## ⚠️ Important Notes

### Dependencies Already Included
- `candle-core` 0.9 - Tensor operations
- `candle-nn` 0.9 - Neural network layers
- `serde` 1.0 - Serialization
- `serde_json` 1.0 - JSON support
- `safetensors` 0.7 - Weight persistence
- `thiserror` 2.0 - Error handling
- `tracing` 0.1 - Logging

### Version Information
- **Current Version**: 0.3.0
- **Rust Version**: 1.75+
- **Edition**: 2021

### Code Style
- Use `cargo fmt` for formatting
- Address `cargo clippy` warnings
- Follow patterns from `lora.rs`
- Document public items
- Include comprehensive tests

## 📖 External References

### Papers
- [LoRA](https://arxiv.org/abs/2106.09685)
- [DoRA](https://arxiv.org/abs/2402.09353)
- [AdaLoRA](https://arxiv.org/abs/2303.10512)
- [IA³](https://arxiv.org/abs/2205.05638)
- [OFT](https://arxiv.org/abs/2306.07280)
- [BOFT](https://arxiv.org/abs/2311.06243)
- [VeRA](https://arxiv.org/abs/2310.11454)
- [Prefix Tuning](https://arxiv.org/abs/2101.00190)
- [Prompt Tuning](https://arxiv.org/abs/2104.08691)

### Repositories
- [HuggingFace PEFT](https://github.com/huggingface/peft) - Python reference implementation
- [LyCORIS](https://github.com/KohakuBlueleaf/LyCORIS) - LoHa, LoKr implementations
- [Candle](https://github.com/huggingface/candle) - Rust ML framework

---

*This guide is maintained as the central navigation hub for AI agents working on peft-rs. Keep it updated as the project evolves.*

*Last Updated: 2026-01-09*
