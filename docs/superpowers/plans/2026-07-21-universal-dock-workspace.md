# Universal Dock Workspace Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. `subagent-driven-development` itself requires four companion skills, all apply here: `superpowers:using-git-worktrees` (this plan is **already** running inside worktree `C:\Users\Owner\vox\.worktrees\axis-frontend-remediation`, branch `axis-frontend-remediation` — do NOT invoke `using-git-worktrees` to create a nested worktree, that's the failure mode it exists to prevent, not a step to repeat), `superpowers:writing-plans` (this document), two-stage spec-then-quality review per `subagent-driven-development`'s own process (not `requesting-code-review`, which is a different, single-pass flow), and `superpowers:finishing-a-development-branch` (after Phase 3). Also cite `superpowers:verification-before-completion` at every whole-effort verification task (1.9/2.7/3.9) and `superpowers:systematic-debugging` for any bug found during them.

> **Revision note (post-adversarial-audit, 3 parallel reviewers vs. the live codebase + skill files):** this revision fixes one plan-breaking defect (a single `DEFAULT_PANEL_IDS` array was being used for two incompatible jobs — "core panels the refresh effect auto-creates" and "Reset layout's target set" — meaning once Phase 2/3's 14 new panels were added to it, Reset would force-open all 19 panels every time, and the new panels would auto-resurrect on any re-render exactly like the original bug this whole effort started by fixing). Also fixes: a wrong file/line citation for `GroupOptions.locked` plus an overstated claim about what it does; Task 3.1's condensed/full toggle had no real implementation and a placeholder test — now built on a verified real API (`props.api.width`/`.height`, `onDidDimensionsChange`); a missing regression-test requirement; a documented but unaddressed duplicate-`<h1>` collision risk (`TasksView.tsx`); a double-polling risk when a surface's dock panel and its top-level tab are both mounted; a missing parallel-research recommendation for the 12 independent per-surface investigation steps; and several skill-citation corrections. See inline `**FIXED:**` markers below for exactly what changed and why.

> **Supersedes an in-flight plan.** `docs/superpowers/plans/2026-07-20-dockview-panel-ux.md` has only its Task 1 landed (commit `1e689ed551` — the closed-panel-tracking fix). Its Tasks 2-6 are **absorbed into this plan's Phase 1** at the generalized-shell level — do not run them separately, they'd duplicate Phase 1 and conflict on the same files. `docs/superpowers/plans/2026-07-20-chat-flow-docking-redesign.md`'s Phase B (B1-B5) is fully landed; its B6 (visual pass) is independent, do it whenever convenient.

**Goal:** Generalize the chat-only dockview shell into a reusable `DockWorkspaceShell`, pin the chat transcript as a fixed non-closable center, rename the Plan panel to "To-dos," quiet the dockview chrome, add a Panels launcher (reopen/reset/add), and wire in the 14 app surfaces confirmed dockable by this session's full-codebase audit as **opt-in, never-auto-opened** panels — distinct from the 5 core panels Reset restores — while leaving Dashboard, Settings, Flow, Catalog, Browser, Console, Policies, and Runs untouched.

**Architecture:** Phase 1 builds and hardens the generalized primitive using Chat as the only consumer. Phase 2 wires in 6 strong dockable candidates. Phase 3 wires in 8 condensed-capable candidates, each with a real width/height-driven condensed/full toggle. **Core vs. opt-in split (new in this revision):** `CORE_PANEL_IDS` (sessions/transcript/executionRail/flow/todos) are auto-created on first mount and are Reset's target set, exactly as today. Every Phase 2/3 panel is `OPT_IN` — created *only* by an explicit Panels-menu click, never auto-created, never touched by Reset, and structurally cannot reproduce the original "refresh effect resurrects a closed panel" bug because it has no auto-create branch to guard in the first place.

**Tech Stack:** React 19, TypeScript, dockview 6.6.1. `DockviewReact`'s `tabComponents` prop confirmed via `IDockviewReactProps.tabComponents?: Record<string, FC<IDockviewPanelHeaderProps>>` (`dockview.d.ts:9`). **FIXED:** a dock panel's own live rendered dimensions are available via `props.api.width`/`props.api.height` (both `readonly number`, `PanelApi` — `dockview-core/dist/esm/api/panelApi.d.ts:84-90`) and `props.api.onDidDimensionsChange` (`Event<PanelDimensionChangeEvent>`, same file line 17) — this is a real, always-on, reactive API, not something requiring investigation to discover; Task 3.1 below uses it directly. Vitest + Testing Library.

**Ground truth confirmed for this plan (re-verify per the note above if time has passed and other work landed on these files):**
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` (85 lines), `LAYOUT_STORAGE_KEY = 'gui.chat.dockview_layout.v3'`, `ChatSurface.tsx` (517 lines, `CHAT_DOCK_COMPONENTS` at lines 55-60 with `sessions`/`transcript`/`executionRail`/`flow`, no `plan`/`todos` entry yet), `closedPanelIds`/`onDidRemovePanel` already landed (lines 143, 409) — all independently re-verified byte-for-byte against the real files, unchanged from the prior draft.
- `PlanPanel.tsx` (139 lines): props exactly `planSessionId`/`planVersion`/`nodes`; the three copy strings to change (`"No active plan"`, `"Start a task to see its plan here."`, `"No plan steps yet."`) all verified verbatim at lines 89/90/109. **FIXED:** `PlanPanel.test.tsx` (read in full) already contains an existing test at lines 65-68 asserting the *old* `"no active plan"` copy — Task 1.3 Step 1 below now explicitly updates it, not left to the final grep as a safety net.
- `IDockviewReactProps.tabComponents` and `AddPanelOptions.tabComponent?: string` (`options.d.ts:288`) both confirmed. No `alwaysShowTabs`/`hideTabsAndAction` option exists anywhere in dockview-core (only `singleTabMode?: 'fullwidth' | 'default'`, styling not visibility) — quiet chrome (Task 1.7) means visually smaller/lower-contrast, always present, not conditionally hidden.
- **FIXED — `GroupOptions.locked`, wrong citation corrected:** the prior draft cited `options.d.ts:118` — that's actually `DockviewOptions.locked?: boolean`, a whole-component option unrelated to any single group. The real group-level property is `CoreGroupOptions.locked?: DockviewGroupPanelLocked` in `dockview-core/dist/esm/dockview/dockviewGroupPanelModel.d.ts:28`, typed `DockviewGroupPanelLocked = boolean | 'no-drop-target'` (same file, line 91) — not a plain boolean. Its real doc comment (`dockviewGroupPanelApi.d.ts:32-37`): *"`true`: panels cannot be dropped into the group (center/tabs), but the group can still be split from its edges. `'no-drop-target'`: all drop zones are disabled."* **This describes preventing drop-INTO, not preventing drag-OUT** — Task 1.8 exists specifically to prevent the pinned transcript from being dragged *out*, which `locked` may not address at all. Access path is real (`panel.group.locked = true` via `IDockviewPanel.group: DockviewGroupPanel`, `dockviewPanel.d.ts:47`), but treat it as an unverified, possibly-inapplicable fallback, not a known-working one — Task 1.8 Step 5 below reflects this honestly.
- The 22-surface audit lives in this conversation's transcript, summarized in `docs/superpowers/specs/2026-07-21-universal-dock-workspace-design.md`.

---

## Phase 1: Generalize the shell, pin chat, rename to To-dos, quiet chrome, Panels menu

### Task 1.1: Extract `DockWorkspaceShell` from `ChatDockShell`

**Files:**
- Create: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx`
- Create: `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx`
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` / `.test.tsx` (Task 1.2 repoints the consumer first)

- [ ] **Step 1: Write the failing test**

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
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/dock/DockWorkspaceShell.test.tsx`
Expected: FAIL with "Cannot find module './DockWorkspaceShell'" (or equivalent) for all 3 tests — confirm it's this import error, not a typo elsewhere in the test file, before proceeding.

- [ ] **Step 3: Create `DockWorkspaceShell`'s mount/restore/key-isolation behavior**

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
 * `storageKeyPrefix` scopes it.
 *
 * IMPORTANT for any future consumer of DockWorkspaceShell: this shell does
 * NOT itself track which panels a user has explicitly closed (see
 * ChatSurface.tsx's `closedPanelIds` ref + `onDidRemovePanel` listener — that
 * mechanism stays host-local, it was NOT folded into this shell). If your
 * host view has any auto-recreate-if-missing logic for its own panels (the
 * way ChatSurface's refresh effect does for its 5 core panels), you MUST
 * build the same closedPanelIds-style guard yourself, or you will
 * reintroduce the exact bug this whole effort started by fixing: a refresh
 * effect silently re-adding a panel the user just closed. See
 * ChatSurface.test.tsx's "does not resurrect the Flow panel on the next
 * render after the user closes it" test as the required template.
 */
export function layoutStorageKeyFor(storageKeyPrefix: string): string {
  return `${storageKeyPrefix}.dockview_layout.v3`;
}

interface DockWorkspaceShellProps {
  storageKeyPrefix: string;
  components: Record<string, React.FunctionComponent<IDockviewPanelProps>>;
  tabComponents?: Record<string, React.FunctionComponent<IDockviewPanelHeaderProps>>;
  onReady: (event: DockviewReadyEvent) => void;
}

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
Expected: PASS, all 3 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx
git commit -m "feat(gui): extract DockWorkspaceShell (mount/restore/key-isolation) — reusable dockview shell generalized from ChatDockShell"
```

- [ ] **Step 6: Write the failing test for params-stripped persistence**

This is a separate behavior (debounced write + params-stripping) from Step 1-5's mount/restore — a distinct increment, not folded into the same red/green cycle:

```tsx
// Add to DockWorkspaceShell.test.tsx
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
```

- [ ] **Step 7: Run test to verify it passes (implementation already exists from Step 3)**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/dock/DockWorkspaceShell.test.tsx`
Expected: PASS, all 4 tests — Step 3's implementation already includes the params-stripping replacer, so this test should pass immediately; if it doesn't, that's a real gap in Step 3 to fix before committing.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.test.tsx
git commit -m "test(gui): lock in DockWorkspaceShell's params-stripping persistence behavior"
```

Do not delete `ChatDockShell.tsx` yet — Task 1.2 repoints its consumer first, then deletes it in the same commit as that repoint.

### Task 1.2: Repoint `ChatSurface` onto `DockWorkspaceShell`, delete `ChatDockShell`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Delete: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` / `.test.tsx`

- [ ] **Step 1: Confirm nothing else imports `ChatDockShell`**

Run: `grep -rln "ChatDockShell" crates/vox-gui/ui/src --include=*.tsx --include=*.ts`
Expected: only `ChatSurface.tsx` and the two files being deleted.

- [ ] **Step 2: Update the import and call site**

```tsx
// Replace
import { ChatDockShell } from './ChatDockShell';
// With
import { DockWorkspaceShell } from '../../dock/DockWorkspaceShell';
```

```tsx
// Replace <ChatDockShell components={CHAT_DOCK_COMPONENTS} onReady={...} /> with:
<DockWorkspaceShell storageKeyPrefix="gui.chat" components={CHAT_DOCK_COMPONENTS} onReady={...} />
```

- [ ] **Step 3: Delete the old shell files**

```bash
rm crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
```

- [ ] **Step 4: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.test.tsx
git commit -m "refactor(gui): ChatSurface uses the generalized DockWorkspaceShell, delete the chat-only predecessor"
```

Run `git status` first — confirm only `ChatSurface.tsx`'s modification plus the two deletions are staged.

### Task 1.3: Migrate the Plan panel into the dock as "To-dos," delete the old collapse toggle

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`, `PlanPanel.tsx` (copy only)
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx`, `PlanPanel.test.tsx` (both extend)

- [ ] **Step 1: Update the existing stale test and write the new failing tests**

**FIXED:** `PlanPanel.test.tsx` already has a test at lines 65-68 asserting the *old* copy — update it now, don't leave it to a final grep:

```tsx
// PlanPanel.test.tsx — UPDATE this existing test (was asserting /no active plan/i)
it('renders an honest empty state when there is no active plan', () => {
  render(<PlanPanel planSessionId={null} planVersion={null} nodes={[]} />);
  expect(screen.getByText(/no to-dos yet/i)).toBeInTheDocument();
});
```

```tsx
// Add to ChatSurface.test.tsx
it('mounts the To-dos panel as a dockview panel, not a hand-rolled collapsible aside', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>}
      planSessionId="sess-1" planVersion={1}
    />,
  );
  expect(screen.getByTestId('chat-dock-todos')).toBeInTheDocument();
  expect(screen.queryByLabelText('Collapse plan panel')).toBeNull();
  expect(screen.queryByLabelText('Expand plan panel')).toBeNull();
});
```

```tsx
// Add to PlanPanel.test.tsx
it('shows to-do-list-labeled zero-step copy, not "plan" language', () => {
  render(<PlanPanel planSessionId="s1" planVersion={1} nodes={[]} />);
  expect(screen.getByText('Nothing to do yet.')).toBeInTheDocument();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx src/components/surfaces/Chat/PlanPanel.test.tsx`
Expected: FAIL — the updated existing test now looks for "no to-dos yet" (not yet in the code), the new `chat-dock-todos` testid doesn't exist, "Nothing to do yet." doesn't exist. Confirm these are the failure reasons, not a syntax error in the test edits.

- [ ] **Step 3: Update `PlanPanel.tsx`'s user-visible copy**

```tsx
// Line ~89 area
<EmptyState title="No to-dos yet" description="Start a task to see its to-do list here." />
```
```tsx
// Line ~109 area
<p className="text-[11px] text-text-muted">Nothing to do yet.</p>
```

- [ ] **Step 4: Add the `TodosDockPanel` component and register it**

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

```tsx
const todosNode = <PlanPanel planSessionId={planSessionId} planVersion={planVersion} nodes={planNodes} />;
```

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

Remove the `planPanelCollapsed`/`useLocalStorage` declaration and the entire old `<aside>` JSX block. Remove the `Glass` import only if unused elsewhere (`grep -n "Glass" ChatSurface.tsx` first).

- [ ] **Step 7: Run tests to verify they pass, then the full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass. Grep for any other reference to `plan_panel_collapsed`/`"No active plan"`/`"No plan steps yet."` before committing.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.test.tsx
git commit -m "feat(gui): migrate Plan panel into the dock as To-dos (relabeled, not renamed internally — avoids colliding with the existing global Tasks surface)"
```

### Task 1.4: Extract `CORE_PANEL_IDS` / `addDefaultPanel` — refactor, not new behavior

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

**FIXED — this task is now framed honestly as a pure extraction, not padded with a fake test.** The prior draft's Task 1.4 had a "failing test" that never called the function it was extracting, skipped the RED-verification step entirely, and left real coverage to Task 1.5 anyway (its own comment admitted this: *"Task 1.5's Panels-menu test supersedes this"*). Per the TDD skill, a pure refactor that changes no observable behavior, done under the safety net of the full existing passing suite, doesn't need an artificial new failing test invented to satisfy the letter of red-green — Task 1.5's real, click-driven reopen test is what actually exercises `addDefaultPanel` for the first time, and is where its correctness is genuinely proven.

**FIXED — `CORE_PANEL_IDS`, not `DEFAULT_PANEL_IDS`.** The name change reflects a real behavior split, not cosmetics: this array is the set the refresh effect auto-creates AND the set Task 1.6's "Reset layout" restores. Phase 2/3's 14 new panels are never added to it (see Task 2.1/3.1).

- [ ] **Step 1: Extract `CORE_PANEL_IDS`, `panelDefs`, `addDefaultPanel`**

```tsx
const CORE_PANEL_IDS = ['sessions', 'transcript', 'executionRail', 'flow', 'todos'] as const;
type CorePanelId = (typeof CORE_PANEL_IDS)[number];
```

Inside `ChatSurface`, after `todosNode` is built:

```tsx
const panelDefs: Record<ChatDockPanelId, { title: string; node: React.ReactNode; referenceChain: ChatDockPanelId[] }> = {
  sessions: { title: 'Sessions', node: sessionRailNode, referenceChain: [] },
  transcript: { title: 'Chat', node: centerContent, referenceChain: ['sessions'] },
  executionRail: { title: 'Execution', node: executionRailNode, referenceChain: ['transcript'] },
  flow: { title: 'Flow', node: flowNode, referenceChain: ['executionRail', 'transcript'] },
  todos: { title: 'To-dos', node: todosNode, referenceChain: ['flow', 'executionRail', 'transcript'] },
};

// Plain function, not useCallback: panelDefs is a fresh object every render.
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

Note: `panelDefs`'s type is `Record<ChatDockPanelId, ...>` where `ChatDockPanelId` is defined as `CorePanelId` for now — Task 2.1 widens it to `CorePanelId | OptInPanelId` once opt-in panels exist:

```tsx
type ChatDockPanelId = CorePanelId;
```

`executionRail`'s node can be `null` — never call `addDefaultPanel(api, 'executionRail')` when `executionRailNode == null`.

- [ ] **Step 2: Run the full frontend suite (regression safety net, not a new-behavior check)**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, identical count to before this task — this extraction changes no observable behavior yet (`addDefaultPanel` isn't called by any UI until Task 1.5).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "refactor(gui): extract CORE_PANEL_IDS + addDefaultPanel with the real reference-panel fallback chain"
```

### Task 1.5: Add the Panels launcher popover with a reopen action

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

Plain popover, **not** `role="menu"` (this codebase's one existing instance of that pattern, `ChatSessionRail.tsx`'s session-actions menu, doesn't implement full keyboard semantics — don't add a second broken instance).

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

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — `screen.getByRole('button', { name: /panels/i })` throws (no such element).

- [ ] **Step 3: Add the trigger button**

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
    <div className="absolute left-0 top-full z-50 mt-1 w-56 rounded-lg border border-border-subtle bg-bg-base p-1 shadow-2xl">
      {/* Step 5 fills this in */}
    </div>
  ) : null}
