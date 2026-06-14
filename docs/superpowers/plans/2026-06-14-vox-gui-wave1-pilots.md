# vox-gui Wave 1 — Pilots (App Shell, Dashboard, Settings) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Apply the per-surface 24-item checklist to the three pilot surfaces (Dashboard cluster, App Shell, Settings), exercising every principle end-to-end and validating the Phase 0 foundations before scaling to Waves 2–6.

**Architecture:** Phase 0 established: VoxTransport IPC hub, useVoxQuery/useVoxMutation, `<Async>` wrapper, Button/Dialog/Skeleton/Toasts primitives, global focus-visible + prefers-reduced-motion, useVirtualList. Wave 1 applies these to the three pilot surfaces. Settings IPC migration is scoped to the two methods already on VoxTransport (getGuiPreference/setGuiPreference); the other 24 Settings invoke calls are deferred to Wave 6.

**Tech Stack:** React 19, TypeScript 5, Vite 6, @tanstack/react-query v5, @radix-ui/react-slot, Tailwind 3.4, vitest 2, @testing-library/react, @testing-library/user-event, pnpm. Tauri v2.

**Source of truth:** spec [`docs/superpowers/specs/2026-06-14-vox-gui-design-principles-application-design.md`](../specs/2026-06-14-vox-gui-design-principles-application-design.md); 24-item checklist in §"The canonical per-surface checklist".

> **All commands run from `crates/vox-gui/ui/` unless noted.** pnpm only, never npm. Tests: `pnpm test`. Typecheck: `pnpm typecheck`. Git: from `C:/Users/Owner/vox/.worktrees/vox-gui-design-principles` (worktree root).

> **Existing test baseline:** 50 test files, 237 tests, all passing. Every task must leave tests green.

---

## Verified baseline (from audit, 2026-06-14)

| Surface | File | LoC | Direct invoke() | ARIA attrs | type="button" | Tests |
|---------|------|-----|-----------------|------------|---------------|-------|
| App Shell | App.tsx | 1,079 | ~12 | 0 | ~0 explicit | None |
| Dashboard | Dashboard.tsx | 125 | 0 | 0 | 0 | None |
| Dashboard | StreamCard.tsx | 54 | 0 | 0 | 0 | None |
| Dashboard | AgentRow.tsx | 93 | 0 | 0 | 0 | None |
| Settings | SettingsView.tsx | 1,398 | ~26 | 1 | 0 explicit | None |

**Phase 0 primitives available:**
- `src/components/ui/Button.tsx` — accessible button, asChild, forwardRef
- `src/components/ui/Async.tsx` — idle/pending/error/empty/success wrapper
- `src/components/ui/Skeleton.tsx` — shimmer loading placeholder
- `src/components/ui/EmptyState.tsx` — deliberate empty state with action
- `src/components/ui/Dialog.tsx` — Radix dialog (focus-trap, Esc, aria-modal)
- `src/components/ui/Toasts.tsx` — aria-live="polite" toast region
- `src/hooks/useVoxQuery.ts` — TanStack Query wrapper
- `src/transport.ts` — VoxTransport singleton (getGuiPreference, setGuiPreference wired)

---

## Scope

**In scope (Wave 1):**
- Dashboard: a11y (aria-label icon buttons, type="button"), EmptyState primitive, tests
- App Shell: a11y (type="button", aria-label icon nav buttons), loading/empty state checks
- Settings: a11y (type="button" on 40+ buttons, aria-label icons, aria-live save feedback), IPC migration of getGuiPreference/setGuiPreference → voxTransport

**Out of scope (later waves):**
- Settings: 24 other invoke() calls (get_orchestrator_config, set_user_config, etc.) → Wave 6
- App.tsx: full TanStack Query migration of bootstrap calls → Wave 6
- Playwright e2e for these surfaces (vitest unit tests only in Wave 1)
- dockview/xterm/@xyflow a11y (third-party, best-effort)

---

## File Structure

