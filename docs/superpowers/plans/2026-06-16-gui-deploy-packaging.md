# vox-gui Deploy and Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship signed Tauri 2 installers from CI with correct bundle paths, a VoxScript-first build entrypoint, and documented fresh-clone bootstrap.

**Architecture:** Tauri project root is `crates/vox-gui/` (no `src-tauri/`). Frontend builds via `pnpm build` in `crates/vox-gui/ui/`. Release workflow uses `tauri-apps/tauri-action` with `projectPath: ./crates/vox-gui`. Cross-build PR workflow compiles sidecar + `cargo build -p vox-gui` but does not yet run full `cargo tauri build`.

**Tech Stack:** Tauri 2, Vite 6, pnpm, GitHub Actions, `scripts/gui-build.vox`.

> **Source of truth:** Master roadmap Track 2; audit items A1–A25.

> **Commands:** `vox run scripts/gui-build.vox` (repo root); `pnpm build` from `crates/vox-gui/ui/`.

---

## Scope

**In scope:**
- Fix `release-gui.yml` artifact paths if bundle output drifts from Tauri 2 layout
- Add one matrix leg in `gui-cross-build.yml` running `cargo tauri build` (Linux self-hosted or documented GitHub-hosted exception)
- Chain `scripts/gui-build.vox` into release workflow
- Document bootstrap in `docs/src/reference/gui-navigation.md`
- Enable Playwright in CI post-merge (`VOX_GUI_PLAYWRIGHT=1` in `ci.yml` gui job)

**Out of scope:** Mobile android/ios targets, native menu/tray (principles §7 post-v1).

---

## Task 1: Verify Tauri 2 bundle output paths

**Files:**
- Read: `crates/vox-gui/tauri.conf.json`
- Modify: `.github/workflows/release-gui.yml` (if artifact glob wrong)

- [ ] **Step 1:** Local smoke: `cd crates/vox-gui/ui && pnpm build && cd .. && cargo tauri build` on one OS
- [ ] **Step 2:** Confirm bundles land under `crates/vox-gui/target/release/bundle/` (not `src-tauri/target`)
- [ ] **Step 3:** Update `tauri-action` `args` / artifact upload globs if mismatch

---

## Task 2: Full bundle leg in gui-cross-build

**Files:**
- Modify: `.github/workflows/gui-cross-build.yml`

- [ ] **Step 1:** Add optional job `gui-tauri-bundle` on `[self-hosted, linux, x64, docker]` (or registered exception)
- [ ] **Step 2:** Run `pnpm build` + `cargo tauri build --bundles appimage` after sidecar staging
- [ ] **Step 3:** Upload bundle artifact with 7-day retention for manual QA

---

## Task 3: VoxScript build entrypoint in release

**Files:**
- Modify: `.github/workflows/release-gui.yml`
- Read: `scripts/gui-build.vox`

- [ ] **Step 1:** Replace ad-hoc `pnpm build` + manual cargo steps with `vox run scripts/gui-build.vox --release`
- [ ] **Step 2:** Ensure script invokes frontend build, sidecar copy, and `cargo tauri build` in order
- [ ] **Step 3:** Document env vars (`VOX_REPO_ROOT`, signing secrets) in script header comment

---

## Task 4: Fresh-clone bootstrap docs

**Files:**
- Modify: `docs/src/reference/gui-navigation.md`

- [ ] **Step 1:** Add **Developer bootstrap** section:
  ```text
  vox ci gui-surface-registry --write
  vox ci config-gui-codegen --write
  cd crates/vox-gui/ui && pnpm install && pnpm build
  cargo run -p vox-gui
  ```
- [ ] **Step 2:** Link to `vox ci gui-smoke` as verification gate
- [ ] **Step 3:** Run scoped doc lint on the file

---

## Task 5: Playwright in CI

**Files:**
- Modify: `.github/workflows/ci.yml` (gui job)

- [ ] **Step 1:** Set `VOX_GUI_PLAYWRIGHT=1` when gui-ui job runs on merge to default branch
- [ ] **Step 2:** Run `pnpm exec playwright test` from `crates/vox-gui/ui/e2e/` after vitest
- [ ] **Step 3:** Store HTML report as artifact on failure

---

## Exit criteria

- Tagged release produces MSI/DMG/AppImage via `release-gui.yml`
- Cross-build CI exercises full `cargo tauri build` on at least one OS
- Bootstrap docs committed; `vox ci gui-smoke` documented as local gate