</div>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 5: Write the failing test for reopening a core panel + outside-click**

```tsx
it('Panels popover lists a closed core panel and reopens it on click; clicking outside closes it', async () => {
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

- [ ] **Step 6: Run test to verify it fails, then fill in the popover contents**

```tsx
{CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null)
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
{CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).every(
  id => !closedPanelIds.current.has(id),
) ? (
  <div className="px-2 py-1.5 text-xs text-text-muted/60">All panels open</div>
) : null}
```

(Task 2.1 extends this same block with an "Add panel" section listing opt-in surfaces — this task only handles the 5 core panels.)

- [ ] **Step 7: Run test to verify it passes, then the full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, zero regressions.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): add Panels popover to reopen a closed core dockview panel (plain buttons, not role=menu)"
```

### Task 1.6: Add "Reset layout" to the Panels popover — resets only `CORE_PANEL_IDS`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

**FIXED:** the prior draft's Reset handler iterated the same array Phase 2/3 kept adding opt-in surfaces to, so after Phase 3 it would have force-opened all 19 panels on every reset. Reset now explicitly only targets `CORE_PANEL_IDS` — opt-in panels a user had open get removed (clean slate) but are never recreated by Reset, matching the design spec's "returns to the curated 5-panel default."

- [ ] **Step 1: Write the failing tests**

