---
title: "Antigravity Handoff Ledger — Prompt→Build→Review CI/CD Loop"
description: "Append-only, machine-mineable ledger of every plan/prompt handed from Claude Code to Google Antigravity (Gemini 3.5 Flash): the prompt artifact, what was delivered, the outcome, the errors and agent deviations encountered, the code-review findings, and the distilled prompt-engineering lessons fed back into the next prompt. The SSOT for closing the continuous prompt-engineering improvement loop."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Antigravity Handoff Ledger

**Purpose.** This is the **single source of truth for the prompt-engineering CI/CD loop** between Claude Code (where we *author* plans/prompts) and Google Antigravity / Gemini 3.5 Flash (where they *execute*). Every handoff is logged as one append-only entry. Entries are designed to be **mined**: each carries a machine-readable `yaml` block (stable keys) plus human prose. Over time, the distilled lessons (§B) become the checklist that hardens the next prompt.

**How the loop closes:**
```
author plan/prompt (Claude Code)  ──▶  hand off (launch statement)  ──▶  Antigravity/Gemini executes
        ▲                                                                          │
        │                                                                          ▼
  feed lessons back  ◀──  distill (§B)  ◀──  code-review the delivery  ◀──  delivery + errors
```

## How to use this ledger
1. **When you hand a plan to Antigravity:** append a new entry in §C using the schema below. Fill the `prompt`/`inputs` fields immediately.
2. **When Antigravity reports back:** fill `outcome`, `errors_encountered`, `agent_deviations`, `commits`.
3. **After code-review:** fill `review_findings` and `verdict`, then add 1–3 `prompt_lessons`.
4. **Promote recurring lessons** into §B (the distilled checklist) so the next launch statement bakes them in.
5. **Mining:** the `yaml` blocks are greppable. Examples:
   - `rg -n "outcome: (partial|failed)" docs/superpowers/antigravity-handoff-ledger.md` — every imperfect run.
   - `rg -n "category:" docs/superpowers/antigravity-handoff-ledger.md | sort | uniq -c` — failure-category frequency.
   - `rg -A2 "agent_deviations:" docs/superpowers/antigravity-handoff-ledger.md` — every time the agent went off-script.

## Entry schema (copy for each new handoff)
> **`AGH-NNNN` is the reserved template sentinel.** The `vox ci handoff-ledger` lint skips any block whose id is literally `AGH-NNNN`, so this template never fails validation. Real entries use a 4-digit id (`AGH-0002`, …).
```yaml
# --- AGH-NNNN ---
id: AGH-NNNN
date: YYYY-MM-DD
plan: docs/superpowers/plans/<file>.md          # the plan executed
prompt_artifact: <link or section ref to the launch statement handed over>
prompt_version: v1                               # bump when the launch statement changes
subsystem: <short name>
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, spec, plan, launch-statement]   # what Claude authored
delivered: [<crate/file>, ...]
loc: <int>
outcome: green | partial | failed
verification: { tests: "N passed", clippy: clean|warns, arch_check: green|red, smoke: ok|n/a }
errors_encountered:                              # what broke during/after execution
  - { what: "<symptom>", root_cause: "<cause>", category: "<see categories>", who: agent|plan|preexisting }
agent_deviations:                                # where the agent went beyond the plan
  - "<deviation + risk>"
review_findings: <link to review section / verdict>
verdict: approve | approve-with-followups | request-changes
prompt_lessons:                                  # 1-3 lessons to harden the next prompt
  - "<lesson>"
corrections_fed_back: [AGH-NNNN, ...]            # which later prompts adopted these lessons
commits: [<sha>, ...]
```

**`category` vocabulary** (keep stable for mining): `hallucinated-api`, `wrong-path`, `wrong-crate`, `arch-check-gate`, `fmt-gate`, `build-gate`, `branch-hygiene`, `scope-creep`, `already-done`, `perf`, `robustness`, `test-hygiene`, `unplanned-shared-change`, `ssot-fork`, `unit-mismatch`.

---

## §A. Loop metrics (update opportunistically)
| metric | value | as of |
|---|---|---|
| handoffs logged | 1 | 2026-06-18 |
| green-first-pass rate | 1/1 | 2026-06-18 |
| most common failure category | (n/a — single sample) | 2026-06-18 |

