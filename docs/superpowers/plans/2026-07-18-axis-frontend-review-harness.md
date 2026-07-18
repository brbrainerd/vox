# Axis Frontend Review Harness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

> **Hardening provenance:** adversarially audited 2026-07-18 by 8 parallel code-verifying reviewers (61 findings, 6 critical — all applied). Every file:line claim below was re-verified against the working tree at commit `95a8441eaf`+.

**Goal:** Execute `docs/superpowers/specs/2026-07-18-axis-frontend-review-harness-design.md`: kill the raw `__TAURI_INTERNALS__` TypeError class user-visibly, build the full-matrix review-bundle capture harness, extend `visus_review` with a defect-focused, frontier-resumable bundle analysis, wire bounded advisory CI, then run the pipeline (with a known-issue recall gate) to produce the comprehensive review.

**Architecture:** `src/lib/backendGuard.ts` is the single source of truth for backend detection + the typed error + the rejection filter (which also swallows raw `__TAURI_INTERNALS__` TypeErrors from the 33 direct-invoke / 7 direct-listen files when no backend exists). `transport.ts` routes all its IPC through marked `safeInvoke`/`safeListen`. The harness (`e2e/review/`) appends per-worker JSONL entries; the Rust analyzer reads `entries-*.jsonl` in a new bundle mode sharing `run()`'s extracted core (fence-tolerant JSON parsing, per-image vision call, model selection) with its own budget/frontier semantics and browser-scoped cache pruning.

**Tech Stack:** TypeScript/React/vitest/Playwright (`@axe-core/playwright` new devDependency; `@msgpack/msgpack` already present), Rust (serde/tokio, existing `call_vision_model` OpenRouter client), VoxScript, GitHub Actions YAML.

**Ground rules (Windows / repo policy):**
- Frontend commands run from `C:\Users\Owner\vox\crates\vox-gui\ui` via **pnpm** (never npm).
- Rust: **never** `cargo fmt --all` — `cargo fmt -p vox-orchestrator-mcp` only. Never pipe cargo output to `head`/`grep` — redirect to a file and read it.
- New automation glue is VoxScript (`scripts/*.vox`); `package.json` scripts are fine.
- CI edits touch only the `gui-playwright-smoke` job's advisory steps; **never** touch `ci-summary.needs` or add PR triggers (fork F2).
- Commits end with `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>`.

