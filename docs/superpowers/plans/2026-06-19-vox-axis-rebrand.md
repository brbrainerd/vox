# Vox Axis / "Axis" Rebrand — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> 🤖 **EXECUTION TARGET — READ FIRST.** Phase B of this plan is written for **Gemini
> Flash 3.5 in Antigravity**. Flash has ~48% unaided in-IDE completion, no mid-task
> checkpoint, and a hard quota cutoff. See
> `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.
> **Phase A (brand assets) is NOT for Flash** — it is generated in Claude Code before
> the handoff and committed; you only reference/verify the committed files.

**Goal:** Rebrand the Vox GUI to **"Axis"** (full brand "Vox Axis") at the identity layer only — window title, in-app mark, footer, a `vox axis` launch alias, and a regenerated icon set — with zero crate/binary/identifier renames.

**Architecture:** Pure brand-layer change across three lanes: (1) a JSON config value, (2) React/TS UI strings + one clap subcommand alias, (3) binary image assets generated ahead of time. No new Rust types, no API changes, no import churn. `productName`/`identifier` in `tauri.conf.json` stay "Vox"/`org.vox-foundation.gui` on purpose (they drive installer/bundle identity).

**Tech Stack:** Tauri 2 (`tauri.conf.json`), TypeScript/React + vitest (`vox-gui/ui`), Rust + clap (`vox-cli`), `tauri icon` for asset fan-out.

**Spec:** `docs/superpowers/specs/2026-06-19-vox-axis-rebrand-design.md`

---

**Operating Rules (apply to EVERY Phase-B task):**
1. Each task is **atomic + green + committed**: tests pass before you commit; never leave a broken tree. A kill between tasks must leave a compiling, tested checkout.
2. **Verify before use.** Every Step-1 `rg`/read is a BLOCKING gate — run it, paste the output, and if reality differs from the plan, **STOP and report** rather than guessing or "fixing" the design.
3. **Two-strike circuit breaker.** If a step fails twice, stop and report; do not thrash.
4. **Split on overrun.** If an implement step would touch >1 file or add >1 new function, make one atomic green commit per sub-bullet.
5. **House rules:** the `ui` package is pnpm-managed; `npx vitest run <path>` and `npx tsc --noEmit` work, run from `crates/vox-gui/ui`. Add `// @vitest-environment jsdom` as the FIRST line of every new component test. Never run `cargo fmt --all` (Windows arg-limit) — use `cargo fmt -p <crate>`. For `vox-gui` Rust, lint **lib-only**: `cargo clippy -p vox-gui --lib -- -D warnings` (`--all-targets` breaks on the Tauri build script). No stubs.
6. **Tags:** `[PARALLEL-SAFE]` tasks touch disjoint files and may run in parallel subagents; `[SEQUENTIAL]` tasks must not share a file with a concurrent subagent.
7. **Naming invariant:** display = "Vox Axis" / "Axis"; identifiers = `axis`/`VoxAxis`. Do **not** rename the `vox` binary, the `vox-gui` crate, `vox-gui.exe`, `productName`, or `identifier`.

---

## Flash Execution Addendum (2026-06-19)

**Global gates (facts this plan relies on — confirm, do not assume):**
- The GUI config is `crates/vox-gui/tauri.conf.json`; `productName` is `"Vox"`, window `title` is `"Vox"`. You change **only** `title`.
- The sidebar brand mark is in `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`: a `V` glyph (~line 175) and a `VOX` wordmark (~line 178); the footer build line is ~line 310.
- The CLI root parser is `VoxCliRoot` (`#[derive(Parser)]`) wrapping `pub enum Cli` (`#[derive(Subcommand)]`); the `Gui` variant is `#[cfg(feature = "gui")]` (`crates/vox-cli/src/lib.rs:431-435`). The `vox axis` alias is added there.
- The committed Axis icon set already exists under `crates/vox-gui/icons/` (produced by Phase A). You do **not** generate images.

