# Vox Axis GUI — IPC Resilience & Command-Parity Linting Implementation Plan

> **For agentic workers (Claude Sonnet 4.6):** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make every GUI surface fail *loudly at build/CI time and gracefully at runtime* instead of silently throwing `window.__TAURI_INTERNALS__ is undefined` / "X unavailable" when an IPC command is missing, unregistered, or the app is opened outside the Tauri webview.

**Architecture:** Four guardrails layered onto existing infra — (1) a single environment-aware `safeInvoke` that all IPC routes through, with a dev mock registry so the UI renders in a bare browser; (2) a CI parity gate that asserts every frontend `invoke('cmd')` target is registered in Rust's `generate_handler!`; (3) a per-surface React error boundary plus a smoke test that mounts every `SURFACE_REGISTRY` entry under a mocked transport; (4) wiring all three into the existing CI lanes and the `ipcBoundaries` guard.

**Tech Stack:** React 19 + TypeScript + Vite + vitest (jsdom) frontend at `crates/vox-gui/ui`; Tauri 2 Rust host at `crates/vox-gui/src`; VoxScript (`.vox`) for the CI gate (per `AGENTS.md`: no new `.ps1`/`.sh`/`.py`).

---

## Why this exists (the observed failures)

During the "Limes" restyle the user clicked through every surface in a **bare browser** (`pnpm dev` → `http://localhost:1420`, no Tauri webview) and hit three distinct, recurring error classes. They are pre-existing fragilities the restyle merely surfaced — none are styling bugs:

| # | Symptom observed | Root cause | Guardrail that catches it |
|---|---|---|---|
| 1 | `TypeError: can't access property "invoke", window.__TAURI_INTERNALS__ is undefined` → "Memory status unavailable" | Frontend `invoke()` called with no environment guard; in a browser (or before Tauri injects its IPC bridge) the bridge object is absent and every call throws raw. | **WS1** — `safeInvoke` detects non-Tauri context and returns a registered dev-mock or one typed, catchable `IpcUnavailableError` instead of an uncaught throw. |
| 2 | A surface calls a command that the Rust host never registered (typo, renamed, or never wired) → runtime rejection only discoverable by clicking | No static check that the set of frontend invoke targets ⊆ the set of `generate_handler!` commands. | **WS2** — CI parity gate fails the build on any unregistered target. |
| 3 | "Many pages not loading" — a surface throws during mount/render and blanks the route (or the whole app) | No per-surface error boundary; no smoke test that each surface renders without throwing. | **WS3** — error boundary localizes the failure to a fallback card; smoke test catches mount throws in CI. |

**Design principle the user named:** *"These are the types of errors that should have been surfaced earlier, audited, edited, and ensured for by parity by design."* Every task below moves a class of runtime failure to **compile/CI time**.

## Existing infrastructure — REUSE, do not rebuild

Read these before writing any code; the plan extends them rather than introducing parallel systems:

- `crates/vox-gui/ui/src/transport.ts` — the `VoxTransport` hub (`voxTransport` singleton) plus free functions (`ptySpawn`, `discoverySuggest`, `sendToAgent`, `getContextBudget`, etc.). **Every one imports `invoke` raw from `@tauri-apps/api/core` with no guard.** This file is the chokepoint WS1 wraps.
- `crates/vox-gui/ui/src/guards/ipcBoundaries.test.ts` — an existing static gate with two checks: infra files must not import `@tauri-apps/api/core` directly, and a shrinking `ALLOW_DIRECT_INVOKE` allowlist tracks the ~30 surface files that still call `invoke` directly. WS1 shrinks this allowlist toward `{ transport.ts }`; WS3's smoke test reuses its `collectTsFiles` walk pattern.
- `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` — `SURFACE_REGISTRY`, the SSOT array of every surface (`viewKey`, `navLabel`, `parentSurface`, `tier`). WS3 iterates this to smoke-test every surface; WS2 may cross-reference it.
- `crates/vox-gui/src/main.rs:108` — the `tauri::generate_handler![ ... ]` block (~169 commands, `commands::<module>::<fn>` form). This is the backend SSOT WS2 parses.
- `crates/vox-gui/ui/src/components/layout/` — `App.tsx` owns routing/surface switching; this is where the error boundary wraps the active surface.

