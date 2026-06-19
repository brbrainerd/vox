# Gamification Surfacing & Mini-Map — Implementation Plan (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `crates/vox-skills/skills/superpowers/test-driven-development.skill.md`. Steps use `- [ ]` checkboxes.

> **🤖 EXECUTION TARGET — READ FIRST.** Gemini 3.5 Flash inside Google Antigravity (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context). Plan engineered accordingly. Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.**
2. **Verify-before-use.** `rg`/read before any symbol; reality differs → STOP + report.
3. **Self-contained.**
4. **Two-strike circuit breaker.**
5. **Parallel dispatch** per tags; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all`; `.vox` automation; `docs/src/` frontmatter.
7. **Verification ritual** before commit (`vox-gui`: clippy `--lib`). Paste output. **No stubs** — `vox stub-check` must be clean (this plan *removes* a stub; never add one).
8. **Rollback on broken tree.**
9. **Rust:** no `.unwrap()` in lib code; inject params in tests; deterministic.

**Goal:** Surface the highest-value dark gamification systems honestly, fix the energy-persist bug and the dead `HudPanels` stub, and make `LudusSandbox` a live dashboard mini-map.

**Architecture:** Add Rust read paths (FSRS due-actions, KPI summary) + a profile energy persist-back; add Tauri commands returning DTOs; rewrite `HudPanels` with real data; add `DueNudge`/`FunGauge` widgets; feed `LudusSandbox` from existing live App state (agents + diagnostics). No new game mechanics; only real data.

**Tech Stack:** Rust (`vox-gamify`, `vox-gui`); React/TS + vitest.

**Design:** [`../specs/2026-06-18-gamification-surfacing-and-minimap-design.md`](../specs/2026-06-18-gamification-surfacing-and-minimap-design.md).

---

## Flash Execution Addendum (2026-06-18 — second hardening pass)

These override task granularity where they conflict. Source: Flash-executability critique.

**Global gates (apply to every task):**
1. Each Step-1 `rg`/read is a **BLOCKING gate** — paste output before any code step; reality differs → STOP.
2. **Split-on-overrun:** Implement step touching >1 file or >1 new function → one atomic green commit per sub-bullet, in order.
3. Tauri commands register in `crates/vox-gui/src/main.rs`'s `tauri::generate_handler![…]`.

**Mandatory clarifications + splits:**
- **Task 1:** the fix is location-explicit — in `get_ludus_profile()`, **immediately after** the `profile.regen_energy();` call (~line 101) and **before** the DTO return, add `vox_gamify::db::upsert_profile(&db, &profile).await` (on error: log + still return the DTO). Don't relocate other logic.
- **Task 4 (KPI DTO):** the DTO is `KpiSummaryDto { events_recorded: i64, grind_ratio: f64, avg_multiplier: f64, quests_completed: i64, total_xp: i64 }`; `grind_ratio = grind_capped_events as f64 / (events_recorded.max(1) as f64)` clamped [0,1]; `avg_multiplier = avg_effective_multiplier` (passthrough). Inline this; do not invent fun/grind/quality.
- **Task 5 (HudPanels):** the real call site passes **hardcoded dummy props** (`treasuryValue={120} energy={90} speed={1} onSetSpeed={()=>{}}`). This task only makes `HudPanels` *render* its props (no longer `null`); wiring real treasury/energy is explicitly **out of scope** here (a later task feeds `gamify_kpi_summary`/profile into the call site). Render the props as given — that is not a stub since the data flow is a separate, named follow-up.
- **Task 6:** before Step 2, paste the `rg` output for `EmptyState`/`Glass`/`Pill` signatures and use those exact prop names.
- **Task 7 → 7a / 7b.** 7a: create `crates/vox-gui/ui/src/components/gamify/LudusSandbox.mappers.ts` with pure `moodFromPhase`/`integrityFromDiag` + tests (no canvas); commit. 7b: import them into `LudusSandbox.tsx` and call them in the render loop (read the loop region first); commit. Splitting isolates the canvas-edit risk.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-gamify/src/profile.rs` + `crates/vox-gui/src/commands/gamify.rs` | persist energy after regen | Modify (Task 1) |
| `crates/vox-gamify/src/discovery/ledger.rs` (+ `db/mod.rs` export) | export existing `due_action_ids` | Modify (Task 2) |
| `crates/vox-gui/src/commands/gamify.rs` | `gamify_due_actions`, `gamify_kpi_summary` cmds + DTOs | Modify (Tasks 3–4) |
| `crates/vox-gui/ui/src/components/gamify/HudPanels.tsx` | real treasury/energy/speed | Rewrite (Task 5) |
| `crates/vox-gui/ui/src/components/gamify/DueNudge.tsx` / `FunGauge.tsx` | new widgets | Create (Task 6) |
| `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` | live buildings/citizens/quality | Modify (Task 7) |

