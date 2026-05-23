---
title: "HITL Doubt Loop (SSOT)"
description: "Canonical authority document for the HITL doubt system and Resolution Agent."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-10"
training_eligible: false
training_rationale: "Key architecture constraints and definitions required for agent context"

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# HITL Doubt Loop (SSOT)

This is the Single Source of Truth (SSOT) for the Human-In-The-Loop (HITL) Doubt Loop architecture. It defines how autonomous agents express uncertainty, how humans intervene, and how safe skepticism is rewarded.

## 1. Triggering Doubt
Agents request human intervention via the `vox_doubt_task` MCP tool.
- This immediately transitions the task state to `TaskStatus::Doubted`.
- The system fires a `TaskDoubted` event to the `vox-orchestrator` event bus.

## 2. The Resolution Agent
When a `TaskDoubted` event is detected, the `ResolutionAgent` (living in the `vox-dei` crate) takes control.
- It pauses all automated execution streams for the affected task.
- It engages the `FreeAiClient` to assist the human in resolving the ambiguity.
- It tracks the resolution budget via `BudgetManager`.

## 3. Audit Report Format
Upon resolution, the `ResolutionAgent` must submit an audit report.
- The report logs the nature of the doubt, the human's input, and the cost incurred.
- It differentiates between "legitimate ambiguity" and "AI obsequiousness".

## 4. Gamification Hook (`vox-ludus`)
The audit report is sent to the `vox-ludus` gamification crate.
- If the doubt was raised due to detected obsequiousness or true capability gaps (healthy skepticism), the `internal_affairs` achievement trigger is fired.
- The agent earns xp for avoiding hallucination. 

## 5. LML Escalation Path

The HITL doubt loop is also the **terminal escalation state** when the proposed LLM Mediation
Layer (LML) exhausts its repair-loop budget. When `RepairPolicy.max_attempts` is reached without
a valid validated output, the LML calls `vox_doubt_task` on behalf of the current task.

See [research-llm-output-mediation-validation-2026.md](research-llm-output-mediation-validation-2026.md)
§6.3 and §11 (Wave 1) for the design of the repair loop and escalation trigger.


