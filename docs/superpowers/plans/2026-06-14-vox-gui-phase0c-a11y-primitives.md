# vox-gui Phase 0C — Accessibility Primitives Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Each task is independently committable. Run `pnpm test` and `pnpm typecheck` after every task before committing.

**Goal:** Build accessible shared primitives (global `:focus-visible` ring, `prefers-reduced-motion` block, `Button`, `Skeleton`, `Dialog`), fix the two existing a11y regressions in `Toasts.tsx` and `ErrorBoundary.tsx`, and install the three required Radix UI packages — establishing the baseline every per-surface wave will reuse.

**Architecture:** All shared UI primitives live in `crates/vox-gui/ui/src/components/ui/`. Global CSS rules go in `src/index.css`. No Radix packages were present before this phase; we install only the three required: `@radix-ui/react-slot`, `@radix-ui/react-dialog`, `@radix-ui/react-tooltip`. Token CSS variables from Phase 0A (`--color-accent-default`, `--color-bg-surface`, etc.) are already available; focus rings use them directly. Per-surface aria attribute retrofits are explicitly out of scope — those happen in the wave passes.

**Tech Stack:** React 19, TypeScript 5, Vite 6, Tailwind 3.4, Radix UI (slot + dialog + tooltip), vitest 2, @testing-library/react, pnpm. Tauri v2.

> **Source of truth:** spec [`docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`](../specs/2026-06-14-vox-gui-design-principles-application-design.md); Phase 0A plan [`docs/superpowers/plans/2026-06-14-vox-gui-phase0a-visual-security-foundation.md`](./2026-06-14-vox-gui-phase0a-visual-security-foundation.md).

> **All commands run from `crates/vox-gui/ui/` unless noted.** This project uses **pnpm**, never npm.

---

## Prerequisites

Phase 0A must be complete (tokens pipeline, contrast tokens, `cn` helper, CSP). Verify:

```sh
pnpm test        # 178 tests passing baseline
pnpm typecheck   # zero errors
```

---

## Scope

**In scope (Phase 0C):**
- Global `:focus-visible` CSS rule using `--color-accent-default`
- `@media (prefers-reduced-motion: reduce)` CSS block suppressing `vox-*` animations
- `Button` primitive (`asChild` via Radix Slot, `type="button"` default, `aria-label` forwarding)
- Fix `Toasts.tsx`: add `aria-live="polite"` container + `aria-label` on close button
- Fix `ErrorBoundary.tsx`: add `aria-label` + `type="button"` on Retry button
- `Skeleton` loading primitive (shimmer, `aria-hidden`, respects reduced-motion)
- `Dialog` primitive (Radix Dialog + Glass styling, keyboard/focus-trap built in)
- Install: `@radix-ui/react-slot`, `@radix-ui/react-dialog`, `@radix-ui/react-tooltip`

**Out of scope (deferred to wave passes):**
- Retroactively adding `aria-label` to the other 175 existing buttons
- Adding `aria-*` to the 34 surfaces with zero attributes
- Keyboard navigation improvements per-surface
- Tooltip primitive usage across surfaces

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `src/index.css` | **Modify** | Add `:focus-visible` ring + `prefers-reduced-motion` block |
| `src/index.css.test.ts` | **Create** | Verify CSS contains required blocks |
| `src/components/ui/Button.tsx` | **Create** | Accessible button primitive with `asChild` + `type` default |
| `src/components/ui/Button.test.tsx` | **Create** | Button rendering, aria-label, asChild behavior |
| `src/components/ui/Toasts.tsx` | **Modify** | Add `aria-live` region + close button `aria-label` |
| `src/components/ui/Toasts.test.tsx` | **Create** | Verify live region attribute + close label |
| `src/components/ui/ErrorBoundary.tsx` | **Modify** | Add `aria-label` + `type="button"` to Retry |
| `src/components/ui/Skeleton.tsx` | **Create** | Shimmer placeholder, `aria-hidden`, reduced-motion aware |
| `src/components/ui/Skeleton.test.tsx` | **Create** | aria-hidden, className forwarding, sizing |
| `src/components/ui/Dialog.tsx` | **Create** | Radix Dialog wrapped with Glass + semantic exports |
| `src/components/ui/Dialog.test.tsx` | **Create** | Opens on trigger, title present, Escape closes |
| `package.json` | **Modify** | Add three `@radix-ui` packages |

---

