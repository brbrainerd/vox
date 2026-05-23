---
title: "Superpowers SSoT"
description: "Single Source of Truth for Vox Superpowers (Procedural Agentic Skills)."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Defines the agentic framework for structured development workflows."
---
# Superpowers (Procedural Agentic Skills)

Superpowers are high-level, structured procedural workflows that enforce disciplined methodologies across the Vox agentic network. Unlike basic tool calls, Superpowers are **multi-phase execution graphs** that orchestrate multiple agents and tools to achieve complex engineering goals.

## Core Philosophy

1.  **Planning-First**: No Superpower may execute code without a verified and approved implementation plan.
2.  **Verification-Locked**: Completion is only granted when automated tests (TDD) and architectural audits pass.
3.  **Cross-Agent Orchestration**: Skills like `Research` and `Review` can trigger handoffs between specialized agents (e.g., Lane G for research synthesis).

## The 14 Standard Superpowers

| Skill | Category | Description |
|---|---|---|
| **Brainstorm** | Strategic | High-level ideation and problem space exploration. |
| **Specify** | Strategic | Formalization of requirements into XML/Markdown specs. |
| **Plan** | Strategic | Architecture of the execution graph and task decomposition. |
| **TDD** | Execution | Red-Green-Refactor loop implementation. |
| **Debug** | Execution | Root-cause analysis and automated repair cycle. |
| **Refactor** | Execution | Improving structure without behavior change. |
| **Review** | Verification | alignment check against specifications and lint rules. |
| **Research** | Strategic | Autonomous context gathering from web/local corpora. |
| **Mockup** | Execution | Browser-based UI prototype generation. |
| **Audit** | Verification | Security and architectural boundary validation. |
| **Document** | Execution | Synchronization of implementation and documentation. |
| **Optimize** | Execution | Performance profiling and hotspot tuning. |
| **Sync** | Strategic | VCS management and conflict resolution. |
| **Deploy** | Execution | build, test, and release orchestration. |

## Integration Wiring

- **SkillRegistry**: Tracks installed Superpowers and their associated prompts/instructions.
- **Orchestrator**: Injects Superpower-specific continuation instructions into the agent context.
- **GUI Discovery**: The `SuperpowersCatalog` provides a visual interface for activating and configuring these workflows.

## Safety & Guardrails

- **Budgeting**: Superpowers are subject to financial and attention budgets.
- **Rollout**: Experimental Superpowers use a rollout percentage (default 0%) until hardened.
- **Shadow Mode**: Superpowers can run in observation mode to collect telemetry without mutating the repository.
