# Axis Frontend Review Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Execute `docs/superpowers/specs/2026-07-18-axis-frontend-review-harness-design.md`: kill the raw `__TAURI_INTERNALS__` TypeError class with a transport-level guard, build the full-matrix review-bundle capture harness (surfaces × states × 3 viewports × chromium+firefox with rich mocks, axe-core, icon/overflow/console audits), extend `visus_review` with a defect-focused bundle analysis mode (occlusion/clipping/icon/error-leak rubric, cached), wire it as the post-merge advisory CI analysis, and then run the whole pipeline to produce the comprehensive tab-by-tab review + coverage audit.

**Architecture:** Frontend work lives in `crates/vox-gui/ui` (React 19 + TS, pnpm, vitest, Playwright). One new module `src/lib/backendGuard.ts` is the single source of truth for backend detection + the typed error; `transport.ts` routes all IPC through `safeInvoke`/`safeListen` defined between marker comments so a source-scan guard can prove no raw Tauri calls exist elsewhere. The capture harness (`e2e/review/`) appends one JSONL line per capture (parallel-worker-safe, no shared-file races); the Rust analyzer (`crates/vox-orchestrator-mcp/src/visus_review/`, feature `gui-visual-review`) reads `entries-*.jsonl` directly in a new bundle mode, reusing the Phase-3 cache (sha256+model+prompt-version keys). A `scripts/frontend-review.vox` wrapper chains capture → analysis (VoxScript-only glue per AGENTS.md).

**Tech Stack:** TypeScript/React/vitest/Playwright (`@axe-core/playwright` new devDependency), Rust (serde/tokio, OpenRouter vision via the existing visus_review client), VoxScript, GitHub Actions YAML.

**Ground rules (Windows / repo policy):**
- Frontend commands run from `C:\Users\Owner\vox\crates\vox-gui\ui` via **pnpm** (never npm).
- Rust: **never** `cargo fmt --all` — `cargo fmt -p vox-orchestrator-mcp` only. Never pipe cargo output to `head`/`grep` — redirect to a file (`> "$env:TEMP\x.log" 2>&1`) and read it.
- New automation glue is VoxScript (`scripts/*.vox`), not `.ps1`/`.sh`/`.py`; `package.json` scripts are fine.
- CI edits touch only the `gui-playwright-smoke` job's advisory steps; **never** touch `ci-summary.needs` or add PR triggers (fork F2).
- Commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

**Verified ground truth (2026-07-18):**
- `transport.ts`: imports `invoke` from `@tauri-apps/api/core` and `listen` from `@tauri-apps/api/event`; ~60 `invoke`/`invoke<` call sites, one `listen<` inside `listenOrchStatus` (module scope) plus event-listener wrappers; singleton `voxTransport` exported at line ~606.
- `playwright.config.ts`: single `chromium` project; `webServer` boots Vite on 1420.
- `package.json` scripts: `dev`, `typecheck`, `test` (vitest run), `test:e2e` (playwright test). devDependencies do NOT yet include `@axe-core/playwright`.
- `SURFACE_REGISTRY` (`src/generated/surfaceRegistry.generated.ts`): entries `{ viewKey, cliGroup, tier, navLabel, navIcon, navGroup, parentSurface }`; ~25 surfaces with non-null viewKey.
- `visus_review/mod.rs`: `ManifestEntry { view_key, file, sha256, capture_ms }` (serde renames `viewKey`/`captureMs`), `Manifest { total_capture_ms, surfaces }`, `RunArgs { manifest_path, screens_dir, cache_path, report_dir, now_iso, do_ai }`, `run(&RunArgs) -> RunReport`, `CACHE_SCHEMA_VERSION = 1`, `decide_status(cache, key, sha, model, prompt_version)`, `prune_dead_views`. `prompt.rs`: `PROMPT_VERSION = "2026-07-16.1"`, `RUBRIC`, `system_prompt()`, `user_prompt(view_key)`. Bin: `src/bin/gui-visual-review.rs` with `--manifest/--screens/--cache/--report-dir/--date/--now/--ai` args.
- Existing e2e mock stack: `e2e/lib/tauriMockShared.ts` (`addMockInitScript`, `mockInitScript`, `runInstallerWithShared`, shared `seedMockEnvironment`/`eventPluginResponse`/`bootstrapResponse`), `e2e/lib/tauriMock.ts` (`installTauriMock`, rich-ish single dataset + stateful `__MOCK_*` stores).
- Source-scan guard idiom: `src/guards/ipcBoundaries.test.ts`, `src/guards/surfaceRegistryEscape.test.ts`.

---

## Task 1: `backendGuard.ts` — detection, typed error, rejection filter (Phase A)

**Files:**
- Create: `crates/vox-gui/ui/src/lib/backendGuard.ts`
- Test: `crates/vox-gui/ui/src/lib/backendGuard.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// crates/vox-gui/ui/src/lib/backendGuard.test.ts
import { describe, it, expect, vi, afterEach } from 'vitest';
import {
  backendAvailable,
  BackendUnavailableError,
  makeBackendUnavailableRejectionFilter,
  __resetBackendAvailabilityForTests,
} from './backendGuard';

afterEach(() => {
  delete (globalThis as any).window;
  __resetBackendAvailabilityForTests();
});

describe('backendAvailable', () => {
  it('is false when window has no __TAURI_INTERNALS__', () => {
    (globalThis as any).window = {};
    expect(backendAvailable()).toBe(false);
  });
  it('is true when __TAURI_INTERNALS__ exists', () => {
    (globalThis as any).window = { __TAURI_INTERNALS__: {} };
    expect(backendAvailable()).toBe(true);
  });
  it('memoizes: flipping the window later does not change the answer', () => {
    (globalThis as any).window = {};
    expect(backendAvailable()).toBe(false);
    (globalThis as any).window = { __TAURI_INTERNALS__: {} };
    expect(backendAvailable()).toBe(false); // memoized per app load
  });
});

describe('BackendUnavailableError', () => {
  it('carries the command and an honest message', () => {
    const e = new BackendUnavailableError('chat_list_sessions');
    expect(e.command).toBe('chat_list_sessions');
    expect(e.message).toContain("desktop backend");
    expect(e.message).toContain('chat_list_sessions');
    expect(e).toBeInstanceOf(Error);
  });
});

describe('makeBackendUnavailableRejectionFilter', () => {
  it('preventDefaults rejections of BackendUnavailableError only', () => {
    const filter = makeBackendUnavailableRejectionFilter();
    const prevented = { reason: new BackendUnavailableError('x'), preventDefault: vi.fn() };
    const passed = { reason: new TypeError('boom'), preventDefault: vi.fn() };
    filter(prevented as unknown as PromiseRejectionEvent);
    filter(passed as unknown as PromiseRejectionEvent);
    expect(prevented.preventDefault).toHaveBeenCalledOnce();
    expect(passed.preventDefault).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run it RED**

Run (from `crates/vox-gui/ui`): `pnpm exec vitest run src/lib/backendGuard.test.ts`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```ts
// crates/vox-gui/ui/src/lib/backendGuard.ts
/**
 * Single source of truth for "is the Tauri desktop backend present?".
 *
 * In a plain browser (dev server, Firefox at localhost:1420) there is no
 * `window.__TAURI_INTERNALS__`, and every raw `invoke`/`listen` from
 * @tauri-apps/api throws `TypeError: can't access property "invoke",
 * window.__TAURI_INTERNALS__ is undefined`. transport.ts routes ALL IPC
 * through safeInvoke/safeListen which consult this module and reject with
 * the typed, honest BackendUnavailableError instead.
 */

let cached: boolean | null = null;

/** Memoized per app load: the host cannot appear after startup. */
export function backendAvailable(): boolean {
  if (cached === null) {
    cached =
      typeof window !== 'undefined' &&
      '__TAURI_INTERNALS__' in (window as unknown as Record<string, unknown>);
  }
  return cached;
}

/** Test-only escape hatch (memoization would leak across vitest cases). */
export function __resetBackendAvailabilityForTests(): void {
  cached = null;
}

export class BackendUnavailableError extends Error {
  readonly command: string;
  constructor(command: string) {
    super(
      `Axis is running without its desktop backend — '${command}' unavailable. ` +
        `(Browser preview mode: data surfaces show empty states.)`,
    );
    this.name = 'BackendUnavailableError';
    this.command = command;
  }
}

const loggedCommands = new Set<string>();

