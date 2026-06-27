---
title: "Plan VG-2 — Top-bar Omnibar (faceted hybrid search, consolidates Search surface + CommandPalette)"
category: "Architecture SSOTs"
date: 2026-06-27
status: plan
plan_id: VG-2
spec: docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md
sources:
  - docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md
  - docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md
---

# Plan VG-2 — Top-bar Omnibar

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. This plan is **workflow-ready**: every task is tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`, grouped into explicit fan-out batches, and ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui` add+commit. Sub-agents MUST NOT run any other git verb (no checkout/reset/clean/push/rebase). On `.git/index.lock` contention, wait ~20s and retry the add+commit **once**.

## Goal

Build the **Omnibar** — a single global top-bar query affordance present on **every** view — that expands (⌘K / click) into a **faceted palette** over five provenance-labeled facets: **SURFACES / COMMANDS / ON-SCREEN / GRAPH / DOCS**. It merges three live sources at query time — the real `useSearchController` backend lane (`vox_search_query`), the VG-1 build-time **content manifest** (`gui-content-manifest.json`, consumed as a corpus), and a **NEW** in-memory `useSearchable()` runtime registry (opt-in per surface, ships as a no-op) — plus `vox_discover` for the GRAPH facet. Each facet is **independently capped** and **fails independently** (a `vox_discover` error shows an honest empty/error GRAPH row, never blanking the bar). `Enter` activates the top hit (navigate / run / `scrollIntoView`); `⇧Enter` sends the raw query to chat; `⌥→` expands graph neighbors.

It **consolidates** today's three overlapping entry points: it **folds** `components/layout/CommandPalette.tsx` into the Omnibar, and **deletes** the orphaned dedicated Search surface (`components/surfaces/Search/SearchView.tsx`) with a migration-ledger redirect so any deep link to `#view=search` opens the Omnibar instead of dead-ending.

## Architecture

**The Omnibar is the renamed, extended evolution of the already-global CommandPalette.** Verified on this branch:

- `components/layout/TopHud.tsx` (lines 233–239, 266–277) **already** renders the global trigger button (`data-testid="omnisearch-trigger"`, ⌘K hint) in both `full` and `slim` HUD modes, wired through `onOpenCommandPalette` (TopHud props line 65). `App.tsx` line 1143 passes `onOpenCommandPalette={() => setIsCommandOpen(true)}` and mounts `<CommandPalette …>` at line ~1170. **No new top-bar mount is needed** — the Omnibar replaces the component the trigger already opens.
- `components/layout/CommandPalette.tsx` is already a faceted palette: it composes `useSearchController` (backend `vox_search_query` lane, via `hooks/useSearchController.ts`) **and** `useFederatedSearchIndex` (client lane over surfaces/settings/policies/commands/actions/docs/skills, via `hooks/useFederatedSearchIndex.ts` + `lib/federatedSearchIndex.ts`), with prefix modes from `lib/paletteSources.ts` (`parsePaletteQuery`). The Omnibar **adds two facets** (ON-SCREEN, GRAPH) and **standardizes** facet headers/caps/provenance, then is renamed.
- The dedicated Search surface (`SearchView.tsx`) is **already orphaned**: `components/layout/surfaceComponents.tsx` has **no `case 'search'`** — the `search` top-level view (`lib/navigation.ts` line 41) defaults its child to `memory` → `MemoryView`. So "deleting the surface" is removing dead code + its tests + retiring its `SEARCH_SEED_KEY` plumbing, and adding an explicit redirect so `#view=search` opens the Omnibar.

**Three merge lanes, one ranker, five facets.** A new pure module `lib/omnibarFacets.ts` takes the three injected sources (backend hits, manifest corpus, runtime registry) + a graph-facet result, buckets them into the five facets, caps each, and stamps each row with `provenance` (`corpus` | `manifest` | `runtime` | `graph` | `docs`). The Omnibar component is a thin shell over this pure function — every merge/cap/provenance assertion is a unit test on `omnibarFacets.ts` with **injected** sources (no transport, no DOM), and the routing assertions (`Enter`/`⇧Enter`/`⌥→`) are component tests with a mocked `vox_discover`.

**The runtime registry is a no-op-by-default singleton.** `lib/searchableRegistry.ts` exports a module-level `Map<string, SearchableEntry[]>` keyed by surface id, plus `useSearchable(surfaceId, entries)` (registers on mount, clears on unmount) and `querySearchableRegistry(query)`. Ships with **zero** call sites — a surface with nothing dynamic contributes only its manifest rows (per spec §2.2 `ponytail:`). The Omnibar reads it synchronously; tests seed it directly.

**The VG-1 manifest is consumed, not produced, here.** VG-1 emits `gui-content-manifest.json` and (per VG-1) a Tauri reader modeled on `commands::docs_index::vox_docs_index` (`crates/vox-gui/src/commands/docs_index.rs`). VG-2 adds a thin `hooks/useContentManifest.ts` that loads it via `voxTransport.voxContentManifest()` and **defaults to `[]`** when the command is absent/errors — so VG-2 is independently testable and degrades honestly if executed before VG-1 lands. The merge code treats the manifest purely as an injected array.

**Honesty firewall (spec §6).** Facets fail independently: `omnibarFacets.ts` accepts a per-facet `{ rows, error }` shape; a facet with `error` renders an honest empty/error row and never removes the other facets. ON-SCREEN/GRAPH rows are **real** (manifest/registry/`vox_discover`), never fabricated — enforced by the existing `vox ci gui-honesty` scanner (`crates/vox-cli/src/commands/ci/gui_honesty.rs`) which we must not regress.

## Tech Stack

- **UI:** React + TypeScript (Vite), Vitest (`crates/vox-gui/ui`). GUI is **pnpm** (never npm). Existing seams reused verbatim: `useSearchController` (`hooks/useSearchController.ts`), `useFederatedSearchIndex` (`hooks/useFederatedSearchIndex.ts`), `buildFederatedIndex`/`searchFederatedIndex` (`lib/federatedSearchIndex.ts`), `parsePaletteQuery` (`lib/paletteSources.ts`), `viewKeyForLocator` (`lib/locatorNavigation.ts`), `UnifiedHit` (`components/surfaces/Search/searchHelpers.ts`), `voxTransport.invokeMcpTool` / `voxTransport.voxSearchQuery` (`transport.ts`).
- **Transport:** `vox_discover` is invoked via `voxTransport.invokeMcpTool('vox_discover', args)` (`transport.ts` line 439). The manifest reader (`voxContentManifest`) is a thin add modeled on `voxDocsIndex` (`transport.ts` line 450).
- **SSOT/codegen:** surface-registry edits go via `contracts/gui/surface-registry.v1.yaml` → `vox ci gui-surface-registry --write` (regenerates `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` — **never hand-edit** the `// AUTO-GENERATED … DO NOT EDIT` file).
- **Gates:** `vox ci gui-honesty` (typecheck + `surfaceHonesty.guard.test.ts`), `pnpm vitest run`. Windows fmt rule: **never** `cargo fmt --all`; use `cargo fmt -p <crate>` for the one Rust touch (Task O8). Per-crate Rust tests only.
- **Tests:** `pnpm -C crates/vox-gui/ui vitest run <file>` for each TDD step; `vox ci gui-honesty` before the final commit.

## Spec

Primary: `docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md` (§2 hybrid index, §3 the Omnibar, §5 architecture, §6 honesty, §7 testing). Umbrella: `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md`. Read both before coding.

## Dependencies (cross-plan)

- **MUST PRECEDE this plan:** **VG-1** (Vox Graph rename + skill + content-manifest emission) — provides `gui-content-manifest.json` and its Tauri reader. **Mitigation so VG-2 is executable/testable now:** the manifest is consumed through `useContentManifest.ts`, which returns `[]` when `voxContentManifest` is unavailable; every manifest assertion in this plan uses an **injected** fixture array, not the real file. VG-2's merge logic, facets, routing, and consolidation all land and test green without VG-1; the live manifest simply lights up the ON-SCREEN facet once VG-1 ships.
- **Registry-chain coordination (3A / 3F):** Plans **P5 (3A) GUI reorg** and **P6 (3F) CLI-governance surfaces** also edit `contracts/gui/surface-registry.v1.yaml` and regenerate `surfaceRegistry.generated.ts`. **Do not collide:** VG-2's only registry change (Task O8) is a **one-line `notes:`/redirect annotation** on the existing `search` row authored in the **YAML**, then `vox ci gui-surface-registry --write` to regenerate. Never hand-edit `surfaceRegistry.generated.ts`. If a 3A/3F branch is mid-flight, rebase the YAML edit and re-run the generator; the generated file is a pure function of the YAML, so there is no semantic merge — only a regen.
- **Independent of VG-3** (Task-Monitor Dashboard) — VG-3 shares only the surface registry, not the Omnibar.

## Key internals (verified against the code — exact)

