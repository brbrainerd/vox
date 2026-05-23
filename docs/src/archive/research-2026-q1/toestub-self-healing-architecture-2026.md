---
title: "TOESTUB self-healing architecture 2026"
description: "Research-backed architecture blueprint for evolving TOESTUB into a self-healing, LLM-aware quality system integrated with Populi and MENS."
category: "Language Reference"
last_updated: "2026-03-26"
training_eligible: false

schema_type: "TechArticle"
archived_date: 2026-04-18
---

## TOESTUB self-healing architecture 2026

This page is the research-backed SSOT for evolving TOESTUB from a regex-heavy static checker into a self-healing, self-protecting, LLM-aware quality system that feeds negative patterns into Populi/MENS training.

## Why this exists

TOESTUB already has strong primitives (`TokenMap`, structured suppressions, run modes, schema contracts), but stub detection is still mostly literal and line-pattern driven. That shape is good for speed but weak for semantic unfinished-work detection and weak for continuous model feedback loops.

## External research synthesis (2026)

### What top systems do well

- **Ruff**: performance-first unified toolchain, built-in caching, cascading monorepo config, broad rule coverage, fast autofix loops.  
  Sources: [Ruff docs](https://docs.astral.sh/ruff/), [Ruff FAQ](https://docs.astral.sh/ruff/faq/), [Ruff configuration discovery](https://docs.astral.sh/ruff/configuration/#config-file-discovery).
- **rust-analyzer + Salsa**: lazy + incremental query graph with durability tiers and architecture invariants around API boundaries.  
  Sources: [Architecture](https://rust-analyzer.github.io/book/contributing/architecture.html), [Three architectures blog](https://rust-analyzer.github.io/blog/2020/07/20/three-architectures-for-responsive-ide.html), [Durable incrementality](https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html).
- **Trunk Code Quality**: hermetic runtime/tool management, daemonized background precompute, hold-the-line gating, git-aware partial scans, plugin extensibility.  
  Sources: [Trunk code-quality overview](https://docs.trunk.io/code-quality/overview), [Trunk plugins](https://github.com/trunk-io/plugins).
- **CodeQL**: semantic extraction into queryable databases, path-problem traces, variant analysis at scale.  
  Sources: [About CodeQL](https://codeql.github.com/docs/codeql-overview/about-codeql/), [About queries](https://codeql.github.com/docs/writing-codeql-queries/about-codeql-queries/), [Path queries](https://codeql.github.com/docs/writing-codeql-queries/creating-path-queries/).
- **Semgrep**: practical custom-rule authoring with cross-file/cross-function dataflow and mature language support matrix.  
  Sources: [Semgrep docs](https://semgrep.dev/docs/), [Feature definitions](https://semgrep.dev/docs/references/feature-definitions), [Language maturity summary](https://semgrep.dev/docs/supported-languages#language-maturity-summary).
- **Biome / Clippy / golangci-lint**: explicit safe-vs-unsafe fixes, rule domains/categories, rich suppression and false-positive controls, large-scale runner ergonomics.  
  Sources: [Biome linter](https://biomejs.dev/linter/), [Clippy docs](https://doc.rust-lang.org/clippy/), [golangci-lint false positives](https://golangci-lint.run/docs/linters/false-positives/).

### Most relevant imported patterns for TOESTUB

1. **Durable incremental analysis** (rust-analyzer): volatile user files vs durable generated/vendor/config domains.
2. **Hermetic reproducibility** (Trunk/Ruff): deterministic tool/rule/runtime versions in CI and local.
3. **Path/evidence explainability** (CodeQL): structured evidence and optional path traces, not only plain-text rule messages.
4. **Rule lifecycle governance** (Biome/Clippy): `experimental -> shadow -> recommended -> strict`.
5. **Hold-the-line rollout** (Trunk/golangci-lint): strict on new deltas, gradual cleanup of legacy baseline.
6. **Config and suppression discipline** (Ruff/golangci-lint): policy in data contracts, not ad hoc in detector code.

## Current TOESTUB architectural baseline (in-repo)

- Engine orchestrates scan -> per-file parse -> detector pass in [`crates/vox-toestub/src/engine.rs`](../../../crates/vox-toestub/src/engine.rs).
- Rust lexical classification for comments/strings in [`crates/vox-toestub/src/analysis/token_map.rs`](../../../crates/vox-toestub/src/analysis/token_map.rs).
- Stub detector in [`crates/vox-toestub/src/detectors/stub.rs`](../../../crates/vox-toestub/src/detectors/stub.rs) still relies on many lexical markers and local exceptions.
- Scanner exclusions in [`crates/vox-toestub/src/scanner.rs`](../../../crates/vox-toestub/src/scanner.rs).
- Existing reporting/snapshot contracts in:
  - [`contracts/toestub/gold-dataset.v1.json`](../../../contracts/toestub/gold-dataset.v1.json)
  - [`contracts/reports/toestub-remediation/delta-after-remediation.json`](../../../contracts/reports/toestub-remediation/delta-after-remediation.json)
  - [`docs/src/architecture/scaling-toestub-rules.md`](scaling-toestub-rules.md)

## Target architecture (self-healing TOESTUB)

```mermaid
flowchart TD
  sourceTree[WorkspaceSourceTree] --> scanner[Scanner]
  scanner --> fileIndex[FileIndexDurabilityTiered]
  fileIndex --> analysisCache[AnalysisContextCache]
  analysisCache --> lexical[LexicalFeatures]
  analysisCache --> ast[ASTFeatures]
  analysisCache --> graph[CallRefGraphFeatures]
  analysisCache --> history[HistoricalFindingFeatures]
  lexical --> scorer[EvidenceScoringModel]
  ast --> scorer
  graph --> scorer
  history --> scorer
  scorer --> findings[FindingsWithConfidenceEvidence]
  findings --> policy[PolicyGateThresholds]
  policy --> fixer[SafeUnsafeFixPlanner]
  fixer --> verify[TargetedVerification]
  verify --> learn[FeedbackCalibrationLoop]
  learn --> populi[PopuliNegativePatternFeed]
  populi --> mens[MENSTrainingCorpus]
```

## Do and do-not rules (LLM maintainability critical path)

### Do

- Keep **detector logic deterministic** and policy-driven through contract files.
- Emit **machine-usable evidence** for each finding (`confidence`, `evidence_kind`, `feature_values`).
- Separate **fast lexical checks** from **slower semantic checks** behind staged gates.
- Require **targeted verification** before any autofix lands.
- Keep suppressions structured, owner-tagged, and expiry-aware.
- Maintain strict JSON schema versioning for all new TOESTUB outputs consumed by CI/MENS pipelines.

### Do not

- Do not expand keyword lists indefinitely to chase false negatives.
- Do not bury exception logic as in-code one-off skips; move to policy contracts.
- Do not auto-apply unsafe fixes in CI.
- Do not couple Populi/MENS ingestion directly to volatile internal structs; use explicit versioned contracts.
- Do not regress `rust_parse_failures` budget for feature expansion.

## LLM-specific anti-pattern taxonomy (for TOESTUB v2)

TOESTUB should detect these as first-class families, not just text tokens:

1. **No-op implementation shells**: function exists, but no side effects, no state transition, no meaningful return.
2. **Behavior-claim mismatch**: comments/docs claim completion while implementation evidence is thin.
3. **Hallucinated call surfaces**: unresolved callsites with near-neighbor symbol hints indicating probable LLM fabrication.
4. **Adapter-only pass-through chains**: wrappers that only relay inputs without semantic contribution across multiple layers.
5. **Dead branch saturation**: complex conditionals with trivial branch bodies.
6. **Synthetic constant clusters**: hard-coded values introduced in bulk edits without central policy references.
7. **Pseudo-refactors**: renamed symbols with stale references across sibling modules.

## Populi + MENS integration avenue

### Objective

Use TOESTUB findings to generate negative training patterns and policy hardening examples so MENS learns to avoid recurrent LLM failure modes.

## VoxDB persistence design (explicit)

This architecture should persist detector and remediation outcomes in VoxDB by reusing existing schema surfaces first, with minimal additive columns where needed.

### Existing scaffolding to reuse

- TOESTUB tables in `toestub_build` domain:
  - `toestub_task_queue`
  - `toestub_baselines`
  - `toestub_file_cache`
  - `toestub_suppressions`
  - Source: [`crates/vox-db/src/schema/domains/toestub_build.rs`](../../../crates/vox-db/src/schema/domains/toestub_build.rs)
- Generic telemetry/event table:
  - `research_metrics(session_id, metric_type, metric_value, metadata_json, created_at)`
  - Source: [`crates/vox-db/src/schema/domains/agents.rs`](../../../crates/vox-db/src/schema/domains/agents.rs)
- Existing event-writing patterns:
  - `benchmark_event` via [`record_benchmark_event`](../../../crates/vox-db/src/benchmark_telemetry.rs)
  - `populi_control_event` via [`record_populi_control_event`](../../../crates/vox-db/src/populi_control_telemetry.rs)

### Proposed persistence model

1. **Run-level telemetry** (reuse `research_metrics`, no new table initially)
   - `session_id`: `toestub:<repository_id>`
   - `metric_type`:
     - `toestub_run_summary`
     - `toestub_rule_quality`
     - `toestub_remediation_outcome`
     - `toestub_training_feedback_export`
   - `metric_value`: compact KPI (for example, precision estimate or runtime_ms normalized scalar)
   - `metadata_json`: structured payload containing run ids, policy digest, confidence histograms, FP/FN counters, remediation class totals, and export ids.
2. **State snapshots** (reuse TOESTUB tables)
   - Keep full findings snapshots in `toestub_baselines.findings_json`.
   - Keep fix queue snapshots in `toestub_task_queue.fix_suggestions_json`.
   - Keep per-file detector cache in `toestub_file_cache`.
3. **Minimal additive extensions** (preferred over new tables)
   - Add optional fields to existing TOESTUB tables for reproducibility and joins:
     - `run_id`
     - `policy_digest`
     - `rules_digest`
     - `engine_mode` (legacy/shadow/v2)
   - If adding columns is too disruptive for immediate rollout, include these in embedded JSON first, then promote to columns in a later schema baseline.

### Why this is preferred

- avoids introducing yet another event table,
- matches existing VoxDB telemetry conventions,
- keeps compatibility with Codex/MCP readers already consuming `research_metrics`,
- allows gradual hardening from JSON payloads to typed columns only where query pressure justifies it.

### Query and maintenance guardrails

- Add lightweight helper APIs in `vox-db` similar to `record_benchmark_event`:
  - `record_toestub_run_summary`
  - `record_toestub_rule_quality`
  - `record_toestub_remediation_outcome`
- Keep payload schema versioned in JSON (`schema_version`) -> avoid brittle readers.
- Enforce retention/cleanup policy for noisy run telemetry (avoid unbounded growth).
- Never store raw secrets or full file contents in telemetry payloads.

### Integration strategy

- Add a TOESTUB export contract for training feedback, e.g. `contracts/toestub/training-feedback.v1.schema.json`.
- Emit records with:
  - `rule_family`
  - `confidence`
  - anonymized structural features
  - optional minimal code window
  - fix class (`safe`, `review_required`, `reject`)
  - outcome label after human/CI adjudication
- In Populi pipeline, map these records into:
  - **negative pattern rows** (what to avoid),
  - **counterexample rows** (preferred correction patterns),
  - **trajectory labels** for recovery behavior.

### Existing docs to align

- [`docs/src/reference/populi.md`](../reference/populi.md)
- [`docs/src/reference/mens-training.md`](../reference/mens-training.md)
- [`docs/src/architecture/mens-training-ssot.md`](mens-training-ssot.md)

## Evolution model (converge to SSOT, avoid magic values)

Use a contract-first control surface:

- `stub-policy.v1.json`: score weights, thresholds, risk multipliers.
- `suppression.v1.schema.json`: keep owner/reason/expiry strict.
- `training-feedback.v1.json`: immutable event feed to Populi.
- `toestub-run-json.v2.schema.json`: add optional evidence summary and calibration stats.

Policy knobs should be loaded dynamically and fingerprinted in output metadata so runs are reproducible and auditable.

## Adoption stages

1. **Stage 0 (shadow)**: new scorer runs in parallel, no gate effect.
2. **Stage 1 (assist)**: emits warnings with confidence/evidence.
3. **Stage 2 (balanced gate)**: high-confidence errors gate, medium-confidence warnings annotate.
4. **Stage 3 (self-heal safe)**: safe autofixes enabled with targeted verification.
5. **Stage 4 (training loop)**: Populi ingestion drives calibrated threshold updates under governance.

## Architecture risks and mitigations

- **Risk**: semantic scoring increases runtime.  
  **Mitigation**: two-phase pipeline; skip deep analysis for low-signal files.
- **Risk**: overfitting to current codebase patterns.  
  **Mitigation**: maintain curated TP/FP/FN fixtures + periodic drift review.
- **Risk**: unsafe auto-remediation regressions.  
  **Mitigation**: safe/unsafe fix classes + mandatory targeted tests + rollback.
- **Risk**: training data poisoning from noisy findings.  
  **Mitigation**: ingest only adjudicated findings with confidence and outcome labels.
- **Risk**: event payload sprawl in generic `research_metrics`.  
  **Mitigation**: strict payload schemas, version tags, and promotion of only high-value fields into typed columns.
- **Risk**: schema churn from over-eager normalization.  
  **Mitigation**: JSON-first for early iterations, then additive columns on proven query paths only.

## Minimal success metrics (first promotion)

- `stub/placeholder` false-positive rate reduced by at least 40% vs current baseline.
- No increase in `rust_parse_failures`.
- Mean TOESTUB runtime increase <= 20% for `crates/` scan in audit mode.
- At least one Populi ingestion path operational with schema-validated training feedback export.

## References

- Ruff: [docs](https://docs.astral.sh/ruff/), [FAQ](https://docs.astral.sh/ruff/faq/)
- rust-analyzer: [architecture](https://rust-analyzer.github.io/book/contributing/architecture.html), [incrementality](https://rust-analyzer.github.io/blog/2023/07/24/durable-incrementality.html)
- Trunk Code Quality: [overview](https://docs.trunk.io/code-quality/overview)
- CodeQL: [about](https://codeql.github.com/docs/codeql-overview/about-codeql/), [path queries](https://codeql.github.com/docs/writing-codeql-queries/creating-path-queries/)
- Semgrep: [docs](https://semgrep.dev/docs/), [feature definitions](https://semgrep.dev/docs/references/feature-definitions)
- Biome: [linter](https://biomejs.dev/linter/)
- Clippy: [docs](https://doc.rust-lang.org/clippy/)
- golangci-lint: [configuration](https://golangci-lint.run/docs/configuration/file/), [false positives](https://golangci-lint.run/docs/linters/false-positives/)


