# Vox Orchestrator — Autonomous Behavior Policy: Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement the per-phase plans referenced below. This master plan is the *index and contract*; each phase has its own TDD-detailed plan file. Steps in phase plans use checkbox (`- [ ]`) syntax for tracking.

**Goal.** Convert [`docs/src/architecture/autonomous-orchestration-policy-research-2026.md`](../../../src/architecture/autonomous-orchestration-policy-research-2026.md) into running code: the orchestrator decides *autonomously and on the user's behalf* when to switch model tier, plan vs act, invoke research (Socrates), spawn sub-agents, escalate to HITL, and recover from runaway loops — under explicit, contract-defined decision rules with telemetry and HITL fallback at every gate.

**Architecture.** Eleven sequential phases on the existing `crates/vox-orchestrator` surface. Phase 1 lands instrumentation, fixtures, benchmarks, and contract scaffolds with **zero behavior change**. Phases 2–10 each ship one decision rule (D6 → D3 → D1 → D2 → D5/D9 → D8 → D7 → D10 → D4 — ordered by dependency). Phase 11 is hygiene consolidation. Every phase passes the same five quality gates (§3) and updates the same telemetry contract (`research_metrics_contract.rs`).

**Tech Stack.** Rust workspace; `vox-orchestrator` crate (L3, no LoC cap, max_dependents=25); `vox-orchestrator-types::socrates_policy` (L0, pure types); `vox-db` (telemetry & DB ops); `vox-secrets` (secrets); `vox-arch-check` (layer enforcement); `vox-doc-pipeline` (docs); `criterion` (benchmarks); `proptest` (property tests where applicable); `serde_yaml` for contract files.

---

## 0. How To Read This Plan

**Index.** This master plan is *not* a TDD step list. It is:
1. The **standards contract** every phase must honor (§3).
2. The **dependency graph** that orders the phases (§4).
3. The **success matrix** that gates phase completion (§5).
4. The **risk register** and rollback policy (§6).
5. The **glossary** so phase plans can use shorthand (§7).
6. Pointers to the **per-phase TDD plans** (§4.1–4.11), each its own file in `docs/superpowers/plans/`.

**Reading order for the agent executing this:**
1. Read this file end-to-end.
2. Read [`docs/src/architecture/autonomous-orchestration-policy-research-2026.md`](../../../src/architecture/autonomous-orchestration-policy-research-2026.md) Parts 1, 11, 12, 13.
3. Open the current phase plan (e.g., `2026-05-08-orchestrator-phase-1-standards-and-baseline.md`).
4. Execute its tasks linearly. Do not jump ahead — phases have dependencies.

**Definition of "done" for the program.** Every decision in the research doc's Part 1 table (D1–D10) has a contract YAML in `contracts/orchestration/`, a code path that reads the contract, an integration test that exercises the path, a benchmark that proves the path is below its perf budget, and a row in `where-things-live.md`. The full `vox-arch-check`, telemetry conformance check, and `vox-doc-pipeline --check` pass on `main`.

---

## 1. Why This Plan Exists, In One Paragraph

The orchestrator today has the *mechanisms* for routing, planning, compaction, Socrates gating, and pub/sub coordination, but the *triggers* — when to use which mechanism — are spread across hand-tuned heuristics in different files, with no single contract describing the policy. The research doc identified ten discrete decision axes (D1–D10) whose triggers should be codified and observable. This plan codifies them as YAML contracts loaded by the orchestrator at startup, instrumented end-to-end via `vox-db`'s telemetry surface, and gated by HITL where automation is unsafe. The plan's secondary purpose is to clean up the orchestrator (the user called it "wrangling") — the plan therefore tightens `vox-arch-check` rules, adds the workspace's first orchestrator benchmark suite, and establishes a perf baseline that subsequent changes must not regress.

---

## 2. Anti-Goals

These are *out of scope* for this plan. Every phase plan must reject scope creep that touches them:

1. **Replacing the existing routing logic.** `ModelRegistry::best_for_task()` stays. We layer new triggers *on top*; we don't rewrite Thompson sampling or the catalog refresh.
2. **Replacing Socrates with a new system.** ADR-005 stands. We extend `ConfidencePolicy` and add the fusion function from the research doc §4 — we do not replace `RiskBand`/`RiskDecision`.
3. **Re-organizing crate boundaries.** `vox-orchestrator` is already L3. New modules go inside it; new crates only when explicitly justified in a phase plan with an ADR.
4. **A new MCP tool surface.** The existing `vox_plan` and chat_tools surface stays. New behavior is reachable through the same tools with new internal triggers.
5. **A general "auto" mode that subsumes user intent.** Per `autonomous-orchestration-policy-research-2026.md` Part 12, ambiguous-intent disambiguation, money/external messaging, and compliance-tagged actions stay HITL by default.
6. **Cross-language reimplementation.** All glue scripts are `.vox` per AGENTS.md §VoxScript-First. No new `.ps1`/`.sh`/`.py`.
7. **Regenerating auto-generated docs by hand.** `SUMMARY.md`, `architecture-index.md`, `feed.xml`, `*.generated.md`, `.cursorignore` are tool-regenerated. Every phase plan ends with a `cargo run -p vox-doc-pipeline` step.