/**
 * window 'unhandledrejection' filter: swallow BackendUnavailableError (log
 * once per command at debug level) so no uncaught degradation path can spam
 * the console or surface a raw error overlay. Everything else passes through.
 */
export function makeBackendUnavailableRejectionFilter(): (ev: PromiseRejectionEvent) => void {
  return (ev) => {
    if (ev.reason instanceof BackendUnavailableError) {
      ev.preventDefault();
      if (!loggedCommands.has(ev.reason.command)) {
        loggedCommands.add(ev.reason.command);
        console.debug('[backendGuard] suppressed (browser mode):', ev.reason.command);
      }
    }
  };
}

export function installBackendUnavailableRejectionFilter(): void {
  if (typeof window === 'undefined') return;
  window.addEventListener('unhandledrejection', makeBackendUnavailableRejectionFilter());
}
```

- [ ] **Step 4: Run it GREEN**

`pnpm exec vitest run src/lib/backendGuard.test.ts` — expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/backendGuard.ts crates/vox-gui/ui/src/lib/backendGuard.test.ts
git commit -m "feat(gui): backendGuard - backend detection, BackendUnavailableError, rejection filter" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 2: Route ALL transport IPC through `safeInvoke`/`safeListen` + source-scan guard (Phase A)

**Files:**
- Modify: `crates/vox-gui/ui/src/transport.ts` (every `invoke`/`listen` call site)
- Create: `crates/vox-gui/ui/src/guards/transportIpcGuard.test.ts`
- Modify (if they call raw invoke/listen directly): none expected — other files go through `voxTransport` or component-level `invoke` imports that stay out of scope for this task (component-level direct invokes are already tracked by `ipcBoundaries.test.ts`).

- [ ] **Step 1: Write the failing source-scan guard**

```ts
// crates/vox-gui/ui/src/guards/transportIpcGuard.test.ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * Phase A guard: transport.ts is the ONLY place allowed to touch raw Tauri
 * IPC, and inside it only the marked safeInvoke/safeListen block may do so.
 * Everything else must call safeInvoke/safeListen so bare-browser mode
 * rejects with BackendUnavailableError instead of a raw TypeError
 * ("can't access property invoke, window.__TAURI_INTERNALS__ is undefined").
 */
const SRC = readFileSync(join(import.meta.dirname, '../transport.ts'), 'utf8');

describe('transport raw-IPC containment', () => {
  const begin = SRC.indexOf('// __VOX_RAW_IPC_BEGIN__');
  const end = SRC.indexOf('// __VOX_RAW_IPC_END__');

  it('has exactly one marked raw-IPC region', () => {
    expect(begin).toBeGreaterThan(-1);
    expect(end).toBeGreaterThan(begin);
    expect(SRC.indexOf('// __VOX_RAW_IPC_BEGIN__', begin + 1)).toBe(-1);
  });

  it('no raw invoke( / invoke< / listen( / listen< outside the marked region', () => {
    const outside = SRC.slice(0, begin) + SRC.slice(end);
    // Match bare identifiers only (not safeInvoke/safeListen/unlisten).
    const offenders = [...outside.matchAll(/(?<![A-Za-z_$.])(invoke|listen)\s*[(<]/g)].map(
      (m) => m[0],
    );
    expect(offenders, `raw IPC outside safe wrappers: ${JSON.stringify(offenders)}`).toEqual([]);
  });
});
```

- [ ] **Step 2: Run it RED**

`pnpm exec vitest run src/guards/transportIpcGuard.test.ts`
Expected: FAIL — no marker region exists yet (first test), and ~61 offenders (second).

- [ ] **Step 3: Add the safe wrappers to `transport.ts`**

Directly below the existing imports (`invoke` from `@tauri-apps/api/core`, `listen`/`UnlistenFn` from `@tauri-apps/api/event`), add:

```ts
import { backendAvailable, BackendUnavailableError } from './lib/backendGuard';

// __VOX_RAW_IPC_BEGIN__
// The ONLY permitted uses of raw Tauri `invoke`/`listen` in the frontend.
// Guarded by src/guards/transportIpcGuard.test.ts.
function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!backendAvailable()) {
    return Promise.reject(new BackendUnavailableError(cmd));
  }
  return invoke<T>(cmd, args);
}

