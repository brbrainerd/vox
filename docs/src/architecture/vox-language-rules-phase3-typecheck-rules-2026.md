---
title: "Vox Language Rules — Phase 3: Cheap Typechecker Rules (2026-05-09)"
description: "Step-by-step plan to add typechecker rules that make wrong programs structurally unrepresentable: Id[T] required at API boundaries (no bare str IDs), named tagged-union error types (no Result[T, str] on public APIs), single workspace syntax_version enforced, @deprecated machine-checked across versions, training_eligible propagation, and a precursor warning for the Phase 5 effect system. Each rule lands as warning for one minor version, escalates to error in the next. Includes vox migrate id-strings codemod for the Id[T] migration."
category: "Architecture SSOTs"
status: "roadmap"
training_eligible: true
training_rationale: "Phase 3 child plan. Strongest 'wrong programs unrepresentable' wins land here; the Id[T] rule alone removes a major LLM hallucination class."
sourced_at: "2026-05-09"
vox_relevance:
  - "vox-compiler/src/typeck/: new rules for id_required_at_boundary, named_error_type, syntax_version, deprecated_chain"
  - "vox-cli: new vox migrate id-strings subcommand"
  - "vox-corpus: training_eligible propagation graph validator"
  - "examples/golden/: any bare str IDs at API boundaries replaced via codemod"
---

# Phase 3 — Cheap Typechecker Rules

> **Parent plan:** [`vox-language-rules-and-enforcement-plan-2026.md`](vox-language-rules-and-enforcement-plan-2026.md)
> **Depends on:** Phase 2 Task 1 (diagnostic catalog populated). Independent of Phase 4.
> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans.

**Goal:** Add typechecker rules that promote known-good policies from convention to compile error. Six rules in this phase: `Id[T]` required at API boundaries, named error types in public `Result`, single workspace `syntax_version`, machine-checked `@deprecated`, `training_eligible` propagation, and a `note`-severity precursor for the Phase 5 `net`-effect declaration. Plus a `vox migrate id-strings` codemod that handles the largest mechanical migration ahead of the rule escalating to error.

**Architecture:** Rules live in `crates/vox-compiler/src/typeck/rules/<rule_name>.rs`, each implementing a `TypeckRule` trait that runs against the typed HIR after type inference. Diagnostics emitted use the Phase 2 catalog and the Phase 2 `LintFix` shape — even though these are typechecker-level rules, their diagnostic shape is identical to lint diagnostics so LLM agents and tooling consume one report format.

The line between "Phase 2 lint" and "Phase 3 typecheck rule" is enforcement *strength*: lint can be suppressed locally with `// toestub-ignore(...)`; typecheck rules in this phase escalate to compile errors that *cannot* be suppressed without a workspace-level allowlist that requires a maintainer review.

**Out of scope for Phase 3:**
- Effect inference (`@uses(net)`, etc.) — Phase 5.
- `@no_panic` reachability proof — research-tier; descope to a Phase 2 lint already.
- Full taint type system for `@secret` — Phase 5.
- State-machine totality proof — descoped to a Phase 2 lint.

---

## Verification setup

- `cargo test -p vox-compiler --lib typeck::rules::` — per-rule unit tests.
- `cargo test -p vox-compiler --test typeck_golden` — input-`.vox`/expected-diagnostic snapshot.
- `cargo run -p vox-cli -- check examples/golden/` — should pass after burn-down + codemod runs.
- `cargo run -p vox-cli -- migrate id-strings examples/golden/` — round-trip test: apply, re-check, expect zero violations.

---

## Task 1: `Id[T]` required at API boundaries

