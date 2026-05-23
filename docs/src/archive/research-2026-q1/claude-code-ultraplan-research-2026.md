---
title: "Research: Claude Code Ultraplan Architecture"
description: "Synthesis of Anthropic's ultraplan methodology for low-hallucination agentic agent execution."
category: "Architecture SSOTs"
status: "research"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# Claude Code Ultraplan — Research Findings (April 2026)

> Status: Research-only. No implementation committed. Findings inform Vox DEI orchestrator and planning mode development.
> Author: AI research synthesis (Antigravity)
> Date: 2026-04-08

---

## 1. What Is Ultraplan?

Claude Code **Ultraplan** (GA'd in early April 2026, requiring v2.1.91+) is a planning-mode variant that offloads the heavy planning step from the user's local terminal to a dedicated remote **Cloud Container Runtime (CCR)** session managed by Anthropic. It is not a separate product — it is a modality within the Claude Code agentic harness activated by `/ultraplan`, a keyword trigger, or by converting an in-progress local plan.

The core design thesis is that **planning is the hardest part of agentic work**, and it should not be blocked on local resources, terminal occupancy, or context-window size. Planning deserves its own compute budget, asynchronous lifecycle, and richer review surface.

archived_date: 2026-04-18
---

## 2. Architecture

### 2.1 Harness Split Model

Claude Code is best described as an **"agent harness"**: a local shell runtime that wraps an LLM with tools (file reads, shell exec, MCP), a memory system, and a permission model. Ultraplan *splits this harness*:

```
Local Terminal (client)                Remote CCR Session
───────────────────────────            ──────────────────────────────
  CLI shell / REPL                       Anthropic cloud container
  Polling for status (~3s)     ◄──────►  Multi-agent orchestrator
  "Teleport" receiver                    Opus 4.6 model
  File system access                     .ultraplan/ state directory
  GitHub repo push/pull                  GitHub clone (read-only snap)
```

The local terminal becomes a **thin polling client**; the full agentic loop (context assembly → planning → critique → finalization) runs in the cloud container.

### 2.2 Multi-Agent Orchestration (Explore → Synthesize → Critique)

Ultraplan's cloud session runs a three-phase multi-agent pipeline:

**Phase 1 — Parallel Exploration**
Multiple specialized sub-agents are spawned concurrently, each investigating a different dimension:
- `ArchAgent`: existing codebase structure and design patterns
- `RiskAgent`: regression surfaces, risky dependency chains, edge cases
- `FileAgent`: concrete file-level modification scope
- `DepsAgent`: downstream consumers, cross-crate or cross-module relationships

**Phase 2 — Synthesis**
A central planner model aggregates findings from the exploration agents into a unified `UltraPlan` structure. This is the equivalent of Vox's `VoxPlan` — a task DAG with assumptions, file-level steps, and risk annotations.

**Phase 3 — Critique and Refinement**
A dedicated **critique agent** (a second LLM pass) reviews the synthesized plan for:
- Logical gaps and missing steps
- Architecture violations (e.g., methods that don't exist being called)
- Risk under-reporting
- Unnecessary complexity (over-scaffolding)

If issues are found, the critique triggers targeted revisions before the plan is delivered. There is no human-in-the-loop during this critique phase.

### 2.3 Context and Memory

Ultraplan uses a **three-layer context compression** strategy to manage the context window during long planning sessions:

| Layer | Mechanism | Triggers When |
|---|---|---|
| Micro-compact | Inline token reduction of recent turns | Rolling context approaches 70% capacity |
| Auto-compact | Aggressive summarization of full transcript | Full context window pressure |
| Transcript management | Snapshot serialization to `.ultraplan/` dir | Session handoff and resume |

The file-based memory system (`memory.md` / `.ultraplan/`) is used as a **persistent anchor** so cloud planning sessions don't need to re-derive project context from scratch on every invocation.

### 2.4 The Teleport Mechanism

When a plan is finalized and approved in the browser UI (`claude.ai/code`), the plan is serialized and returned to the local CLI via a **sentinel value** internally named `__ULTRAPLAN_TELEPORT_LOCAL__`. The local Claude Code session detects this sentinel, deserializes the plan, and can either:

1. **Execute locally**: inject plan steps into the local agentic loop
2. **Execute remotely**: trigger a PR-generation pipeline in the cloud container

### 2.5 A/B Planning Depth Variants

Ultraplan does **not** always execute the deep multi-agent path. There are at least two internal planning variants, assigned based on task complexity detection and A/B experimentation:

- **"Simple Plan"**: Linear outline with file-level notes. No critique phase. Faster (~2 min).
- **"Deep Plan"**: Full explore-synthesize-critique pipeline. Up to 30 min of compute. Multi-section architecture with risk analysis.

**Users cannot force the "Deep Plan" variant.** The selection is opaque to the user. This is a notable ergonomic limitation.

---

## 3. Cost Model

### 3.1 Thinking Token Billing

Extended thinking tokens (the internal reasoning trace) are billed as **standard output tokens** at the model's output rate. There is no separate "thinking" pricing tier.

| Thinking Level | Trigger Keyword | Approx. Token Budget | Est. Cost / Task (API) |
|---|---|---|---|
| Basic | `think` | ~4,000 | ~$0.06 |
| Hard | `think hard` | ~8,000 | ~$0.12 |
| Harder | `think harder` | ~16,000 | ~$0.24 |
| Ultrathink | `ultrathink` | ~32,000 | ~$0.48 |
| Ultraplan (cloud) | `/ultraplan` | Up to 30 min of Opus time | Consumes quota significantly faster |

*Estimates based on ~$15/million output tokens for Sonnet 4.6. Opus 4.6 is more expensive.*

### 3.2 Subscription vs. API

- **Pro ($20/mo) / Max ($100-$200/mo)**: Flat-rate subscription with rolling usage windows (typically 5-hour reset buckets). Ultraplan consumes quota; frequent deep plans can exhaust a 5-hour window.
- **API / BYOK**: Full token-level billing. Ultraplan with Opus 4.6 on a complex codebase can cost several dollars per session.

### 3.3 Cost Controls

- `/effort` command or `MAX_THINKING_TOKENS` config to lower reasoning depth
- `/cost` command shows real-time session token counts and estimated spend
- Model selection in `/config` (downgrade Opus → Sonnet for less critical plans)

archived_date: 2026-04-18
---

## 4. Limitations

### 4.1 Hard Infrastructure Requirements

| Requirement | Detail |
|---|---|
| GitHub only | Requires a GitHub-hosted repo. GitLab, Bitbucket, local-only repos: **not supported** |
| Anthropic cloud only | Incompatible with Amazon Bedrock, Google Vertex AI, Microsoft Foundry backends |
| CLI initiation | Cannot trigger from the web UI; must start from local terminal |
| Claude Code v2.1.91+ | Requires specific version |

### 4.2 Stale Context / Snapshot Problem

Ultraplan creates a **point-in-time snapshot** of the repository when the session starts. Any local edits made after initiation are invisible to the cloud planning session. This is the most practically dangerous limitation:

- If you make a hotfix locally mid-plan, the Ultraplan session will produce a plan targeting the *pre-fix* state
- Schema migrations or generated files that were just run locally are not reflected
- The resulting plan can be structurally incorrect without any visible error

### 4.3 Opaque A/B Depth Selection

As noted above, users cannot control whether they get the "simple" or "deep" planning path. This makes Ultraplan non-deterministic in terms of quality — the same prompt may yield a shallow plan one day and a deep architectural analysis the next.

### 4.4 Silent Context and Memory Limits

Research into Claude Code internals reveals **undocumented hard caps**:
- File read ceilings (large files may be silently truncated)
- Memory cap on `memory.md` (file grows unboundedly; entries beyond a threshold are silently ignored)
- Automatic context truncation without visible warnings

Exceeding these limits produces **hallucinations or subtly incorrect plans** without explicit error messages. This is arguably the most dangerous failure mode.

### 4.5 Mutual Exclusivity with Remote Control

If "Remote Control" features (another Claude Code cloud feature) are active, they disconnect when an Ultraplan session starts — both share the same cloud interface slot.

---

## 5. Failure Modes (Real-World)

Based on aggregated community reports and technical analysis:

### 5.1 "Fading Rigor" Quality Regression
Model updates can cause the planning quality to regress without user notification. Plans that were previously deep and multi-section become shallow outlines. No changelog or quality metric is exposed.

### 5.2 Over-Scaffolding
Without strict task framing, Ultraplan tends to propose more structure than necessary:
- Adds abstraction layers that weren't requested
- Introduces new patterns that conflict with existing project conventions
- Generates boilerplate for use cases that won't be needed

This is worse than local plan mode because the cloud agent lacks the lived context of recent codebase churn that a developer has.

### 5.3 Over-Fixing / Cascade Errors
When debugging tasks are sent to Ultraplan, the critique agent's risk-scanning can surface issues *adjacent* to the actual problem and include them in the plan. The resulting plan fixes more than was asked, increasing the risk of introducing regressions.

### 5.4 Silent Error Masking
The synthesizer agent tends to "paper over" architectural errors it detects rather than flagging them explicitly. Plans may reference methods that don't quite exist, or propose file paths that are structurally incorrect for the project's organization. These surface only during execution.

### 5.5 Inefficiency on Small Tasks
Using Ultraplan for routine tasks (typo fixes, single-file config changes, documentation updates) is almost always counter-productive:
- 5-30 minute plan generation time vs. 30-second direct execution
- Consumes expensive Opus quota
- The critique step introduces latency for decisions that don't require deliberation

archived_date: 2026-04-18
---

## 6. Best Use Cases

Ultraplan delivers meaningful value specifically for:

1. **Large cross-cutting refactors**: Refactors touching 10+ files with complex dependency order requirements
2. **Migration planning**: Major dependency upgrades, DSL migrations, schema migrations with multi-step ordering constraints
3. **Greenfield architecture for a bounded module**: New crates or subsystems with clearly defined interface contracts
4. **Security-sensitive planning**: Scenarios where a critique pass to catch architectural weaknesses is worth the time cost
5. **Asynchronous planning**: When the developer wants to queue a planning task and return to other work while the plan generates

### Worst Use Cases

1. **Anything requiring near-real-time local state** (ongoing migrations, generated code, live schema changes)
2. **Hot debugging loops** (add lag; the snapshot is stale before the plan arrives)
3. **Greenfield exploration of an unfamiliar domain** (the agent lacks business context that only the dev has)
4. **Single-file or trivial changes** (cost/latency ratio is catastrophically poor)
5. **Air-gapped, private, or non-GitHub environments** (structurally incompatible)

---

## 7. What the Architecture Gets Right (Industry-Level Signals)

Beyond this specific product, several design signals from Ultraplan represent frontier thinking in agentic orchestration that are worth studying:

### 7.1 The "Orchestration Moat" Insight

The competitive value is **not** the model. The moat is the **orchestration layer**: cost-control, permission enforcement, context compression, multi-agent coordination, and memory architecture built *around* the model. Any competitor with the same base model but weaker orchestration will produce worse planning output.

> "The real moat of the architecture is not the LLM itself, but the orchestration layer — the complex coordination of agents, memory management, permission enforcement, and cost-control systems built around the model."

### 7.2 Three-Role Agent Topology

The explore/synthesize/critique pattern (or equivalently: research/plan/review) is becoming industry standard for quality-critical planning. A single-agent linear planner is now considered inferior for complex tasks.

### 7.3 Decoupled Plan UX from Execution Context

Separating "where the plan is reviewed" (browser, rich UI, comments, diagrams) from "where the code runs" (local terminal, CI) is a UX that reduces friction significantly. The "teleport" pattern is a concrete implementation of this separation.

### 7.4 Effort/Budget Knobs as First-Class Controls

Exposing `think`, `think hard`, `think harder`, `ultrathink` as graduated effort levels (rather than a binary on/off) gives users cost-awareness and appropriate tool selection. This is better UX than a single "enable reasoning" checkbox.

archived_date: 2026-04-18
---

## 8. Implications for Vox DEI Orchestrator and Planning Mode

Vox already implements several analogous concepts. The following analysis maps the Claude Code Ultraplan findings against Vox's existing architecture and identifies gaps.

### 8.1 Current Vox Parallelism

| Ultraplan Concept | Vox Equivalent | Gap |
|---|---|---|
| Parallel exploration agents | `PlanningOrchestrator` + `ContextAssembler` | Vox assembles context serially; no true parallel sub-agents |
| Synthesizer LLM | `PlannerConfig` + Planner LLM | Present |
| Critique agent | Reviewer LLM (Wave 1) | Present, but single-pass; no targeted revision loop |
| `.ultraplan/` state dir | Arca `plan_sessions` table (V25) | Vox persists to DB; more durable than file system |
| Teleport mechanism | `vox_replan` MCP tool + execution bridge | Partial; no "execute in cloud" path |
| Context compression | `ContextAssembler` embedding search | No active multi-layer compression (micro/auto-compact) |
| Thinking budget tiers | `PlannerConfig.max_planning_tokens` | Single budget value; no graduated user-facing knobs |

### 8.2 High-Priority Gaps to Address

#### (A) Parallel Context Gathering (Wave 4 / Near-term)
Vox's `ContextAssembler` currently builds the context packet **serially**. Ultraplan's parallel exploration agents represent a meaningful quality improvement. The implementation path in Vox would be:
- Spawn concurrent `AgentTask`s for: repo structure scan, recent memory retrieval, KB doc retrieval, prior plan history
- Merge results into the `VoxPlan` context packet via the DEI orchestrator's existing parallel dispatch

#### (B) Critique-Then-Revise Loop (Now labeled Wave 1 complete, but shallow)
Vox's Reviewer LLM does a single-pass review. Ultraplan's architecture shows that a **targeted revision loop** (critique → identify specific gaps → revise only those sections → re-critique) produces materially better output. This is achievable by:
- Having the Reviewer emit structured `CritiqueNote` items (gap, location in plan, severity)
- Passing `CritiqueNote`s back to the Planner for targeted patch generation
- Capping the loop at 2-3 iterations to control cost and latency

#### (C) Graduated Thinking Budget UX
Vox should expose effort tiers as named levels in the CLI and MCP surface, not just a numeric token count:

```
vox plan --depth shallow   # ~4k tokens, fast
vox plan --depth standard  # ~16k tokens (default)
vox plan --depth deep      # ~32k tokens, long form
vox plan --depth ultraplan # async + parallel agents (future)
```

This maps cleanly onto `PlannerConfig` and adds user-facing cost awareness without changing the underlying system.

#### (D) Stale Context Guard (Vox advantage to protect)
Ultraplan's snapshot staleness is a significant real-world failure mode. Vox's architecture **avoids this problem** because planning runs locally with live filesystem access. This is a genuine Vox advantage and should be explicitly documented and preserved. Do not introduce any design that snapshots the repo for planning unless it includes a staleness check and re-sync mechanism.

#### (E) Context Truncation Observability
Ultraplan's silent truncation failures are serious. Vox should:
- Emit a `ContextTruncatedWarning` telemetry event whenever any context source is capped
- Surface this in the VS Code AttentionPanel so users know their plan was assembled on incomplete context
- Log truncation to `plan_events` for post-mortem analysis

#### (F) Plan Quality Observability (Wave 4)
Ultraplan provides no plan quality metric. Vox can differentiate here:
- Score each plan version using the Reviewer LLM output (confidence, completeness, risk coverage)
- Store scores in `plan_versions` table
- Expose via `vox plan status --quality` for user-facing insight and for the planning eval fixtures (Wave 4)

### 8.3 What Vox Should NOT Copy

1. **GitHub-only repo requirement**: Vox is local-first and must remain so. Any future "remote orchestration" mode should support local, GitLab, and arbitrary VCS.
2. **Opaque A/B depth selection**: Users must be able to control plan depth. Never make it non-deterministic and opaque.
3. **File-system-only plan state**: Vox's Arca-based plan persistence is strictly better. Do not regress to `.ultraplan/` file directories.
4. **Silent context limit failures**: Surface all limits as observable events.

---

## 9. Recommended Implementation Items

The following items are derived from the above analysis, ranked by Vox-specific impact:

| Priority | Item | Vox Component | Wave |
|---|---|---|---|
| High | Graduated `--depth` knobs on `vox plan` | `vox-cli`, `PlannerConfig` | 3 (current) |
| High | `ContextTruncatedWarning` telemetry event | `ContextAssembler`, Arca | 3 (current) |
| High | Structured `CritiqueNote` revision loop | `PlanningOrchestrator` | 3 (current) |
| Medium | Parallel context sub-tasks via DEI dispatcher | `ContextAssembler`, DEI | 4 |
| Medium | Plan quality scoring stored in `plan_versions` | Arca, Reviewer LLM | 4 |
| Low | "Async plan" mode: queue deep plan, poll for completion | DEI, MCP, CLI | 5+ |
| Low | Browser-based plan review surface | VS Code WebView | 5+ |

archived_date: 2026-04-18
---

## 10. References

- Anthropic Claude Code docs: `claude.ai/code`
- claudefa.st — Ultraplan deep dive technical analysis (April 2026)
- mejba.me — Ultraplan limitations survey
- businessengineer.ai — "Orchestration moat" analysis
- Reddit /r/ClaudeAI community reports (April 2026)
- Vox planning mode KI: `knowledge/vox_agentic_planning_mode/artifacts/overview.md`
- Vox orchestrator KI: `knowledge/vox_agent_workflow_and_orchestration/artifacts/orchestrator_internals.md`
- This document cross-references: `docs/src/architecture/res_dynamic_agentic_planning_2026.md`

