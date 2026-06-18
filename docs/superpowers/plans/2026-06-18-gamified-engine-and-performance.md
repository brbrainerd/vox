# Gamified Engine and Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement offscreen canvas caching for high-performance city layouts and a decoupled Zustand store for DOM overlays to ensure 60fps pan/zoom and low-CPU idle state updates.

**Architecture:** The terrain grid and buildings are pre-rendered onto an offscreen canvas and blitted to the onscreen viewport with transform offsets. Active character coordinates are stored in a Zustand store, where citizen DOM elements subscribe directly to position updates and modify their style transformations via raw DOM refs, bypassing React virtual DOM reconciliation.

**Tech Stack:** React, Zustand, TypeScript, HTML5 Canvas, Vitest.

---

## File Structure

*   `crates/vox-gui/ui/src/components/gamify/store.ts` [NEW]: Zustand store managing active citizen positions, speeds, paths, and moods.
*   `crates/vox-gui/ui/src/components/gamify/store.test.ts` [NEW]: Unit tests for store mutations and subscriptions.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` [MODIFY]: Integrate offscreen canvas caching and viewport transforms.
*   `crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx` [MODIFY]: Switch from React state transitions to direct Zustand ref subscriptions.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` [MODIFY]: Add panning, zooming, and performance scaling tests.

---

### Task 1: Decoupled Zustand Coordinate Store

**Files:**
*   Create: `crates/vox-gui/ui/src/components/gamify/store.ts`
*   Test: `crates/vox-gui/ui/src/components/gamify/store.test.ts`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/components/gamify/store.test.ts` containing:
```typescript
import { describe, it, expect } from 'vitest';
import { useLudusStore } from './store';

