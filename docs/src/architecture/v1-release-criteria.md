---
title: "v1-release-criteria"
description: "Tiered, machine-verifiable v1.0 release criteria for Vox — Foundation (CR-F), Distribution (CR-K), GUI (CR-U), and Product (CR-P/A/E/D/L) gates, each with an exact verify command."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---
# Vox v1.0 Release Criteria (Hardened)

To reach a stable v1.0, the Vox foundation must satisfy the criteria below.
They are evaluated in **tier order**: the **Foundation tier (CR-F)** gates the
**Distribution (CR-K)**, **Desktop GUI (CR-U)**, and **Product (CR-P/A/E/D/L)**
tiers. Per **[CR-F0]**, no downstream-tier criterion may report `met: true`
while any Foundation criterion is red.

> **Why this revision (2026-06-05).** The prior criteria gated only outcomes
> built *on top of* Vox (deploy loops, uptime, LLM authorship, bundle size)
> and never the **compiler's own completeness or correctness**. A 6-dimension
> forensic audit (see [`v1-foundation-criteria-research-2026.md`](./v1-foundation-criteria-research-2026.md))
> showed core-fix activity *accelerating*, table-stakes semantics landing at
> HEAD, the golden corpus verified by parse/typecheck only, four
> hand-synchronized execution arms with no differential test, and a
> non-existent crate-publish story. The Foundation/Distribution/GUI tiers and
> the machine-readable format below close that gap.

> **Implementation status (2026-06-05): this is a TARGET spec, not current
> state.** Most Foundation (CR-F), Distribution (CR-K), and GUI (CR-U) gate
> harnesses are **unbuilt** — their `verify_cmd`s reference `vox audit --gate`
> targets that are not yet registered. `audit --gate all --strict-block-ga`
> therefore exits non-zero today, by design. **(CR-F1's harness landed
> 2026-06-05 — see its status line.)** Each criterion's `if_failing`
> field is the build pointer. Do not mark any new-tier gate `met: true` until
> its harness lands and writes a real artifact.

## 0. Machine-readable format (CR-META)

Every criterion carries three machine-readable fields so an LLM agent can
drive it to completion without human interpretation:

- **`verify_cmd`** — the exact command that exits `0` on pass, non-zero on fail.
- **`artifact_path`** — the JSON breakdown the gate writes (per-item results).
- **`if_failing`** — the pointer to the plan section / fixture dir to build next.

**[CR-META]** The criteria doc fails its own CI lint
(`crates/vox-arch-check/src/evidence_ledger_check.rs`, extended) if any
`[CR-*]` block is missing a `verify_cmd`, a resolvable `artifact_path` that maps
to a registered gate, or a non-empty `if_failing` pointer.
- `verify_cmd`: `cargo run -p vox-arch-check -- --lint criteria-format`
- `artifact_path`: `contracts/reports/arch/criteria-format/<UTC>.json`
- `if_failing`: add the missing field to the offending criterion block.

> **Marquee app set.** "Marquee app" references resolve to the canonical
> fixture set at `contracts/marquee/manifest.v1.yaml` (exists). Criteria
> [CR-P1], [CR-P3], [CR-E2], [CR-L0], [CR-L7] depend on it.

---

## Tier 0 — Foundation: the language itself is complete and correct

*Evaluated first. While any CR-F is red, all downstream tiers report
`blocked_by_foundation`.*

