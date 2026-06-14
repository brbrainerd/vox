# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [1.0.3] - 2026-01-28

### Changed
- Updated safetensors dependency from 0.4 to 0.7
- Updated candle-core and candle-nn dependencies from 0.8 to 0.9
- Added explicit workspace.dependencies section in Cargo.toml

### Fixed
- Clippy lints for `manual_is_multiple_of` in boft.rs, lokr.rs, oft.rs
- Clippy lint for `manual_midpoint` in training.rs

## [1.0.2] - 2026-01-25

### Changed
- Migrated LoRA/DoRA GPU kernels to CubeCL 0.9 API
- Kernel position variables now use correct types
- Added proper usize casts at array index sites
- `sync_cube()` replaces deprecated `sync_units()`
- Wrapped kernel launches in unsafe blocks with SAFETY comments

## [1.0.1] - 2026-01-24

### Added
- CPU fallback warning when CUDA is unavailable

### Changed
- Bumped minimum Rust version to 1.92
- README license link formatting fix

## [1.0.0] - 2026-01-24

### Added
- **SaveLoad trait implementations for all 11 adapters** - Complete persistence support
  - LoraLayer, DoraLayer (lora.rs)
  - AdaLoraLayer (adalora.rs)
  - Ia3Layer (ia3.rs)
  - LoHaLayer (loha.rs)
  - LoKrLayer (lokr.rs)
  - OftLayer (oft.rs)
  - BoftLayer (boft.rs) - already existed
  - VeraLayer (vera.rs)
  - PrefixTuningLayer (prefix_tuning.rs)
  - PromptTuningLayer (prompt_tuning.rs)
- **Examples directory** with 3 runnable examples:
  - `basic_lora.rs` - Simple LoRA adapter usage
  - `multi_adapter.rs` - Using AdapterRegistry with multiple adapters
  - `save_load.rs` - Persisting and loading adapter weights
- `weights()` method for LoRA adapters (from 0.4.1)
- rsLoRA and LoftQ support for 100% Python PEFT parity
- CLAUDE.md for Claude Code development workflow

### Fixed
- Documentation link warnings in adalora.rs and vera.rs
- Clippy warning for inline format args in lora.rs (Rust 1.92 compatibility)

### Changed
- Bumped to stable 1.0.0 release
- All 128 tests passing
- Full clippy compliance with pedantic lints

## [0.4.1] - 2026-01-16

### Added
- `weights()` method and individual weight accessor methods for LoRA

### Fixed
- Clippy lint fixes for Rust 1.92 compatibility
## [0.4.0] - 2026-01-09

### Added
- Development plan and comprehensive roadmap (DEVELOPMENT_PLAN.md)
- CHANGELOG.md for tracking version history
- Cargo-audit integration for CVE checking
- CI/CD pipeline (.github/workflows/ci.yml)
- Quality check scripts (quality-check.sh, pre-commit.sh, setup-dev.sh)
- Testing branch for proper QA workflow
- PLAN_SUMMARY.md for quick reference

### Changed
- Version strategy documented: strict semantic versioning
- Branch strategy formalized: working → dev → testing → main

### Fixed
- All 207 clippy warnings resolved (documentation, code quality, formatting)
- Added proper error documentation to all Result-returning functions
- Fixed comparison warnings (op_ref)
- Updated format strings to inline syntax

## [0.3.0] - 2026-01-08

### Added
- Model integration system (PeftModel wrapper)
- Training utilities (LR schedules, training state)
- Multi-adapter registry with runtime switching
- Enhanced I/O with HuggingFace PEFT format compatibility
- BOFT (Butterfly Orthogonal Fine-Tuning) adapter
- VeRA (Vector-based Random Matrix Adaptation) adapter
- Comprehensive test suite (124 tests)

### Changed
- Improved model.rs with pattern matching
- Enhanced training.rs with multiple LR schedules

## [0.2.0] - 2026-01-07

### Added
- LoHa (Low-Rank Hadamard Product) adapter
- LoKr (Low-Rank Kronecker Product) adapter
- OFT (Orthogonal Fine-Tuning) adapter
- Prefix Tuning adapter
- Prompt Tuning adapter

## [0.1.0] - 2026-01-06

### Added
- Initial implementation of LoRA adapter
- DoRA (Weight-Decomposed Low-Rank Adaptation) support
- AdaLoRA (Adaptive Low-Rank Adaptation) with SVD
- IA³ (Infused Adapter by Inhibiting and Amplifying) adapter
- Core trait system (Adapter, Mergeable, Trainable, AdapterConfig)
- Error handling (PeftError enum)
- Basic I/O with safetensors
- Configuration system
- Initial documentation (README, AGENT_GUIDE, GAP_ANALYSIS, TASK_TRACKER)

[Unreleased]: https://github.com/tzervas/peft-rs/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/tzervas/peft-rs/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/tzervas/peft-rs/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/tzervas/peft-rs/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/tzervas/peft-rs/releases/tag/v0.1.0
