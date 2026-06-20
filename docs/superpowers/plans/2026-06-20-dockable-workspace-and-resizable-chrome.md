---
title: "Dockable Surface Workspace + Resizable Sidebar — Implementation Plan"
category: "Architecture SSOTs"
---

# Dockable Surface Workspace + Resizable Sidebar Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the Vox GUI Photoshop/VS-Code-style layout control — any surface can be opened as a draggable, splittable, tabbable, closeable panel in the content region with a persisted layout, and the sidebar becomes freely drag-resizable — while keeping the sidebar and top bar as stable, discoverable chrome.

**Architecture:** Generalize the existing single-panel `DockShell` (already wrapping the `dockview` 6.6.1 docking library with persisted layout) into a multi-panel `DockWorkspace` driven by a `panelRegistry` that maps a `viewKey` to its existing surface renderer (`childRenderer` from `surfaceComponents.tsx`). The persisted dockview layout (`gui.layout.v1`) is the layout SSOT. The sidebar gains a drag handle that sets a free pixel width (persisted to `vox_sidebar_width`) layered on top of the existing rail/default/wide preset cycle. The top bar (`TopHud`) stays fixed chrome — documented design decision — retaining its full/slim/hidden collapse modes.

**Tech Stack:** React + TypeScript + Vitest + Testing Library; `dockview` 6.6.1; Tailwind; existing `voxTransport.getGuiPreference`/`setGuiPreference` for persistence (in `crates/vox-gui/ui`).

**Design rationale (good-GUI principles applied):**
- **Fixed chrome, dockable body.** Primary navigation (sidebar) and global status/command (top bar) stay put — like VS Code's activity bar and Photoshop's menu/toolbar. Spatial stability of navigation beats "everything floats," which harms discoverability and orientation. Only the *content region* becomes a free dock space. (This is why the top bar is intentionally **not** made into a floating panel.)
- **Progressive disclosure.** The sidebar keeps its three one-click presets (rail/default/wide) AND gains continuous drag-resize for power users; the drag snaps to the presets near their widths so casual users still land on tidy layouts.
- **Reversibility.** Every layout is persisted and there is always a one-click "Reset layout" escape hatch; unknown/old persisted panels self-heal instead of white-screening.
- **Theme cohesion.** Dockview chrome (tabs, drop zones, separators) is themed to the dark/brass palette via the existing `dockview-vox.css`.

**Scope note:** This plan is the layout/drag subsystem only. The context-window editor and editable-memory panels are a separate, already-written plan (`2026-06-19-dockable-workspace-context-memory-ssot.md`); this plan deliberately does **not** touch them, and does **not** retire the dnd-kit dashboard grid (the dashboard remains its own surface that can itself be docked as a panel). Each task below leaves the tree compiling, tested, and committed.

---

> **🤖 EXECUTION TARGET — READ FIRST.** This plan is written to be executed by **Gemini 3.5 Flash inside Google Antigravity** (known traits: ~48% unaided completion, no mid-task checkpoint, hard quota cutoff, occasional API hallucination, weak long-context recall). Background + mitigations: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). Because of these traits, the rules below are mandatory, not advisory.

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** Every task ends with a commit and a green tree. A crash between tasks must leave a compiling, tested repo. Never split a compile-breaking change across two commits.
2. **Verify-before-use is a BLOCKING gate.** Every task's Step 1 runs the shown `rg`/read command and you must paste its output **before** writing any code. If reality differs from what the task assumes (missing symbol, different signature) → **STOP**, write a one-paragraph handoff note (what you expected, what you found), and do not improvise.
3. **Self-contained.** Everything you need is in the task. Do not invent file paths, props, or APIs not shown here or confirmed by a Step-1 paste.
4. **No new dependencies.** `dockview` 6.6.1 is already installed. Do not run `npm install` / add packages. If a task seems to need one → STOP + handoff note.
5. **Two-strike circuit breaker.** If the same step fails twice, STOP and write a handoff note. Do not attempt a third variant.
6. **No placeholders, ever.** No `TODO`, no “handle edge cases”, no stubbed returns left behind. If you can't complete a step fully, STOP.
7. **Verification ritual before every commit (paste the output):** from `crates/vox-gui/ui` run `npx tsc --noEmit` (must exit 0) then `npx vitest run <the test file(s) for this task>` (must pass). For the final task also run the full `npx vitest run` + `npx vite build`.
8. **House rules (Vox).** TS-only changes here — no Rust, no `.ps1`/`.sh`/`.py`. Do not run `cargo fmt --all`. Keep `docs/src/` files' YAML frontmatter intact. Match the surrounding code's style (Tailwind classes, no semicolyon/format churn).
9. **One file focus per step.** If an Implement step would touch more than the files its task lists, STOP and re-read the task — you've drifted.
10. **Single-threaded.** Do the tasks in numeric order (Tasks 7 and 8 are the only `[PARALLEL-SAFE]` ones and may be done any time after Task 3). Never have two edits in flight on the same file.

