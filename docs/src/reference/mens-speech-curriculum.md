---
title: "MENS curriculum — speech-to-code stages"
description: "Suggested staged training mix for spoken input → Vox code."
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: true

schema_type: "TechArticle"
---

# MENS curriculum (speech-to-code)

Staged supervision to reduce “lost in transcription” drift:

1. **Stage A — Transcript cleanup**: `asr_refine` and deterministic Oratio refine pairs; teach model to fix ASR noise without changing CLI flags/paths.
2. **Stage B — Intent / structure**: Short prompts mapping normalized transcript → outlines (function names, parameters) without full program.
3. **Stage C — Constrained codegen**: Full `.vox` emits with compiler-checked examples only (`speech_to_code` mix rows).
4. **Stage D — Repair supervision**: Prompt = failing snippet + diagnostics; response = minimal fix (MCP retry-loop style).

Weight higher-quality, compiler-validated rows; cap aggressive ASR-only pairs. See [`speech-to-code-pipeline.md`](speech-to-code-pipeline.md) and [`mens-training.md`](mens-training.md).

## QA / labeling

Use [`contracts/speech-to-code/labeling_rubric.md`](../../../contracts/speech-to-code/labeling_rubric.md) for human or LLM-assisted labels (`intent_ok`, `compile_ok`, `semantic_ok`, verbatim-sensitive spans). Export traces with `failure_category` (not a loose free-form `category` string) for KPI joins.

