# Implementation Plan: Claude Design Integration (Tauri/TSX)

## 1. Goal
Integrate the Claude Design assets into the existing Tauri/React/TSX pipeline in `crates/vox-gui`. This involves converting raw JSX into typed TSX components and wiring them to the real Vox orchestrator backend.

## 2. Component Architecture
We will break down the monolith JSX files into a structured component library:

### UI Primitives (`src/components/ui/`)
- [ ] `Glass.tsx`: Glassmorphic container.
- [ ] `Pill.tsx`: Status indicator with pulse animation.
- [ ] `Sparkline.tsx`: SVG-based telemetry sparklines.
- [ ] `Icons.tsx`: Unified SVG icon set.
- [ ] `Backdrop.tsx`: Arcane background with grid and scanlines.

### Layout & Navigation (`src/components/layout/`)
- [ ] `Sidebar.tsx`: Collapsible 3-mode sidebar.
- [ ] `TopHud.tsx`: KPI-dense top bar.
- [ ] `CommandPalette.tsx`: ⌘K command interface.
- [ ] `Toasts.tsx`: Notification queue.

### Surfaces (`src/components/surfaces/`)
- [ ] `Dashboard/`: Stream, Ludus alerts, and Active Agent rail.
- [ ] `Loquela/`: Modern terminal with context chips.
- [ ] `Catalog/`: Skill deployment grid.
- [ ] `Flow/`: Visual agent graph (TBD integration with existing AgentFlow).
- [ ] `Matrix/`: Intention Matrix / Policy view.

## 3. Data Wiring (Tauri + transport.ts)
- [ ] **State Management**: Update `App.tsx` to hold the authoritative orchestrator state.
- [ ] **Polling**: Enhance the existing `get_orchestrator_status` loop to match the density of the new design (peers, VRAM, budget sparklines).
- [ ] **Actions**: Ensure `voxTransport` handles:
  - `vox_submit_task`
  - `vox_pause_agent` / `vox_resume_agent`
  - `vox_doubt_task` / `vox_overrule_task`
  - `vox_gamify_notification_ack`

## 4. Execution Steps
- [ ] **Step 1: UI Foundation**
  - Port `ui.jsx` primitives to TSX.
  - Update `index.css` with the new design tokens (zinc-950, brass, amber-glow).
- [ ] **Step 2: App Shell**
  - Port `Sidebar`, `TopHud`, and `CommandPalette`.
  - Update `App.tsx` to use the new layout.
- [ ] **Step 3: Feature Surfaces**
  - Port `Dashboard` and `StreamCard`.
  - Port `Loquela` (Speak surface).
  - Port `Catalog` and `Matrix`.
- [ ] **Step 4: Live Wiring**
  - Replace all mock `setTimeout` loops in the components with real event listeners or polling data.
  - Wire `voxTransport` calls to buttons and inputs.

## 5. Verification
- `npm run build` in `crates/vox-gui/ui` must pass without type errors.
- `cargo tauri dev` to verify the visual integrity and backend connectivity.
