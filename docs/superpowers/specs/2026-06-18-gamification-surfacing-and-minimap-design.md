# Gamification Surfacing & Mini-Map — Design Spec

**Date:** 2026-06-18
**Status:** Design (approved for planning)
**Author:** Audit + brainstorming session (Claude, Opus 4.8)
**Sibling specs:** [task-list-cascade-spine](2026-06-18-task-list-cascade-spine-design.md) · [activity-log-surface](2026-06-18-activity-log-surface-design.md) · [dashboard-topbar-unification](2026-06-18-dashboard-topbar-unification-design.md)
**Execution target:** Gemini 3.5 Flash inside Antigravity — see [limitations doc](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md).

---

## Amendment 2026-06-18 (unification) — see [unified-task-message-envelope spec](2026-06-18-unified-task-message-envelope-registers-budget-ssot-design.md)

Two ties to the unification: (1) the HUD **treasury reads the budget SSOT**
(the *existing* `BudgetManager`, via `budget_get` + `vox://cost-changed`) so "treasury drain"
is the *same* number as the Office budget widget — not an independent cost sum; (2) the gamified layer is the **opt-in register**
(Office is default) rendered via the register-aware `widgetRegistry`, and mini-map quests/citizens
reflect the **TaskMessage envelope** (skills/context) rather than flattened text. Implemented in
the unified plan (registers/budget) + this plan (the gamified variants).

---

## 1. Problem

The gamification backend (`crates/vox-gamify`) is architecturally rich — profile/XP/
prestige/lumens, streaks+shields, achievements, an **FSRS spaced-repetition discovery
ledger**, quests, companions, battles, shop, arena, collegium, dispute jury, a reward
policy with anti-grind, ~21 vox-db tables. But:

- **~70% is computed and persisted yet never reachable from the GUI.** Surfaced today:
  profile/XP/streak, quests, companions, leaderboard, notifications. Dark: **FSRS
  discovery** (due actions), arena, collegium, disputes, shop, battles, KPI summaries
  (fun-vs-grind), teaching coaching, policy A/B snapshots.
- **Dead stub:** `HudPanels.tsx` returns `null` (so `LudusSandbox`'s treasury/energy/speed
  overlay never renders); `LudusSandbox` itself is only half-wired.
- **Correctness bug:** `get_ludus_profile()` regenerates energy in-memory on each fetch
  but **never persists it back**, so energy never actually advances in the DB.
- **The mini-map already exists but is underused:** `LudusSandbox` is an isometric canvas
  that places files/agents as buildings with quality overlays, embedded on the dashboard
  with an "Immersive View" deep-link — it is exactly the "mini-map with important
  information" the user wants, but it's stubbed and not fed live data.

The user's ask: *"consider the gamification … current limitations … creative ideas to
visualize what is going on in the codebase at any given time, making it a more fun,
interactive view where you both control agents and see what's going on … a mini-map with
important information on the dashboard."*

## 2. Goal

Turn the dark backend into a coherent, **honest** surface and make `LudusSandbox` a
live, first-class dashboard mini-map:
- fix the energy-persist bug and the `HudPanels` dead stub (real data or removed);
- surface the **highest-value dark systems** — FSRS "due to revisit" nudges and the KPI
  **fun-vs-grind / quality** summary — because they change behavior, not just decoration;
- feed `LudusSandbox` live: agents as citizens, files as buildings, quality overlays from
  real diagnostics, current-task focus.

Non-goals (YAGNI): surfacing *every* dark system at once (arena/collegium/disputes are
multiplayer/social and out of scope here), a new game economy, and replacing the existing
profile/quests/companions surfaces (they stay; we add to them).

## 3. What to surface (prioritized)

| System | Backend status | Value | This plan? |
|---|---|---|---|
| Energy persist-back | bug (in-memory only) | correctness | **Yes** (fix) |
| `HudPanels` overlay | dead `null` stub | the sandbox HUD | **Yes** (real or remove) |
| FSRS discovery "due" | full ledger, no command | high — behavioral nudge | **Yes** (read cmd + widget) |
| KPI fun-vs-grind / quality | computed, no command | high — self-awareness | **Yes** (read cmd + widget) |
| LudusSandbox live feed | half-wired | the mini-map | **Yes** |
| Shop / battles / arena / collegium / disputes | built, dark | medium/social | **No** (later spec) |

