# Vox Axis "Roman" restyle (Limes) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restyle the Vox Axis GUI into a clarity-first, Roman-themed dual-scope (basalt dark / travertine light) interface — token layer + 3 hero surfaces + a heavier "groma" app/favicon mark — with nothing applied until previewed.

**Architecture:** Extend the existing style-dictionary → CSS-vars → Tailwind pipeline. Dark (basalt) tokens emit under `:root`; a new travertine build emits under `[data-theme="travertine"]`. Roman character is carried by self-hosted Cinzel/Inter/EB-Garamond fonts, an imperial-gold + verdigris accent pair, and engraved corner-tick / fading-rule utilities. Hero surfaces migrate hardcoded `zinc`/`white-α` utilities to semantic tokens so both scopes render correctly. A contrast unit test guards AA.

**Tech Stack:** Vite + React 19 + TypeScript, Tailwind 3.4, style-dictionary 5, Vitest, Tauri 2. Fonts: Cinzel / EB Garamond / Inter (SIL-OFL). Icon raster via `@tauri-apps/cli icon`.

**Spec:** `docs/superpowers/specs/2026-06-20-vox-axis-roman-restyle-design.md`

**Working dir for all `pnpm` commands:** `crates/vox-gui/ui`

---

## Task 1: Self-host the Roman fonts + wire font-family tokens

**Files:**
- Create: `crates/vox-gui/ui/src/styles/fonts.css`
- Create: `crates/vox-gui/ui/public/fonts/` (woff2 subsets: `Cinzel-SemiBold.woff2`, `Inter-Regular.woff2`, `Inter-Medium.woff2`, `EBGaramond-Italic.woff2`)
- Modify: `crates/vox-gui/ui/src/index.css:1-5`
- Modify: `crates/vox-gui/ui/tailwind.config.js:26-29`

- [ ] **Step 1: Add the woff2 files**

Download Latin-subset woff2 for Cinzel SemiBold, Inter Regular + Medium, EB Garamond Italic (all SIL-OFL) into `crates/vox-gui/ui/public/fonts/`. (Use `pnpm dlx google-webfonts-helper` or fetch from gwfh; commit the `.woff2` binaries.)

- [ ] **Step 2: Create `src/styles/fonts.css`**

```css
/* Self-hosted, Latin-subset. No runtime network fetch (Tauri CSP font-src 'self'). */
@font-face { font-family: 'Cinzel'; font-weight: 600; font-display: swap;
  src: url('/fonts/Cinzel-SemiBold.woff2') format('woff2'); }
@font-face { font-family: 'Inter'; font-weight: 400; font-display: swap;
  src: url('/fonts/Inter-Regular.woff2') format('woff2'); }
@font-face { font-family: 'Inter'; font-weight: 500; font-display: swap;
  src: url('/fonts/Inter-Medium.woff2') format('woff2'); }
@font-face { font-family: 'EB Garamond'; font-style: italic; font-weight: 400; font-display: swap;
  src: url('/fonts/EBGaramond-Italic.woff2') format('woff2'); }
```

- [ ] **Step 3: Import fonts first in `index.css`**

In `index.css`, add as the very first line (before the token imports):
```css
@import './styles/fonts.css';
```

- [ ] **Step 4: Point Tailwind families at the fonts**

Replace `tailwind.config.js:26-29` `fontFamily` with:
```js
      fontFamily: {
        display: ['Cinzel', 'Georgia', 'serif'],
        sans: ['Inter', 'system-ui', 'Segoe UI', 'sans-serif'],
        serif: ['EB Garamond', 'Georgia', 'serif'],
        mono: ['ui-monospace', 'Cascadia Code', 'Consolas', 'JetBrains Mono', 'monospace'],
      },
```

- [ ] **Step 5: Verify build + fonts resolve**

