# Universal Dock Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Also required per `subagent-driven-development`: `superpowers:requesting-code-review` (two-stage review after every task) and `superpowers:finishing-a-development-branch` (after Phase 3).

> **Supersedes an in-flight plan.** `docs/superpowers/plans/2026-07-20-dockview-panel-ux.md` has only its Task 1 landed (commit `1e689ed551` — the closed-panel-tracking fix). Its Tasks 2-6 (migrate Plan into dockview, extract a shared panel creator, add a reopen/reset Panels menu) are **absorbed into this plan's Phase 1** at the generalized-shell level and should be considered superseded, not executed separately — do not run that plan's remaining tasks; they'd duplicate Phase 1 below and conflict on the same files. `docs/superpowers/plans/2026-07-20-chat-flow-docking-redesign.md`'s Phase B (B1-B5) is fully landed; its B6 (visual pass) is a verification-only checklist with no code, independent of this plan — do it whenever convenient, it doesn't block or get superseded by anything here.

**Goal:** Generalize the chat-only dockview shell into a reusable `DockWorkspaceShell`, pin the chat transcript as a fixed non-closable center, rename the Plan panel to "To-dos," quiet the dockview chrome, add a Panels launcher (reopen/reset/add), and wire in the 14 app surfaces confirmed dockable by this session's full-codebase audit — while leaving Dashboard, Settings, Flow, Catalog, Browser, Console, Policies, and Runs untouched (confirmed structurally incompatible with docking).

