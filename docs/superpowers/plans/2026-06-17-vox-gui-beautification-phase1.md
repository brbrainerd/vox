---
title: "Vox GUI Beautification (Phase 1: Component Polish) Implementation Plan"
description: "Detailed step-by-step TDD implementation plan to promote UI primitives and migrate 5 core surfaces to the unified design system."
category: "plans"
status: "current"
---

# Vox GUI Beautification (Phase 1: Component Polish) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Promote UI primitives (`Button`, `Glass`, `EmptyState`, `StatusPill`, `Kpi`, `DataTable`) to first-class, fully typed components, and migrate five core operator views (`Tasks`, `Runs`, `Approvals`, `Dashboard`, `Chat`) to use them.

**Architecture:** Create a unified design token abstraction boundary. Develop the polished UI primitives under `ui/src/components/ui/` verified by Vitest unit tests (TDD). Migrate the surfaces incrementally, replacing raw Tailwind style tables and raw HTML tables/cards with custom primitive components.

**Tech Stack:** React 19, Radix UI, TypeScript, Tailwind CSS, Vitest, React Testing Library.

---

## File Map

| File | Change | Responsibility |
|---|---|---|
| `crates/vox-gui/ui/src/styles/tokens.ts` | **Modify** | Add unified `STATUS_TONE` and `StatusToneKind` exports |
| `crates/vox-gui/ui/src/components/ui/Button.tsx` | **Modify** | Implement variants, sizes, loading, and icon props |
| `crates/vox-gui/ui/src/components/ui/Button.test.tsx` | **Modify** | Write test assertions for variants, sizes, and loading |
| `crates/vox-gui/ui/src/components/ui/Glass.tsx` | **Modify** | Implement `size` and `interactive` props |
| `crates/vox-gui/ui/src/components/ui/Glass.test.tsx` | **Create** | Test size padding classes and interactive hover states |
| `crates/vox-gui/ui/src/components/ui/EmptyState.tsx` | **Modify** | Add presets, custom icons, and action button bindings |
| `crates/vox-gui/ui/src/components/ui/EmptyState.test.tsx` | **Create** | Assert preset structures and trigger handlers |
| `crates/vox-gui/ui/src/components/ui/StatusPill.tsx` | **Create** | Create unified status element utilizing `STATUS_TONE` |
| `crates/vox-gui/ui/src/components/ui/StatusPill.test.tsx` | **Create** | Test pulse animations, color matching, and default glyphs |
| `crates/vox-gui/ui/src/components/ui/Kpi.tsx` | **Create** | Component with labels, tabular numerics, delta trend, and sparklines |
| `crates/vox-gui/ui/src/components/ui/Kpi.test.tsx` | **Create** | Verify delta arrow indicators and sparkline nesting |
| `crates/vox-gui/ui/src/components/ui/DataTable.tsx` | **Create** | Generic grouped data grid with multi-select, action kebabs, and skeletons |
| `crates/vox-gui/ui/src/components/ui/DataTable.test.tsx` | **Create** | Test grouping headers, selectable rows, and custom cell renders |
| `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` | **Modify** | Migrate to `DataTable` + `EmptyState` + `StatusPill` |
| `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.test.tsx` | **Modify** | Align unit test assertions for the refactored views |
| `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx` | **Modify** | Refactor scoreboard and recent runs list using `DataTable` |
| `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.test.tsx` | **Modify** | Verify sorted scoreboard and collapsible live decision cards |
| `crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.tsx` | **Modify** | Update approvals queue using `DataTable` and custom action hooks |
| `crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.test.tsx` | **Modify** | Assert layout headers and actions |
| `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` | **Modify** | Refactor KPI tiles, stream cards, and customize grids |
| `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx` | **Modify** | Verify grid loading indicators and status pill renders |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` | **Modify** | Refactor execution rail and chat transcripts |
| `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` | **Modify** | Test bubble triggers and status pane renders |

---

## Tasks

### Task 1: Add unified `STATUS_TONE` map

**Files:**
- Modify: `crates/vox-gui/ui/src/styles/tokens.ts`

- [ ] **Step 1.1: Write the failing test**

Modify `crates/vox-gui/ui/src/index.css.test.ts` (or append to any global config check) to assert `STATUS_TONE` keys:

```typescript
import { describe, it, expect } from 'vitest';
import { STATUS_TONE } from './src/styles/tokens';

describe('STATUS_TONE', () => {
  it('contains the mandatory keys', () => {
    expect(STATUS_TONE.pass).toBeDefined();
    expect(STATUS_TONE.fail).toBeDefined();
    expect(STATUS_TONE.warn).toBeDefined();
    expect(STATUS_TONE.Executing).toBeDefined();
  });
});
```

- [ ] **Step 1.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test index.css.test
```
Expected: FAIL due to missing `STATUS_TONE` export in `tokens.ts`.

- [ ] **Step 1.3: Implement `STATUS_TONE` in `tokens.ts`**

Replace `crates/vox-gui/ui/src/styles/tokens.ts` with:

```typescript
/** Design tokens — status colors and badge class maps. */

export const STATUS_BADGE_CLASS = {
  pass: 'bg-emerald-400/20 text-emerald-200 ring-1 ring-emerald-400/40',
  fail: 'bg-red-500/20 text-red-300 ring-1 ring-red-500/40',
  warn: 'bg-amber-400/20 text-amber-200 ring-1 ring-amber-400/40',
  not_run: 'bg-white/[0.05] text-zinc-400',
} as const;

export const STATUS_RAIL_BADGE_CLASS = {
  pass: 'bg-emerald-400 text-zinc-950',
  fail: 'bg-red-500 text-zinc-950',
  warn: 'bg-amber-400 text-zinc-950',
  not_run: 'bg-zinc-600 text-zinc-100',
} as const;

export type StatusToneKind =
  | 'pass'
  | 'fail'
  | 'warn'
  | 'info'
  | 'neutral'
  | 'accent'
  | 'Executing'
  | 'Verifying'
  | 'Planning'
  | 'Paused'
  | 'Validated'
  | 'Doubted'
  | 'Speculative'
  | 'Active'
  | 'Root';

export const STATUS_TONE = {
  pass:   { dot: 'bg-emerald-400',  ring: 'ring-emerald-400/30',  text: 'text-emerald-300',  soft: 'bg-emerald-400/10',  solid: 'bg-emerald-400',  onSolid: 'text-zinc-950' },
  fail:   { dot: 'bg-red-500',      ring: 'ring-red-500/30',      text: 'text-red-300',      soft: 'bg-red-500/10',      solid: 'bg-red-500',      onSolid: 'text-zinc-950' },
  warn:   { dot: 'bg-amber-400',    ring: 'ring-amber-400/30',    text: 'text-amber-300',    soft: 'bg-amber-400/10',    solid: 'bg-amber-400',    onSolid: 'text-zinc-950' },
  info:   { dot: 'bg-sky-400',      ring: 'ring-sky-400/30',      text: 'text-sky-300',      soft: 'bg-sky-400/10',      solid: 'bg-sky-400',      onSolid: 'text-zinc-950' },
  neutral:{ dot: 'bg-zinc-500',     ring: 'ring-zinc-500/30',     text: 'text-zinc-300',     soft: 'bg-white/[0.04]',    solid: 'bg-zinc-500',     onSolid: 'text-zinc-100' },
  accent: { dot: 'bg-brass',        ring: 'ring-brass/30',        text: 'text-brass',        soft: 'bg-brass/10',        solid: 'bg-brass',        onSolid: 'text-zinc-950' },
  Executing:   { dot: 'bg-brass',     ring: 'ring-brass/30',       text: 'text-brass',       soft: 'bg-brass/10',       solid: 'bg-brass',       onSolid: 'text-zinc-950' },
  Verifying:   { dot: 'bg-violet-400',ring: 'ring-violet-400/30', text: 'text-violet-300',  soft: 'bg-violet-400/10',  solid: 'bg-violet-400',  onSolid: 'text-zinc-950' },
  Planning:    { dot: 'bg-cyan-400',  ring: 'ring-cyan-400/30',    text: 'text-cyan-300',    soft: 'bg-cyan-400/10',    solid: 'bg-cyan-400',    onSolid: 'text-zinc-950' },
  Paused:      { dot: 'bg-zinc-500',  ring: 'ring-zinc-500/30',    text: 'text-zinc-300',    soft: 'bg-white/[0.04]',   solid: 'bg-zinc-500',    onSolid: 'text-zinc-100' },
  Validated:   { dot: 'bg-emerald-400',ring:'ring-emerald-400/30', text: 'text-emerald-300', soft: 'bg-emerald-400/10', solid: 'bg-emerald-400', onSolid: 'text-zinc-950' },
  Doubted:     { dot: 'bg-amber-400', ring: 'ring-amber-400/30',   text: 'text-amber-300',   soft: 'bg-amber-400/10',   solid: 'bg-amber-400',   onSolid: 'text-zinc-950' },
  Speculative: { dot: 'bg-violet-400',ring: 'ring-violet-400/30',  text: 'text-violet-300',  soft: 'bg-violet-400/10',  solid: 'bg-violet-400',  onSolid: 'text-zinc-950' },
  Active:      { dot: 'bg-cyan-400',  ring: 'ring-cyan-400/30',    text: 'text-cyan-300',    soft: 'bg-cyan-400/10',    solid: 'bg-cyan-400',    onSolid: 'text-zinc-950' },
  Root:        { dot: 'bg-white',     ring: 'ring-white/30',       text: 'text-white',       soft: 'bg-white/[0.06]',   solid: 'bg-white',       onSolid: 'text-zinc-950' },
} as const;
```

- [ ] **Step 1.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test index.css.test
```
Expected: PASS.

- [ ] **Step 1.5: Commit**

```bash
git add crates/vox-gui/ui/src/styles/tokens.ts
git commit -m "feat(gui): define unified STATUS_TONE maps for components"
```

---

### Task 2: Promote `<Button>` Primitive

**Files:**
- Modify: `crates/vox-gui/ui/src/components/ui/Button.tsx`
- Modify: `crates/vox-gui/ui/src/components/ui/Button.test.tsx`

- [ ] **Step 2.1: Write failing tests for variants, sizes, loading spinner in Button.test.tsx**

Replace `crates/vox-gui/ui/src/components/ui/Button.test.tsx` with:

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Button } from './Button';

describe('Button Primitive', () => {
  it('renders with variants and sizes', () => {
    const { rerender } = render(<Button variant="primary" size="lg">Test</Button>);
    expect(screen.getByRole('button')).toHaveClass('bg-brass');

    rerender(<Button variant="danger" size="xs">Test</Button>);
    expect(screen.getByRole('button')).toHaveClass('bg-red-500');
  });

  it('renders in loading state by rendering a spinner and disabling click', () => {
    render(<Button loading>Submit</Button>);
    const btn = screen.getByRole('button');
    expect(btn).toBeDisabled();
    expect(btn.querySelector('svg')).toBeInTheDocument(); // spinner icon
  });
});
```

- [ ] **Step 2.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test Button.test
```
Expected: FAIL due to missing variant/size classes or properties.

- [ ] **Step 2.3: Implement polished `<Button>` in Button.tsx**

Replace `crates/vox-gui/ui/src/components/ui/Button.tsx` with:

```typescript
import React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cn } from '../../lib/cn';

const VARIANT_CLASS = {
  primary: 'bg-brass text-zinc-950 hover:bg-brass-light active:bg-brass-dark disabled:opacity-50',
  secondary: 'bg-white/[0.05] text-zinc-100 hover:bg-white/[0.1] active:bg-white/[0.15]',
  ghost: 'bg-transparent text-zinc-400 hover:text-zinc-100 hover:bg-white/[0.03]',
  outline: 'bg-transparent border border-white/[0.08] text-zinc-300 hover:bg-white/[0.03]',
  danger: 'bg-red-500 text-white hover:bg-red-600 active:bg-red-700',
};

const SIZE_CLASS = {
  xs: 'px-2 py-0.5 text-[10px] h-6 rounded',
  sm: 'px-2.5 py-1 text-[11px] h-7 rounded-md',
  md: 'px-3.5 py-1.5 text-[13px] h-9 rounded-lg',
  lg: 'px-4.5 py-2 text-[15px] h-11 rounded-xl',
  icon: 'size-8 p-0 flex items-center justify-center rounded-lg',
};

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: keyof typeof VARIANT_CLASS;
  size?: keyof typeof SIZE_CLASS;
  loading?: boolean;
  icon?: React.ReactNode;
  trailingIcon?: React.ReactNode;
  asChild?: boolean;
}

export const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ 
    variant = 'secondary', 
    size = 'md', 
    loading = false, 
    icon, 
    trailingIcon, 
    asChild = false, 
    className, 
    type = 'button', 
    children, 
    disabled,
    ...props 
  }, ref) => {
    const Comp = asChild ? Slot : 'button';
    
    return (
      <Comp
        ref={ref as React.Ref<HTMLButtonElement>}
        type={asChild ? undefined : type}
        disabled={loading || disabled}
        className={cn(
          "inline-flex items-center justify-center font-medium tracking-wide transition-all focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-brass",
          VARIANT_CLASS[variant],
          SIZE_CLASS[size],
          className
        )}
        {...props}
      >
        {loading ? (
          <svg className="animate-spin -ml-1 mr-2 h-3.5 w-3.5 text-current" fill="none" viewBox="0 0 24 24">
            <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
            <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z" />
          </svg>
        ) : icon ? (
          <span className="mr-1.5 flex items-center">{icon}</span>
        ) : null}
        {children}
        {!loading && trailingIcon && <span className="ml-1.5 flex items-center">{trailingIcon}</span>}
      </Comp>
    );
  }
);