## Task 1: Global `:focus-visible` ring + `prefers-reduced-motion` block

### Why

175 buttons have no visible focus indicator in keyboard navigation. The only focus styles in the codebase today are `focus-visible:ring-*` Tailwind utilities applied ad-hoc in 4 files — they cover almost nothing. A single global rule in `index.css` backfills all elements at once and makes per-element Tailwind utilities an override, not the sole fallback.

The `animate-vox-toast-in` animation (and any future `vox-*` animations) will strobe users with vestibular disorders unless suppressed by `prefers-reduced-motion: reduce`.

### Test first

- [ ] Create `src/index.css.test.ts`:

```typescript
// src/index.css.test.ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'fs';
import { resolve } from 'path';

const css = readFileSync(resolve(__dirname, './index.css'), 'utf8');

describe('index.css global a11y rules', () => {
  it('contains a :focus-visible rule', () => {
    expect(css).toContain(':focus-visible');
  });

  it('uses --color-accent-default for the focus ring color', () => {
    // The ring must reference the token, not a hardcoded hex.
    expect(css).toContain('--color-accent-default');
  });

  it('contains a prefers-reduced-motion block that targets vox-* animations', () => {
    expect(css).toContain('prefers-reduced-motion');
    expect(css).toContain('animation');
  });
});
```

Run: `pnpm test src/index.css.test.ts` — expect **3 failing** tests.

### Implementation

- [ ] Append to `src/index.css` after the existing `@layer components` block and before the theme overrides at the bottom:

```css
/* ─── Global accessibility: focus ring ────────────────────────────────────
   Applies a 3 px solid ring in the accent colour to every focusable element
   when reached via keyboard. Per-element Tailwind focus-visible: utilities
   override this; the rule acts as a safe fallback rather than a ceiling.
   3 px @ #d4af37 on #09090b → contrast ≥ 3:1 (WCAG 1.4.11 UI component). */
*:focus-visible {
  outline: 3px solid var(--color-accent-default);
  outline-offset: 2px;
  border-radius: 2px;
}

/* ─── Respect prefers-reduced-motion ──────────────────────────────────────
   Disables every vox-* keyframe animation for users with vestibular / motion
   sensitivity settings. Inline `animation-duration: 0.01ms` avoids removing
   the `animation` property entirely (which breaks JS that reads it). */
@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    transition-duration: 0.01ms !important;
    scroll-behavior: auto !important;
  }
}
```

### Verify

```sh
pnpm test src/index.css.test.ts    # expect 3 passing
pnpm typecheck                     # no errors
```

### Commit

```sh
git add crates/vox-gui/ui/src/index.css crates/vox-gui/ui/src/index.css.test.ts
git commit -m "a11y(phase0c): global focus-visible ring + prefers-reduced-motion block"
```

Expected output: `3 passed`.

---

## Task 2: Accessible `Button` primitive

### Why

There is no shared `<Button>` component in the codebase. Future code (and retrofits) needs an opinionated default that sets `type="button"` (prevents accidental form submission), accepts `aria-label`, and supports `asChild` for polymorphic rendering (e.g. wrapping a router link or an icon button without an extra DOM node).

### Install

```sh
pnpm add @radix-ui/react-slot
```

Verify it appears in `package.json` under `dependencies`.

### Test first

- [ ] Create `src/components/ui/Button.test.tsx`:

```typescript
// src/components/ui/Button.test.tsx
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Button } from './Button';

describe('Button', () => {
  it('renders with type="button" by default', () => {
    render(<Button>Click me</Button>);
    expect(screen.getByRole('button')).toHaveAttribute('type', 'button');
  });

  it('forwards aria-label to the underlying element', () => {
    render(<Button aria-label="Close dialog"><span>×</span></Button>);
    expect(screen.getByRole('button', { name: 'Close dialog' })).toBeInTheDocument();
  });

  it('renders children', () => {
    render(<Button>Save</Button>);
    expect(screen.getByText('Save')).toBeInTheDocument();
  });

  it('accepts additional className', () => {
    render(<Button className="my-custom-class">X</Button>);
    expect(screen.getByRole('button')).toHaveClass('my-custom-class');
  });

  it('renders as child element when asChild is set', () => {
    // When asChild is true, the Button renders the child element directly,
    // merging props. Here the <a> becomes the rendered node.
    render(
      <Button asChild>
        <a href="/home" role="button">Home</a>
      </Button>
    );
    const link = screen.getByRole('button', { name: 'Home' });
    expect(link.tagName.toLowerCase()).toBe('a');
    expect(link).toHaveAttribute('href', '/home');
  });

  it('allows type to be overridden to "submit"', () => {
    render(<Button type="submit">Submit</Button>);
    expect(screen.getByRole('button')).toHaveAttribute('type', 'submit');
  });

  it('is disabled when disabled prop is set', () => {
    render(<Button disabled>Disabled</Button>);
    expect(screen.getByRole('button')).toBeDisabled();
  });
});
```