**Architecture:** Phase 1 builds and hardens the generalized primitive using Chat as the only consumer (safest place to iterate — it's the surface already exercising this code). Phase 2 wires in the 6 surfaces confirmed as strong dockable candidates (narrow-friendly as-is). Phase 3 wires in the 8 condensed-capable surfaces, each needing a bespoke narrow/full toggle. Phase 2 and 3 each open with one fully-worked example task establishing the exact pattern, then a table-driven set of per-surface tasks that follow that pattern — this avoids repeating near-identical multi-hundred-line code blocks 14 times while keeping every task's specifics (component names, condensed content) concrete, not placeholder.

**Tech Stack:** React 19, TypeScript, dockview 6.6.1 (`DockviewReact`'s `tabComponents` prop — confirmed via `IDockviewReactProps.tabComponents?: Record<string, FC<IDockviewPanelHeaderProps>>` in the library's own `.d.ts`, distinct from the `components` prop already in use), Vitest + Testing Library.

**Ground truth confirmed for this plan (re-verify per the note above if time has passed and other work landed on these files):**
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` (85 lines) has layout persistence (params-stripped, `LAYOUT_STORAGE_KEY = 'gui.chat.dockview_layout.v3'`) and is otherwise a thin wrapper — this is the file being generalized into `DockWorkspaceShell`.
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (517 lines): `CHAT_DOCK_COMPONENTS` (lines 55-60) currently has `sessions`/`transcript`/`executionRail`/`flow` — **no `plan` entry yet**, the Plan panel is still the old hand-rolled collapsible `<aside>` (search `planPanelCollapsed`). `closedPanelIds` (a `useRef<Set<string>>`) and its `onDidRemovePanel` listener already exist (landed by the superseded plan's Task 1) — reuse this, don't recreate it.
- `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx` (139 lines): props are exactly `planSessionId`, `planVersion`, `nodes` — internal names stay as-is (renaming them is pure churn); only user-visible copy changes ("No active plan" → "No to-dos yet", "Start a task to see its plan here." → "Start a task to see its to-do list here.", "No plan steps yet." → "Nothing to do yet.").
- `crates/vox-gui/ui/src/styles/dockview-vox.css` (22 lines) sets `--dv-activegroup-visiblepanel-tab-background-color: rgba(212, 175, 55, 0.08)` (brass, low-opacity) already — the "blue" chrome the user saw is dockview's own **unthemed default** leaking through somewhere, not this file. Task 1.7 below investigates why.
- `IDockviewReactProps.tabComponents?: Record<string, React.FunctionComponent<IDockviewPanelHeaderProps>>` confirmed in `node_modules/.pnpm/dockview@6.6.1_react@19.2.7/node_modules/dockview/dist/esm/dockview/dockview.d.ts:9` — a distinct prop from `components`, used to override a specific panel's tab renderer. This is the pin-chat mechanism.
- No `alwaysShowTabs`/`hideTabsAndAction`-style single-panel-tab-suppression option was found in `dockview-core`'s `options.d.ts` (only `singleTabMode?: 'fullwidth' | 'default'`, which controls single-tab *styling*, not visibility). **Do not implement "tab row disappears when a group has 1 panel" as a promised feature — it's unverified.** Quiet chrome (Task 1.7) means visually smaller/lower-contrast, always present, not conditionally hidden.
- The 22-surface audit (strong/condensed/excluded categorization, with per-surface natural/minimum dimensions and condensed-content specs) lives in this conversation's transcript and is summarized in the design spec `docs/superpowers/specs/2026-07-21-universal-dock-workspace-design.md` — Phase 2/3's tables below extract the load-bearing specifics from it directly, so implementers don't need to re-read the whole transcript.

---

## Phase 1: Generalize the shell, pin chat, rename to To-dos, quiet chrome, Panels menu

### Task 1.1: Extract `DockWorkspaceShell` from `ChatDockShell`

**Files:**
- Create: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx`
- Create: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx`
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` (after Task 1.2 repoints its one consumer)
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx` (its tests move to the new file)

- [ ] **Step 1: Write the failing test**

Copy `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx`'s three existing tests into a new file, updating only the import path and the one storage-key literal used by the "restores a previously serialized layout" test, plus add a new test proving the key is now parameterized:

```tsx
// crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render } from '@testing-library/react';
import React from 'react';
import { DockviewApi } from 'dockview';
import { DockWorkspaceShell, layoutStorageKeyFor } from './DockWorkspaceShell';

describe('DockWorkspaceShell', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  it('mounts a dockview-theme-vox container and calls onReady with an api', () => {
    const onReady = vi.fn();
    const { container } = render(
      <DockWorkspaceShell storageKeyPrefix="test.host" onReady={onReady} components={{}} />,
    );
    expect(container.querySelector('.dockview-theme-vox')).not.toBeNull();
    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady.mock.calls[0][0]).toHaveProperty('api');
  });

  it('restores a previously serialized layout via fromJSON on mount, keyed per storageKeyPrefix', () => {
    const savedLayout = { grid: {} };
    localStorage.setItem(layoutStorageKeyFor('test.host'), JSON.stringify(savedLayout));

    const fromJSONSpy = vi.spyOn(DockviewApi.prototype, 'fromJSON').mockImplementation(() => {});
    const onReady = vi.fn();
    render(<DockWorkspaceShell storageKeyPrefix="test.host" onReady={onReady} components={{}} />);

    expect(fromJSONSpy).toHaveBeenCalledTimes(1);
    expect(fromJSONSpy).toHaveBeenCalledWith(savedLayout);
    fromJSONSpy.mockRestore();
  });

  it('two different storageKeyPrefix values persist to two different localStorage keys', () => {
    expect(layoutStorageKeyFor('gui.chat')).not.toBe(layoutStorageKeyFor('gui.other-host'));
  });

  it('never persists panel params (live React nodes) — only geometry survives the round trip', async () => {
    function Probe(props: { params: { node: React.ReactNode } }) {
      return <div>{props.params.node}</div>;
    }
    const onReady = vi.fn((event) => {
      event.api.addPanel({ id: 'probe', component: 'probe', params: { node: <span>live content</span> } });
    });
    render(<DockWorkspaceShell storageKeyPrefix="test.host2" onReady={onReady} components={{ probe: Probe }} />);

    await new Promise((resolve) => setTimeout(resolve, 1200)); // past LAYOUT_PERSIST_DEBOUNCE_MS (1000ms)

    const persisted = localStorage.getItem(layoutStorageKeyFor('test.host2'));
    expect(persisted).not.toBeNull();
    expect(persisted).not.toContain('"params"');
    expect(persisted).toContain('probe');
  }, 10000);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/dock/DockWorkspaceShell.test.tsx`
Expected: FAIL — `./DockWorkspaceShell` doesn't exist yet.

- [ ] **Step 3: Create `DockWorkspaceShell`**

```tsx
// crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx
import React, { useCallback, useRef } from 'react';
import {
  DockviewReact,
  type DockviewReadyEvent,
  type IDockviewPanelProps,
  type IDockviewPanelHeaderProps,
} from 'dockview';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../config/constants';

/**
 * Per-host localStorage key for a persisted dockview layout. Each host view
 * (Chat today; any future host) gets its own independent persisted layout —
 * `storageKeyPrefix` scopes it. Exported so callers (e.g. a "reset layout"
 * action) can compute the same key without duplicating the format string.
 */
export function layoutStorageKeyFor(storageKeyPrefix: string): string {
  // v3 versioning carried over from the chat-only predecessor
  // (ChatDockShell): v1/v2 persisted each panel's `params` verbatim, which
  // for every panel in this app is `{ node: <live React element> }`.
  // JSON.stringify silently drops a React element's `type` (a function) and
  // `$$typeof` (a Symbol), leaving a garbled `{key, ref, props}` object that
  // crashed on restore (React error #31) before the refresh mechanism ever
  // got a chance to overwrite it with a real node. Fixed by stripping
  // `params` before every write (see the replacer below) — the versioned
  // key format is kept for any future storage-shape change, not because v3
  // itself is expected to break.
  return `${storageKeyPrefix}.dockview_layout.v3`;
}

interface DockWorkspaceShellProps {
  /** Scopes this shell's persisted layout to one host view, e.g. `'gui.chat'`. */
  storageKeyPrefix: string;
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  /** Per-panel custom tab renderers, e.g. to hide a specific panel's tab
   * strip/close button/drag handle entirely (see the transcript-pinning use
   * in ChatSurface.tsx). Optional — omit for the default tab everywhere. */
  tabComponents?: Record<string, React.FunctionComponent<IDockviewPanelHeaderProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

/**
 * Reusable dockview shell for any host view's panel workspace. Theming via
 * the `dockview-theme-vox` class (crates/vox-gui/ui/src/styles/dockview-vox.css),
 * not the `theme` prop.
 *
 * Layout persistence: the dockview grid layout is serialized to
 * localStorage (debounced, params-stripped) on every change, and restored
 * on mount before the caller's `onReady` runs. Callers must guard their
 * `addPanel` calls with `if (!event.api.getPanel(id))` so a restored layout
 * doesn't get duplicate panels re-added.
 */
export function DockWorkspaceShell({
  storageKeyPrefix,
  components,
  tabComponents,
  onReady,
}: DockWorkspaceShellProps) {
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const storageKey = layoutStorageKeyFor(storageKeyPrefix);

  const handleReady = useCallback(
    (event: DockviewReadyEvent) => {
      const saved = window.localStorage.getItem(storageKey);
      if (saved) {
        try {
          event.api.fromJSON(JSON.parse(saved));
        } catch (err) {
          console.warn('failed to restore dockview layout, using default', err);
        }
      }

      event.api.onDidLayoutChange(() => {
        if (debounceRef.current) clearTimeout(debounceRef.current);
        debounceRef.current = setTimeout(() => {
          try {
            const serialized = JSON.stringify(event.api.toJSON(), (key, value) =>
              key === 'params' ? undefined : value,
            );
            window.localStorage.setItem(storageKey, serialized);
          } catch (err) {
            console.warn('failed to persist dockview layout', err);
          }
        }, LAYOUT_PERSIST_DEBOUNCE_MS);
      });

      onReady(event);
    },
    [onReady, storageKey],
  );

  return (
    <div className="dockview-theme-vox h-full min-h-[60vh] w-full">
      <DockviewReact components={components} tabComponents={tabComponents} onReady={handleReady} />
    </div>
  );
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/dock/DockWorkspaceShell.test.tsx`
Expected: PASS, all 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx
git commit -m "feat(gui): extract DockWorkspaceShell — reusable, per-host dockview shell generalized from ChatDockShell"
```

Do not delete `ChatDockShell.tsx` yet — Task 1.2 repoints its consumer first, then deletes it in the same commit as that repoint (so the tree never has two shells with drifted behavior).

### Task 1.2: Repoint `ChatSurface` onto `DockWorkspaceShell`, delete `ChatDockShell`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx`
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx`

- [ ] **Step 1: Confirm nothing else imports `ChatDockShell`**

Run: `grep -rln "ChatDockShell" crates/vox-gui/ui/src --include=*.tsx --include=*.ts`
Expected: only `ChatSurface.tsx` and the two files being deleted. If anything else appears, stop and investigate before deleting.

- [ ] **Step 2: Update the import and call site in `ChatSurface.tsx`**

Replace:
```tsx
import { ChatDockShell } from './ChatDockShell';
```
with:
```tsx
import { DockWorkspaceShell } from '../../dock/DockWorkspaceShell';
```

Replace the `<ChatDockShell components={CHAT_DOCK_COMPONENTS} onReady={...} />` usage with:
```tsx
<DockWorkspaceShell storageKeyPrefix="gui.chat" components={CHAT_DOCK_COMPONENTS} onReady={...} />
```
(keep the existing `onReady` callback body unchanged for this task — later tasks in this phase modify it).

- [ ] **Step 3: Delete the old shell files**

```bash
rm crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
```

- [ ] **Step 4: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions. `ChatSurface.test.tsx`'s existing dockview-panel tests must still pass unchanged (they assert on `chat-dock-*` testids, which don't depend on which shell component renders them).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
git commit -m "refactor(gui): ChatSurface uses the generalized DockWorkspaceShell, delete the chat-only predecessor"
```

(The `git add` on the two deleted files stages their removal. Run `git status` first to confirm only `ChatSurface.tsx`'s modification plus those two deletions are staged — nothing else.)

### Task 1.3: Migrate the Plan panel into the dock as "To-dos," delete the old collapse toggle

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx` (copy only)
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.test.tsx` (extend, if it exists — check first)

- [ ] **Step 1: Write the failing tests**

```tsx
// Add to ChatSurface.test.tsx
it('mounts the To-dos panel as a dockview panel, not a hand-rolled collapsible aside', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
      planSessionId="sess-1"
      planVersion={1}
    />,
  );
  expect(screen.getByTestId('chat-dock-todos')).toBeInTheDocument();
  expect(screen.queryByLabelText('Collapse plan panel')).toBeNull();
  expect(screen.queryByLabelText('Expand plan panel')).toBeNull();
});
```

```tsx
// Add to PlanPanel.test.tsx (check its current tests' render conventions first)
it('shows to-do-list-labeled empty and zero-step copy, not "plan" language', () => {
  render(<PlanPanel planSessionId="s1" planVersion={1} nodes={[]} />);
  expect(screen.getByText('Nothing to do yet.')).toBeInTheDocument();
});

it('shows a to-do-list-labeled empty state when there is no active session', () => {
  render(<PlanPanel planSessionId={null} planVersion={null} nodes={[]} />);
  expect(screen.getByText('No to-dos yet')).toBeInTheDocument();
  expect(screen.getByText('Start a task to see its to-do list here.')).toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx src/components/surfaces/Chat/PlanPanel.test.tsx`
Expected: FAIL — no `chat-dock-todos` testid, and the old "No active plan"/"Start a task to see its plan here."/"No plan steps yet." copy doesn't match.

- [ ] **Step 3: Update `PlanPanel.tsx`'s user-visible copy**

In `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx`, change only the three literal strings (keep every prop/type/function name — `PlanPanel`, `PlanNodeView`, `planSessionId`, `planVersion` — unchanged, per the plan header's note that renaming those would be pure churn):

```tsx
// Line ~89 area, inside the no-active-plan EmptyState:
<EmptyState
  title="No to-dos yet"
  description="Start a task to see its to-do list here."
/>
```

```tsx
// Line ~109 area, the empty-nodes-list message:
<p className="text-[11px] text-text-muted">Nothing to do yet.</p>
```

- [ ] **Step 4: Add the `TodosDockPanel` component and register it**

In `ChatSurface.tsx`, near the other panel components:

```tsx
function TodosDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-todos" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}
```

```tsx
const CHAT_DOCK_COMPONENTS = {
  sessions: SessionsPanel,
  transcript: TranscriptPanel,
  executionRail: ExecutionRailPanel,
  flow: FlowPanel,
  todos: TodosDockPanel,
};
```

- [ ] **Step 5: Build `todosNode` and add it to the refresh effect**

Near `flowNode`:

```tsx
const todosNode = <PlanPanel planSessionId={planSessionId} planVersion={planVersion} nodes={planNodes} />;
```

In the refresh `useEffect`, mirror the existing `flow` branch exactly (including the `closedPanelIds` guard already landed there):

```tsx
const todosPanel = api.getPanel('todos');
if (todosPanel) {
  todosPanel.update({ params: { node: todosNode } });
} else if (!closedPanelIds.current.has('todos')) {
  api.addPanel({
    id: 'todos',
    component: 'todos',
    title: 'To-dos',
    params: { node: todosNode },
    position: {
      direction: 'right',
      referencePanel: api.getPanel('flow') ? 'flow' : api.getPanel('executionRail') ? 'executionRail' : 'transcript',
    },
  });
}
```

- [ ] **Step 6: Delete the old Plan panel collapse-toggle code**

Remove: the `planPanelCollapsed`/`setPlanPanelCollapsed` `useLocalStorage` declaration (search `gui.chat.plan_panel_collapsed.v1`), and the entire old `{planPanelCollapsed ? <aside>...</aside> : <aside>...</aside>}` JSX block. Remove the `Glass` import only if nothing else in the file still uses `<Glass>` (`grep -n "Glass" crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` first).

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx src/components/surfaces/Chat/PlanPanel.test.tsx`
Expected: PASS, all tests including pre-existing ones.

- [ ] **Step 8: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass. Grep for any other reference to `plan_panel_collapsed`/`Collapse plan panel`/`Expand plan panel`/`"No active plan"`/`"No plan steps yet."` before committing.

- [ ] **Step 9: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.test.tsx
git commit -m "feat(gui): migrate Plan panel into the dock as To-dos (relabeled, not renamed internally — avoids colliding with the existing global Tasks surface)"
```

### Task 1.4: Extract a shared per-id panel creator with the real fallback chain

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

Tasks 1.2-1.3 left near-duplicate `addPanel` shapes inline in the refresh effect (executionRail/flow/todos), each with its own reference-panel fallback. Extract one function so the Task 1.5 Panels menu and Task 1.6 reset action don't reimplement (and potentially diverge from) that fallback logic.

- [ ] **Step 1: Write the failing test**

```tsx
it('recreates a closed panel via the shared per-id creator, preserving its reference-panel fallback', async () => {
  render(
    <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />,
  );
  await screen.findByTestId('chat-dock-flow');

  const flowTab = screen.getByText('Flow').closest('.dv-default-tab') as HTMLElement;
  fireEvent.click(flowTab.querySelector('.dv-default-tab-action') as HTMLElement);
  await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

  // Exercised directly here; Task 1.5's Panels-menu test supersedes this
  // with a real click-driven end-to-end version — don't keep both once
  // Task 1.5 lands, fold this assertion into that task's test instead.
});
```

- [ ] **Step 2: Add `DEFAULT_PANEL_IDS` and `addDefaultPanel`**

```tsx
const DEFAULT_PANEL_IDS = ['sessions', 'transcript', 'executionRail', 'flow', 'todos'] as const;
type ChatDockPanelId = (typeof DEFAULT_PANEL_IDS)[number];
```

Inside `ChatSurface`, after `todosNode` is built (so all five node variables are in scope):

```tsx
const panelDefs: Record<ChatDockPanelId, { title: string; node: React.ReactNode; referenceChain: ChatDockPanelId[] }> = {
  sessions: { title: 'Sessions', node: sessionRailNode, referenceChain: [] },
  transcript: { title: 'Chat', node: centerContent, referenceChain: ['sessions'] },
  executionRail: { title: 'Execution', node: executionRailNode, referenceChain: ['transcript'] },
  flow: { title: 'Flow', node: flowNode, referenceChain: ['executionRail', 'transcript'] },
  todos: { title: 'To-dos', node: todosNode, referenceChain: ['flow', 'executionRail', 'transcript'] },
};

// Plain function, not useCallback: panelDefs is a fresh object every
// render (it closes over freshly-built JSX), so a memoized wrapper would
// never actually skip recomputation.
const addDefaultPanel = (api: DockviewApi, id: ChatDockPanelId) => {
  const def = panelDefs[id];
  const referencePanel = def.referenceChain.find(candidateId => api.getPanel(candidateId));
  api.addPanel({
    id,
    component: id,
    title: def.title,
    params: { node: def.node },
    position: referencePanel ? { direction: 'right', referencePanel } : undefined,
  });
  closedPanelIds.current.delete(id);
};
```

Note `executionRail`'s node can be `null` — never call `addDefaultPanel(api, 'executionRail')` when `executionRailNode == null` (Task 1.5/1.6 callers must filter it out, same as the refresh effect already does).

- [ ] **Step 3: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass. `addDefaultPanel` isn't called by any UI yet (Task 1.5 wires it in) — that's expected.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "refactor(gui): extract addDefaultPanel with the real reference-panel fallback chain"
```

### Task 1.5: Add the Panels launcher popover with a reopen action

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

Plain popover, **not** `role="menu"` — that role obligates full keyboard-menu semantics (arrow-key nav, Home/End) that this codebase's one existing instance of the pattern (`ChatSessionRail.tsx`'s session-actions menu) doesn't implement; don't add a second broken instance. Split into two steps (trigger, then contents) since one step covering both is too large per the writing-plans granularity guidance.

- [ ] **Step 1: Write the failing test for the trigger**

```tsx
it('Panels button toggles a popover open and closed, with Escape and focus-return', () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  const trigger = screen.getByRole('button', { name: /panels/i });
  expect(trigger.getAttribute('aria-expanded')).toBe('false');

  fireEvent.click(trigger);
  expect(trigger.getAttribute('aria-expanded')).toBe('true');

  fireEvent.keyDown(document, { key: 'Escape' });
  expect(trigger.getAttribute('aria-expanded')).toBe('false');
  expect(document.activeElement).toBe(trigger);
});
```

- [ ] **Step 2: Run test to verify it fails, then add the trigger button**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx` — expect FAIL (no "Panels" button yet).

```tsx
const [panelsMenuOpen, setPanelsMenuOpen] = useState(false);
const panelsTriggerRef = useRef<HTMLButtonElement | null>(null);

useEffect(() => {
  if (!panelsMenuOpen) return;
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Escape') {
      setPanelsMenuOpen(false);
      panelsTriggerRef.current?.focus();
    }
  };
  document.addEventListener('keydown', onKey);
  return () => document.removeEventListener('keydown', onKey);
}, [panelsMenuOpen]);

useEffect(() => {
  if (!panelsMenuOpen) return;
  const onPointerDown = (e: MouseEvent) => {
    if (panelsTriggerRef.current?.contains(e.target as Node)) return;
    setPanelsMenuOpen(false);
  };
  document.addEventListener('mousedown', onPointerDown);
  return () => document.removeEventListener('mousedown', onPointerDown);
}, [panelsMenuOpen]);
```

Add the trigger in the JSX, near the top of `ChatSurface`'s root, before the `DockWorkspaceShell` wrapper:

```tsx
<div className="relative">
  <button
    ref={panelsTriggerRef}
    type="button"
    aria-label="Panels"
    aria-expanded={panelsMenuOpen}
    onClick={() => setPanelsMenuOpen(o => !o)}
    className="rounded-lg border border-border-subtle bg-overlay-subtle p-2 text-text-muted transition hover:border-brass/40 hover:text-brass"
  >
    <span className="font-mono text-xs">Panels ▾</span>
  </button>
  {panelsMenuOpen ? (
    <div className="absolute left-0 top-full z-50 mt-1 w-48 rounded-lg border border-border-subtle bg-bg-base p-1 shadow-2xl">
      {/* Step 4 fills this in */}
    </div>
  ) : null}
</div>
```

Run the test again — Expected: PASS.

- [ ] **Step 3: Write the failing test for reopen + outside-click**

```tsx
it('Panels popover lists a closed panel and reopens it on click; clicking outside closes it', async () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  await screen.findByTestId('chat-dock-flow');

  const flowTab = screen.getByText('Flow').closest('.dv-default-tab') as HTMLElement;
  fireEvent.click(flowTab.querySelector('.dv-default-tab-action') as HTMLElement);
  await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^flow$/i }));
  await waitFor(() => expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument());

  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.mouseDown(document.body);
  expect(screen.queryByText('All panels open')).toBeNull();
});
```

- [ ] **Step 4: Run test to verify it fails, then fill in the popover contents**

Replace the `{/* Step 4 fills this in */}` placeholder:

```tsx
{DEFAULT_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null)
  .filter(id => closedPanelIds.current.has(id))
  .map(id => (
    <button
      key={id}
      type="button"
      onClick={() => {
        const api = dockApiRef.current;
        if (api) addDefaultPanel(api, id);
        setPanelsMenuOpen(false);
        panelsTriggerRef.current?.focus();
      }}
      className="block w-full rounded px-2 py-1.5 text-left text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
    >
      {panelDefs[id].title}
    </button>
  ))}
{DEFAULT_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).every(
  id => !closedPanelIds.current.has(id),
) ? (
  <div className="px-2 py-1.5 text-xs text-text-muted/60">All panels open</div>
) : null}
```

Run the test again — Expected: PASS.

- [ ] **Step 5: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): add Panels popover to reopen a closed dockview panel (plain buttons, not role=menu)"
```

### Task 1.6: Add "Reset layout" to the Panels popover

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

- [ ] **Step 1: Write the failing tests**

```tsx
it('Reset layout clears the persisted layout, all closedPanelIds, and recreates the default panels', async () => {
  const { layoutStorageKeyFor } = await import('../../dock/DockWorkspaceShell');
  window.localStorage.setItem(layoutStorageKeyFor('gui.chat'), JSON.stringify({ grid: {} }));
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  await screen.findByTestId('chat-dock-flow');

  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /reset layout/i }));

  expect(window.localStorage.getItem(layoutStorageKeyFor('gui.chat'))).toBeNull();
  await waitFor(() => {
    expect(screen.getByTestId('chat-dock-sessions')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-transcript')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-todos')).toBeInTheDocument();
  });
});