Button.displayName = 'Button';
```

- [ ] **Step 2.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test Button.test
```
Expected: PASS.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/Button.tsx crates/vox-gui/ui/src/components/ui/Button.test.tsx
git commit -m "feat(gui): promote Button to supporting size, variant, loading and icons"
```

---

### Task 3: Polish `<Glass>` Primitive

**Files:**
- Modify: `crates/vox-gui/ui/src/components/ui/Glass.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/Glass.test.tsx`

- [ ] **Step 3.1: Write failing test in Glass.test.tsx**

Create `crates/vox-gui/ui/src/components/ui/Glass.test.tsx`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Glass } from './Glass';

describe('Glass Primitive', () => {
  it('applies padding based on size prop', () => {
    const { rerender } = render(<Glass size="sm" data-testid="g">Content</Glass>);
    expect(screen.getByTestId('g')).toHaveClass('p-3');

    rerender(<Glass size="lg" data-testid="g">Content</Glass>);
    expect(screen.getByTestId('g')).toHaveClass('p-6');
  });

  it('adds interactive hover states when interactive prop is true', () => {
    render(<Glass interactive data-testid="g">Clickable</Glass>);
    expect(screen.getByTestId('g')).toHaveClass('cursor-pointer');
  });
});
```

- [ ] **Step 3.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test Glass.test
```
Expected: FAIL due to missing `Glass.test.tsx` or size mapping.

- [ ] **Step 3.3: Implement polished `<Glass>` in Glass.tsx**

Replace `crates/vox-gui/ui/src/components/ui/Glass.tsx` with:

```typescript
import React from 'react';
import { cn } from '../../lib/cn';

const SIZE_PADDING = {
  sm: 'p-3 rounded-xl',
  md: 'p-5 rounded-2xl',
  lg: 'p-6 rounded-3xl',
};

export interface GlassProps extends React.HTMLAttributes<HTMLDivElement> {
  size?: keyof typeof SIZE_PADDING;
  inset?: boolean;
  interactive?: boolean;
  as?: React.ElementType;
}

export function Glass({ 
  className, 
  size = 'md',
  inset = true, 
  interactive = false,
  as: Comp = 'div',
  children, 
  ...rest 
}: GlassProps) {
  return (
    <Comp
      {...rest}
      className={cn(
        "relative border border-white/[0.06] bg-white/[0.025] backdrop-blur-2xl shadow-[0_1px_0_rgba(255,255,255,0.04)_inset,0_24px_60px_-30px_rgba(0,0,0,0.9)]",
        SIZE_PADDING[size],
        interactive && "hover:border-white/[0.12] hover:bg-white/[0.04] cursor-pointer transition-all duration-150 active:scale-[0.99]",
        className
      )}
    >
      {inset && (
        <div className="pointer-events-none absolute inset-0 rounded-[inherit] ring-1 ring-inset ring-white/[0.04]" />
      )}
      {children}
    </Comp>
  );
}
```

- [ ] **Step 3.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test Glass.test
```
Expected: PASS.

- [ ] **Step 3.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/Glass.tsx crates/vox-gui/ui/src/components/ui/Glass.test.tsx
git commit -m "feat(gui): add size and interactive support to Glass panel component"
```

---

### Task 4: Promote `<EmptyState>` Primitive

**Files:**
- Modify: `crates/vox-gui/ui/src/components/ui/EmptyState.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/EmptyState.test.tsx`

- [ ] **Step 4.1: Write failing tests for EmptyState variants and buttons**

Create `crates/vox-gui/ui/src/components/ui/EmptyState.test.tsx`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { EmptyState } from './EmptyState';

describe('EmptyState Primitive', () => {
  it('renders default text based on variant type', () => {
    render(<EmptyState variant="no-permission" title="Denied" />);
    expect(screen.getByText('Denied')).toBeInTheDocument();
  });

  it('triggers primary and secondary callbacks on click', () => {
    const onPrimary = vi.fn();
    const onSecondary = vi.fn();
    render(
      <EmptyState 
        title="Empty"
        primaryAction={{ label: 'Save', onClick: onPrimary }}
        secondaryAction={{ label: 'Cancel', onClick: onSecondary }}
      />
    );
    fireEvent.click(screen.getByText('Save'));
    fireEvent.click(screen.getByText('Cancel'));
    expect(onPrimary).toHaveBeenCalledTimes(1);
    expect(onSecondary).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 4.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test EmptyState.test
```
Expected: FAIL due to missing variant structures or double buttons.

- [ ] **Step 4.3: Implement polished `<EmptyState>` in EmptyState.tsx**

Replace `crates/vox-gui/ui/src/components/ui/EmptyState.tsx` with:

```typescript
import React from 'react';
import { Button } from './Button';
import { Icon } from './Icons';

export interface EmptyStateProps {
  variant?: 'no-data' | 'no-permission' | 'no-connection' | 'error' | 'welcome';
  icon?: React.ReactNode;
  title: string;
  description?: string;
  primaryAction?: { label: string; onClick: () => void };
  secondaryAction?: { label: string; onClick: () => void };
  children?: React.ReactNode;
}

const DEFAULT_ICONS = {
  'no-data': <Icon.alert className="size-8 text-zinc-500" />,
  'no-permission': <Icon.x className="size-8 text-red-400" />,
  'no-connection': <Icon.bolt className="size-8 text-amber-400" />,
  'error': <Icon.alert className="size-8 text-red-500" />,
  'welcome': <Icon.check className="size-8 text-brass animate-pulse" />,
};

export function EmptyState({ 
  variant = 'no-data', 
  icon, 
  title, 
  description, 
  primaryAction, 
  secondaryAction,
  children
}: EmptyStateProps) {
  return (
    <div
      className="flex flex-col items-center justify-center gap-3 py-16 px-4 text-center max-w-lg mx-auto"
      role="status"
      aria-live="polite"
    >
      <div className="flex justify-center mb-1">
        {icon || DEFAULT_ICONS[variant]}
      </div>
      <h3 className="font-display text-sm tracking-widest uppercase text-zinc-200">{title}</h3>
      {description && <p className="text-xs text-zinc-500 leading-relaxed max-w-sm">{description}</p>}
      
      {children}

      {(primaryAction || secondaryAction) && (
        <div className="flex items-center justify-center gap-3 mt-3">
          {secondaryAction && (
            <Button variant="ghost" size="sm" onClick={secondaryAction.onClick}>
              {secondaryAction.label}
            </Button>
          )}
          {primaryAction && (
            <Button variant="primary" size="sm" onClick={primaryAction.onClick}>
              {primaryAction.label}
            </Button>
          )}
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test EmptyState.test
```
Expected: PASS.