function safeListen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  if (!backendAvailable()) {
    return Promise.reject(new BackendUnavailableError(`listen:${event}`));
  }
  return listen<T>(event, handler);
}
// __VOX_RAW_IPC_END__
```

- [ ] **Step 4: Mechanically rewrite every call site in `transport.ts`**

- Every `invoke<T>('cmd', …)` → `safeInvoke<T>('cmd', …)`; every untyped `invoke('cmd', …)` → `safeInvoke('cmd', …)`. (~60 sites; do it with careful find/replace of `invoke<` → `safeInvoke<` and `invoke(` → `safeInvoke(`, then fix the two definitions inside the marker block back to raw — the marker block is written last to avoid self-clobbering, or apply replacements only outside the block.)
- `listenOrchStatus` and every other `listen<...>(...)` wrapper → `safeListen<...>(...)`.
- Do NOT change signatures, generics, or argument objects — reject-vs-throw behavior is the only semantic change, and only in browser mode.

- [ ] **Step 5: Guard GREEN + full frontend suite**

```
pnpm exec vitest run src/guards/transportIpcGuard.test.ts   # expected: 2 passed
pnpm typecheck                                              # expected: clean
pnpm test                                                   # expected: all pass (existing transport.test.ts mocks @tauri-apps/api, so mocked invoke still flows through safeInvoke)
```

If `transport.test.ts` fails because `backendAvailable()` is false under jsdom (no `__TAURI_INTERNALS__`), add to that file's setup (and any other failing suite) the stub:

```ts
beforeEach(() => {
  (window as any).__TAURI_INTERNALS__ = (window as any).__TAURI_INTERNALS__ ?? {};
});
```

plus `__resetBackendAvailabilityForTests()` from `./lib/backendGuard` in `afterEach` where the availability answer matters. Prefer stubbing in the shared vitest setup file if more than 3 suites need it (check `vitest`/`vite.config` `setupFiles` and add `(globalThis as any).window && ((window as any).__TAURI_INTERNALS__ ??= {})` there once instead).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/guards/transportIpcGuard.test.ts
git commit -m "fix(gui): route all transport IPC through safeInvoke/safeListen (browser-mode honesty)" -m "Raw __TAURI_INTERNALS__ TypeErrors become typed BackendUnavailableError at one choke point; source-scan guard forbids raw IPC outside the marked region." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 3: Browser-mode banner + rejection-filter installation (Phase A)

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/BackendBanner.tsx`
- Test: `crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx`
- Modify: `crates/vox-gui/ui/src/main.tsx` (install rejection filter)
- Modify: `crates/vox-gui/ui/src/App.tsx` (render banner near the toasts container)

- [ ] **Step 1: Failing component test**

```tsx
// crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';
import { BackendBanner } from './BackendBanner';
import { __resetBackendAvailabilityForTests } from '../../lib/backendGuard';

afterEach(() => {
  cleanup();
  __resetBackendAvailabilityForTests();
  delete (window as any).__TAURI_INTERNALS__;
});

describe('BackendBanner', () => {
  it('renders when the backend is unavailable and dismisses on click', () => {
    __resetBackendAvailabilityForTests();
    render(<BackendBanner />);
    const banner = screen.getByRole('status', { name: /browser preview/i });
    expect(banner).toBeInTheDocument();
    fireEvent.click(screen.getByRole('button', { name: /dismiss/i }));
    expect(screen.queryByRole('status', { name: /browser preview/i })).toBeNull();
  });

  it('renders nothing when the backend is present', () => {
    (window as any).__TAURI_INTERNALS__ = {};
    __resetBackendAvailabilityForTests();
    render(<BackendBanner />);
    expect(screen.queryByRole('status', { name: /browser preview/i })).toBeNull();
  });
});
```

- [ ] **Step 2: RED** — `pnpm exec vitest run src/components/ui/BackendBanner.test.tsx` → module not found.

- [ ] **Step 3: Implement**

```tsx
// crates/vox-gui/ui/src/components/ui/BackendBanner.tsx
import React, { useState } from 'react';
import { backendAvailable } from '../../lib/backendGuard';

/** Persistent, dismissible honesty banner for bare-browser (no-Tauri) mode. */
export function BackendBanner() {
  const [dismissed, setDismissed] = useState(false);
  if (backendAvailable() || dismissed) return null;
  return (
    <div
      role="status"
      aria-label="Browser preview mode"
      className="fixed inset-x-0 top-0 z-[100] flex items-center justify-center gap-3 border-b border-amber-500/40 bg-amber-950/90 px-4 py-1.5 text-[12px] text-amber-200"
    >
      <span>
        Browser preview — no desktop backend connected; surfaces show empty states.
      </span>
      <button
        type="button"
        aria-label="Dismiss browser preview notice"
        onClick={() => setDismissed(true)}
        className="rounded px-1.5 text-amber-300 hover:bg-amber-900/60"
      >
        ×
      </button>
    </div>
  );
}
```

- [ ] **Step 4: Wire it**

- `src/main.tsx`: import and call `installBackendUnavailableRejectionFilter()` from `./lib/backendGuard` once, before `createRoot(...)` renders.
- `src/App.tsx`: render `<BackendBanner />` as a sibling adjacent to the global toasts container (search for the `Toasts` render in App's return; place `<BackendBanner />` immediately before it). Import from `./components/ui/BackendBanner`.

- [ ] **Step 5: GREEN + smoke in a real browser**

```
pnpm exec vitest run src/components/ui/BackendBanner.test.tsx   # expected: 2 passed
pnpm typecheck && pnpm test                                     # expected: clean / all pass
```

Then live proof (the original bug's reproduction): with the dev server on 1420, load the app in a plain browser and confirm (a) the amber banner shows, (b) the console contains `[backendGuard] suppressed (browser mode): <cmd>` debug lines but **zero** raw `__TAURI_INTERNALS__` TypeErrors. Automatable check:

```
pnpm exec playwright test e2e/error-states.spec.ts --project=chromium   # regression: still 4 passed
```

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/BackendBanner.tsx crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx crates/vox-gui/ui/src/main.tsx crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): browser-mode honesty banner + BackendUnavailable rejection filter install" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 4: Rich overflow mock dataset (`tauriMockRich.ts`) (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/lib/tauriMockRich.ts`
- Test: `crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts`

- [ ] **Step 1: Failing test**

```ts
// crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts
import { describe, it, expect } from 'vitest';
import { RICH_DATASET, installTauriMockRich } from './tauriMockRich';
import { runInstallerWithShared } from './tauriMockShared';

function withFakeWindow<T>(fn: (win: any) => T): T {
  const prev = (globalThis as any).window;
  const win: any = { localStorage: { setItem() {}, getItem: () => null } };
  (globalThis as any).window = win;
  try {
    return fn(win);
  } finally {
    (globalThis as any).window = prev;
  }
}

describe('RICH_DATASET density', () => {
  it('is dense and overflow-shaped (sparse mocks are why occlusion hid)', () => {
    expect(RICH_DATASET.hopperTasks.length).toBeGreaterThanOrEqual(40);
    expect(RICH_DATASET.chatSessions.length).toBeGreaterThanOrEqual(12);
    expect(RICH_DATASET.models.length).toBeGreaterThanOrEqual(30);
    expect(Math.max(...RICH_DATASET.hopperTasks.map((t) => t.intent.length))).toBeGreaterThanOrEqual(120);
    const all = JSON.stringify(RICH_DATASET);
    expect(all).toMatch(/[Ѐ-ӿ]/); // cyrillic sample
    expect(all).toMatch(/[֐-׿]/); // RTL (hebrew) sample
  });
});

describe('installTauriMockRich', () => {
  it('answers the dense list commands through the shared bootstrap', async () => {
    await withFakeWindow(async (win) => {
      runInstallerWithShared(installTauriMockRich, 'tasks');
      const tasks = await win.__TAURI_INTERNALS__.invoke('hopper_list');
      expect(tasks.length).toBeGreaterThanOrEqual(40);
      const sessions = await win.__TAURI_INTERNALS__.invoke('chat_list_sessions');
      expect(sessions.length).toBeGreaterThanOrEqual(12);
      expect(await win.__TAURI_INTERNALS__.invoke('get_initial_view')).toBe('tasks');
    });
  });
});
```

- [ ] **Step 2: RED** — `pnpm exec vitest run e2e/lib/tauriMockRich.test.ts` → module not found.

- [ ] **Step 3: Implement**

`installTauriMockRich` follows the exact installer contract of `installEmptyStateMock` (guard on `window.__VOX_MOCK_SHARED__`, must be injected via `addMockInitScript`, fully self-contained function body — `RICH_DATASET` is therefore built by a factory that is stringified INTO the installer, or simpler: defined inside the installer function and re-exported by calling the builder at module scope). Shape:

```ts
// crates/vox-gui/ui/e2e/lib/tauriMockRich.ts
/**
 * Dense, overflow-shaped mock dataset for the review-bundle capture matrix.
 * Sparse mocks are why occlusion/clipping never showed in screenshots:
 * realistic density (40+ tasks, 120+-char titles, unicode/RTL, many models)
 * is what makes truncation, overlap, and z-fighting visible.
 *
 * Injection contract identical to tauriMockVariants installers:
 *   await addMockInitScript(page, installTauriMockRich, viewKey)
 */

export function buildRichDataset() {
  const long = (s: string, n: number) => s.repeat(Math.ceil(n / s.length)).slice(0, n);
  const hopperTasks = Array.from({ length: 44 }, (_, i) => ({
    item_id: `hop-rich-${i + 1}`,
    intent:
      i % 7 === 0
        ? long(`Refactor the international pipeline № ${i} — очень длинное название задачи с юникодом ` , 140)
        : i % 5 === 0
          ? `משימה ${i} — bidirectional text sample with a fairly long tail describing acceptance criteria in detail`
          : long(`Task ${i}: implement, verify, and document the surface behavior across viewports `, 120 + (i % 40)),
    priority: i % 3,
    state: i % 6 === 0 ? 'done' : i % 4 === 0 ? 'assigned' : 'inbox',
    task_id: 9100 + i,
    session_id: i % 2 ? `gui-rich-${i}` : null,
    agent_id: i % 3 ? `agent-${i}` : null,
    remote_node: i % 5 === 0 ? 'node-remote-very-long-hostname.example.internal' : null,
  }));
  const chatSessions = Array.from({ length: 14 }, (_, i) => ({
    session_id: `rich-session-${i + 1}`,
    title: long(`Session ${i + 1}: exploratory conversation about the architecture refactor and its long-term implications `, 90 + i * 4),
    message_count: 3 + i * 7,
    updated_at: 'now',
    conversation_id: i + 1,
  }));
  const models = Array.from({ length: 32 }, (_, i) => ({
    id: `provider-${i % 6}/model-family-name-${i}-with-a-rather-long-suffix-v${i}.${i % 10}`,
    provider: ['openai', 'anthropic', 'google', 'ollama', 'mistralai', 'meta-llama'][i % 6],
    tier: ['Frontier', 'Fast', 'Budget'][i % 3],
    cost_per_1k: i * 0.0007,
    max_tokens: 8192 * ((i % 4) + 1),
    is_free: i % 8 === 0,
    latency_p50_ms: 200 + i * 13,
    success_rate: 0.9 + (i % 10) / 100,
    quality_score: 0.5 + (i % 50) / 100,
  }));
  return { hopperTasks, chatSessions, models };
}

export const RICH_DATASET = buildRichDataset();

export function installTauriMockRich(viewKey: string): void {
  const shared = (window as any).__VOX_MOCK_SHARED__;
  if (!shared) {
    throw new Error('installTauriMockRich must be injected via addMockInitScript (tauriMockShared.ts)');
  }
  // NOTE: this function body is serialized into the page — it may reference
  // only `shared`, `viewKey`, and its own locals. The dataset builder is
  // duplicated inline by calling the same factory source attached below.
  const data = (installTauriMockRich as any).__buildRichDataset
    ? (installTauriMockRich as any).__buildRichDataset()
    : (window as any).__VOX_RICH_BUILD__();
  // ... (see Step 3 note)
}
```

**Implementation note (do it this way, not the sketch above):** function serialization means the installer cannot close over module scope. The clean pattern — mirror how `tauriMockShared` solves this: extend `mockInitScript` composition. Add to `tauriMockShared.ts`'s `SHARED_SNIPPET` an optional third helper `buildRichDataset` is NOT appropriate (shared is for all mocks). Instead give `tauriMockRich.ts` its own composer:

```ts
export async function addRichMockInitScript(page: Page, viewKey: string): Promise<void> {
  const content = [
    `window.__VOX_RICH_BUILD__ = ${buildRichDataset.toString()};`,
    mockInitScript(installTauriMockRich, viewKey),
  ].join('\n');
  await page.addInitScript({ content });
}
```

with `installTauriMockRich` reading `const data = (window as any).__VOX_RICH_BUILD__();`, then: `shared.seedMockEnvironment(viewKey)`, and an `invoke` that answers `hopper_list` → `data.hopperTasks`, `chat_list_sessions` → `data.chatSessions`, `list_model_cards` → `data.models`, `inference_provider_status` → a 6-provider list with mixed availability, `get_gamify_settings` → `{ enabled: true, mode: 'balanced' }`, event-plugin via `shared.eventPluginResponse`, everything else via `shared.bootstrapResponse(cmd, viewKey)`. Export `RICH_DATASET = buildRichDataset()` for tests. For the vitest path, make `runInstallerWithShared` compatible by also setting `(globalThis as any).window.__VOX_RICH_BUILD__ = buildRichDataset` inside the test's `withFakeWindow` — adjust the Step-1 test accordingly when implementing.

- [ ] **Step 4: GREEN** — `pnpm exec vitest run e2e/lib/tauriMockRich.test.ts`; then `pnpm typecheck`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/lib/tauriMockRich.ts crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts
git commit -m "feat(gui-e2e): dense overflow-shaped rich mock dataset for the review matrix" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 5: State registry + completeness guard (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/review/states.ts`
- Create: `crates/vox-gui/ui/src/guards/reviewStates.guard.test.ts`

- [ ] **Step 1: Failing guard**

```ts
// crates/vox-gui/ui/src/guards/reviewStates.guard.test.ts
import { describe, it, expect } from 'vitest';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { statesFor, SURFACE_STATES, VIEWPORTS } from '../../e2e/review/states';

describe('review state registry completeness', () => {
  it('every registry surface has at least the default state', () => {
    const missing = SURFACE_REGISTRY.filter((e) => e.viewKey != null).filter(
      (e) => statesFor(e.viewKey as string).length === 0,
    );
    expect(missing.map((e) => e.viewKey)).toEqual([]);
  });
  it('declared extra states only reference registered surfaces (no typo rot)', () => {
    const known = new Set(
      SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string),
    );
    const unknown = Object.keys(SURFACE_STATES).filter((k) => !known.has(k));
    expect(unknown).toEqual([]);
  });
  it('viewports are the spec trio', () => {
    expect(VIEWPORTS.map((v) => v.name)).toEqual(['wide', 'laptop', 'compact']);
  });
});
```

- [ ] **Step 2: RED** — `pnpm exec vitest run src/guards/reviewStates.guard.test.ts` → module not found.

- [ ] **Step 3: Implement**

```ts
// crates/vox-gui/ui/e2e/review/states.ts
import type { Page } from '@playwright/test';

export interface ReviewViewport { name: 'wide' | 'laptop' | 'compact'; width: number; height: number; }
export const VIEWPORTS: ReviewViewport[] = [
  { name: 'wide', width: 1440, height: 900 },
  { name: 'laptop', width: 1100, height: 720 },
  { name: 'compact', width: 900, height: 600 },
];

export interface ReviewState {
  name: string;
  /** Drive the page into the state AFTER the surface has rendered. */
  setup?: (page: Page) => Promise<void>;
}

const DEFAULT: ReviewState = { name: 'default' };

/**
 * Surface-specific interaction states. Selector ground truth: verify each
 * against the current component before trusting (they are correct as of
 * 2026-07-18; the capture spec treats a failed setup as a captured finding,
 * not a hard test failure — see capture.spec.ts).
 */
export const SURFACE_STATES: Record<string, ReviewState[]> = {
  chat: [
    DEFAULT,
    {
      name: 'model-picker-open',
      setup: async (p) => { await p.getByRole('button', { name: /^model:/i }).click(); },
    },
    {
      name: 'session-menu-open',
      setup: async (p) => { await p.getByRole('button', { name: /session actions for/i }).first().click(); },
    },
    {
      name: 'composer-filled',
      setup: async (p) => {
        await p.getByLabel('Task composer').fill(
          'A deliberately long composer draft that should wrap across multiple lines and reveal any clipping or overlap issues in the dock '.repeat(2),
        );
      },
    },
  ],
  tasks: [
    DEFAULT,
    {
      name: 'composer-filled',
      setup: async (p) => {
        await p.getByLabel('Add a task').fill(
          'Draft task with an intentionally very long title to probe truncation and row overflow behavior in the composer',
        );
      },
    },
  ],
  settings: [
    DEFAULT,
    {
      name: 'search-filtered',
      setup: async (p) => { await p.getByLabel('Search settings').fill('key'); },
    },
  ],
  approvals: [DEFAULT],
  dashboard: [
    DEFAULT,
    {
      name: 'omnibar-open',
      setup: async (p) => { await p.keyboard.press('Control+k'); },
    },
    {
      name: 'sidebar-collapsed',
      setup: async (p) => { await p.getByRole('button', { name: 'Collapse sidebar' }).click(); },
    },
  ],
};

/** Every surface gets at least `default`; extras come from SURFACE_STATES. */
export function statesFor(viewKey: string): ReviewState[] {
  return SURFACE_STATES[viewKey] ?? [DEFAULT];
}
```

- [ ] **Step 4: GREEN** — guard passes (3/3); `pnpm typecheck`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/review/states.ts crates/vox-gui/ui/src/guards/reviewStates.guard.test.ts
git commit -m "feat(gui-e2e): review state registry (surface x state matrix) + completeness guard" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 6: Page-audit helpers — icons + overflow (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/review/audits.ts`
- Test: `crates/vox-gui/ui/e2e/review/audits.test.ts`

- [ ] **Step 1: Failing unit test** (the helpers are pure page-function builders returning serializable functions; test their logic against a fake DOM)

```ts
// crates/vox-gui/ui/e2e/review/audits.test.ts
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { auditIconsInPage, auditOverflowInPage } from './audits';

describe('auditIconsInPage', () => {
  it('flags zero-size svgs, childless svgs, and broken imgs; passes healthy ones', () => {
    document.body.innerHTML = `
      <svg id="ok" width="16" height="16"><path d="M0 0h16v16z"/></svg>
      <svg id="empty" width="16" height="16"></svg>
      <img id="broken" src="x.png" alt="icon" />
    `;
    // jsdom: naturalWidth is 0 for all imgs; getBoundingClientRect is 0x0 —
    // emulate the "ok" svg being rendered by monkeypatching its rect.
    const ok = document.getElementById('ok')!;
    (ok as any).getBoundingClientRect = () => ({ width: 16, height: 16 });
    const issues = auditIconsInPage();
    const ids = issues.map((i) => i.id || i.testid || i.selectorHint);
    expect(issues.some((i) => i.kind === 'empty-svg')).toBe(true);
    expect(issues.some((i) => i.kind === 'broken-img')).toBe(true);
    expect(ids.join()).not.toContain('ok');
  });
});

describe('auditOverflowInPage', () => {
  it('reports body horizontal overflow', () => {
    Object.defineProperty(document.body, 'scrollWidth', { value: 1600, configurable: true });
    Object.defineProperty(document.body, 'clientWidth', { value: 1440, configurable: true });
    const r = auditOverflowInPage();
    expect(r.bodyHorizontalOverflowPx).toBe(160);
  });
});
```

- [ ] **Step 2: RED**, then **Step 3: Implement**

```ts
// crates/vox-gui/ui/e2e/review/audits.ts
/**
 * Programmatic per-capture audits. Each function runs IN THE PAGE via
 * page.evaluate(auditIconsInPage) — keep them fully self-contained
 * (no imports referenced inside the function bodies).
 */

export interface IconIssue {
  kind: 'zero-size-svg' | 'empty-svg' | 'broken-img';
  id: string;
  testid: string;
  selectorHint: string;
}

export function auditIconsInPage(): IconIssue[] {
  const issues: IconIssue[] = [];
  const hint = (el: Element) =>
    `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}.${(el.className && String(el.className).split(/\s+/)[0]) || ''}`;
  for (const svg of Array.from(document.querySelectorAll('svg'))) {
    const r = svg.getBoundingClientRect();
    const drawable = svg.querySelector('path, circle, rect, line, polyline, polygon, use, text');
    if (r.width === 0 || r.height === 0) {
      issues.push({ kind: 'zero-size-svg', id: svg.id, testid: svg.getAttribute('data-testid') ?? '', selectorHint: hint(svg) });
    } else if (!drawable) {
      issues.push({ kind: 'empty-svg', id: svg.id, testid: svg.getAttribute('data-testid') ?? '', selectorHint: hint(svg) });
    }
  }
  for (const img of Array.from(document.querySelectorAll('img'))) {
    if (img.complete && img.naturalWidth === 0) {
      issues.push({ kind: 'broken-img', id: img.id, testid: img.getAttribute('data-testid') ?? '', selectorHint: hint(img) });
    }
  }
  return issues;
}

export interface OverflowReport {
  bodyHorizontalOverflowPx: number;
  scrollHostHorizontalOverflowPx: number;
}

export function auditOverflowInPage(): OverflowReport {
  const body = document.body;
  const host = document.querySelector('[data-testid="surface-scroll-host"]');
  const hostOverflow = host ? Math.max(0, (host as HTMLElement).scrollWidth - (host as HTMLElement).clientWidth) : 0;
  return {
    bodyHorizontalOverflowPx: Math.max(0, body.scrollWidth - body.clientWidth),
    scrollHostHorizontalOverflowPx: hostOverflow,
  };
}
```

(Adjust the jsdom test to match exact naturalWidth semantics — jsdom images are `complete=false` until load; set `Object.defineProperty(img, 'complete', {value: true})` in the test if needed.)

- [ ] **Step 4: GREEN** — `pnpm exec vitest run e2e/review/audits.test.ts`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/review/audits.ts crates/vox-gui/ui/e2e/review/audits.test.ts
git commit -m "feat(gui-e2e): in-page icon + overflow audit helpers for the review harness" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 7: Capture spec, Firefox project, `review:capture` script (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/review/capture.spec.ts`
- Modify: `crates/vox-gui/ui/playwright.config.ts` (add firefox project, grep-scoped)
- Modify: `crates/vox-gui/ui/package.json` (devDependency + script)
- Modify: `crates/vox-gui/ui/.gitignore` (ignore `review-bundle/`)

- [ ] **Step 1: Install the axe dependency**

```
cd C:\Users\Owner\vox\crates\vox-gui\ui
pnpm add -D @axe-core/playwright
pnpm exec playwright install firefox
```

- [ ] **Step 2: Firefox project + script wiring**

`playwright.config.ts` projects become:

```ts
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
    {
      // Review-capture only: the user evaluates in Firefox, and Gecko layout
      // differs from Blink. The asserting sweep stays chromium-only.
      name: 'firefox-review',
      grep: /@review-capture/,
      use: { ...devices['Desktop Firefox'] },
    },
  ],
```

`package.json` scripts gain:

```json
    "review:capture": "cross-env VOX_REVIEW_CAPTURE=1 playwright test e2e/review/capture.spec.ts --project=chromium --project=firefox-review --workers=4"
```

If `cross-env` is not already a dependency, do NOT add it — use a Node-free env approach instead: keep the script as plain `playwright test ...` and have the spec self-gate on `process.env.VOX_REVIEW_CAPTURE`, documenting `$env:VOX_REVIEW_CAPTURE='1'; pnpm review:capture` for manual PowerShell runs; the `scripts/frontend-review.vox` wrapper (Task 11) sets the env itself. (Check `package.json` first; there is currently no cross-env.)

`.gitignore` (the ui one; create the entry alongside existing ignores): add `review-bundle/`.

- [ ] **Step 3: Write `capture.spec.ts`**

```ts
// crates/vox-gui/ui/e2e/review/capture.spec.ts
/**
 * Review-bundle capture matrix: every SURFACE_REGISTRY surface x its curated
 * states x 3 viewports, on chromium AND firefox (tag @review-capture routes
 * the firefox-review project). Env-gated: without VOX_REVIEW_CAPTURE=1 every
 * test self-skips so the default sweep/CI is unaffected.
 *
 * Output: review-bundle/latest/<id>.png + entries-<browser>.jsonl (one JSON
 * object per line — parallel-worker-safe append; the analyzer and the .vox
 * wrapper merge these). Capture is EVIDENCE, not a gate: a failed state
 * setup or audit records a degraded entry instead of failing the run.
 */
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { createHash } from 'node:crypto';
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { join } from 'node:path';
import { SURFACE_REGISTRY } from '../../src/generated/surfaceRegistry.generated';
import { VIEWPORTS, statesFor } from './states';
import { auditIconsInPage, auditOverflowInPage } from './audits';
import { addRichMockInitScript } from '../lib/tauriMockRich';

const RUN = process.env.VOX_REVIEW_CAPTURE === '1';
const OUT = join(import.meta.dirname, '..', '..', 'review-bundle', 'latest');
const SURFACES = SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string);

test.describe('review-bundle capture @review-capture', () => {
  test.skip(!RUN, 'set VOX_REVIEW_CAPTURE=1 to run the capture matrix');

  for (const surface of SURFACES) {
    for (const state of statesFor(surface)) {
      for (const vp of VIEWPORTS) {
        test(`${surface} -- ${state.name} -- ${vp.name}`, async ({ page, browserName }) => {
          mkdirSync(OUT, { recursive: true });
          const id = `${surface}--${state.name}--${vp.name}--${browserName}`;
          const consoleErrors: string[] = [];
          const pageErrors: string[] = [];
          page.on('console', (m) => {
            if (m.type() === 'error' || m.type() === 'warning') consoleErrors.push(`${m.type()}: ${m.text()}`);
          });
          page.on('pageerror', (e) => pageErrors.push(e.message));

          await page.setViewportSize({ width: vp.width, height: vp.height });
          await addRichMockInitScript(page, surface);
          await page.goto('/');
          await page.waitForSelector('nav', { timeout: 20_000 });

          let stateOk = true;
          let stateError = '';
          if (state.setup) {
            try {
              await state.setup(page);
              await page.waitForTimeout(350); // settle menus/animations
            } catch (e) {
              stateOk = false;
              stateError = String(e);
            }
          }

          const file = `${id}.png`;
          await page.screenshot({ path: join(OUT, file), fullPage: true });
          const sha256 = createHash('sha256').update(readFileSync(join(OUT, file))).digest('hex');

          let axeViolations: unknown[] = [];
          try {
            const axe = await new AxeBuilder({ page }).analyze();
            axeViolations = axe.violations.filter((v) =>
              ['moderate', 'serious', 'critical'].includes(v.impact ?? ''),
            );
          } catch (e) {
            consoleErrors.push(`axe-failed: ${String(e)}`);
          }
          const iconIssues = await page.evaluate(auditIconsInPage);
          const overflow = await page.evaluate(auditOverflowInPage);

          const entry = {
            id, surface, state: state.name, viewport: vp.name, browser: browserName,
            file, sha256,
            state_ok: stateOk, state_error: stateError,
            axe_violations: axeViolations,
            console_errors: consoleErrors.slice(0, 50),
            page_errors: pageErrors,
            icon_issues: iconIssues,
            overflow,
            captured_at: new Date().toISOString(),
          };
          appendFileSync(join(OUT, `entries-${browserName}.jsonl`), JSON.stringify(entry) + '\n');
          // The only assertion: the app shell mounted. Everything else is evidence.
          expect(pageErrors.filter((e) => /__TAURI_INTERNALS__/.test(e))).toEqual([]);
        });
      }
    }
  }
});
```

- [ ] **Step 4: Smoke-run a slice, then the full chromium matrix**

```
cd C:\Users\Owner\vox\crates\vox-gui\ui
$env:VOX_REVIEW_CAPTURE = '1'
pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium -g "dashboard" --workers=2
```
Expected: dashboard's states × 3 viewports pass; `review-bundle/latest/` contains PNGs + `entries-chromium.jsonl`. Then the full run:
```
pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium --project=firefox-review --workers=4
Remove-Item Env:VOX_REVIEW_CAPTURE
```
Expected: ~200–250 passed (some state setups may record `state_ok:false` — that is data, not failure); both `entries-*.jsonl` present. Spot-open two PNGs and confirm they render dense content, not blank frames. Also verify the default suite is unaffected: `pnpm exec playwright test --project=chromium --grep-invert "@review-capture" --list` still lists the usual specs, and a plain `pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium` WITHOUT the env var reports all skipped.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/review/capture.spec.ts crates/vox-gui/ui/playwright.config.ts crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml crates/vox-gui/ui/.gitignore
git commit -m "feat(gui-e2e): review-bundle capture matrix (surfaces x states x viewports, chromium+firefox, axe+icon+overflow audits)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 8: Bundle-entry types + loader (Rust, Phase C)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/visus_review/bundle.rs`
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (add `pub mod bundle;` — check how sibling modules are declared; `visus_review` is itself a directory module, so declare inside its `mod.rs`)

- [ ] **Step 1: Failing tests (in `bundle.rs`'s own `#[cfg(test)]`)**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_capture_entry_line() {
        let line = r#"{"id":"chat--default--wide--chromium","surface":"chat","state":"default","viewport":"wide","browser":"chromium","file":"chat--default--wide--chromium.png","sha256":"ab","state_ok":true,"state_error":"","axe_violations":[{"id":"color-contrast"}],"console_errors":["error: x"],"page_errors":[],"icon_issues":[],"overflow":{"bodyHorizontalOverflowPx":0,"scrollHostHorizontalOverflowPx":12},"captured_at":"t"}"#;
        let e: BundleEntry = serde_json::from_str(line).unwrap();
        assert_eq!(e.id, "chat--default--wide--chromium");
        assert_eq!(e.axe_violations.len(), 1);
        assert_eq!(e.overflow["scrollHostHorizontalOverflowPx"], 12);
    }
    #[test]
    fn tolerates_missing_optional_fields() {
        let line = r#"{"id":"x","surface":"x","state":"default","viewport":"wide","browser":"firefox","file":"x.png","sha256":"cd"}"#;
        let e: BundleEntry = serde_json::from_str(line).unwrap();
        assert!(e.state_ok); // defaults true
        assert!(e.console_errors.is_empty());
    }
    #[test]
    fn load_bundle_reads_all_jsonl_files_and_skips_bad_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("entries-chromium.jsonl"),
            "{\"id\":\"a\",\"surface\":\"s\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"chromium\",\"file\":\"a.png\",\"sha256\":\"1\"}\nnot-json\n").unwrap();
        std::fs::write(dir.path().join("entries-firefox.jsonl"),
            "{\"id\":\"b\",\"surface\":\"s\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"firefox\",\"file\":\"b.png\",\"sha256\":\"2\"}\n").unwrap();
        let (entries, skipped) = load_bundle(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(skipped, 1);
    }
}
```

- [ ] **Step 2: RED** — `cargo test -p vox-orchestrator-mcp --features gui-visual-review bundle > "$env:TEMP\bundle_red.log" 2>&1`; read the log: compile error (module missing).

- [ ] **Step 3: Implement**

```rust
// crates/vox-orchestrator-mcp/src/visus_review/bundle.rs
//! Review-bundle loader: reads the capture harness's entries-*.jsonl
//! (crates/vox-gui/ui/review-bundle/latest) — one JSON object per line,
//! parallel-writer-safe on the TS side, no merge step needed here.

