---
category: "Architecture SSOTs"
title: "VoxMens / Populi GUI (Plan 3B FULL) — launch + monitor + cost UI, no v2 deferral"
date: 2026-06-26
status: plan
---

# VoxMens / Populi GUI (Plan 3B FULL)

**Goal.** Ship the *complete* `mens` ("Model Lab") and `populi` ("Mesh") GUI
surfaces in one plan — **full launch + monitor**, not a v1/v2 split. This
supersedes both the v1 monitor-only plan
[`2026-06-26-voxmens-gui-v1-plan3b.md`](2026-06-26-voxmens-gui-v1-plan3b.md) and
its deferred "v2" section. We build, in one program: (a) all read/light-action
coverage from the prior v1; (b) the 4–5 **streaming Tauri wrappers** that launch
long jobs (`mens train`/`serve`, `populi up`/`down`) emitting `vox://…` progress
à la `useOrchestratorStatus`; (c) **opencode-style no-nag cost tracking +
gamification** tied to `BudgetManager`/LLM-spend for cloud (`--cloud
runpod/vast`) jobs; and (d) central routing of identity/keys to Settings/Secrets,
with admin ops confirm-gated.

**Architecture.** The GUI rides two seams. Read + fire-and-forget controls ride
the existing `execute_command` sidecar seam
(`crates/vox-gui/src/commands/execute.rs`) — **zero new Rust**. Long-running
launches ride **new streaming `#[tauri::command]` wrappers** in
`crates/vox-gui/src/commands/mens.rs` + `populi.rs` that spawn the `vox` sidecar,
forward `vox://mens-train` / `vox://mens-serve` / `vox://populi-state` events
(the `app.emit(EVENT, value)` pattern from `orchestrator.rs`), and persist run
lifecycle via `start_gui_run`/`finish_gui_run` (`commands/runs.rs`, which already
carries `cost_usd`). The frontend subscribes with new `listen…` helpers in
`transport.ts` (mirroring `listenOrchStatus`) with a polling fallback. Cost is
read from the existing `get_llm_spend` SSOT (`commands/user_config.rs` →
`LlmSpendDto`) and surfaced inline (no confirm popups) per the opencode no-nag
model.

**Tech Stack.** React 18 + TypeScript + Vitest/Testing-Library (frontend,
`crates/vox-gui/ui/`); Rust + Tauri 2 + `tauri_plugin_shell` + `tokio`
(streaming wrappers); `vox-db` `agent_runs` (run + cost persistence); `vox-config`
budget caps. Tests: `pnpm test` (= `vitest run`) for TS; `cargo test -p vox-gui`
for Rust wrappers.

**Spec.**
- Master umbrella SSOT:
  [`2026-06-26-vox-search-unified-code-intelligence-design.md`](../specs/2026-06-26-vox-search-unified-code-intelligence-design.md)
  (context for the unified surface program).
- Source design (parity mapping, §4/§5/§7):
  [`2026-06-26-voxmens-gui-cli-parity-design.md`](../specs/2026-06-26-voxmens-gui-cli-parity-design.md).

**Worktree (STRICT).** All commands run against
`/c/Users/Owner/vox-graphify-gui`. Every implementation commit uses
`git -C /c/Users/Owner/vox-graphify-gui add <paths> && git -C
/c/Users/Owner/vox-graphify-gui commit -m "<msg>"`. **add + commit only** — never
`push`, `rebase`, `reset --hard`, `clean`, `checkout --`, or `merge`. The
workflow performs the final integration; sub-agents only add+commit their step.

---

## Cross-plan dependencies (read first)

- **PRECEDES nothing blocking inside this plan** beyond Phase 0 (the shared exec
  helper) and Phase 5A (the Rust wrapper crate scaffolding). Both are explicit
  prerequisites called out per-batch below.
- **Plan 3C (central Settings/Secrets)** owns `populi identity export` (private
  key) and all key entry. This plan **must not** add a key/export UI; it only
  links to Settings → Secrets. Plan 3C can land in parallel — there is no code
  collision (different files). If 3C is not yet merged, the link text is still
  honest (it points at an existing Settings surface).
- **Surface-registry SSOT** (`contracts/gui/surface-registry.v1.yaml`): `mens`
  and `populi` already exist in the `compute` nav group. This plan **promotes**
  their tier from `curated_decorator` → `live_backend` (Phase 6) **because** we
  now ship streaming backends — that promotion is in-scope here (unlike the v1
  plan which kept `curated_decorator`).
- No dependency on the Vox Search plans (Plan 1/2); those touch
  `vox-graphify-reader`/search surfaces, disjoint from `Mens/`/`Populi/`.

---

## Grounding (verified in repo, branch `claude/graphify-general-gui-ia`)

- `execute_command(path: Vec<String>, args: Value)` shells the `vox` sidecar.
  When `args.__argv` is present it pushes each non-empty token verbatim after
  `path` (verified `execute.rs:22-27`). **Every `exec` control uses `__argv`** to
  keep arg order explicit and bypass the key-mangling object branch.
- Decorator seam: `surfaces/decoratorRegistry.ts` exports
  `surfaceDecorators: Record<string, React.ComponentType<SurfaceDecoratorProps>>`;
  `mens`/`populi` are currently `commandSurface(...)` closures (verified
  `decoratorRegistry.ts:54,59`). `SurfaceDecoratorProps = { pushToast: (item:
  Toast) => void; gamifyEnabled?: boolean }` (verified `:22-25`). Swapping an
  entry is a one-line change.
- Streaming emitter pattern (verified `orchestrator.rs`): a `pub fn
  spawn_…(app_handle: tauri::AppHandle, …)` calls `tokio::spawn`, drains an
  `mpsc` channel, and does `app_handle.emit(EVENT_CONST, value)` per frame
  (`use tauri::Emitter;`). Event constants are `pub const … = "vox://…"`.
- Run + cost persistence (verified `runs.rs`): `start_gui_run(StartGuiRunInput{
  run_id, workflow_name, command, model, … })` and `finish_gui_run(run_id,
  success, completed_steps, error, cost_usd, tokens_in, tokens_out)` upsert into
  `vox_db` `agent_runs` (row carries `cost_usd: f64`). `GuiRunRecord.cost_usd:
  Option<f64>`.
- Cost SSOT (verified `user_config.rs:307-338`): `get_llm_spend(session_id) ->
  LlmSpendDto { session_usd, day_usd, total_usd, daily_budget_usd,
  per_session_budget_usd }` (camelCase over the wire). Frontend hook
  `useLlmSpend()` (verified `hooks/useLlmSpend.ts`) polls `getLlmSpend()` every
  60 s.
- Frontend listen helper pattern (verified `transport.ts:27-31`):
  `export function listenX(onX): Promise<UnlistenFn> { return listen<T>(EVENT,
  e => onX(e.payload)); }` using `import { listen, type UnlistenFn } from
  '@tauri-apps/api/event'`.
- Command registration (verified `main.rs:109-219`): every `#[tauri::command]`
  is listed in `tauri::generate_handler![ … ]`. New wrappers must be added there.
- Honesty guard: `surfaces/__guards__/surfaceHonesty.guard.test.ts` +
  `honestyScan.ts` forbid placeholder prose and `onClick={() => {}}` dead
  handlers across shipped `*.tsx`. Every control must call a real seam.
- Test runner (verified `crates/vox-gui/ui/package.json`): `pnpm test`
  (= `vitest run`); single file `pnpm exec vitest run <path>`; typecheck
  `pnpm typecheck`. Rust: `cargo test -p vox-gui` (run from repo root or
  `crates/vox-gui`).

### Verified CLI command/arg names (source of truth)

`mens` (`crates/vox-ml-cli/src/commands/mens/populi/action_populi_enum.rs`):
- `mens probe [-d|--detailed]`
- `mens status [--quotas] [--config] [--cloud] [--db]`
- `mens models`
- `mens watch-telemetry [--telemetry <path>] [--err-log <path>] [--interval-ms <n>]`
- `mens train` — verified flags: `--preset <s>`, `--domain <s>` (spoke),
  `--device <s>`, `--deployment-target {workstation,mobile_edge}`,
  `--process-priority {normal,low}`, `--background` (bool), `--log-dir <path>`,
  `--cloud {local,vast,runpod,auto}` (default `local`), `--max-budget <f64>`
  (verified `:234-237`).