## File structure

- **Create** `crates/vox-gui/ui/src/lib/ipc.ts` — `isTauri()`, `safeInvoke()`, `IpcUnavailableError`, and the dev-mock registry hook. The one module that imports `invoke`/`@tauri-apps/api/core` long-term.
- **Create** `crates/vox-gui/ui/src/lib/devMocks.ts` — `DEV_MOCKS: Record<string, () => unknown>`, browser-only stub responses keyed by command name.
- **Modify** `crates/vox-gui/ui/src/transport.ts` — replace its `import { invoke } from '@tauri-apps/api/core'` with `import { safeInvoke as invoke } from './lib/ipc'` (single-line swap; all call sites unchanged).
- **Create** `crates/vox-gui/ui/src/components/ui/SurfaceErrorBoundary.tsx` — class component rendering a fallback card on caught error.
- **Modify** `crates/vox-gui/ui/src/App.tsx` (or the surface-switch component) — wrap the active surface in `<SurfaceErrorBoundary>`.
- **Create** `crates/vox-gui/ui/src/guards/surfaceSmoke.test.tsx` — mounts every `SURFACE_REGISTRY` surface under a mocked transport.
- **Create** `crates/vox-gui/ui/src/guards/commandParity.test.ts` — fast vitest mirror of the parity gate (so local `pnpm vitest` catches drift too).
- **Create** `scripts/gui-command-parity.vox` — the CI gate (VoxScript) parsing both SSOTs and exiting non-zero on drift.
- **Modify** `crates/vox-gui/ui/src/guards/ipcBoundaries.test.ts` — add `lib/ipc.ts` to the allowlist; remove files migrated off direct `invoke` as WS1 lands.
- **Create** `docs/src/architecture/gui-ipc-resilience.md` — the durable SSOT doc (with required frontmatter per `AGENTS.md`).

---

## Workstream 1: Environment-aware `safeInvoke` + dev mock registry

### Task 1: `isTauri()` detection

**Files:**
- Create: `crates/vox-gui/ui/src/lib/ipc.ts`
- Test: `crates/vox-gui/ui/src/lib/ipc.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// @vitest-environment jsdom
import { describe, it, expect, afterEach } from 'vitest';
import { isTauri } from './ipc';

afterEach(() => {
  delete (globalThis as any).window.__TAURI_INTERNALS__;
});

describe('isTauri', () => {
  it('is false when the Tauri IPC bridge is absent', () => {
    expect(isTauri()).toBe(false);
  });
  it('is true once the bridge object is present', () => {
    (globalThis as any).window.__TAURI_INTERNALS__ = { invoke: () => {} };
    expect(isTauri()).toBe(true);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/lib/ipc.test.ts`
Expected: FAIL — `isTauri` not exported.

- [ ] **Step 3: Write minimal implementation**

```ts
// crates/vox-gui/ui/src/lib/ipc.ts
export function isTauri(): boolean {
  return typeof window !== 'undefined'
    && typeof (window as any).__TAURI_INTERNALS__ !== 'undefined';
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/lib/ipc.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/ipc.ts crates/vox-gui/ui/src/lib/ipc.test.ts
git commit -m "feat(vox-gui): add isTauri() environment detection"
```

### Task 2: `safeInvoke` with typed unavailable error + dev mock fallback

**Files:**
- Modify: `crates/vox-gui/ui/src/lib/ipc.ts`
- Create: `crates/vox-gui/ui/src/lib/devMocks.ts`
- Test: `crates/vox-gui/ui/src/lib/ipc.test.ts`

- [ ] **Step 1: Write the failing test** (append to `ipc.test.ts`)