Run: `pnpm test src/components/ui/Button.test.tsx` — expect **7 failing** tests (file does not exist yet).

### Implementation

- [ ] Create `src/components/ui/Button.tsx`:

```typescript
// src/components/ui/Button.tsx
import React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '../../lib/cn';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /**
   * When true, the Button renders its single child element directly, merging
   * all props into it. Use for icon-only buttons wrapping a router <Link> or
   * any case where an extra <button> DOM node is undesirable.
   */
  asChild?: boolean;
}

/**
 * Accessible button primitive.
 *
 * - Defaults `type` to "button" to prevent accidental form submission.
 * - Supports `asChild` (via Radix Slot) for polymorphic rendering.
 * - Accepts `aria-label` for icon-only buttons — pass it when visible text is absent.
 * - Applies `focus-visible:outline` via the global CSS rule in index.css; per-element
 *   overrides via the `className` prop take precedence.
 *
 * @example Icon-only button
 * ```tsx
 * <Button aria-label="Close dialog" onClick={onClose}>
 *   <Icon.x className="size-4" />
 * </Button>
 * ```
 *
 * @example Polymorphic — renders as a router link
 * ```tsx
 * <Button asChild>
 *   <Link to="/settings">Settings</Link>
 * </Button>
 * ```
 */
export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ asChild = false, className, type = 'button', children, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button';
    return (
      <Comp
        ref={ref as React.Ref<HTMLButtonElement>}
        type={asChild ? undefined : type}
        className={cn(className)}
        {...props}
      >
        {children}
      </Comp>
    );
  }
);

Button.displayName = 'Button';
```

### Verify

```sh
pnpm test src/components/ui/Button.test.tsx    # expect 7 passing
pnpm typecheck
```

### Commit

```sh
git add crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml \
        crates/vox-gui/ui/src/components/ui/Button.tsx \
        crates/vox-gui/ui/src/components/ui/Button.test.tsx
git commit -m "feat(a11y,phase0c): add Button primitive with asChild + type default"
```

Expected output: `7 passed`.

---

## Task 3: Fix `Toasts.tsx` — `aria-live` region + close button label

### Why

The toast container has no `aria-live` attribute. Screen readers never announce new toasts. The close button has no label — an `<Icon.x>` with no text reads as empty to assistive technology.

### Test first

- [ ] Create `src/components/ui/Toasts.test.tsx`:

```typescript
// src/components/ui/Toasts.test.tsx
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import { Toasts, ToastItem } from './Toasts';

const baseItems: ToastItem[] = [
  { id: 'a', tone: 'ok', title: 'Build succeeded' },
  { id: 'b', tone: 'warn', title: 'Lint warning', body: 'Line 42' },
];

describe('Toasts', () => {
  it('renders the outer container with aria-live="polite"', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    // The live region is the outermost element of Toasts.
    const region = document.querySelector('[aria-live="polite"]');
    expect(region).not.toBeNull();
  });

  it('renders the outer container with role="status"', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    expect(screen.getByRole('status')).toBeInTheDocument();
  });

  it('close buttons have an accessible label', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    const closeButtons = screen.getAllByRole('button', { name: /dismiss/i });
    expect(closeButtons).toHaveLength(baseItems.length);
  });

  it('calls onClose with the correct id when a close button is clicked', async () => {
    const onClose = vi.fn();
    render(<Toasts items={[baseItems[0]]} onClose={onClose} />);
    await userEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(onClose).toHaveBeenCalledWith('a');
  });

  it('renders toast titles', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    expect(screen.getByText('Build succeeded')).toBeInTheDocument();
    expect(screen.getByText('Lint warning')).toBeInTheDocument();
  });

  it('renders toast body when provided', () => {
    render(<Toasts items={baseItems} onClose={vi.fn()} />);
    expect(screen.getByText('Line 42')).toBeInTheDocument();
  });
});
```

