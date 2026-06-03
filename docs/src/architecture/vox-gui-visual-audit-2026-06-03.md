---
title: Vox GUI Visual Audit & Fix Handoff (2026-06-03)
description: Full-surface screenshot audit of the Tauri GUI frontend — the app-blanking icon crash found and fixed, plus a prioritized list of remaining visual/robustness fixes and the reusable Playwright capture harness.
category: "Architecture SSOTs"
---

# Vox GUI Visual Audit & Fix Handoff (2026-06-03)

## How this was captured (scope & method)

The Tauri desktop GUI has **no Tauri-runtime test harness** (long-standing gap). For a visual sweep we
therefore drove the **React frontend** under the Vite dev server with a **mocked Tauri bridge**
(`window.__TAURI_INTERNALS__.invoke`), the same mechanism the existing `e2e/dashboard.spec.ts` uses.

- Harness (reusable, committed): `crates/vox-gui/ui/e2e/screenshots.spec.ts` +
  `crates/vox-gui/ui/playwright.screens.config.ts`.
- Run it: start the dev server (`pnpm --dir crates/vox-gui/ui dev`, port **1420**), then
  `pnpm --dir crates/vox-gui/ui exec playwright test --config=playwright.screens.config.ts`.
- Output: a full-page PNG per surface under `crates/vox-gui/ui/e2e/screens/` (24 surfaces + the command
  palette). The PNGs are regenerable build artifacts (not committed).

**Scope caveat.** This audits the rendered frontend with representative fixture data. It does **not** cover
real-IPC behavior, live event streams, the native Tauri window chrome, or true backend data shapes. Several
"empty/partial" panels below are fixture gaps in the harness, called out as such — they are not app bugs.

---

## P0 — Critical (found *and fixed* during this audit)

### 1. The entire app rendered as a blank screen on every view — missing `Icon.plus`

The always-present Loquela composer (bottom chrome) renders `<Icon.plus />` (`Loquela.tsx:245`), but `plus`
was **not an exported key** of the `Icon` map (`components/ui/Icons.tsx`). React treats an `undefined`
element type as a fatal render error and **unmounts the whole root** — so *every* surface was a blank page.

- **Why it shipped undetected:** the `build`/`lint` script is `vite build` (esbuild) with **no
  typechecking**, so `Icon.plus` (a non-existent property) produced no error, and there is no
  Tauri-runtime smoke test. The first time it surfaced was this Playwright sweep (all 24 screenshots were
  byte-identical blank pages until the fix).
- **Fixed:** added `plus` to `Icons.tsx`. The app now renders.

### 2. Missing `Icon.link` — crashed Search, Memory, and the Command Palette

`Icon.link` is referenced in `SearchView.tsx`, `MemoryView.tsx`, and `CommandPalette.tsx` (the surfaces
added in the 2026-06-03 search work) for the open/link affordance, but `link` was never an exported icon.
Any hit rendering the link affordance would crash that surface.

- **Fixed:** added `link` to `Icons.tsx`.

### 3. Missing `Icon.chevronDown` / `Icon.chevronUp` — `layout/WidgetContainer.tsx`

Latent crash if `WidgetContainer` renders (currently orphaned/unmounted, so lower impact). **Fixed:** added
both to `Icons.tsx` for safety.

> **All four missing icons are fixed in this change.** The systemic cause is P1‑1 below.

---

## P1 — High (systemic + chrome)

### 1. The build does not typecheck — add `tsc --noEmit` to the gate

`package.json` `lint` and `build` are both `vite build`. esbuild strips types without checking them, so
**every** missing-icon / wrong-prop / shape-mismatch bug above was invisible to CI. This is the root cause
of the P0 crashes.

- **Fix:** add a real typecheck — e.g. `"typecheck": "tsc --noEmit"` — and run it in `lint` and in the
  `gui-*` CI gates. (A single `tsc --noEmit` would have caught all four missing icons at build time.)
- Optionally add a one-screen Playwright smoke test (reuse `screenshots.spec.ts`) that asserts the app root
  is non-empty, so an app-blanking regression fails CI.

### 2. Top HUD right edge collides with the Command box — on *every* screen

