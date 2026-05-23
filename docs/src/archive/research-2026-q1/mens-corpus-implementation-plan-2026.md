---
title: "MENS Corpus: Full Implementation Plan (2026)"
description: "Executable, wave-gated implementation plan for escaping the synthetic data paradox in Vox MENS. Grounded in codebase audit of actual mix reports, evaluate code, and research synthesis."
category: "Architecture SSOTs"
status: "roadmap"
research_date: "2026-04-12"
last_updated: "2026-04-12"
training_eligible: false
sort_order: 40

schema_type: "TechArticle"
archived_date: 2026-04-18
---

# MENS Corpus: Full Implementation Plan (2026)

## Audit Findings — What Is Actually Happening

> [!CAUTION]
> The mix report for `train_mixed_vox_lang.jsonl` reveals a **critical failure state** that supersedes the assumptions in the research doc. The vox-lang corpus is **97.3% synthetic data** from a single file.

### Verified Corpus State (from `mens/data/train_mixed_vox_lang.mix_report.json`)

| Lane | File | Lines Emitted | Share |
|---|---|---|---|
| **golden** (weight 6) | `target/dogfood/vox_corpus_extract.jsonl` | **0** | **0%** — missing file |
| organic (weight 3) | `target/dogfood/organic_vox.jsonl` | **0** | **0%** — missing file |
| docs (weight 2) | `mens/data/mix_sources/docs.jsonl` | 234 | 2.7% |
| synthetic (weight 1) | `mens/data/synthetic.jsonl` | 8,481 | **97.3%** |
| distillation (weight 2) | `target/dogfood/distillation_traces.jsonl` | **0** | **0%** — missing file |

**Total: 8,715 lines — nearly all from one template-expanded file.**

The weight system is functioning correctly — but it is working on files that do not exist. The 6× golden weight is a dead letter because there is zero golden data. The pipeline is operating in complete synthetic monoculture.

### Additional Findings from Code Audit

1. **`negative.rs` generates surface-level mutations** (remove `}`, swap `fn` → `fun`, mangle `let` → `lett`). These are lexer-level corruptions, not semantically meaningful errors. They are not wired to any DPO training path.

2. **`vox-eval/src/lib.rs`** has `CollateralDamageReport`, `eval_collateral_damage()`, and `cargo_build_reward()` / `cargo_test_reward()` already implemented — but there is no evidence these are wired to a pre-training gate or promotion check in the actual training loop.

3. **The `detect_constructs()` and `construct_coverage_score()` functions are `#[deprecated(since = "0.4.0")]`** — they are marked deprecated in favor of `vox_compiler::ast_eval()`, but the training pipeline has no evidence of using the parser-backed path.

4. **`healing.rs`** is fully implemented with `HealPair` logging to `~/.vox/corpus/heal_pairs.jsonl` — but this is in `vox-populi/src/mens/healing.rs`, separate from the training pipeline, and there is no corresponding mix lane or DPO training path wired to it.

5. **`research_gen.rs` is implemented** with fictional knowledge graph chains — but does not have a `mix-research-expert.yaml` consuming it (that file is referenced in `domain-profiles.yaml` but does not appear in `mens/config/`).

6. **The rust corpus is 100% from a single `rust_source.jsonl`** — repeated 3× (`351,324 emitted from 117,108 input lines`). There is no Rust-to-Vox cross-pollination pipeline.

7. **`review-weight-policy.yaml`** governs truth-tier weights for review intelligence, not corpus anchor ratios. The existing `eval-gates.yaml` already has `supervised_ratio.min_pct: 10.0` — but this refers to the supervised fraction of a training batch, not the golden corpus fraction.

8. **The `vox-constrained-gen` crate exists** — this is the grammar-constrained decoding infrastructure. The integration with training data generation (generating only compilable code via logit masking) is not yet connected.

---

## Corrected Problem Statement

The original research doc identified the *right failure modes* but underestimated the severity. The actual state is:

| Problem | Severity in Research Doc | Actual Severity |
|---|---|---|
| Template exhaustion / low diversity | High | **Critical** — 97.3% from one file |
| Synthetic monoculture | Addressed as "MAD risk" | **Active, immediate** — no golden data |
| Oracle problem | Critical | Critical |
| Missing DPO lane | Moderate | **High** — HealPair data already exists, just unwired |
| Anchor floor not enforced | Proposed as config change | **Blocked** — no golden data to anchor |
| AST-aware mutation | Proposed | **The correct first response** — must build golden corpus first |

archived_date: 2026-04-18
---

## Execution Strategy

The plan is organized into five waves. Waves are sequential; later waves depend on infrastructure from earlier ones.

```
Wave 0 (Immediate):  Fix the missing golden data — unblock the weight system
Wave 1 (Foundation): Build the two missing critical infrastructure components
Wave 2 (Data Growth): Expand corpus with mutation + DPO wiring
Wave 3 (Quality):    Add semantic quality gates and curator layer
Wave 4 (Automation): Automate the flywheel
```

---

## Wave 0: Corpus Emergency — Bootstrap the Golden Lane (Week 1)

**Goal:** Produce a real `target/dogfood/vox_corpus_extract.jsonl` so the 6× golden weight is not dead.

### W0-01 — Walk All `.vox` Files and Emit a Corpus Extract

The `core.rs`:`walk_vox_files()` and `build_training_record()` functions already exist. The issue is that no CLI command is wired to run them across the workspace and deposit results to `target/dogfood/vox_corpus_extract.jsonl`.

**Files to modify:**
- `crates/vox-cli/src/commands/` — add a `vox populi corpus extract` subcommand (or extend an existing one) that:
  1. Calls `walk_vox_files(examples/golden/)` — the Tier A corpus
  2. Runs each file through `crates/vox-cli/src/pipeline.rs`:`FrontendResult`
  3. For each success, calls `build_training_record()` and appends to `target/dogfood/vox_corpus_extract.jsonl`
  4. Reports a summary: files walked / parse pass / pairs emitted / construct distribution

**Implementation note:** `build_training_record()` emits `{source, code, constructs, difficulty, ast_hash, compiler_version}` but the training pipeline expects `{instruction, response, category}` pairs in ChatML format. A second pass using `instruction.rs`:`instruction_templates()` must be added to convert raw records to instruction pairs.

**Expected output:** The golden lane should produce several hundred to low thousands of verified pairs from `examples/golden/`. This immediately shifts the synthetic share down and activates the 6× weight.

### W0-02 — Add Corpus Extract to CI

Add `vox populi corpus extract` to the weekly CI nightly job so the golden corpus refreshes when new `.vox` examples are added to the `examples/golden/` tree.

**Exit criterion:** `train_mixed_vox_lang.mix_report.json` shows `>0` emitted lines for the golden lane.

archived_date: 2026-04-18
---

## Wave 1: Foundation Infrastructure (Weeks 2–3)

### W1-01 — Wire `heal_pairs.jsonl` to a DPO Lane

**Current state:** `healing.rs` logs `HealPair{description, failed_source, diagnostics, repaired_source, attempts}` to `~/.vox/corpus/heal_pairs.jsonl` when `attempt > 1`.

**Problem:** Nothing reads this file. No mix config references it.

**Implementation steps:**

1. **Add a DPO converter command** `vox populi corpus heal-to-dpo` that reads `~/.vox/corpus/heal_pairs.jsonl` and emits `preference_pairs.jsonl` where each record is:
   ```json
   {
     "prompt": "<description + compiler diagnostics as context>",
     "chosen": "<repaired_source>",
     "rejected": "<failed_source>",
     "category": "vox_heal_dpo",
     "attempts": 2
   }
   ```
   Filter: only include pairs where `attempts == 1` (first-attempt repair quality is highest signal). Multi-attempt pairs have lower confidence.

