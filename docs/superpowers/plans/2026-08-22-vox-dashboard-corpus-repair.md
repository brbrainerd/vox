# vox-dashboard Corpus Repair Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop the corpus telling readers and agents that `crates/vox-dashboard` — a crate deleted 2026-05-12 (`af5f26278`) — is the canonical Vox GUI, when `contracts/frontend/surface-ownership.v1.yaml` has already been corrected to name `crates/vox-gui`.

**Architecture:** 273 live occurrences across 23 files split cleanly by what each file needs: two "single source of truth" reference docs get rewritten to the corrected contract; one audit doc's own self-contradiction gets fixed; four ADRs that made `vox-dashboard` a load-bearing decision get an in-place supersession note pointing at ADR-045 (ADRs are append-only records — never rewritten); the remaining files, which concentrate 229 of the 273 occurrences in two historical planning documents, get a `status`/banner flip rather than a line-by-line rewrite, because their content is a record of what happened, not a live instruction.

**Tech Stack:** Markdown, YAML frontmatter.

**Spec:** `docs/superpowers/specs/2026-08-22-docs-corpus-repair-design.md` (revision 3), workstream W2 (W2.2 specifically).

## Global Constraints

- **Prerequisite already satisfied, verify don't re-fix:** `contracts/frontend/surface-ownership.v1.yaml` already has `vox-gui: status: canonical` and `vox-dashboard: status: retired, path: crates/vox-gui`. Task 1 Step 1 confirms this before any edit — do not modify that contract file in this plan.
- **ADRs are never rewritten, only annotated.** A superseded decision gets a note pointing to what superseded it, in place, at the top of the relevant section. Do not delete or alter the original decision text — it is a historical record of what was decided and why, which is exactly what makes an ADR useful later.
- **Do not touch `docs/src/archive/**`.**
- **Retired-symbol-check dependency:** the W3 severity-valve plan (`docs/superpowers/plans/2026-08-22-retired-symbol-severity-valve.md`) is a **soft** prerequisite, not a hard one — this plan's edits reduce the reference count, which only helps a future `vox-dashboard` contract entry, and this plan does not itself add any contract entry. Land in either order.
- **HARD prerequisite — ADR renumbering must land first.** Task 5 of
  `docs/superpowers/plans/2026-08-22-gate-and-policy-honesty.md` renumbers
  `docs/src/adr/045-tauri-gui-replaces-axum-dashboard.md` to
  **`045-tauri-gui-replaces-axum-dashboard.md`**, because three files currently
  share the number 037. This plan adds roughly ten new links and prose
  citations to that exact file. **Every one of them must say 045, not 037**, or
  they point at a path that will not exist. Before starting, confirm the
  renumber has landed:

  ```bash
  ls docs/src/adr/045-tauri-gui-replaces-axum-dashboard.md
  ```

  If that file does not exist yet, **stop and land the gate-and-policy-honesty
  plan's Task 5 first.** If for some reason this plan must go first instead,
  cite `045-tauri-gui-replaces-axum-dashboard.md` throughout and tell whoever
  runs Task 5 to sweep this plan's citations as part of its Step 7 prose sweep
  — but the listed order (renumber first) is strongly preferred, because Task 5
  already has to sweep prose citations and adding more for it to find is
  gratuitous.
- **`037-tauri-convergence.md` keeps the number 037** and is a *different*
  decision (Tauri convergence, not GUI-replaces-dashboard). Do not conflate
  them: the file this plan cites is the one being renumbered to 045.
- **Verification tier:** `--full`, not `--complete`.
- **Line endings LF** for `md`.
- **No checker enters this plan until it has been run against the real tree and its actual output pasted into the step.**
- **One agent per worktree.**

---

## File Structure

