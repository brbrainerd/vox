# GUI Track B — Scientia Pipeline UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Represent the Scientia *pipelines* (deep-research runtime + self-publication lifecycle) as first-class GUI surfaces — a reusable stage timeline, a real research-run + session-detail surface, and a publication stage board — built on what exists today (typed Tauri commands + the inline CLI run), not on the documented-but-unbuilt REST/WS backend.

**Architecture:** A reusable pure `PipelineTimeline` React component drives every pipeline view. Typed Tauri commands in a new `crates/vox-gui/src/commands/scientia.rs` read research sessions and publication manifests directly from the canonical DB (mirroring the CLI handlers), so surfaces stop parsing CLI stdout for reads. The actual research *run* uses the existing inline `vox research run --json` (which really executes) via `execute_command`. No speculative `/api/v2/scientia/*` REST or WS work — that is explicitly deferred (see end).

**Tech Stack:** Rust (`tauri`, `vox-db` — already a `vox-gui` dep), React 18 + TS + Vite + Tailwind, Vitest for pure helpers.

---

## Reality constraints (verified — do not violate)

- **No typed research session-status enum exists.** The 8 progress states are a hardcoded JSON array at `crates/vox-cli/src/commands/research/mod.rs:234-243`; the persisted DB `status` is a free `String` (`active`/`completed`/`failed`/`orphaned`). The timeline is therefore *coarse-grained* (derived from the terminal status), not per-stage live.
- **`research run --async` enqueues nothing** (`research/mod.rs:304-317` only inserts a row). The run surface must use the **inline** path, which really executes and returns a `ResearchResult`.
- **`/api/v2/scientia/*` REST + `scientia.queue.changed` WS do not exist** and the HTTP gateway is disabled by default. Reads go through typed Tauri commands → `vox_db`, not HTTP.

---

## File Structure

- Create `crates/vox-gui/src/commands/scientia.rs` — typed research + publication read commands.
- Modify `crates/vox-gui/src/commands/mod.rs` — `pub mod scientia;`.
- Modify `crates/vox-gui/src/main.rs` — register the new commands.
- Create `crates/vox-gui/ui/src/lib/pipeline.ts` — `RESEARCH_STAGES`, `PUBLICATION_STAGES`, `deriveStages`, `groupByStage` (pure).
- Create `crates/vox-gui/ui/src/lib/pipeline.test.ts` — Vitest.
- Create `crates/vox-gui/ui/src/components/PipelineTimeline.tsx` — reusable stage strip.
- Create `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx` — run + history + detail.
- Create `crates/vox-gui/ui/src/components/surfaces/Publications/PublicationsView.tsx` — stage board.
- Modify `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts` — replace `research` decorator; add `publications`.
- Modify `crates/vox-gui/ui/src/App.tsx` — add `publications` to the `View` union + validation array.
- Modify `contracts/gui/surface-registry.v1.yaml` — add the `publications` surface; regenerate (Track A).

---

## Task 1: Reusable PipelineTimeline + pure stage helpers (TDD)

**Files:**
- Create: `crates/vox-gui/ui/src/lib/pipeline.ts`
- Create: `crates/vox-gui/ui/src/lib/pipeline.test.ts`
- Create: `crates/vox-gui/ui/src/components/PipelineTimeline.tsx`

- [ ] **Step 1: Write the failing Vitest**

Create `crates/vox-gui/ui/src/lib/pipeline.test.ts`:

```ts
import { describe, it, expect } from 'vitest';
import { deriveStages, groupByStage, RESEARCH_STAGES, PUBLICATION_STAGES } from './pipeline';

describe('deriveStages', () => {
  it('marks every stage done when completed', () => {
    const s = deriveStages('completed');
    expect(s.planning).toBe('done');
    expect(s.completed).toBe('done');
  });
  it('marks every stage error on failure/orphan', () => {
    expect(deriveStages('failed').synthesizing).toBe('error');
    expect(deriveStages('orphaned').retrieving).toBe('error');
  });
  it('shows queued done and the rest pending while active', () => {
    const s = deriveStages('active');
    expect(s.queued).toBe('done');
    expect(s.completed).toBe('pending');
  });
});

describe('groupByStage', () => {
  it('buckets manifests by state and keeps empty stages', () => {
    const groups = groupByStage([
      { publication_id: 'a', content_type: 'paper', state: 'draft', created_at_ms: 1, updated_at_ms: 1 },
      { publication_id: 'b', content_type: 'paper', state: 'published', created_at_ms: 2, updated_at_ms: 2 },
    ]);
    expect(groups.draft.map(m => m.publication_id)).toEqual(['a']);
    expect(groups.published).toHaveLength(1);
    expect(groups.approved).toEqual([]);
  });
  it('exposes the canonical stage order', () => {
    expect(RESEARCH_STAGES[0]).toBe('queued');
    expect(PUBLICATION_STAGES).toContain('submitted');
  });
});
```

- [ ] **Step 2: Run it to verify it fails**

Run: `pnpm --dir crates/vox-gui/ui test -- pipeline`
Expected: FAIL with "Cannot find module './pipeline'".

- [ ] **Step 3: Write the pure module**

Create `crates/vox-gui/ui/src/lib/pipeline.ts`:

```ts
export type StageStatus = 'done' | 'active' | 'pending' | 'error';

// Mirrors crates/vox-cli/src/commands/research/mod.rs:234-243 (no Rust enum exists).
export const RESEARCH_STAGES = [
  'queued', 'planning', 'retrieving', 'verifying_claims',
  'synthesizing', 'auditing_citations', 'persisting_artifact', 'completed',
] as const;

// Mirrors the scientia_publication_queue lifecycle.
export const PUBLICATION_STAGES = [
  'draft', 'doi_reserved', 'orcid_attributed', 'approved', 'submitted', 'published', 'failed',
] as const;

/**
 * Coarse-grained per-stage status derived from the persisted session status.
 * The DB does not track per-stage progress, so this is intentionally coarse:
 * completed → all done; failed/orphaned → all error; otherwise queued done, rest pending.
 */
export function deriveStages(sessionStatus: string): Record<string, StageStatus> {
  const out: Record<string, StageStatus> = {};
  const status = sessionStatus.toLowerCase();
  for (const stage of RESEARCH_STAGES) {
    if (status === 'completed') out[stage] = 'done';
    else if (status === 'failed' || status === 'orphaned') out[stage] = 'error';
    else out[stage] = stage === 'queued' ? 'done' : 'pending';
  }
  return out;
}

export interface PublicationManifest {
  publication_id: string;
  content_type: string;
  state: string;
  created_at_ms: number;
  updated_at_ms: number;
}

/** Bucket manifests by `state`, preserving every canonical stage (empty allowed). */
export function groupByStage(manifests: PublicationManifest[]): Record<string, PublicationManifest[]> {
  const groups: Record<string, PublicationManifest[]> = {};
  for (const s of PUBLICATION_STAGES) groups[s] = [];
  for (const m of manifests) {
    (groups[m.state] ??= []).push(m);
  }
  return groups;
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `pnpm --dir crates/vox-gui/ui test -- pipeline`
Expected: PASS (5 assertions across 2 describes).

- [ ] **Step 5: Write the reusable timeline component**

Create `crates/vox-gui/ui/src/components/PipelineTimeline.tsx`:

```tsx
import React from 'react';
import type { StageStatus } from '../lib/pipeline';

const DOT: Record<StageStatus, string> = {
  done: 'bg-emerald-400 ring-emerald-400/30',
  active: 'bg-brass ring-brass/40 animate-vox-ping',
  pending: 'bg-white/10 ring-white/10',
  error: 'bg-rose-400 ring-rose-400/30',
};

