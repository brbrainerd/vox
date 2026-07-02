---
title: "GUI IA Intent-First Reorg Implementation Plan"
description: "Executes the ratified gui-ia-blueprint merges/cuts plus the intent-first nav reorder (Direct > Review > Agents > Knowledge): promotes needs-you/runs into a Review group, consolidates 4 activity clones into one Discovery surface, folds Matrix into the chat rail, de-Latinizes hollow labels, and keeps all legacy deep-links resolving."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-07-02"
training_eligible: false
authored: "2026-07-02"
---

# GUI IA Intent-First Reorg Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize vox-gui navigation around the human's intent-first priorities — 1) express intent (Chat/Direct), 2) review agent work (new promoted **Review** group: approvals, needs-you, runs, policies), 3) operate agents, 4) comprehend (Knowledge) — executing the ratified blueprint (`docs/agents/gui-ia-blueprint.md`): retire the Search group, merge `claims`→`scientia` (label → Findings), consolidate `discovery-inbox`/`discovery-review`/`archive-panel`/`activity` into ONE Discovery surface with filter presets, fold `matrix` into the chat rail, move `gamify`→Settings and `mesh`/`sub-agents`→Agents, rename `oratio`→Voice / `mens`→Training / `populi`→Nodes, delete 12 registry entries (the phantom `review` surface, the 5 parent-shell duplicates, the Bundle-4 `search` shell, and the merge-absorbed rows), and keep every old `#view=` deep-link resolving via a legacy-alias map.

**Architecture:** Three SSOT sites stay in lockstep: (1) `crates/vox-gui/ui/src/lib/navigation.ts` (`PARENT_CHILD_MAP`, `TOP_LEVEL_VIEWS`, `DEFAULT_CHILD_BY_PARENT`, `NAV_LABELS`, plus new `LEGACY_VIEW_ALIASES` and `CHILD_ORDER_BY_PARENT`); (2) `contracts/gui/surface-registry.v1.yaml` — the *source* of the generated `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` (regenerated via `vox ci gui-surface-registry --write`, NEVER hand-edited); (3) `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` dispatch + `decoratorRegistry.ts`. Sub-tab rows in `ParentSurface.tsx` come from the registry's `parentSurface` field; sidebar labels come from `lexicon.ts` (not `NAV_LABELS` — those feed breadcrumbs only). Deep-link redirects ride a pure alias map resolved inside `resolveNavigation`, so `App.tsx` changes stay minimal.

**Tech Stack:** React 19 + TypeScript + Vite, vitest (jsdom), pnpm (`pnpm -C crates/vox-gui/ui …` — NEVER npm), Rust generator `cargo run -p vox-cli --bin vox -- ci gui-surface-registry --write`.

---

### Task 1: Navigation SSOT — intent-first topology, legacy aliases, child ordering

**Files:**
- `crates/vox-gui/ui/src/lib/navigation.test.ts` (rewrite)
- `crates/vox-gui/ui/src/lib/navigation.ts` (rewrite data structures + `resolveNavigation`)

- [ ] Replace the entire contents of `crates/vox-gui/ui/src/lib/navigation.test.ts` with the new-topology spec:

```typescript
import { describe, expect, it } from 'vitest';
import {
  resolveNavigation,
  parseViewFromLocation,
  breadcrumbsForView,
  TOP_LEVEL_VIEWS,
  DEFAULT_CHILD_BY_PARENT,
  LEGACY_VIEW_ALIASES,
  CHILD_ORDER_BY_PARENT,
  orderedChildren,
  labelForNavKey,
} from './navigation';

describe('intent-first top-level order', () => {
  it('orders groups: Direct, Review, Agents, Knowledge, Workspace, Commands, Compute, Mercatus, Settings', () => {
    expect([...TOP_LEVEL_VIEWS]).toEqual([
      'chat', 'runs', 'agents', 'knowledge', 'workspace', 'commands', 'compute', 'mercatus', 'settings',
    ]);
  });
  it('retires the Search group', () => {
    expect(TOP_LEVEL_VIEWS).not.toContain('search');
    expect(DEFAULT_CHILD_BY_PARENT.search).toBeUndefined();
  });
  it('labels the runs group Review', () => {
    expect(labelForNavKey('runs')).toBe('Review');
  });
});

describe('Review group (runs parent)', () => {
  it('deep-links approvals to runs parent', () => {
    expect(resolveNavigation('approvals')).toEqual({ parent: 'runs', child: 'approvals' });
  });
  it('wires needs-you into nav under runs', () => {
    expect(resolveNavigation('needs-you')).toEqual({ parent: 'runs', child: 'needs-you' });
  });
  it('promotes runs to a named child of its own group', () => {
    expect(resolveNavigation('runs')).toEqual({ parent: 'runs', child: 'runs' });
  });
  it('keeps approvals as the sidebar landing child for the Review group', () => {
    expect(DEFAULT_CHILD_BY_PARENT.runs).toBe('approvals');
  });
});

describe('group moves', () => {
  it('moves mesh from compute to agents', () => {
    expect(resolveNavigation('mesh').parent).toBe('agents');
  });
  it('moves sub-agents under agents (wired via subagent_tree)', () => {
    expect(resolveNavigation('sub-agents').parent).toBe('agents');
  });
  it('moves gamify from agents to settings', () => {
    expect(resolveNavigation('gamify')).toEqual({ parent: 'settings', child: 'gamify' });
  });
  it('reparents memory under knowledge and makes it the default child', () => {
    expect(resolveNavigation('memory')).toEqual({ parent: 'knowledge', child: 'memory' });
    expect(resolveNavigation('knowledge')).toEqual({ parent: 'knowledge', child: 'memory' });
  });
  it('wires the consolidated Discovery surface (activity) under knowledge', () => {
    expect(resolveNavigation('activity')).toEqual({ parent: 'knowledge', child: 'activity' });
  });
});

describe('legacy alias redirects (deep-links must not break)', () => {
  it('claims and review resolve to scientia', () => {
    expect(resolveNavigation('claims')).toEqual({ parent: 'knowledge', child: 'scientia' });
    expect(resolveNavigation('review')).toEqual({ parent: 'knowledge', child: 'scientia' });
  });
  it('discovery clones resolve to the one Discovery surface', () => {
    for (const legacy of ['discovery-inbox', 'discovery-review', 'archive-panel']) {
      expect(resolveNavigation(legacy)).toEqual({ parent: 'knowledge', child: 'activity' });
    }
  });
  it('matrix folds into chat', () => {
    expect(resolveNavigation('matrix')).toEqual({ parent: 'chat', child: 'chat' });
  });
  it('search resolves to memory', () => {
    expect(resolveNavigation('search')).toEqual({ parent: 'knowledge', child: 'memory' });
  });
  it('exposes the alias map for callers that seed presets', () => {
    expect(LEGACY_VIEW_ALIASES['discovery-inbox']).toBe('activity');
  });
});

describe('child ordering', () => {
  it('orders Review children approvals-first', () => {
    expect(CHILD_ORDER_BY_PARENT.runs).toEqual(['approvals', 'needs-you', 'runs', 'policies']);
  });
  it('orders Workspace children console-first', () => {
    expect(orderedChildren('workspace', ['browser', 'console', 'harness', 'repository']))
      .toEqual(['console', 'repository', 'browser', 'harness']);
  });
  it('orders Knowledge children memory-first', () => {
    expect(orderedChildren('knowledge', ['activity', 'memory', 'publications', 'research', 'scientia', 'vox-search']))
      .toEqual(['memory', 'scientia', 'research', 'activity', 'publications', 'vox-search']);
  });
  it('passes unknown parents through untouched', () => {
    expect(orderedChildren('mercatus', ['a', 'b'])).toEqual(['a', 'b']);
  });
});

describe('unchanged plumbing', () => {
  it('parseViewFromLocation reads hash and query', () => {
    expect(parseViewFromLocation({ hash: '#view=console', search: '' })).toBe('console');
    expect(parseViewFromLocation({ hash: '', search: '?view=memory' })).toBe('memory');
    expect(parseViewFromLocation({ hash: '', search: '' })).toBeNull();
  });
  it('breadcrumbsForView includes parent and child', () => {
    expect(breadcrumbsForView('console').map(c => c.key)).toEqual(['workspace', 'console']);
  });
});
```