**Handoff note format (use on any STOP):**
```
## HANDOFF — Task <N>, Step <S>
Expected: <what the task assumed>
Found: <actual output / error, pasted>
Tried: <what you attempted, if anything>
Blocked on: <the specific decision or fact you need>
```

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-gui/ui/src/lib/panelRegistry.tsx` | viewKey → panel metadata (`title`, `render`) SSOT, built from the nav registry; the single place that knows how to render a surface as a panel | Create (Task 1) |
| `crates/vox-gui/ui/src/lib/panelRegistry.test.tsx` | completeness test: every dockable viewKey resolves to a render | Create (Task 1) |
| `crates/vox-gui/ui/src/components/layout/DockWorkspace.tsx` | multi-panel dockview workspace; layout SSOT (serialize/restore); `openPanel(viewKey)`, `resetLayout()`, unknown-panel self-heal | Create (Task 2; generalizes `DockShell.tsx`) |
| `crates/vox-gui/ui/src/components/layout/DockWorkspace.test.tsx` | round-trip + open/dedupe + self-heal tests | Create (Task 2) |
| `crates/vox-gui/ui/src/components/layout/AppShell.tsx` | swap the single `DockShell` for `DockWorkspace`; pass `activeView` + `openPanel` ref | Modify (Task 3) |
| `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` | "open in new panel" affordance on nav items; drag a surface into the workspace | Modify (Task 4) |
| `crates/vox-gui/ui/src/lib/sidebarWidth.ts` | pure clamp + preset-snap helpers for the resizable sidebar | Create (Task 6) |
| `crates/vox-gui/ui/src/lib/sidebarWidth.test.ts` | clamp/snap unit tests | Create (Task 6) |
| `crates/vox-gui/ui/src/components/layout/SidebarResizer.tsx` | drag handle that writes the persisted free width | Create (Task 6) |
| `crates/vox-gui/ui/src/styles/dockview-vox.css` | theme dockview tabs/drop-zones/separators to dark/brass | Modify (Task 7) |
| `crates/vox-gui/ui/src/lib/shellPersistence.ts` | add `sidebarWidth` key to the SSOT | Modify (Task 6) |
| `docs/src/architecture/gui-layout-docking-model.md` | document the fixed-chrome/dockable-body decision + keybinds | Create (Task 8) |

**Pre-flight (run once from `crates/vox-gui/ui`, paste output before starting):**

```bash
# Confirm the dockview serialize/restore API + persistence the new code copies.
rg -n "DockviewReact|toJSON|fromJSON|addPanel|LAYOUT_PERSIST_DEBOUNCE_MS|getGuiPreference|setGuiPreference" src/components/layout/DockShell.tsx
# Confirm the surface renderer + props the panelRegistry reuses.
rg -n "export function renderSurfaceView|function childRenderer|case '|SurfaceProps" src/components/layout/surfaceComponents.tsx
# Confirm nav registry shape (viewKey/navLabel/parentSurface) used to build the panel list.
rg -n "viewKey:|navLabel:|parentSurface:" src/generated/surfaceRegistry.generated.ts | head
# Confirm sidebar width/mode model.
rg -n "SIDEBAR_WIDTHS|SidebarMode|SHELL_PREFERENCE_KEYS" src/components/layout/Sidebar.tsx src/lib/shellPersistence.ts
# Baselines green.
npx tsc --noEmit && npx vitest run
```

Expected: all of the above symbols exist; `tsc` exits 0; `vitest` reports all green (≈710 tests).

---

## Task 1 `[SEQUENTIAL]`: `panelRegistry` — viewKey → panel render SSOT

**Files:**
- Create: `crates/vox-gui/ui/src/lib/panelRegistry.tsx`
- Test: `crates/vox-gui/ui/src/lib/panelRegistry.test.tsx`

The registry reuses the **existing** `childRenderer` (which already knows how to render every surface from `SurfaceProps`) — it does not re-implement any surface. It maps a `viewKey` to a stable `title` + a `render(props)` thunk, and lists which viewKeys are dockable (every nav child with a label).

- [ ] **Step 1 (verify-before-use):** Paste:
  ```bash
  rg -n "export function renderSurfaceView|function childRenderer|export interface SurfaceProps" src/components/layout/surfaceComponents.tsx
  rg -n "labelForNavKey|NAV_LABELS" src/lib/navigation.ts
  ```
  Confirm `childRenderer(props, viewKey)` and `labelForNavKey(key)` exist with those signatures. If `childRenderer` is not exported, note it — Step 4 exports it.

- [ ] **Step 2: Export `childRenderer`.** In `src/components/layout/surfaceComponents.tsx`, change `function childRenderer(` to `export function childRenderer(`. (No behavior change; the registry needs it.)

- [ ] **Step 3: Write the failing test.** Create `src/lib/panelRegistry.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { DOCKABLE_VIEW_KEYS, panelTitle, resolvePanelView } from './panelRegistry';

describe('panelRegistry', () => {
  it('lists the primary surfaces as dockable', () => {
    // A representative spread across parents must be dockable.
    for (const key of ['dashboard', 'chat', 'memory', 'models', 'settings', 'activity']) {
      expect(DOCKABLE_VIEW_KEYS).toContain(key);
    }
  });

  it('gives every dockable view a non-empty human title', () => {
    for (const key of DOCKABLE_VIEW_KEYS) {
      expect(panelTitle(key).length).toBeGreaterThan(0);
    }
  });

  it('resolves a top-level parent key to a renderable child', () => {
    // `knowledge` is a parent with no childRenderer case — must resolve to a child.
    expect(resolvePanelView('knowledge')).not.toBe('knowledge');
    // a key that is already a child resolves to itself (idempotent).
    expect(resolvePanelView('memory')).toBe('memory');
  });
});
```

- [ ] **Step 4: Run → FAIL.** `npx vitest run src/lib/panelRegistry.test.tsx`
  Expected: FAIL — "Cannot find module './panelRegistry'".

- [ ] **Step 5: Implement.** Create `src/lib/panelRegistry.tsx`:

```tsx
import React from 'react';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { labelForNavKey, resolveNavigation } from './navigation';
import { childRenderer, type SurfaceProps } from '../components/layout/surfaceComponents';

/**
 * Dockable surfaces = every registry entry that has a viewKey AND a navLabel
 * (i.e. something a user can actually open). This is derived from the nav SSOT,
 * so adding a surface to the registry makes it dockable automatically.
 */
export const DOCKABLE_VIEW_KEYS: string[] = Array.from(
  new Set(
    SURFACE_REGISTRY
      .filter(e => e.viewKey && e.navLabel)
      .map(e => e.viewKey as string),
  ),
);

export function isDockable(viewKey: string): boolean {
  return DOCKABLE_VIEW_KEYS.includes(viewKey);
}

/**
 * Resolve a viewKey to the CHILD view that `childRenderer` actually renders.
 * Top-level parent keys (knowledge/agents/workspace/commands/compute) have no
 * childRenderer case — they must be mapped to their default child first.
 * `resolveNavigation` is idempotent for keys that are already children, so this
 * is safe to call on any viewKey.
 */
export function resolvePanelView(viewKey: string): string {
  return resolveNavigation(viewKey).child;
}

export function panelTitle(viewKey: string): string {
  return labelForNavKey(resolvePanelView(viewKey));
}

/**
 * Render a surface as a dock panel body. Reuses the EXISTING childRenderer so a
 * panel is pixel-identical to the surface rendered inline — no duplicate views.
 */
export function renderPanel(viewKey: string, props: SurfaceProps): React.ReactNode {
  return childRenderer(props, resolvePanelView(viewKey));
}
```

- [ ] **Step 6: Run → PASS.** `npx vitest run src/lib/panelRegistry.test.tsx` → PASS. Then `npx tsc --noEmit` → exit 0.

- [ ] **Step 7: Commit.**
  ```bash
  git add src/lib/panelRegistry.tsx src/lib/panelRegistry.test.tsx src/components/layout/surfaceComponents.tsx
  git commit -m "feat(gui): panelRegistry — viewKey→panel render SSOT over childRenderer"
  ```

---

## Task 2 `[SEQUENTIAL]`: `DockWorkspace` — multi-panel dockview + layout SSOT

**Files:**
- Create: `crates/vox-gui/ui/src/components/layout/DockWorkspace.tsx`
- Test: `crates/vox-gui/ui/src/components/layout/DockWorkspace.test.tsx`

Generalizes `DockShell`: instead of one fixed panel, it hosts a dockview where each panel's `params.viewKey` selects the renderer via `renderPanel`. It exposes an imperative `openPanel(viewKey)` (focus if already open, else add as a tab next to the active panel) and `resetLayout()`. Layout persists to `gui.layout.v1` (the existing `SHELL_PREFERENCE_KEYS.dockLayout`). Unknown viewKey on restore renders a removable placeholder (self-heal).

- [ ] **Step 1 (verify-before-use):** Re-read `DockShell.tsx` in full. Confirm: `onReady` gets `event.api`; layout via `api.toJSON()` / `api.fromJSON()`; persistence debounced by `LAYOUT_PERSIST_DEBOUNCE_MS`; panels added via `api.addPanel({ id, component, title, params })`. Paste the `onReady` + `persistLayout` blocks.

- [ ] **Step 2: Write the failing test.** Create `src/components/layout/DockWorkspace.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { panelIdForView, planOpen } from './DockWorkspace';

// Pure layout-intent helpers are unit-tested without mounting dockview.
describe('DockWorkspace helpers', () => {
  it('derives a stable panel id from a viewKey', () => {
    expect(panelIdForView('memory')).toBe('surface:memory');
    expect(panelIdForView('memory')).toBe(panelIdForView('memory'));
  });

  it('planOpen focuses an existing panel instead of duplicating', () => {
    const existing = new Set(['surface:memory']);
    expect(planOpen('memory', existing)).toEqual({ action: 'focus', id: 'surface:memory' });
  });

  it('planOpen adds a new panel when not present', () => {
    const existing = new Set(['surface:memory']);
    expect(planOpen('models', existing)).toEqual({ action: 'add', id: 'surface:models', viewKey: 'models' });
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npx vitest run src/components/layout/DockWorkspace.test.tsx`
  Expected: FAIL — "Cannot find module './DockWorkspace'".

- [ ] **Step 4: Implement.** Create `src/components/layout/DockWorkspace.tsx`:

```tsx
import React, { useCallback, useEffect, useImperativeHandle, useRef, forwardRef } from 'react';
import {
  DockviewReact,
  DockviewReadyEvent,
  IDockviewPanelProps,
  themeDark,
} from 'dockview';
import 'dockview/dist/styles/dockview.css';
import '../../styles/dockview-vox.css';
import { voxTransport } from '../../transport';
import { LAYOUT_PERSIST_DEBOUNCE_MS } from '../../config/constants';
import { SHELL_PREFERENCE_KEYS } from '../../lib/shellPersistence';
import { renderPanel, panelTitle, isDockable, resolvePanelView } from '../../lib/panelRegistry';
import type { SurfaceProps } from './surfaceComponents';

/**
 * Panel id is keyed by the RESOLVED child view, so opening a parent (`agents`)
 * and its default child (`dashboard`) target the same panel instead of two.
 */
export function panelIdForView(viewKey: string): string {
  return `surface:${resolvePanelView(viewKey)}`;
}

export type OpenPlan =
  | { action: 'focus'; id: string }
  | { action: 'add'; id: string; viewKey: string };

/** Pure decision: focus an open panel or add a new one. */
export function planOpen(viewKey: string, openIds: Set<string>): OpenPlan {
  const id = panelIdForView(viewKey);
  return openIds.has(id) ? { action: 'focus', id } : { action: 'add', id, viewKey };
}

export interface DockWorkspaceHandle {
  openPanel: (viewKey: string) => void;
  resetLayout: () => void;
}

interface DockWorkspaceProps {
  /** The currently-selected nav view; seeded as the first panel. */
  activeView: string;
  /** Shared surface props passed to every panel body (same object AppShell builds). */
  surfaceProps: SurfaceProps;
  layoutKey?: string;
}

type Api = DockviewReadyEvent['api'];

function PanelHost({ params }: IDockviewPanelProps<{ viewKey: string; surfaceProps: SurfaceProps }>) {
  const { viewKey, surfaceProps } = params;
  if (!isDockable(viewKey)) {
    return (
      <div className="flex h-full items-center justify-center p-6 text-sm text-zinc-500">
        Unknown panel “{viewKey}” — close it from the tab.
      </div>
    );
  }
  return <div className="h-full min-h-0 overflow-auto custom-scrollbar p-1">{renderPanel(viewKey, surfaceProps)}</div>;
}

const components = { panel: PanelHost };

export const DockWorkspace = forwardRef<DockWorkspaceHandle, DockWorkspaceProps>(function DockWorkspace(
  { activeView, surfaceProps, layoutKey = SHELL_PREFERENCE_KEYS.dockLayout },
  ref,
) {
  const apiRef = useRef<Api | null>(null);
  const persistTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Keep the freshest surfaceProps so live panels re-render with new data.
  const propsRef = useRef(surfaceProps);
  propsRef.current = surfaceProps;

  const persist = useCallback((api: Api) => {
    if (persistTimer.current) clearTimeout(persistTimer.current);
    persistTimer.current = setTimeout(() => {
      try {
        voxTransport.setGuiPreference(layoutKey, JSON.stringify(api.toJSON())).catch(() => {});
      } catch { /* ignore serialization errors */ }
    }, LAYOUT_PERSIST_DEBOUNCE_MS);
  }, [layoutKey]);

  const addSurfacePanel = useCallback((api: Api, viewKey: string, setActive: boolean) => {
    api.addPanel({
      id: panelIdForView(viewKey),
      component: 'panel',
      title: panelTitle(viewKey),
      params: { viewKey, surfaceProps: propsRef.current },
      inactive: !setActive,
    });
  }, []);

  const openPanel = useCallback((viewKey: string) => {
    const api = apiRef.current;
    if (!api) return;
    const openIds = new Set(api.panels.map(p => p.id));
    const plan = planOpen(viewKey, openIds);
    if (plan.action === 'focus') {
      api.getPanel(plan.id)?.api.setActive();
    } else {
      addSurfacePanel(api, plan.viewKey, true);
    }
  }, [addSurfacePanel]);

  const resetLayout = useCallback(() => {
    const api = apiRef.current;
    if (!api) return;
    api.clear();
    addSurfacePanel(api, activeView, true);
  }, [activeView, addSurfacePanel]);

  useImperativeHandle(ref, () => ({ openPanel, resetLayout }), [openPanel, resetLayout]);

  const onReady = useCallback((event: DockviewReadyEvent) => {
    apiRef.current = event.api;
    voxTransport.getGuiPreference(layoutKey)
      .then(raw => {
        let restored = false;
        if (raw) {
          try { event.api.fromJSON(JSON.parse(raw)); restored = true; } catch { /* fall through */ }
        }
        if (!restored || event.api.panels.length === 0) {
          addSurfacePanel(event.api, activeView, true);
        }
      })
      .catch(() => addSurfacePanel(event.api, activeView, true));
    event.api.onDidLayoutChange(() => persist(event.api));
  // activeView intentionally read once at mount; live switches go through the effect below.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layoutKey, addSurfacePanel, persist]);

  // When the nav selection changes, open/focus that surface as a panel.
  useEffect(() => {
    if (apiRef.current) openPanel(activeView);
  }, [activeView, openPanel]);

  // Push fresh surfaceProps into every live panel so data stays current.
  useEffect(() => {
    const api = apiRef.current;
    if (!api) return;
    for (const p of api.panels) {
      const vk = (p.params as { viewKey?: string } | undefined)?.viewKey;
      if (vk) p.api.updateParameters({ viewKey: vk, surfaceProps });
    }
  }, [surfaceProps]);

  return (
    <DockviewReact
      className="dockview-theme-vox h-full min-h-0"
      onReady={onReady}
      components={components}
      theme={themeDark}
    />
  );
});
```

- [ ] **Step 5: Run → PASS.** `npx vitest run src/components/layout/DockWorkspace.test.tsx` → PASS. Then `npx tsc --noEmit` → exit 0.

- [ ] **Step 6: Commit.**
  ```bash
  git add src/components/layout/DockWorkspace.tsx src/components/layout/DockWorkspace.test.tsx
  git commit -m "feat(gui): DockWorkspace — multi-panel dockview with layout SSOT + openPanel"
  ```

---

## Task 3 `[SEQUENTIAL]`: wire `DockWorkspace` into `AppShell`

**Files:** Modify `crates/vox-gui/ui/src/components/layout/AppShell.tsx`.

Replace the single `DockShell` with `DockWorkspace`, seeding the active surface and routing nav changes into it. `AppShell` already receives the rendered surface as `children`; `DockWorkspace` needs `activeView` + the `surfaceProps`. The cleanest minimal change keeps `App.tsx` building `surfaceProps` (it already does) and passes them through.

- [ ] **Step 1 (verify-before-use):** Paste:
  ```bash
  rg -n "DockShell|children|surfaceKey|surfaceLabel|activeView" src/components/layout/AppShell.tsx
  rg -n "renderSurfaceView|surfaceProps|<AppShell" src/App.tsx
  ```
  Confirm `App.tsx` builds a `surfaceProps` object and calls `renderSurfaceView(nav.parent, surfaceProps)` into `children`. Confirm `AppShell` receives `activeView`.

- [ ] **Step 2: Thread `surfaceProps` + a workspace ref through `App.tsx`.** In `src/App.tsx`, after `const mainSurface = renderSurfaceView(nav.parent, surfaceProps);`, also pass `surfaceProps` and `activeView` to `AppShell` (add props `workspaceProps={surfaceProps}` and keep `activeView`). Add to the `<AppShell ... >` call:
  ```tsx
  workspaceProps={surfaceProps}
  ```

- [ ] **Step 3a (verify-before-use, BLOCKING):** Paste the top ~40 lines of `src/components/layout/AppShell.test.tsx` (the imports, the `vi.mock` calls, and however it builds props for `render(<AppShell .../>)`). Identify the exact props variable/object the existing tests pass (it may be a `baseProps` const, an inline object, or a helper). You will REUSE that exact construct verbatim in Step 3b — do not invent a new props object.

- [ ] **Step 3b: Write the failing test.** Add a mock of `DockWorkspace` and one test that asserts the content region mounts it with the active view. Use the SAME props construct you found in Step 3a (shown here as `<existing props>` — substitute the real one), adding `activeView="memory"` and the new required `workspaceProps`:

```tsx
// add near the other vi.mock calls at the top of AppShell.test.tsx
vi.mock('./DockWorkspace', () => ({
  DockWorkspace: (props: { activeView: string }) => (
    <div data-testid="dock-workspace" data-active={props.activeView} />
  ),
}));

// add inside describe('AppShell', ...). `<existing props>` = the exact construct
// from Step 3a; `workspaceProps={{} as never}` satisfies the new required prop
// because DockWorkspace is mocked and never reads it.
it('mounts the dock workspace for the active view', () => {
  render(<AppShell <existing props> activeView="memory" workspaceProps={{} as never} />);
  const ws = screen.getByTestId('dock-workspace');
  expect(ws.getAttribute('data-active')).toBe('memory');
});
```

- [ ] **Step 4: Run → FAIL.** `npx vitest run src/components/layout/AppShell.test.tsx`
  Expected: FAIL — no `dock-workspace` testid (still rendering `DockShell`).

- [ ] **Step 5: Implement.** In `AppShell.tsx`:
  1. Replace `import { DockShell } from './DockShell';` with `import { DockWorkspace } from './DockWorkspace';` and `import type { SurfaceProps } from './surfaceComponents';`.
  2. Add to `AppShellProps`: `workspaceProps: SurfaceProps;`.
  3. Replace the content block
     ```tsx
     <DockShell panelId="main-surface" panelTitle={surfaceLabel}>
       {children}
     </DockShell>
     ```
     with
     ```tsx
     <DockWorkspace activeView={activeView} surfaceProps={workspaceProps} />
     ```
  4. `children` may now be unused for the main region — keep the prop (chat dock etc. still use it) but stop rendering it inside the dock area.

- [ ] **Step 6: Run → PASS.** `npx vitest run src/components/layout/AppShell.test.tsx` → PASS. Then `npx tsc --noEmit` → 0 and `npx vitest run` → all green.

- [ ] **Step 7: Manual smoke (paste result).** `npx vite` (or the running `cargo tauri dev`); open the app, switch between Memory / Models / Settings — each opens/focuses as a panel; drag a panel tab to split the area; reload — layout restored. Confirm no console errors.

- [ ] **Step 8: Commit.**
  ```bash
  git add src/components/layout/AppShell.tsx src/components/layout/AppShell.test.tsx src/App.tsx
  git commit -m "feat(gui): host surfaces in DockWorkspace (multi-panel content region)"
  ```

---

## Task 4 `[SEQUENTIAL]`: "open in new panel" + drag a surface from the sidebar

**Files:** Modify `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`; Modify `AppShell.tsx` + `App.tsx` to pass an `onOpenPanel(viewKey)` callback wired to the workspace ref.

Power-user affordance: middle-click / a small "+" on a nav item, or dragging the nav item, opens that surface as an *additional* panel rather than replacing the current one.

- [ ] **Step 1 (verify-before-use):** Paste `rg -n "NavItem|setView|onClick=\{\(\) => setView" src/components/layout/Sidebar.tsx`. Confirm each nav item calls `setView(key)`.

- [ ] **Step 2: Lift a workspace ref in `App.tsx`.** Create `const dockRef = useRef<DockWorkspaceHandle>(null);` (import the type). Pass `dockRef` to `AppShell` as `workspaceRef={dockRef}`, and pass `onOpenPanel={(vk: string) => dockRef.current?.openPanel(vk)}` to `AppShell`. In `AppShell`, forward `workspaceRef` to `<DockWorkspace ref={workspaceRef} ... />` and `onOpenPanel` to `<Sidebar onOpenPanel={onOpenPanel} ... />`.

- [ ] **Step 3: Write the failing test.** In a new `src/components/layout/Sidebar.openPanel.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { Sidebar } from './Sidebar';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn().mockResolvedValue({ display_name: 'x@vox', os_user: 'x' }) }));

const base = {
  view: 'dashboard', setView: vi.fn(), agentsCount: 0,
  data: { agents: [], stream: [], alerts: [], peers: [], skills: [], kpis: {} } as any,
  mode: 'default' as const, setMode: vi.fn(), pushToast: vi.fn(),
};

describe('Sidebar open-in-panel', () => {
  it('middle-click on a nav item opens it as a panel instead of navigating', () => {
    const onOpenPanel = vi.fn();
    render(<Sidebar {...base} onOpenPanel={onOpenPanel} />);
    const agents = screen.getByRole('button', { name: /agents/i });
    fireEvent.mouseDown(agents, { button: 1 }); // middle button
    expect(onOpenPanel).toHaveBeenCalledWith('agents');
    expect(base.setView).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 4: Run → FAIL.** `npx vitest run src/components/layout/Sidebar.openPanel.test.tsx`
  Expected: FAIL — `onOpenPanel` prop not handled.

- [ ] **Step 5: Implement.** In `Sidebar.tsx`:
  1. Add `onOpenPanel?: (viewKey: string) => void;` to `SidebarProps`.
  2. On each top-level `NavItem` and child-tab button, add an `onMouseDown` that, when `e.button === 1` (middle), calls `e.preventDefault(); onOpenPanel?.(key)` and returns. Left-click keeps calling `setView(key)`.
  3. (Optional, same task) add a tiny “⊞” button shown on hover of an expanded nav item that calls `onOpenPanel?.(key)` for discoverability without a middle mouse button.

- [ ] **Step 6: Run → PASS.** test green; `npx tsc --noEmit` → 0; `npx vitest run` → all green.

- [ ] **Step 7: Commit.**
  ```bash
  git add src/components/layout/Sidebar.tsx src/components/layout/Sidebar.openPanel.test.tsx src/components/layout/AppShell.tsx src/App.tsx
  git commit -m "feat(gui): open a surface as an additional workspace panel (middle-click / ⊞)"
  ```

---

## Task 5 `[SEQUENTIAL]`: "Reset layout" control

**Files:** Modify `crates/vox-gui/ui/src/components/layout/TopHud.tsx` (or `BreadcrumbBar.tsx` — whichever hosts content-region controls); Modify `AppShell.tsx`/`App.tsx` to pass `onResetLayout`.

- [ ] **Step 1 (verify-before-use):** `rg -n "BreadcrumbBar|TopHud|onReset|reset" src/components/layout/AppShell.tsx` — pick the control bar already next to the content region; paste a few lines so the new button matches its styling.

- [ ] **Step 2: Write the failing test.** In the chosen bar's test file, assert a "Reset layout" button calls the passed `onResetLayout`.

```tsx
it('reset layout button invokes onResetLayout', () => {
  const onResetLayout = vi.fn();
  render(<BreadcrumbBar viewKey="dashboard" onNavigate={vi.fn()} onResetLayout={onResetLayout} />);
  fireEvent.click(screen.getByRole('button', { name: /reset layout/i }));
  expect(onResetLayout).toHaveBeenCalled();
});
```

- [ ] **Step 3: Run → FAIL.** `npx vitest run <that test file>` → FAIL.

- [ ] **Step 4: Implement.** Add `onResetLayout?: () => void` to the bar's props; render a small ghost button `Reset layout` (only when the prop is provided) styled like the bar's other actions. In `App.tsx`, pass `onResetLayout={() => dockRef.current?.resetLayout()}` down through `AppShell`.

- [ ] **Step 5: Run → PASS.** test green; `npx tsc --noEmit` → 0; `npx vitest run` → all green.

- [ ] **Step 6: Commit.**
  ```bash
  git commit -am "feat(gui): reset-layout control for the dock workspace"
  ```

---

## Task 6 `[SEQUENTIAL]`: drag-resizable sidebar (free width + preset snap)

**Files:**
- Create: `crates/vox-gui/ui/src/lib/sidebarWidth.ts` (+ `.test.ts`)
- Create: `crates/vox-gui/ui/src/components/layout/SidebarResizer.tsx`
- Modify: `crates/vox-gui/ui/src/lib/shellPersistence.ts` (add key)
- Modify: `crates/vox-gui/ui/src/components/layout/Sidebar.tsx` (apply free width + handle)

- [ ] **Step 1 (verify-before-use):** Paste `rg -n "SIDEBAR_WIDTHS|const w = SIDEBAR_WIDTHS|style=\{\{ width" src/components/layout/Sidebar.tsx`. Confirm the aside width comes from `SIDEBAR_WIDTHS[mode]` applied as `style={{ width: w }}`.

- [ ] **Step 2: Add the persistence key.** In `src/lib/shellPersistence.ts`, add to `SHELL_PREFERENCE_KEYS`: `sidebarWidth: 'vox_sidebar_width',`.

- [ ] **Step 3: Write the failing test.** Create `src/lib/sidebarWidth.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { clampSidebarWidth, snapToPreset, SIDEBAR_MIN, SIDEBAR_MAX } from './sidebarWidth';

describe('sidebar width', () => {
  it('clamps to [min,max]', () => {
    expect(clampSidebarWidth(10)).toBe(SIDEBAR_MIN);
    expect(clampSidebarWidth(9999)).toBe(SIDEBAR_MAX);
    expect(clampSidebarWidth(240)).toBe(240);
  });

  it('snaps to a preset within tolerance, else keeps exact width', () => {
    expect(snapToPreset(210)).toBe(212);   // near "default" (212) → snap
    expect(snapToPreset(250)).toBe(250);   // outside tolerance → exact
    expect(snapToPreset(66)).toBe(64);     // near "rail" (64) → snap
  });
});
```

- [ ] **Step 4: Run → FAIL.** `npx vitest run src/lib/sidebarWidth.test.ts` → FAIL (module missing).

- [ ] **Step 5: Implement helper.** Create `src/lib/sidebarWidth.ts`:

```ts
// Continuous sidebar width with snap-to-preset for tidy default layouts.
export const SIDEBAR_MIN = 64;   // == rail
export const SIDEBAR_MAX = 420;
export const SIDEBAR_PRESETS = [64, 212, 280]; // rail / default / wide
const SNAP_TOLERANCE = 12;

export function clampSidebarWidth(px: number): number {
  if (Number.isNaN(px)) return 212;
  return Math.max(SIDEBAR_MIN, Math.min(SIDEBAR_MAX, Math.round(px)));
}

export function snapToPreset(px: number): number {
  const w = clampSidebarWidth(px);
  for (const preset of SIDEBAR_PRESETS) {
    if (Math.abs(w - preset) <= SNAP_TOLERANCE) return preset;
  }
  return w;
}
```

- [ ] **Step 6: Run → PASS.** `npx vitest run src/lib/sidebarWidth.test.ts` → PASS.

- [ ] **Step 7: Implement the resizer component.** Create `src/components/layout/SidebarResizer.tsx`:

```tsx
import React, { useCallback, useEffect, useRef } from 'react';
import { clampSidebarWidth, snapToPreset } from '../../lib/sidebarWidth';

interface SidebarResizerProps {
  /** Called continuously while dragging (clamped px). */
  onResize: (px: number) => void;
  /** Called once on release (snapped px) — the value to persist. */
  onCommit: (px: number) => void;
  /** Reset to the default preset on double-click. */
  onReset: () => void;
}

export function SidebarResizer({ onResize, onCommit, onReset }: SidebarResizerProps) {
  const dragging = useRef(false);
  const latest = useRef(212);

  const onPointerMove = useCallback((e: PointerEvent) => {
    if (!dragging.current) return;
    const px = clampSidebarWidth(e.clientX);
    latest.current = px;
    onResize(px);
  }, [onResize]);

  const stop = useCallback(() => {
    if (!dragging.current) return;
    dragging.current = false;
    document.body.style.cursor = '';
    onCommit(snapToPreset(latest.current));
  }, [onCommit]);

  useEffect(() => {
    window.addEventListener('pointermove', onPointerMove);
    window.addEventListener('pointerup', stop);
    return () => {
      window.removeEventListener('pointermove', onPointerMove);
      window.removeEventListener('pointerup', stop);
    };
  }, [onPointerMove, stop]);

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize sidebar"
      onPointerDown={() => { dragging.current = true; document.body.style.cursor = 'col-resize'; }}
      onDoubleClick={onReset}
      className="absolute top-0 right-0 z-20 h-full w-1.5 -mr-0.5 cursor-col-resize bg-transparent hover:bg-brass/30 transition-colors"
    />
  );
}
```

- [ ] **Step 8: Wire into `Sidebar.tsx`.** 
  1. Add `const [width, setWidth] = useLocalStorage<number>(SHELL_PREFERENCE_KEYS.sidebarWidth, SIDEBAR_WIDTHS[mode]);` and a transient `dragWidth` state.
  2. Compute the effective width: when `mode === 'rail'` keep the rail width (collapsed); otherwise use `dragWidth ?? width`.
  3. Apply it: `style={{ width: collapsed ? SIDEBAR_WIDTHS.rail : (dragWidth ?? width) }}`.
  4. Inside the `<aside>` (which is already `relative`-positioned in this codebase — verify; if not, add `relative`), render `{!collapsed && <SidebarResizer onResize={setDragWidth} onCommit={(px) => { setWidth(px); setDragWidth(null); }} onReset={() => { setWidth(SIDEBAR_WIDTHS.default); }} />}`.
  5. Keep the rail/default/wide cycle buttons working — they set `mode`; when a non-rail mode is chosen, also `setWidth(SIDEBAR_WIDTHS[mode])` so presets and free-drag stay consistent.

- [ ] **Step 9: Write the component test.** Create `src/components/layout/SidebarResizer.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import { SidebarResizer } from './SidebarResizer';

describe('SidebarResizer', () => {
  it('commits a snapped width on pointer up after drag', () => {
    const onCommit = vi.fn();
    render(<SidebarResizer onResize={() => {}} onCommit={onCommit} onReset={() => {}} />);
    const handle = screen.getByRole('separator', { name: /resize sidebar/i });
    fireEvent.pointerDown(handle);
    fireEvent.pointerMove(window, { clientX: 210 });
    fireEvent.pointerUp(window);
    expect(onCommit).toHaveBeenCalledWith(212); // 210 snaps to default preset
  });

  it('double-click resets', () => {
    const onReset = vi.fn();
    render(<SidebarResizer onResize={() => {}} onCommit={() => {}} onReset={onReset} />);
    fireEvent.doubleClick(screen.getByRole('separator', { name: /resize sidebar/i }));
    expect(onReset).toHaveBeenCalled();
  });
});
```

- [ ] **Step 10: Run → PASS.** `npx vitest run src/components/layout/SidebarResizer.test.tsx src/lib/sidebarWidth.test.ts` → PASS; `npx tsc --noEmit` → 0; full `npx vitest run` → green.

- [ ] **Step 11: Manual smoke (paste result):** drag the sidebar edge — width follows the cursor, releases with a snap near presets, persists across reload; double-click resets; rail toggle still collapses.

- [ ] **Step 12: Commit.**
  ```bash
  git add src/lib/sidebarWidth.ts src/lib/sidebarWidth.test.ts src/components/layout/SidebarResizer.tsx src/components/layout/SidebarResizer.test.tsx src/components/layout/Sidebar.tsx src/lib/shellPersistence.ts
  git commit -m "feat(gui): drag-resizable sidebar with preset snap (persisted width)"
  ```

---

## Task 7 `[PARALLEL-SAFE]`: theme the dockview chrome to dark/brass

**Files:** Modify `crates/vox-gui/ui/src/styles/dockview-vox.css`.

- [ ] **Step 1 (verify-before-use):** Paste `sed -n '1,60p' src/styles/dockview-vox.css` and `rg -n "dockview-theme-vox|--dv-" src/styles/dockview-vox.css`. Confirm the file exists and which dockview CSS variables (`--dv-*`) it already sets.

- [ ] **Step 2: Write the failing test.** Create `src/styles/dockview-vox.theme.test.ts` (a string assertion test — no DOM):

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

const css = readFileSync(fileURLToPath(new URL('./dockview-vox.css', import.meta.url)), 'utf8');

describe('dockview-vox theme', () => {
  it('sets the active tab and drop-target to the brass accent token', () => {
    expect(css).toMatch(/--dv-activegroup-visiblepanel-tab-background-color/);
    expect(css).toMatch(/var\(--brass\)|rgb\(var\(--brass\)/);
  });
});
```

- [ ] **Step 3: Run → FAIL.** `npx vitest run src/styles/dockview-vox.theme.test.ts` → FAIL (tokens not yet present).

- [ ] **Step 4: Implement.** In `dockview-vox.css`, under `.dockview-theme-vox`, set the dark palette + brass accents, e.g.:

```css
.dockview-theme-vox {
  --dv-background-color: transparent;
  --dv-group-view-background-color: rgba(255,255,255,0.02);
  --dv-tabs-and-actions-container-background-color: transparent;
  --dv-activegroup-visiblepanel-tab-background-color: rgba(255,255,255,0.05);
  --dv-activegroup-visiblepanel-tab-color: #f4f4f5;
  --dv-inactivegroup-visiblepanel-tab-color: #a1a1aa;
  --dv-separator-border: rgba(255,255,255,0.06);
  --dv-paneview-active-outline-color: rgb(var(--brass));
  --dv-drag-over-background-color: rgb(var(--brass) / 0.12);
  --dv-drag-over-border-color: rgb(var(--brass) / 0.5);
  --dv-tab-divider-color: rgba(255,255,255,0.06);
}
```
(Adjust variable names to those dockview 6.6.1 actually exposes — confirmed in Step 1. Keep the active-tab + drag-over tokens, which the test asserts.)

- [ ] **Step 5: Run → PASS.** `npx vitest run src/styles/dockview-vox.theme.test.ts` → PASS; `npx vite build` → clean.

- [ ] **Step 6: Manual smoke (paste result):** drag a panel — drop zones glow brass, tabs match the dark theme, separators are subtle.

- [ ] **Step 7: Commit.**
  ```bash
  git add src/styles/dockview-vox.css src/styles/dockview-vox.theme.test.ts
  git commit -m "style(gui): theme dockview tabs + drop zones to dark/brass"
  ```

---

## Task 8 `[PARALLEL-SAFE]`: document the layout model + retire dead `DockShell`

**Files:** Create `docs/src/architecture/gui-layout-docking-model.md`; delete `DockShell.tsx` + its test if now unreferenced.

- [ ] **Step 1 (verify-before-use):** Paste `rg -n "DockShell" src/ ` (whole `crates/vox-gui/ui/src`). If `DockShell` is referenced only by its own test after Task 3, it is dead.

- [ ] **Step 2: Delete the dead shell (if unreferenced).** `git rm src/components/layout/DockShell.tsx src/components/layout/DockShell.test.tsx`. Run `npx tsc --noEmit` → 0 and `npx vitest run` → green to prove nothing depended on it. (If still referenced, skip the delete and note why.)

- [ ] **Step 3: Write the doc.** Create `docs/src/architecture/gui-layout-docking-model.md` with frontmatter:

```markdown
---
title: "GUI Layout & Docking Model"
category: "Architecture SSOTs"
---

# GUI Layout & Docking Model

## Decision: fixed chrome, dockable body
- **Sidebar** (primary navigation) and **TopHud** (global status + command) are fixed chrome — always present, never floated. This preserves spatial stability and discoverability of navigation (VS Code activity bar / Photoshop menubar pattern).
- The **content region** is a `dockview` workspace (`DockWorkspace.tsx`): any surface opens as a panel that can be split, tabbed, floated, resized, and closed.

## SSOTs
- **Dockable surfaces:** derived from `SURFACE_REGISTRY` (any entry with `viewKey` + `navLabel`) via `lib/panelRegistry.tsx`.
- **Layout:** persisted dockview JSON at `SHELL_PREFERENCE_KEYS.dockLayout` (`gui.layout.v1`).
- **Sidebar width:** `SHELL_PREFERENCE_KEYS.sidebarWidth` (`vox_sidebar_width`), continuous with snap-to-preset (`lib/sidebarWidth.ts`).

## Interactions
- Left-click a nav item → navigate (replaces/focuses the active panel).
- Middle-click or ⊞ a nav item → open the surface as an additional panel.
- Drag a panel tab → split / tab / float.
- Drag the sidebar edge → resize (double-click handle resets); rail/default/wide presets remain.
- Reset layout → control in the content control bar.
- Keybinds: ⌘\ split active panel, ⌘W close active panel, ⌘B cycle sidebar, ⌘⇧H cycle HUD.

## Why the top bar is not draggable
Floating the global command/status bar is unconventional and costs orientation for little gain. Its existing full/slim/hidden modes (⌘⇧H) already cover density needs.
```

- [ ] **Step 4: Commit.**
  ```bash
  git add docs/src/architecture/gui-layout-docking-model.md
  git rm --cached -q src/components/layout/DockShell.tsx src/components/layout/DockShell.test.tsx 2>/dev/null || true
  git commit -m "docs(gui): document layout/docking model; retire dead DockShell"
  ```

---

## Task 9 `[SEQUENTIAL]`: full verification gate

- [ ] **Step 1:** From `crates/vox-gui/ui`: `npx tsc --noEmit` → exit 0.
- [ ] **Step 2:** `npx vitest run` → all green (baseline ≈710 + new tests).
- [ ] **Step 3:** `npx vite build` → clean (no type/bundle errors).
- [ ] **Step 4:** Launch `cargo tauri dev`; smoke the acceptance checklist below; paste console (must be error-free).
- [ ] **Step 5:** Commit any final fixes; the branch is ready for review.

**Acceptance checklist (manual):**
1. Switching nav opens/focuses each surface as a panel.
2. A panel can be split, tabbed, floated, resized, and closed.
3. Layout persists across reload; "Reset layout" restores the active surface only.
4. An unknown persisted panel shows a removable placeholder (no white screen) — test by hand-editing the stored `gui.layout.v1` to include a bogus `viewKey`.
5. Sidebar resizes by dragging its edge, snaps near presets, persists, double-click resets; rail toggle still collapses.
6. Dockview tabs/drop-zones match the dark/brass theme; scrollbars are the global thin themed ones.
7. Top bar remains fixed; ⌘⇧H still cycles its density.

---

## Self-Review

**Spec coverage:**
- "Drag/dock content surfaces (Photoshop-style)" → Tasks 1–5 (panelRegistry, DockWorkspace, AppShell wiring, open-in-panel, reset).
- "Sidebar click-and-drag" → Task 6 (resizable sidebar).
- "Top bar" → Task 8 documents the deliberate fixed-chrome decision (with the existing HUD collapse modes covering density). This is the recommended GUI-principled answer rather than floating the top bar.
- "Match dark theme" → Task 7 (dockview theming) + the already-landed global thin scrollbars.
- "Based on existing code" → every task reuses `dockview`, `childRenderer`, `SURFACE_REGISTRY`, `voxTransport` prefs, `SHELL_PREFERENCE_KEYS`, `useLocalStorage`.

**Placeholder scan:** no TBD/“handle edge cases”/uncited code — each code step shows the full code; the only adjust-to-reality note (dockview `--dv-*` variable names in Task 7) is gated by a Step-1 verify and a string test.

**Type consistency:** `panelIdForView`/`planOpen`/`DockWorkspaceHandle`/`renderPanel`/`panelTitle`/`isDockable`/`resolvePanelView`/`clampSidebarWidth`/`snapToPreset`/`SHELL_PREFERENCE_KEYS.sidebarWidth` are defined once and referenced with identical signatures across tasks. `SurfaceProps` is imported from `surfaceComponents.tsx` everywhere.

**Codebase audit (done while writing):**
- dockview API used here (`clear()`, `addPanel({inactive})`, `getPanel`, `panels`, `activePanel`, `panel.api.setActive()`, `toJSON`/`fromJSON`) was verified against the installed `dockview-core@6.6.1` type defs — all present.
- **Bug caught + fixed in plan:** rendering a panel by a top-level parent key (`knowledge`/`agents`/`workspace`/`commands`/`compute`) would hit `childRenderer`'s `default → null` (blank panel). Resolved by routing every id/title/render through `resolvePanelView` = `resolveNavigation(viewKey).child` (idempotent for child keys). Task 1's test asserts this.
- Flash-handoff hardening added: execution-target note + Operating Rules (verify-gates, two-strike breaker, verification ritual, handoff-note format).

**Known follow-ups (out of scope, noted not silently dropped):** context-window + memory panels live in `2026-06-19-dockable-workspace-context-memory-ssot.md`; the dnd-kit dashboard grid is left intact (the dashboard is itself a dockable panel); a stale `telemetry` entry remains in `SHELL_PREFERENCE_KEYS` (harmless; remove opportunistically).