| File | Status | Responsibility |
|------|--------|---------------|
| `src/components/surfaces/Dashboard/Dashboard.tsx` | **Modify** | Use EmptyState primitive, Button primitive |
| `src/components/surfaces/Dashboard/StreamCard.tsx` | **Modify** | aria-label on icon buttons, type="button", Button primitive |
| `src/components/surfaces/Dashboard/AgentRow.tsx` | **Modify** | aria-label on icon buttons, type="button", Button primitive |
| `src/components/surfaces/Dashboard/Dashboard.test.tsx` | **Create** | Dashboard render tests |
| `src/components/surfaces/Dashboard/StreamCard.test.tsx` | **Create** | StreamCard render + a11y tests |
| `src/components/surfaces/Dashboard/AgentRow.test.tsx` | **Create** | AgentRow render + a11y tests |
| `src/App.test.tsx` | **Create** | App shell smoke tests |
| `src/components/surfaces/Settings/SettingsView.tsx` | **Modify** | type="button", aria-label, aria-live, voxTransport IPC |

---

## Task 1: Dashboard cluster a11y + tests

**What and why:** Dashboard has 0 ARIA attributes and no tests. StreamCard's doubt/overrule icon buttons and AgentRow's pause/resume/console icon buttons are completely unlabeled — screen readers can't describe them. All buttons lack explicit `type="button"`.

**Key observations from reading the files:**
- `StreamCard.tsx` (~54 lines): two icon-only action buttons — doubt (❓-like icon) and overrule (⚠-like icon). These are the primary interaction points.
- `AgentRow.tsx` (~93 lines): three icon-only buttons — pause, resume, "open in console". Also shows progress bars (inline style — legitimate, keep).
- `Dashboard.tsx` (~125 lines): uses custom `EmptyHint()` for empty states (two of them: no agents, no events). Should use `<EmptyState>` primitive.
- Dashboard receives `data`, `onPause`, `onResume`, `onDoubt`, `onOverrule`, `onAckLudus`, `filterKind`, `setFilterKind`, `onOpenInConsole` props.
- LudusBanner.tsx: alert banner — read and check if it has any icon buttons.

### Step 1.1 — Read the files

- [ ] Read `src/components/surfaces/Dashboard/StreamCard.tsx` (all lines)
- [ ] Read `src/components/surfaces/Dashboard/AgentRow.tsx` (all lines)
- [ ] Read `src/components/surfaces/Dashboard/Dashboard.tsx` (all lines)
- [ ] Read `src/components/surfaces/Dashboard/LudusBanner.tsx` (if it exists)
- [ ] Note exact button text/icons, prop names, and component structure

### Step 1.2 — Write failing tests first

Create `src/components/surfaces/Dashboard/Dashboard.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { Dashboard } from './Dashboard';
import type { DashboardData } from './Dashboard';  // adjust import to match actual export

const noopDashData: DashboardData = {
  agents: [],
  stream: [],
  alerts: [],
};

describe('Dashboard', () => {
  it('renders without crashing', () => {
    render(
      <Dashboard
        data={noopDashData}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind=""
        setFilterKind={vi.fn()}
      />
    );
    // Should render the Dashboard surface heading or top-level container
    expect(document.body.firstChild).not.toBeNull();
  });

  it('shows empty state when no agents are running', () => {
    const { container } = render(
      <Dashboard
        data={{ agents: [], stream: [], alerts: [] } as DashboardData}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind=""
        setFilterKind={vi.fn()}
      />
    );
    // Should show some empty state indicator when agents is empty
    expect(container.textContent).toBeDefined();
  });
});
```

Create `src/components/surfaces/Dashboard/StreamCard.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { StreamCard } from './StreamCard';  // adjust to actual export

// Adjust the item type to match actual StreamItem from StreamCard.tsx after reading it
const mockItem = {
  id: 'item-1',
  kind: 'agent_output',
  label: 'Test agent output',
  body: 'Did something useful',
  ts: Date.now(),
  agentId: 'agent-1',
};

describe('StreamCard', () => {
  it('renders the item body', () => {
    render(
      <StreamCard
        item={mockItem as any}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
      />
    );
    // Should render the item content
    expect(document.body.textContent).toContain('Did something useful');
  });

  it('doubt button has aria-label', () => {
    render(
      <StreamCard
        item={mockItem as any}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
      />
    );
    const doubtBtn = screen.queryByLabelText(/doubt/i);
    expect(doubtBtn).not.toBeNull();
  });

  it('overrule button has aria-label', () => {
    render(
      <StreamCard
        item={mockItem as any}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
      />
    );
    const overruleBtn = screen.queryByLabelText(/overrule/i);
    expect(overruleBtn).not.toBeNull();
  });
});
```