2. **Add a DPO source to `mix-vox-lang.yaml`:**
   ```yaml
   - path: target/dogfood/preference_pairs.jsonl
     weight: 3.0
     optional: true
     record_format: dpo
   ```
   Weight of 3.0 is justified: these are compiler-verified `(chosen, rejected)` pairs with ground-truth error signals.

3. **Add DPO-aware training path in the MENS orchestrator.** The `trl` library's `DPOTrainer` (Python-side, or a compatible Rust binding) should be invoked when `record_format: dpo` lanes are present. β = 0.1 is a safe starting point per 2026 research.

**Important constraint (from research):** DPO requires the model to have been SFT-tuned first. The DPO run must be a *second phase* after the SFT run, not concurrent.

**Risk:** The `negative.rs` mutations (remove `}`, swap `fn` → `fun`) are lexer-level corruptions that would produce low-quality rejected samples. Do **not** use `negative.rs` output for DPO without compiler verification. Use only `heal_pairs.jsonl` entries (which are compiler-verified rejections).

### W1-02 — Create `mix-research-expert.yaml` and Wire `research_gen.rs`

**Current state:** `research_gen.rs` is implemented and emits fictional multi-hop chains, but `mix-research-expert.yaml` is referenced in `domain-profiles.yaml` at line `98` and does not exist in the filesystem.

**Implementation steps:**

1. Create `mens/config/mix-research-expert.yaml`:
   ```yaml
   # Mix configuration for the research-expert domain (Lane G)
   output: mens/data/train_mixed_research_expert.jsonl
   sources:
     - path: target/dogfood/research_chains.jsonl
       weight: 4.0
       optional: true
     - path: target/dogfood/socrates_traces.jsonl
       weight: 3.0
       optional: true
   ```

2. Add a CLI command `vox populi corpus research-gen --count 10000 --output target/dogfood/research_chains.jsonl` that calls `generate_research_chains()`.

3. Add diversity controls to `research_gen.rs`: the current entity pool (`Aetherium`, `Borealis`, etc.) is 20 entities × 8 actions × 8 versions. At 4 hops, the effective unique-chain count is well below 1,000 before deduplication. Add at least 5× more entities and relationship templates. Introduce causal chain types (temporal, conditional, contrastive) to avoid structural homogenization.

### W1-03 — Enforce the `eval-gates.yaml` Collateral Damage Check

**Current state:** `vox-eval` has `eval_collateral_damage()` and `eval_collateral_damage_suite()` implemented and tested. The `eval-gates.yaml` has `pass_at_k` and `review_recurrence` sections. But there is no evidence the `CollateralDamageReport` is computed before adapter promotion.

**Implementation steps:**

1. Add a `vox mens eval collateral-damage --pre-score <path> --post <adapter-path>` subcommand that:
   - Runs a held-out eval against a static general benchmark (MMLU subset, GSM8K subset — see §W3 for dedicated Vox-lang benchmark)
   - Calls `eval_collateral_damage_suite()`
   - Exits with `1` if any benchmark exceeds `max_degradation_rate: 0.05`
   - Outputs a `collateral_damage_report.json`

2. **Add this as a required gate before `vox mens serve` will accept an adapter.** The `FineTuneContract` struct should gain a `collateral_damage_verified: bool` field.

---

## Wave 2: Corpus Expansion (Weeks 3–5)

### W2-01 — AST-Aware Mutation Engine (`vox-corpus` new module)

**Research basis:** 2026 research on AST-guided mutation (TreeDiff, reasoning-centered generation) confirms that mutation from valid seed programs produces structurally diverse, compiler-checkable programs. This is the highest-ROI expansion for the `vox-lang` domain given the existing `extract_constructs()` infrastructure.

**Precondition:** Wave 0 must be complete. The mutation engine starts from golden corpus programs, not from template-expanded synthetics.

**Implementation — new file `crates/vox-corpus/src/ast_mutator.rs`:**

The mutator takes a parsed `Module` (already available from `vox_compiler`) and applies one of four strategies:

| Strategy | Mechanism | Expected Validity Rate |
|---|---|---|
| **Literal substitution** | Replace integer/string literals with random alternatives of same type | ~100% — type-preserving |
| **Identifier rename** | Rename a function/actor/variable to a fresh identifier | ~100% — syntax-preserving |
| **Block decoration** | Wrap an actor handler in a retry policy or add a timeout annotation | ~80% — depends on protocol |
| **Construct transplant** | Extract a field declaration from one type and inject it into another (type-checking required) | ~40% — needs typecheck pass |

For each mutation:
1. Apply the transformation to the AST (in-source form via text manipulation keyed to span information from the parser)
2. Run the resulting source through the compiler pipeline
3. If it compiles: emit as a golden Tier B pair with an instruction generated from `instruction_templates()`
4. If it fails: emit as a `HealPair` candidate for the DPO lane

This directly produces both positive training pairs (for SFT) and negative training pairs (for DPO) from the same mutation pass.

**CLI wire-up:** `vox populi corpus mutate --source-dir examples/golden --count 5000 --output target/dogfood/mutated_vox.jsonl`

**Update `mix-vox-lang.yaml`:**
```yaml
- path: target/dogfood/mutated_vox.jsonl
  weight: 4.0
  optional: true
```
Weight 4.0 (between organic and synthetic) reflects the higher quality of compiler-verified mutations vs. template expansion.

### W2-02 — Upgrade `negative.rs` to Semantic Mutations

**Current state:** `negative.rs` performs 4 surface-level lexer mutations. These are low-signal training pairs.

**Upgrade:** Add semantic-level mutations that produce *meaningful* error signals:

1. **Wrong return type**: change a declared return type so it conflicts with a returned value (requires type information from HIR)
2. **Missing handler**: remove a message handler from an actor implementation, leaving a declared message type with no handler
3. **Cyclic dependency**: add an import that creates a module dependency cycle
4. **Unresolved name**: rename a type in its declaration but leave all use-sites unchanged

These require access to the compiler's AST/HIR, not just source text — use the `extract_constructs()` pipeline.

**Note:** The upgraded negative examples should still be primarily consumed through the DPO lane (`heal_pairs.jsonl` format), not as standalone training examples. Per DPO research, they should be balanced 2:1 positive:negative.

### W2-03 — Rust → Vox Cross-Domain Translation Pairs

**Research basis:** The Rust corpus is extremely large (351,324 lines from 117,108 inputs) and fully compiler-verified. Translating idiomatic Rust patterns into equivalent Vox DSL constructs is uniquely powerful because:
- Intent is grounded in human-authored, compiler-verified Rust code
- Vox actors map structurally to Rust async tasks
- Vox workflows map to Rust future combinators
- The Vox type system has direct ADT equivalents to Rust enums

**Implementation — new file `crates/vox-corpus/src/rust_to_vox.rs`:**

Focus on narrow, high-confidence translation patterns:

| Rust Pattern | Vox Equivalent | Confidence |
|---|---|---|
| `struct` with `impl` block + methods | `actor` declaration | High (structural mapping) |
| `enum` with `match` exhaustive | `type` tagged union + `match` | High (syntactic similarity) |
| `tokio::spawn` + channel | `spawn()` + actor message | Medium (semantic equivalent) |
| `#[derive(Serialize, Deserialize)]` | `@table` or typed field access | Medium (context-dependent) |

For each successful translation:
1. Generate instruction: "Translate this Rust pattern to its Vox equivalent"
2. Response: the Vox code
3. Run through the Vox compiler to verify
4. Emit verified pair to `target/dogfood/rust_to_vox.jsonl`

**Update `mix-vox-lang.yaml`:**
```yaml
- path: target/dogfood/rust_to_vox.jsonl
  weight: 5.0
  optional: true
```
Weight 5.0 — these are the highest-quality pairs because both source (Rust compiler verified) and target (Vox compiler verified) are ground-truth correct.

archived_date: 2026-04-18
---