**Mandatory pre-flight (run from repo root, paste output, confirm before any Phase-B code):**
```
rg -n "\"productName\"|\"title\"|\"identifier\"" crates/vox-gui/tauri.conf.json
rg -n ">V<|>VOX<|build \{appVersion" crates/vox-gui/ui/src/components/layout/Sidebar.tsx
rg -n "appVersion" crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
rg -n "pub enum Cli|Gui \{|cfg\(feature = \"gui\"\)|pub struct VoxCliRoot" crates/vox-cli/src/lib.rs
git -C . status --porcelain crates/vox-gui/icons | head
```
Expected: `productName`/`identifier` present and unchanged; a `V` glyph + `VOX` wordmark + a `build {appVersion}` footer in Sidebar; `VoxCliRoot` struct + `Cli` enum + a `#[cfg(feature = "gui")] Gui` variant; the Axis icons committed. If `crates/vox-gui/icons` is NOT already rebranded, **STOP** — Phase A was skipped.

**Task-split table:**

| Task | Touches | Tag |
|---|---|---|
| B1 — window title → "Axis" | `tauri.conf.json` + new vitest `branding.test.ts` | [PARALLEL-SAFE] |
| B2 — sidebar mark `V`/`VOX` → `A`/`AXIS` | `Sidebar.tsx` (+ test) | [SEQUENTIAL] (shares Sidebar.tsx with B3) |
| B3 — footer "Vox Axis ·" brand line | `Sidebar.tsx` (+ test) | [SEQUENTIAL] (shares Sidebar.tsx with B2) |
| B4 — `vox axis` subcommand alias | `vox-cli/src/lib.rs` (+ test) | [PARALLEL-SAFE] |
| B5 — brand phrasing in help/log | `vox-cli/src/lib.rs`, `commands/gui.rs` | [SEQUENTIAL] (shares lib.rs with B4) |

Run order: B1 ∥ B4 first; then B2 → B3 (same file, sequential); then B5 after B4.

---

# PHASE A — Brand assets 🧑‍🎨 (CLAUDE-CODE PRE-FLIGHT — do NOT hand to Flash)

> ✅ **DONE (2026-06-19, this session).** Executed in Claude Code with ImageMagick +
> `cargo tauri icon`. The mark is the **coordinate-axis frame** (x/y/z arrows from a
> shared origin — the conventional "axis" symbol), not a letterform. Commits:
> `098edc3b9b` (svg) → `dc04893760` (png) → `d309473ef9` (initial set) →
> `8c29861f5e` (axis-frame redesign + regenerate). The icons under
> `crates/vox-gui/icons/` are committed; **do not regenerate** unless redesigning.
> The task steps below are retained as the reproduction recipe.

> These tasks run in **Claude Code** (this harness) before the Antigravity handoff,
> because Gemini Flash cannot reliably author binary image assets. Claude may adapt
> the exact rendering command to whatever is installed (resvg / ImageMagick / sharp);
> the deliverable is a committed, regenerated icon set. Commit at the end of Phase A.

### Task A1 — Author the Axis SVG source

**Files:**
- Create: `crates/vox-gui/icons/source/axis.svg`

- [ ] **Step 1:** Author a 1024×1024 square SVG of the Axis mark per the spec's visual
  direction (§4): a geometric "A" that doubles as crossed x/y axes, in the GUI palette
  (brass `#b08d57` → amber → zinc `#18181b`, matching the existing glyph gradient
  `from-brass via-amber-600 to-zinc-900`). Must remain legible at 32px (thick strokes,
  high contrast, transparent background).
- [ ] **Step 2:** Commit.
  ```bash
  git add crates/vox-gui/icons/source/axis.svg
  git commit -m "feat(axis): brand mark SVG source"
  ```

### Task A2 — Render the 1024px PNG source

**Files:**
- Create: `crates/vox-gui/icons/source/axis-1024.png`