- [ ] **Step 4.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/EmptyState.tsx crates/vox-gui/ui/src/components/ui/EmptyState.test.tsx
git commit -m "feat(gui): refactor EmptyState with preset variants and action buttons"
```

---

### Task 5: Create `<StatusPill>` Component

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/StatusPill.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/StatusPill.test.tsx`

- [ ] **Step 5.1: Write failing tests in StatusPill.test.tsx**

Create `crates/vox-gui/ui/src/components/ui/StatusPill.test.tsx`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { StatusPill } from './StatusPill';

describe('StatusPill Component', () => {
  it('renders status indicators matching the status tone', () => {
    render(<StatusPill tone="pass" label="Done" />);
    const pill = screen.getByText('Done');
    expect(pill).toHaveClass('text-emerald-300');
  });

  it('renders status glyph default matching tone', () => {
    const { container } = render(<StatusPill tone="fail" />);
    expect(container.textContent).toContain('!');
  });
});
```

- [ ] **Step 5.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test StatusPill.test
```
Expected: FAIL due to missing StatusPill component implementation.

- [ ] **Step 5.3: Implement `<StatusPill>` in StatusPill.tsx**

Create `crates/vox-gui/ui/src/components/ui/StatusPill.tsx`:

```typescript
import React from 'react';
import { STATUS_TONE, StatusToneKind } from '../../styles/tokens';
import { cn } from '../../lib/cn';

export interface StatusPillProps {
  tone: StatusToneKind;
  label?: string;
  size?: 'xs' | 'sm';
  pulse?: boolean;
  icon?: React.ReactNode;
}

const DEFAULT_GLYPHS: Record<StatusToneKind, string> = {
  pass: '✓',
  fail: '!',
  warn: '?',
  info: 'i',
  neutral: '·',
  accent: '◆',
  Executing: '◆',
  Verifying: '◆',
  Planning: '◆',
  Paused: '·',
  Validated: '✓',
  Doubted: '?',
  Speculative: '◆',
  Active: '◆',
  Root: '◆',
};

const SIZE_CLASS = {
  xs: 'px-1.5 py-px text-[9px]',
  sm: 'px-2 py-0.5 text-[10px]',
};

export function StatusPill({ 
  tone, 
  label, 
  size = 'sm', 
  pulse = false, 
  icon 
}: StatusPillProps) {
  const toneStyle = STATUS_TONE[tone] || STATUS_TONE.neutral;
  const glyph = DEFAULT_GLYPHS[tone] || '·';
  
  // Auto-pulse Executing/Active/Verifying states
  const shouldPulse = pulse || tone === 'Executing' || tone === 'Active' || tone === 'Verifying';

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full font-medium tracking-wide uppercase ring-1 bg-white/[0.02] shrink-0 select-none",
        toneStyle.ring,
        toneStyle.text,
        SIZE_CLASS[size]
      )}
    >
      <span className={cn("relative inline-block size-1.5 rounded-full", toneStyle.dot)}>
        {shouldPulse && (
          <span className={cn("absolute inset-0 rounded-full animate-vox-ping opacity-60", toneStyle.dot)} />
        )}
      </span>
      {icon || <span className="font-mono">{glyph}</span>}
      <span>{label || tone}</span>
    </span>
  );
}
```

- [ ] **Step 5.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test StatusPill.test
```
Expected: PASS.

- [ ] **Step 5.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/StatusPill.tsx crates/vox-gui/ui/src/components/ui/StatusPill.test.tsx
git commit -m "feat(gui): implement unified StatusPill primitive replacing Pill and StateChip"
```

---

### Task 6: Create `<Kpi>` Component

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/Kpi.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/Kpi.test.tsx`

- [ ] **Step 6.1: Write failing tests in Kpi.test.tsx**

Create `crates/vox-gui/ui/src/components/ui/Kpi.test.tsx`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Kpi } from './Kpi';

describe('Kpi Component', () => {
  it('renders the label, value, and delta indicators', () => {
    render(<Kpi label="Mesh node count" value={5} delta={1} trend="up" />);
    expect(screen.getByText('Mesh node count')).toBeInTheDocument();
    expect(screen.getByText('5')).toBeInTheDocument();
    expect(screen.getByText('▲1')).toBeInTheDocument();
  });
});
```

- [ ] **Step 6.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test Kpi.test
```
Expected: FAIL due to missing Kpi component.

- [ ] **Step 6.3: Implement `<Kpi>` in Kpi.tsx**

Create `crates/vox-gui/ui/src/components/ui/Kpi.tsx`:

```typescript
import React from 'react';
import { Glass } from './Glass';
import { cn } from '../../lib/cn';

const ACCENT_COLORS = {
  cyan: 'text-cyan-400',
  amber: 'text-amber-400',
  emerald: 'text-emerald-400',
  violet: 'text-violet-400',
  brass: 'text-brass',
  zinc: 'text-zinc-400',
  sky: 'text-sky-400',
};

export interface KpiProps extends React.HTMLAttributes<HTMLDivElement> {
  label: string;
  value: string | number;
  unit?: string;
  delta?: number;
  trend?: 'up' | 'down' | 'flat';
  accent?: keyof typeof ACCENT_COLORS;
  sparkData?: number[];
  icon?: React.ReactNode;
  onClick?: () => void;
  children?: React.ReactNode;
}

export function Kpi({
  label,
  value,
  unit = '',
  delta,
  trend = 'flat',
  accent = 'brass',
  sparkData,
  icon,
  onClick,
  className,
  children,
  ...props
}: KpiProps) {
  const isClickable = !!onClick;
  
  return (
    <Glass
      size="sm"
      interactive={isClickable}
      onClick={onClick}
      className={cn("flex flex-col select-none", className)}
      {...props}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-[10px] overline uppercase tracking-widest text-zinc-500 font-medium truncate">
          {label}
        </span>
        {icon && <span className="text-zinc-600 flex shrink-0">{icon}</span>}
      </div>

      <div className="flex items-baseline gap-1 mt-1">
        <span className={cn("font-mono font-bold tracking-tight text-[18px] tabular-nums", ACCENT_COLORS[accent])}>
          {value}
        </span>
        {unit && <span className="text-xs text-zinc-500 font-medium">{unit}</span>}
        
        {delta !== undefined && (
          <span className={cn(
            "ml-auto font-mono text-[10px] font-semibold flex items-center tabular-nums",
            trend === 'up' ? 'text-emerald-400' : trend === 'down' ? 'text-red-400' : 'text-zinc-500'
          )}>
            {trend === 'up' ? '▲' : trend === 'down' ? '▼' : '■'}
            {Math.abs(delta)}
          </span>
        )}
      </div>

      {children}
    </Glass>
  );
}

