# Docs & GUI Program — Roll-Up and Execution Order

> **Not an implementation plan.** This is the index and sequencing contract for
> five sibling plans. Execute the plans; read this to know which one, in what
> order, and what breaks if you reorder them.

**Status as of 2026-08-22:** 5 plans written and audited, 1 in execution
(Task 1 of 5 complete), 0 pushed.

---

## Why this file exists

This program produced five plans in one session. Three of them have **hard
ordering dependencies discovered only by audit, not by design** — meaning a
reasonable person executing them in the order they were written would break the
tree. The most dangerous is silent: two plans referred to the same ADR by two
different numbers, and neither author knew.

This repository also has strong evidence about what happens to unindexed plan
sets: five prior docs plans exist with **328 open checkboxes and zero ticked
between them**. A sixth pile without an index is the predicted outcome. This
file is the index.

---

## The five plans

| # | Plan | Covers | Size | State |
| --- | --- | --- | --- | --- |
| 1 | `2026-08-22-gate-and-policy-honesty.md` | Spec W1, W6, W3.5, W7.1–7.2 | 14 tasks | **In execution** — Task 1 done, Task 2 in progress |
| 2 | `2026-08-22-retired-symbol-severity-valve.md` | Spec W3.6 | 3 tasks | Written, audited |
| 3 | `2026-08-22-vox-dashboard-corpus-repair.md` | Spec W2.2 | 5 tasks | Written, audited |
| 4 | `2026-08-22-mermaid-rendering-and-parse-gate.md` | Spec W4 | 7 tasks | Written, audited |
| 5 | `2026-08-22-gui-documentation-ssot.md` | GUI docs SSOT (own spec) | 7 tasks | Written |

Specs: `2026-08-22-docs-corpus-repair-design.md` (rev 3) for plans 1–4;
`2026-08-22-gui-documentation-ssot-design.md` for plan 5.

---

## Execution order, and what breaks if you deviate

```
  1. gate-and-policy-honesty        ← IN PROGRESS. Must finish Task 5 before plan 3.
        │
        ├── Task 5 renumbers 037-tauri-gui-replaces-axum-dashboard.md → 045
        │        │
        │        ▼
  3. vox-dashboard-corpus-repair    ← HARD DEP: cites that file ~10× as 045
        │
  2. retired-symbol-severity-valve  ← independent of 1 and 3; must precede any
        │                             future W3.1 contract-entry work
        ▼
  4. mermaid-rendering-and-parse-gate   ← fully independent
  5. gui-documentation-ssot             ← fully independent
```

### Hard dependency: plan 1 Task 5 → plan 3

**Plan 1 Task 5 renumbers `037-tauri-gui-replaces-axum-dashboard.md` to
`045-…`** (three files currently share the number 037). **Plan 3 adds roughly
ten links and prose citations to that exact file.**

Plan 3 has been corrected to cite **045** throughout and carries a
hard-prerequisite gate at the top:

```bash
ls docs/src/adr/045-tauri-gui-replaces-axum-dashboard.md
```

If that file does not exist, plan 1 Task 5 has not landed and plan 3 must not
start. Running plan 3 first produces ten links to a path that is about to move.

*This conflict was invisible to both plans' authors and was found by a
verification track. It is the single strongest argument for this roll-up file.*

### Soft dependency: plan 2 before future W3.1

Plan 2 builds the severity valve but **deliberately adds no `warn` entries**.
Its consumer — adding `vox-dashboard`, `vox-oratio`, `vox-dei-shim`,
`@endpoint`, and the decorator class to the retired-symbols contract — is
**not yet planned**. Adding those entries without plan 2 landed first produces
an estimated 460–620 hard CI failures on the first run, because
`retired_symbol_check` has no severity tier today.

Plan 3 reduces the `vox-dashboard` reference count, which makes that future
work smaller, but neither plan blocks the other.

### Independent: plans 4 and 5

