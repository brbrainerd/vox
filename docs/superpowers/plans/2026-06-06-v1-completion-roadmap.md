---
title: Vox v1.0 Completion Roadmap
description: Phased, TDD, gate-framework-first plan to drive every v1.0 release criterion (Foundation, Distribution, GUI, Product, external-infra) to a built, executable, green gate.
category: architecture
---

# Vox v1.0 Completion Roadmap — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans to implement each phase task-by-task. Steps use checkbox (`- [ ]`) syntax. This is a **master plan**. Because of context limits, **each phase is scoped into its own detailed sub-plan when it is picked up** (per the writing-plans scope rule — one working subsystem per plan). Phase 0 is already fully decomposed in its own file (see below); Phases 1–6 are scoped task-lists here and graduate to detail-plans on entry.

**Goal:** Drive Vox to a v1.0 where `vox audit --gate all --strict-block-ga` exits `0` — i.e. **every** Foundation (CR-F), Distribution (CR-K), GUI (CR-U), and Product (CR-A/E/D/L/P) gate is *built, executable, and green*. There is **no manual-checklist carve-out**: the four `external_infra` criteria (CR-P1/P2/P3, CR-E3) get real harnesses + gates built in dedicated PRs (Phase 6); their only residual is the operational *run* leg (cloud creds / elapsed soak time / a GPU), which is a documented release-time step, not a descoped item. **All three compiler arms** (`--mode interp`, `--mode script` codegen-rust, codegen-ts under Node) reach byte-parity — codegen-ts is **in scope for v1.0**, not deferred.

**Architecture — grounded in the real `vox-audit` code (audited 2026-06-06):** There are two existing gate mechanisms; this plan unifies them behind one GA spine.
1. **Registry gates** — `crates/vox-audit/src/lib.rs` `CrlGate` enum + `Subcommand` trait + `registry()`. Each `run()` returns a `RunOutcome { report: AuditReport, exit_code: ExitCode }` and writes `contracts/reports/<thing>/<date>.json`. `retirement.rs` is the template for a **structural** (non-LLM) gate: `overall_pass_rate` 1.0/0.0 + `Threshold { target, met }`. Reachable as `vox audit --gate <name>` (vox-cli `audit.rs` routes `--gate` → `vox_audit::registry()`).
2. **Standalone binaries** — `crates/vox-audit/src/bin/cr-{a1,a2,a4,d3,e1,e2,p1,p2}.rs`. Self-contained `fn main()`, hand-rolled `serde_json::json!` artifact to `contracts/reports/arch|perf/<cr>/<UTC>.json`, `process::exit(1)` on fail. **Not in `registry()`** — so `vox audit --gate all` cannot currently see them.

The **keystone (Phase 0)** is to make the registry the *single GA spine*: add a `Tier` to `CrlGate`, foundation-first ordering (**CR-F0**), a `--gate all --strict-block-ga` roll-up writing `contracts/reports/_snapshot/<UTC>.json`, fold the standalone CR-A/D/E/P binaries into the registry so GA actually sees them, prove the "add a gate" pattern by wiring the already-landed CR-F1 behavioral harness as the first registered foundation gate, and make the criteria doc self-police via the **CR-META** lint. Then gates are filled in tier order, each TDD-built.

**Tech Stack:** Rust (`vox-audit`, `vox-arch-check`, `vox-codegen`, `vox-compiler`, `vox-cli`), `cargo test`/nextest, Node (codegen-ts execution harness), GitHub Actions, Tauri 2 + pnpm/Playwright (GUI), `cargo publish`/`cargo-semver-checks` (distribution).

---

## 1. Current-state map (audited against `main`, 2026-06-06)