it('Reset layout does not throw when no layout was ever persisted', () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  expect(() => fireEvent.click(screen.getByRole('button', { name: /reset layout/i }))).not.toThrow();
});
```

- [ ] **Step 2: Run tests to verify they fail, then add the reset action**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx` — expect FAIL (no "Reset layout" button yet).

Add the import at the top of `ChatSurface.tsx`:

```tsx
import { layoutStorageKeyFor } from '../../dock/DockWorkspaceShell';
```

Add as a sibling to Task 1.5's popover contents, after a thin separator:

```tsx
<div className="my-1 border-t border-border-subtle" />
<button
  type="button"
  onClick={() => {
    window.localStorage.removeItem(layoutStorageKeyFor('gui.chat'));
    const api = dockApiRef.current;
    if (api) {
      api.panels.forEach(p => api.removePanel(p)); // .panels is a fresh snapshot per access — safe to iterate while removePanel mutates the underlying model
      closedPanelIds.current.clear();
      DEFAULT_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).forEach(id =>
        addDefaultPanel(api, id),
      );
    }
    setPanelsMenuOpen(false);
    panelsTriggerRef.current?.focus();
  }}
  className="block w-full rounded px-2 py-1.5 text-left text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
>
  Reset layout
</button>
```

Run the tests again — Expected: PASS.