**[CR-F0] Foundation-first ordering.** `vox audit --gate all` evaluates every
CR-F gate before any CR-K/CR-U/CR-P/A/E/D/L gate, and reports downstream gates
as `blocked_by_foundation` (never `met: true`) whenever any `CR-F.met == false`.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate all --strict-block-ga`
- `artifact_path`: `contracts/reports/_snapshot/<UTC>.json` (`tier:"foundation"` rows first)
- `if_failing`: implement gate ordering in `crates/vox-audit/src/registry.rs`; add a `vox-arch-check` rule asserting foundation gates sort first.

**[CR-F1] Behavioral goldens.** Every `.vox` under `examples/golden/` that
produces output must **execute** and match a committed `// EXPECT:` block (or an
`@test` fn); **zero** parse/typecheck-only goldens remain. Coverage of
top-level goldens by `// EXPECT` ∪ `@test` must be `1.0`.
- `verify_cmd`: `cargo test -p vox-integration-tests --test golden_behavioral_gate`
- `artifact_path`: `contracts/reports/behavioral-goldens/<UTC>.json`
- `if_failing`: extend the `// EXPECT` harness coverage toward 1.0 and add the ratchet assertion. **Status: harness LANDED 2026-06-05** (`crates/vox-integration-tests/tests/golden_behavioral_gate.rs`, green) — 7 EXPECT goldens execute+match; the harness's first catch was a real bug (`vox run --mode interp` Debug-printed `main`'s return value as `Str("ok")` — fixed in `run.rs`). Remaining: author EXPECT/`@test` for the ~41 behaviorally-uncovered goldens (printed by the census test) and flip coverage to a hard 1.0 ratchet.

**CR-F2 — Arm correctness (split by domain, not "three identical renderings").**
Vox emits along a **domain boundary** (maintainer steer 2026-06-07): *logic →
Rust; browser/GUI → TypeScript; never both*. So CR-F2 is **not** "every golden
runs identically under interp/script/ts" — codegen-ts is a web/GUI emitter that
does not (and should not) render a stdout logic program. CR-F2 splits into three
independent criteria (CR-F2a/b/c). See
[`codegen-ts-domain-boundary-and-cr-f2-correction-2026.md`](./codegen-ts-domain-boundary-and-cr-f2-correction-2026.md).

**[CR-F2a] Logic parity (interp ≡ codegen-rust).** Every `examples/golden/**`
program with `fn main` + `// EXPECT:` produces **byte-identical stdout** under
`--mode interp` and `--mode script` (codegen-rust). The divergence allowlist is
**non-growing** (ratchet to empty).
- `verify_cmd`: `cargo test -p vox-integration-tests --test golden_arm_parity_test -- --ignored`
- `artifact_path`: `contracts/eval/arm-parity-allowlist-script.txt` (committed ratchet; empty = met)
- `if_failing`: fix the codegen-rust emit site for the diverging construct, then remove its line from the allowlist. **Status: MEASURED 2026-06-07 — codegen-rust 3/10 (pass: decimal_math, mesh/noop, while_loop_algorithms); 7 in the allowlist with verified rustc errors.**

**[CR-F2b] Web emit correctness (codegen-ts).** Every `examples/golden-ts/**` web
fixture emits TypeScript that type-checks **and** behaves correctly in a
browser/DOM.
- `verify_cmd`: `cargo test -p vox-integration-tests --test ts_emit_typecheck_test -- --ignored` (+ a planned jsdom/Playwright behavioral pass)
- `artifact_path`: `contracts/reports/ts-web-emit/<UTC>.json`
- `if_failing`: fix the codegen-ts emitter for the failing fixture; add the behavioral assertion. **Status: typecheck gate exists (`tsc --noEmit`); behavioral DOM gate NOT yet built.**

**[CR-F2c] Split discipline (the boundary itself).** Each top-level construct
lands in exactly one arm — logic in Rust only, browser/GUI in TS only — nothing
silently dropped by both or emitted by both.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate emit-routing`
- `artifact_path`: `contracts/reports/emit-routing/<UTC>.json`
- `if_failing`: add the construct to the routing classification table and route it to its single correct arm; fix any drop/duplication. **Status: gate NOT yet built; known wart — codegen-ts emits empty app boilerplate for logic-only programs.**

**[CR-F3] Language-spec coverage.** A machine-readable checklist
(`contracts/spec/language-surface-coverage.v1.yaml`) enumerates every grammar
production, decorator, and builtin and maps each to ≥ 1 passing behavioral
fixture; **zero** uncovered rows and **zero** `incomplete-arm` rows. A
`vox-arch-check` rule fails if a new grammar production lands without a row.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate spec-coverage`
- `artifact_path`: `contracts/reports/spec-coverage/<UTC>.json`
- `if_failing`: add the missing fixture or finish the named arm for the uncovered construct. **Status: checklist file does not yet exist.**