use std::path::Path;

fn default_true() -> bool { true }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BundleEntry {
    pub id: String,
    pub surface: String,
    pub state: String,
    pub viewport: String,
    pub browser: String,
    pub file: String,
    pub sha256: String,
    #[serde(default = "default_true")]
    pub state_ok: bool,
    #[serde(default)]
    pub state_error: String,
    #[serde(default)]
    pub axe_violations: Vec<serde_json::Value>,
    #[serde(default)]
    pub console_errors: Vec<String>,
    #[serde(default)]
    pub page_errors: Vec<String>,
    #[serde(default)]
    pub icon_issues: Vec<serde_json::Value>,
    #[serde(default)]
    pub overflow: serde_json::Value,
    #[serde(default)]
    pub captured_at: String,
}

/// Load every `entries-*.jsonl` in `dir`. Returns (entries, skipped_lines).
pub fn load_bundle(dir: &Path) -> std::io::Result<(Vec<BundleEntry>, usize)> {
    let mut entries = Vec::new();
    let mut skipped = 0usize;
    for f in std::fs::read_dir(dir)? {
        let f = f?;
        let name = f.file_name().to_string_lossy().to_string();
        if !(name.starts_with("entries-") && name.ends_with(".jsonl")) {
            continue;
        }
        for line in std::fs::read_to_string(f.path())?.lines() {
            let line = line.trim();
            if line.is_empty() { continue; }
            match serde_json::from_str::<BundleEntry>(line) {
                Ok(e) => entries.push(e),
                Err(_) => skipped += 1,
            }
        }
    }
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((entries, skipped))
}
```

Declare in `visus_review/mod.rs`: `pub mod bundle;` (top of file near other decls). Add `tempfile` to `[dev-dependencies]` of `crates/vox-orchestrator-mcp/Cargo.toml` only if not already there (check first — it likely is, mod.rs tests use temp dirs).

- [ ] **Step 4: GREEN** — same cargo test filter; expect 3 passed. Then `cargo clippy -p vox-orchestrator-mcp --features gui-visual-review -- -D warnings > "$env:TEMP\bundle_clippy.log" 2>&1` (read: clean) and `cargo fmt -p vox-orchestrator-mcp`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/visus_review/bundle.rs crates/vox-orchestrator-mcp/src/visus_review/mod.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat(visual-review): review-bundle JSONL entry types + loader" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 9: Defect rubric + prompts + PROMPT_VERSION bump (Phase C)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/prompt.rs`

