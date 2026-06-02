# CLI→GUI Hybrid Spine — Design

**Date:** 2026-06-01
**Status:** Approved (brainstorming)
**Companion audit:** [`docs/src/architecture/cli-gui-surface-coverage-map-2026.md`](../../src/architecture/cli-gui-surface-coverage-map-2026.md)
**Builds on:** [`docs/src/architecture/vox-gui-capability-audit-2026.md`](../../src/architecture/vox-gui-capability-audit-2026.md) (this realizes its Phase 1)

## Problem

The Vox GUI has a real but thin CLI→GUI derivation spine: `command_catalog::build_catalog()`
introspects the clap tree, `contracts/operations/catalog.v1.yaml` overlays metadata, and the
Tauri GUI renders command *metadata* plus ~23 hand-written invoke handlers. It surfaces
commands but does not generate ergonomic, typed panels.

The result is the coverage gap mapped in the companion audit:

- **Tier 1** (real panels): orchestrator, model, memory, catalog, runs, settings.
- **Tier 2** (~90% of 472 catalog entries): discoverable + runnable via generic shell-out, but
  no typed form, no panel.
- **Tier 3** (CLI-rich, GUI-absent or fixture): **`scientia`** (~40 subcommands, zero GUI),
  **`ludus`** (full `vox-gamify` engine, GamifyView exists but renders fixtures — `ludus.rs`
  unregistered), plus `mens`, `populi`, `oratio`, `schola`, `research`, `visus`, `safety`.

These are not three unrelated TODOs. They are one system: a derivation layer that generates
panels from CLI metadata, a cross-cutting event bus (Ludus already ingests events from every
surface via `route_event()`), and Scientia as the first rich surface promoted on top.

## Goal

"Derive the GUI from the CLI and enhance it." Concretely: every surface gets a generated panel
by default (derive); high-value surfaces register a decorator that takes over rendering
(enhance); every execution flows through one path that emits a canonical event so gamification
is structural, not per-surface.

## Chosen architecture — Hybrid spine

Four cooperating units, one-way dependencies (nothing below points up):

```
GUI (React surfaces)
  • Generated panel renderer  ── reads ActionManifest v2
  • Decorator registry        ── overrides per surface
  • GamifyView / LudusBanner  ── reads Ludus projection
        ▲ Tauri IPC                       ▲ Tauri IPC
Action spine (vox-cli)            Ludus bus (vox-gamify)
  • build_catalog (clap)            • route_event()  (exists)
  • ActionManifest v2 (typed)       • LudusProjection (read)
  • execute() — single path  ──────▶ emits ActionEvent
```

**Key invariant:** every command execution flows through *one* `execute()` function. That single
seam is what lets derivation (manifest in) and gamification (event out) both be universal instead
of per-surface.

### Unit 1 — Action spine (`vox-cli`)

Extends the existing `command_catalog` + `action_manifest`. Owns the typed `ActionManifest v2`.
Derived from clap (arg kinds) + a thin operations-YAML overlay (the things clap can't know:
safety class, expected duration, output schema ref). Per executable command:

```jsonc
{
  "surface": "scientia",              // top-level enum key — decorator + Ludus routing key
  "path": ["scientia", "publication-preflight"],
  "args": [
    { "name": "candidate-id", "kind": "string", "required": true, "help": "..." },
    { "name": "venue", "kind": "enum", "values": ["zenodo","openreview","arxiv"], "required": false },
    { "name": "dry-run", "kind": "flag", "default": false }
  ],
  "safety": { "class": "read|mutate|destructive", "reversible": true, "confirm": false },
  "output": { "format": "json", "schema_ref": "contracts/scientia/preflight-result.v1.schema.json", "streamed": false },
  "duration": "fast|slow|long-running"
}
```

- New `vox commands --format action-manifest`; the existing Tauri `get_action_manifest` consumes it.
- New `ci action-manifest-parity` guard: a command lacking a safety class or output contract is a
  hard error (same drift discipline the catalog already enforces).
- Generate TypeScript types from the manifest schema so the renderer is type-safe.
- **YAGNI:** do *not* model arg conflicts/dependencies in v2. Clap enforces those at run; the form
  may over-permit and let the CLI reject. Add only when a real surface needs it.

### Unit 2 — Generated panel renderer + decorator registry (GUI)

Generated renderer: given a manifest entry, render form + run button + output view. Zero hand-coding
per command — this is what makes all Tier-3 surfaces appear at once.

Decorator registry, checked once at render:

```ts
const decorators: Record<string, SurfaceDecorator> = {
  scientia: ScientiaDashboard,   // custom UI
  ludus:    GamifyView,          // already exists — just register it
};

function renderSurface(surface, manifest, runAction) {
  const Decorator = decorators[surface];
  return Decorator
    ? <Decorator manifest={manifest} runAction={runAction} />   // enhanced
    : <GeneratedPanel manifest={manifest} runAction={runAction} />; // derived skeleton
}
```

