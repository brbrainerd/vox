# Unified Task-Message Envelope, GUI Registers & Budget SSOT — Design Spec

**Date:** 2026-06-18
**Status:** Design (approved for planning)
**Author:** Brainstorming session (Claude, Opus 4.8)
**Integrates / amends:** [task-list-cascade-spine](2026-06-18-task-list-cascade-spine-design.md) · [activity-log-surface](2026-06-18-activity-log-surface-design.md) · [gamification-surfacing-and-minimap](2026-06-18-gamification-surfacing-and-minimap-design.md) · [dashboard-topbar-unification](2026-06-18-dashboard-topbar-unification-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## 1. Problem

Three gaps surfaced once we accept the reframe **"we are chatting with a task list"**:

1. **The composer captures rich structure that the payload throws away.** `Loquela`
   captures six chip kinds (`ChipData.kind: 'file'|'skill'|'agent'|'branch'|'url'|'image'`)
   and supports authoring against multiple skills — but its output type
   **`ChatPayload`** (`crates/vox-gui/ui/src/types/tauri.ts:30`) carries only
   `active_skill?: string` (**one** skill) and `files?: string[]`. Agents, branches,
   URLs, images, and *additional* skills are flattened to prose or dropped. So when a
   chat line becomes a task, the A2A system and orchestrator **cannot read** the
   `@`-references as structured data — they're lost at the payload boundary.

2. **Budget/cost has no single source of truth.** Cost is read by at least four
   surfaces — the TopHud `budget_burn` tile, the dashboard budget widget
   (`useMetricSeries`), the planned activity-log cost-fold, and the planned gamified
   treasury — but there is **no single aggregate**: `CostIncurred`
   (`events.rs:247`, per-call) and `CostTick` (`events.rs:624`, `total_24h_usd`) are
   separate event streams, and `SessionBudgetDisplay {spent, cap, source}`
   (`Loquela.tsx:102`) is populated independently. Four readers, no one truth → drift.

3. **One GUI must serve two very different registers.** A dense, legible
   "Office/standard" register (code view + harness + data tables) is the reliable floor;
   a richer **gamified** register (the `LudusSandbox` city) grows over time. Today these
   would be built as divergent UIs instead of two skins over one SSOT.

## 2. Goal

- **TaskMessage envelope:** a single structured object the composer produces, the hopper
  stores, and the orchestrator/mesh/A2A read — carrying **multiple skills** and **typed
  context refs**, not flattened text.
- **BudgetLedger SSOT:** one cost aggregate, one reactive topic (`vox://cost-changed`),
  consumed identically by every budget surface.
- **Registers:** a single `register` preference (`office` default, `gamified` opt-in)
  that swaps the widget `render` family over the *same* `widgetRegistry` + data — never
  changing truth, only fidelity.

Non-goals (YAGNI): merging the `conversations` and `hopper` persistence layers (the
chosen unification is "structured envelope + shared composer", not one message stream);
a third register; per-model budget enforcement/quotas (display only here).

## 3. The TaskMessage envelope

### 3.1 Frontend (`ChatPayload` extension — back-compatible)
Add to `ChatPayload` (`types/tauri.ts`), keeping existing fields:
```ts
skills?: string[];        // ALL @skill refs (resolved skill ids), supersedes active_skill
context?: ContextRef[];   // typed chips beyond files
// existing: description, session_id, priority, mode, model_hint, tier, dry_run, active_skill, files

interface ContextRef { kind: 'file'|'skill'|'agent'|'branch'|'url'|'image'; id: string; label: string }
```
The composer already holds `ChipData[]` — it maps 1:1 to `ContextRef[]`. `active_skill`
stays for back-compat but `skills[]` is the authority.

### 3.2 Backend (`IntakeItem` extension)
`IntakeItem` (`crates/vox-orchestrator/src/hopper/types.rs`) gains:
```rust
pub skills: Vec<String>,          // resolved skill ids (skill id is a String — vox-skills registry_api.rs:14)
pub context: Vec<ContextRef>,     // { kind: String, id: String, label: String }
```
`affinity_hints` stays (cheap routing hints); `skills`/`context` are the structured,
machine-readable payload the dispatcher and A2A consume. **The dispatcher routes on
`context`/`skills`** (e.g. file/agent affinity, skill availability), not just text.

### 3.3 Propagation
- **Mesh:** the envelope fields ride the existing `HopperOpSync::ItemAdmitted` (extend its
  payload), so peers receive the structured refs (builds on cascade-spine Task 6).
- **A2A:** because skills/context are typed columns, the bulletin/A2A layer can read
  "this task needs skill X / touches file Y" without parsing prose.

## 4. Budget SSOT — extend the EXISTING `BudgetManager` (do NOT add a parallel aggregate)

**Verified:** a cost aggregate already exists — `BudgetManager` (`crates/vox-orchestrator/src/budget/mod.rs`, `total_cost_usd()` ~line 540), instantiated in `Orchestrator::new()` and already surfaced into `OrchestratorStatus.total_cost`/`budget_cap` (`orchestrator/accessors.rs:17`) and broadcast via `vox://orch-status`. Creating a second `BudgetLedger` would re-introduce the very drift we're removing. So the SSOT is the existing manager, *extended* — not replaced.

```
 CostIncurred (per call) ──► BudgetManager (EXISTING aggregate; add snapshot()+by_model)
                                  │ emit vox://cost-changed on update
                                  ▼
              Tauri budget_get ── every budget reader re-reads (never self-computes)
   ┌────────────┬───────────────┬───────────────────┬──────────────────┐
 TopHud tile  budget widget  activity cost-fold  gamified treasury  composer SessionBudgetDisplay
```

- Extend `BudgetManager` with `snapshot() -> BudgetAggregate { spent_usd, cap_usd, by_model }` and emit `vox://cost-changed` from the daemon's cost-update path.
- **`budget_get` Tauri command** returns the snapshot DTO. `SessionBudgetDisplay` and every widget read it; none compute their own total. `OrchestratorStatus.total_cost` keeps working (same manager) — but the canonical, by-model, reactively-pushed surface is `budget_get`/`vox://cost-changed`.
- This is the canonical fix for the "displays disagree on money" failure mode.

## 5. Registers (Office default, gamified opt-in)

- A `register: 'office' | 'gamified'` preference (localStorage + a top-bar toggle).
- The `widgetRegistry` entry's `render` becomes **register-aware**: `render({widget, navigate, register})` returns the Office component by default and the gamified component when `register === 'gamified'` *and* a gamified variant exists (else falls back to Office). Same `kind`, same data, two skins.
- Office is always complete (the floor); gamified variants are added incrementally and never block.

## 6. SSOT sync — the whole picture

| Domain | SSOT | Reactive topic | Readers (must agree) |
|---|---|---|---|
| Tasks | `SqliteHopper` (+ TaskMessage envelope) | `vox://tasks-changed` | task list, chat thread, gamified quests, mini-map citizens |
| Activity | `activity_log` | `vox://activity-appended` | activity timeline, gamified city-life |
| **Budget** | **`BudgetLedger`** | **`vox://cost-changed`** | TopHud tile, budget widget, activity cost-fold, gamified treasury, composer `SessionBudgetDisplay` |

Rule everywhere: **state → topic → re-read.** No surface computes its own truth.

## 7. Error handling
- Unknown skill id in `skills[]` → kept as an unresolved ref (label shown, flagged), never dropped; dispatcher ignores unresolved for routing.
- `budget_get` before any cost → zeroed aggregate, not an error.
- Missing gamified variant for a `kind` → Office fallback (no blank).

## 8. Testing strategy
- **Unit:** `ChipData[] → ContextRef[]` mapping (multi-skill preserved); `IntakeItem` round-trips skills/context through `SqliteHopper`; `BudgetLedger` fold (N `CostIncurred` → correct `spent`/`by_model`); register-aware `render` returns gamified when set + variant exists, Office otherwise.
- **Integration:** submit a TaskMessage with 2 skills + a file + an agent chip → assert the hopper item and the dispatched route carry all of them; emit costs → all budget readers report the same number via `vox://cost-changed`.

## 9. Decomposition into plan tasks (preview)
1. Extend `ChatPayload` + composer emits `skills[]`/`context[]` (TS).
2. Extend `IntakeItem` with `skills`/`context` + `SqliteHopper` round-trip (Rust).
3. `hopper_submit` accepts the envelope; dispatcher routes on skills/context (Rust).
4. Mesh: carry envelope fields on `ItemAdmitted` (Rust).
5. `BudgetLedger` SSOT + `vox://cost-changed` + `budget_get` (Rust).
6. Point every budget reader at `budget_get`/`vox://cost-changed` (TS).
7. Register toggle (Office default/gamified opt-in) in `widgetRegistry.render` (TS).