- **`components/layout/TopHud.tsx`** — `data-testid="omnisearch-trigger"` button at lines 233–239 (slim) and 266–277 (full), both `onClick={openPalette}` where `openPalette = onOpenCommandPalette ?? onCommand` (line 100). Prop `onOpenCommandPalette?: () => void` (line 65). No change required to TopHud for placement.
- **`App.tsx`** — line 1143 `onOpenCommandPalette={() => setIsCommandOpen(true)}`; `<CommandPalette …>` mounted at line ~1170; `handleCommandAction` (lines 959–995) is the action sink — note the existing arm `else if ('id' in cmd && cmd.id === 'search') { navigateTo('search'); }` (lines 985–986) which the "See all results" footer triggers; this arm is **removed/repointed** in Task O7. `scrollToElement`-style helper exists near line 955 (`el.scrollIntoView({ block: 'nearest', behavior: 'smooth' })`).
- **`components/layout/CommandPalette.tsx`** — `SEARCH_SEED_KEY = 'vox_search_seed'` (line 18); composes `useSearchController({ enabled: backendSearchEnabled })` (line 79) and `useFederatedSearchIndex(skillSources)` (line 98); `parsePaletteQuery` from `./paletteSources` (line 8); `activateFedEntry` (lines 123–183) routes by `entry.payload.type`; `openHit` (lines 221–237) routes `UnifiedHit`; keyboard handler lines 262–286 (`ArrowDown`/`ArrowUp`/`Enter`/`Escape`); the "See all results" footer (lines 479–489) sets `SEARCH_SEED_KEY` then `onAction({ id: 'search' })`.
- **`hooks/useSearchController.ts`** — `useSearchController({ enabled })` → `{ state: { query, scopes, hits, loading, requestToken, repoTruncated }, setQuery, setScopes, runSearch }`; debounced (200 ms) `voxTransport.voxSearchQuery(q, 30, backendScopes)`; on error → empty hits (already fails soft).
- **`lib/federatedSearchIndex.ts`** — `FederatedIndexEntry { kind, id, label, detail, score?, payload, keywords? }`; `FederatedIndexKind = 'surface'|'setting'|'policy'|'command'|'action'|'skill'|'doc'`; `searchFederatedIndex(entries, query, { kinds })`.
- **`lib/paletteSources.ts`** — `parsePaletteQuery(raw) -> { mode: 'default'|'commands'|'agents'|'skills', query }` (strips `>` `@` `/`).
- **`components/surfaces/Search/searchHelpers.ts`** — `UnifiedHit { source, kind, path, title: string|null, snippet, score, provenance: string[], locator: OpenLocator }`.
- **`lib/navigation.ts`** — `PARENT_CHILD_MAP.memory = { parent: 'search', child: 'memory' }` (line 17); `DEFAULT_CHILD_BY_PARENT.search = 'memory'` (line 41); `TOP_LEVEL_VIEWS` includes `'search'` (line 53); `parseViewFromLocation` (line 125) reads `#view=` / `?view=`.
- **`components/layout/surfaceComponents.tsx`** — switch has **no `case 'search'`**; `case 'memory'` → `<MemoryView …>` (line 113); `default: return null` (line 191). Confirms `SearchView.tsx` is unrouted dead code.
- **`components/surfaces/Search/SearchView.tsx`** — `SEARCH_SEED_KEY` plumbing at lines 34, 302–303, 440; imports `useSearchController`, `filterCommandCatalogHits`, `filterSettingsIndexHits` from `lib/searchController`. Has a sibling `SearchView.test.tsx`.
- **`transport.ts`** — `invokeMcpTool(tool, args)` (line 439); `voxSearchQuery(query, limit, scope)` (line 462); `voxDocsIndex()` (line 450) — the model for the new `voxContentManifest()`.
- **`contracts/gui/surface-registry.v1.yaml`** — `search` row at lines 196–203 (`representation_tier: live_backend`, `nav_label: Search`, `nav_group: knowledge`, `notes: unified hybrid search (vox-search)`).
- **CI:** `crates/vox-cli/src/commands/ci/gui_honesty.rs` (`gui-honesty`), `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` (writes `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` + report).

## File Structure

**Created**
- `crates/vox-gui/ui/src/lib/searchableRegistry.ts` — in-memory runtime registry + `useSearchable` hook + `querySearchableRegistry`.
- `crates/vox-gui/ui/src/lib/searchableRegistry.test.ts`.
- `crates/vox-gui/ui/src/hooks/useContentManifest.ts` — loads VG-1 manifest, defaults `[]`.
- `crates/vox-gui/ui/src/lib/omnibarFacets.ts` — pure facet merge/cap/provenance + graph-neighbor expansion shape.
- `crates/vox-gui/ui/src/lib/omnibarFacets.test.ts`.
- `crates/vox-gui/ui/src/components/layout/Omnibar.tsx` — the faceted palette (renamed + extended CommandPalette).
- `crates/vox-gui/ui/src/components/layout/Omnibar.test.tsx`.
- `crates/vox-gui/ui/src/components/layout/OmnibarRedirect.test.tsx` — `#view=search` redirect coverage.

**Modified**
- `crates/vox-gui/ui/src/transport.ts` — add `voxContentManifest()`.
- `crates/vox-gui/ui/src/App.tsx` — swap `<CommandPalette>` → `<Omnibar>`; add `⇧Enter`→chat + `#view=search`→open-Omnibar redirect; repoint the `cmd.id === 'search'` arm.
- `crates/vox-gui/ui/src/components/layout/TopHud.tsx` — (rename-only) keep `omnisearch-trigger`; no behavior change.
- `contracts/gui/surface-registry.v1.yaml` — annotate the `search` row (`notes:` redirect) → regenerate via `vox ci gui-surface-registry --write`.

**Deleted**
- `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx` + `SearchView.test.tsx` (orphaned surface).
- `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx` + `CommandPalette.test.tsx` (folded into Omnibar — moved, not lost).

## Workflow batch structure (fan-out plan)

```
BATCH 1  (parallel — independent new files, no shared edits)
  ├─ O1  searchableRegistry.ts + useSearchable                [lib/searchableRegistry.ts]
  ├─ O2  useContentManifest.ts + transport.voxContentManifest [hooks/ + transport.ts]
  └─ O3  omnibarFacets.ts pure merge/cap/provenance           [lib/omnibarFacets.ts]

BATCH 2  (sequential — all build/rename the Omnibar component)
  ├─ O4  Omnibar.tsx = renamed CommandPalette + ON-SCREEN+GRAPH facets (depends O1,O2,O3)
  ├─ O5  ⌥→ graph-neighbor expansion in Omnibar (depends O4)
  └─ O6  delete Search surface + tests; retire SEARCH_SEED_KEY orphan (depends O4)

BATCH 3  (sequential — App wiring touches App.tsx once, then registry)
  ├─ O7  App: mount Omnibar, ⇧Enter→chat, #view=search redirect, repoint 'search' arm (depends O4,O6)
  └─ O8  surface-registry YAML annotate + regen (depends O7)

BATCH 4  (sequential — final gate)
  └─ O9  full vitest + vox ci gui-honesty + self-review checklist (depends O7,O8)
```

Tasks within a batch carry no shared-file edits and are safe to dispatch concurrently. Between batches there is a hard dependency edge. **Workflow cap: 3 concurrent sub-agents.**

---

# BATCH 1 — Foundations (parallel)

## Task O1 — Runtime searchable registry + `useSearchable` (TDD) — [PARALLEL-SAFE]

**Files:** `crates/vox-gui/ui/src/lib/searchableRegistry.ts` (new), `crates/vox-gui/ui/src/lib/searchableRegistry.test.ts` (new).

The Omnibar's ON-SCREEN facet needs an opt-in, in-memory registry of dynamic strings keyed by surface. It ships as a **no-op** (zero call sites); a surface with watch-worthy live text opts in via `useSearchable`.

**Step 1 — write the failing test.** Create `crates/vox-gui/ui/src/lib/searchableRegistry.test.ts`:

```ts
// @vitest-environment jsdom
import { describe, it, expect, beforeEach } from 'vitest';
import { renderHook } from '@testing-library/react';
import {
  registerSearchable,
  unregisterSearchable,
  querySearchableRegistry,
  clearSearchableRegistry,
  useSearchable,
  type SearchableEntry,
} from './searchableRegistry';

const ENTRY = (label: string): SearchableEntry => ({
  label,
  detail: '',
  viewKey: 'activity',
});

describe('searchableRegistry', () => {
  beforeEach(() => clearSearchableRegistry());

  it('starts empty (no-op default)', () => {
    expect(querySearchableRegistry('pending')).toEqual([]);
  });

  it('register → query (case-insensitive substring) → unregister', () => {
    registerSearchable('activity', [ENTRY('3 pending approvals'), ENTRY('queue idle')]);
    const hits = querySearchableRegistry('PENDING');
    expect(hits).toHaveLength(1);
    expect(hits[0].label).toBe('3 pending approvals');
    expect(hits[0].surfaceId).toBe('activity');
    unregisterSearchable('activity');
    expect(querySearchableRegistry('pending')).toEqual([]);
  });

  it('blank query returns nothing', () => {
    registerSearchable('activity', [ENTRY('3 pending approvals')]);
    expect(querySearchableRegistry('   ')).toEqual([]);
  });

  it('useSearchable registers on mount and clears on unmount', () => {
    const { unmount } = renderHook(() => useSearchable('mesh', [ENTRY('4 peers online')]));
    expect(querySearchableRegistry('peers')).toHaveLength(1);
    unmount();
    expect(querySearchableRegistry('peers')).toEqual([]);
  });
});
```

