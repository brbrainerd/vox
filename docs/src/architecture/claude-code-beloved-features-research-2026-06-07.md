---
title: "Claude Code's Most-Beloved Programming Features — Research & Vox Adoption Audit"
description: "Cited research on the highest-value Claude Code features for programming (launch through mid-2026), the harness/architecture patterns behind them, and a gap audit against the Vox orchestrator, MENS training pipeline, Candle, and the Vox language — feeding the adoption plan."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---

# Claude Code's Most-Beloved Programming Features — Research & Vox Adoption Audit (2026-06-07)

> **Companion plan:** [`docs/superpowers/plans/2026-06-07-claude-code-feature-adoption.md`](../../superpowers/plans/2026-06-07-claude-code-feature-adoption.md). This document is the *research + audit*; the plan is the *what-we-build*.

## 0. Scope & method

Six parallel research streams (four web-research agents over primary Anthropic sources + practitioner sentiment; two read-only codebase-inventory agents) were run on 2026-06-07. The web research prioritized `anthropic.com`, `code.claude.com`/`platform.claude.com` docs, the Claude Code GitHub CHANGELOG, and well-regarded practitioner write-ups (Simon Willison, Armin Ronacher, the Pragmatic Engineer survey, Hacker News). The codebase inventories mapped the Vox orchestrator harness and the MENS/Candle training stack against the feature list. Claims below carry inline source links; figures contradicted by official model cards are flagged.

The goal is **not** a feature-clone wishlist. It is to isolate *why* Claude Code is loved, distinguish the few load-bearing ideas from the long tail, and decide which translate into real gains for **(a) our agent harness, (b) the MENS fine-tuning pipeline, (c) our Candle inference/training, and (d) the Vox language itself.**

---

## 1. The headline: it's the harness, not (only) the model

The single most important finding for our purposes: practitioners overwhelmingly attribute Claude Code's lead to the **harness** — the agent loop, context engineering, tools, and permission scaffolding — *more than* to raw model IQ. One widely-shared analysis characterized Claude Code as "98.4% control infrastructure, 1.6% AI logic" ([MindStudio](https://www.mindstudio.ai/blog/what-is-agent-harness-architecture-explained), [Tech86](https://www.tech86.com.br/en/blog/claude-code-harness-mais-importante-que-modelo/)). The market data backs the affection: in the Pragmatic Engineer Feb-2026 survey (~900–1,000 engineers), **46% named Claude Code their most-loved AI coding tool — more than 2× Cursor (19%) and ~5× GitHub Copilot (9%)** ([Pragmatic Engineer](https://newsletter.pragmaticengineer.com/p/ai-tooling-2026); [summary](https://aiproductivity.ai/news/pragmatic-engineer-survey-ai-tooling-2026/)), despite launching only in May 2025.

This is strategically convenient for Vox: **we cannot ship a frontier model, but we can ship a frontier harness.** Most of the beloved features are architecture, not weights.

