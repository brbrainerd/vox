---
title: "Vox GUI Documentation SSOT"
description: "One registry field feeds in-app tooltips and a generated docs page with CI-captured screenshots, so GUI help cannot drift from the GUI."
category: "architecture"
status: "roadmap"
---

# Vox GUI Documentation SSOT — Design

**Date:** 2026-08-22
**Status:** approved for planning
**Scope decision:** extend the existing surface registry. No new SSOT, no new
gate, no per-surface doc files.

---

## 1. Problem

The Vox GUI has **32 user-reachable surfaces** and **zero end-user
documentation**. There is no `docs/src/how-to/*gui*` page, no screenshots
directory, and no tooltip primitive — 34 surface files reach for ad-hoc
`title=` attributes instead.

The naive fix (write a docs page per surface) is the expensive one. This
repository has extensive evidence of what that produces: 22 of 22
`contracts/reports/` directories stale by three weeks or more, a findings
backlog with one commit in 102 days, `ai-ide-feature-matrix-2026.json` with
14 of 31 cited paths dead, and `script-registry.json` with 29 of 34 rows
pointing at deleted files. Hand-maintained parallel descriptions of a moving
system rot, reliably, and this codebase has already paid that cost repeatedly.

**The design constraint is therefore not "write GUI docs." It is "make GUI
docs that cannot drift."**

## 2. What already exists

The substrate is unusually strong, which is why this design adds so little:

| Capability | Where | State |
| --- | --- | --- |
| Surface SSOT | `contracts/gui/surface-registry.v1.yaml` | **Exists.** 107 entries, schema-validated |
| TS generation from it | `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` | **Exists.** `SURFACE_REGISTRY: SurfaceRegistryEntry[]`, imported by the app |
| Generator + CI gate | `vox ci gui-surface-registry --write`, `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` | **Exists.** Hard-errors on a `view_key` not wired into `App.tsx`; runs in `ssot-drift` and the `ssot-autoregen` bot |
| Screenshot capture | `crates/vox-gui/ui/e2e/review/states.ts`, `screenshots.spec.ts`, `@review-capture` tag | **Exists.** `SURFACE_STATES` registry, 3 viewports, `rich`/`empty`/`error` mock installers |
| Headless run | `playwright.config.ts` `webServer: pnpm run dev` @ `localhost:1420` | **Exists.** Vite dev server — **no Tauri packaging, no display server** |
| Popover primitive | `crates/vox-gui/ui/src/components/ui/Popover.tsx` | Exists |
| **Tooltip primitive** | — | **Missing** |
| **User-facing description** | — | **Missing** |
| **Any GUI user doc** | — | **Missing** |

Two missing fields and one missing component. That is the whole gap.

## 3. Scope: 32 surfaces, not 107

The registry's 107 entries are not 107 screens. Measured:

- **75** entries are `representation_tier: none` **and** `nav_label: null` —
  CLI-group placeholders reserving a `view_key`, not user-reachable surfaces.
  Verified: zero `tier: none` entries carry a `nav_label`.
- **32** entries are real surfaces — 25 `live_backend` + 7 `curated_decorator`,
  every one with a `nav_label`.

This split is what makes a hard-fail gate affordable. The requirement binds
**exactly the surfaces a user can navigate to**, and the blocking write-up is
32 sentences, not 107.

**Gate trigger condition:** `description` is required when
`representation_tier != 'none'`. It is forbidden — not merely optional — when
`tier == 'none'`, so the placeholder rows cannot accumulate prose nobody reads.

## 4. Architecture

```
   contracts/gui/surface-registry.v1.yaml        ← the SSOT (existing)
   32 real surfaces gain:  description, help_anchor?
                     │
     vox ci gui-surface-registry --write         ← existing generator + gate
                     │
        ┌────────────┼─────────────────┐
        ▼            ▼                 ▼
  surfaceRegistry  gui-surfaces    gate: description
  .generated.ts    .generated.md   required iff tier != none
        │            │
        ▼            ▼
   in-app        voxlang.org page
   Tooltip       + screenshots
                     ▲
                     │
     e2e @review-capture sweep (existing) publishes rich-mock PNGs
```

One source, one generator, two consumers. The app **imports** the description
it displays, so an app tooltip and a docs page cannot disagree — they are the
same string.

### 4.1 Contract change

Two fields on each real surface entry:

```yaml
- view_key: chat
  representation_tier: live_backend
  nav_label: Chat
  description: >-
    Send instructions to an agent and watch its replies stream back. The
    session rail on the left keeps prior conversations.
  help_anchor: chat        # optional; defaults to view_key
```

