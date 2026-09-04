# Platform Parity Program — Master Sequencer

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> This file is the **sequencer**. Do not implement product code from this file. Implement from the child plans. After each child plan task, flip the ID in §Ledger.

**Goal:** Sequence the six track plans (plus two already-written plans) so every ID in [`2026-08-31-platform-parity-id-coverage.md`](../specs/2026-08-31-platform-parity-id-coverage.md) ships as independently testable software. The coverage file quotes the original 2026-08-31 diagnosis canvas (`ITEMS[]`) and names files + failing tests. Spec **§9 audit errata overrides** any child-plan text that still names a false-positive API. Coverage v1 overrides skip/optional language.

**Architecture:** Track 0 executes existing plans (budget guard already live — skip remaining guard tasks). Tracks 1, 2, and 5 run in parallel (disjoint crates). Track 1 **Task 0** (`vox_write_file` handler) before Apply/Modified-exec. Track 3 waits on Track 1 Apply/modes and **does not** own remaining GUI IDs. Track 6 (GUI product) waits on Gate A. Track 4 waits on Track 2’s `known-slugs.v1.json` (M01/M02 guard already enforced). Shared types live in the spec, not here.

**Tech Stack:** Rust workspace, Tauri 2, React/vitest, GitHub Actions, `vox-db` (libSQL), OpenRouter PKCE (`vox-oauth-pkce`), MCP.

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md)

## Global Constraints

Copied from spec §6 — every child plan inherits these. Do not weaken them.

- Test-first for every new `pub fn`.
- Never `cargo fmt --all`. Use `vox run scripts/fmt.vox` or `cargo fmt -p <crate>`.
- Windows cargo: `& "$env:USERPROFILE\.cargo\bin\cargo.exe"` from repo root.
- Secrets only via `vox_secrets::resolve_secret`.
- No new `VOX_*` without `contracts/config/env-vars.v1.yaml`.
- LLM only through `vox_actor_runtime::llm`.
- No new crate-edges; no `exceptions` ledger edits by agents.
- GUI sidecar: `vox run scripts/gui-build.vox` before `cargo test -p vox-gui`.
- No `--no-verify` commits.

---

## File structure (program-level)

| File | Responsibility |
|---|---|
| `docs/superpowers/specs/2026-08-31-platform-parity-design.md` | Types, IDs, non-goals |
| `docs/superpowers/specs/2026-08-31-platform-parity-id-coverage.md` | **Normative** original-canvas `gap`/`fix` → feasible v1 (beats skip/optional in child plans) |
| `docs/superpowers/plans/2026-08-31-platform-parity-program.md` | This sequencer + ledger |
| `docs/superpowers/plans/2026-08-31-trust-loop-approvals-apply-agent.md` | Track 1 |
| `docs/superpowers/plans/2026-08-31-language-ai-authorship.md` | Track 2 |
| `docs/superpowers/plans/2026-08-31-harness-productization.md` | Track 3 |
| `docs/superpowers/plans/2026-08-31-model-router-local-cloud.md` | Track 4 |
| `docs/superpowers/plans/2026-08-31-runtime-cross-platform-honesty.md` | Track 5 |
| `docs/superpowers/plans/2026-08-31-gui-product-axis.md` | Track 6 GUI product (Axis journeys) |
| `docs/superpowers/plans/2026-08-01-free-tier-onboarding.md` | Track 0A (already written; budget guard largely shipped) |
| `docs/superpowers/plans/2026-08-28-chat-harness-unification.md` | Track 0B (already written) |

## Parallelization

```
Batch A (start immediately, disjoint):
  Track 0A  remaining free-tier UX (skip re-implementing daily_budget_usd guard)
  Track 0B  remaining chat_turn unification tasks
  Track 1   trust loop (Task 0 vox_write_file first)
  Track 2   language (bar sugar on Lambda, not HirExpr::Closure)
  Track 5   runtime/CI (docs + required-check; not add pull_request)

Batch B (after Track 1 Tasks 0–8 green — Gate A):
  Track 3   harness (hooks, rules inject, tool cap, worktrees) — NO G04–G18 dump
  Track 6   GUI product (Apply UI already in Track 1; modes/rollback/honesty/keybinds)

Batch C (after Track 2 L11 snapshot):
  Track 4   model router (M09 = MCP resolver regression; M03 = named const)
```

