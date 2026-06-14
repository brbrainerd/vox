# PEFT-RS Development Plan
**Created:** 2026-01-09  
**Status:** Approved ✅  
**Version:** 0.3.0 → 1.0.0 (future)

## Executive Summary

This document outlines the comprehensive development plan for peft-rs, a Rust port of HuggingFace's Python PEFT library. The plan establishes strict quality controls, branching strategies, and phased development approach to ensure production-ready code.

## Current Project Status

### ✅ Completed (Phase 1 & 2)
- **11 Adapter Implementations**: LoRA, DoRA, AdaLoRA, IA³, LoHa, LoKr, OFT, BOFT, VeRA, Prefix Tuning, Prompt Tuning
- **Core Infrastructure**: Traits, error handling, config system
- **I/O System**: safetensors, HuggingFace PEFT format compatibility
- **Model Integration**: PeftModel wrapper with pattern matching
- **Multi-Adapter Registry**: Runtime adapter switching
- **Training Utilities**: LR schedules, training state management
- **Test Coverage**: 124 tests passing (100 adapter-specific)
- **Current Version**: 0.3.0

### 🔍 Quality Issues Identified
1. **207 Clippy warnings** - Documentation and code quality issues
2. **No cargo-audit** integration - CVE checking not automated
3. **No CI/CD pipeline** - Manual quality checks
4. **No testing branch** - All work merged directly to dev
5. **Formatting inconsistencies** - Some files need cargo fmt

## Project Goals

1. **Complete feature parity** with HuggingFace Python PEFT (core adapters)
2. **Superior performance** through Rust's zero-cost abstractions
3. **Production-ready quality** with comprehensive testing and tooling
4. **Semantic versioning** starting at 0.1.0, building to 1.0.0 only with explicit authorization
5. **Ecosystem expansion** for Rust ML/AI community

## Branch Strategy

### Main Branches
```
main (production releases only)
  ↑
testing (integration, performance, regression testing)
  ↑
dev (feature integration, daily development)
  ↑
working/* (individual features)
exploratory/* (experimental approaches)
```

### Working Branch Pattern
- Base all working branches off `dev`
- Merge pattern: `working/* → dev → testing → main`
- One active working branch at a time per developer/agent
- Mark PRs as "ready for review" before switching branches

### Testing Branch Flow
```
testing
  ├── working/test-isolation-<module>  # Break tests into helper crate
  ├── working/test-optimization        # Streamline stable components
  ├── working/perf-<feature>          # Performance optimizations
  └── working/regression-suite        # Comprehensive regression tests
```

### Exploratory Branches
For indecisive implementation points:
```
exploratory/feature-X-approach-A
exploratory/feature-X-approach-B
```
Merge winning approach back to dev after evaluation.

## Semantic Versioning Strategy

### Starting Point: 0.3.0 → 0.4.0
**Rules:**
- Start at **0.1.0** for new features after reset (if needed)
- Current: **0.3.0** (11 adapters complete)
- **MAJOR.MINOR.PATCH** format
- No 1.0.0 without explicit authorization
- Accept hundreds of minor/patch versions for accuracy

### Version Increments
- **PATCH (0.3.X)**: Bug fixes, docs, non-breaking changes
- **MINOR (0.X.0)**: New features, adapters, non-breaking additions
- **MAJOR (X.0.0)**: Breaking API changes (requires authorization)

### Milestone Versions
- **0.4.0**: Quality improvements, CI/CD, clippy fixes
- **0.5.0**: Quantization support (QLoRA)
- **0.6.0**: Enhanced inference utilities
- **0.7.0**: Additional adapter types (if any from Python PEFT)
- **0.8.0**: Performance optimizations
- **0.9.0**: Pre-1.0 stabilization
- **1.0.0**: Production-ready (requires explicit authorization)

## Quality Tooling Requirements

### Code Quality Tools
| Tool | Purpose | Integration Point |
|------|---------|-------------------|
| `cargo fmt` | Code formatting | Pre-commit, CI |
| `cargo clippy` | Linting (deny warnings) | Pre-commit, CI |
| `cargo test` | Unit/integration tests | Pre-commit, CI |
| `cargo audit` | CVE/security checking | Daily, CI |
| `cargo bench` | Performance regression | Weekly, pre-release |
| `cargo doc` | Documentation building | CI, pre-release |