**Diagnostic:** `vox/types/id-required-at-boundary` (warning at land; error after one minor).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/id_required_at_boundary.rs`
- Modify: `crates/vox-compiler/src/typeck/mod.rs` — register the rule
- Modify: `docs/src/reference/diagnostics/types-id-required-at-boundary.md`
- Create: `examples/golden/anti/types-id-required-at-boundary.vox`

**Rule:** A parameter or return type at the boundary of:
- `@endpoint` fn
- `@table` row type
- `@activity` fn
- `actor` message field
- `workflow` input/output

…that has type `str` AND a name matching `(.*_id|id|.*Id|.*_uid|uuid)` (case-insensitive) fires the rule. Suggested fix: declare a newtype `Id[T]` in the same module and replace all sites.

**Suggested fix shape (`Confidence::Likely`):**

```
Composite([
    InsertBefore { offset: <module top>, text: "type UserId = Id[User];\n" },
    Replace { range: <param: str>, new_text: "param: UserId" },
    // ... call-site replacements
])
```

**Rationale prose for `--explain`:**

> Stringly-typed IDs at API boundaries are the single largest LLM-confusion vector in CRUD code. A function `fn delete(user_id: str, project_id: str)` invites callers (and LLMs) to swap the arguments silently — both have the same type. `Id[User]` and `Id[Project]` make the swap a compile error.
>
> Internal helpers may use `str` IDs; this rule fires only at the API boundaries where wrong arguments propagate to other systems.

**Why "boundary only":** Requiring `Id[T]` everywhere is too disruptive. Boundaries are where the wrong-ID-class has cross-system blast radius.

**Verify:** Golden tests for: (a) endpoint with `user_id: str` fires, (b) endpoint with `user_id: Id[User]` passes, (c) internal helper with `str` ID does not fire, (d) `Composite` autofix is round-trip clean.

---

## Task 2: `vox migrate id-strings` codemod

**Files:**
- Create: `crates/vox-cli/src/migrate/id_strings.rs`
- Modify: `crates/vox-cli/src/main.rs` — wire `vox migrate id-strings`

**Why:** The Phase 3 Task 1 rule will fire on existing corpus. Hand-fixing every callsite is hours of LLM-agent work; a codemod that runs across a workspace fixes it in seconds.

**Algorithm:**

1. Walk all `.vox` files matching `**/*.vox`.
2. For each public `fn` / `@table` / etc. boundary parameter matching the heuristic from Task 1:
3. Compute a candidate newtype name from the parameter name + the table/struct it's used with (heuristic: if the param is named `user_id` and used in a body call to `users.find(user_id)`, the type is `Id[User]`).
4. Insert the newtype declaration at module top if missing.
5. Replace all callsites in the same file.
6. Cross-file callsites: emit a TODO comment at the callsite for human review (the codemod is conservative).

**Flags:**
- `vox migrate id-strings` — apply.
- `vox migrate id-strings --dry-run` — preview.
- `vox migrate id-strings --json` — emit per-file diff descriptors for LLM consumption.

**Verify:** Apply codemod to a synthetic 20-file workspace; assert post-codemod `vox check` produces zero `vox/types/id-required-at-boundary` violations within-file (cross-file TODO comments are out of scope for round-trip clean).

---

## Task 3: Named tagged-union error types

**Diagnostic:** `vox/types/anonymous-error-type` (warning at land; error after one minor).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/named_error_type.rs`
- Modify: `docs/src/reference/diagnostics/types-anonymous-error-type.md`
- Create: `examples/golden/anti/types-anonymous-error-type.vox`

**Rule:** On a public boundary (same definition as Task 1), `Result[T, E]` where `E` is `str` or an anonymous tuple/struct fires the rule. `E` must be a *named* tagged-union type.

**Why:** [A.10]. Stringly-typed errors lose type-directed handling, defeat exhaustive `match`, and force callers to parse strings. Named errors are exhaustively pattern-matchable; LLMs can pattern-match the closed set and pick the right handler.

**Suggested fix:** Cannot autofix the union design. Diagnostic suggests declaring a `pub type ErrorKind = | NotFound | InvalidInput | ...` and provides a template.

**Burn-down:** Across the corpus, identify public `Result[T, str]`; for each, propose a named error type via a follow-up PR.

**Verify:** Golden tests for: (a) `Result[T, str]` on `@endpoint` fires, (b) `Result[T, NotFoundError]` passes, (c) anonymous union `Result[T, A | B]` fires (not yet a *named* type), (d) named tagged union passes.

---

## Task 4: Single workspace `syntax_version`

**Diagnostic:** `vox/syntax/version-mismatch` (error at land — security-class).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/syntax_version.rs`
- Modify: `Vox.toml` schema — add `[workspace.syntax] version = "<semver>"` requirement
- Create: `crates/vox-cli/src/migrate/syntax_version.rs`

**Rule:** Every `.vox` file in the workspace declares `syntax_version: "<X.Y>"` in frontmatter. The workspace's declared version (from `Vox.toml`) must match. Mismatch → hard error with a precise message: which file, which version, the migrate command.

**Why error immediately:** Best-effort parsing across versions silently accepts wrong syntax and produces wrong codegen. This is a correctness, not style, issue.

**Suggested fix:** `vox migrate syntax-version --to <X.Y>` codemod. If the file is just stale frontmatter, the autofix is `Confidence::Certain`.

**Verify:** Mismatched version → CI fails; correct version → passes; codemod applies and round-trips clean.

---

## Task 5: `@deprecated` machine-checked across versions

**Diagnostic:** `vox/api/deprecated-callsite` (note → warning → error, escalating with version distance).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/deprecated_callsite.rs`
- Modify: `crates/vox-compiler/src/lower/decorators/deprecated.rs` — extend to require `since=<X.Y>` and `use=<replacement>` fields
- Modify: `docs/src/reference/diagnostics/api-deprecated-callsite.md`

**Rule:** Calling a `@deprecated(since=<X.Y>, use=<replacement>)` symbol fires:
- `note` if the consumer's `syntax_version` is the same minor as `since`.
- `warning` if one minor newer.
- `error` if two or more minors newer.

The escalation is based on the *consumer's* `syntax_version`, not the workspace, so a vendored library can stay on an older version with grace.

**Suggested fix (`Confidence::Likely`):** `Replace { range: <call>, new_text: "<use replacement>" }`. Only `Likely`, not `Certain`, because the replacement signature may differ.

**Why a Vox-only win [A.56]:** Rust doesn't reach into Vox `syntax_version` to compute version distance; only the Vox compiler can.

**Verify:** Synthetic `@deprecated(since="0.5.0", use="newFn")` — consumer at 0.5.x sees `note`; consumer at 0.6.x sees `warning`; consumer at 0.7.x sees `error`.

---

## Task 6: `training_eligible` propagation

**Diagnostic:** `vox/corpus/training-ineligible-import` (error at land — ships immediately because corpus integrity is security-class).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/training_eligible_propagation.rs`
- Modify: `crates/vox-corpus/src/build.rs` — consume the propagation graph
- Modify: `docs/src/reference/diagnostics/corpus-training-ineligible-import.md`

**Rule:** A `.vox` file with `training_eligible: true` cannot import (directly or transitively) a `.vox` file with `training_eligible: false`. The compiler walks the import graph and checks.

**Why a Vox-only win [A.61]:** Rust cannot reason about Vox file metadata graphs. Only the compiler that already has the import graph can do this in O(N).

**Suggested fix:** Cannot autofix. Diagnostic includes the import path that violates and the upstream file that's `training_eligible: false`.

**Verify:** Synthetic 3-file chain `a (true) → b (false) → c (true)` — `a` fires; flipping `b` to `true` clears it.

---

## Task 7: `note`-severity precursor for `@uses(net)` declaration

**Diagnostic:** `vox/effect/missing-net-decl` (note at land — Phase 5 escalates to warning then error).

**Files:**
- Create: `crates/vox-compiler/src/typeck/rules/effect_net_precursor.rs`

**Rule:** A `pub fn` that transitively calls a builtin in the `net` effect set (per Phase 1 Task 3 manifest) but does not declare `@uses(net)` fires `note`. This is the precursor for the Phase 5 hard rule.

**Why land it now:** Two reasons:
1. The corpus burn-down begins early — by the time Phase 5 escalates to `error`, every fn that needs `@uses(net)` already has it.
2. Establishes the symmetric ID pair (`vox/effect/missing-net-decl` ↔ `vox/effect/unjustified-net-decl`) that an LLM can learn during Phase 5 corpus updates. Honors the LLM-target principle "symmetric error/fix pairs."

**Suggested fix (`Confidence::Certain`):** `InsertBefore { offset: <fn span start>, text: "@uses(net)\n" }`.

**Verify:** Golden test for a fn that calls `populi.complete` (in `net` effect set) without `@uses(net)` declaration.

---

## Task 8: Decorator-position parser policy enforcement

**Diagnostic:** `vox/syntax/decorator-position-policy` (error at land).

**Files:**
- Modify: `crates/vox-compiler/src/parser/decorators.rs` — reject decorators in positions that violate AGENTS.md §131–164
- Modify: `docs/src/reference/diagnostics/syntax-decorator-position-policy.md`

**Rule:** A decorator named in the bare-keyword set (e.g., `@actor`, `@workflow`, `@activity`) fires; bare keywords used in decorator position fire (already covered by parse error today, but rephrased with the catalog ID).

**Why parser-level not lint-level:** [AGENTS.md:154–156] is structural, not stylistic. The Phase 2 lint catches *style* mismatches (`@actor actor X`); this Phase 3 parser policy ensures bare keywords cannot be redeclared as decorators in the future even if someone bypasses the closed-keyword-table check from Phase 1 Task 12.

**Verify:** `@actor` decorator usage → parse error with the catalog ID, not a generic syntax error.

---

## Task 9: AGENTS.md backlinks + where-things-live + ADR landing

**Files:**
- Create: `docs/src/adr/0NN-id-required-at-api-boundaries.md` (number assigned at land)
- Create: `docs/src/adr/0NN-named-error-types-required.md`
- Create: `docs/src/adr/0NN-single-workspace-syntax-version.md`
- Modify: `AGENTS.md` — add §"Type-Level Boundaries (Required)" with the three rules and backlinks
- Modify: `docs/src/architecture/where-things-live.md` — add rows for new typeck rules

**Verify:** ADR pages render via `vox-doc-pipeline --check`. AGENTS.md addition has backlinks both to this phase plan and to the new ADRs.

---

## Risks specific to this phase

| Risk | Mitigation |
|---|---|
| Task 1 codemod (Task 2) misclassifies the underlying type for `Id[T]`, producing wrong newtypes | Codemod is conservative: only emits `Id[<X>]` when it can find a `users.find(user_id)`-style call; otherwise emits `Id[Unknown]` and a `// TODO: name the entity` comment. Conservative wins over clever. |
| Task 3 (named error types) fires more violations than expected; corpus is full of `Result[T, str]` | Land Task 3 with a 60-day grace allowlist auto-populated from existing violations; new violations error immediately, existing violations warn until expiry. |
| Task 5 (`@deprecated` distance) escalates faster than expected, breaking older vendored code | The escalation is per-consumer-`syntax_version`, not workspace; a consumer can pin its version to delay escalation. Document the pin in the rule's `--explain` page. |
| Task 7 (`net`-effect precursor) is a confusing `note` at land time because the actual Phase 5 work is months away | The `--explain` page leads with: "This is a precursor for the Phase 5 effect system. You can fix it now (just add `@uses(net)`) or wait for Phase 5 to escalate the rule." |
| Codemods (Tasks 2, 4) produce malformed output on edge cases | All codemods round-trip through `vox check` after running; CI fails if a codemod's output doesn't re-check clean. |

---

## Phase 3 acceptance gate

- [ ] All six typeck rules implemented with golden tests.
- [ ] `vox migrate id-strings` codemod ships and round-trips clean on the workspace's `examples/`.
- [ ] `vox migrate syntax-version` codemod ships.
- [ ] Three new ADRs landed (`Id[T]`, named errors, single syntax version).
- [ ] AGENTS.md §"Type-Level Boundaries (Required)" landed.
- [ ] Corpus burn-down PRs land for Tasks 1, 3, 6, 7 (each escalates from warning to error in next minor).
- [ ] `where-things-live.md` updated.
- [ ] Retrospective appended.

---

## Retrospective

_Appended within 5 working days of phase completion._