Hot files (serialize even across tracks):

| File | Tracks |
|---|---|
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | 1, 3 |
| `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs` | 1, 3, 4 |
| `crates/vox-gui/ui/src/App.tsx` | 1, 6 |
| `crates/vox-gui/ui/src/components/surfaces/Loquela/*` | 1, 6 |
| `crates/vox-gui/ui/src/components/surfaces/Approvals/*` | 1, 6 |
| `contracts/config/env-vars.v1.yaml` | 1, 2, 4, 5, 6 |

### Task 0: Citation sweep

**Files:** none (read-only).

- [ ] **Step 1: Re-verify spec citations**

Confirm these still match HEAD (the spec was written against them):

- `dispatch.rs` truncates args with `.chars().take(200)` and `APPROVAL_TIMEOUT` 300s; **`vox_write_file` has no match arm**
- `agent_loop.rs` `DEFAULT_MAX_ITERATIONS = 8` and `for call in &calls`; `message.rs` hardcodes `permission_mode: None`
- `App.tsx` `/rollback` invokes `vox_undo` with `args: {}`; MCP list tool is `vox_oplog`; list field is `id`
- `DiffReview.tsx` has no Accept/Reject handlers
- `HirExpr::Lambda` exists; `Token::Bar` exists; **do not add Closure**
- `ai_schema_ctx::schema_for` exists
- `cross-platform-check.yml` already has `pull_request`
- `crates/vox-oauth-pkce` exists
- ContextWindowMeter / IntentPanel / keybinds capture / PlanPanel / secretary propose-only / `decide()` on MCP chat / `scoreboard_feedback_boost * 0.15` / `enforce_budget_guard` — shipped (spec §9.1)

If any drifted, patch the spec **and** the affected child plan in the same commit as the sweep, then continue.

- [ ] **Step 2: Commit only if the spec needed a citation fix**

```bash
git add docs/superpowers/specs/2026-08-31-platform-parity-design.md docs/superpowers/plans/2026-08-31-*.md
git commit -m "docs: refresh platform-parity citations against HEAD"
```

If nothing drifted, skip the commit.

---

### Task 1: Start Batch A

**Files:** none (dispatch).

**Interfaces:**
- Consumes: spec §5 track table
- Produces: four in-flight child-plan executions (0A/0B optional if already green)

- [ ] **Step 1: Check Track 0A status**

Run: `rg -n "daily_budget_usd" crates/vox-orchestrator-mcp/src crates/vox-config --glob "*.rs"`

If a production path **reads** `daily_budget_usd` and blocks dispatch, mark M02 done in the ledger and skip remaining budget tasks in the free-tier plan. If not, execute that plan from Task 1.

- [ ] **Step 2: Dispatch child plans**

Assign agents to:

1. `docs/superpowers/plans/2026-08-31-trust-loop-approvals-apply-agent.md` from Task 1
2. `docs/superpowers/plans/2026-08-31-language-ai-authorship.md` from Task 1
3. `docs/superpowers/plans/2026-08-31-runtime-cross-platform-honesty.md` from Task 1
4. Remaining tasks of `2026-08-01-free-tier-onboarding.md` and `2026-08-28-chat-harness-unification.md` (skip re-implementing `daily_budget_usd` guard)

After Gate A: Track 3 + Track 6 (`2026-08-31-gui-product-axis.md`). After L11: Track 4.

- [ ] **Step 3: Do not start Track 3, 4, or 6 yet**

Expected: Track 3/4/6 plans are read-only until Batch A / Gate A pass. Track 6 starts with Track 3 after Gate A.

---

## Gates