Run: `pnpm build`
Expected: build succeeds; `dist/` contains the `fonts/` assets.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/public/fonts crates/vox-gui/ui/src/styles/fonts.css crates/vox-gui/ui/src/index.css crates/vox-gui/ui/tailwind.config.js
git commit -m "feat(vox-gui): self-host Roman fonts (Cinzel/Inter/EB Garamond) + family tokens"
```

---

## Task 2: Add Roman primitive ramps + font-family tokens

**Files:**
- Modify: `crates/vox-gui/ui/tokens/primitive.json`

- [ ] **Step 1: Add ramps + font.family to `primitive.json`**

Add these keys under `"color"` (keep existing keys; `neutral` stays as the basalt-cool ramp — it is already cool zinc, which suits basalt):
```json
    "basalt":     { "950": { "value": "#0a0c0d" }, "900": { "value": "#0c0e10" }, "850": { "value": "#11151a" }, "800": { "value": "#15191c" }, "700": { "value": "#1f262b" }, "600": { "value": "#2b343a" } },
    "travertine": { "50": { "value": "#f6f1e7" }, "100": { "value": "#f4eee1" }, "200": { "value": "#ece5d6" }, "300": { "value": "#e3dbc8" }, "ink": { "value": "#2a2620" }, "ink-soft": { "value": "#4a443a" }, "ink-muted": { "value": "#6b6457" } },
    "gold":       { "300": { "value": "#e0c478" }, "400": { "value": "#d4af37" }, "500": { "value": "#c9a24a" }, "600": { "value": "#a8842f" }, "700": { "value": "#8a6a26" } },
    "verdigris":  { "300": { "value": "#79c8ba" }, "400": { "value": "#4a9e8f" }, "600": { "value": "#2f7d6f" }, "700": { "value": "#1f5a50" } },
    "terracotta": { "400": { "value": "#d98b6a" }, "600": { "value": "#b25a37" } },
    "oxblood":    { "600": { "value": "#a3402f" }, "700": { "value": "#7e2d20" } }
```

Add a new top-level `"font.family"` group (sibling of `font.size`), referencing the families wired in Task 1:
```json
    "family": { "display": { "value": "Cinzel, Georgia, serif" }, "sans": { "value": "Inter, system-ui, sans-serif" }, "serif": { "value": "EB Garamond, Georgia, serif" }, "mono": { "value": "ui-monospace, Consolas, monospace" } }
```
(Place it inside the existing `"font"` object alongside `size`/`weight`/`leading`.)

- [ ] **Step 2: Rebuild tokens**

Run: `pnpm tokens:build`
Expected: prints `tokens built: ...`; `src/styles/tokens.generated.css` now contains `--color-gold-500`, `--color-verdigris-400`, `--font-family-display`, etc.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/tokens/primitive.json crates/vox-gui/ui/src/styles/tokens.generated.css crates/vox-gui/ui/src/styles/tokens.generated.ts
git commit -m "feat(vox-gui): add basalt/travertine/gold/verdigris primitives + font-family tokens"
```

---

## Task 3: Remap dark semantic tokens to basalt + gold/verdigris

**Files:**
- Modify: `crates/vox-gui/ui/tokens/semantic.json`

- [ ] **Step 1: Rewrite `semantic.json` (dark = default `:root`)**

```json
{
  "color": {
    "bg":     { "base": { "value": "{color.basalt.900}" }, "surface": { "value": "{color.basalt.800}" }, "elevated": { "value": "{color.basalt.700}" } },
    "text":   { "primary": { "value": "{color.neutral.50}" }, "secondary": { "value": "#c4ccd2" }, "muted": { "value": "#8b949a" } },
    "border": { "subtle": { "value": "{color.basalt.700}" }, "strong": { "value": "#3a444b" } },
    "accent": { "default": { "value": "{color.gold.500}" }, "secondary": { "value": "{color.verdigris.400}" } },
    "status": { "pass": { "value": "{color.verdigris.300}" }, "fail": { "value": "#f0a59a" }, "warn": { "value": "{color.terracotta.400}" }, "info": { "value": "#bfdbfe" } },
    "overlay": { "subtle": { "value": "rgba(255,255,255,0.04)" }, "hover": { "value": "rgba(255,255,255,0.07)" } }
  }
}
```

