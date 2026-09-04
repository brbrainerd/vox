# GUI Product — Axis Journeys Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A first 10-minute Axis session feels like a coding-agent product: Ask/Plan/Agent on the composer, diffs you can apply, rollback that cannot fake-succeed, approvals that show the real patch, honest Compute/cloud copy, complete keybinds, thinking/checkpoints, and no second conflicting mode story.

**Architecture:** Track 1 owns Rust HITL, `vox_write_file`, `apply_worktree_hunks`, `rollbackLast`, and `message.rs` permission budgets. This track owns the **React/IPC tree**: Loquela composer, `CHAT_TURN_KEYS`, Approvals/NeedsYou DTO, honesty labels, keybind handlers, CheckpointDrawer, transcript tool cards. Do not invent `SessionMode`. Do not rebuild shipped surfaces (meter, IntentPanel, keybind capture, PlanPanel, `vox_browser_*`).

**Tech Stack:** React 19, vitest, Tauri 2 invoke, existing MCP `invoke_mcp_tool`.

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md) §9 + [`2026-08-31-platform-parity-id-coverage.md`](../specs/2026-08-31-platform-parity-id-coverage.md) (original canvas `gap`/`fix` v1). G03 UI, G04–G18 remaining original fixes, G19–G27, H01 rail, H10 H12 H16-UI H17, M02 `$ remaining`, R11 GUI string.

**Depends on:** Gate A (Track 1 Tasks 0–10, 12–14; rollback helper; `vox_exit_plan_mode`). Composer UI may start after `rollbackLast` + Apply exist.

**Closes:** G03 G04 (LAN daemon queue) G05 G06 (warn/compact/dropped) G07 (harness sentence) G08 G09 G10 (Approve → exit plan) G11 G12 (six actions) G13–G18, G19–G27, H01 rail UI, H10 H12 H16-UI H17, M02 remaining $, R11 GUI cannot-replay. **No skip/optional IDs.**

## Audit corrections (must not violate)

- MCP list tool is **`vox_oplog`**; list field **`id`**. Undo arg is `operation_id`.
- `chat_turn` `ChatTurnInput` has **no** `permission_mode` today. Adding it requires `CHAT_TURN_KEYS` + `buildChatTurn.ts` + Rust serde defaults **same commit**.
- Approvals already expose `ask|plan|accept_edits|accept_all` via `setPermissionMode`. Composer must write the **same** transport field — not a fourth enum.
- DriveConsole clutch/risk stays; G19 is **copy + placement**, not deleting clutch.
- Resolve callers use `outcome: 'approved'|'rejected'`. Migrate **all** (`ApprovalsView`, `useAttentionInbox`, `NeedsYouSurface`) together (G21).
- `App.test.tsx` currently locks `/rollback` `args: {}` — Track 1 should have rewritten it; if not, this track must.
- Honesty-triage HIDE rows for meter/StreamCard/keybinds display are **stale** — KEEP those surfaces.
- New Tauri commands: `gui-surface-coverage --write` same commit. Sidecar: `vox run scripts/gui-build.vox` before `cargo test -p vox-gui`.
- Prefer small contexts over growing `SurfaceProps` (~40 fields).

## File map

| File | Role |
|---|---|
| Create: `crates/vox-gui/ui/src/components/surfaces/Loquela/sessionMode.ts` | Ask/Plan/Agent → `ask\|plan\|accept_edits` |
| Modify: `Loquela.tsx`, `DriveConsole.tsx` | Composer control; clutch copy |
| Modify: `lib/buildChatTurn.ts`, `chat_turn.rs` | Optional `permission_mode` + `CHAT_TURN_KEYS` |
| Modify: `ApprovalsView.tsx`, `useAttentionInbox.ts`, `NeedsYou/` | DTO + resolve schema + batch + poll SSOT |
| Modify: `DiffReview.tsx` | Buttons (Track 1 may have landed; verify) |
| Create: `CheckpointDrawer.tsx` | H10; uses `rollbackLast(id)` |
| Modify: `lib/keybinds.ts`, `App.tsx` `actionHandlers` | send, plan, accept hunk, reject, compact, new session |
| Modify: `SettingsView.tsx` | G04 daemon checkbox, G05 rules, G07 harness sentence |
| Modify: `PlanPanel` | G10 Approve → `vox_exit_plan_mode` |
| Modify: `IntentPanel` / `buildChatTurn.ts` | G08 persist fields; G09 Plan default-on |
| Modify: `ChatExecutionRail.tsx` | G06 70% warn; H01 iterations_left; M02 $ remaining |
| Modify: `decoratorRegistry.ts` | G14 honesty + G26 titles |
| Modify: `ChatTranscript.tsx` | H12, G16, G20, G27 |
| Modify: `navigation.ts` | G25 Matrix |
| Modify: `decoratorRegistry.ts` | G14 honesty + G26 titles |
| Modify: `ChatTranscript.tsx` | H12, G16, G20, G27 |
| Modify: `navigation.ts` | G25 Matrix |

