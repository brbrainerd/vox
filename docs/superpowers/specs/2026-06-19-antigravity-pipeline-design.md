---
title: "Antigravity Pipeline — Claude-Code↔Gemini Delegation Loop (Design)"
description: "Design spec for the four-stage, merge-gated pipeline that mediates between Claude Code (Opus 4.8, plan author + adversarial reviewer) and Google Antigravity / Gemini 3.5 Flash (plan executor): a thin deterministic vox_agy_pipeline harness (delegate→capture spend+diff→run gates→classify→provisional ledger→verdict report), an automated Claude-side adversarial review that records a ledger review-addendum, and a flywheel digest that feeds historical Gemini failures back into the next launch statement. Plus an antigravity-pipeline protocol skill and the full process-skill set ported in-repo so all agents (incl. Antigravity) can mount them."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
---

# Antigravity Pipeline — Design Spec

**Goal.** An end-to-end, mostly-automated, **merge-gated** pipeline mediating between
**Claude Code (Opus 4.8)** as plan *author + adversarial reviewer* and **Antigravity / Gemini
3.5 Flash** as plan *executor*, hardened against Gemini's documented failure modes and built so
the recurring "green gates ≠ correct code" failure is caught by **deterministic gate execution**
(not assertion) and every Gemini failure **learns forward** through a closed flywheel.

**Architecture (one line).** Hybrid (brainstorming Approach C): thin deterministic tools do the
verification-critical mechanical work; an in-repo protocol skill carries the LLM-judgment work
(authoring + adversarial review + correction). Same skill+tool pairing we ship one level down
(`delegate-gemini` + `vox_agy_delegate`).

---

## 1. Decisions locked in this design

| Decision | Choice | Rationale |
|---|---|---|
| Human-in-loop boundary | **Gate at merge only** | Auto-delegate + auto-verify + auto-review in the jail; human reviews + approves merge. |
| Drive model | **Hybrid (Approach C)** | Deterministic where the ledger proved we need it (verification, gating, recording); flexible where judgment is required (authoring, review, correction). |
| First live target | **Deferred** | Design only; pick the first target after spec + plan land and `agy` auth is confirmed. |
| **Ledger storage** | **Flat Markdown file (keep), + a `digest` reader** | The ledger is greppable, git-tracked, human-reviewable, and low-volume (handoffs are infrequent). vox-db is **rejected as premature** (over-engineering for ≤dozens of entries); the `digest` reader is the swap-seam if volume ever justifies a DB. |
| **Ledger lifecycle** | **Provisional entry at delegation + append-only `{id}-review` addendum after review** | Failures are recorded immediately (never lost to the flywheel) and the review outcome is appended, preserving append-only and matching the existing `AGH-0001-review` convention. |
| **Review automation** | **Auto-dispatch `code-reviewer` agent + record via `vox_agy_review`; human still merges** | The adversarial reasoning is automated; the verdict/lessons are recorded deterministically; the merge decision stays human. |

## 2. What already exists (this is mostly composition)

- **MCP tools:** `vox_agy_doctor`, `vox_agy_delegate`, `vox_agy_delegate_batch`, `vox_credentials_status`.
- **Primitives:** `agy_exec` (kill_on_drop, timeout, arg sanitisation), `agy_worktree`
  (jail under `.vox/agy-worktrees/<slug>/` on branch `agy/<slug>`, diff capture, cleanup),
  `agy_ledger` (serialized AGH-NNNN append).
- **SSOTs / docs:** `docs/superpowers/antigravity-handoff-ledger.md` (journal + §B lessons +
  the `{id}-review` addendum convention), the Gemini limitations §5 + agy-credits docs.
- **Skills (in-repo today):** `delegate-gemini`, `brainstorming`, `research`, `deep-research`,
  `requesting-code-review`, `dispatching-parallel-agents`, `verification-before-completion`,
  `using-git-worktrees`.
- **Agent:** `superpowers:code-reviewer` (used for the automated adversarial review).
- **Arch guard:** `raw-agy-exec` forbidden-pattern.

## 3. The four stages

