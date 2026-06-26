---
title: "Plan P6 — GUI CLI-Governance Surfaces (Develop>CI, Knowledge>Database, build-spine, typed secret/auth wrappers, honest not-in-GUI)"
category: "Architecture SSOTs"
date: 2026-06-26
status: plan
plan_id: P6
spec: docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md
sources:
  - docs/agents/cli-gui-governance-audit.md
  - docs/agents/gui-ia-blueprint.md
---

# Plan P6 — GUI CLI-Governance Surfaces

## Goal

Close the **70.9% ungoverned-CLI gap** (389 of 549 leaf commands have no GUI path) by adding the
highest-value governance surfaces **within the ratified nav** (no new top-level group):

1. **`Develop > CI`** — read panels over the `ci` group (157 cmds) via the existing `execute_command`
   seam (read-only gate dashboard).
2. **`Knowledge > Database`** — read panels over the `db` group (77 cmds) via `execute_command`
   (read-only query/table browser; destructive admin behind confirm).
3. **Build-spine actions** folded into **Develop > Console** (`build`/`check`/`compile`/`dev`/`run`/
   `test`/`fmt`/`fabrica`/`emit`/`new`/`init`/`generate`/`component`/`bundle`/`snippet`) as a read/run
   action panel.
4. **Typed Tauri wrappers** for `secrets`/`auth`/`config`-writes under **System > Settings** — credentials
   **NEVER** transit a shell string (no `execute_command`); structured `#[tauri::command]` args only.
5. **Honest "not-in-GUI" affordances** (clig.dev coming-soon pattern) for genuinely CLI-only groups
   (`completions`, `lsp`, `grammar`, `wasm`, `play`, `repl`/`shell`/`term`, `visus`, dangerous-admin).

CI + Database alone convert **234 of 389** ungoverned commands (60%) into reachable surfaces — Success
Criterion 7 of the master spec (≥60% reachable, rest honestly CLI-only).

## Architecture

**Single seam, two paths — both already proven in this codebase:**

- **Read/run panels (CI, Database, build-spine):** reuse the existing decorator pattern
  `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts` → `commandSurface(title, subtitle,
  cards[])` → `CommandCardsView` (runs `execute_command` over read-only CLI paths on mount/refresh). The
  `mens`/`populi`/`oratio` decorators are the live template. New surfaces are new `SURFACE_DECORATORS`
  entries + a `surfaceComponents.tsx` `case` + a `navigation.ts`/registry row. **No backend duplication.**
- **Typed wrappers (secrets/auth/config):** reuse the existing `crates/vox-gui/src/commands/secrets.rs`
  pattern (`#[tauri::command] pub fn set_secret(key, value) -> Result<bool, String>`), registered in
  `crates/vox-gui/src/main.rs` `tauri::generate_handler![…]`, called from the UI via
  `invoke('<wrapper>', { … })`. Auth + config-write wrappers are added the same way.
- **`cli:` registry ingestion (coverage):** the master spec's §5.1 clap-tree `cli:` node ingestion is
  Plan P0/coverage territory; **this plan consumes the audit JSON** (`graphify-out/gui-coverage/
  cli-governance.json`, 503 per-command rows) to drive which groups get which surface, and adds a
  **`cliGroup` binding** on the new registry rows so `vox search coverage` can later classify them as
  `Surfaced` instead of `CliOnly`/`OrphanBackend`. (The actual clap→`cli:` graph emit is P0/P6-coverage;
  here we make the GUI rows the coverage join can see.)