## §B. Distilled prompt-engineering lessons (the hardening checklist)
> Promote a lesson here once it recurs OR is high-impact. Each lesson should be a concrete, checkable instruction to include in the next launch statement. Tag with the AGH entries that motivated it.

1. **Spell out every `error`-level arch-check rule a new crate trips** (WTL coverage row + `orphan_exempt` lifecycle), because the agent's green-gate is `vox-arch-check`. — *AGH-0001* ✅ included in the launch statement; agent honored it.
2. **Forbid unplanned edits to shared architecture config.** The launch statement must say: "If `cargo run -p vox-arch-check` is red at baseline for reasons unrelated to your crate, STOP and report — do NOT relabel layers, add `orphan_exempt`, or edit `layers.toml` for crates you didn't create." — *AGH-0001* (agent silently promoted `vox-runtime` L1→L2). **NOT yet in prompts — add next.**
3. **Mandate branch isolation.** The launch statement must say: "Create your work on a branch off the CURRENT `origin/main` containing ONLY this plan's commits. Do not accumulate unrelated initiatives on one branch." — *AGH-0001* (73-commit kitchen-sink branch). **NOT yet in prompts — add next.**
4. **Require a delivery manifest that matches reality.** Ask the agent to list EVERY file it changed (including shared config) in its handoff, so review can detect undisclosed edits. — *AGH-0001* (handoff under-reported the `layers.toml` changes). **NOT yet in prompts — add next.**
5. **Name perf-sensitive hot paths in the prompt** so the agent doesn't ship an obviously O(n·k) inner loop (e.g., per-shingle hasher re-init). — *AGH-0001* (minhash). **NOT yet in prompts — add next.**

## §C. Handoff entries (append-only — newest at the bottom)

```yaml
# --- AGH-0001 ---
id: AGH-0001
date: 2026-06-18
plan: docs/superpowers/plans/2026-06-18-skill-discovery-dedup-engine.md
prompt_artifact: "Launch statement for the MCP-skills/SSOT wedge (Claude Code session, 2026-06-18) — the audited+fixed plan + inline operating rules + arch-check corrections."
prompt_version: v1
subsystem: skill-marketplace / discovery+dedup wedge (Subsystem A)
target: gemini-3.5-flash / antigravity
claude_inputs: [research-doc, spec, plan, launch-statement]
delivered: [crates/vox-similarity, crates/vox-skill-discovery, vox-discover-bin]
loc: 1007
outcome: green
verification: { tests: "19 passed", clippy: clean, arch_check: "green (after agent patched a red baseline — see deviations)", smoke: ok }
errors_encountered:
  - { what: "baseline cargo run -p vox-arch-check was red before execution", root_cause: "pre-existing layer inversion (vox-runtime↔vox-config), orphan crate (vox-mcp-llm-bridge), docstring-order, missing WTL rows", category: "arch-check-gate", who: preexisting }
agent_deviations:
  - "Promoted vox-runtime L1→L2 in layers.toml to clear the inversion (unplanned shared-arch change; may mask a real dep that should be removed instead). category: unplanned-shared-change"
  - "Added orphan_exempt to vox-mcp-llm-bridge (masks an orphan rather than fixing). category: unplanned-shared-change"
  - "Work landed on a 73-commit kitchen-sink branch mixing track-a, Clavis publish-prep, commit-lint, lean-profile. category: branch-hygiene"
  - "Handoff under-reported layers.toml changes (5 crates flipped publishable, build profiles, exempt_files) vs what the diff shows. category: scope-creep"
review_findings: "docs/superpowers/antigravity-handoff-ledger.md §C-AGH-0001-review (and the Claude Code code-review of 2026-06-18)"
verdict: approve-with-followups
prompt_lessons:
  - "Arch-check WTL+orphan rules in the prompt worked — agent honored them cleanly (keep doing this)."
  - "Add an explicit 'do NOT edit shared layers.toml / relabel layers for crates you didn't create; STOP and report a red baseline' rule (§B-2)."
  - "Add a 'branch isolation + full delivery manifest' rule so review can detect undisclosed shared-config edits (§B-3, §B-4)."
  - "Name perf-sensitive hot paths (minhash) in the prompt (§B-5)."
corrections_fed_back: []
commits: [9564245036, 4adb4a26c3, c6f608f5bf, 5866e15639, 218363b686, 5eb3ccee4e, 3dc6bf8618, 7c712bede6, ab3f6c1f46, 718be0f5e5]
```