- [ ] **Step 1:** Render `axis.svg` → `axis-1024.png` at 1024×1024 with transparency.
  Primary command (adapt if the tool is absent):
  ```bash
  npx -y sharp-cli -i crates/vox-gui/icons/source/axis.svg \
    -o crates/vox-gui/icons/source/axis-1024.png resize 1024 1024
  ```
  Fallback (ImageMagick): `magick -background none -density 512 crates/vox-gui/icons/source/axis.svg -resize 1024x1024 crates/vox-gui/icons/source/axis-1024.png`
- [ ] **Step 2:** Verify the PNG is 1024×1024 and non-empty (open it / check file size > 0).
- [ ] **Step 3:** Commit.
  ```bash
  git add crates/vox-gui/icons/source/axis-1024.png
  git commit -m "feat(axis): 1024px PNG icon source"
  ```

### Task A3 — Fan out the full Tauri icon set

**Files:**
- Modify (regenerate): `crates/vox-gui/icons/{icon.ico,icon.icns,icon.png,32x32.png,64x64.png,128x128.png,128x128@2x.png,Square*Logo.png,StoreLogo.png,android/*,ios/*}`

- [ ] **Step 1:** Regenerate the icon set from the 1024px source:
  ```bash
  cd crates/vox-gui && npx -y @tauri-apps/cli icon icons/source/axis-1024.png -o icons
  ```
  (Fallback if `@tauri-apps/cli` is unavailable: `cargo tauri icon icons/source/axis-1024.png -o icons`.)
- [ ] **Step 2:** Verify the set regenerated: `git status --porcelain crates/vox-gui/icons` shows the `.ico`/`.icns`/`.png`/`Square*Logo` files modified, and each is non-empty.
- [ ] **Step 3:** Commit.
  ```bash
  git add crates/vox-gui/icons
  git commit -m "feat(axis): regenerate app icon set with Axis brand mark"
  ```

**Phase A done →** the icons are committed; the Antigravity handoff can proceed.

---

# PHASE B — Branding wiring (Gemini Flash 3.5 / Antigravity)

### Task B1 — Window title → "Axis" [PARALLEL-SAFE]

**Files:**
- Modify: `crates/vox-gui/tauri.conf.json:14`
- Test: `crates/vox-gui/ui/src/__tests__/branding.test.ts`

- [ ] **Step 1 (gate):** `rg -n "\"productName\"|\"title\"|\"identifier\"" crates/vox-gui/tauri.conf.json` — paste output. Confirm `productName` is `"Vox"`, `identifier` is `"org.vox-foundation.gui"`, window `title` is `"Vox"`. You will change ONLY `title`.

- [ ] **Step 2: Write the failing test**

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

// Resolve relative to THIS test file (robust regardless of vitest cwd):
// src/__tests__ -> src -> ui -> vox-gui/tauri.conf.json
const here = dirname(fileURLToPath(import.meta.url));
const conf = JSON.parse(
  readFileSync(resolve(here, '../../../tauri.conf.json'), 'utf8'),
);

