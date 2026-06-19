# Dockable Workspace + Context-Window Editor + Editable Memory (SSOT) — Design Spec

**Date:** 2026-06-19
**Status:** Design (approved for planning)
**Author:** Brainstorming session (Claude, Opus 4.8)
**Addendum + addition to:** [task-list-cascade-spine](2026-06-18-task-list-cascade-spine-design.md) · [activity-log-surface](2026-06-18-activity-log-surface-design.md) · [gamification-surfacing-and-minimap](2026-06-18-gamification-surfacing-and-minimap-design.md) · [dashboard-topbar-unification](2026-06-18-dashboard-topbar-unification-design.md) · [unified-task-message-envelope-registers-budget-ssot](2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## 1. Problem

Three gaps remain after the prior five specs:

1. **Two layout systems coexist and will drift.** The dashboard uses a dnd-kit **widget grid**
   (`widgetRegistry` SSOT). The shell also has **`DockShell.tsx`** (built on `dockview` 6.6.1,
   with split panels, persisted layouts via `LAYOUT_PERSIST_DEBOUNCE_MS`, ⌘\\/⌘W keybindings, a
   `dockview-vox` theme) — but it wraps *individual* surfaces. There is no single workspace where
   every surface AND every dashboard/gamification widget is a dockable, resizable, draggable,
   bindable panel. The user wants Photoshop/VS-Code docking **all the way down to gamification**.

2. **The context window is read-only.** `ContextWindowMeter.tsx` shows token usage
   (`usedTokens/maxTokens/thresholdTokens/strategy` from `CompactionConfig`) but you **cannot see
   or edit what is actually in the context** that gets serialized to the API/tool calls. The
   assembled context lives in the orchestrator session (`session/state.rs` + `compaction.rs`);
   the FE has a partial model — `lib/loquelaContext.ts` (`AttachItem`) — used to attach composer
   context, but no editable inspector of the full set.

3. **Memory is recall-only.** `MemoryView.tsx` searches corpora (`memory`/`knowledge`/`chunk` via
   `vox_search_query`) and can attach hits to the composer — but it is **not editable**, and
   session/working memory isn't surfaced. The user wants memory **managed, served, and editable**
   the same way as chat, feeding context.

## 2. Goal

- **One panel-registry + dock-layout SSOT over dockview**: every surface — conversational log,
  notepad task list, **context-window editor**, **editable memory**, activity timeline, Q&A,
  agent/resource management, budget/cost, gamification mini-map, and the dashboard widgets — is a
  registered dockview panel: dock to any side, resize, drag, bind, save/restore as named layouts.
  **Retire the dnd-kit dashboard grid into dockview** (the dashboard becomes a dock *group* of
  widget panels). Office-default/gamified registers (spec 5) still apply per panel.
- **Context-Window Editor**: a structured, editable list of context items (system, memory, pinned
  files, chat history, tool results); remove / reorder / pin / collapse; the **committed set is
  the SSOT serialized on the next API/tool call.** Reuses `loquelaContext`/`AttachItem` + chat
  rendering.
- **Editable memory**: persistent project memory + session/working memory, both editable and
  feeding the context SSOT.

Non-goals (YAGNI): a bespoke docking engine (dockview is already present); cross-mesh layout sync;
raw-prompt free-text editing (we edit a *structured* context set, not the assembled string).

## 3. Architecture

```
 panelRegistry.ts  (SSOT: kind → { title, render({panel,navigate,register}), defaultDock, allowMultiple })
        │  (absorbs widgetRegistry; dashboard widgets become panel kinds)
        ▼
 DockWorkspace (generalized DockShell over dockview)
   • panels dock to any side / group, resize, drag, bind
   • layout serialized via dockview api → persisted = LAYOUT SSOT (vox://layout-changed)
   • named presets (save/restore); register-aware (office/gamified)
        │
        ├── Context-Window Editor panel ──► context_get / context_set (Tauri)
        │        backed by session context list (orchestrator session/state.rs + compaction.rs)
        │        commit = exactly what serializes to the next API/tool call (vox://context-changed)
        └── Memory panel ──► memory_get / memory_set (persistent + session) → feeds context (vox://memory-changed)
```

### 3.1 Components

| Unit | File(s) | Responsibility |
|---|---|---|
| `panelRegistry` | `vox-gui/ui/src/lib/panelRegistry.ts` (**new**) | SSOT: panel kind → metadata + render + default dock side/group + `topHudEligible`/`register`. Absorbs `widgetRegistry` entries as panel kinds. |
| `DockWorkspace` | `vox-gui/ui/src/components/layout/DockWorkspace.tsx` (**new**, generalizes `DockShell.tsx`) | Hosts registry panels in dockview; serialize/restore layout (SSOT); save/named presets; emit `vox://layout-changed`. |
| Dashboard-as-group | `.../surfaces/Dashboard/Dashboard.tsx` (refactor) | Dashboard becomes a dockview group of widget panels; dnd-kit grid removed (its widgets become panels). |
| Context editor | `.../surfaces/Context/ContextEditor.tsx` (**new**) + `context.rs` Tauri (**new**) | Structured editable context-item list; commit → context SSOT. Reuses `loquelaContext`/`AttachItem`. |
| Context SSOT (backend) | `vox-orchestrator` session context (extend `session/state.rs`; respect `compaction.rs`) | The ordered context-item list is the truth; `context_get`/`context_set` read/write it; serialized to the LLM next call. |
| Memory editor | `.../surfaces/Memory/MemoryView.tsx` (extend) + `memory.rs` Tauri (**new**) | Edit persistent + session memory; feed context. |

### 3.2 The layout SSOT
dockview already serializes its layout. We persist that serialization (per the existing
`LAYOUT_PERSIST_DEBOUNCE_MS` pattern) as **the** workspace SSOT, plus named presets. No second
layout model: the dnd-kit grid is removed, so there is exactly one truth for "what panel is where."
Stale/unknown panel kinds in a saved layout render a removable placeholder (self-heal), mirroring
the dashboard `UnknownWidget` rule.

### 3.3 The context SSOT (the sharp part)
The orchestrator session owns the ordered list of context items that compaction operates on and
that get serialized to the API. The editor reads it (`context_get`), lets the user
remove/reorder/pin/collapse, and writes it back (`context_set`) — and **that committed list is
exactly what the next API/tool call sends**. Pinned items survive compaction. This makes "what the
model sees" a first-class, user-owned, single source of truth instead of an opaque assembly.

## 4. SSOT sync — full picture (extends spec 5 §6)

| Domain | SSOT | Reactive topic | Surfaces |
|---|---|---|---|
| Layout | persisted dockview layout + presets | `vox://layout-changed` | the whole workspace |
| Context | session context-item list | `vox://context-changed` | context editor, context meter, chat |
| Memory | persistent + session memory | `vox://memory-changed` | memory panel, context editor |
| Tasks / Activity / Budget | (specs 1/2/5) | `vox://tasks-changed` / `activity-appended` / `cost-changed` | their panels |

Rule everywhere: **state → topic → re-read.** Every panel is a subscriber, never a truth-owner.

## 5. Panel catalog (all dockable/resizable/bindable)
conversational log (chat + narration + Q&A) · notepad task list · **context-window editor** ·
**memory** · activity timeline (mesh-wide) · Q&A/approvals · agents & resources · budget/cost ·
gamification mini-map (LudusSandbox) · dashboard widget panels (the 14 kinds). Each is a
`panelRegistry` entry; each honors the office/gamified register and the VUV design validators
(`web_ir/validate_{palette,layer,a11y,overlay}`) — layer/overlay discipline matters for docking.

## 6. Error handling
- Unknown panel kind in a saved layout → removable placeholder (self-heal), never a crash.
- `context_set` that would exceed `maxTokens` → accepted but flagged (the meter shows danger zone); compaction still governs runtime.
- `context_get`/`memory_get` before a session exists → empty list, not an error.

## 7. Testing strategy
- **Unit:** panelRegistry completeness (every catalog kind has an entry); layout serialize→restore round-trip; unknown-kind placeholder; context-item edit ops (remove/reorder/pin) preserve order + pins; memory edit round-trip.
- **Integration:** `context_set` then a model call serializes exactly the committed set; pinned item survives a compaction pass; editing memory updates the context editor via `vox://memory-changed`.

## 8. Decomposition into plan tasks (preview)
1. `panelRegistry` SSOT (absorb widgetRegistry) + completeness test.
2. `DockWorkspace` over dockview: host registry panels + persist layout SSOT + presets.
3. Re-home existing surfaces as panel kinds (chat/task/activity/agents/budget/gamification/memory).
4. Retire the dnd-kit dashboard grid → dashboard as a dockview group.
5. Context SSOT backend (`context_get`/`context_set` over session list, pin-aware) + tests.
6. Context-Window Editor panel (reuse `loquelaContext`/`AttachItem`) + commit→SSOT.
7. Editable memory (persistent + session) feeding context.
8. Reactive sync (`vox://layout-changed`/`context-changed`/`memory-changed`) + named presets.

## 9. How this amends the prior plans
- **Dashboard plan:** `widgetRegistry` is **absorbed into** `panelRegistry`; the dnd-kit grid is retired into dockview (Task 4 here). The 14 widget kinds become panel kinds; the completeness test moves to panelRegistry.
- **Spec 5 (unified):** the context editor consumes the same TaskMessage/skills/context vocabulary; `useBudget`/registers apply per panel.
- **Specs 1/2/3:** their surfaces (task list, activity, gamification mini-map) become registry panels — no change to their backends.
