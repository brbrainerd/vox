---
title: "v1-release-criteria"
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---
# Vox v1.0 Release Criteria (Hardened)

To reach a stable v1.0, the Vox foundation must satisfy the following machine-verified and human-audited criteria.

> **Marquee app set (added 2026-05-15).** "Marquee app" references throughout this document resolve to the canonical fixture set at `contracts/marquee/manifest.v1.yaml`. Without that fixture, criteria [CR-P1], [CR-P3], [CR-E2], [CR-L0], and [CR-L7] are unverifiable. Defining the manifest is a prerequisite for implementation of any Marquee-gated criterion.

## 1. Production Validation
- **[CR-P1]** At least 3 "Marquee" applications must be deployed and live on OCI-compliant infrastructure with zero manual configuration.
- **[CR-P2]** 99.9% uptime for the `vox-ml-cli` inference endpoint over a 7-day soak test.
- **[CR-P3]** Full "Zero-DX" deployment loop: `vox new web → vox deploy` must take under 120 seconds end-to-end.

## 2. Architectural Integrity
- **[CR-A1] K-Complexity Freeze**: The core compiler (`vox-compiler`) must maintain a cyclomatic complexity threshold under 15 for all primary lowering paths.
- **[CR-A2] Non-Null Boundary**: 100% of internal FFI and IPC interfaces must use non-null, machine-verified schemas (VoxProto v1).
- **[CR-A3] Crate Decoupling**: The workspace must maintain zero circular dependencies across the 10 core crates defined in `crates/_frozen.md`.
- **[CR-A4] Lifecycle Metadata Parity**: All orchestration contracts that affect model routing/providers must declare lifecycle metadata (`experimental`/`stable`/`deprecated`) and a migration window, with CI parity checks.

## 3. Performance & Efficiency
- **[CR-E1] Cold Start**: `vox run --interp` must initialize and execute a "Hello World" script in under 50ms on standard x86/ARM hardware.
- **[CR-E2] Bundle Size**: The standard "Marquee" application bundle (React + TanStack) must not exceed 800KB (gzip).
- **[CR-E3] Training Parity**: The native `vox-populi` training pipeline must achieve loss parity with reference PyTorch/LoRA implementations for the `vox-lang` corpus.

## 4. Agentic DX (Developer Experience)
- **[CR-D1] Planning Mode Fidelity**: AI agents must be able to execute a multi-step "Wave 2" plan with at least 85% success rate without human intervention. **Measurement harness and Wave-2 fixture set defined by [CR-L4]** — see [`vox-as-llm-target-audit-and-plan-2026.md`](vox-as-llm-target-audit-and-plan-2026.md).
- **[CR-D2] Self-Healing**: `vox repair` must successfully resolve 90% of syntactically valid but logically broken Vox programs identified during the v1 audit. **Multi-file project-scope variant and measurement defined by [CR-L3]**; the 90% number is the single-file aim and the 70% number is the project-scope gate.
- **[CR-D3] Documentation Coverage**: 100% of `vox-cli` subcommands must have machine-readable help and associated `.vox` example scripts in the training corpus. New commands landing under [CR-L7] (`vox new`, `vox deploy`, `vox doctor`) inherit this requirement at their landing release.

## 5. LLM-Target Fidelity

These criteria operationalize the load-bearing claim that recurs across the marquee/research corpus — that Vox is shaped so AI agents can author code reliably, and the compiler+lint+repair pipeline can heal what they produce. Full audit, evidence, sequencing, and open questions: [`vox-as-llm-target-audit-and-plan-2026.md`](vox-as-llm-target-audit-and-plan-2026.md). Implementation plan with phasing, owners, fixture-corpus budget, and risk register: [`v1-llm-target-implementation-plan-2026.md`](v1-llm-target-implementation-plan-2026.md). Bar height: *realistic v1.0* — measurable foundation gates.

- **[CR-L0] End-to-End Agent Authorship Loop**: Given a canonical English spec from `contracts/eval/spec-to-app/` (10–20 specs of increasing complexity), an autonomous agent loop driving Vox (via MCP) must produce a passing application — `vox check` clean, tests pass, `vox deploy` succeeds, `vox doctor` green — at ≥ 60% success rate with a per-spec token-cost ceiling of ≤ $5.00 against the panel reference LLMs. **This is the integration test for the v1.0 LLM-target claim; CR-L1..CR-L8 are unit tests of its sub-loops.** Sub-bar (block GA): observed rate < 40%.
- **[CR-L1] HumanEval-Vox**: A canonical 200-program benchmark suite (`contracts/eval/humaneval-vox/`) must reach ≥ 80% compile + test-pass rate when prompted to MENS or a reference LLM. Reported quarterly via `vox audit humaneval`.
- **[CR-L2] On-Distribution Rate**: ≥ 95% of MENS-emitted Vox programs must clear `vox check --strict` + the 47-rule vox-code-audit + retirement-guard with zero errors and zero high-confidence warnings. Reported quarterly via `vox audit mens-on-distribution`.
- **[CR-L3] Project-Scope Self-Healing**: `vox repair .` must reach ≥ 70% success rate on a defined 50–100 multi-file broken-project corpus (`contracts/eval/repair-corpus/`). Single-file `vox repair` should aim ≥ 90% as a sub-metric.
- **[CR-L4] Plan-Mode Fidelity Measurement**: The "Wave 2" benchmark set referenced by [CR-D1] must exist as fixtures at `contracts/eval/plan-fidelity/` with an automated harness that produces the 85% measurement (closes [CR-D1]'s underspecification).
- **[CR-L5] ACI Envelope Default-On**: `OrchestratorConfig::agentos_aci_envelope_enabled` defaults to `true` starting in v0.6; guardrail kernel rejects unclassified mutations at v1.0.
- **[CR-L6] Retirement-Guard Parity**: Every row in [`AGENTS.md` §Retired Surfaces](../../../AGENTS.md) has either a parse-time / typeck detector or a `vox-arch-check` rule. CI gate (`vox ci retirement-audit`) fails on drift between policy doc and enforcement.
- **[CR-L7] Deploy CLI Completeness**: `vox new`, `vox deploy`, and `vox doctor` ship with structured JSON output, `vox.deploy.*` / `vox.doctor.*` telemetry, and a CI integration test driving `vox new web → vox deploy → vox doctor` on a Marquee app fixture inside the [CR-P3] 120-second budget.
- **[CR-L8] Diagnostic→Repair→Corpus Feedback Loop**: A quarterly pipeline export from `vox.lint.*` + `vox.repair.*` telemetry into vox-corpus runs in CI, emitting `contracts/reports/corpus-feedback/<quarter>.json` (top-50 firing diagnostics, autofix accept/reject rates, repair outcome histogram). CI fails if the artifact is older than 90 days.

---
*Approved by Vox Foundation Council — April 2026. §5 (CR-L0..CR-L8) and the Marquee-app manifest reference ratified by council 2026-05-15 (council batch D1–D25 in [`v1-llm-target-implementation-plan-2026.md`](v1-llm-target-implementation-plan-2026.md) §8 / 2026-05-15 ratification log). Per ratification D1: CR-L is the measurement source of truth; CR-D1/D2/D3 remain as policy lineage and reference the corresponding CR-L measurement harness. Per D2: realistic-v1.0 bars hold. Per D16: mesh Phase 2 LAN is demoted from v1.0 acceptance contract to v1.1.*