- [ ] **Step 3: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): add Reset layout to the Panels popover"
```

### Task 1.7: Quiet the dockview tab chrome

**Files:**
- Modify: `crates/vox-gui/ui/src/styles/dockview-vox.css`

- [ ] **Step 1: Investigate the actual blue-chrome source**

Run: `grep -n "activegroup\|inactivegroup\|tabs-and-actions" crates/vox-gui/ui/src/styles/dockview-vox.css` and separately `grep -n "^  --dv-" "node_modules/.pnpm/dockview-core@6.6.1/node_modules/dockview-core/dist/styles/dockview.css"` to see dockview's own un-themed default values for every `--dv-*` custom property. Compare against what `dockview-vox.css`'s `.dockview-theme-vox` block actually overrides (it currently sets 12 of them — check dockview's default CSS for how many total exist and whether any commonly-visible one, e.g. an inactive-group or hover-state color, is left un-overridden and defaulting to a blue-ish library default).

- [ ] **Step 2: Add the missing overrides and shrink the tab strip**

Based on Step 1's findings, extend `.dockview-theme-vox` in `crates/vox-gui/ui/src/styles/dockview-vox.css` with whatever `--dv-*` properties were found un-themed (there is no way to give the exact property names without Step 1's real output — this is why Step 1 is a required investigation, not skippable). At minimum, also add:

```css
.dockview-theme-vox .dv-tabs-and-actions-container {
  height: 22px; /* default is taller; a quieter, VS-Code-rail-style strip */
}