**[CR-F4] No incomplete arms.** Zero reachable `todo!()` / `unimplemented!()` /
"not yet" / runtime-`UnsupportedOnPlatform` markers for any construct marked
*supported* in CR-F3. A construct an arm cannot lower must fail at **codegen
time** (the `codegen_rust` WASI `compile_error!` pattern), never at runtime.
**db.\* operations and intra-project `import "./x.vox"` are carved out of the
supported set for the script/ts arms and MUST be rejected there with a stable
codegen-time diagnostic** (council steer 2026-06-05: reject, do not silently
accept), while remaining fully executable under `--mode interp`.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate no-incomplete-arms`
- `artifact_path`: `contracts/reports/no-incomplete-arms/<UTC>.json`
- `if_failing`: implement the lowering arm, or convert the runtime throw into a codegen-time diagnostic. For the carve-out, add `vox/codegen/db-unsupported-here` + `vox/codegen/local-import-unsupported-here` diagnostics in the script/ts arms. **Status: codegen-ts currently runtime-throws for typecheck-passing db ops (`hir_emit/mod.rs:1050`).**

**[CR-F5] Convergence.** Core-fix commits per rolling 2-week window decline for
3 consecutive windows AND the final window is ≤ 25% of the peak window; AND the
release-tagged commit body contains **zero first-time-semantics entries**
(no "implement … interpreter", "parity", "exhaustiveness", "de-stub", or
"NNN green" test-count thrash) and touches no `crates/vox-compiler/src/{eval,hir}`
or `crates/vox-codegen/src` file beyond version bumps.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate core-convergence`
- `artifact_path`: `contracts/reports/core-convergence/<UTC>.json`
- `if_failing`: keep fixing the core until the curve bends; do not tag a release whose own commit lands new semantics. **Status: today the curve is rising (core-fix share 5.2%→19.8% Mar–Jun); HEAD #137 landed first-time semantics.**

**[CR-F6] Regression budget.** Zero `// vox:skip`, zero de-stub-pending mocks,
zero stub/placeholder returns in `crates/vox-compiler`, `crates/vox-codegen`,
and the golden corpus; the count is **non-increasing** between tagged releases.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate regression-budget`
- `artifact_path`: `contracts/reports/regression-budget/<UTC>.json`
- `if_failing`: replace the stub with a real impl, or scope the construct out of the CR-F3 supported set.

---

## Tier 1 — Distribution: Rust users can install Vox and its crates

**[CR-K1] Public set publishes clean.** Every crate in the declared public set
passes `cargo publish --dry-run` (no missing-version, missing-license,
path-only-dep, or `publish=false`-transitive errors). *(Today: 1/103 —
`vox-crypto` only.)*
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate crate-publish` (iterates `crates/_public.toml`, runs `cargo publish --dry-run -p <N>`)
- `artifact_path`: `contracts/reports/crate-publish/<UTC>.json`
- `if_failing`: fix the exact dependency/field cargo names per crate; strip `workspace-hack` from the public set's publish surface.

**[CR-K2] Public-set manifest + metadata.** A canonical `crates/_public.toml`
lists the external-publication set; every listed crate has non-empty
`description`, `license`, `repository`, `readme`.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate public-set-metadata`
- `artifact_path`: `contracts/reports/public-set-metadata/<UTC>.json`
- `if_failing`: create `crates/_public.toml`; add `license.workspace = true` + metadata to each listed crate (~34 omit `license` today; `vox-cli` sets `license = false`).

**[CR-K3] No publish-false deps; versioned intra-deps.** No public crate
depends on a `publish=false` crate (incl. `workspace-hack`) at publish time;
intra-workspace deps in the public set carry a `version` requirement.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate publish-dep-hygiene`
- `artifact_path`: `contracts/reports/publish-dep-hygiene/<UTC>.json`
- `if_failing`: declare `{ path, version }` on workspace deps; add publish-time `workspace-hack` stripping (75/103 manifests carry it today).

