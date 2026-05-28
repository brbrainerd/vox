---
title: "Vox Developer User Journeys: Intent vs. Actualization"
description: "Baseline target workflows mapping how real human developers interface with the Vox orchestrator system."
category: "Architecture SSOTs"
status: "research"
last_updated: "2026-04-05"
training_eligible: false
training_rationale: "Synthesizes architecture constraints and findings for implementation waves."

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Vox Developer User Journeys: Intent vs. Actualization

This document records the baseline target workflows for the Vox orchestrator. As Vox seeks to differentiate itself from simple autocomplete plugins and fully autonomous isolated workers (e.g., Devin, RooCode, Cursor Composer), we must map out how real human developers will *actually* interface with the system.

## The 2026 Developer Landscape

To build the ultimate AI developer tool, we evaluated the current landscape of AI-native programming. Research reveals developers are shifting from "writers of syntax" to "directors of workflows," relying on multi-agent pipelines and iterative co-creation.

Modern tools divide into three dominant usage patterns:

1.  **Editor-Centric Iteration (e.g., Cursor Composer, Windsurf)**
    *   *Philosophy:* Deep IDE integration where the model maintains context over multiple files but requires constant human steering.
    *   *Workflow:* "Vibe Coding" where developers describe features, the AI drafts the multi-file implementation, and the human reviews and refines iteratively.
    *   *Common Tasks:* Local refactoring, boilerplate generation, translating logic, unit test scaffolding.

2.  **Autonomous Sandboxed Execution (e.g., Devin, OpenHands)**
    *   *Philosophy:* Full autonomy. The AI operates in a sandboxed VM with its own shell and browser.
    *   *Workflow:* The developer assigns a ticket or high-level issue; the agent plans, executes shell commands, runs tests, fixes its own errors, and eventually submits a PR.
    *   *Common Tasks:* Backlog elimination, legacy dependency upgrades, bug hunting via stack traces.

3.  **Task-Centric Lifecycle (e.g., GitHub Copilot Workspaces)**
    *   *Philosophy:* Bound to the project management lifecycle.
    *   *Workflow:* Transforming an issue description directly into a spec, plan, and pull request entirely within the browser.
    *   *Common Tasks:* Team collaboration, architectural specification drafting, PR review automation.

## Core Vox User Journeys

Vox aims to be an ultimate, integrated AI tool. This requires unifying the best aspects of the Editor-Centric and Agent-Centric models. Unlike Python or Rust, Vox has an onboard model suite (`vox populi`) and orchestrator (`vox-orchestrator`), allowing us to enforce invariants natively.

Here are the primary user journeys the Vox architecture must support:

### Journey A: Architecture to Artifact (Greenfield Generation)
*   **Goal:** Move from a high-level prompt, requirements document, or conversational design session to a typed, compiled Vox application.
*   **The Flow:** The developer engages the orchestrator to rough out boundaries. The orchestrator scaffolds structures, leverages `vox-pm` for dependencies, and writes the tests first (TDD approach). It then implements the logic, continuously verifying against the Vox AST/HIR.
*   **Vox Advantage:** Native compiler integration ensures the orchestrator doesn't hallucinate invalid syntax. It relies on `vox stub-check` to prevent incomplete implementations.

### Journey B: The Deep-Context Refactor
*   **Goal:** Safely migrating or refactoring an entire sub-system across deep file hierarchies.
*   **The Flow:** A developer highlights a module and instructs: "Convert this data access layer to use the new canonical Arca store." The orchestrator creates a `plan.md` file, traces the references, executes the changes in batches, and remediates cascading type errors autonomously.
*   **Vox Advantage:** Deep semantic understanding of the Vox AST prevents "hallucinated connections" and broken imports common when LLMs use standard regex-driven refactors.

### Journey C: Autonomous Root Cause Isolation & Remediation
*   **Goal:** Ingesting a complex crash log or failing test suite, isolating the root cause, and deploying a fix.
*   **The Flow:** The developer pastes a stack trace. The orchestrator spawns background validation processes dynamically, reads the relevant code blocks, formulates a hypothesis, writes an isolation test, implements the fix, and confirms the green build.
*   **Vox Advantage:** Safe, iterative sandbox execution within the repository leveraging the native shell discipline, bounded by the developer's attention budget (`contracts/operations/completion-policy.v1.yaml`).

### Journey D: Multi-Agent Orchestration (Architect vs. Implementer)
*   **Goal:** Utilizing different model classes (e.g., a "reasoning" model for planning, a "fast" model for typing) -> optimize speed and cost.
*   **The Flow:** The user defines a complex feature. Vox's orchestrator first delegates to the Architect agent, which produces a `plan.md`. The Orchestrator then spins up multiple Implementer agents in parallel to handle distinct files, merging the results.
*   **Vox Advantage:** The native `vox-orchestrator` orchestrator natively understands parallel sub-agents and file affinity, unlike traditional single-threaded IDE plugins.

## Identified Gaps & Seeds for Correction

Transitioning from Intent to Actualization reveals several architectural gaps in the current Vox platform that must be remediated. 

### 1. Human-in-the-Loop Erosion
*   **Gap:** When orchestrating large refactors, humans lose track of the systemic changes. If the AI hallucinates a domain boundary, the human misses it.
*   **Correction Seed:** Introduce interactive diff approvals and "stop conditions" for continuous tasks. Integrate live telemetry so developers can visualize agent progress in VS Code without reading raw terminal logs.

### 2. State & Context Persistence
*   **Gap:** "Lost in the middle" syndrome. If a developer pauses a complex Journey C task, the orchestrator loses the working memory tree upon restart. 
*   **Correction Seed:** Migrate from in-memory agent state to the Durable Workflow Journal contract (ADR 019). Ensure `vox-orchestrator` persists long-running tasks as durable resources in SQLite/Arca.

### 3. Shell Discipline vs. Autonomous Sandbox Isolation
*   **Gap:** Agents need to run compile loops (e.g., `cargo check`, `vox test`), but unbounded shell access leads to destructive side effects (e.g., wiping directories accidentally).
*   **Correction Seed:** Formalize the "Vox Execution Sandbox" via an execution policy. Agents must route commands through a safe virtualized terminal layer that auto-rejects destructive patterns, while allowing compilation.

*(Note: The concrete execution steps for addressing these gaps are maintained in the accompanying AI Implementation plan.)*