Anthropic states the loop explicitly as **"gather context → take action → verify work → repeat,"** with tool results feeding directly back into context ([Building agents with the Claude Agent SDK](https://claude.com/blog/building-agents-with-the-claude-agent-sdk)). The recurring practitioner insight (esp. Simon Willison) is that the loop only converges when **the agent has a validation mechanism it can run itself** — "run the tests first" ([Agentic Engineering Patterns](https://simonwillison.net/2026/Feb/23/agentic-engineering-patterns/)).

---

## 2. Release timeline (load-bearing milestones)

A condensed, cited timeline (full version with caveats in the research transcript). Dates are approximate where no primary source pinned them.

| Date | Milestone | Why it matters for coding |
|---|---|---|
| 2024-10-22 | Upgraded 3.5 Sonnet + **computer use** beta | SWE-bench 33→49%; vision/screen control groundwork ([news](https://www.anthropic.com/news/3-5-models-and-computer-use)) |
| 2025-02-24 | **Claude Code** launches (research preview) + 3.7 Sonnet | First agentic terminal coder; reads/edits/tests/commits ([news](https://www.anthropic.com/news/claude-3-7-sonnet)) |
| ~2025-02 | **CLAUDE.md** project memory | Always-in-context project DNA |
| 2025-05-22 | **GA** + Opus 4 / Sonnet 4; **interleaved thinking** beta | Hybrid instant/extended thinking; parallel tool use; 7-hr autonomy (Rakuten) ([news](https://www.anthropic.com/news/claude-4)) |
| ~2025-06 | **Remote MCP** + **Claude Code SDK** | External tools over HTTP+OAuth; headless embedding ([blog](https://claude.com/blog/claude-code-remote-mcp), [InfoQ](https://www.infoq.com/news/2025/06/claude-code-sdk/)) |
| ~2025-07 | **Sub-agents / Task tool** | Isolated context windows; parallel delegation ([docs](https://code.claude.com/docs/en/sub-agents)) |
| 2025-08-12 | **1M-token context** (Sonnet 4, beta) | Whole-repo loads ([blog](https://claude.com/blog/1m-context)) |
| 2025-09-29 | **Claude Code 2.0** + Sonnet 4.5 | **Checkpoints/rewind**, native VS Code ext, Terminal 2.0, **SDK→Agent SDK** rename; 30+ hr autonomy ([news](https://www.anthropic.com/news/enabling-claude-code-to-work-more-autonomously), [Sonnet 4.5](https://www.anthropic.com/news/claude-sonnet-4-5)) |
| 2025-10-16 | **Agent Skills** | Progressive-disclosure playbooks (SKILL.md) ([engineering](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)) |
| ~2025-10 | **Plugins** + marketplaces | Bundle skills+hooks+subagents+MCP ([docs](https://code.claude.com/docs/en/discover-plugins)) |
| 2025-10-20 | **Claude Code on the web** + **sandboxing** | Cloud async agents; filesystem+network isolation ([sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing)) |
| 2025-11-14 | **Structured Outputs** beta (grammar-constrained JSON) | Model *cannot* emit schema-violating tokens ([docs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)) |
| 2025-11-24 | **Opus 4.5** | First >80% SWE-bench (80.9%); Opus price cut $15/$75→$5/$25 ([news](https://www.anthropic.com/news/claude-opus-4-5)) |
| 2026-02-05 | **Opus 4.6** | ~81.4% SWE-bench; MRCR long-context 76% vs 18.5%; adaptive thinking ([news](https://www.anthropic.com/news/claude-opus-4-6)) |
| 2026-04-16 | **Opus 4.7** | Faster latency + `xhigh` effort; expanded vision; SWE-bench 87.6% is **secondary/unconfirmed** ([news](https://www.anthropic.com/news/claude-opus-4-7)) |

**Coding-benchmark trajectory (official numbers):** 3.7 Sonnet 62–70% → Opus 4 72.5% → Sonnet 4.5 77.2% → **Opus 4.5 80.9% (first >80%)** → Opus 4.6 ~81.4%. Long-horizon autonomy grew 7 hr (Claude 4) → 30+ hr (Sonnet 4.5).

---

## 3. The beloved features, ranked by sentiment strength

Ordered by how *repeatedly* and *emotionally* they appear in practitioner sources, each with the developer-framed benefit and the underlying mechanism.

### 3.1 The autonomous agent loop with self-running validation — **#1**
"It feels like I'm still programming, but I don't need to write code." Developers operate "one level up," testing behaviors instead of editing files, and let the agent read → plan → edit → **run the tests itself, see failures, iterate** without turn-by-turn babysitting ([HN](https://news.ycombinator.com/item?id=44596472); [Willison](https://simonwillison.net/2026/Feb/23/agentic-engineering-patterns/)). **Mechanism:** ReAct loop where tool results (lint, test, compiler output) feed back as context; three documented verification modes — rules-based (linters), visual (screenshots), LLM-as-judge ([Building agents](https://claude.com/blog/building-agents-with-the-claude-agent-sdk)). **The load-bearing requirement: a validation mechanism the agent can run itself.**

### 3.2 Plan Mode + the ExitPlanMode gate — **#2, most-praised single UX feature**
Claude shows a full implementation plan and changes *nothing* until you approve — "prevents 90% of mistakes" for ~30 s of reading; cheaper/faster because read-only ([Blink](https://blink.new/blog/claude-code-plan-mode-guide); [HN](https://news.ycombinator.com/item?id=44596472)). **Mechanism:** a read-only mode restricted to Read/LS/Glob/Grep/WebFetch/WebSearch — no mutation — and a hard `ExitPlanMode` approval gate between research and edits ([docs](https://code.claude.com/docs/en/common-workflows)). Creator Boris Cherny's canonical workflow: plan → iterate on the plan → flip to auto-accept → execute ([Threads](https://www.threads.com/@boris_cherny/post/DKxKMUjPYty)).

### 3.3 CLAUDE.md / persistent project memory — **#3**
A checked-in file of rules/architecture/conventions, always in context; Anthropic's own teams report "the better teams documented their workflows in CLAUDE.md, the better Claude Code performed" ([Anthropic teams](https://claude.com/blog/how-anthropic-teams-use-claude-code)). **Mechanism:** a tiered precedence hierarchy (managed policy → project → user → local → auto), `@path` imports recursive to depth 5, loaded at launch ([memory docs](https://code.claude.com/docs/en/memory)). Complemented by the **dynamic memory tool** that persists across conversations and survives compaction — Claude is warned before context clearing to write important results to memory files first; **context-editing + memory together improved performance 39% over baseline** ([context-management](https://www.anthropic.com/news/context-management)).

### 3.4 Terminal-native / IDE-agnostic / headless composability — **#4**
"Meets developers where they work"; runs headlessly from cron/CI; no lock-in to a VS Code fork (recurring contrast with Cursor/Windsurf) ([HN](https://news.ycombinator.com/item?id=44832662)). `claude -p` + the GitHub Action (`anthropics/claude-code-action@v1`) make the same agent loop a CI primitive ([GitHub Actions docs](https://code.claude.com/docs/en/github-actions)).

### 3.5 Sub-agents + worktree parallelism — **#5**
Each sub-agent gets a fresh context window; big features decompose into independent workstreams that run simultaneously, "compressing calendar time"; native `--worktree` avoids file conflicts ([sub-agents](https://code.claude.com/docs/en/sub-agents); [worktrees](https://www.mindstudio.ai/blog/parallel-agentic-development-claude-code-worktrees)). **Mechanism (orchestrator-worker):** a lead agent spawns 3–5 subagents, each with an objective, output format, tool guidance, and clear boundaries; each returns **only a summary** so verbose intermediate work never pollutes the parent ([multi-agent system](https://www.anthropic.com/engineering/multi-agent-research-system)). **Token economics:** single agents ~4× chat tokens, multi-agent ~15×; "token usage explains 80% of the variance"; an Opus-lead + Sonnet-workers system beat single-agent Opus by **90.2%** on internal eval — but only worth it when task value justifies the spend and work is genuinely parallel. **Documented caveat developers repeat:** parallelism only works on disjoint files; broad tasks can burn the entire token budget.

### 3.6 Large-context whole-codebase comprehension — **#6**
Ingests entire large codebases in one session (≤1M-token Opus betas); Anthropic's Inference team reports **80% less research time**; new hires "feed Claude their entire codebase to get productive" ([Anthropic teams](https://claude.com/blog/how-anthropic-teams-use-claude-code)).

### 3.7 The extensibility stack — Skills, Plugins, Hooks, MCP — **the 2026 surge**
- **Skills (progressive disclosure):** SKILL.md folders; at startup only `name`+`description` load into the system prompt; full content loads when relevant; bundled resources discovered on demand — so "the amount of context bundled into a skill is effectively unbounded" ([Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)). Open standard, portable across tools; community libraries exploded (Obra "Superpowers" ~40k stars).
- **Hooks (determinism wrapping probabilism):** shell/HTTP handlers fire at lifecycle events — `PreToolUse` (can **block/modify input/allow**), `PostToolUse`, `SessionStart/End`, `PreCompact`, `SubagentStop`, etc. Exit code `2` = blocking error fed back to Claude as the reason. "Use a hook when the action must happen the same way every time and doesn't need Claude to think" ([hooks](https://code.claude.com/docs/en/hooks)).
- **MCP + tool search:** open standard for tool/data integration; **tool search defers tool definitions** until needed — "only tool names and server instructions load at session start, so adding more MCP servers has minimal impact on your context window" ([mcp docs](https://code.claude.com/docs/en/mcp)). (This is the same deferred-tool pattern this very session uses.)
- **Slash commands:** markdown-defined prompt expansions in `.claude/commands/`.

### 3.8 Flow-preserving permission modes — **#11**
Shift+Tab cycles `default → acceptEdits → plan → auto → bypassPermissions`. "Permission prompts destroy flow state." The newer **Auto mode** (AI classifies/blocks dangerous actions) is the safer middle ground vs raw `--dangerously-skip-permissions` ([permissions](https://code.claude.com/docs/en/permissions)). Settings are `allow`/`ask`/`deny` rules evaluated deny→ask→allow, tool/pattern scoped.

### 3.9 Checkpoints / rewind — "useful safety net" more than "beloved"
Auto-captures code state before each prompt; `/rewind` or Esc-Esc restores code-only / chat-only / both; persists across sessions, auto-cleaned after 30 days ([checkpointing](https://code.claude.com/docs/en/checkpointing)). **Critical limitation:** does **not** track bash-driven changes (`rm`/`mv`/`cp`) — complements but does not replace Git.

### 3.10 Cost/token transparency — `/cost`, `/usage`, `/context`, `/compact`
Session token spend, plan limits, a colored context-window grid, and auto-compaction before the window fills; prompt caching cuts repeated-content cost. Loved by ops, less emotionally than #1–4.

### Model-level enablers (not harness, but they raise the ceiling)
- **Prompt caching:** ≤4 cache breakpoints; prefix hierarchy `tools → system → messages` (a change busts that level and everything after); reads at **0.1× (90% off)**, 5-min writes 1.25×, 1-hr writes 2×; the conversation breakpoint auto-advances so prior turns read from cache ([prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching)). This is what makes a long agentic loop economically tractable.
- **Structured Outputs:** compiles a JSON schema into a grammar and constrains decoding so schema-violating tokens are impossible ([docs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs)).
- **Parallel tool calls, interleaved thinking, 1M context, vision/computer-use.**

### What gets criticized (so we don't over-index)
Review can't be skipped (strongest pushback — unreviewed AI code is a real risk); token burn/cost; terminal learning curve; hype outrunning evidence on large legacy codebases; "you don't own the harness" (closed/opinionated). Armin Ronacher tried most 2026 features and **didn't stick with most of them** — a caution against maximalist setups ([Things that didn't work](https://lucumr.pocoo.org/2025/7/30/things-that-didnt-work/)).

---

## 4. Audit: what Vox already has vs. what's missing

Grounded in the two read-only codebase inventories (file refs are representative).

### 4.1 Harness — Vox is *much* further along than expected

| Claude Code feature | Vox state | Verdict |
|---|---|---|
| Agentic loop (gather/act/verify) | Real orchestrator loop; `mutation_classifier` + `checkpoint_engine` | **Have loop; verification feedback under-wired** |
| Sub-agents, isolated context, dispatch | `vox-orchestrator/hopper/*`, `orchestrator/agent/spawn.rs`, `METRIC_TYPE_SUBAGENT_DISPATCH` | **Have**, but no worktree isolation |
| Plan mode + approval gate | `planning/plan_mode_trigger.rs` (ReAct vs PlanAndExecute), Socrates judge, `plan_adequacy`, `replan` | **Have** plan decisioning; read-only enforcement + human ExitPlan gate unclear |
| LLM egress / model-agnostic boundary | `vox-actor-runtime/src/llm/*`, `vox-orchestrator/models/routing_table.rs`, cascade fallback | **Have** (strong) |
| Context compaction | `compaction.rs` (Aggressive/Balanced/Conservative, 0.80 threshold, head/tail preserve) | **Have** |
| Context editing (clear tool results) | `agentos/context_budget_manager.rs` (`prune_evidence_value`/`summarize_evidence`) | **Partial** |
| Memory (project + dynamic) | `memory/*` (MEMORY.md, daily logs), memory MCP tools, retrieval bundles | **Have**, but no **project-scoped `VOX.md`** injected per workspace |
| Tool framework | `vox-orchestrator-mcp` MCP server, 100+ tools, YAML contract registry | **Have** (strong) |
| MCP **tool search / deferred discovery** | rmcp server exposes all tools eagerly | **Missing** — no progressive tool disclosure |
| Hooks (PreToolUse/PostToolUse lifecycle) | `agentos/guardrail_kernel.rs`, `mutation_classifier` (internal only) | **Missing** as a *user-defined* surface |
| Slash commands / skills progressive disclosure | `vox-plugin-*`, `SkillManifest`, ABI v12 | **Partial** — skills load, but no startup name+description-only disclosure |
| Checkpoints / rewind | `agentos/checkpoint_engine.rs` (sparse, pre-mutation), `replay_fast_forward.rs` | **Have** engine; no user-facing rewind |
| Permission modes / allowlists | `attention/budget.rs` `ApprovalTier` + `AgentTrustScore` (EWMA), `route_capability_policy.rs` | **Have** — arguably *more advanced* (trust-scored auto-graduation) |
| Cost/token visibility | `budget_gate.rs` (Ok/Downgrade@0.80/Halt@0.95), `vox-telemetry` D1–D10 | **Have** |
| Extended/interleaved thinking | — | **Missing** — no thinking-budget surface |
| Prompt-caching awareness | not in llm egress | **Missing** — no cache-breakpoint structuring |
| GUI agentic surfaces | `vox-gui` Tauri: status/agent-event streams, model cards, control plane | **Have** |

**Net:** the orchestrator already implements the *expensive* parts (dispatch, routing, compaction, trust-scored permissions, checkpoint engine, MCP server). The gaps are mostly **surfacing and a few sharp primitives**: self-validation feedback wiring, deferred tool discovery, user hooks, project memory file, thinking budget, prompt-cache structuring, worktree isolation, user-facing rewind.

### 4.2 MENS pipeline + Candle — concrete, high-value, mostly-absent primitives

| Claude Code / model-era capability | Vox MENS/Candle state | Verdict |
|---|---|---|
| Verification-is-real → **execution-based eval** | `vox-eval` heuristic (format/safety/length); `vox audit humaneval` is **static typecheck only** | **Missing** — no behavioral/run-the-tests eval |
| Structured Outputs (grammar-constrained decode) | `vox-inference` generate path; no constrained decoding | **Missing** — high-value for serving |
| Gradient checkpointing | **Absent** (drives the 9.5 GiB/ktok activation coefficient) | **Missing** — biggest single VRAM win |
| Flash attention | **Absent** (standard Candle attention) | **Missing** |
| Prompt caching → **KV-cache reuse** in serving | `vox-inference` per-call generate | **Missing** |
| Federated/distributed training | `vox-distributed-training` contracts only; `all_reduce` returns unsupported for world_size>1 | **Stub** — LAN path needs LoRA-delta averaging |
| Agentic-trajectory training data | `TrainingPair` already has `interruption_decision`, `agent_trust_score`, `syntax_spans`, trajectory weighting | **Have** (good foundation) |
| Long-horizon autonomy data | curriculum + trajectory boosts present | **Partial** |
| Resilient long runs | `train_resilient.vox` escalation ladder; mid-epoch atomic checkpoints; `cuMemPoolTrimTo` | **Have** (strong) |

**Net for MENS/Candle:** the training *infrastructure* (memory-budget calibration, resume, presets, resilient wrapper) is mature. The gaps are **model-engineering primitives** — gradient checkpointing, flash-attention, constrained decoding, KV-cache — plus the **execution-based eval** that operationalizes the "verification is real" principle for our own model.

### 4.3 Vox the language
- **Constrained generation as a builtin:** Structured Outputs maps naturally onto a Vox `generate(schema:)` effect — grammar-constrained decoding exposed at the language level.
- **Progressive disclosure for Vox tooling:** the Skills name+description pattern is a good model for how `.vox` scripts and tools advertise themselves to the orchestrator.

---

## 5. Where the gains are (preview of the plan's prioritization)

Ranked by *value ÷ effort*, with the load-bearing #1-loved principle (real self-validation) weighted heavily:

**Tier 1 — highest value, tractable:**
1. **Verification-driven agent loop** — wire compiler/test/lint output back as structured tool results with a self-correct retry budget. This is the #1 beloved thing and we already have the loop and a `vox check`/`vox run` toolchain to validate against.
2. **MCP tool search / progressive disclosure** — defer tool definitions; load names+descriptions first. Direct context-budget win as our 100+ tool surface grows.
3. **Execution-based MENS eval harness** — replace static-only HumanEval-Vox with run-the-program behavioral scoring; this is also the keystone from the golden-corpus initiative.
4. **Project-scoped `VOX.md` memory** — per-workspace always-injected project DNA with `@import`, mirroring CLAUDE.md.

**Tier 2 — high value, more effort:**
5. **Gradient checkpointing in the Candle trainer** — the single biggest VRAM lever (bigger models / longer sequences on the 16 GB 4080).
6. **User-defined hooks** (PreToolUse/PostToolUse lifecycle) + **prompt-cache structuring** in `vox-actor-runtime` llm egress.
7. **Grammar-constrained decoding** in `vox-inference` (and a Vox `generate(schema:)` builtin).
8. **Extended/interleaved thinking surface** with a token budget.

**Tier 3 — valuable, heavier or speculative:**
9. **Worktree-isolated parallel sub-agents** + **user-facing rewind**.
10. **Federated LoRA-delta averaging** (the realistic LAN-training path) and **flash-attention**.

---

## 6. Sources

Primary (Anthropic): [Building agents w/ Agent SDK](https://claude.com/blog/building-agents-with-the-claude-agent-sdk) · [Writing tools for agents](https://www.anthropic.com/engineering/writing-tools-for-agents) · [Multi-agent research system](https://www.anthropic.com/engineering/multi-agent-research-system) · [Effective harnesses for long-running agents](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents) · [Context engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents) · [Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills) · [Context management](https://www.anthropic.com/news/context-management) · [MCP](https://www.anthropic.com/news/model-context-protocol) · [Sandboxing](https://www.anthropic.com/engineering/claude-code-sandboxing). Docs: [sub-agents](https://code.claude.com/docs/en/sub-agents) · [hooks](https://code.claude.com/docs/en/hooks) · [mcp](https://code.claude.com/docs/en/mcp) · [memory](https://code.claude.com/docs/en/memory) · [checkpointing](https://code.claude.com/docs/en/checkpointing) · [permissions](https://code.claude.com/docs/en/permissions) · [common workflows](https://code.claude.com/docs/en/common-workflows) · [prompt caching](https://platform.claude.com/docs/en/build-with-claude/prompt-caching) · [context editing](https://platform.claude.com/docs/en/build-with-claude/context-editing) · [structured outputs](https://platform.claude.com/docs/en/build-with-claude/structured-outputs) · [extended thinking](https://docs.claude.com/en/docs/build-with-claude/extended-thinking) · [Agent SDK overview](https://code.claude.com/docs/en/agent-sdk/overview). Model news: [3.7](https://www.anthropic.com/news/claude-3-7-sonnet) · [Claude 4](https://www.anthropic.com/news/claude-4) · [Sonnet 4.5](https://www.anthropic.com/news/claude-sonnet-4-5) · [Opus 4.5](https://www.anthropic.com/news/claude-opus-4-5) · [Opus 4.6](https://www.anthropic.com/news/claude-opus-4-6) · [Opus 4.7](https://www.anthropic.com/news/claude-opus-4-7) · [autonomy 2.0](https://www.anthropic.com/news/enabling-claude-code-to-work-more-autonomously) · [1M context](https://claude.com/blog/1m-context). Practitioner: [Pragmatic Engineer survey](https://newsletter.pragmaticengineer.com/p/ai-tooling-2026) · [Willison agentic patterns](https://simonwillison.net/2026/Feb/23/agentic-engineering-patterns/) · [Ronacher recommendations](https://lucumr.pocoo.org/2025/6/12/agentic-coding/) · [Anthropic teams use Claude Code](https://claude.com/blog/how-anthropic-teams-use-claude-code) · HN threads [44596472](https://news.ycombinator.com/item?id=44596472) / [44832662](https://news.ycombinator.com/item?id=44832662) / [46676554](https://news.ycombinator.com/item?id=46676554).

> **Sourcing caveats:** Opus 4.7's 87.6% SWE-bench is secondary/unconfirmed (not in the official card). First-ship dates for hooks, slash commands, plan mode, status lines, output styles, and background tasks are approximate (the GitHub CHANGELOG doesn't retain early-2025 entries). Several practitioner WebFetch calls were rate-limited; the Ronacher/Pragmatic-Engineer claims lean on summaries.