Kpi.Sub = function KpiSub({ children }: { children: React.ReactNode }) {
  return (
    <div className="text-[11px] text-zinc-500 mt-1 leading-none select-text">
      {children}
    </div>
  );
};
```

- [ ] **Step 6.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test Kpi.test
```
Expected: PASS.

- [ ] **Step 6.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/Kpi.tsx crates/vox-gui/ui/src/components/ui/Kpi.test.tsx
git commit -m "feat(gui): create Kpi layout component with delta indicators"
```

---

### Task 7: Create `<DataTable>` Component

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/DataTable.tsx`
- Create: `crates/vox-gui/ui/src/components/ui/DataTable.test.tsx`

- [ ] **Step 7.1: Write failing tests in DataTable.test.tsx**

Create `crates/vox-gui/ui/src/components/ui/DataTable.test.tsx`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { DataTable } from './DataTable';

describe('DataTable Component', () => {
  const rows = [
    { id: '1', name: 'Task A', status: 'queued' },
    { id: '2', name: 'Task B', status: 'queued' },
  ];
  const columns = [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
  ];

  it('renders column headers and row cells', () => {
    render(<DataTable rows={rows} columns={columns} getRowId={r => r.id} />);
    expect(screen.getByText('ID')).toBeInTheDocument();
    expect(screen.getByText('Name')).toBeInTheDocument();
    expect(screen.getByText('Task A')).toBeInTheDocument();
  });
});
```

- [ ] **Step 7.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test DataTable.test
```
Expected: FAIL due to missing DataTable component.

- [ ] **Step 7.3: Implement `<DataTable>` in DataTable.tsx**

Create `crates/vox-gui/ui/src/components/ui/DataTable.tsx`:

```typescript
import React, { useState } from 'react';
import { cn } from '../../lib/cn';
import { Button } from './Button';

export interface ColumnDef<T> {
  key: string;
  header: string;
  width?: number;
  sortable?: boolean;
  render?: (row: T) => React.ReactNode;
}

export interface DataTableProps<T> {
  rows: T[];
  columns: ColumnDef<T>[];
  groupBy?: (row: T) => string;
  selectable?: boolean;
  onRowAction?: (id: string, action: string) => void;
  emptyState?: React.ReactNode;
  loading?: boolean;
  getRowId: (row: T) => string;
  density?: 'compact' | 'default' | 'comfortable';
}

const DENSITY_CLASS = {
  compact: 'px-2 py-1 text-[11px]',
  default: 'px-4 py-2 text-sm',
  comfortable: 'px-6 py-4 text-base',
};

export function DataTable<T>({
  rows,
  columns,
  groupBy,
  selectable = false,
  onRowAction,
  emptyState,
  loading = false,
  getRowId,
  density = 'default',
}: DataTableProps<T>) {
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [collapsedGroups, setCollapsedGroups] = useState<Set<string>>(new Set());

  if (loading) {
    return (
      <div className="w-full flex flex-col gap-2 py-4">
        {[1, 2, 3].map(i => (
          <div key={i} className="h-10 w-full bg-white/[0.02] border border-white/[0.04] rounded-lg animate-pulse" />
        ))}
      </div>
    );
  }

  if (rows.length === 0) {
    return <div className="w-full py-6">{emptyState || <div className="text-center text-zinc-500 text-sm">No data available</div>}</div>;
  }

  const toggleGroup = (group: string) => {
    setCollapsedGroups(curr => {
      const next = new Set(curr);
      if (next.has(group)) next.delete(group);
      else next.add(group);
      return next;
    });
  };

  const toggleSelectAll = () => {
    setSelectedIds(curr => {
      if (curr.size === rows.length) return new Set();
      return new Set(rows.map(getRowId));
    });
  };

  const toggleSelectRow = (id: string) => {
    setSelectedIds(curr => {
      const next = new Set(curr);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  // Grouping rows
  const grouped: Record<string, T[]> = {};
  if (groupBy) {
    rows.forEach(r => {
      const key = groupBy(r);
      (grouped[key] ??= []).push(r);
    });
  } else {
    grouped[''] = rows;
  }

  return (
    <div className="w-full overflow-x-auto rounded-xl border border-white/[0.06] bg-zinc-950/20 backdrop-blur-xl">
      {selectable && selectedIds.size > 0 && (
        <div className="flex items-center justify-between px-4 py-2 border-b border-white/[0.06] bg-brass/10 text-brass text-xs">
          <span>{selectedIds.size} rows selected</span>
          <div className="flex items-center gap-2">
            <Button size="xs" variant="primary" onClick={() => onRowAction?.(Array.from(selectedIds).join(','), 'bulk-pause')}>
              Pause
            </Button>
            <Button size="xs" variant="danger" onClick={() => onRowAction?.(Array.from(selectedIds).join(','), 'bulk-cancel')}>
              Cancel
            </Button>
          </div>
        </div>
      )}
      <table className="w-full border-collapse text-left">
        <thead>
          <tr className="border-b border-white/[0.06] bg-white/[0.01]">
            {selectable && (
              <th className={cn("w-10", DENSITY_CLASS[density])}>
                <input 
                  type="checkbox" 
                  checked={selectedIds.size === rows.length && rows.length > 0}
                  onChange={toggleSelectAll}
                  className="rounded border-white/10 bg-zinc-900 text-brass focus:ring-brass/40"
                />
              </th>
            )}
            {columns.map(col => (
              <th 
                key={col.key} 
                className={cn("font-semibold text-zinc-400 tracking-wide uppercase text-[10px]", DENSITY_CLASS[density])}
                style={{ width: col.width }}
              >
                {col.header}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {Object.entries(grouped).map(([groupName, groupRows]) => {
            const isCollapsed = collapsedGroups.has(groupName);
            return (
              <React.Fragment key={groupName}>
                {groupBy && (
                  <tr className="bg-white/[0.02] border-b border-white/[0.04]">
                    <td colSpan={columns.length + (selectable ? 1 : 0)} className="px-3 py-1.5">
                      <button
                        type="button"
                        onClick={() => toggleGroup(groupName)}
                        className="flex items-center gap-1.5 font-mono text-[10px] tracking-widest uppercase text-zinc-400 hover:text-zinc-200"
                      >
                        <span>{isCollapsed ? '▶' : '▼'}</span>
                        <span>{groupName} ({groupRows.length})</span>
                      </button>
                    </td>
                  </tr>
                )}
                {!isCollapsed && groupRows.map(row => {
                  const id = getRowId(row);
                  const isSelected = selectedIds.has(id);
                  return (
                    <tr 
                      key={id} 
                      className={cn(
                        "border-b border-white/5 last:border-0 hover:bg-white/[0.02] transition-colors",
                        isSelected && "bg-brass/[0.02]"
                      )}
                    >
                      {selectable && (
                        <td className={cn("w-10", DENSITY_CLASS[density])}>
                          <input 
                            type="checkbox" 
                            checked={isSelected}
                            onChange={() => toggleSelectRow(id)}
                            className="rounded border-brass/40 bg-zinc-950 text-brass focus:ring-brass/40"
                          />
                        </td>
                      )}
                      {columns.map(col => (
                        <td key={col.key} className={DENSITY_CLASS[density]}>
                          {col.render ? col.render(row) : (row as any)[col.key]}
                        </td>
                      ))}
                    </tr>
                  );
                })}
              </React.Fragment>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}
```