.dockview-theme-vox .dv-tab {
  font-size: 9px;
}
```

- [ ] **Step 3: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass — this task is CSS-only, no test assertions change, but confirm nothing broke.

- [ ] **Step 4: Rebuild and manually confirm the chrome reads as quiet, not blue**

Run `pnpm build` (in `crates/vox-gui/ui`) and `cargo build --release -p vox-gui` (from repo root), relaunch, open Chat, visually confirm no blue tab backgrounds remain and the tab strip is visibly slimmer than before. This is a real visual check, not a test — do not skip it.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/styles/dockview-vox.css
git commit -m "fix(gui): theme every dockview chrome color the library exposes, shrink the tab strip"
```

### Task 1.8: Pin the chat transcript panel — investigate, then implement

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

This is an investigation-first task, like Task B3's AgentFlow data-source research earlier this session — `tabComponents` (Task 1.1's `DockWorkspaceShell` already threads this prop through) is confirmed to exist, but whether an empty tab component also blocks drag-out (not just hides the close button) hasn't been verified against this dockview version's real behavior.

- [ ] **Step 1: Write the failing test for "no visible tab chrome on the transcript panel"**

```tsx
it('the transcript panel has no visible tab strip (pinned, not a normal closable/draggable panel)', async () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  await screen.findByTestId('chat-dock-transcript');
  // The default tab renderer always shows the literal title text ("Chat")
  // inside a .dv-default-tab-content span — an empty custom tab component
  // must not render that at all.
  expect(screen.queryByText('Chat', { selector: '.dv-default-tab-content' })).toBeNull();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — the transcript panel currently uses the default tab renderer, which shows "Chat".

- [ ] **Step 3: Add an empty tab component and wire it in via `tabComponents`**

```tsx
// Near the other panel components in ChatSurface.tsx
function EmptyTab() {
  return null;
}