| Tier | Built | Partial | Unbuilt | Notes |
|---|---|---|---|---|
| **Gate framework** | `CrlGate` registry (9 CR-L + 1 tooling), `Subcommand` trait, `AuditReport`, `vox audit --gate <name>` | standalone CR-A/D/E/P binaries exist but are **outside** the registry | `Tier`/`tier()`, `--gate all`, `--strict-block-ga`, `_snapshot` roll-up, CR-META lint, `blocked_by_foundation` | `lib.rs` has zero CR-F/K/U variants. No GA aggregation exists. |
| **CR-F Foundation** | CR-F1 harness landed (`golden_behavioral_gate.rs`, ~10/75 goldens) — **as a test, not a registered gate** | CR-F1, CR-F4 | CR-F0, CR-F2 gate, CR-F3, CR-F5, CR-F6 | No CR-F gate is in `registry()`. |
| **CR-F2 (3-arm parity)** | interp 10/10; codegen-rust **3/10** | codegen-rust arm | 7-class codegen-rust backlog + **codegen-ts arm + Node exec harness (unmeasured)** + `golden_arm_parity_test` | Largest code chunk. See [`cr-f2-arm-parity-findings-2026.md`](../../src/architecture/cr-f2-arm-parity-findings-2026.md). |
| **CR-K Distribution** | #166 made 3 crates `cargo publish --dry-run`-clean | — | K1–K7 gates + `_public.toml`; `voxup` install still a stub | No publish gate, no semver policy, no publish workflow. |
| **CR-U GUI** | vox-gui shell + surface registry exist | — | all 6 gates + tauri installer config + e2e-in-CI + signing | `vox ci gui-surface-registry` runs in zero workflows. |
| **CR-A/E/D Product** | CR-A1/A2/A4/D3/E1/E2 standalone binaries exist (some met) | — | fold into registry; CR-D3 author ~60 examples; CR-E2 bundle gate | Binaries not GA-aggregated. |
| **CR-P/E3 external_infra** | — | CR-P3 overlaps CR-L7 deploy legs | CR-P1 marquee-deploy automation, CR-P2 uptime soak, CR-E3 training-parity — **harnesses + gates** | Built in Phase 6 (separate PRs); live-run leg is operational. |

## 2. Effort, critical path, feasible timeline

| Phase | Scope | Effort |
|---|---|---|
| 0 | **Gate framework** (keystone): `Tier` + foundation-first ordering (CR-F0) + `--gate all --strict-block-ga` roll-up + fold standalone binaries into registry + wire CR-F1 as first real gate + CR-META lint | ~24 h |
| 1 | **CR-F2 all-three-arm parity**: codegen-rust to parity (~60–80 h) **+ codegen-ts arm + Node exec harness to parity (~45–60 h)** + `golden_arm_parity_test` gate | ~110–140 h |
| 2 | CR-F foundation gates: F1 (→100%), F3 (spec-coverage), F4, F5, F6 | ~50 h |
| 3 | CR-K distribution: `_public.toml` + K1–K7 gates + `voxup` real install | ~90 h |
| 4 | CR-U GUI: U1–U6 gates + tauri installer + e2e-in-CI + signing | ~48 h |
| 5 | Product gates (fold + finish): A1, A2, A4, E1, E2, D3 | ~40 h |
| 6 | **External-infra delivery** (CR-P1/P2/P3, CR-E3) — real automation + harness + gate per criterion, each its own PR | ~70–90 h |
| **Total** | | **~430–480 h** (~11–14 focused weeks solo; ~6–7 weeks with 3 parallel streams after Phase 0) |

**Critical path:** Phase 0 (framework) → Phase 1 (CR-F2 is the long pole; codegen-ts roughly doubles it vs the two-arm version) → CR-F0 closes once F1–F6 exist. Phases 3/4/5/6 are **independent** of Phase 1 and run as parallel streams. **Keystone:** Phase 0's "add a gate" pattern — every later task reuses it, so getting it clean first multiplies throughput.

**Feasible sequencing:** Phase 0, then fork four streams — (A) CR-F2 codegen rust+ts + CR-F gates, (B) CR-K distribution, (C) CR-U GUI, (D) Phase 6 external-infra harnesses — converging on CR-F0 closing the foundation tier last.

---

## 3. The reusable TDD pattern for a registry gate (read first)

Every CR-F/K/U/A/E/D/P gate is the **`retirement.rs` shape**. Internalize this; Phases 2–6 are this pattern repeated. (Phase 0 proves it end-to-end by wiring CR-F1.)