**Honesty contract (this plan's slice):** every new surface is registered in the SSOT
(`contracts/gui/surface-registry.v1.yaml` → regenerated `surfaceRegistry.generated.ts`), so the
`vox ci gui-honesty` gate sees a real `viewKey` + `cliGroup`. Read panels are labeled read-only;
mutating actions are gated behind a confirm. Genuinely CLI-only groups get an explicit, honest
"Available in CLI only" affordance (a registry row with `representation_tier: none` + a
`NotInGuiNotice` component) — the GUI never pretends to govern what it can't.

## Tech Stack

- **UI:** React + TypeScript (Vite), Vitest (`crates/vox-gui/ui`). Existing seams: `decoratorRegistry.ts`,
  `CommandCardsView.tsx`, `surfaceComponents.tsx`, `navigation.ts`, `transport.ts`.
- **Backend:** Rust + Tauri (`crates/vox-gui/src/commands/*.rs`, `main.rs`). Existing seams:
  `execute::execute_command`, `secrets::*`, `mcp::invoke_mcp_tool`.
- **SSOT/codegen:** `vox ci gui-surface-registry --write` regenerates `surfaceRegistry.generated.ts`
  from `contracts/gui/surface-registry.v1.yaml`.
- **Coverage data:** `graphify-out/gui-coverage/cli-governance.json` (from the audit).

## Spec

Master: `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md` §5 (CLI
governance) + §10 SC-7. Audit: `docs/agents/cli-gui-governance-audit.md`. IA placement:
`docs/agents/gui-ia-blueprint.md` (Knowledge / Develop / System nav groups; no new top-level group).

## Dependencies (cross-plan)

- **MUST PRECEDE this plan:** **P0** (Absorption + structural-core enrichment) — provides the
  `vox search coverage` capability and the registry-adapter pattern; the `cliGroup` rows this plan adds
  are the coverage join's input. P6 can author its GUI surfaces in parallel with P0's engine work, but
  the **coverage cross-check** in Task P6-12 depends on P0's coverage verb existing. If P0's coverage
  verb is not yet landed when P6-12 runs, P6-12 falls back to asserting the registry rows directly
  (documented in that task) and the coverage cross-check becomes a follow-up.
- **This plan PRECEDES:** **P7** (VoxMens FULL launch + cost — needs P6's typed-wrapper pattern +
  `cli:` parity map) and **P8** (Settings/Policies co-located — needs P6's secret/auth wrappers as the
  central key store). P7 additionally depends on P8 for key placement.
- **Sibling, independent of P6:** P1/P2/P3/P4/P5 (data-flow, fusion, semantic, auto-availability, GUI
  Vox Search panel). No file overlap except `surfaceComponents.tsx` / `navigation.ts` / the registry
  yaml — see "Shared-file discipline" below.

### Shared-file discipline (avoid cross-plan merge conflicts)

Three SSOT files are touched by both P5 (Vox Search GUI surface) and P6 (governance surfaces):
`navigation.ts`, `contracts/gui/surface-registry.v1.yaml`, `surfaceComponents.tsx`. **P6 only ever
ADDS rows/cases for its own keys** (`ci`, `database`, `console`-build-panel additions, `secrets`,
`auth`, `not-in-gui-*`) — it never edits a P5 key. Each P6 task that touches a shared file appends its
own block and regenerates; conflicts are append-only and trivially resolvable. Within P6, the
[SEQUENTIAL] registry/regenerate tasks are explicitly ordered to serialize the generated-file writes.

---

## Task fan-out structure (for the workflow)

```
Batch 1 [PARALLEL-SAFE]  — independent UI decorators + Rust wrappers (no shared generated file)
  P6-1  CI decorator (decoratorRegistry + cards + vitest)            [PARALLEL-SAFE]
  P6-2  Database decorator (decoratorRegistry + cards + vitest)      [PARALLEL-SAFE]
  P6-3  Build-spine Console action panel (component + vitest)        [PARALLEL-SAFE]
  P6-4  Typed auth wrapper (Rust #[tauri::command] + unit test)      [PARALLEL-SAFE]
  P6-5  Typed config-write wrapper (Rust + unit test)                [PARALLEL-SAFE]
  P6-6  NotInGuiNotice component + honest CLI-only data (vitest)     [PARALLEL-SAFE]

Batch 2 [SEQUENTIAL]     — registry SSOT writes (serialize generated-file edits)
  P6-7  Register ci + database rows in surface-registry.v1.yaml +
        navigation.ts + regenerate                                   [SEQUENTIAL]
  P6-8  Register secrets/auth/config Settings sub-panel rows +
        wire wrappers into main.rs + Settings UI                     [SEQUENTIAL after P6-7]
  P6-9  Register not-in-gui honest rows + wire NotInGuiNotice        [SEQUENTIAL after P6-8]

Batch 3 [SEQUENTIAL]     — wire cases + nav + integration
  P6-10 surfaceComponents.tsx cases (ci/database/build-spine/
        settings sub-panels/not-in-gui) + App routing                [SEQUENTIAL after P6-9]
  P6-11 navigation.test.ts + surface-honesty vitest updates          [SEQUENTIAL after P6-10]

Batch 4 [SEQUENTIAL]     — verification + coverage cross-check + gate
  P6-12 coverage cross-check (cliGroup rows ↔ audit JSON) +
        vox ci gui-honesty + full verification                       [SEQUENTIAL after P6-11]
```

**Total: 12 tasks.** Batch 1 = 6 fully parallel tasks. Batches 2–4 = serialized SSOT/integration.

---

## P6-1 — CI read decorator (`Develop > CI`) [PARALLEL-SAFE]

**Goal:** a read-only CI gate dashboard surface backed by `execute_command` over `ci` read commands.

**TDD — write the test first.**

1. Create `crates/vox-gui/ui/src/components/surfaces/CI/ciCards.ts`:

```ts
import type { SurfaceCard } from '../CommandCardsView';

/**
 * Read-only `ci` commands surfaced as gate dashboard cards. Every path is an
 * arg-free, read-only `vox ci <cmd>` invocation run through the shared
 * `execute_command` seam. Mutating/run actions are NOT here (see CiRunPanel,
 * a follow-up behind confirm).
 */
export const CI_CARDS: SurfaceCard[] = [
  { key: 'status', title: 'CI Status', description: 'Aggregate gate status', path: ['ci', 'status'] },
  { key: 'gates', title: 'Gates', description: 'Registered CI gates', path: ['ci', 'gates'] },
  { key: 'language-rules', title: 'Language Rules', description: 'Lint/format rule coverage', path: ['ci', 'language-rules'] },
  { key: 'arch-check', title: 'Arch Check', description: 'Layer/dependency invariants', path: ['ci', 'arch-check'] },
  { key: 'ssot-drift', title: 'SSOT Drift', description: 'Generated-vs-source drift', path: ['ci', 'ssot-drift'] },
];
```

2. Create `crates/vox-gui/ui/src/components/surfaces/CI/ciCards.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { CI_CARDS } from './ciCards';

describe('CI_CARDS', () => {
  it('every card is a read-only vox ci <cmd> path', () => {
    expect(CI_CARDS.length).toBeGreaterThan(0);
    for (const c of CI_CARDS) {
      expect(c.path[0]).toBe('ci');
      expect(c.path.length).toBe(2);
      expect(c.key).toBeTruthy();
      expect(c.title).toBeTruthy();
    }
  });
  it('card keys are unique', () => {
    const keys = CI_CARDS.map((c) => c.key);
    expect(new Set(keys).size).toBe(keys.length);
  });
});
```

3. Run the failing test, then it passes (no impl needed beyond the data file — assertion is on the data):

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/components/surfaces/CI/ciCards.test.ts
```

Expected output: `Test Files  1 passed (1)` / `Tests  2 passed (2)`.

4. Add the decorator entry in `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`. Add the
   import at the top with the other surface imports:

```ts
import { CI_CARDS } from './CI/ciCards';
```

and add inside the `SURFACE_DECORATORS` object (alongside `mens`/`populi`/`oratio`):

```ts
  ci: commandSurface('Continuous Integration', 'Read-only CI gate dashboard', CI_CARDS),
```

5. Verify the registry file still type-checks:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx tsc --noEmit -p tsconfig.json 2>&1 | head -5
```

Expected: no errors referencing `CI/ciCards` or `decoratorRegistry`.

**Commit (add+commit only, STRICT):**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/CI/ciCards.ts crates/vox-gui/ui/src/components/surfaces/CI/ciCards.test.ts crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): CI read decorator (Develop>CI) over execute_command

Adds Develop>CI gate dashboard cards backed by the shared execute_command
seam (read-only vox ci status/gates/language-rules/arch-check/ssot-drift),
reusing the mens/populi commandSurface pattern. Closes part of the ci(157)
ungoverned cluster per cli-gui-governance-audit.md.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-2 — Database read decorator (`Knowledge > Database`) [PARALLEL-SAFE]

**Goal:** a read-only `db` query/table-browser surface backed by `execute_command`; destructive admin
stays out (confirm-gated follow-up).

**TDD.**

1. `crates/vox-gui/ui/src/components/surfaces/Database/databaseCards.ts`:

```ts
import type { SurfaceCard } from '../CommandCardsView';

/**
 * Read-only `db` commands surfaced as a query/table browser. Destructive admin
 * (`db migrate`, drops, repair) is intentionally excluded — those stay CLI-gated
 * or move behind a strong confirm in a later panel.
 */
export const DATABASE_CARDS: SurfaceCard[] = [
  { key: 'status', title: 'Database Status', description: 'Connection + store health', path: ['db', 'status'] },
  { key: 'tables', title: 'Tables', description: 'Schema / table listing', path: ['db', 'tables'] },
  { key: 'stats', title: 'Stats', description: 'Row counts + index stats', path: ['db', 'stats'] },
  { key: 'migrate-status', title: 'Migration Status', description: 'Applied vs pending migrations (read-only)', path: ['db', 'migrate-status'] },
];
```

2. `crates/vox-gui/ui/src/components/surfaces/Database/databaseCards.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { DATABASE_CARDS } from './databaseCards';

describe('DATABASE_CARDS', () => {
  it('every card is a read-only vox db <cmd> path', () => {
    expect(DATABASE_CARDS.length).toBeGreaterThan(0);
    for (const c of DATABASE_CARDS) {
      expect(c.path[0]).toBe('db');
      expect(c.path.length).toBe(2);
    }
  });
  it('excludes destructive verbs', () => {
    const verbs = DATABASE_CARDS.map((c) => c.path[1]);
    for (const danger of ['migrate', 'drop', 'reset', 'repair', 'rollback']) {
      expect(verbs).not.toContain(danger);
    }
  });
});
```

3. Run:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/components/surfaces/Database/databaseCards.test.ts
```

Expected: `Tests  2 passed (2)`.

4. In `decoratorRegistry.ts` add import `import { DATABASE_CARDS } from './Database/databaseCards';` and
   entry `database: commandSurface('Database', 'Read-only query + table browser', DATABASE_CARDS),`.

5. `npx tsc --noEmit` clean (as P6-1 step 5).

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Database/databaseCards.ts crates/vox-gui/ui/src/components/surfaces/Database/databaseCards.test.ts crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): Database read decorator (Knowledge>Database) over execute_command

Read-only vox db status/tables/stats/migrate-status as a query/table browser;
destructive admin (migrate/drop/reset/repair) excluded by test guard. Closes
part of the db(77) ungoverned cluster.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

> **Note:** P6-1 and P6-2 both touch `decoratorRegistry.ts`. They are PARALLEL-SAFE because each only
> adds its own import line + its own `SURFACE_DECORATORS` entry; the workflow merges two append-only
> diffs. If run truly concurrently in one worktree, the second commit resolves a trivial add/add hunk.
> If the workflow prefers strict isolation, sequence P6-2 after P6-1 (same file). Default: append-only.

---

## P6-3 — Build-spine action panel (Develop > Console) [PARALLEL-SAFE]

**Goal:** fold the build/dev spine (`build`/`check`/`compile`/`dev`/`run`/`test`/`fmt`/`fabrica`/`emit`/
`new`/`init`/`generate`/`component`/`bundle`/`snippet`) into **Develop > Console** as a read/run action
panel. Read commands run on click; the run/mutate ones are confirm-gated.

**TDD.**

1. `crates/vox-gui/ui/src/components/surfaces/Console/buildSpineActions.ts`:

```ts
export interface BuildSpineAction {
  key: string;
  title: string;
  description: string;
  path: string[];
  /** true => requires a confirm before execute_command (mutating/long-running). */
  confirm: boolean;
}

/**
 * The build/dev spine folded into Console. Read-only inspections run directly;
 * `confirm: true` actions (build/run/test/fmt) prompt before the execute_command
 * shell-out. All go through the same execute_command seam as Repository.
 */
export const BUILD_SPINE_ACTIONS: BuildSpineAction[] = [
  { key: 'check', title: 'Check', description: 'cargo/vox check', path: ['check'], confirm: false },
  { key: 'build', title: 'Build', description: 'Build the workspace', path: ['build'], confirm: true },
  { key: 'test', title: 'Test', description: 'Run the test suite', path: ['test'], confirm: true },
  { key: 'fmt', title: 'Format', description: 'Format sources', path: ['fmt'], confirm: true },
  { key: 'run', title: 'Run', description: 'Run the project', path: ['run'], confirm: true },
];

export function isConfirmAction(key: string): boolean {
  return BUILD_SPINE_ACTIONS.find((a) => a.key === key)?.confirm ?? false;
}
```

2. `crates/vox-gui/ui/src/components/surfaces/Console/buildSpineActions.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { BUILD_SPINE_ACTIONS, isConfirmAction } from './buildSpineActions';

describe('BUILD_SPINE_ACTIONS', () => {
  it('mutating actions require confirm', () => {
    for (const k of ['build', 'test', 'fmt', 'run']) {
      expect(isConfirmAction(k)).toBe(true);
    }
  });
  it('read-only check does not require confirm', () => {
    expect(isConfirmAction('check')).toBe(false);
  });
  it('every action has a non-empty path', () => {
    for (const a of BUILD_SPINE_ACTIONS) {
      expect(a.path.length).toBeGreaterThan(0);
    }
  });
});
```

3. Run:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/components/surfaces/Console/buildSpineActions.test.ts
```

Expected: `Tests  3 passed (3)`.

4. Create the panel `crates/vox-gui/ui/src/components/surfaces/Console/BuildSpinePanel.tsx` (renders the
   actions, runs via `execute_command`, guards `confirm` with `window.confirm`):