- `mens serve` — verified flags: `--model <path>`, `--port <u16>`, `--host <s>`,
  `--cloud {local,vast,runpod,auto}` (default `local`), `--max-budget <f64>`,
  `--model-hf <s>` (verified `:305-336`).

`mens corpus` (`crates/vox-ml-cli/src/commands/corpus/mod.rs`, `CorpusAction`):
- `mens corpus stats [-i|--input <jsonl>]`
- `mens corpus readiness --spoke <name> [--input <jsonl>] [--min-rows <n>] …`
- `mens corpus eval <input> [-o <out>] [--print-summary]` (input **positional**, required)
- `mens corpus mix [--config <yaml>] [--allow-missing-sources]`
- `mens corpus validate-batch -i <input> [-o <out>] [--no-recheck] …` (alias `validate`)
- `mens corpus fingerprint`

`populi` (`crates/vox-ml-cli/src/commands/populi_cli.rs`, `PopuliCli`):
- `populi status [--json]` / `stats [--json]` / `registry-snapshot [--json]`
- `populi config show` / `config check`
- `populi node list` / `federation list [--json]`
- `populi identity show` / `identity reputation` (read; `export` → Plan 3C)
- `populi init [--force]`
- `populi up` — verified flags: `--mode {lan,…}`, `--bind <addr>` (default
  `127.0.0.1:9847`), `--visibility {private,public,hybrid}`, `--public-mesh`
  (bool), `--gpus <s>` (verified `:63-105`).
- `populi down` (verified `:108`).
- `populi admin maintenance --node <id> --state {on|off}` / `quarantine …` /
  `exec-lease-revoke --lease-id <id>` (confirm-gated).

---

## Conventions for every step

- **TDD:** write the failing test first, run it, confirm it fails for the stated
  reason, write minimal code, run the file test green, then the relevant full
  suite (`pnpm test` and/or `cargo test -p vox-gui`).
- **Commit after every green step** with the exact message shown, using the
  STRICT add+commit form. Steps marked "no commit" do not commit.
- Frontend files under `crates/vox-gui/ui/src/components/surfaces/{Mens,Populi}/`
  and `surfaces/lib/`. Rust wrappers under `crates/vox-gui/src/commands/`.
- `invoke` calls go through `@tauri-apps/api/core`'s `invoke('execute_command',
  { path, args: { __argv: [...] } })` for exec, or `invoke('<wrapper>', { … })`
  for streaming launches. Mock `@tauri-apps/api/core` in tests like
  `Models/ModelsView.test.tsx`.
- Every `<button>` carries `type="button"`. Lists use `role="list"/"listitem"`.
- Run frontend commands from `crates/vox-gui/ui/`; Rust from repo root.

---

## Task graph + [PARALLEL-SAFE] batch structure

Tasks are tagged `[SEQUENTIAL]` (gates later work) or `[PARALLEL-SAFE]` (can run
in its batch concurrently with siblings). Batches are dispatched in order; tasks
inside a batch fan out.

| Batch | Tasks | Tag | Gate |
|---|---|---|---|
| **B0 (prereq)** | P0.1 exec helper | [SEQUENTIAL] | must finish before any view |
| **B1 (Rust wrappers)** | P5A.1 crate scaffold | [SEQUENTIAL] | must finish before P5A.2–.5, P5B |
| **B2 (views, fan-out 2)** | P1 MensView read · P3 PopuliView read | [PARALLEL-SAFE]×2 | both depend on B0 only |
| **B3 (light actions, fan-out 2)** | P2 Mens corpus actions · P4 Populi init+admin | [PARALLEL-SAFE]×2 | depend on B2 (same files, but distinct Mens vs Populi) |
| **B4 (streaming wrappers, fan-out 4)** | P5A.2 mens_train · P5A.3 mens_serve · P5A.4 populi_up/down · P5A.5 transport+listen helpers | [PARALLEL-SAFE]×4 | depend on B1 |
| **B5 (launch UI, fan-out 2)** | P5B Mens launch panels · P5C Populi power toggle | [PARALLEL-SAFE]×2 | depend on B2 + B4 |
| **B6 (cost+gamify)** | P5D cost ribbon + gamify | [SEQUENTIAL] | depends on B5 (consumes run ids) |
| **B7 (register + finalize)** | P6.1 register decorators · P6.2 registry promote · P6.3 verify | [SEQUENTIAL] | depends on all above |

Note on B2/B3 file overlap: P1/P2 both edit `Mens/MensView.tsx`; P3/P4 both edit
`Populi/PopuliView.tsx`. **Mens (P1,P2)** and **Populi (P3,P4)** are independent
file trees, so each *pair* runs sequentially within its tree but the two trees
fan out. A workflow may dispatch P1→P2 and P3→P4 as two parallel lanes.

---

## Phase 0 — shared exec helper (no new Rust)  [B0]

### Task P0.1 — exec helper (TDD)  [SEQUENTIAL]

**Step P0.1a — failing test.** Create
`crates/vox-gui/ui/src/components/surfaces/lib/runVoxCommand.test.ts`:

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

Run (expect fail — module not found):
`pnpm exec vitest run src/components/surfaces/lib/runVoxCommand.test.ts`

**Step P0.1b — implementation.** Create
`crates/vox-gui/ui/src/components/surfaces/lib/runVoxCommand.ts`:

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
  argv: string[] = [],
): Promise<ExecuteOutput> {
  return invoke<ExecuteOutput>('execute_command', { path, args: { __argv: argv } });
}
```

Run the file test (green), then `pnpm test` (full suite green).

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/lib/runVoxCommand.ts crates/vox-gui/ui/src/components/surfaces/lib/runVoxCommand.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): add runVoxCommand exec helper for mens/populi"
```

---

## Phase 1 — Mens ("Model Lab") read coverage  [B2 lane A]

Dependency: B0 (P0.1). [PARALLEL-SAFE] with Phase 3.

### Task P1.1 — MensView read panels (TDD)  [PARALLEL-SAFE]

**Step P1.1a — failing test.** Create
`crates/vox-gui/ui/src/components/surfaces/Mens/MensView.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor, fireEvent } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn((_cmd: string, args?: any) =>
  Promise.resolve({ exit_code: 0, stdout: `out:${args?.path?.join(' ')}`, stderr: '' }),
);
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { MensView } from './MensView';

