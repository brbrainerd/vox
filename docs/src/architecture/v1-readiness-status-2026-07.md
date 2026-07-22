---
title: v1.0 Readiness Status (2026-07-22 audit)
description: "Live-codebase audit of every CR-F/CR-K/CR-U criterion from v1-foundation-criteria-research-2026.md, checked against actual tests, gates, and CI wiring rather than filenames or intent."
category: "Architecture SSOTs"
status: "research"
training_eligible: false
---

# v1.0 Readiness Status

Audit of [`v1-foundation-criteria-research-2026.md`](./v1-foundation-criteria-research-2026.md)'s
CR-F/CR-K/CR-U criteria as of 2026-07-22, alongside a concurrent effort that promoted CR-U6's smoke
test to a required CI gate
([docs/superpowers/plans/2026-07-22-orchestrator-reliability-and-bottom-bar.md](../../superpowers/plans/2026-07-22-orchestrator-reliability-and-bottom-bar.md)).

Each row was checked by reading the actual test/gate/CI-config source, not inferred from a
filename or a comment. "Built & Verified" means a real, non-ignored check exists **and** runs as
a required (non-`continue-on-error`, unconditional) CI gate. "Built, Unverified" means the
mechanism is real code but is missing CI enforcement, is conditionally gated (e.g. path-filtered
or `push`-only), or covers only a fraction of its stated scope. "Unbuilt" means neither the
mechanism nor the gate exists.

## Foundation tier (CR-F1–F6)

