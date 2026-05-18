---
title: "Vox v1.0 LLM-Target Implementation Plan (2026)"
description: "Phased implementation plan for delivering CR-L0..CR-L8 (the §5 LLM-Target Fidelity criteria of v1-release-criteria.md). Names owners, dependencies, fixture-corpus budget, CI contract, risk register, rollback policy."
category: "architecture"
status: "current"
last_updated: "2026-05-15"
training_eligible: false
training_rationale: "Strategic execution plan; reflects 2026-05-15 state; will be superseded by phase-completion reports."
---

# Vox v1.0 LLM-Target Implementation Plan

Companion to:
- [`v1-release-criteria.md`](v1-release-criteria.md) — defines CR-L0..CR-L8.
- [`vox-as-llm-target-audit-and-plan-2026.md`](vox-as-llm-target-audit-and-plan-2026.md) — audit, gaps, attainability verdict, self-critique §9.

This doc moves from "what we measure" to "how we get there." It exists because §9.6 of the audit names what the audit-doc-as-input-to-planning was missing: measurement specification, fixture-corpus budget, owners, dependency graph, CI subcommand contract, risk register, and rollback policy.

**Scope of this plan.** CR-L0..CR-L8 only. Mesh phases, vox-language-rules phases, agentic-VCS phases, and MENS distributed-training are tracked under their own plans and only referenced where a CR-L depends on them.

**Timeline assumption.** Today is 2026-05-15. **Council ratification 2026-05-15 (D11=a, corpus absorbed into team at ~1 day/week per track) accepts a ~2-month slip risk on P3.** Honest revised target: v1.0 GA Q1-2027 (was end-2026). Two-track parallelism is required; serial execution does not fit.

**Council ratification 2026-05-15.** All 25 council-decisions surfaced in this plan + sibling manifests (D1..D25) ratified by council on 2026-05-15. Decision log appended as §8.1 Ratification Log. Tier-1 ratifications (D1 reconciliation, D2 realistic bars, D3 two-track owner model) are operational immediately; Tier-2 (D11=a corpus path) shifts the P3 timeline as noted above.

---

> **Phase progress 2026-05-18.** Across the original 5-phase model:
>
> | Phase | Status |
> |---|---|
> | P0 — Prep | **CLOSED.** Marquee manifest with 3 status:real apps; eval directory structure; reference-LLM panel pinned; AGENTS.md drift fixed in commits fbffa175c + e42ff52b4. |
> | P1 — Quick wins | **CLOSED.** ACI default-on, retirement detectors + parity gate, `vox audit` umbrella with all 9 subcommands registered. |
> | P2 — Measurement infra | **PARTIAL (5/9 runners real).** ACI default (CR-L5), retirement (CR-L6), corpus-feedback (CR-L8), humaneval (CR-L1, 68841b39f), deploy doctor leg (CR-L7, ac9503761) all real. Still stub: SpecToApp (CR-L0), MensOnDistribution (CR-L2), RepairCorpus (CR-L3), PlanFidelity (CR-L4) — addressable per [`2026-05-18-v1-completion-plan.md`](../../superpowers/specs/2026-05-18-v1-completion-plan.md) §P1. |
> | P3 — Corpus engineering | **PARTIAL.** humaneval-vox 18/164 (seed landed, mining toward MV per plan §P3.1). repair-corpus, plan-fidelity, spec-to-app still 0 fixtures. Held-out guard real (5d27ca6f3). |
> | P4 — Hard CR-L work | **PARTIAL.** `vox repair --project` (08c086cc0), `vox doctor --project` (1761e0709), DeployRunner doctor leg (ac9503761), in-process @test (0c67eecb7), LLM-panel client+cache+retry (c5742f1cc + a7bf52785 + feaa0d7c8). Still open: `vox new` / `vox deploy` platform integration (P4.5), cross-file coordinated repair (P4.1 deeper). |
> | P5 — Hardening | Not yet started. |
>
> See [`docs/superpowers/specs/2026-05-18-v1-completion-plan.md`](../../superpowers/specs/2026-05-18-v1-completion-plan.md) for the remaining-work plan and effort estimates.

## §1 Phasing — P0 through P5

Five sequenced phases. Each phase lists its **gate** — the condition for moving to the next phase. Phases overlap; the gate is a logical predicate, not a calendar boundary.

### §1.1 P0 — Prep (weeks 1–2, target close: 2026-05-29)

**Goal.** Land the prerequisites without which no CR-L can be measured.

| Task | Owner role | Effort | Gates |
|---|---|---|---|
| **P0.1** Define `contracts/marquee/manifest.v1.yaml` listing 3–5 Marquee apps with feature inventory, fixture path, expected deploy/run behavior. | DevOps lead | 3 days | Closes G3. |
| **P0.2** Define `contracts/eval/` directory structure — subdirs `humaneval-vox/`, `repair-corpus/`, `plan-fidelity/`, `spec-to-app/`. README per subdir. | Corpus eng | 2 days | Closes G4 prereq. |
| **P0.3** Specify reference-LLM panel: `{MENS-current, Claude-Sonnet-current, GPT-current}`. Document version-pin policy + cost rate-card. | Council liaison | 2 days | Closes G5. |
| **P0.4** Draft `vox audit <thing>` CI contract (§4 below). One short doc; not yet implemented. | Language platform lead | 3 days | Closes G17. |
| **P0.5** Council review of CR-D ↔ CR-L reconciliation notes added 2026-05-15. | Council | 1 week | Closes G2. |
| **P0.6** Fix the stale [`AGENTS.md` §Grammar Unification](../../../AGENTS.md) claim that `workflow`/`activity` are "fully supported." Replace with truthful "reserved per ADR-028." | Language platform lead | 1 hour | Closes the immediate LLM-target footgun named in audit §3.5 + §7 OQ-4. |
| **P0.7** Assign owners across roles to CR-L0..CR-L8 (filling the role slots used throughout this plan). | Council | 1 week | Closes G13. |

