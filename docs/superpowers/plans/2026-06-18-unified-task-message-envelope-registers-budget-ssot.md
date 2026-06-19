# Unified Task-Message Envelope, Registers & Budget SSOT — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use `- [ ]` checkboxes.

> **🤖 EXECUTION TARGET — READ FIRST.** Gemini 3.5 Flash inside Google Antigravity (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context). Plan engineered accordingly. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A crash between tasks leaves a compiling, tested tree.
2. **Verify-before-use.** `rg`/read before referencing any symbol; reality differs → STOP + report.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Two failures → STOP + handoff note.
5. **Parallel dispatch** per tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all` (`cargo fmt -p <crate>`); `.vox` automation; `docs/src/` frontmatter. `vox-gui` clippy `--lib` only.
7. **Verification ritual** before commit: Rust → `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`; TS → `npm test` + `npm run build` from `crates/vox-gui/ui`. Paste output.
8. **Rollback on broken tree:** `git reset --hard HEAD` to last green, re-attempt the one task.
9. **Rust:** no `.unwrap()` in lib code; inject params in tests; deterministic. DB test ctor is `vox_db::VoxDb::connect(vox_db::DbConfig::Memory)` (`local` feature) — there is NO `open_in_memory`.

**Goal:** Make a task a structured TaskMessage (multi-skill + typed context) authored by the existing Loquela composer and readable by orchestrator/A2A/mesh; unify all cost displays behind one budget SSOT (the existing `BudgetManager`, extended); and add an Office-default / gamified-opt-in register toggle over the widget registry.

**Architecture:** Extend `ChatPayload` + `IntakeItem` with `skills[]`/`context[]`; route dispatch on them; replicate over mesh. **Extend the EXISTING `BudgetManager`** (do not add a parallel aggregate) with a `snapshot()` + a `vox://cost-changed` emit, surfaced via `budget_get`, consumed by every cost reader. Make `widgetRegistry.render` register-aware.

**Tech Stack:** React/TS + vitest (`vox-gui/ui`); Rust (`vox-orchestrator`, `vox-gui`).

**Design:** [`../specs/2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md`](../specs/2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md). **Depends on** the cascade-spine + dashboard plans having landed (hopper wired; `widgetRegistry` exists).

---

## Flash Execution Addendum (2026-06-18 — second hardening pass)

These override task granularity where they conflict. Source: Flash-executability critique.

**Global gates:**
1. Each Step-1 `rg`/read is a **BLOCKING gate** — paste output before any code step; reality differs → STOP.
2. **Split-on-overrun:** one atomic green commit per sub-bullet when a step touches >1 file or >1 new function.
3. This plan **depends on the cascade-spine + dashboard plans having landed** (it extends `IntakeItem`, `hopper_submit`, `widgetRegistry`). Task 1 Step 1 must confirm those exist; if not, STOP.