### Quality Checks Before Merge
```bash
# Run all checks (must pass)
cargo fmt --check
cargo clippy -- -D warnings
cargo test --all-features
cargo audit
cargo doc --no-deps --document-private-items
```

### CI/CD Pipeline (To Be Implemented)
```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    - fmt check
    - clippy check
    - test all features
    - audit check
  
  coverage:
    - generate coverage report
    - upload to codecov
  
  benchmark:
    - run benchmarks on perf-critical PRs
```

## Development Phases

### Phase 3: Quality & Tooling (CURRENT PRIORITY)
**Target Version:** 0.4.0  
**Duration:** 1-2 weeks  
**Status:** NOT STARTED

#### Tasks
1. **Fix Clippy Warnings** (207 total)
   - [ ] Documentation backticks (majority)
   - [ ] Missing error docs (allow or document)
   - [ ] Type casting safety
   - [ ] Code style improvements

2. **Set Up CI/CD**
   - [ ] Create `.github/workflows/ci.yml`
   - [ ] Add fmt, clippy, test, audit checks
   - [ ] Add coverage reporting
   - [ ] Add benchmark regression detection

3. **Create Testing Branch Infrastructure**
   - [ ] `git checkout -b testing` from dev
   - [ ] Push to origin
   - [ ] Update branch protection rules
   - [ ] Document testing workflow

4. **Security Audit**
   - [ ] Run `cargo audit`
   - [ ] Review dependencies for CVEs
   - [ ] Set up automated daily audits

5. **Documentation Pass**
   - [ ] Ensure all public items documented
   - [ ] Add crate-level docs to lib.rs
   - [ ] Generate and review rustdoc output
   - [ ] Add more examples to README

**Working Branches:**
- `working/quality-clippy-fixes`
- `working/ci-cd-setup`
- `working/testing-branch-init`

---

### Phase 4: Quantization Support (QLoRA)
**Target Version:** 0.5.0  
**Duration:** 2-3 weeks  
**Status:** NOT STARTED

**Reference:** Python PEFT `src/peft/tuners/lora/bnb.py`

#### Tasks
1. **Research & Design**
   - [ ] Investigate Candle quantization capabilities
   - [ ] Review bitsandbytes equivalents in Rust
   - [ ] Design quantization layer API

2. **Implementation**
   - [ ] Create `src/quantization.rs` module
   - [ ] Implement 4-bit quantization layer
   - [ ] Implement 8-bit quantization layer
   - [ ] Implement `QuantizedLoraLayer`
   - [ ] Add mixed precision support

3. **Testing**
   - [ ] Unit tests for quantization ops
   - [ ] Integration tests with LoRA
   - [ ] Memory usage benchmarks
   - [ ] Accuracy validation

**Working Branches:**
- `working/quantization-research`
- `exploratory/quantization-candle` (if multiple approaches)
- `exploratory/quantization-custom` (if multiple approaches)
- `working/quantization-implementation`

---

### Phase 5: Enhanced Inference Utilities
**Target Version:** 0.6.0  
**Duration:** 1-2 weeks  
**Status:** NOT STARTED

#### Tasks
1. **Batch Adapter Switching**
   - [ ] Design API for batch operations
   - [ ] Implement batch adapter loading
   - [ ] Add caching layer
   - [ ] Performance benchmarks

2. **Merged Inference Mode**
   - [ ] Optimize for inference-only scenarios
   - [ ] Pre-merge weights for speed
   - [ ] Memory-efficient inference

3. **Export Utilities**
   - [ ] Export to ONNX (if applicable)
   - [ ] Export to standard formats
   - [ ] Cross-platform compatibility

**Working Branches:**
- `working/inference-batch-switching`
- `working/inference-merged-mode`
- `working/inference-export-utils`

---

### Phase 6: Test Isolation & Optimization
**Target Version:** 0.7.0  
**Duration:** 2-3 weeks  
**Status:** NOT STARTED

**Based off:** `testing` branch

#### Tasks
1. **Test Helper Crate**
   - [ ] Create `peft-rs-test-utils` crate
   - [ ] Move common test utilities
   - [ ] Reduce code duplication

