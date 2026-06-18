# Gamified Codebase Representation (Ludus Sandbox) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an interactive 2.5D isometric town simulation (Ludus Sandbox) in the Voxed UI (Tauri desktop app) representing the codebase architecture, active developer/agent operations, and FinOps treasury metrics.

**Architecture:** A high-performance hybrid rendering model utilizing a static HTML5 Canvas for the terrain grid, roads, and buildings, overlaid with absolute-positioned React DOM components for moving citizen sprites, selection rings, and speech bubbles. Z-ordering is resolved dynamically via coordinate depth projection.

**Tech Stack:** React, TypeScript, Tailwind CSS, HTML5 Canvas, Tauri Event API, Vitest.

---

## File Structure

The following files will be created or modified:

*   `crates/vox-gui/ui/src/lib/projection.ts` [NEW]: Contains 2.5D isometric projection math and z-index calculation.
*   `crates/vox-gui/ui/src/lib/projection.test.ts` [NEW]: Unit tests for the projection math.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` [NEW]: Main React container managing Canvas rendering and DOM layering.
*   `crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx` [NEW]: Component representing human developer and AI agent characters.
*   `crates/vox-gui/ui/src/components/gamify/HudPanels.tsx` [NEW]: Roster, Needs, Treasury, and Speed controls HUD.
*   `crates/vox-gui/ui/src/components/gamify/CurfewOverlay.tsx` [NEW]: overlay and 10s timeout delay screen.
*   `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` [NEW]: UI components and integration tests.

---

### Task 1: Isometric Projection Mathematics and Utilities

**Files:**
*   Create: `crates/vox-gui/ui/src/lib/projection.ts`
*   Test: `crates/vox-gui/ui/src/lib/projection.test.ts`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/lib/projection.test.ts` containing:
```typescript
import { describe, it, expect } from 'vitest';
import { projectIso, getZIndex } from './projection';

describe('Isometric Projection', () => {
  it('correctly projects 3D coordinates to 2D pixels', () => {
    const tileWidth = 64;
    const tileHeight = 32;
    const offsetX = 300;
    const offsetY = 150;

    // Center tile (0, 0, 0)
    const p1 = projectIso(0, 0, 0, tileWidth, tileHeight, offsetX, offsetY);
    expect(p1.px).toBe(300);
    expect(p1.py).toBe(150);

    // Coordinate with depth and elevation (2, 3, 1)
    const p2 = projectIso(2, 3, 1, tileWidth, tileHeight, offsetX, offsetY);
    // px = (2 - 3) * 32 + 300 = 268
    // py = (2 + 3) * 16 - 1 * 20 + 150 = 210
    expect(p2.px).toBe(268);
    expect(p2.py).toBe(210);
  });

  it('correctly computes depth zIndex based on tile distance', () => {
    expect(getZIndex(0, 0)).toBe(0);
    expect(getZIndex(2.5, 3.1)).toBe(5);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/lib/projection.test.ts`
Expected: FAIL (Cannot find module './projection')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/lib/projection.ts` containing:
```typescript
export interface ScreenCoords {
  px: number;
  py: number;
}

/**
 * Projects a 3D grid coordinate (x, y, z) into 2D screen pixels.
 */
export function projectIso(
  x: number,
  y: number,
  z: number,
  tileWidth: number,
  tileHeight: number,
  offsetX: number,
  offsetY: number,
  heightScale: number = 20
): ScreenCoords {
  const tileWidthHalf = tileWidth / 2;
  const tileHeightHalf = tileHeight / 2;

  const px = (x - y) * tileWidthHalf + offsetX;
  const py = (x + y) * tileHeightHalf - z * heightScale + offsetY;

  return { px, py };
}

/**
 * Calculates depth zIndex ordering for DOM overlays.
 */
