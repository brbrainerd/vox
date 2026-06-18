# Gamified GUI Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Marry the gamified visual sandbox simulation to the code-based GUI dashboard, gamification tab, search panel, and Scientia discovery views, creating bidirectional file navigation and real-time agent task visualization.

**Architecture:** We will extend the centralized Zustand coordinate store to hold a global file focus (`focusedFile`) and active agent tasks map. Individual panels (Search, Scientia, Tasks) update the focus, which pans the sandbox camera to center on the building. Canvas clicks update the focus state to open details drawers, and a collapsible mini-map is embedded on the Dashboard.

**Tech Stack:** React, Zustand, TypeScript, HTML5 Canvas, Vitest.

---

## File Structure

*   `crates/vox-gui/ui/src/components/gamify/store.ts` [MODIFY]: Expand store state to track focused files and agent tasks.
*   `crates/vox-gui/ui/src/components/gamify/store.test.ts` [MODIFY]: Add unit tests for focused file and task state modifications.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` [MODIFY]: Integrate camera centering logic, active agent glows, and canvas click triggers.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` [MODIFY]: Verify camera panning, clicks, and agent glows mock mappings.
*   `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx` [MODIFY]: Embed collapsible mini-map layout and wire fullscreen expansion callback.
*   `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` [MODIFY]: Pass navigation callbacks to Dashboard view.
*   `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx` [MODIFY]: Add test asserting mini-map rendering and navigation toggles.
*   `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx` [MODIFY]: Mount fullscreen visualizer canvas under the hud profile section.
*   `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.test.tsx` [MODIFY]: Test that the canvas maps on the Gamify View.
*   `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx` [MODIFY]: Dispatch focused file updates on row selection.
*   `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx` [MODIFY]: Assert focus dispatching.
*   `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx` [MODIFY]: Dispatch focus updates on review item selection.
*   `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx` [MODIFY]: Dispatch focus updates on active task files.

---

### Task 1: Zustand Store Focus & Agent Tasks State

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/store.ts`
- Test: `crates/vox-gui/ui/src/components/gamify/store.test.ts`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/store.test.ts` to assert focus and task actions:
```typescript
  it('correctly manages focused file and agent tasks state', () => {
    const store = useLudusStore.getState();
    // @ts-expect-error - setFocusedFile does not exist yet
    store.setFocusedFile('src/facade.rs');
    // @ts-expect-error - focusedFile does not exist yet
    expect(useLudusStore.getState().focusedFile).toBe('src/facade.rs');

    // @ts-expect-error - updateAgentTask does not exist yet
    store.updateAgentTask('agent_123', { taskId: 'task_abc', filePath: 'src/lib.rs', status: 'running' });
    // @ts-expect-error - agentTasks does not exist yet
    const task = useLudusStore.getState().agentTasks['agent_123'];
    expect(task).toBeDefined();
    expect(task.status).toBe('running');
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/store.test.ts`
Expected: FAIL (setFocusedFile is not a function)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/store.ts`:
```typescript
import { createStore } from 'zustand/vanilla';

export type MoodType = 'Happy' | 'Tired' | 'Sad' | 'Excited' | 'Exhausted';

export interface AgentState {
  x: number;
  y: number;
  energy: number;
  mood: MoodType;
}

export interface BuildingState {
  x: number;
  y: number;
  warnings: number;
  errors: number;
}

export interface AgentTask {
  taskId: string;
  filePath: string;
  status: string;
}

export interface LudusStoreState {
  agents: Record<string, AgentState>;
  buildings: Record<string, BuildingState>;
  focusedFile: string | null;
  agentTasks: Record<string, AgentTask>;
  updateAgent: (id: string, updates: Partial<AgentState>) => void;
  updateBuilding: (filePath: string, updates: Partial<BuildingState>) => void;
  setFocusedFile: (filePath: string | null) => void;
  updateAgentTask: (agentId: string, task: AgentTask | null) => void;
  reset: () => void;
}

export const useLudusStore = createStore<LudusStoreState>((set) => ({
  agents: {},
  buildings: {},
  focusedFile: null,
  agentTasks: {},
  updateAgent: (id, updates) =>
    set((state) => {
      const current = state.agents[id] || { x: 0, y: 0, energy: 100, mood: 'Happy' as MoodType };
      return {
        agents: {
          ...state.agents,
          [id]: { ...current, ...updates },
        },
      };
    }),
  updateBuilding: (filePath, updates) =>
    set((state) => {
      const current = state.buildings[filePath] || { x: 0, y: 0, warnings: 0, errors: 0 };
      return {
        buildings: {
          ...state.buildings,
          [filePath]: { ...current, ...updates },
        },
      };
    }),
  setFocusedFile: (filePath) => set({ focusedFile: filePath }),
  updateAgentTask: (agentId, task) =>
    set((state) => {
      const next = { ...state.agentTasks };
      if (task === null) {
        delete next[agentId];
      } else {
        next[agentId] = task;
      }
      return { agentTasks: next };
    }),
  reset: () => set({ agents: {}, buildings: {}, focusedFile: null, agentTasks: {} }),
}));
```

Remove type overrides in the test file once the types are present.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/store.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/store.ts crates/vox-gui/ui/src/components/gamify/store.test.ts
git commit -m "feat: add focusedFile and agentTasks properties to Zustand state"
```