---

### Task 1: Mode IA copy (G19)

**Files:** `DriveConsole.tsx`, `ApprovalsView.tsx`, `SettingsView.tsx` (one sentence), `Loquela.tsx` leftover Plan/Act comments.

- [ ] **Step 1:** vitest or string test: DriveConsole caption contains “clutch / risk, not Ask-Plan-Agent”; Approvals Segment labelled “permission mode”.
- [ ] **Step 2:** FAIL if both look like session modes.
- [ ] **Step 3:** Copy only. Do not remove clutch.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `fix: distinguish DriveConsole clutch from permission mode`

---

### Task 2: Composer Ask/Plan/Agent (G03 UI)

**Files:** `sessionMode.ts`, `Loquela.tsx`. Persist UI choice in existing Loquela state.

```ts
export function permissionModeForComposer(ui: 'ask' | 'plan' | 'agent'): 'ask' | 'plan' | 'accept_edits' {
  return ui === 'agent' ? 'accept_edits' : ui;
}
```

- [ ] **Step 1:** vitest the mapper (exact test from Track 1 v1 Task 11).
- [ ] **Step 2:** FAIL (file missing).
- [ ] **Step 3:** Three-way control next to send. Call `setPermissionMode(permissionModeForComposer(ui))` on change (same helper Approvals uses in `transport.ts`). Do **not** invent `SessionMode`. Agent is `accept_edits`, not `accept_all`.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: Ask/Plan/Agent composer writes existing permission_mode`

---

### Task 3: Optional `permission_mode` on `chat_turn` (G03 plumbing)

**Files:** `ChatTurnInput` in `chat_turn.rs`, `CHAT_TURN_KEYS`, `buildChatTurn.ts`. `#[serde(default)]`.

- [ ] **Step 1:** test that `buildChatTurn` includes `permission_mode` when set and omits/defaults when unset; Rust serde default does not break old payloads.
- [ ] **Step 2:** FAIL (`CHAT_TURN_KEYS` assertion).
- [ ] **Step 3:** Add the key everywhere. Dual-write with Task 2 transport mode — values must match.
- [ ] **Step 4:** PASS vitest + `cargo test -p vox-gui` chat_turn serde if present.
- [ ] **Step 5:** commit `feat: chat_turn accepts optional permission_mode`

---

### Task 4: Verify Apply + rollback in the React tree

**Files:** `DiffReview.tsx`, `App.tsx`, `rollbackLast.ts`, `App.test.tsx`.

Track 1 should have landed buttons + helper. This task is a **gate**, not a rebuild.

- [ ] **Step 1:** vitest: DiffReview has Accept/Reject; `rollbackLast` empty oplog does not call `vox_undo`; `App.test.tsx` does **not** expect `args: {}`.
- [ ] **Step 2:** If FAIL, implement only the missing piece (do not duplicate Track 1 Task 9–10).
- [ ] **Step 3–5:** commit only if you had to fill a hole `fix: Axis DiffReview/rollback matches Track 1 contract`

---

### Task 5: Approval DTO + resolve schema tree (G21, H02 UI)

**Files:** `ApprovalsView.tsx` `PendingApproval` type, `parsePendingApprovals` in `mcpToolResult.ts`, `useAttentionInbox.ts`, `NeedsYouSurface.tsx`.

Widen type: `args`, `unified_diff`, `risk_class`. Resolve: accept `outcome` **and** `decision`; `modify` requires `args`.

