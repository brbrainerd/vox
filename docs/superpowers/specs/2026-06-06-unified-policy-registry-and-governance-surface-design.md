---
title: Unified Policy Registry & Governance Surface
description: A single SSOT cataloging every CI/CD gate, language rule, and audit, surfaced (readable, per-branch status, eventually toggleable) in the Vox GUI and CLI.
category: architecture
---

# Unified Policy Registry & Governance Surface — Design

**Status:** Design (approved for Phase 1 implementation planning)
**Date:** 2026-06-06
**Author:** brainstorm session (brbrainerd)

## 1. Problem

Everything that governs the codebase — CI gates, language rules (including the
no-stub / TOESTUB family), architecture guards, and audit commands — is defined
across **six disjoint surfaces** with no single catalog, no uniform way to read
what a rule actually does, and only ad-hoc enable/disable:

| Domain | Defined in | ~Count | Toggle today |
|---|---|---|---|
| CI gates (`vox ci …`) | `CiCmd` enum + `run_body.rs` | ~125 | scattered `VOX_SKIP_*` env vars |
| Audit checks (`vox audit`) | `contracts/ci/check-targets.v1.yaml` | dozens | manifest `quick_skip` only |
| CR-L gates | `contracts/ci/vox-audit-contract.v1.yaml` + `vox-audit` registry | 10 | none |
| Language rules / TOESTUB | `contracts/code-audit/rules.v1.yaml` + `vox-code-audit` detectors | ~56 | `rule_filter`, suppressions |
| Architecture rules | `docs/src/architecture/layers.toml` `[guards]` | 15 | per-rule severity |
| Workflow jobs | `.github/workflows/*.yml` | 34 files | workflow `if:` only |

A contributor cannot answer "what rules exist, what does each one check, is it
on, and did it pass on my branch?" from one place. Owners cannot turn a single
rule on/off without grepping code. There is no transparency surface and no
governance surface.

## 2. Goals / Non-goals

**Goals**
- One **canonical registry (SSOT)** describing every governable policy with
  enough metadata to render and read it.
- **Read the contents** of any rule (its pattern/command/severity/source) from
  both GUI and CLI.
- **Per-branch run status** so the GUI can color groups and badge the nav by the
  latest run — across **multiple active branches/worktrees at once**.
- **Individual enable/disable** of any rule (Phase 2), **local-dev authoritative
  only** — CI on GitHub always runs the full set; the overlay never weakens CI.
- Eventually a **single source for rule definitions** (Phase 3), deprecating the
  fragmented surfaces, gated on enable/disable being proven.
- Adequate automated test coverage at every phase; **before/after parity proof**
  that bootstrap generation transferred every rule losslessly.

**Non-goals**
- Phase 1 does **not** toggle anything (read-only) and does **not** make CI honor
  any overlay (ever, by design).
- We do **not** deprecate the rule *engines* (the matchers in `vox-code-audit`,
  the checks in `vox-arch-check`); Phase 3 migrates their *definition data* into
  the registry, not their execution logic.
- User-authored custom rules are designed-for but **deferred to Phase 4**.

## 3. Phased plan

The registry is built so each phase drops in without reshaping the schema.

- **Phase 1 — Catalog + per-branch status (read-only).** *This spec details it.*
- **Phase 2 — Local-dev enable/disable.** The `[policies]` overlay in
  `Vox.toml`/`~/.vox/config.toml`, honored by local `vox ci`/`vox audit`/lint
  runs; GUI toggles go live. CI ignores the overlay.
- **Phase 3 — Definition migration + deprecation.** The registry becomes the
  SSOT for rule *definitions*; each engine's loader is migrated to read from it;
  `rules.v1.yaml`, `layers.toml [guards]`, `vox-audit-contract.v1.yaml`, and
  `check-targets.v1.yaml` metadata are deprecated with loud, reversible warnings.
  Gated on Phase 2 confirmed working, with before/after parity at each step.
- **Phase 4 — User-defined rules.** Authoring UI + `origin: user` rules,
  persisted to a user rules file, validated and run by the existing engines.

Each phase gets its own spec→plan→implementation cycle. Phases 2–4 below are
**outline only**.

---

## 4. Phase 1 — detailed design

### 4.1 The Unified Policy Registry (SSOT)

New contract: **`contracts/policy/policy-registry.v1.yaml`** + a JSON Schema
`contracts/policy/policy-registry.v1.schema.json`. One entry per policy:

```yaml
- id: code-audit/stub/todo          # stable, namespaced, unique
  domain: code-audit-rule           # ci-gate | audit-check | crl-gate |
                                    #   code-audit-rule | arch-rule | workflow-job
  title: "TODO stub detector"
  group: "Language rules / Stubs (TOESTUB)"   # drives the GUI group tree
  description: "Flags stub placeholders left in shipped code (no-stubs policy)."
  severity: error                   # error | warn | info  (rule-like domains)
  blocking: true
  runs_on: [pre-commit, pre-push, ci]
  source:                           # lets the GUI/CLI SHOW the contents
    kind: pattern                   # pattern | command | guard | subcommand | workflow
    ref: "contracts/code-audit/rules.v1.yaml#stub/todo"
    detail: |                       # the actual matched content / command
      todo!()|unimplemented!()|panic!("not implemented")
  docs: "docs/src/.../toestub.md"   # optional deep-link
  default_enabled: true             # inert in Phase 1
  protected: true                   # reserved: cannot be disabled in Phase 2
  origin: builtin                   # builtin | user  (reserved for Phase 4)
```

Fields `default_enabled`, `protected`, `origin` exist from day one but are inert
in Phase 1. `protected: true` marks rules that even Phase 2 will refuse to
disable locally (e.g. `stub/*`, `llm_provider_call`, layer ordering) — chosen by
policy, surfaced as a locked toggle.

### 4.2 Bootstrap generation + lossless transfer proof

Hand-authoring ~240 entries is infeasible and drift-prone. Phase 1 ships a
**bootstrap generator** that seeds the registry from existing sources:

- `CiCmd` enum + each variant's help/description → `ci-gate` entries.
- `contracts/code-audit/rules.v1.yaml` → `code-audit-rule` entries (id, severity,
  pattern, message).
- `docs/src/architecture/layers.toml [guards]` → `arch-rule` entries (severity
  from the warn/error/strict setting).
- `contracts/ci/check-targets.v1.yaml` → `audit-check` entries.
- `contracts/ci/vox-audit-contract.v1.yaml` + `vox-audit::registry()` →
  `crl-gate` entries.
- `.github/workflows/*.yml` job names → `workflow-job` entries (link-only).

**Transfer verification (your requirement):** the generator runs in two modes:

1. **Snapshot-before:** enumerate every source's full set of (id, description,
   severity, source-ref) into a normalized JSON snapshot.
2. **Generate** the registry.
3. **Snapshot-after / diff:** assert the registry reproduces every snapshot-before
   tuple — **zero dropped, zero altered, zero invented**. Any mismatch fails.

This diff is committed as a golden (`contracts/policy/transfer-parity.golden.json`)
so regressions in the generator are caught.

### 4.3 Completeness / drift gate

New CI gate **`vox ci policy-registry-parity`** (added to the registry as its own
`ci-gate`) enforces, on every run:

- every real `CiCmd` variant, `vox-code-audit` detector id, `layers.toml` guard,
  `check-targets` entry, and CR-L gate **has** a registry entry; and
- every registry entry of those domains **points at something that exists**.

This is what keeps the "new SSOT" honest against the repo's no-drift culture.
Adding a gate/detector without a registry row fails CI.

### 4.4 Ownership & build-time placement

Per the constraint "belongs in VoxConfig unless it would detonate build times":

- **`vox-config`** owns the lightweight runtime side — the `PolicyRegistry`
  model, the YAML loader (via **already-present** `serde_yaml` —
  [`Cargo.toml`](../../../crates/vox-config/Cargo.toml) — **zero new deps**), the
  per-branch status **reader**, and (Phase 2) the `[policies]` overlay +
  `is_enabled(id, branch)` resolution. This is small, stable code; `vox-config`'s
  high fan-in (~37 crates) only costs recompiles when this rarely-changing data
  changes.
- **`vox-cli`** owns the heavy machinery — the bootstrap generator, the
  `policy-registry-parity` gate, the `vox policy` commands, and the status
  **writer** — because they must reflect over the `CiCmd` enum, detectors, and
  workflow files. Putting that in `vox-config` would invert layers and *would*
  blow up build times.
- **`vox-gui`** backend is a thin Tauri IPC shim over the `vox-config` reader.

If the model later adds churn to `vox-config`, it can be extracted to a leaf
`vox-policy` (L1) crate without API change; we start in `vox-config` per the
stated preference.

### 4.5 Per-branch run-status overlay (multi-branch)

Status coloring, the "Needs attention" group, and the nav badge need per-rule
pass/fail **for each active branch**.

- **Schema:** `PolicyRunReport` — `{ branch, commit, ran_at, results: [{ id,
  status: pass|fail|warn|unknown, hits: [{file,line,note}], output_ref }] }`.