**[CR-K4] `voxup` installs a real toolchain.** `voxup install default`
produces a working `vox` binary (not the current proxy-wrapper stub) and the
installed `vox --version` matches the released version.
- `verify_cmd`: `HOME=$tmp voxup install default && $tmp/.vox/bin/vox --version` (CI; asserts not the `Vox Proxy Wrapper` stub)
- `artifact_path`: `contracts/reports/voxup-install/<UTC>.json`
- `if_failing`: implement artifact download + checksum verification + real exec in `crates/voxup/src/install.rs`.

**[CR-K5] SemVer policy + enforcement.** A documented SemVer/public-API policy
exists and `cargo-semver-checks` runs in CI for the public set, blocking
unannounced breaking changes.
- `verify_cmd`: `cargo semver-checks check-release` over `crates/_public.toml`
- `artifact_path`: `contracts/reports/semver/<UTC>.json`
- `if_failing`: author `docs/src/contributors/semver-policy.md` (with frontmatter); add the CI gate.

**[CR-K6] Publish workflow.** A `.github/workflows/publish-crates.yml` publishes
the public set in reverse-topological order on `v*` tags, gated on CR-K1.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate publish-workflow` (asserts workflow exists, triggers on `v*`, runs the dry-run gate first)
- `artifact_path`: `contracts/reports/publish-workflow/<UTC>.json`
- `if_failing`: add the workflow; order jobs by dependency topology.

**[CR-K7] Signed/verifiable binary artifacts.** Release binaries ship a
SHA-256 checksum/provenance manifest; `vox-checksum-manifest` and
`vox-release-artifacts` are landed (not `[planned]`) and wired into the release
workflows.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate release-provenance`
- `artifact_path`: `contracts/reports/release-provenance/<UTC>.json`
- `if_failing`: promote the two `[planned]` crates in `layers.toml`; emit + verify a manifest per asset.

> **Recommended public set (dependency order):** `vox-crypto`,
> `vox-jsonschema-util`, `vox-telemetry`, `vox-journal`, `vox-git`,
> `vox-grammar-export`, `vox-db-types` → `vox-db`. Compiler/orchestrator crates
> stay internal until their API surface is deliberately frozen.

---

## Tier 2 — Desktop GUI: VOX GUI installs and every surface works

**[CR-U1] Surface-registry gate bites in CI.** `vox ci gui-surface-registry`
runs as a **required** CI gate and fails on any drift or wiring violation.
*(Today: runs in zero workflows.)*
- `verify_cmd`: required CI job runs `vox ci gui-surface-registry`; a mutation must turn it red
- `artifact_path`: `contracts/reports/gui-surface-registry/<UTC>.json`
- `if_failing`: add the job to a `.github/workflows/*.yml` listed in the `ci-summary` `needs:` array.

**[CR-U2] Every surface renders.** Every `live_backend` / `curated_decorator`
surface renders a non-empty, non-error panel in a headless run of the **actual
vox-gui frontend** (not the emitted codegen app); `count(surfaces_tested) ==
count(non-none registry entries)`.
- `verify_cmd`: `pnpm --dir crates/vox-gui/ui test:e2e -- surface-reachability`
- `artifact_path`: `contracts/reports/gui-surface-reachability/<UTC>.json`
- `if_failing`: map each registered surface to a real panel; fix `renderView` `default: return null` blanking.

**[CR-U3] vox-gui's own suites run in CI.** vox-gui's Playwright e2e and vitest
suites run as required CI gates.
- `verify_cmd`: `pnpm --dir crates/vox-gui/ui test && pnpm --dir crates/vox-gui/ui test:e2e`
- `artifact_path`: `contracts/reports/gui-suites/<UTC>.json`
- `if_failing`: add both to a required CI job (today neither runs in CI).

