# vox-gui Wave 6 — Remaining Surfaces Implementation Plan

**Goal:** Browser, Gamify, SkillsPlugins, Search, Tasks, Catalog, Matrix, Flow, Runs, Settings deep IPC.

**Note:** Settings retains ~24 invoke calls until this wave; App bootstrap IPC deferred here.

---

## Task 1: Browser surface

- [ ] Add browser commands to VoxTransport (`browser_navigate`, etc.)
- [ ] Migrate `BrowserView.tsx` off raw invoke

---

## Task 2: Search unification

- [ ] Execute `2026-06-16-gui-unified-search.md`

---

## Task 3: Action manifest

- [ ] Execute `2026-06-16-gui-action-manifest-forms.md`

---

## Task 4: Matrix / Flow / Runs

- [ ] Matrix: migrate `invoke` for routing intentions → transport
- [ ] Flow: optional `@xyflow` a11y pass
- [ ] Runs: confirm hard-bound list (no VL needed)

---

## Exit criteria

- `ipcBoundaries.test.ts` production allowlist empty (all invoke via transport)
- All 106 registry surfaces ≥ `read_only` or `generic_form` tier where applicable
