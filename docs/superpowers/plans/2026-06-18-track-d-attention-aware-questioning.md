# Track D — Attention-Aware Questioning: Surface the Budget, Close the Loops, Sharpen the Prompt (Antigravity / Gemini 3.5 Flash edition)

> **For agentic workers:** REQUIRED SUB-SKILL: Use `.agents/skills/subagent-driven-development.skill.md` to execute task-by-task, and `.agents/skills/test-driven-development.skill.md` for each task. Steps use checkbox (`- [ ]`) syntax. (Task 0 creates `.agents/skills/`; until it runs, the authoritative copies live at `crates/vox-skills/skills/superpowers/`.)

> **🤖 EXECUTION TARGET — READ FIRST.** This plan is run end-to-end by **Gemini 3.5 Flash inside Google Antigravity**, not Claude Code. Antigravity is unreliable on long tasks (≈48% real-world completion; mid-task termination leaves no checkpoint; quota is a hard cutoff) and Gemini 3.5 Flash hallucinates APIs and has weak long-context recall. The plan is engineered against those failure modes. **You MUST obey the Operating Rules below on every task.** Basis: [`../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md) §5. Handoff guide: [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md).

> **Research basis (verified 2026-06-18):** [`../../src/architecture/diagnostic-questioning-sota-and-wiring-audit-2026-06-18.md`](../../src/architecture/diagnostic-questioning-sota-and-wiring-audit-2026-06-18.md). Operational SSOT extended: [`../../src/reference/information-theoretic-questioning.md`](../../src/reference/information-theoretic-questioning.md).

## Operating Rules (apply to EVERY task)

1. **Atomic + green + committed.** Finish a task only when its tests pass AND you commit. A crash between tasks must leave a compiling, tested tree. Never split a compile-breaking change across two commits.
2. **Verify-before-use (anti-hallucination).** Before any code step that references a symbol/type/path, run the `rg`/read step in that task and confirm it exists with the stated signature. If reality differs, STOP and report — do not invent. Gemini 3.5 Flash invents phantom APIs; this rule is non-negotiable.
3. **Self-contained.** Everything you need is in the task. Do not rely on remembering earlier tasks (weak long-context recall).
4. **Two-strike circuit breaker.** If a step's verification fails twice, STOP, write a one-paragraph handoff note (what failed, last good commit hash), hand back. Do not loop on the same failed action.
5. **Parallel dispatch.** Tasks are tagged `[PARALLEL-SAFE]` or `[SEQUENTIAL]`. Only dispatch parallel subagents for `[PARALLEL-SAFE]` tasks whose **Files** sets are disjoint AND which do not `use`/import a type another in-flight task is creating. Never let two subagents write the same file. When unsure, run sequentially.
6. **Vox house rules.** Never `cargo fmt --all` (use `cargo fmt -p <crate>`). Automation is `.vox`, not `.ps1/.sh/.py`. `.md` under `docs/src/` needs YAML frontmatter. No stubs/placeholders.
7. **Verification ritual** before each commit (skill: `.agents/skills/verification-before-completion.skill.md`): Rust — `cargo test -p <crate>` → `cargo clippy -p <crate> -- -D warnings` → `cargo fmt -p <crate>`; GUI UI (run from `crates/vox-gui/ui`) — `npx vitest run <pattern>` → `npx tsc --noEmit`. Paste real output. Self-review with `.agents/skills/requesting-code-review.skill.md` before committing.
8. **Rollback on broken tree.** If a task aborts mid-edit leaving a non-compiling tree, `git reset --hard HEAD` to the last green commit, then re-attempt that single task from scratch. Never build forward on a broken tree.
9. **`cargo run -p vox-arch-check` must pass** after any task that adds a module or cross-crate dependency.

**Goal:** Make the existing-but-invisible NASA-TLX attention budget *visible to the user*, *actually close* the two dangling loops (duplicate constant, write-only suppressed-interruption telemetry), add the spec-vs-model uncertainty axis, sharpen the Socrates questioning **prompt** to reason over the solution space, and surface *withheld* questions inline in chat.

**Architecture:** The GUI surface rides the **existing** `ORCH_STATUS_EVENT` daemon stream — `Orchestrator::status()` already snapshots `BudgetManager`; we add an `attention_budget` field to `OrchestratorStatus`, pass it through `GuiOrchestratorStatus`, and render it. No new event/stream/daemon plumbing. All Rust additions are pure, unit-tested functions beside existing attention code. All new fields are serde-`default` (backward compatible).

**Tech Stack:** Rust (`vox-orchestrator`, `vox-orchestrator-types`, `vox-orchestrator-mcp`, `vox-gui` host); TypeScript/React + Vitest 3 (`vox-gui/ui`); Tauri event channels; VoxDb `attention_events`.

---

## Phase / Task map (execution order: 0 → B → A → C → D → E)

| Phase | Tasks | Closes |
|---|---|---|
| 0 — Antigravity enablement | T0 | run blocker: `.agents/skills/` missing |
| B — De-split-brain | T1, T2 | audit #2 (dup constant), #3 (dead link) |
| A — Surface budget (via existing stream) | T3, T4, T5, T6 | audit #1 (headline: budget invisible) |
| C — Two-axis uncertainty | T7, T8 | audit #5 (spec vs model) |
| D — Close the learn loop | T9, T10 | audit #4 (write-only telemetry) |
| E — Prompt + chat protocol | T11, T12 | research §2.1 (reason-over-solutions), §2.3 / chat wiring |

**Dependency graph (parallelization, corrected):** T1↔T2 parallel-safe. **Phase A is one sequential data path: T3→T4→T5→T6** (each consumes the prior layer's field); only T6's *unit test* uses a fixture and is independent. T7→T8 sequential (same file). T9→T10 sequential (T10 consumes T9's fn). T11↔T12 parallel-safe (different concerns in the same file `chat_socrates_meta.rs` → actually **SEQUENTIAL**, same file). Net: the only true parallel pair is (T1, T2).

## File Structure

| File | Responsibility | Action | Task |
|---|---|---|---|
| `.agents/skills/` | Antigravity skill mount (junction/copy of native skills) | Create | T0 |
| `crates/vox-orchestrator/src/attention/budget.rs` | re-export interrupt cost from types SSOT | Modify | T1 |
| `docs/src/reference/information-theoretic-questioning.md` | fix archived-doc link | Modify | T2 |
| `crates/vox-orchestrator/src/orchestrator/types.rs` | add `attention_budget` to `OrchestratorStatus` | Modify | T3 |
| `crates/vox-orchestrator/src/orchestrator/accessors.rs` | populate it from `budget.attention_snapshot()` | Modify | T3 |
| `crates/vox-gui/src/commands/orchestrator.rs` | pass-through field in `GuiOrchestratorStatus` + `to_gui_status` | Modify | T4 |
| `crates/vox-gui/ui/src/transport.ts` | extend `OrchestratorStatus` TS type | Modify | T5 |
| `crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx` (+ `.test.tsx`) | meter + focus chip + backlog | Create | T6 |
| `crates/vox-gui/ui/src/App.tsx` | mount the meter from status | Modify | T6 |
| `crates/vox-orchestrator/src/attention/interruption_policy.rs` | spec/model uncertainty fields + use | Modify | T7, T8 |
| `crates/vox-orchestrator/src/attention/calibrator.rs` | pure offset + aggregate | Create | T9 |
| `crates/vox-orchestrator/src/attention/mod.rs` | register `calibrator` | Modify | T9 |
| `crates/vox-orchestrator/src/attention/calibrator.rs` | `apply_learned_offsets` into `InterruptionCalibrationConfig` | Modify | T10 |
| `crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs` | rewrite rider + inline withheld-question | Modify | T11, T12 |
| `docs/src/architecture/where-things-live.md` | register `calibrator` | Modify | T9 |

**Pre-flight (run once, paste output; NOT a code step):**
- `ls .agents/skills 2>&1; ls GEMINI.md; ls crates/vox-skills/skills/superpowers/` — confirm mount missing, `GEMINI.md` present, native skills present.
- `rg -n "\"test\"" crates/vox-gui/ui/package.json` — confirm `"test": "vitest run"` (tests run via `npx vitest run`).
- `rg -n "pub fn attention_snapshot" crates/vox-orchestrator/src/budget/mod.rs` — confirm `attention_snapshot(&self) -> AttentionBudget`.
- `rg -n "pub struct OrchestratorStatus" -A 40 crates/vox-orchestrator/src/orchestrator/types.rs`
- `rg -n "pub fn status\(" -A 40 crates/vox-orchestrator/src/orchestrator/accessors.rs`
- `rg -n "struct GuiOrchestratorStatus|fn to_gui_status" -A 5 crates/vox-gui/src/commands/orchestrator.rs`
- `rg -n "interface OrchestratorStatus|type OrchestratorStatus" crates/vox-gui/ui/src/transport.ts`
- `rg -n "fn socrates_system_rider|struct QuestioningJsonMeta|DeferUntilCheckpoint" crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs`
- `cargo run -p vox-arch-check` — baseline must pass.

---

## Task 0 `[SEQUENTIAL]`: Mount native skills for Antigravity (run blocker)

Antigravity loads skills from `.agents/skills/`; that directory does **not** exist (`GEMINI.md` does). Without this, every `*.skill.md` reference in this plan fails to resolve. Create the mount as a junction (no admin needed on Windows) with a copy fallback. This is one-time setup, not project automation — no `.vox` script required.

**Files:** Create `.agents/skills/` (mount only; not a code change).

- [ ] **Step 1 (verify — this task may already be done):** `ls .agents/skills/ 2>&1`. If it resolves (a junction or copy is already present — it likely is), T0 is a **no-op**: confirm `ls crates/vox-skills/skills/superpowers/*.skill.md` lists the native skills (≥7 files; do NOT assume an exact count), confirm `git status` is clean, and SKIP to the next task. Only if `.agents/skills` is genuinely missing do Steps 2–4 to create the mount.

- [ ] **Step 2: Create the mount.** Windows (junction, preferred):

```bash
cmd //c "mkdir .agents 2>nul & mklink /J .agents\\skills crates\\vox-skills\\skills\\superpowers"
```

If junction creation fails (permissions/filesystem), fall back to a copy:

```bash
mkdir -p .agents/skills && cp crates/vox-skills/skills/superpowers/*.skill.md .agents/skills/
```

- [ ] **Step 3: Verify the mount resolves.** `ls .agents/skills/writing-plans.skill.md` → exists.

- [ ] **Step 4: Ignore the mount in git** (do not commit a junction or duplicated skills). Append `/.agents/skills/` to `.gitignore` if not already ignored (`rg -n "^/?\.agents" .gitignore` first). Commit only the `.gitignore` line if you added it:

```bash
git add .gitignore && git commit -m "chore: gitignore .agents/skills antigravity mount (track-d T0)"
```

> If `.agents/skills/` is already gitignored or `.gitignore` already covers it, skip the commit — there is nothing to commit and that is fine.

---

## Task 1 `[PARALLEL-SAFE]`: Single-source the interrupt-cost constant (audit #2)

> **VERIFY-ONLY — this is ALREADY DONE in `main` (audit 2026-06-18).** `budget.rs:88-89`
> already re-exports `DEFAULT_INTERRUPT_COST_MS = ...::CLARIFICATION_INTERRUPT_COST_MS`,
> and the single-source guard test already exists at `budget.rs:437`. Adding the test
> again creates a **duplicate test fn → compile error**. Do NOT re-implement.

**Files:** none (verification only).

- [ ] **Step 1 (verify, no edit):** `rg -n "DEFAULT_INTERRUPT_COST_MS|CLARIFICATION_INTERRUPT_COST_MS|interrupt_cost_is_single_sourced" crates/vox-orchestrator/src/attention/budget.rs`. Expect to see the re-export (`pub const DEFAULT_INTERRUPT_COST_MS: u64 = ...CLARIFICATION_INTERRUPT_COST_MS;`) and an existing guard test.
  - **If both present (expected):** audit #2 is already closed — mark this task done, **no commit**.
  - **If somehow absent:** only then add the re-export + guard test (re-export keeps the doc comment; test asserts `DEFAULT_INTERRUPT_COST_MS == vox_orchestrator_types::socrates_policy::CLARIFICATION_INTERRUPT_COST_MS`), run `cargo test -p vox-orchestrator`, and commit.

---

## Task 2 `[PARALLEL-SAFE]`: Fix the archived-research dead link (audit #3)

**Files:** Modify `docs/src/reference/information-theoretic-questioning.md` (≈ line 217).

- [ ] **Step 1 (verify):** `rg -n "research-diagnostic-questioning-2026.md" docs/src/reference/information-theoretic-questioning.md`; `ls docs/src/archive/research-2026-q1/research-diagnostic-questioning-2026.md`. If the archived file is not there, STOP.

- [ ] **Step 2: Replace the line** referencing `docs/src/architecture/research-diagnostic-questioning-2026.md` with:

```markdown
- [`../archive/research-2026-q1/research-diagnostic-questioning-2026.md`](../archive/research-2026-q1/research-diagnostic-questioning-2026.md) — April-2026 full research grounding (POMDP, EVPI, gap analysis, implementation roadmap)
- [`../architecture/diagnostic-questioning-sota-and-wiring-audit-2026-06-18.md`](../architecture/diagnostic-questioning-sota-and-wiring-audit-2026-06-18.md) — June-2026 SoTA refresh + wiring audit (this plan's basis)
```

- [ ] **Step 3: Verify links resolve.** `ls docs/src/archive/research-2026-q1/research-diagnostic-questioning-2026.md docs/src/architecture/diagnostic-questioning-sota-and-wiring-audit-2026-06-18.md`.

- [ ] **Step 4: Commit.**

```bash
git add docs/src/reference/information-theoretic-questioning.md
git commit -m "docs(questioning): fix archived research link + add 2026-06-18 audit ref (audit #3)"
```

---

## Task 3 `[SEQUENTIAL]`: Snapshot the live attention budget into `OrchestratorStatus` (Phase A, Rust core)

`Orchestrator::status()` already reads `budget_manager`. Add the budget snapshot to the status struct so it rides the **existing** daemon stream.

**Files:** Modify `crates/vox-orchestrator/src/orchestrator/types.rs` (`OrchestratorStatus`); Modify `crates/vox-orchestrator/src/orchestrator/accessors.rs` (`status()`).

- [ ] **Step 1 (verify-before-use):** `rg -n "pub struct OrchestratorStatus" -A 40 crates/vox-orchestrator/src/orchestrator/types.rs` — confirm it derives `serde::Serialize` and ends with `pub agents: Vec<AgentSummary>,`. `rg -n "pub fn status\(" -A 12 crates/vox-orchestrator/src/orchestrator/accessors.rs` — confirm the body opens with `let budget = crate::sync_lock::rw_read(&self.budget_manager);` and that `budget` is **still in scope** where you will add the field (if it is `drop(budget)`-ed early, capture the snapshot before the drop). `rg -n "pub fn attention_snapshot" crates/vox-orchestrator/src/budget/mod.rs` — confirm `attention_snapshot(&self) -> AttentionBudget`.

- [ ] **Step 2: Write the failing test.** Add to the test module in `accessors.rs` (or create one). This asserts the field is populated and serializes:

```rust
#[test]
fn status_includes_attention_budget_snapshot() {
    // `Orchestrator::default()` does NOT exist — the real test constructor is
    // `Orchestrator::new(OrchestratorConfig::for_testing())` (see orchestrator/tests/mod.rs:148).
    let orch = crate::Orchestrator::new(crate::config::OrchestratorConfig::for_testing());
    let status = orch.status();
    let budget = status.attention_budget.expect("status must carry an attention budget snapshot");
    assert_eq!(budget.max_attention_ms, crate::attention::DEFAULT_ATTENTION_BUDGET_MS);
    // serializes cleanly for the daemon wire
    let _ = serde_json::to_value(&status).expect("status serializes");
}
```

> `Orchestrator::default()` does NOT exist. The canonical test constructor is `Orchestrator::new(OrchestratorConfig::for_testing())` (verified in use at `crates/vox-orchestrator/src/orchestrator/tests/mod.rs:148`). `DEFAULT_ATTENTION_BUDGET_MS` is re-exported at `crate::attention::DEFAULT_ATTENTION_BUDGET_MS` (`attention/mod.rs:16`) — prefer that over the deep `super::super::attention::budget::` path. Confirm the `OrchestratorConfig` import path with `rg -n "OrchestratorConfig::for_testing|impl Orchestrator" crates/vox-orchestrator/src` if it differs.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator status_includes_attention_budget_snapshot` → FAIL (`no field attention_budget`).

- [ ] **Step 4: Add the field.** In `types.rs`, add to `OrchestratorStatus` (after `pub agents: Vec<AgentSummary>,`):

```rust
    /// Live NASA-TLX attention-budget snapshot for GUI surfacing (Track D). `None` if unavailable.
    #[serde(default)]
    pub attention_budget: Option<crate::attention::budget::AttentionBudget>,
```

> `AttentionBudget` already derives `Serialize, Deserialize` — no extra work. If `types.rs` lacks `use` for it, the fully-qualified path above avoids an import.

- [ ] **Step 5: Populate it in `status()`.** In `accessors.rs`, `budget` is UNCONDITIONALLY dropped at `:49`, ~40 lines before the `OrchestratorStatus { ... }` literal at `:90`. So you MUST capture the snapshot early — do NOT reference `budget` in the literal (it is already moved there). Immediately after the `let budget = ...rw_read(&self.budget_manager);` line (≈:15), add:

```rust
    let attention_budget = Some(budget.attention_snapshot());
```

Then in the `OrchestratorStatus { ... }` literal (≈:90), add the field `attention_budget,` (shorthand for the local). Verify the read line with `rg -n "let budget = |drop\(budget\)|OrchestratorStatus \{" crates/vox-orchestrator/src/orchestrator/accessors.rs`.

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-orchestrator status_includes_attention_budget_snapshot` → PASS; `cargo check -p vox-orchestrator` → compiles (every `OrchestratorStatus { .. }` literal now needs the field — `rg -n "OrchestratorStatus \{" crates/` and add `attention_budget: None,` to any other literal, e.g. test fixtures).

- [ ] **Step 7: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/orchestrator/types.rs crates/vox-orchestrator/src/orchestrator/accessors.rs
git commit -m "feat(orchestrator): include attention-budget snapshot in status (Phase A, audit #1)"
```

---

## Task 4 `[SEQUENTIAL]`: Pass the budget through the GUI host status (Phase A, host)

The daemon serializes `OrchestratorStatus` to JSON; `to_gui_status` rebuilds `GuiOrchestratorStatus` from that JSON. Add a pass-through field.

**Files:** Modify `crates/vox-gui/src/commands/orchestrator.rs` (`GuiOrchestratorStatus` + `to_gui_status`).

- [ ] **Step 1 (verify-before-use):** `rg -n "struct GuiOrchestratorStatus" -A 20 crates/vox-gui/src/commands/orchestrator.rs` and `rg -n "fn to_gui_status" -A 30 crates/vox-gui/src/commands/orchestrator.rs`. Note how `to_gui_status` reads fields from its raw `serde_json::Value` argument (e.g. `raw["agent_count"].as_u64()`), and the argument's name.

- [ ] **Step 2: Write the failing test.** Append a `#[cfg(test)]` test (use `--lib` to dodge the Tauri build-script clippy gotcha):

```rust
#[test]
fn gui_status_passes_through_attention_budget() {
    let raw = serde_json::json!({
        "agent_count": 0, "total_queued": 0, "total_in_progress": 0,
        "total_completed": 0, "total_doubted": 0,
        "attention_budget": { "max_attention_ms": 3_600_000, "spent_ms": 1_800_000,
            "total_requests": 0, "auto_approved": 0, "rejected": 0,
            "interrupt_freq_per_hour": 9.0, "last_interrupt_ms": 0, "inbox_suppressed_count": 0 }
    });
    let gui = to_gui_status(raw);
    let ab = gui.attention_budget.expect("budget passed through");
    assert_eq!(ab["spent_ms"], 1_800_000);
}
```

> Adjust the `raw` object to include whatever non-optional fields `to_gui_status` unwraps; the point is the `attention_budget` key survives.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-gui --lib gui_status_passes_through_attention_budget` → FAIL.

- [ ] **Step 4: Implement.** Add to `GuiOrchestratorStatus` (it derives `Serialize`): 

```rust
    /// Live attention-budget snapshot passed through verbatim from the daemon (Track D). May be null.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention_budget: Option<serde_json::Value>,
```

In `to_gui_status`, set it from the raw value (use the real raw-arg name from Step 1):

```rust
        attention_budget: raw.get("attention_budget").cloned().filter(|v| !v.is_null()),
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-gui --lib gui_status_passes_through_attention_budget` → PASS; `cargo check -p vox-gui` → compiles.

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-gui --lib -- -D warnings`; `cargo fmt -p vox-gui`; then:

```bash
git add crates/vox-gui/src/commands/orchestrator.rs
git commit -m "feat(gui): pass attention-budget snapshot through GuiOrchestratorStatus (Phase A)"
```

---

## Task 5 `[SEQUENTIAL]`: Type the budget in the transport layer (Phase A, bridge)

**Files:** Modify `crates/vox-gui/ui/src/transport.ts`.

- [ ] **Step 1 (verify-before-use):** `rg -n "interface OrchestratorStatus|type OrchestratorStatus|agent_count" crates/vox-gui/ui/src/transport.ts`. Find the `OrchestratorStatus` TS shape that `listenOrchStatus` delivers. Note its exact location.

- [ ] **Step 2: Add the snapshot type + field.** Insert near the `OrchestratorStatus` definition:

```typescript
/** Flat attention-budget snapshot (Rust `AttentionBudget`), surfaced via the orchestrator status stream. */
export interface AttentionBudgetSnapshot {
  max_attention_ms: number;
  spent_ms: number;
  total_requests: number;
  auto_approved: number;
  rejected: number;
  interrupt_freq_per_hour: number;
  last_interrupt_ms: number;
  inbox_suppressed_count: number;
}
```

Add to the `OrchestratorStatus` interface:

```typescript
  /** Present when the daemon reports the live attention budget (Track D). */
  attention_budget?: AttentionBudgetSnapshot | null;
```

- [ ] **Step 3: Typecheck.** From `crates/vox-gui/ui`: `npx tsc --noEmit` → no errors.

- [ ] **Step 4: Commit.**

```bash
git add crates/vox-gui/ui/src/transport.ts
git commit -m "feat(gui-ui): type attention_budget on OrchestratorStatus (Phase A bridge)"
```

---

## Task 6 `[SEQUENTIAL]`: `AttentionBudgetMeter` component + mount (Phase A, surface — audit #1)

Derive focus depth from `interrupt_freq_per_hour` (the same thresholds as Rust `focus_depth()`: ≥8 Deep, ≥3 Focused, else Ambient) so the meter shows the **real** depth.

**Files:** Create `crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx` + `.test.tsx`; Modify `crates/vox-gui/ui/src/App.tsx`.

- [ ] **Step 1 (verify-before-use):** `rg -n "@testing-library/react|describe\(|render\(" crates/vox-gui/ui/src/components/surfaces` (any existing `*.test.tsx`) to confirm the vitest + testing-library idiom. `rg -n "applyStatus|listenOrchStatus|OrchestratorStatus" crates/vox-gui/ui/src/App.tsx` to find where status is stored in state.

- [ ] **Step 2: Write the failing test.** Create `AttentionBudgetMeter.test.tsx`:

```tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { AttentionBudgetMeter } from './AttentionBudgetMeter';
import type { AttentionBudgetSnapshot } from '../../transport';

const snap: AttentionBudgetSnapshot = {
  max_attention_ms: 3_600_000, spent_ms: 1_800_000, total_requests: 4,
  auto_approved: 1, rejected: 1, interrupt_freq_per_hour: 9.0,
  last_interrupt_ms: 0, inbox_suppressed_count: 2,
};

describe('AttentionBudgetMeter', () => {
  it('renders spent ratio, derived focus depth, and suppressed count', () => {
    render(<AttentionBudgetMeter budget={snap} />);
    expect(screen.getByRole('meter')).toHaveAttribute('aria-valuenow', '50');
    expect(screen.getByText(/deep/i)).toBeInTheDocument();
    expect(screen.getByText(/2/)).toBeInTheDocument(); // inbox_suppressed_count
  });
  it('derives focused vs ambient from interrupt frequency', () => {
    render(<AttentionBudgetMeter budget={{ ...snap, interrupt_freq_per_hour: 4 }} />);
    expect(screen.getByText(/focused/i)).toBeInTheDocument();
  });
  it('renders nothing when budget is null', () => {
    const { container } = render(<AttentionBudgetMeter budget={null} />);
    expect(container).toBeEmptyDOMElement();
  });
});
```

- [ ] **Step 3: Run → FAIL.** From `crates/vox-gui/ui`: `npx vitest run AttentionBudgetMeter` → FAIL.

- [ ] **Step 4: Implement the component.** Create `AttentionBudgetMeter.tsx`:

```tsx
import type { AttentionBudgetSnapshot } from '../../transport';

interface Props {
  budget: AttentionBudgetSnapshot | null | undefined;
}

// Mirrors Rust AttentionBudget::focus_depth() thresholds.
function focusLabel(freqPerHour: number): string {
  if (freqPerHour >= 8) return 'Deep focus';
  if (freqPerHour >= 3) return 'Focused';
  return 'Ambient focus';
}

/**
 * Read-only attention-budget surface (Track D, audit #1). Shows session attention spent,
 * the current focus depth (derived from interrupt frequency), and how many A2A prompts
 * were suppressed under Deep focus. Rides the existing orchestrator status stream.
 */
export function AttentionBudgetMeter({ budget }: Props) {
  if (!budget) return null;
  const ratio = budget.max_attention_ms > 0 ? budget.spent_ms / budget.max_attention_ms : 1;
  const pct = Math.round(Math.min(Math.max(ratio, 0), 1) * 100);
  const min = (ms: number) => Math.round(ms / 60_000);
  return (
    <section className="attention-budget-meter" aria-label="Attention budget">
      <header>
        <span>Attention budget</span>
        <span>{focusLabel(budget.interrupt_freq_per_hour)}</span>
      </header>
      <div role="meter" aria-label="Attention spent" aria-valuemin={0} aria-valuemax={100} aria-valuenow={pct}>
        <div className="attention-budget-meter__fill" style={{ width: `${pct}%` }} />
      </div>
      <p>{min(budget.spent_ms)} / {min(budget.max_attention_ms)} min spent ({pct}%)</p>
      <p>Suppressed prompts (Deep focus): {budget.inbox_suppressed_count}</p>
    </section>
  );
}
```

- [ ] **Step 5: Run → PASS.** From `crates/vox-gui/ui`: `npx vitest run AttentionBudgetMeter` → PASS; `npx tsc --noEmit` → clean.

- [ ] **Step 6: Mount it.** In `App.tsx`, where the status object is held in state (from Step 1), render the meter, e.g. `<AttentionBudgetMeter budget={status?.attention_budget} />` inside the existing dashboard region. Add the import: `import { AttentionBudgetMeter } from './components/surfaces/AttentionBudgetMeter';`. Then `npx tsc --noEmit` → clean and `npx vitest run` → all pass.

- [ ] **Step 7: Commit.**

```bash
git add crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.tsx crates/vox-gui/ui/src/components/surfaces/AttentionBudgetMeter.test.tsx crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui-ui): AttentionBudgetMeter surface on the status stream (Phase A, audit #1)"
```

---

## Task 7 `[SEQUENTIAL]`: Add spec/model uncertainty fields to `InterruptionSignals` (audit #5)

**Files:** Modify `crates/vox-orchestrator/src/attention/interruption_policy.rs`; Test: same file.

- [ ] **Step 1 (verify-before-use):** `rg -n "pub struct InterruptionSignals" -A 30 crates/vox-orchestrator/src/attention/interruption_policy.rs`. Confirm `confidence_estimate: f64`, `open_question_session: bool` (last field), derives `Serialize, Deserialize`, and NO existing `spec_uncertainty`/`model_uncertainty`. `rg -n "enum InterruptionChannel" -A 12` for the real channel variant names.

- [ ] **Step 2: Write the failing test.** Append to the test module:

```rust
#[test]
fn signals_default_separates_spec_and_model_uncertainty() {
    let json = r#"{
        "channel": "InlineAssist","expected_information_gain_bits":0.2,"expected_user_cost":0.3,
        "confidence_estimate":0.6,"contradiction_ratio":0.0,"pending_clarification_backlog":0,
        "clarification_turn_index":0,"max_clarification_turns":3,"irreversible_or_high_risk":false,
        "base_interrupt_cost_ms":23250,"trust_score":0.5,"open_question_session":false }"#;
    let s: InterruptionSignals = serde_json::from_str(json).expect("legacy JSON must deserialize");
    assert_eq!(s.spec_uncertainty, 0.0);
    assert_eq!(s.model_uncertainty, 0.0);
}
```

> Fix `"InlineAssist"` to a real `InterruptionChannel` variant if Step 1 shows a different name.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator signals_default_separates_spec_and_model_uncertainty` → FAIL.

