---
title: "Compiler Testing Research Synthesis"
description: "Property-based testing and oracle design for compiled language implementations."
category: "Architecture SSOTs"
status: "research"
research_source: "gemini_deep_research"
research_date: "2026-04-08"
training_eligible: false
last_updated: "2026-04-08"
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Compiler Architecture Verification & Oracles

## 1. Context
Methodologies for validating an LLM-targeted, strongly-typed statically compiled DSL (Vox language), specifically focusing on Property-Based Testing (PBT), snapshot depth, and Oracle frameworks for LLM test generation.

## 2. Empirical Findings & Tradeoffs

### Proptest vs. Quickcheck for ASTs
- **Quickcheck** (Stateless, Trait-bound) has massive input-rejection rates when generating recursive algebraic datatypes (like ASTs).
- **Proptest** (Stateful Strategies) is mandatory for AST coverage due to its capability for deterministic shrinking of massive, complex syntax trees.

### Snapshot Brittleness
- Deep snapshotting (capturing AST, HIR, and Codegen files for every test) induces unmanageable developer friction during early syntax iteration.
- Shallow UI snapshotting (stderr/stdout) normalized for paths is highly stable, but obscures exact optimization layer regressions.

### The LLM "Oracle Problem"
- Relying on LLMs to generate *both* the complex fuzzing input and the expected assertion (the Oracle) for an undocumented, custom DSL yields an unacceptable false-positive rate (hallucination).
- Pure Grammar Fuzzers reliably find parser crashes but fail to exercise the middle-end because their outputs rarely pass polymorphic type-checkers.

### Mutation "Arid Nodes"
- Performing source-level mutation creates noise. IR-level mutation testing generates "Arid Nodes" (e.g., mutating a debug logging statement), causing developer trust to plummet.

## 3. Validated Architectural Adjustments (4 Waves)

1. **Wave 1 (Boundary Defense):** Implement shallow, normalized UI snapshot tests. Enforce the primary parser invariant: `parse(unparse(ast)) == ast`.
2. **Wave 2 (Frontend PBT):** Deploy the `@forall` macro backed by the `proptest` framework to strictly enforce structural boundaries via stateful recursive shrinking.
3. **Wave 3 (Semantic Contracts & MRs):** Integrate lightweight `@spec(requires, ensures)` block constraints. These act as runtime assertion oracles (not SMT blockings), sidestepping the LLM Oracle problem.
4. **Wave 4 (Differential Fuzzing):** Use LLVM IR-layer equivalents (mutation on arithmetic/relational operators). Filter mutation operators strictly away from standard-out/logging paths to prevent Arid Node rejection.


