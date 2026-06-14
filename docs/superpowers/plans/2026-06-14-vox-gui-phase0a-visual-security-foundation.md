# vox-gui Phase 0A — Visual & Security Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a real design-token single-source-of-truth (Style Dictionary → CSS variables + TS constants + Tailwind theme), a high-contrast theme variant, an automated WCAG-contrast test, one consolidated `cn` helper, and a strict Content-Security-Policy — the bedrock every later GUI-principles wave builds on.

**Architecture:** Token sources live in `crates/vox-gui/ui/tokens/*.json` (primitive → semantic layers). Style Dictionary compiles them at build time into `src/styles/tokens.generated.css` (CSS custom properties, one block per theme) and `src/styles/tokens.generated.ts` (typed constants). `tailwind.config.js` and `index.css` consume the generated CSS variables. A vitest test computes WCAG contrast ratios from the generated tokens and fails the build if any required text/UI pair drops below threshold. CSP is set in `tauri.conf.json` (currently `null`).

**Tech Stack:** React 19, TypeScript 5, Vite 6, Tailwind 3.4, Style Dictionary v4, vitest 2, pnpm. Tauri v2.

> **Source of truth:** spec [`docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`](../specs/2026-06-14-vox-gui-design-principles-application-design.md); principles [`docs/src/architecture/gui-frontend-design-principles-2026-06-14.md`](../../src/architecture/gui-frontend-design-principles-2026-06-14.md) (token §6.1 #247–255; color/contrast #99–110, #178–183; CSP #330–335); surface map [`docs/src/architecture/vox-gui-surface-map-2026-06-14.md`](../../src/architecture/vox-gui-surface-map-2026-06-14.md).

> **All commands run from `crates/vox-gui/ui/` unless noted.** This project uses **pnpm**, never npm (npm corrupts the pnpm store — see project memory). Tests: `pnpm test`. Typecheck: `pnpm typecheck`.

---

## Scope

**In scope (Phase 0A):** token SSOT pipeline; formalize the existing dark theme as semantic tokens; add a `high-contrast` theme; automated contrast test; consolidate the duplicated `cn` helper; strict CSP.

**Out of scope (later Phase 0 sub-plans):**
- **0B — IPC→Query:** route all direct `invoke()` through `VoxTransport`; integrate TanStack Query + shared `<Async>` wrapper; typed command boundary.
- **0C — a11y primitives:** Radix/Ark base controls under `Glass`/`Panel`; global `focus-visible` + `prefers-reduced-motion`; skeleton/error primitives.
- **0D — perf utils:** list-virtualization utility; animation conventions.

Phase 0A produces working, testable software on its own: the app still builds and runs, but now every color/space/type decision resolves through tokens, contrast is enforced, and the webview has XSS mitigation.

## File Structure

| File | Responsibility |
|------|---------------|
| `crates/vox-gui/ui/tokens/primitive.json` | Raw scale values: neutral color ramp, accent, spacing, type, radius, shadow, motion, z-index. No semantics. |
| `crates/vox-gui/ui/tokens/semantic.json` | Role tokens (`color.bg.base`, `color.text.primary`, …) referencing primitives. Dark/default theme. |
| `crates/vox-gui/ui/tokens/semantic.contrast.json` | High-contrast overrides (only the roles that change). |
| `crates/vox-gui/ui/style-dictionary.config.mjs` | Build config: emits CSS variables (per theme selector) + TS constants. |
| `crates/vox-gui/ui/src/styles/tokens.generated.css` | **Generated.** CSS custom properties under `:root` and `[data-theme="high-contrast"]`. |
| `crates/vox-gui/ui/src/styles/tokens.generated.ts` | **Generated.** Typed `tokens` object (resolved dark values) for JS consumers + the contrast test. |
| `crates/vox-gui/ui/src/lib/contrast.ts` | WCAG relative-luminance + contrast-ratio helpers (pure, testable). |
| `crates/vox-gui/ui/src/lib/contrast.test.ts` | Asserts required token pairs meet WCAG AA. |
| `crates/vox-gui/ui/src/lib/cn.ts` | Single `cn()` helper (clsx + tailwind-merge). |
| `crates/vox-gui/ui/src/lib/cn.test.ts` | Tests `cn` merge behavior. |
| `crates/vox-gui/ui/src/lib/theme.ts` | **Modify:** add `'high-contrast'` to `ThemeId` + `KNOWN`. |
| `crates/vox-gui/ui/src/lib/theme.test.ts` | **Create:** tests `normalizeTheme`. |
| `crates/vox-gui/ui/src/components/ui/Glass.tsx` | **Modify:** import shared `cn`, drop local copy. |
| `crates/vox-gui/ui/src/components/ui/Pill.tsx` | **Modify:** import shared `cn`, drop local copy. |
| `crates/vox-gui/ui/tailwind.config.js` | **Modify:** map semantic colors to CSS vars. |
| `crates/vox-gui/ui/src/index.css` | **Modify:** `@import` generated tokens; keep `--brass` accent. |
| `crates/vox-gui/ui/package.json` | **Modify:** add `style-dictionary` dev-dep + `tokens:build` script; chain into `build`. |
| `crates/vox-gui/tauri.conf.json` | **Modify:** set strict `csp`. |

---

## Task 1: Consolidate the `cn` helper (DRY)

The `cn()` helper is duplicated in `Glass.tsx` and `Pill.tsx` (surface-map community C91). Extract one copy first — later tasks and primitives import it.

**Files:**
- Create: `crates/vox-gui/ui/src/lib/cn.ts`
- Create: `crates/vox-gui/ui/src/lib/cn.test.ts`
- Modify: `crates/vox-gui/ui/src/components/ui/Glass.tsx:1-7`
- Modify: `crates/vox-gui/ui/src/components/ui/Pill.tsx` (top import + local `function cn`)

- [ ] **Step 1: Write the failing test**

Create `src/lib/cn.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { cn } from './cn';

describe('cn', () => {
  it('joins truthy class names', () => {
    expect(cn('a', 'b')).toBe('a b');
  });
  it('drops falsy values', () => {
    expect(cn('a', false, null, undefined, 'b')).toBe('a b');
  });
  it('merges conflicting tailwind classes (last wins)', () => {
    expect(cn('px-2', 'px-4')).toBe('px-4');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- src/lib/cn.test.ts`
Expected: FAIL — `Cannot find module './cn'`.

- [ ] **Step 3: Write minimal implementation**

Create `src/lib/cn.ts`:

```ts
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Merge Tailwind class names: clsx for conditionals, tailwind-merge to dedupe conflicts. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- src/lib/cn.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Refactor Glass.tsx to use the shared helper**

In `src/components/ui/Glass.tsx`, replace lines 1-7:

```tsx
import React from 'react';
import { clsx, type ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}
```

with:

```tsx
import React from 'react';
import { cn } from '../../lib/cn';
```

- [ ] **Step 6: Refactor Pill.tsx to use the shared helper**

In `src/components/ui/Pill.tsx`, delete its local `function cn(...) { return twMerge(clsx(inputs)); }` and the `clsx`/`tailwind-merge` imports, and add at the top of its imports:

```tsx
import { cn } from '../../lib/cn';
```

(Keep all other imports and the component body unchanged.)

- [ ] **Step 7: Verify typecheck + tests + no remaining local cn**

Run: `pnpm typecheck`
Expected: no errors.
Run: `grep -rn "function cn(" src` (from `crates/vox-gui/ui`)
Expected: no matches.
Run: `pnpm test -- src/lib/cn.test.ts`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/lib/cn.ts crates/vox-gui/ui/src/lib/cn.test.ts crates/vox-gui/ui/src/components/ui/Glass.tsx crates/vox-gui/ui/src/components/ui/Pill.tsx
git commit -m "refactor(vox-gui): consolidate duplicated cn() into lib/cn.ts"
```

---

## Task 2: Install Style Dictionary and author token sources

**Files:**
- Modify: `crates/vox-gui/ui/package.json` (devDependencies + scripts)
- Create: `crates/vox-gui/ui/tokens/primitive.json`
- Create: `crates/vox-gui/ui/tokens/semantic.json`
- Create: `crates/vox-gui/ui/tokens/semantic.contrast.json`

- [ ] **Step 1: Add the dev dependency**

Run: `pnpm add -D style-dictionary@^4.0.0`
Expected: `style-dictionary` appears under `devDependencies` in `package.json`; `pnpm-lock.yaml` updates.

- [ ] **Step 2: Author primitive tokens**

Create `tokens/primitive.json`. These values are chosen to satisfy the contrast test in Task 4 (text on `#09090b`: primary 19:1, secondary 13:1, muted 7.7:1; accent on surface 8.4:1).

```json
{
  "color": {
    "neutral": {
      "950": { "value": "#09090b" },
      "900": { "value": "#18181b" },
      "800": { "value": "#27272a" },
      "700": { "value": "#3f3f46" },
      "400": { "value": "#a1a1aa" },
      "300": { "value": "#d4d4d8" },
      "50":  { "value": "#fafafa" }
    },
    "brass": {
      "default": { "value": "#d4af37" }
    },
    "status": {
      "pass": { "value": "#a7f3d0" },
      "fail": { "value": "#fca5a5" },
      "warn": { "value": "#fde68a" },
      "info": { "value": "#bfdbfe" }
    }
  },
  "space": {
    "0": { "value": "0px" }, "1": { "value": "4px" }, "2": { "value": "8px" },
    "3": { "value": "12px" }, "4": { "value": "16px" }, "6": { "value": "24px" },
    "8": { "value": "32px" }, "12": { "value": "48px" }
  },
  "radius": {
    "sm": { "value": "6px" }, "md": { "value": "10px" }, "lg": { "value": "16px" }, "xl": { "value": "24px" }
  },
  "font": {
    "size": {
      "xs": { "value": "12px" }, "sm": { "value": "14px" }, "base": { "value": "16px" },
      "lg": { "value": "18px" }, "xl": { "value": "24px" }, "2xl": { "value": "32px" }
    },
    "weight": {
      "regular": { "value": "400" }, "medium": { "value": "500" }, "semibold": { "value": "600" }, "bold": { "value": "700" }
    },
    "leading": { "tight": { "value": "1.25" }, "normal": { "value": "1.5" } }
  },
  "motion": {
    "fast": { "value": "120ms" }, "base": { "value": "200ms" }, "slow": { "value": "400ms" }
  },
  "z": {
    "base": { "value": "0" }, "dropdown": { "value": "1000" }, "modal": { "value": "1300" }, "toast": { "value": "1500" }
  }
}
```

- [ ] **Step 3: Author semantic (dark/default) tokens**

Create `tokens/semantic.json` — role tokens that reference primitives:

```json
{
  "color": {
    "bg":     { "base": { "value": "{color.neutral.950}" }, "surface": { "value": "{color.neutral.900}" }, "elevated": { "value": "{color.neutral.800}" } },
    "text":   { "primary": { "value": "{color.neutral.50}" }, "secondary": { "value": "{color.neutral.300}" }, "muted": { "value": "{color.neutral.400}" } },
    "border": { "subtle": { "value": "{color.neutral.800}" }, "strong": { "value": "{color.neutral.700}" } },
    "accent": { "default": { "value": "{color.brass.default}" } }
  }
}
```

- [ ] **Step 4: Author high-contrast overrides**

Create `tokens/semantic.contrast.json` — only roles that change for high-contrast (pure-black surfaces, pure-white text, stronger borders):

```json
{
  "color": {
    "bg":     { "base": { "value": "#000000" }, "surface": { "value": "#000000" }, "elevated": { "value": "#0a0a0a" } },
    "text":   { "primary": { "value": "#ffffff" }, "secondary": { "value": "#ffffff" }, "muted": { "value": "{color.neutral.300}" } },
    "border": { "subtle": { "value": "{color.neutral.400}" }, "strong": { "value": "#ffffff" } },
    "accent": { "default": { "value": "{color.brass.default}" } }
  }
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml crates/vox-gui/ui/tokens/
git commit -m "feat(vox-gui): add Style Dictionary + design-token sources (primitive/semantic/high-contrast)"
```

---

## Task 3: Style Dictionary build → generate CSS variables + TS constants

**Files:**
- Create: `crates/vox-gui/ui/style-dictionary.config.mjs`
- Modify: `crates/vox-gui/ui/package.json` (scripts)
- Generated (do not hand-edit): `src/styles/tokens.generated.css`, `src/styles/tokens.generated.ts`

- [ ] **Step 1: Write the Style Dictionary config**

Create `style-dictionary.config.mjs`. It builds twice: the default (dark) theme into `:root`, and the high-contrast overlay into `[data-theme="high-contrast"]`. It also emits a flat TS object of the resolved **dark** values for JS consumers and the contrast test.

```js
import StyleDictionary from 'style-dictionary';

const CSS_HEADER = '/* AUTO-GENERATED by style-dictionary. Do not edit. Run `pnpm tokens:build`. */\n';

/** kebab CSS var name: color.bg.base -> --color-bg-base */
const cssVarName = (path) => `--${path.join('-')}`;

// Custom format: CSS custom properties under an arbitrary selector.
StyleDictionary.registerFormat({
  name: 'css/vars-selector',
  format: ({ dictionary, options }) => {
    const lines = dictionary.allTokens.map((t) => `  ${cssVarName(t.path)}: ${t.value};`);
    return `${CSS_HEADER}${options.selector} {\n${lines.join('\n')}\n}\n`;
  },
});

// Custom format: typed TS object of resolved values (nested by path).
StyleDictionary.registerFormat({
  name: 'ts/nested',
  format: ({ dictionary }) => {
    const root = {};
    for (const t of dictionary.allTokens) {
      let node = root;
      t.path.forEach((seg, i) => {
        if (i === t.path.length - 1) node[seg] = t.value;
        else node = (node[seg] ??= {});
      });
    }
    return `// AUTO-GENERATED by style-dictionary. Do not edit. Run \`pnpm tokens:build\`.\nexport const tokens = ${JSON.stringify(root, null, 2)} as const;\n`;
  },
});