Create `src/components/surfaces/Dashboard/AgentRow.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';
import { AgentRow } from './AgentRow';  // adjust to actual export

const mockAgent = {
  id: 'agent-1',
  name: 'Test Agent',
  status: 'running',
  progress: 0.5,
  budget: { used: 100, limit: 1000 },
};

describe('AgentRow', () => {
  it('renders agent name', () => {
    render(
      <AgentRow
        agent={mockAgent as any}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onOpenInConsole={vi.fn()}
      />
    );
    expect(screen.getByText('Test Agent')).toBeDefined();
  });

  it('pause/resume button has aria-label', () => {
    render(
      <AgentRow
        agent={mockAgent as any}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onOpenInConsole={vi.fn()}
      />
    );
    // At least one of pause or resume button should have an aria-label
    const pauseBtn = screen.queryByLabelText(/pause/i) ?? screen.queryByLabelText(/resume/i);
    expect(pauseBtn).not.toBeNull();
  });

  it('open in console button has aria-label', () => {
    render(
      <AgentRow
        agent={mockAgent as any}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onOpenInConsole={vi.fn()}
      />
    );
    const consoleBtn = screen.queryByLabelText(/console/i);
    expect(consoleBtn).not.toBeNull();
  });
});
```

> **IMPORTANT:** Run `pnpm test --run` after writing tests — expect failures (aria-label missing, imports may need adjusting). Read the actual files to adjust mock data types before proceeding to implementation. The test mock data shapes must match the actual TypeScript types exactly.

### Step 1.3 — Implement a11y fixes

**In `StreamCard.tsx`:**
- Find every `<button>` element — add `type="button"` if missing
- Find icon-only buttons — add `aria-label="Doubt this action"` and `aria-label="Overrule this action"` (adjust text to match context)
- Add `aria-hidden="true"` to any icon/SVG inside labeled buttons
- Optionally wrap with `Button` primitive: `import { Button } from '../../../components/ui/Button';` (if the existing button structure is simple enough)

**In `AgentRow.tsx`:**
- Find every `<button>` — add `type="button"`
- Pause button: `aria-label="Pause agent"` (or `aria-label={`Pause ${agent.name}`}`)
- Resume button: `aria-label="Resume agent"` (or `aria-label={`Resume ${agent.name}`}`)
- Console button: `aria-label="Open in console"`
- Add `aria-hidden="true"` to icons inside buttons
- Keep the progress bar inline styles — they are legitimate dynamic values

**In `Dashboard.tsx`:**
- Replace `EmptyHint()` inline components with `<EmptyState>`:
  ```tsx
  import { EmptyState } from '../../ui/EmptyState';
  // Replace:
  // <div>No active agents</div>
  // With:
  // <EmptyState message="No active agents" />
  // (adjust to match EmptyState's actual props — read src/components/ui/EmptyState.tsx first)
  ```
- If EmptyState doesn't fit (wrong API or layout), keep the custom inline but add `role="status"` to indicate it's informational

### Step 1.4 — Run tests + fix until green

```
pnpm test --run
pnpm typecheck
```

All 3 new test files must pass. All existing 237 tests must pass. Fix any type errors (mock data shapes).

Expected counts: ≥ 237 + new tests (adjust based on actual test count per file).

