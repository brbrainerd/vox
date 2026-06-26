---
title: "GUI Reorg Execution Plan 3A (ratified moves/merges/renames/cuts)"
category: "Architecture SSOTs"
status: "READY TO EXECUTE — TDD"
generated: "2026-06-26"
blueprint: docs/agents/gui-ia-blueprint.md
branch: claude/graphify-general-gui-ia
repo: /c/Users/Owner/vox-graphify-gui
scopes_out:
  - "Plan 3B (mens/populi GUI-from-CLI parity workstream — Amendment A)"
  - "Plan 3C (Settings consolidation + Settings/Policies unification — Amendment B)"
---

# GUI Reorg Execution Plan 3A

## Workflow Execution

This section is the dispatch contract for a workflow orchestrator. It states cross-plan ordering, classifies
every existing task `[PARALLEL-SAFE]`/`[SEQUENTIAL]`, and groups independent tasks into fan-out batches. **It
adds no new tasks and changes no task body** — it only sequences and parallelizes what already exists below.

### Cross-plan dependency header

- **Predecessors (must land first):** none. 3A is the **first** plan of the reorg trilogy and depends on no
  other plan. It only requires the ratified blueprint `docs/agents/gui-ia-blueprint.md` (§0/§3/§4/§5/§6),
  which is an input artifact, not a plan to execute.
- **Successors (must wait on 3A):** **Plan 3B** (mens/populi GUI-from-CLI parity, Amendment A) and **Plan 3C**
  (Settings consolidation + Settings/Policies unification, Amendment B). Both depend on 3A's surviving surface
  set + nav skeleton being stable. Do **not** dispatch 3B/3C until 3A's Bundle 7 gate (Task 7.1) is green.
- **Intra-plan keystone:** **Task 6.1** (the `parseViewFromLocation` redirect-map seam) is the keystone. It
  must land before any task that deletes a `#view=` key, because every deletion relies on the redirect to keep
  old deep-links resolving. Tasks 6.2, 4.1, 3a.1, 3b.2, 5.2 each cut a key the redirect map covers.

### Shared-file reality (why most tasks are SEQUENTIAL)

A workflow sub-agent edits + commits one task end-to-end. The plan's tasks overwhelmingly touch the **same
four sync sites** — `crates/vox-gui/ui/src/lib/navigation.ts`, `crates/vox-gui/ui/src/lib/navigation.test.ts`,
`contracts/gui/surface-registry.v1.yaml` (+ its regenerated `surfaceRegistry.generated.ts`), and
`crates/vox-gui/ui/src/components/layout/Sidebar.tsx`. Two sub-agents that both edit `navigation.ts` (or both
run `gui-surface-registry --write` on the YAML) will collide on commit. Therefore a task is **`[PARALLEL-SAFE]`
only if it touches a file set disjoint from its batch-mates**; otherwise it is **`[SEQUENTIAL]`** and the
workflow runs it in series on the navigation/YAML hot path. The redirect keystone (6.1) is a hard barrier
gating the entire structural phase.

### Per-task classification

| Task | Class | Touches (hot files) | Gating reason |
|---|---|---|---|
| 1.1 scientia→Findings | `[PARALLEL-SAFE]` | navigation.ts NAV_LABELS, YAML, test | label-only; conflict-free *if* serialized regen (see batch note) |
| 1.2 oratio→Voice | `[PARALLEL-SAFE]` | navigation.ts NAV_LABELS, YAML, test | label-only |
| 1.3 runs→"Runs" | `[PARALLEL-SAFE]` | navigation.ts NAV_LABELS, **Sidebar.tsx**, test | label-only; Sidebar edit is disjoint from 1.1/1.2 |
| 6.1 redirect seam | `[SEQUENTIAL]` | navigation.ts (new exports + parseViewFromLocation), new test | **KEYSTONE barrier** — blocks all cuts |
| 6.2 cut 6 registry rows | `[SEQUENTIAL]` | YAML→regen, new test | after 6.1; YAML hot path |
| 6.3 knowledge default child guard | `[PARALLEL-SAFE]` | navigation.test.ts only (regression guard) | guard-only; no prod edit |
| 4.1 search→memory reparent | `[SEQUENTIAL]` | navigation.ts (4 maps), Sidebar.tsx, YAML→regen, App note | after 6.1; navigation.ts + YAML hot path |
| 3a.1 claims→scientia | `[SEQUENTIAL]` | navigation.ts, YAML→regen, decoratorRegistry.ts | after 6.1; navigation.ts + YAML hot path |
| 3b.1 Discovery presets | `[PARALLEL-SAFE]` | ActivitySurface.tsx + new test (no nav/YAML) | disjoint file set — can run beside the nav serial chain |
| 3b.2 retire 3 clones, move activity | `[SEQUENTIAL]` | navigation.ts, YAML→regen, decoratorRegistry.ts | after 6.1; navigation.ts + YAML hot path; needs 3b.1's `activity` surface intent |
| 5.1 gamify nav→settings | `[SEQUENTIAL]` | navigation.ts, YAML→regen | navigation.ts + YAML hot path |
| 5.2 matrix→chat rail | `[SEQUENTIAL]` | navigation.ts, YAML→regen, surfaceComponents.tsx | after 6.1; navigation.ts + YAML hot path |
| 2.1 runs named child | `[SEQUENTIAL]` | navigation.ts, YAML→regen (no-op) | navigation.ts hot path |
| 2.2 ADD needs-you | `[SEQUENTIAL]` | navigation.ts, YAML→regen | navigation.ts + YAML hot path |
| 2.3 ADD sub-agents | `[SEQUENTIAL]` | navigation.ts, YAML→regen | navigation.ts + YAML hot path |
| 7.1 full gate | `[SEQUENTIAL]` | runs whole suite, no edits | terminal gate — must run last |

### Fan-out batch grouping

The workflow dispatches batches in order. Within a `parallel` batch, sub-agents run concurrently (their file
sets are disjoint); a `serial` batch runs its tasks one-by-one on the navigation.ts/YAML hot path. Each task
ends in its own commit (write-through-workflow), so the workflow can checkpoint per task.

- **Batch A — `parallel` (fan-out 3):** Tasks **1.1, 1.2, 1.3**. All label-only, disjoint enough to dispatch
  together. 1.1/1.2 both touch `NAV_LABELS` + YAML; if the orchestrator cannot guarantee line-disjoint merges
  on `navigation.ts`/YAML, demote A to serial (run 1.1→1.2→1.3). 1.3's Sidebar.tsx edit is independent. Each
  task regenerates the registry; if regen runs concurrently, serialize the `--write` step (run it once after
  all three land, or let the last committer regen). **Safe-degrade default: run A serially if unsure.**
