# GUI Honesty Audit + Durable Prevention — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Find and fix every non-functional GUI element in `crates/vox-gui/ui` (dead buttons, no-op toasts, placeholder text), then make shipping a broken element a `tsc`/CI failure so it cannot recur.

**Architecture:** Six phases. Phases 1 (audit) and 3 (fix) fan out one sub-agent per surface (≈28, concurrency cap 8). Phases 0, 2, 4, 5 run serially in the main loop because they need the whole-repo view or touch shared files. Enforcement is two dependency-free layers — a required `ToastCause` type (compile error) and a vitest source-scan guard — wired into a new `vox ci gui-honesty` gate, mirroring the existing `gui-surface-registry` gate.

**Tech Stack:** React 19 + TypeScript 5 (vite, vitest, Playwright) for the UI; Rust (clap) for the `vox ci` gate. No ESLint (not installed — do not add it). Source-scan guards via vitest, matching the existing registry-parity test pattern.

**Spec:** `docs/superpowers/specs/2026-06-25-gui-honesty-audit-design.md` (read it first).

---

## File Structure

**New files**
- `crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.ts` — pure scanner: source text → violations. Shared by inventory + guard test (DRY).
- `crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.test.ts` — unit tests for the scanner.
- `crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.allowlist.ts` — explicit, documented exceptions.
- `crates/vox-gui/ui/src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts` — vitest gate over `surfaces/**`.
- `crates/vox-gui/ui/scripts/inventory.mjs` — one-shot manifest generator (Node, matches existing `style-dictionary.config.mjs` convention).
- `docs/agents/gui-honesty-manifest.json` — Phase 0 manifest (committed artifact).
- `docs/agents/gui-honesty-findings/<surface>.json` — Phase 1 per-surface findings.
- `docs/agents/gui-honesty-triage.md` — Phase 2 decision table.
- `crates/vox-cli/src/commands/ci/gui_honesty.rs` — Phase 4 CI gate.

**Modified files**
- `crates/vox-gui/ui/src/types/tauri.ts` — add `ToastCause`, make `Toast.cause` required.
- Every real `pushToast(...)` call site — add a truthful `cause`.
- `crates/vox-gui/ui/src/components/surfaces/<Name>/*` — per-surface fixes (Phase 3).
- `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`, `App.tsx`, `components/ui/Toasts.tsx` — shared-file fixes (one serial task).
- `crates/vox-cli/src/commands/ci/cmd_enums.rs`, `mod.rs`, `run_body.rs` — register `gui-honesty`.
- `.github/workflows/ci.yml` — add the gate to the CI job.

---

## Phase 0 — Inventory & Baseline (serial)

### Task 1: Honesty scanner (pure function, TDD)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// honestyScan.test.ts
import { describe, it, expect } from 'vitest';
import { scanSource } from './honestyScan';

