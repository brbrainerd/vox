---
title: "Antigravity Handoff Guide & In-Repo Skill Gaps"
description: "How to execute the auto-GUI and auto-debugging implementation plans inside Google Antigravity with Gemini 3.5 Flash: execution model, parallel-subagent rules, the in-repo skill map, and compact in-repo stubs for the five missing skills (brainstorming, dispatching-parallel-agents, verification-before-completion, code-review, git-worktrees) so the run is self-contained."
category: "Architecture SSOTs"
---

# Antigravity Handoff Guide & In-Repo Skill Gaps

**Status:** Contributor reference for the handoff of the two 2026-06-18 plans to Google Antigravity / Gemini 3.5 Flash.
**Research basis:** [`../architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) (execution-target profile) + [`../architecture/auto-derivation-design-hygiene-2026-06-18.md`](../architecture/auto-derivation-design-hygiene-2026-06-18.md).
**Plans:** [`../superpowers/plans/2026-06-18-track-a-naked-objects-auto-gui.md`](../superpowers/plans/2026-06-18-track-a-naked-objects-auto-gui.md) · [`../superpowers/plans/2026-06-18-track-b-zero-annotation-debugging.md`](../superpowers/plans/2026-06-18-track-b-zero-annotation-debugging.md).

---

## 1. Why this guide exists

The plans will be executed by **Gemini 3.5 Flash inside Antigravity**, not Claude Code. Antigravity reads `GEMINI.md` (its overrides) and `AGENTS.md` (cross-tool, since v1.20.3), and mounts skills from `.agents/skills/`. It **cannot** see Claude's external `~/.claude/` superpowers plugin cache. So any skill a plan references must live **in-repo** — and as of 2026-06-18 **all eleven do** (`crates/vox-skills/skills/superpowers/`, including `deep-research` and `research`). This doc maps them and summarizes each, and lists the handoff prerequisites.

## 2. Antigravity execution model (what the plans target)

- **Orchestrator + dynamic subagents** with **isolated context windows**, dispatched in parallel.
- **Reliability is the hard constraint:** ~48% real-world task completion; mid-task termination leaves no checkpoint; quota is a hard cutoff. Therefore the plans are written so that **each task is atomic, ends green, and is committed** — a kill wastes at most one task.
- **Gemini 3.5 Flash:** strong agentic coder; weaker deep reasoning and long-context recall than Pro; prone to API hallucination. Plans counter this with **verify-before-use** steps and **two-strike circuit breakers** (see research §3.3).

## 3. Parallel vs sequential — the dispatch rule

Each plan task is tagged **`[PARALLEL-SAFE]`** or **`[SEQUENTIAL]`**:
- **`[PARALLEL-SAFE]`** — touches a disjoint file set from other parallel-safe tasks in the same wave; an isolated-context subagent can own it. Dispatch these together.
- **`[SEQUENTIAL]`** — modifies a file an earlier task also modifies, or depends on an earlier task's output; run in order on one agent.

**Golden rule:** never dispatch two subagents that write the same file. Antigravity's isolated contexts will not see each other's edits and will clobber. When in doubt, mark `[SEQUENTIAL]`.

## 4. In-repo skill map

| Plan reference | In-repo path (use this) |
|---|---|
| Writing plans | `crates/vox-skills/skills/superpowers/writing-plans.skill.md` |
| Subagent-driven execution | `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` |
| TDD (red-green-refactor) | `crates/vox-skills/skills/superpowers/test-driven-development.skill.md` |
| Systematic debugging | `crates/vox-skills/skills/superpowers/systematic-debugging.skill.md` |
| Brainstorming | `crates/vox-skills/skills/superpowers/brainstorming.skill.md` (NOW NATIVE) |
| Dispatching parallel agents | `crates/vox-skills/skills/superpowers/dispatching-parallel-agents.skill.md` (NOW NATIVE) |
| Verification-before-completion | `crates/vox-skills/skills/superpowers/verification-before-completion.skill.md` (NOW NATIVE) |
| Code review | `crates/vox-skills/skills/superpowers/requesting-code-review.skill.md` (NOW NATIVE) |
| Git worktrees | `crates/vox-skills/skills/superpowers/using-git-worktrees.skill.md` (NOW NATIVE) |
| Deep research | `crates/vox-skills/skills/superpowers/deep-research.skill.md` (NOW NATIVE) |
| Research (lightweight web) | `crates/vox-skills/skills/superpowers/research.skill.md` (NOW NATIVE) |

> **Update 2026-06-18:** all eleven skills are now in the Vox native library under `crates/vox-skills/skills/superpowers/`. Antigravity mounts repo skills from `.agents/skills/`; ensure that path resolves to (or symlinks/copies) `crates/vox-skills/skills/superpowers/` so Gemini can load every skill above. The §5 entries below are quick-reference summaries — the authoritative skill is the file.

Policy the agent must obey throughout: `AGENTS.md` (root), plus the Vox-specific rules in `CLAUDE.md`/`GEMINI.md` — notably **VoxScript-first automation** (no `.ps1`/`.sh`/`.py`; use `vox run scripts/*.vox`), **never `cargo fmt --all`** (use `cargo fmt -p <crate>`), and **`.md` under `docs/src/` needs YAML frontmatter**.

---

## 5. Quick-reference summaries (authoritative skill = the native file)

These five skills are now native (paths in §4). The summaries below are a fast reference; load the full `.skill.md` for complete guidance.

### 5.1 brainstorming
**Use when:** a task requires a design choice the plan did not fully specify.
**Do:** (1) State the decision in one sentence. (2) List 2–3 concrete options with one-line trade-offs. (3) Pick one and record *why* in a comment or commit message. **Do NOT** start coding until the choice is written down. For Gemini 3.5 Flash: never invent a fourth "clever" option that requires APIs you have not verified exist.

### 5.2 dispatching-parallel-agents
**Use when:** 2+ tasks are tagged `[PARALLEL-SAFE]` in the same wave.
**Do:** (1) Confirm their file sets are disjoint (re-read each task's **Files** block). (2) Spawn one subagent per task with ONLY that task's text as context (isolated window). (3) Wait for all to return green. (4) Integrate sequentially: pull each result, run the full crate test suite once, resolve any surprise overlap. **Never** parallelize tasks that share a file. **Two-strike rule:** if a subagent fails twice, stop it and surface its handoff note rather than re-dispatching.

### 5.3 verification-before-completion
**Use when:** about to mark any task done or commit.
**Do, in order, and paste the actual output:** (1) `cargo test -p <crate>` → must show PASS counts. (2) `cargo clippy -p <crate> -- -D warnings` → must be clean. (3) `cargo fmt -p <crate>` (never `--all`). (4) confirm the tree compiles (`cargo check -p <crate>`). **Rule:** evidence before assertion — do not claim "done" without pasted command output. If any fails, the task is NOT done.

### 5.4 code-review (self-review pass for a fast model)
**Use when:** a task's implementation step is written, before its commit.
**Checklist:** (1) Does every symbol/path I referenced actually exist? (re-grep if unsure — anti-hallucination). (2) Did I add a stub/placeholder? (forbidden — see `AGENTS.md`/`feedback_no_stubs`). (3) Does the change match the task's stated Files block exactly? (4) Did I duplicate logic that already exists (DRY)? Fix inline; do not expand scope.

### 5.5 using-git-worktrees
**Use when:** the plan says to isolate parallel file-mutating work.
**Do:** for `[PARALLEL-SAFE]` waves that each mutate files, give each subagent its own worktree: `git worktree add ../wt-<task> <branch>`; work there; commit; then integrate. Clean up with `git worktree remove`. For this repo's two plans, most tasks touch disjoint files in one crate, so worktrees are optional — use only if two parallel tasks would otherwise race the same target dir. On Windows, prune stale worktrees before deleting branches.

---

## 6. Handoff checklist (run once before starting either plan)

- [ ] `AGENTS.md` and `GEMINI.md` present and loaded (the latter may need creating from `CLAUDE.md` for Antigravity).
- [ ] `cargo run -p vox-arch-check` passes (baseline).
- [ ] The plan's Pre-flight `rg` commands have been run and the real signatures confirmed (anti-hallucination).
- [ ] Decide track order (all independent at the code level; recommended: Track A → Track C → Track B). Plans: [Track A](../../superpowers/plans/2026-06-18-track-a-naked-objects-auto-gui.md) · [Track B](../../superpowers/plans/2026-06-18-track-b-zero-annotation-debugging.md) · [Track C](../../superpowers/plans/2026-06-18-track-c-vox-as-ai-ui-target.md).
- [ ] Confirm parallel waves: list which task IDs are `[PARALLEL-SAFE]` together.