---

## 3. The Five Quality Gates

**Every phase must pass all five before merging.** A phase plan with checkboxes for these is incomplete unless these five appear in order at the end.

### Gate G1: Architecture conformance — `vox-arch-check`

```bash
cargo run -p vox-arch-check
```

Must exit 0. The phase may *introduce* new tightenings to `docs/src/architecture/layers.toml` (e.g., promote a warn → error) but must not bypass existing tightenings.

If a phase adds a new module to `vox-orchestrator`, it must be **categorized in `where-things-live.md`** in the same commit. AGENTS.md treats `where-things-live.md` updates as a release blocker.

### Gate G2: Telemetry conformance

Every new decision point must emit a `metric_type` row to `llm_interactions` or `model_route_event` per the constants in [`crates/vox-db/src/research_metrics_contract.rs`](../../../../crates/vox-db/src/research_metrics_contract.rs). New `metric_type` constants need:

1. A new `pub const METRIC_TYPE_*` in `research_metrics_contract.rs`.
2. The OTel GenAI SemConv mapping (see research doc §10.3) in a doc comment.
3. A test in `crates/vox-db/tests/` asserting the metric is written when the decision fires.

### Gate G3: Performance budget

Every phase has a perf budget in §5 below. The phase plan must include a `criterion` benchmark that asserts the budget. Budgets are *enforced via test*, not just measured: the bench file ends with a `#[bench]`-style assertion that fails the build if p99 exceeds budget by >20%.

```rust
// at end of bench file:
assert!(p99_ns < BUDGET_NS * 12 / 10, "p99 {p99_ns}ns > budget {BUDGET_NS}ns + 20%");
```

The baseline numbers (Phase 1 captures them) live in `docs/src/architecture/orchestrator-perf-baseline-2026.md`. Every subsequent phase appends its own row.

### Gate G4: Contract conformance

Every new YAML/JSON contract under `contracts/orchestration/` must:

1. Have a sibling `*.schema.json` (JSON Schema 2020-12).
2. Be loaded via `serde_yaml` + schema validation in a test (`crates/vox-orchestrator/tests/contract_load_*.rs`).
3. Be referenced from `where-things-live.md`.
4. Be versioned: new files are `*.v1.yaml`; breaking changes bump to `v2`. Old schemas stay until consumers migrate.

### Gate G5: HITL fallback present

Every automated decision must have an `escalate` arm that is reachable, tested, and surfaces a structured `EscalationEvent` to the bulletin board. *Per research doc §12, automation must never silently swallow uncertainty.* The phase plan must include at least one test asserting that when confidence is below the per-axis abstention threshold, the path lands at `EscalationEvent` not at a "best-guess" continuation.

---

## 4. Phase Sequence

Phases are dependency-ordered. The graph:

```
P1 ── P2 ─┬─ P3 ──┬─ P4 ──┬─ P5 ──┬─ P6 ──┬─ P7 ──┬─ P8 ──┬─ P9 ──┬─ P10 ── P11
           └─ (any)         (any)  ...    (parallelizable after P3 lands)
```

Strict prerequisites:
- **P1** must land before any other phase. It establishes fixtures, benches, baselines, and arch-check tightenings.
- **P2** (circuit breaker / D6) must land before P3 because the doom-loop detector is the safety net inside the Socrates re-sample loop in P3.
- **P3** (confidence fusion / D3) must land before P4 because the cascade in P4 reads the fused confidence to decide escalation.
- **P4–P10** can be parallelized after P3, but the recommended sequential order (below) minimizes merge friction.
- **P11** runs last and is the hygiene/retro pass.