const dark = new StyleDictionary({
  source: ['tokens/primitive.json', 'tokens/semantic.json'],
  platforms: {
    css: { transformGroup: 'css', buildPath: 'src/styles/', files: [
      { destination: 'tokens.generated.css', format: 'css/vars-selector', options: { selector: ':root' } },
    ] },
    ts: { transformGroup: 'js', buildPath: 'src/styles/', files: [
      { destination: 'tokens.generated.ts', format: 'ts/nested' },
    ] },
  },
});

const contrast = new StyleDictionary({
  source: ['tokens/primitive.json', 'tokens/semantic.json', 'tokens/semantic.contrast.json'],
  platforms: {
    css: { transformGroup: 'css', buildPath: 'src/styles/', files: [
      { destination: 'tokens.contrast.generated.css', format: 'css/vars-selector', options: { selector: '[data-theme="high-contrast"]' } },
    ] },
  },
});

await dark.buildAllPlatforms();
await contrast.buildAllPlatforms();
console.log('tokens built: tokens.generated.css, tokens.contrast.generated.css, tokens.generated.ts');
```

- [ ] **Step 2: Add the build scripts**

In `package.json` `"scripts"`, add `tokens:build` and chain it before `build`:

```json
"tokens:build": "node style-dictionary.config.mjs",
"build": "pnpm tokens:build && tsc --noEmit && vite build",
```

(Replace the existing `"build"` line; leave `dev`, `typecheck`, `test`, `test:e2e`, `lint` as-is.)

- [ ] **Step 3: Run the build**

Run: `pnpm tokens:build`
Expected: prints `tokens built: …`; creates `src/styles/tokens.generated.css`, `src/styles/tokens.contrast.generated.css`, `src/styles/tokens.generated.ts`.

- [ ] **Step 4: Verify the generated output shape**

Run: `grep -- "--color-bg-base" src/styles/tokens.generated.css`
Expected: `  --color-bg-base: #09090b;`
Run: `grep -- "--color-text-primary" src/styles/tokens.contrast.generated.css`
Expected: `  --color-text-primary: #ffffff;`

