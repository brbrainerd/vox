---
title: "Vox Speech Surface Inventory 2026"
description: "Repository-audited inventory of microphone, speech capture, and speech-to-code entry points across editor, app, dashboard, CLI, MCP, HTTP, and streaming surfaces."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-05-11"
training_eligible: true
training_rationale: "Maps user-visible speech surfaces to concrete code paths and test gaps for speech-to-code agents."
---

# Vox Speech Surface Inventory 2026

This inventory anchors the ASR-primary broad-wave speech-to-code audit. A surface is included when it either captures speech, claims voice support, forwards audio/transcripts into Oratio, or is documented as part of the speech path.

## Summary

| Surface | Capture | Transport | Backend | Current status | Coverage |
|---|---|---|---|---|---|
| Editor file commands | Existing audio file | MCP | Oratio plugin / `vox-oratio` | Implemented | No extension E2E |
| Editor webview mic | Browser `getUserMedia` WAV | VS Code postMessage then MCP | Oratio plugin / `vox-oratio` | Implemented | No automated mic E2E |
| Vox app web speech | Web Speech API or prompt fallback | In-process JS runtime | Browser STT | Implemented but not Oratio | Stubbed E2E only |
| Vox app Android | Native mic | Capacitor bridge | Sherpa ONNX plugin | Present | No accuracy parity test |
| Vox app iOS | Native mic | Capacitor bridge | Apple Speech | Present | No accuracy parity test |
| Dashboard Loquela | None | Text chat only | None | Gap | No speech test |
| CLI eval | Dataset file paths | Local CLI | Oratio | Implemented | WER/CER unit math only |
| CLI record-transcribe | `cpal` mic | Local CLI | Oratio | Feature-gated | No automated mic test |
| MCP tools | Path or prompt | MCP | Oratio + codegen | Implemented | Schema/HIR tests only |
| Streaming WS | s16le 16 kHz chunks | WebSocket | Oratio server | Feature-gated | No runtime stream test |
| HTTP audio ingress | Documented `/api/audio/*` | HTTP | Missing | Orphaned contract | No crate present |

## Surface Details

### Editor file commands

- Entry points: `vox.oratio.transcribeFile`, `vox.oratio.speechToCodeFile` in `apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts`.
- Capture format: `.wav`, `.mp3`, `.flac`, `.ogg`, `.m4a`, `.webm` selected from disk.
- Transport: workspace-relative path to `VoxMcpClient.oratioTranscribe` or `VoxMcpClient.speechToCode`.
- Target tools: `vox_oratio_transcribe`, `vox_speech_to_code`.
- Verified gap: no automated extension test proves file path to MCP to transcript or code.

### Editor webview mic

- Entry points: `vox.oratio.voiceCaptureTranscribe`, `vox.oratio.voiceCaptureSpeechToCode`.
- Capture format: `navigator.mediaDevices.getUserMedia({ audio: true })`, `ScriptProcessorNode`, mono 16-bit PCM WAV.
- Important finding: WAV header uses `audioCtx.sampleRate`, commonly 48 kHz, not a forced 16 kHz stream. Correctness depends on Oratio decode/resample.
- Transport: base64 WAV via `postMessage`, written under `.vox/tmp/vscode_voice_*.wav`, then sent to MCP.
- Verified gap: no synthetic audio injection harness or mic permission test exists.

### Vox-language app speech

- Entry point: `Speech.transcribe_microphone()` in `apps/vox-mental-tracker/src/main.vox`, lowered through TypeScript runtime shims in `apps/vox-mental-tracker/src/runtime.ts`.
- Web capture: `SpeechRecognition` / `webkitSpeechRecognition`.
- Fallback: `window.prompt`, which must be classified as `ASR=N/A` rather than an ASR success.
- Transport: none. The browser path does not call Oratio or the orchestrator.
- Coverage: `apps/vox-mental-tracker/tests/e2e/voice_flow.spec.ts` uses `__VOX_TEST_TRANSCRIPT__`, so it verifies UI plumbing, not ASR.

### Mobile app speech

- Android entry point: `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/android/src/main/java/com/vox/plugins/voxsherpatranscribe/VoxSherpaTranscribePlugin.kt`.
- iOS entry point: `apps/vox-mental-tracker/plugins/vox-sherpa-transcribe/ios/AppleSpeechBackend.swift`.
- Backends: Sherpa ONNX on Android and Apple Speech on iOS.
- Verified gap: no shared corpus parity test compares mobile transcripts against Candle Whisper or Oratio fixtures.

### Dashboard Loquela

- Entry point: `crates/vox-dashboard/src/components/shell/SpeakPanel.tsx`.
- Current behavior: text area plus send button. The subtitle says `VOICE INTERFACE`, but there is no microphone button, `getUserMedia`, `MediaRecorder`, Oratio tool, or speech-to-code call.
- Transport: `useVoxChat` sends text through `vox_chat_message`.
- Audit classification: product gap, not an ASR cell.

### CLI and MCP

- CLI eval: `crates/vox-ml-cli/src/commands/oratio_cmd.rs` exposes WER/CER evaluation and persistence paths.
- CLI mic: `crates/vox-ml-cli/src/commands/oratio_mic.rs` is feature-gated by `oratio-mic`.
- MCP Oratio: `crates/vox-orchestrator-mcp/src/oratio_tools.rs`.
- MCP speech-to-code: `crates/vox-orchestrator-mcp/src/speech_pipeline_tools.rs`.
- Coverage: schema parity, HIR fixture validation, and optional KPI canary exist, but no always-on runtime model canary exists.

### Streaming and HTTP

- Streaming server: `crates/vox-oratio/src/serve.rs`, feature `serve`, expects s16le mono 16 kHz chunks.
- HTTP audio: `examples/oratio/codexAudioTranscribe.ts` and `contracts/codex-api.openapi.yaml` describe `/api/audio/*`, but no `crates/vox-audio-ingress` exists in this checkout.
- Env drift: `contracts/config/env-vars.v1.yaml` still assigns `VOX_ORATIO_STREAM_MAX_BUFFER_MS` and `VOX_ORATIO_WORKSPACE` to `vox-audio-ingress`.

## Audit Consequences

1. The editor webview is the only mouse-driven desktop mic surface that currently reaches Oratio and `vox_speech_to_code`.
2. The dashboard must be scored as a missing speech surface until a mic path exists.
3. The Vox-language app path proves ordinary app speech syntax exists, but its web implementation is browser STT rather than Oratio.
4. Backend accuracy comparisons must separate Candle Whisper, Sherpa, Apple Speech, Web Speech API, and prompt fallback instead of blending them.