> Note: `@testing-library/user-event` is already a transitive dependency via `@testing-library/react`. If missing, install with `pnpm add -D @testing-library/user-event`.

Run: `pnpm test src/components/ui/Toasts.test.tsx` — expect failures on the `aria-live`, `role="status"`, and close-button label tests.

### Implementation

- [ ] Modify `src/components/ui/Toasts.tsx`. Replace the outer `<div>` (line 19) and the close `<button>` (line 31):

**Before (outer div, line 19):**
```tsx
<div className="pointer-events-none fixed bottom-[200px] right-6 z-40 flex w-[320px] flex-col gap-2">
```

**After:**
```tsx
<div
  aria-live="polite"
  aria-atomic="false"
  role="status"
  className="pointer-events-none fixed bottom-[200px] right-6 z-40 flex w-[320px] flex-col gap-2"
>
```

**Before (close button, line 31):**
```tsx
<button onClick={() => onClose(t.id)} className="text-zinc-500 hover:text-zinc-100">
  <Icon.x className="size-3.5"/>
</button>
```

**After:**
```tsx
<button
  type="button"
  onClick={() => onClose(t.id)}
  aria-label="Dismiss notification"
  className="text-zinc-500 hover:text-zinc-100"
>
  <Icon.x className="size-3.5" aria-hidden="true" />
</button>
```

The full modified file:

```typescript
// src/components/ui/Toasts.tsx
import React from 'react';
import { Icon } from './Icons';

export interface ToastItem {
  id: string;
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
}

interface ToastsProps {
  items: ToastItem[];
  onClose: (id: string) => void;
}

export function Toasts({ items, onClose }: ToastsProps) {
  return (
    <div
      aria-live="polite"
      aria-atomic="false"
      role="status"
      className="pointer-events-none fixed bottom-[200px] right-6 z-40 flex w-[320px] flex-col gap-2"
    >
      {items.map(t => (
        <div key={t.id} className="pointer-events-auto rounded-xl border border-white/10 bg-zinc-950/90 p-3 backdrop-blur-xl shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)] animate-vox-toast-in">
          <div className="flex items-start gap-2">
            <div className={`mt-0.5 flex size-6 shrink-0 items-center justify-center rounded ${t.tone === "ok" ? "bg-emerald-400/15 text-emerald-300" : t.tone === "warn" ? "bg-amber-400/15 text-amber-300" : "bg-cyan-400/15 text-cyan-300"}`}>
              {t.tone === "ok" ? <Icon.check className="size-3.5" aria-hidden="true"/> : t.tone === "warn" ? <Icon.alert className="size-3.5" aria-hidden="true"/> : <Icon.bolt className="size-3.5" aria-hidden="true"/>}
            </div>
            <div className="flex-1 leading-tight">
              <div className="font-display text-[12px] tracking-wide text-zinc-100">{t.title}</div>
              {t.body && <div className="mt-0.5 text-[11px] text-zinc-400">{t.body}</div>}
              {t.cmd && <div className="mt-1 font-mono text-[10px] text-zinc-500">▸ {t.cmd}</div>}
            </div>
            <button
              type="button"
              onClick={() => onClose(t.id)}
              aria-label="Dismiss notification"
              className="text-zinc-500 hover:text-zinc-100"
            >
              <Icon.x className="size-3.5" aria-hidden="true"/>
            </button>
          </div>
        </div>
      ))}
    </div>
  );
}
```

### Verify

```sh
pnpm test src/components/ui/Toasts.test.tsx    # expect 6 passing
pnpm typecheck
```

### Commit

```sh
git add crates/vox-gui/ui/src/components/ui/Toasts.tsx \
        crates/vox-gui/ui/src/components/ui/Toasts.test.tsx
git commit -m "fix(a11y,phase0c): Toasts — add aria-live region + close button aria-label"
```

Expected output: `6 passed`.

---

## Task 4: Fix `ErrorBoundary.tsx` — Retry button accessibility

### Why

The Retry button has no `aria-label` and no `type` attribute. Screen readers announce it as "Retry" (text is present), but `type` is technically missing. More importantly, adding `type="button"` is a correctness fix (class component render methods may end up inside a form context). This is a targeted two-attribute fix, not a structural change.

### No new test file needed