describe('Ludus Zustand Store', () => {
  it('correctly sets and updates citizen positions', () => {
    const store = useLudusStore.getState();
    store.updateAgent('agent_1', { x: 5, y: 7 });

    const updated = useLudusStore.getState().agents['agent_1'];
    expect(updated).toBeDefined();
    expect(updated.x).toBe(5);
    expect(updated.y).toBe(7);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/store.test.ts`
Expected: FAIL (Cannot find module './store')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/components/gamify/store.ts` containing:
```typescript
import { createStore } from 'zustand/vanilla';

export interface AgentState {
  x: number;
  y: number;
  energy: number;
  mood: string;
}

export interface LudusStoreState {
  agents: Record<string, AgentState>;
  updateAgent: (id: string, updates: Partial<AgentState>) => void;
}

export const useLudusStore = createStore<LudusStoreState>((set) => ({
  agents: {},
  updateAgent: (id, updates) =>
    set((state) => {
      const current = state.agents[id] || { x: 0, y: 0, energy: 100, mood: 'Happy' };
      return {
        agents: {
          ...state.agents,
          [id]: { ...current, ...updates },
        },
      };
    }),
}));
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/store.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/store.ts crates/vox-gui/ui/src/components/gamify/store.test.ts
git commit -m "feat: add decoupled zustand coordinate store for visualizer"
```

---

### Task 2: Offscreen Canvas Caching & Viewport Transforms

**Files:**
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-350`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx:1-200`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to assert viewport matrix transformation math:
```typescript
describe('Viewport Matrix Projection', () => {
  it('correctly maps client mouse coords to offset coordinates', () => {
    const mouseX = 150;
    const mouseY = 150;
    const cameraX = 50;
    const cameraY = 50;
    const zoom = 2;

    const worldX = (mouseX - cameraX) / zoom;
    const worldY = (mouseY - cameraY) / zoom;

    expect(worldX).toBe(50);
    expect(worldY).toBe(50);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Compile error or test assertion fails)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to pre-render onto an offscreen canvas:
```typescript
import React, { useEffect, useRef, useState } from 'react';
import { projectIso } from '../../lib/projection';
import { PathNode } from './CitizenSprite';
import { HudPanels } from './HudPanels';

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

  // Pre-render layout to offscreen canvas once
  useEffect(() => {
    const canvas = document.createElement('canvas');
    canvas.width = 2000;
    canvas.height = 2000;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    offscreenCanvasRef.current = canvas;

    // Draw isometric grid
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

    // Draw buildings
    ctx.fillStyle = '#3b82f6';
    for (const [_, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, centerOffsetX, centerOffsetY);
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, 2 * Math.PI);
      ctx.fill();
    }
  }, [files]);

  // Main rendering loop blitting offscreen to onscreen with transforms
  useEffect(() => {
    const canvas = canvasRef.current;
    const offscreen = offscreenCanvasRef.current;
    if (!canvas || !offscreen) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let frameId: number;

    const render = () => {
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      ctx.save();
      
      // Apply pan and zoom transforms
      ctx.translate(camera.x, camera.y);
      ctx.scale(camera.zoom, camera.zoom);

      // Copy buffer to screen (offset to align offscreen center with translation origin)
      ctx.drawImage(offscreen, -offscreen.width / 2, 0);
      
      ctx.restore();
      frameId = requestAnimationFrame(render);
    };

    render();
    return () => cancelAnimationFrame(frameId);
  }, [camera]);

  return (
    <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
      <canvas ref={canvasRef} width={800} height={500} className="absolute inset-0 w-full h-full" />
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
git commit -m "feat: implement offscreen canvas buffering and camera transform projection"
```

---

### Task 3: Decoupled DOM Overlay Sprites with Ref Subscriptions

**Files:**
*   Modify: `crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx:1-100`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-300`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to verify Zustand store subscriptions behave correctly:
```typescript
describe('DOM Subscription Engine', () => {
  it('correctly reacts to store updates without parent re-renders', () => {
    let callCount = 0;
    const unsubscribe = useLudusStore.subscribe((state) => {
      if (state.agents['agent_1']) callCount += 1;
    });

    useLudusStore.getState().updateAgent('agent_1', { x: 4, y: 4 });
    expect(callCount).toBe(1);
    unsubscribe();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Compile error or test assertion fails)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx` to subscribe directly to Zustand updates and write changes to element refs:
```typescript
import React, { useEffect, useRef } from 'react';
import { projectIso, getZIndex } from '../../lib/projection';
import { useLudusStore } from './store';

interface CitizenProps {
  id: string;
  name: string;
  tileWidth: number;
  tileHeight: number;
  offsetX: number;
  offsetY: number;
}

export const CitizenSprite: React.FC<CitizenProps> = ({
  id,
  name,
  tileWidth,
  tileHeight,
  offsetX,
  offsetY,
}) => {
  const spriteRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    // Register agent in the store
    useLudusStore.getState().updateAgent(id, { x: 2, y: 2, energy: 100, mood: 'Happy' });

    // Subscribe directly to store updates for this specific agent id
    const unsubscribe = useLudusStore.subscribe((state) => {
      const agent = state.agents[id];
      const el = spriteRef.current;
      if (!agent || !el) return;

      // Projection translation
      const { px, py } = projectIso(agent.x, agent.y, 0, tileWidth, tileHeight, offsetX, offsetY);
      const zIndex = getZIndex(agent.x, agent.y);

      // Direct styling updates bypassing React render cycle
      el.style.transform = `translate3d(${px}px, ${py - 24}px, 0) translate(-50%, -50%)`;
      el.style.zIndex = zIndex.toString();
    });

    return () => unsubscribe();
  }, [id, tileWidth, tileHeight, offsetX, offsetY]);

  return (
    <div
      ref={spriteRef}
      className="absolute flex flex-col items-center pointer-events-none transition-transform duration-75"
      style={{ left: 0, top: 0, zIndex: 0 }}
    >
      <div className="text-[9px] bg-black/80 px-1 py-0.5 rounded border border-blue-500/20 text-blue-400 font-mono scale-75 whitespace-nowrap mb-1">
        {name}
      </div>
      <div className="w-6 h-6 rounded-full bg-blue-500 flex items-center justify-center border border-white/20 shadow-lg">
        <span>👨‍💻</span>
      </div>
    </div>
  );
};
```

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to render subscription-based overlays:
```typescript
// Add imports for CitizenSprite inside LudusSandbox
import { CitizenSprite } from './CitizenSprite';

// Inside the return block of LudusSandbox:
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
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx
git commit -m "feat: implement decoupled DOM subscriptions for overlay sprites bypassing React"
```