- **Store:** `.vox/policy-status/<sanitized-branch>.json` — one file per branch,
  so multiple worktrees/branches coexist. (Already under the repo-scoped `.vox/`.)
- **Writer:** local `vox ci`/`vox audit` runs append/replace results for rules
  they executed. Rules whose gate does **not** yet emit machine-readable results
  record `status: unknown` — never a faked `pass`. Wiring emitters is incremental.
- **Reader:** `vox-config` exposes "load reports for branches X, Y, …".
- **CLI:** `vox policy status [--branch <b>]…` prints the joined catalog+status.

Honesty note: on day one, status completeness equals the set of gates that emit
results. Grey "not run" is the truthful default; we light up rules as emitters
land. We never color a rule green without a real passing result.

### 4.6 CLI surface (read-only)

New top-level group **`vox policy`** (distinct from `vox config`):

- `vox policy list [--domain d] [--group g] [--status s] [--branch b] [--json]`
- `vox policy show <id>` — full detail **including the actual rule contents**
  (the `source.detail`, severity, runs_on, docs) → satisfies "read the contents
  of all CI/CD rules".
- `vox policy status [--branch b]` — per-branch results joined to the catalog.
- `vox policy domains` / `vox policy groups` — the taxonomy.

`enable`/`disable` verbs are **reserved** (Phase 2), not implemented now.

### 4.7 GUI Policies surface

A live-backend surface registered in
[`contracts/gui/surface-registry.v1.yaml`](../../../contracts/gui/surface-registry.v1.yaml)
(`view_key: policies`, `nav_group: operate`). Note: the current "Policies" nav
label points at the **Matrix/routing-policies** surface — that is a different
concept (orchestrator routing); this design **renames Matrix's label** to
"Routing" and gives "Policies" to this new surface, to avoid a collision. (Open
item flagged in §7.)

**Layout** — the rule itself is the primary object; everything else is
secondary and collapses to nothing when empty:

```
MASTER SIDEBAR (collapsible)  |  GROUP RAIL (collapsible,   |  RULE DETAIL & CONTENTS
  …                           |   reuses Sidebar primitives)|   (PRIMARY — largest pane)
  Policies  ●3 ▲1  ◀ badge    |   ⚠ Needs attention  (●4)   |   code-audit/stub/todo
                              |   ● CI Gates  (●1 ✓40)      |   [ Edit ✎ ] [ Disable ⏻ ]
   Branch ▾ (multi-select)    |   ▾ Language rules (●2 ▲1)  |   ── what it does ──
                              |     Stubs (●1)              |   nature, why it exists, severity,
                              |   ● Architecture (▲1 ✓14)   |   runs_on, blocking, source path
                              |   ◇ CR-L gates (—10 grey)   |   ── contents (editable target) ──
                              |  [compact rule list here]   |   pattern / command (the focus)
                              |                             |   ── last run (per branch) ──
                              |                             |   status + hits + view output
```

Key behaviors derived from review feedback:

- **Rule detail/contents is the dominant pane** (widest). It carries more detail
  about the *nature* of the rule and shows its *contents as the editable target*
  (Edit button prominent; the contents block is what Phase 3 editing acts on).
- **"Needs attention" shrinks to an all-clear state** when a branch has no
  failures — it must not dominate empty space. The compact rule list and the
  detail pane fill the space instead.
- **Two collapsible rails reuse the same `Sidebar` component primitives**;
  collapsing the master sidebar frees width for the group rail and the detail
  pane. No competing fixed sidebars.
- **Per-group counts are status-colored failures** (`●` red blocking, `▲` yellow
  warn, `✓` green pass, `—` grey not-run), not plain totals.
- **Master-sidebar badge** on Policies shows worst-status counts for the
  selected branch(es).
- **Branch selector supports multiple active branches at once** (worktrees). The
  "Needs attention" group and every section reflect the selected branch set; with
  multiple selected, a rule's row shows per-branch status chips.

Transport: Tauri IPC commands (`policy_list`, `policy_show`, `policy_status`,
`list_branches`) over the existing persistent-daemon seam; reads only in Phase 1.

### 4.8 Testing strategy (Phase 1)

- **Unit (`vox-config`):** registry deserialize round-trips against the schema;
  `PolicyRunReport` parse; multi-branch reader returns isolated per-branch sets;
  unknown-status default when a rule has no result.
- **Generator / parity (`vox-cli`):** golden test of the before/after transfer
  diff (lossless); `policy-registry-parity` fails when a fixture detector/CiCmd is
  added without a registry row, and when a row points at a missing id.
- **Schema validation:** the committed `policy-registry.v1.yaml` validates against
  its JSON Schema in CI.