- [ ] **Step 1: Failing tests** (extend the existing `prompt::tests` module)

```rust
    #[test]
    fn defect_rubric_names_the_hunted_classes() {
        for needle in ["occlusion", "clipp", "icon", "error text", "blank", "z-"] {
            assert!(DEFECT_RUBRIC.to_lowercase().contains(needle), "missing: {needle}");
        }
    }
    #[test]
    fn defect_prompts_carry_context_and_json_contract() {
        let e = crate::visus_review::bundle::BundleEntry {
            id: "chat--model-picker-open--compact--firefox".into(),
            surface: "chat".into(), state: "model-picker-open".into(),
            viewport: "compact".into(), browser: "firefox".into(),
            file: "f.png".into(), sha256: "s".into(),
            state_ok: true, state_error: String::new(),
            axe_violations: vec![serde_json::json!({"id":"color-contrast"})],
            console_errors: vec!["error: boom".into()],
            page_errors: vec![], icon_issues: vec![],
            overflow: serde_json::json!({"bodyHorizontalOverflowPx": 40}),
            captured_at: "t".into(),
        };
        let up = defect_user_prompt(&e);
        assert!(up.contains("chat") && up.contains("model-picker-open") && up.contains("compact") && up.contains("firefox"));
        assert!(up.contains("color-contrast") && up.contains("boom") && up.contains("40"));
        let sp = defect_system_prompt();
        assert!(sp.contains("ONLY a single JSON object"));
        assert!(sp.contains("\"defects\""));
    }
    #[test]
    fn prompt_version_bumped_for_defect_rubric() {
        assert!(PROMPT_VERSION >= "2026-07-18.1", "bump PROMPT_VERSION when adding the defect rubric");
    }
```

