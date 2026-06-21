# Track F — Top-Bar Dashboard + Metric Series + `vox design execute` Automation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the top bar an optional, dockable dashboard fed by real time-series; fix the white scrollbar; and automate the design→execution pipeline with a `vox design execute` command (agy shell-out + handoff ledger + gui-visual-review gate).

**Architecture:** Add a `vox.metric.series.v1` event-fed source behind the already-built Recharts widgets; make the top bar hideable (its budget summary already moved to the Drive Console in Track C); apply the themed scrollbar everywhere + add a hygiene check; wire the three existing pieces (agy delegation, handoff ledger, gui-visual-review) into one `vox design execute` automation.

**Tech Stack:** React (vox-gui/ui Recharts + widgetRegistry), CSS tokens, Vox-language automation (`.vox`, run `--mode interp`), existing `agy` shell-out + handoff ledger + `vox ci gui-visual-review`.

**Scope marker:** `[PARALLEL-SAFE]` with C/D/E. **Soft dep:** dashboard-topbar-unification spec (widgetRegistry).
**Execution target:** Sonnet 4.6.

---

## Audit Corrections — verified against code 2026-06-20 (read FIRST; overrides stale claims below)

- **CONFIRMED:** the Recharts widgets accept `MetricPoint[]` where `MetricPoint = {t:number; v:number}` (`hooks/useMetricSeries.ts:4-7`, `chartWidgetShared.tsx:8-11`) — the `{t,v}` shape is right. TopHud full→slim→hidden cycle + Ctrl+Shift+H (`TopHud.tsx:100-103`, `App.tsx:503-505`) — making it optional is just state. The white-scrollbar complaint is REAL: `index.css:27-39` only styles `.custom-scrollbar`; there is **no app-wide `*` rule**, so un-classed scroll containers show the browser default. `vox ci gui-visual-review` exists as a CLI subcommand (`vox-cli/src/commands/ci/cmd_enums.rs:~1219`) + `vox-orchestrator-mcp/src/visus_review/` logic.
- **`useMetricSeries.ts` ALREADY EXISTS.** Don't create a duplicate `metricSeries.ts`. **Revised Task 1:** extend the existing `useMetricSeries` hook (and its `MetricPoint`) with the windowed ring-buffer + the event-fed `push`; keep the existing type. Reuse, don't fork.
- **TASK 4 IS INFEASIBLE AS A `.vox` SCRIPT — rewrite as a Rust `vox-cli` command.** Verified against the builtin registry (`vox-compiler/src/builtin_registry.rs`) and interp stdlib (`vox-compiler/src/eval/shell_stdlib.rs`): Vox scripts **cannot spawn processes**, and **none** of `agy_run`/`agy_pool_run`/`worktree_create`/`ledger_append`/`plan_parse_tasks`/`ci_gui_visual_review` exist as builtins. Also there is **no `agy` integration anywhere in-repo** (grep `agy` → zero code hits), and **no `Design` clap subcommand** (`vox-cli/src/lib.rs:119-657`). So:
  - Implement **`vox design execute <plan.md>` as a new Rust subcommand** under `crates/vox-cli/src/commands/design/` and register it in the `Cli` enum (`vox-cli/src/lib.rs`). Delete the `scripts/design-execute.vox` task entirely.
  - The Rust command: (1) parse the plan markdown for `### Task` blocks + `[PARALLEL-SAFE]`/`[SEQUENTIAL]` tags; (2) shell out to the external **`agy` Go binary** via `std::process::Command` (gated behind an `agy --version` presence check with an actionable error if missing — there is no `antigravity-sdk-rust` crate); run each task in a git worktree jail you create with `git worktree add`; (3) for UI tasks, gate via the existing `visus_review` logic (call it in-process from `vox-orchestrator-mcp`, or invoke `vox ci gui-visual-review`); (4) append to `docs/superpowers/antigravity-handoff-ledger.md`.
  - **HARD DEP:** this assumes an `agy` binary on PATH; the "native-agy delegation" shim does not exist yet. If that shim is desired in-repo first, Track F Task 4 depends on it. Otherwise the Rust command shells out directly. **Note the execution target for THESE plans is Sonnet 4.6, not agy** — `vox design execute` is a separate downstream automation, not the tool running Tracks A–F.