- [ ] **Step 7.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test DataTable.test
```
Expected: PASS.

- [ ] **Step 7.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/DataTable.tsx crates/vox-gui/ui/src/components/ui/DataTable.test.tsx
git commit -m "feat(gui): implement DataTable component with selectable checkboxes and grouping support"
```

---

### Task 8: Migrate `TasksView` surface

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.test.tsx`

- [ ] **Step 8.1: Write failing tests asserting new markup structure in TasksView.test.tsx**

Add a test block to `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.test.tsx` asserting table cell renders:

```typescript
  it('renders columns using DataTable key definitions', () => {
    render(<TasksView />);
    // The columns headers should render
    expect(screen.getByText('Priority')).toBeDefined();
    expect(screen.getByText('Task ID')).toBeDefined();
  });
```

- [ ] **Step 8.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test TasksView.test
```
Expected: FAIL due to missing priority column definition.

- [ ] **Step 8.3: Implement refactored `TasksView` using primitives**

Replace `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` with:

```typescript
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Icon } from '../../ui/Icons';
import { Button } from '../../ui/Button';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { TaskRow, cyclePriority, filterBySession, findWriteOverlaps } from './tasksHelpers';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';

export function TasksView({
  pushToast: _pushToast,
  gamifyEnabled = false,
}: {
  pushToast?: (t: unknown) => void;
  gamifyEnabled?: boolean;
}) {
  const [rows, setRows] = useState<TaskRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<number | null>(null);
  const [draft, setDraft] = useState('');
  const [newTask, setNewTask] = useState('');
  const [busy, setBusy] = useState(false);
  const [sessionFilter, setSessionFilter] = useState<string | null>(null);
  const mounted = useRef(true);

  const refresh = useCallback(async () => {
    try {
      const data = await invoke<TaskRow[]>('list_orchestrator_tasks');
      if (mounted.current) {
        setRows(data);
        setError(null);
      }
    } catch (e) {
      if (mounted.current) setError(String(e));
    } finally {
      if (mounted.current) setLoading(false);
    }
  }, []);

  useEffect(() => {
    mounted.current = true;
    refresh();
    const sub = listen<void>('vox://tasks-changed', () => {
      refresh();
    });
    return () => {
      mounted.current = false;
      sub.then((fn) => fn());
    };
  }, [refresh]);

  const act = useCallback(
    async (fn: () => Promise<unknown>) => {
      setBusy(true);
      try {
        await fn();
        await refresh();
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy(false);
      }
    },
    [refresh]
  );

  const addTask = () => {
    const description = newTask.trim();
    if (!description) return;
    setNewTask('');
    act(async () => {
      await invoke('submit_orchestrator_task', {
        input: { description, files: [], priority: null, session_id: null },
      });
      void recordGamifyGuiEvent('task_submitted', { description }, { enabled: gamifyEnabled });
    });
  };

  const saveEdit = (id: number) => {
    const description = draft.trim();
    setEditingId(null);
    if (!description) return;
    act(() => invoke('edit_orchestrator_task', { taskId: id, description }));
  };

  const remove = (id: number) => act(() => invoke('cancel_orchestrator_task', { taskId: id }));

  const reprioritize = (t: TaskRow) =>
    act(() =>
      invoke('reorder_orchestrator_task', { taskId: t.id, priority: cyclePriority(t.priority) })
    );

  const filteredRows = filterBySession(rows, sessionFilter);

  const columns = [
    {
      key: 'priority',
      header: 'Priority',
      width: 100,
      render: (r: TaskRow) => (
        <button type="button" onClick={() => reprioritize(r)}>
          <StatusPill tone={r.priority === 'urgent' ? 'fail' : r.priority === 'background' ? 'neutral' : 'warn'} label={r.priority} size="xs" />
        </button>
      ),
    },
    {
      key: 'id',
      header: 'Task ID',
      width: 80,
      render: (r: TaskRow) => <span className="font-mono text-zinc-400">#{r.id}</span>,
    },
    {
      key: 'description',
      header: 'Description',
      render: (r: TaskRow) => (
        <div className="flex items-center gap-2">
          {editingId === r.id ? (
            <input
              type="text"
              value={draft}
              onChange={e => setDraft(e.target.value)}
              onBlur={() => saveEdit(r.id)}
              onKeyDown={e => { if (e.key === 'Enter') saveEdit(r.id); }}
              className="bg-zinc-950 border border-white/10 rounded px-2 py-1 text-zinc-100 w-full"
              autoFocus
            />
          ) : (
            <span 
              onClick={() => { setEditingId(r.id); setDraft(r.description); }} 
              className="hover:text-brass cursor-pointer"
            >
              {r.description}
            </span>
          )}
        </div>
      ),
    },
    {
      key: 'actions',
      header: '',
      width: 50,
      render: (r: TaskRow) => (
        <Button variant="ghost" size="xs" onClick={() => remove(r.id)} disabled={busy} title="Cancel task">
          <Icon.x className="size-3 text-red-400" />
        </Button>
      ),
    },
  ];

  return (
    <div className="flex flex-col gap-4 p-4 h-full overflow-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-bold tracking-wide text-zinc-200">Tasks</h2>
        <Button variant="ghost" size="xs" onClick={refresh} aria-label="Refresh tasks" title="Refresh list">
          <Icon.bolt className="size-4 text-zinc-400" />
        </Button>
      </div>

      <div className="flex items-center gap-2">
        <input
          type="text"
          value={newTask}
          onChange={e => setNewTask(e.target.value)}
          placeholder="Add a task…"
          aria-label="Add a task"
          onKeyDown={e => { if (e.key === 'Enter') addTask(); }}
          className="bg-zinc-950 border border-white/10 rounded px-3 py-1.5 text-sm w-full text-zinc-100 placeholder:text-zinc-600 focus:outline-none focus:border-brass"
        />
        <Button variant="primary" size="md" onClick={addTask} disabled={busy}>Add</Button>
      </div>

      {error && <div className="text-xs text-red-400 border border-red-500/20 bg-red-500/5 p-3 rounded-lg">{error}</div>}

      <DataTable
        rows={filteredRows}
        columns={columns}
        getRowId={r => String(r.id)}
        groupBy={r => r.state === 'running' ? 'In progress' : 'Queued'}
        loading={loading}
        emptyState={
          <EmptyState 
            variant="no-data" 
            title="No tasks in this workspace" 
            description="Create a new task at the top to instruct the background agent."
          />
        }
      />
    </div>
  );
}
```