- [ ] **Step 5: Mark generated files in .gitignore decision**

These files ARE committed (the app imports them and the contrast test reads them). Confirm they are not ignored:
Run: `git check-ignore src/styles/tokens.generated.ts || echo "tracked"`
Expected: `tracked`.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/style-dictionary.config.mjs crates/vox-gui/ui/package.json crates/vox-gui/ui/src/styles/tokens.generated.css crates/vox-gui/ui/src/styles/tokens.contrast.generated.css crates/vox-gui/ui/src/styles/tokens.generated.ts
git commit -m "feat(vox-gui): compile design tokens to CSS vars + TS constants via Style Dictionary"
```

---

## Task 4: Automated WCAG contrast test

Enforces principles #178–183 with vitest only (no new a11y dep). The test reads the generated dark tokens and asserts required pairs meet AA.

**Files:**
- Create: `crates/vox-gui/ui/src/lib/contrast.ts`
- Create: `crates/vox-gui/ui/src/lib/contrast.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/lib/contrast.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { contrastRatio } from './contrast';
import { tokens } from '../styles/tokens.generated';

const bg = tokens.color.bg.base;       // #09090b
const surface = tokens.color.bg.surface;

describe('contrastRatio', () => {
  it('computes ~21:1 for black on white', () => {
    expect(contrastRatio('#000000', '#ffffff')).toBeCloseTo(21, 0);
  });
  it('computes 1:1 for identical colors', () => {
    expect(contrastRatio('#445566', '#445566')).toBeCloseTo(1, 5);
  });
});

