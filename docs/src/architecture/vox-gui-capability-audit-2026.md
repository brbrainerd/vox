---
title: "Vox GUI Capability Audit 2026"
description: "Reality audit of the Vox GUI, CLI-driven discoverability, Tauri/mobile compatibility, and the path toward a maintainable code harness."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-28"
training_eligible: true
training_rationale: "Grounded repository audit for GUI, CLI, and runtime-surface architecture."
---

# Vox GUI Capability Audit 2026

> **Successor.** This audit diagnoses the GUI's state. The executable, TDD-ready
> build-out sequence derived from it lives in
> [`vox-gui-harness-buildout-plan-2026.md`](vox-gui-harness-buildout-plan-2026.md)
> (three tracks: stateful core, CLI-derived surface, design/UX). The
> `vox-dashboard` → `vox-gui` migration called out in §"Phase 0" below is now
> complete; the dangling references it flagged have been reconciled.

## Executive verdict

Vox already has the right maintainability seed: the CLI has a real compiled command
catalog, and the desktop GUI can read that catalog through Tauri. A fresh local
probe on 2026-05-28 returned **472 nested CLI catalog entries** from:

```powershell
target\debug\vox.exe commands --format json --include-nested
```

That is enough to make the GUI discoverable by default, but not enough to make it
safe or ergonomic as a code harness. The current catalog tells the GUI what
commands exist; it does not yet describe command risk, typed argument forms,
output schemas, streaming behavior, long-running task semantics, required
capabilities, mobile suitability, or whether a command can be previewed before
execution.

The desktop shell in [`crates/vox-gui`](../../../crates/vox-gui/) is real Tauri
2 infrastructure. Several panels inside the React UI are still product mockups or
fixture-backed views. The near-term goal should be a **CLI-shaped dashboard**:
use compiled CLI metadata as the discoverability spine, then layer curated
operator workflows on top for runs, models, repositories, memory, safety, and
MENS training.

## Scope and evidence

This audit inspected live repository files under the Vox workspace and did not
read `archive/` or `docs/src/archive/`, per the repository archival protocol.

Primary surfaces inspected:

- [`crates/vox-gui`](../../../crates/vox-gui/) for the Tauri desktop shell and
  React UI.
- [`crates/vox-cli`](../../../crates/vox-cli/) for the Clap root command,
  command catalog, dynamic registry model, and GUI CI checks.
- [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml)
  for operation metadata.
- [`contracts/frontend/surface-ownership.v1.yaml`](../../../contracts/frontend/surface-ownership.v1.yaml)
  for declared frontend ownership.
- [`clients/runtime-types`](../../../clients/runtime-types/) and
  [`clients/runtime-web`](../../../clients/runtime-web/) for the emerging
  platform runtime contract.
- Current architecture docs for dashboard, Tauri, application packaging, and
  mobile direction.

Two targeted local checks were run:

| Check | Result | Meaning |
| --- | --- | --- |
| `target\debug\vox.exe commands --format json --include-nested` | 472 entries | The compiled CLI catalog is real and large enough to drive discovery. |
| `target\debug\vox.exe ci gui-catalog-parity` | Failed | Guard caught `tauri.conf.json` version `0.5.0` while workspace version is `0.6.0`. |

## What is real

| Surface | Current reality | Maintainability implication |
| --- | --- | --- |
| Desktop shell | [`crates/vox-gui/Cargo.toml`](../../../crates/vox-gui/Cargo.toml) is a real Rust crate with Tauri 2, `vox-cli`, `vox-cli-core`, `vox-orchestrator`, `vox-db`, `vox-secrets`, and related dependencies. | The desktop app is not merely a design mockup. It can be treated as an application surface that deserves CI, version parity, and release discipline. |
| Tauri command registration | [`crates/vox-gui/src/main.rs`](../../../crates/vox-gui/src/main.rs) registers real commands for command catalog, command execution, initial view, orchestrator status, registry metadata, and model/routing views. | There is a real IPC boundary. The next step is to harden command contracts rather than replace the architecture. |
| CLI command catalog | [`crates/vox-cli/src/command_catalog.rs`](../../../crates/vox-cli/src/command_catalog.rs) derives command entries from the compiled Clap root command. | This is the best current source for discoverability because it reflects compiled CLI reality. |
| `vox commands` surface | [`crates/vox-cli/src/lib.rs`](../../../crates/vox-cli/src/lib.rs) exposes the command catalog and advertises `vox gui` as a discovery surface when the GUI feature is enabled. | CLI-first GUI generation is aligned with existing CLI intent, not an external invention. |
| Operation registry metadata | [`contracts/cli/command-registry.yaml`](../../../contracts/cli/command-registry.yaml) adds product lanes, status, groups, feature gates, and handler metadata. | Useful as a metadata overlay, but it must be checked against compiled Clap reality to avoid drift. |
| Sidecar execution | [`crates/vox-gui/src/commands/execute.rs`](../../../crates/vox-gui/src/commands/execute.rs) launches the `vox` sidecar through Tauri shell APIs. | Simple command execution works as a foundation, but argument modeling is too weak for robust command forms. |
| Orchestrator status | [`crates/vox-gui/src/commands/orchestrator.rs`](../../../crates/vox-gui/src/commands/orchestrator.rs) returns JSON and MessagePack status snapshots. | The dashboard can reflect real orchestration state, but polling rebuilds heavy state and should become daemon/event-stream backed. |
| React app shell | [`crates/vox-gui/ui/src/App.tsx`](../../../crates/vox-gui/ui/src/App.tsx) loads the command catalog, decodes orchestrator status, and routes among dashboard/catalog/memory/models/runs/settings views. | The UI has a real application shell and can become the primary operator cockpit. |
| Model surface | The current GUI includes model/routing Rust and React surfaces under `crates/vox-gui`, including active model and routing summary APIs. | This is one of the strongest candidates for a first-class, non-generic dashboard workflow. |
| GUI CI hooks | `vox ci gui-catalog-parity` and `vox ci gui-smoke` exist under the CLI CI commands. | The project already has places to enforce GUI drift, but several expensive checks remain opt-in. |
| Runtime contract | [`clients/runtime-types/src/index.ts`](../../../clients/runtime-types/src/index.ts) defines a portable `VoxRuntime` interface, with a Tauri-backed implementation in [`clients/runtime-web`](../../../clients/runtime-web/). | This is the right seam for desktop/mobile parity if the mobile direction is resolved consistently. |