```
1. Add a variant to `CrlGate` (crates/vox-audit/src/lib.rs) + its `thing_name()` + `tier()` arm + `all()` entry.
2. Create crates/vox-audit/src/subcommands/<gate>.rs implementing `Subcommand`:
     fn gate() -> CrlGate; fn description() -> &'static str;
     fn run(&self, args: &CommonArgs) -> RunOutcome  // measure → AuditReport::complete + Threshold{target,met} → ExitCode
   Honor args.dry_run; never panic on missing inputs (return AuditReport::infra_error + ExitCode::InfrastructureError).
3. Write a FAILING unit test (in that file's `#[cfg(test)]`): run against a fixture with a KNOWN violation; assert ExitCode::BarMissed + threshold.met == false naming the violation.
4. Implement run() → green.
5. Write a SECOND test: clean fixture → ExitCode::Ok + threshold.met == true.
6. Register `Box::new(<Gate>)` in `registry()` and add the clap `CliCommand` arm in main.rs (+ `to_gate_name`).
7. The registry round-trip tests in lib.rs (`every_gate_has_a_subcommand_in_registry`, `registry_size_matches_gate_count`) enforce wiring — bump the size assertion.
8. Commit.
```

Gates that *measure the real repo* (F5 convergence, F6 regression-budget, A1 complexity, D3 docs) test against **fixture inputs** (mock git log, temp files), never the live repo, so they're deterministic. The standalone `bin/cr-a1.rs` scanner logic is reusable — Phase 5 lifts it into a `Subcommand`.

---

## Phase 0 — Gate framework (keystone, ~24 h) → **detailed plan: [`2026-06-06-phase0-gate-framework.md`](2026-06-06-phase0-gate-framework.md)**

Phase 0 is fully task-decomposed in its own file. Summary of what it delivers (all **real**, no new stubs — per the no-stubs rule, we wire one real gate rather than registering empty CR-F1–F6 placeholders):

1. **`Tier` + `CrlGate::tier()` + foundation-first ordering** (CR-F0 machinery) — `registry()`/`all()` yield `foundation → distribution → gui → product → tooling`; unit test asserts the invariant.
2. **`--gate all` + `--strict-block-ga`** roll-up in vox-cli `audit.rs` + `vox_audit::run_all` GA aggregation — writes `contracts/reports/_snapshot/<UTC>.json` with per-gate `{thing, tier, met, blocked_by_foundation}`; if any foundation gate is red, downstream rows are forced `met:false, blocked_by_foundation:true` and exit is non-zero. `external_infra` gates included (their built-but-unrun state reports honestly).
3. **Surface the standalone CR-A/D/E/P binaries in the GA snapshot** via a sibling-exe adapter (`product_binary_gates()`), so `--gate all` sees them without rewriting 8 binaries yet; the full `Subcommand` fold (lifting each scanner into the registry) lands in Phase 5.
4. **Wire the landed CR-F1 behavioral harness as the first registered foundation gate** (`behavioral-goldens`) — promotes `golden_behavioral_gate.rs`'s core to a library fn the `Subcommand` calls; gives CR-F0/strict-block-ga **real foundation data** and proves the whole pattern.
5. **CR-META criteria-format lint** — `cargo run -p vox-arch-check -- --lint criteria-format` parses every `[CR-*]` block in `v1-release-criteria.md`, asserts each has `verify_cmd` + an `artifact_path` resolving to a registered gate/test target + non-empty `if_failing`; wired into the pre-push doc-pipeline.

**Phase-0 exit:** `vox audit --gate all --strict-block-ga` runs, lists foundation gates first, marks downstream `blocked_by_foundation`, includes the folded CR-A/D/E/P gates and the real `behavioral-goldens` gate, and the criteria doc passes its own format lint.

---

## Phase 1 — CR-F2 all-three-arm parity (long pole, ~110–140 h)

Compiler work, not gate-building. Reference: [`cr-f2-arm-parity-findings-2026.md`](../../src/architecture/cr-f2-arm-parity-findings-2026.md). The criteria doc defines CR-F2 as three-arm (`{interp_out, script_out, ts_out, all_agree}`). **codegen-ts is in scope.**

### Task 1.0: Stand up the ratcheting 3-arm parity gate FIRST
- [ ] Create `crates/vox-integration-tests/tests/golden_arm_parity_test.rs`: reuse `collect_golden_vox` + EXPECT parsing from `golden_behavioral_gate.rs`; for each golden with `main()` + `// EXPECT`, run `vox run --mode interp`, `vox run --mode script`, **and** the codegen-ts arm via the Node harness (Task 1.A); normalize stdout; assert all three byte-equal. Maintain non-growing allowlists `contracts/eval/arm-parity-allowlist-{script,ts}.txt`; assert live divergence ⊆ allowlist AND `len <= committed_baseline`. Register as a registry gate `arm-parity` (Phase-0 pattern); artifact `contracts/reports/arm-parity/<UTC>.json`.

