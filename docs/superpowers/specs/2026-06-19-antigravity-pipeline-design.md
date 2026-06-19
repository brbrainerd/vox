---
title: "Antigravity Pipeline — Claude-Code↔Gemini Delegation Loop (Design)"
description: "Design spec for the four-stage, merge-gated pipeline that mediates between Claude Code (Opus 4.8, plan author) and Google Antigravity / Gemini 3.5 Flash (plan executor): a thin deterministic vox_agy_pipeline harness (delegate→capture spend+diff→run gates→classify→write ledger→verdict report) plus an antigravity-pipeline protocol skill (Stage-1 authoring discipline + Stage 3-4 correct-and-fix loop). Builds on the agy delegation primitives landed 2026-06-19."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Antigravity Pipeline — Design Spec

**Goal.** Provide an end-to-end, mostly-automated, **merge-gated** pipeline that mediates
between **Claude Code (Opus 4.8)** as plan *author* and **Antigravity / Gemini 3.5 Flash**
as plan *executor*, hardened against Gemini's documented failure modes and structured so the
recurring "green gates ≠ correct code" failure is caught by **deterministic gate execution**,
not by assertion.

**Architecture (one line).** Hybrid (brainstorming Approach C): a thin deterministic tool
(`vox_agy_pipeline`) does the mechanical, verification-critical work; an in-repo protocol
skill (`antigravity-pipeline`) carries the LLM-judgment work (authoring + correction). It is
the same skill+tool pairing we already ship one level down (`delegate-gemini` +
`vox_agy_delegate`).

---

## 1. Decisions locked in this design

| Decision | Choice | Rationale |
|---|---|---|
| Human-in-loop boundary | **Gate at merge only** | Auto-delegate + auto-verify in the jail; human reviews diff + approves merge. Matches the worktree-jail safety model and the ledger verdict step. |
| Drive model | **Hybrid (Approach C)** | Deterministic where the ledger proved we need it (verification, gating, ledger-writing); flexible where judgment is required (authoring, correction). |
| First live target | **Deferred** | Design only; pick the first end-to-end target after the spec + plan land and `agy` auth is confirmed. |

## 2. What already exists (this is mostly composition)

- **MCP tools:** `vox_agy_doctor`, `vox_agy_delegate`, `vox_agy_delegate_batch`, `vox_credentials_status`.
- **Primitives:** `agy_exec` (kill_on_drop, timeout, arg sanitisation), `agy_worktree`
  (jail under `.vox/agy-worktrees/<slug>/` on branch `agy/<slug>`, diff capture),
  `agy_ledger` (serialized AGH-NNNN append).
- **SSOTs / docs:** `docs/superpowers/antigravity-handoff-ledger.md` (journal + §B lessons
  checklist), `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`
  (§5 plan-engineering constraints), `docs/src/architecture/antigravity-credits-auth-and-limitations-2026-06-19.md`.
- **Skills (in-repo):** `delegate-gemini`, `brainstorming`, `writing-plans`, `research`,
  `deep-research`, `requesting-code-review`, `dispatching-parallel-agents`,
  `verification-before-completion`, `using-git-worktrees`.
- **Arch guard:** `raw-agy-exec` forbidden-pattern (all `agy` spawns go through `AgyExec`).

## 3. The four stages

```
  STAGE 1: AUTHOR                STAGE 2: EXECUTE & MEDIATE         STAGE 3-4: CORRECT-AND-FIX + REPORT
  (Claude Code / Opus 4.8)       (vox_agy_pipeline tool)           (skill-driven, deterministic gates)
  skill-driven judgment          deterministic harness

  a. codebase audit (rg          doctor gate (agy ready)           code-review jailed diff
     verify-before-use)       →  worktree jail                  →  (prove EFFECT not shape)
  b. web research (targeted)     agy -p (auto-accept)              outcome != green?
  c. plan-engineer               capture spend + diff                distill lesson → ledger §B
     (writing-plans + §B         RUN GATES AS WRITTEN                re-delegate ONCE (two-strike)
      + limitations §5)          classify green/partial/failed     coverage report
                                 write ledger entry                 HUMAN MERGE GATE ✋
                                 verdict-ready report
        ▲                                                                   │
        └─────────────── lessons fed back into next launch statement ◀──────┘
```

### Stage 1 — Author (skill-driven judgment)
The skill enforces this discipline before any delegation:
- **a. Codebase audit (anti-hallucination baseline).** Every symbol/path/API the launch
  statement will reference is confirmed in-repo via `rg`/Grep, with exact signatures inlined
  (limitations §5; ledger B-6).
- **b. Web research (targeted).** Only when external/current knowledge is needed; 2–3 focused
  `WebFetch`/`WebSearch`, not mass fan-out (per the deep-research throttle lesson).
- **c. Plan-engineer.** Produce a hardened plan in `docs/superpowers/plans/` + a **launch
  statement** baking in: ledger §B-2…B-10, and the limitations §5 constraints — atomic
  green-committed tasks, verify-before-use, self-contained tasks, two-strike breaker,
  one-decision-per-step, PARALLEL-SAFE/SEQUENTIAL tags, "run gates exactly as written; STOP
  on an unrelated red baseline; do not weaken a gate."

### Stage 2 — Execute & mediate (deterministic tool: `vox_agy_pipeline`)
A single auditable call per task (or per plan-task) that performs, in order:
1. `vox_agy_doctor` gate → fail fast with remediation if agy not ready.
2. Create the worktree jail (reuse `agy_worktree`).
3. `agy -p <launch-statement> --dangerously-skip-permissions` (reuse `agy_exec`; never
   `--sandbox`; kill_on_drop + timeout).
