---
title: "Vox Speech Improvement Backlog 2026"
description: "Prioritized backlog for improving Vox speech-to-code accuracy, surface parity, pipeline reliability, and CI coverage."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Gives future agents a concrete backlog for speech-to-code improvements with owner crates and expected metric lift."
---

# Vox Speech Improvement Backlog 2026

Backlog rows are ranked by expected user impact and ability to improve ASR-primary audit confidence. Rankings now incorporate the measured full runtime suite at `.vox/audit/2026-05-11-oratio-full-runtime/`.

| ID | Gap | Owner | Category | Expected lift | Effort | Dependencies |
|---|---|---|---|---|---|---|
| STB-001 | Add dashboard microphone capture that reaches `vox_speech_to_code`, or change Loquela copy to text-chat until implemented. | `vox-dashboard`, `vox-orchestrator-mcp` | orchestration | High UX lift, compile pass measurable from dashboard | M | MCP tool availability in dashboard transport |
| STB-002 | Restore or retire `vox-audio-ingress` references and reassign orphaned env vars. | `contracts/config`, `vox-oratio` | orchestration | Removes false transport surface | S | Contract owner decision |
| STB-003 | Add committed speech canary cell to CI instead of optional `VOX_SPEECH_CANARY_KPI` only. | `vox-cli`, `vox-integration-tests` | orchestration | High regression-detection lift | M | Corpus v1 and canary KPI |
| STB-004 | Add runtime CUDA decode parity test against CPU for one short fixture. | `vox-oratio`, `vox-cli` | acoustic | Medium WER confidence on GPU systems | M | CUDA runner availability |
| STB-005 | Add editor webview synthetic-audio harness for 48 kHz WAV path. | `apps/editor/vox-vscode` | acoustic | High capture-format confidence | M | Extension test harness |
| STB-006 | Wire mobile Android/iOS STT into shared corpus parity reporting. | `apps/vox-mental-tracker` | lexical | High cross-system accuracy visibility | L | Device/emulator CI lane |
| STB-007 | Replace prompt fallback classification in Vox app tests with explicit `ASR=N/A`. | `apps/vox-mental-tracker` | orchestration | Reduces false-positive ASR scoring | S | E2E update |
| STB-008 | Promote `symbol_error_rate` into CLI scorecards for identifier-heavy speech. | `vox-oratio`, `vox-populi` | lexical | Medium code-domain accuracy lift | S | Scorecard schema extension |
| STB-009 | Add a real silence/no-speech model canary for Candle. | `vox-oratio` | acoustic | High hallucination prevention | M | Model cache / offline fixture |
| STB-010 | Add streaming WS contract test for partial/final events and backpressure. | `vox-oratio` | orchestration | Medium reliability lift | M | `serve` feature lane |
| STB-011 | Replace synthetic WAV fixtures with real spoken audio for code dictation, commands, identifiers, mixed-natural, and noisy domains. | `contracts/speech-to-code`, `tests/speech-to-code` | acoustic | Done for the current Windows SAPI 16 kHz fixture set; keep open for human/device recordings. | M | Consent-cleared corpus recording or generated speech fixtures |
| STB-012 | Add a first-class `vox ci speech-runtime-suite` runner that emits per-cell JSON/KPI artifacts without ad hoc shell commands. | `vox-cli`, `vox-integration-tests` | orchestration | Done for matrix classification + CPU Candle runtime eval; extend as new harnesses land. | M | Stable runtime matrix and artifact schema |
| STB-013 | Fix Windows Candle CUDA linking for `vox-plugin-oratio --features cuda`. | `vox-plugin-oratio`, `patches/candle-*` | orchestration | Enables GPU SHOULD cell measurement on Windows. | M | Resolve `moe_gemm_*` link symbols or gate incompatible kernels |
| STB-014 | Expose a real Oratio streaming route or remove the advertised WS stream URL from MCP status. | `vox-oratio`, `vox-orchestrator-mcp` | orchestration | Eliminates streaming false positive and enables partial/final tests. | M | Transport contract decision |

## Sequencing

1. Land STB-003 next so the generated passing CPU Candle KPI is enforced rather than optional.
2. Land STB-002, STB-007, and STB-014 to remove false-positive runtime surfaces.
3. Land STB-005 and STB-009 to close the highest-risk acoustic harness gaps.
4. Land STB-013, STB-004, and STB-006 when hardware/device runners are available.

## Definition Of Done

Each backlog row is complete only when it has:
- A failing test or failing audit cell before implementation.
- A passing verification command after implementation.
- A scorecard delta or documented skip reason.
- An entry in the speech audit findings doc if it changes audit interpretation.
