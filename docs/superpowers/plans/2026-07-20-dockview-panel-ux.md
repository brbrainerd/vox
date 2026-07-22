# Dockview Panel UX Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. `subagent-driven-development` also names `superpowers:requesting-code-review` (two-stage review after each task) and `superpowers:finishing-a-development-branch` (after Task 6) as required companion skills — follow those, not just this document's own steps.

> **PREREQUISITE — READ BEFORE STARTING:** This plan depends on Tasks B4, B5, and B6 of `docs/superpowers/plans/2026-07-20-chat-flow-docking-redesign.md` having already landed on this branch. Run `git log --oneline -- crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` and confirm commits implementing B4 (layout persistence) and B6 (visual pass) are present. If not, stop and implement those first.
>
> **Do not run this plan's tasks concurrently with an in-progress B4-B6 effort** — same file, same `onReady`/`CHAT_DOCK_COMPONENTS`/refresh-`useEffect` region, guaranteed merge conflicts.
>
> Because B4-B6 will change exact line numbers and possibly the guard/refresh style shown below (written against the current, pre-B4 file), **re-read the actual current file before each task's implementation step** rather than trusting this plan's line numbers verbatim. Before starting Task 3, also grep B4's landed diff for any panel-close/registry tracking it may already have introduced (plausible, since serializing a layout needs to know current panel membership) — reuse it instead of adding a second mechanism if one exists.

**Goal:** Fix the Plan panel's "hangs in empty space, still reserves width while collapsed" bug by migrating it into dockview as a real panel, fix a pre-existing bug where panel auto-recreation fights the user's own close action, and add a small panel-management menu (reopen a closed panel, reset the whole layout).

**Architecture:** `ChatSurface.tsx` hosts four dockview panels (sessions, transcript, executionRail, flow — landed in Tasks B1-B3) plus one hand-rolled non-dockview panel (Plan, with a `useLocalStorage`-backed collapse toggle whose "collapsed" state is still a non-zero-width `<aside>`, not a real removal). This plan: (1) adds a `closedPanelIds` tracking mechanism so the existing no-dependency-array refresh effect stops silently re-adding panels the user just closed — a real bug already present for `executionRail`/`flow` today, not something this plan introduces; (2) migrates Plan into dockview as a fifth panel using the now-fixed pattern; (3) extracts one shared per-id panel-creation function (preserving the existing reference-panel fallback chain) used by the refresh effect, a reopen menu, and a reset action; (4) adds that reopen/reset menu as a plain (non-ARIA-menu) popover with Escape/outside-click/focus-return.

**Tech Stack:** React 19, TypeScript, dockview 6.6.1 (`DockviewApi.panels`, `.getPanel(id)`, `.removePanel(panel)`, `.addPanel(...)`, `.onDidRemovePanel(...)`), Vitest + Testing Library.