| # | Phase | Plan file | Decision axis | Strict deps | Est. tasks |
|---|---|---|---|---|---|
| **P1** | Standards & baseline | [`2026-05-08-orchestrator-phase-1-standards-and-baseline.md`](2026-05-08-orchestrator-phase-1-standards-and-baseline.md) | — | none | ~22 |
| **P2** | Circuit breaker / doom-loop detector | `2026-05-08-orchestrator-phase-2-circuit-breaker.md` | D6 | P1 | ~28 |
| **P3** | Confidence fusion + Socrates v2 | `2026-05-08-orchestrator-phase-3-confidence-fusion.md` | D3 | P1, P2 | ~30 |
| **P4** | Tier classifier + cascade routing | `2026-05-08-orchestrator-phase-4-tier-cascade.md` | D1 | P1, P3 | ~26 |
| **P5** | Plan-mode trigger + adaptive thinking gate | `2026-05-08-orchestrator-phase-5-plan-mode-trigger.md` | D2 | P1, P3 | ~22 |
| **P6** | Risk × confidence matrix + HITL interrupts | `2026-05-08-orchestrator-phase-6-risk-matrix-hitl.md` | D5, D9 | P1, P3 | ~30 |
| **P7** | Privacy / sensitivity routing + guardrails | `2026-05-08-orchestrator-phase-7-privacy-routing.md` | D8 | P1, P4 | ~24 |
| **P8** | Cache-aware routing + compaction layers + per-tenant budgets | `2026-05-08-orchestrator-phase-8-cache-budget-compaction.md` | D7 | P1, P4 | ~32 |
| **P9** | Calibration loop + drift detection + bandit upgrade | `2026-05-08-orchestrator-phase-9-calibration-bandit.md` | D10 | P1, P4 | ~26 |
| **P10** | Sub-agent dispatch + chain-length cap | `2026-05-08-orchestrator-phase-10-subagent-dispatch.md` | D4 | P1, P2, P3, P6 | ~24 |
| **P11** | Hygiene retrospective & consolidation | `2026-05-08-orchestrator-phase-11-hygiene-retrospective.md` | — | P1–P10 | ~14 |

The phase plan files for P2–P11 are **not yet written** — they're written when the user authorizes them. P1 is written alongside this master plan.

### 4.1–4.11 Phase summaries (one paragraph each)

**P1 — Standards & baseline.** Net-new infrastructure with no behavior change. Adds `crates/vox-orchestrator-test-helpers` (mock `ModelRegistry`, mock `BulletinBoard`, golden-fixture loader). Adds `crates/vox-orchestrator/benches/` with five criterion benchmarks (`route_decision`, `socrates_gate`, `bulletin_throughput`, `compaction_pipeline`, `plan_refinement`). Promotes `vox-arch-check` orphan detector from warn → error. Adds golden behavioral tests pinning current routing decisions for ~30 representative tasks. Adds new telemetry columns (`logprob_entropy`, `sep_estimate`, `self_consistency_score`) to `llm_interactions` (schema v59 → v60 migration). Writes [`docs/src/architecture/orchestrator-perf-baseline-2026.md`](../../../src/architecture/orchestrator-perf-baseline-2026.md) with current p50/p99 numbers. Drops contract scaffolds for the four phase contracts (`tier-routing.v2`, `risk-confidence-matrix.v1`, `circuit-breaker.v1`, `socrates-fusion.v1`) — empty bodies, schema-validated.

**P2 — Circuit breaker / doom-loop detector (D6).** Implements the five-signal detector from research doc §6.3 — no-progress, same-error, tool-call-thrash, action-n-gram-overlap, semantic-drift — plus a hard turn cap with graduated CAUTION/WARNING tiers. Lives in a new `crates/vox-orchestrator/src/circuit_breaker.rs`. Reads thresholds from `contracts/orchestration/circuit-breaker.v1.yaml`. Trips hand off to the existing replanner (`mcp_tools/chat_tools/plan_loop.rs::maybe_refine_plan`) with a structured `TripReason`; if replanning fails K times, escalates via the bulletin board's `EscalationEvent`. Adds property tests on the n-gram overlap function via `proptest`. Perf budget: <50µs p99 per check.

**P3 — Confidence fusion + Socrates v2 (D3).** Adds the fusion function from research doc §4 to `vox-orchestrator-types::socrates_policy::confidence_policy`. Three inputs: token logprob entropy (P1 column), SEP estimate (P1 column, optional), per-claim self-consistency (computed on-demand). Emits a single composite score that maps to the existing `RiskDecision` enum. Adds the `socrates-fusion.v1.yaml` contract with weights, thresholds, and the abstention override for compliance-tagged tasks. Net-new: a `ResearchAction::ReSample / Retrieve / SpawnSocrates / Escalate` enum and the dispatcher that turns a composite-score band into one of those actions. Falls back gracefully when logprobs are unavailable from the provider (the Anthropic-via-OpenRouter case noted in research doc §14.2).

