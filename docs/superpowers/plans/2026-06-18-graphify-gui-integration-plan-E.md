# Plan E — Graphify GUI Integration (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILLS: `crates/vox-skills/skills/superpowers/subagent-driven-development.skill.md` + `.../test-driven-development.skill.md`. Steps use `- [ ]`.

> **🤖 EXECUTION TARGET — READ FIRST.** Run by **Gemini 3.5 Flash inside Google Antigravity** (~48% completion, no mid-task checkpoint, hard quota cutoff, API hallucination, weak long-context recall). Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md). Suite: [`2026-06-18-graphify-native-system-suite-index.md`](2026-06-18-graphify-native-system-suite-index.md).
> **DEPENDS ON Plan A** (freshness-correct manifest, so the panel shows real freshness). Land A first.

## Operating Rules (apply to EVERY task)
1. **Atomic + green + committed.** Crash between tasks → compiling, tested tree.
2. **Verify-before-use.** First step is an `rg`/read confirming exact symbols/paths. Differs → STOP.
3. **Self-contained.** Everything needed is in the task.
4. **Two-strike circuit breaker.** Fails twice → STOP + handoff note. No looping.
5. **Parallel dispatch.** Honor `[PARALLEL-SAFE]`/`[SEQUENTIAL]`; never two subagents on one file.
6. **Vox house rules.** No `cargo fmt --all`; automation is `.vox`; `docs/src/` `.md` needs frontmatter; no stubs. **Do not hand-edit `*.generated.ts`** — rerun its generator.
7. **Verification ritual before commit** (skill `verification-before-completion`), paste output. **Rust:** `cargo test -p vox-gui --lib` → `cargo clippy -p vox-gui --lib -- -D warnings` (LIB-ONLY: vox-gui's Tauri build script breaks `--all-targets` clippy — see `feedback_vox_gui_clippy_buildscript_gotcha`) → `cargo fmt -p vox-gui`. **TS:** `pnpm -C crates/vox-gui/ui test` → `pnpm -C crates/vox-gui/ui exec tsc --noEmit`.
8. **Rollback on broken tree:** `git reset --hard HEAD`; re-attempt the single task.
9. **Skills:** `brainstorming` / `dispatching-parallel-agents` / `using-git-worktrees`.
10. **Determinism.** `cargo run -p vox-arch-check` passes before final commit.

**Goal:** Surface graphify corpus health in `vox-gui` — a panel showing each corpus's freshness, node/edge counts, stale reasons, and the exact rebuild command for stale corpora.

**Architecture:** The minimal, verified vertical slice. (E1) a read-only Tauri command `vox_graphify_status` wrapping a pure `build_status_payload` that calls `vox_config::graphify` (already a `vox-gui` dependency). (E2) TS DTO + transport wrapper + React Query hook. (E3) a `GraphifyStatusPanel` component + vitest. (E4) route it via the hand-written `surfaceComponents` switch.

**Tech Stack:** Rust (`#[tauri::command]`, `vox-config`, `chrono`); TypeScript/React; `@tanstack/react-query`; Vitest + Testing Library.

> **Scope note (deferred, not placeholder):** an interactive graph **explorer/visualization** is out of scope — `vox-graphify-reader` is NOT a `vox-gui` dependency and its APIs are sync; a D3/Cytoscape renderer is a new dependency with no existing pattern. A **live "click-to-rebuild"** button is also deferred (write action; needs a command that shells `vox graphify rebuild`). This plan renders the rebuild command for the user to run. Adding a **nav sidebar entry** requires regenerating `surfaceRegistry.generated.ts` from its canonical spec (never hand-edit it) — flagged as a final optional step.

---

## File Structure

| File | Responsibility | Action |
|---|---|---|
| `crates/vox-gui/src/commands/graphify.rs` | Tauri command + pure payload builder | Create (E1) |
| `crates/vox-gui/src/commands/mod.rs` (or where `mod` lives) | declare `pub mod graphify;` | Modify (E1) |
| `crates/vox-gui/src/main.rs` | register command in `generate_handler!` | Modify (E1) |
| `crates/vox-gui/ui/src/types/tauri.ts` | `GraphifyStatusDto` / `CorpusStatusDto` | Modify (E2) |
| `crates/vox-gui/ui/src/transport.ts` | `getGraphifyStatus()` | Modify (E2) |
| `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts` | React Query hook | Create (E2) |
| `crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts` | hook test | Create (E2) |
| `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx` | panel | Create (E3) |
| `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx` | panel test | Create (E3) |
| `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` | route the panel | Modify (E4) |

**Pre-flight (run once, paste output; NOT a code step):**
- `rg -n "pub mod |mod commands" crates/vox-gui/src/main.rs crates/vox-gui/src/commands/mod.rs` — find where command modules are declared.
- `rg -n "generate_handler!" crates/vox-gui/src/main.rs` — the registration site.
- `rg -n "list_model_cards|#\[tauri::command\]" crates/vox-gui/src/commands/models.rs` — an existing async command to mirror.
- `rg -n "chrono" crates/vox-gui/Cargo.toml` — confirm `chrono` is available (add `chrono = { workspace = true }` if absent).
- `rg -n "load_graphify_corpora|assess_corpus_status|resolve_ttl_days|CorpusStatus" crates/vox-config/src/graphify.rs` — confirm these are `pub` (they are).
- `rg -n "invoke\(|export async function" crates/vox-gui/ui/src/transport.ts | head` — the transport `invoke` pattern.
- `rg -n "case 'memory'|function childRenderer|renderSurfaceView" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — the switch to extend.
- `cargo run -p vox-arch-check` — baseline passes.

---

## Task E1 `[SEQUENTIAL]`: Backend command `vox_graphify_status`

**Files:**
- Create: `crates/vox-gui/src/commands/graphify.rs`
- Modify: module-decl file (Pre-flight) + `crates/vox-gui/src/main.rs`
- Test: `#[cfg(test)]` in the new file

- [ ] **Step 1 (verify-before-use):** Run the Pre-flight `rg` lines. Confirm the module-decl file, the `generate_handler!` site, that `chrono` is in `vox-gui/Cargo.toml` (add `chrono = { workspace = true }` if not), and that `vox_config::graphify::{load_graphify_corpora, assess_corpus_status, resolve_ttl_days, CorpusStatus}` are `pub`. Differs → STOP.

- [ ] **Step 2: Create the file with a pure builder + failing test.**

```rust
//! Read-only graphify corpus-health command for the GUI.
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::path::Path;
use vox_config::graphify::{
    CorpusStatus, assess_corpus_status, load_graphify_corpora, resolve_ttl_days,
};

#[derive(Debug, Serialize)]
pub struct GraphifyStatusPayload {
    pub default_corpus_id: String,
    pub corpora: Vec<CorpusStatus>,
}

/// Pure: assemble corpus statuses for a repo. Injecting `head_sha`/`now` keeps it deterministic.
pub fn build_status_payload(
    repo_root: &Path,
    head_sha: Option<&str>,
    now: DateTime<Utc>,
) -> Result<GraphifyStatusPayload, String> {
    let reg = load_graphify_corpora(repo_root).map_err(|e| e.to_string())?;
    let ttl = resolve_ttl_days(reg.ttl_days_default);
    let corpora = reg
        .corpora
        .iter()
        .map(|c| assess_corpus_status(repo_root, c, head_sha, now, ttl))
        .collect();
    Ok(GraphifyStatusPayload {
        default_corpus_id: reg.default_corpus_id,
        corpora,
    })
}

#[tauri::command]
pub async fn vox_graphify_status() -> Result<GraphifyStatusPayload, String> {
    let repo_root = std::env::current_dir().map_err(|e| format!("cwd: {e}"))?;
    let head = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    build_status_payload(&repo_root, head.as_deref(), Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_lists_corpora_with_freshness() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("contracts/retrieval");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("graphify-corpora.v1.yaml"),
            "default_corpus_id: repo-code-graph\nttl_days_default: 30\ncorpora:\n  - id: repo-code-graph\n    title: Repo\n    scope_path: \".\"\n    graph_path: \".vox/cache/graphify/repo-code-graph/graph.json\"\n    manifest_path: \".vox/cache/graphify/repo-code-graph/.graphify_manifest.v1.json\"\n",
        )
        .unwrap();
        let payload = build_status_payload(tmp.path(), Some("abc"), Utc::now()).unwrap();
        assert_eq!(payload.default_corpus_id, "repo-code-graph");
        assert_eq!(payload.corpora.len(), 1);
        // No graph on disk → stale with graph_missing.
        assert!(!payload.corpora[0].is_fresh);
        assert!(payload.corpora[0].stale_reasons.iter().any(|r| r == "graph_missing"));
    }
}
```

> If `tempfile` is not a dev-dependency of `vox-gui`, add `tempfile = { workspace = true }` under `[dev-dependencies]` in `crates/vox-gui/Cargo.toml`.

- [ ] **Step 3: Run → FAIL, then register.** `cargo test -p vox-gui --lib payload_lists_corpora_with_freshness` → FAIL until the module is declared. Add `pub mod graphify;` to the command module-decl file (Pre-flight), and add `commands::graphify::vox_graphify_status,` to the `generate_handler![...]` list in `main.rs`.

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-gui --lib payload_lists_corpora_with_freshness` → PASS. `cargo build -p vox-gui` → clean.

- [ ] **Step 5: Verify (Rule 7, LIB-ONLY clippy) + commit.**

```bash
git add crates/vox-gui/src/commands/graphify.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(gui): vox_graphify_status command (read-only corpus health)"
```

---

## Task E2 `[PARALLEL-SAFE]` (TS only; disjoint from E1's Rust): DTO + transport + hook

**Files:**
- Modify: `crates/vox-gui/ui/src/types/tauri.ts`, `crates/vox-gui/ui/src/transport.ts`
- Create: `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts` + `.test.ts`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "export interface|export async function|invoke<" crates/vox-gui/ui/src/transport.ts | head` and `rg -n "listenOrchStatus|voxTransport|useQuery" crates/vox-gui/ui/src/hooks/useOrchestratorStatus.ts`. Confirm the `invoke<T>('cmd')` wrapper style and the React Query hook pattern. Differs → adapt.

- [ ] **Step 2: Write the failing hook test.** Create `crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { renderHook, waitFor } from '@testing-library/react';
import React from 'react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';

const mockGet = vi.fn();
vi.mock('../transport', () => ({ getGraphifyStatus: () => mockGet() }));

import { useGraphifyStatus } from './useGraphifyStatus';

function wrapper({ children }: { children: React.ReactNode }) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return React.createElement(QueryClientProvider, { client }, children);
}

describe('useGraphifyStatus', () => {
  beforeEach(() => vi.clearAllMocks());
  it('fetches graphify status via transport', async () => {
    mockGet.mockResolvedValue({ default_corpus_id: 'repo-code-graph', corpora: [] });
    const { result } = renderHook(() => useGraphifyStatus(), { wrapper });
    await waitFor(() => expect(result.current.isSuccess).toBe(true));
    expect(result.current.data?.default_corpus_id).toBe('repo-code-graph');
  });
});
```

- [ ] **Step 3: Run → FAIL.** `pnpm -C crates/vox-gui/ui exec vitest run src/hooks/useGraphifyStatus.test.ts` → FAIL (hook missing).

- [ ] **Step 4: Add the DTO** to `crates/vox-gui/ui/src/types/tauri.ts`:

```typescript
export interface CorpusStatusDto {
  corpus_id: string;
  title: string;
  graph_exists: boolean;
  manifest_exists: boolean;
  node_count: number | null;
  edge_count: number | null;
  built_at: string | null;
  manifest_git_sha: string | null;
  head_git_sha: string | null;
  stale_reasons: string[];
  warnings: string[];
  is_fresh: boolean;
}
export interface GraphifyStatusDto {
  default_corpus_id: string;
  corpora: CorpusStatusDto[];
}
```

- [ ] **Step 5: Add the transport wrapper** to `crates/vox-gui/ui/src/transport.ts` (use the file's existing `invoke` import/style):

```typescript
export async function getGraphifyStatus(): Promise<GraphifyStatusDto> {
  return invoke<GraphifyStatusDto>('vox_graphify_status');
}
```

(Add `GraphifyStatusDto` to the existing `import type { ... } from './types/tauri'` in transport.ts.)

- [ ] **Step 6: Create the hook** `crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts`:

```typescript
import { useQuery } from '@tanstack/react-query';
import { getGraphifyStatus } from '../transport';
import type { GraphifyStatusDto } from '../types/tauri';

export const GRAPHIFY_STATUS_QUERY_KEY = ['graphify', 'status'];

export function useGraphifyStatus() {
  return useQuery<GraphifyStatusDto, Error>({
    queryKey: GRAPHIFY_STATUS_QUERY_KEY,
    queryFn: getGraphifyStatus,
    staleTime: 30_000,
    refetchInterval: 60_000,
  });
}
```

- [ ] **Step 7: Run → PASS + typecheck.** `pnpm -C crates/vox-gui/ui exec vitest run src/hooks/useGraphifyStatus.test.ts` → PASS. `pnpm -C crates/vox-gui/ui exec tsc --noEmit` → clean.

- [ ] **Step 8: Commit.**

```bash
git add crates/vox-gui/ui/src/types/tauri.ts crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts crates/vox-gui/ui/src/hooks/useGraphifyStatus.test.ts
git commit -m "feat(gui): graphify status DTO + transport + useGraphifyStatus hook"
```

---

## Task E3 `[SEQUENTIAL]` (after E2): Corpus-health panel + test

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx` + `.test.tsx`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "render|screen|@vitest-environment" crates/vox-gui/ui/src/components/surfaces/Memory/MemoryView.test.tsx`. Confirm the Testing Library + jsdom conventions to mirror.

- [ ] **Step 2: Write the failing test.** Create `crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx`:

```typescript
// @vitest-environment jsdom
import { describe, it, expect, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import React from 'react';

const mockUse = vi.fn();
vi.mock('../../../hooks/useGraphifyStatus', () => ({
  useGraphifyStatus: () => mockUse(),
  GRAPHIFY_STATUS_QUERY_KEY: ['graphify', 'status'],
}));

import { GraphifyStatusPanel } from './GraphifyStatusPanel';

describe('GraphifyStatusPanel', () => {
  it('renders corpus health and rebuild command for stale corpora', () => {
    mockUse.mockReturnValue({
      isLoading: false,
      isError: false,
      data: {
        default_corpus_id: 'repo-code-graph',
        corpora: [
          {
            corpus_id: 'repo-code-graph', title: 'Repo', graph_exists: false, manifest_exists: false,
            node_count: null, edge_count: null, built_at: null, manifest_git_sha: null,
            head_git_sha: 'abc', stale_reasons: ['graph_missing'], warnings: [], is_fresh: false,
          },
        ],
      },
    });
    render(<GraphifyStatusPanel />);
    expect(screen.getByText('Repo')).toBeDefined();
    expect(screen.getByText(/graph_missing/)).toBeDefined();
    expect(screen.getByText(/vox graphify rebuild --corpus repo-code-graph/)).toBeDefined();
  });

  it('shows loading state', () => {
    mockUse.mockReturnValue({ isLoading: true, isError: false });
    render(<GraphifyStatusPanel />);
    expect(screen.getByText(/Loading graphify/i)).toBeDefined();
  });
});
```

- [ ] **Step 3: Run → FAIL.** `pnpm -C crates/vox-gui/ui exec vitest run src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx` → FAIL (component missing).

- [ ] **Step 4: Create the panel** `GraphifyStatusPanel.tsx` (no props → no Toast coupling; renders the rebuild command as copyable text):

```typescript
import React from 'react';
import { useGraphifyStatus } from '../../../hooks/useGraphifyStatus';

export function GraphifyStatusPanel() {
  const { data, isLoading, isError, error } = useGraphifyStatus();

  if (isLoading) return <div className="p-4">Loading graphify status…</div>;
  if (isError) {
    return (
      <div className="p-4" role="alert">
        Graphify status unavailable: {(error as Error)?.message ?? 'unknown error'}
      </div>
    );
  }
  if (!data) return <div className="p-4">No graphify data</div>;

  return (
    <div className="p-4 space-y-3">
      <h2 className="text-lg font-semibold">Graphify Corpus Health</h2>
      <div className="text-sm">Default corpus: {data.default_corpus_id}</div>
      <ul className="space-y-2">
        {data.corpora.map((c) => (
          <li key={c.corpus_id} className="border rounded p-2">
            <div className="font-medium">
              {c.title} — {c.is_fresh ? 'fresh' : 'stale'}
            </div>
            <div className="text-xs">
              {c.node_count ?? '?'} nodes · {c.edge_count ?? '?'} edges
            </div>
            {!c.is_fresh && (
              <div className="text-xs mt-1">
                <div>Stale: {c.stale_reasons.join(', ')}</div>
                <code className="block mt-1">vox graphify rebuild --corpus {c.corpus_id}</code>
              </div>
            )}
          </li>
        ))}
      </ul>
    </div>
  );
}
```

- [ ] **Step 5: Run → PASS + typecheck.** `pnpm -C crates/vox-gui/ui exec vitest run src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx` → PASS. `pnpm -C crates/vox-gui/ui exec tsc --noEmit` → clean.

- [ ] **Step 6: Commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.tsx crates/vox-gui/ui/src/components/surfaces/Graphify/GraphifyStatusPanel.test.tsx
git commit -m "feat(gui): GraphifyStatusPanel — corpus freshness + rebuild command"
```

---

## Task E4 `[SEQUENTIAL]` (after E3): Route the panel

**Files:**
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`

- [ ] **Step 1 (verify-before-use):** Run `rg -n "case 'memory'|childRenderer|import .* MemoryView" crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`. Confirm the `switch (viewKey)` in `childRenderer` and the import style for surface components.

- [ ] **Step 2: Add the import + case.** At the top with the other surface imports:

```typescript
import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel';
```

In `childRenderer`'s `switch (viewKey)`, add a case alongside the others:

```typescript
    case 'graphify':
      return <GraphifyStatusPanel />;
```

- [ ] **Step 3: Typecheck + full UI test suite.** `pnpm -C crates/vox-gui/ui exec tsc --noEmit` → clean. `pnpm -C crates/vox-gui/ui test` → all green (the new case does not affect existing surfaces).

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx
git commit -m "feat(gui): route 'graphify' surface to GraphifyStatusPanel"
```

- [ ] **Step 5 (OPTIONAL, follow-up — do NOT hand-edit generated files):** To make a sidebar nav entry, add a `viewKey: 'graphify'` surface to the **canonical** surface spec the generator reads, then run `vox ci gui-surface-registry --write` to regenerate `surfaceRegistry.generated.ts`. Confirm the canonical spec path first (`rg -n "graphify" contracts/gui/`). If the canonical spec format is unclear, STOP and hand back — the panel is already reachable via the switch; nav surfacing is a separate, generator-driven change.

---

## Parallelization summary
- **E1 (Rust) ∥ E2 (TS)** are PARALLEL-SAFE (disjoint file trees). **E3 SEQUENTIAL after E2** (uses the hook). **E4 SEQUENTIAL after E3** (uses the panel).

## Self-Review
- **Spec coverage:** "integrate with Vox's GUI" + "prompt the user through to the GUI" — corpus-health panel shows freshness/stale reasons and the exact rebuild command (the prompt). Interactive **visualization** and **live click-to-rebuild** + **nav-registry** are explicitly DEFERRED (scoped down, recorded), not stubbed.
- **Placeholder scan:** none. The panel takes no props (no unverified `Toast` shape coupling); the generated-registry trap is handled by a generator step with a hard stop, never a hand-edit.
- **Type consistency:** `GraphifyStatusPayload`/`GraphifyStatusDto` fields mirror `CorpusStatus` (serde); `useGraphifyStatus`/`getGraphifyStatus`/`GRAPHIFY_STATUS_QUERY_KEY` identical across hook + test + transport; `build_status_payload(repo_root, head_sha, now)` is the pure, tested core.
- **Antigravity fit:** atomic+green+commit per task; pure `build_status_payload` is unit-tested (the cwd/git `#[tauri::command]` wrapper is thin + build-verified); LIB-ONLY clippy avoids the known vox-gui Tauri-build-script clippy break (`feedback_vox_gui_clippy_buildscript_gotcha`); E1∥E2 parallelism respects file disjointness.