- [ ] **Step 1:** vitest `parsePendingApprovals` keeps `unified_diff`. Component test: diff `<pre>` visible. Grep `vox_resolve_approval` under `ui/src` — list every caller in the commit message.
- [ ] **Step 2:** FAIL (fields dropped).
- [ ] **Step 3:** Update **all** callers in one commit (split-brain is the bug).
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: approvals show args/diff; resolve schema shared across NeedsYou`

---

### Task 6: Batch select (H16 UI)

**Files:** `ApprovalsView.tsx` only. Sequential `vox_resolve_approval`. Helper `idsToApprove` (Track 1 Task 12 may exist — reuse).

- [ ] **Step 1–5:** checkboxes + “Approve selected”. Commit `feat: batch approve selected HITL rows`

---

### Task 7: Approvals poll SSOT (G22)

**Files:** `ApprovalsView.tsx` own poll vs `useAttentionInbox`.

- [ ] **Step 1:** test or comment+code: Approvals reads `attention` props (or shared hook) — no second interval that can diverge.
- [ ] **Step 2–5:** commit `fix: single pending-approvals poller for Axis`

---

### Task 8: Chat → review journey (G27)

**Files:** `ChatExecutionRail.tsx` / `AttentionStrip` / transcript tool card. Click opens Approvals or DiffReview with `approval_id`.

- [ ] **Step 1:** vitest click handler sets view + id.
- [ ] **Step 2–5:** commit `feat: jump from chat rail to the parked approval`

---

### Task 9: CheckpointDrawer (H10)

**Files:** create `CheckpointDrawer.tsx`; Settings `checkpointMins` link; MCP **`vox_oplog`** `{ limit }`; restore via `rollbackLast` with chosen **`id`**.

- [ ] **Step 1:** vitest drawer renders two fake ops; restore invokes `vox_undo` with `operation_id`.
- [ ] **Step 2–5:** commit `feat: CheckpointDrawer lists oplog and restores by id`

---

### Task 10: Collapsible thinking (H12)

**Files:** `ChatTranscript.tsx`. Field: existing reasoning/thinking on the message if present; else skip UI (honesty: no fake tokens).

- [ ] **Step 1:** vitest: when `reasoning` set, a disclosure renders; when absent, no disclosure.
- [ ] **Step 2–5:** commit `feat: collapsible model reasoning in chat transcript`

---

### Task 11: Attention badge formula (H17)

**Files:** `useAttentionInbox.ts`, `AppShell`/`Sidebar` `needsYouCount`.

`attentionCount = approvals + needsYou + blockedTasksCount` (`blockedTasksCount` is computed today and unused).

- [ ] **Step 1:** unit test `attentionCount(1,2,3) === 6`.
- [ ] **Step 2–5:** commit `fix: Review badge includes blocked hopper tasks`

---

### Task 12: Six keybind actions (G12 original)

Original fix: bindable send, plan, accept hunk, reject, compact, new session. Capture **already exists** — this task adds **ids + handlers**.

**Files:** `lib/keybinds.ts` `ACTION_REGISTRY` / `DEFAULT_BINDINGS`; `App.tsx` `actionHandlers`. Also `dispatch-intent` (G23).

- [ ] **Step 1:** test `ACTION_REGISTRY` includes exactly these ids: `send`, `plan`, `accept-hunk`, `reject-hunk`, `compact`, `new-session`. Handler object has each. Defaults documented in Settings or `keybinds.ts` comment.
- [ ] **Step 2:** FAIL (today only partial).
- [ ] **Step 3:** handlers: `send` focuses/submits Loquela; `plan` sets composer to plan; `accept-hunk`/`reject-hunk` call DiffReview actions if a hunk is focused else no-op; `compact` calls Task 23 compact; `new-session` existing new-chat path (`rg newSession` / `new_session`).
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: bind send/plan/hunk/compact/new-session keybinds`

---

### Task 13: Compute honesty + decorator titles (G14 G26)

**Files:** `decoratorRegistry.ts` `commandSurface` for `mens`/`populi`/`oratio`.

- [ ] **Step 1:** vitest: curated_decorator cards include “CLI (not a live dashboard)”; titles match `NAV_LABELS` (Training / Nodes / Voice) not “Vox Mens”.
- [ ] **Step 2–5:** commit `fix: Compute cards honest CLI shells; titles match nav`

---

### Task 14: Chat footer model explain + cache + axes/$ (G16 + H11 display)

**Files:** transcript/footer. APIs already: `explain_model_selection`, `selection_reason` hydrate. H11 `cache_hit` from Track 3.

- [ ] **Step 1:** vitest: after a turn, footer shows **slug**, **axes** (cost/intelligence/responsiveness or equivalent fields on the payload), **$** if present, **fallback** reason, `cache_hit` bool **or** `cache n/a`. Button invokes `explain_model_selection`.
- [ ] **Step 2–5:** commit `feat: chat footer shows slug, axes, cost, cache honesty, explain`

---

### Task 15: LAN daemon background turn (G04 original)

