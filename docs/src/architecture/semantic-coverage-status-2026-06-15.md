---
title: "Semantic Coverage — Honest Status (2026-06-15)"
description: "Verified accounting of the semantic test-coverage initiative: what is proven, what is merely reached-but-unproven, what was actually tested vs what remains exigent."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Semantic Coverage — Honest Status (2026-06-15)

## TL;DR

- **We proved 15.9% of production behavior, not "most of it."** Of 19,445 production symbols, only **3,088 (15.9%)** are assertion-backed. **5,793** are reached-but-unproven (executed by a test with **zero asserted behavior**) — the keystone gap.
- **The keystone gap is 5,793 (reproducible), not 3.** An earlier committed artifact reported reached-but-unproven as **"3"** (a near-empty partial run) and was then corrected to a directional **7,950**; the now-**reproducible** figure from a real 1.19M-line-execution run is **5,793**. That is the real size of the remaining work.
- **We tested the easy leaf functions, not the exigent structural gaps.** The ~2,409 merged semcov tests are broad and well-written, but they overwhelmingly target tokenizers, validators, span merges, serde round-trips, and formatter idempotence — exactly the "useless touch" surface the initiative was meant to avoid.
- **Of the 7 named structural pipeline-gap patterns, 6 have NO dedicated coverage and 1 (split-brain) has a single weak intra-crate case.** The named headline regression — `top-level let → Decl::Const → catch-all → value vanishes` — is **completely untested** (zero `Decl::Const` references in the entire suite).
- **Reach is now reproducible locally, but not yet a CI gate.** The 5,793 comes from a real full-workspace run (1.19M lines executed, 60 crates) after fixing the `peft-rs` exit-101 blocker. The remaining gap is wiring this into CI as an enforced ratchet.

## What is actually DONE (verified)

