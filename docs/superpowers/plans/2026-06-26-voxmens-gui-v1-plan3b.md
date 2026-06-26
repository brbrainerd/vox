---
category: "Architecture SSOTs"
title: "VoxMens / Populi GUI v1 (Plan 3B) — monitor + light actions, zero new Rust"
date: 2026-06-26
status: plan
---

# VoxMens / Populi GUI v1 (Plan 3B)

Implementation plan for v1 of the `mens` ("Model Lab") and `populi` ("Mesh") GUI
surfaces. Executes the ratified scope of the design spec
[`2026-06-26-voxmens-gui-cli-parity-design.md`](../specs/2026-06-26-voxmens-gui-cli-parity-design.md).

## Ratified scope (read before starting)

- **v1 = MONITOR + LIGHT ACTIONS. ZERO new Rust.** Every control rides the
  existing `execute_command` Tauri seam
  (`crates/vox-gui/src/commands/execute.rs`). v1 expands `mens`/`populi` from the
  current 3+2 arg-free read cards to **full read coverage** of the CLI plus
  **safe fire-and-forget actions** (corpus ops, probe, node list, init).
- **DEFERRED to v2 (do NOT build now):** the 4–5 streaming Tauri wrappers that
  *launch* long jobs (`mens train`/`serve`/`pipeline`, `populi up`/`serve`) and
  the cost/spend UI. See the fenced v2 section at the end.
- **Cost/cloud:** v1 is monitor-only so no spend happens. Every train/serve form
  is **local-only** in v1 — the `--cloud`/`--max-budget` flags are NOT surfaced.
  (v2 principle recorded below.)
- **Keys/identity:** managed centrally in the GUI's existing Settings/Secrets
  (Clavis) area. `populi identity export` (private key) and key handling route to
  central secrets management — **no separate key UI** in mens/populi. Cross-ref
  Plan 3C.
- **Destructive ops:** `populi admin` (maintenance/quarantine/exec-lease-revoke)
  surface behind an explicit in-UI confirm.
- **De-Latinized labels:** mens surface title = "Model Lab"; populi surface title
  = "Mesh". Internal surface keys stay `mens`/`populi` (registry SSOT unchanged).
- **`models` stays distinct.** Do NOT merge into `ModelsView`. Recommend (as a
  follow-up, not v1 work) a single tagged `models` registry with provenance tags.

## Grounding (verified in repo, branch `claude/graphify-general-gui-ia`)

- `execute_command(path: Vec<String>, args: Value)` shells the `vox` sidecar.
  Arg encoding (verified in `execute.rs`): `__argv` = raw tokens; `__positionals`
  = positional values; `__flags` = bare `--flag`; any other key `k: v` becomes
  `--k-with-dashes v` (underscores → dashes), `Bool(true)` = bare flag, arrays
  repeat the flag. **We will use `__argv` for every command** to keep arg order
  explicit and avoid the key-mangling path.
- `CommandCardsView` (`surfaces/CommandCardsView.tsx`) only runs **arg-free**
  reads (`args: { __argv: [] }`). It cannot pass `--quotas`/`--json` or fire
  actions. v1 therefore replaces the two `commandSurface(...)` decorator entries
  with dedicated `MensView`/`PopuliView` decorators that call `execute_command`
  directly with explicit `__argv`.
- Decorator seam: `surfaces/decoratorRegistry.ts` → `surfaceDecorators[key]`,
  consumed by `layout/surfaceComponents.tsx:78`. Registering a view is a
  one-line change; removing it reverts to the default. Decorators receive
  `{ pushToast, gamifyEnabled }` (`SurfaceDecoratorProps`).
- Live-surface pattern to copy: `surfaces/Models/ModelsView.tsx` (invoke in
  `useCallback`, `useEffect` + `setInterval` refresh, `pushToast` on failure,
  `type="button"`, `role="list"/"listitem"`, aria labels).