describe('MensView read coverage', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); });

  it('renders the Model Lab heading', () => {
    render(<MensView pushToast={vi.fn()} />);
    expect(screen.getByText('Model Lab')).toBeTruthy();
  });

  it('runs mens status on mount via execute_command', async () => {
    render(<MensView pushToast={vi.fn()} />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'status'], args: { __argv: [] },
      }),
    );
  });

  it('every button carries type="button"', async () => {
    render(<MensView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('button').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) expect(b.getAttribute('type')).toBe('button');
  });

  it('runs corpus readiness with the selected spoke', async () => {
    render(<MensView pushToast={vi.fn()} />);
    fireEvent.click(await screen.findByText('Check Readiness'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'corpus', 'readiness'], args: { __argv: ['--spoke', 'vox-lang'] },
      }),
    );
  });

  it('runs corpus eval with the entered positional input + summary', async () => {
    render(<MensView pushToast={vi.fn()} />);
    const input = await screen.findByLabelText('Eval corpus JSONL path');
    fireEvent.change(input, { target: { value: 'target/dogfood/train.jsonl' } });
    fireEvent.click(screen.getByText('Run Eval'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'corpus', 'eval'],
        args: { __argv: ['target/dogfood/train.jsonl', '--print-summary'] },
      }),
    );
  });
});
```

Run (expect fail — module not found):
`pnpm exec vitest run src/components/surfaces/Mens/MensView.test.tsx`

**Step P1.1b — implementation.** Create
`crates/vox-gui/ui/src/components/surfaces/Mens/MensView.tsx`. Model it on
`surfaces/Models/ModelsView.tsx` (invoke in `useCallback`, `useEffect` on mount,
`pushToast` on failure). Use `SurfaceDecoratorProps` from `../decoratorRegistry`,
`Glass` from `../../ui/Glass`, and `runVoxCommand`/`ExecuteOutput` from
`../lib/runVoxCommand`. Render a panel grid.

Each **read panel** has a title, a `<pre>` output region (copy `<pre>` styling
from `CommandCardsView.tsx`), runs its command on mount + via a per-panel
**Refresh** `type="button"`, and on `exit_code !== 0` or a thrown error calls
`pushToast({ tone: 'warn', title, message })`.

Read panels (all **exec**):

| Panel | path | argv |
|---|---|---|
| Training Status | `['mens','status']` | `[]` |
| Quotas | `['mens','status']` | `['--quotas']` |
| Cloud Dispatch Summary | `['mens','status']` | `['--cloud']` |
| Intelligence Metrics | `['mens','status']` | `['--db']` |
| Trained Models | `['mens','models']` | `[]` |
| GPU Probe | `['mens','probe']` | `['--detailed']` |
| Corpus Stats | `['mens','corpus','stats']` | `[]` |

Plus two **input-driven** panels:

- **Corpus Readiness**: a `<select aria-label="Training spoke">` with the 5
  spokes `vox-lang`, `rust-expert`, `agents`, `tool-selection`,
  `argument-generation` (default `vox-lang`) and a **Check Readiness**
  `type="button"` that runs `runVoxCommand(['mens','corpus','readiness'],
  ['--spoke', spoke])`.
- **Corpus Eval**: a controlled `<input aria-label="Eval corpus JSONL path">`
  (default `target/dogfood/train.jsonl`) and a **Run Eval** `type="button"`
  building argv `[inputPath.trim(), '--print-summary']`; disable the button when
  the path is empty (no dead handler). `mens corpus eval` takes input as a
  **positional** (verified `#[arg(required = true)]`), so it is the first
  `__argv` token, NOT a `--input` flag.

Under Trained Models add plain text: "See the Models surface for the live routing
registry." (no handler).

Run the file test (green), then `pnpm test`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Mens/MensView.tsx crates/vox-gui/ui/src/components/surfaces/Mens/MensView.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): MensView read panels (status/quotas/cloud/db/models/probe/corpus + readiness/eval)"
```

---

## Phase 2 — Mens safe actions (fire-and-forget corpus ops)  [B3 lane A]

Dependency: P1.1. [PARALLEL-SAFE] with Phase 4 (different file tree).

### Task P2.1 — corpus build actions (TDD)  [PARALLEL-SAFE]

**Step P2.1a — failing test.** Append to `MensView.test.tsx`:

```tsx
describe('MensView safe corpus actions', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); });

  it('fires corpus fingerprint', async () => {
    render(<MensView pushToast={vi.fn()} />);
    fireEvent.click(await screen.findByText('Fingerprint'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'corpus', 'fingerprint'], args: { __argv: [] },
      }),
    );
  });

  it('fires corpus mix', async () => {
    render(<MensView pushToast={vi.fn()} />);
    fireEvent.click(await screen.findByText('Mix Corpus'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'corpus', 'mix'], args: { __argv: [] },
      }),
    );
  });

  it('fires corpus validate-batch with the eval input path', async () => {
    render(<MensView pushToast={vi.fn()} />);
    const input = await screen.findByLabelText('Eval corpus JSONL path');
    fireEvent.change(input, { target: { value: 'target/dogfood/train.jsonl' } });
    fireEvent.click(screen.getByText('Validate Corpus'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['mens', 'corpus', 'validate-batch'],
        args: { __argv: ['-i', 'target/dogfood/train.jsonl'] },
      }),
    );
  });
});
```

Run (expect fail).

**Step P2.1b — implementation.** Add a "Build Corpus" panel to `MensView.tsx`
with three `type="button"` actions (each calls `runVoxCommand`, renders output to
a shared `<pre>`, toasts `{ tone: 'ok' }` on success / `{ tone: 'warn' }` on
non-zero exit):

| Action button | path | argv |
|---|---|---|
| Fingerprint | `['mens','corpus','fingerprint']` | `[]` |
| Mix Corpus | `['mens','corpus','mix']` | `[]` |
| Validate Corpus | `['mens','corpus','validate-batch']` | `['-i', evalInputPath]` (reuse the Corpus Eval input field; disable when empty) |

Caption (plain text): "Writes local JSONL — no cloud spend."

Run the file test (green), then `pnpm test`, then confirm the honesty guard
passes: `pnpm exec vitest run src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Mens/MensView.tsx crates/vox-gui/ui/src/components/surfaces/Mens/MensView.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): MensView safe corpus actions (fingerprint/mix/validate)"
```

---

## Phase 3 — Populi ("Mesh") read coverage  [B2 lane B]

Dependency: B0 (P0.1). [PARALLEL-SAFE] with Phase 1.

### Task P3.1 — PopuliView read panels (TDD)  [PARALLEL-SAFE]

**Step P3.1a — failing test.** Create
`crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const invokeMock = vi.fn((_cmd: string, args?: any) =>
  Promise.resolve({ exit_code: 0, stdout: `out:${args?.path?.join(' ')}`, stderr: '' }),
);
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (cmd: string, args?: unknown) => invokeMock(cmd, args),
}));

import { PopuliView } from './PopuliView';

describe('PopuliView read coverage', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); });

  it('renders the Mesh heading', () => {
    render(<PopuliView pushToast={vi.fn()} />);
    expect(screen.getByText('Mesh')).toBeTruthy();
  });

  it('runs populi status --json on mount', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['populi', 'status'], args: { __argv: ['--json'] },
      }),
    );
  });

  it('every button carries type="button"', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    await waitFor(() => expect(screen.getAllByRole('button').length).toBeGreaterThan(0));
    for (const b of screen.getAllByRole('button')) expect(b.getAttribute('type')).toBe('button');
  });
});
```

Run (expect fail — module not found).

**Step P3.1b — implementation.** Create
`crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.tsx` modeled on
`MensView.tsx`. Read panels (all **exec**):

| Panel | path | argv |
|---|---|---|
| Mesh Health | `['populi','status']` | `['--json']` |
| Queue Stats | `['populi','stats']` | `['--json']` |
| Local Snapshot | `['populi','registry-snapshot']` | `['--json']` |
| Config (resolved) | `['populi','config','show']` | `[]` |
| Config Check | `['populi','config','check']` | `[]` |
| Nodes | `['populi','node','list']` | `[]` |
| Federation | `['populi','federation','list']` | `['--json']` |
| Identity (public) | `['populi','identity','show']` | `[]` |
| Reputation | `['populi','identity','reputation']` | `[]` |

Under Identity add plain text: "Private-key backup is managed in Settings →
Secrets" — NO export button (Plan 3C owns it). Treat non-zero exits (no control
plane running) as `{ tone: 'warn' }` informational toasts, not crashes.

Run the file test (green), then `pnpm test`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.tsx crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): PopuliView read panels (status/stats/registry/config/nodes/federation/identity)"
```

---

## Phase 4 — Populi safe + confirm-gated actions  [B3 lane B]

Dependency: P3.1. [PARALLEL-SAFE] with Phase 2.

### Task P4.1 — populi init + confirm-gated admin (TDD)  [PARALLEL-SAFE]

**Step P4.1a — failing test.** Append to `PopuliView.test.tsx`:

```tsx
import { fireEvent } from '@testing-library/react';

describe('PopuliView actions', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); });

  it('fires populi init', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    fireEvent.click(await screen.findByText('Initialize Mesh'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['populi', 'init'], args: { __argv: [] },
      }),
    );
  });

  it('requires confirm before quarantine; fires only after confirm', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    const nodeInput = await screen.findByLabelText('Admin node id');
    fireEvent.change(nodeInput, { target: { value: 'node-abc' } });

    fireEvent.click(screen.getByText('Quarantine node'));
    expect(invokeMock).not.toHaveBeenCalledWith(
      'execute_command',
      expect.objectContaining({ path: ['populi', 'admin', 'quarantine'] }),
    );

    fireEvent.click(await screen.findByText('Confirm quarantine'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('execute_command', {
        path: ['populi', 'admin', 'quarantine'],
        args: { __argv: ['--node', 'node-abc', '--state', 'on'] },
      }),
    );
  });
});
```