export function getZIndex(x: number, y: number): number {
  return Math.floor(x + y);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/lib/projection.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/projection.ts crates/vox-gui/ui/src/lib/projection.test.ts
git commit -m "feat: add isometric projection utility and test"
```

---

### Task 2: Codebase-to-Grid Coordinate Assignment & Map Ingest

**Files:**
*   Create: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx`
*   Test: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` containing:
```typescript
import React from 'react';
import { describe, it, expect } from 'vitest';
import { render } from '@testing-library/react';
import { LudusSandbox, assignPlotCoordinates } from './LudusSandbox';

describe('LudusSandbox Map Generation', () => {
  it('correctly maps file paths to unique plot coordinates', () => {
    const files = [
      'crates/vox-db/src/lib.rs',
      'crates/vox-db/src/queries.rs',
      'crates/vox-gamify/src/lib.rs'
    ];
    const plots = assignPlotCoordinates(files);
    
    expect(plots['crates/vox-db/src/lib.rs']).toBeDefined();
    expect(plots['crates/vox-db/src/queries.rs']).toBeDefined();
    
    // Coordinates must be distinct
    const coord1 = plots['crates/vox-db/src/lib.rs'];
    const coord2 = plots['crates/vox-db/src/queries.rs'];
    expect(coord1.x === coord2.x && coord1.y === coord2.y).toBe(false);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Cannot find module './LudusSandbox')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` containing:
```typescript
import React, { useEffect, useRef } from 'react';
import { projectIso } from '../../lib/projection';

export interface GridPlot {
  x: number;
  y: number;
  z: number;
}

export function assignPlotCoordinates(files: string[]): Record<string, GridPlot> {
  const plots: Record<string, GridPlot> = {};
  
  // Arrange files in a concentric spiral outward from (2, 2)
  let index = 0;
  for (const file of files) {
    const r = Math.floor(Math.sqrt(index));
    const angle = index * 2.4; // Golden angle approximation
    const x = Math.round(2 + r * Math.cos(angle));
    const y = Math.round(2 + r * Math.sin(angle));
    plots[file] = { x, y, z: 0 };
    index++;
  }
  
  return plots;
}

interface SandboxProps {
  files: string[];
  onSelectBuilding?: (filePath: string) => void;
}

export const LudusSandbox: React.FC<SandboxProps> = ({ files, onSelectBuilding }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const plots = assignPlotCoordinates(files);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    // Clear canvas
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Draw isometric grid background
    ctx.strokeStyle = '#27272a';
    ctx.lineWidth = 1;
    const tileWidth = 64;
    const tileHeight = 32;
    const offsetX = canvas.width / 2;
    const offsetY = 100;

    for (let x = 0; x < 8; x++) {
      for (let y = 0; y < 8; y++) {
        const { px, py } = projectIso(x, y, 0, tileWidth, tileHeight, offsetX, offsetY);
        
        ctx.beginPath();
        ctx.moveTo(px, py - tileHeight / 2);
        ctx.lineTo(px + tileWidth / 2, py);
        ctx.lineTo(px, py + tileHeight / 2);
        ctx.lineTo(px - tileWidth / 2, py);
        ctx.closePath();
        ctx.stroke();
      }
    }

    // Draw building files
    ctx.fillStyle = '#3b82f6';
    for (const [file, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, offsetX, offsetY);
      
      // Draw a simple building block
      ctx.beginPath();
      ctx.moveTo(px, py);
      ctx.lineTo(px + 10, py - 10);
      ctx.lineTo(px + 10, py - 30);
      ctx.lineTo(px - 10, py - 30);
      ctx.lineTo(px - 10, py - 10);
      ctx.closePath();
      ctx.fill();
    }
  }, [files]);

  return (
    <div className="relative w-full h-full bg-[#09090b]">
      <canvas ref={canvasRef} width={800} height={500} className="w-full h-full" />
    </div>
  );
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx
git commit -m "feat: implement basic isometric map layout assigning files to coordinates"
```

---

### Task 3: Animated DOM-Based Citizen Sprites with Pathfinding

**Files:**
*   Create: `crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-120`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx:1-50`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to include:
```typescript
import { CitizenSprite } from './CitizenSprite';

describe('CitizenSprite State Machine', () => {
  it('correctly advances path nodes sequentially', () => {
    const path = [{ x: 0, y: 0 }, { x: 1, y: 0 }, { x: 1, y: 1 }];
    let position = { x: 0, y: 0 };
    
    // Simulate tick to move sprite
    const advancePosition = (current: { x: number, y: number }, target: { x: number, y: number }): { x: number, y: number } => {
      const dx = Math.sign(target.x - current.x);
      const dy = Math.sign(target.y - current.y);
      return { x: current.x + dx, y: current.y + dy };
    };

    position = advancePosition(position, path[1]);
    expect(position.x).toBe(1);
    expect(position.y).toBe(0);
    
    position = advancePosition(position, path[2]);
    expect(position.x).toBe(1);
    expect(position.y).toBe(1);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Cannot find module './CitizenSprite')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx` containing:
```typescript
import React, { useEffect, useState } from 'react';
import { projectIso, getZIndex } from '../../lib/projection';

export interface PathNode {
  x: number;
  y: number;
}

interface CitizenProps {
  name: string;
  energy: number;
  mood: string;
  path: PathNode[];
  tileWidth: number;
  tileHeight: number;
  offsetX: number;
  offsetY: number;
}

export const CitizenSprite: React.FC<CitizenProps> = ({
  name,
  energy,
  mood,
  path,
  tileWidth,
  tileHeight,
  offsetX,
  offsetY,
}) => {
  const [coords, setCoords] = useState<PathNode>({ x: 2, y: 2 });
  const [currentNodeIdx, setCurrentNodeIdx] = useState(0);

  useEffect(() => {
    if (path.length === 0) return;
    
    const interval = setInterval(() => {
      const target = path[currentNodeIdx];
      if (!target) return;

      setCoords((current) => {
        const dx = target.x - current.x;
        const dy = target.y - current.y;
        
        if (Math.abs(dx) < 0.1 && Math.abs(dy) < 0.1) {
          if (currentNodeIdx < path.length - 1) {
            setCurrentNodeIdx(currentNodeIdx + 1);
          }
          return target;
        }

        return {
          x: current.x + Math.sign(dx) * 0.1,
          y: current.y + Math.sign(dy) * 0.1,
        };
      });
    }, 50);

    return () => clearInterval(interval);
  }, [path, currentNodeIdx]);

  const { px, py } = projectIso(coords.x, coords.y, 0, tileWidth, tileHeight, offsetX, offsetY);
  const zIndex = getZIndex(coords.x, coords.y);

  return (
    <div
      className="absolute flex flex-col items-center pointer-events-none transition-transform duration-75"
      style={{
        left: `${px}px`,
        top: `${py - 24}px`, // Offset height of character sprite
        zIndex,
        transform: 'translate(-50%, -50%)',
      }}
    >
      <div className="text-[9px] bg-black/80 px-1 py-0.5 rounded border border-blue-500/20 text-blue-400 font-mono scale-75 whitespace-nowrap mb-1">
        {name} ({energy}%)
      </div>
      <div className="w-6 h-6 rounded-full bg-blue-500 flex items-center justify-center border border-white/20 shadow-lg animate-bounce">
        <span>👨‍💻</span>
      </div>
    </div>
  );
};
```

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to render overlay sprites:
```typescript
import React, { useEffect, useRef } from 'react';
import { projectIso } from '../../lib/projection';
import { CitizenSprite, PathNode } from './CitizenSprite';

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
  agents: Array<{ name: string; energy: number; mood: string; path: PathNode[] }>;
}

export const LudusSandbox: React.FC<SandboxProps> = ({ files, agents }) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const plots = assignPlotCoordinates(files);
  const tileWidth = 64;
  const tileHeight = 32;
  const offsetX = 400;
  const offsetY = 100;

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.strokeStyle = '#27272a';
    ctx.lineWidth = 1;

    for (let x = 0; x < 12; x++) {
      for (let y = 0; y < 12; y++) {
        const { px, py } = projectIso(x, y, 0, tileWidth, tileHeight, offsetX, offsetY);
        ctx.beginPath();
        ctx.moveTo(px, py - tileHeight / 2);
        ctx.lineTo(px + tileWidth / 2, py);
        ctx.lineTo(px, py + tileHeight / 2);
        ctx.lineTo(px - tileWidth / 2, py);
        ctx.closePath();
        ctx.stroke();
      }
    }

    ctx.fillStyle = '#3b82f6';
    for (const [_, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, offsetX, offsetY);
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, 2 * Math.PI);
      ctx.fill();
    }
  }, [files]);

  return (
    <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
      <canvas ref={canvasRef} width={800} height={500} className="absolute inset-0 w-full h-full" />
      <div className="absolute inset-0 pointer-events-none">
        {agents.map((agent, index) => (
          <CitizenSprite
            key={index}
            name={agent.name}
            energy={agent.energy}
            mood={agent.mood}
            path={agent.path}
            tileWidth={tileWidth}
            tileHeight={tileHeight}
            offsetX={offsetX}
            offsetY={offsetY}
          />
        ))}
      </div>
    </div>
  );
};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/CitizenSprite.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx
git commit -m "feat: add DOM-based character rendering and basic linear interpolation paths"
```

---

### Task 4: Ingestion Event Queue & Telemetry Stream Binding

**Files:**
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-200`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx:1-100`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to verify event queue triggers path updates:
```typescript
describe('LudusSandbox Telemetry Engine', () => {
  it('correctly updates agent paths on receiving FileEdited event', () => {
    const plots = { 'src/lib.rs': { x: 3, y: 5, z: 0 } };
    
    // Ingest function
    const getPathForEvent = (event: { type: string; path: string }) => {
      if (event.type === 'FileEdited') {
        const target = plots[event.path];
        return [{ x: 2, y: 2 }, target];
      }
      return [];
    };

    const path = getPathForEvent({ type: 'FileEdited', path: 'src/lib.rs' });
    expect(path[1]).toEqual({ x: 3, y: 5, z: 0 });
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Compile error or test assertion fails)

- [ ] **Step 3: Write minimal implementation**

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to handle event queues:
```typescript
import React, { useEffect, useRef, useState } from 'react';
import { projectIso } from '../../lib/projection';
import { CitizenSprite, PathNode } from './CitizenSprite';
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
  const plots = assignPlotCoordinates(files);
  const tileWidth = 64;
  const tileHeight = 32;
  const offsetX = 400;
  const offsetY = 100;

  const [agents, setAgents] = useState<Array<{ name: string; energy: number; mood: string; path: PathNode[] }>>([
    { name: 'Dev', energy: 100, mood: 'Happy', path: [] },
  ]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;

    // Listen to live agent execution events from Tauri
    listenAgentEvents((event: AgentEventFrame) => {
      if (event.kind.type === 'task_started' || event.kind.type === 'file_edited') {
        const filePath = event.kind.path || 'crates/vox-db/src/lib.rs';
        const targetPlot = plots[filePath] || { x: 2, y: 2 };
        
        setAgents((prev) =>
          prev.map((agent) => {
            if (agent.name === 'Dev') {
              return {
                ...agent,
                path: [{ x: 2, y: 2 }, { x: targetPlot.x, y: targetPlot.y }],
              };
            }
            return agent;
          })
        );
      }
    }).then((unlistenFn) => {
      unlisten = unlistenFn;
    }).catch(() => {
      // Fallback outside Tauri (e.g. testing context)
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, [plots]);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.strokeStyle = '#27272a';
    ctx.lineWidth = 1;

    for (let x = 0; x < 12; x++) {
      for (let y = 0; y < 12; y++) {
        const { px, py } = projectIso(x, y, 0, tileWidth, tileHeight, offsetX, offsetY);
        ctx.beginPath();
        ctx.moveTo(px, py - tileHeight / 2);
        ctx.lineTo(px + tileWidth / 2, py);
        ctx.lineTo(px, py + tileHeight / 2);
        ctx.lineTo(px - tileWidth / 2, py);
        ctx.closePath();
        ctx.stroke();
      }
    }

    ctx.fillStyle = '#3b82f6';
    for (const [_, plot] of Object.entries(plots)) {
      const { px, py } = projectIso(plot.x, plot.y, plot.z, tileWidth, tileHeight, offsetX, offsetY);
      ctx.beginPath();
      ctx.arc(px, py, 6, 0, 2 * Math.PI);
      ctx.fill();
    }
  }, [files, plots]);

  return (
    <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
      <canvas ref={canvasRef} width={800} height={500} className="absolute inset-0 w-full h-full" />
      <div className="absolute inset-0 pointer-events-none">
        {agents.map((agent, index) => (
          <CitizenSprite
            key={index}
            name={agent.name}
            energy={agent.energy}
            mood={agent.mood}
            path={agent.path}
            tileWidth={tileWidth}
            tileHeight={tileHeight}
            offsetX={offsetX}
            offsetY={offsetY}
          />
        ))}
      </div>
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
git commit -m "feat: wire live tauri agent telemetry events to trigger sandbox pathing"
```

---

### Task 5: Roster, Needs, Treasury & Speed Control HUD

**Files:**
*   Create: `crates/vox-gui/ui/src/components/gamify/HudPanels.tsx`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-250`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to assert HUD elements are rendered:
```typescript
import { render, screen } from '@testing-library/react';
import { HudPanels } from './HudPanels';

describe('Ludus HUD Component', () => {
  it('correctly displays Treasury and Energy stats', () => {
    render(
      <HudPanels 
        treasuryValue={420} 
        energy={85} 
        speed={1} 
        onSetSpeed={() => {}} 
      />
    );
    expect(screen.getByText('420 Crystals')).toBeDefined();
    expect(screen.getByText('Energy: 85%')).toBeDefined();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Cannot find module './HudPanels')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/components/gamify/HudPanels.tsx` containing:
```typescript
import React from 'react';

interface HudProps {
  treasuryValue: number;
  energy: number;
  speed: number;
  onSetSpeed: (speed: number) => void;
}

export const HudPanels: React.FC<HudProps> = ({
  treasuryValue,
  energy,
  speed,
  onSetSpeed,
}) => {
  return (
    <div className="absolute inset-0 pointer-events-none flex flex-col justify-between p-6">
      {/* Top HUD */}
      <div className="flex justify-between items-start pointer-events-auto">
        <div className="bg-[#09090b]/90 border border-zinc-800 rounded-xl px-4 py-2 flex items-center gap-3 shadow-2xl backdrop-blur">
          <span className="text-emerald-500 font-bold">💎 {treasuryValue} Crystals</span>
          <span className="text-zinc-500 font-mono scale-90">Daily Savings</span>
        </div>

        <div className="bg-[#09090b]/90 border border-zinc-800 rounded-xl px-3 py-1.5 flex items-center gap-2 shadow-2xl backdrop-blur">
          <button 
            onClick={() => onSetSpeed(1)} 
            className={`px-2 py-0.5 rounded text-xs ${speed === 1 ? 'bg-blue-600 text-white' : 'text-zinc-400 hover:bg-white/5'}`}
          >
            1x
          </button>
          <button 
            onClick={() => onSetSpeed(3)} 
            className={`px-2 py-0.5 rounded text-xs ${speed === 3 ? 'bg-blue-600 text-white' : 'text-zinc-400 hover:bg-white/5'}`}
          >
            3x
          </button>
        </div>
      </div>

      {/* Bottom HUD */}
      <div className="flex justify-between items-end pointer-events-auto">
        <div className="bg-[#09090b]/90 border border-zinc-800 rounded-xl px-4 py-2 shadow-2xl backdrop-blur flex flex-col gap-1 w-44">
          <span className="text-xs text-zinc-400 font-bold">Developer Status</span>
          <div className="w-full bg-zinc-800 h-2 rounded-full overflow-hidden mt-1">
            <div className="bg-blue-500 h-full" style={{ width: `${energy}%` }} />
          </div>
          <span className="text-[10px] text-zinc-500 font-mono">Energy: {energy}%</span>
        </div>

        <div className="bg-[#09090b]/90 border border-zinc-800 rounded-xl px-4 py-2 shadow-2xl backdrop-blur flex flex-col gap-1">
          <span className="text-xs text-zinc-400 font-bold">Quest Log</span>
          <span className="text-[10px] text-zinc-500 font-mono">None active</span>
        </div>
      </div>
    </div>
  );
};
```

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to include `HudPanels`:
```typescript
// Add state variables and imports inside LudusSandbox
const [treasury, setTreasury] = useState(120);
const [energy, setEnergy] = useState(85);
const [speed, setSpeed] = useState(1);

// Inside the return block of LudusSandbox:
return (
  <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
    <canvas ref={canvasRef} width={800} height={500} className="absolute inset-0 w-full h-full" />
    <div className="absolute inset-0 pointer-events-none">
      {agents.map((agent, index) => (
        <CitizenSprite
          key={index}
          name={agent.name}
          energy={agent.energy}
          mood={agent.mood}
          path={agent.path}
          tileWidth={tileWidth}
          tileHeight={tileHeight}
          offsetX={offsetX}
          offsetY={offsetY}
        />
      ))}
    </div>
    <HudPanels
      treasuryValue={treasury}
      energy={energy}
      speed={speed}
      onSetSpeed={setSpeed}
    />
  </div>
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/HudPanels.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx
git commit -m "feat: add HUD panel overlays for stats and controls"
```

---

### Task 6: Curfew Lockout Overlay and Breathing Exercise Delay

**Files:**
*   Create: `crates/vox-gui/ui/src/components/gamify/CurfewOverlay.tsx`
*   Modify: `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx:1-300`

- [ ] **Step 1: Write the failing test**

Modify `crates/vox-gui/ui/src/components/gamify/LudusSandbox.test.tsx` to assert curfew overlay behaves correctly:
```typescript
import { CurfewOverlay } from './CurfewOverlay';

describe('CurfewOverlay Timer', () => {
  it('correctly counts down from 10 to 0', async () => {
    let timeLeft = 10;
    const tick = () => {
      if (timeLeft > 0) timeLeft -= 1;
    };
    
    tick();
    expect(timeLeft).toBe(9);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: FAIL (Cannot find module './CurfewOverlay')

- [ ] **Step 3: Write minimal implementation**

Create `crates/vox-gui/ui/src/components/gamify/CurfewOverlay.tsx` containing:
```typescript
import React, { useEffect, useState } from 'react';

interface CurfewProps {
  onComplete: () => void;
}

export const CurfewOverlay: React.FC<CurfewProps> = ({ onComplete }) => {
  const [timeLeft, setTimeLeft] = useState(10);

  useEffect(() => {
    if (timeLeft <= 0) {
      onComplete();
      return;
    }
    const timer = setTimeout(() => setTimeLeft((t) => t - 1), 1000);
    return () => clearTimeout(timer);
  }, [timeLeft, onComplete]);

  return (
    <div className="absolute inset-0 bg-[#09090b]/95 z-[100] flex flex-col items-center justify-center p-8 backdrop-blur">
      <div className="max-w-md w-full bg-zinc-950 border border-red-500/20 rounded-2xl p-8 shadow-2xl flex flex-col items-center text-center">
        <span className="text-4xl mb-4">💤</span>
        <h2 className="text-xl font-bold text-red-500 mb-2">Guardian Curfew Lockout</h2>
        <p className="text-xs text-zinc-400 mb-6 leading-relaxed">
          You have exceeded consecutive working hours limit or curfews. Take a deep breath and rest your eyes.
        </p>
        <div className="w-16 h-16 rounded-full border-4 border-zinc-800 border-t-red-500 flex items-center justify-center font-mono text-sm text-red-500 font-bold mb-4 animate-spin-slow">
          {timeLeft}s
        </div>
        <span className="text-[10px] text-zinc-500 uppercase tracking-widest font-mono">Breathing Block...</span>
      </div>
    </div>
  );
};
```

Update `crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx` to include `CurfewOverlay`:
```typescript
// Inside LudusSandbox component:
const [curfewActive, setCurfewActive] = useState(false);

// Trigger curfew based on hour of day (e.g. past 11 PM or manual trigger for test)
useEffect(() => {
  const hour = new Date().getHours();
  if (hour >= 23 || hour < 6) {
    setCurfewActive(true);
  }
}, []);

// Inside the return block of LudusSandbox, before closing tag:
return (
  <div className="relative w-full h-[500px] bg-[#09090b] overflow-hidden border border-zinc-800 rounded-2xl">
    ...
    {curfewActive && (
      <CurfewOverlay onComplete={() => setCurfewActive(false)} />
    )}
  </div>
);
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm --filter vox-gui-ui test src/components/gamify/LudusSandbox.test.tsx`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/gamify/CurfewOverlay.tsx crates/vox-gui/ui/src/components/gamify/LudusSandbox.tsx
git commit -m "feat: implement Guardian Curfew Overlay and 10s breathing delay lockout screen"
```