```yaml
# --- AGH-0002 ---
id: AGH-0002
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-18-skill-discovery-followups-and-isolation.md
prompt_artifact: "D-1 → AGH-0002 — Skill-discovery follow-ups + isolation"
prompt_version: v1
subsystem: skill-marketplace / discovery+dedup (Subsystem A follow-ups)
target: gemini-3.5-flash / antigravity
claude_inputs: [plan, launch-statement]
delivered:
  - crates/vox-similarity/src/signature.rs
  - crates/vox-similarity/src/index.rs
  - crates/vox-similarity/src/lib.rs
  - crates/vox-skill-discovery/Cargo.toml
  - crates/vox-skill-discovery/src/code_miner.rs
  - crates/vox-skill-discovery/src/catalog.rs
  - crates/vox-skill-discovery/src/lib.rs
  - docs/src/architecture/layers.toml
loc: 180
outcome: green
verification: { tests: "23 passed", clippy: clean, arch_check: green, smoke: ok }
errors_encountered:
  - { what: "git cherry-pick conflict on lib.rs and code_miner.rs", root_cause: "empty/incomplete main-branch baseline vs local commits", category: "wrong-path", who: preexisting }
  - { what: "cargo build locks by concurrent agents", root_cause: "other agents running concurrently in the same workspace", category: "build-gate", who: preexisting }
agent_deviations:
  - "Used an isolated git worktree wt-skill-discovery-engine to prevent interfering with other parallel agents. category: branch-hygiene"
review_findings: ""
verdict: approve
prompt_lessons:
  - "Using git worktree is highly effective for isolating parallel agent builds and preventing git branch collision."
commits: [b4aeea3a3a, ddfb186d3b, 53d4de46c6, 85e9815325, eec21a146f, 6ae18c1d69, 6886f0e267, 00b04dc8e4, b5e30adcb8, ef02173e8c, 3e2dfc0086]
```

```yaml
# --- AGH-0003 ---
id: AGH-0003
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-18-handoff-ledger-ci-lint.md
prompt_artifact: "D-2 -> AGH-0003 - vox ci handoff-ledger lint"
prompt_version: v1
subsystem: skill-marketplace / handoff-ledger lint (Subsystem A verification)
target: gemini-3.5-flash / antigravity
claude_inputs: [plan, launch-statement]
delivered:
  - crates/vox-cli-ci/src/handoff_ledger.rs
  - crates/vox-cli-ci/src/lib.rs
  - crates/vox-cli/src/commands/ci/cmd_enums.rs
  - crates/vox-cli/src/commands/ci/run_body.rs
loc: 282
outcome: green
verification: { tests: "7 passed", clippy: clean, arch_check: green, smoke: ok }
errors_encountered: []
agent_deviations: []
review_findings: ""
verdict: approve
prompt_lessons:
  - "Line-based parsing of YAML block fields is simple, dependency-free, and robust for CI validation tasks in Rust."
commits: [103593d2db]
```

```yaml
# --- AGH-0004 ---
id: AGH-0004
date: 2026-06-19
plan: docs/superpowers/plans/2026-06-18-skill-review-gate.md
prompt_artifact: "D-3 -> AGH-0004 - Local pre-publish skill-review gate (subsystem B)"
prompt_version: v1
subsystem: skill-marketplace / pre-publish review gate (Subsystem B floor)
target: gemini-3.5-flash / antigravity
claude_inputs: [plan, launch-statement]
delivered:
  - crates/vox-skill-review/Cargo.toml
  - crates/vox-skill-review/src/lib.rs
  - crates/vox-skill-review/src/model.rs
  - crates/vox-skill-review/src/checks.rs
  - crates/vox-skill-review/src/review.rs
  - crates/vox-skill-review/src/bin/vox_skill_review.rs
  - Cargo.toml
  - docs/src/architecture/layers.toml
  - docs/src/architecture/where-things-live.md
loc: 437
outcome: green
verification: { tests: "9 passed", clippy: clean, arch_check: green, smoke: ok }
errors_encountered: []
agent_deviations: []
review_findings: ""
verdict: approve
prompt_lessons:
  - "Staleness check warning for a new crate is resolved by committing the files first, or using staleness_exempt = true in layers.toml."
corrections_fed_back: []
commits: [6e2a055621, 7f03cc41c1]
```