- [ ] **Step 2: Rebuild + commit**

Run: `pnpm tokens:build` (expected: success).
```bash
git add crates/vox-gui/ui/tokens/semantic.json crates/vox-gui/ui/src/styles/tokens.generated.css crates/vox-gui/ui/src/styles/tokens.generated.ts
git commit -m "feat(vox-gui): map dark semantic tokens to basalt scope"
```

---

## Task 4: Add travertine (light) semantic build

**Files:**
- Create: `crates/vox-gui/ui/tokens/semantic.travertine.json`
- Modify: `crates/vox-gui/ui/style-dictionary.config.mjs:45-56`

- [ ] **Step 1: Create `semantic.travertine.json`**

```json
{
  "color": {
    "bg":     { "base": { "value": "{color.travertine.200}" }, "surface": { "value": "{color.travertine.100}" }, "elevated": { "value": "{color.travertine.50}" } },
    "text":   { "primary": { "value": "{color.travertine.ink}" }, "secondary": { "value": "{color.travertine.ink-soft}" }, "muted": { "value": "{color.travertine.ink-muted}" } },
    "border": { "subtle": { "value": "rgba(0,0,0,0.10)" }, "strong": { "value": "rgba(0,0,0,0.22)" } },
    "accent": { "default": { "value": "{color.gold.700}" }, "secondary": { "value": "{color.verdigris.700}" } },
    "status": { "pass": { "value": "{color.verdigris.700}" }, "fail": { "value": "{color.oxblood.600}" }, "warn": { "value": "{color.terracotta.600}" }, "info": { "value": "#1d4e89" } },
    "overlay": { "subtle": { "value": "rgba(0,0,0,0.04)" }, "hover": { "value": "rgba(0,0,0,0.07)" } }
  }
}
```

- [ ] **Step 2: Add a travertine build to `style-dictionary.config.mjs`**

After the `contrast` block (before the final `await`s), add:
```js
const travertine = new StyleDictionary({
  source: ['tokens/primitive.json', 'tokens/semantic.travertine.json'],
  platforms: {
    css: { transformGroup: 'css', buildPath: 'src/styles/', files: [
      { destination: 'tokens.travertine.generated.css', format: 'css/vars-selector', options: { selector: '[data-theme="travertine"]' } },
    ] },
  },
});
```
And add `await travertine.buildAllPlatforms();` next to the other `buildAllPlatforms()` calls; update the final `console.log` to mention `tokens.travertine.generated.css`.

- [ ] **Step 3: Import the travertine sheet in `index.css`**

Add after the two existing token imports (around `index.css:2`):
```css
@import './styles/tokens.travertine.generated.css';
```

- [ ] **Step 4: Rebuild + verify selector**

Run: `pnpm tokens:build`
Expected: `src/styles/tokens.travertine.generated.css` exists and opens with `[data-theme="travertine"] {`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/tokens/semantic.travertine.json crates/vox-gui/ui/style-dictionary.config.mjs crates/vox-gui/ui/src/styles/tokens.travertine.generated.css crates/vox-gui/ui/src/index.css
git commit -m "feat(vox-gui): add travertine (light) semantic token build"
```

---

## Task 5: Rewrite index.css base — basalt default, retire void/glacier, add Roman utilities

**Files:**
- Modify: `crates/vox-gui/ui/src/index.css:7-60`

- [ ] **Step 1: Replace the `@layer base` theme block**

Replace the `:root,[data-theme="arcane"] … [data-theme="glacier"]` block and the hardcoded `html,body,#root` colors (`index.css:11-23`) with token-driven, scope-aware values:
```css
@layer base {
  :root { --brass: 201 162 74; }                         /* gold.500 → basalt scope */
  [data-theme="travertine"] { --brass: 138 106 38; }     /* gold.700 → daylight */

  html, body, #root {
    height: 100%; margin: 0; overflow: hidden;
    background: var(--color-bg-base);
    color: var(--color-text-muted);
    -webkit-font-smoothing: antialiased;
    font-family: var(--font-family-sans);
  }
}
```

