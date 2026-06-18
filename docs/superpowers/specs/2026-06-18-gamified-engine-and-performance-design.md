---
title: "Gamified Engine and Performance Design"
description: "Specification for highly optimized offscreen canvas buffering and decoupled state management (Zustand) for the 2.5D codebase simulation."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Defines rendering optimization and state decoupling patterns to scale the visualizer."
---

# Gamified Engine and Performance Spec

This document specifies the optimizations for codebase scaling and React rendering overhead in the **Ludus Sandbox** view of Voxed UI.

---

## 1. Goal Description

The visualizer must render large projects (hundreds of folders and files) and coordinate many AI agent citizen characters without lagging the host machine. The design targets:
1.  **Rendering Performance:** Panning and zooming across the codebase town must remain at a stable 60fps.
2.  **State Synchronization Performance:** Telemetry stream updates (LSP edits, agent position ticks) must update the UI without triggering global React component tree re-renders.

---

## 2. Rendering Optimization: Offscreen Canvas Buffer

To achieve hardware-accelerated rendering of the city backdrop (terrain tiles, roads, file buildings):

1.  **Buffer Canvas:** An offscreen HTML5 canvas element (`document.createElement('canvas')`) is allocated in memory.
2.  **Pre-rendering:** The static environment is drawn onto this buffer *only* when the codebase files change or when a code quality event (compiler error/warning) modifies a building's visual state.
3.  **Blit Matrix Transform:** The onscreen viewport canvas draws the visible portion of the buffer using the browser's GPU copy operation (`ctx.drawImage`) with transform offsets representing camera pan `(cameraX, cameraY)` and scale `(zoom)`.

---

## 3. UI Synchronization Optimization: Decoupled Zustand Store

To bypass React Virtual DOM diffing overhead:

```
[Telemetry Stream] ---> [Zustand Store Ticks]
                             |
                   (Ref Subscription / Direct Styles)
                             v
                  [DOM Overlay Sprite Element]
```

1.  **Zustand Coordinator Store:** A lightweight store (`vox-gamify-store`) manages positions, current path nodes, and moods of all agents/developer sprites.
2.  **DOM Subscription Ticks:** Overlay components (Citizen Sprites) do *not* read state via standard React hooks. Instead, they subscribe directly to the store on mount (`store.subscribe`) and update their HTML element's CSS `left`, `top`, and `zIndex` properties directly via standard DOM references (`ref.current.style.transform`).
3.  **Frame Rate Clamping:** Coordinate changes are read and applied in a `requestAnimationFrame` loop, ensuring styling updates are locked to the monitor's refresh rate.

---

## 4. Verification & Testing Plan

### 4.1 Automated Tests
*   **Performance Benchmark:** A test verifying that assigning 1000 files completes coordinate assignment in under 5ms.
*   **Viewport Math:** Tests verifying that panning offsets translate screen coordinates back to correct isometric tiles.

### 4.2 Manual Verification
*   Pan and zoom rapidly across a mock workspace with 500 files and verify CPU usage remains under 3% on average.
