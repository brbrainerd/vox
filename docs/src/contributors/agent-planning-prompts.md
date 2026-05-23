---
title: "Agent Planning Prompts"
description: "Prompt-engineered system directives for chunked file writing to avoid LLM token exhaustion."
category: "Contributors"
status: "current"
last_updated: "2026-04-18"
training_eligible: true

schema_type: "TechArticle"
---

# Agent Planning Prompts

## Purpose

When running agentic systems (like Cloud Sonnet or Gemini) in planning mode, models often exhaust their output token limits (e.g., 4,096 to 16,384 tokens) when attempting to output highly complex implementation plans directly into the chat.

This document provides prompt-engineered directives to force the agent to stream its plan iteratively into a temporary file on disk, rather than relying on the chat interface.

## Multi-Tab Planning Considerations

When working with multiple agents or tabs simultaneously, hardcoding a file name like `PLAN.md` at the repository root will cause file-locking issues or state-bleed.

To resolve this, the prompt instructs the agent to create a scoped file path in the `tmp/plans/` directory (e.g., `tmp/plans/plan-<task_id>.md` or `tmp/plans/<tab_context>.md`). The `tmp/` directory is covered by `.gitignore` natively, ensuring no artifact collision in version control.

## The Chunked Planning Prompt

Copy and paste this block into the agent's system prompt or as a pre-prompt instruction before initiating a complex planning task:

```xml
<instruction_override>
<primary_directive>
You are generating a massive, highly complex implementation plan. You MUST NOT attempt to output the full plan in your conversational response, as it will exceed your output token limit and crash the session.

Instead, you will construct the plan iteratively by writing it directly to the filesystem in chunks.
</primary_directive>

<execution_strategy>
1. INITIALIZATION: First, use your file creation tools to create a unique file for this plan. Place it in `tmp/plans/` and give it a context-specific name (e.g., `tmp/plans/plan-<feature-name>.md`) to avoid clashing with other parallel sessions. Initialize it with the top-level architecture headers only.
2. CHUNKED GENERATION: Break the planning into logical phases (e.g., Phase 1: Database, Phase 2: Backend, Phase 3: UI). 
3. FILE APPENDING: For each phase, generate the detailed plan and use your file editing tools to append the content to your scoped plan file.
   - *If your file tools do not natively support append, use your terminal tool to run `echo "..." >> tmp/plans/<filename>.md` (or PowerShell `Add-Content` if on Windows).*
4. STATUS UPDATES ONLY: Your conversational responses to the user should be extremely brief (less than 50 words). Only report: "Phase [X] complete and written to <filename>. Proceeding to Phase [Y]."
5. YIELDING: After writing a chunk to the file, if you feel you are approaching your output limit, gracefully end your turn by stating: "Yielding turn to prevent token exhaustion. Awaiting user approval to continue."
</execution_strategy>

<behavioral_constraints>
- CHAIN OF THOUGHT: Use `<thought>` blocks to outline the specific chunk you are about to write to the file. Keep the thought block under 500 words.
- ACT, DON'T NARRATE: Never print the contents of the plan in the chat. The chat is strictly for telemetry and status updates. The source of truth is your generated markdown file.
- RELENTLESS ITERATION: Do not ask for feedback between phases unless you encounter a blocking ambiguity. Immediately proceed to write the next chunk to the file.
</behavioral_constraints>
</instruction_override>
```

## Maintenance

When updating prompt behavior to address new LLM context window behaviors or system limits, update this document and bump `last_updated`.
