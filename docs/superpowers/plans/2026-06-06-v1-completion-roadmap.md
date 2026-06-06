# Vox v1.0 Completion Roadmap — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement each phase task-by-task. Steps use checkbox (`- [ ]`) syntax. This is a **master plan**: Phases 0–1 are fully task-decomposed (TDD); Phases 2–5 are scoped task-lists that each spawn a detailed sub-plan when picked up (per the writing-plans scope rule — one working subsystem per plan).

**Goal:** Drive Vox to a v1.0 where `cargo run -p vox-cli -- audit --gate all --strict-block-ga` exits 0 — i.e. every Foundation (CR-F), Distribution (CR-K), GUI (CR-U), and measurable Product (CR-A/E/D/L) gate is *built, executable, and green* — with the 4 external-infra gates (CR-P1/P2/P3, CR-E3) explicitly carved out as manual/post-GA.

**Architecture:** Every criterion becomes a **registered `vox-audit` gate subcommand** that writes a JSON artifact under `contracts/reports/<gate>/<UTC>.json` and reports `{observed, target, met}`. The **gate framework** (Phase 0) is the keystone — `CR-META` (the criteria-doc lint) and `CR-F0` (foundation-first ordering) make the whole thing self-verifying. Then the gates are filled in tier order, each TDD-built (failing gate test → implement subcommand → green). The single largest *code* chunk is **CR-F2** (codegen-rust arm parity), which is compiler work, not gate work.

**Tech Stack:** Rust (`vox-audit`, `vox-arch-check`, `vox-codegen`, `vox-compiler`), `cargo test`/`nextest`, GitHub Actions, Tauri 2 + pnpm/Playwright (GUI), `cargo publish`/`cargo-semver-checks` (distribution).

---

## 1. Current-state map (audited against `main`, 2026-06-06)

| Tier | Built | Partial | Unbuilt | Notes |
|---|---|---|---|---|
| **Gate framework** | 9 CR-L gates + 1 tooling gate registered | — | **CR-META lint, CR-F0 ordering** | `vox-audit/src/lib.rs` `CrlGate` enum has zero CR-F/K/U variants. 19 of 29 gates don't exist. |
| **CR-F Foundation** | CR-F1 harness landed (10/75 goldens) | CR-F1, CR-F4 | CR-F0, CR-F3, CR-F5, CR-F6 | No CR-F gate is registered in `vox audit`. |
| **CR-F2 (codegen-rust)** | interp 10/10; codegen-rust **3/10** | the arm | 7 bug-class backlog + `golden_arm_parity_test` + codegen-ts (unmeasured) | Largest code chunk. See [`cr-f2-arm-parity-findings-2026.md`](../../src/architecture/cr-f2-arm-parity-findings-2026.md). |
| **CR-K Distribution** | #166 made 3 crates `cargo publish --dry-run`-clean | CR-K1, CR-K3 | K2,K4,K5,K6,K7 + `_public.toml` | No publish gate, no semver policy, no publish workflow; `voxup` install still a stub. |
| **CR-U GUI** | vox-gui shell + surface registry exist | — | all 6 gates + tauri installer config + e2e-in-CI + signing | `vox ci gui-surface-registry` runs in zero workflows. |
| **CR-P/A/E/D/L Product** | CR-L0..L8 measured (honest-plan §7); CR-A2/E1 claimed met | CR-A2, CR-E1 | CR-A1, CR-A4, CR-E2, CR-D3 gates | CR-P1/P2/P3, CR-E3 are **external-infra** — not auto-satisfiable. |

**Honest framing:** the criteria doc on `main` is a correct *target spec* with an explicit "TARGET spec, not current state" banner. This plan turns it into reality. "v1.0" here means the *measurable* gates are green; the 4 infra gates ship as a documented manual checklist.

## 2. Effort, critical path, feasible timeline

Rough autonomous effort (excludes the 4 external-infra gates):

| Phase | Scope | Effort |
|---|---|---|
| 0 | Gate framework: `vox-audit` gate-subcommand pattern + CR-META lint + CR-F0 ordering | ~20 h |
| 1 | **CR-F2 codegen-rust to parity** + `golden_arm_parity_test` gate (codegen-ts deferred to v1.1) | ~60–80 h |
| 2 | CR-F foundation gates: F1 (→100%), F3 (spec-coverage), F4, F5, F6 | ~50 h |
| 3 | CR-K distribution: `_public.toml` + K1–K7 gates + `voxup` real install | ~90 h |
| 4 | CR-U GUI: U1–U6 gates + tauri installer + e2e-in-CI + signing | ~48 h |
| 5 | Product gates: A1, A2, A4, E1, E2, D3 (+ external-infra checklist) | ~40 h |
| **Total** | | **~310–330 h** (~8–10 focused weeks solo; ~5 weeks with 2–3 parallel streams) |

