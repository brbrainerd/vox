---
title: "Gamified Full Simulation and Wiring Design"
description: "Specification for wiring compiler, LSP, and orchestrator telemetry streams in Rust to the React/Zustand Ludus Sandbox simulation view."
category: "architecture"
status: "current"
training_eligible: true
training_rationale: "Defines event-driven bridge interfaces and pathfinding logic for the visual sandbox."
---

# Gamified Full Simulation and Wiring Spec

This document specifies the telemetry ingestion hooks and simulation mechanics for the **Ludus Sandbox** in Voxed UI.

---

## 1. Goal Description

This feature wires real-time developer operations and AI agent steps into the isometric town visualizer:
1.  **LSP Edits:** Trigger worker sprites commuting to files.
2.  **Compiler Diagnostics:** Render weeds (warnings) or structural cracks/soot (errors) on canvas buildings.
3.  **Orchestrator Actions:** Spawn active AI agent sprites during task runs.
4.  **Interaction:** Let developers clear weeds (fix warnings) or repair cracks (fix errors) via a radial building menu.

---

## 2. Ingestion Telemetry Bridge (Rust)

A thread-safe channel intercepts workspace events and broadcasts them to the Tauri webview:

1.  **Telemetry Event Schema:**
    ```rust
    #[derive(serde::Serialize, Clone)]
    pub enum TelemetryEvent {
        FileEdited { path: String, lines: usize },
        BuildFinished { success: bool, errors: usize, warnings: usize },
        AgentTaskStarted { id: String, agent_name: String, path: String },
        AgentTaskCompleted { id: String, agent_name: String },
    }
    ```
2.  **Tauri Event Publisher:** Broadcasts events to the webview as `vox://ludus-sandbox-sync` JSON payloads.

---

## 3. Simulation & Pathfinding (React/TypeScript)

1.  **A\* Pathfinding Engine:** Runs a standard grid-based A* pathfinder to navigate sprites along walk-weighted roads, treating buildings as impassable obstacles.
2.  **Visual Overlays:**
    *   *Weeds:* Renders pixel grass/weeds on plots with warning counts > 0.
    *   *Cracks:* Renders soot/cracks on plots with error counts > 0.
3.  **Radial Menu:** clicking a building overlay triggers an absolute-positioned radial button wheel, letting users dispatch fix/clippy commands to the Rust backend.

---

## 4. Verification & Testing Plan

### 4.1 Automated Tests
*   **Rust Channel Tests:** Verify that telemetry events correctly route to the broadcast receiver channel.
*   **Pathfinding Unit Tests:** Verify that A* pathfinding correctly circumvents solid buildings to find a route.

### 4.2 Manual Verification
*   Compile a file with a warning and verify that weeds appear. Trigger `cargo clippy --fix` via the radial menu and verify weeds disappear.