## Wave 3: Semantic Quality Gates (Weeks 5–7)

### W3-01 — Vox-Lang Held-Out Benchmark (`vox-bench`)

**Problem:** The collateral damage check (W1-03) currently requires an external general benchmark (MMLU, GSM8K). There is no held-out Vox-specific benchmark that can detect regression in Vox code generation quality.

**Implementation — new directory `mens/bench/`:**

Create a static, frozen benchmark of 200 Vox generation tasks spanning all construct types:

```
mens/bench/
  vox-lang-bench-v1.jsonl    # 200 instruction→reference pairs
  vox-lang-bench-v1.sha256   # integrity check
  run_bench.sh               # vox mens eval bench --adapter <path>
```

The benchmark must be:
- **Frozen**: never updated after initial creation (changing it invalidates historical comparisons)
- **Diverse**: at least 10 examples per construct type across all difficulty tiers
- **Compiler-verified**: every reference response must parse and typecheck

The `pass@1` rate on this benchmark is the Vox-specific regression metric. Gate: `min_pass_rate_at_1: 0.25` (already in `eval-gates.yaml`; needs to be wired to this benchmark).

### W3-02 — Semantic Entropy Monitor in `vox-eval`

**Research basis:** The risk taxonomy in `research-cl-risk-taxonomy-telemetry-2026.md` identifies semantic entropy as the primary early-warning signal for mode collapse. `vox-eval` currently measures only parse validity and construct coverage.

**New function in `crates/vox-eval/src/lib.rs`:**

```rust
pub struct SemanticEntropyReport {
    /// Fraction of sampled outputs that are structurally distinct ASTs.
    pub ast_diversity: f64,
    /// Variance in construct counts across samples.
    pub construct_variance: f64,
    /// Whether the entropy is below the collapse warning threshold.
    pub collapse_warning: bool,
}

/// Sample `n` outputs from the model for the same prompt at temperature T,
/// parse each, and measure structural diversity.
pub fn eval_semantic_entropy(
    outputs: &[String],
    collapse_threshold: f64,
) -> SemanticEntropyReport
```

This function:
1. Parses each output with the Vox compiler
2. Computes a hash of each resulting AST (using the existing `vox_hash_fast()` function from `vox_runtime::builtins`)
3. Measures the fraction of unique AST hashes
4. Reports `collapse_warning: true` if the unique fraction falls below `collapse_threshold` (recommended: 0.6)

**Wire to training loop:** The training orchestrator should call `eval_semantic_entropy` after each epoch on a fixed set of 50 prompts. If `collapse_warning` is triggered, the training run should pause and require manual review before proceeding to the next epoch.

### W3-03 — AST Diversity Monitor for Mix Quality

**Related to W3-02** but applied to the corpus rather than model outputs.

**New command:** `vox populi corpus diversity-check --input <mix.jsonl> --min-ast-diversity 0.40`

This command:
1. Reads all records from the mix output
2. Parses each Vox code field
3. Computes the fraction of unique AST structures (via hash)
4. Emits a `diversity_report.json`
5. Exits with `1` if diversity is below the threshold

**Add to CI:** Block corpus promotion from Tier B to training input if `ast_diversity < 0.40`. This directly prevents the template-exhaustion problem: if 97% of the corpus is from one file (as it currently is), the diversity score will be well below 0.40 and the CI gate will fail loudly.

### W3-04 — Frontier Curator Gate for Prose Lanes

**Applies to:** `mix-research.yaml`, `mix-populi-meta.yaml`, `mix-research-expert.yaml`

**Current state:** No prose quality gate exists. The `research_gen.rs` fictional chains are structurally uniform (20 entities, 8 actions).

**Implementation — new command `vox populi corpus curate-prose`:**