**P4 — Tier classifier + cascade routing (D1).** Adds a complexity classifier (rule-based v1) at `crates/vox-orchestrator/src/routing/tier_classifier.rs`. Composes with the existing `ModelRegistry::best_for_task()` via a new `routing::TierRouter` wrapper. Adds optional cascade mode that escalates from cheap → mid → strong only when P3's fused score crosses a threshold. Bumps `model-routing.v1` → `v2` (additive: new `tier_routing` and `cascade` blocks). v1 stays loadable; v2 unlocks the new behavior under a feature gate in config that defaults off in this phase, on in P11.

**P5 — Plan-mode trigger + adaptive thinking gate (D2).** Hoists the `pick_planning_mode()` decision (research doc §3.3) to a single gateway in `mcp_tools/chat_tools/plan.rs`. Removes the scattered hardcoded heuristics. Adds a `task.tools_predictable` predicate (rule-based detector + future-classifier hook). Wires Anthropic adaptive thinking through a per-call config flag; sets it on for the two specific call sites the research doc names (tool-pick under multiple options; constraint-satisfaction refactor) and off everywhere else. Adds telemetry on planning-mode transitions to `model_route_event`.

**P6 — Risk × confidence matrix + HITL interrupts (D5, D9).** Implements the four-dimension risk score (irreversibility, blast radius, compliance exposure, confidence) and the matrix from research doc §9.1. Adds a new `crates/vox-orchestrator/src/escalation.rs` and an `EscalationEvent` variant on `AgentMessage`. Adds **static interrupt points** at every irreversible-side-effect tool call (writes, deletes, sends, financial). The interrupt mechanism reuses the existing `bulletin.rs` to push the event; resumption uses an existing daemon RPC method (extends `orch-daemon-rpc-methods.schema.json`). Conservative defaults from research doc §9.1 codified in `contracts/orchestration/risk-confidence-matrix.v1.yaml`. Earned-autonomy expansion is *out of scope* in this phase — that's P9.

**P7 — Privacy / sensitivity routing + guardrails (D8).** Adds a two-pass PII detector (regex + ML classifier) at `crates/vox-orchestrator/src/privacy/pii_detector.rs`. Composes with `ConfidentialityTier` filtering in routing: `Public/Internal → all eligible providers; Confidential → ZDR providers only; Restricted/Critical → self-hosted only`. Pulls eligible-provider lists from a new `contracts/orchestration/providers-privacy-eligibility.v1.yaml`. Adds the input/output guardrail layer (per research doc §8.3) with secrets-redaction and prompt-injection detection — runtime guardrails are deferred to P11.

**P8 — Cache-aware routing + compaction layers + per-tenant budgets (D7).** Three nested concerns. (a) Adds an approximate radix tree per provider for prompt-prefix tracking; routing scorer reads this as a new dimension. (b) Replaces the single-threshold compaction trigger in `compaction.rs` with the five-layer pipeline from research doc §7.2 (budget-reduction → snip → microcompact → context-collapse → auto-compact); adds an agent-driven `compact_context` MCP tool callable proactively at task boundaries. (c) Adds per-tenant budget tracking with hierarchical buckets (per-tenant > per-app > per-call) in-memory at the gateway, with token-based limits.

**P9 — Calibration loop + drift detection + bandit upgrade (D10).** Adds a daily background calibration job (in the orchestrator daemon, not as a separate service) that samples recent completions, recomputes per-tier ECE, and updates router weights. Adds Sentence-BERT-free drift detection (cosine-on-pooled-embeddings will do — no new deps required because `vox-ml-cli` already has an embedding pipeline). Upgrades the existing Thompson `arm_stats` to a contextual-bandit-with-preference-vector (BaRP-style, research doc §10.1). Optional: dueling-bandit pairwise feedback if the data substrate supports it. The earned-autonomy threshold expansion runs here, reading from 30+ days of decision logs.

**P10 — Sub-agent dispatch + chain-length cap (D4).** Implements description-driven sub-agent selection consistent with the existing `vox-skills/skills/*.skill.md` surface. Adds a chain-length tracker that escalates to HITL once cumulative agent-chain reliability drops below a threshold (research doc §5.3). Adds the parallel-fan-out path for independent subtasks. The existing `clarification_db_inbox_poll` and `populi-mesh` A2A surfaces are the transport — this phase only adds *trigger logic*, not transport mechanics.

