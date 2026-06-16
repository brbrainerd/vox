# vox-gui Deploy and Packaging Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Ship signed Tauri 2 installers from CI with VoxScript-first build, documented bootstrap, and Playwright in merge CI.

**Architecture:** Tauri root is `crates/vox-gui/` (no `src-tauri/`). `scripts/gui-build.vox` orchestrates `pnpm build` + sidecar copy + `cargo tauri build`. `release-gui.yml` uses `tauri-action` with `projectPath: ./crates/vox-gui`.

**Tech Stack:** Tauri 2, pnpm, GitHub Actions, `vox run scripts/gui-build.vox`.

---

## Task 1: Local bundle smoke

**Files:**
- Read: `crates/vox-gui/tauri.conf.json`
- Read: `scripts/gui-build.vox`

- [ ] **Step 1: Run local build**

```bash
cd c:/Users/Owner/vox
vox run scripts/gui-build.vox
```

Expected: `pnpm build` succeeds; Rust GUI crate builds.

- [ ] **Step 2: Full bundle (one OS)**

```bash
cd crates/vox-gui
cargo tauri build
```

Expected: artifacts under `crates/vox-gui/target/release/bundle/` (msi/dmg/deb/appimage per OS).

- [ ] **Step 3: Record actual paths** in this plan's Task 2 if `tauri-action` glob differs.

---

## Task 2: `gui-cross-build.yml` bundle leg

**Files:**
- Modify: `.github/workflows/gui-cross-build.yml`

- [ ] **Step 1: Add job `gui-tauri-bundle-linux`**

```yaml
  gui-tauri-bundle-linux:
    name: GUI Tauri bundle (Linux)
    runs-on: [self-hosted, linux, x64, docker]
    needs: gui-cross-build
    if: github.event_name == 'workflow_dispatch' || github.ref == 'refs/heads/main'
    steps:
      - uses: actions/checkout@v6
      - uses: pnpm/action-setup@v6
        with:
          version: 9
      - uses: actions/setup-node@v6
        with:
          node-version: 24
      - uses: dtolnay/rust-toolchain@stable
      - name: Build via VoxScript
        run: vox run scripts/gui-build.vox
      - name: Bundle AppImage
        working-directory: crates/vox-gui
        run: cargo tauri build --bundles appimage
      - uses: actions/upload-artifact@v4
        with:
          name: vox-gui-appimage
          path: crates/vox-gui/target/release/bundle/appimage/*.AppImage
          retention-days: 7
```

- [ ] **Step 2: Register exception** in `docs/src/ci/github-hosted-exceptions.md` if using `ubuntu-latest` instead of self-hosted.

- [ ] **Step 3: Dry-run** via `workflow_dispatch`

- [ ] **Step 4: Commit** `ci(gui): add Linux tauri bundle job`

---

## Task 3: Release workflow uses gui-build.vox

**Files:**
- Modify: `.github/workflows/release-gui.yml`

- [ ] **Step 1: Replace duplicate pnpm steps**

Before `tauri-apps/tauri-action`, add:

```yaml
      - name: Install vox CLI
        run: cargo install --locked --path crates/vox-cli --force
      - name: GUI build (VoxScript)
        run: vox run scripts/gui-build.vox
```

Remove redundant `pnpm install` / `pnpm build` if fully covered by script.

- [ ] **Step 2: Verify `projectPath: ./crates/vox-gui`** unchanged

- [ ] **Step 3: Tag dry-run** on test tag `v0.6.0-test.1` (draft release)

- [ ] **Step 4: Commit**

---

## Task 4: Playwright in CI

**Files:**
- Modify: `.github/workflows/ci.yml` (gui job section)

- [ ] **Step 1: Locate gui vitest job** — add env `VOX_GUI_PLAYWRIGHT: "1"`

- [ ] **Step 2: After vitest**

```yaml
      - name: Playwright e2e
        if: env.VOX_GUI_PLAYWRIGHT == '1'
        working-directory: crates/vox-gui/ui
        run: |
          pnpm exec playwright install chromium --with-deps
          pnpm exec playwright test e2e/
```

- [ ] **Step 3: Upload report on failure** (`playwright-report/`)

- [ ] **Step 4: Commit**

---

## Task 5: Bootstrap docs

**Files:**
- Modify: `docs/src/reference/gui-navigation.md`

- [ ] **Step 1: Add frontmatter** if file lacks required keys (run doc-pipeline lint)

- [ ] **Step 2: Add section** (see navigation plan Task 5)

- [ ] **Step 3:** `cargo run -q -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/gui-navigation.md`

- [ ] **Step 4: Commit**

---

## Exit criteria

- [ ] `cargo tauri build` succeeds locally on at least one OS
- [ ] CI bundle artifact uploaded from Linux job
- [ ] `release-gui.yml` calls `vox run scripts/gui-build.vox`
- [ ] Playwright runs on main CI gui job
- [ ] Bootstrap docs lint-clean