**[CR-U4] Installers build.** `tauri.conf.json` is configured to produce
installers for all three platforms with a complete icon set (`.ico`/`.icns` +
PNG sizes); a CI dry-run yields a non-empty bundle.
- `verify_cmd`: full pipeline (`cargo build -p vox-cli --release && pnpm --dir crates/vox-gui/ui build && tauri build`) → assert ≥ 1 installer artifact
- `artifact_path`: `contracts/reports/gui-bundle/<UTC>.json`
- `if_failing`: add `bundle.active`/`targets` + the icon set (today: only `icon.png`, no `targets`).

**[CR-U5] Signed + verified release.** `release-gui.yml` builds the sidecar
binaries before bundling and signs+verifies the installer using existing paths
(no `src-tauri/`).
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate gui-release` (asserts a `cargo build` step precedes bundling, a signature-verify step exists, and no `src-tauri` path)
- `artifact_path`: `contracts/reports/gui-release/<UTC>.json`
- `if_failing`: add the sidecar build step; fix the Windows signing `files-folder` path; add `signtool verify` / `codesign --verify`.

**[CR-U6] Launch + IPC smoke.** A smoke test proves the packaged app starts and
the **real** Tauri invoke handlers (not the mocked e2e bridge) answer for a
representative read-only command.
- `verify_cmd`: `cargo run -p vox-cli -- audit --gate gui-smoke`
- `artifact_path`: `contracts/reports/gui-smoke/<UTC>.json`
- `if_failing`: add a headless launch harness invoking `get_build_info` / `execute_command` through the real handlers.

---

## Tier 3 — Product (evaluated only when Tiers 0–2 are green)

*These criteria are unchanged in intent from the original council-ratified set;
they are re-tiered to sit behind the Foundation gate. Criteria flagged
`external_infra: true` cannot be self-completed by an agent from a checkout and
are excluded from the autonomous-completion loop.*

### 3.1 Production Validation
- **[CR-P1]** ≥ 3 Marquee applications deployed live on OCI-compliant infra with zero manual config. `verify_cmd`: `vox audit --gate cr-p1` · `artifact_path`: `contracts/reports/perf/cr-p1/<UTC>.json` · `if_failing`: §5.2 of the honest-completion plan · **`external_infra: true`**.
- **[CR-P2]** 99.9% uptime for the `vox-ml-cli` inference endpoint over a 7-day soak. `verify_cmd`: `vox audit --gate cr-p2` · `artifact_path`: `contracts/reports/perf/cr-p2/<UTC>-7day.json` · `if_failing`: §5.3 · **`external_infra: true`**.
- **[CR-P3]** `vox new web → vox deploy` under 120s end-to-end. `verify_cmd`: `vox audit --gate deploy` · `artifact_path`: `contracts/reports/deploy/<UTC>.json` · `if_failing`: §3.2 · **`external_infra: true`** (live deploy leg).

### 3.2 Architectural Integrity
- **[CR-A1] K-Complexity Freeze**: cyclomatic complexity < 15 on all primary lowering paths. `verify_cmd`: `vox audit --gate cr-a1` · `artifact_path`: `contracts/reports/arch/cr-a1/<UTC>.json` · `if_failing`: refactor the 14 functions currently > budget (max 28).
- **[CR-A2] Non-Null Boundary**: 100% of FFI/IPC interfaces use non-null VoxProto v1 schemas. `verify_cmd`: `vox audit --gate cr-a2` · `artifact_path`: `contracts/reports/arch/cr-a2/<UTC>.json` · `if_failing`: annotate remaining boundaries (currently 186/186 = met).
- **[CR-A3] Crate Decoupling**: zero circular deps across `contracts/db/data-storage-policy.v1.yaml#frozen_core_crates`. `verify_cmd`: `cargo run -p vox-arch-check` · `artifact_path`: `contracts/reports/arch/cycles/<UTC>.json` · `if_failing`: break the offending edge.
- **[CR-A4] Lifecycle Metadata Parity**: orchestration contracts declare lifecycle + migration window with CI parity checks. `verify_cmd`: `vox audit --gate cr-a4` · `artifact_path`: `contracts/reports/arch/cr-a4/<UTC>.json` · `if_failing`: add lifecycle metadata to the bare contract.