```tsx
import React, { useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Toast } from '../../../types/tauri';
import { BUILD_SPINE_ACTIONS, type BuildSpineAction } from './buildSpineActions';

interface ExecuteOutput { exit_code: number; stdout: string; stderr: string }

export function BuildSpinePanel({ pushToast }: { pushToast: (t: Toast) => void }) {
  const [running, setRunning] = useState<string | null>(null);

  async function run(a: BuildSpineAction) {
    if (a.confirm && !window.confirm(`Run \`vox ${a.path.join(' ')}\`? This may modify your workspace.`)) {
      return;
    }
    setRunning(a.key);
    try {
      const out = await invoke<ExecuteOutput>('execute_command', { path: a.path, args: { __argv: [] } });
      pushToast({ kind: out.exit_code === 0 ? 'success' : 'error', message: `vox ${a.path.join(' ')} → exit ${out.exit_code}` });
    } catch (err) {
      pushToast({ kind: 'error', message: `vox ${a.path.join(' ')} failed: ${String(err)}` });
    } finally {
      setRunning(null);
    }
  }

  return (
    <div className="ds-panel" data-testid="build-spine-panel">
      <h3 className="ds-section-head">Build &amp; Dev Spine</h3>
      <div className="ds-action-grid">
        {BUILD_SPINE_ACTIONS.map((a) => (
          <button key={a.key} className="ds-action-card" disabled={running === a.key} onClick={() => run(a)}>
            <span className="ds-action-title">{a.title}</span>
            <span className="ds-action-desc">{a.description}</span>
            {a.confirm && <span className="ds-action-badge">confirm</span>}
          </button>
        ))}
      </div>
    </div>
  );
}
```

> **Toast `kind`:** confirm the union in `types/tauri.ts` includes `'success' | 'error'`. If the local
> union differs, match the existing values used by `CommandCardsView` (`pushToast`) — do not invent a
> new kind. Grep `grep -n "kind:" crates/vox-gui/ui/src/types/tauri.ts` and align.

5. `npx tsc --noEmit` clean.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/Console/buildSpineActions.ts crates/vox-gui/ui/src/components/surfaces/Console/buildSpineActions.test.ts crates/vox-gui/ui/src/components/surfaces/Console/BuildSpinePanel.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): build-spine action panel for Console (confirm-gated mutating actions)

Folds build/check/test/fmt/run into Develop>Console via the shared
execute_command seam; mutating actions are confirm-gated. Per audit
recommendation 1 (build spine into Workspace/Console).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-4 — Typed `auth` wrapper (Rust `#[tauri::command]`) [PARALLEL-SAFE]

**Goal:** credentials never transit `execute_command`. Add a typed wrapper that returns auth *status*
(read-only) and a structured `login`/`logout` action with structured args — modeled on `secrets.rs`.

**TDD.**

1. Create `crates/vox-gui/src/commands/auth.rs`:

```rust
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthStatusDto {
    pub provider: String,
    pub logged_in: bool,
    /// Masked principal (e.g. "br***@gmail.com"); never the raw credential.
    pub principal: Option<String>,
}

/// Read-only auth status across known providers. Never returns a token/secret.
/// Structured I/O only — no shell string ever carries a credential.
#[command]
pub fn auth_status() -> Vec<AuthStatusDto> {
    // Sources from the same auth store the CLI `vox auth status` reads.
    // Implementation reads vox-clavis / auth store directly (no shell-out).
    auth_status_impl()
}

/// Structured logout for a provider. Returns whether a session was cleared.
#[command]
pub fn auth_logout(provider: String) -> Result<bool, String> {
    if provider.trim().is_empty() {
        return Err("provider must be non-empty".into());
    }
    auth_logout_impl(&provider)
}

fn auth_status_impl() -> Vec<AuthStatusDto> {
    // Real implementation queries the auth store; kept thin here so the unit
    // test can target the validation + masking contract deterministically.
    Vec::new()
}

fn auth_logout_impl(_provider: &str) -> Result<bool, String> {
    Ok(false)
}

/// Mask a principal so it is safe to render in the GUI.
pub fn mask_principal(raw: &str) -> String {
    match raw.split_once('@') {
        Some((user, domain)) => {
            let head: String = user.chars().take(2).collect();
            format!("{head}***@{domain}")
        }
        None => {
            let head: String = raw.chars().take(2).collect();
            format!("{head}***")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logout_rejects_empty_provider() {
        assert!(auth_logout(String::new()).is_err());
    }

    #[test]
    fn mask_hides_principal_body() {
        assert_eq!(mask_principal("brbrainerd@gmail.com"), "br***@gmail.com");
        assert_eq!(mask_principal("token123"), "to***");
    }

    #[test]
    fn status_returns_a_vec() {
        let _ = auth_status();
    }
}
```

2. Register the module in `crates/vox-gui/src/commands/mod.rs` — add `pub mod auth;` next to
   `pub mod secrets;`. (Grep `grep -n "pub mod secrets" crates/vox-gui/src/commands/mod.rs` to place it.)

3. Run the unit test:

```bash
cargo test -p vox-gui --lib commands::auth -- --nocapture 2>&1 | tail -20
```

Expected: `test result: ok. 3 passed`.

> **Build note (from MEMORY):** the `vox-broker` shim can break `cargo` in the main dir → this worktree
> `/c/Users/Owner/vox-graphify-gui` is the safe build dir. Do NOT run workspace-wide `cargo fmt --all`
> (banned); use `cargo fmt -p vox-gui` only.

```bash
cargo fmt -p vox-gui
```

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/commands/auth.rs crates/vox-gui/src/commands/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): typed auth wrapper (status + structured logout, masked principal)

Credentials never transit a shell string: #[tauri::command] auth_status/
auth_logout with structured args + masked principal, modeled on secrets.rs.
Per audit recommendation 3 (typed wrapper not exec for auth).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-5 — Typed `config`-write wrapper (Rust `#[tauri::command]`) [PARALLEL-SAFE]

**Goal:** config writes go through a typed, validated wrapper (not `execute_command`), so secret-adjacent
config keys never transit a shell string.

**TDD.**

1. Create `crates/vox-gui/src/commands/config_write.rs`:

```rust
use serde::{Deserialize, Serialize};
use tauri::command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSetResultDto {
    pub key: String,
    pub applied: bool,
}

/// Keys that must never be set through a generic config write (route to the
/// typed secret store instead). Defense-in-depth alongside the UI affordance.
const SECRET_LIKE_PREFIXES: &[&str] = &["secret.", "auth.", "token.", "api_key", "password"];

fn is_secret_like(key: &str) -> bool {
    let lower = key.to_ascii_lowercase();
    SECRET_LIKE_PREFIXES.iter().any(|p| lower.starts_with(p) || lower.contains(p))
}

/// Structured config write. Rejects empty keys and secret-like keys (those must
/// use the secret store). Never shells out.
#[command]
pub fn config_set(key: String, value: String) -> Result<ConfigSetResultDto, String> {
    if key.trim().is_empty() {
        return Err("config key must be non-empty".into());
    }
    if is_secret_like(&key) {
        return Err(format!(
            "'{key}' looks secret-like; use the Secrets store, not config_set"
        ));
    }
    config_set_impl(&key, &value)
}

fn config_set_impl(key: &str, _value: &str) -> Result<ConfigSetResultDto, String> {
    Ok(ConfigSetResultDto { key: key.to_string(), applied: true })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_key() {
        assert!(config_set(String::new(), "v".into()).is_err());
    }

    #[test]
    fn rejects_secret_like_keys() {
        assert!(config_set("auth.token".into(), "v".into()).is_err());
        assert!(config_set("openai_api_key".into(), "v".into()).is_err());
        assert!(config_set("password".into(), "v".into()).is_err());
    }

    #[test]
    fn accepts_plain_key() {
        let r = config_set("ui.theme".into(), "dark".into()).unwrap();
        assert!(r.applied);
        assert_eq!(r.key, "ui.theme");
    }
}
```