Run (expect fail).

**Step P4.1b — implementation.** Add to `PopuliView.tsx`:

1. An "Initialize" panel with **Initialize Mesh** `type="button"` calling
   `runVoxCommand(['populi','init'])` (prints env vars, no process spawned).
2. An "Operator" panel:
   - controlled `<input aria-label="Admin node id">` and (for lease revoke) a
     `<input aria-label="Exec lease id">`;
   - three `type="button"` confirm-gated actions using a two-click in-component
     confirm (first click sets `armed=<action>` and relabels to "Confirm
     <action>"; second click fires then clears `armed`). No `window.confirm`.

   | Action | path | argv |
   |---|---|---|
   | Drain (maintenance on) | `['populi','admin','maintenance']` | `['--node', node, '--state', 'on']` |
   | Quarantine node | `['populi','admin','quarantine']` | `['--node', node, '--state', 'on']` |
   | Revoke exec lease | `['populi','admin','exec-lease-revoke']` | `['--lease-id', lease]` |

   Disable each action when its required input is empty. Warning caption:
   "Operator actions affect a running mesh control plane."

Run the file test (green), `pnpm test`, honesty guard green.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.tsx crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): PopuliView init + confirm-gated operator actions"
```

---

## Phase 5A — streaming Tauri wrappers (the new Rust)  [B1 + B4]

These spawn the `vox` sidecar for long jobs and emit `vox://…` progress, persist
run lifecycle/cost. Pattern is copied verbatim from `orchestrator.rs` (emit) and
`runs.rs` (persist).

### Task P5A.1 — wrapper crate scaffold + event consts (TDD)  [SEQUENTIAL]  [B1]

This gates all of B4. Create the two module files with the event constants and a
shared `spawn_streaming_command` helper, plus a unit test that does NOT require a
live sidecar (tests argv assembly + event-name constants).

**Step P5A.1a — failing test.** Create
`crates/vox-gui/src/commands/mens_populi_stream_tests.rs` is NOT needed; put unit
tests inline. First add the module declarations. In
`crates/vox-gui/src/commands/mod.rs` add (place with the other `pub mod` lines):

```rust
pub mod mens;
pub mod populi;
```

Create `crates/vox-gui/src/commands/mens.rs` with a failing-by-absence test:

```rust
//! Streaming Tauri wrappers for long-running `vox mens` jobs (train/serve).
//! Read + fire-and-forget mens controls ride `execute_command`; only the
//! launch-and-stream path lives here.

use serde::Deserialize;

/// Tauri event channel carrying live `vox mens train` progress frames.
pub const MENS_TRAIN_EVENT: &str = "vox://mens-train";
/// Tauri event channel carrying live `vox mens serve` lifecycle frames.
pub const MENS_SERVE_EVENT: &str = "vox://mens-serve";

/// Train launch parameters mapped 1:1 onto verified `mens train` flags.
#[derive(Debug, Clone, Deserialize)]
pub struct MensTrainConfig {
    pub preset: Option<String>,
    pub domain: Option<String>,
    pub device: Option<String>,
    /// "local" | "runpod" | "vast" | "auto" (default local).
    pub cloud: Option<String>,
    pub max_budget: Option<f64>,
}

/// Build the `mens train` argv from a config, surfacing only verified flags.
/// `--background` is always added so the sidecar returns promptly and we stream
/// telemetry; cloud/budget are only added when the operator opted into cloud.
pub fn build_train_argv(cfg: &MensTrainConfig) -> Vec<String> {
    let mut argv = vec!["mens".to_string(), "train".to_string(), "--background".to_string()];
    if let Some(p) = &cfg.preset { argv.push("--preset".into()); argv.push(p.clone()); }
    if let Some(d) = &cfg.domain { argv.push("--domain".into()); argv.push(d.clone()); }
    if let Some(dev) = &cfg.device { argv.push("--device".into()); argv.push(dev.clone()); }
    let cloud = cfg.cloud.as_deref().unwrap_or("local");
    if cloud != "local" {
        argv.push("--cloud".into()); argv.push(cloud.to_string());
        if let Some(b) = cfg.max_budget { argv.push("--max-budget".into()); argv.push(b.to_string()); }
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_train_argv_omits_cloud_and_budget() {
        let cfg = MensTrainConfig {
            preset: Some("4080".into()), domain: Some("vox-lang".into()),
            device: None, cloud: None, max_budget: Some(5.0),
        };
        let argv = build_train_argv(&cfg);
        assert_eq!(argv, vec![
            "mens", "train", "--background", "--preset", "4080", "--domain", "vox-lang",
        ]);
    }

    #[test]
    fn cloud_train_argv_includes_cloud_and_budget() {
        let cfg = MensTrainConfig {
            preset: None, domain: None, device: Some("cuda".into()),
            cloud: Some("runpod".into()), max_budget: Some(10.0),
        };
        let argv = build_train_argv(&cfg);
        assert_eq!(argv, vec![
            "mens", "train", "--background", "--device", "cuda",
            "--cloud", "runpod", "--max-budget", "10",
        ]);
    }

    #[test]
    fn event_consts_are_namespaced() {
        assert_eq!(MENS_TRAIN_EVENT, "vox://mens-train");
        assert_eq!(MENS_SERVE_EVENT, "vox://mens-serve");
    }
}
```

Create `crates/vox-gui/src/commands/populi.rs` with the populi event const +
argv builder + tests:

```rust
//! Streaming Tauri wrappers for `vox populi` mesh lifecycle (up/down).

use serde::Deserialize;

/// Tauri event channel carrying live mesh state frames (node up/down, peers).
pub const POPULI_STATE_EVENT: &str = "vox://populi-state";

/// Mesh bring-up parameters mapped onto verified `populi up` flags.
#[derive(Debug, Clone, Deserialize)]
pub struct PopuliUpConfig {
    /// "private" | "public" | "hybrid" (default private).
    pub visibility: Option<String>,
    pub public_mesh: Option<bool>,
    pub bind: Option<String>,
}

pub fn build_up_argv(cfg: &PopuliUpConfig) -> Vec<String> {
    let mut argv = vec!["populi".to_string(), "up".to_string()];
    if let Some(v) = &cfg.visibility { argv.push("--visibility".into()); argv.push(v.clone()); }
    if let Some(b) = &cfg.bind { argv.push("--bind".into()); argv.push(b.clone()); }
    if cfg.public_mesh.unwrap_or(false) { argv.push("--public-mesh".into()); }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn up_argv_defaults_to_bare() {
        let cfg = PopuliUpConfig { visibility: None, public_mesh: None, bind: None };
        assert_eq!(build_up_argv(&cfg), vec!["populi", "up"]);
    }

    #[test]
    fn up_argv_with_visibility_and_public_mesh() {
        let cfg = PopuliUpConfig {
            visibility: Some("hybrid".into()), public_mesh: Some(true),
            bind: Some("127.0.0.1:9847".into()),
        };
        assert_eq!(build_up_argv(&cfg), vec![
            "populi", "up", "--visibility", "hybrid", "--bind", "127.0.0.1:9847", "--public-mesh",
        ]);
    }

    #[test]
    fn state_event_is_namespaced() {
        assert_eq!(POPULI_STATE_EVENT, "vox://populi-state");
    }
}
```

Run (expect fail until mod.rs wires the modules):
`cargo test -p vox-gui build_train_argv up_argv`