```ts
import { safeInvoke, IpcUnavailableError } from './ipc';

describe('safeInvoke (non-Tauri context)', () => {
  it('returns a registered dev mock when one exists', async () => {
    await expect(safeInvoke('get_memory_status')).resolves.toEqual(
      expect.objectContaining({ available: false }),
    );
  });
  it('rejects with IpcUnavailableError naming the command when no mock exists', async () => {
    await expect(safeInvoke('totally_unknown_cmd')).rejects.toBeInstanceOf(IpcUnavailableError);
    await expect(safeInvoke('totally_unknown_cmd')).rejects.toThrow(/totally_unknown_cmd/);
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm vitest run src/lib/ipc.test.ts`
Expected: FAIL — `safeInvoke`/`IpcUnavailableError` not exported.

- [ ] **Step 3: Write minimal implementation**

```ts
// crates/vox-gui/ui/src/lib/devMocks.ts
/**
 * Browser-only stub responses keyed by Tauri command name. Used ONLY when the
 * app runs outside the Tauri webview (e.g. `pnpm dev` in a browser, design
 * review). Keep shapes aligned with the real DTOs in src/types/tauri.ts.
 */
export const DEV_MOCKS: Record<string, () => unknown> = {
  get_memory_status: () => ({ available: false, note: 'dev-mock (no Tauri bridge)' }),
  get_identity_summary: () => ({ display_name: 'operator@vox (dev)' }),
  get_orchestrator_status: () => ({ agent_count: 0, tasks: [] }),
  // Extend as surfaces are previewed in-browser. A missing key is not an error
  // here — it surfaces as a single catchable IpcUnavailableError.
};
```

```ts
// append to crates/vox-gui/ui/src/lib/ipc.ts
import { invoke } from '@tauri-apps/api/core';
import { DEV_MOCKS } from './devMocks';

export class IpcUnavailableError extends Error {
  constructor(public readonly command: string) {
    super(`IPC command "${command}" is unavailable: not running inside the Tauri webview and no dev mock is registered.`);
    this.name = 'IpcUnavailableError';
  }
}

export async function safeInvoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (isTauri()) {
    return invoke<T>(command, args);
  }
  const mock = DEV_MOCKS[command];
  if (mock) return mock() as T;
  throw new IpcUnavailableError(command);
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `pnpm vitest run src/lib/ipc.test.ts`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/ipc.ts crates/vox-gui/ui/src/lib/devMocks.ts crates/vox-gui/ui/src/lib/ipc.test.ts
git commit -m "feat(vox-gui): safeInvoke with dev-mock fallback + typed IpcUnavailableError"
```

### Task 3: Route the transport hub through `safeInvoke`

**Files:**
- Modify: `crates/vox-gui/ui/src/transport.ts:1`
- Modify: `crates/vox-gui/ui/src/guards/ipcBoundaries.test.ts`

- [ ] **Step 1: Swap the import in `transport.ts`**

Replace line 1:
```ts
import { invoke } from '@tauri-apps/api/core';
```
with:
```ts
import { safeInvoke as invoke } from './lib/ipc';
```
All ~40 `invoke(...)` call sites in the file are unchanged — they now go through the guard. (`listen` still comes from `@tauri-apps/api/event`; leave that import as-is — event listeners already reject gracefully outside Tauri per the existing doc comments.)

- [ ] **Step 2: Update the boundary guard allowlist**

In `ipcBoundaries.test.ts`, add `'lib/ipc.ts'` to `ALLOW_DIRECT_INVOKE` and remove `'transport.ts'` from it (transport no longer imports from `@tauri-apps/api/core`).

- [ ] **Step 3: Run the guard + transport tests**

Run: `pnpm vitest run src/guards/ipcBoundaries.test.ts src/transport.test.ts`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/guards/ipcBoundaries.test.ts
git commit -m "refactor(vox-gui): route transport hub through safeInvoke guard"
```

### Task 4 (iterative): migrate surface files off direct `invoke`

For each file in the `ALLOW_DIRECT_INVOKE` list, replace its `import { invoke } from '@tauri-apps/api/core'` with `import { safeInvoke as invoke } from '../../lib/ipc'` (adjust relative depth), remove it from the allowlist, and run that surface's test. Do this in small batches (3–5 files per commit). The end state: `ALLOW_DIRECT_INVOKE` contains only `lib/ipc.ts`. Each batch:

- [ ] Swap imports in the batch.
- [ ] Remove those entries from `ALLOW_DIRECT_INVOKE`.
- [ ] Run: `pnpm vitest run src/guards/ipcBoundaries.test.ts <each surface's .test.tsx>` → PASS.
- [ ] Commit: `refactor(vox-gui): route <surfaces> through safeInvoke (wave N)`.