- **~2,409 semcov `#[test]` functions across ~214 files are merged to `main`** (via PR #299; the larger PR #303 was closed but its content had already landed). This is real, merged, running code — not a branch.
- **3,088 production symbols are genuinely proven** with specific behavioral assertions on real code paths. That is a real floor, not an aspiration.
- **Adversarial quality audit of 7 sampled modules found 65–93% "real signal"** — specific behavioral assertions on real paths, with **no purely vacuous tests** in the sample. Crypto AEAD tests check authentication (not just round-trips).
- **The misreported "3" artifact has been corrected to the reproducible 5,793** (via 7,950). The lie is out of the record.
- **Two new `vox-code-audit` detectors landed and DO target the structural patterns**: `catch_all_swallow` (wildcard match arm drops a value) and `cross_crate_dup` (split-brain duplicated logic). These are the right detectors for patterns #1 and #7 — though detectors are not tests, and the tests that would exercise the patterns still do not exist.

## What is PARTIAL / misleading

- **Reach is reproducible locally but not yet gate-enforced in CI.** The 5,793 is from a real full-workspace run (1.19M lines executed); the earlier 3-crate exclusion (`vox-compiler`/`vox-corpus`/`vox-config`) is resolved — all now compile and execute their own tests after the `peft-rs` fix. What remains is porting the profraw-merge/chunked-export path into CI (Linux, no arg limits) and failing the build when reached-but-unproven rises.
- **The quality audit, while encouraging, named recurring weak patterns** that inflate apparent coverage:
  - `assert_ne!` on **derived enum discriminants** (proves the derive macro works, not your logic).
  - **Re-asserting self-constructed literals** (you built the value two lines up; asserting it equals itself proves nothing).
  - **Overstated `// Catches:` comments on no-panic tests** — a test that only proves "doesn't panic" claiming to catch a behavioral regression.
  - **Crypto hash correctness is under-pinned**: only SHA3 has a known-answer vector. The other hashes are round-trip-only, which does not pin correctness against an external reference.
- **"Proven > Reached" anomalies exist** (e.g. `vox-cli`: 650 proven vs 219 reached). Proven is a static `proves`-edge count (lcov-independent) while reached is matched per-file/line, so the two are measured differently and a crate whose tests live elsewhere can show proven > reached. Read the two columns as distinct signals, not a subset relationship.

## What is NOT started (the exigent core)

The whole point of the initiative was to close **structural pipeline-gap patterns** — places where a construct is silently dropped between stages. As of 2026-06-15, **6 of 7 are now covered** (#1, #3, #4, #5, #6, #7) by dedicated tests in `crates/vox-compiler/src/semcov_struct_pipeline_tests.rs`, `crates/vox-compiler/tests/decl_lowering_test.rs`, `crates/vox-cli/tests/effort_pricing_parity.rs`, and `crates/vox-cli/tests/behavioral_stdout_interp.rs`. Only #2 (doc-drift) remains, and it is not a coverage gap. The lowering match was also made **exhaustive** (`hir/lower/mod.rs`): a new un-lowered `Decl` variant is now a compile error, not a silent runtime warning.

| # | Pattern | Coverage | Evidence |
|---|---------|----------|----------|
| 1 | Silent-drop catch-all match arms | ✅ **DONE** | `semcov_struct_pipeline_tests`: top-level `let`→`Decl::Const` must lower into `hir.consts`, not the `legacy_ast_nodes` catch-all; name, value, type-annotation and multi-binding survival all pinned. |
| 2 | Decorator cliff (keyword recognized, arg ignored) | **NONE** (deferred) | Investigation found this is doc-vs-parser **drift**, not a silent HIR drop: `@deprecated("reason")` is documented but does not parse (`Expected fn, found (`). Better fixed as a doc/parser correction than a coverage test. |
| 3 | Context-dependent silent drop | ✅ **DONE** | `semcov_struct_pipeline_tests`: `@pure` must survive identically on a free fn (→`hir.functions`) and an `@example`-wrapped fn (→`hir.examples`), asserted as a parity relationship. (Investigation falsified 5 drop hypotheses — pipeline is robust here; this is a pin-current guard.) |
| 4 | Dead emitters (value produced, never consumed) | ✅ **DONE** (and *disproven* as dead) | `decl_lowering_test`: investigation found `hir.lower_warnings` is NOT dead — `typecheck_hir_module` drains it into coded diagnostics. The test pins that producer→consumer wiring (RED if the drain loop is deleted). Real follow-ups surfaced: `@traced` IS a uniformly dead decorator (no `HirFn` field consumes it); make `hir/lower/mod.rs` match exhaustive so a new un-lowered variant is a compile error. |
| 5 | Half-wired `when {}` blocks | ✅ **DONE** | `semcov_struct_pipeline_tests`: a `when src { fetching… empty… error e… ok x… }` lowers to `HirExpr::AsyncView` with all four arms surviving (`missing_arms()` empty) and bindings intact. |
| 6 | Structural-only goldens (runtime output unasserted) | ✅ **DONE** | `behavioral_stdout_interp.rs`: runs a real `.vox` program via `vox run --mode interp` and asserts the `print` builtin's token reaches **process stdout** — the first true observable-output assertion in the suite. Used `CARGO_BIN_EXE_vox` for a hermetic build. |
| 7 | Split-brain (duplicated logic diverged across crates) | ✅ **DONE** | `effort_pricing_parity.rs`: asserts the byte-identical `ModelRates::cost_usd` copies in `vox-effort-audit` and `vox-effort-route` agree across a direction-sensitive matrix; fails on any divergence. |

- **Headline bug — `top-level let → Decl::Const → catch-all → value vanishes`: now regression-guarded.** The fix already existed in `hir/lower/mod.rs` (Const is lowered into `hir.consts`); the test pins it so it cannot silently regress.
- **The 5,793 reached-but-unproven set is the largely-untouched core.** Every one of these symbols is executed by a test that asserts nothing about its behavior. Converting them to proven is the actual remaining initiative; patterns #1/#5/#7 have made a first dent in the structural slice.

**Honest 2-sentence verdict:** The semcov suite is broad and well-written but almost entirely targets **leaf utility functions** (tokenizers, identifier validators, span merges, redaction, cost/quota math, serde round-trips, formatter idempotence) — exactly the "useless touch" surface, not the structural wiring gaps. Of the 7 structural patterns, **6 have NO dedicated coverage and 1 (split-brain) has only a single weak intra-crate case**, and the named headline regression (`top-level let → Decl::Const → catch-all`) is **completely untested**.

## True remaining scope (by the numbers)

| Metric | Count | % of symbols |
|---|---:|---:|
| Production symbols (defs) | 19,445 | 100% |
| Proven (assertion-backed) | 3,088 | 15.9% |
| Reached-but-unproven (zero asserted behavior) | **5,793** | 29.8% |
| Crates with reached > 0 | 60 | — |

> **Reproducible figure (2026-06-15).** Supersedes the earlier directional 7,950.
> Produced from a real full-workspace `llvm-cov` run (**1.19M lines executed**),
> reproduced after fixing the `peft-rs` candle-0.10 compile error (the exit-101
> blocker) and merging profraws via `llvm-profdata -f file-list` (Windows
> arg-limit dodge). CI-gating still TODO.

**6 highest-leverage target crates** (largest reached-but-unproven, where proving symbols buys the most):

| Crate | Reached | Proven | Reached-not-proven |
|---|---:|---:|---:|
| vox-compiler | 947 | 392 | **699** |
| vox-orchestrator | 783 | 641 | **666** |
| vox-code-audit | 758 | 343 | **571** |
| vox-publisher | 487 | 112 | **419** |
| vox-codegen | 426 | 80 | **358** |
| vox-populi | 383 | 156 | **330** |

These 6 crates account for **~3,043** of the 5,793 reached-but-unproven symbols (~53%). `vox-codegen` (358 unproven, only 80 proven) is the worst proven-ratio of the group and is where patterns #2/#5/#6 live — the best place to attack structure and volume at once.

## Prioritized next actions

**Done (2026-06-15):**
- ✅ **Reach reproducibility unblocked** — fixed the `peft-rs` candle-0.10 exit-101 error; reach is now a real **5,793** from a 1.19M-line-execution run (profraw batch-merge).
- ✅ **Structural patterns #1, #3, #4, #5, #6, #7 covered** — headline `let`→`Decl::Const` survival, `@pure` context parity, `lower_warnings` producer→consumer wiring, `when{}` four-arm survival, **stdout behavioral golden**, and cross-crate `cost_usd` split-brain parity.
- ✅ **Lowering match made exhaustive** — new un-lowered `Decl` variants are now compile errors.

Remaining, in priority order:
1. **Land the reach run as a CI ratchet — DECISION-GATED.** CI already runs `cargo llvm-cov report --lcov` on Linux (no chunking needed). The blocker is **graph provenance**: `ingest_reaches.py` needs the Phase-1/2 `proves`-edge graph, which is ~109 MB, gitignored, and **not reproducible in CI** (Phase 2 is throttled LLM extraction). The only path is committing a **pruned snapshot** (id/label/source_file/source_location/file_type/_origin + `proves` links) — measured at **~32 MB raw (~5–6 MB gzipped)**. That blob commit **plus** editing `.github/workflows` are hard-to-reverse, repo-affecting changes that need explicit sign-off. *Recommendation:* gzip-commit the pruned snapshot under `contracts/reports/`, add `reached_not_proven` to `semantic-coverage.v1.json` as the baseline, and add a Linux CI step `lcov × snapshot → ingest → fail if rises`.
2. **Pattern #2 (`@deprecated("reason")`) doc-drift** — small: either make the parser accept the arg or fix the docs; not a coverage gap.
3. **Three real follow-ups surfaced by the design passes** (small, high-signal): `@traced` is a *uniformly dead* decorator (set on `FnDecl`, no `HirFn` field consumes it — decide: wire it or remove the dead set path); `@pure`-before-`@example`/`@test` is a parse error (decorator-order asymmetry); both warrant their own tickets.
4. **Grind the reached-but-unproven set** by leverage: `vox-codegen` (358, worst ratio) → `vox-compiler` (699) → `vox-orchestrator` (666) → `vox-code-audit` (571).
5. **Audit and downgrade the weak-test tail** — replace `assert_ne!`-on-derived-discriminant and self-literal tests, add known-answer vectors for every hash (not just SHA3), correct overstated `// Catches:` comments.
6. **Wire `catch_all_swallow` / `cross_crate_dup` into the gate with fixtures** — converts the structural detectors from "detectable" to "regression-guarded."