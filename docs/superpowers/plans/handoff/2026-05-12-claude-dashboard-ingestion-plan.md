# Implementation Plan: Integrating Claude Dashboard Assets (May 2026)

## 1. Goal
Migrate Claude-designed UI assets into the Vox repository to serve as the ground truth for the 7-surface dashboard port. 

## 2. Directory Structure
We will use two locations:
- **Archive (Durable):** `docs/src/archive/claude-design-may-2026/`
  - Purpose: Single Source of Truth for the raw assets.
  - Policy: Read-only, do not modify.
- **Working Surface (Integration):** `crates/vox-gui/ui/src/claude-dashboard/`
  - Purpose: Staging area for VUV conversion and technical wiring.

## 3. Files Mapping
| Source File | Destination | Description |
|---|---|---|
| `Vox Imperium.html` | `.../Vox Imperium.html` | Static HTML mockup |
| `ui.jsx` | `.../ui.jsx` | UI Primitive definitions |
| `topbar.jsx` | `.../topbar.jsx` | TopBar chrome |
| `loquela.jsx` | `.../loquela.jsx` | Speak surface |
| `intention.jsx` | `.../intention.jsx` | Intention Matrix (Mesh) |
| `flow.jsx" | `.../flow.jsx` | Agent Flow (Forge/Mesh) |
| `data.js" | `.../data.js` | Mock data and fixtures |
| `dashboard.jsx" | `.../dashboard.jsx` | Main dashboard layout |
| `catalog.jsx" | `.../catalog.jsx` | Command/Superpowers catalog |
| `app.jsx" | `.../app.jsx` | App shell and routing |

## 4. Execution Steps
- [ ] **Step 1: Create Directories**
  - `mkdir -p docs/src/archive/claude-design-may-2026/`
  - `mkdir -p crates/vox-gui/ui/src/claude-dashboard/`
- [ ] **Step 2: Ingest Assets**
  - Copy all 10 files from `C:\Users\Owner\Downloads\Vox gui\` to both locations.
- [ ] **Step 3: Document Landing**
  - Add entry to `docs/src/architecture/research-index.md`.
  - Add tombstone README to the archive.
- [ ] **Step 4: Update Master Plan**
  - Mark Phase 0 asset ingestion as "Landed" in `2026-05-03-vox-dashboard-claude-design-port.md`.

## 5. Verification
- Verify file presence via `ls -R`.
- Verify no lint/build regressions (since these are just assets in non-compiled paths).