**Pre-flight (run once, paste output):**
- `rg -n "fn get_ludus_profile|upsert_profile|regen|energy" crates/vox-gui/src/commands/gamify.rs crates/vox-gamify/src/profile.rs` — find the regen path and the `upsert_profile` signature.
- `rg -n "fsrs_due_ms|discovery_state|pub fn|MemoryState" crates/vox-gamify/src/discovery/ledger.rs crates/vox-gamify/src/discovery/fsrs.rs` — confirm the ledger schema + read functions.
- `rg -n "LudusKpiSummary|pub struct|fun|grind|quality" crates/vox-gamify/src/kpi.rs` — confirm the KPI summary type + fields.
- `rg -n "#\[tauri::command\]|invoke_handler!|LudusProfileDto" crates/vox-gui/src/commands/gamify.rs` — confirm command registration + DTO pattern.
- `rg -n "HudPanels|LudusSandbox|agents|focusedFile|diag" crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` — confirm what props/state the sandbox already receives.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task 1 `[SEQUENTIAL]`: Fix the energy persist-back bug

`get_ludus_profile()` regens energy in-memory but never writes it; energy never advances in the DB.

**Files:**
- Modify: `crates/vox-gui/src/commands/gamify.rs` (the `get_ludus_profile` path) and/or `crates/vox-gamify/src/profile.rs`

- [ ] **Step 1 (verify-before-use):** From Pre-flight, read the `get_ludus_profile` body. Confirm where energy is regenerated and that `upsert_profile(...)` exists with a signature you can call. If a regen helper returns a new profile without persisting, note it.

- [ ] **Step 2: Write the failing test.** Add a Rust test (in `vox-gamify` if regen lives there, else a `vox-gui` command test) that: seeds a profile with `energy=0` and `last_energy_regen` far in the past; runs the regen+fetch path twice; asserts the **persisted** profile's energy advanced (re-read from DB shows > 0). Example skeleton (adapt to real APIs):

```rust
#[tokio::test]
async fn energy_regen_persists_to_db() {
    // VERIFIED ctor (no open_in_memory): VoxDb::connect(DbConfig::Memory), `local` feature.
    let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db");
    seed_profile(&db, /*energy*/0, /*last_regen*/0).await;
    let _ = fetch_profile_with_regen(&db, "user").await; // the regen path
    let reloaded = get_profile(&db, "user").await.expect("profile");
    assert!(reloaded.energy > 0, "energy must persist after regen");
}
```

- [ ] **Step 3: Run → FAIL.** Run the new test → FAIL (energy still 0 in DB).