- [ ] Run the test and confirm it FAILS (missing exports `LEGACY_VIEW_ALIASES`, `CHILD_ORDER_BY_PARENT`, `orderedChildren`, plus old topology):

```
pnpm -C crates/vox-gui/ui test src/lib/navigation.test.ts
```

Expected: `FAIL src/lib/navigation.test.ts` (import/type errors or assertion failures).

- [ ] In `crates/vox-gui/ui/src/lib/navigation.ts`, replace `PARENT_CHILD_MAP`, `DEFAULT_CHILD_BY_PARENT`, `TOP_LEVEL_VIEWS`, and `NAV_LABELS` (lines 1–104) with:

```typescript
/**
 * Resolve a view key to its top-level nav parent and optional child tab.
 * Intent-first grouping: Direct(chat) → Review(runs) → Agents → Knowledge →
 * Workspace → Commands → Compute → Settings.
 */
export const PARENT_CHILD_MAP: Record<string, { parent: string; child?: string }> = {
  // Review — approvals first: the human's review queue.
  approvals: { parent: 'runs', child: 'approvals' },
  'needs-you': { parent: 'runs', child: 'needs-you' },
  runs: { parent: 'runs', child: 'runs' },
  policies: { parent: 'runs', child: 'policies' },
  // Agents — watch/steer the swarm.
  dashboard: { parent: 'agents', child: 'dashboard' },
  flow: { parent: 'agents', child: 'flow' },
  tasks: { parent: 'agents', child: 'tasks' },
  mesh: { parent: 'agents', child: 'mesh' },
  'sub-agents': { parent: 'agents', child: 'sub-agents' },
  // Knowledge — find/recall/review what the system knows.
  memory: { parent: 'knowledge', child: 'memory' },
  scientia: { parent: 'knowledge', child: 'scientia' },
  research: { parent: 'knowledge', child: 'research' },
  activity: { parent: 'knowledge', child: 'activity' },
  publications: { parent: 'knowledge', child: 'publications' },
  'vox-search': { parent: 'knowledge', child: 'vox-search' },
  // Workspace — act on the dev environment.
  console: { parent: 'workspace', child: 'console' },
  repository: { parent: 'workspace', child: 'repository' },
  browser: { parent: 'workspace', child: 'browser' },
  harness: { parent: 'workspace', child: 'harness' },
  // Commands.
  catalog: { parent: 'commands', child: 'catalog' },
  skills: { parent: 'commands', child: 'skills' },
  // Compute.
  models: { parent: 'compute', child: 'models' },
  mens: { parent: 'compute', child: 'mens' },
  populi: { parent: 'compute', child: 'populi' },
  oratio: { parent: 'compute', child: 'oratio' },
  // Settings.
  coverage: { parent: 'settings', child: 'coverage' },
  gamify: { parent: 'settings', child: 'gamify' },
};

/**
 * Migration ledger (gui-ia-blueprint §5): retired view keys resolve to their
 * surviving absorber so old #view= deep-links and bookmarks never dead-end.
 * Silent alias for one release, then hard-remove.
 */
export const LEGACY_VIEW_ALIASES: Record<string, string> = {
  search: 'memory',
  claims: 'scientia',
  review: 'scientia',
  matrix: 'chat',
  'discovery-inbox': 'activity',
  'discovery-review': 'activity',
  'archive-panel': 'activity',
};

/** Discovery preset carried by retired discovery deep-links (read by DiscoverySurface). */
export const DISCOVERY_PRESET_BY_LEGACY_KEY: Record<string, 'inbox' | 'review' | 'archive'> = {
  'discovery-inbox': 'inbox',
  'discovery-review': 'review',
  'archive-panel': 'archive',
};

export const DISCOVERY_PRESET_SEED_KEY = 'vox_discovery_preset_seed';

/** Seed the Discovery preset when a retired discovery key is navigated to. */
export function seedDiscoveryPresetForLegacyKey(viewKey: string): void {
  const preset = DISCOVERY_PRESET_BY_LEGACY_KEY[viewKey];
  if (!preset) return;
  try {
    window.localStorage.setItem(DISCOVERY_PRESET_SEED_KEY, preset);
  } catch {
    /* localStorage unavailable — surface still switches, preset defaults */
  }
}

/** Stable default child when navigating to a top-level parent (breadcrumb / sidebar). */
export const DEFAULT_CHILD_BY_PARENT: Record<string, string> = {
  chat: 'chat',
  runs: 'approvals',
  agents: 'dashboard',
  knowledge: 'memory',
  workspace: 'console',
  commands: 'catalog',
  compute: 'models',
  mercatus: 'mercatus',
  settings: 'settings',
};

export const TOP_LEVEL_VIEWS = [
  'chat',
  'runs',
  'agents',
  'knowledge',
  'workspace',
  'commands',
  'compute',
  'mercatus',
  'settings',
] as const;

export type TopLevelView = typeof TOP_LEVEL_VIEWS[number];

/** Sub-tab display order per parent (registry rows are alphabetical; UI order is intent order). */
export const CHILD_ORDER_BY_PARENT: Record<string, string[]> = {
  runs: ['approvals', 'needs-you', 'runs', 'policies'],
  agents: ['dashboard', 'flow', 'tasks', 'mesh', 'sub-agents'],
  knowledge: ['memory', 'scientia', 'research', 'activity', 'publications', 'vox-search'],
  workspace: ['console', 'repository', 'browser', 'harness'],
  commands: ['catalog', 'skills'],
  compute: ['models', 'mens', 'populi', 'oratio'],
  settings: ['settings', 'coverage', 'gamify'],
};

/** Sort child view keys by the parent's intent order; unknown keys keep relative order at the end. */
export function orderedChildren(parent: string, children: string[]): string[] {
  const order = CHILD_ORDER_BY_PARENT[parent];
  if (!order) return children;
  const rank = new Map(order.map((k, i) => [k, i]));
  return [...children].sort(
    (a, b) => (rank.get(a) ?? order.length) - (rank.get(b) ?? order.length),
  );
}

/** Human-readable labels for breadcrumb segments. */
export const NAV_LABELS: Record<string, string> = {
  chat: 'Chat',
  runs: 'Review',
  agents: 'Agents',
  knowledge: 'Knowledge',
  workspace: 'Workspace',
  commands: 'Commands',
  compute: 'Compute',
  mercatus: 'Mercatus',
  settings: 'Settings',
  dashboard: 'Dashboard',
  flow: 'Flow',
  tasks: 'Tasks',
  approvals: 'Approvals',
  'needs-you': 'Needs You',
  policies: 'Policies',
  repository: 'Repository',
  browser: 'Browser',
  harness: 'Harness',
  console: 'Console',
  catalog: 'Catalog',
  skills: 'Skills',
  memory: 'Memory',
  research: 'Research',
  scientia: 'Findings',
  activity: 'Discovery',
  'vox-search': 'Search Index',
  publications: 'Publications',
  models: 'Models',
  mens: 'Training',
  populi: 'Nodes',
  oratio: 'Voice',
  mesh: 'Mesh',
  'sub-agents': 'Sub-Agents',
  coverage: 'Coverage',
  gamify: 'Gamify',
};
```