## What is scaffolded, mocked, or drifting

| Surface | Current reality | Risk |
| --- | --- | --- |
| Dashboard crate naming | `contracts/frontend/surface-ownership.v1.yaml` names `crates/vox-dashboard` as canonical, while the actual live shell is `crates/vox-gui`. | Documentation and ownership drift make future work harder to route. |
| Older dashboard docs | Some dashboard docs still describe an Axum SPA or `vox-dashboard` path. | Contributors can easily build against the wrong surface. |
| Tauri version parity | `crates/vox-gui/tauri.conf.json` reports `0.5.0`; workspace package version is `0.6.0`. The parity gate fails today. | Release metadata is already drifting. |
| Unregistered GUI commands | `memory.rs`, `ludus.rs`, and `preferences.rs` exist under `crates/vox-gui/src/commands/` but are not registered in `commands/mod.rs` or `main.rs`. | These files read like features but are not live IPC capabilities. |
| Missing backend dependencies for dead commands | Some unregistered command files reference crates that are not present in the GUI crate dependencies. | Registering them naively may fail compilation or reintroduce retired surfaces. |
| Memory view | The React memory view includes sample hits and fallbacks; the intended `mnemosyne_recall` path does not line up cleanly with the registered Tauri command set. | Users may mistake a demonstration panel for a real memory system. |
| Catalog view | The GUI labels command entries as "Skills" and offers "Deploy" behavior even when a command is just a CLI operation requiring arguments. | This blurs command discovery, skills, and execution into one concept. |
| Loquela composer | The composer UI has slash commands, tier controls, and mic affordances, but most are local UI state or optimistic command dispatch. | It looks like an agent harness before the run/task backend is fully present. |
| Settings view | Mesh peers, signing keys, theme, telemetry, and keybinding controls are partly local state or static data. | Settings can imply persistent policy that is not actually enforced. |
| Runs view | The current runs surface can show recent orchestrator activity and model scoreboards, but it does not yet sit on a canonical run/task store. | A code harness needs persistent, inspectable, replayable runs. |
| Mobile direction | Older Tauri convergence docs and newer React Native/Expo runtime-contract docs do not fully agree. | Mobile architecture decisions can split runtime abstractions if not reconciled. |

## CLI-driven GUI feasibility

Structuring the GUI around the CLI is the right maintainability direction, but
the GUI should not be a raw `vox --help` renderer. The durable pattern is:

1. Treat the compiled Clap catalog as the truth for command existence.
2. Merge operation-registry metadata for product lane, maturity, capability, and
   handler provenance.
3. Add a richer action schema for GUI execution and mobile filtering.
4. Build generated command forms and command-palette entries from that schema.
5. Keep first-class dashboard panels for workflows that need live state, event
   streams, timelines, approvals, or visual comparison.

The current `command_catalog` schema should be extended or paired with a
versioned GUI action manifest containing:

- Typed argument shape: flags, options, positionals, repeated values, enums,
  defaults, conflicts, dependencies, and examples.
- Execution semantics: read-only vs mutating, destructive risk, required
  confirmation, dry-run availability, network/filesystem/process access, and
  expected duration.
- Output contract: human text, JSON, MessagePack, streamed events, artifacts,
  diagnostics, run ids, and machine-readable schemas.
- Capability metadata: required secrets, local tools, daemon availability,
  workspace state, feature gates, and mobile suitability.
- UX hints: recommended grouping, common tasks, form layout, empty-state copy,
  and whether the command belongs in the global command palette only or deserves
  a full panel.