| Criterion | Status | Evidence | Follow-up needed |
|---|---|---|---|
| CR-F1 Behavioral goldens | ⚠️ Built, Unverified | `crates/vox-integration-tests/tests/golden_behavioral_gate.rs` + `crates/vox-audit/src/subcommands/behavioral_goldens.rs` (`CrlGate::F1BehavioralGoldens`, in the `block_ga()` set) really execute goldens under `vox run --mode interp` and diff `// EXPECT:` blocks. But only 11/72 top-level `examples/golden/*.vox` files carry `// EXPECT:`; nowhere near the required 100% coverage. Enforced via `vox audit --gate all --strict-block-ga` in `.github/workflows/cr-l-gates.yml`, which is path-filtered (only triggers on PRs touching `crates/vox-audit/**`, `crates/vox-arch-check/**`, etc.) plus nightly — not unconditional on every PR that touches goldens/compiler. | Add `// EXPECT:` to the remaining ~61 goldens; widen the workflow trigger to cover compiler/golden-corpus paths. |
| CR-F2 Cross-arm parity | ⚠️ Built, Unverified | `crates/vox-codegen/tests/golden_arm_parity_test.rs` and `crates/vox-integration-tests/tests/golden_arm_parity_test.rs` run interp vs `--mode script` and ratchet against a committed divergence allowlist (`contracts/eval/arm-parity-allowlist-script.txt`) — real, non-growing-allowlist enforcement. Not referenced by name in any `.github/workflows/*.yml`; not wired as a named/required CI job. codegen-ts arm is out of scope for this test entirely. | Wire `golden_arm_parity_test` into a required CI job; decide (per the source doc's Open Decision #1) whether codegen-ts needs its own parity gate. |
| CR-F3 Spec coverage | ❌ Unbuilt | No `contracts/spec/language-surface-coverage*.yaml`; no `spec-coverage` gate in `crates/vox-audit` or `vox-cli`. The `vox-language-surface` crate exists but implements something unrelated to a coverage checklist. | Build the checklist file and the `vox audit --gate spec-coverage` cross-reference against CR-F1/F2 results, per the source doc's §4.3. |
| CR-F4 No incomplete arms | ❌ Unbuilt | No `no-incomplete-arms` gate anywhere in `crates/`. Sanity grep for `todo!()`/`unimplemented!()` in `crates/vox-compiler/src` and `crates/vox-codegen/src` returns 0 hits (clean by luck, not by an automated detector). | Implement the stub-detector gate; extend it to catch runtime `UnsupportedOnPlatform` throws for constructs marked supported. |
| CR-F5 Convergence | ❌ Unbuilt | No script or CI check measures core-fix commit-rate trend or greps release-tag commit bodies for first-time-semantics language. This is inherently a trend metric rather than a point-in-time command, matching the source doc's own framing as the hardest-to-automate criterion. | Per source doc Open Decision #4: decide hard-GA-blocker vs advisory dashboard before building anything. |
| CR-F6 Regression budget | ✅ Built & Verified | `crates/vox-audit/src/core_gates.rs` (`run_silent_drop_gate` / `run_weak_test_gate`) is count-based and ratcheted against committed baselines (e.g. `contracts/toestub/silent-drop-baseline.v1.json`). `CrlGate::F6RegressionBudget` is in the `block_ga()` set, exercised by `vox audit --gate all --strict-block-ga` in `.github/workflows/cr-l-gates.yml`. | Same path-filter caveat as CR-F1: the workflow trigger should be widened so this can't be bypassed by a PR that doesn't touch the audit crates. |

## Distribution tier (CR-K1–K7)

| Criterion | Status | Evidence | Follow-up needed |
|---|---|---|---|
| CR-K1 | ❌ Unbuilt (CI enforcement) | `crates/_public.toml` now declares a public set (`vox-crypto`, `voxup`, `vox-plugin-types`, `vox-plugin-api`, `vox-plugin-sdk`) — up from the 1-crate ad hoc state the source doc audited. No workflow runs `cargo publish --dry-run` against any of them. | Add a CI job that dry-run-publishes every crate in `_public.toml`. |
| CR-K2 | ⚠️ Built, Unverified | All 5 public-set `Cargo.toml`s carry `description`, `license.workspace`, and `repository.workspace`; `readme` presence wasn't fully confirmed for `voxup`. The metadata is real, but `_public.toml` itself notes the verifying gate ("`vox audit --gate public-set-metadata`") is "not yet wired." | Wire the metadata-completeness gate; confirm `voxup`'s `readme` field. |
| CR-K3 | ⚠️ Built, Unverified concern | `crates/voxup/Cargo.toml` still depends on `workspace-hack = { workspace = true }` — a public-set crate retains the exact anti-pattern CR-K3 forbids. Not yet checked/blocked by any gate. | Strip `workspace-hack` from public-set crates or add a gate that fails if a public crate depends on it. |
| CR-K4 | ⚠️ Built, Unverified | `crates/voxup/src/install.rs` genuinely fetches a real release asset and verifies its SHA-256 (`crates/voxup/src/download.rs`, with unit tests for `verify_sha256`) — this is a real upgrade from the "proxy wrapper, no checksum" state the source doc found. No e2e/CI-gated install test exists confirming `voxup install default` produces a working `vox` binary end-to-end. | Add an e2e CI job that runs `voxup install default` and checks `vox --version`. |
| CR-K5 | ❌ Unbuilt | `cargo-semver-checks` confirmed absent from workflows, `Cargo.toml`, and scripts. | Adopt `cargo-semver-checks` in CI for the public set once one exists to check. |
| CR-K6 | ❌ Unbuilt | `.github/workflows/publish-ci-runner.yml` — despite the name — builds/pushes the self-hosted CI-runner Docker image to GHCR; it is unrelated to crate publishing. No reverse-topological crate-publish-on-`v*`-tag workflow exists anywhere in the repo. | Build the actual `publish-crates.yml` workflow gated on CR-K1's dry-run passing. |
| CR-K7 | ❌ Unbuilt (as a real signing mechanism) | `contracts/reports/scaling-audit/by-crate/vox-checksum-manifest.md` is an auto-generated scaling-debt report, not a signing tool. The real checksum logic lives in `crates/vox-cli/src/utils/checksum_manifest/mod.rs` — a small module that verifies `checksums.txt` for `vox upgrade`/bootstrap, useful but not the release-artifact-provenance SSOT the criterion describes. | Build (or explicitly scope down) `vox-checksum-manifest`/`vox-release-artifacts` as release-signing infrastructure. |

## GUI tier (CR-U1–U6)

| Criterion | Status | Evidence | Follow-up needed |
|---|---|---|---|
| CR-U1 | ✅ Built & Verified | `.github/workflows/ci.yml` runs `vox --quiet ci ssot-drift` as a plain (non-`continue-on-error`) step; `run_ssot_drift()` in `crates/vox-cli/src/commands/ci/run_body_helpers/docs.rs` verifies `gui_surface_registry` drift in read-only mode. Real and required — this closes the exact gap ("runs in zero CI workflows") the source doc identified in June. | None — keep an eye on drift as new surfaces are added. |
| CR-U2 | ❌ Unbuilt | No test renders every `live_backend`/`curated_decorator` surface headlessly and asserts the rendered-panel count equals the registry count. The closest analog (the GUI visual-audit sweep under CR-U3) is registry-driven for screenshots but isn't a strict count-match gate, and isn't required on PRs regardless. | Build the count-matching headless-render test described in the source doc §4.5. |
| CR-U3 | ⚠️ Built, Unverified | `crates/vox-gui/ui/e2e/*.spec.ts` (24 spec files) plus Playwright/vitest configs are real, hand-authored vox-gui frontend suites. They run in the `gui-playwright-smoke` CI job, which — despite its name suggesting the emitted codegen web app (the source doc's June characterization) — actually sweeps vox-gui's own registry-driven surfaces today. However that job only runs on `push` to main or with a `full-ci` label — not as a required PR gate. | Promote `gui-playwright-smoke` to a required PR check, or split out a PR-scoped subset. |
| CR-U4 | ✅ Built & Verified | `crates/vox-gui/tauri.conf.json` now sets `bundle.icon` with real `.ico`/`.icns`/PNG assets and `bundle.externalBin`; no `targets`/`active` override needed since the defaults now produce installers. This closes the "no installers" gap from the source doc. | None found; revisit if a 4th target platform is added. |
| CR-U5 | ⚠️ Built, Unverified / partially broken | `.github/workflows/release-gui.yml` does build and stage the `vox` CLI sidecar per platform before `tauri-apps/tauri-action@v1` bundles — that half of the gap is closed. But the Windows signing step (`azure/trusted-signing-action@v2`) still points `files-folder` at `./crates/vox-gui/src-tauri/target/release/bundle/msi`, and `crates/vox-gui/src-tauri` does not exist (config lives at `crates/vox-gui/tauri.conf.json` with `projectPath: ./crates/vox-gui`). The "nonexistent path" bug the source doc flagged is still present — signing will not resolve. | Fix the `files-folder` path in `release-gui.yml` to match the real Tauri project layout. |
| CR-U6 | ✅ Built & Verified | `crates/vox-gui/tests/gui_relaunch_smoke.rs` is a real launch+IPC smoke test and is no longer `#[ignore]`d. `.github/workflows/ci.yml` runs it as the `gui-orchestrator-relaunch-smoke` job, and the `ci-summary` job's `needs:` array (line 1479) includes `gui-orchestrator-relaunch-smoke`, so it is unconditional and required on every PR. | None — keep an eye on flakiness given it launches a real GUI process. |

## Summary

4 of 19 criteria confirmed **Built & Verified** (CR-F6, CR-U1, CR-U4, CR-U6 — CR-U6's
`gui-orchestrator-relaunch-smoke` job landed in the `ci-summary` required-gate `needs:` array
during this same plan session). 7 are **Built, Unverified**
(CR-F1, CR-F2, CR-K2, CR-K3, CR-K4, CR-U3, CR-U5) — real mechanisms exist but lack CI enforcement,
full scope coverage, or a fixed configuration bug, making them candidates for the same kind of
promotion-to-required-gate effort CR-U6 just went through. 8 are genuinely **Unbuilt** (CR-F3,
CR-F4, CR-F5, CR-K1, CR-K5, CR-K6, CR-K7, CR-U2) with no mechanism or gate at all — candidates for
new specs, not attempted in this audit.

This meaningfully **updates the stale prior claim** in project memory that "CR-F/K/U harnesses
[are] UNBUILT" — that was accurate as a *summary* in early June 2026, but by 2026-07-22 the
Foundation tier alone has two real, partially-wired gates (CR-F1, CR-F2, CR-F6) integrated into
`vox-audit`'s `block_ga()` set, the GUI tier has closed four of its five 2026-06-05 gaps
(CR-U1 drift-checking, CR-U4 installer config, CR-U6 relaunch-smoke required gate, and half of
CR-U5's sidecar build step), and the
Distribution tier has moved from "1/103 crates publishable" to a declared 5-crate public set with
real metadata — even though CI enforcement for that set still does not exist. The honest read is:
**substantial unglamorous progress has landed since the June audit, but almost none of it is yet
wired as an unconditional, required CI gate** — the recurring gap across all three tiers is not
"the mechanism doesn't exist," it's "the mechanism exists but isn't load-bearing in CI yet."