### Task 1.A: codegen-ts Node execution harness
- [ ] Build the harness that emits a golden via codegen-ts, runs it under Node (mocking platform IPC), and captures stdout — the missing piece the criteria doc's `if_failing` names ("for codegen-ts add a Node execution harness over emitted output"). This unblocks the `ts_out` column of Task 1.0.

### Tasks 1.1–1.N: Per-bug-class codegen fixes (TDD loop, ~1–5 h each, per arm)
For each failing golden × arm: reproduce the compile/runtime error → locate emitter site (`crates/vox-codegen/src/codegen_rust/emit/` or `crates/vox-codegen/src/codegen_ts/`) → write/extend a `vox-codegen` unit test asserting the emitted snippet → minimal fix → rebuild → decrement the arm's parity baseline → `cargo test -p vox-codegen` (no regression to App/web emit) + the parity gate → commit `fix(codegen-<arm>): <construct> (CR-F2 N/10)`. Known codegen-rust classes (verify live): `adt_multi_field` panic, `range_and_indexing` match-arm types, `regex_free_functions` `\w` escape, `json_as_typed` `Error` tuple-variant E0531 + missing `type Json`, `closures_hof` `Fn` boxing, `string_interpolation` coercion. codegen-ts classes are enumerated once Task 1.A makes them measurable.

**Phase-1 exit:** all three arms reach parity ≥ 8/10 on the EXPECT set (any residual divergence documented in the allowlist with a reason); `arm-parity` gate green at the ratcheted baseline. **No arm descoped.**

---

## Phase 2 — CR-F foundation gates (~50 h)

Each is the Phase-0 gate pattern. Sequence: **F1 → F3 → F4 → F5/F6 → (CR-F0 closes the tier).** Spawn detail-plan on entry.

- [ ] **CR-F1 → 100% (~12 h):** ratchet test asserting `(#EXPECT ∪ #@test) == #top-level-goldens`; author EXPECT/`@test` for the ~46 uncovered goldens until green. (Gate itself already registered in Phase 0.)
- [ ] **CR-F3 spec-coverage (~25 h, keystone of the tier):** `contracts/spec/language-surface-coverage.v1.yaml` (one row per grammar production/decorator/builtin from `crates/vox-compiler/src/{grammar,builtin_registry}`, each with `arm-support: {interp, script, ts}` + a linking golden); `spec-coverage` gate (fail on any uncovered/incomplete-arm row); `vox-arch-check` rule failing if a new production lands without a row.
- [ ] **CR-F4 no-incomplete-arms (~8 h):** convert codegen-ts db-op runtime throw (`codegen_ts/hir_emit/mod.rs`) + local-import into **codegen-time diagnostics** (`vox/codegen/db-unsupported-here`, `vox/codegen/local-import-unsupported-here`); `no-incomplete-arms` gate scanning compiler+codegen for reachable `todo!`/`unimplemented!`/runtime-`Unsupported` on CR-F3-supported constructs.
- [ ] **CR-F5 core-convergence (~9 h):** `core-convergence` gate over fixture git-log windows (decline 3 windows + final ≤25% of peak; release-commit body has no first-time-semantics). Mocked window arrays.
- [ ] **CR-F6 regression-budget (~6 h):** `regression-budget` gate counting `// vox:skip` + stub/mock returns vs a committed non-increasing baseline. Temp-file fixtures.
- [ ] **CR-F0 close:** with F1–F6 registered, verify foundation-first ordering + `blocked_by_foundation` roll-up over real gates.

---

## Phase 3 — CR-K distribution (~90 h, independent stream)

> Spawn `docs/superpowers/plans/<date>-cr-k-distribution.md`.