- **Barrier — Task 6.1 (KEYSTONE):** dispatch **alone**, must complete + commit before Batch C. This is the
  redirect seam every cut depends on.
- **Batch B — `parallel` (fan-out 2), may overlap the serial chain:** Tasks **3b.1** (ActivitySurface presets)
  and **6.3** (knowledge-default-child regression guard). Both touch file sets disjoint from `navigation.ts`'s
  structural edits (3b.1 = ActivitySurface.tsx; 6.3 = navigation.test.ts guard-only). They can run any time
  after Batch A and (for the orchestrator's convenience) alongside the start of Batch C, since neither mutates
  the nav structural maps. 3b.1 must precede 3b.2 (3b.2 relies on the `activity` Discovery surface existing).
- **Batch C — `serial` (the navigation.ts / YAML hot path), strictly after 6.1:** Tasks in this order —
  **6.2 → 4.1 → 3a.1 → 3b.2 → 5.2 → 5.1 → 2.1 → 2.2 → 2.3**. Every one of these mutates `navigation.ts` and/or
  the YAML registry and regenerates the TS, so they are serialized to avoid commit/regen collisions. 3b.2 must
  follow 3b.1 (Batch B) and 6.2 (the clone rows are cut/repointed coherently). Order within C is otherwise the
  blueprint's bundle order with the redirect-dependent cuts (6.2/4.1/3a.1/3b.2/5.2) up front.
- **Batch D — `serial` terminal:** Task **7.1** (whole-suite vitest + typecheck + registry drift gate). Runs
  last, after every Batch C task has landed. Its green output is the precondition for dispatching Plan 3B/3C.

Dependency edges (for a DAG-driven scheduler): `{1.1,1.2,1.3} → 6.1`; `6.1 → {6.2,4.1,3a.1,3b.2,5.2}`;
`3b.1 → 3b.2`; `6.2 → 3b.2`; `{all C tasks} → 7.1`; `6.3` and `3b.1` depend only on Batch A completing.
Tasks **5.1, 2.1, 2.2, 2.3** do not strictly require 6.1 (they ADD/reparent rather than cut a redirected key)
but are placed in serial Batch C because they share the navigation.ts/YAML hot path.

### Commit & git discipline (per task, write-through-workflow)

Every task is self-contained: write failing test → run red → implement → run green → **commit**. A sub-agent
commits its own task using **add + commit only** (never push, never branch-switch, never `git commit -a`),
staging exactly the files that task touched:

```
git -C /c/Users/Owner/vox-graphify-gui add <only-the-files-this-task-edited>
git -C /c/Users/Owner/vox-graphify-gui commit -m "<the task's listed commit message>"
```

Use the exact commit message printed in each task's **Commit:** line. Stage only that task's files (the listed
sync sites + regenerated TS + new/edited test) — do not `git add -A`. No `push`, no `--amend`, no rebase, no
branch creation; the workflow owns branch/merge.

> NOTE: this supersedes the "the human commits — do NOT run `git commit`" discipline note in the Tasks header
> below **for workflow (sub-agent) execution**. Under workflow dispatch each sub-agent DOES commit its own
> task (add + commit only). For manual/human execution, the original "human commits" note still applies.

---

## Goal

Execute the **ratified** structural moves/merges/renames/cuts from `docs/agents/gui-ia-blueprint.md`
(§0 RATIFIED, §3 decision table, §4 executable fields, §5 migration ledger, §6 before/after) — and
**only** those. The two amendment workstreams are out of scope: **Amendment A** (mens/populi GUI-from-CLI
parity) is **Plan 3B**; **Amendment B** (Settings consolidation + Settings/Policies unification, including
the gamify *config* consolidation) is **Plan 3C**. This plan does only the gamify **nav reparent**
(agents→settings), not the gamify config move.

Concretely, after this plan:

- **CUT** the phantom `review` surface row + 5 parent-shell registry rows (`agents`, `commands`,
  `compute`, `workspace`, `knowledge`).
- **MERGE** `claims`+`knowledge`(surface)→`scientia` (relabelled **Findings**); the 4 activity clones
  (`activity`, `archive-panel`, `discovery-inbox`, `discovery-review`)→one **Discovery** surface with
  Inbox/Review/Archive filter presets; `search`→`memory`; `matrix`→the chat execution rail.
- **MOVE** `memory` search→knowledge; `activity`(Discovery) orphan→knowledge; `runs` parent-shell→named
  child under Runs; `gamify` agents→settings (nav only).
- **RENAME** (label-only, key unchanged) `scientia`→"Findings", `oratio`→"Voice", `runs` group label
  "Runs & Approvals"→"Runs".
- **ADD-to-nav (conditional, honesty-gated)** `needs-you` (wire `tool:vox_resolve_approval`, already wired)
  under Runs; `sub-agents` (wire `cmd:list_subagent_tree`, real command is `subagent_tree`, already wired)
  under Agents. Both pass the gate → ADD, not CUT.
- **Migration ledger**: every removed `#view=` key redirects to its surviving target so deep-links/bookmarks
  never silently break.

Ordering note (cross-plan): **3A first** (this plan establishes the surviving surface set + nav skeleton),
then **3B** (mens/populi parity), then **3C** (Settings IA). 3B/3C depend on 3A's nav being stable.

## Architecture (current SSOT topology — verified by reading the files)

There are **FOUR** sync sites, not three — the blueprint's "3 SSOT sites" misses `TOP_NAV_META`:

1. **`crates/vox-gui/ui/src/lib/navigation.ts`** — `PARENT_CHILD_MAP` (line 4), `DEFAULT_CHILD_BY_PARENT`
   (line 35), `TOP_LEVEL_VIEWS` (line 47), `NAV_LABELS` (line 62), `parseViewFromLocation` (line 125),
   `resolveNavigation` (line 146).
2. **`contracts/gui/surface-registry.v1.yaml`** — the *real* SSOT for the registry. The generated TS file
   `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts` is produced by the Rust gate
   `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` (`run(... write=true)` → `generate_ts`).
   **Never hand-edit the `.generated.ts`** — edit the YAML, then run `vox ci gui-surface-registry --write`.
   The gate (`existing_ts.trim() != ts.trim()`) fails CI on drift.
3. **`crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`** — `childRenderer` `switch (viewKey)`
   (line 82) is the component dispatch; `crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts`
   (`surfaceDecorators`, line 45) is consulted first by `childRenderer` (line 78).
4. **`crates/vox-gui/ui/src/components/layout/Sidebar.tsx`** — `TOP_NAV_META` (line 53) holds top-level
   labels/icons *independently* of `NAV_LABELS`; child tabs are derived from `SURFACE_REGISTRY`
   `parentSurface`/`navLabel` (`childTabsByParent`, line 125); top-level list = `TOP_LEVEL_VIEWS` minus
   `settings` (line 138). **The `runs` group label rename must update BOTH `NAV_LABELS.runs` (navigation.ts)
   AND `TOP_NAV_META.runs.label` (Sidebar.tsx).**

Additional consumers found:

- **`crates/vox-gui/ui/src/App.tsx`** — `View` union (line 92ff), `LEGACY_VIEWS`/`KNOWN_VIEWS` (line 119),
  `isKnownView` (line 130), bootstrap `parseViewFromLocation` (lines 348/610). A redirected key must be in
  `LEGACY_VIEWS` to deep-link, so the redirect happens **inside `parseViewFromLocation`** (single seam) so
  App.tsx and any other caller automatically get the remapped key.
- **Tests**: `crates/vox-gui/ui/src/lib/navigation.test.ts` (existing nav assertions),
  `crates/vox-cli/src/commands/ci/gui_surface_registry.rs` `#[cfg(test)] mod tests` (Rust generator tests).

Verified wiring facts (de-risks the conditional ADDs):

- `sub-agents`: `SubAgentsView` (`surfaceComponents`? — no; it is a **decorator**, registered in
  `decoratorRegistry.ts` line 69 `'sub-agents': SubAgentsView`). It calls `subAgentClient.fetchTree()` which
  invokes the **`subagent_tree`** Tauri command (`subAgentClient.ts` line 13). The blueprint cites
  `cmd:list_subagent_tree`; the **actual** command name is `subagent_tree`. The honesty gate is satisfied —
  the component already surfaces a real command. **ADD passes.**
- `needs-you`: `NeedsYouSurface` is wired in `surfaceComponents.tsx` line 163 (`case 'needs-you'`). The
  approve/reject resolve path goes through `vox_resolve_approval` / `vox_pending_approvals`
  (`useAgentApprovals.ts` line 18, dispatch in `crates/vox-orchestrator-mcp/src/dispatch.rs`). **ADD passes.**
- `activity` Discovery merge: all four clones already render the **same** `ActivitySurface`/Activity
  component (decorators `discovery-review`/`discovery-inbox`/`archive-panel` in `decoratorRegistry.ts` map to
  distinct Scientia sub-components, while `activity` → `ActivitySurface`). The merge collapses the **nav
  keys** to one `activity` Discovery surface and re-expresses Inbox/Review/Archive as **filter presets** on
  the `activity` surface; the absorbed Scientia decorator components are removed from nav (their files stay,
  unreferenced, until a later cleanup — not in scope).

## Tech Stack

- Frontend tests: `cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run <file>`
- Typecheck: `cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm typecheck`
- Registry regen: `cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry --write`
  (or the installed `vox ci gui-surface-registry --write`). Drift check (no write):
  `cargo run -p vox-cli -- ci gui-surface-registry`.
- Rust generator unit tests: `cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli gui_surface_registry`

## Spec (acceptance)

1. No surviving nav/registry/dispatch references `review`, the 5 parent-shell *surface rows*, `claims`,
   `search` (as a top-level), `discovery-inbox`, `discovery-review`, `archive-panel`, or `matrix` as a nav
   destination.
2. Every removed `#view=` key resolves (via `parseViewFromLocation`) to its blueprint §5 target.
3. `vox ci gui-surface-registry` passes (no drift) after the YAML edits + regen.
4. `pnpm vitest run` (nav + activity + redirect specs) and `pnpm typecheck` are green.
5. The Discovery surface shows Inbox/Review/Archive presets; the absorbed keys redirect to `activity`.
6. `needs-you` appears under Runs and `sub-agents` under Agents, each surfacing a real command.

---

# Tasks

> Discipline: each task is **(a) write a failing test, (b) run it red, (c) implement, (d) run it green,
> (e) commit**. Commits are listed but **the human commits** (per instruction) — do NOT run `git commit`;
> the "commit" line records the message to use.

## Bundle 1 — Retire Latin labels (label-only, no redirects)

### Task 1.1 — `scientia` label → "Findings" [PARALLEL-SAFE] (Batch A)

**Test (red).** Edit `crates/vox-gui/ui/src/lib/navigation.test.ts`, add inside the existing `describe`:

```ts
  it('labels scientia as Findings (key unchanged)', () => {
    expect(labelForNavKey('scientia')).toBe('Findings');
    expect(resolveNavigation('scientia').child).toBe('scientia');
  });
```

Add `labelForNavKey` to the import on line 2:
`import { resolveNavigation, parseViewFromLocation, breadcrumbsForView, labelForNavKey } from './navigation';`

Run: `cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run src/lib/navigation.test.ts`
Expected: FAIL — `expected 'Scientia' to be 'Findings'`.

**Implement.** In `crates/vox-gui/ui/src/lib/navigation.ts` line 86 change:
`  scientia: 'Scientia',` → `  scientia: 'Findings',`

Also regenerate the registry navLabel. In `contracts/gui/surface-registry.v1.yaml` line 735 change
`  nav_label: Scientia` → `  nav_label: Findings` (the `scientia` entry, `cli_group: scientia`).
Then run `cargo run -p vox-cli -- ci gui-surface-registry --write`.

Run vitest again → PASS. Run `pnpm typecheck` → green.

**Commit:** `feat(gui-reorg): relabel scientia → Findings (Bundle 1)`

### Task 1.2 — `oratio` label → "Voice" [PARALLEL-SAFE] (Batch A)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('labels oratio as Voice (key unchanged)', () => {
    expect(labelForNavKey('oratio')).toBe('Voice');
  });