- [ ] **Step 2: RED** — `cargo test -p vox-orchestrator-mcp --features gui-visual-review prompt > "$env:TEMP\prompt_red.log" 2>&1`.

- [ ] **Step 3: Implement** — in `prompt.rs`: bump `PROMPT_VERSION` to `"2026-07-18.1"`, keep `RUBRIC`/`system_prompt`/`user_prompt` (legacy mode unchanged), append:

```rust
/// Defect-hunting rubric for review-bundle analysis. Unlike RUBRIC (general
/// design quality), this targets concrete rendering DEFECTS the capture
/// matrix exists to catch.
pub const DEFECT_RUBRIC: &str = r#"
Hunt for concrete rendering DEFECTS in this screenshot. Report only what is visibly wrong:
A occlusion: elements overlapping/covering each other (menus over content they shouldn't cover, HUD over controls, z- order fights, tooltips/popovers clipped by containers).
B clipping/truncation: text or controls cut off mid-glyph, ellipsis where full text matters, content escaping its card/panel, horizontal scrollbars on the page body.
C icons: blank or missing icon slots, zero-size glyphs, misaligned or mismatched icon sizes.
D error leakage: raw exception text, stack traces, 'undefined'/'NaN'/'[object Object]' visible in UI copy.
E blank regions: panels that render empty where the dense mock data should appear.
F layout breakage: overlapping columns, collapsed rows, controls pushed off-screen — especially at the compact viewport.
G contrast/legibility: text below readable contrast against its actual background.
"#;

pub fn defect_system_prompt() -> String {
    format!(
        "You are a rendering-defect detector for a desktop GUI screenshot. Programmatic scan \
results (axe-core, console errors, icon audit, overflow measurements) are provided — correlate \
with them, then find what they CANNOT see (visual occlusion, clipping, blank panels, error-text \
leakage).\n\nDEFECT RUBRIC:\n{DEFECT_RUBRIC}\n\nOUTPUT CONTRACT: Respond with ONLY a single JSON \
object, no prose, no markdown fence:\n{{\n  \"score\": <integer 0-100, 100 = defect-free>,\n  \
\"verdict\": \"pass\" | \"pass_with_notes\" | \"fail\",\n  \"defects\": [ {{ \"severity\": \
\"critical\"|\"major\"|\"minor\", \"kind\": \"occlusion\"|\"clipping\"|\"icon\"|\"error-leak\"|\
\"blank\"|\"layout\"|\"contrast\"|\"other\", \"description\": \"<what is wrong>\", \"location\": \
\"<where on screen>\" }} ]\n}}\nIf clean, return an empty defects array, verdict \"pass\", score >= 95."
    )
}

pub fn defect_user_prompt(e: &crate::visus_review::bundle::BundleEntry) -> String {
    format!(
        "Capture: surface '{surface}', state '{state}', viewport '{viewport}', browser '{browser}'.\n\
Programmatic findings for THIS capture (correlate, do not merely repeat):\n\
- axe violations: {axe}\n- console errors: {console:?}\n- page errors: {page:?}\n\
- icon issues: {icons}\n- overflow: {overflow}\n- state setup ok: {ok} {err}\n\
Analyze the attached screenshot per the defect rubric and output the JSON verdict.",
        surface = e.surface, state = e.state, viewport = e.viewport, browser = e.browser,
        axe = serde_json::to_string(&e.axe_violations).unwrap_or_default(),
        console = e.console_errors, page = e.page_errors,
        icons = serde_json::to_string(&e.icon_issues).unwrap_or_default(),
        overflow = e.overflow, ok = e.state_ok,
        err = if e.state_error.is_empty() { String::new() } else { format!("(setup error: {})", e.state_error) },
    )
}
```