**Gate A (unlocks Track 3 + Track 6):** Track 1 closes H19, H02, H03 (list-after-restart), H04, H01 (`iterations_left`), H09 (`plan_blocks` + `vox_exit_plan_mode`), G01 (keep-diff), rollback (`vox_oplog` + `.id`). Tests: `cargo test -p vox-orchestrator-mcp -p vox-orchestrator -p vox-gui` plus vitest `parseUnifiedDiff` + `rollbackLast`. Composer G03 **UI** may land in Track 6; Track 1 must thread `permission_mode` through `message.rs`.

**Gate B (unlocks Track 4 remaining):** Track 2 L11 snapshot file exists. M01/M02 guard already live — do not block on re-implementing it. M03 is a named-const + fixture test, not a FAIL-unbounded test.

**Gate C (program done):** spec §7 acceptance checklist; every **original** canvas ID is `done` in the ledger (or-gates R04/R05/R06/G07 may be `done` on the chosen arm with the other arm named residual). Spec §9 FPs mean “do not rebuild the engine,” not “skip the remaining original fix.”

---

## Self-review (2026-08-31 evening audit)

**Spec coverage:** Every original canvas ID maps through the coverage spec. Track 0A (M01 M02-guard G02), Track 0B (chat unification), Track 1 (H01–H04 H09 ExitPlanMode H16 H19 G01; G03 wire), Track 2 (L01–L20 split T8a–T9e), Track 3 (H05–H08 H11 H13-remaining H14 H15 H18), Track 4 (M03–M14 split), Track 5 (R01–R12 split T1b T4a–T5e), Track 6 (G03 UI, G04 daemon, G05–G18 remaining original fixes, G19–G27, H01 rail, H10 H12 H16-UI H17, M02 $, R11 GUI). L19 is a loud diagnostic.

**False positives (do not rebuild engines):** meter, IntentPanel, keybind capture, PlanPanel list, `vox_browser_snapshot` name, secretary propose-only, M09 `chat_turn` rebind, R01 add-`pull_request`, L01 `HirExpr::Closure`, `project_dna.rs`. Remaining original **fixes** for those IDs are still `open` (compact, persist, six keybinds, ExitPlanMode Approve, confidence gate, interp golden, VRAM admit, …).

**Type consistency:** `Modified { args: Value }` drops `Copy`/`Eq`. Rollback uses `vox_oplog` + list `.id`. Closures use `HirExpr::Lambda`. Timeout on `HitlPolicy`. `vox test --json` is one schema.

---

## Ledger

Flip status as child tasks land. Start = `open`. Do not delete IDs.

| ID | Track | Status |
|---|---|---|
| L01–L20 | 2 | open (T8a–T8e, T9a–T9e) |
| R01 | 5 | open (required-check + T1 lib + T1b `--interp`; not add `pull_request`) |
| R02 R07 | 5 | open |
| R03–R06 | 5 | open (T4a–T4d; R04/R05/R06 chosen or-gates) |
| R08–R12 | 5 | open (T5a–T5e; R11 GUI string → T6 T27) |
| H01–H04 H09 H16 H19 | 1 | open (H09 includes `vox_exit_plan_mode`; H01 includes `iterations_left`) |
| H05–H08 H11 H14 H15 H18 | 3 | open |
| H13 | 3 | open remaining (Task 5b accept_all+0.9; propose-only engine already shipped) |
| H10 H12 H17 | 6 | open |
| G01 | 1 | open (keep-diff until applied) |
| G03 wire (`message.rs`) | 1 | open |
| G03 composer UI | 6 | open |
| G02 | 0A | open |
| G04 | 6 | open (T15 LAN daemon queue — not copy-only) |
| G05 G07 G11 G13 G14 G16 G17 G18 | 6 | open |
| G06 G08 G09 G10 G12 G15 | 6 | open (T23 warn/compact, T22 persist, T22b Plan default-on, T21 ExitPlanMode, T12 six actions, T19 live names+gate) |
| G19–G27 | 6 | open (audit plumbing) |
| M01 | 0A | open (wizard) |
| M02 | 0A + 6 T29 + 4 T6 | guard shipped; `$ remaining` + Exceeded local retry open |
| M03–M14 | 4 | open (split T3a–T5d; M09 = resolver regression) |