- **CLI:** snapshot tests for `vox policy list/show/status` (text + `--json`).
- **GUI:** vitest for the group-tree builder (counts/colors from a fixture
  report), the empty "Needs attention" state, multi-branch chip rendering, and
  the collapse behavior; a Playwright golden route (behind the existing GUI
  smoke flag) asserting the surface renders the fixture catalog.
- **Arch/SSOT:** `where-things-live.md` row added; `vox-arch-check` green;
  `policy-registry-parity` wired into the CI summary as non-blocking first, then
  blocking once the catalog is complete.

---

## 5. Phase 2 — local-dev enable/disable (outline)

- `[policies]` table in `Vox.toml` + flat keys in `~/.vox/config.toml`; precedence
  `env > Vox.toml > ~/.vox > registry default_enabled`.
- `vox-config::is_enabled(id, branch)`; local `vox ci`/`vox audit`/lint dispatch
  skips disabled, non-`protected` rules and records `status: skipped`.
- `vox policy enable/disable <id>`; GUI toggles call IPC `policy_set_enabled`.
- **CI never reads the overlay** — a CI-side assertion proves the full set ran.
- Tests: disabled rule is skipped locally but still runs in a simulated CI
  context; `protected` rules refuse disable; precedence resolves correctly.

## 6. Phase 3 — definition migration + deprecation (outline)

- Extend the registry schema to hold full rule *definitions* (the matcher pattern,
  guard thresholds, command argv) — promoting `source.detail` to authoritative.
- Migrate each engine's loader (`vox-code-audit`, `vox-arch-check`, `vox-audit`,
  `vox audit` check-targets) to read definitions from the registry behind a flag.
- For each migrated source: snapshot-before, migrate, snapshot-after diff (same
  lossless proof as §4.2) before flipping the default.
- Deprecate the old files with loud, reversible warnings (`VOX_POLICY_LEGACY=1`
  to fall back) following the GitLab-retirement pattern (reversible, kept, warned).
- Retire `VOX_SKIP_*` ad-hoc env vars in favor of the overlay.

## 7. Phase 4 — user-defined rules (outline)

- `origin: user` entries in a user rules file (`.vox/policy/user-rules.yaml`).
- Authoring UI in the detail/"Add rule" affordance; schema validation;
  pattern rules run by the existing detector engine, command rules subject to
  exec-policy (`contracts/terminal/exec-policy.v1.yaml`).

## 8. Open items

- **Naming collision:** the existing "Policies" nav = Matrix/routing. Proposed:
  rename Matrix's label to "Routing" and claim "Policies" for this surface.
  Confirm during Phase 1 planning.
- **Workflow-job domain depth:** Phase 1 treats `workflow-job` entries as
  link-only (name + file), since their "rule" is orchestration, not a matcher.
  Sufficient for the catalog; revisit if owners want them toggleable.

## 9. Where-things-live

Add a row: *"Unified policy catalog (CI gates, language rules, audits) →
`contracts/policy/policy-registry.v1.yaml`; runtime model/loader/status reader →
`vox-config`; generator/parity/`vox policy` CLI/status writer → `vox-cli`; GUI
surface → `vox-gui` (`view_key: policies`)."*

---

## 10. Verification addendum (2026-06-06)

Load-bearing assumptions were hand-verified against the code (parallel agents +
file:line evidence). Corrections that supersede the body above:

**Confirmed as written:** `CiCmd` is `#[derive(Subcommand)]` with 122 doc-commented
variants ([cmd_enums.rs](../../../crates/vox-cli/src/commands/ci/cmd_enums.rs));
`detectors::all_rules()` enumerates **51** detectors with trait metadata
([detectors/mod.rs:136](../../../crates/vox-code-audit/src/detectors/mod.rs));
`layers.toml [guards]` has 11 keys (`fan_in/loc_budget/orphan/docstring/
description/where_things_live/wtl_parity/loc_delta/staleness/generated_file_drift/
forbidden_deps`); CR-L = 9 + 1 Tooling; the **nav collision is real** —
`view_key: matrix` currently owns `nav_label: Policies`
([surface-registry.v1.yaml:46](../../../contracts/gui/surface-registry.v1.yaml)),
so renaming Matrix's label to "Routing" and claiming "Policies" is a confirmed
plan step (no longer an open item). Sidebar collapse already exists
(`SidebarMode = rail|default|wide` + Ctrl+B + per-section,
[Sidebar.tsx](../../../crates/vox-gui/ui/src/components/layout/Sidebar.tsx)).

**Corrections (these override §4.2–§4.7):**