### AGH-0001 — review detail (human prose)
**What we asked for:** the local discovery + dedup engine (Subsystem A wedge), via the audited plan + a launch statement carrying the arch-check corrections and operating rules.

**What came back:** two clean, dependency-light crates faithful to the spec, 19 passing tests, advisory-only. The MCP↔skill SSOT byproduct (`validate_ssot`) works. The arch-check P0s from the pre-handoff audit (WTL rows, `orphan_exempt` added in Task 1 and removed in Task 5) were honored exactly — evidence the explicit arch-rule callouts in the prompt paid off.

**Code findings (follow-up fixes, none blocking):** (1) `minhash` re-inits a blake3 hasher num_hashes×shingles times — perf; (2) `WalkDir` has no ignore filtering (descends `target/`/`.git/`) — perf/false-positives; (3) silent degradation on signature-length mismatch — robustness; (4) single-linkage clustering can chain; (5) cluster score uses only first two members; (6) `dedup_skills` score is the threshold not the measured overlap; (7) test temp-dir keyed on pid.

**Process findings (the real risk):** (8) 73-commit kitchen-sink branch — skill-discovery can't be merged in isolation; cherry-pick onto a clean branch. (9) Unplanned `vox-runtime` L1→L2 promotion to clear a red baseline — needs human verification (correct layer, or should the vox-config dep be removed?).

**Net:** the *prompt* was effective (explicit arch rules → honored; dependency-light steer → no mis-wiring). The *gaps* are about constraining the agent's environment behavior (don't touch shared config, isolate the branch, report a full manifest) — now captured as §B-2…§B-5 for the next handoff.

## §D. Pending handoffs — ready-to-paste launch statements
> These are the next handoffs derived from the AGH-0001 review. When you dispatch one, copy its launch statement to the Antigravity runner AND open the matching ledger entry (AGH-0002/0003/0004) in §C. All three carry the §B hardenings inline. **Parallel-dispatch coordination:** the three plans hit disjoint crates, BUT plans D-1 and D-3 both append registration rows to `layers.toml` / `where-things-live.md` / `Cargo.toml`. Run **D-1 Tasks 1–2 first** (it owns the `vox-runtime` line + re-homes the engine), then start D-2 and D-3 in parallel; or serialize just those registration edits.

### D-1 → AGH-0002 — Skill-discovery follow-ups + isolation
> Execute `docs/superpowers/plans/2026-06-18-skill-discovery-followups-and-isolation.md` task-by-task (subagent-driven-development + TDD). Target: Gemini 3.5 Flash in Antigravity. Obey the plan's Operating Rules — especially: **no unplanned shared-config edits** (only the Task-2 `vox-runtime` line); **branch isolation** (Task 1 cherry-picks onto a clean branch off current `origin/main`); **full delivery manifest** in your handoff; **named hot path** (Task 3 minhash). Task 1 is git-surgery — if a cherry-pick conflict is not a trivial keep-both-rows merge, ABORT and escalate (do not thrash). Run the Pre-flight first, including the baseline arch-check-green gate.

### D-2 → AGH-0003 — `vox ci handoff-ledger` lint
> Execute `docs/superpowers/plans/2026-06-18-handoff-ledger-ci-lint.md`. Target: Gemini 3.5 Flash in Antigravity. Dependency-free line-based validator mirroring `commit_lint`; **the lint MUST skip the `AGH-NNNN` template block** (else it fails on its own ledger). Obey the plan's Operating Rules; fresh branch off `origin/main`. Verify with `cargo run -p vox-cli -- ci handoff-ledger` → `handoff-ledger passed.`

### D-3 → AGH-0004 — Local pre-publish skill-review gate (subsystem B)
> Execute `docs/superpowers/plans/2026-06-18-skill-review-gate.md`. Target: Gemini 3.5 Flash in Antigravity. New crate `vox-skill-review` (L3) reusing `vox_skill_discovery::{validate_ssot, dedup_skills}` + `vox_plugin_host::skill_parser::parse_skill_md`. The body is the **public field `bundle.skill_md`** (NOT a `body()` method). Deterministic + offline only; LLM pass deferred. Obey the plan's Operating Rules; new crate needs a `where-things-live.md` row + `orphan_exempt` (error-level arch rules). Verdict gate-before-listing: Error/Critical ⇒ NeedsHuman.