**P0 gate.** Marquee manifest exists; eval directories exist; reference-LLM panel pinned; CI contract drafted; council has approved reconciliation; AGENTS.md drift fixed; owners assigned. Approximately 2 calendar weeks.

### §1.2 P1 — Quick wins (weeks 3–6, target close: 2026-06-26)

**Goal.** Land the cheap CR-L items that unblock measurement.

| Task | Owner role | Effort | Closes |
|---|---|---|---|
| **P1.1** Flip `OrchestratorConfig::agentos_aci_envelope_enabled` default to `true`. Add deprecation warning for explicit `false`. Migration shim emits guidance to any consumer reading the old shape. | Agent infra lead | 1 week | **CR-L5** |
| **P1.2** Ship 5 of ~10 retirement-guard detectors from [`AGENTS.md` §Retired Surfaces](../../../AGENTS.md) — start with `@component fn`, `@server fn`, `@query fn`, `@mutation fn`, `@py.import`. Each is one `vox-code-audit` detector following the [`anonymous_error.rs`](crates/vox-code-audit/src/detectors/anonymous_error.rs) template. | Language platform lead | 2 weeks | **CR-L6** (50%) |
| **P1.3** Generate `contracts/retirement/retired-surfaces.v1.yaml` from `AGENTS.md` §Retired Surfaces. Ship `vox ci retirement-audit` CI gate asserting every row has a wired detector and vice versa. | Language platform lead | 1 week | **CR-L6** parity gate |
| **P1.4** Land remaining 5 retirement-guard detectors (`recall()`, `@capacitor/*`, `axum::serve`, `rust-embed`, `vox-sherpa-transcribe`, TURSO env var aliases). | Language platform lead | 1 week | **CR-L6** (100%) |
| **P1.5** Implement `vox audit` umbrella CLI dispatching to per-thing subcommands; ship the contract from §4. Even stub subcommands return the right shape. | Language platform lead | 1 week | Closes G17 implementation. |

**P1 gate.** CR-L5 and CR-L6 measurably shipped. `vox audit` umbrella running with stub subcommands. Approximately 4 calendar weeks.

### §1.3 P2 — Measurement infrastructure (weeks 5–12, parallel with P1)

**Goal.** Make CR-L1, CR-L2, CR-L4, CR-L8 measurable. (Bar achievement comes in P4.)

| Task | Owner role | Effort | Closes |
|---|---|---|---|
| **P2.1** Ship `vox.lint.*` + `vox.repair.*` telemetry collectors emitting structured events with rule-id, severity, source-span, autofix-state, repair-outcome. | Runtime/repair lead | 2 weeks | **CR-L8** collector |
| **P2.2** Ship the quarterly export job (Phase 4 Task 7 of [`vox-language-rules-phase4-runtime-monitors-2026.md`](vox-language-rules-phase4-runtime-monitors-2026.md)). Emits `contracts/reports/corpus-feedback/<quarter>.json`. | Runtime/repair lead | 2 weeks | **CR-L8** export |
| **P2.3** CI gate: `contracts/reports/corpus-feedback/` artifact older than 90 days fails CI. | Runtime/repair lead | 2 days | **CR-L8** gate |
| **P2.4** Build `vox audit humaneval` runner: invokes reference-LLM panel against `contracts/eval/humaneval-vox/*.spec.toml`, scores compile + test-pass, emits report. | Corpus eng + Lang platform lead | 3 weeks | **CR-L1** harness |
| **P2.5** Build `vox audit mens-on-distribution` runner: samples MENS emissions against the full lint/audit/retirement suite, emits rate. | Corpus eng + Lang platform lead | 1 week (reuses P2.4) | **CR-L2** harness |
| **P2.6** Build `vox audit plan-fidelity` runner: drives orchestrator plan-mode against `contracts/eval/plan-fidelity/*.plan.toml`, scores success per fixture's stated criteria. | Agent infra lead | 3 weeks | **CR-L4** harness |
| **P2.7** Build `vox audit repair-corpus` runner: drives `vox repair .` against `contracts/eval/repair-corpus/<project>/`, scores final-state pass/fail per fixture's test suite. | Runtime/repair lead | 3 weeks | **CR-L3** harness |
| **P2.8** Reproducibility policy: pin temperature 0.0 + seed for all measurement runs; CR-L3 counts majority-success over 5 attempts per fixture. Documented in eval README. | Council liaison | 2 days | Closes **G6**. |

**P2 gate.** Five `vox audit` subcommands implemented and producing reports against placeholder fixtures. Approximately 7 calendar weeks (overlaps P1).

### §1.4 P3 — Corpus engineering (weeks 8–20, parallel with P2/P4)

**Goal.** Land the fixture corpora at sufficient quality and size for CR-L bars to be measured meaningfully.

