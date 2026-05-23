---
title: "Unified Agentic Control Surface Research"
description: "Synthesis of Human-in-the-Loop (HITL) steering, 'Second Pass' reflection patterns, and the integration of Vox's existing control concepts into a unified Tri-State Pilot Console."
category: "Architecture SSOTs"
status: "research"
sort_order: 5
last_updated: "2026-04-12"
training_eligible: false
training_rationale: "Survey of HITL steering and control surface patterns valuable for agent decision-making training examples."
schema_type: "TechArticle"

archived_date: 2026-04-18
---

# Unified Agentic Control Surface Research (April 2026)

## Overview

This research document synthesizes industry standards for Human-in-the-Loop (HITL) steering, the "Reflection Pattern" (Self-Reflection and Verification), and how these concepts map to and unify Vox's existing ecosystem constraints. The goal is to provide a single, unified mental model for the "Pilot Console"—the primary interface through which a human orchestrates the AI system.

This document builds upon previous research, specifically the [L.A. Noire Doubt Metaphor](hitl-doubt-loop-ssot.md) and [Continuation Prompt Engineering](../contributors/continuation-prompt-engineering.md).

## Core Concepts & Industry Alignment

### The "Reflection Pattern" (Generate-Validate-Reflect)

Modern autonomous coding agents (e.g., LangGraph, smolagents, OpenHands) rely heavily on a cyclical reasoning process:

1.  **First Pass (Generate):** The agent generates an initial attempt based on the intent (starter prompt).
2.  **Validator (Test):** An automated execution environment or linter runs against the generated output to gather ground truth.
3.  **Second Pass (Reflect):** The agent ingests the error logs or validation failures, acting as a debugger to refine its initial attempt.

The "Second Pass" is where reliability jumps from simple text prediction to robust software engineering.

### Human-in-the-Loop (HITL) Steering 

Effective HITL shifts control from *micro-management* to *delegation and oversight*. The control surface must allow humans to define goals, monitor progress, inject suspicion, and halt the system.

## Unifying Vox's Control Surface: The Tri-State Pilot Console

We must distill Vox's various control vectors (Starter Prompts, Planning Prompts, Continuation Prompts, Suspicious/Doubt signals, validation rules, and Stop commands) into the smallest possible cognitive footprint for the operator. 

We propose the **Tri-State Pilot Console**:

### State 1: Strategic Thrust (Launch & Steer)

This is the system's forward momentum. The human defines *what* to do and *keeps the agent moving*.
*   **Concepts Unified:** Starter Prompt, Planning Prompts, Continuation Prompts.
*   **Behavior:** The agent is operating in "Generation" mode (First Pass). The UI focuses on delegation.
*   **Implementation:** The [Continuation Prompt](../contributors/continuation-prompt-engineering.md) acts as the engine oil here, injected periodically to prevent context rot and enforce parallel bulk actions.

### State 2: Reflective Interrogation (Doubt & Audit)

This state resolves the conflict between the [L.A. Noire "Doubt" metaphor](hitl-doubt-loop-ssot.md) and the "Second Pass Verification." **They are the same action.**
*   **Concepts Unified:** L.A. Noire "Suspicious" / "Doubt", Second Pass Validator, Socrates Output-Evaluation.
*   **Behavior:** When the operator presses "Doubt" (or the system self-triggers doubt due to low Socrates scores), the orchestrator *pivots* rather than halting. It shifts from generation to **Reflective Validation**.
*   **The Action:** The agent explicitly queries the codebase to verify its own recent diffs, runs tests, and applies hallucination checks. 
*   **UI Representation:** Amber heartbeat/pulse. The human says, "I don't trust this," and the machine does the hard work of proving it.

### State 3: Circuit Breakers (Halt)

Immediate, non-negotiable stoppage.
*   **Concepts Unified:** Stop command, Budget Exhaustion, Catastrophic Regression.
*   **Behavior:** Execution halts entirely. The human must intervene to unblock the loop.
*   **Implementation:** Red friction UI. Halts the orchestrator's event loop.

## Design Decisions: Unifying "Doubt" and "Second Pass"

Historically, Vox treated "Suspicious" (a vague human feeling) and "Improve/Audit" (a concrete action) as separate. Industry research strongly suggests they should be linked.

If the human interface provides a "Doubt" button, it should automatically trigger the "Second Pass" reflection loop. The system should switch models (e.g., to a high-reasoning tier), ingest its own output, and execute the local test verification `vox ci check`.

By unifying these, we minimize the UI options for the controller while maximizing the automated response to human intuition.

## Actionable Guidelines

1.  **Reduce Buttons:** The UI should primarily feature elements that map cleanly to Start/Continue, Doubt (Verify), and Stop. 
2.  **Expose Confidence (Socrates):** To guide the manual "Doubt" action, the UI should surface the latent Socrates heuristic score so the operator knows *when* to be suspicious before bugs compound.

## References
*   [L.A. Noire Doubt Metaphor](hitl-doubt-loop-ssot.md)
*   [Continuation Prompt Engineering SSOT](../contributors/continuation-prompt-engineering.md)