**P11 — Hygiene retrospective & consolidation.** The cleanup pass. Audits all phases' contract files for naming consistency. Promotes one or more arch-check rules (TBD per state at P11 start) from warn → error. Removes feature gates introduced earlier (e.g., the cascade gate from P4). Updates [`docs/src/architecture/where-things-live.md`](../../../src/architecture/where-things-live.md) for every concept added in P2–P10 (P11 is the last chance — earlier phases must have already added rows; P11 verifies). Regenerates `SUMMARY.md`, `architecture-index.md`, `feed.xml` via `vox-doc-pipeline`. Writes a retro doc summarizing perf-baseline deltas across all phases.

---

## 5. Per-Phase Success Matrix

Each row is *binding*. A phase plan that doesn't include verification of every cell in its row is not done.

| Phase | Code surface | New contracts | Telemetry rows | Perf budget (p99) | Tests added | HITL surface |
|---|---|---|---|---|---|---|
| **P1** | `+test-helpers crate, +benches/, +llm_interactions schema v60` | scaffolds for 4 contracts | new columns; no new metric_types | benches green, baseline doc filed | 5 benches + ~30 golden routing tests | n/a (no new decisions) |
| **P2** | `+circuit_breaker.rs` | `circuit-breaker.v1.yaml` | `METRIC_TYPE_CIRCUIT_BREAKER_TRIP` | <50µs per check | property tests on n-gram; integration test on full trip | EscalationEvent on K-failed replan |
| **P3** | `+confidence_policy::fuse(), +ResearchAction enum` | `socrates-fusion.v1.yaml` | `METRIC_TYPE_SOCRATES_FUSION` | <2ms per claim (no resample); <500ms with resample n=5 | unit on fusion math; property on monotonicity; e2e on resample chain | EscalationEvent at abstain band |
| **P4** | `+tier_classifier.rs, +TierRouter, model-routing.v2` | `tier-routing.v2 → existing model-routing` superset | `METRIC_TYPE_MODEL_TIER_ROUTE` | <100µs classify + best_for | classifier rule unit tests; cascade integration test | escalate when cascade exhausted |
| **P5** | refactor `plan.rs::pick_planning_mode()` | additive to existing planning contract | `METRIC_TYPE_PLAN_MODE_DECISION` | <500µs decision | unit per branch; integration on full plan→exec | escalate when planner ambiguous |
| **P6** | `+escalation.rs, +risk-score calculator` | `risk-confidence-matrix.v1.yaml` | `METRIC_TYPE_HITL_INTERRUPT, METRIC_TYPE_RISK_SCORE` | <200µs risk score; interrupt latency dominated by user | matrix-cell tests; interrupt-resume e2e | this phase IS the HITL surface |
| **P7** | `+privacy/pii_detector.rs, +eligibility filter` | `providers-privacy-eligibility.v1.yaml` | `METRIC_TYPE_PRIVACY_ROUTE_DECISION` | <1ms PII regex pass; <30ms ML pass | regex coverage tests; eligibility filter tests | escalate on ambiguous sensitivity |
| **P8** | rewrite `compaction.rs` to 5-layer; `+cache_prefix_tree.rs, +tenant_budget.rs` | `cache-routing.v1.yaml, +tenant-budget.v1.yaml` | `METRIC_TYPE_CACHE_HIT_PREDICTION, METRIC_TYPE_BUDGET_DECISION` | compaction <5ms p99 per layer; cache lookup <50µs; budget check <10µs | golden tests on each compaction layer; budget exhaustion test | escalate on budget exhausted |
| **P9** | `+calibration_job.rs, +drift_detector.rs, +contextual_bandit.rs` | additive to existing | `METRIC_TYPE_CALIBRATION_RUN, METRIC_TYPE_DRIFT_ALERT, METRIC_TYPE_BANDIT_UPDATE` | calibration job <60s daily run; drift check <10ms | ECE math tests; bandit convergence sim | escalate on drift > 2σ |
| **P10** | `+subagent_dispatcher.rs, +chain_tracker.rs` | additive | `METRIC_TYPE_SUBAGENT_DISPATCH, METRIC_TYPE_CHAIN_DEPTH_ALERT` | <500µs dispatch decision; chain check <50µs | dispatch unit tests; chain-cap integration | escalate when chain reliability < threshold |
| **P11** | net deletions (gate removals); doc regen | n/a (audit pass) | conformance check | retro doc filed; perf-delta table | conformance regression tests | n/a |

**Total telemetry surface added across program:** 12 new `metric_type` constants. **Total contracts added:** 7 versioned YAML/schema pairs. **Total perf benchmarks added:** 5 baseline + ~12 phase-specific = ~17.

---

## 6. Risk Register & Rollback

