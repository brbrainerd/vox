---
title: "ast-token-alignment-2026.md"
description: "Documentation for ast-token-alignment-2026.md."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
training_rationale: "Project architecture context."
archived_date: 2026-04-18
---
# AST-Token Alignment Research 2026

## Objective
Enhance Vox MENS code generation capabilities by aligning raw token streams with AST semantics. This allows the trainer to apply specific loss weighting to critical syntax elements (identifiers, type definitions, control flow) and potentially inject "syntax markers" into the latent space.

## Architecture
1. **Source Mapping**:
   - `vox-compiler` provides a full AST with byte-level `Span` information.
   - `tokenizers` provides a `TokenEncoding` with byte-to-token offsets.
   - Alignment connects `Span(start, end)` -> `Tokens[i..j]`.

2. **Weighting Heuristics**:
   - **Identifiers**: 2.0x weight (correct naming is critical).
   - **Keyword/Syntax**: 1.0x (standard logit).
   - **Type Signature**: 1.5x (interface consistency).
   - **Docstrings/Comments**: 0.5x (prose variance).

3. **Implementation Path**:
   - [ ] Extend `vox_tensor::data::TrainingPair` to include optional `ast_metadata`.
   - [ ] Implement `align_tokens_to_ast` in `vox-populi`.
   - [ ] Update `candle-qlora-train` to accept token weights.

## References
- `crates/vox-compiler/src/ast/`
- `crates/vox-populi/src/mens/tensor/candle_qlora_train/training_loop/encoding.rs`

