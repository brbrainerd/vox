---
title: "AI-assisted dev loop — compile overhead (2026)"
description: "Evidence-backed practices and tooling to reduce redundant Cargo rebuilds when using coding agents (multi-terminal, pre-push frequency, cache fragmentation)."
category: "Architecture SSOTs"
status: "current"
last_updated: "2026-05-11"
training_eligible: true
schema_type: "TechArticle"
---

# AI-assisted dev loop — compile overhead (2026)

## Evidence baseline

| Signal | Source |
|--------|--------|
| **`CARGO_TARGET_DIR`** varies across agent terminals (`target-agent-ssot`, `target-ci-prepush`, default **`target/`**) | Recent Cursor terminal metadata (same machine, concurrent sessions) |
| **`--quick` pre-push was documented as ~30 s** while implementation **always** runs doc lint + doctest-md + drift-check | Drift between [local-ci-pre-push](../contributors/local-ci-pre-push.md) and [`pre_push.rs`](../../../crates/vox-cli/src/commands/ci/pre_push.rs) — **corrected 2026-05-11** |
| CI already emits comparable test timings | Nextest JUnit → **`vox ci test-runtime-report`** ([runner-contract](../ci/runner-contract.md)) |

## Canonical inner loop (agents)

1. **`vox ci dev-loop-audit`** — confirm **`CARGO_TARGET_DIR`** is unset or points at repo **`target/`**.
2. **`cargo check -p <crate>`** — fastest compile signal for the crate you touched.
3. **`cargo nextest run -p <crate> --profile ci`** or filtered **`cargo test -p <crate> …`** — prove behavior without compiling the workspace test graph when unnecessary.
4. **`vox ci pre-push`** — **push readiness** only (default or **`--quick`**); avoid re-running as a substitute for steps 2–3.
5. **`--full`** — parity with CI workspace nextest; use before merge or when touching cross-crate contracts.

Do **not** rotate **`CARGO_TARGET_DIR`** mid-task unless deliberately isolating (e.g. benchmarks); clear side dirs when done ([`.cursor/rules/build-environment.mdc`](../../../.cursor/rules/build-environment.mdc)).

## Tooling

| Artifact | Command / path |
|----------|----------------|
| Pre-push step timings | **`vox ci pre-push --report-json <path>`** → [`pre-push-report.v1.schema.json`](../../../contracts/reports/pre-push-report.v1.schema.json) |
| Pre-push frequency (append-only log) | Env **`VOX_PREPUSH_AUDIT_LOG=<path>`** (JSON lines on success) |
| Target-dir / habit audit | **`vox ci dev-loop-audit [--json]`** → [`dev-loop-audit.v1.schema.json`](../../../contracts/reports/dev-loop-audit.v1.schema.json) |
| Compile lane budgets | **`vox ci build-timings`** + **`docs/ci/build-timings/budgets.json`** |

## Targets and rollout

| Phase | Goal | Enforcement |
|-------|------|-------------|
| A | Docs + **`dev-loop-audit`** warnings | Advisory |
| B | **`--report-json`** adoption in agent runbooks | Optional CI artifact later |
| C | Compare **`VOX_PREPUSH_AUDIT_LOG`** rates vs **`test-runtime-report`** medians | Maintainer review |

**Success criteria (local):** median **edit → first `cargo check -p`** cycle avoids redundant full workspace compiles; **`fragmentation_risk: none`** from **`dev-loop-audit`** during typical sessions. **Stretch:** 30–50% wall-clock reduction on crate-scoped tasks vs alternating **`CARGO_TARGET_DIR`** + repeated **`vox ci pre-push`**.

## Related

- [runner-contract — Pre-push & cache troubleshooting](../ci/runner-contract.md)
- [local-ci-pre-push](../contributors/local-ci-pre-push.md)
- [coding-agents](../contributors/coding-agents.md)
- [build-timings README](../../../docs/ci/build-timings/README.md)
