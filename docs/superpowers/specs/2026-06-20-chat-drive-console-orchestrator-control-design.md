# Chat Drive Console + Orchestrator/Model Control Surface — Design Spec

**Date:** 2026-06-20
**Status:** Design (approved for planning)
**Author:** Brainstorming session (Claude, Opus 4.8)
**Builds on / addendum to:** [dashboard-topbar-unification](2026-06-18-dashboard-topbar-unification-design.md) · [dockable-workspace-context-memory-ssot](2026-06-19-dockable-workspace-context-memory-ssot-design.md) · [unified-task-message-envelope-registers-budget-ssot](2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## 1. Problem

The chat surface gives the user the wrong controls and hides the ones that matter.

1. **The bottom-left "modes" are theater.** Loquela's bar shows **Plan / Act / Verify** (`LQ_MODES`,
   `Loquela.tsx:20`). The `mode` field flows GUI→backend (`control_plane.rs:73`), is stored on
   `AgentTask.mode` (`types/tasks.rs:380`, *"Advisory: routing/verification policies may consult it"*),
   and is **never read**. The "Verify re-runs with stricter doubt + property tests" hint is aspirational:
   the mechanism exists (`OperatingMode::Verification`, `context_envelope.rs:47`) but only fires from the
   separate `doubt_task()` action, not from picking "Verify."

2. **There is no control over *how much* the system spends.** The backend has every knob —
   `QualityLevel` (Flash/Balanced/Premium → `CostPreference`), 3-axis routing
   (`SelectionAxes` cost/responsiveness/intelligence, `models/select.rs:289`), `PoolRule::Free`
   (`model_pool.rs`), and a budget gate (downgrade@80%/halt@95%, `budget_gate.rs:107`) — but **none is
   wired to a single user-facing control**, and `ExecutionModeProfile` (Efficient/Fast/Precision,
   `mode.rs:48`) is dead.

3. **Risk is a passive read-out.** `riskTone` is derived from mode+dryRun (`Loquela.tsx:446`) and shown
   as a pill. It is not configurable, and "acceptable risk" is not modeled as a spend/safety tradeoff.

4. **Model choice is invisible and unrecorded.** No field on `AgentTask` or `CompletionAttestation`
   records **which model actually completed a task**; `ChatMessageDto` has no `model_id`; no
   request/response payload is captured. The user cannot see what ran or what was sent/received.

5. **Stop is partially missing.** We can cancel *queued* (`control_plane.rs:291`) and *remote* tasks,
   pause/resume agents, doubt/overrule — but **there is no interrupt for an in-progress local task**, no
   mesh execution audit ("which node ran this"), no local-only / exclude-peer policy, no surfaced
   approval inbox, no subagent tree.

6. **The top bar wastes premium space.** `TopHud` (full/slim/hidden) collapses into a budget summary line
   (`TopHud.tsx:240`) — but budget is exactly what the user needs *in the chat tab*, while the top bar
   should be an optional, dockable **dashboard** with time-series. Its scrollbar isn't reliably themed.

## 2. Goal

A single, dense **Drive Console** on the chat composer carries only the five always-on elements the user
needs every session — **clutch · cost · risk · model · send/stop** — each wired to real backend behavior.
Detail and history move to a now-optional, dashboard-ified top bar and to dockable panels. Plan/Act/Verify
becomes a **real automated loop with intervention points**, not a pre-submit toggle. New backend capture
makes model choice and what-was-sent fully transparent via progressive disclosure.

**Non-goals:** rewriting the orchestrator scheduler; new model providers; changing the dockview engine
(we extend the existing `panelRegistry`/dockview from the dockable-workspace spec, not replace it).

## 3. The Drive Console (chat composer)

One continuous **brass-trimmed strip** (`Glass` + existing `Segment`/`Pill` primitives — no new shapes),
reading left→right like a cockpit. No orphaned, isolated controls; risk is a 3px semantic state-bar, never
a colored island. Single brass accent; the semantic ramp (emerald/amber/rose) appears only as thin state.

