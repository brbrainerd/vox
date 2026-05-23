---
title: "Research: Fuzzy & Partial Parsing"
description: "Evaluation of resilient parsing strategies for incremental LLM-driven code generation."
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Research: Fuzzy & Partial Parsing for Iterative LLM Generation

**Date:** April 2026  
**Status:** Emerging (Wave 12 Foundation)  
**Context:** Optimizing the inner loop of LLM-native development

## The Problem: Binary Failure in Classic Parsers
Traditional compilers operate on a "green/red" binary. If a file has a single missing brace at the end, the entire AST is lost. For LLMs, which often generate code incrementally (streamed) or stop prematurely due to context limits, this binary failure destroys the feedback loop.

## The Vox Strategy: Resilient ASTs

### 1. Partial Skeletons
The Vox recursive-descent parser (0.4) is being hardened to emit a "Skeleton AST" even under parse failure. 
- **Graceful Termination:** If EOF is reached inside a block, the parser "synthetically" closes the block and markers the resulting node as `stub/eof-terminated`.
- **Diagnostic Anchoring:** Diagnostics are attached to the partially formed nodes, allowing the LLM to see *where* the parser lost track without discarding the preceding 90% of valid code.

### 2. Fuzzy Token Matchers
Lexing in Vox 0.4 now supports "Phonetic Similarity" for keywords.
- **Intent Detection:** If an LLM emits `compnent` instead of `component`, the lexer identifies the high-probability intent and emits a `Warn` instead of an `Error` (enabled only in `mens-training` mode).
- **Benefit:** Reduces "stupid" hallucination failures that would otherwise trigger a full re-generation cycle.

### 3. Incremental Verification
- **AST Eval:** Integrating the parser into `vox-eval` (Wave 8) allows for verifying *expressions* as they are generated, even if the surrounding *module* is yet incomplete.
- **Micro-Feedback:** Provides the model with a "Self-Correction Gate" at the statement level.

## Future Work (Wave 13)
- **Probabilistic Grammars:** Integrating the `vox-grammar-export` crate with constrained decoding engines (e.g., Guidance, Outlines) to prevent syntax errors entirely at the sampling layer.

## References
- `vox-grammar-export/README.md`
- `parser/descent/mod.rs`
- `research-grpo-ast-reward-hacking-2026.md`