---

## Workstream 2: Frontend ↔ backend command-parity gate

### Task 5: Parity check as a vitest guard (fast local signal)

**Files:**
- Create: `crates/vox-gui/ui/src/guards/commandParity.test.ts`

- [ ] **Step 1: Write the test** (it both defines and enforces the invariant)

```ts
import { describe, it, expect } from 'vitest';
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const SRC_ROOT = join(import.meta.dirname, '..');
const MAIN_RS = join(SRC_ROOT, '..', '..', 'src', 'main.rs');

/** Every `invoke('cmd'` / `invoke<T>('cmd'` / `safeInvoke('cmd'` string literal in src. */
function frontendCommands(): Set<string> {
  const re = /(?:safeInvoke|invoke)\s*(?:<[^>]*>)?\s*\(\s*['"]([a-z0-9_]+)['"]/g;
  const cmds = new Set<string>();
  const walk = (dir: string) => {
    for (const e of readdirSync(dir)) {
      const full = join(dir, e);
      if (statSync(full).isDirectory()) { if (e !== 'guards') walk(full); continue; }
      if (!/\.(ts|tsx)$/.test(e) || e.includes('.test.')) continue;
      const text = readFileSync(full, 'utf8');
      for (const m of text.matchAll(re)) cmds.add(m[1]);
    }
  };
  walk(SRC_ROOT);
  return cmds;
}

/** Every leaf ident registered in `tauri::generate_handler![ ... ]`. */
function backendCommands(): Set<string> {
  const text = readFileSync(MAIN_RS, 'utf8');
  const block = text.slice(text.indexOf('generate_handler!'));
  const re = /commands::[a-z0-9_]+::([a-z0-9_]+)/g;
  const cmds = new Set<string>();
  for (const m of block.matchAll(re)) cmds.add(m[1]);
  return cmds;
}

describe('IPC command parity', () => {
  it('every frontend invoke target is registered in generate_handler!', () => {
    // invoke_mcp_tool is a generic dispatcher; tool names passed to it are NOT
    // Tauri commands and are intentionally excluded from this check.
    const backend = backendCommands();
    const missing = [...frontendCommands()].filter(c => !backend.has(c));
    expect(missing).toEqual([]);
  });
});
```

- [ ] **Step 2: Run it to discover the real drift**

Run: `pnpm vitest run src/guards/commandParity.test.ts`
Expected: It will likely FAIL initially, listing the actual unregistered commands the user hit. **Triage each:** either (a) wire the missing command into `main.rs`'s `generate_handler!` (if the backend fn exists), or (b) fix the frontend typo, or (c) if it is a legitimately-dynamic name (rare), add a narrowly-scoped, commented exception set in the test. Do NOT broaden the regex to make it pass.

- [ ] **Step 3: Once triaged, the test passes** — commit each backend wiring fix separately from the gate.

```bash
git add crates/vox-gui/ui/src/guards/commandParity.test.ts
git commit -m "test(vox-gui): enforce frontend<-backend IPC command parity"
```

### Task 6: Promote parity to a CI gate (VoxScript)

**Files:**
- Create: `scripts/gui-command-parity.vox`

- [ ] **Step 1: Author the gate** in VoxScript mirroring Task 5's two extractors (read `crates/vox-gui/ui/src` recursively for invoke literals, read `crates/vox-gui/src/main.rs` for the handler block), printing each missing command and exiting non-zero if any. Follow an existing `.vox` CI gate as the structural template (`grep -rl "vox ci" scripts/*.vox` to find one; reuse its arg-parsing and exit-code idiom). Per `crate-build-audit.vox` lessons in repo memory: run with `--mode interp`, single-line fn sigs, no multi-line `+` exprs.

- [ ] **Step 2: Run it locally**

Run: `vox run scripts/gui-command-parity.vox --mode interp`
Expected: exit 0 once Task 5 triage is complete.

- [ ] **Step 3: Wire it into the CI lane** next to the other GUI gates (find the lane that runs `arch-check`/`ssot-drift` and add this gate after it; keep it non-zero-on-drift, not advisory). Commit.

---

## Workstream 3: Per-surface error boundary + smoke test

### Task 7: `SurfaceErrorBoundary`

**Files:**
- Create: `crates/vox-gui/ui/src/components/ui/SurfaceErrorBoundary.tsx`
- Test: `crates/vox-gui/ui/src/components/ui/SurfaceErrorBoundary.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { SurfaceErrorBoundary } from './SurfaceErrorBoundary';

function Boom(): never { throw new Error('surface exploded'); }

describe('SurfaceErrorBoundary', () => {
  it('renders a fallback card instead of crashing the tree', () => {
    vi.spyOn(console, 'error').mockImplementation(() => {});
    render(<SurfaceErrorBoundary surface="memory"><Boom /></SurfaceErrorBoundary>);
    expect(screen.getByRole('alert')).toHaveTextContent(/memory/i);
    expect(screen.getByRole('alert')).toHaveTextContent(/surface exploded/i);
  });
});
```

- [ ] **Step 2: Run → FAIL** (`pnpm vitest run src/components/ui/SurfaceErrorBoundary.test.tsx`)

- [ ] **Step 3: Implement** (use restyle tokens — `bg-bg-surface`, `text-status-fail`, `border-border-subtle`, `.vox-display`)

```tsx
import React from 'react';

interface Props { surface: string; children: React.ReactNode; }
interface State { error: Error | null; }

export class SurfaceErrorBoundary extends React.Component<Props, State> {
  state: State = { error: null };
  static getDerivedStateFromError(error: Error): State { return { error }; }
  componentDidUpdate(prev: Props) {
    if (prev.surface !== this.props.surface && this.state.error) this.setState({ error: null });
  }
  render() {
    if (this.state.error) {
      return (
        <div role="alert" className="m-4 rounded-xl border border-border-subtle bg-bg-surface p-4">
          <div className="vox-display text-[11px] text-[var(--color-status-fail)]">
            {this.props.surface} — surface error
          </div>
          <pre className="mt-2 whitespace-pre-wrap break-words font-mono text-[11px] text-text-muted">
            {this.state.error.message}
          </pre>
        </div>
      );
    }
    return this.props.children;
  }
}
```

- [ ] **Step 4: Run → PASS.**
- [ ] **Step 5: Commit** `feat(vox-gui): per-surface error boundary with token-styled fallback`.

### Task 8: Wrap the active surface in `App.tsx`

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1:** Locate where the active surface is rendered from the view switch. Wrap it: `<SurfaceErrorBoundary surface={view}>{renderedSurface}</SurfaceErrorBoundary>`. The `componentDidUpdate` reset (keyed on `surface`/`view`) clears the error when the user navigates away, so a broken surface never wedges navigation.
- [ ] **Step 2:** Run `pnpm vitest run src/App.test.tsx` → PASS.
- [ ] **Step 3:** Commit `feat(vox-gui): wrap active surface in error boundary`.

### Task 9: Smoke-test every surface from `SURFACE_REGISTRY`

**Files:**
- Create: `crates/vox-gui/ui/src/guards/surfaceSmoke.test.tsx`

- [ ] **Step 1: Write the smoke test.** Mock `./lib/ipc` so `safeInvoke` resolves to benign empty values (`{}` / `[]`) for all commands, mock `@tauri-apps/api/event` `listen` to a no-op unlisten, then dynamically import and shallow-render each surface component mapped from `SURFACE_REGISTRY`. Assert no throw. Sketch:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render } from '@testing-library/react';
import { SURFACE_REGISTRY } from '../generated/surfaceRegistry.generated';
import { surfaceComponentFor } from '../lib/navigation'; // or the App-level switch map

vi.mock('../lib/ipc', () => ({
  isTauri: () => false,
  IpcUnavailableError: class extends Error {},
  safeInvoke: vi.fn().mockResolvedValue({}),
}));
vi.mock('@tauri-apps/api/event', () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));

describe('surface smoke', () => {
  for (const entry of SURFACE_REGISTRY) {
    it(`mounts ${entry.viewKey} without throwing`, () => {
      const Comp = surfaceComponentFor(entry.viewKey);
      if (!Comp) return; // registry entries without a direct component (redirects) skip
      expect(() => render(<Comp />)).not.toThrow();
    });
  }
});
```

If no `surfaceComponentFor` map exists, extract the view→component mapping from `App.tsx`'s switch into a small exported `lib/surfaceComponents.ts` map first (its own commit), so both `App.tsx` and this test consume one SSOT — this is the parity-by-design move: the registry and the renderer can no longer drift.

- [ ] **Step 2: Run → triage.** Each surface that throws on mount is a real "page not loading" bug; fix it (usually an unguarded destructure of an awaited IPC result — give it a default). Fix surfaces in separate commits from the gate.
- [ ] **Step 3:** Once green, commit `test(vox-gui): smoke-render every registered surface under mocked IPC`.

---

## Workstream 4: Documentation & CI wiring

### Task 10: Durable SSOT doc

**Files:**
- Create: `docs/src/architecture/gui-ipc-resilience.md`

- [ ] **Step 1:** Write the doc with required frontmatter (per `AGENTS.md`; include `category: "Architecture SSOTs"` per repo convention). Cover: the three error classes, the `safeInvoke` contract, how to add a dev mock, the parity gate and how to clear a failure (wire vs. fix typo), and the smoke test. Link it from any GUI architecture index.
- [ ] **Step 2:** Commit `docs(vox-gui): IPC resilience & command-parity SSOT`.

### Task 11: Final full-suite verification

- [ ] Run `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run` → all green.
- [ ] Run `vox run scripts/gui-command-parity.vox --mode interp` → exit 0.
- [ ] Confirm `ALLOW_DIRECT_INVOKE` in `ipcBoundaries.test.ts` is down to `{ 'lib/ipc.ts' }`.
- [ ] Open `pnpm dev` in a plain browser and confirm surfaces render with dev mocks / localized fallback cards instead of `__TAURI_INTERNALS__` crashes.

---

## Self-review checklist (run before handoff is considered done)

1. **Spec coverage:** All three observed error classes have a guardrail (WS1 runtime + WS2 CI + WS3 mount). ✓
2. **Placeholder scan:** No "TBD"/"add error handling" — every code step shows code. Task 6's `.vox` is described against a named template to copy rather than invented from whole cloth (VoxScript syntax is repo-specific; the worker copies an existing gate). ✓
3. **Type consistency:** `safeInvoke<T>(command, args?)` signature matches Tauri's `invoke<T>(command, args?)` so the `import { safeInvoke as invoke }` alias is drop-in. `SurfaceErrorBoundary` prop is `surface` in both definition and call site. ✓
4. **No new banned automation:** the CI gate is `.vox`, not `.ps1`/`.sh`/`.py` (AGENTS.md). ✓

## Execution notes for the worker

- **Order matters:** WS1 Tasks 1–3 first (they unblock everything). WS2 Task 5 and WS3 Task 9 each *discover* the real backlog of unregistered commands / broken surfaces — expect those test runs to fail first and treat each failure as a tracked fix, not a reason to weaken the gate.
- **Windows/pnpm:** tests need `// @vitest-environment jsdom` as the first line (no global vitest config). Run from `crates/vox-gui/ui`.
- **Do not** pipe cargo/pnpm output through `head`/`grep` on Windows (repo memory: orphan-process leak) — redirect to a file or use `tail` via the Bash tool's own truncation.
