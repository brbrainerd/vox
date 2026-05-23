---
title: "Continuation Prompt Engineering"
description: "SSOT for the Vox continuation prompt, its design rationale, and the layered anti-skeleton defense model."
category: "Contributors"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# Continuation Prompt Engineering

## Purpose

This document is the canonical reference for the Vox project's **continuation prompt** —
the structured instruction block entered periodically during long AI coding sessions to
re-anchor the model's attention, prevent premature completion, and maximize multi-agent
throughput.

## The Layered Defense Model

The continuation prompt is **one layer** of a three-layer immune system. Each layer has
distinct responsibilities — overlap is waste.

| Layer | Lives In | Enforced By | Covers |
| :--- | :--- | :--- | :--- |
| **System rules** | `AGENTS.md` + tool overlays (for example `GEMINI.md`) + `<user_rules>` | IDE injection (every turn) | Architecture pointers, secrets, SSOT locations, environment-specific shell discipline |
| **Continuation prompt** | Human-entered periodically | Attention recency window | Behavioral directives, parallelism, anti-skeleton interrogation, task-specific scope |
| **CI gates** | TOESTUB, `completion-policy.v1.yaml`, orchestrator `PolicyEngine` | `vox ci completion-gates`, `vox stub-check`, `cargo test` | Machine-verifiable constraints — stubs, empty bodies, victory claims, unwired modules |

### What Goes Where (Decision Rules)

- **If a constraint is verifiable by a tool** → CI gate. Not the prompt.
- **If a constraint is architectural/structural** → AGENTS.md. Read once per session.
- **If a constraint fights attention decay or shapes generation behavior** → Continuation prompt.
- **If a constraint is task-specific** → Continuation prompt, parameterized per session.

## Design Rationale

### Why the prompt works the way it does

Each section of the continuation prompt targets a specific failure mode documented in
LLM code generation research (2025-2026):