- Honesty guard: `surfaces/__guards__/surfaceHonesty.guard.test.ts` walks all
  shipped `*.tsx` (excludes `*.test.tsx`/`*.unfinished.tsx`) and forbids
  placeholder prose and `onClick={() => {}}` dead handlers
  (`__guards__/honestyScan.ts`). Every control we add must call a real
  `execute_command` path — no empty handlers, no "coming soon".
- Test runner (verified `crates/vox-gui/ui/package.json`): `pnpm test`
  (= `vitest run`); single file: `pnpm exec vitest run <path>`; typecheck:
  `pnpm typecheck`.

### Verified CLI command/arg names (source of truth)

`mens` (`crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs`):
- `mens probe [-d|--detailed]`
- `mens status [--quotas] [--config] [--cloud] [--db]`
- `mens models`
- `mens watch-telemetry [--telemetry <path>] [--err-log <path>] [--interval-ms <n>]`
- `mens eval-local --model <path> [--bench <path>] [--samples <k>] [-o <out>]`
- `mens train …` (gpu feature; long job — v2)
- `mens serve …` (gpu feature; long job — v2)
- `mens pipeline …` (long job — v2)

`mens corpus` (`crates/vox-ml-cli/src/commands/corpus/mod.rs`, enum `CorpusAction`):
- `mens corpus stats [-i|--input <jsonl>]` (default `target/dogfood/train.jsonl`)
- `mens corpus readiness --spoke <name> [--input <jsonl>] [--min-rows <n>] [--min-diversity <f>] [--output <json>]`
- `mens corpus eval <input> [-o <out>] [--print-summary]` (input positional, required)
- `mens corpus mix [--config <yaml>] [--allow-missing-sources]`
- `mens corpus validate-batch -i <input> [-o <out>] [--no-recheck] …` (alias `validate`)
- `mens corpus fingerprint`

`populi` (`crates/vox-ml-cli/src/commands/populi_cli.rs`, enum `PopuliCli`):
- `populi status [--json]`
- `populi stats [--json] [--control-url <url>]`
- `populi registry-snapshot [--json] [--registry <path>]` (alias `local-status`)
- `populi config show` / `populi config check`
- `populi node list [--control-url <url>]`
- `populi federation list [--json] [--control-url <url>]`
- `populi init [--force]`
- `populi identity show` / `reputation` (read; `export` routes to Settings/Secrets — Plan 3C)
- `populi admin maintenance --node <id> --state {on|off} …` (confirm-gated)
- `populi admin quarantine --node <id> --state {on|off}` (confirm-gated)
- `populi admin exec-lease-revoke --lease-id <id>` (confirm-gated)
- `populi up`/`down`/`serve`/`dispatch` (long job / control plane — v2)

## Conventions for every step

- **TDD:** write the failing test first, run it, see it fail for the stated
  reason, then write the minimal code to pass, then run the full `pnpm test`.
- **Commit after every green step** with the message shown. Do NOT `git commit`
  for steps that say "no commit". The human commits the final plan separately;
  these commits are the *implementation* commits during execution.
- All new files under `crates/vox-gui/ui/src/components/surfaces/{Mens,Populi}/`.
- All `invoke` calls go through `@tauri-apps/api/core`'s `invoke('execute_command', { path, args: { __argv: [...] } })`.
- Mock `@tauri-apps/api/core` in tests exactly like `Models/ModelsView.test.tsx`.
- Every `<button>` carries `type="button"`. Lists use `role="list"/"listitem"`.
- Run commands from `crates/vox-gui/ui/`.

---

## Phase 0 — shared exec helper (no new Rust)

A tiny typed wrapper so every Mens/Populi panel calls the sidecar identically and
tests have one seam to mock.

### Step 0.1 — exec helper test (failing)

Create `crates/vox-gui/ui/src/components/surfaces/lib/runVoxCommand.test.ts`:

```ts
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';

const invokeMock = vi.fn();
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { runVoxCommand } from './runVoxCommand';

describe('runVoxCommand', () => {
  beforeEach(() => invokeMock.mockReset());

  it('passes path and __argv verbatim to execute_command', async () => {
    invokeMock.mockResolvedValue({ exit_code: 0, stdout: 'ok', stderr: '' });
    const out = await runVoxCommand(['mens', 'status'], ['--quotas']);
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['mens', 'status'],
      args: { __argv: ['--quotas'] },
    });
    expect(out.exit_code).toBe(0);
    expect(out.stdout).toBe('ok');
  });

  it('defaults argv to empty array', async () => {
    invokeMock.mockResolvedValue({ exit_code: 0, stdout: '', stderr: '' });
    await runVoxCommand(['populi', 'status']);
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['populi', 'status'],
      args: { __argv: [] },
    });
  });
});
```

Run: `pnpm exec vitest run src/components/surfaces/lib/runVoxCommand.test.ts`
(expect failure: module not found).

### Step 0.2 — exec helper (make green)

Create `crates/vox-gui/ui/src/components/surfaces/lib/runVoxCommand.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';

export interface ExecuteOutput {
  exit_code: number;
  stdout: string;
  stderr: string;
}

/** Run a read-only or fire-and-forget vox CLI command via the shared sidecar seam. */
export async function runVoxCommand(
  path: string[],
  argv: string[] = []
): Promise<ExecuteOutput> {
  return invoke<ExecuteOutput>('execute_command', { path, args: { __argv: argv } });
}
```

Run the file test (green), then `pnpm test` (full suite green).

**Commit:** `feat(gui): add runVoxCommand exec helper for mens/populi v1`

---

## Phase 1 — Mens ("Model Lab") read coverage

A dedicated `MensView` decorator with read panels for status, models, probe,
corpus stats/readiness, and eval. Replaces the `mens` `commandSurface(...)` entry.

### Step 1.1 — MensView skeleton + heading test (failing)

Create `crates/vox-gui/ui/src/components/surfaces/Mens/MensView.test.tsx`
(mirror `Models/ModelsView.test.tsx`'s mock of `@tauri-apps/api/core`, but mock
`execute_command` to return `{ exit_code: 0, stdout: '<panel>', stderr: '' }`):

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn((_cmd: string, args?: any) => {
  const path = args?.path?.join(' ');
  return Promise.resolve({ exit_code: 0, stdout: `out:${path}`, stderr: '' });
});
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { MensView } from './MensView';