const CHAT_DOCK_TAB_COMPONENTS = {
  transcript: EmptyTab,
};
```

Pass it to `DockWorkspaceShell`:

```tsx
<DockWorkspaceShell
  storageKeyPrefix="gui.chat"
  components={CHAT_DOCK_COMPONENTS}
  tabComponents={CHAT_DOCK_TAB_COMPONENTS}
  onReady={...}
/>
```

Add `tabComponent: 'transcript'` to the transcript panel's `addPanel` call in `onReady` (find `id: 'transcript'`):

```tsx
event.api.addPanel({
  id: 'transcript',
  component: 'transcript',
  tabComponent: 'transcript',
  title: 'Chat',
  params: { node: centerContent },
  position: { direction: 'right', referencePanel: 'sessions' },
});
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 5: Manually verify drag-out is actually prevented, not just the visible chrome**

jsdom has no real drag-and-drop, so this cannot be a `vitest` assertion — rebuild (`pnpm build` + `cargo build --release -p vox-gui`), relaunch, and manually attempt to drag the area where the Chat panel's (now invisible) tab would be. **If the panel is still draggable** (e.g. dockview attaches drag listeners to the group's title-bar hit area regardless of tab content), that's a real finding — do not silently accept it. In that case, additionally consult dockview's `GroupOptions.locked` (confirmed to exist in `options.d.ts:118`, prevents drag in/out of the group entirely) and apply it to the transcript panel's group via `event.api.addGroup`/the panel's `.group.locked = true` after creation — write this fallback only if Step 5's manual check shows it's actually needed; don't add unverified code preemptively.