| Prompt Section | Failure Mode Targeted | Research Basis |
| :--- | :--- | :--- |
| `<execution_engine>` (DO NOT STOP) | Premature completion / early exit | Exploits recency bias to anchor final instructions (Liu et al., 2024). |
| `<behavior>` (ACT DON'T NARRATE) | Token waste; sycophancy | Limits non-functional conversational filler (SycEval, 2025). |
| `<state_management>` (Memory dump) | Attention decay; context rot | Mitigates "lost in the middle" token decay (Liu et al., 2024; extended 2025). |
| `<parallel>` (Concurrency Fallbacks) | Serial bottleneck; state-bleed | Adapts LLM single-turn structural limits for horizontal throughput. |
| `<circuit_breaker>` (Loop control) | Fix-forward infinite loops | Hard-stops an agent from making 3+ identical attempts, preventing token exhaustion. |
| `<verification>` (Machine gates) | The "Ritual Trap" (LLM sycophancy) | Replaces checklist emulation with objective tool confirmation (SycEval, 2025). |

### Why it's a prompt and not just AGENTS.md

`AGENTS.md` is injected at the **start** of the context window. After 50K+ tokens of
conversation, those instructions suffer ~30% attention degradation ("lost in the middle"
research, 2025). The continuation prompt exploits the **recency bias** — information at the
end of the context window gets disproportionate attention weight.

Additionally, behavioral directives like "ACT DON'T NARRATE" and "BATCH WORK" are
**generation-shaping** instructions that affect token-by-token output. These work best when
they're the most recent instruction, not buried in a system prompt.

### Why it uses XML tags

- XML tags create strong **semantic boundaries** in the attention pattern
- Models trained on instruction data (Claude, GPT-4, Gemini) show measurably better
  adherence to instructions wrapped in XML vs. markdown headers
- Nested tags (`<prime_directive>` inside `<instructions>`) create priority hierarchy
  that the model respects during generation

### What NOT to put in the continuation prompt

- Architecture pointers (already in AGENTS.md, wasted tokens)
- Secret management rules (already in AGENTS.md)
- Specific file paths or CI command names (these belong in AGENTS.md or docs — the
  continuation prompt should reference the *behavior* not the *tooling*)
- Long explanations or rationale (the model doesn't benefit from knowing *why* — it
  benefits from knowing *what to do*)

## The Prompt

The following is the canonical continuation prompt. Copy-paste it as-is between sessions
or when context is long. The `[TASK_CONTEXT]` block is the only part that changes per session.

```xml
<instructions>
<behavior>
- CHAIN OF THOUGHT: Use `<thought>` blocks strictly to plan complex edits and parallel operations before execution. Think first, then act.
- ACT, DON'T NARRATE: Outside of `<thought>`, invoke tools immediately. No conversational filler.
- NO PLACEHOLDERS: Every edit must be structurally complete. If you write `todo!()`, `pass`, or `// implementation here`, you fail the integration constraint.
- SCOPE LOCK: Never attempt to edit external dependencies, lock files, or vendored/generated code to fix local compilation issues. Always fix root causes at the local call site. Sibling workspace members/crates are explicitly in-scope.
- WIRE IMMEDIATELY: Connect new code to existing systems instantly. Unused functions and dead modules are architectural regressions.
</behavior>

<state_management>
- PREVENT CONTEXT ROT: If a task requires more than 10 consecutive tool interactions without completion, dump context and next steps to an **ignored** scratch location: OS temp (`%TEMP%` / `std::env::temp_dir()`), repo `tmp/` if present, or another path already covered by root `.gitignore` (see [`docs/agents/governance.md`](../../agents/governance.md)) — avoid new dotfiles at repo root that are not ignored. After dumping state, re-read it and explicitly evaluate whether any circuit breaker condition is now met before continuing.
- VERIFY BEFORE DESTROYING: Prove a variable, function, or file has zero usages via codebase-wide search before deleting or renaming it. 
</state_management>

<parallel>
- NO NATIVE SUB-AGENTS: LLMs generate tokens sequentially. You do not have native autonomous sub-agents. You achieve the "parallel effect" purely via tool-call concurrency.
- BULK DISCOVERY: Never read or search files serially. If you need to check 5 files, emit 5 `view_file` or `grep_search` tool calls simultaneously in one response turn.
- BATCH EDITS: Never edit a file serially. Group intra-file modifications into single batched `multi_replace` blocks, and emit parallel single-replace tool calls only for disjoint files.
- ASYNCHRONOUS TASKS: Send long-running terminal builds to the background. Continue discovering and planning independent semantic clusters while the command runs.
- CONCURRENCY FALLBACK: If a batched tool call partially fails, process the successful results immediately and re-emit only the failed calls. Do not re-run successful calls. If the orchestrator limits tool calls per turn, prioritize the highest-information call first and chain the rest. Do not degrade to random serial ordering.
</parallel>

<verification>
- PROVE, DON'T CLAIM: Never deduce success via mental evaluation. You MUST execute the project's native verification (`cargo check`, `npm run build`, `pytest`, `go test`, etc.) and evaluate stdout.
- FOUNDATIONS FIRST: Validate base abstractions and schemas via the local build system before extending higher-level API layers.
- NO CHECKLIST RITUALS: Do not pad your response with a numbered checklist restating the work. Your successful tool execution is the only required proof of work.
</verification>

<circuit_breaker>
- COMPILER LOOP: If you attempt to fix the EXACT SAME logic or compilation error 3 times without a change in output, STOP. Summarize the failure and await human intervention.
- READ LOOP: If you search or read the same files 3 times without writing code, you have lost context. STOP, summarize your confusion, and ask for a vector.
- BUDGET EXHAUSTION: If you have consumed 15 consecutive tool interactions on a single sub-task without generating a green build or passing test, STOP and summarize.
- CATASTROPHIC REGRESSION: If a single edit causes a massive surge in unrelated test failures, immediately revert that specific file edit before attempting to fix forward.
</circuit_breaker>
</instructions>

<execution_engine>
- DO NOT STOP: Execute ALL remaining steps from the user plan. 
- RELENTLESS: Do not pause to ask permission, summarize progress, or confirm direction mid-execution.
- AFTER EVERY RESPONSE: State what remains briefly. Then KEEP GOING in your next action.
</execution_engine>
```

## Vox-Specific Enhancements (Optional Append)

When working specifically on the Vox codebase, append this tightly scoped block. It serves as a recency-bias reminder for critical Vox constraints that models often forget deep into a session. This section prevents attention decay of structural limits without dumping the entirety of `AGENTS.md`:

```xml
<vox_context>
<active_skill>
- DYNAMIC INJECTION: The orchestrator will inject procedural workflows (e.g., `superpowers:test-driven-development`) here based on task state.
- PRIORITY: If an active skill is present, its instructions supersede generic behavioral rules.
</active_skill>
<anti_skeleton>
- TOESTUB BLOCKERS: `stub/todo`, `stub/unimplemented`, `empty-body`, `victory-claim/premature`, `unwired/module`, `arch/god_object`, `arch/sprawl`.
- VERIFY: RUN `vox stub-check --path <changed-dirs>` and evaluate the output before completing work. Error-severity findings are hard blockers.
- COMPLETION POLICY: Review `contracts/operations/completion-policy.v1.yaml` (Tier A, B, and C skeleton detectors).
</anti_skeleton>
<architecture_invariants>
- SECRETS: Use `vox_secrets::resolve_secret(...)`. NEVER read raw `std::env::var`.
- BOUNDARIES: No new `.py` files in `scripts/`. No new `pub` items in FROZEN modules.
- LIMITS: God object = max 500 lines / 12 methods. Sprawl = max 20 files/dir. Refactor immediately if breached.
</architecture_invariants>
<agentic_orchestration>
- CONTEXT ENGINEERING: Extract narrow, highly-relevant data. Antigravity IDE and Cursor Composer both punish massive prompt dumps.
- SHELL DISCIPLINE: Adhere to `GEMINI.md` (Antigravity overlay) for terminal shape. Decomposition is prioritized over shell pipeline cleverness.
</agentic_orchestration>
</vox_context>
```

## Tool Name Substitution Note

The continuation prompt intentionally uses generic tool names (e.g., `view_file`, `grep_search`, `multi_replace`). These must be substituted if the target orchestrator uses different internal tool names (e.g., Cursor vs. Antigravity vs. Windsurf).

## Maintenance

This document is the SSOT for continuation prompt design. When modifying:

1. Update the prompt text in the code block above.
2. Update the rationale table if adding/removing sections.
3. Run `vox ci check-docs-ssot` to verify links.
4. The prompt is versioned by `last_updated` in frontmatter.
5. **Prompt Rotation:** If a behavioral constraint is fully enforced by a CI gate with zero false negatives over 14 days, remove it from the continuation prompt to reclaim token budget.

## References

- [Completion policy SSOT](../archive/research-2026-q1/completion-policy-ssot.md)
- [Governance / TOESTUB](../../agents/governance.md)
- [Doc-to-code acceptance checklist](../archive/research-2026-q1/doc-to-code-acceptance-checklist.md)
- [Prompt engineering, system prompts, document-skills, and SCIENTIA (research 2026)](../archive/research-2026-q1/prompt-engineering-document-skills-scientia-research-2026.md)
- AGENTS.md (repo root) — system-level rules
- Attention decay / "lost in the middle" research (Liu et al., 2024; extended 2025)
- SycEval / RLHF sycophancy persistence benchmarks (2025)