**Critical path:** Phase 0 (framework) → Phase 1 (CR-F2 is the long pole) → CR-F0 can only go green once F1–F6 exist. Phases 3/4/5 are **independent** of Phase 1 and can run in parallel streams. **Keystone:** Phase 0's gate-subcommand pattern — every later task reuses it, so getting it clean first multiplies throughput.

**Feasible sequencing:** do Phase 0, then split into three parallel streams — (A) CR-F2 codegen + CR-F gates, (B) CR-K distribution, (C) CR-U GUI — converging on Phase 5 + CR-F0 last.

---

## 3. The reusable TDD pattern for a `vox-audit` gate (read first)

Every CR-F/K/U/A/E/D gate is the same shape. Internalize this; Phases 2–5 are this pattern repeated.

```
1. Add a variant to the gate enum (vox-audit/src/lib.rs).
2. Write a FAILING unit test: construct the gate, run it against a fixture
   with a KNOWN violation, assert it reports met=false naming the violation.
3. Implement the subcommand: scan/measure, write contracts/reports/<gate>/<UTC>.json
   with {observed, target, met, details}, return non-zero when met=false.
4. Run the test → green.
5. Write a SECOND test: clean fixture → met=true.
6. Register in CrlGate::all() and the --gate <name> dispatch.
7. Commit.
```

Gates that are *measurements over the real repo* (F5 convergence, F6 regression-budget, A1 complexity, D3 docs) test against **fixture inputs** (mock git log, temp files), never the live repo, so they're deterministic.

---

## Phase 0 — Gate framework (keystone, ~20 h)

**Files:**
- Modify: `crates/vox-audit/src/lib.rs` (gate enum + ordering + roll-up)
- Create: `crates/vox-audit/src/subcommands/foundation/mod.rs`
- Create: `crates/vox-arch-check/src/criteria_format_check.rs`
- Test: `crates/vox-audit/tests/gate_ordering.rs`, `crates/vox-arch-check/tests/criteria_format.rs`

### Task 0.1: Foundation gate enum + tier ordering (CR-F0 scaffolding)

- [ ] **Step 1 — failing test** `crates/vox-audit/tests/gate_ordering.rs`:
```rust
use vox_audit::{CrlGate, gate_tier};
#[test]
fn foundation_gates_sort_before_all_others() {
    let order: Vec<_> = CrlGate::all().collect();
    let last_f = order.iter().rposition(|g| gate_tier(g) == "foundation").unwrap();
    let first_non_f = order.iter().position(|g| gate_tier(g) != "foundation").unwrap();
    assert!(last_f < first_non_f, "all CR-F gates must precede non-foundation gates");
}
```
- [ ] **Step 2 — run, expect FAIL** `cargo test -p vox-audit --test gate_ordering` (CrlGate has no foundation variants / `gate_tier` undefined).
- [ ] **Step 3 — implement**: add `F0..F6` (and placeholder `K1..K7`, `U1..U6`, `A1/A2/A4/E2/D3`) variants to `CrlGate`; add `pub fn gate_tier(g: &CrlGate) -> &'static str`; make `CrlGate::all()` yield foundation → distribution → gui → product.
- [ ] **Step 4 — run, expect PASS**.
- [ ] **Step 5 — commit** `feat(vox-audit): foundation-first gate ordering scaffold (CR-F0)`.

### Task 0.2: `blocked_by_foundation` roll-up (CR-F0 behavior)

- [ ] **Step 1 — failing test**: run the umbrella with a mocked foundation gate returning `met=false`; assert every downstream gate in the roll-up JSON carries `blocked_by_foundation=true` and the snapshot exit code is non-zero.
- [ ] **Step 2 — run, expect FAIL.**
- [ ] **Step 3 — implement**: in the `--gate all` roll-up, after evaluating foundation gates, if any `met==false`, short-circuit downstream gates to `{met:false, blocked_by_foundation:true}`.
- [ ] **Step 4 — PASS. Step 5 — commit.**