- **Scrollbar fix (Task 3) is correct** — add the app-wide `*` rule as written; keep `.custom-scrollbar` for the slim opt-in variant.

---

## File Structure

- Create: `crates/vox-gui/ui/src/lib/metricSeries.ts` — `vox.metric.series.v1` ring-buffer fed by orch events.
- Modify: the top-bar/dashboard container — add a `hidden` toggle + dock the dashboard as a panel group.
- Modify: `crates/vox-gui/ui/src/index.css` + scroll containers — apply themed scrollbar; add hygiene check.
- Create: `scripts/design-execute.vox` — the `vox design execute` dispatcher.
- Modify: `docs/superpowers/antigravity-handoff-ledger.md` — appended automatically by the dispatcher.

---

### Task 1: `vox.metric.series.v1` source

**Files:**
- Create: `crates/vox-gui/ui/src/lib/metricSeries.ts` (+ test)

- [ ] **Step 1: Write the failing test**

```ts
import { describe, it, expect } from "vitest";
import { MetricSeries } from "./metricSeries";

describe("MetricSeries", () => {
  it("appends points and caps the window", () => {
    const s = new MetricSeries(3);
    s.push("budget_burn", { t: 1, v: 0.1 });
    s.push("budget_burn", { t: 2, v: 0.2 });
    s.push("budget_burn", { t: 3, v: 0.3 });
    s.push("budget_burn", { t: 4, v: 0.4 });
    expect(s.get("budget_burn").map(p => p.v)).toEqual([0.2, 0.3, 0.4]); // oldest evicted
  });
  it("returns empty for unknown key", () => {
    expect(new MetricSeries(10).get("nope")).toEqual([]);
  });
});
```

- [ ] **Step 2: Run → FAIL**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/metricSeries.test.ts 2>&1 | tail -20` → FAIL.

- [ ] **Step 3: Implement**

```ts
export interface Point { t: number; v: number; }

/** Fixed-window per-key time series, fed by orchestrator events (cost_incurred,
 *  task_completed, queue depth, mesh peers). Persisted to the gui pref store by the caller. */
export class MetricSeries {
  private buf = new Map<string, Point[]>();
  constructor(private cap = 240) {}
  push(key: string, p: Point) {
    const arr = this.buf.get(key) ?? [];
    arr.push(p);
    if (arr.length > this.cap) arr.splice(0, arr.length - this.cap);
    this.buf.set(key, arr);
  }
  get(key: string): Point[] { return this.buf.get(key) ?? []; }
}
```

- [ ] **Step 4: Run → PASS, commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/lib/metricSeries.test.ts 2>&1 | tail -20` → PASS (2).

```bash
git add crates/vox-gui/ui/src/lib/metricSeries.{ts,test.ts}
git commit -m "feat(gui): vox.metric.series.v1 windowed source for dashboard widgets"
```

- [ ] **Step 5: Feed it** — subscribe to the existing orchestrator event stream (the agent-events/ORCH_STATUS
subscription) and `push` cost/queue/mesh points; pass `series.get(kind)` into the existing
`LineChartWidget`/`AreaChartWidget` (they already accept `Array<{t,v}>`). Commit that wiring separately.

---

### Task 2: Make the top bar optional + dock the dashboard

**Files:**
- Modify: the top-bar container (`TopHud.tsx` / `AppShell.tsx`) and the panelRegistry.

- [ ] **Step 1: Confirm the budget summary already moved** — Track C relocated the budget readout into the
Drive Console. Verify the slim-mode budget line (`TopHud.tsx:240`) is no longer the only place budget shows.