- [ ] In the same file, update `resolveNavigation` (currently lines 151–164) to resolve aliases first:

```typescript
export function resolveNavigation(viewKey: string): { parent: string; child: string } {
  const key = LEGACY_VIEW_ALIASES[viewKey] ?? viewKey;
  const mapped = PARENT_CHILD_MAP[key];
  if (mapped) {
    return { parent: mapped.parent, child: mapped.child ?? key };
  }
  if (TOP_LEVEL_VIEWS.includes(key as TopLevelView)) {
    const defaultChild = DEFAULT_CHILD_BY_PARENT[key] ?? key;
    return {
      parent: key,
      child: defaultChild,
    };
  }
  return { parent: key, child: key };
}
```

Leave `labelForNavKey`, `breadcrumbsForView`, `parseViewFromLocation`, `viewToHash`, `syncViewToLocation` unchanged.

- [ ] Run both navigation test files and confirm green:

```
pnpm -C crates/vox-gui/ui test src/lib/navigation.test.ts src/lib/navigation.vox-search.test.ts
```

Expected: `Test Files  2 passed` (navigation.vox-search.test.ts still passes: vox-search stays under knowledge, `NAV_LABELS.knowledge` is still `'Knowledge'`, label `'Search Index'` unchanged).

- [ ] Commit:

```
git add crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/lib/navigation.test.ts
git commit -m "feat(gui-ia): intent-first nav topology with legacy aliases and child ordering"
```

---

### Task 2: Surface registry — YAML edits + regenerate (never hand-edit generated TS)

**Files:**
- `contracts/gui/surface-registry.v1.yaml` (hand-edit — this IS the source)
- `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` (regenerated only)
- `contracts/reports/gui-surface-registry.v1.json` (regenerated only)

- [ ] In `contracts/gui/surface-registry.v1.yaml`, DELETE these 12 surface entries entirely (blueprint Group C cuts + merge-absorbed rows): the entries whose `view_key` is `review`, `agents`, `commands`, `compute`, `workspace`, `knowledge`, `search`, `claims`, `matrix`, `discovery-inbox`, `discovery-review`, `archive-panel`. (Verify each deleted row has `cli_group: null` so no CLI group loses classification; all 12 do.)
- [ ] EDIT these entries in place:
  - `view_key: activity` → `nav_label: Discovery`, `nav_icon: eye`, `nav_group: knowledge`, `parent_surface: knowledge`, `notes: consolidated Discovery surface (timeline + inbox/review/archive presets); absorbs discovery-inbox, discovery-review, archive-panel`
  - `view_key: memory` → `parent_surface: knowledge` (was `search`)
  - `view_key: mesh` → `nav_group: operate`, `parent_surface: agents` (was compute/compute)
  - `view_key: sub-agents` → `nav_group: operate`, `parent_surface: agents`, `notes: wired to subagent_tree/subagent_control (blueprint ADD-conditional satisfied)`
  - `view_key: needs-you` → `parent_surface: runs`
  - `view_key: gamify` → `nav_group: system`, `parent_surface: settings`
  - `view_key: scientia` → `nav_label: Findings`
  - `view_key: oratio` → `nav_label: Voice`
  - `view_key: mens` → `nav_label: Training`
  - `view_key: populi` → `nav_label: Nodes`
