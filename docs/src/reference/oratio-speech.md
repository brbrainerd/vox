---
title: "Oratio & speech SSOT (Candle Whisper, no whisper.cpp)"
description: "Official documentation for Oratio & speech SSOT (Candle Whisper, no whisper.cpp) for the Vox language."
category: "Language Reference"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---
# Oratio & speech SSOT (Candle Whisper, no whisper.cpp)

## Why

- **STT without clang/native C++ toolchains**: inference is **Hugging Face Candle** (Rust), not whisper.cpp bindings.
- **One refined transcript path**: consumers use **display/refined** text where Oratio applies `light_trim` after decode.

## What (artifacts)

| Piece | Role |
|--------|------|
| **`vox-oratio`** | Candle Whisper, symphonia decode, `transcribe_path`, `eval` (WER/CER), env `VOX_ORATIO_*`. |
| **`vox-cli`** `vox oratio` | CLI transcription + status + sessionized `listen` flow (Enter-or-timeout, correction profile, route mode). |
| **`vox-mcp`** | `vox_oratio_transcribe` (thin STT + refine), `vox_oratio_listen` (session + route + optional LLM polish), `vox_oratio_status` (+ JSON schemas in tool registry). |
| **`vox-vscode`** | **onCommand** for contributed **`vox.*`** commands + **onView** sidebar + **`*.vox`**; Oratio palette + Explorer (audio, case-insensitive ext); relative MCP `path` or `.vox/tmp/` copy; voice → WAV. See [speech capture architecture](speech-capture-architecture.md). |
| **`vox-db` + HTTP/OpenAPI** | Codex/audio routes per [`codex-api.openapi.yaml`](../../../contracts/codex-api.openapi.yaml) — no `vox-codex-api` package (see [Codex HTTP API](codex-http-api.md)). |
| **Typeck / codegen** | Builtin **`Speech`**, **`Speech.transcribe(path) → Result[str]`** → `vox_oratio::transcribe_path` + refined text. |
| **Corpus mix** | `record_format: asr_refine` + schema **`mens/schemas/asr_refine_pairs.schema.json`**. |
| **LSP** | Hover for **`Speech`**; **`transcribe`** only when the line looks like **`Speech.transcribe`** (`builtin_hover_markdown_in_line`). |
| **TS codegen** | **`Speech.transcribe`** → **throw** (points at `examples/oratio/codexAudioTranscribe.ts` + `@endpoint(kind: server)` / HTTP). |
| **TS example** | **`examples/oratio/codexAudioTranscribe.ts`** — `fetch` for `/api/audio/status` and `/api/audio/transcribe`. |

## Who / when

- **Implementers**: **`vox-compiler`** (typeck, codegen), `vox-lsp`, `vox-cli`, `vox-mcp`, `vox-vscode`, `vox-db`, `vox-corpus`.
- **When to touch**: any change to Oratio env vars, transcript shape, HTTP contract, or builtin `Speech` API.

## Where (files)

- `crates/vox-oratio/` — STT + `eval`, `traits`, `refine`, `backends/*`
- `crates/vox-cli/src/commands/oratio_cmd.rs`
- `crates/vox-orchestrator/src/mcp_tools/tools/oratio_tools.rs`, `mod.rs` (registry + schemas)
- `apps/editor/vox-vscode/src/speech/registerOratioSpeechCommands.ts`, `src/core/VoxMcpClient.ts` (Oratio MCP wrappers)
- `crates/vox-capability-registry/`, `crates/vox-tools/` (`mens_chat` + `DirectToolExecutor`; Mens chat ∩ executor)
- `crates/vox-db/src/` — Codex store + readiness helpers consumed by HTTP surfaces.
- `crates/vox-compiler/src/typeck/` — `Speech` / builtins.
- `crates/vox-compiler/src/codegen_rust/` — `Cargo.toml` template + `MethodCall` for `Speech`
- `crates/vox-codegen-ts/src/` — `Speech.transcribe` stub
- `crates/vox-lsp/src/lib.rs` — `word_at_position`, `line_has_speech_transcribe`, `builtin_hover_markdown_in_line`; `main.rs` — hover
- `examples/oratio/codexAudioTranscribe.ts`, `examples/oratio/README.md`
- `crates/vox-corpus/src/corpus/mix.rs` — `record_format`, `normalize_training_jsonl_line`
- `mens/schemas/asr_refine_pairs.schema.json`, `mens/config/mix.example.yaml`
- `AGENTS.md`, `docs/src/reference/cli.md`, [`mens-training.md`](mens-training.md), **this file**

## How (contracts)

