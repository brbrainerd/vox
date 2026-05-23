---
title: "Human-In-The-Loop & Doubt"
description: "User-facing reference explaining the Doubt control mechanism and human resolution flow."
category: "Language Reference"
status: "current"
last_updated: "2026-04-10"
training_eligible: true

schema_type: "TechArticle"
---

# Human-In-The-Loop (HITL) & Doubt

*For the architectural SSOT on this topic, see [hitl-doubt-loop-ssot.md](../archive/research-2026-q1/hitl-doubt-loop-ssot.md).*

Autonomous agents in Vox are designed to be confident when they have necessary context, but to express **doubt** when faced with ambiguity, destructive actions, or low-information environments. The Doubt control mechanism is the cornerstone of this Human-In-The-Loop alignment.

## What is Doubt?

Doubt is an explicit state a task can enter (`TaskStatus::Doubted`). It is triggered when an agent calls the `vox_doubt_task` MCP tool instead of blindly making assumptions. 

Common triggers for doubt:
- Conflicting requirements in a prompt.
- Insufficient permissions to execute a discovered tool.
- Ambiguous codebase architecture that requires a design decision.
- Potential destructive execution paths (like data deletion).

## The Resolution State Machine

1. **Detection**: The primary agent identifies ambiguity and invokes `vox_doubt_task`.
2. **Suspension**: The orchestrator pauses the agent's active execution threads and transitions the task to `TaskStatus::Doubted`.
3. **Resolution**: The `ResolutionAgent` engages. It presents the context to the human operator using the `FreeAiClient` or editor overlays, asking for clarification.
4. **Resumption**: Once the human provides the necessary context or authorization, the doubt is marked resolved, and the primary agent resumes execution with the new constraints.

## Rewarding Healthy Skepticism

To combat AI obsequiousness (the tendency to always say "yes" even when wrong), the system actively rewards the choice to doubt. 

When the `ResolutionAgent` concludes a doubt session, it submits an audit report. If the doubt was raised due to genuine ambiguity rather than simple capability failure, it triggers an `internal_affairs` achievement in the `vox-gamify` gamification engine. This reinforces a behavior model where safe, clarified execution is paramount.