describe('MensView', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); });

  it('renders the Model Lab heading', () => {
    render(<MensView pushToast={vi.fn()} />);
    expect(screen.getByText('Model Lab')).toBeTruthy();
  });

  it('runs mens status on mount via execute_command', async () => {
    render(<MensView pushToast={vi.fn()} />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'status'],
        args: { __argv: [] },
      })
    );
  });

  it('every button carries type="button"', async () => {
    render(<MensView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('button').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) expect(b.getAttribute('type')).toBe('button');
  });
});
```

Run the file test (expect failure: module not found).

### Step 1.2 — MensView read panels (make green)

Create `crates/vox-gui/ui/src/components/surfaces/Mens/MensView.tsx`. Use
`SurfaceDecoratorProps` from `../decoratorRegistry`, `Glass` from `../../ui/Glass`,
and `runVoxCommand`. Render a panel grid. Each panel:
- has a title + a `<pre>` output region (copy the styling from `CommandCardsView`);
- runs its command on mount and on a per-panel **Refresh** `type="button"`;
- on `exit_code !== 0` or thrown error, `pushToast({ tone: 'warn', ... })`.

Panels (each row marked **exec** = rides `execute_command`):

| Panel | path | argv | Wire |
|---|---|---|---|
| Training Status | `['mens','status']` | `[]` | exec |
| Quotas | `['mens','status']` | `['--quotas']` | exec |
| Cloud Dispatch Summary | `['mens','status']` | `['--cloud']` | exec |
| Intelligence Metrics | `['mens','status']` | `['--db']` | exec |
| Trained Models | `['mens','models']` | `[]` | exec |
| GPU Probe | `['mens','probe']` | `['--detailed']` | exec |
| Corpus Stats | `['mens','corpus','stats']` | `[]` | exec |

Recommend (do not require) a deep-link note under Trained Models pointing at the
`models` surface (plain text, no dead handler).

Run the file test (green), then `pnpm test`.

**Commit:** `feat(gui): MensView read panels (status/quotas/cloud/db/models/probe/corpus)`

### Step 1.3 — Corpus Readiness panel (spoke-aware) — test first (failing)

Add to `MensView.test.tsx`:

```tsx
it('runs corpus readiness with the selected spoke', async () => {
  const { getByLabelText, getByText } = render(<MensView pushToast={vi.fn()} />);
  // spoke selector defaults to vox-lang; clicking Check fires readiness
  await waitFor(() => getByText('Corpus Readiness'));
  getByText('Check Readiness').click();
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['mens', 'corpus', 'readiness'],
      args: { __argv: ['--spoke', 'vox-lang'] },
    })
  );
});
```

Run (expect failure: no Readiness panel / control).

### Step 1.4 — Corpus Readiness panel (make green)

Add a "Corpus Readiness" panel to `MensView.tsx`:
- a `<select aria-label="Training spoke">` with the 5 spokes from the spec:
  `vox-lang`, `rust-expert`, `agents`, `tool-selection`, `argument-generation`
  (default `vox-lang`);
- a **Check Readiness** `type="button"` that runs
  `runVoxCommand(['mens','corpus','readiness'], ['--spoke', spoke])` and renders
  the output `<pre>`.

Wire = **exec**. Run file test (green), then `pnpm test`.

**Commit:** `feat(gui): MensView corpus-readiness panel (spoke selector)`

### Step 1.5 — Corpus Eval panel (input-driven) — test first (failing)

Add to `MensView.test.tsx`:

```tsx
it('runs corpus eval with the entered input path', async () => {
  render(<MensView pushToast={vi.fn()} />);
  const input = await screen.findByLabelText('Eval corpus JSONL path');
  (input as HTMLInputElement).value = 'target/dogfood/train.jsonl';
  input.dispatchEvent(new Event('input', { bubbles: true }));
  screen.getByText('Run Eval').click();
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['mens', 'corpus', 'eval'],
      args: { __argv: ['target/dogfood/train.jsonl', '--print-summary'] },
    })
  );
});
```

(Use a controlled React input in the component; in the test, set value via the
React-friendly fireEvent if the raw event proves flaky — prefer
`fireEvent.change` from `@testing-library/react`.)

Run (expect failure).

### Step 1.6 — Corpus Eval panel (make green)

Add a "Corpus Eval" panel:
- a controlled `<input aria-label="Eval corpus JSONL path">` (default
  `target/dogfood/train.jsonl`);
- a **Run Eval** `type="button"`. Build argv as
  `[inputPath.trim(), '--print-summary']` and only fire when the path is
  non-empty (disable the button otherwise — no dead handler).

`mens corpus eval` takes the input as a **positional** (verified: `input:
PathBuf` `#[arg(required = true)]`), so it is the first `__argv` token, NOT a
`--input` flag. Wire = **exec**. Run file test (green), `pnpm test`.

**Commit:** `feat(gui): MensView corpus-eval panel (positional input + summary)`

### Step 1.7 — Register MensView, retire the command-card entry — test first (failing)

Create `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { surfaceDecorators } from './decoratorRegistry';
import { MensView } from './Mens/MensView';

describe('decoratorRegistry mens/populi v1', () => {
  it('maps mens to MensView', () => {
    expect(surfaceDecorators.mens).toBe(MensView);
  });
});
```

Run (expect failure: still the `commandSurface` closure).

### Step 1.8 — Register MensView (make green)

In `decoratorRegistry.ts`:
- add `import { MensView } from './Mens/MensView';`
- replace the `mens: commandSurface('Vox Mens', …, [...])` entry with
  `mens: MensView,`.

Run the registry test (green), then `pnpm test`. Confirm the honesty guard still
passes (MensView must contain no placeholder prose and no empty handlers).

**Commit:** `feat(gui): register MensView decorator; retire mens command-cards`

---

## Phase 2 — Mens safe actions (fire-and-forget corpus ops)