- **Build check:** `cargo check -p vox-oratio --features stt-candle`; for the **`vox`** CLI Oratio commands, `cargo check -p vox-cli --features oratio` (Oratio is **not** in default **`mens-base`**).
- **Env**: `VOX_ORATIO_MODEL`, `VOX_ORATIO_REVISION`, `VOX_ORATIO_LANGUAGE`, `VOX_ORATIO_CUDA` (feature-gated), `VOX_ORATIO_WORKSPACE` (HTTP path resolution), `VOX_DASH_HOST` / `VOX_DASH_PORT` (dashboard bind), **`VOX_ORATIO_SPEECH_LEXICON_PATH`** (optional JSON lexicon per `contracts/speech-to-code/lexicon.schema.json`, applied after refine; **merged** with **`$VOX_REPOSITORY_ROOT/.vox/speech_lexicon.json`** or **`$VOX_REPO_ROOT/.vox/speech_lexicon.json`** when those roots are set — explicit lexicon file wins on conflicting alias keys). **Contextual bias / rerank**: `VOX_ORATIO_CONTEXTUAL_BIAS` (`0`/`false` to disable), `VOX_ORATIO_SESSION_HOTWORDS` (comma-separated boosts), `VOX_ORATIO_MAX_BIAS_PHRASES` (cap). **Decoder-time constrained decode**: `VOX_ORATIO_LOGIT_BIAS_STRENGTH`, `VOX_ORATIO_LOGIT_BIAS_MAX_TOKENS`, `VOX_ORATIO_LOGIT_FORBID_TOKENS`, `VOX_ORATIO_CONSTRAINED_TRIE`, `VOX_ORATIO_CONSTRAINED_PHRASES`, `VOX_ORATIO_TRIE_STUCK_STEPS`. **Acoustic preprocess (Whisper path)**: `VOX_ORATIO_ACOUSTIC_PREPROCESS` (`none|peak_normalize`), `VOX_ORATIO_ACOUSTIC_PREPROCESS_BUDGET_MS` (default ~25ms wall budget; returns original PCM if exceeded). **Streaming stubs** (for live clients): `VOX_ORATIO_STREAM_PARTIAL_QUIET_MS`, `VOX_ORATIO_STREAM_MAX_WAIT_MS` — see `vox_oratio::StreamingStabilizationConfig`. **Long-file chunking** (Candle encoder window; optional): `VOX_ORATIO_CHUNK_SEC` (e.g. `20`–`28`, `5`–`28` clamped), `VOX_ORATIO_CHUNK_OVERLAP_SEC` (default `0.5`), optional `VOX_ORATIO_EMIT_PARTIAL_PATH` (append JSONL per chunk), `VOX_ORATIO_STREAM_TOKENS` (token-level event emission in streaming decoder loop). Optional **runtime TOML**: set **`VOX_ORATIO_CONFIG`** to a file with flat keys (`capture_timeout_ms`, `max_duration_ms`, `inference_deadline_ms`, `heartbeat_ms`, refine/routing/HF/LLM tunables plus `logit_*` keys — see `crates/vox-oratio/src/runtime_config.rs`). Env overrides file (**precedence: CLI args → env → file → defaults** for programmatic surfaces; CLI flags win on `vox oratio listen`). With the **`cuda`** feature, default inference is **CPU** until **`VOX_ORATIO_CUDA=1`**; status JSON includes **`cuda_feature_enabled`**, **`cuda_requested_via_env`**, **`inference_note`**. **`RUST_LOG=vox_oratio_gpu=info`** emits **`oratio_inference_cpu_default`** vs **`oratio_inference_gpu`** on first session load.
- **Session payloads** (CLI `listen`, MCP `vox_oratio_transcribe` / `vox_oratio_listen`, `vox-tools` direct executor) support: `timeout_ms` (UX / capture contract), `max_duration_ms` (session wall cap), optional `inference_deadline_ms` (transcribe+refine post-hoc cap), `heartbeat_ms`, `language_hint`, `profile` (`conservative|balanced|aggressive`), `route_mode` (`none|tool|chat|orchestrator`), `debug_parser_payload`. Responses may include **`language_diagnostics`**, **`deadline_diagnostics`**, and MCP **`runtime_config`** when debugging.
- **n-best transcripts**: MCP `vox_oratio_transcribe` and `vox_oratio_listen` expose optional **`n_best`** (best-first `string[]`) when contextual reranking yields multiple candidates; the listen response also includes the same list on the nested **`session`** object. Omitted when only one hypothesis survives rerank.
- **Routing session memory** (tool/chat/orchestrator classifier state): bounded with TTL + max session keys — override with **`VOX_ORATIO_ROUTING_SESSION_CAP`** (default 4096, floor 64) and **`VOX_ORATIO_ROUTING_SESSION_TTL_SECS`** (default 86400s, floor 60s).
- **HTTP transcribe body**: `{"path":"relative-or-absolute","language_hint":null}`; multipart upload: `POST /api/audio/transcribe/upload` with field **`audio`** or **`file`** (see `vox-audio-ingress`, `contracts/codex-api.openapi.yaml`).
- **HTTP streaming WS**: `GET /api/audio/transcribe/stream` (WebSocket). Binary messages are PCM `s16le` mono @ 16 kHz chunks; text control messages are JSON (`{"op":"set_language","language_hint":"en"}`, `{"op":"commit"}`, `{"op":"cancel"}`). Server emits JSON text events `ready`, `partial`, `final`, `error`.
- **Mix YAML**: optional per-source `record_format: asr_refine`.

## Related

- **Speech-to-code pipeline** (MCP validation parity, corpus `speech_to_code`, KPI contracts): [`speech-to-code-pipeline.md`](speech-to-code-pipeline.md).
- **Native fine-tuning** (Burn LoRA / `vox mens train`): [`mens-training.md`](mens-training.md).
- **Mens chat tool allowlist**: `vox-tools` module **`mens_chat`** (`chat_tool_definitions` / `execute_tool_calls`), intersecting `vox-capability-registry` with `DirectToolExecutor` — **same MCP names** as `vox-mcp`. Callers (CLI, daemons, tests) import `vox_tools::mens_chat` when they need OpenAI-style tool JSON or in-process execution.

## Out of scope / deprecated

- **whisper.cpp / ggml / clang STT**: not supported in-tree; old plans under `.cursor/plans/` that cite `whispercpp.rs` are **historical** — canonical STT is **Candle** in `vox-oratio`.

