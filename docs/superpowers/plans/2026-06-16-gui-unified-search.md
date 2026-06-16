# vox-gui Unified Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** One `useSearchController` SSOT consumed by CommandPalette and SearchView — prefix routing, stale-request safety, `open_locator` navigation.

**Architecture:** Reducer in `searchController.ts`; hook in `useSearchController.ts` (landed `e4da27e`, scope token bump landed). Delete duplicate debounce/state in CommandPalette. `navigateFromLocator` in `navigation.ts` calls App's `navigateTo`.

**Tech Stack:** React 19, vitest, Playwright. Commands from `crates/vox-gui/ui/`.

---

## Completed

- [x] `searchReducer` + scope mapping
- [x] `useSearchController` hook + debounce tests
- [x] `setScopes` bumps `requestToken` (stale hit rejection)

---

## Task 2: Delete CommandPalette duplicate search state

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/CommandPalette.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/CommandPalette.test.tsx` (create if missing)

- [ ] **Step 1: Write failing test**

```typescript
import { vi } from 'vitest';
vi.mock('../../hooks/useSearchController', () => ({
  useSearchController: vi.fn(() => ({
    state: { query: 'vox', hits: [], loading: false, scopes: ['code'], requestToken: 1 },
    setQuery: vi.fn(),
    setScopes: vi.fn(),
  })),
}));

it('delegates backend search to useSearchController', () => {
  render(<CommandPalette open agents={[]} skills={[]} onClose={vi.fn()} onAction={vi.fn()} />);
  expect(useSearchController).toHaveBeenCalled();
});
```

- [ ] **Step 2: Run — FAIL**

- [ ] **Step 3: Replace local state**

Remove from CommandPalette:
- `backendHits`, `backendLoading`, `debounceRef`
- Manual `voxTransport.voxSearchQuery` effect

Add:
```typescript
const { state: searchState, setQuery: setSearchQuery } = useSearchController({
  enabled: open && !q.startsWith('>') && !q.startsWith('@'),
});
```

Map `searchState.hits` to `backendHits`; `searchState.loading` to loading indicator.

Wire palette input: `setQ` + `setSearchQuery` together in `onChange`.

- [ ] **Step 4: Run palette tests — PASS**

- [ ] **Step 5: Commit** `refactor(gui): CommandPalette uses useSearchController`

---

## Task 3: Prefix routing

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/paletteSources.ts`
- Modify: `crates/vox-gui/ui/src/components/layout/paletteSources.test.ts`

- [ ] **Step 1: Failing test**

```typescript
it('> prefix filters to CLI catalog only', () => {
  const items = buildPaletteItems({ q: '> ci', agents: [], skills: catalog, docs: [], backendHits: [] });
  expect(items.every(i => i.source === 'commands' || i.kind === 'command')).toBe(true);
});
```

- [ ] **Step 2: Implement in `buildPaletteItems`**

```typescript
const prefix = q.match(/^([>@/])\s*(.*)/);
if (prefix?.[1] === '>') {
  q = prefix[2];
  // skills/agents/docs branches skipped
}
```

| Prefix | Source |
|--------|--------|
| `>` | CLI catalog + manifest safety badges |
| `@` | Agents list |
| `/` | Skills + docs index |

- [ ] **Step 3: Safety badge** — read `tier` / `safety_class` from manifest entry when rendering CLI row

- [ ] **Step 4: Commit**

---

## Task 4: SearchView consumes hook

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/Search/SearchView.test.tsx`

- [ ] **Step 1: Failing test for loading skeleton**

```typescript
it('shows skeleton while searchState.loading', () => {
  vi.mocked(useSearchController).mockReturnValue({
    state: { query: 'foo', hits: [], loading: true, scopes: ['code'], requestToken: 1 },
    setQuery: vi.fn(),
    setScopes: vi.fn(),
  });
  render(<SearchView pushToast={vi.fn()} />);
  expect(screen.getAllByTestId('search-skeleton').length).toBeGreaterThan(0);
});
```

- [ ] **Step 2: Replace local fetch with hook + `<Async>`**

- [ ] **Step 3: MemoryView recall** — replace `invoke('vox_search_query')` with `voxTransport.voxSearchQuery` (shrink ipc allowlist)

- [ ] **Step 4: Commit**

---

## Task 5: `navigateFromLocator` + Playwright

**Files:**
- Create: `crates/vox-gui/ui/src/lib/locatorNavigation.ts`
- Create: `crates/vox-gui/ui/src/lib/locatorNavigation.test.ts`
- Create: `crates/vox-gui/ui/e2e/palette-search-navigate.spec.ts`

- [ ] **Step 1: Failing unit test**

```typescript
import { viewKeyForLocator } from './locatorNavigation';

it('maps file locator to repository', () => {
  expect(viewKeyForLocator({ kind: 'file', value: 'src/foo.rs' })).toBe('repository');
});

it('maps command locator to catalog', () => {
  expect(viewKeyForLocator({ kind: 'command', value: 'vox ci check' })).toBe('catalog');
});
```

- [ ] **Step 2: Implement**

```typescript
export function viewKeyForLocator(locator: { kind: string; value: string }): string {
  switch (locator.kind) {
    case 'file': return 'repository';
    case 'web': return 'browser';
    case 'command': return 'catalog';
    case 'surface': return locator.value;
    default: return 'search';
  }
}
```

- [ ] **Step 3: Wire palette `onAction`** — when hit has locator, call `voxTransport.openLocator` OR `navigateTo(viewKeyForLocator(hit.locator))`

- [ ] **Step 4: Playwright** — open palette, type query, select hit, assert Console or Repository visible

- [ ] **Step 5: Commit**

---

## Exit criteria

- [ ] No duplicate `voxSearchQuery` debounce outside `useSearchController`
- [ ] Prefix routing tested in `paletteSources.test.ts`
- [ ] SearchView loading/error via `<Async>`
- [ ] Playwright palette → navigate green
- [ ] `searchController.test.ts` + `useSearchController.test.ts` + `locatorNavigation.test.ts` all pass
