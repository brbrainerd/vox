# vox-gui Navigation and Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Complete cross-cutting IA — sidebar badges, Policies two-rail layout, Gamify IA move — building on landed breadcrumbs and hash deep links.

**Architecture:** `PARENT_CHILD_MAP` + `DEFAULT_CHILD_BY_PARENT` in `navigation.ts` drive breadcrumbs and parent navigation. `navigateTo` in `App.tsx` is the single hash-sync entry point (landed `e4da27e`). Badges consume existing `approvalsPending` and `policyBadge` state in App.

**Tech Stack:** React 19, Radix, vitest, Playwright.

> **Spec:** `docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md`

---

## Completed (do not redo)

- [x] `BreadcrumbBar.tsx` + tests
- [x] `parseViewFromLocation`, `viewToHash`, `syncViewToLocation`
- [x] `navigateTo` wired for sidebar, tabs, CustomEvent, `hashchange`
- [x] `DEFAULT_CHILD_BY_PARENT` (workspace → console, not repository)
- [x] `KNOWN_VIEWS` aligned with `LEGACY_VIEWS`

---

## Task 3: Sidebar badge aria-labels

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Create: `crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx`

- [ ] **Step 1: Write failing test**

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { Sidebar } from './Sidebar';

const baseProps = {
  view: 'dashboard',
  setView: vi.fn(),
  agentsCount: 2,
  data: { agents: [], stream: [], alerts: [], skills: [], peers: [], kpis: {} as any, contextChips: [] },
  mode: 'default' as const,
  setMode: vi.fn(),
  pushToast: vi.fn(),
  appVersion: '0.6.0',
  policyBadge: 0,
  approvalsPending: 3,
};

describe('Sidebar badges', () => {
  it('includes pending count in Runs nav aria-label', () => {
    render(<Sidebar {...baseProps} />);
    expect(screen.getByRole('button', { name: /Runs.*3 pending/i })).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test — FAIL**

Run: `pnpm exec vitest run src/components/layout/Sidebar.test.tsx`

- [ ] **Step 3: Implement**

In `Sidebar.tsx`, for the Runs parent button:

```tsx
aria-label={
  approvalsPending > 0
    ? `Runs and Approvals, ${approvalsPending} pending`
    : 'Runs and Approvals'
}
```

Repeat for Policies child when `policyBadge > 0`: `Policies, ${policyBadge} failing`.

- [ ] **Step 4: Run test — PASS**

- [ ] **Step 5: Commit** `feat(gui): accessible sidebar badge labels`

---

## Task 4: Policies two-rail layout

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.test.tsx`
- Create: `crates/vox-gui/ui/e2e/policies.spec.ts`

- [ ] **Step 1: Read spec** `docs/superpowers/specs/2026-06-06-unified-policy-registry-and-governance-surface-design.md` §Layout

- [ ] **Step 2: Failing test**

```typescript
it('renders policy tree rail and detail pane at xl', () => {
  render(<PoliciesView pushToast={vi.fn()} />);
  expect(screen.getByRole('navigation', { name: /policy tree/i })).toBeDefined();
  expect(screen.getByRole('region', { name: /policy detail/i })).toBeDefined();
});
```

- [ ] **Step 3: Layout structure**

```tsx
<div className="flex h-full min-h-0 gap-4">
  <aside
    className="hidden xl:block w-64 shrink-0 overflow-y-auto custom-scrollbar"
    aria-label="Policy tree"
  >
    <PolicyTree ... />
  </aside>
  <section className="flex-1 min-w-0 overflow-y-auto" aria-label="Policy detail">
    {/* existing detail / violations */}
  </section>
</div>
```

Mobile: tree collapses into existing select/dropdown (keep current behavior below `xl`).

- [ ] **Step 4: Playwright golden route**

```typescript
test('policies two-rail visible on desktop', async ({ page }) => {
  await page.setViewportSize({ width: 1400, height: 900 });
  // ... tauri mock + goto policies
  await expect(page.getByRole('navigation', { name: 'Policy tree' })).toBeVisible();
});
```

- [ ] **Step 5: Commit**

---

## Task 5: IA fixes (Gamify + Coverage)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- Modify: `crates/vox-gui/ui/src/lib/navigation.ts`
- Modify: `docs/src/reference/gui-navigation.md`

- [ ] **Step 1: Move Gamify under Agents**

Update `PARENT_CHILD_MAP`:

```typescript
gamify: { parent: 'agents', child: 'gamify' },
```

Remove from settings children in sidebar render order.

- [ ] **Step 2: Failing navigation test**

```typescript
it('gamify resolves under agents parent', () => {
  expect(resolveNavigation('gamify').parent).toBe('agents');
});
```

- [ ] **Step 3: Coverage link** — add Coverage under Settings with subtitle "CI surface gaps" in sidebar; link from TopHud when `policyBadge > 0` optional.

- [ ] **Step 4: Doc bootstrap section** in `gui-navigation.md`:

```markdown
## Developer bootstrap

vox ci gui-surface-registry --write
vox ci config-gui-codegen --write
cd crates/vox-gui/ui && pnpm install && pnpm build
cargo run -p vox-gui
vox ci gui-smoke
```

- [ ] **Step 5: Run** `cargo run -q -p vox-doc-pipeline -- --lint-only --paths docs/src/reference/gui-navigation.md`

- [ ] **Step 6: Commit**

---

## Exit criteria

- [ ] Sidebar badges include counts in `aria-label`
- [ ] Policies two-rail at `xl+`; Playwright route green
- [ ] Gamify under Agents in nav + `PARENT_CHILD_MAP`
- [ ] `gui-navigation.md` bootstrap section committed
- [ ] Shareable `#view=` URL round-trip tested in e2e