Light, non-destructive build actions. No spend, no long-lived process. Each is an
`exec` call whose output the panel renders; failures toast. These run quickly and
write local JSONL files.

### Step 2.1 — Corpus build actions — test first (failing)

Add to `MensView.test.tsx`:

```tsx
it('fires corpus fingerprint as a safe action', async () => {
  render(<MensView pushToast={vi.fn()} />);
  (await screen.findByText('Fingerprint')).click();
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['mens', 'corpus', 'fingerprint'],
      args: { __argv: [] },
    })
  );
});

it('fires corpus mix as a safe action', async () => {
  render(<MensView pushToast={vi.fn()} />);
  (await screen.findByText('Mix Corpus')).click();
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['mens', 'corpus', 'mix'],
      args: { __argv: [] },
    })
  );
});
```

Run (expect failure).

### Step 2.2 — Corpus build actions (make green)

Add a "Build Corpus" panel with three `type="button"` actions, each calling
`runVoxCommand` and rendering output (mark each row **exec**):

| Action button | path | argv | Wire |
|---|---|---|---|
| Fingerprint | `['mens','corpus','fingerprint']` | `[]` | exec |
| Mix Corpus | `['mens','corpus','mix']` | `[]` (uses default `mens/config/mix.yaml`) | exec |
| Validate (dry recheck off) | `['mens','corpus','validate-batch']` | `['-i', inputPath]` (reuse the eval input field; disable when empty) | exec |

Show a one-line "writes local JSONL — no cloud spend" caption (plain text).
On success toast `{ tone: 'ok' }`; on non-zero exit toast `{ tone: 'warn' }`.

Run file test (green), `pnpm test`.

**Commit:** `feat(gui): MensView safe corpus actions (fingerprint/mix/validate)`

---

## Phase 3 — Populi ("Mesh") read coverage

Dedicated `PopuliView` decorator: mesh health, stats, registry snapshot, config,
nodes, federation, identity (read). Replaces the `populi` `commandSurface(...)`.

### Step 3.1 — PopuliView skeleton + heading test (failing)

Create `crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.test.tsx`
(same invoke mock shape as `MensView.test.tsx`):

```tsx
import { MensView } from '../Mens/MensView'; // not used; placeholder import removed in real file
```

Replace the above with the real test:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn((_cmd: string, args?: any) =>
  Promise.resolve({ exit_code: 0, stdout: `out:${args?.path?.join(' ')}`, stderr: '' })
);
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { PopuliView } from './PopuliView';