describe('Axis branding — tauri config', () => {
  it('window title is "Axis"', () => {
    expect(conf.app.windows[0].title).toBe('Axis');
  });
  it('productName and identifier are unchanged (brand-layer only)', () => {
    expect(conf.productName).toBe('Vox');
    expect(conf.identifier).toBe('org.vox-foundation.gui');
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run (from `crates/vox-gui/ui`): `npx vitest run src/__tests__/branding.test.ts`
Expected: FAIL — `expected 'Vox' to be 'Axis'`.

- [ ] **Step 4: Make the change**

In `crates/vox-gui/tauri.conf.json`, change the window title only:
```json
        "title": "Axis",
```
(Leave `"productName": "Vox"` and `"identifier": "org.vox-foundation.gui"` exactly as they are.)

- [ ] **Step 5: Run test to verify it passes**

Run: `npx vitest run src/__tests__/branding.test.ts`
Expected: PASS (3 assertions).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/tauri.conf.json crates/vox-gui/ui/src/__tests__/branding.test.ts
git commit -m "feat(axis): window title -> Axis (productName/identifier unchanged)"
```

### Task B2 — Sidebar brand mark `V`/`VOX` → `A`/`AXIS` [SEQUENTIAL]

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` (~lines 175, 178)
- Test: `crates/vox-gui/ui/src/components/layout/Sidebar.branding.test.tsx`

- [ ] **Step 1 (gate):** Confirm the current mark JSX and that the render harness below still matches the real component contract:
  ```
  rg -n ">V<|>VOX<" crates/vox-gui/ui/src/components/layout/Sidebar.tsx
  rg -n "vi.mock|baseProps|surfaceRegistry.generated|@tauri-apps/api/core" crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx
  ```
  Expected: a `>V<` glyph span and a `>VOX<` wordmark div; the existing test mocks `@tauri-apps/api/core` and `../../generated/surfaceRegistry.generated` and renders with a `baseProps` object. If the mock paths or `baseProps` shape differ from Step 2, **STOP and report** — do not guess.

- [ ] **Step 2: Write the failing test.** ⚠️ `Sidebar` does NOT render without these two mocks, and the brand mark/footer only render when `mode` is NOT `'rail'` (use `'default'`). This harness is copied from the verified `Sidebar.test.tsx` fixture:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ display_name: 'operator@vox' }),
}));
vi.mock('../../generated/surfaceRegistry.generated', () => ({
  SURFACE_REGISTRY: [
    { viewKey: 'dashboard', navLabel: 'Dashboard', parentSurface: 'agents', tier: 'surface' },
    { viewKey: 'settings', navLabel: 'Settings', parentSurface: null, tier: 'surface' },
  ],
}));

import { Sidebar } from './Sidebar';

const baseProps = {
  view: 'dashboard',
  setView: vi.fn(),
  agentsCount: 2,
  data: { agents: [], stream: [], alerts: [], skills: [], peers: [], kpis: {} as any, contextChips: [] },
  mode: 'default' as const, // NOT 'rail' — the brand mark is hidden when collapsed
  setMode: vi.fn(),
  pushToast: vi.fn(),
  appVersion: '0.6.0',
} as React.ComponentProps<typeof Sidebar>;