**Step 2 — run it, watch it fail.** `pnpm -C crates/vox-gui/ui vitest run src/lib/searchableRegistry.test.ts` → fails (module missing).

**Step 3 — minimal implementation.** Create `crates/vox-gui/ui/src/lib/searchableRegistry.ts`:

```ts
/**
 * In-memory runtime registry of searchable dynamic text, keyed by surface id.
 * Ships as a no-op: a surface opts in via `useSearchable` only when it has
 * watch-worthy dynamic strings. The Omnibar's ON-SCREEN facet reads this
 * synchronously alongside the build-time content manifest.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §2.2.
 */
import { useEffect } from 'react';

export interface SearchableEntry {
  /** The on-screen text to match (e.g. "3 pending approvals"). */
  label: string;
  /** Optional context shown after the label (e.g. surface name). */
  detail?: string;
  /** View key to navigate to when activated. */
  viewKey: string;
  /** Optional DOM id to scrollIntoView after navigating. */
  anchorId?: string;
}

export interface SearchableHit extends SearchableEntry {
  surfaceId: string;
}

const REGISTRY = new Map<string, SearchableEntry[]>();

export function registerSearchable(surfaceId: string, entries: SearchableEntry[]): void {
  REGISTRY.set(surfaceId, entries);
}

export function unregisterSearchable(surfaceId: string): void {
  REGISTRY.delete(surfaceId);
}

export function clearSearchableRegistry(): void {
  REGISTRY.clear();
}

export function querySearchableRegistry(query: string): SearchableHit[] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: SearchableHit[] = [];
  for (const [surfaceId, entries] of REGISTRY) {
    for (const e of entries) {
      if (
        e.label.toLowerCase().includes(q) ||
        (e.detail ?? '').toLowerCase().includes(q)
      ) {
        out.push({ ...e, surfaceId });
      }
    }
  }
  return out;
}

/**
 * Opt-in hook: a surface registers its dynamic searchable text while mounted.
 * No-op-friendly — surfaces with nothing dynamic never call this.
 */
export function useSearchable(surfaceId: string, entries: SearchableEntry[]): void {
  useEffect(() => {
    registerSearchable(surfaceId, entries);
    return () => unregisterSearchable(surfaceId);
  }, [surfaceId, entries]);
}
```

**Step 4 — run it, watch it pass.** `pnpm -C crates/vox-gui/ui vitest run src/lib/searchableRegistry.test.ts` → green.

**Step 5 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/lib/searchableRegistry.ts \
  crates/vox-gui/ui/src/lib/searchableRegistry.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(omnibar): runtime searchable registry + useSearchable (no-op default) [VG-2 O1]"
```

---

## Task O2 — `useContentManifest` hook + `voxContentManifest` transport (TDD) — [PARALLEL-SAFE]

**Files:** `crates/vox-gui/ui/src/transport.ts` (modify), `crates/vox-gui/ui/src/hooks/useContentManifest.ts` (new), `crates/vox-gui/ui/src/hooks/useContentManifest.test.ts` (new).

VG-1 emits `gui-content-manifest.json` and a Tauri reader. VG-2 consumes it through a hook that **defaults to `[]`** when the command is absent/errors, so VG-2 lands and tests green before VG-1.

**Step 1 — write the failing test.** Create `crates/vox-gui/ui/src/hooks/useContentManifest.test.ts`:

```ts
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';

const voxContentManifest = vi.fn();
vi.mock('../transport', () => ({
  voxTransport: { voxContentManifest: (...a: unknown[]) => voxContentManifest(...a) },
}));

import { useContentManifest } from './useContentManifest';
import type { ContentManifestEntry } from './useContentManifest';

const ROW: ContentManifestEntry = {
  viewKey: 'approvals',
  label: 'Approvals',
  route: '#view=approvals',
  headings: ['Pending', 'Resolved'],
  copy: ['Resolve or reject pending approvals'],
  commands: ['vox_resolve_approval'],
  docs: [],
};

describe('useContentManifest', () => {
  beforeEach(() => voxContentManifest.mockReset());

  it('returns the manifest rows on success', async () => {
    voxContentManifest.mockResolvedValue([ROW]);
    const { result } = renderHook(() => useContentManifest());
    await waitFor(() => expect(result.current).toHaveLength(1));
    expect(result.current[0].label).toBe('Approvals');
  });

  it('defaults to [] when the command rejects (VG-1 not landed)', async () => {
    voxContentManifest.mockRejectedValue(new Error('unknown command'));
    const { result } = renderHook(() => useContentManifest());
    await waitFor(() => expect(voxContentManifest).toHaveBeenCalled());
    expect(result.current).toEqual([]);
  });

  it('defaults to [] when voxContentManifest is undefined', async () => {
    // Simulate the transport method not existing yet.
    voxContentManifest.mockImplementation(() => {
      throw new TypeError('voxContentManifest is not a function');
    });
    const { result } = renderHook(() => useContentManifest());
    await waitFor(() => expect(result.current).toEqual([]));
  });
});
```

**Step 2 — run it, watch it fail.** `pnpm -C crates/vox-gui/ui vitest run src/hooks/useContentManifest.test.ts` → fails.

**Step 3 — minimal implementation.** Create `crates/vox-gui/ui/src/hooks/useContentManifest.ts`:

```ts
/**
 * Loads the VG-1 build-time GUI content manifest (gui-content-manifest.json),
 * exposed by the Tauri command `vox_content_manifest` (modeled on
 * `vox_docs_index`). Defaults to [] when the command is absent or errors so the
 * Omnibar degrades honestly before VG-1 lands.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §2.1.
 */
import { useEffect, useState } from 'react';
import { voxTransport } from '../transport';

export interface ContentManifestEntry {
  viewKey: string;
  label: string;
  route: string;
  headings: string[];
  copy: string[];
  commands: string[];
  docs: string[];
}

export function useContentManifest(): ContentManifestEntry[] {
  const [rows, setRows] = useState<ContentManifestEntry[]>([]);
  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const loaded = await voxTransport.voxContentManifest();
        if (!cancelled) setRows(Array.isArray(loaded) ? loaded : []);
      } catch {
        if (!cancelled) setRows([]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);
  return rows;
}
```

Then add the transport method in `crates/vox-gui/ui/src/transport.ts` immediately after `voxDocsIndex()` (line 450–452):

```ts
  /** VG-1 build-time GUI content manifest (gui-content-manifest.json). */
  voxContentManifest(): Promise<import('./hooks/useContentManifest').ContentManifestEntry[]> {
    return invoke('vox_content_manifest');
  }
```

**Step 4 — run it, watch it pass.** `pnpm -C crates/vox-gui/ui vitest run src/hooks/useContentManifest.test.ts` → green. Then `pnpm -C crates/vox-gui/ui exec tsc --noEmit -p tsconfig.json` (or `pnpm -C crates/vox-gui/ui run typecheck`) to confirm the transport type resolves.

**Step 5 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/hooks/useContentManifest.ts \
  crates/vox-gui/ui/src/hooks/useContentManifest.test.ts \
  crates/vox-gui/ui/src/transport.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(omnibar): useContentManifest hook + voxContentManifest transport (defaults [] pre-VG-1) [VG-2 O2]"
```

---

## Task O3 — `omnibarFacets.ts` pure merge / cap / provenance (TDD) — [PARALLEL-SAFE]

**Files:** `crates/vox-gui/ui/src/lib/omnibarFacets.ts` (new), `crates/vox-gui/ui/src/lib/omnibarFacets.test.ts` (new).

The heart of the Omnibar: a pure function that takes the five sources (federated index hits already split by kind, backend `UnifiedHit[]`, manifest rows, runtime registry hits, and a graph-facet result that may carry an error) and produces five capped, provenance-stamped facets. No transport, no DOM — fully unit-testable with injected data.

**Step 1 — write the failing test.** Create `crates/vox-gui/ui/src/lib/omnibarFacets.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { buildOmnibarFacets, FACET_CAP, type OmnibarSources } from './omnibarFacets';
import type { FederatedIndexEntry } from './federatedSearchIndex';
import type { UnifiedHit } from '../components/surfaces/Search/searchHelpers';
import type { ContentManifestEntry } from '../hooks/useContentManifest';
import type { SearchableHit } from './searchableRegistry';

const surface = (vk: string, label: string): FederatedIndexEntry => ({
  kind: 'surface',
  id: `surface:${vk}`,
  label,
  detail: 'Runs',
  payload: { type: 'surface', viewKey: vk },
});

const docEntry = (path: string, title: string): FederatedIndexEntry => ({
  kind: 'doc',
  id: `doc:${path}`,
  label: title,
  detail: '',
  payload: { type: 'doc', path },
});

const cmdHit = (cmd: string): UnifiedHit => ({
  source: 'commands',
  kind: 'command',
  path: cmd,
  title: cmd,
  snippet: '',
  score: 0.9,
  provenance: ['commands:catalog'],
  locator: { kind: 'command', value: cmd },
});

const manifestRow = (vk: string): ContentManifestEntry => ({
  viewKey: vk,
  label: 'Activity',
  route: `#view=${vk}`,
  headings: ['3 pending approvals'],
  copy: ['3 pending approvals'],
  commands: [],
  docs: [],
});

