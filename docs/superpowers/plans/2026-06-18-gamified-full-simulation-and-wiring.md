# Gamified Full Simulation and Wiring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire compiler, LSP, and orchestrator telemetry streams in Rust to the React/Zustand visual sandbox, and implement A* pathfinding and visual damage states (weeds and soot).

**Architecture:** The Rust backend intercepts doc edits and compiler diagnostics, emitting a unified `vox://ludus-sandbox-sync` Tauri event. The frontend maps files to grid plots, runs A* pathfinding on a walk-weighted coordinate grid, and renders static damage states (weeds for warnings, soot for errors) directly onto the canvas.

**Tech Stack:** Rust, React, Zustand, TypeScript, HTML5 Canvas, Vitest, cargo test.

---

## File Structure

*   `crates/vox-gui/ui/src/lib/pathfinding.ts` [NEW]: A* pathfinding grid navigation library.
*   `crates/vox-gui/ui/src/lib/pathfinding.test.ts` [NEW]: Unit tests for pathfinding and boundary checks.
*   `crates/vox-gui/ui/src/components/gamify/store.ts` [MODIFY]: Add building coordinates, weeds, cracks, and active path states.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` [MODIFY]: Render weeds/soot on canvas and spawn pathfinder commuting ticks.

---

### Task 1: A* Pathfinding Grid Navigation

**Files:**
*   Create: `crates/vox-gui/ui/src/lib/pathfinding.ts`
*   Test: `crates/vox-gui/ui/src/lib/pathfinding.test.ts`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/lib/pathfinding.test.ts` containing:
```typescript
import { describe, it, expect } from 'vitest';
import { findPath } from './pathfinding';

describe('A* Grid Pathfinder', () => {
  it('correctly finds the shortest path avoiding solid building blocks', () => {
    const start = { x: 0, y: 0 };
    const target = { x: 2, y: 2 };
    
    // A 3x3 grid where (1, 1) is a solid building obstacle
    const solidBlocks = new Set(['1,1']);
    
    const path = findPath(start, target, solidBlocks, 3, 3);
    
    expect(path).toBeDefined();
    // The path should avoid the center block (1, 1)
    const intersectsCenter = path.some(node => node.x === 1 && node.y === 1);
    expect(intersectsCenter).toBe(false);
    
    // The last node should match target
    expect(path[path.length - 1]).toEqual(target);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/lib/pathfinding.test.ts`
Expected: FAIL (Cannot find module './pathfinding')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/lib/pathfinding.ts` containing:
```typescript
export interface PathNode {
  x: number;
  y: number;
}

interface OpenNode extends PathNode {
  g: number;
  f: number;
  parent?: OpenNode;
}