### Step 1.5 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx \
  crates/vox-gui/ui/src/components/surfaces/Dashboard/StreamCard.tsx \
  crates/vox-gui/ui/src/components/surfaces/Dashboard/AgentRow.tsx \
  crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx \
  crates/vox-gui/ui/src/components/surfaces/Dashboard/StreamCard.test.tsx \
  crates/vox-gui/ui/src/components/surfaces/Dashboard/AgentRow.test.tsx

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "feat(vox-gui/wave1): Dashboard a11y (aria-label icon buttons, type=button) + tests (Wave 1 T1)"
```

---

## Task 2: App Shell a11y audit

**What and why:** App.tsx is 1,079 lines orchestrating all surfaces. Most UI buttons live in child surfaces (not App.tsx directly). The shell-level fixes are: explicit `type="button"` on any buttons in App.tsx, aria-label on any icon-only shell nav buttons, and a basic smoke test.

**Key observations:**
- App.tsx has global keyboard shortcuts (Ctrl+K, Ctrl+B, Ctrl+Shift+H) — already working, no change needed
- Most buttons in App.tsx are likely passed as callbacks; check for any direct button JSX in the shell
- Create a minimal smoke test that verifies the app renders without crashing

### Step 2.1 — Read App.tsx

- [ ] Read `src/App.tsx` (focus on any `<button>` JSX in the shell wrapper, not inside surface components)
- [ ] Note any shell-level nav/toggle buttons that are icon-only

### Step 2.2 — Write the smoke test first

Create `src/App.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeAll } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';

// Mock all Tauri APIs that App.tsx calls on mount
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue(null),
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

// Import App after mocks
import App from './App';

describe('App shell', () => {
  it('renders without crashing', () => {
    // App.tsx will call invoke on mount; we've mocked it to return null
    expect(() => render(<App />)).not.toThrow();
  });
});
```

Run `pnpm test --run` — this should pass (or at worst warn about unresolved promises). If App.tsx imports additional modules that need mocking, add them. The goal is ONE passing smoke test.

### Step 2.3 — Shell a11y fixes in App.tsx

- [ ] Search App.tsx for `<button` — add `type="button"` to any found
- [ ] Search for icon-only `<button` in the shell (not inside surface components) — add `aria-label`
- [ ] Search for `<Icon.` inside `<button>` — add `aria-hidden="true"` to icons inside labeled buttons
- [ ] Do NOT restructure state management — minimal safe a11y fixes only

### Step 2.4 — Run tests + typecheck

```
pnpm test --run
pnpm typecheck
```

### Step 2.5 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/src/App.tsx \
  crates/vox-gui/ui/src/App.test.tsx

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "feat(vox-gui/wave1): App shell a11y (type=button, aria-label) + smoke test (Wave 1 T2)"
```

---

## Task 3: Settings a11y pass — type="button" + aria-label + aria-live

**What and why:** SettingsView.tsx has ~40+ buttons with no explicit `type="button"` and only 1 ARIA attribute in 1,398 lines. Save feedback messages have no `aria-live` region — screen readers don't announce saves. Icon-only section action buttons have no labels.

**Key observations from the audit:**
- `Toggle` component (internal): renders a `<button>` — needs `type="button"` + `aria-label`
- `RangeInline` component (internal): renders `<input type="range">` — already labeled; check the label wording
- `MeshPeersSection`: has trust/untrust buttons — need aria-label + type="button"
- `SigningKeysSection`: has rotate-key button — need aria-label + type="button"
- `KeysSecretsSection`: has set/delete/import buttons — need aria-label + type="button"
- `RuntimeConfigSection`: has reset buttons — need aria-label + type="button"
- Save feedback: when `set_orchestrator_config` resolves, there's likely a success/error state — wrap in `role="status"` region
- The file is 1,398 lines — read it fully before editing, don't guess

### Step 3.1 — Read SettingsView.tsx

- [ ] Read `src/components/surfaces/Settings/SettingsView.tsx` in full
- [ ] Catalog every `<button` element: line number, current attributes, what it does
- [ ] Catalog every save/feedback UI element (where saves are confirmed)
- [ ] Note any section-level heading hierarchy (h2, h3, etc.)

### Step 3.2 — Write failing tests first