describe('scanSource', () => {
  it('flags placeholder text', () => {
    const v = scanSource('x.tsx', `return <div>Not yet implemented</div>;`);
    expect(v.map(x => x.kind)).toContain('placeholder');
  });
  it('flags an empty arrow handler', () => {
    const v = scanSource('x.tsx', `<button onClick={() => {}}>Go</button>`);
    expect(v.map(x => x.kind)).toContain('dead-handler');
  });
  it('passes a real handler', () => {
    const v = scanSource('x.tsx', `<button onClick={() => invoke('do_it')}>Go</button>`);
    expect(v).toHaveLength(0);
  });
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/__guards__/honestyScan.test.ts`
Expected: FAIL — `scanSource` is not defined.

- [ ] **Step 3: Implement the scanner**

```ts
// honestyScan.ts
export type Violation = {
  file: string;
  line: number;
  kind: 'placeholder' | 'dead-handler';
  snippet: string;
};

const PLACEHOLDER =
  /\b(not\s+(yet\s+)?(implemented|wired|available|working|hooked|connected)|coming\s+soon|placeholder)\b/i;
// empty or brace-only arrow body: onClick={() => {}} / onClick={()=>{ }}
const DEAD_HANDLER =
  /on(Click|Submit|Change|Press)=\{\s*\(\s*[^)]*\)\s*=>\s*\{\s*\}\s*\}/;

export function scanSource(file: string, text: string): Violation[] {
  const out: Violation[] = [];
  text.split('\n').forEach((raw, i) => {
    const line = i + 1;
    if (PLACEHOLDER.test(raw)) out.push({ file, line, kind: 'placeholder', snippet: raw.trim() });
    if (DEAD_HANDLER.test(raw)) out.push({ file, line, kind: 'dead-handler', snippet: raw.trim() });
  });
  return out;
}
```

- [ ] **Step 4: Run tests, verify pass**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/__guards__/honestyScan.test.ts`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.ts crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.test.ts
git commit -m "feat(gui): honesty source scanner (placeholder + dead-handler)"
```

### Task 2: Inventory manifest + baseline screenshots (serial)

**Files:**
- Create: `crates/vox-gui/ui/scripts/inventory.mjs`
- Create (output): `docs/agents/gui-honesty-manifest.json`

- [ ] **Step 1: Write the inventory script**

```js
// crates/vox-gui/ui/scripts/inventory.mjs
import { readdirSync, readFileSync, writeFileSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = 'src/components/surfaces';
const TOAST = /pushToast\(/;
const HANDLER = /on(Click|Submit|Change|Press)=\{/;

function walk(dir) {
  return readdirSync(dir).flatMap(name => {
    const p = join(dir, name);
    return statSync(p).isDirectory() ? walk(p)
      : p.endsWith('.tsx') && !p.endsWith('.test.tsx') ? [p] : [];
  });
}

const rows = [];
for (const file of walk(ROOT)) {
  const surface = file.split('/')[3] ?? '?';
  readFileSync(file, 'utf8').split('\n').forEach((raw, i) => {
    if (TOAST.test(raw)) rows.push({ surface, file, line: i + 1, kind: 'toast', snippet: raw.trim() });
    if (HANDLER.test(raw)) rows.push({ surface, file, line: i + 1, kind: 'handler', snippet: raw.trim() });
  });
}
writeFileSync('../../../docs/agents/gui-honesty-manifest.json', JSON.stringify(rows, null, 2));
console.log(`manifest: ${rows.length} sites across surfaces`);
```

- [ ] **Step 2: Run it**

Run: `cd crates/vox-gui/ui && node scripts/inventory.mjs`
Expected: prints `manifest: <N> sites...`; `docs/agents/gui-honesty-manifest.json` exists and is non-empty.

- [ ] **Step 3: Capture baseline screenshots**

Run: `cd crates/vox-gui/ui && pnpm exec playwright test --config playwright.screens.config.ts`
Expected: screenshots written to the configured output dir. If the config needs a dev server, start it first per `crates/vox-gui/ui/e2e/README` conventions; do not invent flags — read the config.

- [ ] **Step 4: Verify suite is green (baseline)**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm test`
Expected: both green. If red on `main` already, record the pre-existing failures in the manifest commit message — do not fix unrelated breakage here.

- [ ] **Step 5: Commit (Gate G0)**

```bash
git add crates/vox-gui/ui/scripts/inventory.mjs docs/agents/gui-honesty-manifest.json
git commit -m "chore(gui): honesty inventory manifest + baseline"
```

**Gate G0:** manifest committed, baseline screenshots captured, typecheck+test green.

---

## Phase 1 — Audit (parallel, one sub-agent per surface)

This phase is **orchestration**, not fixed code. The orchestrator reads
`docs/agents/gui-honesty-manifest.json`, derives the surface list, and dispatches one
sub-agent per surface (concurrency cap 8) with the template below. Use
`superpowers:dispatching-parallel-agents`.

### Findings schema (every sub-agent returns this, written to `docs/agents/gui-honesty-findings/<surface>.json`)

```json
{
  "surface": "Memory",
  "visual": [{ "issue": "string", "severity": "low|med|high" }],
  "elements": [
    {
      "file": "src/components/surfaces/Memory/MemoryView.tsx",
      "line": 42,
      "label": "Refresh button",
      "verdict": "works|dead|noop-toast|placeholder",
      "cheap_to_wire": true,
      "backend_command": "memory_refresh | null",
      "evidence": "handler calls invoke('memory_refresh')"
    }
  ]
}
```

### Per-surface audit sub-agent prompt (template — substitute `<SURFACE>` and `<DIR>`)

- [ ] Dispatch for each surface:

```
You are auditing ONE Vox GUI surface for non-functional elements. Do not change code.

Surface: <SURFACE>
Directory: crates/vox-gui/ui/src/components/<DIR>

1. Read every non-test .tsx in the directory.
2. For each interactive element (onClick/onSubmit/onChange) and each pushToast call:
   - Trace the handler. Classify verdict as one of:
     works | dead | noop-toast | placeholder  (definitions in the spec).
   - If dead/noop-toast: determine cheap_to_wire — does a Tauri command or existing
     handler already implement it (grep src-tauri / commands for a matching invoke)?
3. Read the visual-review output for this surface if present under
   contracts/reports/gui-visual-review/ ; record visual issues.
4. Write findings JSON (schema above) to
   docs/agents/gui-honesty-findings/<SURFACE>.json
Return the path and a one-line summary (counts per verdict).
```

- [ ] **Gate G1 — adversarial re-check.** After all surfaces report, dispatch ONE verifier
  agent over a random sample of `dead` verdicts: "Confirm these handlers are truly dead and
  not wired through a hook/context you missed. Return confirmed/false-positive per item."
  Demote any false positives to `works` in the findings file. Commit the findings dir.

```bash
git add docs/agents/gui-honesty-findings
git commit -m "chore(gui): per-surface honesty findings + adversarial recheck"
```

---

## Phase 2 — Triage & Synthesis (serial, HUMAN gate)

### Task 3: Build the triage table

**Files:** Create `docs/agents/gui-honesty-triage.md`

- [ ] **Step 1:** Merge all `docs/agents/gui-honesty-findings/*.json` into one table, one row
  per element, columns: `surface | file:line | label | verdict | cheap_to_wire | backend_command | DECISION`.
  Assign `DECISION` by policy:
  - `works` → **KEEP**
  - `dead`/`noop-toast` with `cheap_to_wire=true` → **WIRE**
  - `dead`/`noop-toast` with `cheap_to_wire=false` → **HIDE**
  - `placeholder` → **HIDE** (unless a cheap real value exists → **WIRE**)
  - every `noop-toast` also → **TOAST-FIX**

- [ ] **Step 2: Commit**

```bash
git add docs/agents/gui-honesty-triage.md
git commit -m "docs(gui): honesty triage decision table"
```

- [ ] **Step 3: Gate G2 (HUMAN).** Present the triage table to the user. STOP. Do not start
  Phase 3 until the user approves which elements are hidden vs wired. Apply any edits they
  request back into the table and re-commit.

---

## Phase 3 — Fix (parallel per surface + one serial shared-file task)

Surfaces partition cleanly by directory, so worktrees are NOT needed. Shared files are
handled by Task 5 alone — no parallel agent may touch them.

### Task 4: Per-surface fix sub-agent (template, TDD) — dispatch one per surface that has non-KEEP rows

- [ ] Dispatch for each such surface:

```
You are fixing ONE Vox GUI surface per an approved triage table. TDD, one element at a time.
Surface: <SURFACE>  Directory: crates/vox-gui/ui/src/components/<DIR>
Approved decisions: docs/agents/gui-honesty-triage.md (rows for <SURFACE> only)

Do NOT edit shared files (App.tsx, surfaceComponents.tsx, components/ui/Toasts.tsx,
types/tauri.ts, any *registry*). If a fix needs one, note it in your return for Task 5.

For each row:
  WIRE: write a failing vitest/RTL test asserting the real effect (invoke called /
        state changes), then wire the handler to the existing command. Run test → green.
  HIDE: write a failing test asserting the element is NOT rendered by default, then remove
        it or gate it behind `import.meta.env.VITE_VOX_UNFINISHED` (off by default).
        Run test → green.
  TOAST-FIX: remove the no-op toast, OR if the action is real, keep it (cause added in Phase 4).
Run `pnpm vitest run src/components/<DIR>` after each element. Commit per element:
  git commit -m "fix(gui-<surface>): <wire|hide> <element>"
Return: list of commits + any shared-file changes deferred to Task 5.
```

- [ ] **Gate G3 (per surface):** `pnpm vitest run src/components/<DIR>` green; rerun
  `vox ci gui-visual-review --no-ai` for structural regressions; then dispatch
  `superpowers:code-reviewer` on the surface's diff before moving on.

### Task 5: Shared-file fixes (serial, main loop)

**Files:** Modify `App.tsx`, `surfaceComponents.tsx`, `components/ui/Toasts.tsx` as collected
from Task 4 returns.

- [ ] **Step 1:** Apply each deferred shared-file change with a failing test first where the
  change is behavioral (e.g. a removed nav entry no longer renders). For pure plumbing
  (passing a prop), a typecheck pass is the check.
- [ ] **Step 2:** Run `pnpm typecheck && pnpm test`. Expected: green.
- [ ] **Step 3: Commit** `git commit -m "fix(gui): shared-file wiring from honesty triage"`

---

## Phase 4 — Durable Prevention (serial, TDD)

### Task 6: Required ToastCause type (compile-error enforcement)

**Files:** Modify `crates/vox-gui/ui/src/types/tauri.ts`; touch every `pushToast` site.

- [ ] **Step 1: Add the type and make `cause` required**

```ts
// types/tauri.ts — replace the Toast type
export type ToastCause =
  | 'backend-ok' | 'backend-error' | 'validation'
  | 'navigation' | 'clipboard' | 'external';

export type Toast = {
  tone: 'ok' | 'warn' | 'info';
  title: string;
  body?: string;
  cmd?: string;
  cause: ToastCause; // required — causeless toast = compile error
};
```

- [ ] **Step 2: Run typecheck to see it fail at every causeless site**

Run: `cd crates/vox-gui/ui && pnpm typecheck`
Expected: FAIL — `cause` missing at each real `pushToast` call. This list IS the work.

- [ ] **Step 3: Add a truthful `cause` at every site.** Pick the value by what the action
  actually did (a mutation that succeeded → `backend-ok`; a failed invoke → `backend-error`;
  rejected input → `validation`; copy → `clipboard`; etc.). Any site that can only be
  labelled by lying is a no-op toast missed in Phase 3 — delete it instead.

- [ ] **Step 4: Run typecheck + tests, verify green**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm test`
Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src
git commit -m "feat(gui): require typed ToastCause — causeless toasts no longer compile"
```

### Task 7: vitest guard over surfaces (regression gate)

**Files:**
- Create `crates/vox-gui/ui/src/components/surfaces/__guards__/honestyScan.allowlist.ts`
- Create `crates/vox-gui/ui/src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts`

- [ ] **Step 1: Write the allowlist (empty to start)**

```ts
// honestyScan.allowlist.ts
// Each entry MUST cite why the violation is acceptable (flag-gated / non-shipped path).
export const HIDDEN_ALLOWLIST: { file: string; line: number; reason: string }[] = [];
```

- [ ] **Step 2: Write the guard test (will fail until tree is clean)**

```ts
// surfaceHonesty.guard.test.ts
import { describe, it, expect } from 'vitest';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { scanSource } from './honestyScan';
import { HIDDEN_ALLOWLIST } from './honestyScan.allowlist';

const ROOT = 'src/components/surfaces';
function walk(d: string): string[] {
  return readdirSync(d).flatMap(n => {
    const p = join(d, n);
    return statSync(p).isDirectory() ? walk(p)
      : p.endsWith('.tsx') && !p.endsWith('.test.tsx') ? [p] : [];
  });
}
const allowed = (f: string, l: number) =>
  HIDDEN_ALLOWLIST.some(a => f.endsWith(a.file) && a.line === l);

describe('surface honesty guard', () => {
  it('no placeholder text or dead handlers in shipped surfaces', () => {
    const violations = walk(ROOT)
      .flatMap(f => scanSource(f, readFileSync(f, 'utf8')))
      .filter(v => !allowed(v.file, v.line));
    expect(violations, JSON.stringify(violations, null, 2)).toHaveLength(0);
  });
});
```

- [ ] **Step 3: Run it**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts`
Expected: PASS (Phase 3 cleaned the tree). Any failure lists a real remaining violation —
fix it or add an allowlist entry with a cited reason.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/__guards__
git commit -m "feat(gui): surface-honesty vitest guard"
```

### Task 8: `vox ci gui-honesty` gate (Rust, mirrors gui-surface-registry)

**Files:**
- Create `crates/vox-cli/src/commands/ci/gui_honesty.rs`
- Modify `crates/vox-cli/src/commands/ci/cmd_enums.rs` (add the variant)
- Modify `crates/vox-cli/src/commands/ci/mod.rs` + `run_body.rs` (dispatch)
- Modify `.github/workflows/ci.yml` (add the step)

- [ ] **Step 1: Add the clap variant** in `cmd_enums.rs`, next to `GuiSurfaceRegistry`:

```rust
/// Gate: GUI honesty — typed toasts + no placeholder/dead elements in surfaces.
#[command(name = "gui-honesty")]
GuiHonesty,
```

- [ ] **Step 2: Implement the gate** in `gui_honesty.rs`:

```rust
use anyhow::{Context, Result, anyhow};
use std::process::Command;

const UI_DIR: &str = "crates/vox-gui/ui";

/// Runs the UI typecheck (enforces required ToastCause) and the vitest honesty guard.
pub fn run() -> Result<()> {
    for (label, args) in [
        ("typecheck", vec!["typecheck"]),
        ("honesty-guard", vec!["vitest", "run",
            "src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts"]),
    ] {
        let status = Command::new("pnpm")
            .current_dir(UI_DIR)
            .args(&args)
            .status()
            .with_context(|| format!("spawn pnpm {label}"))?;
        if !status.success() {
            return Err(anyhow!("gui-honesty gate failed at: {label}"));
        }
    }
    println!("gui-honesty: OK");
    Ok(())
}
```

- [ ] **Step 3: Dispatch it** in `mod.rs`/`run_body.rs` where `GuiSurfaceRegistry` is matched:

```rust
CiCmd::GuiHonesty => crate::commands::ci::gui_honesty::run(),
```

(Match the exact enum/module path used by the neighboring `GuiSurfaceRegistry` arm — read that arm and copy its shape.)

- [ ] **Step 4: Red-then-green self-test (Gate G4)**

```bash
# RED: plant a violation, gate must fail
printf 'export const X = () => <div>Not yet implemented</div>;\n' > crates/vox-gui/ui/src/components/surfaces/__guards__/__planted.tsx
cargo run -p vox-cli -- ci gui-honesty; echo "exit=$?"   # expect non-zero
rm crates/vox-gui/ui/src/components/surfaces/__guards__/__planted.tsx
# GREEN: clean tree passes
cargo run -p vox-cli -- ci gui-honesty; echo "exit=$?"   # expect 0
```

Expected: RED run exits non-zero and names `honesty-guard`; GREEN run prints `gui-honesty: OK` and exits 0.

- [ ] **Step 5: Wire into CI** — add to `.github/workflows/ci.yml` next to the existing
  `gui-surface-registry` invocation:

```yaml
      - name: GUI honesty gate
        run: cargo run -p vox-cli -- ci gui-honesty
```

- [ ] **Step 6: Commit**

```bash
git add crates/vox-cli/src/commands/ci/gui_honesty.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/mod.rs crates/vox-cli/src/commands/ci/run_body.rs .github/workflows/ci.yml
git commit -m "feat(ci): vox ci gui-honesty gate (tsc ToastCause + vitest guard)"
```

---

## Phase 5 — Verify & Close (serial)

### Task 9: Full verification + close

- [ ] **Step 1:** `cd crates/vox-gui/ui && pnpm typecheck && pnpm test && pnpm exec playwright test`
  Expected: all green.
- [ ] **Step 2:** `cargo run -p vox-cli -- ci gui-honesty` → `gui-honesty: OK`.
- [ ] **Step 3:** Re-capture screenshots (`playwright.screens.config.ts`) and diff against the
  Phase 0 baseline; attach the before/after diff as proof in the PR description.
- [ ] **Step 4:** Use `superpowers:verification-before-completion` to confirm every spec
  success criterion (1–5) with command output, then `superpowers:finishing-a-development-branch`.

---

## Self-Review

- **Spec coverage:** Goals 1–3 → Phases 1, 3, 4. Toast taxonomy → Tasks 4(TOAST-FIX)+6.
  Fix policy WIRE/HIDE → Tasks 3+4. Enforcement layers A/B/C → Tasks 6/7/8. Gates G0–G5 →
  end of Phases 0/1/2/3/4/5. Agent model → Phases 1+3 templates. No Figma / no ESLint /
  no worktrees non-goals → honored throughout.
- **Placeholder scan:** all code steps carry real code; agent steps carry full prompt
  templates + a concrete findings schema; no TBD/TODO-as-instruction.
- **Type consistency:** `scanSource`/`Violation` (Task 1) reused verbatim in Tasks 2 (concept)
  and 7. `ToastCause`/`Toast.cause` (Task 6) matches the spec. `HIDDEN_ALLOWLIST` shape
  matches between Tasks 7 step 1 and step 2.
- **Known unknowns flagged for the executor (not placeholders):** the exact `playwright.screens.config.ts`
  invocation and the exact `CiCmd` enum/module path for the dispatch arm must be read from the
  neighboring code, not guessed — both steps say so explicitly.
