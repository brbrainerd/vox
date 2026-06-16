# vox-gui Wave 5 — Config and Ops Implementation Plan

**Goal:** Policies, Memory, Mesh, Models, Repository — operator config surfaces.

**Paths:** `Policies/`, `Memory/`, `Mesh/`, `Models/`, `Repository/`

**Depends on:** Navigation layout plan (Policies two-rail) before polish pass.

---

## Task 1: Memory IPC migration

- [ ] `get_gui_preference` / `set_gui_preference` → `voxTransport` in MemoryView
- [ ] `get_memory_status` → `useVoxQuery` wrapper on transport

---

## Task 2: Policies two-rail

- [ ] Implement per `2026-06-16-gui-navigation-layout.md` Task 4

---

## Task 3: Models/Mesh/Repository

- [ ] `<Async>` + `EmptyState` on each data panel
- [ ] Per-surface vitest for error path

---

## Exit criteria

- Memory prefs via VoxTransport only
- Policies layout matches governance spec at xl breakpoint