Original fix: queue session on worker. Chosen: **local daemon**, not rented VM.

**Files:** Settings checkbox; Tauri `queue_background_turn`; existing `vox` daemon / MCP session poll; NeedsYou.

- [ ] **Step 1:** test `queue_background_turn` exists (`gui-surface-coverage` after register). Checkbox “Run in background” → invoke posts the current turn to the daemon; NeedsYou polls until parked/complete. Unset daemon → error toast “daemon not running”, **not** silent drop.
- [ ] **Step 2:** FAIL (honesty copy only today is insufficient).
- [ ] **Step 3:** implement queue + poll. Register command; `gui-surface-coverage --write`. Do **not** add `VOX_CLOUD_WORKER_URL` as the product path (rented VM residual).
- [ ] **Step 4:** PASS vitest mock invoke + `cargo test -p vox-gui` if a rust test exists.
- [ ] **Step 5:** commit `feat: queue chat turn on the local vox daemon`

### Task 15b: Axis is harness-not-IDE (G07 chosen arm)

- [ ] **Step 1:** Settings sentence: “Inline ghost-text lives in the VS Code/Cursor extension; Axis is the harness.” No Monaco. Test: string present; no `monaco` import in `vox-gui/ui`.
- [ ] **Step 2–5:** commit `docs: Axis harness-not-IDE Settings copy`

### Task 15c: Harness surface honesty (G24)

- [ ] **Step 1:** harness surface does not claim a live dashboard. Test: copy.
- [ ] **Step 2–5:** commit `fix: harness surface copy does not claim a live dashboard`

---

### Task 16: Rules editor (G05)

**Files:** Settings CRUD `.vox/rules/*.md` via existing workspace file APIs.

- [ ] **Step 1:** vitest save invoke path under `.vox/rules/`.
- [ ] **Step 2–5:** commit `feat: Settings editor for .vox/rules`

---

### Task 17: Table artifact (G11)

**Files:** `ChatTranscript` artifact renderer. `kind: 'table'`. No Cursor Canvas.

- [ ] **Step 1:** artifact with 2 rows → 2 `role=row`.
- [ ] **Step 2–5:** commit `feat: chat table artifact renderer`

---

### Task 18: Subagent `window_id` (G13)

**Files:** `SubAgentsView.tsx` (documents gap today); spawn/event JSON in orchestrator (`list_subagent_tree`).

- [ ] **Step 1:** test event JSON has `window_id`.
- [ ] **Step 2–5:** commit `feat: stamp window_id on subagent events`

---

### Task 19: Browser tools gated with live names (G15)

**Files:** `tool_selection.rs` or GUI browser lane. Tools: `vox_browser_screenshot` / `page_info` / `text` / `click`. **Never** `vox_browser_snapshot`. Gated like writes (Task 1 HITL).

- [ ] **Step 1:** allowlist contains live names; `!contains("vox_browser_snapshot")`. Click/type are **gated** (`is_gated_tool`).
- [ ] **Step 2–5:** commit `feat: browser MCP tools allowlisted and gated under real names`

---

### Task 20: Vision gate (G17)

**Files:** Loquela image chip → `chat_turn` / model pick.

- [ ] **Step 1:** image + text-only model → structured `vision-model-required` (or pick vision slug). Test both branches if a vision slug exists in fixture.
- [ ] **Step 2–5:** commit `feat: vision-model-required when image attached to text-only model`

---

### Task 21: PlanPanel Approve = ExitPlanMode (G10)

PlanPanel already lists session plan-nodes. Do **not** rebuild. Original remaining: Approve **promotes** Plan.

- [ ] **Step 1:** vitest: Approve on a node invokes MCP `vox_exit_plan_mode` (Track 1 Task 13). Empty state: “Session plan nodes (not CLI `vox plan`).”
- [ ] **Step 2:** FAIL if Approve is a no-op chip.
- [ ] **Step 3:** wire invoke. CLI JSON import is residual.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: PlanPanel Approve calls vox_exit_plan_mode`

---

### Task 22: Persist intent on chat_turn (G08)

**Files:** `intentSpec.ts`; `ChatTurnInput` `#[serde(default)]`; `CHAT_TURN_KEYS`; `buildChatTurn.ts`. Keep `composeDescription`.