Create `src/components/surfaces/Settings/SettingsView.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockImplementation((cmd: string) => {
    if (cmd === 'get_orchestrator_config') return Promise.resolve({});
    if (cmd === 'get_gamify_settings') return Promise.resolve({ enabled: false, mode: 'off' });
    if (cmd === 'get_gui_preference') return Promise.resolve(null);
    if (cmd === 'get_orchestrator_status_bin') return Promise.resolve(null);
    if (cmd === 'list_secret_status') return Promise.resolve([]);
    if (cmd === 'secrets_backend_status') return Promise.resolve({ mode: 'none' });
    if (cmd === 'list_trusted_nodes') return Promise.resolve([]);
    if (cmd === 'signing_key_status') return Promise.resolve({ has_key: false });
    if (cmd === 'get_llm_config') return Promise.resolve({});
    if (cmd === 'openrouter_key_status') return Promise.resolve({ configured: false });
    return Promise.resolve(null);
  }),
}));

import { SettingsView } from './SettingsView';

describe('SettingsView', () => {
  it('renders the Settings heading', () => {
    render(<SettingsView pushToast={vi.fn()} />);
    // Adjust text to match actual heading in the file
    expect(document.body.textContent).toContain('Settings');
  });

  it('settings search input has aria-label', () => {
    render(<SettingsView pushToast={vi.fn()} />);
    // Already confirmed aria-label="Search settings" exists
    const searchInput = screen.getByLabelText('Search settings');
    expect(searchInput).toBeDefined();
  });

  it('all save/action areas have accessible live regions', () => {
    render(<SettingsView pushToast={vi.fn()} />);
    // After Wave 1, there should be at least one aria-live region for feedback
    const liveRegions = document.querySelectorAll('[aria-live]');
    expect(liveRegions.length).toBeGreaterThan(0);
  });
});
```

> NOTE: The mock list above is a starting point. After reading SettingsView.tsx, add any missing invoke commands to the mock. The test will fail until Step 3.3 adds aria-live.

### Step 3.3 — Implement a11y fixes in SettingsView.tsx

**Pass 1 — type="button" on every `<button>`:**

Every `<button>` in the file that doesn't already have `type="submit"` or `type="reset"` needs `type="button"`. Do a systematic pass through the file.

**Pass 2 — aria-label on icon-only buttons:**

For every button that renders only an icon (no visible text), add a meaningful `aria-label`. Examples:
- Save/apply buttons: `aria-label="Apply settings"` or `aria-label="Save orchestrator config"`
- Reset buttons: `aria-label="Reset to default"`
- Trust/untrust peer buttons: `aria-label={`Trust ${peer.name}`}` / `aria-label={`Remove peer ${peer.name}`}`
- Rotate key: `aria-label="Rotate signing key"`
- Delete secret: `aria-label={`Delete secret ${key}`}`
- Section toggle/expand: `aria-label="Expand signing keys"` / `aria-label={aria-expanded ? 'Collapse' : 'Expand'}`

If a button has both an icon AND text, add `aria-hidden="true"` to the icon only.

**Pass 3 — aria-live for save feedback:**

Find where save status is displayed (e.g., "Saved!", "Error saving", success/error banners). Wrap these in a container with `aria-live="polite"` and `role="status"`:

```tsx
<div role="status" aria-live="polite" aria-atomic="true">
  {saveStatus && <span className="text-xs text-green-400">{saveStatus}</span>}
</div>
```

If save status is shown per-section (e.g., orchestrator config, gamify, LLM), add one `aria-live` region per section that shows feedback.

**Pass 4 — form label audit:**

For every `<input>` element:
- If it has a `<label>` or `aria-label` or `aria-labelledby` — it's fine
- If it uses `placeholder` as the only label — add a proper `<label>` or `aria-label`
- Range inputs: should have `aria-label` or a visible label

### Step 3.4 — Run tests + typecheck

```
pnpm test --run
pnpm typecheck
```

All 3 new SettingsView tests must pass. Existing tests must stay green.

### Step 3.5 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx \
  crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.test.tsx

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "feat(vox-gui/wave1): Settings a11y (type=button, aria-label, aria-live) + tests (Wave 1 T3)"
```

---

## Task 4: Settings IPC cleanup — migrate getGuiPreference / setGuiPreference to voxTransport

**What and why:** SettingsView.tsx calls `invoke('get_gui_preference', ...)` and `invoke('set_gui_preference', ...)` directly, bypassing the VoxTransport hub that Phase 0B established. These two IPC methods are already on `voxTransport` (typed, tested). Migrating them removes 4–6 direct invoke bypasses.

**Scope:** ONLY `get_gui_preference` and `set_gui_preference` invoke calls. All other Settings invoke calls (get_orchestrator_config, set_user_config, etc.) are NOT yet in VoxTransport and must be left as-is (deferred to Wave 6).

**Pattern:**
```typescript
// Before (bypass):
import { invoke } from '@tauri-apps/api/core';
const theme = await invoke<string | null>('get_gui_preference', { key: 'gui.theme' });
await invoke('set_gui_preference', { key: 'gui.theme', value: newTheme });

