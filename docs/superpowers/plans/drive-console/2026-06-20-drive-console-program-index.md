# Drive Console + Orchestrator Control — Program Index

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement each track task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Spec:** [2026-06-20-chat-drive-console-orchestrator-control-design](../../specs/2026-06-20-chat-drive-console-orchestrator-control-design.md)
**Execution target:** **Sonnet 4.6** (capable; still wants exact file paths + complete code, but can resolve a read-then-edit discovery step). The Antigravity/Flash path is downstream of this and out of scope for executing these tracks directly.

## Audit status (verified against code 2026-06-20)

Every track was adversarially verified against the real codebase. Each track file now carries an **"Audit Corrections"** block at the top — read it FIRST; it overrides any stale claim in the task bodies below it. Headline corrections:

| Track | Verdict | Must-fix before executing |
|---|---|---|
| A | Sound | Task 5 was WRONG — `ExecutionModeProfile` is used by `vox-research-shim` (6 refs). Convert to a migration, don't delete. |
| B | 2 infeasible-as-written | `ModelSelectionDecision` is NOT in scope at the completion handler; the Task-2 snippet won't compile (`to_string()`/`reason_str()` don't exist). Interrupt (Task 4) needs a new cancellation-token path — none exists. |
| C | Fixable | `Icon.stop` doesn't exist; `Segment`/`Popover` are Loquela-local (not exported); `running` state isn't wired; `ChatPayload` lives in `types/tauri.ts` and has no `context`/`clutch`/`risk` yet. |
| D | Name collision | `TaskPhase` ALREADY EXISTS (`{Inspect,Localize,Hypothesize,Act,Verify,Decide}`) — rename ours to `PavPhase`. `TaskPhaseChanged` event already exists. `PlanModeTrigger::default()` doesn't exist. No `vox://` channel (direct broadcast). |
| E | 1 architectural mismatch + 1 blocker | No `FeedbackStore` — it's `PendingApprovals` (MCP) + `HitlApprovalRow` (DB). Mesh gate fns don't take `task` (hook the caller). `panelRegistry` does NOT exist yet (hard external dep). |
| F | `.vox` path infeasible | Vox scripts can't spawn processes; `agy_*`/`ledger_append`/`worktree_create` aren't builtins. `vox design execute` MUST be a Rust `vox-cli` command. `useMetricSeries.ts` already exists — extend it. No `agy` integration exists in-repo. |

## Why a program, not one plan

The spec touches six subsystems. Per writing-plans scope-check, each becomes its own plan that produces
working, testable software on its own. Build in this order (dependency-driven, per spec §12):

| Track | Plan | Produces | Depends on | Parallelism |
|---|---|---|---|---|
| **A** | [track-a-backend-control-ssot](2026-06-20-track-a-backend-control-ssot.md) | `ClutchProfile` + `RiskPosture` pure-logic SSOT + `drive-console.v1.yaml` contract + parity gate | none | foundational — land first |
| **B** | [track-b-attribution-and-interrupt](2026-06-20-track-b-attribution-and-interrupt.md) | `completing_model`/IO capture on `CompletionAttestation`, `model_id` on `ChatMessageDto`, `orch.interrupt_task` | A (RiskPosture types) | `[SEQUENTIAL]` after A |
| **C** | [track-c-drive-console-ui](2026-06-20-track-c-drive-console-ui.md) | `DriveConsole.tsx`, `RiskPopover.tsx`, `ModelBadge.tsx`; replaces `LQ_MODES`+risk pill in `Loquela.tsx` | A (contract), B (badge data) | `[SEQUENTIAL]` after B |
| **D** | [track-d-plan-act-verify-loop](2026-06-20-track-d-plan-act-verify-loop.md) | wire `mode`→`PlanModeTrigger`+`Verification`, auto-chain plan→act→verify, phase-boundary intervention | A, C | `[SEQUENTIAL]` after C |
| **E** | [track-e-mission-control-mesh](2026-06-20-track-e-mission-control-mesh.md) | `MissionControlPanel.tsx`, mesh executor audit + local-only/exclude-peer policy, "Needs You" approval inbox, subagent tree | dockable-workspace panelRegistry | `[PARALLEL-SAFE]` with C/D |
| **F** | [track-f-dashboard-and-automation](2026-06-20-track-f-dashboard-and-automation.md) | top-bar→dashboard, `vox.metric.series.v1`, scrollbar theming, `vox design execute` Antigravity dispatcher | dashboard-topbar-unification spec | `[PARALLEL-SAFE]` with C/D/E |

## External dependencies (other specs, not re-planned here)

- **panelRegistry/dockview** — from [dockable-workspace-context-memory-ssot](../../specs/2026-06-19-dockable-workspace-context-memory-ssot-design.md). Track E docks into it.
- **Budget SSOT** — from [unified-task-message-envelope-registers-budget-ssot](../../specs/2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md). Tracks A/C consume it for cost co-location + clutch budget aggressiveness.

## Execution conventions (all tracks)

- Each task: exact files, TDD (failing test → run → minimal impl → run → commit), complete code, no placeholders.
- Rust: never pipe `cargo` to `head`/`grep` on Windows (orphan-process leak) — redirect to a file
  (`cargo test … 2>log; tail -30 log`).
- Touch-crate clippy before any admin-merge: `cargo clippy -p <crate> -- -D warnings`.
- Exclude `vox-gui` from workspace clippy sweeps (Tauri build-script breaks `--all-targets`).
- GUI tests: `cd crates/vox-gui/ui && npm test -- <path>` (script `"test": "vitest run"`) and `npm run typecheck`
  (`tsc --noEmit`). `npx vitest run <path>` also works.
- Verify the read-then-edit discovery steps against the live code before editing — line numbers drift.
- Order: A → B → C → D sequential; E and F parallel-safe with C/D but each has a hard external/architectural
  dep (see Audit status). Do A's research-shim migration before anything imports `ClutchProfile`.

## Status

- [x] Spec written + committed
- [x] Track A written
- [x] Tracks B–F written
- [x] Plan audit pass — adversarial verification against codebase; per-track "Audit Corrections" blocks added (2026-06-20)
- [ ] Execution (Sonnet 4.6), in dependency order