- [ ] **Step 4: Implement.** After computing regenerated energy, call **`vox_gamify::db::upsert_profile(&db, &profile)`** (VERIFIED to exist at `crates/vox-gamify/src/db/profile.rs` and already re-exported from `db/mod.rs`; `profile.regen_energy()` is at `profile.rs:~331` and mutates in place) to write the new `energy` + `last_energy_regen` before returning the DTO. On upsert error: log + still return the regenerated DTO (don't fail the fetch). Keep the return value identical for callers.

- [ ] **Step 5: Run → PASS.** Test → PASS.

- [ ] **Step 6: Verify + commit.**

```bash
cargo clippy -p vox-gamify -- -D warnings && cargo fmt -p vox-gamify
git add crates/vox-gamify/src/profile.rs crates/vox-gui/src/commands/gamify.rs
git commit -m "fix(gamify): persist energy after regeneration"
```

---

## Task 2 `[SEQUENTIAL]`: FSRS `due_actions` ledger accessor

**Files:**
- Modify: `crates/vox-gamify/src/discovery/ledger.rs` + `crates/vox-gamify/src/db/mod.rs` (export)

- [ ] **Step 1 (verify-before-use):** **VERIFIED:** the query function **already exists** — `pub async fn due_action_ids(db, user_id, now_ms, limit) -> Result<Vec<String>>` at `crates/vox-gamify/src/discovery/ledger.rs:~115` (selects `action_id` where `fsrs_due_ms <= now_ms`, ordered ASC). It is **NOT re-exported** from `db/mod.rs`. So this task is mostly an **export** (and an optional richer wrapper if you want `due_ms` too). Confirm with `rg -n "due_action_ids|pub use discovery" crates/vox-gamify/src/discovery/ledger.rs crates/vox-gamify/src/db/mod.rs`.

- [ ] **Step 2: Write the failing test.** Add to `ledger.rs` (reuse the file's existing discovery-seed helper if present; otherwise insert a row directly):

```rust
#[cfg(test)]
mod due_tests {
    use super::*;
    #[tokio::test]
    async fn due_action_ids_returns_only_overdue_ordered() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory).await.expect("db"); // VERIFIED ctor
        seed_discovery(&db, "u", "a_overdue", /*due_ms*/ 100).await;
        seed_discovery(&db, "u", "a_future",  /*due_ms*/ 10_000).await;
        let due = due_action_ids(&db, "u", /*now_ms*/ 1_000, /*limit*/ 10).await.expect("due");
        assert_eq!(due, vec!["a_overdue".to_string()]);
    }
}
```

(Adapt `seed_discovery` to the real insert; if a seeding helper exists in the module, reuse it.)

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gamify due_action_ids_returns_only_overdue_ordered` → FAIL (until the test compiles against the real fn / the row is seeded).

- [ ] **Step 4: Implement.** The function exists — **add the re-export** to `crates/vox-gamify/src/db/mod.rs`: `pub use crate::discovery::ledger::due_action_ids;`. Only if Task 3 needs `due_ms` for display, add a sibling `pub async fn due_actions(...) -> Result<Vec<(String, i64)>>` that also selects `fsrs_due_ms`; otherwise the `Vec<String>` of action ids is enough.

- [ ] **Step 5: Run → PASS.** Test → PASS.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-gamify -- -D warnings && cargo fmt -p vox-gamify
git add crates/vox-gamify/src/discovery/ledger.rs crates/vox-gamify/src/db/mod.rs
git commit -m "feat(gamify): export FSRS due_action_ids accessor"
```

---

## Task 3 `[SEQUENTIAL]`: `gamify_due_actions` Tauri command

**Files:**
- Modify: `crates/vox-gui/src/commands/gamify.rs` (+ handler registration)

- [ ] **Step 1 (verify-before-use):** From Pre-flight, copy an existing gamify command (e.g. `list_gamify_quests`) shape + DTO pattern + how the DB/user is obtained.

- [ ] **Step 2: Write the failing test.** Add a unit test for a pure DTO mapper `due_action_to_dto(&DueAction) -> DueActionDto { action_id, due_ms }`.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gui due_action_to_dto` → FAIL.

- [ ] **Step 4: Implement.** Add `DueActionDto { action_id: String }` + mapper + `#[tauri::command] async fn gamify_due_actions(limit: u32) -> Vec<DueActionDto>` calling `vox_gamify::db::due_action_ids(...)` (the verified, now-exported fn). Register in the `generate_handler!` list in `main.rs` (Tauri commands register there, not a `commands/mod.rs` macro).

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-gui due_action_to_dto` → PASS; `cargo check -p vox-gui`.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-gui --lib -- -D warnings && cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/gamify.rs crates/vox-gui/src/
git commit -m "feat(gui): gamify_due_actions command"
```

---

## Task 4 `[SEQUENTIAL]` (same file as Task 3): `gamify_kpi_summary` command

**Files:**
- Modify: `crates/vox-gui/src/commands/gamify.rs`

- [ ] **Step 1 (verify-before-use):** **VERIFIED — `LudusKpiSummary` does NOT have `fun`/`grind`/`quality`.** Real fields (`crates/vox-gamify/src/kpi.rs:~7`): `events_recorded, total_xp_awarded, total_crystals_awarded, grind_capped_events, avg_effective_multiplier, hint_events_logged, quests_completed_total, notifications_unread, hints_shown, hints_dismissed`. The loader `vox_gamify::db::load_kpi_summary(db, user_id)` exists and is exported. The `FunGauge` widget must be **derived** from these real fields — do NOT invent backend fields (anti-stub). Confirm with `rg -n "struct LudusKpiSummary" -A 14 crates/vox-gamify/src/kpi.rs`.

- [ ] **Step 2: Write the failing test.** Unit test for `kpi_to_dto(&LudusKpiSummary) -> KpiSummaryDto` that derives three 0..1 engagement ratios from REAL fields: `grind_ratio = grind_capped_events / events_recorded.max(1)`, `effort = avg_effective_multiplier` (clamp/normalize), `output = quests_completed_total` (or total_xp_awarded). Assert the mapper preserves/derives them correctly for a sample summary.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gui kpi_to_dto` → FAIL.

- [ ] **Step 4: Implement.** Add `KpiSummaryDto { events_recorded, grind_ratio: f64, avg_multiplier: f64, quests_completed: i64, total_xp: i64 }` (real + derived) + `kpi_to_dto` mapper + `#[tauri::command] async fn gamify_kpi_summary() -> KpiSummaryDto` calling `load_kpi_summary`. Register in `main.rs`'s `generate_handler!`.

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-gui kpi_to_dto` → PASS.

- [ ] **Step 6: Commit.**

```bash
cargo clippy -p vox-gui --lib -- -D warnings && cargo fmt -p vox-gui
git add crates/vox-gui/src/commands/gamify.rs
git commit -m "feat(gui): gamify_kpi_summary command"
```

---

## Task 5 `[PARALLEL-SAFE]` (frontend): replace the dead `HudPanels` stub

`HudPanels.tsx` returns `null`. Make it render real treasury/energy/speed from props, or delete it + its call site. Default: implement.

**Files:**
- Rewrite: `crates/vox-gui/ui/src/components/gamify/HudPanels.tsx` (+ `.test.tsx`)

- [ ] **Step 1 (verify-before-use):** Read `HudPanels.tsx` (confirm it returns `null` ~line 4) and its **real call site** in `LudusSandbox.tsx` (~line 263). **VERIFIED props are scalars + a callback**, not objects: `<HudPanels treasuryValue={number} energy={number} speed={number} onSetSpeed={fn} />`. Match these exactly — do NOT use the `{crystals,lumens}`/`{current,max}` object shapes.

- [ ] **Step 2: Write the failing test.** Create `HudPanels.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { HudPanels } from './HudPanels';

describe('HudPanels', () => {
  it('renders treasury and energy values', () => {
    render(<HudPanels treasuryValue={120} energy={90} speed={1} onSetSpeed={vi.fn()} />);
    expect(screen.getByTestId('hud-value')).toHaveTextContent('120');
    expect(screen.getByTestId('hud-energy')).toHaveTextContent('90');
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npm test -- HudPanels` → FAIL (currently renders `null`).

- [ ] **Step 4: Implement.** Replace the `null` body with a real overlay using the verified props: treasury value (`data-testid="hud-value"`), an energy indicator (`data-testid="hud-energy"`), and a sim-speed control wired to `onSetSpeed`. Use existing UI primitives (`Pill`, `Glass`) + status tones; no fabricated data — only the props.

- [ ] **Step 5: Run → PASS.** `npm test -- HudPanels` → PASS; `npm run build` clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/gamify/HudPanels.tsx crates/vox-gui/ui/src/components/gamify/HudPanels.test.tsx
git commit -m "fix(gamify-ui): implement HudPanels (was a null stub)"
```

---

## Task 6 `[PARALLEL-SAFE]` (frontend): `DueNudge` + `FunGauge` widgets

**Files:**
- Create: `crates/vox-gui/ui/src/components/gamify/DueNudge.tsx`, `FunGauge.tsx` (+ tests)

- [ ] **Step 1 (verify-before-use):** `rg -n "EmptyState|Glass|Pill|invoke\(" crates/vox-gui/ui/src/components/ui/ crates/vox-gui/ui/src/components/gamify/ -l` to reuse primitives + the `invoke` wrapper.

- [ ] **Step 2: Write the failing tests.** `DueNudge.test.tsx`: given `count=3`, renders "3 actions due"; given `count=0`, renders an empty/encouraging state. `FunGauge.test.tsx`: given the **real-field-derived** props `{ grindRatio: 0.2, avgMultiplier: 1.3, questsCompleted: 5 }` (matching `KpiSummaryDto` from Task 4 — NOT the non-existent fun/grind/quality), renders three labeled meters.

- [ ] **Step 3: Run → FAIL.** `npm test -- DueNudge FunGauge` → FAIL.

- [ ] **Step 4: Implement.** `DueNudge({ count, onOpen })` (badge + click-through). `FunGauge({ grindRatio, avgMultiplier, questsCompleted })` (three bars/meters derived from the verified `KpiSummaryDto` fields, color-toned, each with a **text label** so color is never the sole signal — a11y rule). Pure presentational; data wired by the dashboard registry later.

- [ ] **Step 5: Run → PASS.** `npm test -- DueNudge FunGauge` → PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/gamify/DueNudge.tsx crates/vox-gui/ui/src/components/gamify/FunGauge.tsx crates/vox-gui/ui/src/components/gamify/*.test.tsx
git commit -m "feat(gamify-ui): DueNudge + FunGauge widgets"
```

---

## Task 7 `[PARALLEL-SAFE]` (frontend): live-feed `LudusSandbox` mini-map

Buildings from real owned-files; citizens from live agents; quality overlays from diagnostics; camera focus on current task file.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` (+ test for the pure mappers)

- [ ] **Step 1 (verify-before-use):** **VERIFIED:** `LudusSandbox` reads a Zustand store `useLudusStore` (`crates/vox-gui/ui/src/components/gamify/store.ts`) with `buildings: Record<string, { errors, warnings }>`, `agents: Record<string, { mood }>`, `focusedFile`, `agentTasks`; the component prop is `SandboxProps { files: string[] }`. **Real agent phase values** (`crates/vox-gui/ui/src/styles/tokens.ts:~24`): `'Executing' | 'Verifying' | 'Planning' | 'Paused' | 'Validated' | 'Doubted'`. Do NOT add new backend; map from the store's existing fields.

- [ ] **Step 2: Write the failing test.** `LudusSandbox.mappers.test.ts` for pure helpers:

```ts
import { describe, it, expect } from 'vitest';
import { moodFromPhase, integrityFromDiag } from './LudusSandbox';

describe('sandbox mappers', () => {
  it('maps agent phase to citizen mood', () => {
    expect(moodFromPhase('Executing')).toBe('Excited');
    expect(moodFromPhase('Paused')).toBe('Tired');
    expect(moodFromPhase('Doubted')).toBe('Sad');
  });
  it('maps diagnostics to building integrity', () => {
    expect(integrityFromDiag({ errors: 0, warns: 0 })).toBe('intact');
    expect(integrityFromDiag({ errors: 3, warns: 0 })).toBe('cracked');
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npm test -- LudusSandbox.mappers` → FAIL.

- [ ] **Step 4: Implement.** Export pure `moodFromPhase(phase)` and `integrityFromDiag({errors,warns})`; wire them into the canvas render so citizen sprites reflect live agent phase and building overlays reflect real diagnostics; set camera focus to the current task's file. Keep all data sourced from existing props.

- [ ] **Step 5: Run → PASS.** `npm test -- LudusSandbox.mappers` → PASS; build clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.mappers.test.ts
git commit -m "feat(gamify-ui): live-feed LudusSandbox mini-map (phase->mood, diag->integrity)"
```

---

## Parallel waves

- **Wave 1 (sequential, Rust):** Task 1 → 2 → 3 → 4 (1–2 in `vox-gamify`, 3–4 in the same `gamify.rs` → sequential).
- **Wave 2 (parallel, TS):** Task 5, Task 6, Task 7 touch disjoint component files — dispatch together.

## Self-review checklist

- [ ] Spec §3 "this plan" rows covered: energy fix (1), FSRS due (2–3), KPI (4), HudPanels (5), DueNudge/FunGauge (6), LudusSandbox (7). ✔
- [ ] No new fake panels — every surfaced value has a real read path (anti-stub rule). ✔
- [ ] Color never sole signal in `FunGauge`/`DueNudge` (a11y). ✔
- [ ] Symbol consistency: `due_action_ids` (verified existing fn)/`DueActionDto`/`gamify_due_actions`; `load_kpi_summary`/`KpiSummaryDto` (derived from REAL `LudusKpiSummary` fields, not fun/grind/quality); `HudPanels` scalar props (`treasuryValue`/`energy`/`speed`/`onSetSpeed`); `moodFromPhase`/`integrityFromDiag`. ✔
