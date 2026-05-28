---
title: "TOESTUB line limit and MENS corpus size research (2026)"
description: "Investigation into Vox's actual TOESTUB God Object limits versus documentation, and research into optimal code file chunking for Qwen3-4B MENS training."
category: "Architecture SSOTs"
status: "research"
sort_order: 18
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Documents God Object limits and chunking strategy directly applicable to MENS QLoRA training corpus curation."
schema_type: "TechArticle"

archived_date: 2026-04-18
---

# TOESTUB line limit and MENS corpus size research (2026)

## Executive Summary

There is a significant divergence between Vox's documented "God Object" policy and the actual runtime enforcement. While `AGENTS.md` and `docs/agents/governance.md` strictly assert a 500-line hard cap, the `vox-toestub` compiler engine silently raised this limit to **1,700 lines** in Q1 2025 to accommodate legacy crates.

Simultaneously, we must define an ideal file size target that balances human maintainability with the **MENS synthetic training pipeline**, particularly fine-tuning target models like **Qwen3-4B**. Our research indicates that while modern context windows are massive, **supervised fine-tuning (SFT) and RAG density** perform optimally at much smaller code granularities (50-200 tokens per chunk or ~300-500 lines per file).

## 1. The TOESTUB Discrepancy

### Documented Policy
- **`AGENTS.md` / `governance.md`:** "God Object Limit: Maximum 500 lines or 12 methods per struct/class. Refactor into domains before adding logic."

### Actual Codebase Enforcement (`crates/vox-toestub/src/detectors/god_object.rs`)
- **`max_lines`: 1700**
- **`max_methods`: 38**
- *Rationale (from source comment):* "TOESTUB remediation (2025-Q1): raised from 500 — several first-party crates (integration tests, CLI publication, MCP dispatch) legitimately exceed 500 non-blank lines until phased splits land."

**Conclusion:** The 300 (soft) → 400 (warning) → 500 (hard) threshold does not exist in code. The system fails silently on files between 500 and 1,699 lines.

## 2. LLM Context Research: Qwen3-4B and MENS Pipeline

When designing our line limits, we must consider how the code is digested by the MENS QLoRA / DPO pipeline.

### Model Architecture: Qwen3-4B
- **Parameters:** ~4.0 Billion (3.6B non-embedding)
- **Architecture:** Dense Transformer with Grouped Query Attention (GQA).
- **Native Context Window:** 32,768 tokens (extensible to 131k via YaRN scaling).
- **Training Data:** Pretrained on over ~36 Trillion tokens (Qwen3) / 5.5T+ tokens (Qwen2.5-Coder series), combining high-quality STEM, GitHub repos, and synthetic data.

### SFT & Chunking Best Practices (2025/2026)
While models like Qwen3-4B can technologically ingest a 1,700-line file (~10,000 to 15,000 tokens depending on density), this is an **anti-pattern for Supervised Fine-Tuning (SFT) and RAG**:

1. **Context Density / Lost-in-the-Middle:** Providing large 1,700-line blobs dilutes the attention mechanism. If the MENS training objective is to teach the model a specific Rust trait implementation or a Vox behavior, surrounding it with 1,200 lines of unrelated integration test boilerplate reduces semantic convergence.
2. **Optimal SFT Granularity:** Industry standard practice favors **function-level or class-level chunking**.
   - Ideal chunk size: **50–200 tokens** for high-precision retrieval.
   - Ideal file size: **300–500 lines** (roughly 1,500 – 4,000 tokens). This represents a contiguous block of logic small enough that the LLM can maintain full attention density across the entire file during generation.
3. **SOTA Data Preparation:** Frameworks like StarCoder2 and DeepSeek-Coder filter out extreme bloat (e.g., files with >100,000 lines or >100 chars/line average). However, for *fine-tuning* code intelligence as opposed to *pre-training*, brevity and single-responsibility principles massively improve the model's ability to learn coding patterns.

## 3. Recommendations for the Ideal Limit

To align the Vox repository's architecture with the MENS training flywheel and human cognitive load, we propose resetting the TOESTUB limits:

### Proposed Multi-Tier Threshold (The "Ideal Limit")
Instead of a binary pass/fail at 1700 lines, we should implement a graduated penalty system in TOESTUB:

- **Soft Limit (300 Lines):** `Info` (or Ludus XP penalty). Triggers a prompt to consider trait extraction.
- **Warning Threshold (400 Lines):** `Warning` severity. MENS crawler marks these files as "low density" context for training.
- **Hard Limit (500 Lines):** `Error` severity (Blocks CI entirely, reverting to the documented `AGENTS.md` constraint). Restoring the 500-line limit guarantees that any file fed into the Qwen3-4B pipeline remains under ~4,000 tokens—the sweet spot for dense attention and logical isolation.

### Remediation Path
To enact this without breaking the build:
1. We must introduce a `#[toestub(ignore_god_object)]` suppression or a blessed `.toestubignore` list specifically for the existing legacy files like `orchestrator.rs` (70 KB) and `memory.rs` (31 KB).
2. Revert `max_lines` back to 500 and `max_methods` back to 12 in `vox-toestub/src/detectors/god_object.rs`.
3. Inform the MENS pipeline `ast_mutator` to slice files larger than 150 lines into AST-bounded chunks (functions/impls) rather than treating the file as a single training row.