- [ ] **Step 2: Add a `hidden`/optional state** — the HUD already cycles full→slim→hidden (App.tsx:507). Make
`hidden` the no-cost default-available state and ensure nothing essential is lost when hidden (budget is in the
console; KPIs available via the dashboard panel). Add a vitest assertion that hidden HUD still renders the app
and the Drive Console budget.

- [ ] **Step 3: Register the dashboard as a dock panel group** — per the dockable-workspace spec, register the
dashboard (the Recharts time-series widgets) as a dockview panel so it can dock to any surface, then the top
bar becomes purely optional chrome. Commit.

```bash
git add -A && git commit -m "feat(gui): top bar optional; dashboard dockable; budget lives in Drive Console"
```

---

### Task 3: Theme the scrollbar everywhere + hygiene check

**Files:**
- Modify: `crates/vox-gui/ui/src/index.css` (add a base rule) + scroll containers missing the class.

- [ ] **Step 1: Add a global dark scrollbar base** so no container ships the default white one:

```css
/* index.css — dark scrollbar applies app-wide; .custom-scrollbar remains for opt-in slim variant */
* { scrollbar-width: thin; scrollbar-color: rgba(255,255,255,.12) transparent; }
*::-webkit-scrollbar { width: 8px; height: 8px; }
*::-webkit-scrollbar-track { background: transparent; }
*::-webkit-scrollbar-thumb { background: rgba(255,255,255,.08); border-radius: 8px; }
*::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,.14); }
```

- [ ] **Step 2: Add a hygiene check** — a vitest (or a `vox ci` grep gate) asserting no component sets
`overflow: auto/scroll` on an element that also opts out of the theme (e.g. a `light`/default scrollbar class).
Minimal version: a test that greps `crates/vox-gui/ui/src` for `overflow-(auto|scroll|y-auto)` occurrences and
fails if a new one appears without the dark scrollbar in effect (snapshot the count; bump deliberately).

- [ ] **Step 3: Playwright visual check** — snapshot the top-bar/dashboard scroll area to prove the thumb is
dark, not white. Add to the existing gui-visual snapshot set.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/index.css <hygiene-test>
git commit -m "fix(gui): app-wide dark scrollbar + hygiene check (no default white)"
```

---

### Task 4: `vox design execute` — automate the Antigravity pipeline

This answers the user's closing question. Wire three existing pieces into one command: the `agy` delegation
shell-out (native-agy-delegation memory), the append-only handoff ledger
(`docs/superpowers/antigravity-handoff-ledger.md`), and `vox ci gui-visual-review`.

**Files:**
- Create: `scripts/design-execute.vox`

- [ ] **Step 1: Write the dispatcher** (Vox; run with `--mode interp` per repo gotchas — no multi-line `+`,
single-line fn sigs, no `list.set`)

```
// scripts/design-execute.vox — vox design execute <plan.md>
// Parses a track plan's tasks, runs each [PARALLEL-SAFE]/[SEQUENTIAL] task via agy (Gemini Flash)
// in a worktree jail, then gates each built surface through gui-visual-review before marking the
// handoff ledger green.

fn parse_tasks(plan_path) { return plan_parse_tasks(plan_path) }   // -> list of {id, scope, body}

fn run_task(task) {
  let worktree = worktree_create("agy-" + task.id)                 // Vox worktree jail (NOT agy --sandbox)
  let result = agy_run(task.body, worktree, "--dangerously-skip-permissions")
  return { task: task.id, ok: result.ok, log: result.log, worktree: worktree }
}

fn review_surface(task) {
  // Only for UI-producing tasks: screenshot + AI design review against the spec principles.
  return ci_gui_visual_review(task.surface)
}

