---
title: Vox GUI ↔ CLI / Scientia Coverage Audit (2026-06-03)
description: Verified gap map between the Tauri GUI and the CLI/Scientia surface, with the three-part self-surfacing recommendation and pointers to the implementation plans.
category: "Architecture SSOTs"
---

# Vox GUI ↔ CLI / Scientia Coverage Audit (2026-06-03)

> Companion to [`vox-gui-capability-audit-2026.md`](./vox-gui-capability-audit-2026.md) and
> [`cli-gui-surface-coverage-map-2026.md`](./cli-gui-surface-coverage-map-2026.md). This audit was produced
> by a five-agent verified sweep of `crates/vox-gui`, `crates/vox-scientia`, `crates/vox-dei-shim`,
> `crates/vox-gamify`, and the `vox ci` GUI-coverage checks. Every claim below is anchored to a file path.

## Headline finding

The GUI represents *commands* well and *surfaces* poorly, and the asymmetry is structural:

> **Commands → form is ~90 % automatic. Surfaces → panel / nav is 100 % manual and unenforced.**

- The clap tree is reflected into a typed catalog (`crates/vox-cli/src/command_catalog.rs::build_catalog`)
  and `crates/vox-gui/ui/src/components/CommandCatalogForm.tsx` renders a typed form for *any* command. A new
  CLI subcommand is runnable in the GUI with zero GUI code.
- A new *surface* (a curated dashboard like Scientia) requires three hand-synced edits with no generator and
  no parity check: `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`, the 20 hand-written
  `<NavItem>` rows in `crates/vox-gui/ui/src/components/layout/Sidebar.tsx:88-108`, and the `renderView`
  switch in `crates/vox-gui/ui/src/App.tsx:517-588` (plus a fourth, the duplicate validation array at
  `App.tsx:230`).
- There is **no single surface-registry SSOT**. `crates/vox-cli/src/commands/ci/gui_surface_coverage.rs`
  classifies only **8 hardcoded capabilities** and `enforce_policy` hard-fails on just **4** of them
  (`gui_surface_coverage.rs:197-218`). Nothing fails the build when a new CLI group has no curated home.

## Coverage scorecard

| Dimension | Reality |
| --- | --- |
| Top-level CLI groups | ~66 (`Cli` enum, `crates/vox-cli/src/lib.rs`) |
| GUI sidebar views | ~20 (several thin proxies) |
| Scientia subcommands surfaced | ~3–4 of ~31 |
| Vox-language toolchain (`build`/`compile`/`check`/`test`/`run`/`fmt`/`lsp`/`doc`) | **No panel** — generic form only |
| AI plan family, pkg-mgmt, audit/doctor, auth/login/config | **No panels** |
| Gamification (`vox-gamify`) | Backend mature, **GUI shows a raw text dump** |
| Manifest safety metadata (`safety_class`/`confirmation_policy`/`reversible`/`scope`) | Computed, **never rendered** — used only for dispatch routing in `transport.ts` |
| Dead code | `ui/src/claude-dashboard/*` + 7 orphaned top-level components, unreachable from `App.tsx` |

## Scientia is two pipelines, both mostly CLI-only

**Pipeline A — deep-research runtime** (`crates/vox-dei-shim/src/research/orchestrator/pipeline.rs`). The GUI
surfaces only `status`/`history`/`config` (three arg-free cards in `decoratorRegistry.ts`). Missing: `run`,
`preview` (the plan is marked `editable:true` but there is no editor), `show`/`result` (durable artifacts in
`scientia_research_artifacts` are never rendered), `watch`, `eval`.

> **Reality check:** there is **no typed session-status enum**. The eight progress states
> (`queued → planning → retrieving → verifying_claims → synthesizing → auditing_citations →
> persisting_artifact → completed`) exist only as a hardcoded JSON string array at
> `crates/vox-cli/src/commands/research/mod.rs:234-243`. The persisted DB status is a free-form `String`
> with a *different* vocabulary (`active`/`completed`/`failed`/`orphaned`). `research run --async` only
> inserts a session row — **no executor consumes it**. A real run surface must run inline.

