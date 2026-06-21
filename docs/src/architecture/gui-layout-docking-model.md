---
title: "GUI Layout & Docking Model"
description: "Documentation of the fixed-chrome and dockable-body layout system in the GUI."
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