```
  STAGE 1: AUTHOR                 STAGE 2: EXECUTE & VERIFY        STAGE 3-4: ADVERSARIAL REVIEW,
  (Claude Code / Opus 4.8)        (vox_agy_pipeline tool)          CORRECT-AND-FIX, LEARN + MERGE
  skill-driven judgment           deterministic harness            (skill + code-reviewer agent + tools)

  0. flywheel: vox_agy_ledger_     doctor gate (agy ready)          code-reviewer agent vs jailed diff
     digest → failure-category  →  worktree jail                 →  (adversarial, §B-seeded template)
     freqs + active §B lessons      agy -p (auto-accept)             → vox_agy_review records
  a. codebase audit (rg             capture spend + diff               {id}-review addendum
     verify-before-use)             RUN GATES AS WRITTEN (jail)        (verdict + categories + lessons)
  b. web research (targeted)        classify green/partial/failed    outcome != green OR review fails?
  c. plan-engineer (writing-plans   write PROVISIONAL ledger entry     re-delegate ONCE (two-strike)
     + digest + §B + limits §5)     verdict-ready report             coverage report
                                    cleanup jail iff no changes      HUMAN MERGE GATE ✋
        ▲                                                                       │
        └──── digest distils review addenda → injected into next launch ◀───────┘  (flywheel closed)
```

### Stage 1 — Author (skill-driven judgment)
- **0. Flywheel input.** Call `vox_agy_ledger_digest` → historical failure-category frequencies
  + the active §B lessons. Inject the top recurring categories as explicit "avoid this" rules in
  the launch statement (this is the learn-forward step).
- **a. Codebase audit (anti-hallucination).** Confirm every symbol/path/API in-repo via
  `rg`/Grep; inline exact signatures (limitations §5; ledger B-6).
- **b. Web research (targeted).** Only when needed; 2–3 focused fetches (deep-research throttle).
- **c. Plan-engineer** the launch statement, baking in §B-2…B-10 + limitations §5 (atomic
  green-committed tasks, self-contained tasks, one-decision-per-step, PARALLEL-SAFE/SEQUENTIAL,
  "run gates exactly as written; STOP on an unrelated red baseline").