const regHit = (label: string): SearchableHit => ({
  surfaceId: 'activity',
  label,
  detail: 'Activity',
  viewKey: 'activity',
});

function sources(partial: Partial<OmnibarSources>): OmnibarSources {
  return {
    query: 'pending',
    federated: [],
    backendHits: [],
    manifest: [],
    runtimeHits: [],
    graph: { rows: [], error: null },
    ...partial,
  };
}

describe('buildOmnibarFacets', () => {
  it('buckets sources into the five facets with provenance labels', () => {
    const facets = buildOmnibarFacets(
      sources({
        federated: [surface('approvals', 'Approvals'), docEntry('a.md', 'Approvals workflow')],
        backendHits: [cmdHit('vox_resolve_approval')],
        manifest: [manifestRow('activity')],
        runtimeHits: [regHit('3 pending approvals (live)')],
        graph: {
          rows: [{ id: 'n1', label: 'Chat rail', viewKey: 'chat' }],
          error: null,
        },
      }),
    );
    const byKey = Object.fromEntries(facets.map((f) => [f.key, f]));
    expect(byKey.surfaces.rows[0].provenance).toBe('manifest'); // surface-kind → manifest-class label
    expect(byKey.commands.rows[0].provenance).toBe('corpus');
    expect(byKey.onScreen.rows.some((r) => r.provenance === 'runtime')).toBe(true);
    expect(byKey.graph.rows[0].provenance).toBe('graph');
    expect(byKey.docs.rows[0].provenance).toBe('docs');
    expect(facets.map((f) => f.key)).toEqual([
      'surfaces',
      'commands',
      'onScreen',
      'graph',
      'docs',
    ]);
  });

  it('caps each facet at FACET_CAP', () => {
    const many = Array.from({ length: FACET_CAP + 5 }, (_, i) => cmdHit(`cmd_${i}`));
    const facets = buildOmnibarFacets(sources({ backendHits: many }));
    const commands = facets.find((f) => f.key === 'commands')!;
    expect(commands.rows).toHaveLength(FACET_CAP);
  });

  it('graph facet failure is isolated — other facets still populate', () => {
    const facets = buildOmnibarFacets(
      sources({
        federated: [surface('approvals', 'Approvals')],
        graph: { rows: [], error: 'vox_discover unavailable' },
      }),
    );
    const graph = facets.find((f) => f.key === 'graph')!;
    const surfaces = facets.find((f) => f.key === 'surfaces')!;
    expect(graph.error).toBe('vox_discover unavailable');
    expect(graph.rows).toHaveLength(0);
    expect(surfaces.rows.length).toBeGreaterThan(0); // not blanked
  });

  it('topHit returns the highest-priority row across non-empty facets', () => {
    const facets = buildOmnibarFacets(
      sources({ federated: [surface('approvals', 'Approvals')] }),
    );
    const order = facets.flatMap((f) => f.rows);
    expect(order[0].facet).toBe('surfaces');
  });
});
```

**Step 2 — run it, watch it fail.** `pnpm -C crates/vox-gui/ui vitest run src/lib/omnibarFacets.test.ts` → fails.

**Step 3 — minimal implementation.** Create `crates/vox-gui/ui/src/lib/omnibarFacets.ts`:

```ts
/**
 * Pure faceting for the Omnibar: merge the five sources into capped,
 * provenance-labeled facets. No transport, no DOM. Facets fail independently —
 * a graph-source error is carried on the facet, never propagated to the others.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §3.3, §6.
 */
import type { FederatedIndexEntry } from './federatedSearchIndex';
import type { UnifiedHit } from '../components/surfaces/Search/searchHelpers';
import type { ContentManifestEntry } from '../hooks/useContentManifest';
import type { SearchableHit } from './searchableRegistry';

export const FACET_CAP = 6;

export type FacetKey = 'surfaces' | 'commands' | 'onScreen' | 'graph' | 'docs';
export type Provenance = 'manifest' | 'corpus' | 'runtime' | 'graph' | 'docs';

export interface GraphNeighbor {
  id: string;
  label: string;
  viewKey?: string;
}

export interface OmnibarGraphResult {
  rows: GraphNeighbor[];
  error: string | null;
}

export interface OmnibarSources {
  query: string;
  federated: FederatedIndexEntry[];
  backendHits: UnifiedHit[];
  manifest: ContentManifestEntry[];
  runtimeHits: SearchableHit[];
  graph: OmnibarGraphResult;
}

export type OmnibarActivation =
  | { type: 'navigate'; viewKey: string; anchorId?: string }
  | { type: 'command'; command: string }
  | { type: 'doc'; path: string }
  | { type: 'graph'; node: GraphNeighbor };

export interface OmnibarRow {
  id: string;
  facet: FacetKey;
  label: string;
  detail: string;
  provenance: Provenance;
  activate: OmnibarActivation;
}

export interface OmnibarFacet {
  key: FacetKey;
  label: string;
  provenanceHint: string;
  rows: OmnibarRow[];
  error: string | null;
}

const FACET_LABELS: Record<FacetKey, string> = {
  surfaces: 'Surfaces',
  commands: 'Commands',
  onScreen: 'On Screen',
  graph: 'Graph',
  docs: 'Docs',
};

const FACET_PROVENANCE_HINT: Record<FacetKey, string> = {
  surfaces: 'manifest',
  commands: 'corpus',
  onScreen: 'runtime + manifest',
  graph: 'vox-graph',
  docs: 'docs',
};

function matches(query: string, ...fields: string[]): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return false;
  return fields.some((f) => f.toLowerCase().includes(q));
}

export function buildOmnibarFacets(src: OmnibarSources): OmnibarFacet[] {
  const cap = <T>(rows: T[]) => rows.slice(0, FACET_CAP);

  // SURFACES — federated surface entries + manifest labels (manifest provenance).
  const surfaceRows: OmnibarRow[] = [];
  for (const e of src.federated) {
    if (e.kind !== 'surface' || e.payload.type !== 'surface') continue;
    surfaceRows.push({
      id: e.id,
      facet: 'surfaces',
      label: e.label,
      detail: e.detail,
      provenance: 'manifest',
      activate: { type: 'navigate', viewKey: e.payload.viewKey },
    });
  }
  for (const m of src.manifest) {
    if (!matches(src.query, m.label, ...m.headings)) continue;
    if (surfaceRows.some((r) => r.activate.type === 'navigate' && r.activate.viewKey === m.viewKey)) {
      continue;
    }
    surfaceRows.push({
      id: `manifest-surface:${m.viewKey}`,
      facet: 'surfaces',
      label: m.label,
      detail: m.route,
      provenance: 'manifest',
      activate: { type: 'navigate', viewKey: m.viewKey },
    });
  }

  // COMMANDS — backend hits whose kind is command + federated command/action.
  const commandRows: OmnibarRow[] = [];
  for (const h of src.backendHits) {
    if (h.kind !== 'command') continue;
    commandRows.push({
      id: `cmd:${h.path}`,
      facet: 'commands',
      label: h.title ?? h.path,
      detail: h.snippet,
      provenance: 'corpus',
      activate: { type: 'command', command: String(h.locator.value ?? h.path) },
    });
  }
  for (const e of src.federated) {
    if (e.kind === 'command' && e.payload.type === 'command') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'command', command: e.payload.command },
      });
    } else if (e.kind === 'action' && e.payload.type === 'action') {
      commandRows.push({
        id: e.id,
        facet: 'commands',
        label: e.label,
        detail: e.detail,
        provenance: 'corpus',
        activate: { type: 'command', command: e.payload.actionId },
      });
    }
  }

  // ON SCREEN — runtime registry hits + manifest copy/headings.
  const onScreenRows: OmnibarRow[] = [];
  for (const r of src.runtimeHits) {
    onScreenRows.push({
      id: `runtime:${r.surfaceId}:${r.label}`,
      facet: 'onScreen',
      label: r.label,
      detail: r.detail ?? '',
      provenance: 'runtime',
      activate: { type: 'navigate', viewKey: r.viewKey, anchorId: r.anchorId },
    });
  }
  for (const m of src.manifest) {
    const text = [...m.copy, ...m.headings].find((t) => matches(src.query, t));
    if (!text) continue;
    onScreenRows.push({
      id: `manifest-copy:${m.viewKey}:${text}`,
      facet: 'onScreen',
      label: text,
      detail: m.label,
      provenance: 'manifest',
      activate: { type: 'navigate', viewKey: m.viewKey },
    });
  }

  // GRAPH — vox_discover neighbors; error carried, never propagated.
  const graphRows: OmnibarRow[] = src.graph.error
    ? []
    : src.graph.rows.map((n) => ({
        id: `graph:${n.id}`,
        facet: 'graph' as const,
        label: n.label,
        detail: 'relates to',
        provenance: 'graph' as const,
        activate: { type: 'graph' as const, node: n },
      }));

  // DOCS — federated doc entries.
  const docRows: OmnibarRow[] = [];
  for (const e of src.federated) {
    if (e.kind !== 'doc' || e.payload.type !== 'doc') continue;
    docRows.push({
      id: e.id,
      facet: 'docs',
      label: e.label,
      detail: e.detail,
      provenance: 'docs',
      activate: { type: 'doc', path: e.payload.path },
    });
  }

  return [
    { key: 'surfaces', label: FACET_LABELS.surfaces, provenanceHint: FACET_PROVENANCE_HINT.surfaces, rows: cap(surfaceRows), error: null },
    { key: 'commands', label: FACET_LABELS.commands, provenanceHint: FACET_PROVENANCE_HINT.commands, rows: cap(commandRows), error: null },
    { key: 'onScreen', label: FACET_LABELS.onScreen, provenanceHint: FACET_PROVENANCE_HINT.onScreen, rows: cap(onScreenRows), error: null },
    { key: 'graph', label: FACET_LABELS.graph, provenanceHint: FACET_PROVENANCE_HINT.graph, rows: cap(graphRows), error: src.graph.error },
    { key: 'docs', label: FACET_LABELS.docs, provenanceHint: FACET_PROVENANCE_HINT.docs, rows: cap(docRows), error: null },
  ];
}