describe('PopuliView', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); });

  it('renders the Mesh heading', () => {
    render(<PopuliView pushToast={vi.fn()} />);
    expect(screen.getByText('Mesh')).toBeTruthy();
  });

  it('runs populi status --json on mount', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['populi', 'status'],
        args: { __argv: ['--json'] },
      })
    );
  });

  it('every button carries type="button"', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('button').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) expect(b.getAttribute('type')).toBe('button');
  });
});
```

Run (expect failure: module not found).

### Step 3.2 — PopuliView read panels (make green)

Create `crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.tsx` modeled
on `MensView.tsx`. Panels (each **exec**):

| Panel | path | argv | Wire |
|---|---|---|---|
| Mesh Health | `['populi','status']` | `['--json']` | exec |
| Queue Stats | `['populi','stats']` | `['--json']` | exec |
| Local Snapshot | `['populi','registry-snapshot']` | `['--json']` | exec |
| Config (resolved) | `['populi','config','show']` | `[]` | exec |
| Config Check | `['populi','config','check']` | `[]` | exec |
| Nodes | `['populi','node','list']` | `[]` | exec |
| Federation | `['populi','federation','list']` | `['--json']` | exec |
| Identity (public) | `['populi','identity','show']` | `[]` | exec |
| Reputation | `['populi','identity','reputation']` | `[]` | exec |

Note (plain text under Identity): "Private-key backup is managed in Settings →
Secrets" — do NOT add an export button here (Plan 3C). Stats/federation/nodes may
exit non-zero when no control plane is running; treat non-zero as an informational
`{ tone: 'warn' }` toast, not an error crash.

Run file test (green), `pnpm test`.

**Commit:** `feat(gui): PopuliView read panels (status/stats/registry/config/nodes/federation/identity)`

### Step 3.3 — Register PopuliView — test first (failing)

Add to `decoratorRegistry.test.ts`:

```ts
import { PopuliView } from './Populi/PopuliView';
it('maps populi to PopuliView', () => {
  expect(surfaceDecorators.populi).toBe(PopuliView);
});
```

Run (expect failure).

### Step 3.4 — Register PopuliView (make green)

In `decoratorRegistry.ts`: import `PopuliView`, replace the
`populi: commandSurface(...)` entry with `populi: PopuliView,`.

Run registry test (green), `pnpm test`, honesty guard green.

**Commit:** `feat(gui): register PopuliView decorator; retire populi command-cards`

---

## Phase 4 — Populi safe + confirm-gated actions

`init` (safe), and `admin` ops (confirm-gated). No long-running launches.

### Step 4.1 — populi init safe action — test first (failing)

Add to `PopuliView.test.tsx`:

```tsx
it('fires populi init as a safe action', async () => {
  render(<PopuliView pushToast={vi.fn()} />);
  (await screen.findByText('Initialize Mesh')).click();
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['populi', 'init'],
      args: { __argv: [] },
    })
  );
});
```

Run (expect failure).

### Step 4.2 — populi init (make green)

Add an "Initialize" panel with an **Initialize Mesh** `type="button"` calling
`runVoxCommand(['populi','init'])` and rendering output (it prints env vars, no
process spawned). Wire = **exec**.

Run file test (green), `pnpm test`.

**Commit:** `feat(gui): PopuliView init safe action`

### Step 4.3 — Admin confirm-gated ops — test first (failing)

Add to `PopuliView.test.tsx`:

```tsx
it('requires confirm before quarantine; fires only after confirm', async () => {
  render(<PopuliView pushToast={vi.fn()} />);
  const nodeInput = await screen.findByLabelText('Admin node id');
  // controlled input: prefer fireEvent.change in the real file
  (nodeInput as HTMLInputElement).value = 'node-abc';
  nodeInput.dispatchEvent(new Event('input', { bubbles: true }));

  screen.getByText('Quarantine node').click();
  // first click arms a confirm; no exec yet
  expect(invokeMock).not.toHaveBeenCalledWith('execute_command', expect.objectContaining({
    path: ['populi', 'admin', 'quarantine'],
  }));

  (await screen.findByText('Confirm quarantine')).click();
  await waitFor(() =>
    expect(invokeMock).toHaveBeenCalledWith('execute_command', {
      path: ['populi', 'admin', 'quarantine'],
      args: { __argv: ['--node', 'node-abc', '--state', 'on'] },
    })
  );
});
```

Run (expect failure).

### Step 4.4 — Admin confirm-gated ops (make green)

Add an "Operator" panel:
- controlled `<input aria-label="Admin node id">` and (for lease revoke) a
  separate `<input aria-label="Exec lease id">`;
- three `type="button"` actions, each using a two-click in-component confirm
  pattern (first click sets `armed=<action>` and relabels the button to
  "Confirm <action>"; second click fires then clears `armed`). No native
  `window.confirm` (keeps it testable and on-brand).

Actions (mark each **exec**, all confirm-gated):

| Action | path | argv | Wire |
|---|---|---|---|
| Drain (maintenance on) | `['populi','admin','maintenance']` | `['--node', node, '--state', 'on']` | exec (confirm) |
| Quarantine node | `['populi','admin','quarantine']` | `['--node', node, '--state', 'on']` | exec (confirm) |
| Revoke exec lease | `['populi','admin','exec-lease-revoke']` | `['--lease-id', lease]` | exec (confirm) |

Disable each action when its required input is empty (no dead handler). Show a
warning-toned caption: "Operator actions affect a running mesh control plane."

Run file test (green), `pnpm test`, honesty guard green.

**Commit:** `feat(gui): PopuliView confirm-gated operator actions (drain/quarantine/lease-revoke)`

---

## Phase 5 — Surface registry note + final verification

### Step 5.1 — registry regeneration (no behavior change)

`mens`/`populi` already live in the `compute` nav group
(`contracts/gui/surface-registry.v1.yaml`); v1 keeps tier
`curated_decorator` (we swapped the decorator body, not the tier). No YAML edit
is required for v1. If `vox ci gui-surface-registry --write` reports drift from an
unrelated change, regenerate and commit separately. Do NOT promote to
`live_backend` in v1 (that is a v2 concern once streaming wrappers land).

No commit unless the generator reports drift.

### Step 5.2 — full verification (no commit)

Run, in `crates/vox-gui/ui/`:
- `pnpm typecheck` — clean.
- `pnpm test` — full vitest suite green, including:
  - `surfaces/__guards__/surfaceHonesty.guard.test.ts` (no placeholder/dead-handler),
  - `Mens/MensView.test.tsx`, `Populi/PopuliView.test.tsx`,
  - `decoratorRegistry.test.ts`,
  - `lib/runVoxCommand.test.ts`.

Record the pass counts in the execution summary. Do NOT commit; report results.

---

## Self-Review — spec coverage

Mapping each spec §4 row to a v1 step (or explicit v2 deferral).

### mens surface (spec §4)

| Spec row | v1 coverage | Wire |
|---|---|---|
| `mens probe [-d]` | Step 1.2 GPU Probe panel | exec |
| `mens status [--quotas/--cloud/--db]` | Step 1.2 Status/Quotas/Cloud/DB panels | exec |
| `mens models` | Step 1.2 Trained Models panel | exec |
| `mens corpus stats/readiness/eval` | Steps 1.2 / 1.4 / 1.6 | exec |
| `mens corpus mix/validate/extract*` | Step 2.2 (mix + validate; extract* deferred — heavy) | exec |
| `mens pipeline` | **needs-v2-stream** (long job) | v2 |
| `mens train` / `mens dogfood` | **needs-v2-stream** | v2 |
| `mens watch-telemetry` | **needs-v2-stream** (live chart) | v2 |
| `mens eval-local`/`eval-gate`/`baseline`/`eval` | corpus `eval` covered (Step 1.6); `eval-local` (needs `--model` checkpoint) deferred to v2 with the run picker | partial / v2 |
| `mens serve` | **needs-v2-stream** | v2 |
| `mens merge-qlora` / `export-gguf` | deferred (export form) — v2 | v2 |
| `mens bench-completion` | deferred (needs served URL) — v2 | v2 |
| `mens system-prompt-template` | optional follow-up (copy action) — not v1 | follow-up |

### populi surface (spec §4)

| Spec row | v1 coverage | Wire |
|---|---|---|
| `populi status`/`stats`/`registry-snapshot` | Step 3.2 panels | exec |
| `populi config show`/`check` | Step 3.2 Config panels | exec |
| `populi init` | Step 4.2 | exec |
| `populi up`/`down` | **needs-v2-stream** | v2 |
| `populi serve --enable` | **needs-v2-stream** | v2 |
| `populi node list` | Step 3.2 Nodes panel | exec |
| `populi node join`/`leave` | deferred (spawn long-lived worker) — v2 | v2 |
| `populi federation list` | Step 3.2 Federation panel | exec |
| `populi federation pair` | deferred (token form) — follow-up | follow-up |
| `populi dispatch`/`result` | deferred (script picker + poll) — v2 | v2 |
| `populi identity show`/`reputation` | Step 3.2 Identity/Reputation panels | exec |
| `populi identity export` + key handling | **routed to Settings/Secrets (Clavis), Plan 3C** — not built here | Plan 3C |
| `populi admin maintenance/quarantine/exec-lease-revoke` | Step 4.4 (confirm-gated) | exec (confirm) |
| `trust/untrust mesh node` | existing `trust_mesh_node`/`untrust_mesh_node` tauri cmds — Mesh surface owns this; not duplicated here | tauri✓ (elsewhere) |
| `populi corpus …` | deferred — overlaps `mens corpus`; link, don't duplicate (spec §7.3) | follow-up |
| `populi attest`/`join` | deferred (public-mesh join) — v2 | v2 |

### Ratified-decision checklist

- [x] Zero new Rust — every step rides `execute_command` (Phase 0 helper + all panels).
- [x] Monitor + light actions only; no spend (local-only forms; no `--cloud`/`--max-budget` surfaced).
- [x] Long-job launches (train/serve/pipeline/up/serve/dispatch) all marked **needs-v2-stream**.
- [x] Keys/identity central — `identity export` routed to Settings/Secrets (Plan 3C), no key UI here.
- [x] `populi admin` confirm-gated (Step 4.4 two-click confirm).
- [x] De-Latinized labels — "Model Lab" / "Mesh".
- [x] `models` kept distinct (no merge; recommended tagged-registry follow-up noted, not built).
- [x] Honesty guard green — no placeholder prose, no empty handlers; disabled buttons when inputs empty.

### Open questions resolved by ratification (spec §7)

- Q1 (local vs RunPod): **local-only in v1** — cloud flags not surfaced.
- Q2 (launch vs monitor): **monitor + light actions in v1**; launches → v2.
- Q3 (populi vs mens corpus): canonical corpus under `mens`; `populi corpus` deferred/linked.
- Q4 (`models` provenance): keep distinct; single tagged registry = follow-up.
- Q5 (operator/admin gating): confirm-gated in-UI (Step 4.4); `identity export` → Plan 3C.
- Q6 (spoke ladder UX): spoke selector is first-class on the Readiness panel (Step 1.4); full hub+spoke train form is v2.

---

## v2 (deferred): launch wrappers + cost UI

> **DO NOT BUILD IN v1.** Recorded here so v1 stays scoped and v2 has a starting
> point. This section is design intent, not implementation steps.

**1. Streaming launch wrappers (the only new Rust — v2).** Add
`crates/vox-gui/src/commands/mens.rs` and `populi.rs` with `#[tauri::command]`
wrappers that spawn the sidecar and emit a `vox://…` event stream the GUI
subscribes to (mirror `ui/src/hooks/useOrchestratorStatus.ts` with a polling
fallback). Persist runs via `start_gui_run`/`finish_gui_run`
(`crates/vox-gui/src/commands/runs.rs`):
- `mens_train_start(config) -> run_id` + `vox://mens-train` (step/loss/eta), wraps
  `mens train --background`; `mens_train_stop(run_id)` cooperative cancel.
