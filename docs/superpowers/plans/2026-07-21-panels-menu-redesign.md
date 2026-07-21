# Panels Menu Redesign Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development. This plan is already running inside worktree `C:\Users\Owner\vox\.worktrees\axis-frontend-remediation` — do not create another. Cite `superpowers:verification-before-completion` at the final task; `superpowers:systematic-debugging` for any real bug found during it.

**Goal:** Checkboxes (live-apply) replace click-buttons in the Panels menu; new panels position by activation order; the dockview shell fills the Chat tab's full content area; panel backgrounds use the app's real token instead of a hand-tuned near-match; the 6 already-wired "strong" panels get a width-driven condensed view using this session's own measured thresholds.

**Tech Stack:** React 19, TypeScript, dockview 6.6.1 (`props.api.width`/`onDidDimensionsChange`, already proven for Approvals). Verification: `npx vitest run` for jsdom-reachable behavior, live CDP screenshots (`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`) for anything jsdom cannot see (layout, color, width thresholds) — this session found 2 real bugs 100%-passing jsdom suites missed entirely, so CDP verification is mandatory for Tasks 3-5, not optional.

**Ground truth:** `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` currently has `CORE_PANEL_IDS`/`OPT_IN_PANEL_IDS`, `panelDefs` (title/params/referenceChain per id), `addDefaultPanel(api, id)`, and a two-section Panels popover (core reopen list, opt-in "Add" list). `crates/vox-gui/ui/src/styles/dockview-vox.css` has `--dv-background-color: rgba(9, 9, 11, 0.92)` (hand-tuned, should be `var(--color-bg-base)` = `#0c0e10`, confirmed in `tokens.generated.css:41`). Original audit's per-panel minimum widths (for Task 5): Mercatus ~320-360px, Repository ~260-300px, Needs You ~260-280px, Search Index (vox-search) ~220-260px per card, Discovery (activity) ~360px, Harness ~200px (trivial, likely never needs condensing).

---

### Task 1: Activation-order list replaces `referenceChain` for opt-in panels

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`, extend `ChatSurface.test.tsx`.

- [ ] **Step 1: Write the failing test**

```tsx
it('positions newly-activated opt-in panels after the most-recently-activated one, not a fixed referenceChain slot', async () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
  await screen.findByTestId('chat-dock-mercatus');
  fireEvent.click(screen.getByRole('checkbox', { name: /^harness$/i }));
  await screen.findByTestId('chat-dock-harness');

  // Harness activated after Mercatus — its addPanel call must have used
  // Mercatus as its referencePanel, not panelDefs' old hardcoded chain.
  // Assert via the dockview API directly (position isn't reliably
  // observable from rendered DOM order in jsdom).
  // (Concrete assertion approach depends on Step 2's real implementation —
  // if activation order is tracked in a ref exposed for testing, assert
  // against that; otherwise assert dockApiRef's panel.group relationship.)
});
```

Note: this test's exact assertion mechanism must be finalized once Step 2's real data structure exists — do not leave it vague; the implementer must write a real, specific assertion here before calling this RED, per this session's established no-placeholder-tests discipline (a prior task in this same effort was flagged in review for exactly this gap).

- [ ] **Step 2: Replace `referenceChain` with an activation-order ref**

```tsx
// Remove `referenceChain` from panelDefs' type and every entry (core
// panels' fixed chain stays — this only affects opt-in panels).
const activationOrderRef = useRef<string[]>([]);

const positionForActivation = (api: DockviewApi): { direction: 'right'; referencePanel: string } | undefined => {
  const last = activationOrderRef.current[activationOrderRef.current.length - 1];
  if (last && api.getPanel(last)) return { direction: 'right', referencePanel: last };
  // Fall back to the anchor chain only when nothing has been activated yet.
  const anchor = ['todos', 'flow', 'executionRail', 'transcript'].find(id => api.getPanel(id));
  return anchor ? { direction: 'right', referencePanel: anchor } : undefined;
};
```

Update `addDefaultPanel` to use `positionForActivation(api)` for opt-in ids (keep core ids' existing fixed-chain behavior unchanged), and to push/re-push the id to the end of `activationOrderRef.current` (removing any existing occurrence first) on every successful add.

- [ ] **Step 3: Run test to verify it passes, then the full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, zero regressions from current baseline (confirm the exact count yourself first).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): opt-in panels position by activation order, not a fixed referenceChain"
```

### Task 2: Checkboxes replace Add-list buttons, live-apply, unified core+opt-in list