/** Flattened, facet-ordered rows (Surfaces → Commands → On-Screen → Graph → Docs). */
export function omnibarRowsInOrder(facets: OmnibarFacet[]): OmnibarRow[] {
  return facets.flatMap((f) => f.rows);
}

/** The top hit Enter activates: first row in facet order. */
export function omnibarTopHit(facets: OmnibarFacet[]): OmnibarRow | null {
  return omnibarRowsInOrder(facets)[0] ?? null;
}
```

**Step 4 — run it, watch it pass.** `pnpm -C crates/vox-gui/ui vitest run src/lib/omnibarFacets.test.ts` → green.

**Step 5 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/lib/omnibarFacets.ts \
  crates/vox-gui/ui/src/lib/omnibarFacets.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(omnibar): pure faceted merge/cap/provenance with isolated facet errors [VG-2 O3]"
```

---

# BATCH 2 — The Omnibar component (sequential)

## Task O4 — `Omnibar.tsx` = renamed CommandPalette + ON-SCREEN/GRAPH facets (TDD) — [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/components/layout/Omnibar.tsx` (new — content moved from CommandPalette + extended), `crates/vox-gui/ui/src/components/layout/Omnibar.test.tsx` (new). **Depends on O1, O2, O3.**

The Omnibar is CommandPalette folded forward: same backend + federated lanes, plus the runtime registry (O1), the manifest (O2), `vox_discover` for the GRAPH facet, all merged via `buildOmnibarFacets` (O3). It renders one labeled section per facet with a provenance hint, and routes `Enter`/`⇧Enter` (⌥→ lands in O5). The component delegates all merge logic to O3 and all data to injectable hooks, so tests inject sources directly.

**Step 1 — write the failing test.** Create `crates/vox-gui/ui/src/components/layout/Omnibar.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import React from 'react';
import { clearSearchableRegistry, registerSearchable } from '../../lib/searchableRegistry';

const invokeMcpTool = vi.fn();
vi.mock('../../transport', () => ({
  voxTransport: {
    voxDocsIndex: vi.fn().mockResolvedValue([]),
    voxContentManifest: vi.fn().mockResolvedValue([]),
    listPolicies: vi.fn().mockResolvedValue([]),
    openLocator: vi.fn().mockResolvedValue(undefined),
    invokeMcpTool: (...a: unknown[]) => invokeMcpTool(...a),
  },
}));

vi.mock('../../hooks/useSearchController', () => ({
  useSearchController: vi.fn(() => ({
    state: { query: 'pending', hits: [], loading: false, scopes: ['code'], requestToken: 1 },
    setQuery: vi.fn(),
    setScopes: vi.fn(),
  })),
}));

vi.mock('../../hooks/useFederatedSearchIndex', () => ({
  useFederatedSearchIndex: () => ({
    entries: [],
    search: () => [
      {
        kind: 'surface',
        id: 'surface:approvals',
        label: 'Approvals',
        detail: 'Runs',
        payload: { type: 'surface', viewKey: 'approvals' },
      },
    ],
  }),
}));

vi.mock('../../hooks/useContentManifest', () => ({
  useContentManifest: () => [
    {
      viewKey: 'activity',
      label: 'Activity',
      route: '#view=activity',
      headings: ['3 pending approvals'],
      copy: ['3 pending approvals'],
      commands: [],
      docs: [],
    },
  ],
}));

import { Omnibar } from './Omnibar';

const noop = () => {};

function renderOmnibar(overrides: Partial<React.ComponentProps<typeof Omnibar>> = {}) {
  return render(
    <Omnibar
      open
      onClose={noop}
      onNavigate={overrides.onNavigate ?? vi.fn()}
      onRunCommand={overrides.onRunCommand ?? vi.fn()}
      onSendToChat={overrides.onSendToChat ?? vi.fn()}
      onOpenDoc={overrides.onOpenDoc ?? vi.fn()}
      agents={[]}
      skills={[]}
      {...overrides}
    />,
  );
}

describe('Omnibar', () => {
  beforeEach(() => {
    clearSearchableRegistry();
    invokeMcpTool.mockReset();
    invokeMcpTool.mockResolvedValue({ result: { neighbors: [] } });
  });

  it('renders SURFACES and ON-SCREEN facets from federated + manifest', async () => {
    renderOmnibar();
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'pending' } });
    await waitFor(() => expect(screen.getByText('Approvals')).toBeTruthy());
    expect(screen.getByText('3 pending approvals')).toBeTruthy();
    expect(screen.getByText(/On Screen/i)).toBeTruthy();
  });

  it('Enter activates the top hit via onNavigate', async () => {
    const onNavigate = vi.fn();
    renderOmnibar({ onNavigate });
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'pending' } });
    await waitFor(() => expect(screen.getByText('Approvals')).toBeTruthy());
    fireEvent.keyDown(window, { key: 'Enter' });
    expect(onNavigate).toHaveBeenCalledWith('approvals', undefined);
  });

  it('Shift+Enter sends the raw query to chat', async () => {
    const onSendToChat = vi.fn();
    renderOmnibar({ onSendToChat });
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'why is queue stuck' } });
    fireEvent.keyDown(window, { key: 'Enter', shiftKey: true });
    expect(onSendToChat).toHaveBeenCalledWith('why is queue stuck');
  });

  it('runtime registry feeds the ON-SCREEN facet', async () => {
    registerSearchable('mesh', [{ label: 'mesh: 4 peers online', detail: 'Mesh', viewKey: 'mesh' }]);
    renderOmnibar();
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'peers' } });
    await waitFor(() => expect(screen.getByText('mesh: 4 peers online')).toBeTruthy());
  });
});
```

**Step 2 — run it, watch it fail.** `pnpm -C crates/vox-gui/ui vitest run src/components/layout/Omnibar.test.tsx` → fails.

**Step 3 — implementation.** Create `crates/vox-gui/ui/src/components/layout/Omnibar.tsx`. Start from the **current** `CommandPalette.tsx` body (move it), then make these surgical changes: (a) replace the props with the activation-callback shape below; (b) add the manifest + runtime + graph lanes; (c) render facets via `buildOmnibarFacets`/`omnibarRowsInOrder`; (d) route `Enter`/`⇧Enter`. (⌥→ added in O5.)