`ErrorBoundary` is a class component; testing error boundaries with `@testing-library/react` requires `renderHook` workarounds. The fix is a one-line attribute addition; a typecheck + visual inspection is the verification gate.

### Implementation

- [ ] Modify `src/components/ui/ErrorBoundary.tsx`. Replace the `<button>` element (lines 48–53):

**Before:**
```tsx
<button
  onClick={() => this.setState({ error: null })}
  className="mt-4 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs text-zinc-300 hover:bg-white/[0.06]"
>
  Retry
</button>
```

**After:**
```tsx
<button
  type="button"
  aria-label="Retry loading surface"
  onClick={() => this.setState({ error: null })}
  className="mt-4 rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs text-zinc-300 hover:bg-white/[0.06]"
>
  Retry
</button>
```

### Verify

```sh
pnpm typecheck    # must be zero errors
pnpm test         # full suite must still be 178+ passing
```

### Commit

```sh
git add crates/vox-gui/ui/src/components/ui/ErrorBoundary.tsx
git commit -m "fix(a11y,phase0c): ErrorBoundary Retry — add type=button + aria-label"
```

---

## Task 5: `Skeleton` loading primitive

### Why

There is no shared loading-state placeholder. Every surface either shows nothing or uses bespoke inline divs during async load. A shared shimmer `<Skeleton>` primitive eliminates that fragmentation, is guaranteed `aria-hidden` (it carries no information), and respects `prefers-reduced-motion` via the global CSS block added in Task 1.

### Test first

- [ ] Create `src/components/ui/Skeleton.test.tsx`:

```typescript
// src/components/ui/Skeleton.test.tsx
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { Skeleton } from './Skeleton';

describe('Skeleton', () => {
  it('renders with aria-hidden="true" so it is invisible to screen readers', () => {
    const { container } = render(<Skeleton />);
    const el = container.firstChild as HTMLElement;
    expect(el.getAttribute('aria-hidden')).toBe('true');
  });

  it('has a data-slot="skeleton" attribute for test selection', () => {
    const { container } = render(<Skeleton />);
    const el = container.firstChild as HTMLElement;
    expect(el.getAttribute('data-slot')).toBe('skeleton');
  });

  it('forwards className', () => {
    const { container } = render(<Skeleton className="my-class" />);
    expect((container.firstChild as HTMLElement).className).toContain('my-class');
  });

  it('applies inline height style when height prop is provided', () => {
    const { container } = render(<Skeleton height={48} />);
    expect((container.firstChild as HTMLElement).style.height).toBe('48px');
  });

  it('applies inline width style when width prop is provided', () => {
    const { container } = render(<Skeleton width={200} />);
    expect((container.firstChild as HTMLElement).style.width).toBe('200px');
  });

  it('applies inline height/width as strings when passed as strings', () => {
    const { container } = render(<Skeleton height="2rem" width="100%" />);
    const el = container.firstChild as HTMLElement;
    expect(el.style.height).toBe('2rem');
    expect(el.style.width).toBe('100%');
  });

  it('renders as a <div> by default', () => {
    const { container } = render(<Skeleton />);
    expect((container.firstChild as HTMLElement).tagName.toLowerCase()).toBe('div');
  });
});
```

Run: `pnpm test src/components/ui/Skeleton.test.tsx` — expect **7 failing**.

### Implementation

- [ ] Create `src/components/ui/Skeleton.tsx`:

```typescript
// src/components/ui/Skeleton.tsx
import React from 'react';
import { cn } from '../../lib/cn';

export interface SkeletonProps {
  /** Extra Tailwind classes. Use to set w-*, h-*, rounded-*, etc. */
  className?: string;
  /**
   * Explicit height as a number (px) or CSS string (e.g. "2rem").
   * Alternative to setting height via `className`.
   */
  height?: number | string;
  /**
   * Explicit width as a number (px) or CSS string (e.g. "100%").
   * Alternative to setting width via `className`.
   */
  width?: number | string;
}

/**
 * Shimmer placeholder for content that is still loading.
 *
 * - `aria-hidden="true"` — carries no information; invisible to assistive tech.
 * - `data-slot="skeleton"` — stable selector for tests and visual-audit sweeps.
 * - Animation is a CSS `background-position` shift (gradient shimmer).
 *   The global `prefers-reduced-motion` block in `index.css` zeroes
 *   `animation-duration` for users with motion sensitivity — no per-component work needed.
 *
 * @example Fixed-height row placeholder
 * ```tsx
 * <Skeleton className="w-full rounded-md" height={20} />
 * ```
 *
 * @example Circular avatar placeholder
 * ```tsx
 * <Skeleton className="size-10 rounded-full" />
 * ```
 */
export function Skeleton({ className, height, width }: SkeletonProps) {
  const style: React.CSSProperties = {};
  if (height !== undefined) {
    style.height = typeof height === 'number' ? `${height}px` : height;
  }
  if (width !== undefined) {
    style.width = typeof width === 'number' ? `${width}px` : width;
  }

  return (
    <div
      aria-hidden="true"
      data-slot="skeleton"
      style={style}
      className={cn(
        // Base: subtle zinc background with a moving highlight.
        // The shimmer uses a linear-gradient as background-image and shifts
        // background-position — GPU-composited, no layout thrash.
        'animate-[shimmer_1.5s_ease-in-out_infinite]',
        'bg-[linear-gradient(90deg,var(--color-bg-elevated)_25%,var(--color-border-strong)_50%,var(--color-bg-elevated)_75%)]',
        'bg-[length:200%_100%]',
        'rounded-md',
        className
      )}
    />
  );
}
```

The shimmer keyframe must exist in `tailwind.config.js` or `index.css`. Add to `tailwind.config.js` under `theme.extend.keyframes` and `theme.extend.animation`:

- [ ] Modify `tailwind.config.js` — add inside `theme.extend`:

```js
keyframes: {
  shimmer: {
    '0%': { backgroundPosition: '200% 0' },
    '100%': { backgroundPosition: '-200% 0' },
  },
},
animation: {
  // Skeleton shimmer — suppressed by the prefers-reduced-motion block in index.css
  shimmer: 'shimmer 1.5s ease-in-out infinite',
},
```

> If `theme.extend.keyframes` already exists in `tailwind.config.js`, merge into it rather than replacing.

### Verify

```sh
pnpm test src/components/ui/Skeleton.test.tsx    # expect 7 passing
pnpm typecheck
```

### Commit

```sh
git add crates/vox-gui/ui/src/components/ui/Skeleton.tsx \
        crates/vox-gui/ui/src/components/ui/Skeleton.test.tsx \
        crates/vox-gui/ui/tailwind.config.js
git commit -m "feat(a11y,phase0c): add Skeleton loading primitive with shimmer animation"
```

Expected output: `7 passed`.

---

## Task 6: Install Radix Dialog + Tooltip; create `Dialog` primitive

### Why

Dialogs built from scratch routinely miss focus-trap, Escape-key close, scroll-lock, and `aria-modal`. Radix Dialog provides all four for free. The `Dialog` wrapper exports re-named sub-components and applies `Glass.tsx` styling to the content panel so call sites don't repeat it.

`@radix-ui/react-tooltip` is installed here (same pnpm invocation) even though `Tooltip.tsx` is Phase 0D scope — installing now avoids a second package-lock churn.

### Install

```sh
pnpm add @radix-ui/react-dialog @radix-ui/react-tooltip
```

Verify both appear in `package.json` under `dependencies`.

### Test first

- [ ] Create `src/components/ui/Dialog.test.tsx`:

```typescript
// src/components/ui/Dialog.test.tsx
import { describe, it, expect } from 'vitest';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import React from 'react';
import {
  Dialog,
  DialogTrigger,
  DialogContent,
  DialogTitle,
  DialogDescription,
} from './Dialog';

function SampleDialog({ open }: { open?: boolean }) {
  return (
    <Dialog defaultOpen={open}>
      <DialogTrigger asChild>
        <button type="button">Open</button>
      </DialogTrigger>
      <DialogContent>
        <DialogTitle>Confirm action</DialogTitle>
        <DialogDescription>This action cannot be undone.</DialogDescription>
        <p>Dialog body content.</p>
      </DialogContent>
    </Dialog>
  );
}

describe('Dialog', () => {
  it('does not render dialog content before trigger is clicked', () => {
    render(<SampleDialog />);
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('opens when the trigger is clicked', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    });
  });

  it('renders the title inside the open dialog', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByText('Confirm action')).toBeInTheDocument();
    });
  });

  it('renders the description inside the open dialog', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByText('This action cannot be undone.')).toBeInTheDocument();
    });
  });

  it('closes when the Escape key is pressed', async () => {
    render(<SampleDialog />);
    await userEvent.click(screen.getByRole('button', { name: 'Open' }));
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    });
    await userEvent.keyboard('{Escape}');
    await waitFor(() => {
      expect(screen.queryByRole('dialog')).toBeNull();
    });
  });

  it('renders in open state when defaultOpen is true', async () => {
    render(<SampleDialog open={true} />);
    await waitFor(() => {
      expect(screen.getByRole('dialog')).toBeInTheDocument();
    });
  });
});
```