```
Run red.

**Implement.** `navigation.ts` line 93 `  oratio: 'Oratio',` → `  oratio: 'Voice',`.
`surface-registry.v1.yaml` line 599 `  nav_label: Oratio` → `  nav_label: Voice`. Regen. Green + typecheck.

> The optional `component_dir Loquela→Voice` code refactor (blueprint §4 RENAME row) is **deferred** — it is
> a label-independent code rename and not required for the IA reorg. Note it as a follow-up; do NOT do it here.

**Commit:** `feat(gui-reorg): relabel oratio → Voice (Bundle 1)`

### Task 1.3 — `runs` group label "Runs & Approvals" → "Runs" (touches TWO sites) [PARALLEL-SAFE] (Batch A)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('labels runs group as Runs', () => {
    expect(labelForNavKey('runs')).toBe('Runs');
  });
```
Run red (current value `'Runs & Approvals'`).

**Implement — site 1 (navigation.ts):** line 65 `  runs: 'Runs & Approvals',` → `  runs: 'Runs',`.

**Implement — site 2 (Sidebar.tsx):** `TOP_NAV_META.runs` line 56
`  runs: { label: 'Runs & Approvals', icon: 'scale' },` → `  runs: { label: 'Runs', icon: 'scale' },`.
Also update the aria-label/title fallbacks at Sidebar.tsx lines 234-238 and 284-288 that hardcode
`'Runs and Approvals'` — change the human strings to `'Runs'` / `Runs, ${approvalsPending} pending`.

