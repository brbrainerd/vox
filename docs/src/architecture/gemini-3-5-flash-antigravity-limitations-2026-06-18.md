---
title: "Gemini 3.5 Flash & Google Antigravity — Limitations and Execution Constraints"
description: "Reference profile of Gemini 3.5 Flash and the Google Antigravity agentic IDE as a plan-execution target: capability profile, documented reliability failure modes, customization surface (GEMINI.md/AGENTS.md/skills), and the concrete plan-engineering constraints these imply. For any future session handing autonomous work to this stack."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-06-18"
---

# Gemini 3.5 Flash & Google Antigravity — Limitations and Execution Constraints

**Status:** Execution-target reference (not a feature design).
**Why this exists:** the 2026-06-18 implementation plans are written to be executed autonomously by Gemini 3.5 Flash inside Antigravity. This doc records *why* those plans are shaped as they are, so future sessions targeting the same stack reuse the constraints instead of rediscovering them.
**Confidence grading:** ★★★ official/primary · ★★ reputable practitioner report · ★ inferred.

---

## 1. What the stack is ★★★

- **Gemini 3.5 Flash** — the fast model Google **co-optimized for the Antigravity agent harness** (not a general model shoehorned in). ([Antigravity overview](https://www.mindstudio.ai/blog/what-is-google-anti-gravity-2-agentic-ide))
- **Google Antigravity (2.0)** — an agent-first IDE (VS Code-based) whose orchestrator decomposes a goal and spawns **dynamic subagents with isolated context windows**, in parallel (demoed at 93 parallel subagents). Subagent capabilities are also exposed via a Managed Agents API. ([Antigravity agent docs](https://ai.google.dev/gemini-api/docs/antigravity-agent), [DataCamp](https://www.datacamp.com/tutorial/antigravity-cli))

## 2. Model capability profile ★★★

- **Context:** 1M-token input, 64k-token output. ([Gemini 3 dev guide](https://ai.google.dev/gemini-api/docs/gemini-3))
- **Strengths:** GA, strong at agentic execution, coding, long-horizon tasks. ([model card](https://deepmind.google/models/model-cards/gemini-3-5-flash/))
- **Weaknesses (vs 3.1 Pro):** trails on Humanity's Last Exam, ARC-AGI-2, and the **128K MRCR v2 long-context retrieval** test — "not a clean replacement for Pro." → **weaker deep reasoning and weaker long-context recall.** ([Appwrite deep-dive](https://appwrite.io/blog/post/gemini-3-5-flash-deep-dive))
- **Feature gaps:** Computer Use not supported in 3.5 Flash (use 3 Flash Preview). ([what's new](https://ai.google.dev/gemini-api/docs/whats-new-gemini-3.5))

## 3. Documented reliability failure modes ★★ (practitioner reports)

- **Low real-world completion:** ~48% task completion in-IDE vs ~80% benchmark — a large reliability gap. ([3-month failure analysis](https://dev.to/vikas_sahani_3a7e2706846c/i-analyzed-3-months-of-google-antigravity-ide-failures-heres-whats-actually-breaking-4ba5))
- **"Agent execution terminated due to error"** mid-run, **no checkpoint**, leaving files partially edited / non-compiling. ([Agent-Terminated crisis](https://medium.com/@krishpatil120/google-antigravitys-recurring-agent-terminated-crisis-5a274f81858b))
- **Quota = binary hard cutoff, no warning** — execution stops immediately, can leave a broken state.
- **Hallucination cascades:** invents APIs / phantom libraries with confidence; serialized session state lets an early wrong assumption become "ground truth" in later sessions. ([hallucination report](https://yakhil25.medium.com/the-gemini-hallucination-crisis-how-google-antigravity-is-destroying-developer-trust-55d0773302f1))
- **Poor self-correction:** repeats the same failed action instead of recovering.

## 4. Customization surface ★★

- Reads **`GEMINI.md`** (Antigravity-specific, highest priority) and **`AGENTS.md`** (shared across Antigravity/Cursor/Claude Code, since v1.20.3 / 2026-03-05).
- Skills are mounted from **`.agents/skills/`** into the sandbox; agent personas via `agents.md`. ([Antigravity rules guide](https://agentpedia.codes/blog/user-rules), [agents.md codelab](https://codelabs.developers.google.com/autonomous-ai-developer-pipelines-antigravity))
- **Implication:** it cannot see Claude's external `~/.claude/` skill cache — any referenced skill must be **in-repo** (Vox now ships them under `crates/vox-skills/skills/superpowers/`).

## 5. Plan-engineering constraints (the rules implementation plans MUST follow)

| Failure mode | Required plan constraint |
|---|---|
| No-checkpoint mid-task termination → broken tree | **Every task is atomic and ends GREEN + committed.** A kill between tasks leaves a compiling, tested tree. Never split a compile-breaking change across two commits. |
| Hallucinated APIs / phantom symbols | **Verify-before-use:** an explicit `rg`/read step precedes any code step that references a symbol/path; inline exact signatures. Forbid "assume the API." |
| Weak long-context recall (MRCR) | **Self-contained tasks:** repeat needed context/code in each task; never rely on remembering earlier tasks. |
| Poor self-correction / repeats failures | **Two-strike circuit breaker:** verification fails twice → STOP + handoff note; do not loop. |
| Weak deep reasoning | **One decision per step**, explicit instructions; no open-ended "design X" steps in execution plans. |
| Quota hard cutoff | **Small tasks + frequent commits** so a cutoff wastes ≤ one task. |
| Isolated-context parallel subagents | **Tag tasks PARALLEL-SAFE / SEQUENTIAL** by file-write disjointness; never two subagents on one file. |

## 6. Handoff prerequisites

- `AGENTS.md` present; generate `GEMINI.md` (e.g. from `CLAUDE.md`) for Antigravity-specific overrides.
- In-repo skills available under `crates/vox-skills/skills/superpowers/` and referenced by the plans (see `../contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`).
- Run plan Pre-flight `rg` commands first (anti-hallucination baseline).

## Sources
Antigravity overview/agent docs/DataCamp · Gemini 3 dev guide / 3.5 Flash model card / Appwrite deep-dive / what's-new · failure-analysis, Agent-Terminated, hallucination reports · Antigravity rules guide / agents.md codelab — full URLs inline above.