In the top HUD (`layout/TopHud.tsx`), the 5th tile (a gear/settings glyph at ~x=1135) is **clipped and
overlapped** by the floating `Command ⌘K / LIVE` box. Visible on all 24 captures (it's fixed chrome).

- **Fix:** reserve horizontal space for the command box (or move it out of the HUD tile row / make the HUD
  tiles responsive so the last tile isn't occluded).

---

## P2 — Medium (visible polish / data display)

### 3. Settings: reversed unit formatting

`SettingsView` shows `%60` (should be `60%`) for the Auto-doubt threshold and `min5` (should be `5 min`) for
the checkpoint cadence — the unit and value are concatenated in the wrong order. Fix the template strings.

### 4. Memory: header says "0 indexed entries" while shards show counts

The Mnemosyne header reads `… 0 indexed entries` even though the shard cards show real counts
(1,280 / 540 / 96 / 210). `totalEntries` is summing the wrong field (or a field that's 0). Sum
`corpus_counts` / shard `entries`.

### 5. Runs: Model Scoreboard "MODEL" column is blank

In `RunsView` the scoreboard renders OK%/Q values but the **MODEL** name column is empty (and CAT/CALLS/
P50/$/SUCC are dashes). Partly a harness fixture gap, but verify the real `get_model_scoreboard` DTO field
the MODEL column reads — a name field mismatch would leave it blank in production too.

### 6. Publications board: last stage columns clip off-screen

The 7-stage Kanban (`PublicationsView`) overflows horizontally; PUBLISHED/FAILED are cut at the right edge
with no visible scroll affordance. Narrow the columns, add a visible horizontal scrollbar, or wrap.

---

## P3 — Low (robustness, empty states, cosmetics)

7. **Unguarded numeric access can white-screen on partial payloads.** `ModelsView`
   (`summary.exploration_spent_usd.toFixed(...)`) and `MemoryView` (`…toLocaleString()`) crash if the
   backend returns a present-but-incomplete object (the `summary ?` guard only checks `null`, not field
   presence). Safe today with the typed Rust DTOs, but add `?? 0` / optional chaining to harden against
   error/partial payloads. (These two surfaces white-screened in the harness until the fixtures were
   completed — the same would happen on a malformed real payload.)
8. **Empty-state copy.** Dashboard "The Stream" (0 events), Claims (pre-input), and other zero-data regions
   show large blank areas. Add empty-state messaging like the good examples already present (Approvals "No
   pending approvals", Memory "No recall yet", Mesh "dispatch disabled" — keep these as the pattern).
9. **Loquela slash-command list is missing React `key` props** (console warning). Add `key`.
10. **Policies/Matrix renders at an inconsistent scale** vs other surfaces (cosmetic; verify it's not a
    stray transform/zoom on `IntentionMatrix`).
11. **Harness MODEL dropdown is empty** — wire the `<select>` to `list_model_cards` (or the active model
    list) so a model can be chosen.
12. **Bottom content can sit under the fixed Loquela composer** (e.g. Memory shard cards). Ensure surfaces
    have enough bottom padding that the fixed composer never obscures content.

---

## What looks good (keep)

- Consistent, polished "operator console" aesthetic (brass-on-void, frosted `Glass` panels, display/mono
  type) across all surfaces.
- **All 2026-06-03 work renders correctly:** Search (scope chips, path filter, empty state), Gamify
  (`LudusHud` XP/level/crystals + notifications with Ack), Scientia dashboard (KPI tiles, by-class, top/
  stalled candidates), Claims, the Publications stage board, Research (sessions with status colors),
  Coverage (the self-surfacing tier table), and the **Command Palette backend search** (with working link
  icons).
- Honest disabled/empty states (Mesh "dispatch disabled" with the exact env var to set; Approvals; Memory
  "No recall yet").

## Harness/fixture caveats (not app bugs)

Several panels showed empty/zero or dashed cells purely because the Playwright invoke mock didn't replicate
every DTO exactly: Approvals pending list, Skills/Plugins list, Runs scoreboard columns, Harness model
dropdown, and the Mesh node detail columns (HOST/GPU/TRUST/MODELS). These all rendered their empty/zero
states **without crashing** — a positive resilience signal. To capture them populated, extend the fixtures
in `screenshots.spec.ts` to match those DTOs.

## Suggested fix order

1. **Commit the icon fix (done here)** — unblocks the entire GUI.
2. **P1‑1 add `tsc --noEmit` to lint + CI** — prevents the whole class of P0 bugs from recurring.
3. **P1‑2 TopHud overlap** — it's on every screen.
4. P2 batch (Settings units, Memory total, Publications overflow, Runs scoreboard field).
5. P3 robustness + empty-state polish.