Plan 4 (mermaid) touches `docs-astro/` and one broken diagram. Plan 5 (GUI
docs) touches the surface registry, `vox-gui/ui`, and a generated docs page.
Neither shares a file with plans 1–3. Both may run at any point, including in
parallel with the others **if run in separate worktrees** — see below.

---

## Worktree discipline

**One agent per worktree.** Earlier in this program, parallel write-capable
agents in a single worktree deleted each other's files mid-build, and a
verification agent reported a modified file it had not touched. Two plans
running concurrently in one tree will corrupt each other.

If parallelizing plans 4 and 5 against 1–3, use `git worktree add` per plan.

---

## Cross-cutting rules every plan inherits

These emerged from audit and bind all five:

1. **No number is authored.** Corpus counts come from
   `vox run scripts/docs-corpus-census.vox`, not from prose. Spec rev 3's §3
   contains no numbers at all, by design.
2. **No checker enters a plan until it has been RUN against the real tree and
   its actual output pasted into the step.** Across the first two plans, five
   guards were written to catch drift and **five could not fire** — two read
   the wrong markdown column, one skipped the rows it protected, one was
   permanently red, one asserted a hardcoded string against a file it never
   read. Every one was reasoned about instead of executed. "Expected: FAIL" is
   a transcript, not a prediction.
3. **Verification tier is `--full`.** `--complete` runs fmt, line-endings,
   ssot-drift, doc lint, doc-inventory, clippy, and TOESTUB — but **no tests**.
   Only `--full` adds `cargo nextest run --workspace`.
4. **`doc-inventory.json` drifts on nearly every task** in every plan and is
   verified in `--complete` and CI. Regenerate and commit it.
5. **Read before write.** Line numbers in these plans have already drifted from
   concurrent edits. Every plan's code steps instruct a fresh `grep`; trust
   that, not the numbers in the prose.
6. **CodeRabbit reviews once per PR on open.** Batch commits, push once.

---

## Known-outstanding, not yet planned

From spec rev 3, still without a plan:

| Item | Why not planned yet |
| --- | --- |
| **W3.1** — add retired-symbol contract entries | Blocked on plan 2 landing (the severity valve). The work-list exists in the spec. |
| **W5** — retirement/archival of 68 candidates | The candidate list was produced by audit but never written into the spec; needs enumerating before it can be planned. Spec explicitly marks the inbound-edge count UNVERIFIED. |
| **W7.3–W7.8** — remaining agent-artifact repairs | `inventory_gen.rs` hardcoded `vox-mcp` path, `ai-ide-feature-matrix` 14 dead paths, `llms.txt` archive pointer, README, CONTRIBUTING, `script-registry.json` (29 of 34 rows dead). All individually small; a single batch plan would suit. |
| **W8** — doctest reality | Deflated significantly by audit: the concat bug accounts for only ~15% of skipped fences, not "any multi-example file". Needs re-scoping before planning. |
| **W9** — MENS corpus poisoning | **Highest-value unplanned item.** Six golden examples' `@training_prompt` strings use retired syntax while their bodies use correct syntax — the model is being trained that `@table` is how you *request* `table`. Plus `extract_qa_sections` does not filter `vox:skip`, shipping retired-syntax fences into `vox_docs_qa` verbatim. The QA-lane filter is a one-line fix. |
| **VUV diagram projection** | Design section drafted (real HIR types, crate placement, CLI shape identified). Blocked on one open question: whether a VUV-rendered diagram can keep an agent-legible textual source, the property the mermaid plan was specifically designed to preserve. |

---

## Progress tracking

Plan 1 has an SDD ledger at
`.superpowers/sdd/2026-08-22-gate-and-policy-honesty/progress.md`. Plans 2–5
get their own on first execution (`scripts/sdd-workspace <plan>`).

**The ledger, not this file, is the source of truth for what has been done.**
This file sequences; the ledger records. After a compaction, trust the ledger
and `git log` over any summary.
