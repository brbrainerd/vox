---
title: "Gamified Codebase Representation and Simulation Design"
description: "Specification for an isometric, hybrid Canvas-DOM simulation representing codebase architecture, developer/agent telemetry, and project health in Voxed UI."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Defines the architecture and design of the gamified codebase simulation for the Tauri desktop UI."
---

# Gamified Codebase Representation (Ludus Sandbox) Spec

This document specifies the design and architecture for the **Ludus Sandbox**, an interactive 2.5D isometric town simulation representing the workspace files, developer activity, AI agent workflows, and FinOps metrics in the Tauri desktop application.

---

## 1. Goal Description

Developers and AI agents work collaboratively on codebases, yet the execution flow and workspace health remain abstract or invisible. This feature introduces a **Sims-like Hybrid Sandbox** into Voxed UI that:
*   Visualizes codebase structure as physical buildings (factories, offices, utility stations).
*   Visualizes code quality metrics (AST complexity, warnings, errors) as environmental states (wear-and-tear, weeds, fires).
*   Visualizes active processes (human developer edits and AI subagent execution) as animated citizen characters moving between plots.
*   Gamifies wellness (curfew lockouts, break quests) and cost thrift (credits treasury) into interactive HUD panels.

---

## 2. Architecture: Hybrid Canvas-DOM Renderer

To ensure maximum performance while maintaining styling flexibility, the simulation uses a layered rendering approach:

```
+-----------------------------------------------------------+
| Overlay Layer (DOM: React, Tailwind, Framer Motion)      |
| -> Citizen Sprites, Speech Bubbles, Menus, Hover Tooltips|
+-----------------------------------------------------------+
| Base Layer (HTML5 Canvas: Context 2D)                     |
| -> Terrain Grid Tiles, Roads, Buildings, Fire Animations  |
+-----------------------------------------------------------+
```

1.  **Canvas Base Layer:** A 2D canvas renders the static elements (the grass tiles, roads, and buildings). This runs once on load or when buildings upgrade, avoiding DOM clutter.
2.  **DOM Overlay Layer:** Animated citizen characters, progress bars, chat bubbles, and menu popups are rendered as absolute-positioned HTML/React components, layered exactly over their canvas coordinate counterparts.
3.  **Coordinate Projection:** Coordinates are stored in world units `(x, y, z)`. They are projected to screen pixels `(px, py)` using the standard isometric projection matrix:
    $$\text{px} = (x - y) * \text{tileWidthHalf} + \text{offsetX}$$
    $$\text{py} = (x + y) * \text{tileHeightHalf} - (z * \text{heightScale}) + \text{offsetY}$$
4.  **Z-Ordering:** For overlay DOM elements (e.g. sprites walking behind buildings), their CSS `zIndex` is dynamically calculated based on their grid depth: `zIndex = Math.floor(x + y)`.

---

## 3. Components

### 3.1 Codebase Map Generator
*   **Coordinate Assignment:** Scans the directory tree. Each folder maps to a "District/Neighborhood" grid offset. Each file maps to a specific tile building.
*   **Building Upgrades:**
    *   *Simple file (low LoC / low AST complexity):* Renders as a small cottage or cabin.
    *   *Large/Refactored file:* Upgrades to a multi-story modern factory or high-rise.
*   **Quality Overlays:**
    *   *Compiler Warnings:* Renders overgrown grass/weeds.
    *   *Compiler Errors:* Plays a small fire sprite animation over the building.
    *   *Test Coverage:* High coverage renders white picket fences or protective energy bubbles around the plot.

### 3.2 Citizen State Machine (AI & Dev Sprites)
*   **A\* Pathfinding:** Calculates paths along roads between buildings.
*   **State Machine:**
    *   `Idle`: Wander near the Workspace Plaza or rest in the Lounge.
    *   `Commuting`: Walk along paths toward a target file.
    *   `Working`: Typing or hammer animation at the building.
    *   `Exhausted`: Slow walk speed, coffee/sleep bubble above head.

### 3.3 HUD Panels
*   **Left Corner (Roster):** Renders needs bars (Energy, Focus, Mood) for the Developer and active Agents.
*   **Top HUD (Treasury & Time):**
    *   *Treasury:* USD savings derived from estimated vs. actual tokens, shown as gold coins or crystals.
    *   *Speed Controller:* Standard play, fast-forward (3x speed when cargo builds are active), and pause buttons.
*   **Right Corner (Quest Board & Action Radial):** Lists active workspace issues and wellness quests (e.g., "15-minute screen break"). Radial menu on building click lets the user trigger a refactoring subagent or run tests.

---

## 4. Data Flow & Telemetry Bridge

```
[Rust Tauri Backend] --(Tauri Event Channel: 'ludus_sync_event')--> [React UI Layer]
       |                                                                    |
       +-> Ingests LSP edits, cargo compile logs, and subagent runs        +-> Pushes events to Sim Queue
```

1.  **Backend Telemetry Ingestion:** The Rust backend intercepts compiler logs, LSP file changes, and agent orchestrator tasks.
2.  **Tauri Event Emission:** Broadcasts a `ludus_sync_event` containing payload:
    ```typescript
    type LudusSyncEvent = 
      | { type: 'FileEdited', path: string, author: 'Developer' | string }
      | { type: 'TaskStarted', id: string, description: string, agentName: string }
      | { type: 'CompileStarted' }
      | { type: 'CompileFinished', success: boolean, errors: number, warnings: number }
      | { type: 'CostIncurred', costUsd: number, estimatedUsd: number };
    ```
3.  **UI Event Processing:** The React simulation loop consumes this queue, scheduling paths for agents, changing building health, and updating treasury variables.

---

## 5. Error Handling & Edge Cases

*   **Missing Coordinates:** If a new file is created on the fly, the map generator assigns the nearest open plot dynamically.
*   **Tauri Channel Dropouts:** If the event channel disconnects, the UI displays a "Sim Paused" status, retaining the last known state, and attempts reconnection.
*   **Token Overruns:** If actual USD cost exceeds budget caps, the HUD treasury flashes red, all subagents shift to `Exhausted` mood, and task dispatching is blocked (soft/hard lockout).

---

## 6. Verification & Testing Plan

### 6.1 Automated Tests
*   **Unit Tests (`vox-gamify/src/projection.rs`):** Verify that 3D world coordinates correctly map to 2D pixel coordinates, and that `zIndex` matches expected spatial ordering.
*   **UI Tests (`vitest`):** Mock `ludus_sync_event` payloads and assert that the React queue processes events and transitions states.

### 6.2 Manual Verification
*   Launch the Tauri app, open the visualizer, edit a file in the workspace, and verify that a worker sprite commutes to that building and plays a construction animation.
*   Run a cargo build containing warnings/errors and check that weeds or fires are spawned on the corresponding file plots.