For each record in a prose-domain JSONL:
1. Call a frontier model via the existing Clavis-managed API keys (Anthropic/Gemini) with a curator prompt
2. The curator prompt asks: "Does this explanation contain logical inconsistencies, hallucinated APIs, structural repetition (em-dash overuse, 'It's not just X, it's Y' patterns), or claims that are unfalsifiable?" 
3. Records scoring below a `semantic_integrity_threshold` are moved to a quarantine file
4. Accepted records flow to the training mix

**Cost estimate:** ~$0.002 per record (Gemini Flash pricing). At 10,000 records, this is a $20 one-time cost per corpus refresh.

---

## Wave 4: Automated Flywheel (Weeks 7–9)

### W4-01 — Flywheel State Machine in `vox-corpus/src/flywheel.rs`

**Current state:** The flywheel is manual. An operator must run `vox populi corpus extract` and trigger training. Research confirms that automated, continuously improving flywheels compound quality faster than manual ones.

**Implementation — new struct `FlywheelState`:**

```rust
pub struct FlywheelConfig {
    /// Minimum new dogfood records before triggering a corpus refresh.
    pub sample_floor: usize,                // Default: 500
    /// Must exceed this diversity score before triggering a training run.
    pub min_ast_diversity: f64,             // Default: 0.40
    /// Maximum hours between forced check-ins.
    pub max_interval_hours: u64,            // Default: 168 (1 week)
    /// Enable automatic training trigger (vs. emit signal only).
    pub auto_train: bool,                   // Default: false (HITL gate)
}
```

The flywheel state machine runs as a background task in the Vox daemon (`vox-dei`) and:
1. **Monitors** the dogfood directory for new session logs
2. **Gates** on `sample_floor` (hysteresis to prevent flapping)
3. **Validates** ast_diversity of the candidate new corpus
4. **Signals** `vox mens train --trigger flywheel` when gates pass (if `auto_train: false`, emits a CLI notification instead)
5. **Records** the trigger event to Arca for telemetry

**HITL default:** `auto_train: false` is the right default. The research on flywheel automation recommends human-in-the-loop for critical production systems. The flywheel should *signal* rather than *trigger* until the pipeline has been proven stable through multiple manual iterations.

### W4-02 — Hysteresis and Flap Prevention

**From research:** Training pipelines that trigger too eagerly waste compute and introduce instability. The flywheel should require:

1. A minimum sample floor (500 new traces — configurable via `FlywheelConfig`)
2. A temporal hysteresis window (minimum 24h since last training run)
3. A diversity gate (above §W3-03 threshold)

These thresholds must be externalized to `mens/config/flywheel.yaml` (a new config file) so they can be tuned without recompilation.

### W4-03 — Integration with `vox-ludus` for Flywheel Visibility

When the flywheel triggers, award an XP event (`FlywheelTrigger`) in `vox-ludus` to make the corpus improvement loop visible in the gamification system. This surfaces the health of the data pipeline to developers during normal workflow.

archived_date: 2026-04-18
---

## Implementation Dependency Graph

```
W0-01 (golden corpus extract)
  └─→ W0-02 (CI integration)
       ├─→ W2-01 (AST mutation — needs golden seeds)
       │    └─→ W3-03 (diversity check)
       └─→ W3-01 (held-out benchmark — uses golden examples)

W1-01 (heal_pairs → DPO lane)
  └─→ W2-02 (upgrade negative.rs → semantic mutations)

W1-02 (research-expert mix + research_gen diversity)
  └─→ W3-04 (frontier curator gate)

W1-03 (collateral damage gate)
  └─→ W3-01 (Vox-lang benchmark wires into this gate)
  └─→ W3-02 (semantic entropy monitor triggers gate)

W2-03 (Rust→Vox pairs) — independent; can run in parallel with W2-01

W3-02 + W3-03 (entropy + diversity monitors)
  └─→ W4-01 (flywheel state machine uses these gates)
       └─→ W4-02 (hysteresis config)
       └─→ W4-03 (ludus integration)
```

---

## Detailed Specification by File

### New Files

