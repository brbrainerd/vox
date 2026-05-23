---
title: "Vox Oratio: Speech-to-Code Findings 2026"
description: "Analysis of the deterministic intent bottleneck and roadmap for LLM-backed refinement."
category: "Architecture SSOTs"
status: "legacy"
last_updated: "2026-04-18"
training_eligible: false
training_rationale: "Analysis of speech-to-code gaps and Phase 1/2 roadmap."
archived_date: 2026-04-18
---
# Vox Oratio: Speech-to-Code Journey Analysis and Gaps (2026)

## Overview
Vox Oratio currently provides a pure-Rust, Candle-based speech-to-text (STT) foundation with a deterministic intent classification layer. While functional for basic commands, the "speech to code" journey faces several architectural and UX gaps that limit its utility for complex development tasks.

## Current Implementation (April 2026)
- **STT Engine**: Candle Whisper (pure Rust + Hugging Face weights).
- **Intent Classification**: Deterministic keyword/phrase matching (`routing.rs`).
- **Slot Filling**: Heuristic extraction of paths and symbols via regex/splitting (`speech_intent.rs`).
- **Routing Modes**: `Tool` (immediate execution), `Chat` (session memory), `Orchestrator` (DEI queuing).
- **Acoustic Preprocessing**: PCM f32 normalization and VAD integration.

## Identified Gaps

### 1. Linguistic Fragility (Deterministic Bottleneck)
The current `classify_intent` logic relies on hardcoded phrases like "create a function" or "run test". This fails on:
- **Synonyms**: "add a handler", "spawn a file", "verify the build".
- **Complex Phrasing**: "I'm thinking we should probably refactor the main loop to use a more efficient data structure."
- **Ambiguity**: "change it" (where "it" is context-dependent).

### 2. Lack of IDE Context Awareness
The `vox-oratio` crate operates in a vacuum. It does not ingest:
- **Cursor Position**: "add a parameter here" has no "here".
- **Active File**: "explain this file" requires knowing what is open.
- **Build Diagnostics**: "fix the error" requires access to the compiler output.
- **Project Structure**: Searching for `foo.vox` is heuristic and doesn't use the actual file tree.

### 3. "Code Dictation" vs. "Structural Intent"
Users rarely want to dictate code character-by-character. The current gap is in translating high-level structural intent into precise AST transformations. 
- **Current**: "create function main" -> `CodeCreate` action with `symbol_name: "main"`.
- **Desired**: "add an async function called fetch_data that takes a url string and returns a result" -> Structured AST plan.

### 4. Interactive Correction Loop
ASR (Automatic Speech Recognition) is never 100% accurate. 
- **Gap**: There is no standard UI/UX for "I misheard you, did you mean X?" or "Correction: I said 'vox', not 'box'".
- **Tiering**: The `speech_escalation_recommended` flag exists but isn't deeply integrated into a correction UI.

### 5. Multi-turn State Management
While `RouteMode::Chat` exists, it is currently a simple message log. It lacks:
- **Slot Continuity**: "Create a function." -> "What name?" -> "main." (The system needs to remember it's still in the `CodeCreate` flow).
- **Clarification Cycles**: Active probing for missing slots (`missing_slot_ids`).

## Proposed Improvements & Roadmap

### Phase 1: Contextual Hardening (Near Term)
- **Context Injection**: Update `route_transcript` to accept an `IdeContext` struct (file, cursor, errors).
- **Project Lexicon**: Dynamically bias the Whisper decoder (using `contextual_bias.rs`) with symbol names from the current workspace.
- **Improved Heuristics**: Expand the deterministic phrases using a fuzzy-matching or embedding-based classifier (e.g., `vox-populi` small model).

### Phase 2: LLM Intent Refinement (Mid Term)
- **Local Refiner**: Use a tiny local model (Qwen 0.5B or similar) to map "raw transcript + context" to "structured intent envelope".
- **Direct AST Targeting**: Enhance `ast_mapper.rs` to support precise navigation (e.g., "go to the return statement of the second helper").

### Phase 3: Semantic Dictation (Long Term)
- **Natural Language to AST**: Direct translation of speech to `vox-compiler` HIR/AST nodes.
- **Visual Feedback Loop**: Real-time "ghost text" showing the projected code as the user speaks, allowing immediate "undo/redo" via voice.

## References
- [`crates/vox-oratio/src/routing.rs`](file:///c:/Users/Owner/vox/crates/vox-oratio/src/routing.rs)
- [`crates/vox-oratio/src/speech_intent.rs`](file:///c:/Users/Owner/vox/crates/vox-oratio/src/speech_intent.rs)
- [Oratio IDE Protocol (SSOT)](oratio-ide-protocol-ssot.md)
- [Vox DEI: Orchestrator SSOT](dei-ssot.md)

