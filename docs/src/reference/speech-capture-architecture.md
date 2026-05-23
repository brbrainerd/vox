---
title: "Speech capture architecture (edge vs backend)"
description: "Where audio is captured, how it reaches Oratio/MCP, and Docker-friendly deployment."
category: "Language Reference"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---

# Speech capture architecture

## Principle

- **Edge / client:** microphone, file drops, browser `MediaRecorder`, mobile native capture.
- **Backend:** STT, refinement, routing, codegen, and HIR validation run where **`vox-oratio`**, **`vox-mcp`**, and **`vox-lsp`** validation can execute (developer machine, CI agent host, or container **without** requiring a container-attached mic).

Containers **should not** assume direct microphone device access; bind-mount a workspace directory or use HTTP upload instead.

## Surfaces (canonical)

| Surface | Role | Notes |
|--------|------|--------|
| **`vox-audio-ingress`** binary | HTTP `/api/audio/status`, `/api/audio/transcribe`, `/api/audio/transcribe/upload` | Bind via `VOX_DASH_HOST` / `VOX_DASH_PORT`; workspace root from `VOX_ORATIO_WORKSPACE` or CWD. |
| **MCP** `vox_oratio_transcribe`, `vox_oratio_listen` | File-path STT inside MCP workspace | Compatibility path for agents; same Oratio pipeline as CLI. |
| **MCP** `vox_speech_to_code` | **Orchestration:** path or text → `vox_generate_code` (+ optional `emit_trace_path` JSONL) | Shares `session_id` / repair KPI metadata with codegen. |
| **CLI** `vox oratio transcribe` / `listen` | File + UX gates | Feature **`oratio`**. |
| **CLI** `vox oratio record-transcribe` | Default mic → temp WAV → transcribe | Feature **`oratio-mic`** (`cpal` + `hound`). |

OpenAPI mirror (Codex HTTP catalog): `contracts/codex-api.openapi.yaml` under `/api/audio/*`.

## Platform clients (same contracts)

- **VS Code / Cursor (`vox-vscode`):** Command Palette **Vox: Oratio —** … (`vox.oratio.transcribeFile`, `vox.oratio.speechToCodeFile`, `vox.oratio.voiceCaptureTranscribe`, `vox.oratio.voiceCaptureSpeechToCode`), **Explorer** context menu on audio files (case-insensitive extension match), plus **`onView:vox-sidebar.chat`** and **`onCommand`** entries for contributed **`vox.*`** commands (including Oratio and inline-edit keybindings) so MCP + speech work without **`*.vox`** in the workspace. Files already under the workspace use a **relative** MCP `path`; outside picks copy to **`.vox/tmp/`**. Voice capture encodes **mono 16-bit PCM WAV** in the webview before the same MCP calls. Alternatively POST audio to **`vox-audio-ingress`** when a shared HTTP endpoint is configured.
- **Browser / web:** `MediaRecorder` (or file upload) → **`POST /api/audio/transcribe/upload`** (or finalize to disk and JSON transcribe in trusted environments).
- **Mobile:** native capture → same upload contract; do not require the monorepo Docker image on-device (see [`mobile-edge-ai.md`](mobile-edge-ai.md) for inference ownership).

## Trace and correlation

- Generate correlation IDs with `vox_oratio::trace::new_correlation_id()` and pass **`session_id`** through MCP for chat/model affinity.
- Optional **`emit_trace_path`** on `vox_speech_to_code` appends one JSON object per call; fields align with `contracts/speech-to-code/speech_trace.schema.json` (plus `codegen_meta` for tooling).

## Related

- [Speech-to-code pipeline](speech-to-code-pipeline.md)
- [Operations & rollout](speech-to-code-operations.md)
- [Oratio & speech SSOT](oratio-speech.md)
- [Deployment compose](deployment-compose.md)