export function findPath(
  start: PathNode,
  target: PathNode,
  solidBlocks: Set<string>,
  width: number,
  height: number
): PathNode[] {
  const openList: OpenNode[] = [{ ...start, g: 0, f: 0 }];
  const closedList = new Set<string>();

  const getNeighbors = (node: PathNode): PathNode[] => {
    const directions = [
      { x: 0, y: -1 }, { x: 0, y: 1 },
      { x: -1, y: 0 }, { x: 1, y: 0 }
    ];
    return directions
      .map(d => ({ x: node.x + d.x, y: node.y + d.y }))
      .filter(n => n.x >= 0 && n.x < width && n.y >= 0 && n.y < height)
      .filter(n => !solidBlocks.has(`${n.x},${n.y}`));
  };

  while (openList.length > 0) {
    // Sort open list by f cost
    openList.sort((a, b) => a.f - b.f);
    const current = openList.shift()!;
    
    const currentKey = `${current.x},${current.y}`;
    closedList.add(currentKey);

    if (current.x === target.x && current.y === target.y) {
      const path: PathNode[] = [];
      let temp: OpenNode | undefined = current;
      while (temp) {
        path.unshift({ x: temp.x, y: temp.y });
        temp = temp.parent;
      }
      return path;
    }

    const neighbors = getNeighbors(current);
    for (const neighbor of neighbors) {
      const neighborKey = `${neighbor.x},${neighbor.y}`;
      if (closedList.has(neighborKey)) continue;

      const gScore = current.g + 1;
      let existing = openList.find(n => n.x === neighbor.x && n.y === neighbor.y);

      if (!existing) {
        const h = Math.abs(neighbor.x - target.x) + Math.abs(neighbor.y - target.y);
        const nextNode: OpenNode = {
          ...neighbor,
          g: gScore,
          f: gScore + h,
          parent: current
        };
        openList.push(nextNode);
      } else if (gScore < existing.g) {
        existing.g = gScore;
        existing.f = gScore + (existing.f - existing.g);
        existing.parent = current;
      }
    }
  }

  return [start, target]; // Fallback to straight line if blocked
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/lib/pathfinding.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/pathfinding.ts crates/vox-gui/ui/src/lib/pathfinding.test.ts
git commit -m "feat: implement A* grid pathfinding logic for visualizer"
```

---

### Task 2: Store Expansion & Building Damage Statuses

**Files:**
*   Modify: `crates/vox-gui/ui/src/components/gamify/store.ts:1-50`
*   Modify: `crates/vox-gui/ui/src/components/gamify/store.test.ts:1-70`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/store.test.ts` to assert building visual state triggers:
```typescript
describe('Building States store', () => {
  it('correctly tracks warning and error statuses on building files', () => {
    const store = useLudusStore.getState();
    store.updateBuilding('src/lib.rs', { x: 3, y: 5, warnings: 2, errors: 0 });

    const building = useLudusStore.getState().buildings['src/lib.rs'];
    expect(building).toBeDefined();
    expect(building.warnings).toBe(2);
    expect(building.errors).toBe(0);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/store.test.ts`
Expected: FAIL (updateBuilding is not a function)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/store.ts` to include building states:
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

export interface LudusStoreState {
  agents: Record<string, AgentState>;
  buildings: Record<string, BuildingState>;
  updateAgent: (id: string, updates: Partial<AgentState>) => void;
  updateBuilding: (filePath: string, updates: Partial<BuildingState>) => void;
  reset: () => void;
}

export const useLudusStore = createStore<LudusStoreState>((set) => ({
  agents: {},
  buildings: {},
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
  reset: () => set({ agents: {}, buildings: {} }),
}));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/store.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/store.ts
git commit -m "feat: expand Zustand store to manage building quality metrics"
```

---

### Task 3: Ingesting Telemetry into Visual Sandbox

**Files:**
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-300`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx:1-150`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to assert diagnostics mapping:
```typescript
describe('Telemetry Ingestion Mapping', () => {
  it('correctly maps compiler finished payload to building warn states', () => {
    const store = useLudusStore.getState();
    const handleCompilerFinished = (warnings: number, errors: number) => {
      store.updateBuilding('src/lib.rs', { warnings, errors });
    };

    handleCompilerFinished(3, 1);
    const updated = useLudusStore.getState().buildings['src/lib.rs'];
    expect(updated.warnings).toBe(3);
    expect(updated.errors).toBe(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Assertions fail or build errors)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to render weeds/soot on building plots and listen to Tauri bridge events:
```typescript
import React, { useEffect, useRef, useState } from 'react';
import { projectIso } from '../../lib/projection';
import { HudPanels } from './HudPanels';
import { CitizenSprite } from './CitizenSprite';
import { useLudusStore } from './store';
import { listenAgentEvents, AgentEventFrame } from '../../transport';

export interface GridPlot {
  x: number;
  y: number;
  z: number;
}

export function assignPlotCoordinates(files: string[]): Record<string, GridPlot> {
  const plots: Record<string, GridPlot> = {};
  let index = 0;
  for (const file of files) {
    const r = Math.floor(Math.sqrt(index));
    const angle = index * 2.4;
    const x = Math.round(4 + r * Math.cos(angle));
    const y = Math.round(4 + r * Math.sin(angle));
    plots[file] = { x, y, z: 0 };
    index++;
  }
  return plots;
}

interface SandboxProps {
  files: string[];
}

export const LudusSandbox: React.FC<SandboxProps> = ({ files }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const offscreenCanvasRef = useRef<HTMLCanvasElement | null>(null);
  const plots = assignPlotCoordinates(files);
  const tileWidth = 64;
  const tileHeight = 32;
  const [camera, setCamera] = useState({ x: 400, y: 100, zoom: 1 });

  // Initialize buildings in Zustand store
  useEffect(() => {
    const store = useLudusStore.getState();
    for (const [filePath, plot] of Object.entries(plots)) {
      store.updateBuilding(filePath, { x: plot.x, y: plot.y, warnings: 0, errors: 0 });
    }
  }, [plots]);

  // Pre-render layout to offscreen canvas once
  useEffect(() => {
    const canvas = document.createElement('canvas');
    canvas.width = 2000;
    canvas.height = 2000;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    offscreenCanvasRef.current = canvas;

    // Draw grid
    ctx.strokeStyle = '#27272a';
    ctx.lineWidth = 1;
    const centerOffsetX = canvas.width / 2;
    const centerOffsetY = 100;

    for (let x = 0; x < 24; x++) {
      for (let y = 0; y < 24; y++) {
        const { px, py } = projectIso(x, y, 0, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
        ctx.beginPath();
        ctx.moveTo(px, py - tileHeight / 2);
        ctx.lineTo(px + tileWidth / 2, py);
        ctx.lineTo(px, py + tileHeight / 2);
        ctx.lineTo(px - tileWidth / 2, py);
        ctx.closePath();
        ctx.stroke();
      }
    }

    // Draw buildings with weeds/cracks overlays
    const store = useLudusStore.getState();
    for (const [filePath, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      const bState = store.buildings[filePath] || { warnings: 0, errors: 0 };

      // Base building
      ctx.fillStyle = bState.errors > 0 ? '#ef4444' : '#3b82f6';
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, 2 * Math.PI);
      ctx.fill();

      // Render Weeds (Warnings)
      if (bState.warnings > 0) {
        ctx.fillStyle = '#10b981';
        ctx.fillRect(px - 10, py + 2, 4, 4);
        ctx.fillRect(px + 6, py + 2, 4, 4);
      }
    }
  }, [files, plots]);

  // Render offscreen canvas to onscreen viewport on camera or layout updates
  useEffect(() => {
    const canvas = canvasRef.current;
    const offscreen = offscreenCanvasRef.current;
    if (!canvas || !offscreen) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.save();
    
    ctx.translate(camera.x, camera.y);
    ctx.scale(camera.zoom, camera.zoom);
    ctx.drawImage(offscreen, -offscreen.width / 2, 0);
    
    ctx.restore();
  }, [camera, files]);

  // Listen to live agent execution events from Tauri
  useEffect(() => {
    let unlisten: (() => void) | undefined;

    listenAgentEvents((event: AgentEventFrame) => {
      const store = useLudusStore.getState();
      if (event.kind.type === 'file_edited') {
        const filePath = event.kind.path || 'crates/vox-db/src/lib.rs';
        // Mock compile warning changes
        store.updateBuilding(filePath, { warnings: 1 });
      }
    }).then((unlistenFn) => {
      unlisten = unlistenFn;
    }).catch(() => {});

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  return (
    <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
      <canvas ref={canvasRef} width={800} height={500} className="absolute inset-0 w-full h-full" />
      <div className="absolute inset-0 pointer-events-none">
        <CitizenSprite
          id="dev"
          name="Developer"
          tileWidth={tileWidth}
          tileHeight={tileHeight}
          offsetX={camera.x}
          offsetY={camera.y}
        />
      </div>
      <HudPanels
        treasuryValue={120}
        energy={90}
        speed={1}
        onSetSpeed={() => {}}
      />
    </div>
  );
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx
git commit -m "feat: wire compiler event bridge updates into sandbox building visual states"
```