### 3.3 Performance & Efficiency
- **[CR-E1] Cold Start**: `vox run --interp` initializes + executes Hello World < 50ms. `verify_cmd`: `vox audit --gate cr-e1` · `artifact_path`: `contracts/reports/perf/cr-e1/<UTC>.json` · `if_failing`: profile interp init (currently p99 = 0.25ms = met).
- **[CR-E2] Bundle Size**: Marquee bundle (React + TanStack) ≤ 800KB gzip. `verify_cmd`: `vox audit --gate cr-e2` · `artifact_path`: `contracts/reports/perf/cr-e2/<UTC>.json` · `if_failing`: add the build-time bundle gate in `vox build`.
- **[CR-E3] Training Parity**: `vox-populi` training reaches loss parity with reference PyTorch/LoRA on the `vox-lang` corpus. `verify_cmd`: `vox audit --gate cr-e3` · `artifact_path`: `contracts/reports/perf/cr-e3/<UTC>.json` · `if_failing`: tune the native training loop · **`external_infra: true`** (GPU).

### 3.4 Agentic DX
- **[CR-D1] Planning Mode Fidelity**: ≥ 85% multi-step "Wave 2" plan success without intervention (harness = [CR-L4]). `verify_cmd`: `vox audit --gate plan-fidelity` · `artifact_path`: `contracts/reports/plan-fidelity/<UTC>.json` · `if_failing`: port the refinement loop; corpus §4.3 (currently 0.40 vs 0.85).
- **[CR-D2] Self-Healing**: `vox repair` resolves 90% single-file / 70% project-scope (project variant = [CR-L3]). `verify_cmd`: `vox audit --gate repair-corpus` · `artifact_path`: `contracts/reports/repair-corpus/<UTC>.json` · `if_failing`: expand corpus §4.2.
- **[CR-D3] Documentation Coverage**: 100% of `vox-cli` subcommands have machine-readable help + a `.vox` example. `verify_cmd`: `vox audit --gate cr-d3` · `artifact_path`: `contracts/reports/arch/cr-d3/<UTC>.json` · `if_failing`: author the ~60 missing examples (currently 8/68 = 11.8%).

### 3.5 LLM-Target Fidelity

These operationalize the claim that Vox is shaped so AI agents can author code
reliably. Full audit: [`vox-as-llm-target-audit-and-plan-2026.md`](vox-as-llm-target-audit-and-plan-2026.md).
Implementation: [`v1-llm-target-implementation-plan-2026.md`](v1-llm-target-implementation-plan-2026.md).