```tsx
it('Reset layout clears the persisted layout and closedPanelIds, and recreates only the 5 core panels', async () => {
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

```tsx
import { layoutStorageKeyFor } from '../../dock/DockWorkspaceShell';
```

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
      // Only CORE_PANEL_IDS — opt-in panels (Phase 2/3) are intentionally
      // NOT recreated by Reset, even if they were open before the reset.
      CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null).forEach(id =>
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

- [ ] **Step 3: Run tests to verify they pass, then the full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): add Reset layout to the Panels popover, scoped to the 5 core panels only"
```

### Task 1.7: Quiet the dockview tab chrome

**Files:** Modify: `crates/vox-gui/ui/src/styles/dockview-vox.css`

- [ ] **Step 1: Investigate the actual blue-chrome source**

Run: `grep -n "activegroup\|inactivegroup\|tabs-and-actions" crates/vox-gui/ui/src/styles/dockview-vox.css` and `grep -n "^  --dv-" "node_modules/.pnpm/dockview-core@6.6.1/node_modules/dockview-core/dist/styles/dockview.css"` — compare the full set of `--dv-*` custom properties dockview defines against the 12 `.dockview-theme-vox` currently overrides; find which ones are left at the library's un-themed default.

- [ ] **Step 2: Add the missing overrides and shrink the tab strip**