Run vitest green. `pnpm typecheck` green.

> `mens`/`populi` renames are **Plan 3B** (Amendment A wires them; the rename rides that workstream). Do
> NOT touch their labels here.

**Commit:** `feat(gui-reorg): relabel Runs group, drop "& Approvals" (Bundle 1)`

---

## Bundle 6 — Delete registry phantom + 5 parent-shells (Group C, structural)

### Task 6.1 — Add the redirect-map seam + a deep-link redirect test FIRST [SEQUENTIAL] (KEYSTONE barrier)

This must precede the deletions so removed keys never break.

**Test (red).** New file `crates/vox-gui/ui/src/lib/navigationRedirect.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { parseViewFromLocation } from './navigation';

describe('parseViewFromLocation deep-link redirects (migration ledger §5)', () => {
  const cases: Array<[string, string]> = [
    ['search', 'memory'],
    ['claims', 'scientia'],
    ['knowledge', 'scientia'],
    ['review', 'scientia'],
    ['archive-panel', 'activity'],
    ['discovery-inbox', 'activity'],
    ['discovery-review', 'activity'],
    ['matrix', 'chat'],
  ];
  it.each(cases)('#view=%s redirects to %s', (oldKey, newKey) => {
    expect(parseViewFromLocation({ hash: `#view=${oldKey}`, search: '' })).toBe(newKey);
    expect(parseViewFromLocation({ hash: '', search: `?view=${oldKey}` })).toBe(newKey);
  });
  it('non-redirected keys pass through unchanged', () => {
    expect(parseViewFromLocation({ hash: '#view=memory', search: '' })).toBe('memory');
    expect(parseViewFromLocation({ hash: '#view=dashboard', search: '' })).toBe('dashboard');
  });
});
```

Run: `pnpm vitest run src/lib/navigationRedirect.test.ts` → FAIL (e.g. `search` returns `'search'`).

**Implement.** In `navigation.ts`, above `parseViewFromLocation` (line 125) add:

```ts
/**
 * Migration ledger (gui-ia-blueprint §5): removed `#view=` keys remap to their surviving
 * target so old deep-links/bookmarks resolve. Silent alias for one release, then hard-remove.
 */
export const VIEW_REDIRECTS: Record<string, string> = {
  search: 'memory',
  claims: 'scientia',
  knowledge: 'scientia',
  review: 'scientia',
  'archive-panel': 'activity',
  'discovery-inbox': 'activity',
  'discovery-review': 'activity',
  matrix: 'chat',
};

/** Apply the migration redirect map; identity for live keys. */
export function redirectViewKey(key: string): string {
  return VIEW_REDIRECTS[key] ?? key;
}
```

Then change `parseViewFromLocation` so both return paths pass through the redirect:

```ts
export function parseViewFromLocation(loc: Pick<Location, 'hash' | 'search'>): string | null {
  if (loc.hash.startsWith(VIEW_HASH_PREFIX)) {
    const key = decodeURIComponent(loc.hash.slice(VIEW_HASH_PREFIX.length));
    return key ? redirectViewKey(key) : null;
  }
  const params = new URLSearchParams(loc.search);
  const q = params.get('view');
  return q && q.length > 0 ? redirectViewKey(q) : null;
}
```

> Note: `navigation.test.ts:18` already asserts `#view=console`→`'console'` and `?view=memory`→`'memory'`;
> both are non-redirected and still pass. Re-run `pnpm vitest run src/lib/navigation.test.ts` to confirm.

Run both nav test files → green. `pnpm typecheck` → green.

**Commit:** `feat(gui-reorg): add migration redirect map in parseViewFromLocation (ledger §5)`

### Task 6.2 — CUT the 6 registry rows (phantom `review` + 5 parent-shells) [SEQUENTIAL] (Batch C, after 6.1)

**Test (red).** New `crates/vox-gui/ui/src/generated/surfaceRegistry.cut.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { SURFACE_REGISTRY } from './surfaceRegistry.generated';

describe('registry parent-shell / phantom CUTs (Bundle 6)', () => {
  // The phantom `review` row and the 5 parent-shell *surface* rows are deleted.
  // Note: `agents`/`commands`/`compute`/`workspace`/`knowledge` survive only as
  // navigation.ts TOP_LEVEL/group keys, NOT as registry surface rows.
  it.each(['review', 'agents', 'commands', 'compute', 'workspace', 'knowledge'])(
    'has no surface row for %s',
    (key) => {
      expect(SURFACE_REGISTRY.find((e) => e.viewKey === key)).toBeUndefined();
    },
  );
  it('keeps the live children that the shells mirrored', () => {
    for (const key of ['dashboard', 'catalog', 'models', 'console', 'scientia']) {
      expect(SURFACE_REGISTRY.find((e) => e.viewKey === key)).toBeTruthy();
    }
  });
});
```

Run: `pnpm vitest run src/generated/surfaceRegistry.cut.test.ts` → FAIL (rows still present).

**Implement.** In `contracts/gui/surface-registry.v1.yaml` delete these six surface blocks (identified by
`view_key`): `review` (lines 180-187), `agents` (12-19), `commands` (60-67), `compute` (68-75),
`workspace` (220-227), `knowledge` (132-139). Leave every `cli_group`-only `none`-tier row untouched and
leave the live children (`dashboard`, `catalog`, `models`, `console`, `scientia`) untouched.

Regen: `cargo run -p vox-cli -- ci gui-surface-registry --write`.

> Drift guard: the generator re-sorts by `(cli_group, view_key)`; deleting rows is safe. Confirm
> `cargo run -p vox-cli -- ci gui-surface-registry` (no `--write`) prints "up to date" (no drift) and does
> not re-introduce the rows. The `wiring_violations` check only fires for `curated_decorator`/`live_backend`
> rows whose `view_key` is absent from App.tsx — since we deleted the rows, no violation. `review` was
> `curated_decorator` but is gone, so its prior (latent) requirement disappears.