- `mens_serve_start/stop` + `vox://mens-serve`.
- `mens_watch_telemetry(run_id)` (live loss/step chart) — or keep the cheaper
  `execute_command` + client poll of `mens watch-telemetry`.
- `populi_up/down` + `vox://populi-state`; `populi serve --enable` toggle;
  `populi dispatch` + `result` poll.
- `mens eval-local` run picker (needs a `--model` checkpoint path) + gate pass/fail
  `StatusPill`.
- Full Qwen3 hub+spoke train form (base hub + one of 5 domain spokes), device,
  epochs, preset/domain — local-only unless cost UI ships.

**2. Cost UI + gamification (v2 principle).** When cloud launches are enabled,
follow the **no-nag** model: cloud jobs may be launched from the GUI WITHOUT
confirmation popups, with spend made obvious via prominent, always-visible cost
tracking and gamified spend surfacing (reference how `opencode` surfaces running
cost inline). Tie into the existing `BudgetManager` / LLM-spend cost UI rather
than a new meter. Only then surface `--cloud {local,runpod,vast}` and
`--max-budget` on the train/serve forms.

**3. Registry promotion (v2).** Once `MensView`/`PopuliView` carry live streaming
backends, optionally promote tier `curated_decorator → live_backend` in
`contracts/gui/surface-registry.v1.yaml` and regenerate
(`vox ci gui-surface-registry --write`).

**4. Single tagged `models` registry (follow-up).** Have finished `mens` adapters
and `populi` mesh-served models register into the existing `models` registry with
a provenance tag (`mens`/`mesh`), with deep-links from Mens/Populi → `models`.