This lets the dashboard be generated where that is safe, while preserving custom
interfaces where the CLI is only one part of the interaction.

## Recommended product shape

The GUI should converge on five primary surfaces:

| Surface | Purpose | Data source |
| --- | --- | --- |
| Command Center | Search, inspect, and run CLI operations with generated forms. | Compiled CLI catalog plus GUI action schema. |
| Runs | Persistent task/run timeline with status, logs, artifacts, model decisions, approvals, and replay. | VoxDb run store plus daemon event stream. |
| Models | Model registry, active model, routing policy, scoreboards, cost, capability fit, and local/remote availability. | Orchestrator model registry, routing contracts, secrets readiness. |
| Repositories | Workspace/worktree health, diffs, tests, CI tiers, docs drift, and code harness entrypoints. | `vox repo`, `vox ci`, git metadata, diagnostics, artifacts. |
| Memory and Knowledge | Search, corpus, symbols, documentation, MENS training data, and retrieval provenance. | `vox-search`, VoxDb, docs contracts, training manifests. |

Everything else should be secondary navigation or command-palette driven until it
has a real backend contract.

## Tauri and mobile compatibility

The desktop app should remain Tauri 2. The existing shell is real, and Tauri is a
good fit for local repository work, sidecar CLI execution, file access,
notifications, and native packaging.

Mobile needs a clearer architectural decision. Current evidence points to a
portable runtime contract as the right abstraction:

- Keep desktop shell APIs behind `VoxRuntime`.
- Lower mobile UI against `@vox/runtime` rather than direct `@tauri-apps/api/*`
  imports.
- Let the mobile implementation use the selected mobile stack while preserving
  the same workflow, model, inference, notification, speech, and actor/workflow
  primitives.

The unresolved documentation conflict is important: older packaging/Tauri docs
describe Tauri for desktop and mobile, while newer mobile architecture work
leans toward React Native/Expo for mobile and Tauri for desktop. Until that is
ratified, new GUI code should depend on the runtime contract and avoid direct
mobile-only assumptions.

## Work required to reach the goal

### Phase 0: Reconcile reality and docs

- Rename or clearly alias `vox-dashboard` documentation to the real
  `crates/vox-gui` surface, or intentionally create a separate dashboard crate.
- Fix Tauri version parity so `vox ci gui-catalog-parity` passes.
- Mark unregistered GUI command files as scaffold, wire them correctly, or remove
  them from the live GUI crate.
- Replace Latin or opaque user-facing navigation labels with plain product
  language.
- Decide the mobile architecture statement: Tauri mobile, React Native/Expo, or
  runtime-contract-first with explicit adapters.

### Phase 1: Make CLI metadata GUI-grade

- Extend the command catalog or add a `contracts/gui/action-manifest.v1.yaml`
  generated from Clap plus hand-authored metadata.
- Add parity checks that every GUI-executable command has typed arguments,
  safety metadata, and an output contract.
- Generate TypeScript command/action types for the React UI.
- Teach the GUI to render forms from the action schema, not from ad hoc object
  key conversion.

### Phase 2: Build the run/task backbone

- Introduce a canonical run store in VoxDb: run id, command, repo, worktree,
  model, cost/tokens, logs, artifacts, diagnostics, approvals, and status.
- Route long-running CLI/orchestrator work through a daemon or job service rather
  than blocking sidecar calls.
- Stream events to the GUI instead of rebuilding orchestrator status on a 500 ms
  polling loop.
- Make Loquela create real runs and attach messages, files, model routing, and
  approvals to those runs.

### Phase 3: Promote real panels

- Keep Models as a first-class surface and connect active model changes to
  persistent config or secrets policy rather than process-local environment
  mutation.
- Replace Memory fixtures with registered search/retrieval commands and clear
  provenance.
- Make Runs the center of the code harness, with reproducible command invocations
  and artifact replay.
- Add Repository health panels backed by `vox ci`, `vox check`, `vox repair`,
  docs drift checks, and git/worktree state.

### Phase 4: Harden compatibility and CI

- Make GUI TypeScript build part of the default fast or complete local gate once
  cost is acceptable.
- Add an IPC mock layer so Playwright can test the React UI outside Tauri without
  silently falling back to fixtures.
- Keep `gui-catalog-parity` mandatory for version drift and command catalog
  health.
- Add mobile runtime-contract tests that prove generated UI code does not import
  desktop-only Tauri APIs directly.

## Bottom line

The best path is not to hand-design every dashboard feature and not to blindly
generate the whole app from the CLI. Vox should make the CLI catalog the
discoverability backbone, enrich it with GUI-grade action metadata, and reserve
custom UI for the workflows that become a real code harness: runs, repositories,
models, memory, approvals, and MENS training.

That gives Vox a maintainable center of gravity: develop capabilities in the CLI
and daemon, surface them automatically in the GUI, then promote the highest-value
operations into rich panels only after their backend contracts are real.