### 3.1 ① Clutch — "how much gas"

A 4-detent `Segment`: **Free · Efficiency · Balanced · Genius**. One selection sets a coherent bundle:

| Detent | `QualityLevel` | `SelectionAxes` | Pool | Budget gate | Free delegation |
|---|---|---|---|---|---|
| **Free** | Flash | COST_FIRST (70/15/15) | `PoolRule::Free` (hard) | aggressive downgrade | always |
| **Efficiency** | Flash | COST_FIRST | all enabled | downgrade@80/halt@95 | **yes — when task complexity is low AND a free model is available** |
| **Balanced** | Balanced | BALANCED (33/33/34) | all enabled | default | only when free quality suffices |
| **Genius** | Premium | QUALITY_FIRST (15/15/70) | all enabled | **budget gate relaxed** (warn, don't halt) | never |

- The clutch is the **single SSOT user control**; it resolves to a new `ClutchProfile` that maps to the
  existing knobs at the **scorer candidate boundary** (`models/select::decide`, after pool application).
  Efficiency's "delegate to free agents" decision is `complexity < threshold && free_candidate_available`.
- **Fast** is *not* a fifth detent — it is a lean expressed by the risk/latency interaction (§3.3): the
  console exposes Fast as a one-tap modifier chip only when it changes the outcome, otherwise hidden.

### 3.2 ② Cost — co-located with the clutch

Because the clutch *is* the spend control, the live consequence sits immediately beside it:
`$spent / $budget` (`mono`, brass) + a 54px burn meter + `↑ $x/min` during a run. Sourced from the
existing `UsageTracker`/`RemainingBudget` (`usage.rs`) and the budget SSOT (per the budget-SSOT spec).
The slim top-bar budget summary is **removed from the top bar and lives here**. Cost *over time* is a
dashboard widget (`budget_burn`, already registered), not in the bar.

### 3.3 ③ Risk — configurable dropdown (modeled as a spend/safety tradeoff)

A compact control showing a 3px state-bar + label (`High · Moderate · Low · Locked`); click opens a popover
to set **acceptable risk rate** and **safety-token budget**. Risk maps to real gates:

| Posture | Approval | Gates | Verification spend | Model lean |
|---|---|---|---|---|
| **High** ("break things", early dev) | AutoApprove more | shadow-only | minimal | clutch free to pick cheap/fast |
| **Moderate** (default) | Confirm | grounding enforce, socrates shadow | normal | neutral |
| **Low** (careful) | Review | completion-grounding + socrates **enforce** | extra "safety tokens" multiplier | **nudges model choice up even under Auto** |

Backed by `ApprovalTier`, `OperatingMode::Verification`, `completion_grounding_{shadow,enforce}`,
`socrates_gate_{shadow,enforce}` (`orchestrator_fields.rs`), `risk_scoring.rs`, plus a new
`safety_token_multiplier`. Risk **visibly interacts with the clutch**: Low risk overrides Efficiency's
cheap pick toward an intelligence-weighted candidate; the console reflects this in the model read-out.

### 3.4 ④ Model — always-on read-out + per-task attribution

- **In the bar:** `Auto · flash ⓘ` — under Auto, the *live selected* model name; ⓘ hover shows the
  `ModelSelectionDecision` reasoning (`score_breakdown`, alternatives, rejection reasons).
- **Per atomic task in the transcript:** each execution block is stamped with the **completing model**
  badge: `claude-opus · 4.2k↑ 1.1k↓ · $0.06 ⓘ`. Hover → progressive disclosure: what was sent, what came
  back, selection reasoning, latency. Collapsed by default (advanced info is opt-in).
- **New backend capture required** (none exists today): add `completing_model`, `provider`,
  `selection_reason`, `request_tokens`, `response_tokens`, `latency_ms`, and an optional
  `io_digest_ref` (pointer to captured request/response, gated by a privacy setting) to
  `CompletionAttestation` (`types/tasks.rs:292`); thread onto `ChatMessageDto` (`chat.rs:16`).

### 3.5 ⑤ Send / Stop — Enter on the button

The send button shows the **Enter glyph on it** (`Run ↵`), and the glyph is part of the click target
(not a separate hint). During a run the same button flips **in place** to `⏹ Stop ↵` (rose), same hotkey.
Requires a new `orch.interrupt_task` for in-progress local work (only queued/remote cancel exists today).

## 4. Plan / Act / Verify — real automated loop with intervention

Plan/Act/Verify is **removed from the composer bar** and reborn as an orchestrator-run pipeline surfaced as
live **phase state** inside the execution block (`Planning… → Acting… → Verifying…`).

- **Wire `mode`/automation for real:** drive the loop from `PlanModeTrigger` (`plan_mode_trigger.rs`) and
  auto-chain plan synthesis → `ai.plan.execute` → verification (`OperatingMode::Verification`) instead of
  requiring separate user RPCs.
- **Intervention points** at phase boundaries (this is the agentic control we lack): **approve/edit plan**,
  **skip verify**, **force verify**, **stop**. Applies per-agent and across the mesh.
- Default automation level follows the clutch (Genius plans+verifies more; Efficiency reacts) and risk
  (Low forces verify; High allows skip). The user can always override the auto-decision at the boundary.

## 5. Missing controls to build (the agentic-control layer)

Surfaced through a compact `agents N ▾` affordance in the console that expands into a docked **Mission
Control** panel (reusing the dockable-workspace `panelRegistry`):

1. **`orch.interrupt_task`** — interrupt in-progress local task (new); wired to Stop. *(REAL gap)*
2. **Model attribution capture** — §3.4 fields on `CompletionAttestation` + DTO. *(MISSING)*
3. **Mesh execution audit + policy** — record executor node on the task; expose "ran on node X";
   add per-task/per-class policy: **local-only**, **exclude peer**. *(MISSING)*
4. **Approval inbox ("Needs You")** — surface `ApprovalTier::Review` tasks for human approval instead of
   autonomic assignment; reuse the soft-HITL `FeedbackStore` (do not auto-route like the clarification
   inbox). *(PARTIAL → surface)*
5. **Subagent tree** — visualize `AgentDelegationBinding` parent→child lineage; per-agent pause/stop
   (commands exist: `pause/resume_orchestrator_agent`, `reorder_orchestrator_task`). *(PARTIAL → surface)*

## 6. Top bar → optional dashboard

- The top bar is **made optional/hideable**; its essential budget summary is **relocated into the Drive
  Console** (§3.2). The top bar becomes a **dashboard** of time-series widgets — cost over time, queue
  depth, agent/orchestrator behavior, mesh — using the already-implemented Recharts widgets and
  `widgetRegistry` SSOT (`dashboardLayout.ts`), fed by a new `vox.metric.series.v1` source (currently
  hardcoded).
- **Photoshop-style docking** for task list, context-window editor, memory, Mission Control, and dashboard
  widgets via the dockable-workspace `panelRegistry` over dockview — **dependency on that spec**, not
  re-specified here.
- **Scrollbar theming:** the `.custom-scrollbar` token (`index.css:27`, `rgba(255,255,255,.05)`) is correct
  but not applied in the top bar/dashboard scroll containers — apply it (or a `dark-scrollbar` utility)
  to every scroll surface; add a lint/check so new scroll containers can't ship a default white scrollbar.

## 7. Styling / design-critique resolution

- **One drive console, zero orphans** — the prior mockup's floating risk pill is folded into a continuous
  strip; risk demoted to a 3px state-bar. Single brass accent; semantic ramp only as thin state.
- **Reuse primitives** — `Glass`, `Segment`, `Pill`, `Popover`, existing tier-popover pattern
  (`Loquela.tsx:573`). No net-new component vocabulary.
- **Hierarchy** — composer textarea is the focal element; the console is secondary (smaller type, muted
  until hovered/active); send/stop is the only saturated affordance at rest.
- **Accessibility** — console controls are ≥24px hit targets, `aria-label` per control, risk popover is
  keyboard-navigable; model badge hover content is also focus-reachable (progressive disclosure must not be
  hover-only). Honor `tokens.contrast.generated.css` high-contrast variant.

## 8. Data flow & components

- **New FE:** `DriveConsole.tsx` (clutch+cost+risk+model strip), `RiskPopover.tsx`, `ModelBadge.tsx`
  (per-task, progressive disclosure), `MissionControlPanel.tsx`. Replaces the `LQ_MODES` segment +
  risk pill in `Loquela.tsx`.
- **New Rust:** `ClutchProfile` SSOT (maps detent → QualityLevel/SelectionAxes/Pool/budget); wire into
  `models/select::decide`; `orch.interrupt_task`; attribution fields on `CompletionAttestation`; mesh
  executor field + policy; `safety_token_multiplier`; auto plan→act→verify chaining.
- **Contracts:** extend the budget SSOT and add `contracts/gui/drive-console.v1.yaml` (detent→knob map,
  risk posture→gate map) so FE and BE share one source of truth and a parity gate can enforce it.

## 9. Error handling & edge cases

- Empty model pool after clutch=Free with no free providers → fall back to all-enabled (existing
  `model_pool.rs:86` behavior) and **surface a console warning** ("no free models available — using cheapest").
- Budget halt mid-run under Efficiency → console shows halt state, offers one-tap clutch bump to Genius
  (with explicit cost consent) or stop.
- `interrupt_task` on a task already completing → no-op with toast; never corrupt attestation.
- Missing attribution (older tasks) → badge shows `model unknown`, no hover.

## 10. Testing

- **Rust:** unit tests for `ClutchProfile` detent→knob mapping; risk posture→gate mapping; interrupt_task
  state transitions; attribution capture round-trips through `CompletionAttestation`; parity test that the
  `drive-console.v1.yaml` map matches code (like existing registry-parity gates).
- **FE (vitest):** DriveConsole renders all five elements; clutch selection emits correct profile; risk
  popover writes config; send↔stop flip; model badge progressive disclosure is keyboard-reachable.
- **Playwright:** console snapshot at rest and during-run; scrollbar-theme visual check on dashboard.

## 11. Antigravity execution-pipeline automation (the user's closing question)

"We have the beginnings of a design pipeline — what do we need to automate it?" Today the loop is
brainstorm → spec → plan → audit → hand a prompt to Antigravity/Flash manually. To automate execution
(tracked in the plan, not built here): (1) a **design-to-Antigravity dispatcher** that takes the written
plan + the `agy` delegation path (per the native-agy-delegation spec) and runs Flash against each
`[PARALLEL-SAFE]`/`[SEQUENTIAL]` task in a worktree jail; (2) **auto-append to the handoff ledger**
(AGH-series) with verification results; (3) a **DesignSync / gui-visual-review gate** so each built surface
is screenshotted and AI-reviewed against these design principles before the ledger marks it green. The plan
will specify wiring these existing pieces (agy shell-out, handoff ledger, gui-visual-review) into one
`vox design execute` command.

## 12. Dependencies & sequencing

- **Hard dep:** dockable-workspace `panelRegistry` (Mission Control + dashboard docking).
- **Hard dep:** budget SSOT (cost co-location + clutch budget aggressiveness).
- **Independent:** clutch/risk mapping, attribution capture, interrupt_task, console UI — can land first.
- Suggested order: backend SSOT (ClutchProfile + attribution + interrupt) → DriveConsole UI →
  Plan/Act/Verify loop + intervention → Mission Control + mesh policy → top-bar dashboard + scrollbar.