- [ ] Regenerate from repo root (this rewrites the YAML in sorted form, the generated TS, and the report — do NOT touch the `.generated.ts` by hand):

```
cargo run -p vox-cli --bin vox -- ci gui-surface-registry --write
```

Expected output: `gui-surface-registry: wrote registry, generated TS, and report`

- [ ] Run the drift/wiring gate and confirm it passes (all surviving `view_key`s still appear in `App.tsx` — the legacy keys remain in `LEGACY_VIEWS`, and deleted rows are simply gone):

```
cargo run -p vox-cli --bin vox -- ci gui-surface-registry
```

Expected output: `gui-surface-registry: registry and generated TS are up to date`

- [ ] Verify the generated file no longer contains the cut rows and contains the moves:

```
grep -n "viewKey: 'review'\|viewKey: 'search'\|viewKey: 'claims'\|viewKey: 'matrix'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
```

Expected: no matches (exit code 1).

```
grep -n "viewKey: 'activity'\|viewKey: 'mesh'\|viewKey: 'needs-you'\|viewKey: 'gamify'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
```

Expected: `activity … parentSurface: 'knowledge'`, `mesh … parentSurface: 'agents'`, `needs-you … parentSurface: 'runs'`, `gamify … parentSurface: 'settings'`.

- [ ] Run the frontend suite to confirm nothing broke (Sidebar's `settings`/`coverage` registry lookups survive):

```
pnpm -C crates/vox-gui/ui test
```

Expected: all test files pass.

- [ ] Commit:

```
git add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts contracts/reports/gui-surface-registry.v1.json
git commit -m "feat(gui-ia): registry cuts (review + 5 parent shells), merges, and reparents; regen surfaceRegistry"
```

---

### Task 3: App.tsx wiring — resolve aliases on load, seed Discovery presets

**Files:**
- `crates/vox-gui/ui/src/App.tsx`

- [ ] Update the import at line 8 to pull the seeder:

```typescript
import { resolveNavigation, parseViewFromLocation, syncViewToLocation, seedDiscoveryPresetForLegacyKey } from './lib/navigation';
```

- [ ] In `navigateTo` (lines 532–536), seed presets and keep resolving through the alias map:

```typescript
const navigateTo = useCallback((viewKey: string) => {
  seedDiscoveryPresetForLegacyKey(viewKey);
  const { child } = resolveNavigation(viewKey);
  setActiveView(child as View);
  syncViewToLocation(child);
}, [setActiveView]);
```

- [ ] In the bootstrap effect (`get_initial_view`, lines 376–384), resolve legacy hashes to their surviving child instead of storing the raw legacy key. Replace:

```typescript
      if (fromHash && LEGACY_VIEWS.includes(fromHash)) {
        setActiveView(fromHash as View);
        syncViewToLocation(fromHash);
        return;
      }
      if (view && LEGACY_VIEWS.includes(view)) {
        setActiveView(view as View);
        syncViewToLocation(view);
      }
```

with:

```typescript
      if (fromHash && LEGACY_VIEWS.includes(fromHash)) {
        seedDiscoveryPresetForLegacyKey(fromHash);
        const { child } = resolveNavigation(fromHash);
        setActiveView(child as View);
        syncViewToLocation(child);
        return;
      }
      if (view && LEGACY_VIEWS.includes(view)) {
        const { child } = resolveNavigation(view);
        setActiveView(child as View);
        syncViewToLocation(child);
      }
```

- [ ] In the `hashchange` handler (lines 640–643), add preset seeding before resolution:

```typescript
      if (fromHash && LEGACY_VIEWS.includes(fromHash)) {
        seedDiscoveryPresetForLegacyKey(fromHash);
        const { child } = resolveNavigation(fromHash);
        setActiveView(child as View);
      }
```

- [ ] Do NOT trim the `View` union or `LEGACY_VIEWS` — legacy keys must stay accepted for one release (silent alias policy, blueprint §5), and the registry wiring gate greps `App.tsx` for surviving keys.
- [ ] Typecheck and run the suite:

```
pnpm -C crates/vox-gui/ui typecheck
pnpm -C crates/vox-gui/ui test
```

Expected: typecheck clean; all tests pass.

- [ ] Commit:

```
git add crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui-ia): resolve legacy deep-links through alias map and seed Discovery presets"
```

---

### Task 4: Sidebar + lexicon — Review group label, icon, default-child landing, de-Latinized labels

**Files:**
- `crates/vox-gui/ui/src/lib/lexicon.ts`
- `crates/vox-gui/ui/src/lib/lexicon.test.ts`
- `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`
- `crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx` (asserts the OLD labels — must be updated or the suite stays red)

- [ ] Update `crates/vox-gui/ui/src/lib/lexicon.test.ts` FIRST (failing): the proper-noun example currently uses `mens`, which gains a translation. Replace the test at lines 12–15 and extend:

```typescript
  it('proper noun has no la and stays en in Latin mode', () => {
    expect(LEXICON['set-orchestrator'].la).toBeUndefined();
    expect(pick(LEXICON['set-orchestrator'], 'la')).toBe('Orchestrator');
  });
  it('de-Latinizes compute surface labels (Bundle 1 + Amendment A)', () => {
    expect(pick(LEXICON.oratio, 'en')).toBe('Voice');
    expect(pick(LEXICON.mens, 'en')).toBe('Training');
    expect(pick(LEXICON.populi, 'en')).toBe('Nodes');
  });
  it('labels the promoted Review group and the Discovery surface', () => {
    expect(pick(LEXICON['nav:runs'], 'en')).toBe('Review');
    expect(pick(LEXICON.activity, 'en')).toBe('Discovery');
  });
```

- [ ] Run and confirm FAIL:

```
pnpm -C crates/vox-gui/ui test src/lib/lexicon.test.ts
```

- [ ] In `crates/vox-gui/ui/src/lib/lexicon.ts`, change these entries (keys and all other entries — including legacy `claims`/`review`/`search`/`matrix`/`discovery-*` labels still referenced by tests and by the one-release-aliased components — stay):

```typescript
  'nav:runs': { en: 'Review', la: 'Recensio' },
  activity: { en: 'Discovery', la: 'Acta' },
  mens: { en: 'Training', la: 'Mens' },
  oratio: { en: 'Voice', la: 'Oratio' },
  populi: { en: 'Nodes', la: 'Populi' },
```

- [ ] In `crates/vox-gui/ui/src/components/layout/Sidebar.tsx`:
  - Update the import at line 8 to include the default-child map:

    ```typescript
    import { TOP_LEVEL_VIEWS, DEFAULT_CHILD_BY_PARENT, resolveNavigation } from '../../lib/navigation';
    ```
  - Replace `TOP_NAV_ICON` (lines 53–64) — drop `search`, give the Review group a review-shaped icon:

    ```typescript
    const TOP_NAV_ICON: Record<string, string> = {
      chat: 'message',
      runs: 'shield',
      agents: 'users',
      knowledge: 'book',
      workspace: 'folder',
      commands: 'terminal',
      compute: 'cpu',
      mercatus: 'scale',
      settings: 'settings',
    };
    ```
  - In the top-level `NavItem` map (line 183), land parents on their default child so the Review group opens on Approvals even though `runs` now maps to its own child:

    ```typescript
    onClick={() => setView(DEFAULT_CHILD_BY_PARENT[key] ?? key)}
    ```
  - Replace the hardcoded aria label (lines 171–176):

    ```typescript
    const navAriaLabel =
      key === 'runs'
        ? approvalsPending != null && approvalsPending > 0
          ? `Review, ${approvalsPending} pending approvals`
          : 'Review'
        : undefined;
    ```

- [ ] Update `crates/vox-gui/ui/src/components/layout/Sidebar.test.tsx`, which hard-asserts the old labels (audit-verified): line 59 `getByRole('button', { name: /Runs.*3 pending/i })` → `{ name: /Review.*3 pending approvals/i }`, and line 64 `getByRole('button', { name: 'Runs and Approvals' })` → `{ name: 'Review' }`. (The lexicon `nav:runs` change alone already breaks line 64.)

- [ ] Run tests:

```
pnpm -C crates/vox-gui/ui test src/lib/lexicon.test.ts
pnpm -C crates/vox-gui/ui test
```

Expected: lexicon tests pass; full suite green.

- [ ] Commit:

```
git add crates/vox-gui/ui/src/lib/lexicon.ts crates/vox-gui/ui/src/lib/lexicon.test.ts crates/vox-gui/ui/src/components/layout/Sidebar.tsx
git commit -m "feat(gui-ia): sidebar intent order, Review group label/icon, de-Latinized compute labels"
```

---

### Task 5: ParentSurface — intent-ordered sub-tabs

**Files:**
- `crates/vox-gui/ui/src/components/layout/ParentSurface.test.tsx` (extend)
- `crates/vox-gui/ui/src/components/layout/ParentSurface.tsx`

- [ ] Add a failing ordering test to `ParentSurface.test.tsx`. Audit-verified facts: the file's existing `vi.mock` (lines 6–11) replaces `surfaceRegistry.generated` **wholesale** with only mercatus/activity rows, and `SubTabs.tsx:29-40` renders plain `<button>` elements with **no** `role="tab"`. So: extend the existing `vi.mock` factory with `browser`/`console`/`harness`/`repository` rows (`{ viewKey, navLabel, parentSurface: 'workspace', tier: 'live_backend' }`, listed alphabetically — registry order), do NOT import `SURFACE_REGISTRY` in the test, and query buttons:

```typescript
describe('ParentSurface sub-tab ordering', () => {
  it('renders workspace tabs in intent order (console first), not registry order', () => {
    render(
      <LanguageProvider>
        <ParentSurface parentKey="workspace" activeChild="console" onChildChange={vi.fn()} renderChild={() => <div />} />
      </LanguageProvider>,
    );
    const tabs = screen.getAllByRole('button').map(t => t.textContent);
    expect(tabs).toEqual(['Console', 'Repository', 'Browser', 'Harness']);
  });
});
```

(If other buttons render inside ParentSurface, scope the query to the tab strip's container per the file's existing query style.)

- [ ] Run and confirm FAIL (alphabetical order comes back):

```
pnpm -C crates/vox-gui/ui test src/components/layout/ParentSurface.test.tsx
```

- [ ] In `ParentSurface.tsx`, sort tabs through the SSOT order. Replace the `tabs` memo (lines 24–30):

```typescript
import { orderedChildren } from '../../lib/navigation';

  const tabs = useMemo(() => {
    const raw = SURFACE_REGISTRY
      .filter(e => e.parentSurface === parentKey && e.viewKey && e.navLabel)
      .map(e => ({ viewKey: e.viewKey as string, label: labelFor(e.viewKey as string, lang) }));
    const order = orderedChildren(parentKey, raw.map(t => t.viewKey));
    const rank = new Map(order.map((k, i) => [k, i]));
    return [...raw].sort((a, b) => (rank.get(a.viewKey) ?? 0) - (rank.get(b.viewKey) ?? 0));
  }, [parentKey, lang]);
```

- [ ] Run and confirm PASS, then full suite:

```
pnpm -C crates/vox-gui/ui test src/components/layout/ParentSurface.test.tsx
pnpm -C crates/vox-gui/ui test
```

- [ ] Commit:

```
git add crates/vox-gui/ui/src/components/layout/ParentSurface.tsx crates/vox-gui/ui/src/components/layout/ParentSurface.test.tsx
git commit -m "feat(gui-ia): sub-tabs follow CHILD_ORDER_BY_PARENT intent order"
```

---

### Task 6: Discovery consolidation — one surface, timeline + inbox/review/archive presets

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Discovery/DiscoverySurface.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/Discovery/DiscoverySurface.test.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`
- `crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryInbox.tsx`

- [ ] Write the failing test `DiscoverySurface.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';
import { DISCOVERY_PRESET_SEED_KEY } from '../../../lib/navigation';

vi.mock('../Activity/ActivitySurface', () => ({
  ActivitySurface: () => <div data-testid="preset-timeline" />,
}));
vi.mock('../Scientia/DiscoveryInbox', () => ({
  DiscoveryInbox: () => <div data-testid="preset-inbox" />,
}));
vi.mock('../Scientia/DiscoveryReview', () => ({
  DiscoveryReview: () => <div data-testid="preset-review" />,
}));
vi.mock('../Scientia/ArchivePanel', () => ({
  ArchivePanel: () => <div data-testid="preset-archive" />,
}));

import { DiscoverySurface } from './DiscoverySurface';

const noopToast = () => {};

describe('DiscoverySurface', () => {
  beforeEach(() => window.localStorage.clear());

  it('defaults to the activity timeline', () => {
    render(<DiscoverySurface pushToast={noopToast} />);
    expect(screen.getByTestId('preset-timeline')).toBeInTheDocument();
  });

  it('switches presets via the tab strip', () => {
    render(<DiscoverySurface pushToast={noopToast} />);
    fireEvent.click(screen.getByRole('tab', { name: 'Inbox' }));
    expect(screen.getByTestId('preset-inbox')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Review' }));
    expect(screen.getByTestId('preset-review')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Archive' }));
    expect(screen.getByTestId('preset-archive')).toBeInTheDocument();
  });

  it('opens on the preset seeded by a legacy deep-link and consumes the seed', () => {
    window.localStorage.setItem(DISCOVERY_PRESET_SEED_KEY, 'review');
    render(<DiscoverySurface pushToast={noopToast} />);
    expect(screen.getByTestId('preset-review')).toBeInTheDocument();
    expect(window.localStorage.getItem(DISCOVERY_PRESET_SEED_KEY)).toBeNull();
  });
});
```

- [ ] Run and confirm FAIL (module not found):

```
pnpm -C crates/vox-gui/ui test src/components/surfaces/Discovery/DiscoverySurface.test.tsx
```

- [ ] Create `DiscoverySurface.tsx`. Note the honesty caveat: the inbox/review/archive presets host the existing scientia-command panels (their wired commands move, none drop — blueprint MERGE rule); the base preset is the `activity_query` timeline. All four share one surface key: `activity`.

```tsx
import React, { useState } from 'react';
import { ActivitySurface } from '../Activity/ActivitySurface';
import { DiscoveryInbox } from '../Scientia/DiscoveryInbox';
import { DiscoveryReview } from '../Scientia/DiscoveryReview';
import { ArchivePanel } from '../Scientia/ArchivePanel';
import { DISCOVERY_PRESET_SEED_KEY } from '../../../lib/navigation';
import type { Toast } from '../../../types/tauri';

export type DiscoveryPreset = 'timeline' | 'inbox' | 'review' | 'archive';

const PRESETS: Array<{ id: DiscoveryPreset; label: string }> = [
  { id: 'timeline', label: 'Timeline' },
  { id: 'inbox', label: 'Inbox' },
  { id: 'review', label: 'Review' },
  { id: 'archive', label: 'Archive' },
];

function consumeSeed(): DiscoveryPreset {
  try {
    const seed = window.localStorage.getItem(DISCOVERY_PRESET_SEED_KEY);
    if (seed === 'inbox' || seed === 'review' || seed === 'archive') {
      window.localStorage.removeItem(DISCOVERY_PRESET_SEED_KEY);
      return seed;
    }
  } catch {
    /* localStorage unavailable */
  }
  return 'timeline';
}

export interface DiscoverySurfaceProps {
  pushToast: (t: Toast) => void;
  gamifyEnabled?: boolean;
}

/**
 * One Discovery surface (view key `activity`) absorbing the four former
 * activity clones: Timeline (activity_query), Inbox, Review, Archive
 * (gui-ia-blueprint §4 MERGE: archive-panel/discovery-inbox/discovery-review → activity).
 */
export function DiscoverySurface({ pushToast, gamifyEnabled }: DiscoverySurfaceProps) {
  const [preset, setPreset] = useState<DiscoveryPreset>(consumeSeed);

  return (
    <div className="flex min-h-0 flex-col">
      <div role="tablist" aria-label="Discovery presets" className="flex gap-1 px-4 pt-3">
        {PRESETS.map(p => (
          <button
            key={p.id}
            type="button"
            role="tab"
            aria-selected={preset === p.id}
            onClick={() => setPreset(p.id)}
            className={`rounded-md px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.16em] transition ${
              preset === p.id
                ? 'bg-overlay-subtle text-brass'
                : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
            }`}
          >
            {p.label}
          </button>
        ))}
      </div>
      {preset === 'timeline' && <ActivitySurface pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
      {preset === 'inbox' && <DiscoveryInbox pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
      {preset === 'review' && <DiscoveryReview pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
      {preset === 'archive' && <ArchivePanel pushToast={pushToast} gamifyEnabled={gamifyEnabled} />}
    </div>
  );
}
```

- [ ] In `surfaceComponents.tsx`, repoint the `activity` case (lines 167–168):

```tsx
import { DiscoverySurface } from '../surfaces/Discovery/DiscoverySurface';
// …
    case 'activity':
      return <DiscoverySurface pushToast={props.pushToast} gamifyEnabled={props.gamifyEnabled} />;
```

Remove the now-unused `import { ActivitySurface } from '../surfaces/Activity/ActivitySurface';` from this file (it is imported by DiscoverySurface instead).

- [ ] In `decoratorRegistry.ts`, delete the three absorbed entries and their now-unused imports:

```typescript
// DELETE these lines from surfaceDecorators:
  'discovery-review': DiscoveryReview,
  'discovery-inbox': DiscoveryInbox,
  'archive-panel': ArchivePanel,
// DELETE the matching imports of DiscoveryReview, DiscoveryInbox, ArchivePanel.
```

- [ ] In `DiscoveryInbox.tsx` line 172, the "Open review" cross-navigation dispatches `view: 'discovery-review'` — repoint it at the surviving surface with the preset carried explicitly:

```typescript
        detail: { view: 'activity', publicationId },
```

and immediately before the `dispatchEvent`, seed the preset so the Discovery surface opens on Review:

```typescript
      try { window.localStorage.setItem('vox_discovery_preset_seed', 'review'); } catch { /* ignore */ }
```

(The existing `vox_discovery_review_seed` publication-id seed logic is untouched.)

- [ ] Run:

```
pnpm -C crates/vox-gui/ui test src/components/surfaces/Discovery/DiscoverySurface.test.tsx
pnpm -C crates/vox-gui/ui test
pnpm -C crates/vox-gui/ui typecheck
```

Expected: all pass (existing `ActivitySurface.test.tsx` / `ActivitySurface.container.test.tsx` still pass — the component is unchanged, only rehosted).

- [ ] Commit:

```
git add crates/vox-gui/ui/src/components/surfaces/Discovery crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/DiscoveryInbox.tsx
git commit -m "feat(gui-ia): consolidate 4 activity clones into one Discovery surface with presets"
```

---

### Task 7: Scientia merge — Findings surface with Dashboard | Claims tabs

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.test.tsx` (new)
- `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`

- [ ] Write the failing test `ScientiaSurface.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen, fireEvent } from '@testing-library/react';
import React from 'react';

vi.mock('./ScientiaDashboard', () => ({
  ScientiaDashboard: () => <div data-testid="scientia-dashboard" />,
}));
vi.mock('./ClaimsView', () => ({
  ClaimsView: () => <div data-testid="scientia-claims" />,
}));

import { ScientiaSurface } from './ScientiaSurface';

describe('ScientiaSurface (Findings)', () => {
  it('defaults to the dashboard and exposes a Claims tab (claims MERGE)', () => {
    render(<ScientiaSurface pushToast={() => {}} />);
    expect(screen.getByTestId('scientia-dashboard')).toBeInTheDocument();
    fireEvent.click(screen.getByRole('tab', { name: 'Claims' }));
    expect(screen.getByTestId('scientia-claims')).toBeInTheDocument();
  });
});
```

- [ ] Run and confirm FAIL:

```
pnpm -C crates/vox-gui/ui test src/components/surfaces/Scientia/ScientiaSurface.test.tsx
```

- [ ] Create `ScientiaSurface.tsx` (type-only import avoids a runtime cycle with `decoratorRegistry`):

```tsx
import React, { useState } from 'react';
import { ScientiaDashboard } from './ScientiaDashboard';
import { ClaimsView } from './ClaimsView';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';

const TABS = [
  { id: 'dashboard', label: 'Dashboard' },
  { id: 'claims', label: 'Claims' },
] as const;

type ScientiaTab = typeof TABS[number]['id'];

/**
 * Findings surface (view key `scientia`). Absorbs the former `claims` surface
 * as a tab — both shared the identical 12-command set and Scientia component
 * dir (gui-ia-blueprint §4 MERGE: claims + knowledge-surface → scientia).
 */
export function ScientiaSurface(props: SurfaceDecoratorProps) {
  const [tab, setTab] = useState<ScientiaTab>('dashboard');
  return (
    <div className="flex min-h-0 flex-col">
      <div role="tablist" aria-label="Findings sections" className="flex gap-1 px-4 pt-3">
        {TABS.map(t => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
            className={`rounded-md px-3 py-1.5 font-display text-[11px] uppercase tracking-[0.16em] transition ${
              tab === t.id
                ? 'bg-overlay-subtle text-brass'
                : 'text-text-muted hover:bg-overlay-hover hover:text-text-secondary'
            }`}
          >
            {t.label}
          </button>
        ))}
      </div>
      {tab === 'dashboard' ? <ScientiaDashboard {...props} /> : <ClaimsView {...props} />}
    </div>
  );
}
```

- [ ] In `decoratorRegistry.ts`:
  - Change `scientia: ScientiaDashboard,` → `scientia: ScientiaSurface,` and import `ScientiaSurface` from `./Scientia/ScientiaSurface`.
  - Delete the `claims: ClaimsView,` and `review: DiscoveryReviewView,` entries (both view keys are cut/aliased — `resolveNavigation` sends them to `scientia`), and delete the now-unused `ClaimsView`, `DiscoveryReviewView`, and `ScientiaDashboard` imports from this file.
- [ ] Run:

```
pnpm -C crates/vox-gui/ui test src/components/surfaces/Scientia/ScientiaSurface.test.tsx
pnpm -C crates/vox-gui/ui test
pnpm -C crates/vox-gui/ui typecheck
```

Expected: all pass (`ClaimsView.test.tsx` / `ScientiaDashboard.test.tsx` test the components directly and remain green).

- [ ] Commit:

```
git add crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.tsx crates/vox-gui/ui/src/components/surfaces/Scientia/ScientiaSurface.test.tsx crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
git commit -m "feat(gui-ia): merge claims into Findings (scientia) as a tab; drop phantom review decorator"
```

---

### Task 8: Fold Matrix into the chat execution rail (Routing drawer)

**Files:**
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx`
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.test.tsx`
- `crates/vox-gui/ui/src/components/surfaces/Chat/ChatSurface.tsx`
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`

- [ ] Update the failing test first. In `ChatExecutionRail.test.tsx`, the intent-button test (line ~157) currently asserts `expect(onNavigate).toHaveBeenCalledWith('matrix')`. Replace that test so it passes an `onOpenRouting` spy into the rendered `<ChatExecutionRail …/>` props and asserts:

```typescript
    const onOpenRouting = vi.fn();
    // render with the file's existing prop fixture, adding: onOpenRouting={onOpenRouting}
    // then, after clicking an intent button:
    expect(onOpenRouting).toHaveBeenCalledTimes(1);
    expect(onNavigate).not.toHaveBeenCalledWith('matrix');
```

Also update the Mesh-segment test in the same file (if it asserts `onNavigate('compute')`) to expect `onNavigate('mesh')` — mesh now lives under Agents.

- [ ] Run and confirm FAIL:

```
pnpm -C crates/vox-gui/ui test src/components/surfaces/Chat/ChatExecutionRail.test.tsx
```

- [ ] In `ChatExecutionRail.tsx`:
  - Add to `ChatExecutionRailProps`:

    ```typescript
    /** Opens the inline Routing panel (folded Matrix surface — gui-ia-blueprint: matrix → chat rail). */
    onOpenRouting?: () => void;
    ```
  - Destructure `onOpenRouting` in the component signature.
  - In the Intents section (line ~169), replace `onClick={() => onNavigate('matrix')}` with `onClick={() => onOpenRouting?.()}`.
  - In the Resources section, change the Mesh segment `onClick={() => onNavigate('compute')}` (line ~202) to `onClick={() => onNavigate('mesh')}`.
- [ ] In `ChatSurface.tsx`:
  - Import `Matrix`:

    ```typescript
    import { Matrix } from '../Matrix/Matrix';
    ```
  - Add drawer state near the other `useState` hooks:

    ```typescript
    const [routingOpen, setRoutingOpen] = useState(false);
    ```
  - Pass `onOpenRouting={() => setRoutingOpen(true)}` where `<ChatExecutionRail` is rendered (line ~186).
  - Render the drawer at the end of the surface's root JSX (sibling of the rail):

    ```tsx
    {routingOpen && (
      <div className="fixed inset-0 z-50" role="dialog" aria-modal="true" aria-label="Routing">
        <div className="absolute inset-0 bg-black/60" onClick={() => setRoutingOpen(false)} />
        <div className="absolute right-0 top-0 h-full w-[760px] max-w-full overflow-y-auto border-l border-border-subtle bg-bg-base shadow-2xl">
          <div className="flex items-center justify-between px-5 pt-4">
            <h2 className="font-display text-[13px] uppercase tracking-[0.2em] text-text-secondary">Routing</h2>
            <button
              type="button"
              aria-label="Close routing panel"
              onClick={() => setRoutingOpen(false)}
              className="rounded-md border border-border-subtle px-2 py-1 font-mono text-xs text-text-muted hover:bg-overlay-hover hover:text-text-primary"
            >
              ✕
            </button>
          </div>
          <Matrix pushToast={pushToast} />
        </div>
      </div>
    )}
    ```
- [ ] In `surfaceComponents.tsx`, delete `case 'matrix': return <Matrix …/>;` (lines 113–114) and the `import { Matrix } from '../surfaces/Matrix/Matrix';` (line 5). The `Matrix` component itself stays — it is now hosted by ChatSurface (`#view=matrix` already aliases to `chat` via Task 1).
- [ ] Run:

```
pnpm -C crates/vox-gui/ui test src/components/surfaces/Chat/ChatExecutionRail.test.tsx
pnpm -C crates/vox-gui/ui test
pnpm -C crates/vox-gui/ui typecheck
```

Expected: all pass.

- [ ] Commit:

```
git add crates/vox-gui/ui/src/components/surfaces/Chat crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
git commit -m "feat(gui-ia): fold matrix into chat as a Routing drawer on the execution rail"
```

---

### Task 9: Full verification sweep — suites, gates, stale-reference audit

**Files:** none new (verification only; fix-forward anything found)

- [ ] Full frontend suite and typecheck:

```
pnpm -C crates/vox-gui/ui test
pnpm -C crates/vox-gui/ui typecheck
```

Expected: `Test Files  N passed` with 0 failures; tsc silent.

- [ ] Registry gate and generator unit tests:

```
cargo run -p vox-cli --bin vox -- ci gui-surface-registry
cargo test -p vox-cli gui_surface_registry
```

Expected: `gui-surface-registry: registry and generated TS are up to date`; all Rust tests pass.

- [ ] Stale-reference audit — each grep below must return no hits in non-test, non-alias source (aliases in `navigation.ts`, legacy keys in `App.tsx` `LEGACY_VIEWS`, and lexicon legacy entries are the sanctioned survivors for the one-release alias window):

```
grep -rn "navigateTo('matrix')\|onNavigate('matrix')" crates/vox-gui/ui/src
grep -rn "case 'matrix'\|case 'claims'\|case 'review'" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
grep -rn "'discovery-inbox':\|'archive-panel':" crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
grep -rn "parentSurface: 'search'\|parentSurface: 'compute'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
```

Expected: first three return nothing; the fourth may match only `mens`/`populi`/`models`/`oratio` rows (`parentSurface: 'compute'` is correct for those — verify no `mesh`/`sub-agents` hits).

- [ ] Fix the blueprint's stale status metadata (audit finding 2026-07-02): in `docs/agents/gui-ia-blueprint.md`, the body §0 records RATIFIED (2026-06-26) but the YAML frontmatter line 4 still says `PRE-RATIFICATION — HUMAN GATE REQUIRED (Phase J)` and the title says "(pre-ratification)". Update both to reflect RATIFIED so future agents don't re-litigate the gate.

- [ ] Playwright smoke (if the environment can run it; otherwise record as a follow-up):

```
pnpm -C crates/vox-gui/ui test:e2e
```

Fix any spec that asserts the old sidebar order, "Runs & Approvals" label, or navigates to `matrix`/`claims`/`discovery-*` views (update assertions to the new topology; deep-link specs should still pass thanks to the alias map). Known instance (audit-verified): `crates/vox-gui/ui/e2e/dashboard.spec.ts:84` clicks `getByRole('button', { name: 'Runs & Approvals' })` — update to `{ name: 'Review' }`.

- [ ] Final commit for any audit fixes:

```
git add -A
git commit -m "chore(gui-ia): verification sweep fixes for intent-first reorg"
```

---

**Out-of-scope notes (flagged, not executed here):** `mercatus` is not mentioned in the ratified blueprint or the target nav; it is deliberately retained as a top-level view between Compute and Settings. Amendment A's VoxMens train/run CLI-parity GUI and Amendment B's Settings/Policies unification IA pass are separate authorized workstreams, not part of this reorg. The `vox_active_view` localStorage value from prior releases may hold a retired key; `resolveNavigation`'s alias map absorbs it at render time, so no migration code is needed.