Run vitest green. Note `surfaceComponents.tsx` has **no** `case 'review'`/`'agents'`/`'commands'`/
`'workspace'`/`'knowledge'` (verified — they were registry-only), so dispatch needs no edit.

**Commit:** `feat(gui-reorg): cut phantom review + 5 parent-shell registry rows (Bundle 6)`

### Task 6.3 — Set `DEFAULT_CHILD_BY_PARENT.knowledge` to a surviving child [PARALLEL-SAFE] (Batch B)

After `knowledge`(surface) is cut and `claims` merges away, the `knowledge` group's default child must point
at a surviving surface (blueprint §5 row "knowledge").

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('knowledge parent resolves to scientia default child', () => {
    const nav = resolveNavigation('knowledge');
    expect(nav.parent).toBe('knowledge');
    expect(nav.child).toBe('scientia');
  });
```
`DEFAULT_CHILD_BY_PARENT.knowledge` is already `'scientia'` (line 42) — this test likely passes already;
keep it as a **regression guard** so a later edit can't silently break it. Run it green now.

**Commit:** `test(gui-reorg): guard knowledge default child = scientia`

---

## Bundle 4 — Kill Search↔Memory collision (`search`→`memory`, MOVE memory under knowledge)

### Task 4.1 — Reparent `memory` to knowledge; retire `search` top-level [SEQUENTIAL] (Batch C, after 6.1)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('memory now lives under knowledge, not search', () => {
    const nav = resolveNavigation('memory');
    expect(nav.parent).toBe('knowledge');
    expect(nav.child).toBe('memory');
  });
  it('search is no longer a top-level view', () => {
    expect((TOP_LEVEL_VIEWS as readonly string[]).includes('search')).toBe(false);
  });
```
Import `TOP_LEVEL_VIEWS` on line 2. Run red (memory→search currently; search in TOP_LEVEL_VIEWS).

**Implement (navigation.ts):**
- Line 17: `  memory: { parent: 'search', child: 'memory' },` → `  memory: { parent: 'knowledge', child: 'memory' },`
- Line 41: remove `  search: 'memory',` from `DEFAULT_CHILD_BY_PARENT`.
- Lines 47-57 `TOP_LEVEL_VIEWS`: remove the `'search',` entry.
- Line 68 `NAV_LABELS`: remove `  search: 'Search',` (optional; harmless to keep, but the key is gone).

**Implement (Sidebar.tsx):** `TOP_NAV_META` line 59 remove `  search: { label: 'Search', icon: 'search' },`
(it would otherwise render a dead top-level since `visibleTopLevel` iterates `TOP_LEVEL_VIEWS` — already
removed there, so this is cleanup to keep the map honest).

**Implement (registry YAML):** the `search` surface row (`view_key: search`, lines 196-203) and the `memory`
row `parent_surface: search` (line 562) must update:
- Delete the `search` surface row (lines 196-203).
- `memory` row: `parent_surface: search` → `parent_surface: knowledge` (line 562). This makes the Sidebar's
  `childTabsByParent` list `memory` under the `knowledge` parent (matches blueprint §6 AFTER tree).
Regen: `cargo run -p vox-cli -- ci gui-surface-registry --write`.

**Implement (App.tsx):** `LEGACY_VIEWS` (line 119) — keep `'search'` in the list so `#view=search`
deep-links still validate *after* redirect. But the redirect (Task 6.1) already maps `search`→`memory`
before App sees it, so `'memory'` must be present (it is, line 120). `'search'` can stay in LEGACY_VIEWS as a
harmless alias; no edit required. (Document this in the commit body.)

Run vitest (`navigation.test.ts` + redirect test) green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): retire search top-level, reparent memory under knowledge (Bundle 4)`

---

## Bundle 3a — MERGE claims + knowledge(surface) → scientia (Findings)

### Task 3a.1 — Retire `claims` nav child; redirect to `scientia` [SEQUENTIAL] (Batch C, after 6.1)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('claims is no longer a knowledge child', () => {
    // claims merged into scientia; resolveNavigation falls through to identity,
    // but the deep-link redirect maps it to scientia.
    expect(PARENT_CHILD_MAP['claims']).toBeUndefined();
  });
```
Import `PARENT_CHILD_MAP` on line 2. Run red.

**Implement (navigation.ts):** delete line 21 `  claims: { parent: 'knowledge', child: 'claims' },`. Remove
`  claims: 'Claims',` from `NAV_LABELS` (line 89, optional cleanup).

**Implement (registry YAML):** delete the `claims` surface row (`view_key: claims`, lines 52-59). Regen.

**Implement (decoratorRegistry.ts):** remove the `claims: ClaimsView,` registration (line 50) and the
`review: DiscoveryReviewView,` registration (line 51 — `review` is being cut) so the merged keys no longer
render their own decorator. Leave `scientia: ScientiaDashboard` (line 46) intact — that is the absorber. The
`ClaimsView`/`DiscoveryReviewView` files remain on disk, now unreferenced (cleanup out of scope).

> `claims` deep-link already redirects to `scientia` (Task 6.1). `surfaceComponents.tsx` has no
> `case 'claims'` (it was decorator-only) → no dispatch edit.

Run the redirect test + nav test green. `pnpm typecheck` green (verify no dangling imports: if removing the
decorator entries leaves `ClaimsView`/`DiscoveryReviewView` imports unused, delete those two import lines at
`decoratorRegistry.ts` lines 4 and 5 too — typecheck/lint will flag them).

**Commit:** `feat(gui-reorg): merge claims (+review) into scientia/Findings (Bundle 3a)`

---

## Bundle 3b — MERGE 4 activity clones → one Discovery surface (Inbox/Review/Archive presets)

### Task 3b.1 — Add filter presets to the Activity (Discovery) surface [PARALLEL-SAFE] (Batch B, before 3b.2)

The four clones all already render via the `activity` key's `ActivitySurface` OR via Scientia decorators
(`discovery-inbox`/`discovery-review`/`archive-panel`). The merge: keep `activity` → `ActivitySurface` as the
single Discovery home, add **named presets** (Inbox/Review/Archive) as a preset selector that seeds the
existing `kindFilter`, and redirect the three absorbed keys to `activity` (already done in Task 6.1).

**Test (red).** New `crates/vox-gui/ui/src/components/surfaces/Activity/ActivitySurface.presets.test.tsx`:

```tsx
import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import { ActivitySurface, DISCOVERY_PRESETS } from './ActivitySurface';

vi.mock('../../../transport', () => ({
  activityQuery: vi.fn().mockResolvedValue([]),
  listenActivityAppended: vi.fn().mockResolvedValue(() => {}),
  listenAgentEvents: vi.fn().mockResolvedValue(() => {}),
}));

describe('Discovery surface presets (Bundle 3b)', () => {
  it('exposes Inbox/Review/Archive presets', () => {
    expect(DISCOVERY_PRESETS.map((p) => p.label)).toEqual(['Inbox', 'Review', 'Archive']);
  });
  it('renders a preset selector', () => {
    render(<ActivitySurface pushToast={() => {}} />);
    expect(screen.getByTestId('discovery-preset-Inbox')).toBeTruthy();
    expect(screen.getByTestId('discovery-preset-Review')).toBeTruthy();
    expect(screen.getByTestId('discovery-preset-Archive')).toBeTruthy();
  });
});
```

Run: `pnpm vitest run src/components/surfaces/Activity/ActivitySurface.presets.test.tsx` → FAIL
(`DISCOVERY_PRESETS` undefined).

**Implement (ActivitySurface.tsx).** Add an exported preset list and a selector row. Near the top of the file
(after imports), add:

```tsx
export interface DiscoveryPreset { label: 'Inbox' | 'Review' | 'Archive'; kind: string | null; }
/** Named presets that re-express the 3 absorbed activity clones as filters (blueprint §4 MERGE). */
export const DISCOVERY_PRESETS: DiscoveryPreset[] = [
  { label: 'Inbox', kind: null },
  { label: 'Review', kind: 'TaskCompleted' },
  { label: 'Archive', kind: 'WorkflowCompleted' },
];
```

In the `ActivitySurface` component body add preset state and a selector. After the existing
`const [kindFilter, setKindFilter] = useState<string>('');` (line 251) add:

```tsx
  const [preset, setPreset] = useState<DiscoveryPreset['label']>('Inbox');
  const applyPreset = useCallback((p: DiscoveryPreset) => {
    setPreset(p.label);
    setKindFilter(p.kind ?? '');
  }, []);
```

In the JSX, just inside the header block (after the `<p>` description, ~line 321) insert a preset toolbar:

```tsx
        <div className="flex gap-1.5" role="tablist" aria-label="Discovery presets">
          {DISCOVERY_PRESETS.map((p) => (
            <button
              key={p.label}
              type="button"
              role="tab"
              aria-selected={preset === p.label}
              data-testid={`discovery-preset-${p.label}`}
              onClick={() => applyPreset(p)}
              className={`px-2.5 py-1 rounded text-xs font-medium transition-colors ${
                preset === p.label
                  ? 'bg-zinc-700 text-zinc-100'
                  : 'bg-zinc-900 border border-zinc-800 text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {p.label}
            </button>
          ))}
        </div>
```

Rename the surface heading text (line 318 `Agent Activity Timeline`) → `Discovery`. Run vitest green.
`pnpm typecheck` green.

**Commit:** `feat(gui-reorg): Discovery surface with Inbox/Review/Archive presets (Bundle 3b)`

### Task 3b.2 — Retire the 3 absorbed nav keys; MOVE `activity` into knowledge [SEQUENTIAL] (Batch C, after 6.1+6.2+3b.1)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('discovery clones retired from PARENT_CHILD_MAP', () => {
    for (const k of ['discovery-inbox', 'discovery-review', 'archive-panel']) {
      expect(PARENT_CHILD_MAP[k]).toBeUndefined();
    }
  });
  it('activity is now a knowledge child (Discovery home)', () => {
    const nav = resolveNavigation('activity');
    expect(nav.parent).toBe('knowledge');
    expect(nav.child).toBe('activity');
  });
```
Run red.

**Implement (navigation.ts):**
- Delete line 20 `  'discovery-review': { parent: 'knowledge', child: 'discovery-review' },`
- Delete line 30 `  'discovery-inbox': { parent: 'knowledge', child: 'discovery-inbox' },`
- Delete line 31 `  'archive-panel': { parent: 'knowledge', child: 'archive-panel' },`
- **ADD** `  activity: { parent: 'knowledge', child: 'activity' },` to `PARENT_CHILD_MAP`.
- In `NAV_LABELS`: remove `'discovery-review'`, `'discovery-inbox'`, `'archive-panel'` (lines 87, 97, 98);
  change the `activity` label by adding `  activity: 'Discovery',` (no activity entry exists today).

**Implement (registry YAML):**
- Delete surface rows `discovery-inbox` (lines 100-107), `discovery-review` (108-115), `archive-panel`
  (28-35).
- `activity` row (lines 4-11): `parent_surface: null` → `parent_surface: knowledge`; `nav_label: Activity`
  → `nav_label: Discovery`. Regen.

**Implement (decoratorRegistry.ts):** remove `'discovery-review': DiscoveryReview,` (line 47),
`'discovery-inbox': DiscoveryInbox,` (line 48), `'archive-panel': ArchivePanel,` (line 49) and their now-unused
imports (lines 5-8). `activity` is dispatched by `surfaceComponents.tsx` `case 'activity'` (line 161) →
`ActivitySurface`, which is unaffected.

Run vitest (nav + redirect + presets) green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): move activity→knowledge as Discovery, retire 3 clones (Bundle 3b)`

---

## Bundle 5 — Tighten Agents (`gamify`→settings nav reparent; `matrix`→chat rail)

### Task 5.1 — Reparent `gamify` agents→settings (NAV ONLY — config move is Plan 3C) [SEQUENTIAL] (Batch C)

**Test (red).** Replace the existing `gamify resolves under agents parent` test (lines 34-37 of
`navigation.test.ts`) with:

```ts
  it('gamify now resolves under settings parent (nav reparent only)', () => {
    expect(resolveNavigation('gamify').parent).toBe('settings');
    expect(resolveNavigation('gamify').child).toBe('gamify');
  });
```
Run red.

**Implement (navigation.ts):** line 29 `  gamify: { parent: 'agents', child: 'gamify' },` →
`  gamify: { parent: 'settings', child: 'gamify' },`.

**Implement (registry YAML):** the `gamify` row (`cli_group: ludus`, lines 540-547) has
`parent_surface: null`; set `parent_surface: settings` so the Sidebar lists Gamify under Settings. Regen.

> The Sidebar renders Settings + its children from `SURFACE_REGISTRY` `parentSurface` (line 290 region uses
> `coverage`); with `gamify` `parent_surface: settings`, the existing `childTabsByParent` logic surfaces it.
> Verify visually-equivalent via the cut/registry test below. Do NOT move any gamify *config* — that is
> Plan 3C (Amendment B).

Run vitest green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): reparent gamify nav agents→settings (Bundle 5, nav only)`

