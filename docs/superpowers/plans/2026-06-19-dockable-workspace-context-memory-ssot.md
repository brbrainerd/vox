# Dockable Workspace + Context-Window Editor + Editable Memory (SSOT) — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use `- [ ]` checkboxes.

> **🤖 EXECUTION TARGET — READ FIRST.** Gemini 3.5 Flash inside Google Antigravity (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** A crash between tasks leaves a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use.** Every Step-1 `rg`/read is a BLOCKING gate — paste output before any code step; reality differs → STOP and report.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Two failures → STOP + handoff note.
5. **Parallel dispatch** per tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all` (`cargo fmt -p <crate>`); `.vox` automation; `docs/src/` frontmatter. `vox-gui` clippy `--lib`.
7. **Verification ritual** before commit: Rust → `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `vox stub-check` → `cargo fmt -p <crate>`; TS → from `crates/vox-gui/ui`: `npm test` + `npm run build`. Paste output.
8. **Rollback on broken tree:** `git reset --hard HEAD` to last green, re-attempt the one task.
9. **Split-on-overrun:** if an Implement step would touch >1 file or add >1 new component/function, commit each sub-bullet as its own atomic green commit, in order.

**Goal:** One panel-registry + dock-layout SSOT over the already-present `dockview`, hosting every surface and dashboard/gamification widget as a dockable/resizable/draggable/bindable panel; plus a structured editable **context-window editor** (committed edit = what's sent to the API) and **editable memory** (persistent + session) feeding the context SSOT.

**Architecture:** Generalize `DockShell.tsx` → `DockWorkspace`; a `panelRegistry` (absorbing `widgetRegistry`) drives it; the persisted dockview layout is the layout SSOT. The orchestrator session context-item list is the context SSOT, read/written by `context_get`/`context_set` and edited in the context panel (reusing `lib/loquelaContext.ts` `AttachItem`). Memory edits flow through `memory_get`/`memory_set`.

**Tech Stack:** React/TS + vitest + `dockview` 6.6.1 (`vox-gui/ui`); Rust (`vox-orchestrator`, `vox-gui`).

**Design:** [`../specs/2026-06-19-dockable-workspace-context-memory-ssot-design.md`](../specs/2026-06-19-dockable-workspace-context-memory-ssot-design.md). **Depends on** the dashboard plan (`widgetRegistry`) and spec-5 (`useBudget`/registers) having landed.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-gui/ui/src/lib/panelRegistry.ts` | panel SSOT (absorbs widgetRegistry) | Create (Task 1) |
| `crates/vox-gui/ui/src/components/layout/DockWorkspace.tsx` | dockview workspace + layout SSOT + presets | Create (Task 2; generalizes `DockShell.tsx`) |
| `.../surfaces/Dashboard/Dashboard.tsx` + `lib/dashboardLayout.ts` | dashboard → dockview group; retire dnd-kit grid | Modify (Task 4) |
| `.../surfaces/Context/ContextEditor.tsx` | structured context editor | Create (Task 6) |
| `crates/vox-gui/src/commands/context.rs` | `context_get`/`context_set` + `vox://context-changed` | Create (Task 5) |
| `crates/vox-orchestrator/src/session/state.rs` | expose ordered context-item list (pin-aware) | Modify (Task 5) |
| `crates/vox-gui/src/commands/memory.rs` + `.../surfaces/Memory/MemoryView.tsx` | editable memory feeding context | Create/Modify (Task 7) |

**Pre-flight (run once, paste output):**
- `rg -n "DockviewReact|api\.fromJSON|api\.toJSON|serialize|LAYOUT_PERSIST" crates/vox-gui/ui/src/components/layout/DockShell.tsx` — confirm dockview layout serialize/restore API in use.
- `rg -n "widgetRegistry|DashboardWidgetKind|DASHBOARD_WIDGET_KINDS" crates/vox-gui/ui/src/lib/` — confirm the registry to absorb (from the dashboard plan).
- `rg -n "AttachItem|attachItemsFromHits|export" crates/vox-gui/ui/src/lib/loquelaContext.ts` — confirm the context-item model to reuse.
- `rg -n "CompactionConfig|struct SessionState|context|pinned|history" crates/vox-orchestrator/src/session/state.rs crates/vox-orchestrator/src/compaction.rs` — find the session context list + compaction; confirm where the API-serialized context is assembled.
- `rg -n "generate_handler!" crates/vox-gui/src/main.rs` — Tauri command registration site.
- `cd crates/vox-gui/ui && npm test -- --run` ; `cargo run -p vox-arch-check` — baselines green.

---

## Task 1 `[SEQUENTIAL]`: `panelRegistry` SSOT (absorb widgetRegistry) + completeness test

**Files:** Create `crates/vox-gui/ui/src/lib/panelRegistry.ts` (+ `.test.ts`); Modify `lib/dashboardLayout.ts` (export the panel-kind union).

- [ ] **Step 1 (verify-before-use):** Paste `rg -n "widgetRegistry|DASHBOARD_WIDGET_KINDS" crates/vox-gui/ui/src/lib/` and read `widgetRegistry.ts`'s entry shape. Confirm the render signature `render({widget, navigate})` and `topHudEligible`.

- [ ] **Step 2: Write the failing test.** `panelRegistry.test.ts`: every kind in a `PANEL_KINDS` const has a registry entry with a `title` string, a `render` function, and a `defaultDock` (`'left'|'right'|'top'|'bottom'|'center'`). Iterate the const (order-independent).

- [ ] **Step 3: Run → FAIL.** `npm test -- panelRegistry` → FAIL.

- [ ] **Step 4: Implement.** Create `panelRegistry.ts`: `export const PANEL_KINDS = [...] as const; export type PanelKind = typeof PANEL_KINDS[number];` covering the catalog (chat, task_list, context, memory, activity, qa, agents, budget, gamify_minimap) PLUS the dashboard widget kinds (re-export/spread from `widgetRegistry`). Each entry: `{ title, description?, render({panel, navigate, register?}), defaultDock, allowMultiple?, topHudEligible? }`. For surfaces not yet panel-wrapped, use a placeholder `render: () => <EmptyState/>` (replaced in Task 3). Keep `widgetRegistry` as the source for widget kinds (panelRegistry imports it) — do NOT duplicate.

- [ ] **Step 5: Run → PASS.** `npm test -- panelRegistry` → PASS; `npm run build` clean.

- [ ] **Step 6: Commit.** `git add crates/vox-gui/ui/src/lib/panelRegistry.ts crates/vox-gui/ui/src/lib/panelRegistry.test.ts crates/vox-gui/ui/src/lib/dashboardLayout.ts && git commit -m "feat(gui): panelRegistry SSOT (absorbs widgetRegistry)"`

---

## Task 2 `[SEQUENTIAL]`: `DockWorkspace` over dockview (layout SSOT)

**Files:** Create `crates/vox-gui/ui/src/components/layout/DockWorkspace.tsx` (+ test).

- [ ] **Step 1 (verify-before-use):** Read `DockShell.tsx` fully (from Pre-flight). Confirm the exact dockview layout serialize/restore calls (`api.toJSON()`/`api.fromJSON()` or equivalent) and the persistence debounce. Copy that pattern.

- [ ] **Step 2: Write the failing test.** `DockWorkspace.test.tsx`: rendering `<DockWorkspace />` with a stub layout adds one panel per the default preset; a `serializeLayout()` helper returns a JSON object and `restoreLayout(json)` is the inverse (round-trip equals input).

- [ ] **Step 3: Run → FAIL.** `npm test -- DockWorkspace` → FAIL.

- [ ] **Step 4: Implement.** Create `DockWorkspace.tsx` wrapping `DockviewReact`, mounting panels from `panelRegistry` (component factory dispatches on panel kind → `entry.render`). Persist `api.toJSON()` (debounced via the existing constant) to localStorage as the layout SSOT; restore on mount; expose `serializeLayout`/`restoreLayout`. Unknown kind on restore → a removable placeholder panel (self-heal). Do NOT wire presets yet (Task 8).

- [ ] **Step 5: Run → PASS.** `npm test -- DockWorkspace` → PASS; build clean.

- [ ] **Step 6: Commit.** `git commit -m "feat(gui): DockWorkspace over dockview with persisted layout SSOT"`

---

## Task 3 `[SEQUENTIAL]`: re-home existing surfaces as panels

**Files:** Modify `panelRegistry.ts` (swap placeholders for real renders).

- [ ] **Step 1 (verify-before-use):** `rg -n "case '" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` to list the existing surface components (chat, tasks, activity, agents, etc.) and their props.

- [ ] **Step 2–6 (one commit per surface — split-on-overrun):** For each catalog surface (chat, task_list, activity, agents, budget, gamify_minimap, memory), replace its placeholder `render` in `panelRegistry` with the real component wrapper, add a render test, run, commit. **One surface per commit.** Reuse the existing surface components verbatim (do not rewrite them).

```bash
git commit -m "feat(gui): re-home <surface> as a workspace panel"
```

---

## Task 4 `[SEQUENTIAL]`: retire the dnd-kit dashboard grid into dockview

**Files:** Modify `Dashboard.tsx`, `lib/dashboardLayout.ts`.

- [ ] **Step 1 (verify-before-use):** Read `Dashboard.tsx`: confirm the dnd-kit grid + `widgetRegistry` dispatch. Confirm the 14 widget kinds are now panel kinds in `panelRegistry` (Task 1).

- [ ] **Step 2: Write the failing test.** `Dashboard.group.test.tsx`: the dashboard renders its widgets as dockview panels in a single group (assert a known widget appears via the workspace path, not the dnd-kit grid).

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement.** Replace the dnd-kit `SortableContext`/grid in `Dashboard.tsx` with a dockview group of widget panels (each widget = a `panelRegistry` panel). Remove the dnd-kit grid code + its localStorage layout (superseded by the DockWorkspace layout SSOT). Keep `upgradeLayoutIfNeeded` semantics by seeding the default dashboard group.

- [ ] **Step 5: Run → PASS.** Full suite + build clean.

- [ ] **Step 6: Commit.** `git commit -m "refactor(gui): dashboard widgets become dockview panels (retire dnd-kit grid)"`

---

## Task 5 `[SEQUENTIAL]`: context SSOT backend (`context_get`/`context_set`)

**Files:** Modify `crates/vox-orchestrator/src/session/state.rs`; Create `crates/vox-gui/src/commands/context.rs`.

- [ ] **Step 1 (verify-before-use):** From Pre-flight, read `session/state.rs` + `compaction.rs`. Confirm where the ordered context-item list lives and how it's serialized to the API. If context is assembled ad-hoc (no stored list), STOP and report — the SSOT requires a persisted ordered list; note the smallest change to introduce one.

- [ ] **Step 2: Write the failing test.** In `session/state.rs` tests: a `context_items()` accessor returns the ordered list; `set_context_items(items)` replaces it; a `pinned` item survives a `compact()` pass while an unpinned trimmable one is dropped.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator context_items` → FAIL.

- [ ] **Step 4: Implement.** Add `ContextItem { id, role, kind, text, pinned, token_estimate }` + `context_items()`/`set_context_items()` to the session; make `compact()` preserve `pinned`. Add `#[tauri::command] async fn context_get()/context_set(items)` in `commands/context.rs` + `pub const CONTEXT_CHANGED_EVENT: &str = "vox://context-changed";` (emit on set). Register in `main.rs` `generate_handler!`. **The serialized context for the next API call must read from this list** (wire the assembly to it).

- [ ] **Step 5: Run → PASS.** Tests PASS; `cargo check -p vox-gui`.

- [ ] **Step 6: Commit.** `git commit -m "feat(orchestrator): session context-item SSOT + context_get/set (pin-aware)"`

---

## Task 6 `[SEQUENTIAL]`: Context-Window Editor panel

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/Context/ContextEditor.tsx` (+ test); Modify `panelRegistry.ts` (real render for `context`).

- [ ] **Step 1 (verify-before-use):** Paste `rg -n "AttachItem|export" crates/vox-gui/ui/src/lib/loquelaContext.ts`. Reuse `AttachItem`/the context-item model; confirm `ContextWindowMeter` props for the header meter.

- [ ] **Step 2: Write the failing test.** `ContextEditor.test.tsx`: given context items, renders one card each (newest grouping ok); remove drops one; pin marks it; "commit" calls `onCommit(items)` with the edited order/pins.

- [ ] **Step 3: Run → FAIL.** `npm test -- ContextEditor` → FAIL.

- [ ] **Step 4: Implement.** `ContextEditor` loads via `invoke('context_get')`, renders removable/reorderable/pinnable/collapsible cards (reuse chat message rendering + `loquelaContext` model), shows `ContextWindowMeter` in the header, and on Commit calls `invoke('context_set', { items })`; subscribe `vox://context-changed` to refresh. Swap the `context` panel placeholder in `panelRegistry`.

- [ ] **Step 5: Run → PASS.** `npm test -- ContextEditor` → PASS; build clean.

- [ ] **Step 6: Commit.** `git commit -m "feat(gui): editable context-window panel (commit = API context SSOT)"`

---

## Task 7 `[SEQUENTIAL]`: editable memory (persistent + session) feeding context

**Files:** Create `crates/vox-gui/src/commands/memory.rs`; Modify `.../surfaces/Memory/MemoryView.tsx`.

- [ ] **Step 1 (verify-before-use):** Read `MemoryView.tsx` (corpus search + `attachItemsFromHits`). Confirm the persistent memory write path (`rg -n "memory" crates/vox-db/src -l`) and the session working-memory location (from Task 5).

- [ ] **Step 2–6 (split: 7a persistent, 7b session).** 7a: `memory_get`/`memory_set` (+ `vox://memory-changed`) for persistent memory; make `MemoryView` allow inline edit/save of a memory entry; commit. 7b: surface session/working memory as editable entries that also appear as context items (feed the context editor); commit. Each with a test, register commands in `main.rs`.

```bash
git commit -m "feat(gui): editable persistent memory feeding context"   # 7a
git commit -m "feat(gui): editable session memory as context items"     # 7b
```

---

## Task 8 `[SEQUENTIAL]`: reactive layout sync + named presets

**Files:** Modify `DockWorkspace.tsx` (+ test).

- [ ] **Step 1 (verify-before-use):** Read your `DockWorkspace` serialize/restore from Task 2.

- [ ] **Step 2: Write the failing test.** Saving a named preset then restoring it reproduces the layout; switching register (office↔gamified) is persisted; `vox://layout-changed` fires on dock changes.

- [ ] **Step 3: Run → FAIL.**

- [ ] **Step 4: Implement.** Add named presets (save/list/restore) keyed in localStorage; emit `vox://layout-changed` on layout mutation; honor the office/gamified register per panel. Add a small workspace menu (save preset / reset / pick preset).

- [ ] **Step 5: Run → PASS.** Full suite + build clean.

- [ ] **Step 6: Commit.** `git commit -m "feat(gui): workspace presets + reactive layout sync"`

---

## Parallel waves
Mostly sequential — `panelRegistry.ts` is the shared spine (Tasks 1,3,6,7 touch it) and `DockWorkspace.tsx`/`Dashboard.tsx` are shared. Order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8. The only safe parallelism: within Task 3, two surfaces that touch ONLY their own component files can run together, but each still appends to `panelRegistry.ts` (shared) → keep Task 3 surfaces sequential to avoid clobbering the registry.

## Self-review checklist
- [ ] Spec §8 covered: panelRegistry (1), DockWorkspace (2), re-home surfaces (3), retire grid (4), context backend (5), context editor (6), memory (7), presets/sync (8). ✔
- [ ] ONE layout SSOT (dockview serialized) — dnd-kit grid retired (Task 4); no second layout model. ✔
- [ ] Context SSOT = session list; `context_set` commit is exactly what serializes to the next API call; pins survive compaction. ✔
- [ ] Reuses dockview (`DockShell`), `widgetRegistry`, `loquelaContext`/`AttachItem`, existing surface components — not rewrites. ✔
- [ ] Flash: every code step has a verify gate; oversized tasks (3, 7) split per-item; Tauri cmds register in `main.rs` `generate_handler!`; unknown-kind self-heal. ✔
- [ ] Symbol consistency: `panelRegistry`/`PanelKind`/`PANEL_KINDS`; `DockWorkspace`/`serializeLayout`/`restoreLayout`; `ContextItem`/`context_get`/`context_set`/`CONTEXT_CHANGED_EVENT`; `memory_get`/`memory_set`/`vox://memory-changed`. ✔