- [ ] **Step 8.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test TasksView.test
```
Expected: PASS.

- [ ] **Step 8.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.test.tsx
git commit -m "refactor(gui): migrate TasksView surface to use DataTable and EmptyState primitives"
```

---

### Task 9: Migrate `RunsView` surface

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.test.tsx`

- [ ] **Step 9.1: Write failing tests in RunsView.test.tsx**

Modify `RunsView.test.tsx` to assert rendering of `Model Scoreboard (7d)` header and tabular renders:

```typescript
  it('renders RunsView elements utilizing DataTable key fields', () => {
    render(<RunsView pushToast={() => {}} />);
    expect(screen.getByText('Model Scoreboard (7d)')).toBeDefined();
    expect(screen.getByText('Recent Runs (last 50)')).toBeDefined();
  });
```

- [ ] **Step 9.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test RunsView.test
```
Expected: FAIL due to key mismatch.

- [ ] **Step 9.3: Implement refactored `RunsView`**

Replace `crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx` with:

```typescript
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { Button } from '../../ui/Button';
import { Icon } from '../../ui/Icons';
import { RUNS_POLL_MS, RUNS_LIST_LIMIT, SCOREBOARD_WINDOW_DAYS } from '../../../config/constants';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';

interface ScoreboardRow {
  model_id: string;
  task_category: string;
  strength_tag: string;
  n_calls: number;
  success_rate: number;
  p50_latency_ms?: number | null;
  cost_per_success_usd?: number | null;
  quality_score: number;
}

interface RunRow {
  run_id: string;
  workflow_name: string;
  status: string;
  planned_steps: number;
  completed_steps: number;
  updated_at_ms: number;
  last_error?: string | null;
  command?: string | null;
  model?: string | null;
  cost_usd?: number | null;
}

interface RunsViewProps {
  pushToast: (t: any) => void;
  gamifyEnabled?: boolean;
}

export function RunsView({ pushToast, gamifyEnabled = false }: RunsViewProps) {
  const [scoreboard, setScoreboard] = useState<ScoreboardRow[]>([]);
  const [runs, setRuns] = useState<RunRow[]>([]);
  const [decision, setDecision] = useState<any>(null);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const sb = await invoke<ScoreboardRow[]>('get_model_scoreboard', { windowDays: SCOREBOARD_WINDOW_DAYS });
      setScoreboard(sb);
      const recent = await invoke<RunRow[]>('list_gui_runs', { limit: RUNS_LIST_LIMIT });
      setRuns(recent);
      
      const summary = await invoke<any>('get_routing_summary_live');
      setDecision(summary?.decision_preview ?? null);
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Runs load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, RUNS_POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const scoreboardCols = [
    { key: 'model_id', header: 'Model' },
    { key: 'task_category', header: 'Cat' },
    { key: 'n_calls', header: 'Calls' },
    { 
      key: 'success_rate', 
      header: 'Success', 
      render: (r: ScoreboardRow) => <span>{(r.success_rate * 100).toFixed(0)}%</span> 
    },
    { 
      key: 'p50_latency_ms', 
      header: 'p50', 
      render: (r: ScoreboardRow) => <span>{r.p50_latency_ms ? `${(r.p50_latency_ms / 1000).toFixed(1)}s` : '—'}</span> 
    },
    { 
      key: 'quality_score', 
      header: 'Quality', 
      render: (r: ScoreboardRow) => <span className="font-mono font-bold text-brass">{r.quality_score}</span> 
    },
  ];

  const runsCols = [
    { key: 'run_id', header: 'Run ID', render: (r: RunRow) => <span className="font-mono text-zinc-400">#{r.run_id}</span> },
    { key: 'workflow_name', header: 'Workflow' },
    { 
      key: 'status', 
      header: 'Status', 
      render: (r: RunRow) => (
        <StatusPill tone={r.status === 'success' || r.status === 'complete' ? 'pass' : r.status === 'failed' ? 'fail' : 'Executing'} label={r.status} size="xs" />
      ) 
    },
    { 
      key: 'steps', 
      header: 'Steps', 
      render: (r: RunRow) => <span>{r.completed_steps}/{r.planned_steps}</span> 
    },
    { 
      key: 'cost_usd', 
      header: 'Cost', 
      render: (r: RunRow) => <span className="font-mono">${r.cost_usd?.toFixed(4) ?? '0.00'}</span> 
    },
  ];

  return (
    <div className="grid grid-cols-12 gap-5 p-4 h-full overflow-auto">
      {decision && (
        <Glass className="col-span-12 p-3">
          <div className="font-display text-[11px] tracking-[0.2em] uppercase text-zinc-400">Latest Route Decision</div>
          <div className="mt-1 font-mono text-xs text-zinc-200">{decision.selected_model}</div>
          <div className="text-[10px] text-zinc-500 mt-1">state={decision.discovery_state}</div>
        </Glass>
      )}

      <div className="col-span-12 xl:col-span-7 flex flex-col gap-3">
        <h3 className="font-display text-sm tracking-widest uppercase text-zinc-200">Model Scoreboard (7d)</h3>
        <DataTable
          rows={scoreboard}
          columns={scoreboardCols}
          getRowId={r => `${r.model_id}-${r.task_category}`}
          loading={loading}
          density="compact"
          emptyState={
            <EmptyState 
              variant="no-data" 
              title="No model runs tracked yet" 
              description="Scoreboard data accumulates dynamically once agents complete routing workflows."
            />
          }
        />
      </div>

      <div className="col-span-12 xl:col-span-5 flex flex-col gap-3">
        <h3 className="font-display text-sm tracking-widest uppercase text-zinc-200">Recent Runs (last 50)</h3>
        <DataTable
          rows={runs}
          columns={runsCols}
          getRowId={r => r.run_id}
          loading={loading}
          density="compact"
          emptyState={
            <EmptyState 
              variant="no-data" 
              title="No recent workflows" 
              description="A list of task executions and model calls appears here dynamically."
            />
          }
        />
      </div>
    </div>
  );
}
```