### Task 5.2 — Fold `matrix` into the chat execution rail; retire the matrix nav key [SEQUENTIAL] (Batch C, after 6.1)

The blueprint folds the single `nudge_routing_intention` command inline into the chat rail. Minimal honest
move: retire `matrix` as a nav destination (redirect → `chat`, already done in 6.1), keep the `Matrix`
component reachable only if/when the rail control is added. For this plan we do the **nav retirement + redirect**
and leave a TODO marker for the inline rail control (a UI affordance, low-risk to defer; the routing nudge is
not lost — it is reachable via the redirect target `chat`).

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('matrix retired from PARENT_CHILD_MAP (folded into chat)', () => {
    expect(PARENT_CHILD_MAP['matrix']).toBeUndefined();
  });
```
Run red.

**Implement (navigation.ts):** delete line 7 `  matrix: { parent: 'agents', child: 'matrix' },`. Remove
`  matrix: 'Matrix',` from `NAV_LABELS` (line 76, optional).

**Implement (registry YAML):** delete the `matrix` surface row (`view_key: matrix`, lines 140-147). Regen.

**Implement (surfaceComponents.tsx):** the `case 'matrix'` (line 111) can stay (harmless dead branch) OR be
removed. Remove it + the `Matrix` import (line 5) to keep dispatch honest; if removed, also confirm no other
file imports `Matrix` for nav. (Grep `from '../surfaces/Matrix/Matrix'` — only this file.) `matrix` deep-links
redirect to `chat` (Task 6.1).

> **Deferred (documented TODO, not this plan):** surface the routing nudge inline as a ChatExecutionRail
> intent button. Add a `// TODO(plan3a-followup): inline matrix nudge_routing_intention in ChatExecutionRail`
> comment in `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx`. This is a UI affordance
> follow-up; the command remains reachable and nothing is silently dropped.

Run vitest (nav + redirect) green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): retire matrix nav key, redirect to chat rail (Bundle 5)`

---

## Bundle 2 — Fold orphan-nav surfaces (runs child + conditional ADDs)

### Task 2.1 — `runs` parent-shell → named `runs` child; default child = runs [SEQUENTIAL] (Batch C)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('runs parent resolves to its own runs child (not approvals)', () => {
    const nav = resolveNavigation('runs');
    expect(nav.parent).toBe('runs');
    expect(nav.child).toBe('runs');
  });
```
Run red (today `DEFAULT_CHILD_BY_PARENT.runs = 'approvals'`, so `resolveNavigation('runs')` →
parent `runs`, child... currently `runs` is a TOP_LEVEL? No — `runs` is NOT in TOP_LEVEL_VIEWS; it is a group
key with a default child. Verify: `runs` is in `DEFAULT_CHILD_BY_PARENT` (line 37) and `NAV_LABELS` but not
`TOP_LEVEL_VIEWS`. `resolveNavigation('runs')` falls to the final identity branch → `{parent:'runs',
child:'runs'}` already. The blueprint wants the **default child** to be `runs` so navigating the Runs group
lands on the scoreboard. The meaningful change is `DEFAULT_CHILD_BY_PARENT.runs` + adding `runs` to
`PARENT_CHILD_MAP`.)

**Implement (navigation.ts):**
- Line 37 `  runs: 'approvals',` → `  runs: 'runs',` in `DEFAULT_CHILD_BY_PARENT`.
- **ADD** `  runs: { parent: 'runs', child: 'runs' },` to `PARENT_CHILD_MAP` (so the Sidebar tab + breadcrumb
  treat `runs` as a first-class child).

**Implement (registry YAML):** `runs` surface row (lines 188-195) already has `parent_surface: runs`,
`nav_label: Runs` — no change needed; confirm it survives (it is NOT one of the cut shells). Regen is a no-op
but run it to be safe.

Run vitest green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): make runs a named child, default Runs landing = scoreboard (Bundle 2)`

### Task 2.2 — ADD `needs-you` under Runs (honesty gate: `vox_resolve_approval` wired) [SEQUENTIAL] (Batch C)

**Gate check (do first, record evidence).** Confirm the command is wired:
`cd /c/Users/Owner/vox-graphify-gui && rg -n "vox_resolve_approval|vox_pending_approvals" crates/vox-gui/ui/src crates/vox-orchestrator-mcp/src`
Expected: hits in `useAgentApprovals.ts` (line 18) and `dispatch.rs`. **Gate passes → ADD (not CUT).**

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('needs-you is wired under Runs', () => {
    const nav = resolveNavigation('needs-you');
    expect(nav.parent).toBe('runs');
    expect(nav.child).toBe('needs-you');
  });
```
Run red.

**Implement (navigation.ts):** ADD `  'needs-you': { parent: 'runs', child: 'needs-you' },` to
`PARENT_CHILD_MAP`; ADD `  'needs-you': 'Needs You',` to `NAV_LABELS`.

**Implement (registry YAML):** `needs-you` row (lines 156-163) has `parent_surface: null`; set
`parent_surface: runs` so the Sidebar lists it under Runs. Regen.

`needs-you` already dispatches (`surfaceComponents.tsx` line 163 `case 'needs-you'`) and is in `LEGACY_VIEWS`
(App.tsx line 124). No dispatch/App edit needed.

Run vitest green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): ADD needs-you to nav under Runs (wired vox_resolve_approval) (Bundle 2)`

### Task 2.3 — ADD `sub-agents` under Agents (honesty gate: `subagent_tree` wired) [SEQUENTIAL] (Batch C)

**Gate check.** `rg -n "subagent_tree|list_subagent_tree" crates/vox-gui/ui/src crates/vox-orchestrator-mcp/src`
Expected: `subAgentClient.ts:13` invokes `'subagent_tree'`; dispatch in orchestrator. The blueprint's cited
`list_subagent_tree` is the **logical** name; the **wired** command is `subagent_tree`. Gate satisfied (a real
command is surfaced) → **ADD**. (If, contrary to this evidence, the command were absent, this task reverts to
a CUT: delete the `sub-agents` registry row + `decoratorRegistry` entry instead. Evidence says ADD.)

**Test (red).** Add to `navigation.test.ts`:

```ts
  it('sub-agents is wired under Agents', () => {
    const nav = resolveNavigation('sub-agents');
    expect(nav.parent).toBe('agents');
    expect(nav.child).toBe('sub-agents');
  });
