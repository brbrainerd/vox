# Vox Axis / "Axis" Rebrand — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> 🤖 **EXECUTION TARGET — READ FIRST.** Phase B of this plan is written for **Gemini
> Flash 3.5 in Antigravity**. Flash has ~48% unaided in-IDE completion, no mid-task
> checkpoint, and a hard quota cutoff. See
> `docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`.
> **Phase A (brand assets) is NOT for Flash** — it is generated in Claude Code before
> the handoff and committed; you only reference/verify the committed files.

**Goal:** Rebrand the Vox GUI to **"Axis"** (full brand "Vox Axis") at the identity layer only — window title, in-app mark, footer, a `vox axis` launch alias, and a regenerated icon set — with zero crate/binary/identifier renames.

**Architecture:** Brand-layer change across four lanes: (1) a JSON config value, (2) a **professional front-end brand layer** — a reusable, theme-reactive `AxisMark` SVG component that consumes the existing accent token (`--brass`), the sidebar brand lockup, favicon/`index.html`, and a token-hygiene test (no new/dead tokens) — (3) one clap subcommand alias + help/log strings, (4) binary image assets generated ahead of time. No new Rust types, no API churn. `productName`/`identifier` in `tauri.conf.json` stay "Vox"/`org.vox-foundation.gui` on purpose (they drive installer/bundle identity).

**Execution split (this is the load-bearing decision):** Gemini Flash breaks on inline-SVG JSX, asset files, HTML-head wiring, and multi-element component restructures (its hallucination/path zone). So **Claude Code pre-builds the entire React/asset/token surface** (Phases A + D) before the handoff, and **Flash gets only rock-solid atomic edits** (Phase B: a JSON value, a clap alias, string/comment swaps, a docs file). Flash never touches `Sidebar.tsx`, `AxisMark`, the token sources, or `index.html`.

**Execution tiers (who runs what, and why):**

| Tier | Runs | Workload | Why this tier |
|---|---|---|---|
| **Opus** (this session) | design + author | Phase A asset design; the `AxisMark` geometry/JSX, token *semantics*, and the design-handoff spec; this plan + critique | Taste/visual judgment + cross-file design decisions. Already done where marked ✅. |
| **Sonnet** (Claude harness, interactive session — NOT a subagent: subagents are read-only in this sandbox) | implement | Phase D mechanical execution: port the committed SVG→JSX verbatim, add tokens + regenerate, wire the sidebar lockup, favicon, `index.html` | Fully specified, deterministic TDD with code already written in the plan + the design-handoff spec — Sonnet is reliable and cheaper for spec-following work that runs tests. |
| **Gemini Flash 3.5** (Antigravity) | implement | Phase B: B1 (JSON title), B4 (clap alias), B5 (strings), B6 (docs) | Atomic, gated, no JSX/asset/HTML. Exercises the handoff loop on its safe envelope. |

> **Design-handoff spec:** `docs/superpowers/specs/2026-06-19-vox-axis-brand-design-handoff.md`
> — token/state/responsive/a11y/edge-case detail for every Phase-D surface. Phase D
> tasks below carry the code; the handoff spec carries the design contract. Read it
> before executing Phase D.

**Tech Stack:** Tauri 2 (`tauri.conf.json`), TypeScript/React + vitest (`vox-gui/ui`), Style Dictionary (`pnpm tokens:build`), Rust + clap (`vox-cli`), `tauri icon` + `resvg` for assets.

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
- The CLI root parser is `VoxCliRoot` (`#[derive(Parser)]`) wrapping `pub enum Cli` (`#[derive(Subcommand)]`); the `Gui` variant is `#[cfg(feature = "gui")]` (`crates/vox-cli/src/lib.rs:431-435`). The `vox axis` alias is added there.
- **Claude's Phases A + D are already committed**: the Axis icon set (`crates/vox-gui/icons/`), the `AxisMark` component, the brand tokens, the rebranded `Sidebar.tsx`, and `index.html`. You do **not** generate images, touch `Sidebar.tsx`/`AxisMark.tsx`/`index.html`/token sources, or restyle anything — that surface is DONE.