| File | Wave | Purpose |
|---|---|---|
| `crates/vox-corpus/src/ast_mutator.rs` | W2-01 | AST mutation engine producing diverse compiler-checked pairs |
| `crates/vox-corpus/src/rust_to_vox.rs` | W2-03 | Rust-pattern-to-Vox instruction pair generator |
| `crates/vox-corpus/src/flywheel.rs` | W4-01 | Flywheel state machine with hysteresis gates |
| `mens/config/mix-research-expert.yaml` | W1-02 | Mix config for Lane G (currently missing) |
| `mens/config/flywheel.yaml` | W4-02 | Operator-configurable flywheel thresholds |
| `mens/bench/vox-lang-bench-v1.jsonl` | W3-01 | Frozen Vox-lang held-out benchmark |

### Modified Files

| File | Wave | Change |
|---|---|---|
| `crates/vox-eval/src/lib.rs` | W3-02 | Add `SemanticEntropyReport` and `eval_semantic_entropy()` |
| `crates/vox-corpus/src/research_gen.rs` | W1-02 | Expand entity pool ×5, add causal chain types |
| `crates/vox-corpus/src/synthetic_gen/negative_pairs.rs` | W2-02 | Semantic-level mutations (type conflict, missing handler, cyclic import) |
| `mens/config/mix-vox-lang.yaml` | W1-01, W2-01, W2-03 | Add DPO lane (weight 3), mutated pairs (weight 4), Rust→Vox pairs (weight 5) |
| `mens/config/mix-research-expert.yaml` | W1-02 | Created: add research_chains + socrates_traces sources |

### CLI Commands to Add/Extend

| Command | Wave | Description |
|---|---|---|
| `vox populi corpus extract` | W0-01 | Walk golden `.vox` files → instruction pairs → `vox_corpus_extract.jsonl` |
| `vox populi corpus heal-to-dpo` | W1-01 | Convert `heal_pairs.jsonl` → DPO preference pairs |
| `vox populi corpus research-gen` | W1-02 | Run `generate_research_chains()` → `research_chains.jsonl` |
| `vox populi corpus mutate` | W2-01 | AST mutation pass on golden files → `mutated_vox.jsonl` |
| `vox populi corpus rust-to-vox` | W2-03 | Rust pattern → Vox translation pair generator |
| `vox populi corpus diversity-check` | W3-03 | AST diversity score on a mix output |
| `vox populi corpus curate-prose` | W3-04 | Frontier LLM curator gate for prose lanes |
| `vox mens eval collateral-damage` | W1-03 | Pre/post training collateral damage evaluation |
| `vox mens eval bench` | W3-01 | Run held-out Vox-lang benchmark against an adapter |

archived_date: 2026-04-18
---

## Corpus Volume Projections (Post-Implementation)

| Source | Estimated Pairs | Quality Tier |
|---|---|---|
| Golden walk (`examples/golden/`) | 500–2,000 | Tier A (compiler-verified) |
| AST mutations from golden | 3,000–8,000 | Tier A (compiler-verified) |
| Rust→Vox translations | 1,000–3,000 | Tier A (both compilers verified) |
| `heal_pairs.jsonl` DPO pairs | 500–2,000/month | Tier B (live, compiler-verified) |
| Template-expanded synthetic | 8,481 | Tier B (template-bounded) |
| Docs pairs | 234 | Tier B |
| **Total** | **~13,700–23,700** | — |

This approaches the 10,000–50,000 range required for "robust, reliable code generation in a novel syntax" per the minimum corpus research. More critically, the golden:synthetic ratio shifts from **0:97.3** to approximately **60:40** — within the 10–20% anchor floor requirement for MAD resistance.

---

## Gaps Identified in Original Research Doc

The following corrections are made to `mens-synthetic-corpus-limitations-research-2026.md`:

1. **§3.4 Anchor Floor Policy**: The research doc proposed adding `anchor_floor: 0.10` to `review-weight-policy.yaml`. This is **incorrect** — that file governs finding-truth weights, not corpus ratios. The correct enforcement surface is the **`vox populi corpus diversity-check`** command (W3-03) and the CI gate on `train_mixed_vox_lang.mix_report.json`.

