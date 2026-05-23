---
title: "Comprehensive Vox audit and improvement plan (April 2026, v2)"
description: "Full-spectrum audit + ~90 prioritized improvements. Targets the 'premier LLM destination for web app code' thesis."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-04-18"
training_eligible: true
training_rationale: "Strategic audit context for engineering prioritization."
schema_type: "TechArticle"
---

# Comprehensive Vox audit and improvement plan (April 2026, v2)

Bert — full audit triangulating `crates/`, the 280 docs in `docs/src/architecture/`, ADRs 001–023, the CHANGELOG, parser/build artifacts, and the 43 golden examples. Goal: name the real gaps blocking *Vox is the premier programming language for LLMs to write web apps* and give a sequenced improvement list you can actually execute.

This v2 supersedes the prior file; it corrects two count errors (crates=63, ADRs=23) and notes the `vox-doc-pipeline` build break is **fixed** as of today.

## Part I — Executive diagnosis

### What is real and good

- A real Rust compiler (lexer/parser/HIR/IR/typeck + dual Rust+TS codegen) in `crates/vox-compiler/`.
- A real ~17 kLOC orchestrator with A2A messaging, observability, daemon parity (ADR 022), trust telemetry.
- A real Populi mesh control plane with NVML probes and HTTP scope tenancy (ADR 008/017/018).
- A real native MENS training pipeline (Candle QLoRA, Qwen 3.5 4B on 16 GB) with deterministic corpus and OpenAI-compatible serve.
- A real RAG stack (Scientia/`vox-search`) with BM25 + FTS5 + vector + RRF + CRAG + Socrates gating, plus the brand-new symbol-proximity split-brain detector.
- Serious governance: 23 ADRs, `.voxignore` SSoT, Clavis secret resolution, TOESTUB detectors, completion-policy gates, `vox ci ssot-drift`.
- A working `@table` → SQL → typed client → typed server collapse on the golden corpus. This is the moat.
- 102 MCP tools + VS Code extension + OpenCode bridge + dashboard.

### The five biggest structural problems

1. **The parser still rejects top-level statements.** `parse_top_level_item` accepts only `fn`/`import`/`type`. `RunMode::Script` exists at the CLI but no implicit `main()` wrapping or script grammar exists. AGENTS.md mandates `vox run scripts/foo.vox` as the only project glue, yet `examples/actor.vox` and `examples/mcp_tool.vox` were archived as "non-parseable on current grammar" and `VOX_EXAMPLES_STRICT_PARSE` is opt-in. You are simultaneously banning Python/PowerShell glue and shipping a parser that can't run the replacement.
2. **The web-UI pillar embeds React + JSX directly inside `.vox` source.** Authors write `<div className=…>`, `use_state`, `onClick`. The "Path C / Vox-native reactivity" recommended in `web-architecture-analysis-2026.md` is documented but optional. This contaminates the MENS corpus with React idioms and undermines the K-complexity thesis.
3. **LLM-alignment scaffolding is 60–70 % unshipped.** GBNF export is ~30 lines (expressions only) with CVE-2026-2069 in downstream llama.cpp; XGrammar-2 not adopted. `correction_hint` is populated in 1 of 31 HIR validation sites. `@llm` decorator parses but emits no runtime dispatch. AST-token weighted loss is checkboxes-only. There is **no public HumanEval-Vox or SWE-bench-Vox** showing that Vox-generating models hallucinate less than TS/Python-generating models at matched compute.
4. **Doc/code divergence is becoming a governance crisis.** 280 architecture docs, 43 currently passing goldens, ~6 parallel SSoT generators (operations / MCP / CLI / capability / language-surface / completion-policy), retired surfaces (HTMX, Pico, TanStack virtual routes) still ghost-referenced in many pages, research → blueprint pipeline unclear. The doc tree is growing faster than the code tree is shipping.
5. **Scope is too wide for the evidence base.** 63 workspace crates, 280+ CLI subcommands, 23 ADRs, telemetry subsystem, cryptography SSoT, gamification, speech-to-code, vision lane, plugins marketplace, MENS, mesh, RAG, publication pipeline, workflow runtime, package manager. There is no plausible world in which all of these are simultaneously stable. `feature-growth-boundaries.md` and `vox-bell-curve-strategy.md` are trying to restore discipline; the horse left the barn months ago.

### Single-sentence verdict

*Vox has the best architectural governance of any v0.5 language I've seen, the only real compiler+training+orchestrator stack at this tier, and zero empirical evidence for its headline LLM-native claim — while being too sprawled to ship that evidence at current scope.*

## Part V — Reshaped plan, given the answers

Bert answered on 2026-04-18:

| Question | Answer |
|---|---|
| Audience | **Solo devs shipping web apps** |
| Web stack | **React-embedded forever (status quo)** |
| Team | **Solo + AI agents** |
| v1.0 gate | **Real production users** (N external apps deployed) |

This is a *coherent* posture. The plan collapses significantly.

### What those four answers together mean

