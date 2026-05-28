---
title: "Vox Speech CI Gates Proposal 2026"
description: "Proposal for turning speech-to-code audit checks into required and advisory CI gates."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Defines concrete CI guardrails for speech-to-code quality and future agents."
---

# Vox Speech CI Gates Proposal 2026

The current CI posture validates schemas and some HIR fixtures, but it does not run a default numeric ASR gate unless `VOX_SPEECH_CANARY_KPI` is supplied. This proposal separates required gates from advisory gates so expensive model or hardware work does not block every contributor while still preventing silent regression.

## Required Gates

| Gate | Command | Why required |
|---|---|---|
| Speech audit contracts | `cargo test -p vox-integration-tests --test speech_audit_contract_test` | Ensures matrix, committed KPI, docs, and index entries exist. |
| Speech manifest triples | `cargo test -p vox-integration-tests --test speech_benchmark_manifest_test` | Ensures corpus rows have audio/transcript/expected metadata. |
| Speech schema parity | Existing `speech_schema_parity_test` | Prevents schema drift across contracts and MENS exports. |
| HIR fixture validation | Existing `speech_fixture_validate_test` | Prevents expected Vox outputs from becoming uncompilable. |
| Committed canary shape | `VOX_SPEECH_CANARY_KPI=<absolute path to contracts/speech-to-code/canary.kpi.json> cargo test -p vox-integration-tests --test speech_canary_test` | Makes canary validation non-optional at least for the baseline snapshot. |
| Forced OOM regression | `cargo test -p vox-plugin-oratio single_window_branch_does_not_force_simulated_oom` | Prevents the single-window Whisper path from regressing to a simulated inference failure. |

## Advisory Gates

| Gate | Trigger | Skip reason |
|---|---|---|
| Runtime Candle ASR canary | Nightly or release branch with model cache | `offline` |
| CUDA decode parity | Runner has `nvcc` and CUDA runtime | `no-cuda` |
| Sherpa parity | `stt-sherpa` feature and model dir available | `feature-off` |
| Mobile speech parity | Device/emulator runner available | `not-available` |
| Streaming WS partial/final | `serve` feature lane | `transport_error` |

## Current Runtime-Suite Result

The `.vox/audit/2026-05-11-oratio-full-runtime/` scorecard is CI-ready as an artifact, but it is not a passing ASR gate:

- CPU Candle audio evaluation executes end-to-end after the forced-OOM fix. The current spoken 16 kHz corpus measures `WER=0.1071`, `CER=0.0976`, under the current canary thresholds.
- CUDA is not a `no-cuda` skip on the Windows host; `nvcc` exists, but CUDA plugin linking fails on unresolved Candle `moe_gemm_*` symbols.
- iOS and Android cells are runtime-unavailable in this host because `xcrun` is unavailable and Android SDK `adb` reports no attached/emulated devices.
- The advertised Oratio streaming WebSocket URL has no matching `vox-oratio` WS server route; only HTTP `POST /transcribe` exists.

## CLI Work Items

1. Add an `oratio` feature lane to `FEATURE_SETS` in `crates/vox-cli/src/commands/ci/constants.rs` and unignore `feature_sets_include_populi_oratio_lane` in the matrix tests.
2. Add `vox ci speech-canary` as a wrapper that:
   - loads `contracts/speech-to-code/audit-matrix.v1.yaml`,
   - runs the smallest MUST CLI eval cell when model assets are available,
   - writes `.vox/audit/<run_id>/scorecard.json`,
   - writes a KPI snapshot,
   - runs `speech_canary_test` against that snapshot.
3. Extend `crates/vox-cli/src/commands/ci/run_body_helpers/cuda.rs` from compile-only coverage to include a tiny runtime decode parity cell when the corpus and model cache exist.
4. Extend `vox ci speech-runtime-suite` beyond its current CPU Candle + classification implementation as browser, mobile, CUDA, Sherpa, and streaming harnesses become executable.

## Policy

Required gates must not download models. Advisory gates may use cached models or explicitly skip with a machine-readable reason. A skipped advisory cell is acceptable; an unreported skip is a CI bug.