**Ground truth confirmed for this plan (verified against the actual current file — re-verify per the prerequisite note above if B4-B6 have landed since):** `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` is 501 lines. The root JSX container (line 390) is `className="relative flex min-h-[60vh] gap-4"` — **still a flex row**; do not assume it lost `flex` (an earlier draft of this plan's paired design spec incorrectly claimed that; corrected). `CHAT_DOCK_COMPONENTS` (lines 55-60) maps `sessions`/`transcript`/`executionRail`/`flow`. `dockApiRef` (line 137, `useRef<DockviewApi | null>(null)`) is set in `onReady` (line 402). The refresh `useEffect` (lines 341-386, no dependency array) calls `panel.update({ params: { node } })` on each existing panel, and for `executionRail`/`flow` lazily calls `api.addPanel(...)` when the panel doesn't yet exist — **this lazy-create branch has no way to distinguish "not yet ready" from "user closed it," which is the bug Task 1 below fixes.** The old Plan panel block is at lines 424-464 (`{planPanelCollapsed ? <aside>...collapsed strip...</aside> : <aside>...expanded panel...</aside>}`), collapse state at lines 129-132 (`useLocalStorage<boolean>('gui.chat.plan_panel_collapsed.v1', false)`). `PlanPanel`'s real props (verified against `crates/vox-gui/ui/src/components/surfaces/Chat/PlanPanel.tsx`): exactly `planSessionId`, `planVersion`, `nodes` — no `planNodes` prop; `planNodes` is only the `ChatSurface`-local state variable name passed as `nodes`.

`DockviewApi` (verified against `node_modules/.pnpm/dockview-core@6.6.1/node_modules/dockview-core/dist/esm/api/component.api.d.ts`) exposes `.panels: IDockviewPanel[]` (a **freshly computed array on every access**, not a live reference — safe to iterate while mutating via `removePanel`), `.getPanel(id: string): IDockviewPanel | undefined`, `.removePanel(panel: IDockviewPanel): void` (takes only the panel, no second argument, unlike `SplitviewApi`/`GridviewApi`'s `removePanel`), `.addPanel<T>(options: AddPanelOptions<T>): IDockviewPanel`, and `.onDidRemovePanel(callback): IDisposable` (fires when any panel is removed, whether by a user's tab-close click or a programmatic `removePanel` call — this plan does not need to distinguish the two, since the fix's job is just "don't auto-recreate a panel that was removed until something explicitly asks for it back," regardless of why it was removed).

**Real dockview-react tab DOM (verified against `node_modules/.pnpm/dockview@6.6.1_react@19.2.7/.../dist/esm/dockview/defaultTab.js`, since an earlier draft of this plan guessed wrong):** a tab renders as `<div data-testid="dockview-dv-default-tab" className="dv-default-tab">`, containing `<span className="dv-default-tab-content">{title}</span>` and, as a **sibling** (not descendant) of that span, `<div className="dv-default-tab-action" onClick={onClose}>` wrapping the close icon. No element has "close" as a CSS class fragment — `[class*="close"]` matches nothing. The close button renders unconditionally (no `hideClose`/`closeable` option is passed anywhere in this codebase's `ChatDockShell.tsx`/`addPanel` calls) — every tab always has one.

---

### Task 1: Stop the refresh effect from re-adding panels the user just closed

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

This fixes a real, pre-existing bug (affects the already-landed `executionRail`/`flow` panels today) before this plan's later tasks give the Plan panel the same lazy-create pattern and would otherwise import the bug a third time.

- [ ] **Step 1: Read the current file fresh**

Read `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` in full right now — do not trust this plan's line numbers if B4-B6 landed since this plan was written. Find the current refresh `useEffect` (search for `api.getPanel('flow')`) and the current `onReady` callback (search for `dockApiRef.current = event.api`).

- [ ] **Step 2: Write the failing test**

Add to `ChatSurface.test.tsx` (reuse the render/mock conventions of the existing `'mounts sessions, chat, and execution rail as dockview panels'` test):

```tsx
it('does not resurrect the Flow panel on the next render after the user closes it', async () => {
  const { rerender } = render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
    />,
  );
  await screen.findByTestId('chat-dock-flow');

  // Close the way a user would: find the real dockview tab close action,
  // not a guessed selector — confirmed DOM shape (see this plan's "Real
  // dockview-react tab DOM" note): the tab is
  // [data-testid="dockview-dv-default-tab"], the close click target inside
  // it is .dv-default-tab-action.
  const flowTab = screen.getByText('Flow').closest('[data-testid="dockview-dv-default-tab"]') as HTMLElement;
  const closeBtn = flowTab.querySelector('.dv-default-tab-action') as HTMLElement;
  fireEvent.click(closeBtn);
  await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

  // Force an unrelated re-render — the exact trigger a real close-fighting
  // bug would react to (a streamed token, a session poll, anything that
  // isn't the user reopening the panel).
  rerender(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[{ id: 'm1', role: 'user', text: 'hello', status: 'done' } as any]}
      composer={<div>composer</div>}
    />,
  );

  // The bug: without a fix, the refresh effect sees getPanel('flow') is
  // undefined and immediately re-adds it, fighting the user's own close.
  expect(screen.queryByTestId('chat-dock-flow')).toBeNull();
});
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — the final `expect(screen.queryByTestId('chat-dock-flow')).toBeNull()` fails because the panel comes back (the refresh effect re-added it on the forced re-render). Confirm the failure is specifically this assertion failing after the panel reappears, not an error closing the tab in the first place — if the close-button click itself doesn't work, your selector doesn't match reality; re-verify against the real rendered DOM (`screen.debug()`) before proceeding.

- [ ] **Step 4: Add closed-panel tracking**

Add near `dockApiRef`:

```tsx
// Tracks panels removed (by the user's tab-close, or by the reset action
// below) so the refresh effect below can tell "not yet ready to create"
// apart from "the user closed this — leave it closed until something
// explicitly asks for it back." Cleared per-id by the reopen/reset actions
// (Tasks 4/5), not by this effect.
const closedPanelIds = useRef<Set<string>>(new Set());
```

In the `onReady` callback, right after `dockApiRef.current = event.api;`, register the listener:

```tsx
event.api.onDidRemovePanel(panel => {
  closedPanelIds.current.add(panel.id);
});
```

In the refresh `useEffect`, gate each lazy-create branch on the panel not being in `closedPanelIds`. For the existing `executionRail` branch (find `const executionPanel = api.getPanel('executionRail');`):

```tsx
const executionPanel = api.getPanel('executionRail');
if (executionRailNode) {
  if (executionPanel) {
    executionPanel.update({ params: { node: executionRailNode } });
  } else if (!closedPanelIds.current.has('executionRail')) {
    api.addPanel({
      id: 'executionRail',
      component: 'executionRail',
      title: 'Execution',
      params: { node: executionRailNode },
      position: { direction: 'right', referencePanel: 'transcript' },
    });
  }
} else if (executionPanel) {
  api.removePanel(executionPanel);
}
```

Apply the same `else if (!closedPanelIds.current.has('flow'))` guard to the existing `flow` lazy-create branch (find `const flowPanel = api.getPanel('flow');`), keeping its existing fallback `position` logic unchanged — only add the guard, don't alter the fallback chain.

- [ ] **Step 5: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS, including all pre-existing tests in this file.

- [ ] **Step 6: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "fix(gui): stop dockview refresh effect from re-adding panels the user just closed (executionRail, flow)"
```

---

### Task 2: Migrate the Plan panel into dockview, delete the old collapse toggle

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

- [ ] **Step 1: Write the failing test**

```tsx
it('mounts the Plan panel as a dockview panel, not a hand-rolled collapsible aside', () => {
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
  expect(screen.getByTestId('chat-dock-plan')).toBeInTheDocument();
  expect(screen.queryByLabelText('Collapse plan panel')).toBeNull();
  expect(screen.queryByLabelText('Expand plan panel')).toBeNull();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — no `chat-dock-plan` testid exists yet. Confirm it's this assertion failing, not an unrelated error.

- [ ] **Step 3: Add the `PlanDockPanel` component and register it**

```tsx
function PlanDockPanel(props: IDockviewPanelProps<{ node: React.ReactNode }>) {
  return <div data-testid="chat-dock-plan" className="h-full overflow-y-auto p-2">{props.params.node}</div>;
}
```

```tsx
const CHAT_DOCK_COMPONENTS = {
  sessions: SessionsPanel,
  transcript: TranscriptPanel,
  executionRail: ExecutionRailPanel,
  flow: FlowPanel,
  plan: PlanDockPanel,
};
```

- [ ] **Step 4: Build `planNode` and add it to the refresh effect, guarded like Task 1's fix**

Near where `flowNode` is built:

```tsx
const planNode = <PlanPanel planSessionId={planSessionId} planVersion={planVersion} nodes={planNodes} />;
```

In the refresh effect, add a `plan` branch mirroring `flow`'s shape exactly, including the `closedPanelIds` guard from Task 1:

```tsx
const planPanel = api.getPanel('plan');
if (planPanel) {
  planPanel.update({ params: { node: planNode } });
} else if (!closedPanelIds.current.has('plan')) {
  api.addPanel({
    id: 'plan',
    component: 'plan',
    title: 'Plan',
    params: { node: planNode },
    position: {
      direction: 'right',
      referencePanel: api.getPanel('flow') ? 'flow' : api.getPanel('executionRail') ? 'executionRail' : 'transcript',
    },
  });
}
```

Do not add `plan` to the initial `onReady` panel-creation block — like `executionRail`/`flow`, it's created lazily by the refresh effect once `planNode` exists.

- [ ] **Step 5: Delete the old Plan panel code**

Remove:
- The `planPanelCollapsed`/`setPlanPanelCollapsed` `useLocalStorage` declaration (search `gui.chat.plan_panel_collapsed.v1`).
- The entire old Plan-panel JSX block (both `<aside aria-label="Plan panel" ...>` variants and their toggle buttons).
- The `Glass` import, only if nothing else in this file still uses `<Glass>` (`grep -n "Glass" crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` — check before removing).

- [ ] **Step 6: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS, including all pre-existing tests.

- [ ] **Step 7: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass. Grep for any other file referencing `plan_panel_collapsed`/`Collapse plan panel`/`Expand plan panel` before committing.

- [ ] **Step 8: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): migrate Plan panel into dockview, removing the hand-rolled collapse toggle that reserved width even when collapsed"
```

---

### Task 3: Extract a shared per-id panel creator (with the real fallback chain), used by the refresh effect

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

Tasks 1-2 left three near-duplicate `addPanel` call shapes (executionRail, flow, plan) inline in the refresh effect, each with its own `position`/`referencePanel` fallback logic. Extract one function so Task 4's reopen menu and Task 5's reset don't have to reimplement (and potentially diverge from) that fallback logic.

- [ ] **Step 1: Write the failing test**

This must be a **behavioral** test — exercise the function through the real dock API, not a source-text grep (a text-match test can pass without the function ever running, which proves nothing). Extend the Task 1 close/reopen flow to prove the extracted creator is what actually gets called on reopen:

```tsx
it('recreates a closed panel via the shared per-id creator, preserving its reference-panel fallback', async () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
    />,
  );
  await screen.findByTestId('chat-dock-flow');

  const flowTab = screen.getByText('Flow').closest('[data-testid="dockview-dv-default-tab"]') as HTMLElement;
  fireEvent.click(flowTab.querySelector('.dv-default-tab-action') as HTMLElement);
  await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

  // Task 4 wires a real menu button to this; until then, exercise the
  // creator directly to prove it exists and works — this line will be
  // replaced by a menu click in Task 4's test, not duplicated.
  // (See Task 4 Step 1 for the end-to-end version of this scenario.)
});
```

Note: this step's test is intentionally provisional — Task 4 supersedes it with a real end-to-end menu-click test. Keep this one only long enough to prove `addDefaultPanel` compiles and runs against the real dockview API before wiring UI to it; if Task 4 is implemented in the same sitting, skip committing this provisional test standalone and fold the assertions directly into Task 4's test instead (do not leave two near-duplicate tests in the suite).

- [ ] **Step 2: Add `DEFAULT_PANEL_IDS` and the shared creator**

```tsx
const DEFAULT_PANEL_IDS = ['sessions', 'transcript', 'executionRail', 'flow', 'plan'] as const;
type ChatDockPanelId = (typeof DEFAULT_PANEL_IDS)[number];
```

Inside `ChatSurface`, after `planNode` is built (so all five node variables are in scope), add a lookup with an explicit **fallback chain** per panel — not a single hardcoded reference, since `flow`'s existing logic already falls back through `executionRail` → `transcript`, and `plan`'s Task 2 logic falls back through `flow` → `executionRail` → `transcript`; the shared creator must preserve this, not regress to one fixed reference:

```tsx
const panelDefs: Record<ChatDockPanelId, { title: string; node: React.ReactNode; referenceChain: ChatDockPanelId[] }> = {
  sessions: { title: 'Sessions', node: sessionRailNode, referenceChain: [] },
  transcript: { title: 'Chat', node: centerContent, referenceChain: ['sessions'] },
  executionRail: { title: 'Execution', node: executionRailNode, referenceChain: ['transcript'] },
  flow: { title: 'Flow', node: flowNode, referenceChain: ['executionRail', 'transcript'] },
  plan: { title: 'Plan', node: planNode, referenceChain: ['flow', 'executionRail', 'transcript'] },
};

// Plain function, not useCallback: panelDefs is a fresh object every
// render (it closes over freshly-built JSX like centerContent), so a
// memoized wrapper around it would never actually skip recomputation —
// wrapping it in useCallback would only look memoized without being so.
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

`addDefaultPanel` clearing `closedPanelIds` on call is what makes it safe for the refresh effect to keep treating `closedPanelIds` membership as authoritative — once a panel is explicitly recreated (via reopen or reset), it's no longer "closed."

Note: `executionRail`'s `node` is `executionRailNode`, which can be `null` — do not call `addDefaultPanel(api, 'executionRail')` when `executionRailNode == null` (callers in Tasks 4-5 must filter it out, same as the refresh effect already does).

- [ ] **Step 3: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass. `addDefaultPanel` is defined but not yet called by any UI in this task (Task 4 wires it in) — that's expected; if you kept Step 1's provisional test, it should pass by directly invoking `addDefaultPanel` through `dockApiRef` (e.g., via a test-only hook) rather than asserting nothing.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "refactor(gui): extract addDefaultPanel with the real reference-panel fallback chain, one source of truth for panel (re)creation"
```

---

### Task 4: Add the "Panels" popover with a reopen action

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

Per the design spec's explicit ARIA decision: this is a plain popover of buttons, not a `role="menu"` — `role="menu"` obligates full keyboard-menu semantics this codebase's one existing instance of the pattern (`ChatSessionRail.tsx`) doesn't implement, and this plan won't add a second broken instance of it. Split into two steps (trigger, then contents) per the writing-plans granularity guidance — Step 3 alone is too much for one step.

- [ ] **Step 1: Write the failing test for the trigger button**

```tsx
it('Panels button toggles a popover open and closed, with Escape and focus-return', () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
    />,
  );
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
Expected: FAIL — no "Panels" button exists yet.

- [ ] **Step 3: Add the trigger button and open/close/Escape/focus-return state**

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
```

Add the trigger button in the JSX, near the top of `ChatSurface`'s root (before the `<ChatDockShell>` wrapper):

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
      {/* Task 4 Step 5 fills this in */}
    </div>
  ) : null}
</div>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 5: Write the failing test for reopen + close-outside**

```tsx
it('Panels popover lists a closed panel and reopens it on click; clicking outside closes it', async () => {
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
    />,
  );
  await screen.findByTestId('chat-dock-flow');

  const flowTab = screen.getByText('Flow').closest('[data-testid="dockview-dv-default-tab"]') as HTMLElement;
  fireEvent.click(flowTab.querySelector('.dv-default-tab-action') as HTMLElement);
  await waitFor(() => expect(screen.queryByTestId('chat-dock-flow')).toBeNull());

  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /^flow$/i }));
  await waitFor(() => expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument());

  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.mouseDown(document.body);
  expect(screen.queryByText('All panels open')).toBeNull(); // popover closed, contents unmounted
});
```

- [ ] **Step 6: Run test to verify it fails**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — the popover has no list contents yet, and no outside-click handling.

- [ ] **Step 7: Fill in the popover contents and outside-click handling**

Replace the `{/* Task 4 Step 5 fills this in */}` placeholder:

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

Add outside-click handling alongside the Escape effect from Step 3 (extend that same `useEffect`, or add a sibling one keyed on `panelsMenuOpen`):

```tsx
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

Note this outside-click handler does not return focus to the trigger (only Escape and an explicit menu-item click do, per the design spec — clicking away is the user choosing to look elsewhere, forcing focus back would fight that).

- [ ] **Step 8: Run test to verify it passes**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS, including Task 3's provisional test if you kept it (fold its assertions in here and delete the standalone provisional version now, per Task 3 Step 1's note).

- [ ] **Step 9: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 10: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): add Panels popover to reopen a closed dockview panel (plain buttons, not role=menu — see design spec's ARIA decision)"
```

---

### Task 5: Add "Reset layout" to the Panels popover

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- Check: **both** `crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx` **and** `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` for wherever Task B4 actually put the layout-persistence storage key — B4's own plan text defines `LAYOUT_STORAGE_KEY` as a module-scope constant, but doesn't fix which of the two files it ends up in; check both, don't assume `ChatDockShell.tsx` alone.
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx` (extend)

- [ ] **Step 1: Confirm the real persistence mechanism from Task B4**

Run: `grep -rn "LAYOUT_STORAGE_KEY\|localStorage" crates/vox-gui/ui/src/components/surfaces/Chat/ChatDockShell.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`

Two possible outcomes:
- **A plain storage key exists** (e.g. `LAYOUT_STORAGE_KEY = 'gui.chat.dockview_layout.v1'`, accessed via `window.localStorage`): import the constant if it's exported from wherever it lives, rather than re-typing the string literal — grep confirmed whether it's exported; if not, ask whether to export it as part of this task rather than duplicating the literal in two files.
- **A `resetLayout()`-style callback is exposed instead** (e.g. a prop on `ChatDockShell`): this task's Step 3 code below assumes direct storage access and will not adapt by search-and-replace — if this is what you find, stop and write new code calling that real callback instead of `localStorage.removeItem`, then adjust Step 2's test to assert the callback fires rather than asserting on `localStorage` directly.

The steps below assume the plain-storage-key outcome, since that's what B4's own plan text specifies. Adjust if reality differs.

- [ ] **Step 2: Write the failing tests**

```tsx
it('Reset layout clears the persisted layout, all closedPanelIds, and recreates the default panels', async () => {
  window.localStorage.setItem('gui.chat.dockview_layout.v1', JSON.stringify({ grid: {} }));
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
    />,
  );
  await screen.findByTestId('chat-dock-flow');

  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('button', { name: /reset layout/i }));

  expect(window.localStorage.getItem('gui.chat.dockview_layout.v1')).toBeNull();
  await waitFor(() => {
    expect(screen.getByTestId('chat-dock-sessions')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-transcript')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-flow')).toBeInTheDocument();
    expect(screen.getByTestId('chat-dock-plan')).toBeInTheDocument();
  });
});