1. "Real production users" + "solo + AI agents" + "solo devs shipping web apps" means the top-line KPI is now **time-from-`vox new` to a public URL** for a solo dev. Everything that doesn't move that number is secondary.
2. Choosing "React-embedded forever" kills a dozen items that presuppose a Vox-native component model. No Vox-native reactivity DSL. No multi-backend codegen. Web-stack work is narrow: make @island + TanStack Start boring and reliable, and stop the raw JSX leakage into `.vox` source.
3. "Solo + AI agents" means the 63-crate → 10-crate compression (item 93) is no longer a suggestion, it's the governing constraint. Every non-core crate needs a dated freeze/retire decision.
4. "v1.0 = production users" means **the runtime, deploy, and support story** outrank everything else — including MENS and Scientia — because a user's app going down on Saturday is the only thing that matters at that point.

### Items promoted (now P0 or P1 regardless of original tag)

- **1, 2, 3, 4** — parser statement-position regression, `vox-doc-pipeline` stale artifact, examples v0.5.0 migration, golden corpus. Still first. Nothing matters without a parser.
- **28** — `vox new web` scaffold → `vox deploy` → public URL in one command. **Now the single most important feature in the repo.** Promote from P0 to "this is the product."
- **33** — TanStack Start integration hardening. Promoted to P0.
- **16** — reduce raw JSX leakage from `.vox` source into compiler-owned surface. Promoted to P1 (was P2). Even though we're React-embedded forever, LLM-authoring suffers when components are 50% JSX strings.
- **84, 85, 86** — one marquee app in production, public deploy template, written case study. Promoted to P0/P1. These *are* the v1.0 gate.
- **93, 94** — 10-crate freeze + public v1.0 criteria. Promoted to Week 1.
- **49, 50, 51** — runtime crash recovery, supervisor trees, graceful degradation. Promoted to P1 (from P2). "Production users" means a running app that doesn't die.
- **62, 63, 64** — observability basics: structured logs, error reporting, uptime ping. Promoted to P1. You cannot support users blind.
- **40** — `vox fmt` + linter stability. P1. Required for AI authoring to be clean.

### Items demoted or shelved

- **15** — Vox-native reactivity primitives. **Shelved.** User explicitly chose status quo.
- **23, 26, 35** — Solid / Svelte / Next adapters. **Shelved indefinitely** (multi-backend was the rejected option).
- **17 (Vox-native component DSL R&D)** — Shelved.
- **18, 19, 20** — GBNF/XGrammar-2, AST-token alignment, correction-hint expansion. **Demoted to P2.** Still valuable but not on the solo-dev-to-production critical path. These become compelling *after* someone is shipping an app.
- **30, 31** — HumanEval-Vox, SWE-bench-Vox. **Demoted to P2.** Labs aren't the audience.
- **60, 61** — MENS training pipeline hardening. **Demoted to P2.** MENS is a research pillar but no solo dev cares that their Vox code is training data on day one.
- **41** — Scientia/RAG hardening beyond current state. **Demoted to P2.**
- **77, 78** — SSoT generator consolidation. **Demoted to P2** (was P1). Still needed for contributor sanity, but 1 contributor ≈ you, so the pain is smaller than the audit assumed.
- **96, 97** — crate ledger, resurrecting retired orchestrator codenames. P1 → P2; the freeze decision (93) supersedes the ledger.

### Items deleted from scope entirely

- Foundation governance (92), weekly status post (90), OSS onboarding funnel (91), Linguist submission (88), public MCP endpoint (89) — these were community-scale moves premised on either OSS contributors or labs as audience. With solo-dev-as-audience, they become post-v1.0.
- Everything downstream of a Foundation or funded-team shape.

### The reshaped week-by-week

1. **Week 1 — bleed stops, scope honest.** 1, 2, 3, 4, 93, 94, 69 (archive stale research). Publish a `crates/_frozen.md` list. Publish three-criterion v1.0 gate: (a) 3 external solo-dev apps in production for 30 days, (b) `vox new web` → public URL in under 10 minutes, (c) zero P0 open on `vox-compiler`/`vox-actor-runtime`.
2. **Weeks 2–3 — the product *is* `vox new web`.** 28, 33, 40. One command from empty dir to a deployed TanStack Start app on a free tier (Fly.io / Vercel / Cloudflare). Golden path in docs. Record the video.
3. **Weeks 4–6 — the first real user.** 84, 85, 86. Find one solo dev externally (could be a friend, could be you in a second persona) and walk them through shipping a real app. File every paper-cut as an issue. Fix it or freeze it within the week.
4. **Months 2–3 — keep the app up.** 49, 50, 51, 62, 63, 64. Runtime recovery, observability, error reporting. Users on production = support channel.
5. **Months 4–6 — second and third users.** Repeat the loop. Count: production apps deployed, days-without-runtime-incident, time-from-clone-to-URL. Publish the number weekly to yourself (private dashboard, not public).
6. **After the third production user, then and only then** — reopen the P2 shelf: grammar-constrained decoding, HumanEval-Vox, MENS hardening, SSoT consolidation. The research pillars become v1.x work.

### The one-sentence product posture

**Vox v1.0 is the fastest way for a single human-plus-AI team to ship and operate a real web app, and we will not add any feature that does not measurably shorten that loop.**

Everything in the audit is still valid as a long-term backlog. But the active working set, given these four answers, is ~25 items, not 100.