**Verified ground truth (2026-07-18, reviewer-confirmed):**
- `transport.ts`: **60** `invoke(`/`invoke<` call sites + **11** `listen<` call sites in 11 module-scope wrappers (`listenOrchStatus`:30, `listenAgentEvents`:56, `listenScientiaQueue`:81, `listenDiscoverySurfaced`:105, `listenBrowserFrames`:150, `listenPreviewAvailable`:156, `listenSecretaryProposed`:174, `listenPtyOutput`:660, `listenPtyExit`:666, `listenActivityAppended`:724, `listenFeedbackChanged`:782) = **71 raw sites**. The find/replace corrupts nothing (`invokeMcpTool` identifiers and `'invoke_mcp_tool'` string literals don't match `invoke(`/`invoke<`; `UnlistenFn` never precedes `(`/`<`).
- Beyond transport.ts: **33 production files import `invoke` directly** (ipcBoundaries.test.ts:43-78 allowlist; App.tsx:2 + uses at :361,:365,:397; Sidebar.tsx:2,:110) and **7 non-test files import `listen`** from `@tauri-apps/api/event` (App.tsx, ChatSurface.tsx, CodeRabbitView.tsx, SettingsView.tsx, subAgentClient.ts, TasksView.tsx, useAttentionInbox.ts) — no guard tracks the listen imports.
- `vitest.config.ts:5-12`: **no `environment` set → node default**; `setupFiles: ['src/test-setup.ts']` (currently jest-dom + cleanup only). `transport.test.ts` has no jsdom pragma; 40 files under `src/`+`e2e/lib` mock `@tauri-apps/api/core`. vitest `include` covers `src/**` and `e2e/lib/**` test files but NOT `e2e/review/**`.
- `AppShell.tsx:93`: shell root is `flex h-screen`, no fixed header; layers: Toasts z-40, Omnibar/Dialog z-50, achievement toasts z-[60]. `main.tsx:25` has an inline no-Tauri check to consolidate.
- `SURFACE_REGISTRY`: **31** entries with non-null viewKey.
- `visus_review/mod.rs`: `parse_verdict` (:11-15) strips markdown fences; `review_surface` does fs::read + `Instant` timing + `call_vision_model(...)` (vision_call module, returns `(String, Usage)`); config/model selection block at :237-256; `run()` is **sequential** — `max_concurrent_reviews`/`per_surface_review_budget_ms` are dead config and `total_review_budget_ms: 90_000` stops reviewing after ~11 entries (the audit's critical economics finding); cache persisted only when `do_ai` (:367). `tempfile` is already in `[dependencies]` (Cargo.toml:132).
- `ci.yml` `gui-playwright-smoke` step names (exact): `GUI visual AI review (advisory, non-gating)` and `Commit visual-review cache + report (main only)`; the job has Node+pnpm and a Rust toolchain (the legacy AI step is already a `cargo run`); `contracts/reports/gui-visual-review/*.json` is gitignore-negated selectively — `bundle-cache.v1.json` needs its own negation entry.
- VoxScript idioms (`gui-build.vox`, `ci-runners-up.vox`): `process.run` returns an **Option** (null on spawn failure — retry `"pnpm.cmd"` on Windows); `.unwrap()` yields `{code, stdout, stderr}`; env is set process-wide via `std.env.set` (children inherit; `run_capture_ex`'s env-list arg is ignored by the interpreter, eval/builtins.rs:1743-1744).
- Themes: `document.documentElement.dataset.theme` drives theming; audited themes for capture: `high-contrast` minimum.
- `screenshots-variants.spec.ts:26-29` lists the 10 KEY_SURFACES; its CI step is `GUI variant states sweep (empty/error, advisory)` (ci.yml:1679-1684).

---

## Task 1: `backendGuard.ts` — env-agnostic detection, typed error, extended rejection filter (Phase A)

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
  delete (globalThis as any).__TAURI_INTERNALS__;
  delete (globalThis as any).window;
  __resetBackendAvailabilityForTests();
});

describe('backendAvailable', () => {
  it('is false with no __TAURI_INTERNALS__ anywhere (node env)', () => {
    expect(backendAvailable()).toBe(false);
  });
  it('is true when globalThis has __TAURI_INTERNALS__ (node-env test stub)', () => {
    (globalThis as any).__TAURI_INTERNALS__ = {};
    expect(backendAvailable()).toBe(true);
  });
  it('is true when a window with __TAURI_INTERNALS__ exists (jsdom/Tauri)', () => {
    (globalThis as any).window = { __TAURI_INTERNALS__: {} };
    expect(backendAvailable()).toBe(true);
  });
  it('memoizes per app load', () => {
    expect(backendAvailable()).toBe(false);
    (globalThis as any).__TAURI_INTERNALS__ = {};
    expect(backendAvailable()).toBe(false);
  });
});

describe('BackendUnavailableError', () => {
  it('carries the command and an honest message', () => {
    const e = new BackendUnavailableError('chat_list_sessions');
    expect(e.command).toBe('chat_list_sessions');
    expect(e.message).toContain('desktop backend');
    expect(e.message).toContain('chat_list_sessions');
    expect(e).toBeInstanceOf(Error);
  });
});

describe('makeBackendUnavailableRejectionFilter', () => {
  it('preventDefaults BackendUnavailableError rejections', () => {
    const filter = makeBackendUnavailableRejectionFilter();
    const ev = { reason: new BackendUnavailableError('x'), preventDefault: vi.fn() };
    filter(ev as unknown as PromiseRejectionEvent);
    expect(ev.preventDefault).toHaveBeenCalledOnce();
  });
  it('preventDefaults raw __TAURI_INTERNALS__ TypeErrors ONLY when backend unavailable', () => {
    // 33 files import invoke directly and 7 import listen — their raw
    // TypeErrors must not surface uncaught in browser mode.
    const filter = makeBackendUnavailableRejectionFilter();
    const raw = {
      reason: new TypeError("can't access property \"invoke\", window.__TAURI_INTERNALS__ is undefined"),
      preventDefault: vi.fn(),
    };
    filter(raw as unknown as PromiseRejectionEvent);
    expect(raw.preventDefault).toHaveBeenCalledOnce();
    // With a backend present, the same TypeError is a REAL bug — pass through.
    (globalThis as any).__TAURI_INTERNALS__ = {};
    __resetBackendAvailabilityForTests();
    const filter2 = makeBackendUnavailableRejectionFilter();
    const raw2 = { reason: new TypeError('x __TAURI_INTERNALS__ y'), preventDefault: vi.fn() };
    filter2(raw2 as unknown as PromiseRejectionEvent);
    expect(raw2.preventDefault).not.toHaveBeenCalled();
  });
  it('passes unrelated rejections through', () => {
    const filter = makeBackendUnavailableRejectionFilter();
    const ev = { reason: new TypeError('boom'), preventDefault: vi.fn() };
    filter(ev as unknown as PromiseRejectionEvent);
    expect(ev.preventDefault).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: RED** — `pnpm exec vitest run src/lib/backendGuard.test.ts` → module not found.

- [ ] **Step 3: Implement**

```ts
// crates/vox-gui/ui/src/lib/backendGuard.ts
/**
 * Single source of truth for "is the Tauri desktop backend present?".
 *
 * In a plain browser there is no `window.__TAURI_INTERNALS__` and every raw
 * `invoke`/`listen` from @tauri-apps/api throws
 * `TypeError: can't access property "invoke", window.__TAURI_INTERNALS__ is undefined`.
 * transport.ts routes its IPC through safeInvoke/safeListen (typed rejection);
 * the 33 files importing `invoke` directly and 7 importing `listen`
 * (ipcBoundaries allowlist debt) are covered user-visibly by the rejection
 * filter's raw-TypeError branch below.
 *
 * Detection is env-agnostic (window OR globalThis) so node-env vitest suites
 * can stub `globalThis.__TAURI_INTERNALS__` without fabricating a window.
 */

let cached: boolean | null = null;

export function backendAvailable(): boolean {
  if (cached === null) {
    const host = (typeof window !== 'undefined' ? window : globalThis) as unknown as Record<string, unknown>;
    cached = '__TAURI_INTERNALS__' in host;
  }
  return cached;
}

/** Test-only: memoization would leak across vitest cases. */
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

const logged = new Set<string>();

/**
 * 'unhandledrejection' filter: in browser (no-backend) mode, swallow
 * (a) BackendUnavailableError and (b) raw __TAURI_INTERNALS__ TypeErrors from
 * direct-import call sites, logging once per distinct command/message.
 * With a backend present, (b) passes through — it would be a real bug.
 */
export function makeBackendUnavailableRejectionFilter(): (ev: PromiseRejectionEvent) => void {
  return (ev) => {
    const r = ev.reason;
    const isTyped = r instanceof BackendUnavailableError;
    const isRawNoBackend =
      !backendAvailable() && r instanceof TypeError && /__TAURI_INTERNALS__/.test(r.message);
    if (isTyped || isRawNoBackend) {
      ev.preventDefault();
      const key = isTyped ? (r as BackendUnavailableError).command : r.message;
      if (!logged.has(key)) {
        logged.add(key);
        console.debug('[backendGuard] suppressed (browser mode):', key);
      }
    }
  };
}

export function installBackendUnavailableRejectionFilter(): void {
  if (typeof window === 'undefined') return;
  window.addEventListener('unhandledrejection', makeBackendUnavailableRejectionFilter());
}
```

- [ ] **Step 4: GREEN** — all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/backendGuard.ts crates/vox-gui/ui/src/lib/backendGuard.test.ts
git commit -m "feat(gui): backendGuard - env-agnostic detection, BackendUnavailableError, extended rejection filter" -m "Filter also swallows raw __TAURI_INTERNALS__ TypeErrors in no-backend mode: 33 direct-invoke + 7 direct-listen files exist outside the transport choke point." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 2: Route ALL transport IPC through `safeInvoke`/`safeListen` (Phase A)

**Files:**
- Modify: `crates/vox-gui/ui/src/test-setup.ts` (**FIRST** — mandatory, not contingency)
- Modify: `crates/vox-gui/ui/src/transport.ts` (71 raw sites)
- Create: `crates/vox-gui/ui/src/guards/transportIpcGuard.test.ts`

- [ ] **Step 0 (MANDATORY, land before the rewrite): test-suite survival stub**

vitest here defaults to **node** env (no global `window`) and 40 test files mock `@tauri-apps/api/core`. `safeInvoke` checks `backendAvailable()` BEFORE the mocked `invoke` — without a stub, every one of those suites rejects with `BackendUnavailableError` and their `expect(mockInvoke).toHaveBeenCalledWith(...)` assertions all fail. This is a certainty, not an "if".

Append to `src/test-setup.ts`:

```ts
// Phase A backendGuard: tests exercise transport against mocked
// @tauri-apps/api — make detection succeed in BOTH node and jsdom envs.
// Suites asserting no-backend behavior delete this key and call
// __resetBackendAvailabilityForTests() in their own beforeEach.
(globalThis as any).__TAURI_INTERNALS__ ??= {};
```

(This pairs with Task 1's env-agnostic detection — no fake `window` is fabricated in node-env suites.)

- [ ] **Step 1: Write the failing source-scan guard**

```ts
// crates/vox-gui/ui/src/guards/transportIpcGuard.test.ts
import { describe, it, expect } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

/**
 * Phase A guard: inside transport.ts, only the marked region may touch raw
 * Tauri IPC. (Direct imports in components are separate tracked debt:
 * ipcBoundaries.test.ts allowlist for `invoke`; `listen` imports are
 * covered user-visibly by backendGuard's rejection filter.)
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
    const offenders = [...outside.matchAll(/(?<![A-Za-z_$.])(invoke|listen)\s*[(<]/g)].map((m) => m[0]);
    expect(offenders, `raw IPC outside safe wrappers: ${JSON.stringify(offenders)}`).toEqual([]);
  });
});
```

- [ ] **Step 2: RED** — first test fails (no markers); second reports **~71 offenders (60 invoke + 11 listen)**. The import line does not match (`invoke` there is followed by ` }`).

- [ ] **Step 3: Add the safe wrappers** — below the imports in `transport.ts`:

```ts
import { backendAvailable, BackendUnavailableError } from './lib/backendGuard';

// __VOX_RAW_IPC_BEGIN__
// The ONLY permitted raw Tauri `invoke`/`listen` uses in this file.
// Guarded by src/guards/transportIpcGuard.test.ts.
function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (!backendAvailable()) return Promise.reject(new BackendUnavailableError(cmd));
  return invoke<T>(cmd, args);
}

function safeListen<T>(
  event: string,
  handler: (event: { payload: T }) => void,
): Promise<UnlistenFn> {
  if (!backendAvailable()) return Promise.reject(new BackendUnavailableError(`listen:${event}`));
  return listen<T>(event, handler);
}
// __VOX_RAW_IPC_END__
```

- [ ] **Step 4: Mechanical rewrite of all 71 sites** — `invoke<` → `safeInvoke<`, `invoke(` → `safeInvoke(` at the 60 invoke sites; `listen<` → `safeListen<` inside **all 11** wrapper functions (`listenOrchStatus`, `listenAgentEvents`, `listenScientiaQueue`, `listenDiscoverySurfaced`, `listenBrowserFrames`, `listenPreviewAvailable`, `listenSecretaryProposed`, `listenPtyOutput`, `listenPtyExit`, `listenActivityAppended`, `listenFeedbackChanged`) — do not stop after the first. Apply replacements only outside the marker block (or write the block last). No signature/generic/argument changes.

- [ ] **Step 5: GREEN + full frontend suite (required, not conditional)**

```
pnpm exec vitest run src/guards/transportIpcGuard.test.ts   # 2 passed
pnpm typecheck                                              # clean
pnpm test                                                   # ALL suites pass thanks to Step 0's stub
```

Any suite that asserts unavailable-mode behavior must `delete (globalThis as any).__TAURI_INTERNALS__` + `__resetBackendAvailabilityForTests()` in its own `beforeEach` (currently only the new backendGuard/BackendBanner tests do this).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/test-setup.ts crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/guards/transportIpcGuard.test.ts
git commit -m "fix(gui): route all 71 transport IPC sites through safeInvoke/safeListen" -m "Mandatory test-setup stub lands with it: node-env vitest suites mock @tauri-apps/api but safeInvoke consults backendAvailable() first." -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 3: Normal-flow banner + filter install + no-tauri consolidation (Phase A)

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/BackendBanner.tsx`
- Test: `crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx`
- Modify: `crates/vox-gui/ui/src/main.tsx` (install filter; consolidate the inline no-Tauri check at :25 onto `backendAvailable()`)
- Modify: `crates/vox-gui/ui/src/App.tsx` (flex-column wrapper)
- Modify: `crates/vox-gui/ui/src/components/layout/AppShell.tsx:93` (`h-screen` → `h-full`)

- [ ] **Step 1: Failing component test**

```tsx
// crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx
// @vitest-environment jsdom
import { describe, it, expect, afterEach, beforeEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/react';
import React from 'react';
import { BackendBanner } from './BackendBanner';
import { __resetBackendAvailabilityForTests } from '../../lib/backendGuard';

beforeEach(() => {
  // test-setup.ts stubs globalThis.__TAURI_INTERNALS__ for the suite at
  // large; this suite asserts no-backend behavior, so remove it first.
  delete (globalThis as any).__TAURI_INTERNALS__;
  delete (window as any).__TAURI_INTERNALS__;
  __resetBackendAvailabilityForTests();
});
afterEach(() => {
  cleanup();
  (globalThis as any).__TAURI_INTERNALS__ = {};
  __resetBackendAvailabilityForTests();
});

describe('BackendBanner', () => {
  it('renders in no-backend mode and dismisses on click', () => {
    render(<BackendBanner />);
    expect(screen.getByRole('status', { name: /browser preview/i })).toBeInTheDocument();
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

- [ ] **Step 2: RED**, then **Step 3: Implement** — **normal flow, no fixed overlay** (a fixed top-0 z-[100] bar would permanently occlude the sidebar header + TopHud — the exact defect class this project hunts; AppShell has no fixed header, layers top out at z-[60]):

```tsx
// crates/vox-gui/ui/src/components/ui/BackendBanner.tsx
import React, { useState } from 'react';
import { backendAvailable } from '../../lib/backendGuard';

/** Normal-flow honesty banner for bare-browser mode: pushes the shell down
 * instead of overlaying it (no occlusion). Dismissible. */
export function BackendBanner() {
  const [dismissed, setDismissed] = useState(false);
  if (backendAvailable() || dismissed) return null;
  return (
    <div
      role="status"
      aria-label="Browser preview mode"
      className="flex shrink-0 items-center justify-center gap-3 border-b border-amber-500/40 bg-amber-950/90 px-4 py-1.5 text-[12px] text-amber-200"
    >
      <span>Browser preview — no desktop backend connected; surfaces show empty states.</span>
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

- `App.tsx`: wrap the AppShell render in `<div className="flex h-screen flex-col"><BackendBanner /><AppShell …/></div>`.
- `AppShell.tsx:93`: root `h-screen` → `h-full` (one word).
- `main.tsx`: call `installBackendUnavailableRejectionFilter()` before `createRoot`; replace the inline no-Tauri check at :25 with `if (!backendAvailable()) …` (single source of truth — don't leave the drift).

- [ ] **Step 5: GREEN + live proof (correctly scoped)**

`pnpm exec vitest run src/components/ui/BackendBanner.test.tsx` (2 passed); `pnpm typecheck && pnpm test`; `pnpm exec playwright test e2e/error-states.spec.ts --project=chromium` (still 4 passed). Live: dev server + plain browser → banner visible in flow (content pushed down, nothing occluded), console shows `[backendGuard] suppressed…` debug lines, and **zero uncaught** raw TypeErrors. Note: caught paths still *display* raw text (e.g. the "Chat sessions" toast renders `String(err)`) — that leakage is a Phase D finding class, not a Task 3 failure. The automated regression for all of this is Task 5/7's `no-backend` capture state.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/ui/BackendBanner.tsx crates/vox-gui/ui/src/components/ui/BackendBanner.test.tsx crates/vox-gui/ui/src/main.tsx crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/components/layout/AppShell.tsx
git commit -m "feat(gui): normal-flow browser-mode banner + rejection-filter install + no-tauri consolidation" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 4: Rich mock layered over `installTauriMock` (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/lib/tauriMockRich.ts`
- Test: `crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts`

Design (audit-corrected): layer over `installTauriMock` — which already answers ~60 commands with representative shapes — instead of the sparse bootstrap; otherwise dashboard/flow/console/search render blank and the review yields false "blank panel" positives. The dense msgpack `OrchestratorStatus` is encoded at **compose time in Node** (`@msgpack/msgpack` is already a ui dependency, used by `useOrchestratorStatus.ts:4`) and injected as a byte-array literal.

- [ ] **Step 1: Write the failing test** — exactly this final test (no adjust-later escape hatch); the `new Function(richMockInitScript(...))()` idiom mirrors `tauriMockVariants.test.ts:126-135` and proves self-containment against a bare window:

```ts
// crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts
import { describe, it, expect } from 'vitest';
import { decode } from '@msgpack/msgpack';
import { RICH_DATASET, buildRichOrchestratorStatus, richMockInitScript } from './tauriMockRich';

function makeFakeWindow(): any {
  const storage: Record<string, string> = {};
  return {
    localStorage: {
      setItem: (k: string, v: string) => { storage[k] = v; },
      getItem: (k: string) => storage[k] ?? null,
    },
  };
}
async function withFakeWindow<T>(fn: (win: any) => Promise<T> | T): Promise<T> {
  const prev = (global as any).window;
  const win = makeFakeWindow();
  (global as any).window = win;
  try { return await fn(win); } finally { (global as any).window = prev; }
}

describe('RICH_DATASET density', () => {
  it('is dense and overflow-shaped (sparse mocks are why occlusion hid)', () => {
    expect(RICH_DATASET.hopperTasks.length).toBeGreaterThanOrEqual(40);
    expect(RICH_DATASET.chatSessions.length).toBeGreaterThanOrEqual(12);
    expect(RICH_DATASET.models.length).toBeGreaterThanOrEqual(30);
    expect(RICH_DATASET.providers.length).toBeGreaterThanOrEqual(6);
    expect(Math.max(...RICH_DATASET.hopperTasks.map((t) => t.intent.length))).toBeGreaterThanOrEqual(120);
    const all = JSON.stringify(RICH_DATASET);
    expect(all).toMatch(/[Ѐ-ӿ]/);
    expect(all).toMatch(/[֐-׿]/);
  });
  it('dataset shapes stay on the wire contract', () => {
    for (const t of RICH_DATASET.hopperTasks) {
      expect([0, 1, 2]).toContain(t.priority);
      expect(['inbox', 'assigned', 'done']).toContain(t.state);
    }
    expect(RICH_DATASET.models.some((m) => m.id.includes('ollama') || m.id.startsWith('mens/') || m.id.startsWith('mesh/'))).toBe(true);
    expect(RICH_DATASET.models.some((m) => !(m.id.includes('ollama') || m.id.startsWith('mens/') || m.id.startsWith('mesh/')))).toBe(true);
  });
});

describe('richMockInitScript serialization', () => {
  it('composed script is self-contained and answers dense commands on a bare window', async () => {
    await withFakeWindow(async (win) => {
      // eslint-disable-next-line no-new-func -- exercising the exact addInitScript path
      new Function(richMockInitScript('tasks'))();
      expect(win.__VOX_MOCK_SHARED__).toBeDefined();
      expect((await win.__TAURI_INTERNALS__.invoke('hopper_list')).length).toBeGreaterThanOrEqual(40);
      expect((await win.__TAURI_INTERNALS__.invoke('chat_list_sessions', { limit: 40 })).length).toBeGreaterThanOrEqual(12);
      expect(await win.__TAURI_INTERNALS__.invoke('chat_list_sessions', { limit: 1 })).toHaveLength(1);
      expect(await win.__TAURI_INTERNALS__.invoke('get_initial_view')).toBe('tasks');
    });
  });
  it('delegates non-dense commands to the full base mock, not bootstrap nulls', async () => {
    await withFakeWindow(async (win) => {
      new Function(richMockInitScript('vox-search'))();
      const catalog = await win.__TAURI_INTERNALS__.invoke('get_command_catalog');
      expect(catalog.entries.length).toBeGreaterThan(0);
    });
  });
  it('serves a dense msgpack orchestrator snapshot (dashboard/flow are not blank)', async () => {
    await withFakeWindow(async (win) => {
      new Function(richMockInitScript('dashboard'))();
      const bin = await win.__TAURI_INTERNALS__.invoke('get_orchestrator_status_bin');
      const status = decode(bin) as ReturnType<typeof buildRichOrchestratorStatus>;
      expect(status.agents.length).toBeGreaterThanOrEqual(8);
      expect(status.recent_events.length).toBeGreaterThanOrEqual(20);
      expect(status.alerts.length).toBeGreaterThan(0);
    });
  });
});
```

- [ ] **Step 2: RED** — module not found.

- [ ] **Step 3: Implement** (final, complete):

```ts
// crates/vox-gui/ui/e2e/lib/tauriMockRich.ts
/**
 * Dense, overflow-shaped mock for the review-bundle capture matrix, layered
 * over installTauriMock. Sparse mocks are why occlusion/clipping never
 * showed: 44 tasks with 120+-char unicode/RTL titles, 32 models, 6
 * providers, and a dense msgpack orchestrator snapshot make truncation,
 * overlap, and z-fighting visible.
 *
 * Serialization contract (mirrors tauriMockShared.mockInitScript):
 * addInitScript serialises function SOURCE only, so richMockInitScript()
 * composes one self-contained script string:
 *   1. mockInitScript(installTauriMock, viewKey)   // shared + base mock
 *   2. window.__VOX_RICH_BUILD__      = <buildRichDataset source>
 *   3. window.__VOX_RICH_STATUS_BIN__ = new Uint8Array([...])  // msgpack,
 *      encoded HERE in Node — @msgpack/msgpack can't run inside the page
 *   4. (<installTauriMockRich source>)(viewKey)    // wraps base invoke
 */
import type { Page } from '@playwright/test';
import { encode } from '@msgpack/msgpack';
import { mockInitScript } from './tauriMockShared';
import { installTauriMock } from './tauriMock';

/** Self-contained (no captured module scope) — stringified into the page. */
export function buildRichDataset() {
  const long = (s: string, n: number) => s.repeat(Math.ceil(n / s.length)).slice(0, n);
  const hopperTasks = Array.from({ length: 44 }, (_, i) => ({
    item_id: `hop-rich-${i + 1}`,
    intent:
      i % 7 === 0
        ? long(`Refactor the international pipeline № ${i} — очень длинное название задачи с юникодом `, 140)
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
  const models = Array.from({ length: 32 }, (_, i) => {
    const id = i % 5 === 0
      ? ['ollama/llama3-rich-', 'mens/finetune-rich-', 'mesh/node-model-rich-'][i % 3] + i
      : `provider-${i % 6}/model-family-name-${i}-with-a-rather-long-suffix-v${i}.${i % 10}`;
    return {
      id, model_id: id,
      display_name: long(`Model Family ${i} Extended Display Name `, 40 + (i % 30)),
      provider: ['openai', 'anthropic', 'google', 'ollama', 'mistralai', 'meta-llama'][i % 6],
      tier: ['Frontier', 'Fast', 'Budget'][i % 3],
      cost_per_1k: i * 0.0007,
      max_tokens: 8192 * ((i % 4) + 1),
      is_free: i % 8 === 0,
      latency_p50_ms: 200 + i * 13,
      success_rate: 0.9 + (i % 10) / 100,
      quality_score: 0.5 + (i % 50) / 100,
    };
  });
  const providers = Array.from({ length: 6 }, (_, i) => ({
    provider: ['OpenRouter', 'Anthropic', 'OpenAI', 'Ollama', 'Mens Local Inference Cluster (long provider name)', 'Mesh'][i],
    key_present: i !== 2,
    is_local: i >= 3,
    local_reachable: i >= 3 ? i !== 5 : null,
    local_models: i >= 3 ? ['llama3.2', 'qwen-coder-7b', 'mens-8b-instruct-longname'] : [],
  }));
  return { hopperTasks, chatSessions, models, providers };
}

export const RICH_DATASET = buildRichDataset();

/** Dense OrchestratorStatus for dashboard/flow/console. Encoded to msgpack
 * at compose time — NOT inside the page. */
export function buildRichOrchestratorStatus() {
  const agents = Array.from({ length: 9 }, (_, i) => ({
    id: i + 1,
    codename: ['Aquila', 'Bellona', 'Cato', 'Drusus', 'Egeria', 'Faunus', 'Gallus', 'Hersilia', 'Iovis'][i],
    name: `agent-${i + 1}`,
    in_progress: i % 3 !== 0,
    paused: i === 4,
    progress: i % 3 === 0 ? null : ((i * 11) % 100) / 100,
    current_phase: ['plan', 'implement', 'verify', 'review'][i % 4],
    task_description: `Task ${i + 1}: a deliberately long in-flight task description that should truncate or wrap inside the agent card rather than overflow its container boundaries`,
    cost: i * 0.42,
    budget: i % 2 ? 5 : null,
    eta: `${5 + i}m`,
    active_skill: i % 2 ? 'superpowers:test-driven-development' : undefined,
  }));
  const recent_events = Array.from({ length: 24 }, (_, i) => ({
    id: i + 1,
    kind: (['task_started', 'phase_change', 'task_completed', 'doubt_raised'] as const)[i % 4],
    tag: `agent-${(i % 9) + 1}`,
    title: `Event ${i + 1}: ${['started', 'phase → verify', 'completed', 'doubt raised'][i % 4]}`,
    body: 'A stream event body long enough to exercise two-line clamping in the console event feed rendering path.',
    timestamp: 'now',
  }));
  return {
    agent_count: agents.length,
    total_queued: 44, total_in_progress: 6, total_completed: 128, total_doubted: 3,
    total_weighted_load: 7.5, predicted_load: 8.2,
    agents, recent_events,
    alerts: [
      { id: 'al-1', level: 'warn', title: 'Budget 80% consumed', body: 'Exploration spend approaching the configured cap.' },
      { id: 'al-2', level: 'ok', title: 'Mesh healthy', body: 'All peers reachable.' },
    ],
    peers: [
      { id: 'node-a', status: 'online' },
      { id: 'node-b', status: 'online' },
      { id: 'node-remote-very-long-hostname.example.internal', status: 'degraded' },
    ],
    total_cost: 12.34, budget_cap: 50, mesh_throughput: 3.2,
  };
}

/** Self-contained installer: runs AFTER installTauriMock in the same init
 * script; wraps the base invoke and overrides only the dense commands. */
export function installTauriMockRich(viewKey: string): void {
  const internals = (window as any).__TAURI_INTERNALS__;
  const base: ((cmd: string, args?: any) => Promise<unknown>) | undefined = internals?.invoke;
  const build = (window as any).__VOX_RICH_BUILD__;
  const statusBin = (window as any).__VOX_RICH_STATUS_BIN__;
  if (typeof base !== 'function' || typeof build !== 'function' || !statusBin) {
    throw new Error('installTauriMockRich must be injected via addRichMockInitScript after installTauriMock');
  }
  void viewKey; // navigation is seeded by installTauriMock
  const data = build();
  internals.invoke = async (cmd: string, args?: any) => {
    switch (cmd) {
      case 'hopper_list':
        return data.hopperTasks.map((t: any) => ({ ...t }));
      case 'chat_list_sessions': {
        const limit = typeof args?.limit === 'number' ? args.limit : data.chatSessions.length;
        return data.chatSessions.slice(0, limit).map((s: any) => ({ ...s }));
      }
      case 'list_model_cards':
        return data.models;
      case 'inference_provider_status':
        return data.providers;
      case 'get_gamify_settings':
        return { enabled: true, mode: 'balanced' };
      case 'get_orchestrator_status_bin':
        return statusBin;
      default:
        return base(cmd, args);
    }
  };
}

/** Compose the full self-contained init script (exported for unit tests). */
export function richMockInitScript(viewKey: string): string {
  const statusBytes = Array.from(encode(buildRichOrchestratorStatus())).join(',');
  return [
    mockInitScript(installTauriMock, viewKey),
    `window.__VOX_RICH_BUILD__ = ${buildRichDataset.toString()};`,
    `window.__VOX_RICH_STATUS_BIN__ = new Uint8Array([${statusBytes}]);`,
    `(${installTauriMockRich.toString()})(${JSON.stringify(viewKey)});`,
  ].join('\n');
}

/** The ONLY supported way to inject the rich mock into a Playwright page. */
export async function addRichMockInitScript(page: Page, viewKey: string): Promise<void> {
  await page.addInitScript({ content: richMockInitScript(viewKey) });
}
```

- [ ] **Step 4: GREEN** — `pnpm exec vitest run e2e/lib/tauriMockRich.test.ts`; `pnpm typecheck`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/lib/tauriMockRich.ts crates/vox-gui/ui/e2e/lib/tauriMockRich.test.ts
git commit -m "feat(gui-e2e): dense rich mock layered over installTauriMock + msgpack orchestrator snapshot" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 5: State registry (strict guard, viewport/mock-aware) (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/review/states.ts`
- Create: `crates/vox-gui/ui/src/guards/reviewStates.guard.test.ts`

- [ ] **Step 1: Failing guard** — the rot guard is **strict**: every registry surface needs an *explicit* entry (even just `[DEFAULT]`), so adding a surface forces a states decision:

```ts
// crates/vox-gui/ui/src/guards/reviewStates.guard.test.ts
import { describe, it, expect } from 'vitest';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { SURFACE_STATES, VIEWPORTS } from '../../e2e/review/states';

describe('review state registry completeness', () => {
  const known = SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string);

  it('every registry surface has an EXPLICIT states entry (even just [DEFAULT])', () => {
    const missing = known.filter((k) => !(k in SURFACE_STATES));
    expect(missing, `add states (or [DEFAULT]) for: ${missing}`).toEqual([]);
  });
  it('declared states only reference registered surfaces (no typo rot)', () => {
    const unknown = Object.keys(SURFACE_STATES).filter((k) => !known.includes(k));
    expect(unknown).toEqual([]);
  });
  it('viewports are the spec trio', () => {
    expect(VIEWPORTS.map((v) => v.name)).toEqual(['wide', 'laptop', 'compact']);
  });
  it('viewport constraints reference real viewport names', () => {
    const names = new Set(VIEWPORTS.map((v) => v.name));
    for (const states of Object.values(SURFACE_STATES)) {
      for (const s of states) for (const v of s.viewports ?? []) expect(names.has(v)).toBe(true);
    }
  });
});
```

- [ ] **Step 2: RED**, then **Step 3: Implement**

```ts
// crates/vox-gui/ui/e2e/review/states.ts
import type { Page } from '@playwright/test';

export interface ReviewViewport { name: 'wide' | 'laptop' | 'compact'; width: number; height: number; }
export const VIEWPORTS: ReviewViewport[] = [
  { name: 'wide', width: 1440, height: 900 },
  { name: 'laptop', width: 1100, height: 720 },
  { name: 'compact', width: 900, height: 600 },
];

export type MockKind = 'rich' | 'empty' | 'error' | 'none';

export interface ReviewState {
  name: string;
  /** Drive the page into the state AFTER the surface has rendered. */
  setup?: (page: Page) => Promise<void>;
  /** Restrict to viewports where this state's UI exists. */
  viewports?: Array<'wide' | 'laptop' | 'compact'>;
  /** Which mock installer backs this capture (default 'rich').
   * 'empty'/'error' subsume screenshots-variants.spec.ts; 'none' captures
   * true no-backend browser mode (BackendBanner regression). */
  mock?: MockKind;
}

const DEFAULT: ReviewState = { name: 'default' };
/** Empty/error coverage inherited from screenshots-variants KEY_SURFACES. */
const VARIANT: ReviewState[] = [
  { name: 'empty', mock: 'empty' },
  { name: 'error', mock: 'error' },
];
const VARIANT_SURFACES = new Set([
  'dashboard', 'chat', 'runs', 'approvals', 'models',
  'memory', 'vox-search', 'policies', 'gamify', 'settings',
]);

/** Selector ground truth verified 2026-07-18; a failed setup records
 * state_ok:false in the entry (evidence), it does not fail the run. */
export const SURFACE_STATES: Record<string, ReviewState[]> = Object.fromEntries(
  // Every surface starts with an explicit [DEFAULT]; specifics below override.
  ([
    'activity', 'approvals', 'browser', 'catalog', 'chat', 'coderabbit', 'console',
    'coverage', 'dashboard', 'flow', 'harness', 'memory', 'mercatus', 'mesh',
    'models', 'needs-you', 'policies', 'publications', 'runs', 'settings',
    'skills', 'sub-agents', 'tasks', 'vox-search', 'gamify', 'repository',
    'scientia', 'mens', 'populi', 'research', 'oratio',
  ] as string[]).map((k) => [k, [DEFAULT]]),
);

SURFACE_STATES['chat'] = [
  DEFAULT,
  {
    name: 'model-picker-open',
    // Scoped: 'model:' prefix could collide with transcript text.
    setup: async (p) => { await p.getByTestId('chat-surface-layout').getByRole('button', { name: /^model:/i }).click(); },
  },
  {
    name: 'session-menu-open',
    // Viewport-tolerant: at compact width the rail hides behind a toggle;
    // opening it ALSO captures the overlay-over-transcript occlusion case.
    setup: async (p) => {
      const toggle = p.getByTestId('chat-session-rail-toggle');
      if (await toggle.isVisible()) await toggle.click();
      await p.getByRole('button', { name: /session actions for/i }).first().click();
    },
  },
  {
    name: 'composer-filled',
    setup: async (p) => {
      await p.getByLabel('Task composer').fill(
        'A deliberately long composer draft that should wrap across multiple lines and reveal any clipping or overlap issues in the dock '.repeat(2),
      );
    },
  },
  {
    name: 'rails-overlay-open',
    viewports: ['compact'],
    setup: async (p) => { await p.getByTestId('chat-session-rail-toggle').click(); },
  },
  ...VARIANT,
];

SURFACE_STATES['tasks'] = [
  DEFAULT,
  {
    name: 'composer-filled',
    setup: async (p) => {
      await p.getByLabel('Add a task').fill(
        'Draft task with an intentionally very long title to probe truncation and row overflow behavior in the composer',
      );
    },
  },
  // NOTE: priority-select-open is intentionally omitted — native <select>
  // popups render outside the page and cannot be screenshotted.
];

SURFACE_STATES['settings'] = [
  DEFAULT,
  { name: 'search-filtered', setup: async (p) => { await p.getByLabel('Search settings').fill('key'); } },
  { name: 'section-keybinds', setup: async (p) => { await p.getByRole('button', { name: 'Keybinds' }).click(); } },
  ...VARIANT,
];

SURFACE_STATES['approvals'] = [
  DEFAULT,
  {
    name: 'row-focused',
    setup: async (p) => { for (let i = 0; i < 4; i++) await p.keyboard.press('Tab'); },
  },
  ...VARIANT,
];

SURFACE_STATES['dashboard'] = [
  DEFAULT,
  { name: 'omnibar-open', setup: async (p) => { await p.keyboard.press('Control+k'); } },
  { name: 'sidebar-collapsed', setup: async (p) => { await p.getByRole('button', { name: 'Collapse sidebar' }).click(); } },
  { name: 'achievements-open', setup: async (p) => { await p.getByRole('button', { name: 'Open achievements' }).click(); } },
  {
    name: 'hud-hidden',
    setup: async (p) => { await p.keyboard.press('Control+Shift+H'); await p.keyboard.press('Control+Shift+H'); },
  },
  {
    name: 'focus-visible',
    setup: async (p) => { for (let i = 0; i < 4; i++) await p.keyboard.press('Tab'); },
  },
  // The Phase A regression: no mock at all -> banner must render, zero raw
  // TypeErrors; the banner's own placement gets AI-reviewed.
  { name: 'no-backend', mock: 'none', viewports: ['wide'] },
  ...VARIANT,
];

for (const k of VARIANT_SURFACES) {
  if (!SURFACE_STATES[k].some((s) => s.name === 'empty')) SURFACE_STATES[k].push(...VARIANT);
}

/** Themes captured for default states at wide/chromium (Task 7). */
export const AUDIT_THEMES = ['high-contrast'] as const;

export function statesFor(viewKey: string): ReviewState[] {
  return SURFACE_STATES[viewKey] ?? [DEFAULT];
}
```

**Adjust the surface-key list to the real registry** (31 keys — Step 2's guard RED run prints the missing/unknown sets; fix until green; do NOT pad with fabricated keys).

- [ ] **Step 4: GREEN** — guard 4/4; `pnpm typecheck`.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/review/states.ts crates/vox-gui/ui/src/guards/reviewStates.guard.test.ts
git commit -m "feat(gui-e2e): strict review state registry - viewport/mock-aware states incl. variants + no-backend + theme list" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 6: Page-audit helpers + vitest include extension (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/review/audits.ts`
- Test: `crates/vox-gui/ui/e2e/review/audits.test.ts`
- Modify: `crates/vox-gui/ui/vitest.config.ts` (**include `e2e/review/**/*.test.ts`** — currently only `src/**` + `e2e/lib/**` patterns run, so these tests would silently never execute)

- [ ] **Step 1: Failing unit test** (jsdom-corrected semantics):

```ts
// crates/vox-gui/ui/e2e/review/audits.test.ts
// @vitest-environment jsdom
import { describe, it, expect } from 'vitest';
import { auditIconsInPage, auditOverflowInPage } from './audits';

describe('auditIconsInPage', () => {
  it('flags zero-size svgs, drawless svgs, and broken imgs; passes healthy ones', () => {
    document.body.innerHTML = `
      <svg id="ok"><path d="M0 0h16v16z"/></svg>
      <svg id="drawless"></svg>
      <svg id="zero"><path d="M0 0h16v16z"/></svg>
      <img id="broken" src="x.png" alt="icon" />
    `;
    // jsdom rects are 0x0 — emulate rendered sizes explicitly.
    const rect16 = () => ({ width: 16, height: 16 }) as DOMRect;
    (document.getElementById('ok') as any).getBoundingClientRect = rect16;
    (document.getElementById('drawless') as any).getBoundingClientRect = rect16;
    // 'zero' keeps the 0x0 default -> zero-size branch.
    // jsdom imgs: complete=false by default -> force the loaded-but-broken shape.
    const broken = document.getElementById('broken') as HTMLImageElement;
    Object.defineProperty(broken, 'complete', { value: true });
    // naturalWidth is 0 in jsdom already.

    const issues = auditIconsInPage();
    expect(issues.some((i) => i.kind === 'empty-svg' && i.id === 'drawless')).toBe(true);
    expect(issues.some((i) => i.kind === 'zero-size-svg' && i.id === 'zero')).toBe(true);
    expect(issues.some((i) => i.kind === 'broken-img' && i.id === 'broken')).toBe(true);
    expect(issues.some((i) => i.id === 'ok')).toBe(false);
  });
});

describe('auditOverflowInPage', () => {
  it('reports body horizontal overflow', () => {
    Object.defineProperty(document.body, 'scrollWidth', { value: 1600, configurable: true });
    Object.defineProperty(document.body, 'clientWidth', { value: 1440, configurable: true });
    expect(auditOverflowInPage().bodyHorizontalOverflowPx).toBe(160);
  });
});
```

- [ ] **Step 2: RED** (after the vitest include change — verify the test is *collected* then fails on module-not-found), then **Step 3: Implement**:

```ts
// crates/vox-gui/ui/e2e/review/audits.ts
/**
 * Per-capture in-page audits. Each function is passed to page.evaluate(fn)
 * — Playwright serializes the SOURCE, so keep them fully self-contained.
 */

export interface IconIssue {
  kind: 'zero-size-svg' | 'empty-svg' | 'broken-img';
  id: string;
  testid: string;
  selectorHint: string;
}

export function auditIconsInPage(): IconIssue[] {
  const issues: IconIssue[] = [];
  // getAttribute('class'): el.className is an SVGAnimatedString on SVG.
  const hint = (el: Element) =>
    `${el.tagName.toLowerCase()}${el.id ? `#${el.id}` : ''}.${(el.getAttribute('class') || '').split(/\s+/)[0]}`;
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
  contentHeightPx: number;
}

export function auditOverflowInPage(): OverflowReport {
  const body = document.body;
  const host = document.querySelector('[data-testid="surface-scroll-host"]') as HTMLElement | null;
  return {
    bodyHorizontalOverflowPx: Math.max(0, body.scrollWidth - body.clientWidth),
    scrollHostHorizontalOverflowPx: host ? Math.max(0, host.scrollWidth - host.clientWidth) : 0,
    contentHeightPx: Math.max(body.scrollHeight, document.documentElement.scrollHeight),
  };
}
```

- [ ] **Step 4: GREEN**; **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/review/audits.ts crates/vox-gui/ui/e2e/review/audits.test.ts crates/vox-gui/ui/vitest.config.ts
git commit -m "feat(gui-e2e): in-page icon/overflow audits + vitest include for e2e/review tests" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 7: Capture spec — determinism, per-worker JSONL, themes (Phase B)

**Files:**
- Create: `crates/vox-gui/ui/e2e/review/capture.spec.ts`
- Create: `crates/vox-gui/ui/e2e/review/globalSetup.ts`
- Modify: `crates/vox-gui/ui/playwright.config.ts` (firefox project + globalSetup)
- Modify: `crates/vox-gui/ui/package.json` (script), `.gitignore` (`review-bundle/`)

This is a **harness task**: verified by env-gated smoke runs, not RED/GREEN (the pure logic lives in Tasks 4-6 which are TDD).

- [ ] **Step 1: Dependencies** — `pnpm add -D @axe-core/playwright && pnpm exec playwright install firefox`.

- [ ] **Step 2: Config**

`playwright.config.ts`:

```ts
  globalSetup: './e2e/review/globalSetup.ts',
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
    {
      // Review-capture only: the user evaluates in Firefox; Gecko layout
      // differs from Blink. The asserting sweep stays chromium-only.
      name: 'firefox-review',
      grep: /@review-capture/,
      use: { ...devices['Desktop Firefox'] },
    },
  ],
```

`globalSetup.ts` (clears stale entries so reruns don't mix bundles; no-op without the env gate so default runs are untouched):

```ts
import { rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

export default function globalSetup() {
  if (process.env.VOX_REVIEW_CAPTURE !== '1') return;
  const out = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'review-bundle', 'latest');
  rmSync(out, { recursive: true, force: true });
}
```

`package.json` scripts: `"review:capture": "playwright test e2e/review/capture.spec.ts --project=chromium --project=firefox-review --workers=4"` (env set by the caller: `$env:VOX_REVIEW_CAPTURE='1'` in PowerShell, or the Task-11 wrapper — do NOT add cross-env). `.gitignore`: `review-bundle/`.

- [ ] **Step 3: Write `capture.spec.ts`**

```ts
// crates/vox-gui/ui/e2e/review/capture.spec.ts
/**
 * Review-bundle capture matrix @review-capture. Env-gated (VOX_REVIEW_CAPTURE=1).
 * Captures are EVIDENCE: failed state setups record state_ok:false, they do
 * not fail the run. Viewport-clipped screenshots (fullPage explodes on rich
 * lists and downscales to unreadability for the vision model); content
 * height is recorded per entry instead.
 */
import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
import { createHash } from 'node:crypto';
import { appendFileSync, mkdirSync, readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SURFACE_REGISTRY } from '../../src/generated/surfaceRegistry.generated';
import { VIEWPORTS, statesFor, AUDIT_THEMES, type ReviewState } from './states';
import { auditIconsInPage, auditOverflowInPage } from './audits';
import { addRichMockInitScript } from '../lib/tauriMockRich';
import { addMockInitScript } from '../lib/tauriMockShared';
import { installEmptyStateMock, installErrorStateMock } from '../lib/tauriMockVariants';

const RUN = process.env.VOX_REVIEW_CAPTURE === '1';
const OUT = join(dirname(fileURLToPath(import.meta.url)), '..', '..', 'review-bundle', 'latest');
const SURFACES = SURFACE_REGISTRY.filter((e) => e.viewKey != null).map((e) => e.viewKey as string);
// Benign noise filter (mirrors screenshots.spec.ts): favicon fetches etc.
const BENIGN = [/favicon/i];

async function installMock(page: import('@playwright/test').Page, kind: string, surface: string) {
  if (kind === 'empty') return addMockInitScript(page, installEmptyStateMock, surface);
  if (kind === 'error') return addMockInitScript(page, installErrorStateMock, surface);
  if (kind === 'none') return; // true browser mode — Phase A regression
  return addRichMockInitScript(page, surface);
}

async function captureOne(
  page: import('@playwright/test').Page,
  browserName: string,
  surface: string,
  state: ReviewState,
  vpName: string,
  theme: string | null,
) {
  mkdirSync(OUT, { recursive: true });
  const id = [surface, state.name, vpName, browserName, ...(theme ? [`theme-${theme}`] : [])].join('--');
  const consoleErrors: string[] = [];
  const consoleWarnings: string[] = [];
  const pageErrors: string[] = [];
  page.on('console', (m) => {
    const text = m.text();
    if (BENIGN.some((re) => re.test(text) || re.test(m.location()?.url ?? ''))) return;
    if (m.type() === 'error') consoleErrors.push(text);
    else if (m.type() === 'warning') consoleWarnings.push(text);
  });
  page.on('pageerror', (e) => pageErrors.push(e.message));

  const t0 = Date.now();
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await installMock(page, state.mock ?? 'rich', surface);
  await page.goto('/');
  await page.waitForSelector('nav', { timeout: 20_000 });
  await page.evaluate(() => (document as any).fonts?.ready);
  if (theme) await page.evaluate((t) => { document.documentElement.dataset.theme = t; }, theme);

  let stateOk = true;
  let stateError = '';
  if (state.setup) {
    try {
      await state.setup(page);
    } catch (e) {
      stateOk = false;
      stateError = String(e);
    }
  }
  await page.waitForTimeout(400); // settle: menus, theme swap, layout

  const file = `${id}.png`;
  await page.screenshot({ path: join(OUT, file), animations: 'disabled' }); // viewport clip
  const sha256 = createHash('sha256').update(readFileSync(join(OUT, file))).digest('hex');

  let axeViolations: unknown[] = [];
  try {
    const axe = await new AxeBuilder({ page }).analyze();
    axeViolations = axe.violations.filter((v) => ['moderate', 'serious', 'critical'].includes(v.impact ?? ''));
  } catch (e) {
    consoleWarnings.push(`axe-failed: ${String(e)}`);
  }
  const iconIssues = await page.evaluate(auditIconsInPage);
  const overflow = await page.evaluate(auditOverflowInPage);

  const entry = {
    id, surface, state: state.name, viewport: vpName, browser: browserName,
    theme: theme ?? 'default', file, sha256,
    state_ok: stateOk, state_error: stateError,
    axe_violations: axeViolations,
    console_errors: consoleErrors.slice(0, 50),
    console_warnings: consoleWarnings.slice(0, 50),
    page_errors: pageErrors,
    icon_issues: iconIssues,
    overflow,
    capture_ms: Date.now() - t0,
    captured_at: new Date().toISOString(),
  };
  appendFileSync(
    join(OUT, `entries-${browserName}-w${test.info().workerIndex}.jsonl`),
    JSON.stringify(entry) + '\n',
  );
  if (state.mock === 'none') {
    // Phase A regression, now automated: banner renders; zero raw TypeErrors.
    await expect(page.getByRole('status', { name: /browser preview/i })).toBeVisible();
  }
  expect(pageErrors.filter((e) => /__TAURI_INTERNALS__/.test(e))).toEqual([]);
}

test.describe('review-bundle capture @review-capture', () => {
  test.skip(!RUN, 'set VOX_REVIEW_CAPTURE=1 to run the capture matrix');

  for (const surface of SURFACES) {
    for (const state of statesFor(surface)) {
      for (const vp of VIEWPORTS) {
        if (state.viewports && !state.viewports.includes(vp.name)) continue;
        test(`${surface} -- ${state.name} -- ${vp.name}`, async ({ page, browserName }) => {
          await page.setViewportSize({ width: vp.width, height: vp.height });
          await captureOne(page, browserName, surface, state, vp.name, null);
        });
      }
    }
  }

  // Theme sub-dimension: default state x wide x chromium only (bounded cost).
  for (const surface of SURFACES) {
    for (const theme of AUDIT_THEMES) {
      test(`${surface} -- default -- wide -- theme:${theme}`, async ({ page, browserName }) => {
        test.skip(browserName !== 'chromium', 'theme captures are chromium-only');
        await page.setViewportSize({ width: 1440, height: 900 });
        await captureOne(page, browserName, surface, { name: 'default' }, 'wide', theme);
      });
    }
  }
});
```

- [ ] **Step 4: Smoke slice → full run**

```
cd C:\Users\Owner\vox\crates\vox-gui\ui
$env:VOX_REVIEW_CAPTURE = '1'
pnpm exec playwright test e2e/review/capture.spec.ts --project=chromium -g "dashboard" --workers=2
pnpm review:capture
Remove-Item Env:VOX_REVIEW_CAPTURE
```

Expected: slice green; full run ≈ **370–400 captures** with per-worker `entries-*.jsonl`; spot-open PNGs (dense content, banner visible in the `no-backend` capture); a plain run WITHOUT the env var reports all skipped; `pnpm exec playwright test --project=chromium` default suite unaffected.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/e2e/review/capture.spec.ts crates/vox-gui/ui/e2e/review/globalSetup.ts crates/vox-gui/ui/playwright.config.ts crates/vox-gui/ui/package.json crates/vox-gui/ui/pnpm-lock.yaml crates/vox-gui/ui/.gitignore
git commit -m "feat(gui-e2e): review-bundle capture matrix - deterministic, per-worker JSONL, themes, no-backend regression" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 8: Bundle-entry types + loader (Rust, Phase C)

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/visus_review/bundle.rs`
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs` (`pub mod bundle;`)

(`tempfile` is already in `[dependencies]` (Cargo.toml:132) — no manifest change, no Cargo.toml in the commit.)

- [ ] **Step 1: Failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_a_capture_entry_line() {
        let line = r#"{"id":"chat--default--wide--chromium","surface":"chat","state":"default","viewport":"wide","browser":"chromium","theme":"default","file":"chat--default--wide--chromium.png","sha256":"ab","state_ok":true,"state_error":"","axe_violations":[{"id":"color-contrast","impact":"serious"}],"console_errors":["error: x"],"console_warnings":["warn: y"],"page_errors":[],"icon_issues":[],"overflow":{"bodyHorizontalOverflowPx":0,"scrollHostHorizontalOverflowPx":12,"contentHeightPx":2400},"capture_ms":1234,"captured_at":"t"}"#;
        let e: BundleEntry = serde_json::from_str(line).unwrap();
        assert_eq!(e.id, "chat--default--wide--chromium");
        assert_eq!(e.axe_violations.len(), 1);
        assert_eq!(e.overflow["scrollHostHorizontalOverflowPx"], 12);
        assert_eq!(e.capture_ms, 1234);
    }
    #[test]
    fn tolerates_missing_optional_fields() {
        let line = r#"{"id":"x","surface":"x","state":"default","viewport":"wide","browser":"firefox","file":"x.png","sha256":"cd"}"#;
        let e: BundleEntry = serde_json::from_str(line).unwrap();
        assert!(e.state_ok);
        assert!(e.console_errors.is_empty());
        assert_eq!(e.theme, "default");
    }
    #[test]
    fn load_bundle_reads_all_jsonl_files_and_skips_bad_lines() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("entries-chromium-w0.jsonl"),
            "{\"id\":\"a\",\"surface\":\"s\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"chromium\",\"file\":\"a.png\",\"sha256\":\"1\"}\nnot-json\n").unwrap();
        std::fs::write(dir.path().join("entries-firefox-w1.jsonl"),
            "{\"id\":\"b\",\"surface\":\"s\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"firefox\",\"file\":\"b.png\",\"sha256\":\"2\"}\n").unwrap();
        let (entries, skipped) = load_bundle(dir.path()).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(skipped, 1);
    }
}
```

- [ ] **Step 2: RED** (`cargo test -p vox-orchestrator-mcp --features gui-visual-review bundle > "$env:TEMP\bundle_red.log" 2>&1`), then **Step 3: Implement**:

```rust
// crates/vox-orchestrator-mcp/src/visus_review/bundle.rs
//! Review-bundle loader: reads the capture harness's per-worker
//! entries-*.jsonl files (crates/vox-gui/ui/review-bundle/latest).

use std::path::Path;

fn default_true() -> bool { true }
fn default_theme() -> String { "default".into() }

#[derive(Debug, Clone, serde::Deserialize)]
pub struct BundleEntry {
    pub id: String,
    pub surface: String,
    pub state: String,
    pub viewport: String,
    pub browser: String,
    #[serde(default = "default_theme")]
    pub theme: String,
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
    pub console_warnings: Vec<String>,
    #[serde(default)]
    pub page_errors: Vec<String>,
    #[serde(default)]
    pub icon_issues: Vec<serde_json::Value>,
    #[serde(default)]
    pub overflow: serde_json::Value,
    #[serde(default)]
    pub capture_ms: u64,
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
        if !(name.starts_with("entries-") && name.ends_with(".jsonl")) { continue; }
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

- [ ] **Step 4: GREEN + clippy + fmt** (`cargo fmt -p vox-orchestrator-mcp`).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/visus_review/bundle.rs crates/vox-orchestrator-mcp/src/visus_review/mod.rs
git commit -m "feat(visual-review): review-bundle JSONL entry types + loader" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 9: Defect rubric + prompts + PROMPT_VERSION bump (Phase C)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/prompt.rs`

- [ ] **Step 1: Failing tests** (extend `prompt::tests`):

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
            viewport: "compact".into(), browser: "firefox".into(), theme: "default".into(),
            file: "f.png".into(), sha256: "s".into(),
            state_ok: true, state_error: String::new(),
            axe_violations: vec![serde_json::json!({"id":"color-contrast","impact":"serious"})],
            console_errors: vec!["error: boom".into()], console_warnings: vec![],
            page_errors: vec![], icon_issues: vec![],
            overflow: serde_json::json!({"bodyHorizontalOverflowPx": 40}),
            capture_ms: 0, captured_at: "t".into(),
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
        assert!(PROMPT_VERSION >= "2026-07-18.1");
    }
    #[test]
    fn defect_user_prompt_forwards_only_serious_and_critical_axe() {
        let mut e = crate::visus_review::bundle::BundleEntry {
            id: "x".into(), surface: "x".into(), state: "default".into(),
            viewport: "wide".into(), browser: "chromium".into(), theme: "default".into(),
            file: "x.png".into(), sha256: "s".into(), state_ok: true, state_error: String::new(),
            axe_violations: vec![
                serde_json::json!({"id":"region","impact":"moderate"}),
                serde_json::json!({"id":"color-contrast","impact":"serious"}),
            ],
            console_errors: vec![], console_warnings: vec![], page_errors: vec![],
            icon_issues: vec![], overflow: serde_json::Value::Null, capture_ms: 0, captured_at: "t".into(),
        };
        let up = defect_user_prompt(&e);
        assert!(up.contains("color-contrast"));
        assert!(!up.contains("\"region\""), "moderate violations stay in the JSONL, out of the prompt");
        e.axe_violations.clear();
        let _ = defect_user_prompt(&e); // no panic on empty
    }
```

- [ ] **Step 2: RED**, then **Step 3: Implement** — bump `PROMPT_VERSION` to `"2026-07-18.1"` (deliberate one-time legacy-cache invalidation), keep legacy items, append:

```rust
/// Defect-hunting rubric for review-bundle analysis (vs RUBRIC's general
/// design quality): concrete rendering DEFECTS the capture matrix exists
/// to catch.
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
    // Noise policy: only serious/critical axe violations reach the model;
    // moderate ones stay in the JSONL for Phase D triage.
    let axe_hot: Vec<&serde_json::Value> = e
        .axe_violations
        .iter()
        .filter(|v| matches!(v["impact"].as_str(), Some("serious") | Some("critical")))
        .collect();
    format!(
        "Capture: surface '{surface}', state '{state}', viewport '{viewport}', browser '{browser}', theme '{theme}'.\n\
Programmatic findings for THIS capture (correlate, do not merely repeat):\n\
- axe (serious/critical): {axe}\n- console errors: {console:?}\n- page errors: {page:?}\n\
- icon issues: {icons}\n- overflow: {overflow}\n- state setup ok: {ok} {err}\n\
Analyze the attached screenshot per the defect rubric and output the JSON verdict.",
        surface = e.surface, state = e.state, viewport = e.viewport, browser = e.browser,
        theme = e.theme,
        axe = serde_json::to_string(&axe_hot).unwrap_or_default(),
        console = e.console_errors, page = e.page_errors,
        icons = serde_json::to_string(&e.icon_issues).unwrap_or_default(),
        overflow = e.overflow, ok = e.state_ok,
        err = if e.state_error.is_empty() { String::new() } else { format!("(setup error: {})", e.state_error) },
    )
}
```

- [ ] **Step 4: GREEN + clippy + fmt.** **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/visus_review/prompt.rs
git commit -m "feat(visual-review): defect-hunting rubric + bundle prompts (serious/critical axe only); PROMPT_VERSION 2026-07-18.1" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 10: `run_bundle` — shared core, frontier budget, browser-scoped prune (Phase C)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/visus_review/mod.rs`
- Modify: `crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs`

Concrete design (audit-verified against `run()`'s body — no `todo!` placeholders):

1. **Extract the shared core** from the legacy path:
   - `fn extract_json_object(raw: &str) -> Result<&str, String>` — generalize `parse_verdict` (mod.rs:11-15, strips markdown fences); add `fn parse_defect_report(raw: &str) -> Result<DefectReport, String>` on top of it (model output arrives fenced — a bare `serde_json::from_str` fails in production even though unit tests pass).
   - `async fn review_image(png_path: &Path, model: &str, system: &str, user: &str) -> Result<(String, vision_call::Usage, u64), String>` — fs::read + `Instant` timing + `call_vision_model`, used by BOTH `review_surface` and `run_bundle` (do NOT try to reuse `review_surface` itself).
   - `fn select_review_model(cfg: &VisualReviewConfig) -> String` — extract the config/model-selection block (mod.rs:237-256).
2. **Budget/frontier semantics** (the audit's critical economics fix — `run()` is sequential and `total_review_budget_ms: 90_000` dies after ~11 entries):
   - `BundleRunArgs { bundle_dir, cache_path, report_dir, now_iso, do_ai, total_budget_ms: u64 /* default 1_800_000 local */, max_reviews: Option<usize>, browsers: Vec<String> /* default ["chromium"] for AI */ }`.
   - Priority-order the AI frontier: New/Changed first; within that, `compact` viewport, then non-default states, then chromium before firefox.
   - Entries not reviewed (budget/cap/browser-filtered) are recorded with `status: "deferred"` in the report; **the frontier resumes on rerun** via the cache. Programmatic findings are reported for ALL entries regardless.
3. **Cache**: dedicated `bundle-cache.v1.json` — separate file because each mode prunes keys absent from its own input set (sharing one file means each `--ai` run wipes the other mode's entries). **Browser-scoped pruning**: only prune a cached key when its browser (id suffix after the last `--`, ignoring a `theme-` segment) has at least one live entry in this run — a firefox key survives a chromium-only run. Persist only when `do_ai` (mirror mod.rs:367).
4. **Reports** under `report_dir`: `bundle-report.v1.json` `{ schema_version: 1, generated_at, entries: [ { id, surface, state, viewport, browser, theme, status: "reviewed"|"cached"|"deferred", score, verdict, defects, programmatic: { axe_serious_critical, axe_total, console_errors, icon_issues, overflow_px, state_ok } } ], totals }` + `bundle-digest.md` grouped by surface, severity-ordered, with a summary table.
5. **Bin**: `--bundle <dir>` branch constructs `BundleRunArgs` (`--cache` default `contracts/reports/gui-visual-review/bundle-cache.v1.json`, `--total-budget-ms`, `--max-reviews`, `--browsers` comma-list) and **skips `write_report`/`--date` entirely** (run_bundle owns its outputs); keep the eprintln summary + `std::process::exit(0)` — exit-0-always preserves CI advisory parity; loud `::warning::` when defects or deferred > 0.

- [ ] **Step 1: Failing tests** (mod.rs test modules):

```rust
    #[test]
    fn bundle_cache_key_is_the_capture_id() {
        let mut c = CacheIndex::default();
        c.entries.insert("chat--default--wide--chromium".into(), CacheEntry {
            screenshot_sha256: "aa".into(), score: 90, verdict: "pass".into(),
            model: "m".into(), reviewed_at: "t".into(),
            prompt_version: crate::visus_review::prompt::PROMPT_VERSION.into(),
        });
        let pv = crate::visus_review::prompt::PROMPT_VERSION;
        assert_eq!(decide_status(&c, "chat--default--wide--chromium", "aa", "m", pv), ReviewDecision::Cached);
        assert_eq!(decide_status(&c, "chat--default--wide--chromium", "bb", "m", pv), ReviewDecision::Changed);
        assert_eq!(decide_status(&c, "chat--default--laptop--chromium", "aa", "m", pv), ReviewDecision::New);
    }
    #[test]
    fn defect_report_parses_fenced_model_output() {
        let raw = "```json\n{\"score\": 40, \"verdict\": \"fail\", \"defects\": [{\"severity\":\"critical\",\"kind\":\"occlusion\",\"description\":\"HUD covers the composer\",\"location\":\"bottom center\"}]}\n```";
        let d = parse_defect_report(raw).unwrap();
        assert_eq!(d.defects.len(), 1);
        assert_eq!(d.defects[0].kind, "occlusion");
    }
    #[test]
    fn bundle_prune_is_browser_scoped() {
        let mut c = CacheIndex::default();
        for id in ["a--default--wide--chromium", "b--default--wide--firefox"] {
            c.entries.insert(id.into(), CacheEntry {
                screenshot_sha256: "s".into(), score: 90, verdict: "pass".into(),
                model: "m".into(), reviewed_at: "t".into(),
                prompt_version: crate::visus_review::prompt::PROMPT_VERSION.into(),
            });
        }
        // Live run contains only chromium entries, and 'a' is gone.
        let live_ids: std::collections::BTreeSet<String> = ["c--default--wide--chromium".to_string()].into();
        let live_browsers: std::collections::BTreeSet<String> = ["chromium".to_string()].into();
        prune_bundle_cache(&mut c, &live_ids, &live_browsers);
        assert!(!c.entries.contains_key("a--default--wide--chromium"), "stale chromium key pruned");
        assert!(c.entries.contains_key("b--default--wide--firefox"), "firefox key survives a chromium-only run");
    }
    #[tokio::test]
    async fn run_bundle_no_ai_writes_reports_and_leaves_cache_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let report_dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("entries-chromium-w0.jsonl"),
            "{\"id\":\"a--default--wide--chromium\",\"surface\":\"a\",\"state\":\"default\",\"viewport\":\"wide\",\"browser\":\"chromium\",\"file\":\"a.png\",\"sha256\":\"1\"}\n").unwrap();
        let cache_path = dir.path().join("bundle-cache.v1.json");
        let args = BundleRunArgs {
            bundle_dir: dir.path(), cache_path: &cache_path, report_dir: report_dir.path(),
            now_iso: "t".into(), do_ai: false, total_budget_ms: 1000, max_reviews: None,
            browsers: vec!["chromium".into()],
        };
        let report = run_bundle(&args).await;
        assert!(report_dir.path().join("bundle-report.v1.json").exists());
        assert!(report_dir.path().join("bundle-digest.md").exists());
        assert!(!cache_path.exists(), "cache persisted only when do_ai");
        assert_eq!(report.total_surfaces, 1);
    }
```

(If `RunReport`'s field names differ, use the real ones — read the struct first; the assertions' *behaviors* are the contract.)

- [ ] **Step 2: RED**, then **Step 3: Implement** per the concrete design above — `Defect`/`DefectReport` serde structs as in the audit (`severity`, `kind`, `description`, `location` with `#[serde(default)]` on location; `score`/`verdict`/`defects` defaulted), `prune_bundle_cache(cache, live_ids, live_browsers)` as its own testable fn, `run_bundle` assembling decide→review→parse→cache-insert→report exactly as specified.

- [ ] **Step 4: GREEN + full visus suite + clippy + fmt**, then no-AI end-to-end against the Task-7 bundle:

```
cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- --bundle crates/vox-gui/ui/review-bundle/latest > "$env:TEMP\bundle_dry.log" 2>&1
```

- [ ] **Step 5: AI smoke on a slice** — scratch dir with 5 entries + PNGs, `--ai --max-reviews 5`; verify verdicts in the report, cache grows; rerun → 5/5 `cached`, zero cost; delete scratch.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/visus_review/mod.rs crates/vox-orchestrator-mcp/src/bin/gui-visual-review.rs
git commit -m "feat(visual-review): frontier-resumable bundle analysis - shared vision core, priority budget, browser-scoped prune" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 11: `scripts/frontend-review.vox` (Phase C)

**Files:**
- Create: `scripts/frontend-review.vox`

Idioms verified against `gui-build.vox`/`ci-runners-up.vox`: `process.run` returns an **Option** (null on spawn failure — Windows needs the `pnpm.cmd` retry); `.unwrap()` → `{code, stdout, stderr}`; `std.env.set` is process-wide so the Playwright child inherits `VOX_REVIEW_CAPTURE` (the `run_capture_ex` env-list argument is ignored by the interpreter — eval/builtins.rs:1743-1744, so `std.env.set` is the ONLY working mechanism).

- [ ] **Step 1: Write the script**

```vox
// vox:caps subprocess env
// scripts/frontend-review.vox — one-command frontend review pipeline.
//   vox run --mode interp scripts/frontend-review.vox                 -> capture + programmatic analysis
//   VOX_REVIEW_AI=1 vox run --mode interp scripts/frontend-review.vox -> also AI defect analysis (looped until frontier empty)

fn run_pnpm(args: list[str]) to int {
    let mut proc = process.run("pnpm", args)
    if proc is null {
        proc = process.run("pnpm.cmd", args)  // Windows: pnpm is a .cmd shim (see gui-build.vox)
    }
    if proc is null {
        log.error("frontend-review: could not spawn pnpm — is it on PATH?")
        process.exit(1)
    }
    let res = proc.unwrap()
    if res.stdout != "" { print(res.stdout) }
    if res.stderr != "" { print(res.stderr) }
    return res.code
}

fn run_analysis(ai: bool) to int {
    let mut cargo_args = ["run", "-p", "vox-orchestrator-mcp", "--features", "gui-visual-review",
        "--bin", "gui-visual-review", "--", "--bundle", "crates/vox-gui/ui/review-bundle/latest"]
    if ai { cargo_args = cargo_args.push("--ai") }
    let proc = process.run("cargo", cargo_args)
    if proc is null {
        log.error("frontend-review: could not spawn cargo")
        process.exit(1)
    }
    let res = proc.unwrap()
    if res.stdout != "" { print(res.stdout) }
    if res.stderr != "" { print(res.stderr) }
    return res.code
}

fn main() {
    std.env.set("VOX_REVIEW_CAPTURE", "1")
    print("[frontend-review] capturing matrix (chromium + firefox)...")
    let code = run_pnpm(["--dir", "crates/vox-gui/ui", "exec", "playwright", "test",
        "e2e/review/capture.spec.ts", "--project=chromium", "--project=firefox-review", "--workers=4"])
    if code != 0 {
        print("[frontend-review] capture reported failures (continuing — capture is evidence, entries were still written)")
    }

    let ai = std.env.get("VOX_REVIEW_AI")
    let mut do_ai = false
    if ai.is_some() {
        if ai.unwrap() == "1" { do_ai = true }
    }
    print("[frontend-review] analyzing bundle...")
    let mut rounds = 0
    let mut analysis = run_analysis(do_ai)
    // Frontier resumability: with AI on, rerun until the analyzer reports a
    // drained frontier (deferred == 0 -> it prints DONE; bounded loop guard).
    while do_ai and analysis == 0 and rounds < 10 {
        rounds = rounds + 1
        // The analyzer exits 0 always (CI advisory parity); it prints
        // "deferred: N" — rerun while N > 0 by checking the digest freshness
        // is handled inside the binary via cache; a re-run with empty
        // frontier is a fast no-op, so a fixed small loop is safe and simple.
        analysis = run_analysis(true)
    }
    if analysis != 0 {
        log.error("frontend-review: analysis FAILED (exit " + str(analysis) + ")")
        process.exit(1)
    }
    print("[frontend-review] done — digest: contracts/reports/gui-visual-review/bundle-digest.md")
}
```

(If the analyzer later gains a `--exit-nonzero-on-deferred` flag the loop can key off it; for now a bounded re-run loop over a cache-hit-fast no-op is the simple honest version. Verify each API shape with `vox run --mode interp` before trusting; the interpreter is ground truth.)

- [ ] **Step 2: Verify staged** — first a ~1-minute scoped shakeout (temporarily add `"-g", "dashboard"` to the playwright args), then the full run: `vox run --mode interp scripts/frontend-review.vox`.

- [ ] **Step 3: Commit**

```bash
git add scripts/frontend-review.vox
git commit -m "feat(scripts): one-command frontend review pipeline (capture + frontier-looped bundle analysis)" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 12: CI switch + legacy retirement (Phase C)

**Files:**
- Modify: `.github/workflows/ci.yml` (`gui-playwright-smoke` advisory steps only)
- Modify: `.gitignore` (negation for `bundle-cache.v1.json`)
- Delete: `crates/vox-gui/ui/e2e/screenshots-variants.spec.ts` (subsumed by Task 5's `empty`/`error` states)
- Delete: `crates/vox-gui/ui/e2e/visual-review.spec.ts` + `crates/vox-gui/ui/e2e/lib/screenshotManifest.ts` (legacy manifest capture, superseded by the bundle)

- [ ] **Step 1: Locate exact steps** — grep the job for: `GUI variant states sweep (empty/error, advisory)` (ci.yml:1679-1684), `GUI visual AI review (advisory, non-gating)`, `Commit visual-review cache + report (main only)`, and the legacy visual-review capture step. Do not touch the asserting sweep, `needs:`, `if:`, or `ci-summary`.

- [ ] **Step 2: Replace the advisory block**

Remove: the variants sweep step, the legacy manifest capture step, and the legacy AI review step. Insert (after the asserting sweep + its screenshot upload):

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
        env:
          OPENROUTER_API_KEY: ${{ secrets.OPENROUTER_API_KEY }}
        run: cargo run -p vox-orchestrator-mcp --features gui-visual-review --bin gui-visual-review -- --bundle crates/vox-gui/ui/review-bundle/latest --ai --max-reviews 40
        continue-on-error: true
```

(The `-p` flag and the `OPENROUTER_API_KEY` env were both audit-critical omissions; `--max-reviews 40` bounds per-merge cost — the frontier resumes next merge.)

- [ ] **Step 3: Cache-commit + artifacts + gitignore**

- `.gitignore`: add `!/contracts/reports/gui-visual-review/bundle-cache.v1.json` alongside the existing selective negations, and seed an initial empty cache in this commit (`{"schema_version":1,"entries":{}}`) so the commit step's tracked-diff guard can fire.
- The `Commit visual-review cache + report (main only)` step: commit **only** `bundle-cache.v1.json` + `bundle-digest.md` (drop the legacy cache/report paths it committed); `bundle-report.v1.json` stays artifact-only.
- Artifact upload step's `path:` gains `crates/vox-gui/ui/review-bundle/latest/` and `contracts/reports/gui-visual-review/bundle-report.v1.json`; drop the deleted variants globs.

- [ ] **Step 4: Frontend follow-through for the deletions** — remove `screenshots-variants.spec.ts` + `visual-review.spec.ts` + `screenshotManifest.ts` and any imports/references (grep `screenshotManifest` and `VOX_VARIANT_SCREENSHOTS` across the repo — including docs and the ui `package.json`); the legacy Rust `Manifest`/`run()` path stays compiled (still unit-tested) but the bin's legacy mode is now unreachable from CI — note it as deprecated-pending-removal in `mod.rs`'s doc comment.

- [ ] **Step 5: Guard-rails** — diff touches only the `gui-playwright-smoke` job + deletions; `git diff .github/workflows/ci.yml | grep "^[+-].*needs:"` → empty; exactly two `continue-on-error: true` added, three removed with their steps; YAML parses.

- [ ] **Step 6: Run the remaining frontend suites** — `pnpm test` and `pnpm exec playwright test --project=chromium` (default suite green without the deleted specs).

- [ ] **Step 7: Commit**

```bash
git add .github/workflows/ci.yml .gitignore contracts/reports/gui-visual-review/bundle-cache.v1.json
git rm crates/vox-gui/ui/e2e/screenshots-variants.spec.ts crates/vox-gui/ui/e2e/visual-review.spec.ts crates/vox-gui/ui/e2e/lib/screenshotManifest.ts
git commit -m "ci(gui): advisory analysis switches to the bounded review bundle; retire variants sweep + legacy manifest capture" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 13: Phase D — run the pipeline, recall gate, write the review

**Files:**
- Create: `docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md`

- [ ] **Step 1: Full local run** — `$env:VOX_REVIEW_AI='1'; vox run --mode interp scripts/frontend-review.vox`; rerun until `bundle-report.v1.json` shows zero `deferred` entries. Confirm the digest + full matrix exist.
- [ ] **Step 2: Triage automated findings** — dedupe defects repeating across viewports/browsers/themes (same surface+kind+description → one finding listing affected cells); open the PNG for every critical/major and confirm visually (kill hallucinations).
- [ ] **Step 3: KNOWN-ISSUE RECALL GATE** — enumerate the user's known-real complaints: (a) the Firefox occlusion issues they observed, (b) the `__TAURI_INTERNALS__` TypeError leakage. For each: confirm the pipeline recalled it (a defect in a firefox entry's report / zero `__TAURI_INTERNALS__` matches across all entries' page_errors — structurally impossible post-Phase-A). **Any known issue not recalled is a pipeline gap: extend states/rubric/mock density and re-run before writing the review.** Record the recall table for the methodology section.
- [ ] **Step 4: Manual LLM pass** — read every compact-viewport and non-default-state capture in both browsers (minimum), tab by tab; record findings the model missed with the same fields.
- [ ] **Step 5: Tauri-shell spot check** — sidecar prerequisite per AGENTS.md, then screenshot ~6 surfaces (chat, dashboard, tasks, settings, models, approvals); diff against chromium captures for engine-specific issues.
- [ ] **Step 6: Coverage audit table (derived)** — rows from `SURFACE_REGISTRY`; columns by globbing `src/components/surfaces/**/​*.test.tsx`, `e2e/*.spec.ts`, `states.ts` keys, bundle-report presence, and `ci.yml` wiring. List surfaces with zero coverage; record excluded UI (doc-reader tabs, toasts) as explicit exclusions.
- [ ] **Step 7: Write the review doc** — executive summary; methodology (incl. recall table); ranked findings register (id, severity, kind, surface, cells affected, evidence path, remediation sketch); per-surface tab-by-tab detail; coverage table; recommended remediation order.
- [ ] **Step 8: Commit**

```bash
git add docs/superpowers/reviews/2026-07-18-axis-frontend-comprehensive-review.md
git commit -m "docs(reviews): comprehensive Axis frontend review - ranked findings + recall-validated methodology + coverage audit" -m "Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Task 14: Whole-effort verification sweep

- [ ] Frontend: `pnpm typecheck` clean; `pnpm test` green (backendGuard, transportIpcGuard, BackendBanner, reviewStates guard, audits, tauriMockRich — and the 40 pre-existing mock-based suites, proving Step 0's stub); `pnpm exec playwright test --project=chromium` green (default suite; capture spec self-skips; deleted specs gone).
- [ ] Negative guard proofs: (a) temporary raw `invoke<string>('x')` outside the marker → `transportIpcGuard` fails naming it; revert. (b) temporary registry surface key without a states entry → `reviewStates` guard fails; revert.
- [ ] Rust: `cargo test -p vox-orchestrator-mcp --features gui-visual-review > "$env:TEMP\p14.log" 2>&1` all green; clippy `-D warnings` clean.
- [ ] Live browser-mode proof: dev server + plain browser → banner in normal flow, nothing occluded, zero **uncaught** raw TypeErrors (the `no-backend` capture state automates this from now on).
- [ ] `vox run --mode interp scripts/frontend-review.vox` (no AI) end-to-end.
- [ ] Contracts: regenerate `test-inventory` + `gui-surface-coverage` with a fresh-built `./target/release/vox`; commit drift.
- [ ] Push to main per session policy (no PR; long-timeout push; pre-push hooks tolerated).

---

## Out of scope (explicitly deferred)

- Remediation of Phase D findings (separate plan; user re-prioritizes from the register).
- Visual-diff baselines; programmatic occlusion detection; tauri-driver automation; PR gating; scroll-position states; loading/skeleton states.
- Migrating the 33 direct-invoke / 7 direct-listen files onto the transport hub (tracked debt; the rejection filter covers user-visible fallout).
- Extending `vox ci test-inventory` to scan vitest/Playwright files (mechanical coverage tables) — worth its own follow-up.
