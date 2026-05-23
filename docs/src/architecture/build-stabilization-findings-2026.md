---
title: "build-stabilization-findings-2026"
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---
# Vox Orchestration Build Stabilization Findings (2026-04-21)

## Summary
This research document records the findings and fixes implemented during the April 2026 stabilization sweep of the Vox orchestration build system. The primary goal was to restore system-wide compilation and integration test stability following significant schema hardening and structural refactors.

## Findings

### F1: Schema Hardening Regressions (vox-populi)
The hardening of the `A2ADeliverRequest` struct introduced mandatory fields for wire-format delivery that were not updated in mock objects used across integration tests.
- **Problem**: `priority`, `task_kind`, and `model_id` were missing from test initializers.
- **Fix**: Updated `crates/vox-populi/tests/http_control_plane.rs` and related mocks to include these fields with safe defaults.
- **Privacy Model**: Standardized `privacy_class` to `Some("private".into())` to ensure predictable local claiming in test environments.

### F2: Serving Layer Signature Drift (vox-oratio)
Structural changes to the transcription backend caused a signature mismatch in the Axum serving worker.
- **Problem**: `transcribe_pcm_internal` was being called with a redundant `config` argument, and the return type lacked the `AsrOutput` wrapper expected by the JSON extractor.
- **Fix**: Synchronized the call site in `serve.rs` and implemented proper result wrapping.

### F3: Feature Gate & Type Inference Ambiguities (vox-ml-cli)
Consuming crates required explicit opt-in for certain transitive capabilities that were previously implicit or handled via Hallucinated APIs.
- **Problem**: `vox-bounded-fs` required the `async` feature, and `vox-corpus` required the `database` feature to be enabled within `vox-ml-cli`.
- **Problem**: Type inference for `read_utf8_path_capped_async` was ambiguous in `corpus/mod.rs`.
- **Fix**: Updated `Cargo.toml` and applied explicit `String` type annotations to resolve E0282.

### F4: Cloud Provider Trait Incompleteness
The `LocalProvider` implementation had lagged behind the evolved `CloudProvider` trait definition.
- **Problem**: Missing `kind()` method; mismatched `dispatch` and `poll_status` signatures; deprecated field names in `GpuOffer` and `JobHandle`.
- **Fix**: Refactored `local_provider.rs` and `resolver.rs` to fully implement the modern trait and align with the `HardwareRegistry` probe results.

### F5: CLI Test Parameter Mismatch
Recent changes to the `vox build` command (adding `BuildMode` support) broke integration tests in `vox-cli`.
- **Problem**: `build::run` calls in `full_stack_minimal_build.rs` were missing the 6th argument.
- **Fix**: Standardized all test calls to use `BuildMode::App`.

## Conclusion
The Vox workspace has been restored to a stable state. All core orchestration components (`mens`, `populi`, `oratio`, `orchestrator`) now compile cleanly under all feature sets, and the central HTTP control plane integration tests pass.

## Related Artifacts
- Walkthrough (IDE-private artifact, not versioned)
- Implementation Plan (IDE-private artifact, not versioned)