- [ ] **CR-K1 (~14 h):** `crates/_public.toml` (recommended set: `vox-crypto`, `vox-jsonschema-util`, `vox-telemetry`, `vox-journal`, `vox-git`, `vox-grammar-export`, `vox-db-types`, `vox-db`); add `version` to intra-workspace path deps; strip `workspace-hack` from the public set; `crate-publish` gate runs `cargo publish --dry-run -p <N>` per crate.
- [ ] **CR-K2 (~6 h):** `public-set-metadata` gate — each public crate has `description`/`license`/`repository`/`readme`.
- [ ] **CR-K3 (~6 h):** `publish-dep-hygiene` gate — no public crate depends on a `publish=false` crate; intra-deps versioned.
- [ ] **CR-K4 (~16 h):** make `voxup install default` download a real artifact + verify SHA-256 (replace the `Vox Proxy Wrapper` stub in `crates/voxup/src/install.rs`); gate asserts installed `vox --version` and `! grep "Vox Proxy Wrapper"`.
- [ ] **CR-K5 (~6 h):** author `docs/src/contributors/semver-policy.md`; `cargo semver-checks check-release` gate over the public set.
- [ ] **CR-K6 (~8 h):** `.github/workflows/publish-crates.yml` (on `v*`, dry-run gate → publish reverse-topo); `publish-workflow` gate validates it.
- [ ] **CR-K7 (~12 h):** promote the two `[planned]` crates in `layers.toml` (`vox-checksum-manifest`, `vox-release-artifacts`); `release-provenance` gate emits + verifies a SHA-256 manifest per asset.

---

## Phase 4 — CR-U GUI (~48 h, independent stream)

> Spawn `docs/superpowers/plans/<date>-cr-u-gui.md`.

- [ ] **CR-U1 (~6 h):** run `vox ci gui-surface-registry` as a **required** CI job; gate bites on drift.
- [ ] **CR-U2 (~12 h):** Playwright suite over the real `crates/vox-gui/ui` rendering each `live_backend`/`curated_decorator` surface (mocked Tauri IPC); assert `count(tested) == count(non-none registry entries)`.
- [ ] **CR-U3 (~6 h):** vox-gui vitest + e2e as required CI gates.
- [ ] **CR-U4 (~8 h):** `tauri.conf.json` `bundle.active`/`targets` + full icon set; CI dry-run produces a non-empty installer.
- [ ] **CR-U5 (~8 h):** `release-gui.yml` builds `externalBin` sidecars before bundling + signs/verifies with a real path (fix the nonexistent `src-tauri/` path).
- [ ] **CR-U6 (~8 h):** headless launch+IPC smoke invoking real Tauri handlers (`get_build_info`).

---

## Phase 5 — Product gates: fold + finish (~40 h)

Lift the standalone `bin/cr-*.rs` scanners into registry `Subcommand`s (Phase 0 folds them; here we finish the ones not yet met).

- [ ] **CR-A2 (~4 h), CR-E1 (~3 h):** already met — confirm the folded gates re-verify (FFI non-null schema scan; interp cold-start profile) and emit artifacts.
- [ ] **CR-A1 (~6 h):** complexity gate over `vox-compiler/src/lower/` (logic exists in `bin/cr-a1.rs`); refactor the 14 functions over budget.
- [ ] **CR-A4 (~4 h):** orchestration-contract lifecycle-metadata gate.
- [ ] **CR-E2 (~6 h):** Marquee bundle-size gate (≤800 KB gzip) wired into `vox build`.
- [ ] **CR-D3 (~14 h):** CLI-doc-coverage gate (currently 8/68); author the ~60 missing `.vox` examples.

---

## Phase 6 — External-infra delivery (CR-P1/P2/P3, CR-E3, ~70–90 h)

**These are NOT carved out.** Each gets real automation + a harness + a registered gate, built in its **own PR**. The criterion goes green when the harness runs against real infra; the *engineering* (automation, gate, artifact schema, CI dry-run) is fully in scope and self-contained. Where a green requires resources an agent can't conjure from a checkout (cloud creds, 7 elapsed days, a GPU), the plan ships a **`--dry-run` / fixture-replay mode** that exercises the whole harness deterministically in CI, plus a documented operational run-step for the real green.