**`runAction` contract (load-bearing):** a decorator is handed `runAction` — the *same* bound
`execute()` call the generated panel uses. A decorator may build any bespoke UI but cannot bypass
the single execute path. This guarantees Ludus events fire even from hand-crafted panels. CI lints
against any decorator calling Tauri execute directly.

- Decoration is per-*surface*, not per-command. A decorator owns its whole panel and chooses which
  commands to render richly vs. leave as generated sub-forms.
- Fallback is always live: registering a decorator is optional and removable; deleting it reverts to
  the generated panel with no other change. This kills the "fixture rots into fake feature" failure
  the capability audit flagged.

### Unit 3 — Ludus bus (`vox-gamify`)

The single `execute()` path emits one canonical event after every run:

```rust
let event = ActionEvent {
    surface, command_path,
    outcome: Outcome::Ok | Outcome::Err,
    duration_ms,
    dedupe_id,   // surface+path+timestamp — satisfies route_event's existing contract
};
vox_gamify::event_router::route_event(event);   // exists; we feed it
```

- Reuses the built engine (reward policy, grind caps, trust tiers, quests). We connect, not build.
- Non-blocking, never gates (per `ludus-non-goals.md`); behind the existing `gamify_enabled` config;
  fire-and-forget; a bus failure can never fail a command.
- Read path: register the existing-but-unregistered `ludus.rs` as `get_ludus_projection`
  (XP/level/streak/active-quest). `GamifyView`/`LudusBanner` swap fixtures for it — converting Ludus
  from Tier-3 fixture to Tier-1 live with one registration.
- Map `ActionEvent` outcomes onto the existing `agent-event-kind-ludus-matrix.md` reward table.

### Unit 4 — Scientia decorator (worked example)

`ScientiaDashboard`, registered for surface `scientia`:

- Consumes Scientia's planned Phase H `dashboard` JSON (pipeline stages, candidate queue, worthiness
  verdicts, approval state). The CLI was already built to emit GUI-shaped data; the decorator is the
  missing consumer.
- Hand-crafts the pipeline view + dual-approval flow (high-value, stateful); the long tail of
  `scientia *` validation/utility subcommands render as generated sub-forms — no extra code.
- Every action routes through `runAction`, so `vox scientia publication-approve` from the rich UI
  emits a Ludus event for free (approving a publication can earn lumens). The association made literal.

## Data flow

1. GUI loads `ActionManifest v2` via Tauri (`get_action_manifest`).
2. For a surface, registry returns decorator or generated panel.
3. User submits a form → `runAction(path, args)` → spine `execute()` → `vox` sidecar.
4. `execute()` returns output (rendered) AND emits `ActionEvent` → `route_event()`.
5. GUI polls/reads `get_ludus_projection` → `GamifyView`/`LudusBanner` update.

## Error handling

- **Manifest drift** → `ci action-manifest-parity` fails the build.
- **Command failure** → surfaced in output view with exit code + stderr; emits `Outcome::Err`
  (reward policy already handles: no reward, streak-preserving per policy).
- **Bus failure** → swallowed + logged; never propagates to the command result.
- **Decorator bypass** → CI lint flags decorators calling Tauri execute directly instead of `runAction`.

## Testing (each unit isolated)

| Unit | Test | Independent of |
|---|---|---|
| ActionManifest v2 | snapshot: clap tree + fixture YAML → expected typed JSON | GUI, Ludus |
| Generated renderer | fixture manifest entry → rendered form (RTL) | real CLI, Tauri |
| Decorator registry | lookup returns decorator/fallback | everything else |
| Ludus bus | `ActionEvent` in → `LudusProjection` delta out | GUI |
| Scientia decorator | fixture Phase H JSON → rendered dashboard | live pipeline |
| End-to-end | one real `execute()` → event observed in projection | — |

## Decomposition (dependency order)

- **SP-1 — Action spine.** `ActionManifest v2` + generator + `ci action-manifest-parity` + TS types.
  Foundation; everything depends on it. (Capability-audit Phase 1.) **First plan-ready slice.**
- **SP-2 — Generated renderer + decorator registry seam.** Consumes SP-1; makes all Tier-3 surfaces
  appear as generated panels.
- **SP-3 — Ludus bus wiring.** `execute()` emission + register `ludus.rs` + GamifyView live. Connective;
  depends on the single execute path from SP-2.
- **SP-4 — Scientia decorator** over Phase H JSON. Flagship; depends on SP-2 + SP-3 + Scientia Phase H.

Each sub-project gets its own spec → plan → implementation cycle. This document is the umbrella SSOT.

## Non-goals

- Not modeling clap arg conflicts/dependencies in the manifest (YAGNI; clap enforces at run).
- Not building new gamification mechanics — `vox-gamify` is already rich; we connect it.
- Not replacing the Tauri architecture or the existing real panels (orchestrator/model/memory/runs).
- Not resolving the desktop-vs-mobile runtime question (tracked in the capability audit).