```
Run red.

**Implement (navigation.ts):** ADD `  'sub-agents': { parent: 'agents', child: 'sub-agents' },` to
`PARENT_CHILD_MAP`; ADD `  'sub-agents': 'Sub-Agents',` to `NAV_LABELS`.

**Implement (registry YAML):** `sub-agents` row (lines 204-211) has `parent_surface: compute`; change to
`parent_surface: agents` (blueprint §6 places sub-agents under Agents). Regen.

`sub-agents` renders via `decoratorRegistry.ts` line 69 (`SubAgentsView`); `App.tsx` `LEGACY_VIEWS` includes
`'sub-agents'` (line 124). No further edit.

Run vitest green. `pnpm typecheck` green.

**Commit:** `feat(gui-reorg): ADD sub-agents to nav under Agents (wired subagent_tree) (Bundle 2)`

---

## Bundle 7 (final) — Whole-suite + drift gate

### Task 7.1 — Full vitest + typecheck + registry drift gate [SEQUENTIAL] (Batch D, terminal)

Run, in order, and paste the tail of each into the commit body:

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm vitest run
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && pnpm typecheck
cd /c/Users/Owner/vox-graphify-gui && cargo run -p vox-cli -- ci gui-surface-registry
cd /c/Users/Owner/vox-graphify-gui && cargo test -p vox-cli gui_surface_registry
```

Expected:
- `pnpm vitest run`: all suites pass (pre-existing Axis-branding failures, if any, are unrelated — record
  count; do not "fix" by editing branding).
- `pnpm typecheck`: exit 0.
- `vox ci gui-surface-registry`: prints `gui-surface-registry: registry and generated TS are up to date`
  (no drift). If it reports drift, you forgot a `--write` after a YAML edit — re-run `--write`.
- `cargo test -p vox-cli gui_surface_registry`: the 5 generator unit tests pass (unchanged by this plan).

If any surface that was cut still appears: grep the four sync sites:
`rg -n "review|'matrix'|'claims'|'search'|discovery-inbox|discovery-review|archive-panel" crates/vox-gui/ui/src/lib/navigation.ts crates/vox-gui/ui/src/components/layout/Sidebar.tsx crates/vox-gui/ui/src/components/surfaces/decoratorRegistry.ts contracts/gui/surface-registry.v1.yaml`

**Commit:** `test(gui-reorg): full vitest + typecheck + registry drift gate green (3A complete)`

---

# Self-Review — blueprint decision → task map

| Blueprint unit (§3/§4/§5) | Verb | Task(s) | Sync sites touched |
|---|---|---|---|
| `scientia` → "Findings" | RENAME | 1.1 | navigation.ts NAV_LABELS, YAML→regen |
| `oratio` → "Voice" | RENAME | 1.2 | NAV_LABELS, YAML→regen |
| `runs` group → "Runs" | RENAME | 1.3 | NAV_LABELS **+ Sidebar TOP_NAV_META + aria** |
| `review` (phantom) | CUT | 6.2 + redirect 6.1 | YAML→regen; redirect map |
| `agents`/`commands`/`compute`/`workspace`/`knowledge` shells | CUT | 6.2 | YAML→regen (keys survive in navigation.ts) |
| `knowledge` default child | (support) | 6.3 | DEFAULT_CHILD_BY_PARENT |
| `search` → `memory` | MERGE | 4.1 + redirect 6.1 | navigation.ts (4 maps) + Sidebar + YAML→regen + App note |
| `memory` search→knowledge | MOVE | 4.1 | PARENT_CHILD_MAP + YAML parent_surface |
| `claims` → `scientia` | MERGE | 3a.1 + redirect 6.1 | PARENT_CHILD_MAP, NAV_LABELS, YAML, decoratorRegistry |
| 4 activity clones → Discovery | MERGE | 3b.1 (presets) + 3b.2 (nav) + redirect 6.1 | ActivitySurface, PARENT_CHILD_MAP, NAV_LABELS, YAML, decoratorRegistry |
| `activity` orphan→knowledge | MOVE | 3b.2 | PARENT_CHILD_MAP + YAML parent_surface |
| `gamify` agents→settings (nav) | MOVE | 5.1 | PARENT_CHILD_MAP + YAML parent_surface |
| `matrix` → chat rail | MERGE | 5.2 + redirect 6.1 | PARENT_CHILD_MAP, YAML, surfaceComponents (+ deferred rail TODO) |
| `runs` parent-shell→named child | MOVE | 2.1 | PARENT_CHILD_MAP + DEFAULT_CHILD_BY_PARENT |
| `needs-you` EXPAND + ADD-to-nav | ADD (gated) | 2.2 | PARENT_CHILD_MAP, NAV_LABELS, YAML; gate `vox_resolve_approval` |
| `sub-agents` ADD-to-nav conditional | ADD (gated) | 2.3 | PARENT_CHILD_MAP, NAV_LABELS, YAML; gate `subagent_tree` |
| Migration ledger (all removed keys) | — | 6.1 | parseViewFromLocation redirect map + tests |

**Explicitly NOT in 3A (deferred):**
- `mens`/`populi` rename + GUI-from-CLI parity → **Plan 3B** (Amendment A).
- gamify **config** consolidation + Settings/Policies unification + settings CONDENSE → **Plan 3C**
  (Amendment B). 3A does only the gamify *nav* reparent.
- `oratio` `component_dir Loquela→Voice` code refactor → follow-up (label done in 1.2).
- `console` agent-adjacency (flag-only, default KEEP) → no-op.
- Inline matrix `nudge_routing_intention` rail control → documented TODO in 5.2.

**Discrepancies found vs blueprint (resolved in-plan):**
1. Blueprint says "3 SSOT sites"; there is a **4th** — `Sidebar.tsx::TOP_NAV_META` (independent top-level
   labels). Task 1.3 + the move tasks update it.
2. Blueprint cites `cmd:list_subagent_tree`; the **wired** command is `subagent_tree` (Task 2.3 gate uses the
   real name). Gate still passes.
3. Blueprint §5 line refs (e.g. `surfaceRegistry.generated.ts:37`) are advisory; the **real** edit target is
   the **YAML** (`contracts/gui/surface-registry.v1.yaml`) + regen, never the generated TS.
4. The registry already shows `matrix` navLabel "Routing" and `parentSurface: agents` (a prior partial edit);
   Task 5.2 removes the row entirely, superseding that.