2. **Test Optimization**
   - [ ] Identify stable, unchanging components
   - [ ] Archive or remove redundant tests
   - [ ] Focus on regression-critical areas
   - [ ] Add integration test suite

3. **Performance Testing**
   - [ ] Comprehensive benchmark suite
   - [ ] Memory profiling
   - [ ] Regression detection

**Working Branches (from testing):**
- `working/test-isolation-adapters`
- `working/test-isolation-core`
- `working/test-optimization`

---

### Phase 7: Performance Optimizations
**Target Version:** 0.8.0  
**Duration:** 2-3 weeks  
**Status:** NOT STARTED

**Based off:** `testing` branch (after Phase 6)

#### Tasks
1. **Profiling**
   - [ ] CPU profiling with perf/flamegraph
   - [ ] Memory profiling
   - [ ] Identify bottlenecks

2. **Optimizations**
   - [ ] SIMD operations where applicable
   - [ ] Reduce allocations
   - [ ] Optimize hot paths
   - [ ] Parallel operations

3. **CUDA Optimizations**
   - [ ] GPU kernel optimizations
   - [ ] Async operations
   - [ ] Batch processing

4. **Regression Testing**
   - [ ] Ensure no functionality regression
   - [ ] Benchmark comparisons
   - [ ] Accuracy validation

**Working Branches (from testing):**
- `working/perf-profiling`
- `working/perf-simd`
- `working/perf-cuda`
- `working/perf-memory`

---

### Phase 8: Additional Adapters (If Any)
**Target Version:** 0.9.0  
**Duration:** Variable  
**Status:** NOT STARTED

**Depends on:** Python PEFT updates

Monitor HuggingFace PEFT repository for new adapter types:
- [ ] Review Python PEFT changelog
- [ ] Identify new adapter types
- [ ] Prioritize by usage/demand
- [ ] Implement with full test coverage

---

### Phase 9: Pre-1.0 Stabilization
**Target Version:** 0.9.x → 1.0.0  
**Duration:** 1-2 months  
**Status:** NOT STARTED

**Requirements for 1.0.0 (requires authorization):**
1. [ ] All clippy warnings resolved
2. [ ] Zero security vulnerabilities
3. [ ] 90%+ test coverage
4. [ ] Complete documentation
5. [ ] Production usage validation
6. [ ] Performance benchmarks published
7. [ ] API stability guarantee
8. [ ] Migration guide from Python PEFT
9. [ ] Community feedback incorporated
10. [ ] Explicit authorization from maintainer

**Working Branches:**
- `working/stabilization-api`
- `working/stabilization-docs`
- `working/stabilization-examples`

---

## Development Workflow

### Daily Workflow
1. **Pull latest dev**: `git checkout dev && git pull origin dev`
2. **Create/switch to working branch**: `git checkout -b working/feature-name` or `git checkout working/feature-name`
3. **Implement feature** with tests
4. **Run quality checks**:
   ```bash
   cargo fmt
   cargo clippy -- -D warnings
   cargo test
   cargo audit
   ```
5. **Commit with descriptive messages**
6. **Push and create PR** targeting `dev`
7. **Mark PR as ready for review**
8. **Switch to next working branch** only after PR is ready

### PR Checklist
- [ ] All tests pass
- [ ] Clippy passes with `-D warnings`
- [ ] Code formatted with `cargo fmt`
- [ ] No security vulnerabilities (`cargo audit`)
- [ ] Documentation updated (README, TASK_TRACKER, etc.)
- [ ] Version bumped appropriately in Cargo.toml
- [ ] CHANGELOG.md updated
- [ ] PR description explains changes
- [ ] Marked as "ready for review"

### Merge Process
```
working/feature-name (PR #X)
  ↓ Review, approve, merge
dev (accumulate features)
  ↓ Integration testing, stabilization
testing (comprehensive testing, performance)
  ↓ Release candidate, final validation
main (tagged release)
```

### Release Process
1. **From dev to testing**:
   - Merge accumulated features
   - Run integration test suite
   - Fix any integration issues

2. **From testing to main**:
   - Final validation
   - Performance benchmarks
   - Security audit
   - Version tag: `v0.X.Y`
   - GitHub release with notes
   - Publish to crates.io