- [ ] **Step 4: GREEN** — prompt tests pass (note the legacy `PROMPT_VERSION` bump forces exactly one full legacy-cache re-review post-merge — that is the designed behavior; say so in the commit body). `cargo fmt -p vox-orchestrator-mcp`; clippy clean.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/visus_review/prompt.rs
git commit -m "feat(visual-review): defect-hunting rubric + bundle prompts; PROMPT_VERSION 2026-07-18.1" -m "Version bump deliberately invalidates the legacy cache once (occlusion/clipping/icon/error-leak classes were previously unreviewed)." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 10: Bundle analysis run + report + CLI `--bundle` mode (Phase C)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (add `run_bundle`)
- Modify: `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs` (`--bundle <dir>` arg)

- [ ] **Step 0: Re-read `run()`** (mod.rs lines ~216–300) to identify the exact per-entry review invocation — the function that takes PNG bytes + system/user prompts and returns the parsed model JSON (plus the model-selection and budget logic around it). `run_bundle` MUST reuse those same helpers, not duplicate the OpenRouter client. Record the helper names in the commit body.

- [ ] **Step 1: Failing tests** — add to mod.rs's test modules:

```rust
    #[test]
    fn bundle_cache_key_is_the_capture_id_and_respects_sha_model_prompt() {
        let mut c = CacheIndex::default();
        c.entries.insert("chat--default--wide--chromium".into(), CacheEntry {
            screenshot_sha256: "aa".into(), score: 90, verdict: "pass".into(),
            model: "m".into(), reviewed_at: "t".into(), prompt_version: crate::visus_review::prompt::PROMPT_VERSION.into(),
        });
        assert_eq!(decide_status(&c, "chat--default--wide--chromium", "aa", "m", crate::visus_review::prompt::PROMPT_VERSION), ReviewDecision::Cached);
        assert_eq!(decide_status(&c, "chat--default--wide--chromium", "bb", "m", crate::visus_review::prompt::PROMPT_VERSION), ReviewDecision::Changed);
        assert_eq!(decide_status(&c, "chat--default--laptop--chromium", "aa", "m", crate::visus_review::prompt::PROMPT_VERSION), ReviewDecision::New);
    }
    #[test]
    fn defect_report_parses_model_output() {
        let raw = r#"{"score": 40, "verdict": "fail", "defects": [{"severity":"critical","kind":"occlusion","description":"HUD covers the composer","location":"bottom center"}]}"#;
        let d: DefectReport = serde_json::from_str(raw).unwrap();
        assert_eq!(d.defects.len(), 1);
        assert_eq!(d.defects[0].kind, "occlusion");
    }
```

- [ ] **Step 2: RED**, then **Step 3: Implement** in mod.rs:

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Defect {
    pub severity: String,
    pub kind: String,
    pub description: String,
    #[serde(default)]
    pub location: String,
}
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct DefectReport {
    #[serde(default)]
    pub score: u32,
    #[serde(default)]
    pub verdict: String,
    #[serde(default)]
    pub defects: Vec<Defect>,
}

pub struct BundleRunArgs<'a> {
    pub bundle_dir: &'a Path,
    pub cache_path: &'a Path,
    pub report_dir: &'a Path,
    pub now_iso: String,
    pub do_ai: bool,
}

pub async fn run_bundle(args: &BundleRunArgs<'_>) -> RunReport {
    // 1. let (entries, skipped) = bundle::load_bundle(args.bundle_dir) — warn on skipped.
    // 2. Load cache exactly as run() does (schema check, discard on mismatch).
    // 3. Per entry: decide_status(&cache, &entry.id, &entry.sha256, &model, prompt::PROMPT_VERSION);
    //    on New/Changed and args.do_ai: read PNG from bundle_dir.join(&entry.file), call the SAME
    //    per-image review helper run() uses but with prompt::defect_system_prompt() /
    //    prompt::defect_user_prompt(&entry); parse DefectReport; insert CacheEntry
    //    { screenshot_sha256, score, verdict, model, reviewed_at: args.now_iso, prompt_version }.
    // 4. Prune: cache.entries.retain(|k, _| live_ids.contains(k)) where live_ids = entries' ids
    //    (guard: skip prune when entries.is_empty(), mirroring prune_dead_views).
    // 5. Write reports under args.report_dir:
    //    - bundle-report.v1.json: { schema_version: 1, generated_at, entries: [ { id, surface, state,
    //      viewport, browser, score, verdict, defects, programmatic: { axe: n, console: n, icons: n,
    //      overflow_px } } ] }
    //    - bundle-digest.md: markdown grouped by surface, ordered severity critical>major>minor,
    //      each line: `- [<severity>/<kind>] <surface> (<state>/<viewport>/<browser>): <description> — <location>`
    //      plus a summary table (surface, captures, defect counts by severity).
    // 6. Persist cache (schema_version stamped), return RunReport with counts (mirror run()'s fields).
    todo!("flesh out following run()'s structure — see Step 0 helper names")
}
```

The `todo!` above is a planning sketch — the implementing engineer replaces it with the real body in this task (Step 0 identified the helpers; the structure is fully specified in the comments). No `todo!` may be committed.

Bin (`gui-visual-review.rs`): add `--bundle <dir>`; when present, call `run_bundle` with `cache` defaulting to `contracts/reports/gui-visual-review/bundle-cache.v1.json` (separate file from the legacy cache — different key space) and `report-dir` default unchanged; otherwise legacy path as today.

- [ ] **Step 4: GREEN + end-to-end dry run**

```
cargo test -p vox-orchestrator-mcp --features gui-visual-review visus_review > "$env:TEMP\visus2.log" 2>&1
```
Read: all pass (legacy + new). Clippy clean; `cargo fmt -p vox-orchestrator-mcp`. Then a no-AI smoke against the real bundle from Task 7:
```
cargo run --features gui-visual-review --bin gui-visual-review -- --bundle crates/vox-gui/ui/review-bundle/latest > "$env:TEMP\bundle_dry.log" 2>&1
```
Expected: report files written; every entry listed with its programmatic findings; zero AI calls (no `--ai`).

- [ ] **Step 5: AI smoke on a slice** — temporarily copy 5 entries' JSONL lines + PNGs into a scratch dir, run with `--ai`, confirm `bundle-report.v1.json` carries model verdicts and the cache file grows; rerun and confirm 5/5 Cached (zero cost). Delete the scratch dir.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/visus_review/mod.rs crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs
git commit -m "feat(visual-review): bundle analysis mode - defect reports, digest, dedicated cache, --bundle CLI" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 11: `scripts/frontend-review.vox` one-command wrapper (Phase C)

**Files:**
- Create: `scripts/frontend-review.vox`

- [ ] **Step 1: Write the script** (VoxScript; subprocess+env caps like `scripts/crate-build-audit.vox` — copy its cap header idiom):

```vox
// vox:caps subprocess env
// One-command frontend review pipeline:
//   vox run scripts/frontend-review.vox            -> capture (both browsers) + programmatic analysis
//   VOX_REVIEW_AI=1 vox run scripts/frontend-review.vox -> also AI defect analysis
fn main() {
    env.set("VOX_REVIEW_CAPTURE", "1")
    print("[frontend-review] capturing matrix (chromium + firefox)...")
    let cap = process.run("pnpm", ["--dir", "crates/vox-gui/ui", "exec", "playwright", "test", "e2e/review/capture.spec.ts", "--project=chromium", "--project=firefox-review", "--workers=4"])
    if cap.status isnt 0 {
        print("[frontend-review] capture reported failures (continuing - capture is evidence, entries were still written)")
    }
    let mut args = ["run", "--features", "gui-visual-review", "--bin", "gui-visual-review", "--", "--bundle", "crates/vox-gui/ui/review-bundle/latest"]
    let ai_opt = env.get("VOX_REVIEW_AI")
    match ai_opt {
        Some(v) => { if v is "1" { args = args.push("--ai") } }
        None => {}
    }
    print("[frontend-review] analyzing bundle...")
    let an = process.run("cargo", args)
    if an.status isnt 0 {
        print("[frontend-review] analysis FAILED")
        process.exit(1)
    }
    print("[frontend-review] done - digest: contracts/reports/gui-visual-review/bundle-digest.md")
}
```

(Verify the exact `process.run`/`.status`/`.push` API shapes against `scripts/crate-build-audit.vox` before writing — match its idioms; the interpreter is ground truth: `vox run --mode interp scripts/frontend-review.vox`.)

- [ ] **Step 2: Verify** — `vox run --mode interp scripts/frontend-review.vox` (no AI) runs both stages end-to-end.

- [ ] **Step 3: Commit**

```bash
git add scripts/frontend-review.vox
git commit -m "feat(scripts): one-command frontend review pipeline (capture + bundle analysis)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 12: CI — switch the post-merge advisory analysis to the bundle (Phase C)