fn main(plan_path) {
  let tasks = parse_tasks(plan_path)
  let seq = tasks_sequential(tasks)
  let par = tasks_parallel_safe(tasks)
  let results = agy_pool_run(par)                                  // batch pool for PARALLEL-SAFE
  let seq_results = agy_run_each(seq)                              // ordered for SEQUENTIAL
  let all = list_concat(results, seq_results)
  for r in all {
    let review = review_surface(r)
    ledger_append(plan_path, r, review)                           // append-only AGH entry + verification
  }
  return ledger_digest(plan_path)
}
```

- [ ] **Step 2: Add the CLI alias** — register `vox design execute` to invoke `scripts/design-execute.vox`
(find the clap subcommand table; add a `design` group with an `execute <plan>` arg, mirroring an existing
`vox ci <gate>` registration). NOTE: do not use the `--` form (`vox -- design`), per the double-dash gotcha.

- [ ] **Step 3: Dry-run test** — run against Track A (smallest, no UI surfaces): it should parse 5 tasks,
attempt agy runs (or stub if `agy` absent → clear error), and append a ledger entry. Run:
`vox run scripts/design-execute.vox --mode interp -- docs/superpowers/plans/drive-console/2026-06-20-track-a-backend-control-ssot.md 2>de.log; tail -30 de.log`
Expected: parses tasks; if `agy` binary missing, fails with an actionable "agy not found" message (the agy
binary is a Go CLI per the native-agy-delegation memory — shell out, no new crate).

- [ ] **Step 4: Wire the gui-visual-review gate** — ensure `review_surface` calls the existing
`vox ci gui-visual-review` reviewer (`vox-orchestrator-mcp::visus_review`) only for UI tasks; backend tracks
(A/B/D) skip it. Non-gating/advisory, exit 0 (matches the existing reviewer's contract).

- [ ] **Step 5: Commit**

```bash
git add scripts/design-execute.vox <clap-registration>
git commit -m "feat: vox design execute — agy dispatch + ledger append + gui-visual-review gate"
```

---

### Task 5: Document the automated route

**Files:**
- Modify: `docs/superpowers/antigravity-handoff-ledger.md` (header note) + the program index status.

- [ ] **Step 1: Add a "How to run" section** to the program index pointing at `vox design execute <track>` and
the order (A→B→C→D, E/F parallel), and note the ledger is auto-appended.

- [ ] **Step 2: Commit**

```bash
git add docs/superpowers/plans/drive-console/2026-06-20-drive-console-program-index.md docs/superpowers/antigravity-handoff-ledger.md
git commit -m "docs: document vox design execute automated route for Drive Console program"
```

---

## Self-Review

**Spec coverage:** §6 top bar→dashboard + metric series → Tasks 1–2; scrollbar theming → Task 3; §11 Antigravity
automation → Tasks 4–5. **Type consistency:** `MetricSeries`/`Point` (Task 1) feed the existing Recharts widgets
(`Array<{t,v}>`); `vox design execute` reuses agy/ledger/gui-visual-review names from prior memories.
**Placeholder scan:** `<hygiene-test>`, `<clap-registration>`, `<panelRegistry-file>` are real path placeholders
resolved by reading the named registration sites (concrete actions, not vague TODOs); the `.vox` dispatcher uses
documented helper names — if a helper (`agy_pool_run`, `plan_parse_tasks`) isn't yet exposed in Vox std, Task 4
Step 1 implies adding a thin native binding (note this is the one genuinely new surface and may need a Rust shim
in `vox-orchestrator-mcp`, per the native-agy-delegation plan). **Gotchas honored:** `--mode interp`, no `--`
before `design`, no cargo-pipe-to-head.

## Cross-track caveat for the plan-audit pass

Track F Task 4 assumes Vox-language bindings for `agy_run`/`ledger_append`/`ci_gui_visual_review`. The
native-agy-delegation plan (memory) specifies these as a Rust shim in `vox-orchestrator-mcp` shelling out to the
Go `agy` binary — **not** an `antigravity-sdk-rust` crate (which does not exist). The audit pass must confirm
whether those bindings exist yet; if not, Track F depends on landing that shim first (or `vox design execute`
ships as a thin orchestrator command in Rust rather than `.vox`).