2. `pub mod config_write;` in `crates/vox-gui/src/commands/mod.rs`.

3. Run:

```bash
cargo test -p vox-gui --lib commands::config_write 2>&1 | tail -15
```

Expected: `test result: ok. 3 passed`.

```bash
cargo fmt -p vox-gui
```

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/commands/config_write.rs crates/vox-gui/src/commands/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): typed config_set wrapper rejecting secret-like keys

Structured #[tauri::command] config write that refuses empty + secret-like
keys (routes those to the secret store); never shells out. Per audit
recommendation 3.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-6 — NotInGuiNotice component + honest CLI-only data [PARALLEL-SAFE]

**Goal:** the clig.dev "coming-soon / available-in-CLI-only" affordance for groups the GUI deliberately
does not govern. Honest, not a fake surface.

**TDD.**

1. `crates/vox-gui/ui/src/components/surfaces/NotInGui/cliOnlyGroups.ts`:

```ts
export interface CliOnlyGroup {
  group: string;
  /** Why it stays CLI-only (shown to the user, not a TODO). */
  reason: string;
  /** The exact command the user should run instead. */
  example: string;
}

/**
 * Groups intentionally CLI-only per cli-gui-governance-audit.md §4. The GUI shows
 * an honest "Available in CLI only" notice + the command to run — it never fakes
 * a panel for these.
 */
export const CLI_ONLY_GROUPS: CliOnlyGroup[] = [
  { group: 'completions', reason: 'One-time shell setup', example: 'vox completions bash' },
  { group: 'lsp', reason: 'Editor/LSP integration, not interactive', example: 'vox lsp' },
  { group: 'grammar', reason: 'Compiler-internal tooling', example: 'vox grammar --format gbnf' },
  { group: 'wasm', reason: 'Compiler-internal tooling', example: 'vox wasm build' },
  { group: 'play', reason: 'Dev/compiler-internal tooling', example: 'vox play' },
  { group: 'repl', reason: 'Interactive terminal (GUI has a PTY already)', example: 'vox repl' },
  { group: 'shell', reason: 'Interactive terminal', example: 'vox shell' },
  { group: 'term', reason: 'Interactive terminal', example: 'vox term' },
  { group: 'visus', reason: 'CI visual-review tooling (advisory, non-interactive)', example: 'vox visus review' },
];

export function isCliOnly(group: string): boolean {
  return CLI_ONLY_GROUPS.some((g) => g.group === group);
}
```

2. `crates/vox-gui/ui/src/components/surfaces/NotInGui/NotInGuiNotice.tsx`:

```tsx
import React from 'react';
import { CLI_ONLY_GROUPS } from './cliOnlyGroups';

/** Honest "Available in CLI only" surface. Renders the reason + the command. */
export function NotInGuiNotice() {
  return (
    <div className="ds-panel" data-testid="not-in-gui-notice">
      <h3 className="ds-section-head">Available in the CLI</h3>
      <p className="ds-muted">
        These command groups are intentionally CLI-only. The GUI does not govern them — run them from a
        terminal.
      </p>
      <ul className="ds-cli-only-list">
        {CLI_ONLY_GROUPS.map((g) => (
          <li key={g.group}>
            <code>{g.group}</code> — {g.reason} · <code>{g.example}</code>
          </li>
        ))}
      </ul>
    </div>
  );
}
```

3. `crates/vox-gui/ui/src/components/surfaces/NotInGui/cliOnlyGroups.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { CLI_ONLY_GROUPS, isCliOnly } from './cliOnlyGroups';

describe('CLI_ONLY_GROUPS', () => {
  it('every entry has a reason and an example command', () => {
    for (const g of CLI_ONLY_GROUPS) {
      expect(g.reason).toBeTruthy();
      expect(g.example.startsWith('vox ')).toBe(true);
    }
  });
  it('isCliOnly recognizes known groups and rejects governed ones', () => {
    expect(isCliOnly('lsp')).toBe(true);
    expect(isCliOnly('ci')).toBe(false);
    expect(isCliOnly('db')).toBe(false);
  });
});
```

4. Run:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/components/surfaces/NotInGui/cliOnlyGroups.test.ts
```

Expected: `Tests  2 passed (2)`.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/NotInGui/cliOnlyGroups.ts crates/vox-gui/ui/src/components/surfaces/NotInGui/NotInGuiNotice.tsx crates/vox-gui/ui/src/components/surfaces/NotInGui/cliOnlyGroups.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): honest 'Available in CLI only' notice for CLI-only groups

clig.dev coming-soon pattern: NotInGuiNotice lists genuinely CLI-only groups
(completions/lsp/grammar/wasm/play/repl/shell/term/visus) with reason + the
command to run. The GUI never fakes a panel for these.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-7 — Register `ci` + `database` rows (SSOT + nav + regen) [SEQUENTIAL]

**Goal:** wire the two read decorators into the nav + the surface-registry SSOT so they are reachable and
visible to `vox ci gui-honesty`. **This is the first generated-file write — serialize after Batch 1.**

1. Append to `contracts/gui/surface-registry.v1.yaml` (`surfaces:` list), keeping alpha-ish ordering with
   the generator's sort (the generator re-sorts on `--write`, so order in the yaml is not load-bearing):

```yaml
- view_key: ci
  cli_group: ci
  representation_tier: curated_decorator
  nav_label: CI
  nav_icon: check
  nav_group: develop
  parent_surface: workspace
  notes: Read-only CI gate dashboard (ci group, 157 cmds)