| Task | Owner role | Effort | Closes |
|---|---|---|---|
| **P3.1** Mine `examples/golden/**` for HumanEval-Vox candidates. Spec out 164 problems (anchored to HumanEval-Python's count for direct comparability — see G18). Each problem = `name.spec.toml` (input prompt + reference solution + test cases). | Corpus eng | 6 weeks | **CR-L1** fixtures |
| **P3.2** Held-out subset of `humaneval-vox/`: explicitly mark 30 problems as `training_eligible: false` and verify MENS training pipeline excludes them. CI check in [`vox-corpus`](crates/vox-corpus/). | Corpus eng + MENS lead | 1 week (overlaps P3.1) | Closes **omission #4** (corpus contamination). |
| **P3.3** Build 50 `repair-corpus/` projects: each is a multi-file Vox project with deliberately introduced bugs (compile errors / type errors / test-failing logic / effect-row violations). Author with mechanical mutators where possible; hand-curate quality bar. | Corpus eng | 6 weeks | **CR-L3** fixtures (50% — minimum viable) |
| **P3.4** Build 50 `plan-fidelity/` fixtures: each a multi-step plan in `plan.toml` with success criteria (e.g., "produces a PR that satisfies test X"). Source from real orchestrator session transcripts where possible. | Agent infra lead + Corpus eng | 4 weeks | **CR-L4** fixtures (50%) |
| **P3.5** Build Marquee app fixtures: 3 apps living in `apps/marquee/{todo-crud, chat, dashboard}/` with manifest registration. Each compiles, deploys, and runs in CI today. | DevOps lead + DX | 4 weeks | **CR-L7** fixtures + closes **G3** content |
| **P3.6** Build 10 `spec-to-app/` fixtures: English spec → expected app shape. Spec format: `{name, prompt, success_criteria, max_cost_usd, max_iterations}`. Range from "todo CRUD" through "dashboard with auth." | Corpus eng + Agent infra lead | 4 weeks | **CR-L0** fixtures |
| **P3.7** Eval-data versioning policy: each `contracts/eval/<thing>/manifest.v1.yaml` carries a content hash; rate measurements are pinned to the hash. Re-measurement on hash change is mandatory and logged. | Corpus eng | 1 week | Closes G18 (sub-issue: versioning). |

**P3 gate.** 350+ fixtures landed across 5 corpora at v1-ready quality. Approximately 12 calendar weeks.

### §1.5 P4 — Hard CR-L work (weeks 12–28, parallel with P3 tail)

**Goal.** Achieve the bars. Most P4 tasks start once P2 measurement infra is up and P3 fixtures are at minimum-viable size (50%).

| Task | Owner role | Effort | Closes |
|---|---|---|---|
| **P4.1** Extend [`vox-cli/src/commands/repair.rs`](crates/vox-cli/src/commands/repair.rs) to project scope: walk the project, collect cross-file diagnostics, propose a coordinated fix set, apply with per-file budget, re-check at project level. Reuse single-file `vox repair` as the inner loop. | Runtime/repair lead | 8 weeks | **CR-L3** (multi-file capability) |
| **P4.2** Iterate `vox repair .` against `repair-corpus/` until ≥ 70% project pass + ≥ 90% single-file aim. Twiddle prompt, attempt budget, fix-application order. | Runtime/repair lead | 4 weeks (overlap with P4.1) | **CR-L3** bar |
| **P4.3** Iterate plan-mode (`plan_loop.rs`) against `plan-fidelity/` until ≥ 85% success. Most changes in `chat_tools/plan_loop.rs` and the underlying orchestrator dispatch. | Agent infra lead | 4 weeks | **CR-L4** bar |
| **P4.4** Ship `vox new` (extends [`phase1-build-targets-spec-2026.md`](phase1-build-targets-spec-2026.md) `vox init --kind` work). | DevOps lead + DX | 3 weeks | **CR-L7** part 1 |
| **P4.5** Ship `vox deploy` with structured JSON output, `vox.deploy.*` telemetry, OCI publish path. Reference: [`vox-deploy-codegen`](crates/vox-deploy-codegen/) crate is the codegen surface; needs CLI wiring + a default deployment target story (probably Fly.io or Railway for v1.0). | DevOps lead | 6 weeks | **CR-L7** part 2 |
| **P4.6** Ship `vox doctor` — top-level health check that runs `vox check`, `vox audit retirement`, `vox audit on-distribution` if a model is configured, validates installed runtime + CLI consistency. JSON output. | DevOps lead + Lang platform | 2 weeks | **CR-L7** part 3 |
| **P4.7** CI integration test: `vox new web → vox deploy → vox doctor` on each Marquee app fixture, under the 120-second [CR-P3] budget. | DevOps lead | 2 weeks | **CR-L7** gate |
| **P4.8** Build the **CR-L0 agent loop driver**: a `vox audit spec-to-app` subcommand that, per spec, spawns an autonomous agent (MCP-driven) with a fixed system prompt and lets it iterate against the reference-LLM panel. Cost-meters every LLM call. Reports per-spec outcome (final-state pass/fail + cost + iteration count). | Agent infra lead + Runtime/repair lead | 8 weeks | **CR-L0** harness |
| **P4.9** Iterate CR-L0 loop against `spec-to-app/` until ≥ 60% pass at ≤ $5/spec median cost. This iteration touches *everything* — orchestrator prompts, repair loop, plan-mode, retirement guards, generated-code quality. CR-L0 is the integration test; tuning it shakes out latent gaps in all other CR-L's. | All leads | Continuous through P4 | **CR-L0** bar |
| **P4.10** Measure CR-L1 / CR-L2 against the final corpora. These should "fall out" once P2.4/P2.5 are running and P3.1/P3.2 are at full size. | Corpus eng | 1 week per measurement run | **CR-L1**, **CR-L2** bars |

**P4 gate.** CR-L0..CR-L8 each have a measured number recorded in `contracts/reports/<thing>/2026-Q4.json`. Approximately 16 calendar weeks.

### §1.6 P5 — Hardening (weeks 24–32)

**Goal.** Bar-or-demote decisions; release.

| Task | Owner role | Effort | |
|---|---|---|---|
| **P5.1** Run all eight `vox audit` subcommands quarterly + on every release-candidate tag. Lock the report shape; ensure release notes auto-cite the report. | Lang platform lead | 1 week | |
| **P5.2** Per-CR-L bar-or-demote decision per §6 policy below. Council reviews actual numbers. | Council | 2 weeks | |
| **P5.3** Update [`AGENTS.md` §Retired Surfaces](../../../AGENTS.md), `docs/agents/vox-language-surface.v1.json` (to be created in P5.3), `.well-known/llms.txt` (to be created in P5.3), and `cli-command-surface.generated.md` to reflect v1.0 reality. | Lang platform lead | 1 week | |
| **P5.4** Publish v1.0 with full CR table including measured numbers (not just bars), CR-L0 result called out prominently. | Release manager | — | |

**P5 gate.** v1.0 GA.

---

## §2 Dependency DAG

```
                       P0 (prep)
                       /     \
                      v       v
                   CR-L5    CR-L6           ← P1 quick wins
                                            (independent)

                  ┌─ vox-language-rules
                  │   Phase 2 detector framework
                  v
            ┌─ CR-L6 detectors (P1.2–P1.4)
            v
       AGENTS.md retirement contract
            │
            v
   ┌── vox-language-rules Phase 4 Task 7
   │       (telemetry export)
   v
CR-L8 telemetry pipeline (P2.1–P2.3)
   │
   ├──────────────────┐
   v                  v
CR-L1 humaneval     CR-L2 on-distribution
harness (P2.4)      harness (P2.5)
   │                  │
   v                  v
CR-L1 fixtures      CR-L2 measurement run
164 problems        (depends on CR-L1
(P3.1, P3.2)         fixtures)
   │
   └──> CR-L1 bar (P4.10)

                  ┌─ plan_loop.rs (existing)
                  v
            CR-L4 harness (P2.6)
                  │
                  v
            plan-fidelity fixtures (P3.4)
                  │
                  v
            CR-L4 bar (P4.3)

                  ┌─ repair.rs (existing single-file)
                  v
            CR-L3 harness (P2.7)
                  │
                  v
            repair-corpus fixtures (P3.3)
                  │
                  v
            project-scope repair (P4.1, P4.2) → CR-L3 bar

                  ┌─ phase1-build-targets (vox init)
                  │
                  v
            vox new (P4.4)
                  │
                  v
            vox deploy (P4.5)  ──┐
                  │             │
                  v             │
            vox doctor (P4.6) ──┤
                                v
                          Marquee fixtures (P3.5)
                                │
                                v
                          CR-L7 CI integration test (P4.7)

                          ┌── all of the above ──┐
                          v                      v
                    spec-to-app fixtures    agent loop driver
                    (P3.6)                  (P4.8)
                          \                /
                           \              /
                            v            v
                          CR-L0 bar (P4.9)
                          ★ Integration test
```

**Critical-path insight.** CR-L0 sits at the bottom of the DAG. *Every* other CR-L feeds into it. CR-L0 cannot be tuned to its 60% bar until CR-L1..CR-L8 each contribute. Conversely, *CR-L0 results reveal which sub-CR-L is the weakest link* — making CR-L0 the most efficient debugging surface during P4.

**Parallelism analysis.**
- **P0 ⊥ everything** (sequential prereq).
- **P1.1 ⊥ P1.2 ⊥ P1.3** (independent quick wins).
- **P2 ⊥ P3** (measurement infra vs corpus engineering) — fully parallel.
- **P4.1+P4.2 (CR-L3) ⊥ P4.3 (CR-L4) ⊥ P4.4-P4.7 (CR-L7)** — three parallel tracks.
- **P4.8+P4.9 (CR-L0)** depends on all three above being at least at minimum-viable.

**Minimum staffing.** Three working tracks:
- Track A (Lang platform): CR-L1, CR-L2, CR-L6, CR-L8 infra
- Track B (Runtime/repair + Agent infra): CR-L3, CR-L4, CR-L0
- Track C (DevOps + DX): CR-L7, Marquee fixtures

Plus a Corpus engineer floating across A/B/C.

---

## §3 Fixture-Corpus Budget

Total: **~340 high-quality fixtures across 5 corpora, ~16 person-weeks of corpus engineering.** This is the single largest invisible cost in the audit doc and the most likely scope-slip vector.

| Corpus | Count | Per-fixture cost (hours) | Total (hours) | Mining shortcuts |
|---|---|---|---|---|
| `humaneval-vox/` | 164 | 2 (mostly mechanical from `examples/golden/`) | 328 | High — existing `@example` blocks + mechanical mutators |
| `humaneval-vox/` held-out | (30 of the 164) | +1 hour to verify exclusion from MENS training | 30 | Just a flag flip + CI check |
| `repair-corpus/` | 50 | 8 (each is a multi-file project with introduced bugs that compile-or-test-fail) | 400 | Medium — `ast_mutator` from [vox-corpus](crates/vox-corpus/) can introduce bugs at scale; quality bar still requires hand review |
| `plan-fidelity/` | 50 | 4 (each is a plan + success criteria + reference outcome) | 200 | Medium — mine real orchestrator transcripts |
| `marquee/` apps | 3–5 | 80 (each is a working app with tests + deploy config) | 320 | Low — these are real apps |
| `spec-to-app/` | 10 | 16 (each is a curated English spec with success criteria) | 160 | Low — depends on Marquee apps existing first |
| **Total** | ~280 | | **~1438 hours** | |

**Conversion.** At ~30 productive hours/week per engineer, ~48 person-weeks. Across 1 corpus engineer + 0.25 FTE help from other leads, ~12 calendar weeks if started 2026-06-01.

**Council ratification 2026-05-15 (D11=a, "corpus absorbed into team").** No dedicated corpus engineer hired or contracted. Corpus work absorbed into both project tracks at ~1 day/week per track (~16 hours/week combined). At ~16 hours/week against ~1438 hours, P3 stretches from ~12 to **~24 calendar weeks** (≈ 6 months). The council explicitly accepts a ~2-month slip risk on P3; honest accounting puts it closer to 4 months relative to the original 12-week plan. v1.0 GA target accordingly revises from end-2026 to **Q1-2027**.

**Owner (post-ratification).** Both tracks share corpus responsibility. Compiler/Lang track owns `humaneval-vox/` and `repair-corpus/` fixtures; Agent/Runtime/CLI/Corpus track owns `plan-fidelity/`, `spec-to-app/`, and the Marquee app fixtures.

**Held-out subset rationale.** 30 of the 164 HumanEval-Vox problems must be `training_eligible: false` and verified as never having been ingested by the MENS training pipeline. This is the corpus-contamination guard (audit doc omission #4). Without it, the 80% CR-L1 number is leaked-evaluation marketing.

**Versioning.** Each `contracts/eval/<thing>/manifest.v1.yaml` carries a `corpus_hash: <blake3>` derived from the sorted content hashes of its fixtures. CR-L reports always cite the corpus hash they measured against. Adding/removing/editing a fixture requires a new hash and a re-measurement run with both old and new hash reported.

---

## §4 CI Contract — `vox audit <thing>`

Single contract for all CR-L measurement subcommands.

### §4.1 Surface

```
vox audit <thing> [--json | --markdown | --html] [--baseline=<path>] [--threshold=<num>] [--corpus=<path>] [--llm-panel=<spec>]
```

### §4.2 Behavior

- **Default output.** `--json`. Single JSON object printed to stdout.
- **Report file.** Always written to `contracts/reports/<thing>/<date>.json`, regardless of stdout format. Atomic write.
- **Exit codes.**
  - `0` — measurement complete, bar met (if `--threshold` was set), or no threshold set.
  - `1` — measurement complete, bar not met.
  - `2` — infrastructure error (corpus missing, LLM panel unreachable, etc.). Does not block CI on its own; logs to telemetry and skips.
- **Telemetry.** Every run emits `vox.audit.<thing>` event with: corpus hash, llm panel version, duration, outcome, cost (if applicable).
- **Baseline comparison.** `--baseline=<path>` reads a prior report and emits delta in the result.

### §4.3 Report JSON shape (canonical)

```json
{
  "thing": "humaneval-vox",
  "schema_version": 1,
  "measured_at": "2026-Q4-2026-12-15T12:00:00Z",
  "corpus_hash": "blake3:abc123...",
  "corpus_size": 164,
  "llm_panel": [
    {"id": "mens-current", "version": "v0.6.1"},
    {"id": "claude-sonnet", "version": "claude-sonnet-4-7-20260801"}
  ],
  "reproducibility": {"temperature": 0.0, "seed": 42, "attempts_per_fixture": 5},
  "results": {
    "overall_pass_rate": 0.84,
    "median_pass_rate": 0.83,
    "per_llm": [
      {"id": "mens-current", "pass_rate": 0.81, "median_cost_usd": 0.02},
      {"id": "claude-sonnet", "pass_rate": 0.88, "median_cost_usd": 0.18}
    ]
  },
  "threshold": {"target": 0.80, "met": true},
  "delta_vs_baseline": {"baseline_hash": "blake3:def456...", "absolute": 0.03, "relative_pct": 3.7}
}
```

### §4.4 Subcommand inventory

| Subcommand | CR-L | Notes |
|---|---|---|
| `vox audit spec-to-app` | CR-L0 | Cost-metered. Required for v1.0 GA. |
| `vox audit humaneval` | CR-L1 | Median-of-panel scoring. |
| `vox audit mens-on-distribution` | CR-L2 | Single-model (MENS-current). |
| `vox audit repair-corpus` | CR-L3 | Majority-of-5 attempts. |
| `vox audit plan-fidelity` | CR-L4 | Reuses orchestrator MCP. |
| `vox audit aci-default` | CR-L5 | Boolean check; returns immediately. |
| `vox audit retirement` | CR-L6 | Reads `contracts/retirement/retired-surfaces.v1.yaml` and verifies wiring. |
| `vox audit deploy` | CR-L7 | Drives `vox new → vox deploy → vox doctor` on Marquee fixtures. |
| `vox audit corpus-feedback` | CR-L8 | Checks artifact age + shape. |

**Implementation footprint.** A single `vox-audit` crate with a `Subcommand` trait. Each CR-L registers an impl. CI runs `vox audit all` which fans out and emits a combined report.

---

## §5 Risk Register (CR-L Specific)

| # | Risk | Likelihood | Impact | Mitigation | Owner |
|---|---|---|---|---|---|
| **R1** | **Corpus contamination.** MENS training ingested `examples/golden/`; HumanEval-Vox built from same source = leaked eval. | High | Severe — measurement becomes marketing | Held-out subset of 30 problems with `training_eligible: false`; CI gate verifies exclusion (P3.2). | Corpus eng + MENS lead |
| **R2** | **Reference-LLM rate limits / cost during eval runs.** Running CR-L0 against 10 specs × 5 attempts × panel-of-3 = 150 sessions per release-candidate. | Medium | Major — CI breaks or budget overruns | Cache identical-input responses; rate-limit retry budget per spec; alert if monthly LLM cost exceeds ceiling. | DevOps lead |
| **R3** | **Doc-drift recurrence** (AGENTS.md vs pipeline.rs again). | Medium | Major — re-introduces the LLM-target footgun | CR-L6's `vox ci retirement-audit` gate (P1.3). Plus a quarterly manual audit of AGENTS.md vs codebase per [`docs-reality-audit-program`](../contributors/docs-reality-audit-program.md). | Council liaison |
| **R4** | **Council rejects CR-D ↔ CR-L reconciliation.** | Low | Major — full plan must be re-shaped | P0.5 council review block; resolve before P1 starts. | Council |
| **R5** | **Marquee-app definition disagreement.** Three apps too few? Five too many? Wrong feature mix? | Medium | Major — CR-L7 + CR-P1 unverifiable until resolved | P0.1 publishes manifest for review; council approves before P3.5 starts. | DevOps lead + Council |
| **R6** | **CR-L0 measured at 40–60%.** Either we ship at the lower bar (and dilute the v1.0 claim) or we slip GA. | High | Severe — central marketing claim depends on it | Sub-bar at 40% (block GA per §6 below); reserve 4 weeks of buffer in P4 for CR-L0 iteration; treat CR-L0 as continuous-tuning during P4. | All leads |
| **R7** | **ACI default-on (CR-L5) breaks existing IDE consumers** (Cursor / Antigravity / others reading old shape). | Medium | Moderate — release-note migration cost | Deprecation warning for one minor version before enforce; coordinate with downstream IDE vendors via the agent-feature-matrix channel. | Agent infra lead |
| **R8** | **`vox audit` subcommand sprawl.** Each CR-L's runner is built by a different lead → inconsistent shapes. | Medium | Moderate — debugging cost amplifies | P0.4 + P1.5 enforce single contract; PR template requires conforming to §4 shape; CI gate validates report JSON against schema. | Lang platform lead |
| **R9** | **Repair-corpus rot.** Real-world bugs don't match the 50 hand-crafted fixtures; CR-L3 70% on the corpus ≠ 70% on real broken projects. | Medium | Moderate — measurement validity | Quarterly refresh of 5 fixtures with bug patterns mined from `vox.repair.*` telemetry (CR-L8 feedback closes this loop). | Runtime/repair lead |
| **R10** | **Latency/cost ceilings not budgeted into CI compute.** | High | Major — CI either breaks or runs only manually | CR-L0 cost-ceiling ($5/spec) bounds per-run cost; CI compute budget per release-candidate quantified in P0.3 LLM panel doc. | DevOps lead |
| **R11** | **Fixture-corpus engineering slips.** No owner named → invisible work → 2+ month slip. | High | Severe — entire v1.0 timeline | Name owner in P0.7; treat as blocking-prereq; weekly progress check during P3. | Council + DevOps lead |
| **R12** | **MENS reaches v0.6 with poor on-distribution rate** (CR-L2 < 95%). CR-L0 caps at MENS's CR-L2. | Medium | Major — CR-L0 hits ceiling unrelated to language design | Run CR-L0 measurement against full panel; if MENS is the weakest panelist, demote MENS to "candidate" tier and report panel-median as primary number. | MENS lead |
| **R13** | **Council bandwidth.** P0.5 + P5.2 + open-question resolution all require council. | Medium | Moderate — phase gates slip | Bundle council asks into 2 fixed reviews (start of P0, end of P4). Async-decide via the Foundation's existing rubric. | Council liaison |

---

## §6 Rollback / Demotion Policy

For each CR-L, define what happens if the bar isn't met by GA.

| CR-L | Bar | Sub-bar (block GA) | If measured between bar and sub-bar | If measured below sub-bar |
|---|---|---|---|---|
| **CR-L0** | ≥ 60% pass at ≤ $5/spec | < 40% pass **or** > $10/spec median | Ship at observed rate; flag prominently in release notes; immediate v1.1 plan to close gap | **Block GA.** This is the integration test for the v1.0 claim. |
| **CR-L1** | ≥ 80% HumanEval-Vox | < 60% | Ship at observed rate; release note explains panel median; quarterly re-measurement gate | Demote to v1.1; ship v1.0 without the claim |
| **CR-L2** | ≥ 95% on-distribution | < 85% | Ship at observed rate; corpus-curation plan filed | Demote to v1.1 |
| **CR-L3** | ≥ 70% project / ≥ 90% single-file | < 50% project | Ship at observed; CR-D2 90% number stays as the aspirational reference | Demote project-scope to v1.1; ship single-file only |
| **CR-L4** | ≥ 85% plan fidelity | < 65% | Ship at observed; iterate plan-mode prompts in v1.0.x patches | Demote CR-D1 measurement to v1.1 |
| **CR-L5** | Binary: default-on | n/a | Must land. No demotion. | **Block GA** if not landed |
| **CR-L6** | Binary: parity gate passing | n/a | Must land. No demotion. | **Block GA** if not landed |
| **CR-L7** | Binary: all three commands + CI integration test passing on Marquee fixtures | n/a | Must land all three. No demotion. | **Block GA** if any one missing |
| **CR-L8** | Binary: pipeline running, < 90 days stale | n/a | Must land. No demotion. | **Block GA** if pipeline not running |

**Summary.** Three criteria are binary v1.0 gates (CR-L5, CR-L6, CR-L8 + CR-L7 sub-bar). Two are integration must-haves (CR-L0 sub-bar, CR-L3 sub-bar). The rest can demote with public release-note honesty.

**The demotion policy itself** is a v1.0 claim. We are saying: "If we miss a number, we say so plainly." This is half the credibility of the LLM-target positioning. A council that quietly removes a CR-L when it doesn't measure well destroys the entire framework.

**Demotion publishing form (D18, ratified 2026-05-15).** Per council decision:

- **Release-note line is the minimum form** for every demoted CR-L. One paragraph: criterion name, target bar, measured number, decision (demote / partial / re-baseline), rationale, follow-on plan reference.
- **Full post-mortem is required** when any CR-L demotes by more than 10 percentage points below its bar. Post-mortem lives at `docs/news/<release-date>-postmortem-<crl-id>.md` and includes timeline, what we tried, what we learned, what we'll change.
- **Quarterly re-measurement post-v1.0 (D19, ratified 2026-05-15).** All eight `vox audit` subcommands re-run quarterly against the latest panel pin. Numbers published in v1.0.x patch release notes; if any number drifts > 10pp below the v1.0 GA measurement, the relevant CR-L gets a quarterly post-mortem regardless of its v1.0 demotion status.

---

## §7 New Criteria Proposals

### §7.1 CR-L0 (adopted 2026-05-15)

Already added to [`v1-release-criteria.md`](v1-release-criteria.md) §5. Full text reproduced for convenience:

> **[CR-L0] End-to-End Agent Authorship Loop**: Given a canonical English spec from `contracts/eval/spec-to-app/` (10–20 specs of increasing complexity), an autonomous agent loop driving Vox (via MCP) must produce a passing application — `vox check` clean, tests pass, `vox deploy` succeeds, `vox doctor` green — at ≥ 60% success rate with a per-spec token-cost ceiling of ≤ $5.00 against the panel reference LLMs. **This is the integration test for the v1.0 LLM-target claim; CR-L1..CR-L8 are unit tests of its sub-loops.** Sub-bar (block GA): observed rate < 40%.

**Rationale for the numbers.** A naive product of the other CR-L bars (85% planning × 95% on-distribution × 70% project repair × 95% deploy ≈ 54%) suggests 60% is mildly ambitious but reachable. $5/spec at 2026-05 panel rates allows ~50K input + 10K output tokens — enough headroom for 5–10 LLM round trips per spec.

**Why this is the most important CR-L.** It is the only one that exercises the *interaction* between Vox's primitives and a real agent. Every other CR-L can pass while CR-L0 fails — that would mean Vox is locally good but doesn't compose into an agentic workflow. CR-L0's measurement reveals exactly where the composition breaks.

### §7.2 Deferred-but-tracked (CR-L9..CR-L12, not adopted for v1.0)

Per council scoping decision 2026-05-15. Each is a real gap; each is tracked under an existing follow-on plan rather than expanded into v1.0 criteria:

| Proposed | Topic | Tracked under |
|---|---|---|
| CR-L9 | Endpoint auth coverage (`@auth` or `@public` on every `@endpoint`) | Should become a `vox-code-audit` rule + CI denial in v0.6; not v1.0-gated |
| CR-L10 | LSP ↔ CLI diagnostic parity | [`vox-lsp-capabilities-ssot-2026.md`](vox-lsp-capabilities-ssot-2026.md) follow-on |
| CR-L11 | Emit-correctness gate (Vox→TS→React smoke vs rolling upstream) | [`vox-react-backend-interop-audit-2026.md`](vox-react-backend-interop-audit-2026.md) follow-on |
| CR-L12 | Latency budget (`vox check` p99 / `vox repair` median) + per-repair cost ceiling | Sub-bullet under CR-E in a future revision of [`v1-release-criteria.md`](v1-release-criteria.md); partial coverage via CR-L0's $5/spec ceiling |

These are deliberately *not* v1.0 criteria. Adding them would either inflate scope past end-2026 or dilute the bar height of the criteria that are kept.

---

## §8 Open Questions Distilled

**Status: 22 of 25 ratified by council 2026-05-15.** Ratification log in §8.1 below. Items 3 carried open (mostly because they only become actionable later in the timeline).

Carried forward from [`vox-as-llm-target-audit-and-plan-2026.md`](vox-as-llm-target-audit-and-plan-2026.md) §7 plus new ones surfaced by this plan:

1. **OQ-1 (carried).** Is the bar height right at "realistic v1.0"? Numbers in this plan assume yes. Council confirmation needed at P0.5.
2. **OQ-2 (carried).** CR-L items append-only vs replace CR-D1/D2/D3? This plan reconciles via "point to CR-L for measurement, keep CR-D as policy lineage." Council confirmation at P0.5.
3. **OQ-3 (carried).** Does v1.0 include the mesh? This plan **does not gate on mesh**. Confirmation needed: is that the right framing?
4. **OQ-4 (carried, partially closed).** AGENTS.md §Grammar Unification corrective edit — P0.6 closes this.
5. **OQ-5 (carried).** "Humans become orchestrators, not authors" as v1.0 marketing or v1.x stretch? This plan treats it as v1.0 *marketing* (CR-L0 is the integration test) but *not* as a measurable "0% human-edit ratio" gate. Council to confirm.
6. **OQ-6 (carried, closed).** Eval corpus location — confirmed `contracts/eval/`.
7. **OQ-7 (new).** Reference-LLM panel composition and version-pin policy. P0.3 surfaces this for council approval.
8. **OQ-8 (new).** Corpus engineer hiring / staffing decision. P0.7 surfaces this; if no full-time corpus engineer can be staffed, P3 timeline doubles.
9. **OQ-9 (new).** Marquee-app composition (which 3–5 apps?). P0.1 publishes manifest for council approval.
10. **OQ-10 (new).** CR-L0 cost ceiling ($5/spec) — does it reflect 2026-Q4 panel rates, or do we re-baseline at GA?
11. **OQ-11 (new).** What happens when MENS reaches v1.0 quality after v1.0 ships? Re-measure all CR-L's quarterly? Update bars?
12. **OQ-12 (new).** Demotion policy in §6 is council-controllable; do we publish demotions as a release-note line or a full post-mortem? The former is honest minimum; the latter builds credibility.

---

## §8.1 Ratification Log (Council, 2026-05-15)

Twenty-five decisions surfaced across this plan + sibling manifests; council ratified 22 in a single batch and noted 3 as "decide when actionable." This log is the canonical record. Where a decision contradicts an earlier draft in this doc or in a manifest, this log governs and the upstream file has been updated to point here.

### Tier 1 — Operational immediately

| # | Decision | Ratification |
|---|---|---|
| **D1** | CR-D ↔ CR-L reconciliation | CR-D1/D2/D3 are policy lineage; CR-L is measurement source of truth. No duplicate CI gates. Reflected in [`v1-release-criteria.md`](v1-release-criteria.md) §4 amended paragraphs. |
| **D2** | Bar height — realistic v1.0 vs aspirational | Realistic bars hold: 60/80/95/70/85 (CR-L0..L4). Aspirational pushes v1.0 to 2027+ and risks measurement-bending no-ship pressure. |
| **D3** | Owner role assignments | Collapse to two tracks: (a) Compiler/Lang/Detector and (b) Agent/Runtime/CLI/Corpus. Corpus staffing path picked separately in D11. |

### Tier 2 — Blocks P3 corpus engineering

| # | Decision | Ratification |
|---|---|---|
| **D4** | Marquee slot 2 | Todo-CRUD with `@auth` (archetype `marquee-todo-auth`). Reflected in [`contracts/marquee/manifest.v1.yaml`](../../../contracts/marquee/manifest.v1.yaml). |
| **D5** | Marquee slot 3 | Realtime chat with actor primitives (archetype `marquee-chat`). |
| **D6** | Marquee slot count | 3 apps (CR-P1 minimum). No expansion. |
| **D7** | LLM panel composition | 3-member panel as drafted (MENS-current + Claude-Sonnet + GPT-frontier). Reflected in [`contracts/eval/llm-panel.v1.yaml`](../../../contracts/eval/llm-panel.v1.yaml). |
| **D8** | Panel rebaseline cadence | Rebaseline at release-candidate tag. |
| **D9** | MENS self-reference | Report separately for CR-L0; include in panel median for CR-L1/L2/L4. |
| **D10** | HumanEval-Vox count anchor | 164 problems (matches HumanEval-Python). |
| **D11** | Corpus engineering staffing | **(a) Absorb into both tracks at ~1 day/week each.** Accepts ~2-month slip risk on P3; honest accounting closer to 4 months. v1.0 GA target revises end-2026 → Q1-2027. |

### Tier 3 — Affects later phases or framing

| # | Decision | Ratification |
|---|---|---|
| **D12** | CR-L0 cost ceiling rebaseline at GA | Yes, rebaseline at RC against panel pricing. |
| **D13** | Repair-corpus iteration & cost budget | 10 outer attempts × $1.50/project median. |
| **D14** | Plan-fidelity iteration budget | 5 plan iterations × $0.75/plan. |
| **D15** | Spec-to-app iteration budget | 25 iterations × $5/spec. |
| **D16** | Mesh in v1.0? | **Mesh Phase 2 LAN demoted from v1.0 to v1.1.** v1.0 = single-machine + LLM-target only. v0.6/v0.7 targets unchanged. Reflected in [`mesh-and-language-distribution-ssot-2026.md`](mesh-and-language-distribution-ssot-2026.md) banner. |
| **D17** | "Humans as orchestrators" framing | v1.0 *marketing*, NOT a measurable percentage gate. CR-L0 is the closest measurable proxy. |
| **D18** | Demotion publishing form | Release-note line minimum; full post-mortem if >10pp below bar. §6 amended. |
| **D19** | Post-v1.0 re-measurement cadence | Quarterly. §6 amended. |
| **D20** | ACI default-on migration | Flip default to `true` in v0.6 with one-minor deprecation warning, NOT in v0.5.x. Coordinate via `docs/agents/ai-ide-feature-matrix-2026.json` channel. |

### Tier 4 — Operational / process

| # | Decision | Ratification |
|---|---|---|
| **D21** | `vox-audit` home crate | New top-level `vox-audit` crate (not a vox-cli submodule). Reflected in [`contracts/ci/vox-audit-contract.v1.yaml`](../../../contracts/ci/vox-audit-contract.v1.yaml). |
| **D22** | Exit-code-2 semantics | Infrastructure error logs telemetry, does NOT block CI. |
| **D23** | Canonical baseline | Latest tagged release; manual-pin reserved for emergency rollback. |
| **D24** | Plan-fidelity Wave 1/2/3 taxonomy | Accepted as drafted (Wave 1 single-file, Wave 2 multi-file with decorator discipline, Wave 3 cross-cutting). |
| **D25** | HumanEval-Vox mutation policy | AST-mutations count toward held-out budget only if source was hand-authored AND held-out. Mutations of public examples remain training-eligible. Reflected in [`contracts/eval/humaneval-vox/manifest.v1.yaml`](../../../contracts/eval/humaneval-vox/manifest.v1.yaml). |

### Items intentionally carried forward (decide when actionable)

- **OQ-3 mesh framing detail** — resolved at high level via D16; specific phase-by-phase v1.1 sequencing deferred to a future mesh SSOT revision.
- **OQ-11 post-v1.0 cadence specifics** — resolved at "quarterly" via D19; specific publication channel and bar-drift thresholds tuned during P5 hardening.
- **OQ-12 demotion form** — resolved at high level via D18; per-criterion templates for post-mortem authoring will be added as the first demotion (if any) occurs.

---

## §9 Quick reference: per-CR-L summary

| CR-L | Bar | Owner role | Phase | Status (2026-05-15) | Critical dep |
|---|---|---|---|---|---|
| CR-L0 | ≥ 60% pass + ≤ $5/spec | Agent infra + Runtime/repair | P4 | Proposed | All other CR-L's at minimum-viable |
| CR-L1 | ≥ 80% HumanEval-Vox | Lang platform + Corpus eng | P2 harness, P3 corpus, P4 measure | Proposed | 164-problem corpus + held-out 30 |
| CR-L2 | ≥ 95% on-distribution | Lang platform + MENS lead | P2/P4 | Proposed | Reuses CR-L1 corpus; MENS quality |
| CR-L3 | ≥ 70% multi-file / ≥ 90% single-file | Runtime/repair | P2/P3/P4 | Proposed | 50-project repair-corpus |
| CR-L4 | ≥ 85% plan-mode fidelity | Agent infra | P2/P3/P4 | Proposed | 50-plan fixtures |
| CR-L5 | Default-on ACI envelope | Agent infra | P1 (week 3–4) | Proposed | None — single config flip |
| CR-L6 | Retirement-guard parity with AGENTS.md | Lang platform | P1 (weeks 3–6) | Proposed | 10 retired-pattern detectors |
| CR-L7 | vox new + deploy + doctor E2E | DevOps + DX | P4 | Proposed | Marquee app fixtures |
| CR-L8 | Telemetry export pipeline running | Runtime/repair | P2 (weeks 5–9) | Proposed | Phase 4 Task 7 of vox-language-rules |

---

## §10 Where to look next

- **CR-L0 spec source:** [`v1-release-criteria.md`](v1-release-criteria.md) §5 (adopted 2026-05-15).
- **CR-L1..CR-L8 audit context:** [`vox-as-llm-target-audit-and-plan-2026.md`](vox-as-llm-target-audit-and-plan-2026.md) — §2 evidence, §3 gaps, §5 criteria detail, §9 self-critique.
- **Phase plans this implementation plan pegs to:**
  - [`vox-language-rules-phase2-lint-extension-2026.md`](vox-language-rules-phase2-lint-extension-2026.md) — CR-L1 `@example` decorator, CR-L6 detector framework
  - [`vox-language-rules-phase4-runtime-monitors-2026.md`](vox-language-rules-phase4-runtime-monitors-2026.md) — CR-L8 telemetry export (Task 7)
  - [`phase1-build-targets-spec-2026.md`](phase1-build-targets-spec-2026.md) — CR-L7 vox new lineage
  - [`agentos-ssot-2026.md`](agentos-ssot-2026.md) — CR-L5 envelope contract
  - [`mens-training-ssot.md`](mens-training-ssot.md) — held-out corpus guard (R1 mitigation)
- **External orientations:**
  - HumanEval (Chen et al., 2021) — 164 problems; CR-L1 anchored here
  - GRPO (Shao et al., 2024) — not gated for v1.0 but referenced in CR-L8 as the future closed-loop direction

---

*Implementation plan dated 2026-05-15. Next review: end of P0 (2026-05-29) for council go/no-go.*