4. **Capture spend signal.** Credits are not queryable headlessly, so record the *proxy*:
   `elapsed_ms`, `exit_code`, `timed_out`, `attempts` + an honest `billing: antigravity-credits`
   note (no fake USD).
5. **Capture diff.** `files_changed` + unified diff (reuse `agy_worktree.capture`).
6. **Run the gates exactly as the plan specifies** inside the jail (build / test / arch-check),
   capturing structured results. *This is the net-new defence against "hollow green."*
7. **Classify outcome:** `green` (gates pass AND files changed) / `partial` (files changed but
   a gate fails) / `failed` (no changes, hard error, or timeout).
8. **Write the ledger entry** (reuse `agy_ledger`) carrying the **real** verification block
   (not "n/a").
9. **Return a verdict-ready report:** outcome, diff, gate results, spend proxy, ledger id,
   recommended next action.

### Stage 3-4 — Correct-and-fix + report (skill-driven, deterministic gates)
- **Review the delivery** with `requesting-code-review` / the code-reviewer agent on the jailed
  diff — prove the EFFECT, not the shape (ledger B-9).
- **Two-strike correction loop.** If outcome != green OR review finds a real defect: distill the
  failure into a corrected launch statement, append the lesson to ledger §B, and re-delegate
  **once**. A second failure → STOP and hand off with a note. Never loop indefinitely
  (limitations §5: poor self-correction).
- **Coverage report.** "To what extent implemented": which plan tasks landed green / partial /
  failed, the spend proxy, and the ledger trail.
- **Human merge gate.** Present the jailed `agy/<slug>` branch + report; human approves the
  merge to `main`. The pipeline never auto-merges.

## 4. Components & responsibilities

| Unit | Responsibility | Reuses |
|---|---|---|
| `vox_agy_pipeline` (MCP tool) | Stage-2 orchestration: doctor→jail→delegate→capture→gates→classify→ledger→report | `agy_doctor`, `agy_exec`, `agy_worktree`, `agy_ledger` |
| gate-runner (helper) | Run the plan-specified gates inside the jail; capture structured pass/fail + output tail | `git_exec`/process spawn |
| classifier (pure fn) | Map (files_changed, gate results, timed_out, exit_code) → green/partial/failed | — |
| ledger verification block | Extend `agy_ledger` render to carry real gate results | `agy_ledger` |
| `antigravity-pipeline` skill | Encode the 4-stage protocol + Stage-1 authoring discipline + Stage 3-4 loop | composes existing skills |

## 5. Data flow / SSOT
- **Journal:** `docs/superpowers/antigravity-handoff-ledger.md` (written by `agy_ledger`).
- **Untrusted output:** worktree jail `.vox/agy-worktrees/<slug>/`, branch `agy/<slug>`, until
  human merge.
- **Plans:** `docs/superpowers/plans/`; **specs:** `docs/superpowers/specs/`.

## 6. Error handling (maps to limitations §5)

| Gemini failure mode | Pipeline defence |
|---|---|
| No-checkpoint mid-task termination → broken tree | Atomic green-committed tasks; jail isolates breakage; classifier marks `failed`. |
| Hallucinated APIs / phantom symbols | Stage-1 verify-before-use audit + Stage-2 gate run catches non-compiling output. |
| Quota hard cutoff (no warning) | Small tasks + spend proxy; classify `partial`/`failed`; re-delegate only the remaining tasks. |
| Poor self-correction (repeats failures) | Two-strike breaker in the skill; never infinite loop. |
| Weak long-context recall (MRCR) | Self-contained tasks in the authored plan. |

## 7. Testing strategy
- **Unit:** classifier (green/partial/failed) — pure-function tests, no live agy.
- **Unit:** gate-runner builds the right command set; report shape is well-formed.
- **Integration:** `#[ignore]`d live smoke (extends `agy_delegate_smoke.rs`) running the full
  loop on a tiny target; run manually (bills credits).
- **Self-gates:** `cargo test` + `vox-arch-check` (the `raw-agy-exec` rule already guards spawns).

## 8. Gaps to build (net-new, minimal)
1. `vox_agy_pipeline` MCP tool (orchestration over existing primitives + gate-runner + classifier).
2. gate-runner helper (run plan gates inside the jail; structured results).
3. classifier pure function.
4. `antigravity-pipeline.skill.md` (in-repo so Antigravity can mount it; limitations §4).
5. Register the tool: `dispatch.rs`, `input_schemas.rs`, `tool-registry.canonical.yaml`.
6. Extend `agy_ledger` verification block to carry real gate results.

## 9. Out of scope (YAGNI)
- A full programmatic state machine (Approach B) — deferred until the live loop is proven.
- Auto-merge / fully-autonomous mode (Approach: rejected; merge-gate chosen).
- Real credit-balance querying (not exposed by agy headlessly — documented constraint).
- GUI surface for the pipeline.

## 10. Known constraints / risks
- **agy auth unproven on this machine** (`agy models` returned empty). The pipeline's first
  action is always the doctor gate; a real live run requires interactive Google sign-in first.
- **Spend is a proxy**, not a real balance — by design, surfaced honestly.
- Skill + tool must stay in sync (same maintenance pattern as `delegate-gemini` +
  `vox_agy_delegate`).