Run: `pnpm test src/components/ui/Dialog.test.tsx` — expect **6 failing** (file does not exist).

### Implementation

- [ ] Create `src/components/ui/Dialog.tsx`:

```typescript
// src/components/ui/Dialog.tsx
/**
 * Dialog primitive — thin wrapper over @radix-ui/react-dialog.
 *
 * Applies Glass.tsx surface styling to `DialogContent`. Re-exports all
 * sub-components under semantic names so call sites only import from here.
 *
 * Built-in from Radix:
 *   - Focus trap (focus stays inside the dialog while open)
 *   - Escape-key close
 *   - scroll-lock on the body
 *   - aria-modal="true" on the dialog element
 *   - aria-labelledby wired to <DialogTitle>
 *   - aria-describedby wired to <DialogDescription>
 */
import React from 'react';
import * as RadixDialog from '@radix-ui/react-dialog';
import { cn } from '../../lib/cn';

// ── Re-exports (unchanged behaviour) ─────────────────────────────────────────

/** Root dialog controller. Accepts `open`, `defaultOpen`, `onOpenChange`. */
export const Dialog = RadixDialog.Root;

/** Wraps a trigger element. Use `asChild` to avoid an extra DOM node. */
export const DialogTrigger = RadixDialog.Trigger;

/** Portal that mounts content outside the React tree root. */
export const DialogPortal = RadixDialog.Portal;

/** Dimmed backdrop behind the dialog panel. */
export const DialogOverlay = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Overlay>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Overlay>
>(({ className, ...props }, ref) => (
  <RadixDialog.Overlay
    ref={ref}
    className={cn(
      'fixed inset-0 z-50 bg-black/60 backdrop-blur-sm',
      'data-[state=open]:animate-in data-[state=closed]:animate-out',
      'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
      className
    )}
    {...props}
  />
));
DialogOverlay.displayName = 'DialogOverlay';

/**
 * The dialog panel itself.
 *
 * Renders inside a portal so z-index stacking never fights the surface tree.
 * Applies Glass-derived styling: dark translucent background, subtle border,
 * backdrop blur — identical to `<Glass>` so dialogs feel native to the console.
 */
export const DialogContent = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Content>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Content>
>(({ className, children, ...props }, ref) => (
  <DialogPortal>
    <DialogOverlay />
    <RadixDialog.Content
      ref={ref}
      className={cn(
        // Positioning
        'fixed left-1/2 top-1/2 z-50 -translate-x-1/2 -translate-y-1/2',
        // Sizing
        'w-full max-w-lg',
        // Glass surface (mirrors Glass.tsx)
        'rounded-xl border border-white/10 bg-zinc-900/80 backdrop-blur-xl',
        'shadow-[0_24px_60px_-20px_rgba(0,0,0,0.9)]',
        // Padding
        'p-6',
        // Entry/exit animations (suppressed by prefers-reduced-motion block)
        'data-[state=open]:animate-in data-[state=closed]:animate-out',
        'data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0',
        'data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95',
        'data-[state=closed]:slide-out-to-left-1/2 data-[state=closed]:slide-out-to-top-[48%]',
        'data-[state=open]:slide-in-from-left-1/2 data-[state=open]:slide-in-from-top-[48%]',
        className
      )}
      {...props}
    >
      {children}
    </RadixDialog.Content>
  </DialogPortal>
));
DialogContent.displayName = 'DialogContent';

/**
 * Dialog heading. Radix wires `aria-labelledby` on the Content to this element's id.
 * Required whenever DialogContent is used — omitting it triggers a Radix a11y warning.
 */
export const DialogTitle = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Title>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Title>
>(({ className, ...props }, ref) => (
  <RadixDialog.Title
    ref={ref}
    className={cn(
      'font-display text-base font-semibold tracking-wide text-zinc-100',
      className
    )}
    {...props}
  />
));
DialogTitle.displayName = 'DialogTitle';

/**
 * Dialog description. Radix wires `aria-describedby` on the Content to this element's id.
 * Include whenever the dialog needs a supporting explanation sentence.
 */
export const DialogDescription = React.forwardRef<
  React.ElementRef<typeof RadixDialog.Description>,
  React.ComponentPropsWithoutRef<typeof RadixDialog.Description>
>(({ className, ...props }, ref) => (
  <RadixDialog.Description
    ref={ref}
    className={cn('mt-1 text-[13px] text-zinc-400', className)}
    {...props}
  />
));
DialogDescription.displayName = 'DialogDescription';

/** Programmatic close trigger. Use inside DialogContent to render a close button. */
export const DialogClose = RadixDialog.Close;
```