it('Reset layout does not throw when no layout was ever persisted', () => {
  window.localStorage.removeItem('gui.chat.dockview_layout.v1');
  render(
    <ChatSurface
      pushToast={vi.fn()}
      onNavigate={vi.fn()}
      messages={[]}
      composer={<div>composer</div>}
    />,
  );
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  expect(() => fireEvent.click(screen.getByRole('button', { name: /reset layout/i }))).not.toThrow();
});
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: FAIL — no "Reset layout" button exists yet.

- [ ] **Step 4: Add the reset action**

Add as a sibling to Task 4's popover contents, after a thin separator:

```tsx
<div className="my-1 border-t border-border-subtle" />
<button
  type="button"
  onClick={() => {
    window.localStorage.removeItem('gui.chat.dockview_layout.v1'); // or the real key/mechanism found in Step 1
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

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run src/components/surfaces/Chat/ChatSurface.test.tsx`
Expected: PASS.

- [ ] **Step 6: Run the full frontend suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit`
Expected: 100% pass, zero regressions.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): add Reset layout to the Panels popover"
```

---

### Task 6: Whole-effort verification

**Files:** none (verification only)

This task executes `superpowers:verification-before-completion`'s core rule — no completion claim without fresh, actually-observed evidence — and, if manual smoke-testing finds a real bug, `superpowers:systematic-debugging` (root-cause first, not a same-turn "I see the problem, let me fix it" patch).