- **[CR-L0] End-to-End Agent Authorship Loop**: autonomous agent loop over `contracts/eval/spec-to-app/` produces passing apps (`vox check` clean, tests pass, `vox deploy` succeeds, `vox doctor` green) at ≥ 60% (sub-bar < 40%). `verify_cmd`: `vox audit --gate spec-to-app` · `artifact_path`: `contracts/reports/spec-to-app/<UTC>.json` · `if_failing`: §3.7 (currently 0.667 = met, generate→check loop only).
- **[CR-L1] HumanEval-Vox**: 164-program suite reaches ≥ 80% **behavioral** compile+test-pass when prompted to MENS or a reference LLM. The gate now runs `vox run --mode interp` per `tests.vox`. **Split:** L1a (corpus integrity, reference solutions, met 164/164) vs **L1b (LLM-generated pass@k ≥ 0.80 — BLOCKING, harness `per_llm` currently empty)**. `verify_cmd`: `vox audit --gate humaneval --llm-panel` · `artifact_path`: `contracts/reports/humaneval/<UTC>.json` · `if_failing`: wire the LLM-panel generation phase (`humaneval.rs` P2.4+).
- **[CR-L2] On-Distribution Rate**: ≥ 95% of MENS-emitted Vox clears `vox check --strict` + the vox-code-audit + retirement-guard. `verify_cmd`: `vox audit --gate mens-on-distribution` · `artifact_path`: `contracts/reports/mens-on-distribution/<UTC>.json` · `if_failing`: §3.4 MENS sampling (NOT YET FILLED).
- **[CR-L3] Project-Scope Self-Healing**: `vox repair .` ≥ 70% on the 50-project corpus. `verify_cmd`: `vox audit --gate repair-corpus` · `artifact_path`: `contracts/reports/repair-corpus/<UTC>.json` · `if_failing`: §4.2 (currently 0.80 on 5 projects).
- **[CR-L4] Plan-Mode Fidelity Measurement**: Wave-2 fixtures at `contracts/eval/plan-fidelity/` with an automated 85% harness. `verify_cmd`: `vox audit --gate plan-fidelity` · `artifact_path`: `contracts/reports/plan-fidelity/<UTC>.json` · `if_failing`: §3.6/§4.3.
- **[CR-L5] ACI Envelope Default-On**: `agentos_aci_envelope_enabled` defaults `true`; guardrail kernel rejects unclassified mutations at v1.0. `verify_cmd`: `vox audit --gate aci-default` · `artifact_path`: `contracts/reports/aci-default/<UTC>.json` · `if_failing`: flip the default (met).
- **[CR-L6] Retirement-Guard Parity**: every row in [`AGENTS.md` §Retired Surfaces](../../../AGENTS.md) has a detector or arch-check rule. `verify_cmd`: `vox ci retirement-audit` · `artifact_path`: `contracts/reports/retirement/<UTC>.json` · `if_failing`: add the missing detector (met 16/16).
- **[CR-L7] Deploy CLI Completeness**: `vox new`/`vox deploy`/`vox doctor` ship structured JSON + telemetry + a CI integration test inside the 120s budget. `verify_cmd`: `vox audit --gate deploy` · `artifact_path`: `contracts/reports/deploy/<UTC>.json` · `if_failing`: §3.2 (met for all three legs).
- **[CR-L8] Diagnostic→Repair→Corpus Feedback Loop**: quarterly telemetry export into vox-corpus runs in CI. `verify_cmd`: `vox audit --gate corpus-feedback` · `artifact_path`: `contracts/reports/corpus-feedback/<UTC>.json` · `if_failing`: §3.1 (met; CI fails if artifact > 90 days).

---

## Acceptance for v1.0 GA

```bash
cargo run -p vox-cli -- audit --gate all --strict-block-ga
```

Must exit `0`. The roll-up at `contracts/reports/_snapshot/<UTC>.json` must show
**every Foundation (CR-F), Distribution (CR-K), and GUI (CR-U) gate** at
`met: true`, AND every block-GA Product gate (`external_infra: false`) at
`met: true` and `incomplete: false`. Per **[CR-F0]**, the snapshot must not
report any downstream gate green while a Foundation gate is red.

---
*Original criteria (CR-P/A/E/D/L) approved by Vox Foundation Council — April 2026;
§3.5 (CR-L0..L8) + Marquee manifest ratified 2026-05-15. Foundation tier (CR-F),
Distribution tier (CR-K), GUI tier (CR-U), CR-META format, and tier re-ordering
added 2026-06-05 per [`v1-foundation-criteria-research-2026.md`](./v1-foundation-criteria-research-2026.md);
arm-parity scope (all three arms) and the db.\*/import reject-with-diagnostic
disposition reflect the 2026-06-05 maintainer steer. Council ratification of the
new tiers is pending. Per ratification D2: realistic-v1.0 bars hold. Per D16:
mesh Phase 2 LAN demoted to v1.1.*