## Reference Materials

### Python PEFT Repository
- **URL**: https://github.com/huggingface/peft
- **Key Directories**:
  - `src/peft/tuners/` - Adapter implementations
  - `src/peft/config.py` - Configuration patterns
  - `src/peft/mapping.py` - Model integration
  - `src/peft/utils/` - Utilities

### Adapter Papers
See [AGENT_GUIDE.md](AGENT_GUIDE.md#-external-references) for full paper list.

### Rust Patterns
- **Candle Examples**: https://github.com/huggingface/candle/tree/main/candle-examples
- **Rust ML**: https://github.com/rust-ml
- **Best Practices**: https://rust-lang.github.io/api-guidelines/

## Tracking & Indices

### Local Development Trackers
1. **TASK_TRACKER.md** - Implementation status, task breakdown
2. **GAP_ANALYSIS.md** - Feature parity with Python PEFT
3. **AGENT_GUIDE.md** - Navigation and quick reference
4. **DEVELOPMENT_PLAN.md** (this file) - Overall development strategy
5. **CHANGELOG.md** (to be created) - Version history

### Metrics to Track
- Test coverage percentage
- Clippy warning count (goal: 0)
- Benchmark performance trends
- CVE count (goal: 0)
- Documentation completeness

## Decision Points & Exploratory Work

### When to Create Exploratory Branches
- **API design uncertainty** - Multiple valid approaches
- **Performance trade-offs** - Speed vs memory vs accuracy
- **Dependency choices** - Different library options
- **Algorithm variations** - Different mathematical approaches

### Example Scenarios
1. **Quantization Backend**:
   - `exploratory/quant-candle-native` - Use Candle's built-in
   - `exploratory/quant-custom-impl` - Custom implementation
   - Evaluate: performance, accuracy, maintenance

2. **Inference Optimization**:
   - `exploratory/inference-merge-ahead` - Merge weights at load
   - `exploratory/inference-lazy-merge` - Merge on first use
   - Evaluate: memory, speed, flexibility

## Success Criteria

### Phase Completion
- ✅ All tasks in phase completed
- ✅ All tests passing
- ✅ Clippy warnings = 0
- ✅ Cargo audit clean
- ✅ Documentation updated
- ✅ PR merged to dev
- ✅ Version incremented

### Project Completion (Pre-1.0)
- ✅ All phases 3-8 completed
- ✅ Comprehensive test coverage
- ✅ Production usage examples
- ✅ Community validation
- ✅ Performance benchmarks published
- ✅ Full documentation
- ✅ Zero critical issues

## Risk Management

### Technical Risks
| Risk | Mitigation |
|------|-----------|
| Candle quantization limitations | Explore custom implementations, contribute to Candle |
| Performance not meeting targets | Profiling, optimization phases, CUDA acceleration |
| API breaking changes in Candle | Pin versions, contribute upstream, adapt carefully |
| Test maintenance overhead | Test helper crate, focus on critical paths |

### Process Risks
| Risk | Mitigation |
|------|-----------|
| Scope creep | Strict phase boundaries, Python PEFT as reference |
| Quality regression | Automated CI/CD, mandatory checks before merge |
| Documentation drift | Update docs as part of every PR |
| Version confusion | Strict semantic versioning, CHANGELOG.md |

## Next Steps

### Immediate Actions (Phase 3)
1. **Create working branch**: `working/quality-clippy-fixes`
2. **Fix clippy warnings systematically**:
   - Start with documentation backticks (bulk of warnings)
   - Address casting safety issues
   - Handle missing error docs
3. **Set up cargo-audit automation**
4. **Create testing branch**
5. **Implement CI/CD pipeline**

### Agent Execution Plan
Once this plan is approved:
1. ✅ Mark this plan as approved
2. Switch to implementation agent mode
3. Create first working branch: `working/quality-clippy-fixes`
4. Iteratively fix clippy warnings (batch by type)
5. Create PR for phase 3, mark ready for review
6. Move to next phase working branch
7. Repeat until phase completion

---

**Status**: ✅ PLAN APPROVED - Ready for Implementation  
**Next Phase**: Phase 3 - Quality & Tooling  
**First Working Branch**: `working/quality-clippy-fixes`