> Spawn one detail-plan per criterion: `docs/superpowers/plans/<date>-cr-p{1,2,3}.md`, `<date>-cr-e3.md`.

- [ ] **CR-P3 — `vox new web → vox deploy` < 120s (~12 h, PR #1):** build the end-to-end deploy timing harness (overlaps CR-L7's existing deploy legs); `deploy` gate asserts the 120s budget against a real OCI target, with a `--dry-run` mode timing the local build+emit legs. Start here — least external, most reuse.
- [ ] **CR-P1 — ≥3 Marquee apps live on OCI infra, zero manual config (~30 h, PR #2):** build the marquee-deploy automation (Terraform/OCI provisioning + `vox deploy` orchestration for the 3 Marquee manifests) + `cr-p1` gate that probes the live endpoints; fixture-replay mode validates the orchestration plan offline. Live green needs an OCI account (operational step).
- [ ] **CR-P2 — 99.9% uptime over a 7-day soak (~16 h, PR #3):** build the uptime-monitoring harness (scheduled probe + rolling-window SLO calc + artifact) + `cr-p2` gate; CI runs it in accelerated/replay mode over a synthetic 7-day series. Live green needs the real 7-day soak (operational step, kicked off at release-candidate freeze).
- [ ] **CR-E3 — training loss parity vs reference PyTorch/LoRA (~30 h, PR #4):** build the training-parity harness in `vox-populi` (run the native loop + the reference loop on the `vox-lang` corpus, compare final loss within tolerance) + `cr-e3` gate; CPU smoke mode over a tiny corpus for CI, full run gated behind a GPU runner (operational step).

**Phase-6 exit:** all four gates are registered, GA-aggregated, and green in their CI dry-run/replay mode; each criterion's live green is a single documented operational command run at release freeze. `--strict-block-ga` no longer excludes any criterion.

---

## 4. Self-review notes

- **Spec coverage:** every CR-* in `v1-release-criteria.md` maps to a phase task — including the four former `external_infra` criteria (now Phase 6). No criterion is descoped; codegen-ts (CR-F2 third arm) is in Phase 1.
- **Architecture grounded in real code:** gates use the `CrlGate`/`Subcommand` registry (`lib.rs`) with the `retirement.rs` structural template and the `AuditReport`/`Threshold` shape — not the imagined `registry.rs`/`gate_tier()`/`{observed,target,met}` of the prior draft. The standalone `bin/cr-*.rs` binaries are folded into the registry in Phase 0 so `--gate all` sees them.
- **Dependencies honored:** Phase 0 framework precedes all gates; CR-F0 closes after F1–F6; CR-K2/3/5/6/7 depend on CR-K1's `_public.toml`; CR-U5/6 depend on CR-U4; CR-F4 depends on CR-F3's supported-construct list; Phase 6 PRs are independent of each other (do CR-P3 first for reuse).
- **TDD throughout:** every gate is built failing-test-first; every codegen fix is golden-driven (all three arms agree). External-infra harnesses are TDD'd via their dry-run/replay fixtures.
- **No-stubs rule honored:** Phase 0 does **not** register empty CR-F1–F6 placeholders; it wires one already-landed real gate (CR-F1) and folds existing real binaries. New gates land with a real impl or not at all.
- **Criteria-doc steer applied:** the `external_infra: true` "excluded from the autonomous-completion loop" framing in `v1-release-criteria.md` §Tier 3 is superseded by Phase 6 (harnesses built in dedicated PRs). Reflect this with a one-line edit when Phase 6 starts.

## 5. Execution handoff

**Recommended next action:** execute **Phase 0** per its detailed plan ([`2026-06-06-phase0-gate-framework.md`](2026-06-06-phase0-gate-framework.md)) — it's the keystone every later task reuses, ~24 h, and makes the criteria doc self-verifying. Then fork four parallel streams (CR-F2 rust+ts / CR-K / CR-U / Phase-6 harnesses). On entering any of Phases 1–6, **first re-audit current state** (the framework and main move fast) and graduate that phase's task-list into a detailed sub-plan before implementing. The GA gate is the single command `vox audit --gate all --strict-block-ga` exiting `0` with **no carve-outs**.
