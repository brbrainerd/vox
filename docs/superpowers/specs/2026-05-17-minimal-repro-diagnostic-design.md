# `minimal_repro` field on diagnostic envelope

**Date:** 2026-05-17
**Closes:** the "single biggest delta between Vox-as-LLM-target and a typical compiler" per [`vox-as-llm-target-audit-and-plan-2026.md`](docs/src/architecture/vox-as-llm-target-audit-and-plan-2026.md) §2.2 and [`vox-language-rules-and-enforcement-plan-2026.md`](docs/src/architecture/vox-language-rules-and-enforcement-plan-2026.md) §1.2.

## Goal

Give LLM consumers of `vox check --for-llm` and structured diagnostics a *minimal reproducible excerpt* — the smallest contiguous source slice that reproduces the diagnostic — so the model can repair without the full file.

## Shape

Add one field to [`VoxCompilerDiagnosticPayload`](crates/vox-compiler/src/typeck/diagnostics.rs:96):

```rust
pub minimal_repro: Option<MinimalRepro>,
```

```rust
pub struct MinimalRepro {
    /// Excerpt of source containing the diagnostic span plus surrounding
    /// context. Always ends in a newline.
    pub excerpt: String,
    /// 1-based line number of the first line in `excerpt`.
    pub excerpt_first_line: usize,
    /// Span of the offending region, expressed in coordinates relative to
    /// `excerpt` (not the full source).
    pub local_span: SpanPayload,
}
```

`Option<>` rather than required so the field can be skipped when source isn't available (LSP scenarios with synthetic input) and so legacy consumers stay forward-compatible.

## Algorithm

Three lines of context before the start line, three lines after the end line, capped at file boundaries.

```
context_lines = 3
first = max(1, span.start_line - context_lines)
last  = min(total_lines, span.end_line + context_lines)
excerpt = source lines [first..=last] joined by '\n'
excerpt_first_line = first
local_span = span with start_line/end_line shifted by (1 - first)
```

Cols are preserved unchanged (the excerpt does not re-indent).

## Why this shape

- **Contiguous excerpt, not "smallest AST subtree"** — simpler to implement, matches what humans read in compiler output, easier for LLMs to keep token-aligned with the original file.
- **3 lines of context** — matches typical `git diff -U3` defaults; enough to see surrounding scope (function header, opening brace) without dragging in unrelated code.
- **Local-span coordinates** — so consumers can highlight the offending region within the excerpt without re-resolving file-absolute positions.
- **`Option<>`** — forward-compat. The `--for-llm` envelope serializes None as omitted; downstream code paths that don't have `source` (e.g. cached payloads) leave it as None.

## Surface

- `VoxCompilerDiagnosticPayload::from_diagnostic(diag, file_path, source)` populates `minimal_repro: Some(...)` when source is non-empty.
- `--for-llm` envelope (already at [vox-cli/src/pipeline.rs:160](crates/vox-cli/src/pipeline.rs:160)) automatically carries it (the field is serde-default-skipped on None).

## Tests (TDD)

1. `minimal_repro_basic` — multi-line source, diagnostic in middle line; excerpt has 3 lines before + diagnostic line + 3 lines after; local_span is correct.
2. `minimal_repro_near_start` — diagnostic on line 1; excerpt clipped to file start, no underflow.
3. `minimal_repro_near_end` — diagnostic on last line; excerpt clipped to file end.
4. `minimal_repro_single_line_file` — file is one line; excerpt is that line; local_span equals span.
5. `minimal_repro_multi_line_span` — diagnostic spans 3 lines; context still 3 lines before/after the span endpoints.
6. `minimal_repro_empty_source_is_none` — `from_diagnostic(diag, path, "")` returns `minimal_repro: None`.

## Out of scope

- "Smallest AST subtree that reproduces" — would require a delta-debugging pass; deferred. The 3-line-window heuristic is honest minimum-viable.
- Re-indentation / dedenting of the excerpt.
- LSP `textDocument/diagnostic` integration (the LSP path goes through a different envelope today).