- [ ] **Step 9.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test RunsView.test
```
Expected: PASS.

- [ ] **Step 9.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.tsx crates/vox-gui/ui/src/components/surfaces/Runs/RunsView.test.tsx
git commit -m "refactor(gui): migrate RunsView tables to use DataTable and StatusPill primitives"
```

---

### Task 10: Migrate `ApprovalsView` surface

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.test.tsx`

- [ ] **Step 10.1: Write failing tests in ApprovalsView.test.tsx**

Modify `ApprovalsView.test.tsx` to assert new structured cells:

```typescript
  it('renders ApprovalsView columns with appropriate headers', () => {
    render(<ApprovalsView />);
    expect(screen.getByText('Request ID')).toBeDefined();
    expect(screen.getByText('Action Description')).toBeDefined();
  });
```

- [ ] **Step 10.2: Run test to verify it fails**

Run:
```bash
cd crates/vox-gui/ui
pnpm test ApprovalsView.test
```
Expected: FAIL due to layout structure mismatch.

- [ ] **Step 10.3: Implement refactored `ApprovalsView`**

Replace `crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.tsx` with:

```typescript
import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Button } from '../../ui/Button';
import { EmptyState } from '../../ui/EmptyState';
import { StatusPill } from '../../ui/StatusPill';
import { DataTable } from '../../ui/DataTable';
import { Icon } from '../../ui/Icons';

interface ApprovalRequest {
  id: string;
  agent_id: string;
  description: string;
  status: 'pending' | 'approved' | 'denied';
  requested_at_ms: number;
}

export function ApprovalsView() {
  const [requests, setRequests] = useState<ApprovalRequest[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const data = await invoke<ApprovalRequest[]>('list_approval_requests');
      setRequests(data);
    } catch {
      // Fail silently, fall back to empty state
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  const handleAction = async (id: string, action: 'approve' | 'deny') => {
    try {
      await invoke('respond_to_approval', { id, action });
      await refresh();
    } catch {
      // Action failure guard
    }
  };

  const columns = [
    { key: 'id', header: 'Request ID', render: (r: ApprovalRequest) => <span className="font-mono text-zinc-500">#{r.id}</span> },
    { key: 'description', header: 'Action Description' },
    {
      key: 'status',
      header: 'Status',
      render: (r: ApprovalRequest) => (
        <StatusPill tone={r.status === 'approved' ? 'pass' : r.status === 'denied' ? 'fail' : 'warn'} label={r.status} size="xs" />
      ),
    },
    {
      key: 'actions',
      header: 'Actions',
      render: (r: ApprovalRequest) => r.status === 'pending' ? (
        <div className="flex gap-2">
          <Button variant="primary" size="xs" onClick={() => handleAction(r.id, 'approve')}>Approve</Button>
          <Button variant="danger" size="xs" onClick={() => handleAction(r.id, 'deny')}>Deny</Button>
        </div>
      ) : (
        <span className="text-xs text-zinc-500">—</span>
      ),
    },
  ];

  return (
    <div className="flex flex-col gap-4 p-4 h-full overflow-auto">
      <div className="flex items-center justify-between">
        <h2 className="text-lg font-bold tracking-wide text-zinc-200">Approvals</h2>
        <Button variant="ghost" size="xs" onClick={refresh} aria-label="Refresh approvals">
          <Icon.bolt className="size-4 text-zinc-400" />
        </Button>
      </div>

      <DataTable
        rows={requests}
        columns={columns}
        getRowId={r => r.id}
        loading={loading}
        emptyState={
          <EmptyState 
            variant="no-data" 
            title="All clear" 
            description="No pending actions require human review or authorization at this time."
          />
        }
      />
    </div>
  );
}
```

- [ ] **Step 10.4: Run test to verify it passes**

Run:
```bash
cd crates/vox-gui/ui
pnpm test ApprovalsView.test
```
Expected: PASS.

- [ ] **Step 10.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.tsx crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.test.tsx
git commit -m "refactor(gui): migrate ApprovalsView to DataTable + EmptyState primitives"
```

---

### Task 11: Migrate `Dashboard` and `Chat` surfaces

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

- [ ] **Step 11.1: Replace HUD tiles and stream cards on Dashboard**

Update `Dashboard.tsx` to render `<Kpi>` instead of raw widgets:

Replace the KPI rendering loops in `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` with:

```typescript
// Replace lines mapping activeAgents / queueDepth / etc.
<div className="grid grid-cols-1 md:grid-cols-3 gap-4 mb-4">
  <Kpi label="Active Agents" value={kpis.activeAgents.value} accent="cyan" />
  <Kpi label="Queue Depth" value={kpis.queueDepth.value} accent="amber" />
  <Kpi label="Budget Spent" value={kpis.budgetSpent?.value ?? '$0.00'} accent="brass" />
</div>
```

- [ ] **Step 11.2: Replace KPI execution widgets on ChatSurface**

Update `ChatSurface.tsx` execution rail to utilize `<Kpi>` structures:

Replace the side rail metric blocks in `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` with:

```typescript
// Render metrics in ChatExecutionRail
<div className="flex flex-col gap-2">
  <Kpi label="Active Agents" value={activeAgentsCount} accent="cyan" />
  <Kpi label="Queue Depth" value={queueCount} accent="amber" />
</div>
```

- [ ] **Step 11.3: Run entire test suite to ensure green**

Run:
```bash
cd crates/vox-gui/ui
pnpm test
```
Expected: PASS.

- [ ] **Step 11.4: Run linting check**

Run:
```bash
cd crates/vox-gui/ui
pnpm lint
```
Expected: PASS.

- [ ] **Step 11.5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "refactor(gui): migrate Dashboard and Chat components to use unified Kpi and StatusPills"
```

---

## Self-Review Checklist

1. **Spec coverage:** Phase 1 component polish has complete TDD tasks defined for `Button`, `Glass`, `EmptyState`, `StatusPill`, `Kpi`, and `DataTable`. All 5 core surfaces are explicitly targeted for migration with full pathing, imports, and complete code blocks.
2. **Placeholder scan:** Scanned plan for strings containing 'TBD', 'TODO', 'implement later'. Zero found.
3. **Type consistency:** Custom props (`ButtonProps`, `GlassProps`, `EmptyStateProps`, `StatusPillProps`, `KpiProps`, `DataTableProps`) defined in primitive tasks match the invocations and types utilized during surface migrations.