```tsx
import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { voxTransport } from '../../transport';
import { Icon } from '../ui/Icons';
import { CommandCatalogEntry } from '../../types/catalog';
import { Agent } from '../../types/dashboard';
import { UnifiedHit } from '../surfaces/Search/searchHelpers';
import { parsePaletteQuery } from './paletteSources';
import { useSearchController } from '../../hooks/useSearchController';
import { useFederatedSearchIndex } from '../../hooks/useFederatedSearchIndex';
import { useContentManifest } from '../../hooks/useContentManifest';
import { querySearchableRegistry } from '../../lib/searchableRegistry';
import {
  buildOmnibarFacets,
  omnibarRowsInOrder,
  type OmnibarGraphResult,
  type OmnibarRow,
} from '../../lib/omnibarFacets';
import { recordGamifyGuiEvent } from '../../lib/gamifyGuiEvents';

const FACET_ORDER = ['surfaces', 'commands', 'onScreen', 'graph', 'docs'] as const;

interface OmnibarProps {
  open: boolean;
  onClose: () => void;
  onNavigate: (viewKey: string, anchorId?: string) => void;
  onRunCommand: (command: string) => void;
  onSendToChat: (query: string) => void;
  onOpenDoc: (path: string) => void;
  agents: Agent[];
  skills: CommandCatalogEntry[];
  gamifyEnabled?: boolean;
}

function ProvenanceBadge({ hint }: { hint: string }) {
  return (
    <span className="shrink-0 rounded border border-border-subtle bg-overlay-subtle px-1.5 py-px font-mono text-[9px] uppercase tracking-widest text-text-muted">
      {hint}
    </span>
  );
}

export function Omnibar({
  open,
  onClose,
  onNavigate,
  onRunCommand,
  onSendToChat,
  onOpenDoc,
  agents,
  skills,
  gamifyEnabled = false,
}: OmnibarProps) {
  const [q, setQ] = useState('');
  const [selectedRowIdx, setSelectedRowIdx] = useState(-1);
  const [graph, setGraph] = useState<OmnibarGraphResult>({ rows: [], error: null });

  const { mode: prefixMode, query: effectiveQ } = parsePaletteQuery(q);
  const backendSearchEnabled = open && prefixMode === 'default';

  const { state: searchState, setQuery: setSearchQuery } = useSearchController({
    enabled: backendSearchEnabled,
  });
  const backendHits = useMemo(
    () => (backendSearchEnabled ? (searchState.hits as UnifiedHit[]) : []),
    [searchState.hits, backendSearchEnabled],
  );

  const skillSources = useMemo(
    () => skills.map((s) => ({ id: s.capability_id ?? s.command, name: s.command, description: s.about })),
    [skills],
  );
  const { search: searchFederated } = useFederatedSearchIndex(skillSources);
  const federated = useMemo(
    () => (effectiveQ.trim() ? searchFederated(effectiveQ) : []),
    [searchFederated, effectiveQ],
  );

  const manifest = useContentManifest();
  const runtimeHits = useMemo(
    () => (effectiveQ.trim() ? querySearchableRegistry(effectiveQ) : []),
    [effectiveQ],
  );

  // GRAPH facet: vox_discover, independently fallible.
  useEffect(() => {
    if (!open || !effectiveQ.trim()) {
      setGraph({ rows: [], error: null });
      return;
    }
    let cancelled = false;
    voxTransport
      .invokeMcpTool('vox_discover', { query: effectiveQ, limit: 6 })
      .then((res) => {
        if (cancelled) return;
        const r = res as { is_error?: boolean; result?: { neighbors?: unknown[] } };
        if (r?.is_error) {
          setGraph({ rows: [], error: 'vox_discover unavailable' });
          return;
        }
        const neighbors = Array.isArray(r?.result?.neighbors) ? r.result!.neighbors! : [];
        setGraph({
          rows: neighbors
            .map((n) => n as { id?: string; node_id?: string; label?: string; view_key?: string })
            .filter((n) => n.id || n.node_id)
            .map((n) => ({ id: String(n.id ?? n.node_id), label: n.label ?? String(n.id ?? n.node_id), viewKey: n.view_key })),
          error: null,
        });
      })
      .catch(() => {
        if (!cancelled) setGraph({ rows: [], error: 'vox_discover unavailable' });
      });
    return () => {
      cancelled = true;
    };
  }, [open, effectiveQ]);

  const facets = useMemo(
    () =>
      buildOmnibarFacets({
        query: effectiveQ,
        federated,
        backendHits,
        manifest,
        runtimeHits,
        graph,
      }),
    [effectiveQ, federated, backendHits, manifest, runtimeHits, graph],
  );
  const rows = useMemo(() => omnibarRowsInOrder(facets), [facets]);

  const activateRow = useCallback(
    (row: OmnibarRow) => {
      recordGamifyGuiEvent('palette_navigation', { facet: row.facet }, { enabled: gamifyEnabled });
      switch (row.activate.type) {
        case 'navigate':
          onNavigate(row.activate.viewKey, row.activate.anchorId);
          break;
        case 'command':
          onRunCommand(row.activate.command);
          break;
        case 'doc':
          onOpenDoc(row.activate.path);
          break;
        case 'graph':
          if (row.activate.node.viewKey) onNavigate(row.activate.node.viewKey);
          break;
      }
      onClose();
    },
    [onNavigate, onRunCommand, onOpenDoc, onClose, gamifyEnabled],
  );

  const rowsRef = useRef(rows);
  const idxRef = useRef(selectedRowIdx);
  rowsRef.current = rows;
  idxRef.current = selectedRowIdx;

  useEffect(() => {
    if (!open) {
      setQ('');
      setSearchQuery('');
      setSelectedRowIdx(-1);
    }
  }, [open, setSearchQuery]);

  useEffect(() => setSelectedRowIdx(-1), [rows.length, q]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      const list = rowsRef.current;
      let idx = idxRef.current;
      if (e.key === 'Escape') {
        onClose();
      } else if (e.key === 'Enter' && e.shiftKey) {
        e.preventDefault();
        if (q.trim()) onSendToChat(q.trim());
        onClose();
      } else if (e.key === 'ArrowDown' && list.length > 0) {
        e.preventDefault();
        idx = idx < 0 ? 0 : Math.min(idx + 1, list.length - 1);
        idxRef.current = idx;
        setSelectedRowIdx(idx);
      } else if (e.key === 'ArrowUp' && list.length > 0) {
        e.preventDefault();
        idx = idx < 0 ? list.length - 1 : Math.max(idx - 1, 0);
        idxRef.current = idx;
        setSelectedRowIdx(idx);
      } else if (e.key === 'Enter') {
        e.preventDefault();
        const target = idx >= 0 && idx < list.length ? list[idx] : list[0];
        if (target) activateRow(target);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [open, onClose, onSendToChat, activateRow, q]);

  if (!open) return null;

  let rowCursor = 0;

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 backdrop-blur-sm pt-[14vh]"
      onClick={onClose}
    >
      <div
        className="w-[680px] max-w-[92vw] rounded-2xl border border-border-subtle bg-bg-base/90 shadow-[0_40px_120px_-30px_rgba(0,0,0,0.9)] backdrop-blur-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center gap-2 border-b border-border-subtle px-4 py-3">
          <Icon.search className="size-4 text-brass" />
          <input
            autoFocus
            value={q}
            onChange={(e) => {
              const v = e.target.value;
              setQ(v);
              setSearchQuery(v);
            }}
            placeholder="Search surfaces, commands, on-screen text, graph, docs…  ⇧⏎ to ask chat"
            className="flex-1 bg-transparent text-[14px] text-text-primary placeholder:text-text-muted outline-none"
          />
          <kbd className="rounded border border-border-subtle bg-overlay-subtle px-1.5 py-0.5 font-mono text-[10px] text-text-muted">esc</kbd>
        </div>

        <div className="max-h-[480px] overflow-auto p-2 custom-scrollbar">
          {FACET_ORDER.map((key) => {
            const facet = facets.find((f) => f.key === key)!;
            if (facet.rows.length === 0 && !facet.error) return null;
            return (
              <React.Fragment key={key}>
                <div className="flex items-center justify-between px-3 py-2 mt-2 text-[10px] uppercase tracking-widest text-text-muted border-b border-border-subtle">
                  <span>{facet.label}</span>
                  <ProvenanceBadge hint={facet.provenanceHint} />
                </div>
                {facet.error ? (
                  <div className="px-3 py-2 text-[11px] text-text-muted italic">
                    {facet.label} unavailable — {facet.error}
                  </div>
                ) : (
                  facet.rows.map((row) => {
                    const idx = rowCursor++;
                    const selected = idx === selectedRowIdx;
                    return (
                      <button
                        key={row.id}
                        onClick={() => activateRow(row)}
                        className={`flex w-full items-center justify-between rounded-lg px-3 py-2 text-left transition ${
                          selected ? 'bg-brass/[0.08] border border-brass/20' : 'hover:bg-overlay-subtle'
                        }`}
                      >
                        <div className="flex flex-col min-w-0">
                          <span className="text-[13px] text-text-secondary truncate max-w-[460px]">{row.label}</span>
                          {row.detail ? (
                            <span className="text-[11px] text-text-muted truncate max-w-[460px]">{row.detail}</span>
                          ) : null}
                        </div>
                        <span className="font-mono text-[9px] uppercase tracking-widest text-text-muted shrink-0 ml-2">
                          {row.provenance}
                        </span>
                      </button>
                    );
                  })
                )}
              </React.Fragment>
            );
          })}

          {q.length > 0 && rows.length === 0 && !facets.some((f) => f.error) && (
            <div className="px-3 py-6 text-center text-[12px] text-text-muted">
              No matches for "{q}" — press ⇧⏎ to ask chat
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
```

**Step 4 — run it, watch it pass.** `pnpm -C crates/vox-gui/ui vitest run src/components/layout/Omnibar.test.tsx` → green.

**Step 5 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/components/layout/Omnibar.tsx \
  crates/vox-gui/ui/src/components/layout/Omnibar.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(omnibar): faceted Omnibar component (folds CommandPalette + ON-SCREEN/GRAPH; Enter/Shift+Enter) [VG-2 O4]"
```

---

## Task O5 — `⌥→` graph-neighbor expansion (TDD) — [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/components/layout/Omnibar.tsx` (modify), `crates/vox-gui/ui/src/components/layout/Omnibar.test.tsx` (append). **Depends on O4.**

`⌥→` (Alt+ArrowRight) on the selected (or top) GRAPH row expands its neighbors in-place by re-querying `vox_discover` seeded from that node, appending the new neighbors to the graph facet.

**Step 1 — add the failing test** to `Omnibar.test.tsx`:

```tsx
  it('Alt+ArrowRight expands graph neighbors of the selected node', async () => {
    invokeMcpTool
      .mockResolvedValueOnce({ result: { neighbors: [{ id: 'n1', label: 'Chat rail', view_key: 'chat' }] } })
      .mockResolvedValueOnce({ result: { neighbors: [{ id: 'n2', label: 'doubt_task', view_key: 'approvals' }] } });
    renderOmnibar();
    fireEvent.change(screen.getByPlaceholderText(/search/i), { target: { value: 'pending' } });
    await waitFor(() => expect(screen.getByText('Chat rail')).toBeTruthy());
    // Select the graph row (ArrowDown until it lands on the graph facet is brittle;
    // instead assert expansion appends without losing existing rows).
    fireEvent.keyDown(window, { key: 'ArrowDown' });
    fireEvent.keyDown(window, { key: 'ArrowRight', altKey: true });
    await waitFor(() => expect(screen.getByText('doubt_task')).toBeTruthy());
    expect(screen.getByText('Chat rail')).toBeTruthy(); // original neighbor retained
  });
```

