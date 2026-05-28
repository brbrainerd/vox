---
name: subagent-driven-development
description: Use when executing implementation plans with independent tasks in the Vox Orchestrator
---

# Subagent-Driven Development (Vox Adaptation)

Execute the JSON Plan by dispatching goals concurrently through the Vox Orchestrator (`vox-orchestrator`), with a strict two-stage review: spec compliance, then quality.

**Core principle:** Fresh goal dispatch per task + parallel tool concurrency = high quality, fast iteration.

## The Process

1. **Read Plan:** Orchestrator consumes the `PlanNode` JSON array.
2. **Dispatch Goals:** For each independent task, the orchestrator dispatches a new task campaign.
3. **Execution:** The assigned agent uses parallel tool execution (`multi_replace`, `grep_search`) to fulfill the `PlanNode` requirements.
4. **Verification Gate:** Once the code is written, the agent MUST run the `verification` script defined in the `PlanNode` (e.g., `cargo test` or `vox stub-check`).
5. **Quality Review Gate:** A secondary review step evaluates codebase constraints (TOESTUB blockers, architectural limits).
6. **Continuation:** If the agent idles, the Continuation Engine injects the `active_skill` (e.g., `test-driven-development`) into the prompt.

## Anti-Patterns (NEVER DO THESE)

- **NEVER use Native Sub-Agents:** LLMs generate tokens sequentially. You do not have native autonomous sub-agents. You achieve the "parallel effect" purely via tool-call concurrency (`campaigns.rs` in Vox).
- **NEVER skip the Verification script:** Deduce success via stdout, not mental evaluation.
- **NEVER proceed with un-fixed TOESTUBS:** Review the output of `vox stub-check`.

## Handling Implementer Status

- **DONE:** Proceed to the next `PlanNode`.
- **BLOCKED:** Assess the blocker. If a Circuit Breaker is triggered (e.g., COMPILER LOOP), stop and summarize the failure for the human operator.

## Integration

**Required workflow skills:**
- **superpowers:writing-plans** - Creates the JSON plan this skill executes.
- **superpowers:test-driven-development** - The active skill injected into the agent during execution.