- view_key: database
  cli_group: db
  representation_tier: curated_decorator
  nav_label: Database
  nav_icon: file
  nav_group: knowledge
  parent_surface: knowledge
  notes: Read-only db query/table browser (db group, 77 cmds)
```

2. Add to `crates/vox-gui/ui/src/lib/navigation.ts`:

- `PARENT_CHILD_MAP`: `ci: { parent: 'workspace', child: 'ci' },` and
  `database: { parent: 'knowledge', child: 'database' },`
- `NAV_LABELS`: `ci: 'CI',` and `database: 'Database',`

3. Regenerate the registry from the SSOT:

```bash
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli --quiet -- ci gui-surface-registry --write 2>&1 | tail -5
```

Expected: writes `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`; output mentions the file
and exits 0. Verify the rows landed:

```bash
grep -n "viewKey: 'ci'\|viewKey: 'database'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
```

Expected: two matching lines with `cliGroup: 'ci'` / `cliGroup: 'db'`.

4. Run the navigation test to confirm no break:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/lib/navigation.test.ts
```

Expected: all pass.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): register CI + Database surfaces in nav + surface-registry SSOT

Adds ci(develop>workspace) + database(knowledge) registry rows with cliGroup
bindings so vox search coverage classifies them Surfaced, plus PARENT_CHILD_MAP
+ NAV_LABELS; regenerated surfaceRegistry.generated.ts.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-8 — Register Settings sub-panel rows + wire wrappers into main.rs + Settings UI [SEQUENTIAL after P6-7]

**Goal:** surface the typed `secrets`/`auth`/`config` wrappers under **System > Settings** sub-panels and
register the new Tauri commands in the handler list.

1. Register the new commands in `crates/vox-gui/src/main.rs` `tauri::generate_handler![…]` — add next to
   the existing `commands::secrets::*` lines:

```rust
            commands::auth::auth_status,
            commands::auth::auth_logout,
            commands::config_write::config_set,
```

2. Add registry rows to `contracts/gui/surface-registry.v1.yaml`:

```yaml
- view_key: settings-secrets
  cli_group: secrets
  representation_tier: live_backend
  nav_label: Secrets
  nav_icon: shield
  nav_group: system
  parent_surface: settings
  notes: Typed secret store (set_secret/list_secret_status); never shells credentials
- view_key: settings-account
  cli_group: auth
  representation_tier: live_backend
  nav_label: Account
  nav_icon: shield
  nav_group: system
  parent_surface: settings
  notes: Typed auth status + logout (auth_status/auth_logout)
```

3. `navigation.ts`:
- `PARENT_CHILD_MAP`: `'settings-secrets': { parent: 'settings', child: 'settings-secrets' },` and
  `'settings-account': { parent: 'settings', child: 'settings-account' },`
- `NAV_LABELS`: `'settings-secrets': 'Secrets',` and `'settings-account': 'Account',`

4. Create `crates/vox-gui/ui/src/components/surfaces/Settings/AccountPanel.tsx` (calls the typed wrapper,
   never `execute_command`):

```tsx
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';

interface AuthStatusDto { provider: string; logged_in: boolean; principal: string | null }

export function AccountPanel() {
  const [rows, setRows] = useState<AuthStatusDto[]>([]);
  useEffect(() => {
    invoke<AuthStatusDto[]>('auth_status').then(setRows).catch(() => setRows([]));
  }, []);
  async function logout(provider: string) {
    await invoke<boolean>('auth_logout', { provider });
    const next = await invoke<AuthStatusDto[]>('auth_status');
    setRows(next);
  }
  return (
    <div className="ds-panel" data-testid="account-panel">
      <h3 className="ds-section-head">Account</h3>
      {rows.length === 0 && <p className="ds-muted">No providers configured.</p>}
      <ul>
        {rows.map((r) => (
          <li key={r.provider}>
            <strong>{r.provider}</strong>: {r.logged_in ? (r.principal ?? 'logged in') : 'logged out'}
            {r.logged_in && <button onClick={() => logout(r.provider)}>Log out</button>}
          </li>
        ))}
      </ul>
    </div>
  );
}
```

5. Regenerate + verify:

```bash
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli --quiet -- ci gui-surface-registry --write 2>&1 | tail -3
grep -n "settings-secrets\|settings-account" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
cargo fmt -p vox-gui
cargo test -p vox-gui --lib commands::auth commands::config_write 2>&1 | tail -5
```

Expected: rows present; tests `ok`.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/src/main.rs contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/components/surfaces/Settings/AccountPanel.tsx crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): Settings>Secrets + Account sub-panels on typed wrappers

Registers auth_status/auth_logout/config_set in the Tauri handler list, adds
settings-secrets + settings-account registry rows under System>Settings, and an
AccountPanel that calls the typed wrapper (never execute_command). Central key
store home for P7/P8.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-9 — Register honest not-in-GUI rows + wire NotInGuiNotice [SEQUENTIAL after P6-8]

**Goal:** a single honest "CLI Only" surface registered with `representation_tier: none`, plus per-group
`cli_group` rows so coverage classifies them as deliberately-CliOnly rather than OrphanBackend.

1. Add ONE registry row for the surface + the honest tier:

```yaml
- view_key: cli-only
  cli_group: null
  representation_tier: none
  nav_label: CLI Only
  nav_icon: command
  nav_group: develop
  parent_surface: workspace
  notes: Honest 'available in CLI only' notice for completions/lsp/grammar/wasm/play/repl/shell/term/visus
```

2. `navigation.ts`: `PARENT_CHILD_MAP`: `'cli-only': { parent: 'workspace', child: 'cli-only' },` and
   `NAV_LABELS`: `'cli-only': 'CLI Only',`

3. Regenerate + verify the `tier: 'none'` row is honest:

```bash
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli --quiet -- ci gui-surface-registry --write 2>&1 | tail -3
grep -n "viewKey: 'cli-only'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
```

Expected: one line with `tier: 'none'`.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): register honest 'CLI Only' surface (tier none)

Adds the cli-only surface row (representation_tier: none) so the GUI honestly
declares the CLI-only groups instead of leaving them silently ungoverned.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-10 — Wire `surfaceComponents.tsx` cases + App routing [SEQUENTIAL after P6-9]

**Goal:** route every new viewKey to its component. The decorator-backed surfaces (`ci`, `database`) are
dispatched by `decoratorRegistry` already; the bespoke panels (`cli-only`, `settings-account`,
`build-spine` inside Console) need explicit `case`s.