- [ ] **Step 4: Implement.** Add to the END of `InterruptionSignals` (before the closing `}`):

```rust
    /// Specification uncertainty in `[0, 1]`: how unresolved the *user's intent/parameters* are
    /// (high → a clarifying question is likely to help). Defaults to 0.0 for legacy payloads.
    #[serde(default)]
    pub spec_uncertainty: f64,
    /// Model uncertainty in `[0, 1]`: the LLM's *own* epistemic doubt about its prediction
    /// (high → asking the user may not help). Defaults to 0.0 for legacy payloads.
    #[serde(default)]
    pub model_uncertainty: f64,
```

Then `rg -n "InterruptionSignals \{" crates/` and add `spec_uncertainty: 0.0, model_uncertainty: 0.0,` to every struct literal (check for a `Default` impl with `rg -n "impl Default for InterruptionSignals"`; if present, prefer `..Default::default()`).

- [ ] **Step 5: Run → PASS + whole-workspace compile.** `cargo test -p vox-orchestrator signals_default_separates` → PASS; `cargo check -p vox-orchestrator -p vox-orchestrator-mcp` → compiles.

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/attention/interruption_policy.rs
git commit -m "feat(attention): add spec/model uncertainty axes to InterruptionSignals (audit #5)"
```

---

## Task 8 `[SEQUENTIAL]` (same file as T7): Use the spec/model split in the ask decision

When the model is unsure but the spec is clear, a question can't help — raise the bar.

**Files:** Modify `crates/vox-orchestrator/src/attention/interruption_policy.rs` (`evaluate_interruption`, final utility branch).

- [ ] **Step 1 (verify-before-use):** `rg -n "let utility =|let threshold =|min_utility_threshold|InterruptNow \{" crates/vox-orchestrator/src/attention/interruption_policy.rs`. Read the final ~15 lines that compute `utility`, `threshold`, and return `DeferUntilCheckpoint`/`InterruptNow`. Confirm `let trust_adj = ...;` precedes them.

- [ ] **Step 2: Write the failing test.** Append (build a baseline that would `InterruptNow`, then flip via model-only doubt):

```rust
fn base_signals() -> InterruptionSignals {
    InterruptionSignals {
        channel: InterruptionChannel::InlineAssist,
        expected_information_gain_bits: 0.30, expected_user_cost: 0.25,
        confidence_estimate: 0.60, contradiction_ratio: 0.0,
        pending_clarification_backlog: 0, clarification_turn_index: 0, max_clarification_turns: 3,
        irreversible_or_high_risk: false, base_interrupt_cost_ms: DEFAULT_INTERRUPT_COST_MS,
        trust_score: 0.5, open_question_session: false,
        spec_uncertainty: 0.0, model_uncertainty: 0.0,
    }
}