**Step 2 — run it, watch it fail.**

**Step 3 — implement.** In `Omnibar.tsx`, add an expansion handler and wire it into the keydown effect. Add near `activateRow`:

```tsx
  const expandGraphNeighbors = useCallback(
    (row: OmnibarRow) => {
      if (row.activate.type !== 'graph') return;
      const seed = row.activate.node;
      voxTransport
        .invokeMcpTool('vox_discover', { seed: seed.id, mode: 'expand', limit: 6 })
        .then((res) => {
          const r = res as { is_error?: boolean; result?: { neighbors?: unknown[] } };
          if (r?.is_error) return;
          const neighbors = Array.isArray(r?.result?.neighbors) ? r.result!.neighbors! : [];
          setGraph((prev) => {
            if (prev.error) return prev;
            const seen = new Set(prev.rows.map((n) => n.id));
            const added = neighbors
              .map((n) => n as { id?: string; node_id?: string; label?: string; view_key?: string })
              .filter((n) => (n.id || n.node_id) && !seen.has(String(n.id ?? n.node_id)))
              .map((n) => ({ id: String(n.id ?? n.node_id), label: n.label ?? String(n.id ?? n.node_id), viewKey: n.view_key }));
            return { rows: [...prev.rows, ...added], error: null };
          });
        })
        .catch(() => {/* facet stays as-is — honest no-op */});
    },
    [],
  );
```

Then inside the `onKey` handler, before the final `Enter` arm, add:

```tsx
      } else if (e.key === 'ArrowRight' && e.altKey) {
        e.preventDefault();
        const target = idx >= 0 && idx < list.length ? list[idx] : list[0];
        if (target && target.activate.type === 'graph') expandGraphNeighbors(target);
```