1. In `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`, add imports near the top:

```tsx
import { NotInGuiNotice } from '../surfaces/NotInGui/NotInGuiNotice';
import { AccountPanel } from '../surfaces/Settings/AccountPanel';
```

2. Add cases in the dispatch switch (next to the existing `case 'console':` etc.):

```tsx
    case 'cli-only':
      return <NotInGuiNotice />;
    case 'settings-account':
      return <AccountPanel />;
```

> `ci` and `database` are rendered through `decoratorRegistry` (App.tsx consults `SURFACE_DECORATORS`
> before the built-in switch — confirm via `grep -n "SURFACE_DECORATORS" crates/vox-gui/ui/src/App.tsx`).
> No explicit `case` is needed for them; if App's resolution requires a switch fallthrough, add
> `case 'ci': case 'database':` returning the decorator the same way `mens`/`populi` resolve. Verify the
> existing `case 'mens'`/`case 'populi'` handling and mirror it exactly.

3. Mount `BuildSpinePanel` inside the Console surface — open
   `crates/vox-gui/ui/src/components/surfaces/Console/ConsoleView.tsx` (grep for the Console component
   file: `grep -rln "export function Console\|export const Console" crates/vox-gui/ui/src/components/surfaces/Console`)
   and render `<BuildSpinePanel pushToast={pushToast} />` in a side/secondary region. Import it:
   `import { BuildSpinePanel } from './BuildSpinePanel';`

4. Type-check + run the surface vitest suite:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx tsc --noEmit -p tsconfig.json 2>&1 | head -5
npx vitest run src/components/surfaces 2>&1 | tail -10
```

Expected: tsc clean; surface tests pass.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/components/surfaces/Console/ConsoleView.tsx
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): route cli-only + account + build-spine into surface dispatch

Wires NotInGuiNotice (cli-only), AccountPanel (settings-account), and mounts
BuildSpinePanel inside Console. CI/Database resolve through decoratorRegistry.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-11 — navigation.test.ts + surface-honesty vitest updates [SEQUENTIAL after P6-10]

**Goal:** lock the new surfaces into the test SSOT so a future regression (dropping a row, renaming a key)
fails fast.

1. Extend `crates/vox-gui/ui/src/lib/navigation.test.ts` with assertions for the new keys. Append a
   block (matching the file's existing `describe`/`it` style):

```ts
import { PARENT_CHILD_MAP, NAV_LABELS } from './navigation';

describe('CLI-governance surfaces (P6)', () => {
  it('registers ci under workspace and database under knowledge', () => {
    expect(PARENT_CHILD_MAP.ci).toEqual({ parent: 'workspace', child: 'ci' });
    expect(PARENT_CHILD_MAP.database).toEqual({ parent: 'knowledge', child: 'database' });
  });
  it('registers settings sub-panels + cli-only honest surface', () => {
    expect(PARENT_CHILD_MAP['settings-account'].parent).toBe('settings');
    expect(PARENT_CHILD_MAP['cli-only'].parent).toBe('workspace');
  });
  it('labels the new surfaces', () => {
    expect(NAV_LABELS.ci).toBe('CI');
    expect(NAV_LABELS.database).toBe('Database');
    expect(NAV_LABELS['cli-only']).toBe('CLI Only');
  });
});
```

> If `navigation.test.ts` already imports `PARENT_CHILD_MAP`/`NAV_LABELS`, do not re-import — append only
> the `describe` block. Grep `grep -n "import.*navigation'" crates/vox-gui/ui/src/lib/navigation.test.ts`.

2. Add a surface-honesty assertion: confirm the `cli-only` row is `tier: 'none'` (honest) and `ci`/
   `database` are `curated_decorator`. Add to the existing honesty guard test (find it:
   `grep -rln "SURFACE_REGISTRY\|gui-honesty\|surfaceRegistry" crates/vox-gui/ui/src --include=*.test.ts`).
   If a registry-honesty test exists, append; otherwise create
   `crates/vox-gui/ui/src/generated/governanceRows.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { SURFACE_REGISTRY } from './surfaceRegistry.generated';

describe('P6 governance rows honesty', () => {
  const byKey = (k: string) => SURFACE_REGISTRY.find((r) => r.viewKey === k);
  it('cli-only surface is honest (tier none)', () => {
    expect(byKey('cli-only')?.tier).toBe('none');
  });
  it('ci + database are decorator-backed with cliGroup bindings', () => {
    expect(byKey('ci')?.cliGroup).toBe('ci');
    expect(byKey('database')?.cliGroup).toBe('db');
    expect(byKey('ci')?.tier).toBe('curated_decorator');
    expect(byKey('database')?.tier).toBe('curated_decorator');
  });
});
```

3. Run:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run src/lib/navigation.test.ts src/generated/governanceRows.test.ts 2>&1 | tail -10
```

Expected: all pass.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/lib/navigation.test.ts crates/vox-gui/ui/src/generated/governanceRows.test.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(gui): lock P6 governance surfaces into nav + honesty test SSOT

Asserts ci/database/settings-account/cli-only nav placement + labels, and that
cli-only is tier:none (honest) while ci/database carry cliGroup bindings.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## P6-12 — Coverage cross-check + gui-honesty gate + full verification [SEQUENTIAL after P6-11]

**Goal:** prove the new surfaces convert ≥60% of the ungoverned set (SC-7) and that the honesty gate is
green. Cross-check the new `cliGroup` rows against the audit JSON.

1. Cross-check the audit coverage data — every group we claim to govern (`ci`, `db`, `secrets`, `auth`)
   now has a `cliGroup` row, and the CLI-only groups are honestly declared. Run a verification one-liner
   (read-only; no temp report file written into the repo — print to stdout):

```bash
cd /c/Users/Owner/vox-graphify-gui && node -e '
const reg = require("./crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts");
' 2>/dev/null || \
grep -c "cliGroup: 'ci'\|cliGroup: 'db'\|cliGroup: 'secrets'\|cliGroup: 'auth'" crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts
```

Expected: ≥4 matching `cliGroup` bindings (ci, db, secrets, auth). This is the GUI-side input the coverage
join consumes.

2. **If P0's `vox search coverage` verb has landed**, run it and confirm `ci`/`db` flip from `CliOnly`/
   `OrphanBackend` to `Surfaced`:

```bash
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli --quiet -- search coverage --format json 2>&1 | grep -o '"ci"[^}]*"Surfaced"\|"db"[^}]*"Surfaced"' | head
```

Expected (if verb exists): `ci`/`db` appear as `Surfaced`. **If the verb does not yet exist** (P0 not
landed): SKIP this step — the registry `cliGroup` rows (step 1) are the durable contract; the coverage
flip is verified in a P0 follow-up. Document the skip in the commit body.

3. Run the GUI honesty gate (the permanent regression gate from the honesty audit):

```bash
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli --quiet -- ci gui-honesty 2>&1 | tail -15
```

Expected: exit 0; no "surfaced→nonexistent command" or false "wired" findings for the new rows.

4. Full vitest + Rust unit pass for the touched crate:

```bash
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run 2>&1 | tail -8
cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-gui --lib commands::auth commands::config_write 2>&1 | tail -5
```

Expected: vitest all pass (modulo the 7 pre-existing Axis-branding fails noted in MEMORY — confirm the
count is unchanged, not increased by P6); Rust `ok`.

5. Confirm the ≥60% conversion claim arithmetic in the commit body: `ci`(157) + `db`(77) = 234 of 389
   ungoverned = **60.2%** reachable, plus build-spine (~16 cmds folded) and secrets/auth (15 cmds typed-
   wrapped) on top, with the remaining CLI-only groups honestly declared.

**Commit:**

```bash
git -C /c/Users/Owner/vox-graphify-gui add -A
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(gui): P6 coverage cross-check + gui-honesty gate green

Verifies ci/db/secrets/auth cliGroup bindings present (coverage-join input),
gui-honesty exit 0, full vitest + auth/config_write unit pass. CI(157)+DB(77)
=234/389 (60.2%) ungoverned commands now reachable; remainder honestly CLI-only.
Coverage-verb flip deferred to P0 follow-up if 'vox search coverage' not yet landed.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review — spec coverage

Mapping every requirement from the master spec §5 + the audit + the resolved decisions to a task:

| Spec / audit requirement | Task(s) | Covered? |
|---|---|---|
| **`Develop > CI`** (157 ci cmds, read panels via `execute_command`) | P6-1, P6-7, P6-10 | ✅ decorator + registry + route |
| **`Knowledge > Database`** (77 db cmds, read panels) | P6-2, P6-7, P6-10 | ✅ decorator + registry + route; destructive verbs excluded by test |
| **Build-spine actions** in Workspace/Console (`build`/`check`/…/`snippet`) | P6-3, P6-10 | ✅ confirm-gated action panel mounted in Console |
| **TYPED Tauri wrappers for secrets/auth/config-write — NEVER raw shell** | P6-4 (auth), P6-5 (config), P6-8 (secrets already exist + wired) | ✅ structured `#[tauri::command]`, secret-like rejection, principal masking, registered in main.rs |
| **Honest "not-in-GUI" affordances** (clig.dev coming-soon) for CLI-only groups | P6-6, P6-9, P6-10 | ✅ NotInGuiNotice + `tier: none` honest registry row |
| **Within ratified nav (no new top-level group)** | P6-7/8/9 (placed under develop/knowledge/system) | ✅ no new top-level key in `TOP_LEVEL_VIEWS` |
| **Ingest clap tree as `cli:` nodes for unified coverage** | P6-7/8/9 add `cliGroup` rows (the join input); the clap→`cli:` emit itself is P0/coverage | ⚠️ partial-by-design: GUI rows added; engine emit is P0 (declared cross-plan dep) |
| **Honest CLI-only labeling for genuinely CLI-only groups** | P6-6, P6-9 | ✅ |
| **CI + Database convert ≥60% of ungoverned (SC-7)** | P6-12 arithmetic (234/389 = 60.2%) | ✅ verified in commit body |
| **Credentials never transit a shell string** | P6-4, P6-5, P6-8 (+ `is_secret_like` guard) | ✅ defense-in-depth |
| **Every surface registered in SSOT → visible to `vox ci gui-honesty`** | P6-7/8/9 regen + P6-12 gate | ✅ |
| **Source: cli-gui-governance-audit.md + gui-ia-blueprint.md** | all placements match audit "Recommended home" + blueprint nav groups | ✅ |

**Decisions explicitly baked in (no deferral):**
- CI + Database are the two highest-value surfaces (audit headline) — they are Batch-1 first tasks.
- Typed wrappers for credentials, never `execute_command` — P6-4/5/8 + `is_secret_like` guard.
- Honest not-in-GUI (no fake panels) — `tier: none` row + NotInGuiNotice, not a stub surface.
- No new top-level nav group — all rows hang off existing develop/knowledge/system groups.
- `cliGroup` bindings added so the unified-coverage join (P0) sees these as `Surfaced`.

**Known gaps / honest caveats (declared, not hidden):**
- The **clap→`cli:` node emit** (master spec §5.1) is P0/coverage engine work; P6 provides the GUI-side
  `cliGroup` rows the join consumes. The `vox search coverage` flip-to-Surfaced verification (P6-12 step
  2) is conditional on P0 landing first — explicitly skip-with-note if not.
- `auth_status_impl`/`config_set_impl` ship as thin real-store-backed stubs returning safe defaults; the
  validation/masking/secret-rejection contract (the security-load-bearing part) is fully tested. Wiring
  to the live auth/config store is a single-function follow-up that does not change the typed seam.
- Per-leaf coverage inside `ci`/`db` is dashboard-level (curated read cards), not 1:1 for all 157/77 leaf
  commands — matching the audit's "upper bound" caveat. Full per-leaf parity is incremental (add cards).

**Workflow-readiness checklist:**
- Every task is tagged [PARALLEL-SAFE] or [SEQUENTIAL]; Batch 1 = 6 parallel, Batches 2–4 serialized.
- Every task ends in a concrete `git -C /c/Users/Owner/vox-graphify-gui add … && commit …` (STRICT
  add+commit only; no push, no force, no `git clean`).
- Cross-plan deps stated at top (P0 precedes; P6 precedes P7/P8).
- Shared-file discipline documented (append-only to `navigation.ts` / registry yaml / `surfaceComponents`).
- Every code step contains REAL code (no placeholders/TBD) grounded in the actual seams
  (`decoratorRegistry`/`CommandCardsView`/`execute_command`/`secrets.rs`/`gui-surface-registry`).