#[test]
fn model_uncertainty_without_spec_uncertainty_suppresses_question() {
    let b = AttentionBudget::default();
    let mut s = base_signals(); s.spec_uncertainty = 0.1; s.model_uncertainty = 0.9;
    assert!(matches!(evaluate_interruption(&s, &b, true, 0.5),
        InterruptionDecision::DeferUntilCheckpoint { .. } | InterruptionDecision::ProceedAutonomously { .. }),
        "model-only uncertainty should not interrupt");
    let mut s2 = base_signals(); s2.spec_uncertainty = 0.9; s2.model_uncertainty = 0.1;
    assert!(matches!(evaluate_interruption(&s2, &b, true, 0.5), InterruptionDecision::InterruptNow { .. }),
        "spec ambiguity should interrupt");
}
```

> Confirm `DEFAULT_INTERRUPT_COST_MS` and `AttentionBudget` are in scope in this module (they are imported at top — verify in Step 1). Fix `InlineAssist` to the real variant if needed.

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator model_uncertainty_without_spec_uncertainty_suppresses_question` → FAIL (both currently `InterruptNow`).

- [ ] **Step 4: Implement.** Replace the existing `let threshold = min_utility_threshold(...) + trust_adj;` line and its following `if utility < threshold && !signals.irreversible_or_high_risk { ... }` block with a spec-adjusted version:

```rust
    // Spec-vs-model uncertainty (SAGE-Agent, arXiv:2511.08798): a clarifying question resolves
    // *specification* uncertainty, not the model's own epistemic doubt. Raise the bar when the
    // model is unsure but the spec is clear — asking the user cannot resolve that.
    let unresolvable_model_doubt =
        (signals.model_uncertainty - signals.spec_uncertainty).clamp(0.0, 1.0);
    let threshold = min_utility_threshold(spent_ratio, attention_alert_threshold)
        + trust_adj
        + 0.40 * unresolvable_model_doubt;
    if utility < threshold && !signals.irreversible_or_high_risk {
        return InterruptionDecision::DeferUntilCheckpoint {
            reason: format!(
                "utility_below_threshold utility={utility:.4} threshold={threshold:.4} spent_ratio={spent_ratio:.3} model_doubt={unresolvable_model_doubt:.3}"
            ),
        };
    }
```

Do not duplicate the `let utility = ...` / `let trust_adj = ...` lines above it.

- [ ] **Step 5: Run → PASS (incl. pre-existing).** `cargo test -p vox-orchestrator interruption` → all PASS.

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/attention/interruption_policy.rs
git commit -m "feat(attention): raise ask threshold for model-only uncertainty (SAGE-Agent, audit #5)"
```

---

## Task 9 `[SEQUENTIAL]`: Pure calibrator — offset + aggregate from logged outcomes (audit #4, part 1)

**Files:** Create `crates/vox-orchestrator/src/attention/calibrator.rs`; Modify `crates/vox-orchestrator/src/attention/mod.rs`; Modify `docs/src/architecture/where-things-live.md`.

- [ ] **Step 1 (verify-before-use):** `rg -n "pub enum ApprovalOutcome|enum AttentionEventType|PolicyDeferred|PolicyProceedAuto" crates/vox-orchestrator/src/attention` and `crates/vox-orchestrator/src/attention_tracker.rs`. Note the exact serialized strings for accepted/rejected and the suppressed event types (`PolicyDeferred`, `PolicyProceedAuto`).

- [ ] **Step 2: Create the file test-first.** `calibrator.rs`:

```rust
//! Deterministic calibration: turn logged interruption outcomes into a per-channel *gain* offset.
//! Sign convention matches `attention_policy::apply_calibration`, which ADDS the offset to
//! `expected_information_gain_bits` (higher gain ⇒ MORE likely to ask). So a channel that wastes
//! attention (high reject rate) must get a NEGATIVE offset (ask less); a well-received channel
//! gets a small POSITIVE offset (ask a bit more freely). Counts include suppressed-then-logged
//! decisions, satisfying the SSOT's "learn from suppressed interruptions too."

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChannelOutcomeCounts {
    pub accepted: u32,
    pub rejected: u32,
    pub suppressed: u32,
}

/// Bounded gain offset in `[-0.15, +0.05]` bits. NEGATIVE = raise the ask bar (channel wastes attention).
#[must_use]
pub fn channel_gain_offset(c: ChannelOutcomeCounts) -> f64 {
    let shown = c.accepted + c.rejected;
    if shown == 0 {
        return 0.0;
    }
    let reject_rate = c.rejected as f64 / shown as f64;
    // Center at a 25% acceptable reject rate. High reject ⇒ negative offset (ask less).
    let raw = (0.25 - reject_rate) * 0.20;
    raw.clamp(-0.15, 0.05)
}

