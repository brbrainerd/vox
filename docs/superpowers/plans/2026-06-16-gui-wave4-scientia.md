# vox-gui Wave 4 — Scientia Implementation Plan

**Goal:** Apply 24-item checklist to Scientia cluster (11 components, largely built).

**Paths:** `surfaces/Scientia/*`

---

## Task 1: Checklist audit

- [ ] Run manual pass against 24-item spec; record gaps in component README comment
- [ ] Ensure all panels use `<Async>` where IPC-backed

---

## Task 2: Registry sync

- [ ] Verify all Scientia view keys in `surface-registry.v1.yaml`
- [ ] `vox ci gui-surface-registry` green

---

## Exit criteria

- Existing Scientia vitest suite (10+ files) remains green
- No new direct `invoke` in Scientia/
