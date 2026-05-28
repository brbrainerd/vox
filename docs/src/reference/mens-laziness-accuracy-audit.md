---
title: "Mens laziness and accuracy audit"
description: "Severity-ranked audit of the current VoxMens research implementation, focusing on LLM-style mistakes, drift, brittle heuristics, and durability risks."
category: "Language Reference"
last_updated: "2026-03-28"
training_eligible: false

schema_type: "TechArticle"
---
# Mens laziness and accuracy audit

This document records a targeted audit of the current VoxMens groundwork implementation. It is intentionally focused on the kinds of issues large language models often introduce when asked to implement broad plans:

- duplicated logic instead of wiring through an existing shared path,
- hard-coded thresholds without a durable contract,
- producer/consumer drift across files,
- metrics that sound right but do not actually measure the stated objective,
- partial implementations that create a second parallel system.

This is a research audit, not a remediation plan. The next pass should convert the highest-priority findings into implementation milestones.

## Audit target

Primary implementation surfaces reviewed:

- [`crates/vox-cli/src/commands/ci/mens_scorecard.rs`](../../../crates/vox-cli/src/commands/ci/mens_scorecard.rs)
- [`crates/vox-cli/src/commands/generate.rs`](../../../crates/vox-cli/src/commands/generate.rs)
- [`crates/vox-orchestrator-mcp/src/compiler_tools.rs`](../../../crates/vox-orchestrator-mcp/src/compiler_tools.rs)
- [`crates/vox-orchestrator-mcp/src/speech_constraints.rs`](../../../crates/vox-orchestrator-mcp/src/speech_constraints.rs)
- [`crates/vox-orchestrator-mcp/src/text_normalization.rs`](../../../crates/vox-orchestrator-mcp/src/text_normalization.rs)
- [`crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs`](../../../crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs)
- [`crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs`](../../../crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs)
- [`crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs`](../../../crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs)
- [`crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs`](../../../crates/vox-populi/src/mens/tensor/backend_candle_qlora.rs)
- [`contracts/eval/mens-scorecard.schema.json`](../../../contracts/eval/mens-scorecard.schema.json)
- [`contracts/eval/mens-scorecard.baseline.json`](../../../contracts/eval/mens-scorecard.baseline.json)

## Summary judgment

The current work is directionally good. It adds genuinely useful scaffolding:

- a scorecard path for model-vs-model comparisons,
- stronger generation repair behavior,
- post-validation canonicalization,
- a first practical constrained-output guard,
- better training run summaries.

The main weakness is not that the work is wrong. The main weakness is that parts of it are still **prototype-shaped** rather than **SSOT-shaped**. Several behaviors are implemented in parallel across CLI, MCP, and CI rather than routed through one shared contract.

That matters because VoxMens is now trying to optimize three things simultaneously:

1. valid `.vox`,
2. canonical/de-whitespaced `.vox`,
3. fast generation with low repair cost.

Those goals are tightly coupled. If the measuring path, repair path, and output normalization path drift apart, the system can look like it is improving while the real product behavior remains flat.

## Severity matrix

| Severity | Finding | Why it matters |
|---|---|---|
| Critical | `voxelized_strictness` semantics are weaker than intended in scorecard | A misleading metric can create false confidence and distort the custom-model decision gate |
| Critical | MCP prompt policy conflicts with surface guard in constrained mode | The model can be asked to emit fenced code and then be penalized for doing so |
| High | Fence-stripping and surface-normalization logic is duplicated across CLI, MCP, and scorecard | Small drift here produces hard-to-debug disagreement between code paths |
| High | Scorecard schema validates too little; runtime errors carry contract burden | Invalid benchmark specs pass verification and fail later |
| High | Decision thresholds are hard-coded and string-heuristic based | The go/no-go gate is fragile and not reusable across benchmark sets |
| High | Multiple “valid Vox” gates exist without one canonical API contract | CLI, MCP, and scorecard can disagree about what counts as valid |
| Medium | Token counts in scorecard are whitespace proxies, not model tokens | Can lead to incorrect speed/cost comparisons |
| Medium | Training DB event persistence is uneven and some failures are swallowed | Important telemetry can disappear silently |
| Medium | Event naming and schema ownership are split between JSONL, DB, and gate readers | Increases long-term divergence risk |
| Low | Baseline scorecard defaults are local-smoke oriented and easy to mistake for production SSOT | Fine for bootstrap, risky if treated as policy |

## Critical findings

### 1. Scorecard strictness is not yet a trustworthy product metric

Current scorecard work introduced `voxelized_strictness`, but it is still a heuristic. In practice it currently behaves more like:

- “did we avoid obvious prose wrappers?”

than:

- “did the model emit exactly the canonical code-shaped payload we want?”

This matters because strictness is one of the central reasons to consider a custom model at all. If this metric is weak, then the custom-model gate in the scorecard becomes weak too.

Observed issues:

- strictness is still based on wrapper/prose heuristics rather than a true canonical-output contract,
- the metric is evaluated in a different environment from the MCP/CLI serving path,
- strictness is not yet tied to a shared normalization function that all surfaces use.

Durable direction:

- define one shared output-surface contract for Vox code generation,
- score strictness off the same contract used by CLI and MCP,
- distinguish:
  - `rawSurfaceStrict`,
  - `postNormalizationStrict`,
  - `canonicalOutputStrict`.

### 2. Constrained mode still contains an internal contradiction

The constrained-decode scaffold is useful, but the current policy still mixes two incompatible ideas:

- “wrap in a fenced Vox block,” and
- “do not emit non-code wrapper text.”

This is exactly the kind of LLM implementation flaw that looks harmless during development but creates noisy repair loops in production. The model receives mixed incentives. Once the guard is enabled, a fenced answer can be both encouraged and punished.

Durable direction:

- define two explicit surface modes:
  - `fenced_transport_mode`
  - `raw_code_mode`
- make prompt policy, stripping, and validation all choose the same mode.

## High findings

### 3. Shared normalization logic is not centralized yet

There are multiple copies of fence stripping / surface cleanup behavior:

- CLI generation,
- MCP generation,
- scorecard harness,
- existing MCP text normalization helpers.

This is a classic divergence trap. The second pass should not keep adding “small local copies” of this logic.

Durable direction:

- centralize into one shared helper module or crate,
- define one normalization sequence:
  1. surface cleanup,
  2. validation,
  3. canonicalization,
  4. strictness scoring.

### 4. Scorecard contract is still runtime-first, not schema-first

The schema for `mens-scorecard` is a strong start, but it still leaves some mode-specific requirements to runtime checks. For example, benchmark specs can still be structurally valid while missing fields required by a specific condition mode.

That pushes correctness into Rust control flow instead of the declared contract. This is another common LLM error pattern: “implement the happy path and let code branch guards do the rest.”

Durable direction:

- extend schema conditionals for mode-specific requirements,
- add artifact schemas for generated outputs too, not just input spec,
- version the scorecard output contract separately from the input spec.

### 5. Decision thresholds are too magical

Examples of likely unstable hard-coded values:

- strictness thresholds,
- plateau percentages,
- burn-vs-qlora delta cutoffs,
- grammar artifact truncation sizes,
- fixed retry caps in some paths without an explicit contract.

Hard-coded values are not always wrong. The issue is that several of them currently live in code without a durable explanation of:

- what they optimize,
- what they trade off,
- how to tune them per benchmark set or lane.

Durable direction:

- move threshold ownership into one of:
  - scorecard spec,
  - policy file,
  - telemetry schema defaults documented in docs,
- require each threshold to declare:
  - owner,
  - unit,
  - failure mode,
  - expected tuning cadence.

### 6. “Valid Vox” is still expressed through multiple near-equivalent APIs

Today, validity can be checked through:

- the CLI frontend pipeline,
- LSP/HIR validation,
- scorecard frontend checks,
- MCP validation loop.

These are related but not yet presented as one canonical validity contract.

That is dangerous because the project’s main product claim is not “the text looks plausible.” It is “the model emits valid, usable Vox.”

Durable direction:

- define one public `validate_generated_vox` contract,
- specify exactly which stages it includes:
  - lex,
  - parse,
  - typecheck,
  - HIR validation,
  - optional canonicalization re-parse,
- route all external surfaces through that contract or document the narrower variants explicitly.

## Medium findings

### 7. Current scorecard speed metrics are only partial proxies

The scorecard records latency, which is useful, but its token accounting is not true tokenizer-level accounting. That makes it unsuitable for serious cost/speed comparison across backends or models.

This is not fatal, but it should be documented as a temporary proxy, not as a production KPI.

### 8. Training telemetry got better, but not yet fully coherent

Adding `run_summary.json` and epoch summary events was a good improvement. The remaining concern is coherence:

- some values live in telemetry JSONL,
- some are mirrored into DB events,
- some gates still read older or mismatched field names.

This is a “half-integrated” state. It is useful for exploration, but not yet a durable measurement contract.

### 9. Error handling in DB and telemetry paths still has silent edges

Some paths log failures clearly; others use best-effort patterns that may drop useful evidence. In a training pipeline that is already long-running and difficult to reproduce, silent loss of telemetry is costly.

## Low findings

### 10. Baseline benchmark defaults are bootstrap-oriented

The default scorecard spec is fine as a local example, but it should be treated as:

- a smoke harness starter,

not:

- the canonical benchmark design for strategic decisions.

The second pass should separate:

- example specs,
- team-owned benchmark packs,
- release-quality benchmark packs.

## Where existing systems should be reused more aggressively

The most important architectural lesson from this audit is simple:

**VoxMens should reuse the same contracts across training, generation, evaluation, and documentation, rather than building local approximations in each layer.**

The highest-value reuses are:

1. **One normalization pipeline**
   - Reuse existing MCP text normalization helper rather than embedding more local copies.

2. **One validity contract**
   - Reuse a shared generated-code validation function across CLI, MCP, and scorecard.

3. **One telemetry/event vocabulary**
   - Reuse stable event names and field ownership between JSONL telemetry, DB mirrors, and eval gates.

4. **One output-surface policy**
   - Reuse the same notion of “raw code only” or “fenced transport” everywhere.

## Audit conclusion

The implementation is a strong first pass, but it still shows the classic signs of an LLM-assisted rollout:

- good feature coverage,
- good local reasoning,
- incomplete contract centralization,
- several heuristic decisions embedded in code before their ownership model is defined.

That is acceptable at the groundwork stage. It is **not** acceptable as the long-term basis for measuring whether QLoRA is enough or whether Vox needs a more custom model path.

## Required follow-up questions for the next pass

The second-pass implementation plan should answer these explicitly:

1. What is the one canonical “generated Vox output contract”?
2. Which validity function is the SSOT across CLI, MCP, CI, and benchmarks?
3. Which thresholds belong in schema/policy rather than code?
4. Which scorecard metrics are strategic KPIs vs temporary heuristics?
5. Which helper paths should be merged before adding any more generation features?