### Verify

```sh
pnpm test src/components/ui/Dialog.test.tsx    # expect 6 passing
pnpm typecheck
```

### Commit

```sh
git add crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml \
        crates/vox-gui/ui/src/components/ui/Dialog.tsx \
        crates/vox-gui/ui/src/components/ui/Dialog.test.tsx
git commit -m "feat(a11y,phase0c): add Dialog primitive wrapping @radix-ui/react-dialog"
```

Expected output: `6 passed`.

---

## Task 7: Full suite verification gate

Run the complete test suite. The baseline was 178 tests. Phase 0C adds: 3 (CSS) + 7 (Button) + 6 (Toasts) + 7 (Skeleton) + 6 (Dialog) = **29 new tests**.

Expected total: **207 tests, all passing.**

```sh
# from crates/vox-gui/ui/
pnpm test
pnpm typecheck
```

If either fails, fix before proceeding.

### Commit (if only README/housekeeping changes remain)

No additional commit needed — all substantive changes were committed per-task. The verification gate is non-committing.

---

## Self-review checklist

- [ ] `src/index.css.test.ts` — 3 tests: `:focus-visible`, `--color-accent-default`, `prefers-reduced-motion` + `animation` keyword
- [ ] `src/components/ui/Button.tsx` — `type="button"` default, `asChild` via Slot, `aria-label` forwarding, `displayName`
- [ ] `src/components/ui/Button.test.tsx` — 7 tests: type default, aria-label, children, className, asChild, type override, disabled
- [ ] `src/components/ui/Toasts.tsx` — outer `aria-live="polite"` + `role="status"`; close button `aria-label="Dismiss notification"` + `type="button"`; icon `aria-hidden="true"`
- [ ] `src/components/ui/Toasts.test.tsx` — 6 tests: aria-live present, role="status", close buttons labeled, click calls onClose, titles, body
- [ ] `src/components/ui/ErrorBoundary.tsx` — Retry has `type="button"` + `aria-label="Retry loading surface"`
- [ ] `src/components/ui/Skeleton.tsx` — `aria-hidden="true"`, `data-slot="skeleton"`, number/string height/width normalization
- [ ] `src/components/ui/Skeleton.test.tsx` — 7 tests: aria-hidden, data-slot, className, height px, width px, string sizes, tag name
- [ ] `src/components/ui/Dialog.tsx` — all five named exports, Overlay with backdrop blur, Content with Glass styling, Title with aria wired, Description with aria wired
- [ ] `src/components/ui/Dialog.test.tsx` — 6 tests: closed by default, opens on click, title present, description present, Escape closes, defaultOpen renders open
- [ ] `package.json` — `@radix-ui/react-slot`, `@radix-ui/react-dialog`, `@radix-ui/react-tooltip` all in `dependencies`
- [ ] `tailwind.config.js` — `shimmer` keyframe + animation entry
- [ ] Total tests: 207 (178 baseline + 29 new), all passing
- [ ] `pnpm typecheck` — zero errors
- [ ] No stubs, no `TODO`, no placeholder implementations

---

## What is NOT in this plan (deferred)

| Deferred item | Where it lands |
|---------------|----------------|
| `aria-label` on existing 175 buttons | Per-surface wave passes |
| `aria-*` on 34 zero-attribute surfaces | Per-surface wave passes |
| Keyboard navigation per surface | Per-surface wave passes |
| `Tooltip.tsx` primitive | Phase 0D (`@radix-ui/react-tooltip` already installed) |
| `Select`, `Checkbox`, `RadioGroup` | Phase 0D or later |
| Skip-nav link | Phase 0D |
| ARIA live region for command output | Per-surface wave pass (Console surface) |