**Mandatory pre-flight (run from repo root, paste output, confirm before any Phase-B code):**
```
rg -n "\"productName\"|\"title\"|\"identifier\"" crates/vox-gui/tauri.conf.json
rg -n "pub enum Cli|Gui \{|cfg\(feature = \"gui\"\)|pub struct VoxCliRoot" crates/vox-cli/src/lib.rs
git -C . status --porcelain crates/vox-gui/icons | head
```
Expected: `productName`/`identifier` present and unchanged; window `title` still `"Vox"` (B1 flips it); `VoxCliRoot` + `Cli` + a `#[cfg(feature = "gui")] Gui` variant; the Axis icons committed (Phase A).

> **Tandem note:** Phase D (the React/asset/token surface — `Sidebar.tsx`, `AxisMark`,
> token sources, `index.html`, `public/favicon.svg`) is owned by **Claude Code and may
> be landing in parallel with your run**. Your Phase-B tasks (B1/B4/B5/B6) touch
> **disjoint files** (`tauri.conf.json`, `vox-cli/**`, `docs/**`) and do **not** depend
> on Phase D — proceed regardless of whether those Phase-D files exist yet. You must
> **never create or edit** any Phase-D file. (No two-owner file → no merge conflict.)

**Task-split table (Phase B = Flash only — the React/asset surface is Claude's Phases A + D):**

| Task | Touches | Tag |
|---|---|---|
| B1 — window title → "Axis" | `tauri.conf.json` + new vitest `tauriConf.branding.test.ts` | [PARALLEL-SAFE] |
| B4 — `vox axis` subcommand alias | `vox-cli/src/lib.rs` (+ test) | [PARALLEL-SAFE] |
| B5 — brand phrasing in help/log | `vox-cli/src/lib.rs`, `commands/gui.rs` | [SEQUENTIAL] (shares lib.rs with B4) |
| B6 — one-line docs reference | `docs/src/contributors/axis-brand.md` | [PARALLEL-SAFE] |

Run order: B1 ∥ B4 ∥ B6 first; then B5 after B4 (same file).

> **RETIRED:** old **B2** (sidebar `V`/`VOX`→`A`/`AXIS`) and **B3** (footer brand line)
> are superseded by **Phase D (Claude-side)** — the sidebar gets the real `AxisMark`
> component, not a letter swap. Do NOT edit `Sidebar.tsx` as Flash.
> The B1 test file is renamed `tauriConf.branding.test.ts` so it can't collide with
> the Claude-authored `indexHtml.branding.test.ts` (Phase D4).

---

# PHASE A — Brand assets 🧑‍🎨 (CLAUDE-CODE PRE-FLIGHT — do NOT hand to Flash)

> ✅ **DONE (2026-06-19, this session).** Final mark = a **gimbal / gyroscope**
> (nested tilted rings + outer ring pierced by a bold spin-axis arrow — "axis at a
> distance"), **monochrome white** on the brass→amber→zinc tile. Rendered with
> **resvg** (`cargo install resvg` → `resvg.exe`; accurate gradients/geometry — used
> instead of ImageMagick, whose gradient-on-stroke render put orange in the arrow
> tip), then `cargo tauri icon`. Commits: …→ `8c29861f5e` (axis-frame) →
> `b484eee94b` (gimbal, monochrome, resvg). The icons under `crates/vox-gui/icons/`
> are committed; **do not regenerate** unless redesigning. Recipe below.

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
  Primary tool **resvg** (accurate SVG renderer; install once with `cargo install resvg`):
  ```bash
  resvg --width 1024 --height 1024 crates/vox-gui/icons/source/axis.svg crates/vox-gui/icons/source/axis-1024.png
  ```
  (On Windows the binary is `~/.cargo/bin/resvg.exe`.) Avoid ImageMagick for this SVG:
  its gradient-on-stroke rendering tinted the white arrow tip orange. If resvg is
  unavailable, an Inkscape `--export-type=png -w 1024` is the next-best fallback.
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

# PHASE D — Professional front-end blend 🧑‍🎨 (CLAUDE-CODE PRE-FLIGHT — do NOT hand to Flash)

> ✅ **DONE (2026-06-19, Claude Code, executing-plans, TDD).** 19 vitest tests green
> across 5 files; existing Sidebar suite still 10/10; only pre-existing unrelated tsc
> error is `DueNudge.tsx` (untracked WIP, not part of this work). Commits: AxisMark,
> token-hygiene test, sidebar lockup (+ rail-mode mark), favicon + `index.html`.
>
> Runs in **Claude Code** alongside Phase A, before the handoff. Scope = **brand
> essentials + light theme tokens**. This is the inline-SVG / asset / HTML-head /
> token surface where Flash hallucinates — Claude owns all of it. TDD; atomic green
> commits.
>
> **Token reuse (anti-split-brain):** the UI already has a Style-Dictionary pipeline —
> sources `crates/vox-gui/ui/tokens/{primitive,semantic}.json`, built via
> `pnpm tokens:build` → `src/styles/tokens.generated.{css,ts}`; `--brass` is
> theme-switched in `src/index.css` (arcane=gold `#d4af37`, void=violet, glacier=cyan).
> Brand tokens **extend** `semantic.json` (referencing existing primitives); never add
> a parallel color system.

### Task D1 — `AxisMark` brand component

**Files:**
- Create: `crates/vox-gui/ui/src/components/brand/AxisMark.tsx`
- Test: `crates/vox-gui/ui/src/components/brand/AxisMark.test.tsx`

- [ ] **Step 1:** Write the failing test (jsdom): renders an accessible SVG and the spin-axis arrow.

```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { AxisMark } from './AxisMark';

describe('AxisMark', () => {
  it('renders an accessible gimbal SVG with the spin axis', () => {
    const { container } = render(<AxisMark className="size-6 text-brass" title="Axis" />);
    const svg = container.querySelector('svg');
    expect(svg).toBeTruthy();
    expect(svg?.getAttribute('viewBox')).toBe('0 0 1024 1024');
    // monochrome via currentColor (caller controls hue through text-*)
    expect(svg?.innerHTML).toMatch(/currentColor/);
    expect(container.querySelector('title')?.textContent).toBe('Axis');
  });
});
```

- [ ] **Step 2:** Run → FAIL (module not found). `npx vitest run src/components/brand/AxisMark.test.tsx`

- [ ] **Step 3:** Implement the component. Mark strokes/arrow use `currentColor` (themeable); the hub uses the base-bg token so it punches through on any tile. Port the committed `crates/vox-gui/icons/source/axis.svg` geometry into JSX (camelCase attrs, `strokeOpacity`, `strokeWidth`, `strokeLinecap`):

```tsx
export function AxisMark({ className, title = 'Axis' }: { className?: string; title?: string }) {
  return (
    <svg viewBox="0 0 1024 1024" className={className} role="img" aria-label={title}
         xmlns="http://www.w3.org/2000/svg" fill="none">
      <title>{title}</title>
      {/* gimbal rings — monochrome via currentColor */}
      <g stroke="currentColor" strokeLinecap="round">
        <circle cx="512" cy="512" r="292" strokeOpacity="0.5" strokeWidth="24" />
        <ellipse cx="512" cy="512" rx="292" ry="116" strokeOpacity="0.85" strokeWidth="30" transform="rotate(34 512 512)" />
        <ellipse cx="512" cy="512" rx="292" ry="116" strokeOpacity="0.85" strokeWidth="30" transform="rotate(-34 512 512)" />
      </g>
      {/* spin axis + arrow */}
      <line x1="512" y1="236" x2="512" y2="872" stroke="currentColor" strokeWidth="46" strokeLinecap="round" />
      <polygon points="512,140 466,244 558,244" fill="currentColor" />
      {/* hub */}
      <circle cx="512" cy="512" r="54" className="fill-bg-base" />
      <circle cx="512" cy="512" r="54" fill="none" stroke="currentColor" strokeWidth="22" />
    </svg>
  );
}
```

- [ ] **Step 4:** Run → PASS; `npx tsc --noEmit`. **Step 5:** Commit `feat(axis): AxisMark brand component (themeable gimbal SVG)`.

### Task D2 — Brand token consistency (consume existing tokens; add none)

**Files:**
- Test: `crates/vox-gui/ui/src/components/brand/AxisMark.tokens.test.ts`
- (No changes to `semantic.json` / `tailwind.config.js` — see the reframe note below.)

> **Reframed after design audit (do NOT add a `brand` color group).** The accent is
> **already a shared, themeable token** — `--brass`, switched arcane/void/glacier in
> `src/index.css`, exposed as the Tailwind `brass` color. Adding Style-Dictionary
> `color.brand.*` tokens would (a) be **dead tokens** (no consumer once the mark uses
> `text-brass`) and (b) **break theming** — an SD brand token resolves to the *static*
> `#d4af37`, not the live `--brass` var. So "light theme tokens" here = **consume the
> existing tokens; introduce zero new color tokens; zero hardcoded brand hexes** in the
> new surfaces.

- [ ] **Step 1 (gate):** confirm the accent token + theme switching exist:
  ```
  rg -n "\-\-brass" crates/vox-gui/ui/src/index.css
  rg -n "brass:|--brass" crates/vox-gui/ui/tailwind.config.js
  ```
  Expected: `--brass` defined for `arcane`/`void`/`glacier`, and `brass: 'rgb(var(--brass) / <alpha-value>)'` in Tailwind. This is the token the mark uses (`text-brass`).

- [ ] **Step 2 (consistency assertion):** add a token-hygiene test so a future hardcoded brand hex regresses loudly:

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
const here = dirname(fileURLToPath(import.meta.url));
const read = (p: string) => readFileSync(resolve(here, p), 'utf8');
describe('brand surfaces use tokens, not hardcoded hexes', () => {
  it('AxisMark uses currentColor + the bg-base token (no hex)', () => {
    const src = read('../brand/AxisMark.tsx');
    expect(src).toMatch(/currentColor/);
    expect(src).not.toMatch(/#[0-9a-fA-F]{6}/); // no literal hex colors
  });
});
```
Place at `crates/vox-gui/ui/src/components/brand/AxisMark.tokens.test.ts`.

- [ ] **Step 3:** Run → it should PASS once D1's `AxisMark` is token-clean (strokes `currentColor`, hub `fill-bg-base`). If it fails, fix `AxisMark` to remove the hex — do NOT add a token to satisfy it. **Step 4:** Commit `test(axis): brand surfaces consume existing tokens (no new/dead tokens)`.

> If a *genuine* new consumer ever needs a brand-specific value, add it to
> `semantic.json` then — not speculatively. YAGNI.

### Task D3 — Sidebar brand block (retires old B2/B3)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` (brand block ~lines 173–180; footer ~line 310)
- Test: `crates/vox-gui/ui/src/components/layout/Sidebar.branding.test.tsx`

- [ ] **Step 1 (gate):** `rg -n ">V<|>VOX<|from-brass via-amber|build \{appVersion" crates/vox-gui/ui/src/components/layout/Sidebar.tsx` — paste the brand-box JSX (the `from-brass via-amber-600 to-zinc-900` tile with `>V<`, the `>VOX<` wordmark) and the footer line.

- [ ] **Step 2:** Write the failing test (reuse the verified mocks + `baseProps` fixture, `mode: 'default'` — see the Sidebar test harness in `Sidebar.test.tsx`):

```tsx
// @vitest-environment jsdom
// ... (same vi.mock('@tauri-apps/api/core') + vi.mock('../../generated/surfaceRegistry.generated')
//      + baseProps with mode:'default' as in Sidebar.test.tsx) ...
import { AxisMark } from '../brand/AxisMark';
describe('Axis branding — sidebar', () => {
  it('renders the AxisMark glyph + AXIS wordmark, no VOX/V letterform', () => {
    const { container } = render(<Sidebar {...baseProps} />);
    expect(container.querySelector('svg[aria-label="Axis"]')).toBeTruthy();
    expect(screen.getByText('AXIS')).toBeTruthy();
    expect(screen.queryByText('VOX')).toBeNull();
  });
  it('footer spells out the Vox Axis full brand', () => {
    render(<Sidebar {...baseProps} />);
    expect(screen.getByText(/Vox Axis/)).toBeTruthy();
  });
});
```

- [ ] **Step 3:** Run → FAIL. **Step 4:** Implement (per the design-handoff spec — mark is **themeable** via `text-brass`, and the brand stays visible when collapsed):
  - Replace the brand-box `<div className="relative size-6 … from-brass via-amber-600 to-zinc-900 …"><span …>V</span></div>` with a subtle glass tile wrapping the **theme-reactive** mark:
    ```tsx
    <div className="relative grid size-6 place-items-center rounded-md bg-white/[0.04] ring-1 ring-brass/40">
      <AxisMark className="size-4 text-brass" />
    </div>
    ```
  - `>VOX<` wordmark → `>AXIS<`.
  - Footer: `…>build {appVersion ?? 'unknown'} · tauri 2</div>` → `…>Vox Axis · build {appVersion ?? 'unknown'} · tauri 2</div>`.
  - **Collapsed (rail) improvement:** the brand block today only renders when `!collapsed`. Add a collapsed branch that still shows the mark centered, e.g. in the `collapsed ? … : …` header area render `<AxisMark className="size-6 text-brass" />` when `collapsed` (so the brand never disappears in rail mode). Confirm the exact `collapsed`/`mode` guard from the Step-1 gate before wiring.
  - Add `import { AxisMark } from '../brand/AxisMark';` at the top.

- [ ] **Step 4b (extend the test):** add a rail-mode assertion (the improvement):
  ```tsx
  it('keeps the mark visible in rail (collapsed) mode', () => {
    const { container } = render(<Sidebar {...baseProps} mode={'rail' as const} />);
    expect(container.querySelector('svg[aria-label="Axis"]')).toBeTruthy();
  });
  ```

- [ ] **Step 5:** Run → PASS; `npx tsc --noEmit`. **Step 6:** Commit `feat(axis): sidebar brand block uses AxisMark + Vox Axis footer`.

### Task D4 — Favicon + index.html

**Files:**
- Create: `crates/vox-gui/ui/public/favicon.svg`
- Modify: `crates/vox-gui/ui/index.html`
- Test: `crates/vox-gui/ui/src/__tests__/indexHtml.branding.test.ts`

- [ ] **Step 1:** Create `public/favicon.svg` = the committed gimbal mark, web-trimmed (fixed brass tile + white mark; the static favicon does NOT theme-switch). Vite serves `public/` at `/`, so the href is `/favicon.svg`.

- [ ] **Step 2:** Write the failing test (resolve via `import.meta.url`, like `tauriConf.branding.test.ts`):

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';
const here = dirname(fileURLToPath(import.meta.url)); // src/__tests__ -> src -> ui
const html = readFileSync(resolve(here, '../../index.html'), 'utf8');
describe('Axis branding — index.html', () => {
  it('document title is Axis', () => { expect(html).toMatch(/<title>Axis<\/title>/); });
  it('links the favicon', () => { expect(html).toMatch(/rel="icon"[^>]*href="\/favicon\.svg"/); });
});
```

- [ ] **Step 3:** Run → FAIL. **Step 4:** Edit `index.html`: `<title>Vox</title>` → `<title>Axis</title>`; add inside `<head>`: `<link rel="icon" type="image/svg+xml" href="/favicon.svg" />`.

- [ ] **Step 5:** Run → PASS. **Step 6:** Commit `feat(axis): favicon + Axis document title`.

**Phase D done →** the React/asset/token brand layer is committed; combined with Phase A, the Antigravity handoff (Phase B) can proceed.

---

# PHASE B — Branding wiring (Gemini Flash 3.5 / Antigravity)

### Task B1 — Window title → "Axis" [PARALLEL-SAFE]

**Files:**
- Modify: `crates/vox-gui/tauri.conf.json:14`
- Test: `crates/vox-gui/ui/src/__tests__/tauriConf.branding.test.ts`

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

Run (from `crates/vox-gui/ui`): `npx vitest run src/__tests__/tauriConf.branding.test.ts`
Expected: FAIL — `expected 'Vox' to be 'Axis'`.

- [ ] **Step 4: Make the change**

In `crates/vox-gui/tauri.conf.json`, change the window title only:
```json
        "title": "Axis",
```
(Leave `"productName": "Vox"` and `"identifier": "org.vox-foundation.gui"` exactly as they are.)

- [ ] **Step 5: Run test to verify it passes**

Run: `npx vitest run src/__tests__/tauriConf.branding.test.ts`
Expected: PASS (3 assertions).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/tauri.conf.json crates/vox-gui/ui/src/__tests__/tauriConf.branding.test.ts
git commit -m "feat(axis): window title -> Axis (productName/identifier unchanged)"
```

### Tasks B2 & B3 — RETIRED (superseded by Phase D, Claude-side)

The old B2 (swap the sidebar `V`/`VOX` letters to `A`/`AXIS`) and B3 (prepend
"Vox Axis" to the footer) are **removed**. A letter swap next to a real gimbal mark
looks unfinished. Instead, **Phase D (Claude-side)** replaces the sidebar's hardcoded
gradient-`V` box with the `AxisMark` component + "AXIS" wordmark and sets the footer
brand line, all tested and committed before the Flash handoff. **Flash must not edit
`Sidebar.tsx`.**

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
  - `cd crates/vox-gui/ui && npx vitest run src/__tests__/tauriConf.branding.test.ts src/components/layout/Sidebar.branding.test.tsx && npx tsc --noEmit`
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

- **Spec coverage:** every spec §3 touchpoint maps to a task — window title→B1, brand glyph+wordmark→**D3** (real `AxisMark`, retires B2), footer→**D3** (retires B3), `vox axis`→B4, log/help→B5, docs→B6, icon set→Phase A, favicon/doc-title + tokens→**D4/D2**. ✅
- **Placeholder scan:** no TBD/TODO. The Phase-D Sidebar test reuses the verified mocks + `baseProps` (`mode:'default'`) from `Sidebar.test.tsx`; D2 token values reference confirmed primitives. ✅
- **Type consistency:** `VoxCliRoot`/`Cli`/`Cli::Gui` match `lib.rs`; `visible_alias` is the real clap attribute (used 9× already); `AxisMark`'s `{ className, title }` API is consumed exactly that way in D3; brand Tailwind aliases match the `var(--color-brand-*)` tokens emitted by D2. ✅
- **Split integrity:** Flash (Phase B) touches only `tauri.conf.json`, `vox-cli/**`, and `docs/**` — never `Sidebar.tsx`/`AxisMark`/tokens/`index.html` (Claude's Phases A+D). No two-owner file. ✅
- **Scope:** single subsystem (brand layer); no decomposition needed. ✅
