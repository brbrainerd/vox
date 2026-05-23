---
title: "ADR 016: Oratio streaming Whisper and constrained decode"
description: "Decide how Vox ships wire-level streaming Whisper and decoder-time constrained generation in Candle."
category: "Architecture Decisions (ADRs)"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "TechArticle"
---
# ADR 016: Oratio streaming Whisper and constrained decode

## Status

Accepted.

## Context

Oratio already supports offline Whisper transcription and chunked long-file processing. Product and extension flows require:

- wire-level partial transcript delivery while a user is speaking,
- stronger speech-to-code constraints than post-hoc reranking alone,
- explicit guidance on what stock Whisper can and cannot deliver at low latency.

## Decision

1. Keep Whisper/Candle as the default STT backend, and expose streaming over the wire using server-side partial events.
2. Implement constrained decode inside the decoder loop via a logit-processor hook.
3. Treat sub-second acoustic streaming as a quality/latency tradeoff mode, not a guarantee from stock Whisper.

## Implementation shape

- Decoder hook: `LogitProcessor` in `candle_engine`, called before suppress-token masking and token selection.
- Constraint tiers:
  - additive hotword/lexicon token bias,
  - explicit forbidden token masks,
  - optional token-trie constraints for finite command vocab.
- Streaming transport:
  - `vox-audio-ingress` WebSocket endpoint (`/api/audio/transcribe/stream`) for PCM chunk ingest + partial/final events.
  - MCP/clients discover streaming endpoint metadata via `vox_oratio_status`.

## Consequences

Positive:

- Better speech-to-code controllability without retraining.
- Shared streaming contract for CLI/editor/browser clients.
- Minimal change to existing offline pathways.

Tradeoffs:

- Token-trie constraints are approximate because BPE tokenization is not character-grammar exact.
- True low-latency partials may regress WER vs full-window decode.
- Single-process model mutex still limits concurrent decode sessions.

## Follow-ups

- Add VAD-gated incremental decode policy knobs for production defaults.
- Add nightly/e2e streaming tests with deterministic fixtures.
- Evaluate alternate streaming ASR backend behind the same ingress contract if latency SLA requires it.