The principle (matches the project's anti-stub rule): **only surface what is real**;
where a system is dark, either wire a real read path or leave it out — never a fake panel.

## 4. Architecture

```
 vox-gamify (Rust)                       vox-gui Tauri commands            React
 ┌───────────────┐   discovery::ledger   ┌──────────────────────┐   ┌──────────────────┐
 │ FSRS ledger   │ ─────────────────────►│ gamify_due_actions   │──►│ DueNudge widget  │
 │ kpi summary   │ ─────────────────────►│ gamify_kpi_summary   │──►│ FunGauge widget  │
 │ profile (fix  │ ─ persist energy ────►│ get_ludus_profile    │──►│ LudusHud         │
 │  energy)      │                       └──────────────────────┘   └──────────────────┘
 └───────────────┘
 live agents + file diags ── App state ──────────────────────────►  LudusSandbox (mini-map)
                                                                      + HudPanels (real)
```

### 4.1 Components

| Unit | File(s) | Responsibility |
|---|---|---|
| Energy persist fix | `vox-gamify/src/profile.rs` + the `get_ludus_profile` path in `vox-gui/src/commands/gamify.rs` | After regen, `upsert_profile` so energy advances durably. |
| `gamify_due_actions` cmd | `vox-gui/src/commands/gamify.rs` (extend) | Read FSRS ledger for items with `fsrs_due_ms <= now`; return top-N as `DueActionDto`. |
| `gamify_kpi_summary` cmd | `vox-gui/src/commands/gamify.rs` (extend) | Return `LudusKpiSummary` (fun/grind/quality) as DTO. |
| Ledger read accessor | `vox-gamify/src/discovery/ledger.rs` (extend/export) | `due_actions(user, now_ms, limit)` query (export from `db/mod.rs`). |
| `HudPanels` real impl | `vox-gui/ui/src/components/gamify/HudPanels.tsx` (rewrite) | Render treasury (crystals/lumens), energy bar, sim speed from real props — or delete + remove call site. |
| `DueNudge` widget | `vox-gui/ui/src/components/gamify/DueNudge.tsx` (**new**) | "N actions due to revisit" with click-through. A dashboard widget kind. |
| `FunGauge` widget | `vox-gui/ui/src/components/gamify/FunGauge.tsx` (**new**) | fun-vs-grind/quality gauge. A dashboard widget kind. |
| `LudusSandbox` live feed | `vox-gui/ui/src/components/gamify/LudusSandbox.tsx` (extend) | Buildings from real owned-files; citizen sprites from live agents; quality overlays from `FileDiagChanged`/diagnostics; camera focus on current task file. |

### 4.2 Mini-map data contract

`LudusSandbox` consumes the same App-state arrays the dashboard already has (`agents`,
file diagnostics, focused file). No new backend: it is a **visualization of existing live
state**. Buildings = owned files; citizen = agent (mood from agent phase: Executing=Excited,
Paused=Tired, Doubted=Sad); cracks/overlays = error/warn counts per file.

## 5. Creative-but-grounded visualization ideas (scoped)

Only ideas backed by data that already exists:
- **Activity → city life:** agent `TaskPhaseChanged` drives citizen animation state (real `vox://agent-events`).
- **Cost as "treasury drain":** `CostIncurred` decrements the HUD treasury visibly (real event).
- **FSRS as "neglected districts":** files/actions overdue for revisit glow on the map (real ledger).
- **Quality as building integrity:** `FileDiagChanged` error/warn → visible cracks (real diags).

These are mappings of existing streams to the existing canvas — not new mechanics.

## 6. Error handling

- Energy upsert failure → log; return the in-memory-regenerated value (don't fail the fetch).
- FSRS/KPI command with no data → empty list / zeroed gauge, not an error.
- `LudusSandbox` with no agents/files → idle city (existing empty behavior), no crash.

## 7. Testing strategy

- **Unit (Rust):** energy regen→persist round-trip (fetch twice across a time delta, assert DB advanced); `due_actions` query returns only `due_ms <= now`, ordered, limited; KPI summary DTO mapping.
- **Unit (TS):** `HudPanels` renders treasury/energy from props (no more `null`); mood-from-phase mapping; quality-overlay-from-diag mapping.
- **Integration:** `gamify_due_actions` / `gamify_kpi_summary` commands return well-formed DTOs against a seeded DB.

## 8. Decomposition into plan tasks (preview)

1. Fix energy persist-back (Rust) + round-trip test.
2. `due_actions` ledger accessor + export + test (Rust).
3. `gamify_due_actions` Tauri command + DTO (Rust).
4. `gamify_kpi_summary` Tauri command + DTO (Rust).
5. `HudPanels` real implementation (TS) + test.
6. `DueNudge` + `FunGauge` widgets (TS) + tests.
7. `LudusSandbox` live feed: buildings/citizens/quality/focus (TS) + tests.

Rust (1–4) and TS (5–7) split by file disjointness for `[PARALLEL-SAFE]` tagging.