| # | Risk | Likelihood | Mitigation | Rollback |
|---|---|---|---|---|
| R1 | Telemetry schema migration v59→v60 breaks existing CLI consumers | Medium | Migration is additive (new columns are NULLABLE); old queries unaffected. | Drop new columns in a follow-up migration; no data loss. |
| R2 | Mock `ModelRegistry` diverges from real one over time | Medium | The mock is generated from a snapshot of real `ModelSpec`; CI regenerates monthly. | Phase plans must read current real specs in addition to mock; not a blocker. |
| R3 | Phase 4 cascade adds latency on the cheap path | Medium | Cascade is opt-in via config; default-off until P11; perf budget gates it. | Flip config flag off; existing `best_for_task` path unchanged. |
| R4 | Phase 6 HITL interrupts cause UX friction | High | Conservative thresholds from research doc §9.1; **30-day calibration window in P9 recalibrates**; static breakpoints only on irreversible actions. | Threshold tunables in YAML; ops can tune live without redeploy. |
| R5 | Phase 7 PII detector false positives block legitimate work | Medium | Two-pass design: ML classifier confirms regex hit; threshold tunable; failures emit `EscalationEvent` rather than silent block. | Disable ML pass via config; regex-only mode is a viable fallback. |
| R6 | Phase 8 cache-prefix routing degrades when caches are cold | Low | Routing falls back to existing scoring when prefix-tree confidence is low; no new perf path forced. | Disable cache-aware dim via routing-weight=0. |
| R7 | Phase 9 bandit policy converges to local optimum | Medium | Ε-greedy floor ensures exploration; existing Thompson code stays as fallback; weekly drift report flags pathological convergence. | Revert weights to Thompson via config; existing arm_stats remain authoritative. |
| R8 | Phase 11 doc regeneration introduces SUMMARY churn | Low | `vox-doc-pipeline --check` runs in CI; regen is mechanical. | n/a — generated artifact; no rollback meaning. |
| R9 | A phase exceeds its perf budget | Low (gated) | Gate G3 fails the build; phase cannot merge. | Phase plan must be revised before merge — no rollback needed because nothing merged. |
| R10 | A new contract breaks an external consumer | Low | All net-new contracts are `*.v1.yaml` (additive); breaking changes require explicit `v2` and 30-day deprecation per AGENTS.md spirit. | Old `vN` schema kept until consumers migrate. |
| R11 | `where-things-live.md` not updated, blocking merge | Low (gated) | Gate G1 includes the `where-things-live` row check (a new arch-check rule, added in P1). | Add the row in the same PR. |
| R12 | Auto-generated docs regenerate to a different shape than expected | Low | `cargo run -p vox-doc-pipeline -- --check` runs as a final step in every phase; the phase plan ends with regen + commit. | n/a — generated artifact. |

**Rollback policy.** Every phase ships behind a config gate where reasonable. The gate is named `vox.orchestrator.<phase-feature>.enabled` in `contracts/orchestration/feature-flags.v1.yaml` (created in P1). Default-off until P11. P11 is the only phase that flips defaults.

---

## 7. Glossary (used throughout phase plans)

- **Decision axis Dn.** One of the ten decisions (D1–D10) defined in [`docs/src/architecture/autonomous-orchestration-policy-research-2026.md`](../../../src/architecture/autonomous-orchestration-policy-research-2026.md) Part 1.
- **Composite confidence.** The single fused score from P3 — a weighted combination of logprob entropy, SEP estimate, and per-claim self-consistency. Range [0, 1]; higher = more confident.
- **Risk score.** The four-dimension product (irreversibility × blast_radius × compliance_exposure × (1 − composite_confidence)) defined in P6.
- **Tier.** One of `Cheap | Mid | Strong` from research doc §2.2; mapped to existing `StrengthTag` in `crates/vox-orchestrator/src/models/`.
- **EscalationEvent.** A new `AgentMessage` variant on `bulletin.rs` introduced in P6, carrying `{ task_id, axis, score, reason, snapshot }`.
- **Trip.** Circuit-breaker firing — a non-fatal condition that hands off to the replanner.
- **Cascade.** Sequential model invocation: try `Cheap`; if composite_confidence < threshold, retry on `Mid`; etc. Defined in P4.
- **Adaptive thinking.** Anthropic's `thinking_budget = adaptive` mode (research doc §3.1). Different from "extended thinking with explicit budget" — adaptive lets Claude decide.
- **Layer.** `vox-arch-check` layer assignment in `docs/src/architecture/layers.toml`. `vox-orchestrator` = L3.
- **Where-things-live row.** A row in [`docs/src/architecture/where-things-live.md`](../../../src/architecture/where-things-live.md) of the form `| concept | crate | function/struct |`. Required for every new concept.
- **HITL.** Human-in-the-loop. Per research doc Part 12 and EU AI Act Article 14, mandatory for several action classes regardless of confidence.
- **Net-new.** Code/contract that does not exist today and is being introduced. Distinct from *extended* (existing surface getting more behavior).
- **Schema bump.** A change to `crates/vox-db` schema version (currently v59). Per `research_metrics_contract.rs`.

