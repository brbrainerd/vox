---
title: "Detector & Heuristic Rule SSOT — Design"
description: "Single source of truth for detector regex/heuristic patterns and Scientia heuristics, with authoring-time benchmarking and no new runtime dependencies on vox-search."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Architectural SSOT; load-bearing for code-audit and Scientia heuristic surfaces."
---

# Detector & Heuristic Rule SSOT — Design

> Companion plan: [2026-05-09-detector-rule-ssot-plan.md](./2026-05-09-detector-rule-ssot-plan.md).

## Problem

Two surfaces in the workspace today encode runtime decisions as **hard-coded values inside Rust source**:

1. **Code-audit detectors** — 23 detectors in [`crates/vox-code-audit/src/detectors/`](../../../crates/vox-code-audit/src/detectors/), most regex-driven. ~225 `regex::Regex::new(...)` call sites across 51 files. Patterns, severity, language scope, and finding messages live inline in Rust source. Adding/tuning a rule requires a Rust edit + recompile + re-review. Examples: [`victory_claim.rs`](../../../crates/vox-code-audit/src/detectors/victory_claim.rs), [`ai_laziness.rs`](../../../crates/vox-code-audit/src/detectors/ai_laziness.rs), [`scaling.rs`](../../../crates/vox-code-audit/src/detectors/scaling.rs), [`magic_value.rs`](../../../crates/vox-code-audit/src/detectors/magic_value.rs).
2. **Scientia heuristics** — already SSOT-driven via [`crates/vox-publisher/src/scientia_heuristics.rs`](../../../crates/vox-publisher/src/scientia_heuristics.rs) loading [`contracts/scientia/impact-readership-projection.seed.v1.yaml`](../../../contracts/scientia/impact-readership-projection.seed.v1.yaml). Scoring weights, thresholds, and gates come from the seed. **This is the pattern we want to generalize.**

Costs of the current detector approach:

- **Tuning friction.** Changing a regex requires a `pub fn` edit, which trips test-first policy and triggers cargo rebuild of `vox-code-audit` and downstream consumers.
- **No precision/recall record.** No place to record "this regex catches X but produces Y false positives on the corpus." Patterns drift over time without measurement.
- **Hard-coded values without justification.** Numeric thresholds (`hard_max_lines`, `prior_art_token_min_len`, `worthiness_*`) and string lists are scattered. Some are correct but unlabeled; some are stale.
- **Runtime hybrid search would tank build times.** `vox-search` (tantivy + qdrant + embeddings) is the right tool for *some* problems, but pulling it into every detector is a non-starter.

## Goals

1. **Single source of truth** for detector patterns/thresholds and Scientia heuristics, in the same shape, loaded by a shared crate.
2. **Reduce hard-coded values in Rust source** to ones that are either (a) deliberately performance-critical, or (b) accompanied by a benchmark justifying the value.
3. **Build-time neutral.** No detector or `vox-publisher` consumer gains a transitive dependency on `vox-search`. The shared loader is zero-heavy-deps (`serde`, `serde_yaml`, `regex` only).
4. **Authoring-time benchmarking.** Patterns and thresholds are stress-tested against labeled fixtures by a `vox ci` command. LLM and `vox-search` are used **at authoring time only** to suggest, label, and grade — never at runtime.
5. **Backward-compatible migration.** Detectors migrate one at a time; each migration is provable parity (same findings on a fixture corpus before/after).

## Non-goals

- Replacing `regex` with semantic search at runtime.
- Adding a new heavy crate. The loader is a thin library.
- Generating Rust source from the SSOT (no codegen step). Detectors load the SSOT at startup; rules live in YAML.
- Touching detectors that are inherently AST-driven (`untested_pub_api`, `unresolved_ref`, `reachability`, `workspace_drift`, `god_object`) — these stay as-is. Only regex/heuristic-driven detectors are in scope.

## Architecture

### Three components

```
contracts/code-audit/rules.v1.yaml            ← rule SSOT (regex, severity, lang, message)
contracts/code-audit/rules.v1.schema.json     ← JSON Schema for the SSOT
contracts/code-audit/fixtures/                ← labeled positive/negative samples per rule
                                                ↓
                            crates/vox-rule-pack/  ← shared, zero-heavy-deps loader
                                                ↓
                ┌───────────────────────────────┴───────────────────────────────┐
                ↓                                                               ↓
   crates/vox-code-audit/src/detectors/        crates/vox-publisher/src/scientia_heuristics.rs
   (regex/heuristic detectors load             (already SSOT-driven; refactor to consume
    pre-compiled patterns from RulePack)        the same RulePack abstraction)
                                                                ↓
                            crates/vox-cli/src/commands/ci/detect_rules_bench.rs
                            (authoring-time tool: runs rules against fixtures, scores
                             precision/recall, optionally calls LLM/vox-search to
                             suggest pattern improvements; produces a report committed
                             to contracts/reports/code-audit/)
```

