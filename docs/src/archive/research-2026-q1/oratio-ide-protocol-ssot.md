---
title: "Oratio IDE Protocol (SSOT)"
description: "Protocol for injecting IDE/Workspace context into the Vox Oratio speech-to-code pipeline."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-18"
training_eligible: false
training_rationale: "Defines the context injection protocol for Oratio."
archived_date: 2026-04-18
---
# Oratio IDE Protocol (SSOT)

This document defines the protocol for injecting IDE/Workspace context into the Vox Oratio speech-to-code pipeline.

## Context Discovery

When `vox oratio` (or the underlying `vox-oratio` crate) performs intent classification, it attempts to "ground" the transcript in the current workspace state. Context is discovered in the following order of precedence:

1. **Environment Variables**: Overrides for specific session-scoped values.
2. **IDE State File**: A JSON file at `.vox/ide_state.json` maintained by the editor extension.
3. **Workspace Database**: Historical data (e.g., build errors) queried from `.vox/store.db`.

## Data Schema (`IdeContext`)

The context is represented by the `IdeContext` structure:

```rust
pub struct IdeContext {
    pub active_file: Option<String>,
    pub cursor_line: Option<usize>,
    pub recent_errors: Vec<String>,
    pub local_symbols: Vec<String>,
}
```

### JSON Mapping (`.vox/ide_state.json`)

```json
{
  "active_file": "src/main.rs",
  "cursor_line": 42,
  "local_symbols": ["my_function", "MyStruct"]
}
```

## IDE Extension Responsibilities

To enable high-fidelity speech-to-code, the IDE extension SHOULD:

1. Update `.vox/ide_state.json` whenever the active editor changes or the cursor moves significantly (debounce recommended).
2. Set the `VOX_ACTIVE_FILE` environment variable when spawning `vox oratio` as a subprocess.
3. (Optional) Provide a list of visible symbols in the current viewport in `local_symbols`.

## Intent Biasing Rules

The deterministic classifier applies the following biases based on context:

- **Pronoun Resolution**: "edit this" → `CodeEdit` intent if `active_file` is set.
- **Error Grounding**: "fix the error" → `CodeEdit` intent if `recent_errors` is non-empty.
- **Keyword Boosting**: If a transcript token matches a word in `recent_errors` or `local_symbols`, the `CodeEdit` confidence is boosted to `0.88+`.

## Related Documents
- [Vox Oratio: Speech-to-Code Findings 2026](vox-oratio-speech-to-code-findings-2026.md)
- [Vox Oratio: Intent Classification Research 2026](research-index.md)

