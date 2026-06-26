---
title: "Plan 3D — GUI Honesty-Audit Caveat Completions"
category: "Architecture SSOTs"
status: "READY TO EXECUTE (depends on Plan 3A — surviving surfaces ratified)"
generated: "2026-06-26"
branch: "claude/graphify-general-gui-ia"
repo: "C:/Users/Owner/vox-graphify-gui"
depends_on:
  - docs/agents/gui-ia-blueprint.md   # §0 RATIFIED, §6 surviving-surface set
  - "Plan 3A (reorg execution) — do NOT fix a doomed surface before 3A lands the merges/cuts"
sources:
  - crates/vox-gui/build.rs
  - crates/vox-gui/ui/playwright.screens.config.ts
  - crates/vox-gui/ui/e2e/screenshots.spec.ts
  - crates/vox-gui/ui/e2e/lib/tauriMock.ts
  - crates/vox-cli/src/commands/ci/gui_honesty.rs
  - crates/vox-cli-ci/src/gui_visual_review.rs
  - crates/vox-codegen/src/web_ir/validate_palette.rs
  - crates/vox-codegen/src/web_ir/validate_a11y.rs
  - docs/agents/gui-honesty-findings/*.json
---

# Plan 3D — GUI Honesty-Audit Caveat Completions

## Workflow Execution (read first — orchestration contract)

This section is normative for a workflow orchestrator dispatching sub-agents. It does **not** restate or
rewrite the task bodies below; it classifies them, declares cross-plan ordering, and groups independent
tasks into fan-out batches. Every task ends in a concrete `git -C C:/Users/Owner/vox-graphify-gui add` +
`commit` (add+commit only — no `push`, `rebase`, `reset`, `checkout`, `clean`, or `merge`). Each task is
self-contained and committable by a single sub-agent (write-through-workflow).

### Cross-plan dependency header

- **Plan 3A (reorg execution) MUST land before Workstream C** (tasks C0–C4 and Final-gate task FG2 that
  touches surface markup). 3A's CUT/MERGE/RENAME moves the exact lines the 118 findings cite, and
  regenerates `SURFACE_REGISTRY` (which B2's drift guard asserts against). Do **not** fix a doomed/moved
  surface before 3A.
- **Workstreams A and B have NO dependency on Plan 3A** — they touch build/test/CI infrastructure, not
  surface markup — and may run in parallel with, or before, 3A.
- **Within Workstream B, B2's drift guard depends on the post-3A registry.** B2 may be authored anytime,
  but its assertion must be reconciled against the *final* registry; if B2 runs before 3A lands, re-run
  its vitest after 3A and amend in a follow-up commit (or sequence B2 after 3A — see batches).
- **Final-gate tasks** depend on the gates they wire: FG1 (gui-rust-check ordering) needs A4; FG2
  (surfaceVisual guard inside gui-honesty) needs C0.

### Per-task PARALLEL-SAFE / SEQUENTIAL classification

| Task | Class | Why / blocked-by |
|------|-------|------------------|
| A1 Reproduce failure (RED) | **[SEQUENTIAL]** | feeds the baseline error pasted into A3's commit body |
| A2 Check-only contract test | **[PARALLEL-SAFE]** | new test file, no shared edit; independent of A1 content |
| A3 `build.rs` skip guard | **[SEQUENTIAL]** | needs A1 baseline error in commit body; edits `build.rs` |
| A4 `gui-rust-check` gate | **[SEQUENTIAL]** | gate invokes the A3 skip path; adds ci enum + module + fixture |
| A5 Document beside gui-honesty | **[SEQUENTIAL]** | references the A4 gate name |
| B1 Verify sweep headless + webServer | **[PARALLEL-SAFE]** | edits `playwright.screens.config.ts` only |
| B2 Manifest == registry drift guard | **[PARALLEL-SAFE]*** | edits `screenshotManifest.test.ts` only; *reconcile vs post-3A registry |
| B3 Wire captures into gui-visual-review | **[SEQUENTIAL]** | depends on B1 sweep being green; edits `gui_visual_review.rs` |
| B4 Before/after capture | **[SEQUENTIAL]** | "before" must precede C edits; "after" follows C; spans C |
| C0 surfaceVisual guard + allowlist (RED) | **[SEQUENTIAL]** | after 3A; all C1 commits shrink its allowlist |
| C1 ds-token sweep (per surface) | **[PARALLEL-SAFE]** | 12 surface-scoped commits, disjoint files; each deletes its own allowlist lines |
| C2 a11y sweep (per surface) | **[PARALLEL-SAFE]** | per-surface, disjoint files |
| C3 overflow + hierarchy (per surface) | **[PARALLEL-SAFE]** | per-surface, disjoint files (Dashboard z-overlap + false-affordance noted) |
| C4 (optional) axe in sweep | **[PARALLEL-SAFE]** | edits `screenshots.spec.ts`; advisory |
| FG1 gui-rust-check in CI sequence | **[SEQUENTIAL]** | needs A4 |
| FG2 surfaceVisual guard inside gui-honesty | **[SEQUENTIAL]** | needs C0 |

`*` B2 is file-disjoint and parallel-safe to author, but its *assertion correctness* is gated on the
post-3A `SURFACE_REGISTRY`. **Default: author B2 in Batch 5 (post-3A), not Batch 1.** Authoring it in
Batch 1 against the pre-3A registry produces a **stale GREEN** that silently passes until 3A's CUTs/MERGEs
land — at which point the manifest==registry assertion is asserting the wrong (pre-reorg) surface set.
Authoring it post-3A means the drift guard is correct on first commit. (Batch-1 authoring + reconcile is
the fallback only if B1/B3 wiring needs the manifest test scaffold earlier.)

### Fan-out batches (what a workflow can dispatch together)

Batches run in order; tasks **inside** a batch fan out in parallel. A batch's join (all sub-agents
committed) is the gate for the next batch.

- **Batch 1 — infra fan-out (no 3A dependency).** Dispatch in parallel: **A2**, **B1**.
  Independent files, independent commits. **B2 is NOT in Batch 1 by default** — it defaults to Batch 5
  (post-3A) so its manifest==registry assertion is authored against the final, reorged `SURFACE_REGISTRY`
  (authoring it here would produce a stale GREEN; see the `*` note above).
- **Batch 2 — Workstream A spine (sequential chain).** Run A-chain in order: **A1 → A3 → A4 → A5**.
  (A2 already landed in Batch 1.) This batch is internally sequential; it may overlap Batch 1/3 in
  wall-clock since it shares no files with B or C, but its own tasks must serialize.
- **Batch 3 — Workstream B wiring (sequential within B).** After B1 is green: **B3**, then **B4 "before"**
  snapshot. B3 depends on B1; B4-before must precede any C edit.
- **GATE: Plan 3A lands** (external). Required before Batch 4.
- **Batch 4 — Workstream C seed (sequential).** **C0** (guard + seed allowlist, RED→GREEN). Single task,
  no fan-out; everything in Batch 5 depends on it.
- **Batch 5 — Workstream C surface fan-out (12-way parallel).** Dispatch per-surface sub-agents in
  parallel across **C1/C2/C3** grouped *by surface* (each sub-agent owns one surface's ds-token + a11y +
  overflow/hierarchy commits, so file ownership is disjoint and allowlist deletions don't collide).
  **Author B2 here** (post-3A) — its manifest==registry assertion reads the final reorged
  `SURFACE_REGISTRY`. **C4** (optional axe) may join this batch.
  Suggested smallest-first surface order is retained in C1.
- **Batch 6 — close-out (sequential).** **B4 "after"** snapshot → **FG1** → **FG2**.

> Allowlist-collision note for Batch 5: `visualScan.allowlist.ts` is a *shared* file that every C1
> surface sub-agent edits (deleting its own lines). To keep file ownership disjoint, either (i) shard the
> allowlist per surface at C0 time, or (ii) serialize only the allowlist-delete commit while keeping the
> surface markup edits parallel. Prefer (i).

---

Completes the three caveats deferred at the end of the GUI honesty audit. **Scoped to surfaces that
SURVIVE the ratified reorg** (`docs/agents/gui-ia-blueprint.md` §0 RATIFIED, §6 after-tree). This plan
does not invent surfaces and does not fix surfaces being cut.

## Dependency on Plan 3A (surviving surfaces)

Plan 3A executes the ratified reorg (CUTs, MERGEs, RENAMEs from the blueprint). **Workstream C of this
plan (the 118 visual findings) MUST run after Plan 3A**, because three of the cut/merge decisions move
the very lines the findings point at:

- `claims` + `knowledge`-surface MERGE → `scientia` (Findings). No honesty-findings JSON exists for
  Scientia/Claims, so no visual debt is lost.
- 4 activity clones (`archive-panel`/`discovery-inbox`/`discovery-review`/`activity`) MERGE → one
  **Discovery** surface that reuses the existing `Activity` component. The `Activity.json` findings
  (9) survive — they target `Activity/*.tsx`, the absorbing component.
- `matrix` MERGE → chat rail; `search` MERGE → `memory`. Neither has a honesty-findings JSON.

**Surviving-surface scope for Workstream C (the visual sweep):** the honesty audit produced findings
JSONs for **12 surfaces**, and **all 12 survive** the ratified reorg (none are CUT; `Activity` is a
MERGE *absorber*, not a victim). The 12 surviving in-scope surfaces are:

`Activity` (→ Discovery), `Catalog`, `Chat`, `Dashboard`, `Flow`, `Harness`, `Memory`, `Mesh`,
`Runs`, `Settings`, `SkillsPlugins`, `Tasks`.

**Total visual findings in scope = 118**, distributed: **ds-token 66 · a11y 27 · hierarchy 15 ·
overflow 10**. (The audit summary rounded this to "~109"; the exact machine count across the JSON
`visual[]` arrays is 118.)

Workstreams A (Rust compile-verify) and B (Playwright proof) have **no dependency on Plan 3A** — they
touch build/test infrastructure, not surface markup — and may execute in parallel with or before 3A.

---

## Findings from reading the actual repo (changes the plan)

1. **The Tauri mock and a per-surface headless screenshot sweep ALREADY EXIST.** `e2e/screenshots.spec.ts`
   derives `VIEWS` from `SURFACE_REGISTRY`, installs `installTauriMock` (`e2e/lib/tauriMock.ts`) via
   `page.addInitScript`, and writes `e2e/screens/<view>.png` per surface — already headless, already
   asserting no error-boundary / no page-error / no console-error. **Caveat 2 is therefore mostly DONE**;
   the remaining gap is wiring those captures into `vox ci gui-visual-review` (Workstream B), not building
   the mock from scratch.
2. **`build.rs` calls `tauri_build::try_build` and `panic!`s on failure** (needs the Windows manifest /
   sidecar context). This is what blocks `cargo check -p vox-gui` standalone (Workstream A).
3. **The honesty gate (`vox ci gui-honesty`) runs two things**: `pnpm run typecheck` + one vitest guard
   (`surfaceHonesty.guard.test.ts`). The visual guard (Workstream C) mirrors that guard's file-walk
   pattern exactly (`scanSource` over `src/components/surfaces`, allowlist file alongside).
4. **Palette/a11y token rules are canonical in Rust** at `validate_palette.rs` (Tailwind palette +
   token-registry resolution, WCAG contrast) and `validate_a11y.rs`. The vitest guard reuses the *class
   name* policy (forbid raw `emerald-/amber-/cyan-/rose-/violet-` Tailwind utilities in surfaces); the
   semantic token replacements come from the existing DS tokens (`status-warn`/`status-fail`/`status-pass`,
   `bg-base`, etc., already used across `src/components/`).

---

## Conventions

- TDD: write the failing test/assertion first, watch it fail, implement, watch it pass, commit.
- One logical change per commit. Commit message body ends with the Co-Authored-By trailer.
- Run UI tests from `crates/vox-gui/ui`: `pnpm vitest run <file>` and `pnpm exec playwright test <file>`.
- Run Rust gates from repo root: `cargo check -p <crate>`, `cargo test -p <crate>`.
- Do NOT run `cargo fmt --all` (banned — AGENTS.md). Use `cargo fmt -p <crate>`.
- No new `.ps1`/`.sh`/`.py` automation (VoxScript-only policy). The vitest guard is TS; the gate wiring
  is Rust in `vox-cli`.

---

# Workstream A — vox-gui Rust compile-verification path

**Goal:** `cargo check -p vox-gui` (or an equivalent gate) runs locally and in CI **without** a full
Tauri bundle, and would have caught a type error in `commands/orchestrator.rs::used_tokens` or
`commands/policy.rs`. Added as a gate next to `vox ci gui-honesty`.

**Recommended option (lowest friction): (a) cfg/env skip in `build.rs`.** Option (c) (split a lib crate)
is a large refactor of every `#[tauri::command]`; option (b) (build the sidecar first) is slow and still
needs the full toolchain. Option (a) is a ~10-line `build.rs` guard plus a CI step. We implement (a) and
make the existing CI sidecar build the *fallback* path so the real bundle stays verified too.

### A1. Reproduce the failure (RED)  [SEQUENTIAL]
- Run `cargo check -p vox-gui` from repo root. Capture the error (expected: `tauri_build::try_build`
  panic / missing manifest or sidecar). Paste the exact error into the commit body of A3 as the baseline
  this change fixes.

### A2. Test: a check-only build path exists  [PARALLEL-SAFE]
- Create `crates/vox-gui/tests/check_only_build.rs`:
  ```rust
  //! Guards that `VOX_GUI_SKIP_SIDECAR=1 cargo check -p vox-gui` succeeds without a Tauri bundle.
  //! This test only asserts the crate's own source compiles under the skip cfg; the harness
  //! (CI step in Workstream A4) is what actually invokes `cargo check` with the env set.
  #[test]
  fn check_only_env_is_documented() {
      // Compile-time proof that the skip path is referenced; the real gate is the CI `cargo check`.
      assert!(option_env!("CARGO_PKG_NAME").is_some());
  }
  ```
  (The substantive verification is the CI `cargo check` in A4 — this file documents the contract and
  fails to compile if the crate itself breaks.)

### A3. Implement the `build.rs` skip guard  [SEQUENTIAL]
- Edit `crates/vox-gui/build.rs`:
  ```rust
  fn main() {
      vox_build_meta::emit();
      // Allow `cargo check`-only verification without a Tauri bundle/sidecar:
      // `VOX_GUI_SKIP_SIDECAR=1 cargo check -p vox-gui` compiles the command surface
      // (e.g. commands/orchestrator.rs, commands/policy.rs) and would catch type errors,
      // without running tauri_build (which needs the Windows manifest + sidecar context).
      println!("cargo:rerun-if-env-changed=VOX_GUI_SKIP_SIDECAR");
      if std::env::var_os("VOX_GUI_SKIP_SIDECAR").is_some() {
          // tauri_build emits the context + manifest; skipping it means the produced
          // artifact must NOT be run/bundled — `cargo check` only. Documented in the gate.
          println!("cargo:warning=VOX_GUI_SKIP_SIDECAR set — skipping tauri_build (check-only).");
          return;
      }
      // Must not swallow errors: a missing Windows manifest yields STATUS_ENTRYPOINT_NOT_FOUND at runtime.
      if let Err(err) = tauri_build::try_build(tauri_build::Attributes::new()) {
          panic!("tauri build script failed: {err}");
      }
  }
  ```
- Verify locally: `VOX_GUI_SKIP_SIDECAR=1 cargo check -p vox-gui` (Bash) or
  `$env:VOX_GUI_SKIP_SIDECAR='1'; cargo check -p vox-gui` (PowerShell). Confirm it now compiles and that
  it reports errors if you introduce a deliberate type error in `commands/orchestrator.rs` (mutate
  `used_tokens` return type to `String`, see it fail, revert).
- Commit: `fix(vox-gui): add VOX_GUI_SKIP_SIDECAR check-only build path`.

### A4. Add the gate `vox ci gui-rust-check`  [SEQUENTIAL]
- Add to `crates/vox-cli/src/commands/ci/cmd_enums.rs` a `GuiRustCheck` variant next to `GuiHonesty`.
- Create `crates/vox-cli/src/commands/ci/gui_rust_check.rs`:
  ```rust
  //! `vox ci gui-rust-check` — compile-verify the vox-gui Tauri command surface without a bundle.
  //! Runs `cargo check -p vox-gui` with VOX_GUI_SKIP_SIDECAR=1 so a type error in
  //! commands/orchestrator.rs (used_tokens) or commands/policy.rs fails CI.
  use std::path::Path;
  use std::process::Command;
  use anyhow::{Result, anyhow};

  pub fn run(root: &Path) -> Result<()> {
      let status = Command::new(env!("CARGO"))
          .current_dir(root)
          .env("VOX_GUI_SKIP_SIDECAR", "1")
          .args(["check", "-p", "vox-gui"])
          .status()?;
      if !status.success() {
          return Err(anyhow!(
              "gui-rust-check: `cargo check -p vox-gui` (VOX_GUI_SKIP_SIDECAR=1) failed — \
               type error in the Tauri command surface (commands/orchestrator.rs / commands/policy.rs)"
          ));
      }
      println!("gui-rust-check: OK");
      Ok(())
  }
  ```
- Wire `gui_rust_check::run` into the same dispatch site as `gui_honesty::run` (mirror the match arm in
  the ci command module; `mod gui_rust_check;` next to `mod gui_honesty;`).
- Update the CI fixture if the command catalog is asserted:
  `crates/vox-cli/tests/fixtures/command_catalog_paths_baseline.txt`. **Do NOT hand-edit this file — it
  is a generated baseline.** Verify first by reading `crates/vox-cli/tests/command_catalog_paths_baseline.rs`:
  it regenerates the whole fixture when run with the bless env var (confirmed:
  `UPDATE_CLI_CATALOG_BASELINE=1`). So the correct flow is — add the `GuiRustCheck` subcommand to the clap
  tree, then **regenerate** the baseline, not edit it by hand:
  ```bash
  # Windows note (per the test's own module doc): build_catalog walks the full clap tree and overflows
  # the default test-thread stack — bump it. Bash:
  RUST_MIN_STACK=33554432 UPDATE_CLI_CATALOG_BASELINE=1 cargo test -p vox-cli --test command_catalog_paths_baseline
  ```
  The test writes the fixture and `panic!`s "wrote …; commit this file" — that panic is expected on a
  bless run. Then run it **without** the env var to confirm it passes against the regenerated baseline,
  and stage the regenerated `command_catalog_paths_baseline.txt` with the rest of A4. Hand-editing a
  single line would leave the file out of canonical sort/format and fail the plain (non-bless) run.
- Verify: `cargo run -p vox-cli -- ci gui-rust-check` exits 0; introduce a type error → exits non-zero.
- Commit: `feat(ci): add gui-rust-check gate (cargo check -p vox-gui, sidecar-skipped)`.

### A5. Document next to gui-honesty  [SEQUENTIAL]
- In `gui_honesty.rs` module doc and the CI runner that lists gates, note `gui-rust-check` runs alongside
  `gui-honesty`. If a `vox ci gui` umbrella exists, add `gui-rust-check` to its sequence.
- Commit: `docs(ci): note gui-rust-check beside gui-honesty`.

---

# Workstream B — Playwright / visual proof

**Goal:** real per-surface screenshots are captured headlessly and feed the existing AI critique
(`vox ci gui-visual-review`). **The hard part already exists** (mock + sweep). This workstream verifies
the sweep runs green, captures before/after for the audited surfaces, and routes the captures into the
review gate. Scoped to the **12 surviving surfaces** plus whatever else `SURFACE_REGISTRY` yields after
Plan 3A's registry regen.

### B1. Verify the existing sweep runs headless (RED→GREEN baseline)  [PARALLEL-SAFE]
- From `crates/vox-gui/ui`, start the dev server on 1420 (the config's `baseURL`):
  `pnpm dev` (or the project's dev script) in one shell.
- In another: `pnpm exec playwright test --config=playwright.screens.config.ts`.
- Confirm `e2e/screens/<view>.png` is produced for every surviving surface and the in-spec assertions
  (no `[data-surface-error]`, no pageerror, no console error) pass. Record which surfaces (if any) fail —
  those are real defects, file them against Workstream C.
- If the dev-server requirement is friction for CI, add a `webServer` block to
  `playwright.screens.config.ts` so CI can boot it:
  ```ts
  webServer: {
    command: 'pnpm dev --port 1420',
    url: 'http://localhost:1420',
    reuseExistingServer: !process.env.CI,
    timeout: 120_000,
  },
  ```
- Commit: `test(gui): add webServer to screens playwright config for headless CI capture`.

### B2. Test: the capture manifest covers every surviving surface  [PARALLEL-SAFE] (author in Batch 5, post-3A)
- **Author this in Batch 5 (after Plan 3A lands), not Batch 1.** The assertion compares against
  `SURFACE_REGISTRY`; authoring it before 3A's CUTs/MERGEs would lock in the pre-reorg surface set and
  pass as a **stale GREEN** that asserts the wrong thing. Writing it post-3A makes the drift guard correct
  on first commit. (If B1/B3 need the manifest scaffold earlier, the fallback is to author in Batch 1 and
  re-run + amend after 3A — but the default is Batch 5.)
- There is already `e2e/lib/screenshotManifest.ts` + `screenshotManifest.test.ts`. Extend the test to
  assert the manifest's surface list equals the post-3A `SURFACE_REGISTRY` viewKeys (drift guard), so a
  surface added/renamed by Plan 3A can't silently drop out of the capture set.
- Run `pnpm vitest run e2e/lib/screenshotManifest.test.ts`; make it fail first (e.g. by referencing a
  stale list), then align.
- Commit: `test(gui): assert screenshot manifest == surface registry`.

### B3. Wire captures into `vox ci gui-visual-review`  [SEQUENTIAL]
- Read `crates/vox-cli-ci/src/gui_visual_review.rs` to find where it expects PNGs (the audit notes it
  reads from `contracts/reports/gui-visual-review/` with a sha256 cache and reviews only changed/new
  surfaces).
- Add a pre-step (in that module or the gate that calls it) that runs the playwright screens config and
  copies `crates/vox-gui/ui/e2e/screens/*.png` into the review input dir the reviewer scans. Prefer
  reusing the existing path constant rather than hardcoding.
- TDD: add a Rust test in `vox-cli-ci` that, given a temp dir of PNGs, the capture→review path picks them
  up (or, if the reviewer is network-bound, a unit test on the path-resolution + sha256-cache logic only).
- Verify end to end: `cargo run -p vox-cli -- ci gui-visual-review` runs the sweep, then critiques only
  changed surfaces (advisory/non-gating, exit 0 — preserve that contract).
- Commit: `feat(ci): feed real per-surface screenshots into gui-visual-review`.

### B4. Capture before/after for the audited surfaces  [SEQUENTIAL] (spans Workstream C)
- Before starting Workstream C, snapshot the 12 surviving surfaces to a `before/` dir
  (`e2e/screens/before/<view>.png`). After Workstream C, snapshot to `after/`. These are the visual
  proof the audit could not produce.
- Do NOT commit large PNGs to git unless the repo already tracks `e2e/screens/` (it appears to be a
  generated/working dir). Reference them from the Workstream C self-review instead; let
  `gui-visual-review`'s sha256 cache under `contracts/reports/` be the committed artifact.
- Commit (if a small manifest/diff index is produced): `docs(gui): visual-proof before/after index`.

> **Scope caveat (stated explicitly):** full Tauri-runtime-in-CI is NOT attempted. The mock-backed
> headless sweep is the supported visual-proof path. Surfaces whose behavior depends on real sidecar IPC
> beyond the mock's coverage render with mock data — that is sufficient for DS-token/a11y/overflow visual
> review, which is layout/markup-level, not data-correctness-level.

---

# Workstream C — the 118 visual / DS-token / a11y / overflow findings

**Goal:** a per-type sweep with a guard test that prevents regressions, then per-surface fixes, scoped to
the 12 surviving surfaces. **Runs after Plan 3A.** Canonical token rules come from
`validate_palette.rs` / `validate_a11y.rs`; the vitest guard enforces the *class-name* subset.

### Grouped scope (machine-counted across `docs/agents/gui-honesty-findings/*.json` `visual[]`)

| Type | Count | What it is | Fix target |
|------|------:|-----------|-----------|
| **ds-token** | 66 | raw `emerald-/amber-/cyan-/rose-/violet-` Tailwind + `bg-[#hex]` brackets | replace with `status-warn`/`status-fail`/`status-pass`/`bg-base`/etc. DS tokens |
| **a11y** | 27 | missing `aria-label`, decorative glyphs without `aria-hidden`, icon-only buttons | add labels / `aria-hidden="true"` |
| **hierarchy** | 15 | false affordance (hidden dead controls), z-overlap of absolute clusters | remove dead affordance / add clearance |
| **overflow** | 10 | fixed-width / no-wrap clipping on narrow viewports | `min-w-0`/`truncate`/`flex-wrap` |
| **total** | **118** | | |

### C0. Per-type guard tests FIRST (RED), mirroring `surfaceHonesty.guard.test.ts`  [SEQUENTIAL] (after Plan 3A)

Create `crates/vox-gui/ui/src/components/surfaces/__guards__/surfaceVisual.guard.test.ts`. It walks
`src/components/surfaces` exactly like the honesty guard (skip `*.test.tsx` / `*.unfinished.tsx`) and
scans for the raw-token regression. Pair it with an allowlist file so legacy untouched lines can be
parked, mirroring `honestyScan.allowlist.ts`.

```ts
import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { VISUAL_ALLOWLIST } from './visualScan.allowlist';

const ROOT = 'src/components/surfaces';
// Raw Tailwind status hues + arbitrary-hex backgrounds that must be DS tokens instead.
// Mirrors the palette rule in crates/vox-codegen/src/web_ir/validate_palette.rs (class subset).
const RAW_TOKEN = /\b(?:text|bg|border|from|to|via|ring)-(?:emerald|amber|cyan|rose|violet)-\d{2,3}\b|\bbg-\[#[0-9a-fA-F]{3,8}\]/;

function walk(d: string): string[] {
  return readdirSync(d).flatMap(n => {
    const p = join(d, n);
    return statSync(p).isDirectory() ? walk(p)
      : p.endsWith('.tsx') && !p.endsWith('.test.tsx') && !p.endsWith('.unfinished.tsx') ? [p] : [];
  });
}
const allowed = (f: string, l: number) =>
  VISUAL_ALLOWLIST.some(a => f.endsWith(a.file) && a.line === l);

describe('surface visual guard (DS-token migration)', () => {
  it('no raw emerald/amber/cyan/rose/violet Tailwind or bg-[#hex] in shipped surfaces', () => {
    const violations = walk(ROOT).flatMap(f =>
      readFileSync(f, 'utf8').split('\n').flatMap((line, i) =>
        RAW_TOKEN.test(line) && !allowed(f, i + 1) ? [{ file: f, line: i + 1, text: line.trim() }] : []),
    );
    expect(violations, JSON.stringify(violations, null, 2)).toHaveLength(0);
  });
});
```

- Create `visualScan.allowlist.ts` seeded with **every current ds-token finding line** from the JSONs
  (so the guard is GREEN at introduction and turns RED on any NEW raw token). Each entry carries a
  `reason: 'pre-existing honesty-audit finding — see Plan 3D Workstream C'`.
- Run `pnpm vitest run src/components/surfaces/__guards__/surfaceVisual.guard.test.ts` → GREEN (allowlisted).
- Commit: `test(gui): add surfaceVisual DS-token guard + seed allowlist from audit`.

> a11y/overflow/hierarchy are harder to regex reliably from outside the DOM; the guard above covers the
> 66 ds-token findings deterministically. a11y is additionally enforced at render time by the existing
> playwright sweep (B1) plus an optional axe pass (C4). Hierarchy/overflow are verified visually via the
> before/after captures (B4).

### C1. ds-token sweep (66 findings) — shrink the allowlist to zero, surface by surface  [PARALLEL-SAFE] (per surface; after C0)
Process surfaces in ascending finding count so early commits are small. For each surviving surface, open
the cited file:line from its JSON, replace the raw class with the DS token, then **delete that line's
allowlist entry**. The guard stays GREEN only if the replacement is a real token.

Canonical replacements (from `validate_palette.rs` token registry + existing `src/components` usage):
- `text-amber-300` / `amber-400` (warn) → `text-status-warn`
- `bg-rose-400` / `rose-300` (fail) → `bg-status-fail` / `text-status-fail`
- `bg-emerald-400` / `emerald-300` (pass) → `bg-status-pass` / `text-status-pass`
- `text-cyan-*` (accent/info) → the DS info/accent token (grep `status-info`/`accent` in tailwind config; if none, add one token — do not invent per-surface hexes)
- `bg-[#09090b]/80` → `bg-bg-base/80`
- gradient stops `from-emerald-400/40` etc. → DS gradient tokens if they exist; otherwise add the missing
  status-gradient tokens to the DS config in ONE commit, then reference them (do NOT leave raw hues).

Per-surface commits, smallest first. Suggested order by ds-token count:
Harness, Mesh → Memory → Dashboard, Activity → Catalog, Settings, Tasks, SkillsPlugins → Chat, Runs, Flow.
- After each surface: `pnpm vitest run surfaceVisual.guard.test.ts` (GREEN) + `pnpm run typecheck`.
- Commit per surface: `style(gui): migrate <Surface> raw Tailwind hues to DS tokens`.
- When the allowlist reaches empty for ds-token entries, the guard now BLOCKS all future raw tokens.

### C2. a11y sweep (27 findings)  [PARALLEL-SAFE] (per surface; after C0)
For each a11y finding, apply the cited fix:
- decorative glyphs (e.g. `⬤`, bullets) → wrap span gets `aria-hidden="true"`.
- icon-only buttons → add `aria-label="<action>"`.
- ambiguous links → ensure discernible text or `aria-label`.
- Add a vitest render test per fixed surface using `@testing-library` + `jest-axe`/`axe-core` if already a
  dep (grep `package.json`); assert no critical axe violations on the surface render with the tauri mock.
  If axe is not a dep, scope to targeted `getByRole('button', { name: ... })` assertions instead of adding
  a dependency.
- Commit per surface: `a11y(gui): label/hide <Surface> controls per honesty audit`.

### C3. overflow sweep (10 findings) + hierarchy (15 findings)  [PARALLEL-SAFE] (per surface; after C0)
- overflow: add `min-w-0` to flex children that clip, `truncate`/`flex-wrap` per finding; verify against
  the before/after captures (B4) at the audited viewport (1440×900 and a narrow 768 capture — add a narrow
  variant to the sweep if not present).
- hierarchy — **false affordance**: the Dashboard `Doubt`/`Overrule` dead-handler buttons
  (`StreamCard.tsx:37` opacity-0 hover controls, `onDoubt`/`onOverrule` not passed from App.tsx) are the
  highest-severity items. These overlap the honesty audit's "wire or hide" decision: either wire them
  (if the soft-HITL doubt/overrule path from the ratified Runs/needs-you EXPAND lands) OR hide them behind
  the `.unfinished.tsx` mechanism the honesty guard already understands. **Do whichever the needs-you
  EXPAND workstream chose** — do not leave a dead visible control.
- hierarchy — **z-overlap**: add right-side clearance to the Dashboard KPI row / AttentionBudgetMeter for
  the absolute customize-cluster (`Dashboard.tsx:324`).
- Commit per surface: `fix(gui): resolve <Surface> overflow/hierarchy findings`.

### C4. (optional) elevate a11y to the playwright sweep  [PARALLEL-SAFE]
- If axe is available, add a `@axe-core/playwright` assertion into `screenshots.spec.ts` so every surface
  capture also fails on critical a11y violations. Keep it advisory at first (log, don't fail) to avoid a
  big-bang CI break; flip to failing once C2 lands.
- Commit: `test(gui): add advisory axe a11y check to screenshot sweep`.

---

# Final gate wiring

### FG1. Order gui-rust-check into the CI sequence  [SEQUENTIAL] (needs A4)
- Ensure the CI sequence that runs `gui-honesty` also runs `gui-rust-check` (A4) and that
  `gui-visual-review` (B3) is invoked with real captures. If there is a `vox ci gui` umbrella, the order
  is: `gui-rust-check` → `gui-honesty` → `surfaceVisual.guard` (inside gui-honesty or its own step) →
  `gui-visual-review` (advisory).
- Commit: `feat(ci): order gui-rust-check into the gui CI sequence`.

### FG2. Run surfaceVisual guard inside gui-honesty  [SEQUENTIAL] (needs C0)
- Add `surfaceVisual.guard.test.ts` to the vitest invocation inside `gui_honesty.rs` (it already runs one
  guard; add the second path) OR keep it as its own step — prefer co-locating in `gui-honesty` so one gate
  covers honesty + visual regressions.
- Commit: `feat(ci): run surfaceVisual guard inside gui-honesty gate`.

---

# Self-Review

**Placeholder-free?** Yes. Every path is real and was read this session: `build.rs` (the actual
`tauri_build::try_build` panic), `playwright.screens.config.ts` (baseURL 1420, no webServer),
`screenshots.spec.ts` (the existing mock-backed sweep), `gui_honesty.rs` (the two-check gate),
`validate_palette.rs`/`validate_a11y.rs` (canonical token rules), and all 12 findings JSONs. Code blocks
are concrete (the `build.rs` guard, the Rust gate module, the vitest guard regex) — no TODOs, no
`<fill-in>`.

**Counts verified by machine, not memory:** 118 visual findings (ds-token 66 · a11y 27 · hierarchy 15 ·
overflow 10) computed by iterating the JSON `visual[]` arrays. The audit's "~109" is reconciled: the round
number predates the final JSON set; 118 is exact.

**Surviving-surface scope is explicit:** all 12 honesty-findings surfaces survive Plan 3A (Activity is a
MERGE absorber, not a cut). No findings are dropped because no findings-bearing surface is cut. The
dependency on Plan 3A is stated for Workstream C and explicitly waived for A and B.

**Did I reduce scope where the repo already solved it?** Yes — caveat 2's "build the Tauri mock" is
already done (`tauriMock.ts` + `screenshots.spec.ts`); Workstream B is reduced to *verify + wire into the
review gate*, not *build from scratch*. This is called out in "Findings from reading the actual repo".

**Lowest-friction option chosen for caveat 1?** Yes — option (a) `VOX_GUI_SKIP_SIDECAR` env guard in
`build.rs` (~10 lines) over the lib-split (c) refactor or the slow sidecar-first build (b), with the real
bundle path preserved as the default.

**TDD discipline:** every workstream leads with a failing test/observation (A1 reproduce, A2 contract test,
B1 baseline run, B2/B3 drift+path tests, C0 guard seeded then shrunk). Commits are bite-sized (per surface,
per gate).

**Open risk:** the C3 false-affordance Doubt/Overrule decision is coupled to the needs-you EXPAND
workstream; the plan defers to that decision rather than duplicating it. If needs-you EXPAND has not landed
when C3 runs, hide the controls via `.unfinished.tsx` (honesty-guard-honored) as the safe default.