**Step P5A.1b — make green.** Confirm `mod.rs` has the two `pub mod` lines.
Run `cargo test -p vox-gui --lib mens:: populi::` (or
`cargo test -p vox-gui build_train_argv up_argv local_train cloud_train event_consts state_event`)
— all green.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/commands/mens.rs crates/vox-gui/src/commands/populi.rs crates/vox-gui/src/commands/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): scaffold mens/populi streaming wrapper modules (argv builders + event consts)"
```

### Task P5A.2 — mens_train_start/stop streaming command  [PARALLEL-SAFE]  [B4]

Dependency: P5A.1. Edits only `mens.rs` + `main.rs` handler list (the latter is a
shared file — see "shared-file note" below).

**Step P5A.2a — failing test.** Append to the `tests` mod in `mens.rs`:

```rust
#[test]
fn train_run_record_uses_workflow_and_command() {
    // The run we persist must carry a stable workflow name + the assembled command
    // so the Runs surface and cost ribbon can attribute spend.
    let cfg = MensTrainConfig {
        preset: Some("safe".into()), domain: None, device: None,
        cloud: Some("vast".into()), max_budget: Some(3.0),
    };
    let argv = build_train_argv(&cfg);
    let command = argv.join(" ");
    assert!(command.contains("--cloud vast"));
    assert!(command.starts_with("mens train --background"));
}
```

Run (expect fail — references only existing fns, should compile+pass; if it
passes immediately that is acceptable — its purpose is to pin the command string
shape before adding the command). Then add the real failing assertion by writing
the command test that calls a not-yet-existing helper `train_workflow_name`:

```rust
#[test]
fn train_workflow_name_is_stable() {
    assert_eq!(train_workflow_name(), "mens-train");
}
```

Run `cargo test -p vox-gui train_workflow_name` (expect fail — undefined).

**Step P5A.2b — implementation.** In `mens.rs` add the streaming command. It
spawns the sidecar via `tauri_plugin_shell::ShellExt` (like `execute.rs`),
persists a run via the existing `start_gui_run`/`finish_gui_run` logic — call
those functions directly (they are `pub async fn` in `commands::runs`) — and
emits frames on `MENS_TRAIN_EVENT` using `tauri::Emitter`:

```rust
use tauri::Emitter;
use tauri_plugin_shell::ShellExt;
use crate::commands::runs::{StartGuiRunInput, start_gui_run, finish_gui_run};

pub fn train_workflow_name() -> &'static str { "mens-train" }

#[derive(serde::Serialize, Clone)]
pub struct MensTrainStarted { pub run_id: String }

/// Launch `vox mens train --background …`, persist a GUI run, and stream
/// telemetry frames as `vox://mens-train`. Returns the run id immediately
/// (the sidecar backgrounds itself; we tail telemetry via watch-telemetry).
#[tauri::command]
pub async fn mens_train_start(
    app: tauri::AppHandle,
    config: MensTrainConfig,
) -> Result<MensTrainStarted, String> {
    let argv = build_train_argv(&config);
    let command = argv.join(" ");
    let run_id = format!("mens-train-{}", now_ms());

    start_gui_run(StartGuiRunInput {
        run_id: run_id.clone(),
        workflow_name: train_workflow_name().to_string(),
        planned_steps: None,
        command: Some(command.clone()),
        repo: None,
        worktree: None,
        model: config.preset.clone(),
    })
    .await?;

    // Launch the background trainer; it returns promptly with --background.
    let out = app.shell().sidecar(super::execute::VOX_SIDECAR_NAME)
        .map_err(|e| e.to_string())?
        .args(argv).output().await.map_err(|e| e.to_string())?;
    let success = out.status.code() == Some(0);
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();

    let _ = app.emit(MENS_TRAIN_EVENT, serde_json::json!({
        "run_id": run_id, "phase": "launched", "ok": success,
        "stderr": if success { String::new() } else { stderr.clone() },
    }));

    finish_gui_run(
        run_id.clone(), success, Some(1),
        if success { None } else { Some(stderr) },
        None, None, None,
    ).await?;

    Ok(MensTrainStarted { run_id })
}

/// Cooperative cancel: shells `vox mens train` stop is not a CLI verb, so we
/// emit a cancel frame and mark the run failed/cancelled. Background trainers
/// honor their own log-dir lifecycle; this is the GUI-side state transition.
#[tauri::command]
pub async fn mens_train_stop(app: tauri::AppHandle, run_id: String) -> Result<(), String> {
    let _ = app.emit(MENS_TRAIN_EVENT, serde_json::json!({
        "run_id": run_id, "phase": "cancelled",
    }));
    finish_gui_run(run_id, false, None, Some("cancelled by operator".into()), None, None, None).await
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default().as_millis() as i64
}
```

Make `VOX_SIDECAR_NAME` reachable: in `execute.rs` change
`const VOX_SIDECAR_NAME: &str = "vox";` to `pub(crate) const VOX_SIDECAR_NAME:
&str = "vox";`.

Register both commands in `main.rs` `generate_handler!` (after
`commands::execute::execute_command,`):

```rust
            commands::mens::mens_train_start,
            commands::mens::mens_train_stop,
```

Run `cargo test -p vox-gui train_workflow_name train_run_record` (green) then
`cargo build -p vox-gui` (compiles).

**Shared-file note (main.rs / execute.rs):** P5A.2/.3/.4 each append distinct
lines to the same `generate_handler!` block and P5A.2 flips one `const` to
`pub(crate)`. To keep B4 parallel-safe, the workflow must serialize the
`main.rs`/`execute.rs` edits: dispatch P5A.2 first (it makes the `const` change +
adds its two lines), then P5A.3 and P5A.4 append their lines. If run truly
concurrently, treat `main.rs` + `execute.rs` as a lock and rebase the handler
list additively (add-only, no conflict on distinct lines). Tag remains
[PARALLEL-SAFE] for the *crate logic*; the handler-list append is a trivial
add-only merge.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/commands/mens.rs crates/vox-gui/src/commands/execute.rs crates/vox-gui/src/main.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): mens_train_start/stop streaming wrappers (vox://mens-train + run persist)"
```

### Task P5A.3 — mens_serve_start/stop streaming command  [PARALLEL-SAFE]  [B4]

Dependency: P5A.1 (and the `main.rs`/`execute.rs` serialization note above).

**Step P5A.3a — failing test.** Append to `mens.rs` tests:

```rust
#[test]
fn serve_argv_local_and_cloud() {
    let local = build_serve_argv(&MensServeConfig {
        model: Some("mens/runs/lora/model.safetensors".into()), port: Some(8089),
        cloud: None, max_budget: None, model_hf: None,
    });
    assert_eq!(local, vec![
        "mens", "serve", "--model", "mens/runs/lora/model.safetensors", "--port", "8089",
    ]);
    let cloud = build_serve_argv(&MensServeConfig {
        model: None, port: None, cloud: Some("runpod".into()),
        max_budget: Some(8.0), model_hf: Some("Qwen/Qwen3-4B".into()),
    });
    assert_eq!(cloud, vec![
        "mens", "serve", "--cloud", "runpod", "--max-budget", "8", "--model-hf", "Qwen/Qwen3-4B",
    ]);
}
```

Run `cargo test -p vox-gui serve_argv` (expect fail — undefined).

**Step P5A.3b — implementation.** Add to `mens.rs`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MensServeConfig {
    pub model: Option<String>,
    pub port: Option<u16>,
    pub cloud: Option<String>,
    pub max_budget: Option<f64>,
    pub model_hf: Option<String>,
}

pub fn build_serve_argv(cfg: &MensServeConfig) -> Vec<String> {
    let mut argv = vec!["mens".to_string(), "serve".to_string()];
    if let Some(m) = &cfg.model { argv.push("--model".into()); argv.push(m.clone()); }
    if let Some(p) = cfg.port { argv.push("--port".into()); argv.push(p.to_string()); }
    let cloud = cfg.cloud.as_deref().unwrap_or("local");
    if cloud != "local" {
        argv.push("--cloud".into()); argv.push(cloud.to_string());
        if let Some(b) = cfg.max_budget { argv.push("--max-budget".into()); argv.push(b.to_string()); }
        if let Some(hf) = &cfg.model_hf { argv.push("--model-hf".into()); argv.push(hf.clone()); }
    }
    argv
}
```

Add `mens_serve_start(app, config) -> { run_id }` and `mens_serve_stop(app,
run_id)` mirroring train, emitting `MENS_SERVE_EVENT`. `serve` is a long-lived
process — spawn it detached via `.spawn()` (not `.output().await`) so the command
returns; emit a `{ phase: "serving", port }` frame. On spawn error, toast via the
returned `Err`. Persist run with `workflow_name = "mens-serve"`. Register in
`main.rs`:

```rust
            commands::mens::mens_serve_start,
            commands::mens::mens_serve_stop,