export function PipelineTimeline({ stages, statuses }: {
  stages: readonly string[];
  statuses: Record<string, StageStatus>;
}) {
  return (
    <div className="flex flex-wrap items-center gap-1">
      {stages.map((stage, i) => (
        <React.Fragment key={stage}>
          <div className="flex items-center gap-1.5">
            <span className={`size-2.5 rounded-full ring-2 ${DOT[statuses[stage] ?? 'pending']}`} />
            <span className="font-mono text-[10px] text-zinc-500">{stage.replace(/_/g, ' ')}</span>
          </div>
          {i < stages.length - 1 && <span className="h-px w-3 bg-white/10" />}
        </React.Fragment>
      ))}
    </div>
  );
}
```

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/lib/pipeline.ts crates/vox-gui/ui/src/lib/pipeline.test.ts crates/vox-gui/ui/src/components/PipelineTimeline.tsx
git commit -m "feat(gui): reusable PipelineTimeline + pure stage helpers (vitest)"
```

---

## Task 2: Typed research read commands

**Files:**
- Create: `crates/vox-gui/src/commands/scientia.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`
- Modify: `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Write the research read commands**

Create `crates/vox-gui/src/commands/scientia.rs`. Connection mirrors the research CLI (`crates/vox-cli/src/commands/research/mod.rs` `connect_research_db`):

```rust
//! Typed Scientia-domain read commands (research sessions + publication manifests).
//! Reads go directly to the canonical DB, mirroring the CLI handlers — no CLI
//! stdout parsing and no dependency on the (disabled) HTTP gateway.

#[derive(Debug, serde::Serialize)]
pub struct ResearchSessionDto {
    pub id: i64,
    pub status: String,
    pub query_text: String,
    pub started_at_ms: i64,
    pub finished_at_ms: Option<i64>,
}

#[derive(Debug, serde::Serialize)]
pub struct ResearchDetailDto {
    pub session: ResearchSessionDto,
    pub report_markdown: Option<String>,
    pub artifact_json: Option<String>,
}