| File | What changes | Task |
| --- | --- | --- |
| `docs/src/reference/frontend-surface-ownership.md` | Rewrite `crates/vox-dashboard` → `crates/vox-gui` at all 4 sites; the "new dashboard panel" guidance is corrected to name the current surface | 1 |
| `docs/src/reference/vox-web-stack.md` | Rewrite the "single source of truth" block and the entry-point paths to `crates/vox-gui` | 1 |
| `docs/src/architecture/vox-gui-capability-audit-2026.md` | Fix the one row that describes the contract drift as current — the drift it describes no longer exists | 2 |
| `docs/src/adr/024-dashboard-axum-spa.md`, `010-tanstack-web-spine.md`, `030-state-machine-ssot.md`, `031-deprecate-vox-vscode.md` | Add a supersession note pointing at ADR-045; text of the original decision is untouched | 3 |
| `docs/src/architecture/mesh-phase4-dashboard-control-plan-2026.md`, `vox-gui-native-roadmap-2026.md` | Frontmatter `status` flip + a one-line banner; the 205 occurrences inside (129 + 76) are task-target file paths in a historical plan and are left as-is | 4 |

Not in this plan: the remaining ~13 files each carry 1-10 occurrences in
research/roadmap docs whose own frontmatter already marks them non-current, or
are ADRs already `status: deprecated` (`027-dual-track-ui-surfaces.md`). Spot-check
list, not a task: `mesh-mens-distributed-training-and-execution-plan-2026.md`,
`agentic-vcs-automation-impl-plan-phase{1,3}-2026.md`,
`mesh-dashboard-and-distributed-compute-research-2026.md`,
`mesh-and-language-distribution-ssot-2026.md` (this one is `status: current` —
verify in Task 4's sweep and fold in if so),
`agentic-version-control-automation-research-2026.md`,
`unified-task-hopper-research-2026.md`, `gui-authoring-syntax-2026.md`,
`vox-speech-surface-inventory-2026.md`, `vox-speech-audit-findings-2026.md`,
`gui-native-roadmap-status-2026.md`, `dashboard-migration-research-2026.md`.

---

### Task 1: Rewrite the two "single source of truth" reference docs

These two files are `status: current` and explicitly present themselves as
canonical guidance — unlike the ADRs (historical decisions) or the plans
(point-in-time records), these are live reference pages a contributor reads
today to learn where to build a GUI feature.

**Files:**
- Modify: `docs/src/reference/frontend-surface-ownership.md` (lines 18, 28, 36, 44)
- Modify: `docs/src/reference/vox-web-stack.md` (lines 33-41)

**Interfaces:** none.

- [ ] **Step 1: Confirm the contract is already fixed — do not re-fix it**

```bash
grep -n -A2 '^  - id: vox-gui$\|^  - id: vox-dashboard$' contracts/frontend/surface-ownership.v1.yaml
```

Expected: `vox-gui` shows `status: canonical`, `path: crates/vox-gui`;
`vox-dashboard` shows `status: retired`, `path: crates/vox-gui`. If this is not
the case, stop — the prerequisite this plan assumes is not actually met, and
that contract must be fixed first, in a separate change.

- [ ] **Step 2: Rewrite `frontend-surface-ownership.md`**

Read the file first to confirm line numbers haven't drifted:

```bash
grep -n 'vox-dashboard' docs/src/reference/frontend-surface-ownership.md
```

Line 18 currently reads:
> **Surface class:** a new dashboard panel is **`canonical`** — implement under `crates/vox-dashboard` first; only then mirror stubs into `apps/interop/marquee_app` if interop needs proving.

Replace `crates/vox-dashboard` with `crates/vox-gui`.

Line 28, in a table row:
```
| `crates/vox-dashboard` | canonical | Primary Vox user-facing GUI and orchestration UX | New product UX lands here first |
```
Replace with:
```
| `crates/vox-gui` | canonical | Primary Vox user-facing GUI and orchestration UX (Tauri 2; superseded the Axum `vox-dashboard` per ADR-045) | New product UX lands here first |
```

Line 36:
> - **Necessary:** `vox-dashboard` and one external interop exemplar (`apps/interop/marquee_app`) to validate "Vox backend + React frontend" workflows.

Replace `vox-dashboard` with `vox-gui`.

Line 44:
> - Canonical UX changes require updates in `crates/vox-dashboard` first.

Replace `crates/vox-dashboard` with `crates/vox-gui`.

- [ ] **Step 3: Rewrite `vox-web-stack.md`**

```bash
grep -n 'vox-dashboard\|vox dashboard' docs/src/reference/vox-web-stack.md
```

Line 33-34 currently reads:
> **`vox-dashboard` is the Single Source of Truth** for the Vox user-facing frontend experience (see [ADR 030](../adr/030-state-machine-ssot.md) and [ADR 031](../adr/031-deprecate-vox-vscode.md)).
> `apps/editor/vox-vscode/` is **deprecated** and retained only for its LSP client. Ship new MCP behavior, capability UX, and visualization in `crates/vox-dashboard/` — not in the VS Code extension.

Replace with:
> **`vox-gui` is the Single Source of Truth** for the Vox user-facing frontend experience (superseding the Axum `vox-dashboard` per [ADR-045](../adr/045-tauri-gui-replaces-axum-dashboard.md); see also [ADR 030](../adr/030-state-machine-ssot.md) and [ADR 031](../adr/031-deprecate-vox-vscode.md), both written when `vox-dashboard` was current).
> `apps/editor/vox-vscode/` is **deprecated** and retained only for its LSP client. Ship new MCP behavior, capability UX, and visualization in `crates/vox-gui/` — not in the VS Code extension.

Line 36:
> The **orchestration dashboard** (`crates/vox-dashboard/`) is the primary Vox user surface. It is served by the Axum backend (`vox dashboard` command) and communicates with the orchestrator over a local MCP WebSocket proxy. All reactive UI state within the dashboard uses the Vox `state_machine` compiler primitive as the single source of truth (see below).

Replace with:
> The **orchestration GUI** (`crates/vox-gui/`) is the primary Vox user surface. It is a native Tauri 2 application (`vox gui` command) and communicates with the orchestrator over Tauri IPC. All reactive UI state uses the Vox `state_machine` compiler primitive as the single source of truth (see below) — this predates and survives the ADR-045 migration from the earlier Axum `vox-dashboard`.

Lines 40-41:
> - **Dashboard entry point:** `crates/vox-dashboard/app/src/app.vox` — lowered to `app/src/generated/` by `vox build`
> - **Backend:** `crates/vox-dashboard/src/` — Axum routes, MCP proxy, settings API

Replace with:
```
- **GUI entry point:** `crates/vox-gui/ui/src/App.tsx`
- **Backend:** `crates/vox-gui/src/commands/` — Tauri IPC handlers
```

Do not invent the exact `vox-gui` paths beyond what's given here — if the
entry-point path above does not exist, run
`ls crates/vox-gui/ui/src/App.tsx crates/vox-gui/src/commands/` to confirm
before committing, and correct to the real path if it has moved.

- [ ] **Step 4: Verify no `vox-dashboard` reference remains in either file**

```bash
grep -n 'vox-dashboard' docs/src/reference/frontend-surface-ownership.md docs/src/reference/vox-web-stack.md
```

Expected: no output.

- [ ] **Step 5: Lint and check links**

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths reference/frontend-surface-ownership.md
cargo run -p vox-doc-pipeline -- --lint-only --paths reference/vox-web-stack.md
cargo run -q -p vox-cli -- ci check-links
```

Expected: all clean. The ADR-045 link added in Step 3 must resolve —
`check-links` is what proves it.

- [ ] **Step 6: Commit**

```bash
git add docs/src/reference/frontend-surface-ownership.md docs/src/reference/vox-web-stack.md
git commit -m "fix(docs): frontend-surface-ownership and vox-web-stack point at the deleted vox-dashboard crate"
```

---

### Task 2: Fix the self-contradicting capability audit

`vox-gui-capability-audit-2026.md` is itself one of the `evidence_sources` the
surface-ownership contract cites for the `vox-gui` entry — and it currently
contains a table row asserting the contract "names `crates/vox-dashboard` as
canonical," which was true when the audit was written and is false now that
the contract has been corrected.

**Files:**
- Modify: `docs/src/architecture/vox-gui-capability-audit-2026.md` (the "Dashboard crate naming" table row)

**Interfaces:** none.

- [ ] **Step 1: Confirm the current text**

```bash
grep -n -B2 -A2 'Dashboard crate naming' docs/src/architecture/vox-gui-capability-audit-2026.md
```

Expected to match:
```
| Dashboard crate naming | `contracts/frontend/surface-ownership.v1.yaml` names `crates/vox-dashboard` as canonical, while the actual live shell is `crates/vox-gui`. | Documentation and ownership drift make future work harder to route. |
```

- [ ] **Step 2: Replace the row**

Replace with:
```
| Dashboard crate naming | **Fixed.** `contracts/frontend/surface-ownership.v1.yaml` now names `crates/vox-gui` as canonical and `crates/vox-dashboard` as `status: retired`. This audit originally found the contract pointing at the deleted crate; that has since been corrected. | None — resolved. |
```

Do not delete the row. The audit's value is as a record of what was found;
marking it resolved in place preserves that record while stopping it from
misleading a reader into re-fixing an already-fixed contract.

- [ ] **Step 3: Confirm no other row in this file references `vox-dashboard`**

```bash
grep -n 'vox-dashboard' docs/src/architecture/vox-gui-capability-audit-2026.md
```

If any other hit appears, read it in context before deciding whether it also
needs the "Fixed" treatment or is a legitimate historical mention (e.g.
describing what the audit found, not asserting current state) — do not
blanket-replace.

- [ ] **Step 4: Lint**

```bash
cargo run -p vox-doc-pipeline -- --lint-only --paths architecture/vox-gui-capability-audit-2026.md
```

- [ ] **Step 5: Commit**

```bash
git add docs/src/architecture/vox-gui-capability-audit-2026.md
git commit -m "fix(docs): vox-gui-capability-audit's own dashboard-naming finding is now resolved"
```

---

### Task 3: Add a supersession note to the four load-bearing ADRs

Each of these four ADRs made `vox-dashboard` part of a formal decision. Per
Global Constraints, the original decision text is never rewritten — only
annotated with what superseded it.

**Files:**
- Modify: `docs/src/adr/024-dashboard-axum-spa.md` (near line 25)
- Modify: `docs/src/adr/010-tanstack-web-spine.md` (near line 16)
- Modify: `docs/src/adr/030-state-machine-ssot.md` (near line 22)
- Modify: `docs/src/adr/031-deprecate-vox-vscode.md` (near line 25-26)

**Interfaces:** none. All four edits are the same shape — do this as one
batch, one commit, per the standard practice of not spinning up separate
review surfaces for four identical-shape edits.

**Two details confirmed in review, before you start:**

1. `024` and `010` already have a blank line between the closing frontmatter
   `---` and their H1; `030` and `031` do **not**. Add blank lines around the
   inserted note yourself in those two, or the blockquote will collide with
   the heading. No lint rule forbids content between frontmatter and the first
   heading (checked `lint.rs` — only `lint_duplicate_frontmatter` looks there,
   and a blockquote does not trigger it), so insertion itself is safe.
2. **The 037 collision is resolved before this plan runs.** Three ADR files
   used to share the number 037; the sibling gate-and-policy-honesty plan's
   Task 5 renumbers two of them, so the file this plan cites becomes
   `045-tauri-gui-replaces-axum-dashboard.md` and only
   `037-tauri-convergence.md` keeps 037. Cite **045** everywhere here. Write
   the prose as "per [ADR-045 (Tauri GUI replaces the Axum dashboard)](045-tauri-gui-replaces-axum-dashboard.md)"
   so the reader cannot confuse it with the convergence ADR.

- [ ] **Step 1: Add the note to `024-dashboard-axum-spa.md`**

This ADR's decision (line 25: `crates/vox-dashboard` is the canonical home for
the orchestration UI) is the ADR that ADR-045 directly supersedes. Add
immediately after the frontmatter, before the first heading:

```markdown
> **Superseded by [ADR-045](045-tauri-gui-replaces-axum-dashboard.md).** The
> Axum-served `crates/vox-dashboard` decided here was replaced by the Tauri 2
> `crates/vox-gui` shell; `vox-dashboard` was deleted 2026-05-12 (`af5f26278`).
> This ADR's reasoning for a standalone crate is preserved below as the
> historical record of that decision.
```

- [ ] **Step 2: Add the note to `010-tanstack-web-spine.md`**

This ADR's scope clarification (line 16) explicitly carves `vox-dashboard` out
of its own scope. Add immediately after the frontmatter:

```markdown
> **Note (2026-08):** the "Scope clarification" below refers to
> `crates/vox-dashboard`, which no longer exists — it was replaced by
> `crates/vox-gui` (Tauri 2) per [ADR-045](045-tauri-gui-replaces-axum-dashboard.md).
> This ADR's actual scope — the web stack for Vox-compiled *user* applications
> — is unaffected; only the carve-out's named example crate is stale.
```

- [ ] **Step 3: Add the note to `030-state-machine-ssot.md`**

This ADR's decision text says `state_machine` is the SSOT "for all reactive
state in `crates/vox-dashboard/`". Add immediately after the frontmatter:

```markdown
> **Note (2026-08):** this ADR was written against `crates/vox-dashboard`,
> replaced 2026-05-12 by `crates/vox-gui` per
> [ADR-045](045-tauri-gui-replaces-axum-dashboard.md). The `state_machine`
> SSOT decision below is a compiler-level primitive and applies unchanged to
> `vox-gui`'s reactive state — only the crate name in the text is stale.
```

- [ ] **Step 4: Add the note to `031-deprecate-vox-vscode.md`**

This ADR's decision explicitly names `vox-dashboard` as "the primary user
surface" and a "feature parity gate" blocking `vox-vscode`'s archival. Add
immediately after the frontmatter:

```markdown
> **Note (2026-08):** this ADR named `crates/vox-dashboard` as the
> replacement primary surface; that crate was replaced by `crates/vox-gui`
> (Tauri 2) per [ADR-045 (Tauri GUI replaces the Axum dashboard)](045-tauri-gui-replaces-axum-dashboard.md)
> before `vox-vscode`'s deprecation was acted on. The decision to deprecate
> `vox-vscode` stands; its replacement target is now `crates/vox-gui`.
>
> **Decision 4 (the feature-parity gate) is hereby restated against
> `crates/vox-gui`:** `vox-vscode` may not be archived until `vox-gui`
> achieves parity with the Phase 2 feature list. As written, Decision 4 gates
> archival on a crate that no longer exists, which would make the gate
> unsatisfiable — this note is the operative version.

This is the one place in this task where the note does more than annotate: it
**redirects a still-live gate**, because Decision 4 is a currently-operative
condition a reader would act on today, not a record of something already
decided and done. That is the line this plan draws between "annotate as
history" and "restate as operative" — tense alone does not decide it; whether
the passage still governs an action does.
```

- [ ] **Step 5: Confirm all four links resolve and nothing else broke**

```bash
cargo run -q -p vox-cli -- ci check-links
for f in 024-dashboard-axum-spa 010-tanstack-web-spine 030-state-machine-ssot 031-deprecate-vox-vscode; do
  cargo run -p vox-doc-pipeline -- --lint-only --paths adr/$f.md
done
```

Expected: all clean.

- [ ] **Step 6: Commit**

```bash
git add docs/src/adr/024-dashboard-axum-spa.md docs/src/adr/010-tanstack-web-spine.md docs/src/adr/030-state-machine-ssot.md docs/src/adr/031-deprecate-vox-vscode.md
git commit -m "docs(adr): add ADR-045 supersession notes to four vox-dashboard-era ADRs"
```

---

### Task 4: Status-flip the two large historical planning documents

`mesh-phase4-dashboard-control-plan-2026.md` (129 occurrences) and
`vox-gui-native-roadmap-2026.md` (76 occurrences) concentrate 205 of the 273
live references. Both are `status: current` today despite being records of
completed or superseded planning work whose paths were never in `vox-gui` to
begin with (they were `vox-dashboard`-era task targets) — the occurrences are
historical file-path citations, not live instructions, so a line-by-line
rewrite would falsify the historical record for no benefit. The fix is
`status`, not content.

**Files:**
- Modify: `docs/src/architecture/mesh-phase4-dashboard-control-plan-2026.md` (frontmatter `status`)
- Modify: `docs/src/architecture/vox-gui-native-roadmap-2026.md` (frontmatter `status`; plus its one live **Decision** line at :302, which is prescriptive, not historical)

**Interfaces:** none.

- [ ] **Step 1: Confirm the occurrence counts and check for any live prescriptive language**

```bash
grep -c 'crates/vox-dashboard' docs/src/architecture/mesh-phase4-dashboard-control-plan-2026.md docs/src/architecture/vox-gui-native-roadmap-2026.md
grep -n 'Decision' docs/src/architecture/vox-gui-native-roadmap-2026.md | head -10
```

- [ ] **Step 2: Flip `mesh-phase4-dashboard-control-plan-2026.md`'s frontmatter**

Change `status: "current"` (or bare `current`, whatever the file actually has —
confirm with `head -8` first) to `status: "roadmap"`, and add immediately
after the frontmatter:

```markdown
> **Historical note (2026-08):** written against `crates/vox-dashboard`
> (deleted 2026-05-12, replaced by `crates/vox-gui` per
> [ADR-045](../adr/045-tauri-gui-replaces-axum-dashboard.md)). The task-target
> file paths below are preserved as the historical planning record and are
> not live paths in the current tree.
```

- [ ] **Step 3: Flip `vox-gui-native-roadmap-2026.md`'s frontmatter — content untouched**

Same frontmatter and banner treatment as Step 2. **Do not rewrite line ~302.**

An earlier draft of this plan called that line a live `**Decision**` needing a
content fix. That was a misreading, caught in review: read it in context and it
is a *task brief instructing a contributor how to draft ADR-024* ("Files to
create: `docs/src/adr/024-dashboard-axum-spa.md` ... **Decision** —
`crates/vox-dashboard` is the canonical home ..."). That ADR now exists and
Task 3 handles it with a supersession note, deliberately leaving its decision
text intact. Rewriting the roadmap's transcription of that decision would make
the roadmap disagree with the real, unrewritten ADR-024 — a self-inflicted
inconsistency. It is historical record, same as the file's other 75
occurrences.

```bash
sed -n '295,306p' docs/src/architecture/vox-gui-native-roadmap-2026.md
```

Read it to confirm the above before deciding; if it genuinely reads as this
document's own operative decision rather than ADR-drafting instructions, apply
Task 1's rewrite pattern and note the deviation.

- [ ] **Step 4: Confirm the flip and the one content fix**

```bash
grep -n 'status:' docs/src/architecture/mesh-phase4-dashboard-control-plan-2026.md docs/src/architecture/vox-gui-native-roadmap-2026.md | head -4
cargo run -p vox-doc-pipeline -- --lint-only --paths architecture/mesh-phase4-dashboard-control-plan-2026.md
cargo run -p vox-doc-pipeline -- --lint-only --paths architecture/vox-gui-native-roadmap-2026.md
```

Expected: both show `roadmap`; both lint clean.

- [ ] **Step 5: Sweep the remaining spot-check list for the same pattern**

For each file in this plan's "Not in this plan" list above, check its current
`status`:

```bash
for f in mesh-mens-distributed-training-and-execution-plan-2026 \
         agentic-vcs-automation-impl-plan-phase1-2026 \
         agentic-vcs-automation-impl-plan-phase3-2026 \
         mesh-dashboard-and-distributed-compute-research-2026 \
         mesh-and-language-distribution-ssot-2026 \
         agentic-version-control-automation-research-2026 \
         unified-task-hopper-research-2026 \
         gui-authoring-syntax-2026 \
         vox-speech-surface-inventory-2026 \
         vox-speech-audit-findings-2026 \
         gui-native-roadmap-status-2026 \
         dashboard-migration-research-2026; do
  echo -n "$f: "
  grep -m1 'status' docs/src/architecture/$f.md
done
```

**The sweep was run during review; here is the real result — six files, not
one, are `status: "current"`:**

| File | status | occurrences |
| --- | --- | --- |
| `mesh-mens-distributed-training-and-execution-plan-2026` | **current** | 10 |
| `mesh-dashboard-and-distributed-compute-research-2026` | **current** | 6 |
| `mesh-and-language-distribution-ssot-2026` | **current** | 3 |
| `agentic-version-control-automation-research-2026` | **current** | 3 |
| `unified-task-hopper-research-2026` | **current** | 2 |
| `gui-native-roadmap-status-2026` | **current** | 1 |
| `dashboard-migration-research-2026` | **current** | 1 |
| `agentic-vcs-automation-impl-plan-phase1-2026` | roadmap | 1 |
| `agentic-vcs-automation-impl-plan-phase3-2026` | roadmap | 8 |
| `gui-authoring-syntax-2026` | roadmap | 2 |
| `vox-speech-surface-inventory-2026` | research | 1 |
| `vox-speech-audit-findings-2026` | research | 1 |

An earlier draft of this plan predicted at most one such file. That prediction
was wrong, and several of the six are worse than historical citations — they
assert `vox-dashboard` as **currently shipping** architecture and carry dead
relative links into the deleted crate:

- `mesh-dashboard-and-distributed-compute-research-2026.md:80` — "**What ships.**
  [`vox-dashboard`](../../../crates/vox-dashboard/) is Phase-1..." (present
  tense, dead link)
- `agentic-version-control-automation-research-2026.md:196` — "Today the
  dashboard ([`crates/vox-dashboard/`](../../../crates/vox-dashboard/)) has no
  VCS surface at all..." (dead link)
- `unified-task-hopper-research-2026.md:110,778` — dead links
- `dashboard-migration-research-2026.md:11` — describes the crate as a still-
  standing migration destination

**Apply the Step 2 banner + `status` flip to all seven `current` files** (the
six above plus `mesh-and-language-distribution-ssot-2026`), and additionally
fix the four dead relative links listed above to point at `crates/vox-gui/`.
Leave the `roadmap`/`research` files alone — their frontmatter already tells a
reader not to treat them as current.

- [ ] **Step 6: Commit**

```bash
git add docs/src/architecture/mesh-phase4-dashboard-control-plan-2026.md docs/src/architecture/vox-gui-native-roadmap-2026.md
git commit -m "docs: flip two vox-dashboard-era planning docs to roadmap status"
```

(If Step 5 found `mesh-and-language-distribution-ssot-2026.md` needs the same
treatment, `git add` it in this same commit.)

---

### Task 5: Full gate and push

- [ ] **Step 1: Regenerate the doc inventory**

Every file this plan touches drifts `doc-inventory.json`'s line counts.

```bash
cargo run -q -p vox-cli -- ci doc-inventory generate --output docs/agents/doc-inventory.json
git add docs/agents/doc-inventory.json
```

- [ ] **Step 2: Confirm the total live occurrence count dropped as expected**

```bash
grep -roh 'crates/vox-dashboard' docs/src --include='*.md' | wc -l
```

Expected: well under 273 — the exact number depends on how many of the 205
historical occurrences in Task 4's two files remain (they are deliberately
left in place as historical record), so this is a sanity check, not an
exact-zero assertion. If it is still 273, something in Tasks 1-4 did not
actually commit — check `git log` before proceeding.

- [ ] **Step 3: Run the docs gates**

```bash
cargo run -p vox-doc-pipeline -- --lint-only
cargo run -q -p vox-cli -- ci check-links
cargo run -q -p vox-cli -- ci retired-symbol-check
```

Expected: all clean.

- [ ] **Step 4: Run the full pre-push tier**

Run: `vox ci pre-push --full`

- [ ] **Step 5: Push once**

```bash
git push -u origin HEAD
```

---

## Self-Review

**1. Spec coverage.** W2.2 in full: the five ADRs and two reference docs the
spec names as naming vox-dashboard "as the canonical implementation target"
are Task 1 (2 reference docs) and Task 3 (4 of the 5 ADRs — `027-dual-track-ui-surfaces.md`
is excluded per Global Constraints, already `status: deprecated`). The
self-contradiction in `vox-gui-capability-audit-2026.md` (found during the
audit, not originally in W2.2's file list) is Task 2. The two files
concentrating 229 of 273 occurrences are Task 4.

**2. Placeholder scan.** No TBDs. Task 4 Step 5's sweep is a real command with
a real conditional outcome, not a deferred item.

**3. Type consistency.** N/A — this plan touches only markdown, no code
interfaces to track across tasks.

**Ordering:** Tasks 1-4 are mutually independent (disjoint files) and may be
done in any order; listed order groups by edit type (rewrite → self-fix →
ADR-annotate → status-flip). Task 5 last.
