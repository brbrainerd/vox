---
title: "Vox Speech Audit Findings 2026"
description: "ASR-primary findings from the broad-wave speech-to-code audit setup, including verified repository gaps and the scorecard model used for matrix runs."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Captures measured and verified gaps for speech-to-code accuracy work."
---

# Vox Speech Audit Findings 2026

This findings document records the broad-wave audit pass and the first full runtime suite over all MUST+SHOULD cells. Runtime artifacts are under `.vox/audit/2026-05-11-oratio-full-runtime/`.

## Repository-Verified Findings

| ID | Finding | Evidence | Impact | Category |
|---|---|---|---|---|
| STF-001 | Dashboard Loquela is text-only despite voice labeling. | `crates/vox-dashboard/src/components/shell/SpeakPanel.tsx` has a textarea and send button, no mic capture or Oratio tool. | Users cannot use speech-to-code from dashboard. | orchestration |
| STF-002 | HTTP audio ingress is documented but source crate is absent. | `contracts/codex-api.openapi.yaml` and examples reference `/api/audio/*`; no `crates/vox-audio-ingress` exists. | HTTP speech path cannot be audited or shipped from this repo. | orchestration |
| STF-003 | Editor webview records at native `AudioContext.sampleRate`. | `registerOratioSpeechCommands.ts` posts WAV with `sampleRate: sr`. | 48 kHz and device-rate audio depend on decode/resample correctness. | acoustic |
| STF-004 | Benchmark manifest previously had no audio rows. | `contracts/speech-to-code/benchmark-fixtures.manifest.txt` listed transcript and expected Vox only. | No ASR-quality gate could run from committed fixtures. | acoustic |
| STF-005 | Canary KPI gate was opt-in only. | `speech_canary_test.rs` returns early when `VOX_SPEECH_CANARY_KPI` is unset. | CI can pass without checking ASR metrics. | orchestration |
| STF-006 | Mobile STT is backend-divergent. | Android Sherpa plugin and iOS Apple Speech backend differ from Oratio Candle path. | Cross-system accuracy may drift without shared corpus tests. | lexical |
| STF-007 | Web Vox-app speech can fall back to typed prompt. | `runtime.ts` uses `window.prompt` when Web Speech API is absent. | Prompt fallback must not be counted as ASR success. | orchestration |
| STF-008 | `vox-plugin-oratio` single-window Whisper inference had a forced test error. | Runtime eval initially failed with `Whisper inference`; `candle_whisper.rs` contained a hard-coded simulated OOM branch. | Real audio inference could not run until the branch was removed. | orchestration |
| STF-009 | CUDA is installed but Candle CUDA plugin linking fails on Windows. | `nvcc 13.1` is present; `cargo build -p vox-plugin-oratio --features cuda` fails with unresolved `moe_gemm_*` symbols. | CUDA SHOULD cell is a measured build/link failure, not a no-CUDA skip. | orchestration |

## Scorecard Model

The audit uses `contracts/speech-to-code/kpi-baseline.schema.json` for per-cell snapshots. The committed baseline at `contracts/speech-to-code/canary.kpi.json` mirrors the example threshold policy and exists so CI can validate the shape before real measured snapshots land.

Primary metrics:
- `wer`
- `cer`

Supporting metrics:
- `compile_pass_at_1`
- `compile_pass_at_k`
- `semantic_pass_rate`
- `latency_ms_median`
- `latency_ms_p95`

Reference threshold policy:
- `wer <= 0.35`
- `cer <= 0.15`
- `compile_pass_at_1 >= 0.65`
- `compile_pass_at_k >= 0.72`
- `latency_ms_p95 <= 12000`

## Runtime Matrix Status

| Matrix subset | Current state | Evidence |
|---|---|---|
| Windows CPU Candle audio eval | Runtime completed through the audit-local `oratio` plugin. The spoken 16 kHz corpus now measures `WER=0.1071`, `CER=0.0976`, under the current canary thresholds. | `.vox/audit/2026-05-11-oratio-full-runtime/win-cli-eval-whisper-cpu-16k-all/` |
| Windows editor webview mic | Capture code is present and static probes pass, but no executable VS Code webview mic harness exists. Backend surrogate uses the same CPU Candle result and is classified as an orchestration failure. | `speech_pipeline_stage_probe_test` plus runtime scorecard |
| MCP audio path | MCP code shares the Oratio plugin path, but no committed MCP audio harness exists for a per-cell invocation. | `.vox/audit/2026-05-11-oratio-full-runtime/win-mcp-path-whisper-cpu-16k-command/` |
| Prompt-only MCP route | ASR is N/A; compile/HIR support metrics pass. | `.vox/audit/2026-05-11-oratio-full-runtime/win-mcp-prompt-no-asr-route/` |
| CUDA Candle | Toolkit is available, but the plugin does not link with CUDA features on this Windows host. | `.vox/audit/2026-05-11-oratio-full-runtime/win-cli-eval-whisper-cuda-16k-code/` |
| Linux/Sherpa/iOS/Android/browser Web Speech | Runtime-unavailable in this execution host with concrete evidence (`xcrun` absent, Android SDK `adb` present but no attached/emulated devices, no Linux/browser mic runner exposed). | Per-cell `cell_result.json` files |
| Dashboard and HTTP audio ingress | Confirmed runtime gaps. | Static probes and source inventory |

## Immediate Conclusions

The repository is closer to speech-to-code in the editor and MCP pipeline than in dashboard or ordinary Vox apps. After replacing the silent/synthetic WAV fixtures with spoken 16 kHz fixtures and removing the forced OOM branch, the Windows CPU Candle eval now passes the current WER/CER canary thresholds. Remaining gaps are surface/runtime coverage: dashboard mic, HTTP ingress, streaming WS, CUDA linking, Linux/Sherpa, browser Web Speech, and real mobile device execution.
