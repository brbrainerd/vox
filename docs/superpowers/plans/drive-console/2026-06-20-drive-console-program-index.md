# Drive Console + Orchestrator Control — Program Index

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement each track task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Spec:** [2026-06-20-chat-drive-console-orchestrator-control-design](../../specs/2026-06-20-chat-drive-console-orchestrator-control-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

## Why a program, not one plan

The spec touches six subsystems. Per writing-plans scope-check, each becomes its own plan that produces
working, testable software on its own. Build in this order (dependency-driven, per spec §12):

| Track | Plan | Produces | Depends on | Parallelism |
|---|---|---|---|---|
| **A** | [track-a-backend-control-ssot](2026-06-20-track-a-backend-control-ssot.md) | `ClutchProfile` + `RiskPosture` pure-logic SSOT + `drive-console.v1.yaml` contract + parity gate | none | foundational — land first |
| **B** | track-b-attribution-and-interrupt *(to write)* | `completing_model`/IO capture on `CompletionAttestation`, `model_id` on `ChatMessageDto`, `orch.interrupt_task` | A (RiskPosture types) | `[SEQUENTIAL]` after A |
| **C** | track-c-drive-console-ui *(to write)* | `DriveConsole.tsx`, `RiskPopover.tsx`, `ModelBadge.tsx`; replaces `LQ_MODES`+risk pill in `Loquela.tsx` | A (contract), B (badge data) | `[SEQUENTIAL]` after B |
| **D** | track-d-plan-act-verify-loop *(to write)* | wire `mode`→`PlanModeTrigger`+`Verification`, auto-chain plan→act→verify, phase-boundary intervention | A, C | `[SEQUENTIAL]` after C |
| **E** | track-e-mission-control-mesh *(to write)* | `MissionControlPanel.tsx`, mesh executor audit + local-only/exclude-peer policy, "Needs You" approval inbox, subagent tree | dockable-workspace panelRegistry | `[PARALLEL-SAFE]` with C/D |
| **F** | track-f-dashboard-and-automation *(to write)* | top-bar→dashboard, `vox.metric.series.v1`, scrollbar theming, `vox design execute` Antigravity dispatcher | dashboard-topbar-unification spec | `[PARALLEL-SAFE]` with C/D/E |

## External dependencies (other specs, not re-planned here)

- **panelRegistry/dockview** — from [dockable-workspace-context-memory-ssot](../../specs/2026-06-19-dockable-workspace-context-memory-ssot-design.md). Track E docks into it.
- **Budget SSOT** — from [unified-task-message-envelope-registers-budget-ssot](../../specs/2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md). Tracks A/C consume it for cost co-location + clutch budget aggressiveness.

## Flash execution conventions (all tracks)

- Each task: exact files, TDD (failing test → run → minimal impl → run → commit), complete code, no placeholders.
- Vox-language gotchas (from prior sessions): run `.vox` automation with `--mode interp`; no multi-line `+`
  expressions; no `list.set`; single-line fn sigs. Rust: never pipe `cargo` to `head`/`grep` on Windows
  (orphan-process leak) — redirect to a file.
- Touch-crate clippy before any admin-merge: `cargo clippy -p <crate> -- -D warnings`.
- Exclude `vox-gui` from workspace clippy sweeps (Tauri build-script breaks `--all-targets`).
- Mark each task `[PARALLEL-SAFE]` or `[SEQUENTIAL]` in its header for the Antigravity dispatcher (Track F).

## Status

- [x] Spec written + committed
- [x] Track A written
- [ ] Tracks B–F written (generate on request)
- [ ] Plan audit pass (correct against codebase, quality)
- [ ] Antigravity/Flash execution