---

## 8. Cross-Cutting Standards (the user's "wrangle the orchestrator" mandate)

These apply to **every** file touched by **every** phase, not just net-new code.

### 8.1 Naming consistency

- New types use suffix conventions: `*Trigger`, `*Detector`, `*Decider`, `*Score` for decision components; `*Event`, `*Message` for bus payloads; `*Policy`, `*Contract` for configuration types.
- Booleans are positively phrased (`is_eligible`, not `is_not_blocked`).
- Functions returning a `Result` use `try_*` prefix when there's a non-`try_` version available; otherwise plain.

### 8.2 Module size limits

- **No file in `crates/vox-orchestrator/src/` may exceed 600 lines** without justification in a code comment (// LARGE-FILE-RATIONALE: ...).
- The arch-check `loc_budget` rule is currently default-warn for vox-orchestrator (no cap). P11 considers tightening this; phases must not push files past the soft cap without rationale.

### 8.3 Telemetry hygiene

- **No `tracing::info!` for routine events** — those go to telemetry. `tracing` is for operator-facing logs only.
- Every `metric_type` constant has a doc comment naming the OTel SemConv span/attribute it maps to.

### 8.4 Test hygiene

- **TDD is required.** Every code-changing step in a phase plan starts with a failing test, then implementation, then passing test, then commit. Phase plan steps follow the bite-sized format from `superpowers:writing-plans`.
- Integration tests under `crates/vox-integration-tests/tests/` are append-only — phases add tests, do not delete.
- Property tests (via `proptest`) are mandatory for any function with structural invariants (e.g., the n-gram overlap function in P2; the fusion-monotonicity in P3).

### 8.5 Performance discipline

- **Hot paths must not allocate per-call when avoidable.** Use `&str` over `String`, `&[T]` over `Vec<T>`, in code on the routing/socrates path.
- Use `#[inline]` only where bench numbers justify it. No speculative inlining.
- Bounded-channel sizes for any new `tokio::sync::mpsc` or `broadcast` — pick a size; document why; never default to unbounded.

### 8.6 Concurrency hygiene

- Locks held during `await` are forbidden. Use `parking_lot::Mutex` for short critical sections; for async, structure via channels or `tokio::sync::RwLock` only when the read pattern justifies it.
- Every spawned `tokio::task` must have a `JoinHandle` tracked somewhere — no fire-and-forget. ADR-022 (orchestrator bootstrap & daemon boundaries) is the canon.

### 8.7 Error surface

- New error types use `thiserror::Error`; integrate into the existing `OrchestratorError` if there is one in scope, else a new fenced enum.
- `unwrap()` is forbidden in production code paths. `expect("invariant: …")` is acceptable when the invariant is documented in the message.

### 8.8 Doc surface

- Every new public type has a `///` doc comment naming the decision axis (D1–D10) it belongs to.
- Every new module has a `//!` module-level doc comment with a one-paragraph "What lives here" summary.
- After every phase merges, run `cargo run -p vox-doc-pipeline` and commit the regen.

### 8.9 Auto-generated artifacts

- Never hand-edit `docs/src/SUMMARY.md`, `docs/src/architecture/architecture-index.md`, `docs/src/feed.xml`, `*.generated.md`, `.cursorignore`, `.aiignore`, `.aiexclude`. Per AGENTS.md and the user's persistent feedback memory.
- The phase plan's last task is always: regenerate via `cargo run -p vox-doc-pipeline` (and `vox ci sync-ignore-files` if `.voxignore` changed) and commit the diff.

### 8.10 VoxScript-first glue

- No new `.ps1`, `.sh`, or `.py` scripts. Per AGENTS.md §VoxScript-First Glue Code.
- Any new automation script (e.g., the calibration runner in P9) is `.vox` invoked via `vox run scripts/<name>.vox`.

### 8.11 Secret hygiene

- No new `std::env::var("ANTHROPIC_API_KEY")` etc. — secrets resolve via `vox_secrets::resolve_secret(SecretId::*)`. Per AGENTS.md §Secret Management.
- New secrets need a `SecretSpec` entry in `crates/vox-secrets/src/spec.rs`.

### 8.12 Crypto hygiene

- All cryptographic primitives via `vox-crypto`. No `ring`, no AEGIS, no `cmake`/`nasm`-pulling deps. Per AGENTS.md §Cryptography Policy.

---

## 9. Execution Cadence

For an agent (Sonnet 4.6 or equivalent) working through this plan:

1. **Per phase:** open the phase plan; execute tasks linearly; commit after each step that produces a green test or a clean compile. Do not batch commits.
2. **Per task:** TDD — failing test, run it (verify it fails), implementation, run it (verify it passes), commit.
3. **Per phase end:** run all five quality gates (§3); fix any failures *before* declaring the phase done; regen auto-docs; merge.
4. **Per program:** after P11, write the retrospective at `docs/src/architecture/orchestrator-policy-program-retrospective-2026.md` summarizing what shipped, what perf changed, what HITL escalation rate looks like, and what the open questions from research doc §14 are now resolved or still open.

Estimated total task count across all phases: ~278.
Estimated wall time at one task per ~5 minutes (skilled agent, no blockers): ~23 hours of focused execution. With reviews, build cycles, doc regens, and merge friction: budget 6–10 working days.

---

## 10. Source Document Pointers

Every phase plan opens by re-citing these. Do not skip them.

- [`docs/src/architecture/autonomous-orchestration-policy-research-2026.md`](../../../src/architecture/autonomous-orchestration-policy-research-2026.md) — the research; defines the decisions and the state of the art.
- [`docs/src/architecture/model-orchestration-ssot-audit-2026.md`](../../../src/architecture/model-orchestration-ssot-audit-2026.md) — current routing surface.
- [`docs/src/architecture/orchestrator-companion-audit-findings-2026.md`](../../../src/architecture/orchestrator-companion-audit-findings-2026.md) — non-routing surface; many P1–P11 tasks close FIX items here.
- [`docs/src/architecture/nextgen-orchestrator-research-2026.md`](../../../src/architecture/nextgen-orchestrator-research-2026.md) — failure modes & advanced concepts.
- [`docs/src/adr/005-socrates-anti-hallucination-ssot.md`](../../../src/adr/005-socrates-anti-hallucination-ssot.md) — Socrates contract; P3 extends, does not replace.
- [`docs/src/adr/025-multi-agent-lock-coherence.md`](../../../src/adr/025-multi-agent-lock-coherence.md) — A2A coherence; P10 builds on top.
- [`docs/src/adr/030-state-machine-ssot.md`](../../../src/adr/030-state-machine-ssot.md) — mode/state machine; P5/P6 plug into.
- [`AGENTS.md`](../../../../AGENTS.md) — project policy; non-negotiable.
- [`CLAUDE.md`](../../../../CLAUDE.md) — Claude-specific overlay; reinforces the above.
- [`docs/src/architecture/where-things-live.md`](../../../src/architecture/where-things-live.md) — concept-to-crate index; every new concept gets a row.
- [`docs/src/architecture/layers.toml`](../../../src/architecture/layers.toml) — layer rules; no inversions.

---

## 11. Sign-Off Criteria for the Whole Program

The program is done when:

- [ ] All 11 phase plans are written, executed, and merged.
- [ ] All 12 new `metric_type` constants exist in `research_metrics_contract.rs` and are emitted in tests.
- [ ] All 7 net-new contract YAML/schema pairs exist in `contracts/orchestration/` and load via tests.
- [ ] Performance baseline doc has 11 rows of post-phase deltas; net regression on any path is ≤ 5% versus the P1 baseline.
- [ ] HITL escalation rate (from telemetry, after 30 days of post-P9 data) is between 1% and 5% of decisions — calibrated, not pathological.
- [ ] Doom-loop trip rate (post-P2) is non-zero (proves the detector is firing) but below 0.5% of tasks (proves it's not over-firing).
- [ ] `vox-arch-check` passes with at least one previously-warn rule promoted to error.
- [ ] [`docs/src/architecture/where-things-live.md`](../../../src/architecture/where-things-live.md) has rows for every new concept.
- [ ] `cargo run -p vox-doc-pipeline -- --check` passes; `vox ci secret-env-guard` passes; `vox ci clavis-parity` passes.
- [ ] Retrospective doc `orchestrator-policy-program-retrospective-2026.md` filed with deltas, learnings, open questions.

---

*End of master plan. Phase 1 plan in [`2026-05-08-orchestrator-phase-1-standards-and-baseline.md`](2026-05-08-orchestrator-phase-1-standards-and-baseline.md).*