### Stage 2 — Execute & verify (deterministic: `vox_agy_pipeline`)
1. `vox_agy_doctor` gate (fail fast with remediation).
2. Create the worktree jail.
3. `agy -p <launch-statement> --dangerously-skip-permissions` (never `--sandbox`; kill_on_drop + timeout).
4. **Capture spend proxy** (elapsed_ms/attempts/exit/timed_out + honest credits note — credits aren't queryable).
5. **Capture diff** (files_changed + unified diff).
6. **Run the plan-specified gates inside the jail**, each with timeout + kill_on_drop, optional
   `env` (e.g. `CARGO_TARGET_DIR` pointing at the main repo's target so cargo gates reuse cache
   instead of cold-rebuilding the worktree). *Net-new defence against hollow-green.*
7. **Classify** green / partial / failed by EFFECT (files_changed + gate results + timed_out).
8. **Write a PROVISIONAL ledger entry** (verdict: request-changes, review_findings: pending) so
   the failure is recorded even if review never happens.
9. **Cleanup the jail iff no changes** (a `failed`/empty run leaves no dead worktree).
10. **Return a verdict-ready report** (outcome, diff, gates, spend proxy, ledger id, next step).

### Stage 3-4 — Adversarial review, correct-and-fix, learn + merge (skill + agent + tools)
- **Automated adversarial review.** Dispatch the `superpowers:code-reviewer` agent against the
  jailed diff with a §B-seeded template that hunts the known Gemini failure modes: hallucinated
  APIs, hollow-green (tests assert shape not behavior), unplanned shared-config edits, scope
  creep, gate-weakening, effect-vs-shape. Prove the EFFECT (ledger B-9).
- **Record the review.** `vox_agy_review` appends a `{id}-review` addendum to the ledger:
  `verdict` (approve / approve-with-followups / request-changes), `categories` (from the stable
  §B vocabulary), `findings`, and 1–3 `prompt_lessons`. This is what the flywheel mines.
- **Two-strike correction.** If outcome != green OR the review verdict is request-changes:
  distill the failure into a corrected launch statement and re-delegate **once**. Second failure
  → STOP + hand off. Never loop (limitations §5).
- **Coverage report** ("to what extent implemented") + the ledger trail.
- **Human merge gate.** Present the jailed `agy/<slug>` branch + report + review addendum; human
  approves the merge. The pipeline never auto-merges.

## 4. Components & responsibilities

| Unit | Responsibility | New/reuse |
|---|---|---|
| `agy_gates.rs` (`Gate`/`GateResult`/`run_gate`/`run_gates`) | Run a plan gate in the jail; timeout+kill_on_drop+optional `env`; structured pass/fail | New |
| `agy_pipeline.rs` `classify_outcome` (pure) | (files_changed, gates, timed_out) → green/partial/failed | New |
| `agy_pipeline.rs` `vox_agy_pipeline` (tool) | Stage-2 orchestration (provisional ledger + jail cleanup) | New, reuses primitives |
| `agy_ledger.rs` `with_verification` | Real verification block in the entry | Extend |
| `agy_ledger.rs` `ReviewRecord` + `append_review_locked` | Append `{id}-review` addendum (verdict/categories/findings/lessons) | New |
| `agy_ledger.rs` `ledger_digest` (read) | Aggregate failure-category freqs + active lessons | New |
| `vox_agy_review` (tool) | Stage-3 deterministic record of the adversarial review | New |
| `vox_agy_ledger_digest` (tool) | Flywheel input for Stage 1 | New |
| `antigravity-pipeline` skill | The 4-stage protocol incl. adversarial-review template + flywheel step; references in-repo skills | New |
| ported skills | `writing-plans`, `executing-plans`, `subagent-driven-development`, `test-driven-development` into `crates/vox-skills/skills/superpowers/` so all agents (incl. Antigravity) mount them | New (copy+adapt) |

## 5. Data flow / SSOT
- **Journal (human SSOT):** `docs/superpowers/antigravity-handoff-ledger.md` — provisional entry
  (`agy_ledger`) + `{id}-review` addendum (`append_review_locked`); mined by `ledger_digest`.
- **Untrusted output:** worktree jail `.vox/agy-worktrees/<slug>/`, branch `agy/<slug>`, until human merge.
- **Plans:** `docs/superpowers/plans/`; **specs:** `docs/superpowers/specs/`.
- **Storage decision:** flat file (not vox-db) — see §1.

## 6. Error handling (maps to limitations §5)

| Gemini failure mode | Pipeline defence |
|---|---|
| No-checkpoint mid-task termination → broken tree | Atomic green-committed tasks; jail isolates breakage; classifier marks `failed`; provisional entry records it. |
| Hallucinated APIs / phantom symbols | Stage-1 verify-before-use + Stage-2 gate run + Stage-3 adversarial review category `hallucinated-api`. |
| Quota hard cutoff | Small tasks + spend proxy; classify `partial`/`failed`; re-delegate remaining only. |
| Poor self-correction | Two-strike breaker; never infinite loop. |
| Weak long-context recall | Self-contained authored tasks. |
| Same error recurs across handoffs | **Flywheel:** `ledger_digest` surfaces the recurring category → Stage-1 bakes an explicit avoid-rule. |

## 7. Testing strategy
- **Unit (no live agy):** `classify_outcome`; gate-runner (pass/fail/missing-binary/order);
  `with_verification` override; `append_review_locked` round-trip; `ledger_digest` aggregation;
  `pipeline_validate`/`review_validate` parsing.
- **Integration:** `#[ignore]` live smoke (full loop on a tiny target; bills credits).
- **Self-gates:** `cargo test` + `vox-arch-check` (`raw-agy-exec` guards spawns).

## 8. Gaps to build (net-new, minimal)
1. `agy_gates.rs` gate-runner (with optional `env`).
2. `agy_pipeline.rs` classifier + `vox_agy_pipeline` (provisional ledger + jail cleanup).
3. `agy_ledger.rs`: `with_verification`, `ReviewRecord`+`append_review_locked`, `ledger_digest`.
4. `vox_agy_review` + `vox_agy_ledger_digest` tools.
5. Register all tools (dispatch, input_schemas, tool-registry.canonical.yaml).
6. `antigravity-pipeline.skill.md` (references in-repo skills + adversarial template + flywheel).
7. Port `writing-plans`/`executing-plans`/`subagent-driven-development`/`test-driven-development`
   into `crates/vox-skills/skills/superpowers/`.
8. `#[ignore]` live smoke.

## 9. Out of scope (YAGNI)
- Full programmatic state machine (Approach B) — until the live loop is proven.
- Auto-merge / fully-autonomous — merge-gate chosen.
- vox-db ledger storage — flat file is sufficient at this volume (revisit if it grows).
- Real credit-balance querying (not exposed by agy headlessly).
- GUI surface.

## 10. Known constraints / risks
- **agy auth unproven** (`agy models` empty) — doctor gate is always first; live run needs interactive sign-in.
- **Spend is a proxy**, surfaced honestly.
- **Cargo gates in a fresh worktree cold-rebuild** unless `CARGO_TARGET_DIR` is shared via the
  gate `env` — gates must be scoped to the touched crate AND set the shared target dir.
- Skill + tools must stay in sync (same pattern as `delegate-gemini` + `vox_agy_delegate`).