- [ ] **Step 6: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): pin the chat transcript panel — empty tab renderer, no close/drag chrome"
```

If Step 5 required the `locked` fallback, include that change in this same commit with a note in the message explaining what Step 5 found.

### Task 1.9: Phase 1 whole-effort verification

**Files:** none (verification only)

- [ ] **Step 1: Run the full backend and frontend suites**

Run `cargo test -p vox-orchestrator --lib` (expect no change — this phase touches no Rust code) and, in `crates/vox-gui/ui`: `npx tsc --noEmit` and `npx vitest run`. Read the actual output; don't summarize from memory of earlier task-level runs.

- [ ] **Step 2: Rebuild, relaunch, manually smoke-test**

Rebuild (`pnpm build` + `cargo build --release -p vox-gui`), kill any running `vox-gui.exe`/`vox-orchestrator-d.exe`, relaunch. On the Chat tab: confirm the transcript has no visible tab and can't be dragged/closed; confirm Sessions/Execution/Flow/To-dos each show as real, closeable, draggable, resizable dockview panels with quiet (not blue) chrome; close one, reopen it via Panels▾; drag one to a new edge, then Reset layout and confirm it returns to default; restart the app and confirm the (non-reset) layout persisted.

- [ ] **Step 3: Report and use `systematic-debugging` for any real bug found**

If smoke-testing surfaces a real bug, root-cause it with `superpowers:systematic-debugging` before writing its regression test and fix — don't same-turn-patch. Once clean, report the commit range for Phase 1.

---

## Phase 2: Wire in the 6 strong dockable candidates

Each of these surfaces was audited (this conversation's transcript) as narrow-tolerant secondary/glanceable content requiring no condensed/full toggle — the surface's own existing component renders fine at dock-panel width as-is.

### Task 2.1 (worked example): Wire `needs-you` in as a dockable panel

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

`NeedsYouSurface` (`crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx`) is the audit's strongest candidate: an approval/notification inbox, already responsively collapsing rows below `sm`, core value is a single "N need you" count. Read its actual props before wiring (it's mounted today via `surfaceComponents.tsx`'s `case 'needs-you':` — check what data it's given there) so the dock-panel wrapper passes the same real props, not invented ones.

- [ ] **Step 1: Read the real prop source**

Run: `grep -n "NeedsYouSurface" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` and read the surrounding props passed at that call site — reuse that exact data source for the new dock-panel wrapper (mirrors how Task B3 reused the top-level Flow tab's `agents` data for the in-chat Flow panel, rather than inventing a second fetch).

- [ ] **Step 2: Write the failing test**

```tsx
it('mounts a Needs You panel dockable from Chat, reusing the same data source as the top-level Needs You tab', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
      needsYouCount={3}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^needs you$/i }));
  expect(screen.getByTestId('chat-dock-needs-you')).toBeInTheDocument();
});
```

(Adjust the prop names/shape to match whatever Step 1's real read reveals — this snippet assumes a `needsYouCount`-style summary prop pattern consistent with how `flowAgents`/`flowSelectedAgentId` were threaded into `ChatSurface` for the Flow panel in Task B3; if the real data source is structured differently, use the real shape, not this guess.)

- [ ] **Step 3: Run test to verify it fails, then wire it in**

Add a `NeedsYouDockPanel` component, a `needs-you` entry in `CHAT_DOCK_COMPONENTS`, thread whatever props Step 1 found through `ChatSurface`'s own props (same pattern as `flowAgents`/`flowSelectedAgentId`/`onFlowSelectAgent` in Task B3), add it to `DEFAULT_PANEL_IDS`/`panelDefs`/the refresh effect's lazy-create branch — **not** to the initial always-created `onReady` set, since (like Flow/To-dos) it should start closed/not-present until a user opts in via the Panels menu, given it's a secondary surface, not core-always-visible like Sessions/Chat.

- [ ] **Step 4: Run test to verify it passes, then the full suite**

Run the target test, then `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, zero regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): wire Needs You as a dockable panel reachable from Chat"
```

### Tasks 2.2 – 2.6: the remaining 5 strong candidates, same pattern as Task 2.1

Each is its own task, its own commit, following Task 2.1's exact steps (read the real data source at its `surfaceComponents.tsx` call site, add a dock-panel wrapper + testid, wire into `CHAT_DOCK_COMPONENTS`/`DEFAULT_PANEL_IDS`/`panelDefs`/the lazy-create refresh branch, write a real test asserting the Panels-menu-launch → testid-appears flow, run full suite, commit). Do not batch multiple surfaces into one task/commit — each gets independent review.

| Task | Surface (`surfaceComponents.tsx` case) | Component | Panel id / testid | Condensed view needed? |
|---|---|---|---|---|
| 2.2 | `vox-search` / `graphify` | `VoxGraphStatusPanel.tsx` | `voxgraph` / `chat-dock-voxgraph` | No — already responsive to 1-column below `md`, cards work as-is |
| 2.3 | `activity` | `DiscoverySurface.tsx` | `activity` / `chat-dock-activity` | No — all 4 sub-views are single-column lists; the Review sub-view's fixed 460px inspector popover should be spot-checked at dock-panel width during this task's manual smoke step, but no code change is assumed necessary unless that check finds a real problem |
| 2.4 | `repository` | `RepositoryView.tsx` | `repository` / `chat-dock-repository` | No — button grid + scrollable `<pre>` output + small isolation panel, narrow-safe as-is |
| 2.5 | `mercatus` | `Mercatus.tsx` | `mercatus` / `chat-dock-mercatus` | No — already wrapped in `overflow-x-auto`, degrades gracefully |
| 2.6 | `harness` | `HarnessRedirect.tsx` | `harness` / `chat-dock-harness` | No — trivial empty-state stub, works at any size |

### Task 2.7: Phase 2 whole-effort verification

**Files:** none (verification only)

Same shape as Task 1.9: run backend+frontend suites with real output, rebuild+relaunch+manually smoke-test each of the 6 new panels (open via Panels▾, confirm content renders, confirm close/reopen/drag/resize all work), root-cause any real bug via `systematic-debugging` before fixing, report the commit range.

---

## Phase 3: Wire in the 8 condensed-capable candidates

Each of these needs a bespoke **condensed vs. full** toggle — the full view wants real width, but a narrow docked state should show a specific, evidence-based summary instead of squeezing the full UI. Task 3.1 is the worked example establishing the toggle pattern; the table gives every other surface's specific condensed content (sourced directly from this session's audit — not invented).

### Task 3.1 (worked example): Wire `approvals` in with a condensed/full toggle

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

The audit found: Approvals' working table needs ~850-1000px (4 fixed-width columns sum to 710px before the flexible description column gets any room); a docked panel will essentially never be that wide. Condensed content: **pending-approval count + current permission mode** — both already exist as real state (`pendingApprovals` prop, already threaded through `App.tsx`/`AppShell.tsx` per the codebase's existing "Task tools" plumbing — verify the exact prop name at its current call site before using it, the same discipline as every prior data-source task this session).

- [ ] **Step 1: Read the real prop source and the panel width dockview reports at runtime**

Run: `grep -n "pendingApprovals\|permission.*mode\|ApprovalsView" crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — confirm the exact prop/state names for pending-approval count and the current permission mode (Ask/Accept Edits/Accept All/Plan) before writing the condensed component. Also confirm dockview panel components can read their own rendered width — check `IDockviewPanelProps` for a `width`/`api.width` field (`node_modules/.pnpm/dockview-core@6.6.1/node_modules/dockview-core/dist/esm/panel/types.d.ts` or similar) so the toggle can be driven by real measured panel width, not a guess.