1. **CI-gate enumeration source.** Prefer `contracts/operations/catalog.v1.yaml`
   (78 CI ops with `description`/`description_human`; `command-registry.yaml` is
   derived from it) over reflecting the enum. The existing clap introspection
   `build_catalog()` over `VoxCliRoot::command()`
   ([command_catalog.rs](../../../crates/vox-cli/src/command_catalog.rs)) is what
   the parity gate uses to catch enum↔catalog drift.

2. **Detector enumeration source.** Use `detectors::all_rules()` (51,
   authoritative). `rules.v1.yaml` holds only the **45** static-pattern rules; the
   6 `scaling/*` detectors are generated dynamically (`___vox_scaling_dynamic___`
   sentinel) via `vox_scaling_policy`. Transfer-parity must reconcile 51 ⊇ 45 + 6.

3. **Status overlay is mostly greenfield, and granularity changes (supersedes
   §4.5).** Verified: `vox ci <gate>` returns `Result<()>` only — **no uniform
   result type, no per-rule emission**; `vox-arch-check` is text-only;
   `.vox/policy-status/` does not exist. Only `vox-code-audit` (`Finding` struct
   is serde-ready with an `OutputFormat::Json` path but **no `--json` CLI flag
   wired**, [rules.rs:134](../../../crates/vox-code-audit/src/rules.rs)) and
   `vox-effort-audit` (`findings.jsonl`) emit structure today. Therefore:
   - **`ci-gate` / `audit-check` / `crl-gate` → per-GATE status** (pass/fail/ms),
     captured by **one dispatch wrapper** in
     [run_body.rs](../../../crates/vox-cli/src/commands/ci/run_body.rs) — not
     invasive per-gate edits. A gate's registry `id` is the result key.
   - **`code-audit-rule` / `arch-rule` → per-FINDING (per-rule) status**, by
     wiring a `--json` flag onto the existing `Finding`/arch `Report` structs.
   - **`workflow-job` → not captured locally** (CI-only), shows grey.
   - The `.vox/policy-status/<branch>.json` store and the `PolicyRunReport` type
     are net-new. Grey "not run" is the truthful default; never a faked pass.

4. **vox-config repo-root.** `vox-config` does **not** self-discover the workspace
   root ([impl_ops.rs:31](../../../crates/vox-config/src/config/impl_ops.rs) takes
   an explicit `repo_root`). The new `load_policy_registry(repo_root)` and the
   status reader follow the same convention; callers (CLI, GUI daemon) pass the
   root. (Workspace discovery exists in `vox-audit::workspace_root()`, which
   `vox-config` must not depend on — layer order.)

5. **GUI git access is net-new.** No branch/worktree listing exists in the
   `vox-gui` backend. The multi-branch selector requires net-new handlers
   (`list_branches`, `current_branch`, worktree detection), simplest via
   `execute_command` wrapping `git worktree list` / `git branch`, registered in
   `tauri::generate_handler!`
   ([main.rs:75](../../../crates/vox-gui/src/main.rs)).

**Net effect on Phase 1 scope:** the catalog/registry/parity work is as designed;
the status overlay is larger than implied (dispatch wrapper + two `--json`
wirings + new store), but tractable and centralized. Sidebar-collapse and
enum-reflection work is pruned (reuse existing).

## Addendum (2026-06-06): store-capture coverage is `ci-gate` + `code-audit` only

Honesty note on what actually populates `.vox/policy-status/<branch>.json` today,
so the overlay is not read as "fully wired":

- **`ci-gate/*`** — captured per-gate by the dispatch wrapper in
  [run_body.rs](../../../crates/vox-cli/src/commands/ci/run_body.rs). A tracked
  gate's `Ok`/`Err` outcome maps to `Pass`/`Fail` via `gate_status_result`; the
  failure path of every tracked arm now *evaluates* to `Err` (no early `return`),
  so a failing gate overwrites a stale `Pass` instead of staying green.
- **`code-audit-rule/*`** — captured per-rule via `code_audit_results`.
- **`arch-rule/*`, `crl-gate/*`, `audit-check/*`** — **NOT captured into the store
  yet.** `vox-arch-check --json` emits per-rule results to *stdout only*
  ([main.rs `to_rule_results()`](../../../crates/vox-arch-check/src/main.rs)); no
  consumer reads them into `.vox/policy-status/`. These ids therefore render the
  truthful grey "not run" until a follow-on wires a consumer (run the arch `--json`
  projection from the relevant `vox ci` arch gate and `write_results` keyed by
  `arch-rule/<guard>`). `workflow-job/*` remains CI-only grey by design.

This is a documented follow-on, not a regression: grey "not run" stays honest.