```

Run `cargo test -p vox-gui serve_argv` (green), `cargo build -p vox-gui`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/commands/mens.rs crates/vox-gui/src/main.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): mens_serve_start/stop streaming wrappers (vox://mens-serve)"
```

### Task P5A.4 — populi_up/down streaming command  [PARALLEL-SAFE]  [B4]

Dependency: P5A.1 (+ main.rs serialization note).

**Step P5A.4a — failing test.** Append to `populi.rs` tests:

```rust
#[test]
fn down_command_name_is_stable() {
    assert_eq!(populi_workflow_name(), "populi-mesh");
}
```

Run `cargo test -p vox-gui down_command_name` (expect fail).

**Step P5A.4b — implementation.** Add to `populi.rs`:
- `populi_workflow_name() -> &'static str { "populi-mesh" }`
- `#[tauri::command] populi_up(app, config: PopuliUpConfig) -> Result<{run_id},
  String>`: build argv via `build_up_argv`, spawn the sidecar **detached**
  (`.spawn()`), persist run (`workflow_name = "populi-mesh"`, `command =
  argv.join(" ")`), emit `POPULI_STATE_EVENT` `{ phase: "up", visibility }`.
- `#[tauri::command] populi_down(app) -> Result<(), String>`: run `vox populi
  down` via `.output().await`, emit `{ phase: "down" }`.

Register in `main.rs`:

```rust
            commands::populi::populi_up,
            commands::populi::populi_down,
```

Run `cargo test -p vox-gui down_command_name up_argv` (green), `cargo build -p
vox-gui`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/commands/populi.rs crates/vox-gui/src/main.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): populi_up/down streaming wrappers (vox://populi-state)"
```

### Task P5A.5 — transport listen helpers + types (TDD)  [PARALLEL-SAFE]  [B4]

Dependency: P5A.1 (only needs the event-name strings; independent of .2–.4).

**Step P5A.5a — failing test.** Create
`crates/vox-gui/ui/src/transport.mensPopuli.test.ts`:

```ts
import { describe, it, expect, vi } from 'vitest';

const listenMock = vi.fn(() => Promise.resolve(() => {}));
vi.mock('@tauri-apps/api/event', () => ({ listen: (...a: unknown[]) => (listenMock as any)(...a) }));

import {
  MENS_TRAIN_EVENT, MENS_SERVE_EVENT, POPULI_STATE_EVENT,
  listenMensTrain, listenMensServe, listenPopuliState,
} from './transport';

describe('mens/populi stream transport', () => {
  it('event names match the Rust consts', () => {
    expect(MENS_TRAIN_EVENT).toBe('vox://mens-train');
    expect(MENS_SERVE_EVENT).toBe('vox://mens-serve');
    expect(POPULI_STATE_EVENT).toBe('vox://populi-state');
  });

  it('listenMensTrain subscribes to its event', async () => {
    await listenMensTrain(() => {});
    expect(listenMock).toHaveBeenCalledWith('vox://mens-train', expect.any(Function));
  });

  it('listenPopuliState subscribes to its event', async () => {
    await listenPopuliState(() => {});
    expect(listenMock).toHaveBeenCalledWith('vox://populi-state', expect.any(Function));
  });
});
```

Run (expect fail — exports missing).

**Step P5A.5b — implementation.** Append to `crates/vox-gui/ui/src/transport.ts`
(it already imports `listen, UnlistenFn`):

```ts
/** Live frame from `mens_train_start` / `_stop` (see mens.rs MENS_TRAIN_EVENT). */
export const MENS_TRAIN_EVENT = 'vox://mens-train';
export interface MensTrainFrame { run_id: string; phase: string; ok?: boolean; stderr?: string }
export function listenMensTrain(onFrame: (f: MensTrainFrame) => void): Promise<UnlistenFn> {
  return listen<MensTrainFrame>(MENS_TRAIN_EVENT, (e) => onFrame(e.payload));
}

export const MENS_SERVE_EVENT = 'vox://mens-serve';
export interface MensServeFrame { run_id: string; phase: string; port?: number }
export function listenMensServe(onFrame: (f: MensServeFrame) => void): Promise<UnlistenFn> {
  return listen<MensServeFrame>(MENS_SERVE_EVENT, (e) => onFrame(e.payload));
}

export const POPULI_STATE_EVENT = 'vox://populi-state';
export interface PopuliStateFrame { phase: string; visibility?: string }
export function listenPopuliState(onFrame: (f: PopuliStateFrame) => void): Promise<UnlistenFn> {
  return listen<PopuliStateFrame>(POPULI_STATE_EVENT, (e) => onFrame(e.payload));
}
```

Run the file test (green), then `pnpm test`, `pnpm typecheck`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/transport.mensPopuli.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): transport listen helpers for mens-train/serve + populi-state streams"
```

---

## Phase 5B — Mens launch panels (train + serve forms)  [B5 lane A]

Dependency: P1.1 (MensView) + P5A.2/.3/.5. [PARALLEL-SAFE] with Phase 5C.

### Task P5B.1 — train/serve launch forms with live progress (TDD)  [PARALLEL-SAFE]

**Step P5B.1a — failing test.** Append to `MensView.test.tsx`. Add a second
mock for the streaming commands + listen helpers:

```tsx
import { vi } from 'vitest';
const listenMensTrainMock = vi.fn(() => Promise.resolve(() => {}));
vi.mock('../../../transport', async (orig) => ({
  ...(await orig<any>()),
  listenMensTrain: (cb: any) => listenMensTrainMock(cb),
  listenMensServe: () => Promise.resolve(() => {}),
}));

describe('MensView launch forms', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); listenMensTrainMock.mockClear(); });

  it('starts a LOCAL training run (no cloud/budget surfaced) via mens_train_start', async () => {
    invokeMock.mockImplementation((cmd: string) =>
      cmd === 'mens_train_start'
        ? Promise.resolve({ run_id: 'mens-train-1' })
        : Promise.resolve({ exit_code: 0, stdout: '', stderr: '' }));
    render(<MensView pushToast={vi.fn()} />);
    fireEvent.click(await screen.findByText('Start Training'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('mens_train_start', {
        config: expect.objectContaining({ preset: 'safe', domain: 'vox-lang', cloud: 'local' }),
      }));
  });

  it('subscribes to live train progress on mount', async () => {
    render(<MensView pushToast={vi.fn()} />);
    await waitFor(() => expect(listenMensTrainMock).toHaveBeenCalled());
  });
});
```

Run (expect fail).

**Step P5B.1b — implementation.** Add a "New Training Run" panel + "Serve Model"
panel to `MensView.tsx`:

- **Train form** controls (all controlled React state): `<select aria-label="Preset">`
  (`tiny`/`safe`/`4080`/`a100`/`auto`, default `safe`), `<select aria-label="Spoke
  (domain)">` (the 5 spokes, default `vox-lang`), `<select aria-label="Device">`
  (`auto`/`cuda`/`cpu`), and a **Cloud** group: `<select aria-label="Cloud
  target">` (`local`/`runpod`/`vast`, default `local`) + `<input aria-label="Max
  budget USD">` that is **disabled and hidden when cloud === 'local'** (no spend
  in local mode). A **Start Training** `type="button"` calls
  `invoke('mens_train_start', { config: { preset, domain, device, cloud,
  max_budget } })`, stores the returned `run_id`, and shows a **Stop** button
  that calls `invoke('mens_train_stop', { run_id })`.
- Subscribe via `listenMensTrain` in a `useEffect` (with cleanup) and render the
  latest frame's `phase` in a live status line; on listen failure, fall back to
  polling `runVoxCommand(['mens','watch-telemetry'])` every
  `ORCH_POLL_FALLBACK_MS` (import the constant from `../../../config/constants`),
  mirroring `useOrchestratorStatus`.
- **Serve form**: `<input aria-label="Model checkpoint path">`, `<input
  aria-label="Serve port">` (default 8089), the same Cloud group, **Start
  Server** → `invoke('mens_serve_start', {...})`, **Stop Server** →
  `invoke('mens_serve_stop', { run_id })`; subscribe via `listenMensServe`.

When `cloud !== 'local'`, render the cost ribbon (Phase 5D) inline above the
Start button — no confirm popup (no-nag).

Run the file test (green), then `pnpm test`, `pnpm typecheck`, honesty guard.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Mens/MensView.tsx crates/vox-gui/ui/src/components/surfaces/Mens/MensView.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): MensView train/serve launch forms with live vox:// progress"
```

---

## Phase 5C — Populi mesh power toggle (up/down)  [B5 lane B]

Dependency: P3.1 (PopuliView) + P5A.4/.5. [PARALLEL-SAFE] with Phase 5B.

### Task P5C.1 — mesh power toggle with live state (TDD)  [PARALLEL-SAFE]

**Step P5C.1a — failing test.** Append to `PopuliView.test.tsx`:

```tsx
const listenPopuliStateMock = vi.fn(() => Promise.resolve(() => {}));
vi.mock('../../../transport', async (orig) => ({
  ...(await orig<any>()),
  listenPopuliState: (cb: any) => listenPopuliStateMock(cb),
}));