### Task 0.3: CR-META criteria-format lint (`vox-arch-check`)

- [ ] **Step 1 — failing test** `crates/vox-arch-check/tests/criteria_format.rs`: feed a fixture criteria doc with a `[CR-X]` block missing `if_failing`; assert the lint returns an error naming `CR-X`.
- [ ] **Step 2 — run, expect FAIL** (`--lint criteria-format` mode doesn't exist).
- [ ] **Step 3 — implement** `criteria_format_check.rs`: parse every `[CR-*]` block in `docs/src/architecture/v1-release-criteria.md`, assert each has a fenced `verify_cmd`, an `artifact_path` resolving to a registered gate or test target, and a non-empty `if_failing`. Wire `--lint criteria-format` into `vox-arch-check`'s arg parser.
- [ ] **Step 4 — PASS. Step 5** — add a second test: a well-formed block passes. **Commit.**
- [ ] **Step 6** — add the lint to the `vox ci`/pre-push doc-pipeline so the criteria doc self-polices.

**Phase-0 exit:** `cargo run -p vox-cli -- audit --gate all --strict-block-ga` runs, lists foundation gates first, marks downstream `blocked_by_foundation`, and the criteria doc passes its own format lint. (All gates still report `incomplete` until Phases 1–5 fill them — that's expected.)

---

## Phase 1 — CR-F2 codegen-rust arm parity (long pole, ~60–80 h)

This is **compiler work**, not gate-building. Reference: [`cr-f2-arm-parity-findings-2026.md`](../../src/architecture/cr-f2-arm-parity-findings-2026.md). Current: `--mode script` (codegen-rust, now default-on) compiles **3/10** `main()`-goldens (`noop`, `while_loop_algorithms`, `decimal_math`). Two prerequisite fixes already landed (tail-expr return, `f64` literal suffix — PR #169).

### Task 1.0: Stand up the ratcheting parity gate FIRST (so progress is measured)

- [ ] **Step 1 — failing test** `crates/vox-integration-tests/tests/golden_arm_parity_test.rs`: reuse `collect_golden_vox` + EXPECT parsing from `golden_behavioral_gate.rs`; for each golden with `main()` + `// EXPECT`, run `vox run --mode interp` and `vox run --mode script`, normalize stdout (strip the INFO tracing line the script lane prints), assert byte-equal. Maintain a **non-growing allowlist** `contracts/eval/arm-parity-allowlist.txt` of currently-diverging goldens; assert the live divergence set ⊆ allowlist AND `allowlist.len() <= committed_baseline`.
- [ ] **Step 2 — run** with `baseline=7` (the 7 currently-failing); PASS at 3/10.
- [ ] **Step 3 — register** as `vox audit --gate arm-parity` (Phase-0 pattern), artifact `contracts/reports/arm-parity/<UTC>.json` with the per-golden table.
- [ ] **Step 4 — commit.** Now every codegen fix below *ratchets the baseline down* — the gate enforces no regression.

### Tasks 1.1–1.7: Per-bug-class codegen fixes (TDD loop, ~1–5 h each)

For each failing golden, the loop is identical (do them lowest-effort first per the findings doc):

- [ ] **Step 1** — `vox run --mode script examples/golden/<g>.vox` → capture the Rust compile error (or runtime panic).
- [ ] **Step 2** — locate the emitter site in `crates/vox-codegen/src/codegen_rust/emit/` (the findings doc names likely sites: `stmt_expr.rs` StringLit/Binary arms, `stmt_expr_tail.rs` Lambda/For arms, `hir/lower/json_as.rs` for the `Json` type alias).
- [ ] **Step 3** — write/extend a `vox-codegen` unit test asserting the emitted snippet for that construct (or add the golden's EXPECT and tighten the arm-parity allowlist by one).
- [ ] **Step 4** — fix the emitter to the minimal change; rebuild `vox`; re-run `--mode script` → matches interp.
- [ ] **Step 5** — **decrement the arm-parity baseline** by 1; run `cargo test -p vox-codegen` (no regression to App/web emit) + the parity gate.
- [ ] **Step 6** — commit `fix(codegen-rust): <construct> (CR-F2 N/10)`.

Known remaining classes (verify against live state — some may have shifted): `adt_multi_field` runtime panic; `range_and_indexing` match-arm types; `regex_free_functions` `\w` string-escape; `json_as_typed` `Error` tuple-variant E0531 + missing `type Json = serde_json::Value`; `closures_hof` `Fn` boxing/trait-object; `string_interpolation` interpolant `String` coercion. Each is a self-contained PR.

**Phase-1 exit:** codegen-rust parity ≥ 8/10 (allow up to 2 genuinely-codegen-only goldens documented in the allowlist with a reason), `arm-parity` gate green at the ratcheted baseline. **codegen-ts parity is explicitly deferred to v1.1** (the criteria doc's "all three arms" is descoped to two for v1.0 GA — record this as a steer in the criteria doc).

---

## Phase 2 — CR-F foundation gates (~50 h)

Each is the Phase-0 gate pattern. Sequence: **F1 → F3 → F4 → F5/F6 → (F0 closes the tier).**

- [ ] **CR-F1 → 100% (~12 h):** write the ratchet test in `golden_behavioral_gate.rs` asserting `(#EXPECT ∪ #@test) == #top-level-goldens`; it fails at 36/75; author EXPECT/`@test` for the ~46 uncovered goldens until green; register as `vox audit --gate behavioral-goldens`.
- [ ] **CR-F3 spec-coverage (~25 h, keystone of the tier):** create `contracts/spec/language-surface-coverage.v1.yaml` (one row per grammar production/decorator/builtin from `crates/vox-compiler/src/{grammar,builtin_registry}`, each with `arm-support: {interp, script, ts}` + a linking golden); implement `vox audit --gate spec-coverage` (fail if any row uncovered/incomplete-arm); add a `vox-arch-check` rule failing if a new production lands without a row. TDD: empty checklist → gate fails; 10-row checklist mapped to goldens → green; expand.
- [ ] **CR-F4 no-incomplete-arms (~8 h):** convert the codegen-ts db-op runtime `throw VoxRuntimeError("UnsupportedOnPlatform")` (`codegen_ts/hir_emit/mod.rs:1051`) into a codegen-time diagnostic (`vox/codegen/db-unsupported-here`); add `local-import-unsupported-here`; implement `vox audit --gate no-incomplete-arms` scanning compiler+codegen for reachable `todo!`/`unimplemented!`/runtime-`Unsupported` on constructs marked supported in CR-F3. TDD: lower a typecheck-passing on-device `db.get()` under codegen-ts → assert codegen-time diagnostic, not runtime panic.
- [ ] **CR-F5 core-convergence (~9 h):** `vox audit --gate core-convergence` over fixture git-log windows (decline 3 windows + final ≤25% of peak; release-commit body has no first-time-semantics). TDD with mocked window arrays (declining → pass; rising → fail).
- [ ] **CR-F6 regression-budget (~6 h):** `vox audit --gate regression-budget` counting `// vox:skip` + stub/mock returns in compiler/codegen/goldens vs a committed baseline (non-increasing). TDD: inject a `vox:skip` into a temp file → count ≥1; remove → 0.
- [ ] **CR-F0 close:** with F1–F6 registered, verify the foundation-first ordering + `blocked_by_foundation` roll-up (Phase 0) now has real gates to order.

---

## Phase 3 — CR-K distribution (~90 h, independent stream)

> Spawn a dedicated sub-plan: `docs/superpowers/plans/<date>-cr-k-distribution.md`. Scoped tasks:

- [ ] **CR-K1 (~14 h):** create `crates/_public.toml` (recommended set: `vox-crypto`, `vox-jsonschema-util`, `vox-telemetry`, `vox-journal`, `vox-git`, `vox-grammar-export`, `vox-db-types`, `vox-db`); add `version` reqs to intra-workspace path deps; strip `workspace-hack` from the public set (extend #166's hakari pattern); implement `vox audit --gate crate-publish` running `cargo publish --dry-run -p <N>` per crate. TDD: gate calls cargo once per listed crate; a missing-version dep → met=false.
- [ ] **CR-K2 (~6 h):** `vox audit --gate public-set-metadata` — each public crate has `description`/`license`/`repository`/`readme`.
- [ ] **CR-K3 (~6 h):** `vox audit --gate publish-dep-hygiene` — no public crate depends on a `publish=false` crate; intra-deps versioned.
- [ ] **CR-K4 (~16 h):** make `voxup install default` download a real artifact + verify SHA-256 (replace the `Vox Proxy Wrapper` stub in `crates/voxup/src/install.rs`); CI gate asserts installed `vox --version` and `! grep "Vox Proxy Wrapper"`.
- [ ] **CR-K5 (~6 h):** author `docs/src/contributors/semver-policy.md`; `cargo semver-checks check-release` gate over the public set.
- [ ] **CR-K6 (~8 h):** `.github/workflows/publish-crates.yml` (on `v*` tag, dry-run gate → publish in reverse-topo order); `vox audit --gate publish-workflow` validates it.
- [ ] **CR-K7 (~12 h):** land `vox-checksum-manifest` + `vox-release-artifacts` (promote from `[planned]` in `layers.toml`); `vox audit --gate release-provenance` emits + verifies a SHA-256 manifest per asset.

---

## Phase 4 — CR-U GUI (~48 h, independent stream)

> Spawn a dedicated sub-plan: `docs/superpowers/plans/<date>-cr-u-gui.md`. Scoped tasks:

- [ ] **CR-U1 (~6 h):** run `vox ci gui-surface-registry` as a **required** CI job (it exists as code but runs in zero workflows); gate bites on drift.
- [ ] **CR-U2 (~12 h):** Playwright suite over the *real* `crates/vox-gui/ui` rendering each `live_backend`/`curated_decorator` surface (mocked Tauri IPC), asserting non-empty/non-error; assert `count(tested) == count(non-none registry entries)`.
- [ ] **CR-U3 (~6 h):** run vox-gui vitest + e2e as required CI gates (currently neither runs in CI).
- [ ] **CR-U4 (~8 h):** `tauri.conf.json` `bundle.active`/`targets` + full icon set (`.ico`/`.icns`/PNGs); CI dry-run produces a non-empty installer.
- [ ] **CR-U5 (~8 h):** `release-gui.yml` builds the `externalBin` sidecars before bundling + signs/verifies with a *real* path (fix the nonexistent `src-tauri/` path).
- [ ] **CR-U6 (~8 h):** launch+IPC smoke test invoking real Tauri handlers (`get_build_info`) headless.

---

## Phase 5 — Product gates + external-infra carve-out (~40 h)

- [ ] **CR-A2 (~4 h), CR-E1 (~3 h):** claimed-met — just build the gate subcommands that re-verify (FFI non-null schema scan; interp cold-start profile) and emit artifacts.
- [ ] **CR-A1 (~6 h):** cyclomatic-complexity gate over `vox-compiler/src/lower/`; refactor the 14 functions over budget.
- [ ] **CR-A4 (~4 h):** orchestration-contract lifecycle-metadata gate.
- [ ] **CR-E2 (~6 h):** Marquee bundle-size gate (≤800 KB gzip) wired into `vox build`.
- [ ] **CR-D3 (~14 h):** CLI-doc-coverage gate (currently 8/68); author the ~60 missing `.vox` examples.
- [ ] **External-infra (~3 h, docs only):** CR-P1/P2/P3 + CR-E3 — add a manual-validation checklist to the honest-completion plan §5; mark them `external_infra: true` in the gate registry so `--strict-block-ga` excludes them from the autonomous GA bar (GA = all non-external gates green; infra gates validated manually before tagging).

---

## 4. Self-review notes

- **Spec coverage:** every CR-* in `v1-release-criteria.md` maps to a phase task above; the 4 external-infra gates are explicitly carved out, and codegen-ts (CR-F2 "all three arms") is descoped to v1.1 — both decisions need a one-line maintainer steer recorded in the criteria doc.
- **Dependencies honored:** Phase 0 framework precedes all gates; CR-F0 closes after F1–F6; CR-K2/3/5/6/7 depend on CR-K1's `_public.toml`; CR-U5/6 depend on CR-U4; CR-F4 depends on CR-F3's supported-construct list.
- **TDD throughout:** every gate is built failing-test-first; every codegen fix is golden-driven (interp==script). The two exceptions are inherently non-TDD: the external-infra gates (manual) and the `_public.toml`/icon-asset authoring (data, not logic).

## 5. Execution handoff

**Recommended next action:** start **Phase 0** (the gate framework) — it's the keystone every later task reuses, it's small (~20 h), and it makes the criteria doc self-verifying. Then fork three parallel streams (CR-F2+CR-F / CR-K / CR-U). The GA gate is the single command `vox audit --gate all --strict-block-ga` exiting 0 with the external-infra carve-out.