**Files:**
- Modify: `.github/workflows/ci.yml` (`gui-playwright-smoke` job's advisory steps ONLY)

- [ ] **Step 1: Locate the advisory block** — the steps named `GUI visual-review capture (manifest)`, the AI review step, and `Commit visual-review cache + report` (all `continue-on-error: true`). Do not touch the asserting sweep, `needs:`, `if:`, or `ci-summary`.

- [ ] **Step 2: Replace capture+review inputs with bundle equivalents**

- New step after the asserting sweep (before the legacy capture step, which is REMOVED along with its manifest-mode AI step):

```yaml
      # Review-bundle capture (chromium-only in CI to bound cost; firefox is
      # local/on-demand via scripts/frontend-review.vox). Advisory per F2.
      - name: Review-bundle capture (chromium)
        working-directory: crates/vox-gui/ui
        env:
          VOX_REVIEW_CAPTURE: "1"
        run: pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium --workers=2
        continue-on-error: true
      - name: Review-bundle AI defect analysis (advisory)
        run: cargo run --features gui-visual-review --bin gui-visual-review -- --bundle crates/vox-gui/ui/review-bundle/latest --ai
        continue-on-error: true
```

- The cache-commit step's paths gain `contracts/reports/gui-visual-review/bundle-cache.v1.json`, `bundle-report.v1.json`, `bundle-digest.md`; the artifact-upload step's `path:` list gains `crates/vox-gui/ui/review-bundle/latest/`.

- [ ] **Step 3: Guard-rails** — `git diff .github/workflows/ci.yml`: only the `gui-playwright-smoke` job changed; `git diff | grep "^[+-].*needs:"` → empty; `continue-on-error: true` added only on the two new advisory steps (net count: legacy removed steps' flags disappear, two appear); YAML parses (`python -c "import yaml,io; yaml.safe_load(io.open('.github/workflows/ci.yml', encoding='utf-8'))"`).

- [ ] **Step 4: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci(gui): post-merge advisory analysis switches to the review bundle (chromium matrix + defect rubric)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 13: Phase D — run the pipeline and write the comprehensive review

**Files:**
- Create: `docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`

- [ ] **Step 1: Full local run** — `$env:VOX_REVIEW_AI='1'; vox run scripts/frontend-review.vox` (both browsers + AI). Confirm `bundle-digest.md` exists and `review-bundle/latest/` holds the full matrix.
- [ ] **Step 2: Triage automated findings** — read `bundle-report.v1.json`; dedupe defects that repeat across viewports/browsers (same surface+kind+description → one finding, note the affected matrix cells); cross out model hallucinations by opening the referenced PNG for every `critical`/`major` defect and confirming visually.
- [ ] **Step 3: Manual LLM pass** — read EVERY surface's screenshots (at minimum: every `compact` viewport capture + every non-default state capture, both browsers — these are where occlusion lives), tab by tab; record findings the model missed with the same fields (severity, kind, description, location, evidence path).
- [ ] **Step 4: Tauri-shell spot check** — build/launch the desktop shell (sidecar prerequisite per AGENTS.md 'vox-gui sidecar' pattern: `cargo build --release -p vox-cli` then copy to `target/release/vox-<host-triple>.exe`, `pnpm --dir crates/vox-gui/ui build`, `cargo run -p vox-gui`) and screenshot ~6 surfaces (chat, dashboard, tasks, settings, models, approvals) with the OS screenshot tooling; diff against the chromium captures for engine-specific issues.
- [ ] **Step 5: Coverage audit table** — per registry surface, columns: unit tests (grep `src/components/surfaces/<Surface>` test files) / e2e spec (ls `e2e/*.spec.ts`) / capture states (states.ts) / AI-analyzed (bundle) / CI-monitored (which job) — source from `contracts/reports/test-inventory.v1.json`, the e2e dir, and `ci.yml`. Explicitly list surfaces with NO coverage in any column.
- [ ] **Step 6: Write the review doc** — sections: executive summary; methodology; ranked findings register (id, severity, kind, surface, states/viewports affected, evidence path, remediation sketch); per-surface tab-by-tab detail; coverage audit table; recommended remediation order. Frontmatter not required (docs/superpowers/ is exempt from docs/src frontmatter rules).
- [ ] **Step 7: Commit**

```bash
git add docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md
git commit -m "docs(reviews): comprehensive Axis frontend review - ranked findings + coverage audit" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 14: Whole-effort verification sweep

- [ ] Frontend: `pnpm typecheck` clean; `pnpm test` green (includes backendGuard, transportIpcGuard, BackendBanner, reviewStates guard, audits, tauriMockRich tests); `pnpm exec playwright test --project=chromium` green (default suite unaffected; capture spec self-skips).
- [ ] Negative guard proof: temporarily add `invoke<string>('x')` outside the marker block in transport.ts → `transportIpcGuard` fails naming it; revert; green again.
- [ ] Rust: `cargo test -p vox-orchestrator-mcp --features gui-visual-review > "$env:TEMP\p14.log" 2>&1` all green; clippy `-D warnings` clean.
- [ ] Live browser-mode proof (the original bug): dev server + plain browser → banner visible, zero raw `__TAURI_INTERNALS__` TypeErrors in console.
- [ ] `vox run --mode interp scripts/frontend-review.vox` end-to-end (no AI) succeeds.
- [ ] Contracts: regenerate `test-inventory` (`./target/release/vox ci test-inventory --output contracts/reports/test-inventory.v1.json` with a fresh-built binary) and `gui-surface-coverage --write` if drift; commit.
- [ ] `git log --oneline` shows one commit per task; push to main per session policy (no PR), pre-push hooks tolerated with long timeout.

---

## Out of scope (explicitly deferred)

- Remediation of Phase D findings (separate plan, user re-prioritizes from the full register).
- Visual-diff baselines (`toHaveScreenshot`) — post-remediation add-on.
- Programmatic occlusion detection; tauri-driver automation; PR gating; touching `ci-summary.needs`.