async fn connect_canonical_db() -> Result<vox_db::VoxDb, String> {
    let cfg = vox_db::DbConfig::resolve_canonical().map_err(|e| e.to_string())?;
    vox_db::VoxDb::connect(cfg).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_research_sessions(limit: Option<u32>) -> Result<Vec<ResearchSessionDto>, String> {
    let db = connect_canonical_db().await?;
    let rows = db
        .list_recent_research_sessions(limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .map(|r| ResearchSessionDto {
            id: r.id,
            status: r.status.clone(),
            query_text: r.query_text.clone(),
            started_at_ms: r.started_at_ms,
            finished_at_ms: r.finished_at_ms,
        })
        .collect())
}

#[tauri::command]
pub async fn get_research_session_detail(session_id: i64) -> Result<ResearchDetailDto, String> {
    let db = connect_canonical_db().await?;
    let s = db
        .get_research_session(session_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("research session {session_id} not found"))?;
    let artifact = db
        .get_research_artifact(session_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(ResearchDetailDto {
        session: ResearchSessionDto {
            id: s.id,
            status: s.status.clone(),
            query_text: s.query_text.clone(),
            started_at_ms: s.started_at_ms,
            finished_at_ms: s.finished_at_ms,
        },
        report_markdown: artifact.as_ref().map(|a| a.report_markdown.clone()),
        artifact_json: artifact.as_ref().map(|a| a.artifact_json.clone()),
    })
}
```

> Verify against `crates/vox-db/src/research_pipeline.rs`: `list_recent_research_sessions(limit)` (l.96), `get_research_session(id)` (l.65), `get_research_artifact(id)` (l.408). If `get_research_session` returns the record directly instead of `Option`, drop the `.ok_or_else(...)` and bind `s` directly. If `list_recent_research_sessions` takes an `i64`/`usize`, cast `limit.unwrap_or(20)` accordingly.

- [ ] **Step 2: Declare the module + register**

In `crates/vox-gui/src/commands/mod.rs` add `pub mod scientia;`. In `crates/vox-gui/src/main.rs` add to the handler list:

```rust
            commands::scientia::list_research_sessions,
            commands::scientia::get_research_session_detail,
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p vox-gui`
Expected: clean build. (Logic here is field-copying; correctness is verified by the build + the surface in Task 4. No separate unit test — the value lives in the read surface.)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/src/commands/scientia.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs
git commit -m "feat(gui): typed research session read commands"
```

---

## Task 3: Typed publication-manifest read command

**Files:**
- Modify: `crates/vox-gui/src/commands/scientia.rs`
- Modify: `crates/vox-gui/src/main.rs`

- [ ] **Step 1: Add the command**

Append to `crates/vox-gui/src/commands/scientia.rs`. This mirrors `scientia_dashboard()` (`scientia_phase_handlers.rs:411-456`), which uses `VoxDb::connect_default()` + `list_publication_manifests(Some("scientia"), None, 200)`:

```rust
#[derive(Debug, serde::Serialize)]
pub struct PublicationManifestDto {
    pub publication_id: String,
    pub content_type: String,
    pub state: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

#[tauri::command]
pub async fn list_publication_manifests(limit: Option<u32>) -> Result<Vec<PublicationManifestDto>, String> {
    let db = vox_db::VoxDb::connect_default().await.map_err(|e| e.to_string())?;
    let manifests = db
        .list_publication_manifests(Some("scientia"), None, limit.unwrap_or(200) as i64)
        .await
        .map_err(|e| e.to_string())?;
    Ok(manifests
        .iter()
        .map(|m| PublicationManifestDto {
            publication_id: m.publication_id.clone(),
            content_type: m.content_type.clone(),
            state: m.state.clone(),
            created_at_ms: m.created_at_ms,
            updated_at_ms: m.updated_at_ms,
        })
        .collect())
}
```

> Verify the third arg type of `list_publication_manifests` against the DB facade; `scientia_dashboard` passes `200` (untyped literal). Adjust the `as i64` cast if the signature is `usize`/`u32`.

- [ ] **Step 2: Register**

In `crates/vox-gui/src/main.rs` add:

```rust
            commands::scientia::list_publication_manifests,
```

- [ ] **Step 3: Build + commit**

```bash
cargo build -p vox-gui
git add crates/vox-gui/src/commands/scientia.rs crates/vox-gui/src/main.rs
git commit -m "feat(gui): typed list_publication_manifests command"
```

---

## Task 4: Research surface — run + history + detail with timeline

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`

- [ ] **Step 1: Write the Research surface**

Create `crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx`. Reads are typed; the run is the real inline CLI path via `execute_command`:

```tsx
import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { PipelineTimeline } from '../../PipelineTimeline';
import { RESEARCH_STAGES, deriveStages } from '../../../lib/pipeline';

interface ExecuteOutput { exit_code: number; stdout: string; stderr: string; }
interface ResearchSession { id: number; status: string; query_text: string; started_at_ms: number; finished_at_ms: number | null; }
interface ResearchDetail { session: ResearchSession; report_markdown: string | null; artifact_json: string | null; }
interface ResearchResult { answer: string; sources: unknown[]; citations: unknown[]; }

export function ResearchView({ pushToast }: SurfaceDecoratorProps) {
  const [query, setQuery] = useState('');
  const [running, setRunning] = useState(false);
  const [result, setResult] = useState<ResearchResult | null>(null);
  const [sessions, setSessions] = useState<ResearchSession[]>([]);
  const [detail, setDetail] = useState<ResearchDetail | null>(null);

  const loadHistory = useCallback(async () => {
    try {
      setSessions(await invoke<ResearchSession[]>('list_research_sessions', { limit: 25 }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'History load failed', body: String(err) });
    }
  }, [pushToast]);

  useEffect(() => { loadHistory(); }, [loadHistory]);

  const run = async () => {
    if (!query.trim()) return;
    setRunning(true);
    setResult(null);
    try {
      // Inline run (really executes; --async enqueues nothing). --json must precede the trailing query.
      const out = await invoke<ExecuteOutput>('execute_command', {
        path: ['research', 'run'],
        args: { __argv: ['--json', query] },
      });
      if (out.exit_code !== 0) {
        pushToast({ tone: 'warn', title: 'Research run failed', body: out.stderr || `exit ${out.exit_code}` });
      } else {
        setResult(JSON.parse(out.stdout) as ResearchResult);
        await loadHistory();
      }
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Research run failed', body: String(err) });
    } finally {
      setRunning(false);
    }
  };

  const openDetail = async (id: number) => {
    try {
      setDetail(await invoke<ResearchDetail>('get_research_session_detail', { sessionId: id }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Session load failed', body: String(err) });
    }
  };

  return (
    <section className="space-y-4">
      <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Research</h2>

      <div className="flex gap-2">
        <input value={query} onChange={e => setQuery(e.target.value)} placeholder="Ask a research question…"
          className="flex-1 rounded-lg border border-white/10 bg-black/40 px-3 py-2 text-sm text-zinc-200 outline-none focus:border-brass/40" />
        <button onClick={run} disabled={running}
          className="rounded-lg border border-brass/30 bg-brass/10 px-4 py-2 text-sm text-brass hover:bg-brass/20 disabled:opacity-50">
          {running ? 'Running…' : 'Run'}
        </button>
      </div>
      {running && (
        <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages('active')} />
          <div className="mt-2 text-[11px] text-zinc-500">Running inline — this can take a while.</div>
        </div>
      )}
      {result && (
        <div className="rounded-lg border border-emerald-400/20 bg-emerald-500/[0.03] p-3">
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages('completed')} />
          <div className="mt-2 whitespace-pre-wrap text-[13px] text-zinc-200">{result.answer}</div>
          <div className="mt-1 font-mono text-[10px] text-zinc-500">{result.sources.length} sources · {result.citations.length} citations</div>
        </div>
      )}

      <div>
        <div className="mb-2 flex items-center justify-between">
          <span className="font-display text-[12px] uppercase tracking-wide text-zinc-400">Recent sessions</span>
          <button onClick={loadHistory} className="text-[11px] text-zinc-500 hover:text-zinc-200">Refresh</button>
        </div>
        <ul className="space-y-1">
          {sessions.map(s => (
            <li key={s.id}>
              <button onClick={() => openDetail(s.id)}
                className="flex w-full items-center justify-between rounded-lg border border-white/10 bg-white/[0.02] px-3 py-2 text-left hover:bg-white/[0.04]">
                <span className="truncate text-[12px] text-zinc-300">{s.query_text}</span>
                <span className="ml-3 shrink-0 font-mono text-[10px] text-zinc-500">{s.status}</span>
              </button>
            </li>
          ))}
        </ul>
      </div>

      {detail && (
        <div className="rounded-lg border border-white/10 bg-white/[0.02] p-3">
          <div className="mb-2 flex items-center justify-between">
            <span className="text-[12px] text-zinc-300">Session {detail.session.id}</span>
            <button onClick={() => setDetail(null)} className="text-[11px] text-zinc-500 hover:text-zinc-200">Close</button>
          </div>
          <PipelineTimeline stages={RESEARCH_STAGES} statuses={deriveStages(detail.session.status)} />
          <pre className="mt-2 max-h-[360px] overflow-auto whitespace-pre-wrap text-[12px] text-zinc-300">
            {detail.report_markdown ?? detail.artifact_json ?? '(no artifact persisted)'}
          </pre>
        </div>
      )}
    </section>
  );
}
```

- [ ] **Step 2: Swap the decorator from command-cards to the real surface**

In `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`: add the import and replace the `research:` entry (currently a `commandSurface(...)` call) with the component.

```tsx
import { ResearchView } from './Research/ResearchView';
```

Replace the existing `research: commandSurface('Vox Research', ...)` block with:

```tsx
  research: ResearchView,
```

- [ ] **Step 3: Build + commit**

```bash
pnpm --dir crates/vox-gui/ui build
git add crates/vox-gui/ui/src/components/surfaces/Research/ResearchView.tsx crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts
git commit -m "feat(gui): real Research surface — run + history + session timeline"
```

---

## Task 5: Publication stage board surface

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Publications/PublicationsView.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`
- Modify: `crates/vox-gui/ui/src/App.tsx` (`View` union + validation array)

- [ ] **Step 1: Write the board surface (consumes the pure `groupByStage`)**

Create `crates/vox-gui/ui/src/components/surfaces/Publications/PublicationsView.tsx`:

```tsx
import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { SurfaceDecoratorProps } from '../decoratorRegistry';
import { PUBLICATION_STAGES, groupByStage, PublicationManifest } from '../../../lib/pipeline';

export function PublicationsView({ pushToast }: SurfaceDecoratorProps) {
  const [manifests, setManifests] = useState<PublicationManifest[]>([]);
  const [loading, setLoading] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      setManifests(await invoke<PublicationManifest[]>('list_publication_manifests', { limit: 200 }));
    } catch (err) {
      pushToast({ tone: 'warn', title: 'Publications load failed', body: String(err) });
    } finally {
      setLoading(false);
    }
  }, [pushToast]);

  useEffect(() => { refresh(); }, [refresh]);

  const groups = groupByStage(manifests);

  return (
    <section className="space-y-4">
      <div className="flex items-center justify-between">
        <h2 className="font-display text-lg text-zinc-100 tracking-wider uppercase">Publication Pipeline</h2>
        <button onClick={refresh} disabled={loading}
          className="rounded-lg border border-white/10 bg-white/[0.03] px-3 py-1.5 text-xs hover:bg-white/[0.06]">
          {loading ? 'Loading…' : 'Refresh'}
        </button>
      </div>
      <div className="flex gap-3 overflow-x-auto pb-2">
        {PUBLICATION_STAGES.map(stage => (
          <div key={stage} className="w-56 shrink-0">
            <div className="mb-2 flex items-center justify-between">
              <span className="font-mono text-[10px] uppercase tracking-wide text-zinc-400">{stage.replace(/_/g, ' ')}</span>
              <span className="rounded-full bg-white/[0.05] px-1.5 font-mono text-[9px] text-zinc-500">{groups[stage].length}</span>
            </div>
            <div className="space-y-2">
              {groups[stage].map(m => (
                <div key={m.publication_id} className="rounded-lg border border-white/10 bg-white/[0.02] p-2">
                  <div className="truncate font-mono text-[11px] text-zinc-200">{m.publication_id}</div>
                  <div className="text-[10px] text-zinc-500">{m.content_type}</div>
                </div>
              ))}
              {groups[stage].length === 0 && <div className="rounded-lg border border-dashed border-white/5 p-2 text-center text-[10px] text-zinc-600">—</div>}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Register the surface**

In `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts` add the import and the entry:

```tsx
import { PublicationsView } from './Publications/PublicationsView';
```

Add to `surfaceDecorators` (after `claims: ClaimsView,`):

```tsx
  publications: PublicationsView,
```

In `crates/vox-gui/ui/src/App.tsx`: add `| 'publications'` to the `View` union, and `'publications'` to the validation array at line ~230.

- [ ] **Step 3: Build + commit**

```bash
pnpm --dir crates/vox-gui/ui build
git add crates/vox-gui/ui/src/components/surfaces/Publications/PublicationsView.tsx crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui): publication pipeline stage board"
```

---

## Task 6: Register surfaces in the Track A registry + full verification

> Prerequisite: Track A (`gui-surface-registry`) is landed. If Track A is not yet done, skip Step 1 and instead add a `<NavItem>` for `publications` directly in `Sidebar.tsx` (mirroring the existing rows) so the surface is reachable.

**Files:**
- Modify: `contracts/gui/surface-registry.v1.yaml`
- Modify: `contracts/reports/gui-surface-coverage.v1.json` (regenerated)

- [ ] **Step 1: Add the `publications` surface to the registry and regenerate**

In `contracts/gui/surface-registry.v1.yaml`, add:

```yaml
  - { view_key: publications, cli_group: null, representation_tier: curated_decorator, nav_label: Publications, nav_icon: file, nav_group: research, notes: scientia publication lifecycle board }
```

Then regenerate the nav projection and verify:

Run: `cargo run -p vox-cli -- ci gui-surface-registry --write`
Run: `cargo run -p vox-cli -- ci gui-surface-registry`
Expected: up to date; no wiring violations (`'publications'` is now in the App.tsx `View` union, and it appears in `decoratorRegistry`). The sidebar (driven by the generated registry from Track A) now shows the Publications nav item.

- [ ] **Step 2: Regenerate the surface-coverage report (new IPC commands)**

The new `commands::scientia::*` handlers shift the IPC list scraped by `gui_surface_coverage`:

Run: `cargo run -p vox-cli -- ci gui-surface-coverage --write`
Run: `cargo run -p vox-cli -- ci gui-surface-coverage`
Expected: second run passes.

- [ ] **Step 3: All gates green**

Run: `cargo run -p vox-arch-check`
Run: `cargo build -p vox-gui`
Run: `pnpm --dir crates/vox-gui/ui build`
Run: `pnpm --dir crates/vox-gui/ui test`
Run: `cargo run -p vox-cli -- ci gui-catalog-parity`
Expected: all pass.

- [ ] **Step 4: Commit regenerated artifacts**

```bash
git add contracts/gui/surface-registry.v1.yaml crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts contracts/reports/gui-surface-registry.v1.json contracts/reports/gui-surface-coverage.v1.json
git commit -m "chore(gui): register research/publications surfaces; regenerate reports"
```

---

## Self-Review

- **Spec coverage:** reusable stage timeline (Task 1); deep-research pipeline run + history + detail (Tasks 2, 4); publication lifecycle board (Tasks 3, 5); surfaces registered into the Track A SSOT (Task 6). The "represent the full pipeline/workflow" goal is met for both Scientia pipelines within the bounds of what executes today.
- **Reality honored:** no `--async` hollow run (inline only); no speculative REST/WS; coarse timeline because no per-stage signal exists — all three documented in the constraints block and reflected in code comments.
- **Type consistency:** `PublicationManifest` (pipeline.ts) field names match `PublicationManifestDto` serde output. `ResearchSession`/`ResearchDetail` TS shapes match `ResearchSessionDto`/`ResearchDetailDto`. `sessionId` (JS) → `session_id` (Rust) per the Tauri convention used across the codebase.
- **No placeholders:** every code step is complete; the two "verify the DB method signature" notes point at exact file:line anchors to confirm, not gaps to invent.

## Deferred (explicit future enabler — out of scope)

- **Live `/api/v2/scientia/{queue,cost}` REST + `scientia.queue.changed` WS topic.** The shapes (`QueueSnapshot`, `CostRollup`) and the HTTP gateway (`crates/vox-orchestrator-mcp/src/http_gateway/`) exist but the routes do not, and the gateway is disabled by default. Building them (register handlers in `dashboard_api::router()`; add a topic-multiplex to `ws.rs`) would let these surfaces subscribe to live events instead of polling typed commands. This is its own spec→plan cycle.
- **Cost rollup surface** (`build_cost_rollup` has no live-data DB producer yet — a CLI/backend gap to close first).
- **Editable plan preview, replay, manuscript/critic-gate/venue/prereg surfaces** — each is a discrete addition that classifies into the Track A registry when built.