**Mandatory splits + clarifications:**
- **Task 1:** the composer ALREADY sends `context: chips.map(c => ({kind, ref}))` at the build site — before coding, `rg -n "payload\.context|\.context" crates/vox-gui/ui/src -l` to find consumers of the old `{kind,ref}` shape; **refactor** that mapping to `ContextRef{kind,id,label}` (don't add a second `context`); update any consumer.
- **Task 3 → 3a / 3b.** 3a: extend `hopper_submit` to accept `skills`/`context` DTOs + test round-trip; commit. 3b: add `route_target(item, agents)` + tests + wire into `run_dispatcher`; commit. (3b note: agents do NOT advertise skills today — match only on file/agent `context` refs; skills-capability routing is deferred.)
- **Task 4:** Step 1 must `rg` the `canonical_signing_bytes` function and confirm `HopperOpSync::ItemAdmitted`'s current fields — adding `skills`/`context` changes the signed payload (**BREAKING, lockstep rollout**). Write the test body concretely against the post-change variant; don't leave it as prose.
- **Task 5 → 5a / 5b / 5c.** 5a: add `BudgetAggregate` + `BudgetManager::snapshot()` (+ `by_model` accumulation in the existing record path) + unit test; commit. 5b: `rg` where `CostIncurred` is already folded into the manager, add the `on_change` emit hook there (spawned post-`init_db`, not the sync ctor); commit. 5c: create `commands/budget.rs` (`budget_get` + `COST_CHANGED_EVENT`) + register in `main.rs`; commit.
- **Task 6:** before Step 2, run `rg -n "SessionBudgetDisplay|useMetricSeries|total_cost|total_24h_usd" crates/vox-gui/ui/src -n` and paste the **enumerated list** of reader sites; refactor exactly those to `useBudget()`.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-gui/ui/src/types/tauri.ts` | `ChatPayload` + `ContextRef` | Modify (Task 1) |
| `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` | emit `skills[]`/`context[]` from chips | Modify (Task 1) |
| `crates/vox-orchestrator/src/hopper/types.rs` | `IntakeItem.skills/context` + `ContextRef` | Modify (Task 2) |
| `crates/vox-orchestrator/src/hopper/{store,sqlite_store}.rs` | persist envelope fields | Modify (Task 2) |
| `crates/vox-gui/src/commands/orchestrator.rs` | `hopper_submit` accepts envelope | Modify (Task 3) |
| `crates/vox-orchestrator/src/orchestrator/dispatch.rs` | route on skills/context | Modify (Task 3) |
| `crates/vox-orchestrator/src/hopper/mesh_adapter.rs` | envelope on `ItemAdmitted` | Modify (Task 4) |
| `crates/vox-orchestrator/src/budget/mod.rs` | extend EXISTING `BudgetManager`: `snapshot()` + by_model + change hook | Modify (Task 5) |
| `crates/vox-gui/src/commands/budget.rs` | `budget_get` + `vox://cost-changed` | Create (Task 5) |
| `crates/vox-gui/ui/src/...` (budget readers) | read `budget_get` | Modify (Task 6) |
| `crates/vox-gui/ui/src/lib/widgetRegistry.ts` | register-aware `render` | Modify (Task 7) |

**Pre-flight (run once, paste output):**
- `rg -n "interface ChatPayload" -A 12 crates/vox-gui/ui/src/types/tauri.ts` — VERIFIED: has `active_skill?: string` (single) + `files?: string[]`; no `skills[]`/`context[]`.
- `rg -n "interface ChipData|onSubmit\(payload|ChatPayload" crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` — VERIFIED: `ChipData{id,kind:'file'|'skill'|'agent'|'branch'|'url'|'image',label,meta?}`, `onSubmit(payload: ChatPayload)`.
- `rg -n "pub struct IntakeItem" -A 25 crates/vox-orchestrator/src/hopper/types.rs` — confirm fields to extend.
- `rg -n "CostIncurred|CostTick|pub fn subscribe" crates/vox-orchestrator/src/events.rs` — VERIFIED: `CostIncurred{agent_id,provider,model,input_tokens,output_tokens,cost_usd,temporal_context}` (events.rs:247); `CostTick{...,total_24h_usd}` (events.rs:624).
- `rg -n "SessionBudgetDisplay|sessionBudget|budget_cap|total_cost" crates/vox-gui/ui/src crates/vox-gui/src -l` — find every current budget reader.
- `rg -n "widgetRegistry|render\(" crates/vox-gui/ui/src/lib/widgetRegistry.ts` — confirm the render signature from the dashboard plan.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task 1 `[SEQUENTIAL]`: Composer emits multi-skill + typed context

**Files:**
- Modify: `crates/vox-gui/ui/src/types/tauri.ts`, `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`
- Test: `Loquela.payload.test.tsx`

- [ ] **Step 1 (verify-before-use):** Confirm `ChatPayload` (tauri.ts:30) and the `onSubmit(payload)` build site (`Loquela.tsx:~420`). **VERIFIED:** the composer ALREADY sends a `context` field there mapping chips to `{ kind, ref }` (note: `ref`, not `id`/`label`) — so this task **refactors that existing mapping** to the typed `ContextRef { kind, id, label }`, it does NOT add a second `context`. Note where `files`/`active_skill` are derived from chips today.

- [ ] **Step 2: Write the failing test.** `Loquela.payload.test.tsx` — a pure helper `chipsToEnvelope(chips, activeSkill)` preserves multiple skills + typed context:

```tsx
import { describe, it, expect } from 'vitest';
import { chipsToEnvelope } from './Loquela';

describe('chipsToEnvelope', () => {
  it('keeps multiple skills and typed context', () => {
    const env = chipsToEnvelope([
      { id: 'brainstorming', kind: 'skill', label: 'brainstorming' },
      { id: 'writing-plans', kind: 'skill', label: 'writing-plans' },
      { id: 'src/a.rs', kind: 'file', label: 'a.rs' },
      { id: 'agent-7', kind: 'agent', label: 'compiler' },
    ], null);
    expect(env.skills).toEqual(['brainstorming', 'writing-plans']);
    expect(env.context.map(c => c.kind)).toEqual(['skill','skill','file','agent']);
    expect(env.files).toEqual(['src/a.rs']); // back-compat field still populated
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npm test -- Loquela.payload` → FAIL (`chipsToEnvelope` missing).

- [ ] **Step 4: Implement.** In `tauri.ts`, add `export interface ContextRef { kind: 'file'|'skill'|'agent'|'branch'|'url'|'image'; id: string; label: string }` and add `skills?: string[]; context?: ContextRef[];` to `ChatPayload`. Keep `active_skill`/`files` as **legacy fallback**: `files` is still populated (file-kind ids) for back-compat, but `context` is the authority (consumers prefer `context`, fall back to `files`). In `Loquela.tsx`, add `export function chipsToEnvelope(chips, activeSkill)` returning `{ skills, context, files }` (skills = kind-'skill' ids + `activeSkill` if not already present; context = all chips as `ContextRef` with `id`+`label`; files = kind-'file' ids). **Replace the existing `{ kind, ref }` context mapping at the build site** with `chipsToEnvelope(...)` — do not leave two context shapes.

- [ ] **Step 5: Run → PASS.** `npm test -- Loquela.payload` → PASS; `npm run build` clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/types/tauri.ts crates/vox-gui/ui/src/components/surfaces/Loquela/
git commit -m "feat(gui): composer emits multi-skill + typed context envelope"
```

---

## Task 2 `[SEQUENTIAL]`: `IntakeItem` carries the envelope (persisted)

**Files:**
- Modify: `crates/vox-orchestrator/src/hopper/types.rs`, `hopper/store.rs`, `hopper/sqlite_store.rs`

- [ ] **Step 1 (verify-before-use):** Read `IntakeItem` (`types.rs`) + the `submit()` constructor path in `store.rs`. Read the `hopper_inbox` columns added by the cascade-spine plan in `sqlite_store.rs`. Confirm `submit()`'s current arity.

- [ ] **Step 2: Write the failing test.** Add to `sqlite_store.rs` tests:

```rust
#[tokio::test]
async fn envelope_round_trips() {
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    let hopper = SqliteHopper::new(db.clone());
    let item = hopper.submit_envelope(
        "do it".into(), vec![], PriorityHint::Normal, IntakeSource::Developer, None,
        vec!["brainstorming".into(), "writing-plans".into()],
        vec![ContextRef { kind: "file".into(), id: "src/a.rs".into(), label: "a.rs".into() }],
    ).await;
    let reloaded = SqliteHopper::new(db).inbox().await;
    assert_eq!(reloaded[0].skills, item.skills);
    assert_eq!(reloaded[0].context.len(), 1);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator envelope_round_trips` → FAIL.

- [ ] **Step 4: Implement.** Add `pub struct ContextRef { pub kind: String, pub id: String, pub label: String }` (Serialize/Deserialize/Clone) to `types.rs`; add `pub skills: Vec<String>` and `pub context: Vec<ContextRef>` to `IntakeItem`. **DECISION (avoids breaking the struct literal + existing `submit` callers/tests):** add a NEW trait method `submit_envelope(intent, affinity, hint, source, session, skills, context)` and make the existing `submit(...)` delegate to it with empty `skills`/`context`. Update the **one** `IntakeItem { .. }` struct literal in `InMemoryHopper` (and any other literal site — grep `IntakeItem {`) to set the two new fields to `Vec::new()`. In `SqliteHopper`, store `skills`/`context` as JSON columns (`skills_json`, `context_json`) and rehydrate them in `inbox()`/`assigned()`/`history()`.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator envelope_round_trips` → PASS; full suite PASS.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/hopper/
git commit -m "feat(hopper): IntakeItem carries multi-skill + typed context envelope"
```

---

## Task 3 `[SEQUENTIAL]`: `hopper_submit` envelope + dispatch routes on it

**Files:**
- Modify: `crates/vox-gui/src/commands/orchestrator.rs`, `crates/vox-orchestrator/src/orchestrator/dispatch.rs`

- [ ] **Step 1 (verify-before-use):** Read the `hopper_submit` command added by the cascade-spine plan + `intake_to_task`/`run_dispatcher` in `dispatch.rs`. Confirm how a task is routed to an agent today (least-loaded vs affinity).

- [ ] **Step 2: Write the failing test.** Add to `dispatch.rs` tests: a pure `route_target(item, agents)` that, given an item whose `context` contains a `kind:"file"` ref an agent already owns, returns that agent (affinity by context), else falls back to least-loaded.

```rust
#[test]
fn routes_by_context_file_affinity() {
    let item = sample_item_with_context("src/auth.rs");           // helper builds IntakeItem
    let agents = vec![agent_owning("a1", "src/auth.rs"), agent_idle("a2")];
    assert_eq!(route_target(&item, &agents), "a1");
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator routes_by_context_file_affinity` → FAIL.

- [ ] **Step 4: Implement.** Add `route_target(item, agents)` that prefers an agent matching a `context` file/agent ref (or a `skills` capability if agents advertise skills), else least-loaded; wire it into `run_dispatcher`'s enqueue closure. Extend the `hopper_submit` Tauri command to accept `skills: Vec<String>` + `context: Vec<ContextRefDto>` and pass them to `submit_envelope`.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator routes_by_context_file_affinity` → PASS; `cargo check -p vox-gui`.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator vox-gui
git add crates/vox-orchestrator/src/orchestrator/dispatch.rs crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(orchestrator): route dispatch on task context/skills"
```

---

## Task 4 `[SEQUENTIAL]`: Replicate the envelope over the mesh

**Files:**
- Modify: `crates/vox-orchestrator/src/hopper/mesh_adapter.rs`, `hopper/store.rs` (`AdmittedReplay`)

- [ ] **Step 1 (verify-before-use):** Read `HopperOpSync::ItemAdmitted` + `AdmittedReplay` (store.rs) + `apply_op_fragment`. Confirm what fields the admitted op currently carries.

- [ ] **Step 2: Write the failing test.** Add to `mesh_adapter.rs` tests: build an `ItemAdmitted` op carrying `skills`/`context`, apply it on a peer hopper, assert the replayed item has them.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator <new test>` → FAIL.

- [ ] **Step 4: Implement.** Add `skills: Vec<String>` + `context: Vec<ContextRef>` to `HopperOpSync::ItemAdmitted` and `AdmittedReplay`; populate them on outbound emit; carry them through `apply_op_fragment` → `replay_admitted`. Keep the trust gate unchanged. **⚠️ BREAKING (note in commit + handoff):** these fields are part of `OpFragmentEnvelope::canonical_signing_bytes()`, so the signed payload changes — peers on the old layout cannot verify new envelopes and vice versa. This requires **lockstep peer rollout** (no mixed-version mesh). If mixed-version support is later needed, version the variant (`ItemAdmittedV2`) rather than mutating V1. Confirm the canonical-bytes function includes these fields before committing.

- [ ] **Step 5: Run → PASS.** New test PASS; full suite PASS.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/hopper/
git commit -m "feat(mesh): replicate task envelope (skills/context) on ItemAdmitted"
```

---

## Task 5 `[SEQUENTIAL]`: extend the EXISTING `BudgetManager` + `budget_get` + `vox://cost-changed`

> **VERIFIED — do NOT create a new aggregate.** `BudgetManager` already exists
> (`crates/vox-orchestrator/src/budget/mod.rs`, `total_cost_usd()` ~line 540), is built in
> `Orchestrator::new()`, and already feeds `OrchestratorStatus.total_cost`/`budget_cap`
> (`orchestrator/accessors.rs:17`). A second `BudgetLedger` would re-create the drift we're
> killing. **Extend the manager; surface it reactively by-model.**

**Files:**
- Modify: `crates/vox-orchestrator/src/budget/mod.rs` (add `snapshot()` + `by_model` tracking + change hook)
- Create: `crates/vox-gui/src/commands/budget.rs`

- [ ] **Step 1 (verify-before-use):** Read `crates/vox-orchestrator/src/budget/mod.rs` around `total_cost_usd()` (~540) and `orchestrator/accessors.rs:14–20` to see how `total_cost`/`budget_cap` are produced today and where per-call cost is recorded. Confirm `CostIncurred` fields (events.rs:247, VERIFIED). Decide where to add `by_model` accumulation inside the existing cost-recording path (NOT a parallel struct).

- [ ] **Step 2: Write the failing test.** Add to `budget/mod.rs` tests a `snapshot()` returning the existing total plus a new by-model map:

```rust
#[test]
fn snapshot_reports_total_and_by_model() {
    let mut mgr = BudgetManager::new(None);            // existing ctor
    mgr.record_cost("a1", "opus", 0.02);               // use the REAL cost-record method from Step 1
    mgr.record_cost("a1", "opus", 0.03);
    mgr.record_cost("a2", "haiku", 0.01);
    let s = mgr.snapshot();                             // NEW method
    assert!((s.spent_usd - 0.06).abs() < 1e-9);
    assert!((s.by_model["opus"] - 0.05).abs() < 1e-9);
}
```

(Replace `record_cost`/`new(None)` with the manager's REAL recording method + constructor confirmed in Step 1.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator snapshot_reports_total_and_by_model` → FAIL (`snapshot`/`by_model` missing).

- [ ] **Step 4: Implement.** Add `pub struct BudgetAggregate { pub spent_usd: f64, pub cap_usd: f64, pub by_model: std::collections::BTreeMap<String, f64> }` and `pub fn snapshot(&self) -> BudgetAggregate` to `BudgetManager`, accumulating `by_model` in the existing cost-record path (reuse `total_cost_usd()` for `spent_usd`, the existing cap for `cap_usd`). Have the daemon's cost-update path (where `CostIncurred` is already handled to update the manager) also fire an `on_change(snapshot)` hook. In `vox-gui`, add `pub const COST_CHANGED_EVENT: &str = "vox://cost-changed";`, emit it from that hook, and add `#[tauri::command] async fn budget_get() -> BudgetDto` returning the snapshot. Register `budget_get` in `main.rs`'s `generate_handler!`.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator snapshot_reports_total_and_by_model` → PASS; `cargo check -p vox-gui`.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-orchestrator -- -D warnings && cargo fmt -p vox-orchestrator vox-gui
git add crates/vox-orchestrator/src/budget/mod.rs crates/vox-gui/src/commands/budget.rs crates/vox-gui/src/
git commit -m "feat(budget): BudgetManager.snapshot() + budget_get + vox://cost-changed (one SSOT)"
```

---

## Task 6 `[PARALLEL-SAFE]` (frontend, disjoint from Task 7): every budget reader uses the SSOT

**Files:**
- Modify: each current budget reader found in Pre-flight (TopHud tile data, budget widget, `SessionBudgetDisplay` source, activity cost-fold if present)

- [ ] **Step 1 (verify-before-use):** Use the Pre-flight reader list. Confirm each currently derives spend independently (e.g. from `useMetricSeries` or `GuiOrchestratorStatus.total_cost`).

- [ ] **Step 2: Write the failing test.** `budget.source.test.tsx`: a shared hook/util `useBudget()` reads `budget_get` and updates on `vox://cost-changed`; given a mocked `budget_get` returning `{spent:0.06, cap:10}`, two separate components rendered with `useBudget()` show the **same** value.

- [ ] **Step 3: Run → FAIL.** `npm test -- budget.source` → FAIL.

- [ ] **Step 4: Implement.** Add `useBudget()` (invoke `budget_get`, subscribe `vox://cost-changed`). Point the TopHud budget tile, the budget widget, and `SessionBudgetDisplay` at `useBudget()` instead of their independent sources. Delete the now-redundant per-surface cost derivations.

- [ ] **Step 5: Run → PASS.** `npm test -- budget.source` → PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/
git commit -m "refactor(gui): all budget surfaces read the BudgetLedger SSOT (useBudget)"
```

---

## Task 7 `[PARALLEL-SAFE]` (frontend, disjoint from Task 6): register-aware widget render

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/widgetRegistry.ts` (+ a `useRegister` preference hook) (+ test)

- [ ] **Step 1 (verify-before-use):** Read `widgetRegistry.ts` render signature (from the dashboard plan: `render({ widget, navigate })`). Confirm the localStorage preference pattern (`SHELL_PREFERENCE_KEYS`).

- [ ] **Step 2: Write the failing test.** `widgetRegistry.register.test.tsx`: a registry entry with both an Office and a gamified variant returns the gamified component when `register==='gamified'` and the Office one when `register==='office'`; an entry with only Office returns Office for both (fallback).

- [ ] **Step 3: Run → FAIL.** `npm test -- widgetRegistry.register` → FAIL.

- [ ] **Step 4: Implement.** Extend `render` to `render({ widget, navigate, register })`; entries may declare `gamifiedRender?`; the dispatcher passes the current register (from a `useRegister()` hook backed by `SHELL_PREFERENCE_KEYS`, default `'office'`); `render` returns `gamifiedRender` only when `register==='gamified' && gamifiedRender` else the Office render. Add a top-bar register toggle (office ↔ gamified). **Default office.**

- [ ] **Step 5: Run → PASS.** `npm test -- widgetRegistry.register` → PASS; `npm test -- widgetRegistry` (completeness) still PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/lib/widgetRegistry.ts crates/vox-gui/ui/src/
git commit -m "feat(gui): register-aware widgets (Office default, gamified opt-in)"
```

---

## Parallel waves

- **Wave 1 (sequential backend chain — shared hopper/orchestrator files):** Task 1 (TS composer, independent) can run alongside; Tasks 2 → 3 → 4 → 5 are sequential Rust.
- **Wave 2 (parallel frontend):** Task 6 (budget readers) ∥ Task 7 (register render) — disjoint files. Task 1 is also frontend but touches Loquela/tauri.ts (disjoint from 6/7), so it may run in Wave 1 concurrently with the Rust chain.

## Self-review checklist
- [ ] Spec §9 covered: composer envelope (1), IntakeItem (2), submit+dispatch (3), mesh (4), BudgetLedger (5), readers (6), registers (7). ✔
- [ ] VERIFIED anchors: `ChatPayload` (active_skill single + files), `ChipData` kinds, `CostIncurred` fields (events.rs:247), skill id = String, DB ctor = `VoxDb::connect(DbConfig::Memory)`. ✔
- [ ] The EXISTING `BudgetManager` is the ONLY cost aggregate (extended, not duplicated); all readers go through `useBudget()`/`budget_get` (no per-surface totals). ✔
- [ ] Budget change-hook spawns in `init_db()`/post-init, not the sync `Orchestrator::new()` (manager exists at construction but the daemon emit path is wired post-init). ✔
- [ ] Composer's existing `{kind,ref}` context mapping is REFACTORED to `ContextRef{kind,id,label}` (not a second field); `submit_envelope` added (existing `submit` delegates) to avoid breaking the struct literal/tests. ✔
- [ ] Mesh `ItemAdmitted` field addition flagged as a canonical-signing-bytes BREAKING change (lockstep rollout). ✔
- [ ] Register default = office; gamified falls back to office when no variant (never blank); `SHELL_PREFERENCE_KEYS`+`useLocalStorage` verified to exist. ✔
- [ ] Symbol consistency: `chipsToEnvelope`/`ContextRef`/`skills`/`context`; `submit_envelope`; `route_target`; `BudgetManager.snapshot()`/`budget_get`/`COST_CHANGED_EVENT`/`useBudget`; `gamifiedRender`/`useRegister`. ✔