// After (VoxTransport):
import { voxTransport } from '../../../transport';
const theme = await voxTransport.getGuiPreference('gui.theme');
await voxTransport.setGuiPreference('gui.theme', newTheme);
```

### Step 4.1 — Find all getGuiPreference / setGuiPreference invoke calls in SettingsView.tsx

```
grep -n "get_gui_preference\|set_gui_preference" src/components/surfaces/Settings/SettingsView.tsx
```

Expected: several lines. Note each one.

### Step 4.2 — Replace each one

For each `invoke('get_gui_preference', { key: X })`:
→ Replace with `voxTransport.getGuiPreference(X)` (same return type: `Promise<string | null>`)

For each `invoke('set_gui_preference', { key: X, value: Y })`:
→ Replace with `voxTransport.setGuiPreference(X, Y)` (returns `Promise<void>`)

### Step 4.3 — Remove the @tauri-apps/api/core import if now unused

Check if `invoke` is still used anywhere else in the file. If all `invoke` usages have been migrated, remove the import. If some remain (the other 24 calls), keep the import.

### Step 4.4 — Verify tests + typecheck

```
pnpm test --run
pnpm typecheck
```

No test regressions. TypeScript clean.

### Step 4.5 — Commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" add \
  crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx

git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit -m \
  "refactor(vox-gui/wave1): Settings getGuiPreference/setGuiPreference → voxTransport (Wave 1 T4)"
```

---

## Task 5: Wave 1 verification gate

**What and why:** Confirm all Wave 1 changes leave the repo in a clean, shippable state.

### Step 5.1 — Run full test suite + typecheck

```
pnpm test --run 2>&1 | tail -20
pnpm typecheck
```

Expected: all tests pass, typecheck clean.

### Step 5.2 — Self-review checklist

- [ ] Dashboard: all icon-only buttons have `aria-label`
- [ ] Dashboard: all `<button>` have `type="button"`
- [ ] Dashboard: tests exist (Dashboard.test.tsx, StreamCard.test.tsx, AgentRow.test.tsx)
- [ ] App Shell: smoke test in App.test.tsx
- [ ] App Shell: any shell-level buttons have `type="button"` + `aria-label` where needed
- [ ] Settings: ALL `<button>` have `type="button"`
- [ ] Settings: icon-only buttons have `aria-label`
- [ ] Settings: at least one `aria-live` region for save feedback
- [ ] Settings: `get_gui_preference` + `set_gui_preference` → voxTransport (4–6 calls migrated)
- [ ] Settings: tests exist (SettingsView.test.tsx)
- [ ] RunsView / SearchView / TasksView / MemoryView NOT touched (already done in Phase 0D)
- [ ] No inline styles removed (legitimate dynamic styles preserved)
- [ ] Total tests: ≥ 237 + (new tests from T1–T3)

### Step 5.3 — Gate commit

```
git -C "C:/Users/Owner/vox/.worktrees/vox-gui-design-principles" commit --allow-empty -m \
  "chore(vox-gui): Wave 1 verification gate passed (N tests, typecheck clean)"
```

---

## Self-Review Checklist (after all tasks)

- [ ] All tests pass
- [ ] TypeScript typecheck passes (no errors)
- [ ] Dashboard: 3 test files created
- [ ] App Shell: smoke test created
- [ ] Settings: test file created
- [ ] All icon-only buttons have aria-label (Dashboard + Settings)
- [ ] All buttons have type="button" (Dashboard + Settings)
- [ ] aria-live region exists in Settings for save feedback
- [ ] Settings getGuiPreference/setGuiPreference migrated to voxTransport
- [ ] Five commits total (T1–T4 + gate)