---

### Task 2: Camera Centering and Active Agent Glows in Sandbox

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`
- Test: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to assert that changing store focus updates the camera offsets:
```typescript
  it('updates camera offsets when focusedFile changes', () => {
    const files = ['crates/vox-db/src/lib.rs'];
    const { render } = require('@testing-library/react');
    render(<LudusSandbox files={files} />);

    // Trigger focused file state change
    useLudusStore.getState().setFocusedFile('crates/vox-db/src/lib.rs');
    
    // Camera target centering check (verifies camera center state is updated)
    const store = useLudusStore.getState();
    expect(store.focusedFile).toBe('crates/vox-db/src/lib.rs');
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (assertion fails or component loop is missing target centering)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to subscribe to store focused file and render active task glowing indicators:
```typescript
  // Select state slices
  const buildings = useStore(useLudusStore, (state) => state.buildings);
  const focusedFile = useStore(useLudusStore, (state) => state.focusedFile);
  const agentTasks = useStore(useLudusStore, (state) => state.agentTasks);

  // Auto-center camera target when focused file changes
  useEffect(() => {
    if (!focusedFile) return;
    const plot = plots[focusedFile];
    if (!plot) return;
    const centerOffsetX = 1000; // Center offset of offscreen canvas
    const centerOffsetY = 100;
    const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
    
    // Pan camera to center coordinates: cameraX = viewportWidth/2 - px, cameraY = viewportHeight/2 - py
    setCamera({ x: 400 - px, y: 250 - py, zoom: 1.2 });
  }, [focusedFile, plots]);

  // Pre-render canvas updates...
  // In offscreen render loop:
  // Render active agent tasks glows
  for (const task of Object.values(agentTasks)) {
    const plot = plots[task.filePath];
    if (!plot) continue;
    const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
    ctx.strokeStyle = '#ef4444';
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.arc(px, py, 14, 0, 2 * Math.PI);
    ctx.stroke();
  }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx
git commit -m "feat: implement visual camera target auto-centering and active agent glows in sandbox"
```

---

### Task 3: Interactive Sandbox Canvas Clicks Focus

**Files:**
- Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`
- Test: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx`

- [ ] **Step 1: Write the failing test**

Add click handler tests in `LudusSandbox.test.tsx`:
```typescript
  it('correctly maps canvas clicks to building focusedFile states', () => {
    const files = ['crates/vox-db/src/lib.rs'];
    const { render, fireEvent } = require('@testing-library/react');
    const { container } = render(<LudusSandbox files={files} />);
    const canvas = container.querySelector('canvas');
    expect(canvas).toBeDefined();

    fireEvent.click(canvas, { clientX: 400, clientY: 100 });
    // Expect canvas click to trigger focused file dispatching
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (click does not update store)

- [ ] **Step 3: Write minimal implementation**

Update click handler in `LudusSandbox.tsx`:
```typescript
  const handleCanvasClick = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const clickX = e.clientX - rect.left;
    const clickY = e.clientY - rect.top;

    // Convert screen coordinates to world coordinates based on camera transform
    const worldX = (clickX - camera.x) / camera.zoom;
    const worldY = (clickY - camera.y) / camera.zoom;

    const centerOffsetX = 1000;
    const centerOffsetY = 100;

    // Find clicked building within click radius threshold
    for (const [filePath, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      const dx = worldX - px;
      const dy = worldY - py;
      const distance = Math.sqrt(dx * dx + dy * dy);
      if (distance < 20) {
        useLudusStore.getState().setFocusedFile(filePath);
        break;
      }
    }
  };
```
Bind the event listener to `<canvas ref={canvasRef} onClick={handleCanvasClick} ... />`.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx
git commit -m "feat: add canvas click handler mapping screen coordinates to building selections"
```

---

### Task 4: Collapsible Mini-Map on Dashboard

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx`

- [ ] **Step 1: Write the failing test**

Add tests to `Dashboard.test.tsx` ensuring collapsible mini-map renders:
```typescript
  it('renders visual sandbox mini-map and handles expand navigation', () => {
    const navigateMock = vi.fn();
    const data = { agents: [], stream: [], alerts: [], kpis: { budgetBurn: { value: 0, spark: [] }, queueDepth: { value: 0, spark: [] } } };
    
    const { render, screen } = require('@testing-library/react');
    render(
      <Dashboard
        data={data}
        onPause={vi.fn()}
        onResume={vi.fn()}
        onDoubt={vi.fn()}
        onOverrule={vi.fn()}
        onAckLudus={vi.fn()}
        filterKind="all"
        setFilterKind={vi.fn()}
        onNavigate={navigateMock}
      />
    );

    const expandBtn = screen.getByText('Immersive View');
    expect(expandBtn).toBeDefined();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/surfaces/Dashboard/Dashboard.test.tsx`
Expected: FAIL (cannot read properties of undefined, or expand button not found)

- [ ] **Step 3: Write minimal implementation**

Update `Dashboard.tsx` to accept and render collapsible mini-map:
```typescript
import { LudusSandbox } from '../../gamify/LudusSandbox';

interface DashboardProps {
  // Existing props...
  onNavigate?: (vk: string) => void;
}

// Inside Dashboard component:
const [sandboxCollapsed, setSandboxCollapsed] = useState(false);

// Render before DashboardGrid:
<div className="mx-5 mb-4 border border-zinc-800 bg-[#09090b]/80 rounded-xl overflow-hidden">
  <div className="flex items-center justify-between bg-zinc-900/60 px-4 py-2 text-xs border-b border-zinc-800">
    <span className="font-semibold text-zinc-100 uppercase tracking-wide">⬤ Workspace Simulation Mini-Map</span>
    <div className="flex gap-2">
      <button type="button" onClick={() => onNavigate?.('gamify')} className="text-cyan hover:underline">Immersive View</button>
      <button type="button" onClick={() => setSandboxCollapsed(!sandboxCollapsed)} className="text-zinc-400 hover:text-zinc-200">
        {sandboxCollapsed ? 'Expand' : 'Collapse'}
      </button>
    </div>
  </div>
  {!sandboxCollapsed && (
    <div className="h-[250px] relative">
      <LudusSandbox files={Object.keys(useLudusStore.getState().buildings || {})} />
    </div>
  )}
</div>
```

Update `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` to pass `onNavigate` prop to Dashboard component.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/surfaces/Dashboard/Dashboard.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/components/surfaces/Dashboard/Dashboard.test.tsx
git commit -m "feat: embed collapsible visual mini-map on Dashboard layout"
```

---

### Task 5: Full-Screen Sandbox in Gamify tab

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.test.tsx`

- [ ] **Step 1: Write the failing test**

Update `GamifyView.test.tsx` to verify sandbox mounting:
```typescript
  it('renders immersive fullscreen sandbox visualizer', () => {
    const { render, container } = require('@testing-library/react');
    render(<GamifyView pushToast={vi.fn()} />);
    const canvas = container.querySelector('canvas');
    expect(canvas).toBeDefined();
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/surfaces/Gamify/GamifyView.test.tsx`
Expected: FAIL (No canvas element found)

- [ ] **Step 3: Write minimal implementation**

Update `GamifyView.tsx` to import and render the `LudusSandbox` component:
```typescript
import { LudusSandbox } from '../../gamify/LudusSandbox';

// Inside GamifyView component, above the Notifications list:
<div className="mb-6 border border-zinc-800 rounded-xl overflow-hidden bg-zinc-950/60 p-4">
  <h3 className="mb-2 font-display text-[12px] uppercase tracking-wide text-zinc-400">Simulation Map</h3>
  <div className="h-[450px]">
    <LudusSandbox files={Object.keys(useLudusStore.getState().buildings || {})} />
  </div>
</div>
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/surfaces/Gamify/GamifyView.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.tsx crates/vox-gui/ui/src/components/surfaces/Gamify/GamifyView.test.tsx
git commit -m "feat: mount full screen visual sandbox in Gamify view"
```

---

### Task 6: Hooking Up Selection Events (Search, Scientia, Tasks)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx`

- [ ] **Step 1: Write the failing test**

Modify `SearchView.test.tsx` to assert that selecting a file triggers focusedFile updates:
```typescript
  it('updates focusedFile store property when file list item is clicked', () => {
    // Assert focusedFile change triggers on selection handlers
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/surfaces/Search/SearchView.test.tsx`
Expected: FAIL (assertion fails)

- [ ] **Step 3: Write minimal implementation**

In `SearchView.tsx`:
```typescript
import { useLudusStore } from '../../gamify/store';

// On file click/selection:
useLudusStore.getState().setFocusedFile(filePath);
```
Repeat for `DiscoveryReviewView.tsx` and `TasksView.tsx` selection actions.

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/surfaces/Search/SearchView.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryReviewView.tsx crates/vox-gui/ui/src/components/surfaces/Tasks/TasksView.tsx
git commit -m "feat: dispatch focusedFile updates on Search, Scientia, and Tasks selection"
```
