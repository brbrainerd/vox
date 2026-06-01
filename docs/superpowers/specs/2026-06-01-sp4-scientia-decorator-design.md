# SP-4: Scientia Decorator + Decorator Registry — Design

**Date:** 2026-06-01
**Status:** Approved (build all next steps)
**Umbrella:** [`2026-06-01-cli-gui-hybrid-spine-design.md`](2026-06-01-cli-gui-hybrid-spine-design.md) (Unit 2 decorator seam + Unit 4 Scientia)
**Depends on:** SP-1, SP-2, SP-3 — landed.

## Scope decision

The umbrella SP-4 had the Scientia decorator consume Scientia's **Phase H `dashboard` JSON** — which is
planned-but-unbuilt (`vox-scientia::dashboard` is a stub). Per the no-stubs rule, SP-4 instead ships a
**real decorator against Scientia read commands that exist today**, and is upgradeable to Phase H JSON
later without changing the registry seam.

This is also where the **decorator registry** lands (it was moved out of SP-2 because it had no consumer
then). Now it has one: `ScientiaDashboard`.

## Goal

A first-class `scientia` surface in the GUI: a hand-built dashboard that runs real Scientia read commands
through the shared execute path and renders their results — plus a decorator registry that lets any
surface key override the generated view, formalizing the seam SP-2 deferred.

## Design

### Decorator registry (the seam)

New file `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`:

```ts
import type React from 'react';
import { ScientiaDashboard } from './Scientia/ScientiaDashboard';

export interface SurfaceDecoratorProps {
  pushToast: (item: { tone: 'ok' | 'warn' | 'info'; title: string; body?: string }) => void;
}

/** Surface key → custom view that replaces the default for that surface. */
export const surfaceDecorators: Record<string, React.ComponentType<SurfaceDecoratorProps>> = {
  scientia: ScientiaDashboard,
};
```

`App.tsx::renderView` consults the registry **before** its `switch`, so registering a decorator needs no
edit to the switch body:

```tsx
const Decorator = surfaceDecorators[activeView];
if (Decorator) return <Decorator pushToast={pushToast} />;
switch (activeView) { /* unchanged generated/built-in views */ }
```

This makes the registry genuinely load-bearing (used on every render), not a hollow indirection: it is the
single place a surface is promoted from generated/built-in to decorated. Removing the `scientia` entry
reverts to the default with no other change.

### ScientiaDashboard (the decorator)

New file `crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaDashboard.tsx`. Mirrors the existing
`GamifyView` decorator pattern (`invoke('execute_command', { path, args })`) and the `ModelsView`
load/refresh pattern. On mount and on Refresh, it runs three **read-only** Scientia commands concurrently
and renders each in a card (exit code + stdout/stderr, errors surfaced):

| Card | Command | Why |
| --- | --- | --- |
| Retrieval Status | `vox scientia retrieval-status` | research ingest / retrieval readiness |
| Publication Discovery Queue | `vox scientia publication-discovery-scan` | candidate publications awaiting routing |
| Capability Map | `vox scientia capability-list` | registered research capabilities |

All three are arg-free reads delegating to `vox db` handlers (verified in
`crates/vox-cli/src/commands/scientia.rs`). The component handles empty/error output gracefully (shows the
stderr / error string) so it is honest when there is no research data yet.

### runAction contract

The decorator routes every command through `invoke('execute_command', …)` — the same single Tauri execute
path the generated panels and `GamifyView` use. It does not call the sidecar or Tauri APIs by any other
route. This satisfies the umbrella's "decorators must use the shared execute path" rule, so when Scientia
commands are later added to the SP-3 reward map they will earn rewards through this surface automatically.

### Wiring

- `App.tsx`: add `'scientia'` to the `View` union; import `surfaceDecorators`; front `renderView` with the
  registry check.
- `Sidebar.tsx`: add a `Scientia` nav item (`view === 'scientia'`).

## Non-goals / known limits

- **No Phase H JSON consumption.** When `vox-scientia::dashboard` lands, the cards can be replaced by a
  parsed structured view; the registry seam and surface stay the same. Documented as the upgrade path.
- **No JSON parsing of command output.** Output is rendered as text (like `GamifyView`); the read commands
  emit human tables today. Structured parsing waits for a `--json` contract or Phase H.
- **Scientia commands are not yet in the SP-3 reward map** (they are not fabrica lanes). Wiring Scientia
  rewards is a separate slice; the decorator is built so it would benefit automatically.
- No backend/Rust changes; this is GUI-only.

## Verification

- `pnpm --dir crates/vox-gui/ui build` succeeds (the repo's lint/verification bar).
- Manual review: registry is consulted before the switch; ScientiaDashboard routes solely through
  `execute_command`; Sidebar entry toggles the surface.