- [ ] **Step 2: Write the failing test**

```tsx
it('Approvals panel shows a condensed pending-count badge, not the full table, when docked narrow', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
      pendingApprovals={3}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^approvals$/i }));
  const panel = screen.getByTestId('chat-dock-approvals');
  expect(panel).toHaveTextContent('3 pending');
  // The full 4-column DataTable must not render in the condensed state —
  // assert on a testid/role specific to ApprovalsView's real table, found
  // during Step 1's read, not guessed here.
});
```

- [ ] **Step 3: Run test to verify it fails, then implement the condensed wrapper**

Build an `ApprovalsDockPanel` that renders a condensed summary (pending count + permission mode pill) by default, with a small "Open full view" affordance that either expands the panel's rendered content in place (if Step 1's width-detection API exists and is reliable) or — the safer, always-correct fallback if width detection proves unreliable in practice — a button that calls `onNavigate('approvals')` to jump to the real top-level Approvals tab for the full working table. **Prefer the navigate-to-full-tab fallback unless Step 1 finds a genuinely reliable in-panel width signal** — a docked panel silently trying and failing to render a 1000px-wide table in 300px of space is worse than a clear "open full view" link.

- [ ] **Step 4: Run test to verify it passes, then the full suite**

Run the target test, then `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, zero regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): wire Approvals as a dockable panel with a condensed pending-count view"
```

### Tasks 3.2 – 3.8: the remaining 7 condensed-capable surfaces, same pattern as Task 3.1

Each is its own task/commit. The condensed content specified below is the audit's actual finding for that surface — use it verbatim, don't invent a different summary.

| Task | Surface | Component | Panel id / testid | Condensed content (from the audit) | Special note |
|---|---|---|---|---|---|
| 3.2 | `mesh` | `MeshView.tsx` | `mesh` / `chat-dock-mesh` | Node count + source + pending queue count (already exist as header summary chips — reuse them, don't re-derive) | Full 7-column node table + dispatch-form textarea only render above ~400px |
| 3.3 | `tasks` (global task queue) | `TasksView.tsx` | `tasks` / `chat-dock-tasks` | Per-lifecycle counts: "N blocked · M queued · K in progress" (from the existing `groupBy` categories) | **Naming collision guard**: this is the global task queue, unrelated to the chat To-dos panel from Task 1.3 — do not reuse the `todos` id/title anywhere here |
| 3.4 | `coderabbit` | `CodeRabbitView.tsx` | `coderabbit` / `chat-dock-coderabbit` | Token status (present/absent) + "Planned N PRs · M files" (existing summary line) | The 5-field control row should stack/collapse, not truncate, when condensed |
| 3.5 | `skills` | `SkillsPluginsView.tsx` | `skills` / `chat-dock-skills` | "Skills: N · Plugins: M" install counts (the `Section` component's existing count badge) | Full 8/4 grid + marketplace search only render above ~700-750px |
| 3.6 | `gamify` | `GamifyView.tsx` | `gamify` / `chat-dock-gamify` | HP/level/leaderboard rank from the profile HUD | **The `h-[560px]` `LudusSandbox` mini-map must never render in the condensed state** — hard-fixed dimension, structurally incompatible with a narrow dock |
| 3.7 | `models` | `ModelsView.tsx` | `models` / `chat-dock-models` | Active model name + total model count (from the existing header stat bar) | Full 1/2/3-column card grid only useful above ~700-1000px |
| 3.8 | `memory` | `MemoryView.tsx` | `memory` / `chat-dock-memory` | "{N} corpora active · {M} indexed entries" (existing header line) | Note the pre-existing `SHARD_COLS = 6` virtualizer bug found during the audit (row-height math assumes 6 columns regardless of actual responsive column count) is out of scope for this task — flag it as a follow-up, don't fix it here |

### Task 3.9: Phase 3 whole-effort verification

**Files:** none (verification only)

Same shape as Tasks 1.9/2.7: run backend+frontend suites with real output, rebuild+relaunch+manually smoke-test all 8 new panels at both a wide and a narrow docked width (confirming the condensed/full toggle actually engages), root-cause any real bug via `systematic-debugging`, report the commit range.

- [ ] **Step (final): Use `superpowers:finishing-a-development-branch`**

Once Phase 3 is clean, invoke `superpowers:finishing-a-development-branch` to decide how this whole effort (Phases 1-3) gets integrated — do not assume "commit and stop."

---

## Explicitly not in this plan

Per the design spec: Dashboard/`DashboardGrid` unification, true external drag-from-sidebar-to-dock, double-click-splitter-to-reset, a native OS menu bar, and wiring Settings/Flow/Catalog/Browser/Console/Policies/Runs into any dock workspace. None of these get a task here — they're deferred, not forgotten; raise them as their own future spec if wanted.