`description` is **one or two plain sentences, user-facing**. It is not
`notes:`, which is existing developer shorthand ("absorbs discovery-inbox,
discovery-review, archive-panel") and stays exactly as it is — a separate
field, a separate audience. Conflating them would put internal migration
history into a user tooltip.

`help_anchor` exists only so a tooltip can deep-link to the generated docs
page; it defaults to `view_key` and will rarely be set.

### 4.2 The tooltip

A new `Tooltip.tsx` primitive alongside `Popover.tsx`, reading
`SURFACE_REGISTRY` by `viewKey`. Requirements that are not negotiable:

- **Keyboard reachable.** Focus shows the tooltip, `Escape` dismisses it.
  Hover-only help is invisible to keyboard and screen-reader users.
- **`aria-describedby`**, not `title=`. The native `title` attribute has no
  reliable screen-reader behavior and cannot be styled or dismissed.
- The 34 files currently using ad-hoc `title=` are **not** migrated by this
  work. That is a separate cleanup; mixing it in would balloon the diff and
  couple an accessibility refactor to a docs feature.

### 4.3 The generated page

`docs/src/reference/gui-surfaces.generated.md`, grouped by `nav_group`
(`operate` 11, `develop` 8, `knowledge` 6, `compute` 4, `system` 3), each
surface contributing its `nav_label`, `description`, and its captured
screenshot.

It is a `.generated.md` file, which this repo already has a convention for:
`linguist-generated` in `.gitattributes` (so diffs collapse), regenerated by
its generator, never hand-edited. Four such files already exist.

### 4.4 Screenshots

The `@review-capture` sweep already produces per-surface, per-viewport,
per-state PNGs against mock data. This design adds a publish step, not a
capture step.

- **`rich` mock fixtures only.** Deterministic (no diff churn from real data
  changing), and no risk of publishing a developer's real paths, task titles,
  or keys.
- **One viewport (`wide`, 1440×900) published.** The other two stay as
  regression-review artifacts. Publishing three viewports triples page weight
  to show the same information.
- Screenshots are build artifacts, **not committed to git**. They are produced
  in CI and published with the site, so a stale image is impossible: it either
  regenerates or the capture fails.

## 5. Why the gate is hard-fail

`vox ci gui-surface-registry` already hard-errors when a surface's `view_key`
is not found in `App.tsx`. Adding "…and `description` must be non-empty when
`tier != none`" is the same gate, same command, same failure mode — not a new
enforcement surface.

Warn-tier was considered and rejected on this repository's own evidence: warn
findings here are not consumed. The severity valve being added elsewhere in
this program exists precisely because warn-tier output accumulated unread.

The cost is honest: **32 descriptions must be written before the gate can turn
on.** The plan sequences the backfill before the gate for that reason. There is
no allowlist — an allowlist that never shrinks is a documented failure pattern
in this codebase, and with only 32 entries the backfill is small enough not to
need one.

## 6. Maintenance debt, accounted honestly

| Debt source | Mitigation |
| --- | --- |
| Description drifts from behavior | Cannot drift *between app and docs* — one string, both consumers. Can still drift from *reality* if a surface changes and nobody updates it. Nothing here fixes that; it is a smaller problem than today's, where there is no description at all. |
| Screenshot rot | Structurally impossible — regenerated every CI run, never committed. |
| New surface ships undocumented | Impossible — hard-fail gate. |
| Generated page rots | It is generated; `ssot-drift` catches divergence like the other four `.generated.md` files. |
| Tooltip and docs disagree | Impossible — same field. |

**The residual risk, stated plainly:** a description can become *wrong* without
becoming *absent*. No mechanism in this design detects a stale-but-present
sentence. That is a real limitation and the honest answer is that catching it
would require exactly the kind of semantic drift-detection machinery this
program has already concluded is not worth building.

## 7. Non-goals

- No per-surface markdown files (32 files to rot).
- No in-app markdown renderer or embedded doc browser — `DocViewerDrawer` and
  the omnibar `docs` facet already exist for reading docs in-app.
- No migration of the 34 ad-hoc `title=` usages.
- No screenshots committed to the repository.
- No documentation of the 75 `tier: none` placeholder entries.
- No coupling of the app build to the docs tree.

## 8. Verification

```bash
vox ci gui-surface-registry            # gate: description required iff tier != none
vox ci ssot-drift                      # generated TS + generated md in sync
pnpm --dir crates/vox-gui/ui exec playwright test --grep @review-capture
cargo test -p vox-gui                  # tooltip component tests
vox ci pre-push --full
```

`--full`, not `--complete`: `--complete` runs no tests.