- [ ] **Step 1: Run the full backend and frontend suites, read the real output**

Run `cargo test -p vox-orchestrator --lib` (expect no change — this plan touches no Rust code) and, in `crates/vox-gui/ui`: `npx tsc --noEmit` and `npx vitest run`. Read the actual pass/fail counts and exit codes; do not summarize from memory of earlier task-level runs.

- [ ] **Step 2: Rebuild and manually smoke-test**

Rebuild the frontend (`pnpm build`) and the `vox-gui` release binary (`cargo build --release -p vox-gui`), kill any running `vox-gui.exe`, relaunch it. On the Chat tab, by hand:
- Confirm the Plan panel is a real dockview tab: draggable to another edge, closeable via its tab's × button.
- Close it, confirm it does **not** reappear on its own after normal chat activity (send a message, wait) — this is Task 1's fix; if it reappears, that regression must be root-caused via `systematic-debugging`, not patched blind.
- Reopen it via the Panels popover; confirm Escape and outside-click both close the popover, and Escape returns focus to the Panels button (Tab from there should reach the next real focusable element, not get lost).
- Drag a panel to a different edge, then click "Reset layout"; confirm it returns to the default five-panel arrangement.

- [ ] **Step 3: Report, then use `superpowers:finishing-a-development-branch`**

If manual smoke-testing surfaces a real bug, root-cause it with `superpowers:systematic-debugging` before writing its regression test and fix. Once clean, report the final commit range for this plan's work, then follow `superpowers:finishing-a-development-branch` to decide how this work gets integrated (merge, PR, or further cleanup) — do not assume "commit and stop" is the finish line.