2. **§2.8 "negative examples are discarded"**: The research doc said `heal_pairs.jsonl` is not used for DPO. This is true — but the research doc did **not note** that `negative.rs` already exists as a separate, surface-level mutation system. The plan must distinguish between `negative.rs`-style lexer corruptions (low value for DPO) and `heal_pairs.jsonl`-style compiler-verified failures (high value).

3. **§3.6 CURLoRA / FAPM**: These are the correct techniques, but implementation requires replacing LoRA layers in the training backend. CURLoRA has a Python implementation (`MNoorFawi/curlora` on GitHub) compatible with HuggingFace PEFT. FAPM requires post-hoc pruning of the task vector. For the MENS pipeline (which uses a Python training harness under `vox mens train` despite Rust orchestration), the HuggingFace PEFT integration is the correct insertion point. This wave is deferred to post-Wave 4 as it requires the training backend to be stable first.

4. **§3.2 Fictional Knowledge Graphs**: The research doc proposed this as a future implementation. `research_gen.rs` already implements this. The gap is: (a) the entity pool is too small, (b) there is no mix config consuming it. Both are fixed in W1-02.

archived_date: 2026-04-18
---

## Risk Mitigation Summary (Updated)

| Risk | Wave Addressing It | Mitigation |
|---|---|---|
| **Synthetic monoculture (97.3%)** | **W0** | Golden corpus extract → activate dead weight lanes |
| Template exhaustion | W2-01 | AST mutation from verified seeds |
| Hollow-program reward hacking | W3-01, W3-02 | Held-out benchmark + semantic entropy gate |
| MAD / mode collapse | W0 (anchor data), W3-03 (diversity check) | Anchor ratio + AST diversity CI gate |
| Negative examples unused | W1-01 | heal_pairs → DPO lane |
| Missing research-expert mix | W1-02 | Create `mix-research-expert.yaml` |
| No collateral damage gating | W1-03 | `vox mens eval collateral-damage` |
| Manual flywheel | W4-01-03 | Flywheel state machine with HITL default |
| Catastrophic forgetting (sequential) | Deferred | CURLoRA (post Wave 4) |

---

## Verification Plan per Wave

### Wave 0 Verification
- Run `vox populi corpus extract`
- Confirm `train_mixed_vox_lang.mix_report.json` shows `> 0` emitted lines for golden lane
- Confirm synthetic share drops below 90%

### Wave 1 Verification
- Run `vox populi corpus heal-to-dpo` — confirm `preference_pairs.jsonl` emits valid DPO triples
- Run `vox populi corpus research-gen` — confirm `research_chains.jsonl` has `> 1000` diverse chains
- Run `vox mens eval collateral-damage` — confirm it exits non-zero on a degraded adapter

### Wave 2 Verification
- Run `vox populi corpus mutate --count 2000` — confirm `> 80%` of mutations compile
- Confirm `train_mixed_vox_lang.mix_report.json` shows >3 active lanes with >0 emitted lines
- Confirm synthetic share drops below 50%

### Wave 3 Verification
- Run `vox populi corpus diversity-check` on the new mix — confirm `ast_diversity > 0.40`
- Run a training run and check that `SemanticEntropyReport` is emitted per epoch
- Run `vox mens eval bench` against baseline and a new adapter — confirm `pass@1 > 0.25`

### Wave 4 Verification
- Confirm `flywheel.yaml` is loaded and `FlywheelState` transitions are logged to Arca telemetry
- Confirm flywheel emits `FlywheelTrigger` notification after accumulating ≥500 new traces
- Confirm no training run fires automatically when `auto_train: false`

archived_date: 2026-04-18
---

*Document date: 2026-04-12. This plan supersedes the recommendations in `mens-synthetic-corpus-limitations-research-2026.md` where they conflict. The research doc should be treated as background context; this document is the execution SSOT.*