**Files:** Modify `ChatSurface.tsx`, extend `ChatSurface.test.tsx`.

- [ ] **Step 1: Write the failing test**

```tsx
it('Panels menu uses checkboxes, live-applying on check without closing the dropdown', async () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  const checkbox = screen.getByRole('checkbox', { name: /^mercatus$/i });
  expect(checkbox).not.toBeChecked();
  fireEvent.click(checkbox);
  await screen.findByTestId('chat-dock-mercatus');
  expect(checkbox).toBeChecked();
  // Dropdown stays open — the trigger's aria-expanded must still be true.
  expect(screen.getByRole('button', { name: /panels/i }).getAttribute('aria-expanded')).toBe('true');
});

it('unchecking a panel closes it', async () => {
  render(<ChatSurface pushToast={vi.fn()} onNavigate={vi.fn()} messages={[]} composer={<div>composer</div>} />);
  fireEvent.click(screen.getByRole('button', { name: /panels/i }));
  fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
  await screen.findByTestId('chat-dock-mercatus');
  fireEvent.click(screen.getByRole('checkbox', { name: /^mercatus$/i }));
  await waitFor(() => expect(screen.queryByTestId('chat-dock-mercatus')).toBeNull());
});
```

- [ ] **Step 2: Run tests to verify they fail, then replace the popover contents**

Merge the two existing sections (core reopen list + opt-in Add list) into one list over `[...CORE_PANEL_IDS.filter(...), ...OPT_IN_PANEL_IDS]`, each row a labeled checkbox:

```tsx
{[...CORE_PANEL_IDS.filter(id => id !== 'executionRail' || executionRailNode != null), ...OPT_IN_PANEL_IDS].map(id => {
  const isOpen = !!dockApiRef.current?.getPanel(id);
  return (
    <label key={id} className="flex items-center gap-2 rounded px-2 py-1.5 text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary">
      <input
        type="checkbox"
        checked={isOpen}
        onChange={() => {
          const api = dockApiRef.current;
          if (!api) return;
          const panel = api.getPanel(id);
          if (panel) {
            api.removePanel(panel);
            closedPanelIds.current.add(id); // core panels still need this guard
          } else {
            addDefaultPanel(api, id);
          }
        }}
      />
      {panelDefs[id].title}
    </label>
  );
})}
```

Remove the old separate button-list + "All panels open" fallback + "Add" divider/label — the unified checkbox list replaces both.

- [ ] **Step 3: Run tests to verify they pass, then the full suite**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.test.tsx
git commit -m "feat(gui): Panels menu uses a unified live-apply checkbox list instead of separate reopen/add button sections"
```

### Task 3: Dockview shell fills the Chat tab's full content area

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx` (root container classes) and/or `crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx` if the ceiling is there instead.

- [ ] **Step 1: Investigate the current height/width ceiling**

Read `ChatSurface.tsx`'s root JSX (`data-testid="chat-surface-layout"`) and trace every ancestor up through `AppShell.tsx`/`SurfaceScrollHost.tsx` (already documented in this session's own commit history — search `h-full`/`min-h-[60vh]`) to find any remaining fixed/min-height or max-width constraint that stops the dockview area from using 100% of the Chat tab's real available space.

- [ ] **Step 2: Fix, verify via CDP, no jsdom test possible for this one**

This is a pure layout task — jsdom cannot verify real fill behavior (already proven twice this session). After the CSS change, rebuild (`pnpm build` + `cargo build --release -p vox-gui`), relaunch with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`, and use `Runtime.evaluate` + `getBoundingClientRect()` on `.dockview-theme-vox` (or its current equivalent) to confirm its measured width/height matches (or comes acceptably close to) the Chat tab's actual content-area dimensions, not a `60vh`-capped island. Take a real screenshot as visual confirmation.

- [ ] **Step 3: Run the full jsdom suite as a regression check**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, identical counts (CSS-only change).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx
git commit -m "fix(gui): dockview shell fills the Chat tab's full content area, not a 60vh-capped island"
```

### Task 4: Panel background uses the real `--color-bg-base` token

**Files:** Modify `crates/vox-gui/ui/src/styles/dockview-vox.css`.

- [ ] **Step 1: Replace the hand-tuned rgba with the real token**

```css
.dockview-theme-vox {
  --dv-background-color: var(--color-bg-base);
  /* was: rgba(9, 9, 11, 0.92) — a separately hand-tuned near-match that
     created a visible seam against the app's real background. */
  ...
}
```