**Pipeline B — self-publication cluster** (`crates/vox-scientia/src/lib.rs`, Phases A–H). The GUI surfaces
the Phase H `QueueSnapshot` (`ScientiaDashboard.tsx`) and the claims ledger (`ClaimsView.tsx`). Missing:
scout, replay, manuscript/LaTeX/arXiv, critic-gate, venue routing, findings-page, the cost rollup
(`build_cost_rollup` exists but has no live-data producer), the publication lifecycle board
(`scientia_publication_queue` stages), discovery/novelty, pre-registration, feeds, and the external-jobs
dead-letter queue.

> **Reality check:** the documented `GET /api/v2/scientia/queue`, `/cost`, and WS topic
> `scientia.queue.changed` (`crates/vox-scientia/src/dashboard/mod.rs:13-17`) **do not exist**. The HTTP
> gateway (`crates/vox-orchestrator-mcp/src/http_gateway/`) is real but **disabled by default**
> (`VOX_MCP_HTTP_ENABLED`), and its `/v1/ws` has no topic multiplex. The GUI talks to Scientia by shelling
> the CLI (`execute_command`) and parsing stdout — not over REST/WS.

## Gamification is mature in the backend and dead in the GUI

`crates/vox-gamify` implements XP, infinite quadratic levels, crystals, lumens, energy, streaks, prestige,
trust-tier multipliers, achievements, quests, battles, leaderboards, and companions with SVG sprites. The
GUI shows a raw `vox ludus profile` text dump (`GamifyView.tsx`). Three concrete breaks:

1. The banner alert source is hardcoded `alerts: Vec::new()` (`crates/vox-gui/src/commands/orchestrator.rs:222`),
   so persisted level-ups and unlocks never display — even though the whole UI pipeline
   (`mapAlert` → `Dashboard` → `LudusBanner` → `handleAckAlert`) already exists.
2. There is no typed `get_ludus_profile` Tauri command; the rich `LudusProfile` is never rendered.
3. Gamify config (`gamify_enabled`, `gamify_mode` = `Balanced|Serious|Learning`) is settable only via the CLI
   (`VoxConfig::save()`); `set_gui_preference` writes a *different* store and there is no Settings gamify
   section.

> **Reality check:** `vox_gamify_notification_ack` is **not** a missing backend — it is a working MCP tool
> (`crates/vox-orchestrator-mcp/src/gamify_tools.rs:386`) the GUI already reaches via the MCP bridge. The
> high-value fix is wiring `list_unread_notifications` into the hardcoded-empty `alerts` vector.

## What is missing from the *CLI* (the inverse gap)

- ~35 `vox_scientia_*` MCP tools, some MCP-only, with no CLI peer.
- Per-event reward tuning (`event_config` overrides / `set_event_config_override`) has no clear CLI surface.
- The Phase H REST/WS routes are documented but unimplemented.

## Recommendation — a three-part self-surfacing loop

The hard half already exists (clap → typed form). The missing half is the *signal* that a new piece needs
curation, plus a *single source* to drive nav from:

1. **One typed surface-registry SSOT** the GUI enumerates to build the sidebar, routes, and palette — instead
   of three hand-synced lists.
2. **A CI parity gate** that iterates every top-level CLI group (it already has the catalog) and *fails the
   build* when a group is not classified into a representation tier
   (`none` / `generic_form` / `curated_decorator` / `live_backend`).
3. **An in-GUI coverage view** that renders the ledger so contributors discover what still needs curation.

Net effect: *add a CLI command → it is instantly runnable as a typed form, the palette finds it, and CI tells
you whether it deserves a curated panel.*

## Implementation plans

This audit is executed by three independently-shippable plans:

- **Track A — self-surfacing gate** (the keystone): `docs/superpowers/plans/2026-06-03-gui-track-a-self-surfacing-gate.md`
- **Track C — gamification surfacing** (low-risk wins): `docs/superpowers/plans/2026-06-03-gui-track-c-gamification-surfacing.md`
- **Track B — Scientia pipeline UI** (largest; typed reads, no speculative REST/WS): `docs/superpowers/plans/2026-06-03-gui-track-b-scientia-pipeline-ui.md`

Recommended order: A → C → B (A establishes the registry the other two register into; C is the lowest-risk
visible win; B is the largest and depends on the typed-command pattern C exercises first).