Extend `.dockview-theme-vox` with whatever Step 1 found un-themed. At minimum also add:

```css
.dockview-theme-vox .dv-tabs-and-actions-container {
  height: 22px;
}
.dockview-theme-vox .dv-tab {
  font-size: 9px;
}
```

- [ ] **Step 3: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass (CSS-only, confirms nothing broke).

- [ ] **Step 4: Rebuild and manually confirm**

`pnpm build` + `cargo build --release -p vox-gui`, relaunch, open Chat, visually confirm no blue tab backgrounds remain and the strip is visibly slimmer.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/styles/dockview-vox.css
git commit -m "fix(gui): theme every dockview chrome color the library exposes, shrink the tab strip"
```

### Task 1.8: Pin the chat transcript panel — investigate, then implement

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

**FIXED:** Step 5's fallback now correctly cites `panel.group.locked` (not the wrong `DockviewOptions.locked` file/line from the prior draft) and is honest that `locked` prevents drop-*into* the group, with drag-*out* prevention unconfirmed — treat it as a real experiment, not a known-working fix.

- [ ] **Step 1: Write the failing test for "no visible tab chrome on the transcript panel"**

```tsx
it('the transcript panel has no visible tab strip (pinned, not a normal closable/draggable panel)', async () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  await screen.findByTestId('chat-dock-transcript');
  expect(screen.queryByText('Chat', { selector: '.dv-default-tab-content' })).toBeNull();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — the transcript panel currently uses the default tab renderer, which shows "Chat".

- [ ] **Step 3: Add an empty tab component and wire it in**

```tsx
function EmptyTab() {
  return null;
}
const CHAT_DOCK_TAB_COMPONENTS = { transcript: EmptyTab };
```

```tsx
<DockWorkspaceShell
  storageKeyPrefix="gui.chat"
  components={CHAT_DOCK_COMPONENTS}
  tabComponents={CHAT_DOCK_TAB_COMPONENTS}
  onReady={...}
/>
```

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

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx` — Expected: PASS.

- [ ] **Step 5: Manually verify drag-out is prevented, AND that other panels can still dock next to it**

jsdom has no real drag-and-drop — rebuild, relaunch, and manually try to drag the (now invisible) transcript tab area. **If it's still draggable**, that's a real finding, not silently acceptable. Per the corrected ground-truth note above, `panel.group.locked = true` (real property: `IDockviewPanel.group: DockviewGroupPanel`, `.locked` setter in `dockviewGroupPanel.d.ts:27`) only *documented* effect is blocking drop-into; whether it also blocks drag-out is unverified — try it and observe. **Critically, also verify the reverse**: with `locked` applied (if you applied it), drag a *different* open panel (e.g. Execution) to a new position adjacent to the transcript group and confirm the drop still succeeds — `locked`'s doc comment says drops *into* the group are blocked, but the group "can still be split from its edges," so this should work; confirm it actually does, since this is the exact interaction the whole "pin chat, dock everything else around it" premise depends on. If locking breaks that too, `locked` is the wrong tool and needs a different approach (report it, don't force a broken fix through).

- [ ] **Step 6: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): pin the chat transcript panel — empty tab renderer, no close/drag chrome"
```

Include Step 5's findings (locked applied or not, and why) in the commit message.

### Task 1.9: Phase 1 whole-effort verification

**Files:** none (verification only)

Per `superpowers:verification-before-completion` — no completion claim without fresh evidence.

- [ ] **Step 1: Run the full backend and frontend suites, read the real output**

`cargo test -p vox-orchestrator --lib` (expect no change), `npx tsc --noEmit`, `npx vitest run` in `crates/vox-gui/ui`. Read the actual output; don't summarize from memory of earlier task-level runs.

- [ ] **Step 2: Rebuild, relaunch, manually smoke-test**

Confirm: transcript has no visible tab and can't be dragged/closed (and per Task 1.8 Step 5, other panels CAN still dock next to it); Sessions/Execution/Flow/To-dos are real closeable/draggable/resizable panels with quiet chrome; close one, reopen via Panels▾; drag one to a new edge, Reset layout, confirm it returns to the 5-panel default; restart the app, confirm the (non-reset) layout persisted.

- [ ] **Step 3: Report; use `systematic-debugging` for any real bug found**

Root-cause any real bug via `superpowers:systematic-debugging` before writing its regression test and fix — no same-turn patches. Report the commit range for Phase 1.

---

## Phase 2: Wire in the 6 strong dockable candidates as opt-in panels

**FIXED — recommended parallel-research pass before the sequential implementation tasks below.** Tasks 2.2-2.6 (and Phase 3's 3.2-3.8) each open with an independent, read-only "find the real data source" step against a different target file — these have zero shared state and are the textbook case `superpowers:dispatching-parallel-agents` describes ("each problem can be understood without context from others"). Before starting Tasks 2.2-2.6's sequential implement/test/commit work, dispatch one research-only agent per remaining surface (5 agents for `voxgraph`/`activity`/`repository`/`mercatus`/`harness`) in parallel, each told: read `surfaceComponents.tsx`'s call site for that surface plus the target component file, and return (not write to any file) the real prop names/data source and a drafted panel-wrapper code sketch. Then run Tasks 2.2-2.6 sequentially as written, using each memo as the implementer's starting context instead of re-deriving it live — this does not conflict with `subagent-driven-development`'s "no parallel implementers" rule, since research/drafting agents make zero repo writes.

Each of these surfaces was audited as narrow-tolerant secondary/glanceable content requiring no condensed/full toggle.

### Task 2.1 (worked example): Wire `needs-you` in as an opt-in dockable panel

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

**FIXED — three additions vs. the prior draft**: (1) `needs-you` is added to a new `OPT_IN_PANEL_IDS` array, never `CORE_PANEL_IDS` — it gets **no auto-create branch** in the refresh effect at all, only a guarded `.update()` if already present, which by construction cannot reproduce the original "resurrects after close" bug (there is no code path that recreates it once removed, except the explicit Panels-menu click). (2) A grep check for a duplicate `<h1>` in the target component before wiring it in (`NeedsYouSurface.tsx` doesn't have one, but the check itself must become the standard pattern every table-driven task inherits, since `TasksView.tsx` in Phase 3 does). (3) A real "stays closed across an unrelated re-render" test, proving point (1) rather than asserting it in prose.

- [ ] **Step 1: Read the real prop source and check for a duplicate `<h1>`**

Run: `grep -n "NeedsYouSurface" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` and read the surrounding props at that call site. Also run: `grep -n "<h1" crates/vox-gui/ui/src/components/surfaces/NeedsYou/NeedsYouSurface.tsx` — if it renders its own unconditional `<h1>`, the dock-panel wrapper must not mount that part of it (ChatSurface.tsx already has its own `<h1 className="sr-only">Chat</h1>`; two `<h1>`s on one page is an axe `page-has-heading-one` violation this codebase has explicitly guarded against elsewhere).

- [ ] **Step 2: Write the failing tests**

```tsx
it('mounts a Needs You panel dockable from Chat via the Panels menu Add section', () => {
  render(
    <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} needsYouCount={3} />,
  );
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^needs you$/i }));
  expect(screen.getByTestId('chat-dock-needs-you')).toBeInTheDocument();
});

it('the Needs You panel does not resurrect on the next render after being closed', async () => {
  render(
    <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} needsYouCount={3} />,
  );
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^needs you$/i }));
  await screen.findByTestId('chat-dock-needs-you');

  const tab = screen.getByText('Needs You').closest('.dv-default-tab') as HTMLElement;
  fireEvent.click(tab.querySelector('.dv-default-tab-action') as HTMLElement);
  await waitFor(() => expect(screen.queryByTestId('chat-dock-needs-you')).toBeNull());

  // Force an unrelated re-render — opt-in panels have NO auto-create
  // branch, so this must not bring it back (by construction, not by a
  // closedPanelIds guard, since opt-in panels don't use one).
  const { rerender } = render(
    <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[{ id: 'm1', role: 'user', text: 'hi', status: 'done' } as any]} composer={<div>composer</div>} needsYouCount={3} />,
  );
  expect(screen.queryByTestId('chat-dock-needs-you')).toBeNull();
});
```

(Adjust prop names to Step 1's real findings — `needsYouCount` here mirrors the `flowAgents`-style threading pattern from Task B3; use the real shape if different.)

- [ ] **Step 3: Run tests to verify they fail, then wire it in**

Define the opt-in id list and widen `ChatDockPanelId` (both near `CORE_PANEL_IDS`):

```tsx
const OPT_IN_PANEL_IDS = ['needs-you'] as const; // Tasks 2.2-2.6/3.1-3.8 append one id each
type OptInPanelId = (typeof OPT_IN_PANEL_IDS)[number];
type ChatDockPanelId = CorePanelId | OptInPanelId; // supersedes Task 1.4's CorePanelId-only alias
```

Add a `NeedsYouDockPanel` component + `'needs-you': NeedsYouDockPanel` entry in `CHAT_DOCK_COMPONENTS` (quote the key — it contains a hyphen). Thread whatever props Step 1 found through `ChatSurface`'s own props (mirroring `flowAgents`/`flowSelectedAgentId`/`onFlowSelectAgent` from Task B3). Add a `panelDefs['needs-you']` entry with its own `referenceChain` — for all opt-in panels in this plan, use the consistent chain `['todos', 'flow', 'executionRail', 'transcript']` (anchors new panels near the other secondary content, falls back toward the always-present transcript; a deliberate, shared choice so 14 independently-implemented tasks don't each invent a different, untested chain).

In the refresh effect, add **only an update branch, no create branch**:

```tsx
api.getPanel('needs-you')?.update({ params: { node: needsYouNode } });
```

Creating the panel happens *only* via the Panels-menu "Add" button (Step 4).

- [ ] **Step 4: Add an "Add panel" section to the Panels popover for opt-in surfaces**

Extend the popover contents built in Task 1.5 Step 6 with a second section:

```tsx
<div className="my-1 border-t border-border-subtle" />
<div className="px-2 py-1 text-[10px] uppercase tracking-wide text-text-muted/60">Add</div>
{OPT_IN_PANEL_IDS.filter(id => !dockApiRef.current?.getPanel(id)).map(id => (
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
```

- [ ] **Step 5: Run tests to verify they pass, then the full suite**

Run the two target tests, then `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, zero regressions.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): wire Needs You as an opt-in dockable panel reachable from Chat's Panels menu"
```

### Tasks 2.2 – 2.6: the remaining 5 strong candidates, same pattern as Task 2.1

Each is its own task, its own commit, sequential (shared-file writes) but ideally preceded by the parallel research pass described above. Order per task: **(1) read real data source + check for a duplicate `<h1>` in the target component, (2) write the failing tests — both the mount-via-Panels-menu test AND the close-then-unrelated-rerender-stays-closed test, following Task 2.1's exact two-test shape, (3) run tests to verify they fail, (4) implement — add to `OPT_IN_PANEL_IDS`, `CHAT_DOCK_COMPONENTS` (quote hyphenated keys), `panelDefs` with the shared `['todos', 'flow', 'executionRail', 'transcript']` referenceChain, an update-only refresh-effect line, and an Add-menu entry (already generic from Task 2.1, no per-task change needed there), (5) run tests to verify they pass, then the full suite, (6) commit.** Do not batch multiple surfaces into one task/commit.

| Task | Surface (`surfaceComponents.tsx` case) | Component | Panel id / testid | Duplicate `<h1>` check needed? |
|---|---|---|---|---|
| 2.2 | `vox-search` / `graphify` | `VoxGraphStatusPanel.tsx` | `voxgraph` / `chat-dock-voxgraph` | Check — not yet verified |
| 2.3 | `activity` | `DiscoverySurface.tsx` | `activity` / `chat-dock-activity` | Check — not yet verified; also spot-check the Review sub-view's fixed 460px inspector popover at dock-panel width |
| 2.4 | `repository` | `RepositoryView.tsx` | `repository` / `chat-dock-repository` | Check — not yet verified |
| 2.5 | `mercatus` | `Mercatus.tsx` | `mercatus` / `chat-dock-mercatus` | Check — not yet verified |
| 2.6 | `harness` | `HarnessRedirect.tsx` | `harness` / `chat-dock-harness` | Check — not yet verified (unlikely, it's a near-empty stub) |

### Task 2.7: Phase 2 whole-effort verification

**Files:** none (verification only)

Per `superpowers:verification-before-completion`. Run backend+frontend suites with real output. Rebuild+relaunch+manually smoke-test: open all 6 new panels together via Panels▾'s Add section (not one at a time — confirm the layout stays sane with several optional panels open simultaneously, not just each in isolation), confirm close/reopen/drag/resize all work, confirm Reset layout removes them without recreating them. Root-cause any real bug via `systematic-debugging` before fixing. Report the commit range.

---

## Phase 3: Wire in the 8 condensed-capable candidates

Each needs a real condensed/full toggle. **FIXED — this is now built on a verified API, not left as an unresolved hedge**: `props.api.width`/`props.api.height` (confirmed real, `PanelApi`) plus `props.api.onDidDimensionsChange` give every dock panel component a live, reactive read of its own rendered size — no `ResizeObserver`, no navigate-away fallback needed as the default.

### Task 3.1 (worked example): Wire `approvals` in with a real width-driven condensed/full toggle

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

Approvals' working table needs ~850-1000px (4 fixed columns sum to 710px before the description column gets room). Condensed content: pending-approval count + current permission mode. **FIXED — double-polling avoidance**: `ApprovalsView.tsx` runs its own `setInterval` poll loop; the condensed state must use only the already-threaded `pendingApprovals` summary prop (confirmed real at `App.tsx:1271`), never mount a second live `ApprovalsView` instance — if a user opens both the top-level Approvals tab and this dock panel, only the full-view "Open full view" link (which navigates to the real tab, not an inline embed) avoids a second poll loop competing with the first.

- [ ] **Step 1: Read the real prop source, check for a duplicate `<h1>`**

Run: `grep -n "pendingApprovals\|permission.*mode" crates/vox-gui/ui/src/App.tsx` — confirm the exact prop/state names. Run: `grep -n "<h1" crates/vox-gui/ui/src/components/surfaces/Approvals/ApprovalsView.tsx`.

- [ ] **Step 2: Write the failing tests**

```tsx
it('Approvals panel shows a condensed pending-count badge when docked narrow', () => {
  render(
    <ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} pendingApprovals={3} />,
  );
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^approvals$/i }));
  const panel = screen.getByTestId('chat-dock-approvals');
  expect(panel).toHaveTextContent('3 pending');
  // The full 4-column table must not render — condensed state renders
  // ApprovalsDockPanel's own summary markup only, never <ApprovalsView>.
  expect(screen.queryByRole('table')).toBeNull();
});

it('Approvals panel switches to a full-view link, not an inline table, when the toggle mechanism decides it is wide enough', () => {
  // dockview's own width isn't measurable in jsdom (no real layout engine),
  // so this test exercises ApprovalsDockPanel directly with a mocked
  // DockviewPanelApi rather than through the full ChatSurface dock — mirrors
  // how other width-dependent behavior in this codebase is unit-tested at
  // the component level when the full dockview integration can't produce
  // real pixel measurements under jsdom.
  const mockApi = { width: 900, onDidDimensionsChange: vi.fn(() => ({ dispose: vi.fn() })) } as any;
  render(
    <ApprovalsDockPanel
      api={mockApi}
      params={{ pendingApprovals: 3, permissionMode: 'Ask', onNavigate: vi.fn() }}
    />,
  );
  expect(screen.getByRole('link', { name: /open full view/i })).toBeInTheDocument();
});
```

- [ ] **Step 3: Run tests to verify they fail, then implement**

```tsx
const APPROVALS_FULL_WIDTH_PX = 850; // from the audit: 4 fixed columns sum to 710px before the description column gets room

function ApprovalsDockPanel(props: IDockviewPanelProps<{ pendingApprovals: number; permissionMode: string; onNavigate?: (viewKey: string) => void }>) {
  const [width, setWidth] = React.useState(props.api.width);
  React.useEffect(() => {
    const disposable = props.api.onDidDimensionsChange(() => setWidth(props.api.width));
    return () => disposable.dispose();
  }, [props.api]);

  return (
    <div data-testid="chat-dock-approvals" className="h-full overflow-y-auto p-2 text-xs">
      <div className="mb-2">{props.params.pendingApprovals} pending · {props.params.permissionMode}</div>
      {width >= APPROVALS_FULL_WIDTH_PX ? (
        <a
          role="link"
          href="#"
          onClick={e => { e.preventDefault(); props.params.onNavigate?.('approvals'); }}
          className="text-brass hover:underline"
        >
          Open full view
        </a>
      ) : null}
    </div>
  );
}
```

Wire `approvals` into `OPT_IN_PANEL_IDS`, `CHAT_DOCK_COMPONENTS` (`approvals: ApprovalsDockPanel`), `panelDefs` (referenceChain `['todos', 'flow', 'executionRail', 'transcript']`, matching Phase 2's shared chain), the refresh effect's update-only line, and thread `pendingApprovals`/permission-mode/`onNavigate` through as `params`.

- [ ] **Step 4: Run tests to verify they pass, then the full suite**

Run the target tests, then `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, zero regressions.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): wire Approvals as an opt-in dockable panel with a real width-driven condensed/full toggle"
```

### Tasks 3.2 – 3.8: the remaining 7 condensed-capable surfaces, same pattern as Task 3.1

Same task shape as Task 3.1 (read source + h1 check, two failing tests including the resurrection-guard test from Phase 2's pattern, implement using `props.api.width`/`.height` with the per-surface threshold below, run+commit). **FIXED — every row now has an explicit numeric threshold** (from the audit) instead of silently inheriting Approvals' 850px, and Gamify is explicitly flagged as **height-driven**, not width-driven, since `props.api.height` exists for exactly this case.

| Task | Surface | Component | Panel id / testid | Condensed content | Toggle threshold | Special note |
|---|---|---|---|---|---|---|
| 3.2 | `mesh` | `MeshView.tsx` | `mesh` / `chat-dock-mesh` | Node count + source + pending queue count (existing header chips — reuse, don't re-derive) | `width >= 400` | `MeshView` runs its own poll loop — condensed state uses only the already-existing summary chips' data source, never mounts `<MeshView>` inline |
| 3.3 | `tasks` (global queue) | `TasksView.tsx` | `tasks` / `chat-dock-tasks` | "N blocked · M queued · K in progress" (existing `groupBy` categories) | `width >= 420` | **Naming collision guard**: unrelated to the `todos` panel from Task 1.3 — never reuse that id/title. **`<h1>` collision confirmed**: `TasksView.tsx:303` renders an unconditional `<h1>Tasks</h1>` — the dock-panel wrapper must render only the count summary, never mount `<TasksView>`'s own root, so its `<h1>` never appears alongside `ChatSurface`'s own `<h1 className="sr-only">Chat</h1>` |
| 3.4 | `coderabbit` | `CodeRabbitView.tsx` | `coderabbit` / `chat-dock-coderabbit` | Token status + "Planned N PRs · M files" | `width >= 640` | The 5-field control row should stack, not truncate, when condensed |
| 3.5 | `skills` | `SkillsPluginsView.tsx` | `skills` / `chat-dock-skills` | "Skills: N · Plugins: M" | `width >= 700` | Full 8/4 grid + marketplace search only render above threshold |
| 3.6 | `gamify` | `GamifyView.tsx` | `gamify` / `chat-dock-gamify` | HP/level/leaderboard rank | **`height >= 560`** (not width — `LudusSandbox`'s constraint is a fixed 560px height) | The `LudusSandbox` mini-map must never render below the height threshold — this is the one surface where the toggle must check `props.api.height`, not `.width` |
| 3.7 | `models` | `ModelsView.tsx` | `models` / `chat-dock-models` | Active model name + total count | `width >= 700` | Full 1/2/3-column card grid only useful above threshold |
| 3.8 | `memory` | `MemoryView.tsx` | `memory` / `chat-dock-memory` | "{N} corpora active · {M} indexed entries" | `width >= 550` | The pre-existing `SHARD_COLS = 6` virtualizer bug (row-height math ignores actual responsive column count) is out of scope here — flag as a follow-up, don't fix |

### Task 3.9: Phase 3 whole-effort verification

**Files:** none (verification only)

Per `superpowers:verification-before-completion`.

- [ ] **Step 1: Run the full backend and frontend suites, read the real output**

- [ ] **Step 2: Add the persisted-layout backward-compatibility test**

```tsx
// Add to ChatSurface.test.tsx
it('restores cleanly from a pre-Phase-3 persisted layout that only knows about the 5 core panels', async () => {
  const { layoutStorageKeyFor } = await import('../../dock/DockWorkspaceShell');
  const oldLayout = {
    grid: {
      root: { type: 'branch', data: [], size: 100 },
      height: 100,
      width: 100,
      orientation: 'HORIZONTAL',
    },
  }; // shape doesn't need to be fully valid dockview JSON — this only proves the try/catch fallback engages cleanly, not a specific restored geometry
  window.localStorage.setItem(layoutStorageKeyFor('gui.chat'), JSON.stringify(oldLayout));
  expect(() =>
    render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />),
  ).not.toThrow();
});
```

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx` — Expected: PASS (the existing `try/catch` in `DockWorkspaceShell.handleReady` already handles this; this test proves it, closing a gap the audit flagged as untested).

- [ ] **Step 3: Rebuild, relaunch, manually smoke-test**

Open several optional panels together (not one at a time) at both a wide and a narrow docked width, confirming each condensed/full toggle actually engages at its documented threshold — including Gamify's height-based one (resize the panel's height, not width, to confirm). Root-cause any real bug via `systematic-debugging`. Report the commit range.

- [ ] **Step 4: Commit the backward-compat test**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "test(gui): lock in graceful restore from a pre-Phase-3 persisted layout"
```

- [ ] **Step 5: Use `superpowers:finishing-a-development-branch`**

Once Phase 3 is clean, invoke it to decide how this whole effort (Phases 1-3) gets integrated — do not assume "commit and stop."

---

## Explicitly not in this plan

Dashboard/`DashboardGrid` unification, true external drag-from-sidebar-to-dock, double-click-splitter-to-reset, a native OS menu bar, and wiring Settings/Flow/Catalog/Browser/Console/Policies/Runs into any dock workspace. None of these get a task here — deferred, not forgotten.