Check whether `--dv-tabs-and-actions-container-background-color` or any other var should also reference `--color-bg-base` (or a small, deliberate step up from it) rather than its own separate rgba guess — read the full current file and decide per-property, don't blanket-replace everything without checking each one still makes visual sense (some, like the brass active-tab tint, are deliberately different from the base background and should stay).

- [ ] **Step 2: Run the jsdom suite (regression check), then verify via CDP**

Run: `cd crates/vox-gui/ui && pnpm exec vitest run` and `npx tsc --noEmit` — 100% pass, identical counts. Then rebuild, relaunch, take a real screenshot, and visually confirm the panel background no longer reads as a distinct dark blue against the rest of the app.

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/src/styles/dockview-vox.css
git commit -m "fix(gui): dockview panel background uses the real --color-bg-base token instead of a hand-tuned near-match"
```

### Task 5: Width-driven condensed view for the 6 already-wired strong panels

**Files:** Modify `ChatSurface.tsx`, extend `ChatSurface.test.tsx`. One sub-task per panel (5.1-5.6), same TDD shape as Task 3.1's `ApprovalsDockPanel` (already landed, proven pattern) — do not batch, each is its own commit.

For each panel below, wrap its existing `XDockPanel` component (currently `<div>{props.params.node}</div>`) with the same `props.api.width` + `onDidDimensionsChange` pattern `ApprovalsDockPanel` already uses, showing a condensed 1-line summary below the panel's own audited minimum width and the full component at/above it. Condensed summary content: reuse whatever the panel's own top area already surfaces as a glanceable stat (e.g. Mercatus's `"{parts.length} parts · {sources.length} enabled sources"` line, Repository's active-conflict count, Needs You's total-count badge) — do not invent new summary text; find and reuse what's already computed inside each component, same discipline as Approvals reusing `pendingApprovals`.

| Task | Panel | Threshold (from the original audit) |
|---|---|---|
| 5.1 | Mercatus | `width >= 340` |
| 5.2 | Repository | `width >= 280` |
| 5.3 | Needs You | `width >= 270` |
| 5.4 | Search Index (voxgraph) | `width >= 240` |
| 5.5 | Discovery (activity) | `width >= 360` |
| 5.6 | Harness | none needed — already a trivial empty-state stub, skip the toggle entirely, just confirm via CDP it still renders fine narrow |

- [ ] **Step (per panel, 5.1-5.5): Write the failing test, implement using `ApprovalsDockPanel`'s exact mechanism, verify, commit**

Mirror Task 3.1's real committed code (`crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`, search `ApprovalsDockPanel`) exactly — same `useState`/`useEffect`/`onDidDimensionsChange`/`dispose()` shape, different threshold and different condensed content per the table above. Test with a mocked `DockviewPanelApi` the same way Task 3.1's test did (`{ width: N, onDidDimensionsChange: vi.fn(() => ({dispose: vi.fn()})) }`).

- [ ] **Step (5.6): Confirm Harness needs no change**

Run a quick CDP check that Harness already renders fine at typical docked widths (it's a near-empty `EmptyState` stub per the original audit) — no code change, just confirmation; note this in a short commit-free report rather than a commit if truly nothing changes.

### Task 6: Whole-effort verification

**Files:** none (verification only)

Per `superpowers:verification-before-completion`.

- [ ] **Step 1: Run the full backend and frontend suites, read the real output**

`cargo test -p vox-orchestrator --lib` (expect no change), `npx tsc --noEmit`, `npx vitest run` in `crates/vox-gui/ui`.

- [ ] **Step 2: Rebuild, relaunch, verify live via CDP**

Rebuild both frontend and `vox-gui` release binary, relaunch with `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222`. Via CDP: open the Panels menu, check several boxes in sequence without closing the dropdown, confirm each panel appears immediately and in activation order (left-to-right or whichever direction the dockview default-split resolves to); uncheck one, confirm it closes; confirm the dockview area visually fills the Chat tab's full content region; confirm the panel background no longer looks distinctly blue; resize the window narrow and confirm at least 2 of the 5 newly-condensed panels (Task 5) show their condensed summary instead of overflowing/breaking. Take real screenshots as evidence, not verbal claims.

- [ ] **Step 3: Report; use `systematic-debugging` for any real bug found**

Root-cause any real bug via `superpowers:systematic-debugging` before writing its regression test and fix. Report the final commit range for this plan's work.
