---
title: "Speech-to-code pipeline (Oratio → MCP → compiler → MENS)"
description: "Architecture and contracts for spoken input to validated Vox code."
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# Speech-to-code pipeline

End-to-end flow: **audio or transcript** → **Oratio** (`vox-oratio`, optional peak normalize + contextual phrase rerank) → optional **routing intents** (token-aware classifier) → **MCP** tools (**`vox_speech_to_code`** orchestrates transcribe + `vox_generate_code`; or use `vox_oratio_*` + `vox_generate_code` separately; `validate_file` for explicit checks) → **full frontend validation** (including **HIR**) via `vox_lsp::validate_document_with_hir` → **MENS** training data (`asr_refine`, `speech_to_code` mix formats).

**Ingress:** HTTP **`vox-audio-ingress`** (`/api/audio/transcribe` JSON path body, `/api/audio/transcribe/upload` multipart) plus edge capture doc: [`speech-capture-architecture.md`](speech-capture-architecture.md).

### Failure-oriented notes

- **Schema SSOT**: telemetry traces use `contracts/speech-to-code/speech_trace.schema.json`; supervised export adds `vox_code` via `speech_trace.mens.schema.json` (`mens/schemas/speech_to_code_trace.schema.json` re-exports). `failure_category` matches `failure-taxonomy.schema.json` and `SpeechFailureCategory` in Rust.
- **Grammar hints, not grammar guarantees**: `contracts/speech-to-code/vox_grammar_artifact.json` is lexicon surface for prompt hints; hard gate remains compiler validation + bounded repair (stall detection on repeated diagnostics).
- **Benchmark fixtures**: `contracts/speech-to-code/benchmark-fixtures.manifest.txt` lists frozen paths under `tests/speech-to-code/fixtures/` (validated in integration tests + HIR smoke on expected `.vox`).

## KPIs and contracts

- JSON schemas: [`contracts/speech-to-code/`](../../../contracts/speech-to-code/README.md)
- Failure taxonomy: `SpeechFailureCategory` in `vox-oratio::failure_taxonomy`
- Correlation IDs: `vox-oratio::trace::new_correlation_id()` (propagate in MCP responses)

## Validation parity

- **LSP-fast path**: `validate_document` — lex, parse, typecheck (plus mesh warnings).
- **CLI / speech gate**: `validate_document_with_hir` — same plus **HIR structural validation** (matches `vox-cli` `run_frontend_str` for type/HIR diagnostics).

MCP **`vox_validate_file`** joins relative paths to the **MCP repository root**, then canonicalizes and rejects paths outside that root (absolute paths must still resolve under the bound workspace). **`vox_generate_code`** MCP input schema is strict (`additionalProperties: false`) for `prompt`, optional `validate`, `max_retries`, and `session_id`.

MCP `validate_file` and `generate_vox_code` validation retries use **`validate_document_with_hir`**.

## Corpus mix

- `record_format: speech_to_code` — see [`crates/vox-corpus/src/corpus/mix.rs`](../../../crates/vox-corpus/src/corpus/mix/mod.rs) and `mens/schemas/speech_to_code_trace.schema.json`.

## Deterministic speech helpers

- **Lexicon** (`SpeechLexicon::from_json_slice` + `apply`): project aliases → identifiers.
- **Normalize** (`speech_normalize`): spoken symbols (`fat arrow` → `=>`) and casing commands (`camel case foo bar` → identifiers).

## Related

- [Speech capture architecture (edge vs backend)](speech-capture-architecture.md)
- [Oratio & speech SSOT](oratio-speech.md)
- [Operations / security / rollout](speech-to-code-operations.md)
- [MENS training](mens-training.md)
- [MENS speech curriculum](mens-speech-curriculum.md)