describe('token pairs meet WCAG AA', () => {
  it('text.primary on bg.base >= 4.5', () => {
    expect(contrastRatio(tokens.color.text.primary, bg)).toBeGreaterThanOrEqual(4.5);
  });
  it('text.secondary on bg.base >= 4.5', () => {
    expect(contrastRatio(tokens.color.text.secondary, bg)).toBeGreaterThanOrEqual(4.5);
  });
  it('text.muted on bg.base >= 4.5', () => {
    expect(contrastRatio(tokens.color.text.muted, bg)).toBeGreaterThanOrEqual(4.5);
  });
  it('accent.default on bg.surface >= 3 (UI component)', () => {
    expect(contrastRatio(tokens.color.accent.default, surface)).toBeGreaterThanOrEqual(3);
  });
  it('every status color on bg.surface >= 3', () => {
    for (const c of Object.values(tokens.color.status)) {
      expect(contrastRatio(c, surface)).toBeGreaterThanOrEqual(3);
    }
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- src/lib/contrast.test.ts`
Expected: FAIL — `Cannot find module './contrast'`.

- [ ] **Step 3: Implement the contrast helpers**

Create `src/lib/contrast.ts`:

```ts
/** WCAG 2.x relative luminance + contrast ratio for #rrggbb hex colors. */

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  const full = h.length === 3 ? h.split('').map((c) => c + c).join('') : h;
  const n = parseInt(full, 16);
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}

function channelLuminance(c8: number): number {
  const c = c8 / 255;
  return c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
}

/** WCAG relative luminance (0=black, 1=white). */
export function relativeLuminance(hex: string): number {
  const [r, g, b] = hexToRgb(hex);
  return 0.2126 * channelLuminance(r) + 0.7152 * channelLuminance(g) + 0.0722 * channelLuminance(b);
}

/** WCAG contrast ratio between two hex colors (1..21). Order-independent. */
export function contrastRatio(a: string, b: string): number {
  const la = relativeLuminance(a);
  const lb = relativeLuminance(b);
  const [hi, lo] = la >= lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- src/lib/contrast.test.ts`
Expected: PASS (all pairs ≥ threshold). If any token-pair assertion fails, the chosen token value is non-compliant — fix the value in `tokens/semantic.json` (lighten the text role) and re-run `pnpm tokens:build` before re-testing. Do not weaken the threshold.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/contrast.ts crates/vox-gui/ui/src/lib/contrast.test.ts
git commit -m "test(vox-gui): enforce WCAG AA contrast on design tokens (vitest)"
```

---

## Task 5: Add the high-contrast theme to the theme module

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/theme.ts:11-13`
- Create: `crates/vox-gui/ui/src/lib/theme.test.ts`

- [ ] **Step 1: Write the failing test**

Create `src/lib/theme.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { normalizeTheme } from './theme';

describe('normalizeTheme', () => {
  it('keeps known accent themes', () => {
    expect(normalizeTheme('void')).toBe('void');
    expect(normalizeTheme('glacier')).toBe('glacier');
  });
  it('accepts high-contrast', () => {
    expect(normalizeTheme('high-contrast')).toBe('high-contrast');
  });
  it('defaults unknown/empty to arcane', () => {
    expect(normalizeTheme('nope')).toBe('arcane');
    expect(normalizeTheme(null)).toBe('arcane');
    expect(normalizeTheme(undefined)).toBe('arcane');
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm test -- src/lib/theme.test.ts`
Expected: FAIL — `normalizeTheme('high-contrast')` returns `'arcane'` (not yet known).

- [ ] **Step 3: Add `high-contrast` to the type and known set**

In `src/lib/theme.ts`, change line 11:

```ts
export type ThemeId = 'arcane' | 'void' | 'glacier' | 'high-contrast';
```

and line 13:

```ts
const KNOWN: ReadonlySet<string> = new Set(['arcane', 'void', 'glacier', 'high-contrast']);
```

(Leave `normalizeTheme` and `applyTheme` bodies unchanged — they already key off `KNOWN`.)

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm test -- src/lib/theme.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/theme.ts crates/vox-gui/ui/src/lib/theme.test.ts
git commit -m "feat(vox-gui): register high-contrast theme id"
```

---

## Task 6: Wire generated tokens into index.css and Tailwind

**Files:**
- Modify: `crates/vox-gui/ui/src/index.css:1-6` (add imports)
- Modify: `crates/vox-gui/ui/tailwind.config.js:4-15` (map semantic colors)

- [ ] **Step 1: Import the generated token CSS**

In `src/index.css`, after the existing `@import url('https://fonts.googleapis.com/...')` line and before `@tailwind base;`, add:

```css
@import './styles/tokens.generated.css';
@import './styles/tokens.contrast.generated.css';
```

(Keep the existing `--brass` accent definitions in `@layer base` — the accent system is orthogonal and still drives `void`/`glacier`/`arcane`.)

- [ ] **Step 2: Map semantic tokens into the Tailwind theme**

In `tailwind.config.js`, extend the `colors` block (lines 4-15) to expose the semantic CSS vars as Tailwind utilities, keeping the existing keys:

```js
    extend: {
      colors: {
        void: '#09090b',
        steel: '#71717a',
        brass: 'rgb(var(--brass) / <alpha-value>)',
        "amber-glow": 'rgb(var(--brass) / 0.5)',
        border: 'rgba(255,255,255,0.06)',
        background: '#09090b',
        primary: 'rgb(var(--brass) / <alpha-value>)',
        // Semantic tokens (Style Dictionary → tokens.generated.css):
        'bg-base': 'var(--color-bg-base)',
        'bg-surface': 'var(--color-bg-surface)',
        'bg-elevated': 'var(--color-bg-elevated)',
        'text-primary': 'var(--color-text-primary)',
        'text-secondary': 'var(--color-text-secondary)',
        'text-muted': 'var(--color-text-muted)',
        'border-subtle': 'var(--color-border-subtle)',
        'border-strong': 'var(--color-border-strong)',
        'accent': 'var(--color-accent-default)',
      },
```

(Leave `fontFamily`, `animation`, `keyframes` unchanged.)

- [ ] **Step 3: Verify the build still compiles**

Run: `pnpm tokens:build && pnpm typecheck`
Expected: no errors.
Run: `pnpm build`
Expected: build succeeds (`tokens:build` runs first, then `tsc`, then `vite build`).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/index.css crates/vox-gui/ui/tailwind.config.js
git commit -m "feat(vox-gui): consume design tokens in index.css + tailwind theme"
```

---

## Task 7: Set a strict Content-Security-Policy

Currently `"csp": null` → no XSS mitigation (#330/#358). Tauri auto-injects nonces/hashes for bundled assets at compile time; we configure only app-specific sources. The app loads Google Fonts (`index.css` `@import`), uses the Tauri asset protocol, and talks over IPC.

**Files:**
- Modify: `crates/vox-gui/tauri.conf.json:20-22`

- [ ] **Step 1: Replace the null CSP**

In `tauri.conf.json`, change the `security` block (lines 20-22):

```json
    "security": {
      "csp": "default-src 'self'; img-src 'self' asset: http://asset.localhost data: blob:; font-src 'self' https://fonts.gstatic.com; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; script-src 'self'; connect-src 'self' ipc: http://ipc.localhost"
    }
```

> **Note on `'unsafe-inline'` for styles:** the codebase currently has 53 inline `style={{}}` usages and Tailwind injects inline styles, so `style-src 'unsafe-inline'` is required for now. `script-src` deliberately omits `'unsafe-inline'` — Tauri injects per-script nonces/hashes, so bundled scripts still load. Removing the style `'unsafe-inline'` is tracked as later debt (after inline styles are migrated to tokens/classes).

- [ ] **Step 2: Verify the config parses**

Run (from repo root `C:/Users/Owner/vox`): `node -e "JSON.parse(require('fs').readFileSync('crates/vox-gui/tauri.conf.json','utf8')); console.log('valid json')"`
Expected: `valid json`.

- [ ] **Step 3: Verify the desktop app still loads (manual)**

Run (from `crates/vox-gui`): `pnpm --dir ui dev` in one terminal, then `cargo tauri dev` (or the project's run skill) and confirm: the window renders, fonts load, no CSP violation errors in the webview devtools console (`Refused to … because it violates the … Content Security Policy`). If fonts are blocked, confirm `fonts.googleapis.com`/`fonts.gstatic.com` are present in the CSP; if IPC calls fail, confirm `ipc:`/`http://ipc.localhost` are present.

> If you cannot launch the desktop shell in this environment, document that Step 3 is deferred to a machine that can run `cargo tauri dev`, and do NOT mark the task complete until a human confirms the app loads. Never claim it works unverified.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/tauri.conf.json
git commit -m "fix(vox-gui): set strict CSP (was null — no XSS mitigation)"
```

---

## Task 8: Full Phase 0A verification gate

**Files:** none (verification only).

- [ ] **Step 1: Typecheck**

Run: `pnpm typecheck`
Expected: no errors.

- [ ] **Step 2: Full unit test suite**

Run: `pnpm test`
Expected: all suites pass, including `cn.test.ts`, `contrast.test.ts`, `theme.test.ts`, and the pre-existing 37 test files.

- [ ] **Step 3: Production build (token build + tsc + vite)**

Run: `pnpm build`
Expected: succeeds end-to-end.

- [ ] **Step 4: e2e smoke (if runnable in this environment)**

Run: `pnpm test:e2e`
Expected: existing Playwright specs (dashboard, dock-layout, browser, screenshots) pass. If the environment cannot run Playwright, record that and defer to CI — do not claim a pass you did not observe.

- [ ] **Step 5: Repo gate**

Run (from repo root): the project gate for the touched crate, e.g. `vox ci` per AGENTS.md (NOT `vox -- ci` — see project memory). Confirm green.

- [ ] **Step 6: Final commit (if any verification fixups were needed)**

```bash
git add -A
git commit -m "chore(vox-gui): Phase 0A visual+security foundation green"
```

---

## Subsequent Phase 0 sub-plans (to be written next, not part of 0A)

These are **not** tasks in this plan — they are the remaining foundations, each warranting its own plan document so each stays within one execution session:

- **0B — IPC → TanStack Query** (`docs/superpowers/plans/2026-06-14-vox-gui-phase0b-ipc-query.md`): wrap every direct `invoke()` in surfaces (App, Browser, Gamify, Loquela, Matrix, Memory, Models, Search, Settings, Tasks, CommandPalette, DockShell + `hooks/usePersistedDbState.ts`, `lib/consoleBridge.ts`) into `VoxTransport` methods; add TanStack Query provider + query/mutation hooks; build the shared `<Async>` wrapper (idle/loading/empty/error/success). Decide ts-rs vs hand-maintained boundary types (no ts-rs present today).
- **0C — a11y primitives** (`…-phase0c-a11y-primitives.md`): adopt Radix or Ark headless controls under `Glass`/`Panel`; build accessible `Button`/`Dialog`/`Menu`/`Tabs`/`Select`/`Tooltip`; add global `:focus-visible` ring (using `--color-accent-default`, ≥3:1) and a `prefers-reduced-motion` media block that disables the `vox-*` animations; standardize a loading-skeleton + error-boundary primitive alongside the existing `EmptyState`.
- **0D — perf utilities** (`…-phase0d-perf.md`): list-virtualization utility for the long data views (Tasks, Runs, Search, Memory); document RAIL budgets + the "heavy work → Rust command" and "animate transform/opacity only" conventions.

After Phase 0 (0A–0D) lands, the per-surface waves (1–6 in the spec) apply the canonical per-surface checklist, inheriting tokens, Query, a11y primitives, and perf utils.

---

## Self-Review

**Spec coverage (0A scope only):** token SSOT pipeline ✅ (Tasks 2–3, 6); semantic token layers ✅ (Task 2); light/dark/high-contrast → **dark + high-contrast** ✅ (Tasks 2,5; full light theme explicitly out of scope, noted); contrast enforcement ✅ (Task 4); CSP ✅ (Task 7); `cn` consolidation ✅ (Task 1); type/spacing scale codified ✅ (Task 2 `space`/`font`). IPC→Query, Radix primitives, focus-visible/reduced-motion, virtualization → deferred to 0B/0C/0D (explicit, with file names).

**Placeholder scan:** every code step shows complete code; every command shows expected output; no "TBD"/"similar to"/"add error handling" left. The manual CSP verification (Task 7 Step 3) and e2e (Task 8 Step 4) include an explicit "do not claim unverified" guard rather than a vague instruction.

**Type/name consistency:** `cn` (Task 1) imported by Glass/Pill; `tokens` object (Task 3 `ts/nested`) consumed by `contrast.test.ts` (Task 4) as `tokens.color.bg.base` etc. — matches the nested shape emitted from `semantic.json` paths (`color.bg.base`); CSS var naming `--color-bg-base` (Task 3 `cssVarName`) matches Tailwind `var(--color-bg-base)` (Task 6) and the grep checks; `ThemeId`/`KNOWN` both updated together (Task 5). Generated filenames `tokens.generated.css` / `tokens.contrast.generated.css` / `tokens.generated.ts` are consistent across Tasks 3, 4, 6.

**Risk note:** Style Dictionary v4 ESM API (`new StyleDictionary(...)`, `buildAllPlatforms()`, `registerFormat`) is assumed; if the installed version's format API differs, adjust the two `registerFormat` calls — the token sources and downstream consumers are unaffected.