- [ ] **Step 2: Delete the `void`/`glacier` background overrides**

Remove the non-layered `:root[data-theme='void'] …` override block (`index.css:54-60` and any `glacier` sibling).

- [ ] **Step 3: Add Roman ornament utilities**

Append to the `@layer components` block:
```css
  .vox-rule { height: 1px; background: linear-gradient(90deg, rgb(var(--brass)), rgb(var(--brass) / 0)); }
  .vox-tick { position: absolute; width: 11px; height: 11px; pointer-events: none; }
  .vox-tick-tl { top: 9px; left: 9px; border-top: 1px solid rgb(var(--brass)); border-left: 1px solid rgb(var(--brass)); }
  .vox-tick-tr { top: 9px; right: 9px; border-top: 1px solid var(--color-accent-secondary); border-right: 1px solid var(--color-accent-secondary); }
  .vox-display { font-family: var(--font-family-display); letter-spacing: 0.13em; text-transform: uppercase; }
```

- [ ] **Step 4: Verify both scopes build**

Run: `pnpm build` (expected: success). Manually toggle `document.documentElement.dataset.theme = 'travertine'` in dev to confirm the page background flips warm.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/index.css
git commit -m "feat(vox-gui): basalt default scope, retire void/glacier, add Roman ornament utilities"
```

---

## Task 6: Contrast guard test (AA) — TDD

**Files:**
- Create: `crates/vox-gui/ui/src/styles/__tests__/contrast.test.ts`
- Create: `crates/vox-gui/ui/src/styles/contrastTokens.ts`

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from 'vitest';
import { BASALT, TRAVERTINE, contrastRatio } from '../contrastTokens';

const PAIRS: Array<[string, 'text' | 'ui']> = [
  ['textPrimaryOnBase', 'text'], ['textSecondaryOnSurface', 'text'],
  ['accentOnBase', 'ui'], ['accentSecondaryOnSurface', 'ui'],
];

describe.each([['basalt', BASALT], ['travertine', TRAVERTINE]] as const)('%s contrast', (_name, scope) => {
  it.each(PAIRS)('%s meets AA', (key, kind) => {
    const ratio = contrastRatio(scope[key].fg, scope[key].bg);
    expect(ratio).toBeGreaterThanOrEqual(kind === 'text' ? 4.5 : 3.0);
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `pnpm vitest run src/styles/__tests__/contrast.test.ts`
Expected: FAIL — cannot resolve `../contrastTokens`.

- [ ] **Step 3: Implement `contrastTokens.ts`**

```ts
function lum(hex: string): number {
  const h = hex.replace('#', '');
  const n = h.length === 3 ? h.split('').map(c => c + c).join('') : h;
  const [r, g, b] = [0, 2, 4].map(i => parseInt(n.slice(i, i + 2), 16) / 255)
    .map(c => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
export function contrastRatio(fg: string, bg: string): number {
  const a = lum(fg), b = lum(bg);
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}
type Pairs = Record<string, { fg: string; bg: string }>;
export const BASALT: Pairs = {
  textPrimaryOnBase: { fg: '#fafafa', bg: '#0c0e10' },
  textSecondaryOnSurface: { fg: '#c4ccd2', bg: '#15191c' },
  accentOnBase: { fg: '#c9a24a', bg: '#0c0e10' },
  accentSecondaryOnSurface: { fg: '#4a9e8f', bg: '#15191c' },
};
export const TRAVERTINE: Pairs = {
  textPrimaryOnBase: { fg: '#2a2620', bg: '#ece5d6' },
  textSecondaryOnSurface: { fg: '#4a443a', bg: '#f4eee1' },
  accentOnBase: { fg: '#8a6a26', bg: '#ece5d6' },
  accentSecondaryOnSurface: { fg: '#1f5a50', bg: '#f4eee1' },
};
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm vitest run src/styles/__tests__/contrast.test.ts`
Expected: PASS (all pairs ≥ threshold). If any fails, darken/lighten that token in the relevant `semantic*.json` and the mirror value here until green.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/styles/contrastTokens.ts crates/vox-gui/ui/src/styles/__tests__/contrast.test.ts
git commit -m "test(vox-gui): AA contrast guard for basalt + travertine token pairs"
```

---

## Task 7: The groma mark — vector master + React component

**Files:**
- Create: `crates/vox-gui/icons/axis-mark.svg`
- Create: `crates/vox-gui/icons/axis-mark-16.svg`
- Create: `crates/vox-gui/ui/public/axis-mark.svg`
- Create: `crates/vox-gui/ui/src/components/ui/AxisMark.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/AxisMark.test.tsx`

- [ ] **Step 1: Write the full vector master `icons/axis-mark.svg`**

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" role="img" aria-label="Vox Axis">
  <circle cx="50" cy="50" r="40" fill="none" stroke="currentColor" stroke-width="7"/>
  <path d="M50 12 V88 M12 50 H88" stroke="currentColor" stroke-width="9" stroke-linecap="round"/>
  <circle cx="50" cy="50" r="9" fill="none" stroke="currentColor" stroke-width="7"/>
  <path d="M50 6 l6 10 h-12 z M50 94 l6 -10 h-12 z M6 50 l10 6 v-12 z M94 50 l-10 6 v-12 z" fill="currentColor"/>
</svg>
```

- [ ] **Step 2: Write the simplified 16px variant `icons/axis-mark-16.svg`**

```svg
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" role="img" aria-label="Vox Axis">
  <path d="M50 10 V90 M10 50 H90" stroke="currentColor" stroke-width="14" stroke-linecap="round"/>
  <circle cx="50" cy="50" r="11" fill="currentColor"/>
</svg>
```

- [ ] **Step 3: Copy the full master into the web public dir**

```bash
cp crates/vox-gui/icons/axis-mark.svg crates/vox-gui/ui/public/axis-mark.svg
```

- [ ] **Step 4: Write the failing component test**

```tsx
import { render } from '@testing-library/react';
import { AxisMark } from './AxisMark';

it('renders an accessible axis mark that inherits color', () => {
  const { getByRole } = render(<AxisMark />);
  const svg = getByRole('img', { name: /vox axis/i });
  expect(svg).toBeInTheDocument();
  expect(svg.querySelector('[stroke="currentColor"]')).toBeTruthy();
});
```

- [ ] **Step 5: Run it — expect FAIL** (`Cannot find module './AxisMark'`).

Run: `pnpm vitest run src/components/ui/AxisMark.test.tsx`

- [ ] **Step 6: Implement `AxisMark.tsx`**

```tsx
import React from 'react';

export function AxisMark({ size = 24, className }: { size?: number; className?: string }) {
  return (
    <svg width={size} height={size} viewBox="0 0 100 100" role="img" aria-label="Vox Axis"
      className={className} style={{ color: 'rgb(var(--brass))' }}>
      <circle cx="50" cy="50" r="40" fill="none" stroke="currentColor" strokeWidth="7" />
      <path d="M50 12 V88 M12 50 H88" stroke="currentColor" strokeWidth="9" strokeLinecap="round" />
      <circle cx="50" cy="50" r="9" fill="none" stroke="currentColor" strokeWidth="7" />
      <path d="M50 6 l6 10 h-12 z M50 94 l6 -10 h-12 z M6 50 l10 6 v-12 z M94 50 l-10 6 v-12 z" fill="currentColor" />
    </svg>
  );
}
```

- [ ] **Step 7: Run the test — expect PASS.** Commit.

```bash
git add crates/vox-gui/icons/axis-mark.svg crates/vox-gui/icons/axis-mark-16.svg crates/vox-gui/ui/public/axis-mark.svg crates/vox-gui/ui/src/components/ui/AxisMark.tsx crates/vox-gui/ui/src/components/ui/AxisMark.test.tsx
git commit -m "feat(vox-gui): groma axis mark (SVG master + AxisMark component)"
```

---

## Task 8: Generate raster icon set + wire Tauri + favicon

**Files:**
- Modify: `crates/vox-gui/tauri.conf.json`
- Modify: `crates/vox-gui/ui/index.html:3-7`
- Generate: `crates/vox-gui/icons/{32x32.png,128x128.png,128x128@2x.png,icon.png,icon.ico,icon.icns}`

- [ ] **Step 1: Rasterize the master into the Tauri icon set**

From `crates/vox-gui`, run the Tauri icon generator against a 1024px gold-on-basalt PNG export of `axis-mark.svg` (render the SVG to `axis-mark-1024.png` first with `sharp` or any rasterizer):
```bash
pnpm dlx @tauri-apps/cli icon ./icons/axis-mark-1024.png --output ./icons
```
Expected: writes `32x32.png`, `128x128.png`, `128x128@2x.png`, `icon.png`, `icon.ico`, `icon.icns` into `./icons`.

- [ ] **Step 2: Add the `bundle.icon` array to `tauri.conf.json`**

In `tauri.conf.json`, change the `"bundle"` object to include the icon set:
```json
  "bundle": {
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "externalBin": [
      "../../target/release/vox"
    ]
  }
```

- [ ] **Step 3: Add the web favicon to `index.html`**

Insert into `<head>` (after the `<title>`):
```html
    <link rel="icon" type="image/svg+xml" href="/axis-mark.svg" />
```

- [ ] **Step 4: Verify**

Run (from `crates/vox-gui/ui`): `pnpm build` — expected: success, `dist/axis-mark.svg` present.
Confirm `crates/vox-gui/icons/icon.ico` is a multi-resolution `.ico` (contains 16/32/48) so the taskbar/favicon render is crisp.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/icons crates/vox-gui/tauri.conf.json crates/vox-gui/ui/index.html
git commit -m "feat(vox-gui): regenerate groma icon set, wire tauri bundle.icon + favicon"
```

---

## Task 9: Hero surface — App shell (Sidebar + TopHud)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/TopHud.tsx`
- Test: existing `Sidebar.test.tsx`, `TopHud.test.tsx` must stay green.

- [ ] **Step 1: Migrate hardcoded neutrals to semantic tokens in `Sidebar.tsx`**

In the `NavItem` className (`Sidebar.tsx:34-39`), swap hardcoded colors for tokens so light works:
- `text-zinc-100` → `text-text-primary`; `text-zinc-500` → `text-text-muted`; `text-zinc-200` → `text-text-secondary`.
- `bg-white/[0.04]` → `bg-[var(--color-overlay-subtle)]`; `hover:bg-white/[0.025]` → `hover:bg-[var(--color-overlay-hover)]`.
- Keep `bg-brass`, `text-brass`, `ring-brass/30` (already token-driven via `--brass`).
- The active rail bar `shadow-[0_0_12px_2px_rgb(var(--brass)_/_0.5)]` → reduce glow per "no glow" principle: replace with a solid `bg-brass` bar, drop the shadow.
- Nav label already `font-display … tracking-[0.12em] uppercase` → now renders Cinzel automatically. No change needed.

- [ ] **Step 2: Add the brand mark to the rail/HUD**

Import and render `AxisMark` at the top of the sidebar header (replace any existing thin logo glyph):
```tsx
import { AxisMark } from '../ui/AxisMark';
// ...in the sidebar header row:
<AxisMark size={22} />
```

- [ ] **Step 3: Apply engraved treatment to the TopHud**

In `TopHud.tsx`, wrap the HUD title row container with `class="relative"` and add the rule + ticks:
```tsx
<div className="relative">
  <span className="vox-tick vox-tick-tl" />
  <span className="vox-tick vox-tick-tr" />
  {/* existing HUD title/content */}
  <div className="vox-rule mt-2" />
</div>
```
Swap any `text-zinc-*` / `bg-white/[0.0x]` in TopHud to the same semantic tokens as Step 1.

- [ ] **Step 4: Run shell tests + typecheck**

Run: `pnpm vitest run src/components/layout/Sidebar.test.tsx src/components/layout/TopHud.test.tsx && pnpm typecheck`
Expected: PASS. If a test asserts a removed class string, update the assertion to the new token class.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/layout/TopHud.tsx
git commit -m "feat(vox-gui): Roman restyle app shell (groma mark, tokenized colors, engraved HUD)"
```

---

## Task 10: Hero surface — Loquela (chat)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Transcript.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/InlineApprovals.tsx`
- Test: existing Loquela/Transcript/InlineApprovals tests must stay green.

- [ ] **Step 1: Tokenize colors across the three files**

Replace every hardcoded `text-zinc-*`, `bg-zinc-*`, `bg-white/[0.0x]`, `border-white/[0.0x]` with the semantic equivalents (`text-text-primary/secondary/muted`, `bg-bg-surface/elevated`, `bg-[var(--color-overlay-subtle/hover)]`, `border-border-subtle`). Use a grep to find them:
```bash
grep -rnE "zinc-[0-9]|white/\[" crates/vox-gui/ui/src/components/surfaces/Loquela/
```
Replace each hit with its token. Leave `brass`/`accent` classes intact.

- [ ] **Step 2: Roman section headers + speaker labels**

For transcript group/section headers and the composer label, apply `className="vox-display text-[12px] text-text-secondary"` so they render in Cinzel caps. Keep message body copy in `font-sans` (Inter) for readability — do NOT serif the body.

- [ ] **Step 3: Verdigris for positive/approved, gold for primary action**

In `InlineApprovals.tsx`, the approve/confirmed state uses `text-[var(--color-accent-secondary)]` (verdigris); the single primary "Approve" button uses solid `bg-brass text-bg-base`; secondary actions use outline (`border border-brass text-brass bg-transparent`).

- [ ] **Step 4: Run tests + typecheck**

Run: `pnpm vitest run src/components/surfaces/Loquela && pnpm typecheck`
Expected: PASS (update any class-string assertions to the new tokens).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Loquela
git commit -m "feat(vox-gui): Roman restyle Loquela (tokenized, Cinzel headers, verdigris approvals)"
```

---

## Task 11: Hero surface — Dashboard / Mesh

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/AgentRow.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/StreamCard.tsx`
- Test: existing Dashboard/AgentRow/StreamCard tests must stay green.

- [ ] **Step 1: Tokenize colors (same recipe as Task 10 Step 1)**

```bash
grep -rnE "zinc-[0-9]|white/\[" crates/vox-gui/ui/src/components/surfaces/Dashboard/
```
Replace each with the semantic token equivalent.

- [ ] **Step 2: Engraved metric cards + Cinzel numerals**

Card headers get `vox-display text-[12px] text-text-muted` + a `vox-rule` divider; the big metric value gets `font-display text-text-primary` (Cinzel numerals) while small/dense numbers stay `font-mono`. Add corner ticks (`vox-tick vox-tick-tl/-tr`) to the primary KPI card container (set it `relative`).

- [ ] **Step 3: Status semantics**

Status dots/badges map: online/pass → `--color-accent-secondary` (verdigris); active/primary → `bg-brass`; waiting/warn → `--color-status-warn` (terracotta); error/fail → `--color-status-fail`. Replace any literal greens/ambers with these.

- [ ] **Step 4: Run tests + typecheck**

Run: `pnpm vitest run src/components/surfaces/Dashboard && pnpm typecheck`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Dashboard
git commit -m "feat(vox-gui): Roman restyle Dashboard/Mesh (engraved cards, Cinzel numerals, status semantics)"
```

---

## Task 12: Draft the Claude Design handoff conventions

**Files:**
- Create: `crates/vox-gui/ui/.design-sync/conventions.md`

- [ ] **Step 1: Write the conventions draft**

Author `conventions.md` naming the REAL vocabulary so a future design agent builds on-brand. Include: the `data-theme` basalt(default)/travertine switch; the token families (`--color-bg-*`, `--color-text-*`, `--color-accent-default` gold / `--color-accent-secondary` verdigris, `--color-status-*`); the type idiom (`font-display`=Cinzel caps for headings/labels only, `font-sans`=Inter for body/data, `font-serif`=EB Garamond italic for captions, `font-mono` for code); the ornament recipes (`vox-rule`, `vox-tick-tl/-tr`, `vox-display`); and the rule that clarity/AA beats theme. Add one idiomatic snippet (a tokenized metric card). Keep it 2–4k chars.

- [ ] **Step 2: Validate every name exists**

```bash
grep -oE "color-(bg|text|border|accent|status|overlay)-[a-z]+" crates/vox-gui/ui/src/styles/tokens.generated.css | sort -u
```
Confirm each token named in `conventions.md` appears here (and travertine sheet). Fix or cut any name that doesn't resolve.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/.design-sync/conventions.md
git commit -m "docs(vox-gui): design-sync conventions draft for claude.ai/design handoff"
```

---

## Task 13: Full build, verification, and preview package

**Files:** none new (verification only).

- [ ] **Step 1: Full build + all gates**

Run (from `crates/vox-gui/ui`):
```bash
pnpm tokens:build && pnpm typecheck && pnpm vitest run && pnpm build
```
Expected: tokens build; typecheck clean; ALL vitest suites pass; vite build succeeds.

- [ ] **Step 2: Capture preview screenshots (both scopes)**

Launch the app/dev server and capture, for sign-off:
- App shell, Loquela, Dashboard — each in `basalt` and in `travertine` (toggle via `document.documentElement.dataset.theme`).
- The icon set at 16/32/128 and the taskbar/window title-bar icon.
(Use the project `run` skill or `playwright` against the dev server; place PNGs under `docs/superpowers/plans/assets/2026-06-20-roman-restyle/`.)

- [ ] **Step 3: Present for review — DO NOT MERGE**

Summarize what changed, attach the screenshots, and explicitly ask for approval before any integration to `main`. Per the spec, nothing is applied automatically.

- [ ] **Step 4: Commit the preview assets**

```bash
git add docs/superpowers/plans/assets/2026-06-20-roman-restyle
git commit -m "docs(vox-gui): Roman restyle preview screenshots (basalt + travertine)"
```

---

## Self-review notes

- **Spec coverage:** §2 type/accent → Tasks 1,2,5,9–11; §3 tokens → Tasks 2–6; §4 hero surfaces → Tasks 9–11; §5 icon → Tasks 7–8; §6 handoff → Task 12; §7 preview-first → Task 13; §1 AA clarity → Task 6 (guard). All covered.
- **Light-theme breakage risk** (hardcoded `zinc`/`white-α`) is addressed by the tokenize steps (Tasks 9–11 Step 1) + the `--color-overlay-*` token.
- **Naming consistency:** semantic slots (`color.accent.secondary`, `color.overlay.subtle/hover`, `color.status.*`) are defined in Task 3/4 and consumed verbatim in Tasks 5,9–12.
- **Non-hero surfaces** intentionally inherit global tokens only (spec §8) — follow-up pass if light reveals contrast issues there.