describe('PopuliView power toggle', () => {
  beforeEach(() => { cleanup(); invokeMock.mockClear(); listenPopuliStateMock.mockClear(); });

  it('brings the mesh up via populi_up with selected visibility', async () => {
    invokeMock.mockImplementation((cmd: string) =>
      cmd === 'populi_up' ? Promise.resolve({ run_id: 'populi-mesh-1' })
        : Promise.resolve({ exit_code: 0, stdout: '', stderr: '' }));
    render(<PopuliView pushToast={vi.fn()} />);
    fireEvent.click(await screen.findByText('Bring Mesh Up'));
    await waitFor(() =>
      expect(invokeMock).toHaveBeenCalledWith('populi_up', {
        config: expect.objectContaining({ visibility: 'private', public_mesh: false }),
      }));
  });

  it('subscribes to live mesh state', async () => {
    render(<PopuliView pushToast={vi.fn()} />);
    await waitFor(() => expect(listenPopuliStateMock).toHaveBeenCalled());
  });
});
```

Run (expect fail).

**Step P5C.1b — implementation.** Add a "Mesh Power" panel to `PopuliView.tsx`:
- `<select aria-label="Mesh visibility">` (`private`/`public`/`hybrid`, default
  `private`), a `<input type="checkbox" aria-label="Process public mesh tasks">`
  (default off), a **Bring Mesh Up** `type="button"` →
  `invoke('populi_up', { config: { visibility, public_mesh, bind: undefined } })`
  and a **Bring Mesh Down** `type="button"` → `invoke('populi_down')`.
- Subscribe via `listenPopuliState` (cleanup on unmount); render the latest
  `phase`/`visibility` in a live status line. On listen failure fall back to
  polling `runVoxCommand(['populi','status'], ['--json'])` every
  `ORCH_POLL_FALLBACK_MS`.

Run the file test (green), then `pnpm test`, `pnpm typecheck`, honesty guard.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.tsx crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.test.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): PopuliView mesh power toggle with live vox://populi-state"
```

---

## Phase 5D — no-nag cost ribbon + gamification  [B6]

Dependency: P5B.1 + P5C.1. [SEQUENTIAL]. Opencode no-nag model: cloud jobs launch
WITHOUT confirmation popups; spend is always-visible inline and gamified.

### Task P5D.1 — CostRibbon component (TDD)  [SEQUENTIAL]

**Step P5D.1a — failing test.** Create
`crates/vox-gui/ui/src/components/surfaces/lib/CostRibbon.test.tsx`:

```tsx
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, cleanup, waitFor } from '@testing-library/react';
import React from 'react';

const getSpendMock = vi.fn(() => Promise.resolve({
  sessionUsd: 1.25, dayUsd: 4.5, totalUsd: 42.0,
  dailyBudgetUsd: 10, perSessionBudgetUsd: 5,
}));
vi.mock('../../../../transport', async (orig) => ({
  ...(await orig<any>()),
  voxTransport: { getLlmSpend: () => getSpendMock() },
}));

import { CostRibbon } from './CostRibbon';

describe('CostRibbon', () => {
  beforeEach(() => { cleanup(); getSpendMock.mockClear(); });

  it('shows session spend against the per-session cap with no confirm popup', async () => {
    render(<CostRibbon active />);
    await waitFor(() => expect(screen.getByText(/\$1\.25/)).toBeTruthy());
    expect(screen.getByText(/\$5/)).toBeTruthy(); // per-session cap
    // no dialog/confirm element rendered
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('renders nothing when not active (local job)', () => {
    const { container } = render(<CostRibbon active={false} />);
    expect(container.firstChild).toBeNull();
  });
});
```

Run (expect fail).

**Step P5D.1b — implementation.** Create
`crates/vox-gui/ui/src/components/surfaces/lib/CostRibbon.tsx`. It reuses the
existing `voxTransport.getLlmSpend()` SSOT (the same BudgetManager-backed
`LlmSpendDto`) and the `useLlmSpend` polling cadence pattern:

```tsx
import React, { useEffect, useState } from 'react';
import { voxTransport } from '../../../transport';
import { recordGamifyGuiEvent } from '../../../lib/gamifyGuiEvents';

interface Spend {
  sessionUsd: number; dayUsd: number; totalUsd: number;
  dailyBudgetUsd: number; perSessionBudgetUsd: number;
}

/** Always-visible, no-nag cloud-spend ribbon. Renders only for cloud jobs. */
export function CostRibbon({ active, gamifyEnabled }: { active: boolean; gamifyEnabled?: boolean }) {
  const [spend, setSpend] = useState<Spend | null>(null);
  useEffect(() => {
    if (!active) return;
    let cancelled = false;
    const refresh = async () => {
      try {
        const s = (await voxTransport.getLlmSpend()) as unknown as Spend;
        if (!cancelled) setSpend(s);
      } catch { if (!cancelled) setSpend(null); }
    };
    void refresh();
    const id = window.setInterval(refresh, 30_000);
    // gamify: surface a cost-awareness pip when a cloud job is active
    void recordGamifyGuiEvent('cloud_cost_visible', undefined, { enabled: !!gamifyEnabled });
    return () => { cancelled = true; window.clearInterval(id); };
  }, [active, gamifyEnabled]);

  if (!active) return null;
  const sessionUsd = spend?.sessionUsd ?? 0;
  const cap = spend?.perSessionBudgetUsd ?? 0;
  const pct = cap > 0 ? Math.min(100, Math.round((sessionUsd / cap) * 100)) : 0;
  return (
    <div role="status" aria-label="Cloud spend" className="ds-cost-ribbon">
      <span>Session spend: ${sessionUsd.toFixed(2)} / ${cap.toFixed(0)} cap</span>
      <span aria-hidden> · {pct}%</span>
      <span> · Today ${ (spend?.dayUsd ?? 0).toFixed(2) }</span>
    </div>
  );
}
```

Signature (verified `lib/gamifyGuiEvents.ts:38`):
`recordGamifyGuiEvent(eventType, metadata?, options?: { enabled?: boolean })` —
the call above (`'cloud_cost_visible', undefined, { enabled }`) matches exactly;
passing `{ enabled: false }` no-ops, so the gamify pip respects the surface's
`gamifyEnabled` flag. Then wire `<CostRibbon active={cloud !== 'local'}
gamifyEnabled={gamifyEnabled} />` into the train + serve forms (MensView) and pass
`gamifyEnabled` from `SurfaceDecoratorProps` into both views (update the two view
signatures to accept `gamifyEnabled`).