(Add `expandGraphNeighbors` to the effect's dependency array.)

**Step 4 — run it, watch it pass.**

**Step 5 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/layout/Omnibar.tsx crates/vox-gui/ui/src/components/layout/Omnibar.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(omnibar): Alt+ArrowRight expands graph neighbors via vox_discover [VG-2 O5]"
```

---

## Task O6 — Delete the orphaned Search surface + retire `SEARCH_SEED_KEY` dead path (TDD) — [SEQUENTIAL]

**Files:** delete `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx` + `SearchView.test.tsx`; delete `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx` + `CommandPalette.test.tsx`; verify `lib/searchController.ts` helpers (`filterCommandCatalogHits`, `filterSettingsIndexHits`) have no remaining importers. **Depends on O4.**

`SearchView.tsx` is unrouted (no `case 'search'` in `surfaceComponents.tsx`; `search` → `memory`). With the Omnibar live, both the dedicated surface and `CommandPalette` are superseded.

**Step 1 — prove no live importers (the "test" is a grep gate).** Run and confirm **zero** non-test, non-self hits:

```bash
git -C /c/Users/Owner/vox-graphify-gui grep -n "SearchView" -- 'crates/vox-gui/ui/src/**/*.tsx' ':!*SearchView*'
git -C /c/Users/Owner/vox-graphify-gui grep -n "CommandPalette" -- 'crates/vox-gui/ui/src/**/*.tsx' ':!*CommandPalette*' ':!App.tsx'
```

(Expect: `SearchView` → no hits; `CommandPalette` → only `App.tsx`, handled in O7. If `SearchView` shows any live importer, STOP — re-scope and report; do not delete.)

**Step 2 — delete the files.**

```bash
git -C /c/Users/Owner/vox-graphify-gui rm \
  crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx \
  crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx \
  crates/vox-gui/ui/src/components/layout/CommandPalette.tsx \
  crates/vox-gui/ui/src/components/layout/CommandPalette.test.tsx
```

**Step 3 — clean up now-dead helpers.** `filterCommandCatalogHits` and `filterSettingsIndexHits` in `lib/searchController.ts` existed only for `SearchView`. Confirm:

```bash
git -C /c/Users/Owner/vox-graphify-gui grep -n "filterCommandCatalogHits\|filterSettingsIndexHits" -- crates/vox-gui/ui/src
```

If the only remaining hits are their definitions + `SearchView.test` (now deleted), remove both functions and the now-unused `searchSettings` import from `lib/searchController.ts`. Leave `userScopeToBackend`, `backendScopesFromUserScopes`, the reducer, and `UserScope` — these are still used by `useSearchController`. (If any other file imports the two filters, KEEP them and note it; do not break a live importer.)

**Step 4 — run the suite to confirm nothing else referenced the deleted code.**

```bash
pnpm -C crates/vox-gui/ui vitest run
```

Expect green except the App-level CommandPalette mount, fixed in O7 (if O7 not yet done, this command is run after O7; sequence O6→O7 keeps App compiling — so run this in O7's verification instead and here run only the targeted Omnibar + lib suites).

**Step 5 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add -A crates/vox-gui/ui/src/lib/searchController.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "refactor(omnibar): delete orphaned Search surface + CommandPalette, retire dead SEARCH_SEED_KEY helpers [VG-2 O6]"
```

---

# BATCH 3 — App wiring + registry (sequential)

## Task O7 — App: mount Omnibar, `⇧Enter`→chat, `#view=search`→open-Omnibar redirect, repoint 'search' arm (TDD) — [SEQUENTIAL]

**Files:** `crates/vox-gui/ui/src/App.tsx` (modify), `crates/vox-gui/ui/src/components/layout/OmnibarRedirect.test.tsx` (new). **Depends on O4, O6.**

Swap the `<CommandPalette>` mount for `<Omnibar>` with the activation callbacks; add the migration-ledger redirect so any deep link to `#view=search` opens the Omnibar instead of dead-ending into `memory`; repoint the old `cmd.id === 'search'` arm (it used to navigate to the Search surface) to open the Omnibar.

**Step 1 — write the failing redirect test.** Create `crates/vox-gui/ui/src/components/layout/OmnibarRedirect.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { redirectSearchViewToOmnibar } from './omnibarRedirect';

describe('redirectSearchViewToOmnibar', () => {
  it('opens the Omnibar and clears #view=search instead of navigating to a dead surface', () => {
    const openOmnibar = vi.fn();
    const navigateTo = vi.fn();
    const handled = redirectSearchViewToOmnibar('search', { openOmnibar, navigateTo, fallbackChild: 'memory' });
    expect(handled).toBe(true);
    expect(openOmnibar).toHaveBeenCalledTimes(1);
    expect(navigateTo).toHaveBeenCalledWith('memory'); // park on a real child, not the dead 'search' shell
  });

  it('passes through non-search views untouched', () => {
    const openOmnibar = vi.fn();
    const navigateTo = vi.fn();
    const handled = redirectSearchViewToOmnibar('approvals', { openOmnibar, navigateTo, fallbackChild: 'memory' });
    expect(handled).toBe(false);
    expect(openOmnibar).not.toHaveBeenCalled();
    expect(navigateTo).not.toHaveBeenCalled();
  });
});
```

**Step 2 — run it, watch it fail.**

**Step 3 — implement the redirect helper.** Create `crates/vox-gui/ui/src/components/layout/omnibarRedirect.ts`:

```ts
/**
 * Migration-ledger redirect: the dedicated Search surface is retired (VG-2).
 * Any deep link to `#view=search` opens the Omnibar instead of dead-ending,
 * and parks navigation on a real child surface.
 *
 * See docs/superpowers/specs/2026-06-27-vox-graph-omnibar-dashboard-design.md §3.2.
 */
export interface RedirectDeps {
  openOmnibar: () => void;
  navigateTo: (viewKey: string) => void;
  fallbackChild: string;
}

export function redirectSearchViewToOmnibar(viewKey: string, deps: RedirectDeps): boolean {
  if (viewKey !== 'search') return false;
  deps.navigateTo(deps.fallbackChild);
  deps.openOmnibar();
  return true;
}
```

**Step 4 — wire into `App.tsx`.** Three edits:

1. Replace the import `import { CommandPalette } from './components/layout/CommandPalette';` with:
```tsx
import { Omnibar } from './components/layout/Omnibar';
import { redirectSearchViewToOmnibar } from './components/layout/omnibarRedirect';
```

2. In `navigateTo` (wherever it resolves a view; it is defined above `handleCommandAction`), add the redirect as the first guard so requesting `search` opens the Omnibar:
```tsx
  // (inside navigateTo, before the normal resolution)
  if (redirectSearchViewToOmnibar(viewKey, {
    openOmnibar: () => setIsCommandOpen(true),
    navigateTo: (vk) => setActiveViewRaw(vk),   // the underlying non-redirecting setter
    fallbackChild: 'memory',
  })) {
    return;
  }
```
(Use whatever the underlying state setter is named; the redirect must call the *non-redirecting* setter to avoid recursion. If `navigateTo` is the only setter, inline the resolution: `setActiveView('memory'); setIsCommandOpen(true); return;`.)

3. Repoint the legacy arm at lines 985–986 of `handleCommandAction`:
```tsx
    } else if ('id' in cmd && cmd.id === 'search') {
      setIsCommandOpen(true);
```

4. Replace the `<CommandPalette …>` mount (line ~1170) with:
```tsx
      <Omnibar
        open={isCommandOpen}
        onClose={() => setIsCommandOpen(false)}
        onNavigate={(vk, anchorId) => {
          navigateTo(vk);
          if (anchorId) {
            requestAnimationFrame(() => {
              const el = document.getElementById(anchorId);
              el?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
            });
          }
        }}
        onRunCommand={(command) => {
          handleCommandAction({ id: 'hit', type: 'hit', locator: { kind: 'command', value: command }, viewKey: 'console' });
        }}
        onSendToChat={(query) => {
          navigateTo('chat');
          handleLoquelaSubmit({ description: query, session_id: activeSessionId });
        }}
        onOpenDoc={(path) => { voxTransport.openLocator({ kind: 'file', value: path }).catch(() => {}); }}
        agents={data.agents}
        skills={installedSkillEntries}
        gamifyEnabled={gamifyEnabled}
      />
```
(Match the exact prop names already in scope: `isCommandOpen`/`setIsCommandOpen`, `data.agents`, `installedSkillEntries`, `gamifyEnabled`, `handleLoquelaSubmit`, `activeSessionId`. Verify each against the surrounding `App.tsx` before saving — they are all referenced earlier in the file.)

**Step 5 — run.** `pnpm -C crates/vox-gui/ui vitest run src/components/layout/OmnibarRedirect.test.tsx` → green, then the full suite `pnpm -C crates/vox-gui/ui vitest run` → green (this is where the O6 deletion is fully validated), then `pnpm -C crates/vox-gui/ui run typecheck`.

**Step 6 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  crates/vox-gui/ui/src/App.tsx \
  crates/vox-gui/ui/src/components/layout/omnibarRedirect.ts \
  crates/vox-gui/ui/src/components/layout/OmnibarRedirect.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(omnibar): mount Omnibar in App, Shift+Enter→chat, #view=search redirect [VG-2 O7]"
```

---

## Task O8 — Annotate the `search` surface-registry row + regenerate (SSOT) — [SEQUENTIAL]

**Files:** `contracts/gui/surface-registry.v1.yaml` (modify the `search` row), regenerate `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` via codegen. **Depends on O7.**

The `search` row stays in the registry (so `vox ci gui-honesty` keeps seeing a real `viewKey` and deep links resolve), but its `notes` now record the redirect so the SSOT documents that `#view=search` opens the Omnibar. **Edit the YAML, never the generated TS.**

**Step 1 — edit the YAML.** In `contracts/gui/surface-registry.v1.yaml`, change the `search` row's `notes` (line 203):

```yaml
- view_key: search
  cli_group: null
  representation_tier: live_backend
  nav_label: Search
  nav_icon: search
  nav_group: knowledge
  parent_surface: null
  notes: redirects to the global Omnibar (top bar, ⌘K) — VG-2; deep links to #view=search open the Omnibar
```

**Step 2 — regenerate** (this is the codegen; do **not** hand-edit the generated file):

```bash
git -C /c/Users/Owner/vox-graphify-gui --no-pager diff --stat -- contracts/gui/surface-registry.v1.yaml
( cd /c/Users/Owner/vox-graphify-gui && cargo run -q -p vox-cli -- ci gui-surface-registry --write )
```

(If a build broker shim breaks `cargo` in the main dir, run from this worktree as already cwd'd. The command rewrites `surfaceRegistry.generated.ts` + `contracts/reports/gui-surface-registry.v1.json`.)

**Step 3 — verify** the generated file changed only the `notes` for `search` and re-typecheck:

```bash
git -C /c/Users/Owner/vox-graphify-gui --no-pager diff -- crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
pnpm -C crates/vox-gui/ui run typecheck
```

**Step 4 — commit.**

```bash
git -C /c/Users/Owner/vox-graphify-gui add \
  contracts/gui/surface-registry.v1.yaml \
  crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts \
  contracts/reports/gui-surface-registry.v1.json
git -C /c/Users/Owner/vox-graphify-gui commit -m "chore(omnibar): annotate search surface row as Omnibar redirect + regen registry [VG-2 O8]"
```

---

# BATCH 4 — Gate (sequential)

## Task O9 — Full vitest + `vox ci gui-honesty` + self-review (SEQUENTIAL) — [SEQUENTIAL]

**Depends on O7, O8.** No new code — verification only.

**Step 1 — full GUI suite.**
```bash
pnpm -C crates/vox-gui/ui vitest run
```
Expect green. The deleted `CommandPalette.test.tsx` / `SearchView.test.tsx` are gone; the new `searchableRegistry`, `useContentManifest`, `omnibarFacets`, `Omnibar`, `OmnibarRedirect` suites are green.

**Step 2 — honesty gate (must not regress).**
```bash
( cd /c/Users/Owner/vox-graphify-gui && cargo run -q -p vox-cli -- ci gui-honesty )
```
Expect `gui-honesty: OK`. This runs `pnpm run typecheck` + `surfaceHonesty.guard.test.ts`.

**Step 3 — self-review checklist** (assert each, fix before final commit):
- [ ] Omnibar present on every view: it is the component the global TopHud `omnisearch-trigger` opens; TopHud is mounted on all views — no per-view wiring needed.
- [ ] Five facets render with provenance badges; each capped at `FACET_CAP`.
- [ ] `vox_discover` error → GRAPH shows honest empty/error row; SURFACES/ON-SCREEN/COMMANDS/DOCS still populate (the O3 isolation test + O4 mock cover this).
- [ ] `Enter` activates top hit; `⇧Enter` sends raw query to chat; `⌥→` expands graph neighbors.
- [ ] Search surface deleted; `#view=search` redirect opens the Omnibar (OmnibarRedirect test).
- [ ] `CommandPalette` folded in; no live importer remains (grep clean).
- [ ] Runtime registry ships as no-op (zero call sites); manifest defaults `[]` pre-VG-1.
- [ ] Registry change went via YAML→`--write`; generated TS not hand-edited.
- [ ] No `cargo fmt --all` was run; no `.ps1`/`.sh`/`.py` added.

**Step 4 — final marker commit** (only if Steps 1–3 are green and any fixups were made; if no fixups, skip).
```bash
git -C /c/Users/Owner/vox-graphify-gui add -A
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(omnibar): full vitest + gui-honesty green; VG-2 close [VG-2 O9]" --allow-empty
```

---

## Workflow Batch Plan

| Batch | Task | Title | Tag | Files | Depends |
|---|---|---|---|---|---|
| 1 | O1 | Runtime searchable registry + `useSearchable` (no-op default) | [PARALLEL-SAFE] | `lib/searchableRegistry.ts` (+test) | — |
| 1 | O2 | `useContentManifest` + `voxContentManifest` transport (defaults `[]`) | [PARALLEL-SAFE] | `hooks/useContentManifest.ts` (+test), `transport.ts` | — |
| 1 | O3 | `omnibarFacets.ts` pure merge/cap/provenance, isolated errors | [PARALLEL-SAFE] | `lib/omnibarFacets.ts` (+test) | — |
| 2 | O4 | `Omnibar.tsx` faceted component (folds CommandPalette + ON-SCREEN/GRAPH; Enter/⇧Enter) | [SEQUENTIAL] | `components/layout/Omnibar.tsx` (+test) | O1, O2, O3 |
| 2 | O5 | `⌥→` graph-neighbor expansion | [SEQUENTIAL] | `components/layout/Omnibar.tsx` (+test) | O4 |
| 2 | O6 | Delete orphaned Search surface + CommandPalette; retire dead helpers | [SEQUENTIAL] | (deletions) + `lib/searchController.ts` | O4 |
| 3 | O7 | App: mount Omnibar, ⇧Enter→chat, `#view=search` redirect, repoint 'search' arm | [SEQUENTIAL] | `App.tsx`, `components/layout/omnibarRedirect.ts` (+test) | O4, O6 |
| 3 | O8 | Annotate `search` registry row + regen (YAML→`--write`) | [SEQUENTIAL] | `surface-registry.v1.yaml`, generated TS, report | O7 |
| 4 | O9 | Full vitest + `gui-honesty` + self-review | [SEQUENTIAL] | (verification) | O7, O8 |

**Concurrency cap: 3 sub-agents.** Batch 1 fans out 3-wide (no shared files). Batch 2 serializes on `Omnibar.tsx` (O4→O5) with O6 after O4. Batch 3 serializes on `App.tsx` then the registry. Batch 4 is the gate.

**Repo-rule reminders for every sub-agent:** GUI is **pnpm** (never npm); never `cargo fmt --all` on Windows (use `cargo fmt -p <crate>` — VG-2 has no `.rs` changes beyond none, so fmt is not invoked); surface-registry edits go **only** via `contracts/gui/surface-registry.v1.yaml` → `vox ci gui-surface-registry --write` (never hand-edit the generated TS); do not regress `vox ci gui-honesty`; `git -C /c/Users/Owner/vox-graphify-gui` add+commit only — no checkout/clean/reset/rebase/push; on `.git/index.lock` wait ~20s and retry once.