- [ ] **Step 1:** serde roundtrip `goal`, `constraints`, `budget_usd`, `acceptance` (or the IntentPanel field names that exist). Persist `intent_id` on session — reuse plan-node id **or** existing DB row; **no new crate**. PlanPanel `insert_plan_node` includes goal.
- [ ] **Step 2:** FAIL (`CHAT_TURN_KEYS` missing fields).
- [ ] **Step 3:** add keys everywhere same commit. Do not skip as description-only.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: persist intent fields on chat_turn`

### Task 22b: IntentPanel default-on in Plan (G09)

- [ ] **Step 1:** vitest: composer=`plan` → IntentPanel **visible** by default.
- [ ] **Step 2–5:** commit `feat: IntentPanel open by default in Plan mode`

---

### Task 23: Context 70% warn + Compact + dropped string (G06 remaining)

Meter already `usedTokens={budget.used_tokens}`. Original remaining: warn 70%, compact button, show dropped.

- [ ] **Step 1:** vitest: `used/limit >= 0.7` applies warn CSS class. Compact button calls existing `compact_auto` / session compact (`rg compact_auto`). After compact, string `dropped N tokens` if the engine returns a count, else `compacted`.
- [ ] **Step 2:** FAIL (no warn class / no button).
- [ ] **Step 3:** implement. Do not rebuild ContextWindowMeter.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: context 70% warn, Compact, dropped-token string`

---

### Task 24: Chat tool cards (G20)

**Files:** `ChatTranscript.tsx`. Minimal: one card per tool call (name + status). Stop/retry YAGNI unless invoke already exists.

- [ ] **Step 1:** fixture timeline with one tool → one card.
- [ ] **Step 2–5:** commit `feat: tool-call cards in chat transcript`

---

### Task 25: Matrix orphan + Bugbot CLI (G25 G18)

**G25:** `LEGACY_VIEW_ALIASES.matrix → chat`. If Matrix is still a registered surface, HIDE from nav / `CHILD_ORDER`.

**G18:** `vox review pr --readonly` clap; dry-run does not post comments. Test clap exists **even if** `gh` is unavailable (do not skip the clap test).

- [ ] **Step 1–5:** two commits if both real: `fix: hide Matrix orphan surface` / `feat: vox review pr --readonly plan-mode analog`

---

### Task 26: Vitest journey smoke

**Files:** `crates/vox-gui/ui/src/journeys/parity.journey.test.ts` (or next to Loquela).

- [ ] **Step 1:** composer mode → `permission_mode` string; DiffReview buttons present; `parsePendingApprovals` keeps diff; keybind `send` in registry.
- [ ] **Step 2–5:** commit `test: Axis parity journey smoke`

---

### Task 27: GUI “cannot replay” when determinism lint fires (R11)

**Files:** PlanPanel or workflow card. Track 5 Task 5d is the compiler fixture.

- [ ] **Step 1:** vitest: when chat/plan payload includes diagnostic `determinism` / `time.now` in workflow, card text contains `cannot replay`.
- [ ] **Step 2–5:** commit `feat: workflow card shows cannot replay when determinism lint fires`

### Task 28: Execution rail remaining iterations (H01)

**Files:** `ChatExecutionRail.tsx`. Field `iterations_left` from Track 1 Task 14.

- [ ] **Step 1:** vitest: payload `iterations_left: 7`, `max_iterations: 32` → rail shows `7/32` (or `8/32` remaining convention — pick one and test it).
- [ ] **Step 2–5:** commit `feat: chat rail shows remaining agent iterations`

### Task 29: Rail remaining $ (M02 remaining)

Guard already shipped. Original remaining: GUI remaining $.

- [ ] **Step 1:** vitest: budget API remaining USD renders `$` on the rail. If API has no remaining field, add it on the existing budget DTO (`#[serde(default)]`) in the same commit.
- [ ] **Step 2–5:** commit `feat: chat rail shows remaining daily budget USD`

---

## Track 6 gate

HARD: `pnpm -C crates/vox-gui/ui exec vitest run src/components/surfaces/Loquela/sessionMode.ts src/lib/rollbackLast.test.ts src/lib/parseUnifiedDiff.test.ts`

HARD: grep `vox_resolve_approval` under `ui/src` — every caller shares the same args shape.

HARD: `cargo run -p vox-cli -- ci gui-surface-coverage` clean if a new Tauri command landed.

HARD: keybind registry includes `send`, `plan`, `accept-hunk`, `reject-hunk`, `compact`, `new-session`

HARD: no `vox_browser_snapshot`, no `vox_oplog_list`, no `SessionMode` enum.

SOFT: 10-minute manual: Ask/Plan/Agent on composer, park a write, see diff, reject, `/rollback` fail-closed on empty oplog.