describe('Axis branding — sidebar', () => {
  it('shows the AXIS wordmark, not VOX', () => {
    render(<Sidebar {...baseProps} />);
    expect(screen.getByText('AXIS')).toBeTruthy();
    expect(screen.queryByText('VOX')).toBeNull();
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `npx vitest run src/components/layout/Sidebar.branding.test.tsx`
Expected: FAIL — `Unable to find an element with the text: AXIS`.

- [ ] **Step 4: Make the change** in `Sidebar.tsx`:
  - `>V<` → `>A<` (the glyph span, ~line 175)
  - `>VOX<` → `>AXIS<` (the wordmark div, ~line 178)

- [ ] **Step 5: Run test + typecheck**

Run: `npx vitest run src/components/layout/Sidebar.branding.test.tsx` → PASS
Run: `npx tsc --noEmit` → no errors

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/Sidebar.branding.test.tsx
git commit -m "feat(axis): sidebar brand mark V/VOX -> A/AXIS"
```

### Task B3 — Footer spells out "Vox Axis" [SEQUENTIAL — after B2]

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` (~line 310)
- Test: extend `crates/vox-gui/ui/src/components/layout/Sidebar.branding.test.tsx`

- [ ] **Step 1 (gate):** `rg -n "build \{appVersion" crates/vox-gui/ui/src/components/layout/Sidebar.tsx` — paste the exact footer line.
  Expected: `<div className="font-mono text-[9px] text-zinc-500">build {appVersion ?? 'unknown'} · tauri 2</div>`

- [ ] **Step 2: Add the failing assertion** to `Sidebar.branding.test.tsx`:

```tsx
  it('footer spells out the Vox Axis full brand', () => {
    render(<Sidebar {...baseProps} />);
    expect(screen.getByText(/Vox Axis/)).toBeTruthy();
  });
```
(Add this inside the same `describe` block from Task B2, reusing its `baseProps` and mocks.)

- [ ] **Step 3: Run to verify it fails**

Run: `npx vitest run src/components/layout/Sidebar.branding.test.tsx`
Expected: FAIL — text `Vox Axis` not found.

- [ ] **Step 4: Make the change** — prepend the full brand to the footer line in `Sidebar.tsx`:
```tsx
                <div className="font-mono text-[9px] text-zinc-500">Vox Axis · build {appVersion ?? 'unknown'} · tauri 2</div>
```

- [ ] **Step 5: Run test + typecheck**

Run: `npx vitest run src/components/layout/Sidebar.branding.test.tsx` → PASS (2 assertions)
Run: `npx tsc --noEmit` → no errors

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/Sidebar.branding.test.tsx
git commit -m "feat(axis): footer spells out Vox Axis full brand"
```

### Task B4 — `vox axis` subcommand alias [PARALLEL-SAFE]

**Files:**
- Modify: `crates/vox-cli/src/lib.rs:431-435` (the `Gui` variant)
- Test: `crates/vox-cli/tests/axis_alias.rs`

- [ ] **Step 1 (gate):** `rg -n "pub struct VoxCliRoot|pub enum Cli|Gui \{|cfg\(feature = \"gui\"\)" crates/vox-cli/src/lib.rs` — paste output. Confirm `VoxCliRoot` wraps `cmd: Cli`, and the `Gui` variant is `#[cfg(feature = "gui")]`. Also confirm a `gui` feature exists: `rg -n "^\[features\]" -A 20 crates/vox-cli/Cargo.toml`.

- [ ] **Step 2: Write the failing test**

```rust
// crates/vox-cli/tests/axis_alias.rs
#![cfg(feature = "gui")]
use clap::Parser;
use vox_cli::{Cli, VoxCliRoot};

#[test]
fn vox_axis_is_an_alias_for_gui() {
    let parsed = VoxCliRoot::try_parse_from(["vox", "axis"]).expect("`vox axis` should parse");
    assert!(matches!(parsed.cmd, Cli::Gui { .. }), "`vox axis` must resolve to the Gui subcommand");
}
```
(If `VoxCliRoot`/`Cli` are not re-exported from the crate root, paste `rg -n "pub use|pub mod" crates/vox-cli/src/lib.rs` and import from the correct path; do not invent a path.)

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli --features gui --test axis_alias`
Expected: FAIL — `vox axis` is an unrecognized subcommand.

> ✅ Verified idiomatic: this `Cli` enum already uses `#[command(visible_alias = "…")]`
> on 9 variants (e.g. `fabrica`/`fab`, `secrets`/`clavis`, `gamify`/`ludus`). clap is
> 4.6.1. The `gui = []` feature exists (`Cargo.toml:93`). `VoxCliRoot` + `Cli` are `pub`
> at the crate root, so `use vox_cli::{Cli, VoxCliRoot};` resolves.

- [ ] **Step 4: Add the alias** — in `crates/vox-cli/src/lib.rs`, annotate the `Gui` variant:

```rust
    /// Launch the native Vox Axis (Axis) GUI — the Vox GUI under its product brand.
    /// Alias: `vox axis`. Use --command <view> to open directly to a surface (e.g. 'catalog', 'flow').
    #[cfg(feature = "gui")]
    #[command(visible_alias = "axis")]
    Gui {
        #[command(flatten)]
        args: cli_args::GuiArgs,
    },
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli --features gui --test axis_alias`
Expected: PASS.
Then: `cargo clippy -p vox-cli --features gui -- -D warnings` → clean; `cargo fmt -p vox-cli`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/lib.rs crates/vox-cli/tests/axis_alias.rs
git commit -m "feat(axis): add `vox axis` alias for the GUI subcommand"
```

### Task B5 — Brand phrasing in launch log + root help [SEQUENTIAL — after B4]

**Files:**
- Modify: `crates/vox-cli/src/commands/gui.rs:7` (tracing line)
- Modify: `crates/vox-cli/src/lib.rs:107` (root `long_about` mentions `vox gui`)

- [ ] **Step 1 (gate):**
  ```
  rg -n "Launching Vox Native GUI" crates/vox-cli/src/commands/gui.rs
  rg -n "vox gui +— launch" crates/vox-cli/src/lib.rs
  ```
  Paste both. These are string/comment changes (no behavior), so they are verified by re-grep, not a unit test.

- [ ] **Step 2: Update the tracing line** in `commands/gui.rs`:
```rust
    tracing::info!("Launching Vox Axis (Axis) — the native Vox GUI…");
```

- [ ] **Step 3: Update the root `long_about` line** in `lib.rs` so the help mentions the brand and the alias. Change the `Visualization:` block to:
```
\n\nVisualization:\n  vox gui (alias: vox axis)   — launch Axis, the native Vox GUI dashboard & catalog
```
  (Edit the existing `long_about` string literal at `lib.rs:107`; keep the rest of the string intact.)

- [ ] **Step 4: Verify**

Run: `rg -n "Vox Axis|alias: vox axis" crates/vox-cli/src/commands/gui.rs crates/vox-cli/src/lib.rs` — paste output (both hits present).
Run: `cargo build -p vox-cli --features gui` → compiles; `cargo clippy -p vox-cli --features gui -- -D warnings` → clean; `cargo fmt -p vox-cli`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/gui.rs crates/vox-cli/src/lib.rs
git commit -m "docs(axis): brand phrasing in GUI launch log and root help"
```

### Task B6 — One-line docs reference [PARALLEL-SAFE]

**Files:**
- Create: `docs/src/contributors/axis-brand.md`

- [ ] **Step 1:** Create the doc with the required `docs/src/` frontmatter (the repo enforces it). Content states the brand mapping so future edits don't re-fork the name:

```markdown
---
title: "Axis — the Vox GUI product brand"
description: "Branding reference: 'Axis' (full 'Vox Axis') is the product name of the Vox GUI. Launch with `vox axis` (alias of `vox gui`). Display uses 'Vox Axis'/'Axis'; identifiers use `axis`/`VoxAxis`. The `vox-gui` crate, `vox` binary, productName, and identifier are intentionally unchanged."
category: "Contributors"
status: "current"
training_eligible: true
---

# Axis — the Vox GUI product brand

**Axis** (full brand **Vox Axis**) is the product name of the Vox GUI. It launches via
`vox axis` (a clap alias of `vox gui`) and renders "Axis" in the window title and
sidebar mark, with "Vox Axis" spelled out in the footer.

**Naming convention:** display/prose = "Vox Axis" or "Axis"; identifiers/commands/files
= `axis` / `VoxAxis`. On first mention in a file, write "Vox Axis (Axis)".

**Deliberately unchanged** (brand-layer only): the `vox-gui` crate name, the `vox`
binary, `vox-gui.exe`, `tauri.conf.json` `productName` ("Vox") and `identifier`
(`org.vox-foundation.gui`). Renaming those is an out-of-scope follow-up.
```

- [ ] **Step 2: Verify** the docs frontmatter gate accepts the file (this is the exact CI/pre-push gate — verified against `.github/workflows/docs-quality.yml`):

Run: `cargo run -p vox-doc-pipeline -- --lint-only`
Expected: pass (the lint validates `category` ∈ the enum — `"Contributors"` is valid — plus `status` and `training_eligible`). Note: the `category` MUST be exactly `"Contributors"`; `"Contributor Guides"` is rejected.

- [ ] **Step 3: Commit**

```bash
git add docs/src/contributors/axis-brand.md
git commit -m "docs(axis): brand reference + naming convention"
```

---

# PHASE C — Handback (close the loop)

> This phase is **not** code. After all Phase-B tasks are green and committed, Flash
> produces a **handback block** for the human to paste back into Claude Code. The
> ledger is updated **in Claude Code from that paste** — NOT by the Antigravity runner.

### Task C1 — Emit the handback block (Flash, final step)

- [ ] **Step 1:** Confirm the whole tree is green:
  - `cd crates/vox-gui/ui && npx vitest run src/__tests__/branding.test.ts src/components/layout/Sidebar.branding.test.tsx && npx tsc --noEmit`
  - `cargo test -p vox-cli --features gui --test axis_alias`
  - `cargo clippy -p vox-cli --features gui -- -D warnings`
  - `cargo run -p vox-doc-pipeline -- --lint-only` (docs frontmatter gate for Task B6)
  - **Note:** no task changes `vox-gui` Rust (the title is a JSON value, the marks are TS), so do NOT run `cargo clippy -p vox-gui` — it forces a slow/flaky Tauri build for no covered change.
- [ ] **Step 2:** Gather: the commit SHAs you made, the green test counts per lane, and any pre-flight-gate mismatch or deviation you hit.
- [ ] **Step 3:** Emit EXACTLY this markdown block as your final message (fill the angle-bracket fields). Do **not** edit the ledger yourself.

````markdown
## VOX-AXIS HANDBACK → paste into Claude Code to update the ledger

```yaml
# --- AGH-0010 ---
id: AGH-0010
date: <YYYY-MM-DD>
plan: docs/superpowers/plans/2026-06-19-vox-axis-rebrand.md
prompt_artifact: docs/superpowers/plans/2026-06-19-vox-axis-GEMINI-FLASH-HANDOFF.md
prompt_version: v1
subsystem: vox-axis-rebrand
target: gemini-3.5-flash / antigravity
claude_inputs: [spec, plan, launch-statement, brand-assets]
delivered: [crates/vox-gui/tauri.conf.json, crates/vox-gui/ui/src/components/layout/Sidebar.tsx, crates/vox-cli/src/lib.rs, crates/vox-cli/src/commands/gui.rs, docs/src/contributors/axis-brand.md]
loc: <int>
outcome: <green|partial|failed>
verification: { tests: "<N> passed (vitest <a>, cargo <b>)", clippy: <clean|warns>, tsc: <clean|errors>, smoke: <ok|n/a> }
errors_encountered:
  - { what: "<symptom or 'none'>", root_cause: "<cause>", category: "<hallucinated-api|wrong-path|build-gate|fmt-gate|scope-creep|already-done|none>", who: <agent|plan|preexisting> }
agent_deviations:
  - "<deviation + risk, or 'none'>"
review_findings: pending
verdict: pending
prompt_lessons:
  - "<1-3 lessons that would harden the next Flash prompt>"
commits: [<sha>, <sha>, ...]
```

**Prose summary:** <2-4 sentences: what shipped, what (if anything) deviated, what to double-check.>
````

### (Claude Code, on receiving the handback) — ledger append protocol
When the human pastes the handback block back into Claude Code:
1. Append the `yaml` block verbatim to §C of `docs/superpowers/antigravity-handoff-ledger.md` (use the next free `AGH-NNNN` if `AGH-0010` is taken — current last id is `AGH-0009`).
2. Run `vox ci handoff-ledger` (the ledger lint) to validate the entry.
3. Code-review the delivered commits; fill `review_findings` + `verdict`; promote any recurring lesson into §B.
4. Commit: `docs(ledger): AGH-0010 vox-axis rebrand handback`.

---

## Self-Review (completed by plan author)

- **Spec coverage:** every spec §3 touchpoint maps to a task — title→B1, glyph/wordmark→B2, footer→B3, `vox axis`→B4, log/help→B5, docs→B6, icon set→Phase A. ✅
- **Placeholder scan:** no TBD/TODO; the one intentional fill-in (Sidebar prop fixture in B2) has an explicit gate step to source it from the existing `Sidebar.test.tsx`. ✅
- **Type consistency:** `VoxCliRoot`/`Cli`/`Cli::Gui` match `lib.rs`; `visible_alias` is the real clap attribute; test imports flagged to verify the re-export path before use. ✅
- **Scope:** single subsystem (brand layer); no decomposition needed. ✅