Run the file test (green), then `pnpm test`, `pnpm typecheck`, honesty guard.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/lib/CostRibbon.tsx crates/vox-gui/ui/src/components/surfaces/lib/CostRibbon.test.tsx crates/vox-gui/ui/src/components/surfaces/Mens/MensView.tsx crates/vox-gui/ui/src/components/surfaces/Populi/PopuliView.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): no-nag CostRibbon (BudgetManager spend SSOT) + gamify pip on cloud jobs"
```

---

## Phase 6 — register decorators, promote registry, verify  [B7]

Dependency: all prior. [SEQUENTIAL].

### Task P6.1 — register MensView/PopuliView decorators (TDD)

**Step P6.1a — failing test.** Create
`crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { surfaceDecorators } from './decoratorRegistry';
import { MensView } from './Mens/MensView';
import { PopuliView } from './Populi/PopuliView';

describe('decoratorRegistry mens/populi', () => {
  it('maps mens to MensView', () => expect(surfaceDecorators.mens).toBe(MensView));
  it('maps populi to PopuliView', () => expect(surfaceDecorators.populi).toBe(PopuliView));
});
```

Run (expect fail — still `commandSurface` closures).

**Step P6.1b — implementation.** In `decoratorRegistry.ts`: add imports for
`MensView` and `PopuliView`, replace the `mens: commandSurface(...)` and `populi:
commandSurface(...)` entries with `mens: MensView,` and `populi: PopuliView,`.

Run the registry test (green), `pnpm test`, honesty guard green.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): register MensView/PopuliView decorators; retire command-cards"
```

### Task P6.2 — promote surface registry tier to live_backend

**Step P6.2a.** Edit `contracts/gui/surface-registry.v1.yaml`: for the `mens` and
`populi` entries in the `compute` group, change `tier: curated_decorator` →
`tier: live_backend` (we now ship streaming backends). Then regenerate:

```
vox ci gui-surface-registry --write
```

Expected: the generated registry consumer (`gui_surface_registry.rs` /
`generated` TS) updates with no drift error; command exits 0.

**Step P6.2b.** Run `pnpm test` (any registry snapshot test stays green) and
`cargo build -p vox-gui`.

**Commit:**
```
git -C /c/Users/Owner/vox-graphify-gui add contracts/gui/surface-registry.v1.yaml
git -C /c/Users/Owner/vox-graphify-gui commit -m "chore(gui): promote mens/populi surfaces to live_backend tier"
```

(If `--write` also regenerated tracked files, add those exact paths to the same
commit.)

### Task P6.3 — full verification (no commit)

Run, in `crates/vox-gui/ui/`:
- `pnpm typecheck` — clean.
- `pnpm test` — full vitest suite green, including: `lib/runVoxCommand.test.ts`,
  `Mens/MensView.test.tsx`, `Populi/PopuliView.test.tsx`,
  `transport.mensPopuli.test.ts`, `lib/CostRibbon.test.tsx`,
  `decoratorRegistry.test.ts`, and `__guards__/surfaceHonesty.guard.test.ts`.

Run, from repo root:
- `cargo test -p vox-gui` — green (argv builders, workflow-name pins).
- `cargo build -p vox-gui` — compiles with the new handler registrations.

Record pass counts in the execution summary. **No commit** — report results.

---

## Self-Review — spec coverage

Mapping each spec §4 row to a task in this FULL plan (no v2 deferral remains).

### mens surface (spec §4)

| Spec row | Coverage | Wire |
|---|---|---|
| `mens probe [-d]` | P1.1 GPU Probe panel | exec |
| `mens status [--quotas/--cloud/--db]` | P1.1 Status/Quotas/Cloud/DB panels | exec |
| `mens models` | P1.1 Trained Models panel (+ link to Models) | exec |
| `mens corpus stats/readiness/eval` | P1.1 (stats/readiness/eval) | exec |
| `mens corpus mix/validate` | P2.1 (mix + validate) | exec |
| `mens train` / `mens dogfood` | **P5A.2 + P5B.1** (streaming launch form, hub+spoke via preset/domain) | **stream** |
| `mens watch-telemetry` | P5B.1 (live frame + polling fallback) | stream/poll |
| `mens serve` | **P5A.3 + P5B.1** (serve form + toggle) | **stream** |
| `mens eval-local`/`eval-gate`/`baseline` | corpus `eval` covered (P1.1); checkpoint-picker eval-local = noted follow-up | partial |
| `mens merge-qlora`/`export-gguf` | follow-up (export form) — not blocking | follow-up |
| `mens pipeline` | follow-up (wizard over the same exec seam) | follow-up |

### populi surface (spec §4)

| Spec row | Coverage | Wire |
|---|---|---|
| `populi status`/`stats`/`registry-snapshot` | P3.1 panels | exec |
| `populi config show`/`check` | P3.1 Config panels | exec |
| `populi init` | P4.1 | exec |
| `populi up`/`down` | **P5A.4 + P5C.1** (mesh power toggle + live state) | **stream** |
| `populi node list` | P3.1 Nodes panel | exec |
| `populi federation list` | P3.1 Federation panel | exec |
| `populi identity show`/`reputation` | P3.1 Identity/Reputation panels | exec |
| `populi identity export` + keys | **routed to Settings/Secrets (Plan 3C)** — link only | Plan 3C |
| `populi admin maintenance/quarantine/exec-lease-revoke` | P4.1 (confirm-gated) | exec (confirm) |
| `populi serve --enable` | follow-up (control-plane toggle, same stream pattern) | follow-up |
| `populi dispatch`/`result` | follow-up (script picker + poll) | follow-up |
| `trust/untrust mesh node` | existing `trust_mesh_node` tauri cmds (Mesh surface) — not duplicated | tauri✓ |

### Ratified-decision checklist (FULL plan)

- [x] **Full launch + monitor, NO v2 deferral** — streaming wrappers
  (P5A.2–.4) + launch UI (P5B/P5C) shipped in this plan.
- [x] **4–5 streaming Tauri wrappers** emitting `vox://…` à la
  `useOrchestratorStatus`: `mens_train_start/stop`, `mens_serve_start/stop`,
  `populi_up/down` + transport listen helpers (P5A.2/.3/.4/.5).
- [x] **Read + light-action coverage** from prior 3B preserved (P1–P4).
- [x] **Opencode-style no-nag cost tracking + gamification** for cloud jobs,
  tied to `BudgetManager`/`get_llm_spend` SSOT (P5D — `CostRibbon`, no confirm
  popup, gamify pip).
- [x] **Identity/keys routed to central Settings/Secrets** — no key UI here,
  Plan 3C cross-ref (P3.1 note).
- [x] **Admin ops confirm-gated** (P4.1 two-click confirm).
- [x] De-Latinized labels "Model Lab" / "Mesh"; `models` kept distinct.
- [x] Honesty guard green — every control wires a real seam; disabled buttons
  when inputs empty; no placeholder prose.
- [x] Cloud flags (`--cloud`, `--max-budget`) surfaced ONLY in cloud mode; local
  is the default and surfaces no budget field (no accidental spend).
- [x] Registry promoted `curated_decorator → live_backend` (P6.2) because
  streaming backends now exist.

### Open questions resolved by ratification (spec §7)

- Q1 (local vs RunPod): **both** — local default; cloud (`runpod`/`vast`)
  selectable with always-visible no-nag cost ribbon (P5B/P5D).
- Q2 (launch vs monitor): **launch + monitor** (this plan) — streaming wrappers
  built now.
- Q3 (populi vs mens corpus): canonical under `mens`; `populi corpus` not
  duplicated.
- Q4 (`models` provenance): keep distinct; tagged registry = follow-up.
- Q5 (admin/identity gating): confirm-gated (P4.1); `identity export` → Plan 3C.
- Q6 (spoke ladder UX): hub+spoke exposed first-class on the train form
  (preset + spoke selectors, P5B.1).

### Workflow-readiness checklist

- [x] Every task tagged [PARALLEL-SAFE] or [SEQUENTIAL].
- [x] Fan-out batches B2/B3 (Mens lane ⟂ Populi lane) and B4 (4 wrappers) and B5
  (2 launch UIs) defined explicitly.
- [x] Cross-plan deps stated up top (Plan 3C secrets; surface-registry SSOT).
- [x] Every task ends in a concrete STRICT add+commit (except the explicit
  verification-only P6.3).
- [x] Shared-file edits (`main.rs` handler list, `execute.rs` const,
  `decoratorRegistry.ts`) flagged with serialization guidance so a workflow does
  not race them.