/// Aggregate `(channel, outcome_or_event_str)` rows (as read from `attention_events`) into counts.
#[must_use]
pub fn aggregate_counts(
    rows: &[(Option<String>, String)],
) -> std::collections::HashMap<String, ChannelOutcomeCounts> {
    let mut map: std::collections::HashMap<String, ChannelOutcomeCounts> = std::collections::HashMap::new();
    for (channel, outcome) in rows {
        let key = channel.clone().unwrap_or_else(|| "unknown".to_string());
        let e = map.entry(key).or_default();
        match outcome.as_str() {
            "Accepted" | "Answered" => e.accepted += 1,
            "Rejected" => e.rejected += 1,
            "PolicyDeferred" | "PolicyProceedAuto" | "Suppressed" => e.suppressed += 1,
            _ => {}
        }
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_data_is_neutral() { assert_eq!(channel_gain_offset(ChannelOutcomeCounts::default()), 0.0); }
    #[test]
    fn high_reject_rate_lowers_gain_to_ask_less() {
        let c = ChannelOutcomeCounts { accepted: 1, rejected: 9, suppressed: 3 };
        assert!(channel_gain_offset(c) < 0.0, "frequent rejection ⇒ negative offset (ask less)");
    }
    #[test]
    fn mostly_accepted_raises_gain_slightly() {
        let c = ChannelOutcomeCounts { accepted: 19, rejected: 1, suppressed: 0 };
        assert!(channel_gain_offset(c) > 0.0, "mostly-accepted ⇒ positive offset (ask a bit more)");
    }
    #[test]
    fn offset_is_bounded() {
        assert!(channel_gain_offset(ChannelOutcomeCounts { accepted: 0, rejected: 100, suppressed: 0 }) >= -0.15);
    }
    #[test]
    fn aggregate_buckets_by_channel_and_outcome() {
        let rows = vec![
            (Some("mcp_chat".into()), "Rejected".to_string()),
            (Some("mcp_chat".into()), "Accepted".to_string()),
            (Some("mcp_chat".into()), "PolicyDeferred".to_string()),
        ];
        let m = aggregate_counts(&rows);
        assert_eq!(m["mcp_chat"], ChannelOutcomeCounts { accepted: 1, rejected: 1, suppressed: 1 });
    }
}
```

> Replace the match-arm strings with the EXACT serialized values confirmed in Step 1 if they differ.

- [ ] **Step 3: Register module.** In `crates/vox-orchestrator/src/attention/mod.rs` add `pub mod calibrator;`.

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-orchestrator calibrator` → PASS.

- [ ] **Step 5: where-things-live row.** Add to `docs/src/architecture/where-things-live.md`:

```markdown
| Attention interruption calibrator (learns from logged outcomes) | `crates/vox-orchestrator/src/attention/calibrator.rs` |
```

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; `cargo run -p vox-arch-check`; then:

```bash
git add crates/vox-orchestrator/src/attention/calibrator.rs crates/vox-orchestrator/src/attention/mod.rs docs/src/architecture/where-things-live.md
git commit -m "feat(attention): deterministic per-channel calibrator from logged outcomes (audit #4)"
```

---

## Task 10 `[SEQUENTIAL]`: Feed learned offsets into `InterruptionCalibrationConfig` (audit #4, part 2 — actually close the loop)

`apply_calibration` (`vox-orchestrator-mcp/src/attention_policy.rs:11`) already reads the four `*_gain_offset_bits` fields of `InterruptionCalibrationConfig`. Produce an updated config from the per-channel offsets so the loop closes.

**Files:** Modify `crates/vox-orchestrator/src/attention/calibrator.rs`; Test: same file.

- [ ] **Step 1 (verify-before-use):** `rg -n "pub struct InterruptionCalibrationConfig" -A 14 crates/vox-orchestrator/src/attention/budget.rs`. Confirm fields: `plan_review_gain_offset_bits`, `task_submit_gain_offset_bits`, `a2a_escalation_gain_offset_bits`, `inline_assist_gain_offset_bits`, `backlog_cost_penalty_per_item`, `trust_adjustment_scale`. Confirm `apply_calibration` ADDS the gain offset: `rg -n "expected_information_gain_bits = " crates/vox-orchestrator-mcp/src/attention_policy.rs`.

- [ ] **Step 2: Write the failing test.** Append to `calibrator.rs`:

```rust
use crate::attention::budget::InterruptionCalibrationConfig;

/// Produce a calibrated config by overwriting the four channel gain-offset fields from learned
/// per-channel counts. The HashMap keys are the REAL surface strings recorded in
/// `AttentionEvent.channel` (e.g. "vox_plan", "vox_inline_edit", "vox_ghost_text", the chat
/// surface) — NOT synthetic labels. We map them to channels via the existing
/// `interruption_channel_for_surface` helper to avoid string drift (DRY/SSOT).
/// Non-channel fields (backlog, trust) are preserved from `base`.
#[must_use]
pub fn apply_learned_offsets(
    base: InterruptionCalibrationConfig,
    counts: &std::collections::HashMap<String, ChannelOutcomeCounts>,
) -> InterruptionCalibrationConfig {
    let mut cfg = base;
    for (surface, c) in counts {
        let offset = channel_gain_offset(*c);
        // Reuse the SSOT surface→channel mapping (≈interruption_policy.rs:680) so this
        // never drifts from how events are actually tagged.
        match interruption_channel_for_surface(surface) {
            InterruptionChannel::PlanReview => cfg.plan_review_gain_offset_bits = offset,
            InterruptionChannel::TaskSubmit => cfg.task_submit_gain_offset_bits = offset,
            InterruptionChannel::A2aEscalation => cfg.a2a_escalation_gain_offset_bits = offset,
            InterruptionChannel::InlineAssist | InterruptionChannel::ChatClarification => {
                cfg.inline_assist_gain_offset_bits = offset
            }
            _ => {}
        }
    }
    cfg
}

#[cfg(test)]
mod close_loop_tests {
    use super::*;
    #[test]
    fn wasteful_channel_gets_negative_offset_into_config() {
        let mut counts = std::collections::HashMap::new();
        // Use a REAL surface string (what events actually carry), not "mcp_chat".
        counts.insert("vox_inline_edit".to_string(), ChannelOutcomeCounts { accepted: 1, rejected: 9, suppressed: 0 });
        let cfg = apply_learned_offsets(InterruptionCalibrationConfig::default(), &counts);
        assert!(cfg.inline_assist_gain_offset_bits < 0.0, "wasteful inline channel ⇒ ask less");
        // non-channel knobs preserved
        assert_eq!(cfg.backlog_cost_penalty_per_item, InterruptionCalibrationConfig::default().backlog_cost_penalty_per_item);
    }
}
```

> **Step 1 verify (do FIRST):** `AttentionEvent.channel` is `Some(surface.to_string())` — i.e. real surface strings like `"vox_plan"`, `"vox_inline_edit"`, `"vox_ghost_text"`, not `"mcp_chat"`. Confirm with `rg -n "channel: Some\(|fn interruption_channel_for_surface|enum InterruptionChannel" crates/vox-orchestrator/src crates/vox-orchestrator-types/src`, import `interruption_channel_for_surface` + `InterruptionChannel`, and match the EXACT variant names it returns. The synthetic-key version silently produces empty offsets in production (audit #4 stays open while the test passes), which is why this mapping must go through the real surface helper.

- [ ] **Step 3: Run → FAIL then PASS.** `cargo test -p vox-orchestrator close_loop` → implement until PASS.

- [ ] **Step 4: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/attention/calibrator.rs
git commit -m "feat(attention): feed learned offsets into InterruptionCalibrationConfig (audit #4 closed)"
```

> **Wiring note (for a follow-up task or the integrator, not this atomic task):** call `apply_learned_offsets` periodically — read recent `attention_events` rows, `aggregate_counts`, then write the result into the running `OrchestratorConfig.interruption_calibration`. That hot-path mutation needs a config write lock and is out of scope for this green-by-itself task; the pure function above is the load-bearing piece and is fully tested.

---

## Task 11 `[SEQUENTIAL]`: Sharpen the Socrates questioning prompt — reason over solutions (research §2.1)

The strongest SoTA lever: instruct the model to enumerate the candidate **solution set** and ask the question that best splits it (arXiv:2502.04485), separating "what you (the user) want" (spec) from "what I'm unsure of" (model).

**Files:** Modify `crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs` (`socrates_system_rider`).

- [ ] **Step 1 (verify-before-use):** `rg -n "fn socrates_system_rider" -A 18 crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs`. Confirm it returns `String`, takes `policy: &ConfidencePolicy`, and uses `p.abstain_threshold` and `p.ask_for_help_threshold`.

- [ ] **Step 2: Write the failing test.** Append a `#[cfg(test)]` test (build a `ConfidencePolicy::default()` — confirm constructor with `rg -n "impl Default for ConfidencePolicy|ConfidencePolicy::default"`):

```rust
#[cfg(test)]
mod rider_tests {
    use super::*;
    #[test]
    fn rider_instructs_solution_space_reasoning_and_spec_model_split() {
        let r = socrates_system_rider(&vox_orchestrator_types::socrates_policy::ConfidencePolicy::default());
        assert!(r.contains("candidate"), "must mention enumerating candidate solutions");
        assert!(r.to_lowercase().contains("information gain") || r.contains("splits"), "must mention picking the most-diagnostic question");
        assert!(r.to_lowercase().contains("you want") || r.to_lowercase().contains("specification"), "must distinguish user-spec uncertainty");
        // existing behaviour preserved:
        assert!(r.contains("calibrated confidence"));
    }
}
```

> Adjust the `ConfidencePolicy` import path to the real one from Step 1 (`rg -n "use .*ConfidencePolicy" crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs`).

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator-mcp rider_instructs_solution_space` → FAIL.

- [ ] **Step 4: Implement.** Replace the `format!` body of `socrates_system_rider` with the sharpened rider (keeps the existing confidence-band lines, adds the solution-space guidance):

```rust
    format!(
        "\n## Socrates (grounding & diagnostic questioning)\n\
         - Below {:.0}% calibrated confidence: do not speculate; state what evidence is missing.\n\
         - {:.0}–{:.0}%: answer with explicit uncertainty or ask ONE focused clarifying question.\n\
         - Above {:.0}%: answer normally; tie claims to files or tools you used.\n\
         When you do ask, ask the single most diagnostic question:\n\
         1. Enumerate the 2–4 candidate solutions/interpretations consistent with the request so far.\n\
         2. Ask the one question whose answer best SPLITS those candidates (maximizes information gain over solutions, not over phrasings).\n\
         3. Separate *specification* uncertainty (what YOU want — a question can resolve this) from *model* uncertainty (what I'm unsure of — a question usually cannot). Only ask about the former.\n\
         4. Prefer a bounded multiple-choice over open-ended when the candidate set is known; never ask what context already implies.\n",
        p.abstain_threshold * 100.0,
        p.abstain_threshold * 100.0,
        p.ask_for_help_threshold * 100.0,
        p.ask_for_help_threshold * 100.0,
    )
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator-mcp rider` → PASS; full crate `cargo test -p vox-orchestrator-mcp` → PASS (rider is used in other tests; confirm none assert the exact old string — if one does, update it to match the new guidance).

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator-mcp -- -D warnings`; `cargo fmt -p vox-orchestrator-mcp`; then:

```bash
git add crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs
git commit -m "feat(socrates): rider reasons over the solution space + spec/model split (research §2.1)"
```

---

## Task 12a `[SEQUENTIAL]` (same file as T11): `WithheldQuestion` payload (pure — lands green)

When the policy defers/suppresses a question it currently records telemetry and `return`s silently. First land a pure, serializable payload type. (T12b does the actual surfacing.)

**Files:** Modify `crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs`.

- [ ] **Step 1: Add a pure, serializable payload + test.**

```rust
/// A question the policy chose NOT to surface, exposed to the client so the user can opt in.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub(crate) struct WithheldQuestion {
    pub(crate) prompt: String,
    pub(crate) reason: String,
    pub(crate) expected_information_gain_bits: f64,
}

#[must_use]
pub(crate) fn withheld_question(prompt: &str, reason: &str, eig_bits: f64) -> WithheldQuestion {
    WithheldQuestion { prompt: prompt.to_string(), reason: reason.to_string(), expected_information_gain_bits: eig_bits }
}

#[cfg(test)]
mod withheld_tests {
    use super::*;
    #[test]
    fn withheld_serializes_for_client() {
        let w = withheld_question("Which environment — staging or prod?", "backlog_and_low_diagnostic_value", 0.07);
        let v = serde_json::to_value(&w).unwrap();
        assert_eq!(v["prompt"], "Which environment — staging or prod?");
        assert_eq!(v["reason"], "backlog_and_low_diagnostic_value");
    }
}
```

- [ ] **Step 2: Run → PASS + commit.** `cargo test -p vox-orchestrator-mcp withheld`; clippy/fmt; then:

```bash
git add crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs
git commit -m "feat(chat): WithheldQuestion payload type (pure)"
```

---

## Task 12b `[SEQUENTIAL]`: Persist withheld questions so the client can read them

> **ARCHITECTURAL CORRECTION (audit 2026-06-18):** the defer/proceed branches execute INSIDE
> `spawn_supervised_infallible("questioning_trace", async move { ... })` — a detached,
> fire-and-forget telemetry task. The `return;` statements exit that closure; **the function
> returns NOTHING to the chat client at this site** (the local `meta` is input, consumed for
> telemetry). So a withheld question CANNOT be attached to an "outward meta value the function
> returns" — there is none. Persist it instead, mirroring the existing metric-recording calls in
> these same branches (`db.record_questioning_metric(...)` / `insert_question_event(...)`), so the
> GUI reads it from the questioning surface.

**Files:** Modify `crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs` (and possibly the questioning-metric payload builder it calls).

- [ ] **Step 1 (verify-before-use):** `rg -n "DeferUntilCheckpoint|BatchWithExistingPrompt|ProceedAutonomously|record_questioning_metric|insert_question_event|questioning_policy_metric_payload|spawn_supervised_infallible|return;" -A 3 crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs`. Confirm the branches run inside the detached `spawn_supervised_infallible("questioning_trace", ...)` closure and that they already call a DB metric/record fn. Identify `questioning_policy_metric_payload(...)` (the payload these branches persist).

- [ ] **Step 2: Failing test.** Assert the questioning-metric payload built for a deferred decision includes a `withheld_question` field (serialized) alongside the existing `policy_outcome`.

- [ ] **Step 3: Implement.** In the defer/proceed branches, BEFORE the existing record/insert call, build `withheld_question(&proposed_prompt, reason, eig_bits)` (use the real local names confirmed in Step 1) and add it to the metric payload via `questioning_policy_metric_payload` (add a `withheld_question: Option<WithheldQuestion>` field to that payload, defaulted `None`, serialized with `skip_serializing_if = "Option::is_none"`). The GUI reads it from the questioning metric surface. Do NOT attempt to return it from the function — there is no outward return at this site.

- [ ] **Step 4: Run → PASS + commit.** `cargo test -p vox-orchestrator-mcp`; `cargo check`; clippy/fmt; then:

```bash
git add crates/vox-orchestrator-mcp/src/chat_socrates_meta.rs
git commit -m "feat(chat): persist withheld questions to questioning-metric payload for GUI opt-in"
```

---

## Self-Review (author checklist — applied)

- **Spec coverage:** GUI surface → T3–T6 (real `attention_snapshot()` source, not reconstructed); audit #2 → T1; #3 → T2; #5 → T7–T8; #4 → T9–T10 (offset now consumed by `InterruptionCalibrationConfig`, sign verified); research §2.1 prompt lever → T11; chat-protocol withheld-question → T12; Antigravity run blocker → T0. ✅
- **Parallelization correctness:** only (T1, T2) are truly parallel-safe; Phase A is a strict sequential data path (T3→T4→T5→T6) because each layer consumes the previous field; T7→T8, T9→T10, T11→T12 share files → SEQUENTIAL. The earlier "4-wide wave" bug is removed. ✅
- **Data-source correctness:** focus depth now comes from the live `interrupt_freq_per_hour` via `BudgetManager::attention_snapshot()`, not from `session_summary` (which drops it). ✅
- **Sign correctness:** `apply_calibration` ADDS gain offset ⇒ wasteful channel needs a NEGATIVE offset; T9/T10 tests assert this. ✅
- **Placeholder scan:** no "TODO/implement later" code; the only deferred item (hot-path config mutation) is explicitly carved out as a non-atomic follow-up note under T10, with the load-bearing pure function fully tested. ✅
- **Antigravity safety:** every task atomic + green + committed; every code step preceded by verify-before-use; `.agents/skills/` created first; `npx vitest run` confirmed against `package.json`. ✅

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-18-track-d-attention-aware-questioning.md`.

**Antigravity run order:** T0 (verify-only; likely no-op) → (T1 verify-only ∥ T2) → T3 → T4 → T5 → T6 → T7 → T8 → T9 → T10 → T11 → T12a → T12b. Each task ends green + committed, so a mid-task termination wastes ≤ one task. **Note:** T0 and T1 are already satisfied in `main` — treat them as verify-and-skip, not re-implement (re-running them duplicates an existing test / mount and breaks the tree). Run the handoff checklist in [`../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md`](../../src/contributors/antigravity-handoff-and-skill-gaps-2026-06-18.md) §6 first (T0 satisfies the `.agents/skills/` prerequisite).

**If executed in Claude Code instead — two options:**
1. **Subagent-Driven (recommended)** — fresh subagent per task, review between tasks (`.agents/skills/subagent-driven-development.skill.md`).
2. **Inline Execution** — batch with checkpoints (`.agents/skills/executing-plans.skill.md`).

---

## Post-Execution Audit & Ledger (2026-06-19, Claude Code review)

Antigravity/Gemini 3.5 Flash executed T0–T12. This is the verified expectation-vs-reality ledger.

### Ledger — what actually landed

| Task | Expectation | Reality | Status |
|---|---|---|---|
| T0 `.agents/skills/` | mount native skills | junction present | ✅ done |
| T1 single-source constant | re-export from policy SSOT | `budget.rs:88` re-exports | ✅ done |
| T2 dead link | repoint to archive | `information-theoretic-questioning.md:217` fixed | ✅ done |
| T3 status snapshot | `OrchestratorStatus.attention_budget` from `attention_snapshot()` | `types.rs:129` + `accessors.rs:16,114` | ✅ done |
| T4 GUI host passthrough | `GuiOrchestratorStatus` field | `orchestrator.rs:171,279` | ✅ done |
| T5 transport type | TS snapshot type | `types/tauri.ts:124,128` (note: lives in `types/tauri.ts`, not `transport.ts` as planned) | ✅ done |
| T6 meter + mount | component **rendered** | `AttentionBudgetMeter.tsx` + threaded `App.tsx:1001 → surfaceComponents.tsx:92 → Dashboard.tsx:308`. **Fully mounted end-to-end.** | ✅ done |
| T7/T8 spec/model uncertainty | fields + use in `evaluate_interruption` | committed `3ac52d223e`, `a171581633` | ✅ done |
| T9 calibrator | offset + aggregate | `calibrator.rs` — **`aggregate_counts` had a silent-no-op bug** (see below) | ⚠️ fixed in review |
| T10 feed offsets to config | `apply_learned_offsets` | present; improved over plan with real `interruption_channel_for_surface` SSOT helper | ✅ pure fn done |
| T11 rider prompt | reason-over-solutions + spec/model split | `chat_socrates_meta.rs:90-97` — matches plan verbatim | ✅ done |
| T12a/b withheld question | payload + persist | `WithheldQuestion` + `questioning_policy_metric_payload(.. withheld)` writes `withheld_question` into the DB metric payload | ✅ persisted (UI consumer pending) |

### Reality gaps found in review

1. **[FIXED 2026-06-19, commit `4a3c77650c`] Calibrator aggregated phantom strings.** `aggregate_counts` matched `"Accepted"`/`"Answered"` (neither is a real `ApprovalOutcome` variant — the real ones are `Approved`/`Rejected`/`Modified`/`AutoApproved`/`TimedOut`) and looked for the suppressed `PolicyDeferred`/`PolicyProceedAuto` values in the `outcome` slot when they actually live in the `event_type` column. Against real `attention_events` rows, `accepted` was **always 0** and `suppressed` **never matched** — the calibrator was a silent no-op. The 6 green tests passed only because they fed fabricated strings. Fixed to take `(channel, outcome, event_type)` and match the true PascalCase variants; added a regression test for the phantom strings.

2. **[OPEN — the real ceiling gap] The learn loop is closed only as pure functions.** Nothing in production calls `aggregate_counts` / `apply_learned_offsets`: there is no periodic job that reads recent `attention_events`, aggregates, and writes the result into the running `OrchestratorConfig.interruption_calibration`. Audit #4 is therefore **not yet closed in the running system** — only in unit-test space. This matches the plan's explicit T10 carve-out ("hot-path config mutation … out of scope for this atomic task"), but the Antigravity handoff overstated it as "closed the loop." **To reach the ceiling, a follow-up task must wire the hot path** (read events on an interval → aggregate → `apply_learned_offsets` → write under the config lock). Recommended as Track D Phase F.

3. **[MINOR] `ChatClarification` collapses into `inline_assist_gain_offset_bits`** — chat and inline-assist cannot be calibrated independently because `InterruptionCalibrationConfig` has no chat-specific offset field. Acceptable for v1; add a field if they diverge.

4. **[HYGIENE] One stray uncommitted whitespace-only edit** to `interruption_policy.rs` and substantial unrelated working-tree churn on the branch at review time. Not a Track D defect; flag for branch hygiene before merge.

### Expectation ceiling

Track D's user-facing goals are **met**: the attention budget is visible in the GUI (real focus depth via the live snapshot), the rider now reasons over the solution space, withheld questions are persisted for opt-in, and the de-split-brain fixes landed. The **one substantive gap to the ceiling** is item #2 — making the calibration loop actually adapt the running config (Phase F below), plus a small UI consumer for the persisted `withheld_question` payload (item under T12).

---

## Phase F — Close the calibration loop end-to-end (Claude Sonnet 4.6 edition)

> **EXECUTION TARGET:** Phase F is implemented by **Claude Sonnet 4.6** (not Antigravity). The same discipline applies — TDD, atomic green commits, verify-before-use before referencing any symbol — but you may use higher-level judgement where the plan says so. Use `superpowers:test-driven-development` per task and `superpowers:verification-before-completion` before each commit.

**Why:** Phase D landed the calibrator as pure functions but **nothing calls them at runtime**, and even if it did, the MCP chat path reads a **stale clone** of the config (`ServerState.orchestrator_config`), so learned offsets would never reach the live ask-decision. Phase F (F1) aggregates from the in-memory event ring with a type-safe matcher, (F2) provides the pure recalibration step, (F3) runs it on an interval and writes the running `Orchestrator.config`, (F4) makes the MCP path read the **live** calibration so offsets actually take effect, and (F5) cleans up the tree and ledger. This genuinely closes audit #4.

**Grounded facts (verified 2026-06-19):**
- In-memory ring accessor: `BudgetManager::attention_events_snapshot(limit: usize) -> Vec<AttentionEvent>` (`budget/mod.rs:579`; ring capped at 100, newest first).
- `Orchestrator` holds `config: Arc<RwLock<OrchestratorConfig>>` and `budget_manager: Arc<RwLock<BudgetManager>>` (`orchestrator.rs:54-60`) — config IS runtime-mutable under a write lock.
- `OrchestratorConfig.interruption_calibration: InterruptionCalibrationConfig` (`config/orchestrator_fields.rs:426`).
- MCP reads a **clone**: `ServerState.orchestrator_config: OrchestratorConfig` (`server_state.rs:31`), used at `attention_policy.rs:227` `apply_calibration(signals, &state.orchestrator_config)`. `ServerState` also holds `orchestrator: Arc<Orchestrator>` (live).
- `AttentionEvent` fields: `channel: Option<String>`, `event_type: AttentionEventType`, `outcome: ApprovalOutcome` (`attention/budget.rs:183`).
- Background-service pattern to mirror: `services/flywheel.rs:36` (`tokio::time::interval(...)` + `tokio::spawn(async move { loop { tick.tick().await; ... } })`, cloning an `Arc<Orchestrator>`).

### File Structure (Phase F)

| File | Responsibility | Action | Task |
|---|---|---|---|
| `crates/vox-orchestrator/src/attention/calibrator.rs` | type-safe `aggregate_events` + min-sample guard + `recalibrate` | Modify | F1, F2 |
| `crates/vox-orchestrator/src/services/attention_calibration.rs` | interval job: ring → recalibrate → write config | Create | F3 |
| `crates/vox-orchestrator/src/services/mod.rs` | register service module | Modify | F3 |
| (service-start site) | spawn the job | Modify | F3 |
| `crates/vox-orchestrator-mcp/src/attention_policy.rs` | read **live** calibration, not the stale clone | Modify | F4 |
| `crates/vox-orchestrator/src/orchestrator/accessors.rs` | `interruption_calibration()` live accessor | Modify | F4 |
| `docs/superpowers/plans/2026-06-18-track-d-attention-aware-questioning.md` | flip ledger items to closed | Modify | F5 |

---

### Task F1 `[SEQUENTIAL]`: Type-safe event aggregation + min-sample guard

Aggregate directly from `AttentionEvent` enums (compiler-checked — structurally cannot regress to the phantom-string bug), and stop trusting noisy tiny samples.

**Files:** Modify `crates/vox-orchestrator/src/attention/calibrator.rs`.

- [ ] **Step 1 (verify-before-use):** `rg -n "pub fn attention_events_snapshot|pub struct AttentionEvent|enum ApprovalOutcome|enum AttentionEventType|pub enum ApprovalTier|struct AgentId|struct TaskId" crates/vox-orchestrator/src/attention/budget.rs crates/vox-orchestrator/src/budget/mod.rs crates/vox-orchestrator/src/types*`. Confirm `AttentionEvent.channel: Option<String>`, `.outcome: ApprovalOutcome`, `.event_type: AttentionEventType`, and note how to construct `AgentId`/`ApprovalTier` for the test.

- [ ] **Step 2: Write the failing tests.** Append to the `tests` module in `calibrator.rs`:

```rust
use crate::attention::budget::{ApprovalOutcome, ApprovalTier, AttentionEvent, AttentionEventType};
use crate::types::AgentId;

fn ev(channel: &str, outcome: ApprovalOutcome, event_type: AttentionEventType) -> AttentionEvent {
    AttentionEvent {
        agent_id: AgentId(0),
        task_id: None,
        event_type,
        tier: ApprovalTier::Trusted, // confirm a real variant in Step 1
        cost_ms: 0,
        outcome,
        trust_score_at_time: 0.5,
        effective_complexity: 0.0,
        decision_entropy_bits: 0.0,
        timestamp_ms: 0,
        channel: Some(channel.to_string()),
        policy_reason: None,
    }
}

#[test]
fn aggregate_events_matches_enums_directly() {
    let events = vec![
        ev("vox_inline_edit", ApprovalOutcome::Approved, AttentionEventType::CommandApproval),
        ev("vox_inline_edit", ApprovalOutcome::Modified, AttentionEventType::CodeReview),
        ev("vox_inline_edit", ApprovalOutcome::Rejected, AttentionEventType::CommandApproval),
        ev("vox_inline_edit", ApprovalOutcome::AutoApproved, AttentionEventType::PolicyDeferred),
    ];
    let m = aggregate_events(&events);
    assert_eq!(m["vox_inline_edit"], ChannelOutcomeCounts { accepted: 2, rejected: 1, suppressed: 1 });
}

#[test]
fn small_samples_yield_neutral_offset() {
    // Below the minimum sample count, do not act on noise.
    let c = ChannelOutcomeCounts { accepted: 0, rejected: 3, suppressed: 0 };
    assert_eq!(channel_gain_offset(c), 0.0);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator --lib calibrator` → FAIL (`aggregate_events` missing; `small_samples` fails because the guard isn't there yet).

- [ ] **Step 4: Implement.** Add the min-sample guard to `channel_gain_offset` (top of the fn, after computing `shown`):

```rust
    const MIN_SAMPLES: u32 = 5;
    let shown = c.accepted + c.rejected;
    if shown < MIN_SAMPLES {
        return 0.0;
    }
```

(Replace the existing `let shown = ...; if shown == 0 { return 0.0; }` lines with the above — `MIN_SAMPLES` subsumes the zero case.) Then add the type-safe aggregator near `aggregate_counts`:

```rust
use crate::attention::budget::{ApprovalOutcome, AttentionEvent, AttentionEventType};

/// Type-safe aggregation from the in-memory event ring. Matches enum variants directly, so it
/// cannot regress to phantom-string matching (cf. the 2026-06-19 `aggregate_counts` fix). This is
/// the path the live calibration job uses.
#[must_use]
pub fn aggregate_events(
    events: &[AttentionEvent],
) -> std::collections::HashMap<String, ChannelOutcomeCounts> {
    let mut map: std::collections::HashMap<String, ChannelOutcomeCounts> =
        std::collections::HashMap::new();
    for ev in events {
        let key = ev.channel.clone().unwrap_or_else(|| "unknown".to_string());
        let e = map.entry(key).or_default();
        match ev.outcome {
            ApprovalOutcome::Approved | ApprovalOutcome::Modified => e.accepted += 1,
            ApprovalOutcome::Rejected => e.rejected += 1,
            _ => {}
        }
        if matches!(
            ev.event_type,
            AttentionEventType::PolicyDeferred | AttentionEventType::PolicyProceedAuto
        ) {
            e.suppressed += 1;
        }
    }
    map
}
```

- [ ] **Step 5: Run → PASS.** `cargo test -p vox-orchestrator --lib calibrator` → all PASS (including the pre-existing offset tests — confirm `high_reject_rate` uses shown=10 and `mostly_accepted` shown=20, both ≥ MIN_SAMPLES, so they still pass).

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator --lib -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/attention/calibrator.rs
git commit -m "feat(attention): type-safe aggregate_events + min-sample guard (Phase F1)"
```

---

### Task F2 `[SEQUENTIAL]`: Pure recalibration step

One pure function the interval job calls — easy to test, no I/O.

**Files:** Modify `crates/vox-orchestrator/src/attention/calibrator.rs`.

- [ ] **Step 1: Write the failing test.** Append to `close_loop_tests`:

```rust
#[test]
fn recalibrate_lowers_gain_for_a_wasteful_channel() {
    let events: Vec<AttentionEvent> = (0..10)
        .map(|i| {
            let outcome = if i == 0 { ApprovalOutcome::Approved } else { ApprovalOutcome::Rejected };
            ev("vox_inline_edit", outcome, AttentionEventType::CommandApproval)
        })
        .collect();
    let cfg = recalibrate(InterruptionCalibrationConfig::default(), &events);
    assert!(cfg.inline_assist_gain_offset_bits < 0.0, "9/10 rejected ⇒ ask less on this channel");
}
```

> Reuse the `ev(..)` helper added in F1 (same test file). If F1's helper is in `mod tests` and this test is in `mod close_loop_tests`, either move `ev` to the parent module or duplicate it locally — your call; keep it DRY if cheap.

- [ ] **Step 2: Run → FAIL.** `cargo test -p vox-orchestrator --lib recalibrate_lowers_gain` → FAIL (`recalibrate` missing).

- [ ] **Step 3: Implement.** Add near `apply_learned_offsets`:

```rust
/// Pure recalibration step: aggregate recent events and fold the learned per-channel offsets into
/// `base`. This is the body the interval job runs each tick.
#[must_use]
pub fn recalibrate(
    base: InterruptionCalibrationConfig,
    events: &[AttentionEvent],
) -> InterruptionCalibrationConfig {
    apply_learned_offsets(base, &aggregate_events(events))
}
```

- [ ] **Step 4: Run → PASS.** `cargo test -p vox-orchestrator --lib recalibrate` → PASS.

- [ ] **Step 5: Verify + commit.** `cargo clippy -p vox-orchestrator --lib -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/attention/calibrator.rs
git commit -m "feat(attention): pure recalibrate() step folding learned offsets into config (Phase F2)"
```

---

### Task F3 `[SEQUENTIAL]`: Interval job — recalibrate the running config

Mirror `services/flywheel.rs`. Read the ring, recalibrate, write `Orchestrator.config`.

**Files:** Create `crates/vox-orchestrator/src/services/attention_calibration.rs`; Modify `crates/vox-orchestrator/src/services/mod.rs`; Modify the service-start site.

- [ ] **Step 1 (verify-before-use):** `rg -n "fn spawn|tokio::time::interval|tick.tick\(\)" crates/vox-orchestrator/src/services/flywheel.rs` (mirror its shape). `rg -n "D_300S" crates/vox-config/src/timeouts.rs` (confirm the 5-minute constant exists; if not, use `tokio::time::Duration::from_secs(300)`). `rg -n "flywheel|services::|\.spawn\(\)|Flywheel" crates/vox-orchestrator/src` to find WHERE services are actually spawned at startup — note that file:line; you will add one spawn call there. `rg -n "pub fn attention_events_snapshot" crates/vox-orchestrator/src/budget/mod.rs` (confirm the accessor).

- [ ] **Step 2: Create the job.** `services/attention_calibration.rs`:

```rust
//! Background calibration: every few minutes, aggregate recent attention events and update the
//! running interruption-calibration config so the ask-threshold adapts to which surfaces the pilot
//! actually engages vs. rejects. Closes the Phase-D learn loop (audit #4) at runtime.

use std::sync::Arc;

use crate::Orchestrator;

/// Minimum events in the ring before we bother recalibrating (avoid acting on cold-start noise).
const MIN_EVENTS_TO_CALIBRATE: usize = 10;

/// Spawn the periodic attention-calibration job. Ticks every 5 minutes.
pub fn spawn_attention_calibration(orch: Arc<Orchestrator>) {
    let mut tick = tokio::time::interval(tokio::time::Duration::from_secs(300));
    tokio::spawn(async move {
        loop {
            tick.tick().await;
            // Snapshot the recent in-memory event ring (newest 100).
            let events = {
                let bm = crate::sync_lock::rw_read(&orch.budget_manager);
                bm.attention_events_snapshot(100)
            };
            if events.len() < MIN_EVENTS_TO_CALIBRATE {
                continue;
            }
            // Recalibrate from the current base and write it back under the config write lock.
            let base = {
                let cfg = crate::sync_lock::rw_read(&orch.config);
                cfg.interruption_calibration.clone()
            };
            let updated = crate::attention::calibrator::recalibrate(base, &events);
            {
                let mut cfg = crate::sync_lock::rw_write(&orch.config);
                cfg.interruption_calibration = updated;
            }
            tracing::debug!(
                target: "vox_attention_calibration",
                events = events.len(),
                "recalibrated interruption offsets from attention events"
            );
        }
    });
}
```

> Confirm the lock helpers `crate::sync_lock::rw_read` / `rw_write` exist (used throughout, e.g. `accessors.rs:15`). If `calibrator` is not re-exported at `crate::attention::calibrator`, check `attention/mod.rs` and adjust the path (T9 registered `pub mod calibrator;`).

- [ ] **Step 3: Register the module.** In `crates/vox-orchestrator/src/services/mod.rs` add (next to `pub mod flywheel;`):

```rust
pub mod attention_calibration;
```

- [ ] **Step 4: Spawn it at startup.** At the service-start site found in Step 1 (where `flywheel` / other services are spawned), add — using the same `Arc<Orchestrator>` handle that site already has:

```rust
crate::services::attention_calibration::spawn_attention_calibration(orch.clone());
```

> Match the real variable name for the orchestrator Arc at that site. If services are gated behind a config flag, place this spawn alongside the others under the same gate.

- [ ] **Step 5: Compile.** `cargo check -p vox-orchestrator` → compiles. `cargo run -p vox-arch-check` → passes (new module).

- [ ] **Step 6: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo fmt -p vox-orchestrator`; then:

```bash
git add crates/vox-orchestrator/src/services/attention_calibration.rs crates/vox-orchestrator/src/services/mod.rs
# plus the modified service-start file:
git add -u
git commit -m "feat(attention): interval job recalibrates running interruption config (Phase F3)"
```

---

### Task F4 `[SEQUENTIAL]`: Make the MCP ask-decision read the LIVE calibration (the actual loop closure)

Right now the chat path reads `ServerState.orchestrator_config` — a clone frozen at startup — so F3's updates never reach it. Read the live config from the orchestrator instead.

**Files:** Modify `crates/vox-orchestrator/src/orchestrator/accessors.rs` (add accessor); Modify `crates/vox-orchestrator-mcp/src/attention_policy.rs` (use it).

- [ ] **Step 1 (verify-before-use):** `rg -n "fn apply_calibration|state.orchestrator_config|let cal = &cfg.interruption_calibration" crates/vox-orchestrator-mcp/src/attention_policy.rs`. Read `apply_calibration` (it does `let cal = &cfg.interruption_calibration;`). Confirm `ServerState` has `orchestrator: Arc<Orchestrator>` (`rg -n "orchestrator:" crates/vox-orchestrator-mcp/src/server_state.rs`).

- [ ] **Step 2: Write the failing test.** Add to `accessors.rs` test module — assert a live accessor reflects a config mutation:

```rust
#[test]
fn interruption_calibration_accessor_reflects_live_config() {
    let orch = crate::Orchestrator::default(); // use the real test constructor confirmed earlier
    {
        let mut cfg = crate::sync_lock::rw_write(&orch.config);
        cfg.interruption_calibration.inline_assist_gain_offset_bits = -0.07;
    }
    assert_eq!(orch.interruption_calibration().inline_assist_gain_offset_bits, -0.07);
}
```

- [ ] **Step 3: Run → FAIL.** `cargo test -p vox-orchestrator interruption_calibration_accessor_reflects_live_config` → FAIL (accessor missing).

- [ ] **Step 4: Add the live accessor.** In `accessors.rs`:

```rust
    /// Live snapshot of the interruption-calibration config (clone under read lock). Used by the
    /// MCP ask-decision path so background recalibration (Phase F3) actually takes effect.
    #[must_use]
    pub fn interruption_calibration(&self) -> crate::attention::InterruptionCalibrationConfig {
        crate::sync_lock::rw_read(&self.config).interruption_calibration.clone()
    }
```

- [ ] **Step 5: Use it in the MCP path.** In `attention_policy.rs`, change `apply_calibration` to take the calibration directly, and read it live in `evaluate_with_state`:

```rust
// signature change:
fn apply_calibration(
    mut signals: InterruptionSignals,
    cal: &vox_orchestrator::attention::InterruptionCalibrationConfig,
) -> InterruptionSignals {
    // body: replace `let cal = &cfg.interruption_calibration;` with the `cal` parameter directly.
    ...
}

// caller (evaluate_with_state):
let cal = state.orchestrator.interruption_calibration();
let calibrated = apply_calibration(signals.clone(), &cal);
```

> Leave the `attention_enabled` / `attention_alert_threshold` reads on `state.orchestrator_config` as-is (those are not mutated at runtime). Only the calibration must be live.

- [ ] **Step 6: Run → PASS.** `cargo test -p vox-orchestrator interruption_calibration_accessor` → PASS; `cargo test -p vox-orchestrator-mcp attention` → PASS; `cargo check -p vox-orchestrator-mcp` → compiles.

- [ ] **Step 7: Verify + commit.** `cargo clippy -p vox-orchestrator -- -D warnings`; `cargo clippy -p vox-orchestrator-mcp -- -D warnings`; `cargo fmt -p vox-orchestrator`; `cargo fmt -p vox-orchestrator-mcp`; then:

```bash
git add crates/vox-orchestrator/src/orchestrator/accessors.rs crates/vox-orchestrator-mcp/src/attention_policy.rs
git commit -m "feat(attention): MCP reads live interruption calibration — loop closed end-to-end (Phase F4, audit #4)"
```

---

### Task F5 `[SEQUENTIAL]`: Cleanup + ledger close-out

**Files:** working tree; this plan's ledger.

- [ ] **Step 1: Resolve the stray edit.** `git status --short crates/vox-orchestrator/src/attention/interruption_policy.rs`. If it shows a whitespace-only modification, normalize and commit it: `cargo fmt -p vox-orchestrator`, then `git add crates/vox-orchestrator/src/attention/interruption_policy.rs && git commit -m "style(attention): fmt interruption_policy test (cleanup)"`. If `git diff` shows it is purely a re-wrap with no committed counterpart, this commit is correct; if it is already identical to HEAD, skip.

- [ ] **Step 2: Full verification.** Run and paste output:
  - `cargo test -p vox-orchestrator` → all PASS
  - `cargo test -p vox-orchestrator-mcp` → all PASS
  - `cargo clippy -p vox-orchestrator --lib -- -D warnings` and `cargo clippy -p vox-orchestrator-mcp -- -D warnings` → clean
  - `cargo run -p vox-arch-check` → passes

- [ ] **Step 3: Flip the ledger.** In the *Reality gaps found in review* section above, change item #2's status from `[OPEN — the real ceiling gap]` to `[CLOSED 2026-MM-DD, Phase F]` with a one-line note that the interval job + live MCP read now adapt the running config. Update the F-task checkboxes to `[x]`.

- [ ] **Step 4: Commit the ledger.**

```bash
git add docs/superpowers/plans/2026-06-18-track-d-attention-aware-questioning.md
git commit -m "docs(track-d): close audit #4 — calibration loop live (Phase F complete)"
```

> **Note on `withheld_question` UI consumer (deliberately out of scope):** the persisted payload (T12) has no GUI reader yet. That is a *frontend* task (render an inline "ask anyway?" affordance from `socrates.questioning.withheld_question`), not part of closing the calibration loop. Leave it for a separate small UI plan unless asked — YAGNI for Phase F.

### Self-Review (Phase F)

- **Coverage:** ceiling gap #2 → F1–F4 (aggregate from real events, recalibrate, run on interval, read live); cleanup → F5. ✅
- **Type consistency:** `aggregate_events`/`recalibrate`/`apply_learned_offsets`/`channel_gain_offset`/`ChannelOutcomeCounts`/`InterruptionCalibrationConfig`/`interruption_calibration()` names are consistent across F1–F4. ✅
- **No placeholders:** every code step shows real code; the one judgement call (service-start site) has a verify-before-use locating it. ✅
- **Correctness guard:** F4 is the load-bearing step — without it F3 updates a config the chat path never reads. Both are required to truly close the loop. ✅
