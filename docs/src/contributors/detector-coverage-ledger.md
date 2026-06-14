---
title: "Detector Coverage Ledger"
description: "Which perennial bug classes are caught by which static guardrail, at what severity and enforcement point — and which classes are still open gaps."
category: "Contributors"
---

# Detector Coverage Ledger

A single auditable view of "**bug class → detector → severity → enforcement point**" for the
static guardrails (TOESTUB detectors, `rules.v1.yaml` rules, arch-check, CI gates). Derived
from the graphify codebase audit (`graphify-out/`) and a scan of 528 `fix()` commits — see
[`docs/superpowers/plans/2026-06-14-guardrail-capability-plan.md`](../../superpowers/plans/2026-06-14-guardrail-capability-plan.md)
and AGENTS.md §"Perennial Bug Patterns".

**When you add or change a detector, add/update its row here.** Keep `GAP` rows for classes we
have deliberately not yet automated, so they stay visible rather than forgotten.

## Severity vs enforcement (how to read the table)

Findings only *fail a gate* per the run mode: `enforce-warn` (the CI `toestub-scoped` step)
fails on **Critical** only; `enforce-strict` (the pre-commit `tdd-guard`, scoped to skeleton
rules) fails on **Warning+**. So **`Info`** detectors are advisory everywhere — they surface in
reports and PR review but never block. New, higher-FP detectors land at `Info` and are promoted
only after precision is proven.

## Covered classes

| Bug class | Detector id | Kind | Severity | Enforcement | Notes |
|---|---|---|---|---|---|
| Cross-crate split-brain (byte-identical body in ≥2 crates) | `vox/cross-crate/duplicate-logic` | Rust batch (`detectors::cross_crate_dup`) | Info | CI full scan | Skips platform-sibling crates (`*-cuda`/`*-metal`) + trivial bodies. Exact string equality. |
| Catch-all-swallow (`match _ =>` returns empty while real arms exist) | `vox/catch-all-swallow` | Rust AST per-file | Info | pre-push complete / CI | Neutral set = `None`/empty std containers/`Default::default()`/`0`/`false`/`""`/`()`/`{}`. Skips diverging + guarded arms. |
| Toolchain-bump lint wave (new clippy/rustdoc lints on `rust-toolchain.toml` bump) | — (CI gate `toolchain-lint-wave`) | CI job | blocking (non-required) | CI, only on toolchain change | Fresh `cargo clean` + sccache-defeated clippy+rustdoc. |
| Stub / hollow / empty body | `skeleton/stub`, `skeleton/hollow-fn`, `arch/empty_body` | Rust AST + regex | Warning | pre-commit `tdd-guard` (skeleton, strict) / CI | Existing. |
| Same-file near-duplicate | `dry-violation` | Rust AST | Warning | CI | Existing; 0.98 similarity threshold. Complements cross-crate-dup. |
| Effect / purity / determinism violations | `vox/effect/effect-net-decl`, `vox/effect/pure-fn-impure`, `vox/workflow/nondeterministic` | Rust/Vox AST | Error | CI | Existing. |
| Direct LLM-provider call (bypasses facade) | `vox/llm/direct-provider-call` | Rust/Vox | Error | CI | Existing. [[project_model_agnostic_llm_boundary]]. |
| Secret-shaped env reads / hardcoded secrets / banned crypto | `vox/secret/env-secret-shape`, `security/hardcoded-secret`, `vox/crypto/ban` | Rust/Vox | Error | CI | Existing. |
| Retired surfaces (decorator/crate/env/memory-API/Capacitor) | `vox/retired/*` | Rust/Vox | Error | CI (CR-L6) | Existing. |
| Import cycles | `vox/import/cycle` (per-file) + `detect_import_cycles_in_batch` | Rust/Vox | Error | CI | Per-file wired; batch path exists but is not engine-wired. |
| Arch layering / fan-in / LoC / orphan / generated-file drift | `vox-arch-check` (15 rules) | dep graph | Error (layering) / warn (rest) | CI | Existing. |
| SSOT / generated-doc drift | `vox ci ssot-drift` (+ `ssot-autoregen` PR bot) | aggregate | blocking | pre-push fast / CI / merge-queue regen | Existing + PR bot from this PR. |
| Pattern drift (reqwest bypass, path/timeout/version literals, bearer header, serde-default dup) | `vox-drift-check` (6 rules) | regex | Warning | pre-push fast / CI | Existing. |

## Open gaps (deliberately not yet automated)

| Bug class | Why still a GAP | Owner / plan |
|---|---|---|
| **Test asserts nothing** (reached-but-unproven symbols — ~7,950 in the graph) | High value but high FP; "meaningful assertion" is hard to define statically. Actively worked as a **coverage ratchet** (not a TOESTUB detector) on the `semantic-coverage-wave0` branch — building a detector here would collide. | `semantic-coverage.v1.json` ratchet + `docs/src/architecture/semantic-coverage-remediation-plan-2026-06-13.md` |
| **Semantic-equivalence split-brain** (same logic, different body) | `cross-crate-dup` only catches *byte-identical* bodies; logically-equivalent-but-reworded copies escape it. Needs a normalized-AST or behavioral oracle. | guardrail-capability-plan (future) |
| **Per-symbol assertion depth** | `catch-all-swallow` + the coverage ratchet cover slices; a per-symbol "is this return value asserted" detector is a larger effort. | guardrail-capability-plan (future) |
| **FP floor on weakest existing rules** | The `detect-rules-bench` F1 gate is `0.70`; raising it requires mining real false-positives per rule and expanding fixtures. | guardrail-capability-plan T5 |

## How to add a detector

1. Implement `DetectionRule` (per-file) in `crates/vox-code-audit/src/detectors/<name>.rs`, or a
   `detect_<name>_in_batch(&[SourceFile])` free fn for cross-file analysis; register in
   `detectors/mod.rs` (`all_rules()` + `rule_count()` for per-file, or an engine hook for batch).
2. Add a `pub const` to `diagnostics/catalog.rs`.
3. **New, higher-FP detectors land at `Severity::Info`.** Prove precision on the real corpus
   before proposing a promotion.
4. Add `#[cfg(test)]` unit tests (pos + neg). For simple line-pattern rules, prefer a
   `rules.v1.yaml` rule with pos/neg fixtures measured by `vox ci detect-rules-bench`.
5. Add a row above.