### Component 1 — `contracts/code-audit/rules.v1.yaml`

Single declarative file. Each rule entry:

```yaml
- id: victory-claim/premature
  parent_id: victory-claim
  name: "Premature victory claim"
  description: "..."
  severity: warning
  confidence: medium
  languages: [rust, typescript, python, vox, gdscript]
  match:
    kind: line-regex          # or: multiline-regex | substring | byte-range
    pattern: "(?i)(?://|#|/\\*|todo!|panic!|unimplemented!).*?(?:\\bdone\\b|...)"
    skip_in:
      - rust-comment-doc      # /// and //!
      - rust-non-code         # comments + strings (uses TokenMap)
  message: "Premature victory claim — verify the implementation is truly complete"
  suggestion: "Remove the comment if complete, or replace with a descriptive comment."
  fixtures:
    positive: ["fixtures/victory-claim/premature_pos_*.txt"]
    negative: ["fixtures/victory-claim/premature_neg_*.txt"]
```

Validated by [`contracts/code-audit/rules.v1.schema.json`](../../../contracts/code-audit/rules.v1.schema.json) (created in Phase 1). The schema is checked in CI via existing `vox-jsonschema-util` infrastructure.

### Component 2 — `crates/vox-rule-pack/`

New crate. Layer L1 (per [`layers.toml`](./layers.toml)). Dependencies: `serde`, `serde_yaml`, `regex`, `thiserror`, `vox-jsonschema-util`. **Forbidden** dependencies (asserted by `vox-arch-check` rule added in Phase 1): `vox-search`, `tantivy`, `qdrant-client`, anything embedding-related.

Public surface:

```rust
pub struct RulePack { /* immutable, Arc-cloneable */ }

impl RulePack {
    pub fn load_from_path(path: &Path) -> Result<Self, RulePackError>;
    pub fn load_embedded() -> Result<Self, RulePackError>;  // include_str! at compile time
    pub fn rule(&self, id: &str) -> Option<&CompiledRule>;
    pub fn rules_for_language(&self, lang: Language) -> impl Iterator<Item = &CompiledRule>;
}

pub struct CompiledRule {
    pub id: &'static str,                  // interned via leak-on-load
    pub severity: Severity,
    pub confidence: Option<Confidence>,
    pub languages: &'static [Language],
    pub matcher: Matcher,                  // enum: LineRegex(Regex) | MultilineRegex(Regex) | Substring(String) | …
    pub message_template: &'static str,
    pub suggestion: Option<&'static str>,
    pub skip_in: &'static [SkipScope],     // RustComment, RustNonCode, Doc, …
}
```

The loader compiles regexes once at startup. `Matcher::matches(text, ctx)` is the only runtime hot path. Skip scopes are evaluated by callers using their existing `RustFileContext` / `TokenMap` infrastructure — `vox-rule-pack` does not parse Rust.

### Component 3 — `vox ci detect-rules-bench`

A new CLI subcommand under [`crates/vox-cli/src/commands/ci/`](../../../crates/vox-cli/src/commands/ci/). Wired through the existing CI command catalog so it shows up in [`docs/src/reference/cli-command-surface.generated.md`](../reference/cli-command-surface.generated.md).

Runtime: pure benchmarking. For each rule:

1. Load `contracts/code-audit/fixtures/<rule-id>/positive_*.txt` and `negative_*.txt`.
2. Run the compiled `Matcher` against each fixture; record TP / FP / FN.
3. Emit precision, recall, F1 per rule into `contracts/reports/code-audit/rules-bench-latest.json`.
4. **Authoring-time only optional flag** `--suggest`: when set, sends the false positives to the existing review/providers infra ([`crates/vox-code-audit/src/review/providers.rs`](../../../crates/vox-code-audit/src/review/providers.rs)) for an LLM-suggested pattern refinement. May also use `vox-search` from the **CLI process only** to retrieve similar real-corpus snippets. Neither dependency is added to `vox-code-audit` or `vox-rule-pack`; both already exist behind `vox-cli`.
5. CI gate (Phase 5): rules with F1 below a per-rule threshold (declared in the YAML) fail the bench command.

### Component 4 — Generalize Scientia heuristics onto `vox-rule-pack`

`scientia_heuristics.rs` already loads from a YAML seed. Phase 6 refactors it to express its tunables as a `RulePack`-style document, sharing schema validation and the loader. This eliminates a parallel loader and lets `vox ci detect-rules-bench` cover both surfaces.

## Build-time invariants

Asserted by `cargo run -p vox-arch-check` after the rules crate lands:

1. `vox-rule-pack` MUST NOT depend on `vox-search`, `vox-corpus`, `vox-embeddings`, `tantivy`, `qdrant-client`, `vox-orchestrator-mcp`.
2. `vox-code-audit` MUST NOT depend on `vox-search`. (Already true; this is a new pin.)
3. `vox-publisher` MUST NOT depend on `vox-search` *as a hard dep*. (Already true.)
4. New regex literal in `crates/vox-code-audit/src/detectors/**/*.rs` MUST be either (a) under `// rules-pack-exempt: <reason>` or (b) sourced from a `RulePack`. Enforced by a new lint in `vox-code-audit/src/bin/toestub.rs` extension or `vox-arch-check` rule (Phase 7).

## Migration order

Detectors are migrated in waves, ranked by ratio of pattern-volume to AST-coupling:

| Wave | Detector(s) | Why first / last |
|---|---|---|
| 1 (pilot) | `victory_claim` | 4 regexes, no AST, parity-checkable in a day. Proves the pattern. |
| 2 | `ai_laziness` | 7 regexes, partial AST coupling (`is_test_gated`). Largest single bang. |
| 3 | `magic_value`, `stub`, `secrets` | Regex + numeric thresholds. |
| 4 | `scaling`, `dry_violation`, `stringly_typed_enum`, `unwrap_call`, `hollow_fn`, `empty_body` | Mixed regex + AST, slower. |
| 5 | Scientia heuristics SSOT consolidation | Refactor existing seed loader onto `vox-rule-pack`. |
| 6 | Enforcement: `vox-arch-check` rule + CI bench gate | Lock in the gains. |

AST-driven detectors out of scope: `untested_pub_api`, `unresolved_ref`, `reachability`, `unwired_module`, `workspace_drift`, `god_object`, `file_organization`, `sprawl`, `schema_compliance`, `no_test_for_pub_fn`, `line_endings`. These keep their current shape; their *thresholds* migrate (see Phase 4) but the matching code does not.

## What stays hard-coded

The design explicitly preserves hard-coded values when:

- Performance: bytewise scanning loops with literal patterns where regex compilation overhead matters.
- Language semantics: `Language::from_extension`, `BUILTIN_DEFAULT_TYPES`, Rust keyword sets — these encode language facts, not policy.
- Stable schemas: `Severity` enum variants, `FindingConfidence` variants.

These are all annotated with a `// rules-pack-exempt: <reason>` marker (Phase 7), so the lint can pass them and reviewers can audit the set.

## Testing strategy

Per [AGENTS.md §Test-First Policy](../../../AGENTS.md):

- Every new `pub fn` in `vox-rule-pack` ships with a failing test first.
- Every detector migration: parity test that runs the pre-migration detector and post-migration detector against the same fixture corpus and asserts identical findings (modulo deterministic ordering).
- Bench command: gold dataset under `crates/vox-code-audit/tests/gold_dataset.rs` is extended, not replaced.

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| Regex compile cost at startup grows | Benchmark in Phase 1 acceptance: load + compile of full SSOT must be ≤ 50 ms on dev hardware. Lazy-compile per-language if exceeded. |
| YAML errors break the build | `RulePack::load_embedded` is exercised by a unit test; CI also runs schema validation. Bad YAML fails fast at startup, not in the middle of a scan. |
| Pattern parity drift during migration | Each migration PR runs the parity harness; PRs that change findings on the fixture corpus must update the fixture explicitly and label the change. |
| Authoring-time LLM/`vox-search` use leaks into runtime | `vox-arch-check` dependency rule (invariant #1) blocks it at the crate-graph level, not by review. |
| Scope creep (rewriting AST detectors) | Out-of-scope list above is normative. AST detectors keep their code; only their *threshold constants* migrate. |

## Acceptance criteria

1. `cargo build --workspace` clean.
2. `cargo run -p vox-arch-check` clean, including the four new dependency invariants.
3. `cargo test -p vox-code-audit` clean, including the parity tests for every migrated detector.
4. `vox ci detect-rules-bench` produces `contracts/reports/code-audit/rules-bench-latest.json` with non-empty entries for every migrated rule.
5. `vox ci detect-rules-bench --check` passes (no F1 below per-rule threshold).
6. `scientia_heuristics.rs` consumes the unified `RulePack` API; the existing acceptance tests in [`crates/vox-publisher/tests/scientia_novelty_acceptance.rs`](../../../crates/vox-publisher/tests/scientia_novelty_acceptance.rs) remain green.
7. Doc-pipeline and pre-commit hooks pass (`cargo run -p vox-doc-pipeline -- --check`).

## Out of scope (deliberately deferred)

- Real-time pattern updates without rebuild (requires runtime config-watch infrastructure).
- Cross-repo rule sharing (a separate concern; punt until at least one external consumer asks).
- Replacing detector severity tuning with a learned model.
- Migrating non-detector regex usage in `vox-cli`, `vox-orchestrator`, `vox-corpus`. The SSOT pattern can spread later if it proves useful here.
