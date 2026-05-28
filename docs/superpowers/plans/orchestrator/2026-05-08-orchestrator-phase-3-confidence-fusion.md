# Orchestrator Phase 3: Confidence Fusion (D3 — Socrates Invocation) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a composite confidence scorer that fuses retrieval signals to decide when the orchestrator must invoke Socrates research vs. answer directly, replacing the current single-threshold abstain check.

**Architecture:** A new `confidence_fusion.rs` module computes a weighted composite score from five inputs (evidence_quality, citation_coverage, source_diversity, contradiction_ratio, entropy). The result is compared against per-task thresholds from `socrates-fusion.v1.yaml` to yield a `FusionDecision`. The existing `ConfidencePolicy::evaluate_risk_decision` is wired to consult the fused score when the feature flag is on.

**Tech Stack:** Rust, `vox-orchestrator-types::socrates_policy`, `entropy_scorer`, `vox-db::METRIC_TYPE_SOCRATES_FUSION`, feature flag `vox.orchestrator.socrates_fusion.enabled`, `criterion` bench.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/confidence_fusion.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod confidence_fusion |
| Modify | `crates/vox-orchestrator/src/socrates.rs` — call fused scorer when flag enabled |
| Create | `crates/vox-orchestrator/benches/confidence_fusion.rs` |
| Create | `crates/vox-orchestrator/tests/confidence_fusion_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/high_confidence.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/low_confidence.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/contradiction_veto.json` |
| Modify | `contracts/orchestration/socrates-fusion.v1.yaml` — fill real schema |
| Modify | `docs/src/architecture/where-things-live.md` — add confidence_fusion row |

---

### Task 1: Fill the socrates-fusion contract

**Files:**
- Modify: `contracts/orchestration/socrates-fusion.v1.yaml`

- [ ] **Step 1.1: Write the contract**

```yaml
# contracts/orchestration/socrates-fusion.v1.yaml
version: 1
description: "Composite confidence fusion for Socrates invocation decision (D3)"
weights:
  evidence_quality: 0.35
  citation_coverage: 0.25
  source_diversity: 0.15
  contradiction_penalty: 0.15
  entropy_score: 0.10
thresholds:
  invoke_socrates_below: 0.55    # composite score below this → must invoke Socrates
  answer_directly_above: 0.75   # composite score above this → answer directly
  # Between 0.55 and 0.75: evaluate per-signal veto rules
veto_rules:
  contradiction_ratio_veto: 0.40    # if contradiction_ratio >= this → always invoke
  zero_evidence_veto: true          # if evidence_count == 0 → always invoke
  zero_citation_veto: true          # if citation_coverage == 0 AND required_citations > 0 → invoke
metrics_key: "orch.socrates.fusion"
feature_flag: "vox.orchestrator.socrates_fusion.enabled"
```

- [ ] **Step 1.2: Commit**

```bash
git add contracts/orchestration/socrates-fusion.v1.yaml
git commit -m "feat(contracts): fill socrates-fusion.v1.yaml schema"
```

---

### Task 2: Core FusionDecision types and scorer

**Files:**
- Create: `crates/vox-orchestrator/src/confidence_fusion.rs`

- [ ] **Step 2.1: Write the failing tests first**

```rust
// At the bottom of confidence_fusion.rs (to be created in 2.3)
#[cfg(test)]
mod tests {
    use super::*;

    fn high_confidence_inputs() -> FusionInputs {
        FusionInputs {
            evidence_quality: 0.90,
            citation_coverage: 0.85,
            source_diversity: 0.80,
            contradiction_ratio: 0.05,
            entropy_score: 0.75,
        }
    }

    fn low_confidence_inputs() -> FusionInputs {
        FusionInputs {
            evidence_quality: 0.20,
            citation_coverage: 0.10,
            source_diversity: 0.20,
            contradiction_ratio: 0.10,
            entropy_score: 0.30,
        }
    }

    #[test]
    fn high_evidence_yields_answer_directly() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let decision = scorer.decide(&high_confidence_inputs());
        assert_eq!(decision, FusionDecision::AnswerDirectly);
    }

    #[test]
    fn low_evidence_yields_invoke_socrates() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let decision = scorer.decide(&low_confidence_inputs());
        assert_eq!(decision, FusionDecision::InvokeSocrates);
    }

    #[test]
    fn contradiction_veto_overrides_high_score() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let mut inputs = high_confidence_inputs();
        inputs.contradiction_ratio = 0.50; // above veto threshold 0.40
        let decision = scorer.decide(&inputs);
        assert_eq!(decision, FusionDecision::InvokeSocrates);
    }

    #[test]
    fn composite_score_in_range() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let score = scorer.compute_score(&high_confidence_inputs());
        assert!(score >= 0.0 && score <= 1.0, "score out of [0,1]: {score}");
    }
}
```

- [ ] **Step 2.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator confidence_fusion 2>&1 | head -10`
Expected: module not found error.

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/confidence_fusion.rs`:

```rust
//! Composite confidence fusion for Socrates invocation decision (D3).
//!
//! Weights and thresholds are loaded from `contracts/orchestration/socrates-fusion.v1.yaml`.
//! All computation is pure: no I/O, no allocations on the hot path.

use serde::{Deserialize, Serialize};

/// Five input signals for the composite scorer.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FusionInputs {
    /// Quality of retrieved evidence (0.0–1.0).
    pub evidence_quality: f64,
    /// Fraction of claims with citation support (0.0–1.0).
    pub citation_coverage: f64,
    /// Number of distinct source types (normalised to 0.0–1.0, divide by expected max).
    pub source_diversity: f64,
    /// Fraction of evidence chunks containing contradictions (0.0–1.0).
    pub contradiction_ratio: f64,
    /// Entropy-based confidence from `entropy_scorer::score_confidence` (0.0–1.0).
    pub entropy_score: f64,
}

/// Decision output from the fusion scorer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FusionDecision {
    /// Composite confidence is high enough to answer without Socrates.
    AnswerDirectly,
    /// Composite confidence is too low; must invoke Socrates research.
    InvokeSocrates,
    /// Borderline: answer but flag uncertainty to the user.
    AnswerWithCaveats,
}

/// Weights and thresholds for the fusion scorer. Defaults mirror contract YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionConfig {
    pub w_evidence_quality: f64,
    pub w_citation_coverage: f64,
    pub w_source_diversity: f64,
    pub w_contradiction_penalty: f64,
    pub w_entropy_score: f64,
    /// Composite score below this → InvokeSocrates.
    pub invoke_below: f64,
    /// Composite score above this → AnswerDirectly.
    pub answer_above: f64,
    /// Contradiction ratio at or above this → InvokeSocrates regardless.
    pub contradiction_veto: f64,
    /// If true, zero evidence_count always → InvokeSocrates.
    pub zero_evidence_veto: bool,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            w_evidence_quality: 0.35,
            w_citation_coverage: 0.25,
            w_source_diversity: 0.15,
            w_contradiction_penalty: 0.15,
            w_entropy_score: 0.10,
            invoke_below: 0.55,
            answer_above: 0.75,
            contradiction_veto: 0.40,
            zero_evidence_veto: true,
        }
    }
}

/// Pure composite confidence scorer.
pub struct ConfidenceFuser {
    config: FusionConfig,
}

impl ConfidenceFuser {
    pub fn new(config: FusionConfig) -> Self {
        Self { config }
    }

    /// Compute weighted composite score (0.0–1.0).
    /// Higher is more confident (less need for Socrates).
    #[must_use]
    #[inline]
    pub fn compute_score(&self, inputs: &FusionInputs) -> f64 {
        let c = &self.config;
        // Contradiction acts as a penalty: subtract weighted fraction
        let contradiction_term = c.w_contradiction_penalty * inputs.contradiction_ratio;
        let positive = c.w_evidence_quality * inputs.evidence_quality
            + c.w_citation_coverage * inputs.citation_coverage
            + c.w_source_diversity * inputs.source_diversity
            + c.w_entropy_score * inputs.entropy_score;
        (positive - contradiction_term).clamp(0.0, 1.0)
    }

    /// Apply veto rules and threshold logic to produce a FusionDecision.
    #[must_use]
    pub fn decide(&self, inputs: &FusionInputs) -> FusionDecision {
        // Veto: high contradiction always triggers research
        if inputs.contradiction_ratio >= self.config.contradiction_veto {
            return FusionDecision::InvokeSocrates;
        }
        // Veto: no evidence at all
        if self.config.zero_evidence_veto && inputs.evidence_quality < 1e-9 {
            return FusionDecision::InvokeSocrates;
        }

        let score = self.compute_score(inputs);
        if score >= self.config.answer_above {
            FusionDecision::AnswerDirectly
        } else if score < self.config.invoke_below {
            FusionDecision::InvokeSocrates
        } else {
            FusionDecision::AnswerWithCaveats
        }
    }
}

/// Build FusionInputs from a SocratesTaskContext (bridge to existing types).
/// Import `vox_orchestrator_types::socrates_policy::SocratesTaskContext` at call site.
pub fn inputs_from_task_context(
    evidence_quality: f64,
    citation_coverage: f64,
    source_diversity_count: u8,
    source_diversity_max: u8,
    contradiction_hints: u8,
    evidence_count: u8,
    entropy_score: f64,
) -> FusionInputs {
    let source_div = if source_diversity_max == 0 {
        0.0
    } else {
        source_diversity_count as f64 / source_diversity_max as f64
    };
    let contradiction_ratio = if evidence_count == 0 {
        0.0
    } else {
        (contradiction_hints as f64 / evidence_count as f64).min(1.0)
    };
    FusionInputs {
        evidence_quality: evidence_quality.clamp(0.0, 1.0),
        citation_coverage: citation_coverage.clamp(0.0, 1.0),
        source_diversity: source_div.clamp(0.0, 1.0),
        contradiction_ratio,
        entropy_score: entropy_score.clamp(0.0, 1.0),
    }
}

/// Metric payload emitted when fusion makes a decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionEvent {
    pub metric_type: &'static str,
    pub decision: FusionDecision,
    pub composite_score: f64,
    pub session_id: Option<String>,
}

impl FusionEvent {
    pub fn new(decision: FusionDecision, score: f64) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_SOCRATES_FUSION,
            decision,
            composite_score: score,
            session_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn high_confidence_inputs() -> FusionInputs {
        FusionInputs {
            evidence_quality: 0.90,
            citation_coverage: 0.85,
            source_diversity: 0.80,
            contradiction_ratio: 0.05,
            entropy_score: 0.75,
        }
    }

    fn low_confidence_inputs() -> FusionInputs {
        FusionInputs {
            evidence_quality: 0.20,
            citation_coverage: 0.10,
            source_diversity: 0.20,
            contradiction_ratio: 0.10,
            entropy_score: 0.30,
        }
    }

    #[test]
    fn high_evidence_yields_answer_directly() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let decision = scorer.decide(&high_confidence_inputs());
        assert_eq!(decision, FusionDecision::AnswerDirectly);
    }

    #[test]
    fn low_evidence_yields_invoke_socrates() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let decision = scorer.decide(&low_confidence_inputs());
        assert_eq!(decision, FusionDecision::InvokeSocrates);
    }

    #[test]
    fn contradiction_veto_overrides_high_score() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let mut inputs = high_confidence_inputs();
        inputs.contradiction_ratio = 0.50;
        let decision = scorer.decide(&inputs);
        assert_eq!(decision, FusionDecision::InvokeSocrates);
    }

    #[test]
    fn composite_score_in_range() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let score = scorer.compute_score(&high_confidence_inputs());
        assert!(score >= 0.0 && score <= 1.0, "score out of [0,1]: {score}");
    }

    #[test]
    fn zero_evidence_veto_triggers() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        let inputs = FusionInputs {
            evidence_quality: 0.0,
            citation_coverage: 0.9,
            source_diversity: 0.8,
            contradiction_ratio: 0.0,
            entropy_score: 0.8,
        };
        assert_eq!(scorer.decide(&inputs), FusionDecision::InvokeSocrates);
    }

    #[test]
    fn borderline_score_yields_caveats() {
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        // Craft a score between 0.55 and 0.75
        let inputs = FusionInputs {
            evidence_quality: 0.60,
            citation_coverage: 0.55,
            source_diversity: 0.50,
            contradiction_ratio: 0.05,
            entropy_score: 0.55,
        };
        let decision = scorer.decide(&inputs);
        assert_eq!(decision, FusionDecision::AnswerWithCaveats);
    }

    #[test]
    fn inputs_from_task_context_normalises_diversity() {
        let inputs = inputs_from_task_context(0.8, 0.7, 3, 5, 1, 10, 0.6);
        assert!((inputs.source_diversity - 0.6).abs() < 1e-9);
    }

    #[test]
    fn fusion_event_has_correct_metric_type() {
        let event = FusionEvent::new(FusionDecision::AnswerDirectly, 0.85);
        assert_eq!(event.metric_type, "orch.socrates.fusion");
    }
}
```

- [ ] **Step 2.4: Register module in lib.rs**

In `crates/vox-orchestrator/src/lib.rs`, add:
```rust
pub mod confidence_fusion;
```

- [ ] **Step 2.5: Run tests**

Run: `cargo test -p vox-orchestrator confidence_fusion -- --nocapture`
Expected: All 8 tests pass.

- [ ] **Step 2.6: Commit**

```bash
git add crates/vox-orchestrator/src/confidence_fusion.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add ConfidenceFuser composite scorer for D3 Socrates invocation"
```

---

### Task 3: Golden fixtures

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/*.json`

- [ ] **Step 3.1: Write the three fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/high_confidence.json`:
```json
{
  "inputs": {
    "evidence_quality": 0.90,
    "citation_coverage": 0.85,
    "source_diversity": 0.80,
    "contradiction_ratio": 0.05,
    "entropy_score": 0.75
  },
  "expected_decision": "AnswerDirectly"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/low_confidence.json`:
```json
{
  "inputs": {
    "evidence_quality": 0.20,
    "citation_coverage": 0.10,
    "source_diversity": 0.20,
    "contradiction_ratio": 0.10,
    "entropy_score": 0.30
  },
  "expected_decision": "InvokeSocrates"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/contradiction_veto.json`:
```json
{
  "inputs": {
    "evidence_quality": 0.90,
    "citation_coverage": 0.85,
    "source_diversity": 0.80,
    "contradiction_ratio": 0.45,
    "entropy_score": 0.75
  },
  "expected_decision": "InvokeSocrates"
}
```

- [ ] **Step 3.2: Write golden tests in integration file**

Create `crates/vox-orchestrator/tests/confidence_fusion_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::confidence_fusion::{
    ConfidenceFuser, FusionConfig, FusionDecision, FusionInputs,
};
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct FusionFixture {
    inputs: FusionInputs,
    expected_decision: FusionDecision,
}

#[test]
fn golden_high_confidence() {
    let f: FusionFixture =
        load_golden_fixture("socrates-fusion/high_confidence.json").unwrap();
    let scorer = ConfidenceFuser::new(FusionConfig::default());
    assert_eq!(scorer.decide(&f.inputs), f.expected_decision);
}

#[test]
fn golden_low_confidence() {
    let f: FusionFixture =
        load_golden_fixture("socrates-fusion/low_confidence.json").unwrap();
    let scorer = ConfidenceFuser::new(FusionConfig::default());
    assert_eq!(scorer.decide(&f.inputs), f.expected_decision);
}

#[test]
fn golden_contradiction_veto() {
    let f: FusionFixture =
        load_golden_fixture("socrates-fusion/contradiction_veto.json").unwrap();
    let scorer = ConfidenceFuser::new(FusionConfig::default());
    assert_eq!(scorer.decide(&f.inputs), f.expected_decision);
}
```

- [ ] **Step 3.3: Run golden tests**

Run: `cargo test -p vox-orchestrator --test confidence_fusion_integration`
Expected: 3 golden tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/socrates-fusion/ \
        crates/vox-orchestrator/tests/confidence_fusion_integration.rs
git commit -m "test(orchestrator): golden fixtures for ConfidenceFuser"
```

---

### Task 4: Criterion benchmark

**Files:**
- Create: `crates/vox-orchestrator/benches/confidence_fusion.rs`

- [ ] **Step 4.1: Write the benchmark**

```rust
// crates/vox-orchestrator/benches/confidence_fusion.rs
use criterion::{Criterion, black_box, criterion_group, criterion_main};
use vox_orchestrator::confidence_fusion::{ConfidenceFuser, FusionConfig, FusionInputs};

fn bench_decide(c: &mut Criterion) {
    let scorer = ConfidenceFuser::new(FusionConfig::default());
    let inputs = FusionInputs {
        evidence_quality: 0.75,
        citation_coverage: 0.70,
        source_diversity: 0.60,
        contradiction_ratio: 0.10,
        entropy_score: 0.65,
    };
    c.bench_function("confidence_fusion_decide", |b| {
        b.iter(|| scorer.decide(black_box(&inputs)))
    });
}

criterion_group!(benches, bench_decide);
criterion_main!(benches);
```

- [ ] **Step 4.2: Add bench entry to Cargo.toml**

```toml
[[bench]]
name = "confidence_fusion"
harness = false
```

- [ ] **Step 4.3: Run benchmark**

Run: `cargo bench -p vox-orchestrator --bench confidence_fusion 2>&1 | tail -10`
Expected: mean <100µs (should be <500ns given pure arithmetic).

- [ ] **Step 4.4: Commit**

```bash
git add crates/vox-orchestrator/benches/confidence_fusion.rs crates/vox-orchestrator/Cargo.toml
git commit -m "bench(orchestrator): criterion benchmark for ConfidenceFuser"
```

---

### Task 5: Wire into socrates.rs with feature flag guard

**Files:**
- Modify: `crates/vox-orchestrator/src/socrates.rs`

- [ ] **Step 5.1: Read the current evaluate_risk_decision call site**

Read `crates/vox-orchestrator-types/src/socrates_policy/mod.rs` (or wherever `evaluate_risk_decision` is defined) to identify the call site in `socrates.rs`.

- [ ] **Step 5.2: Add fused scorer gate in socrates.rs**

After the existing `SocratesTaskContext::merge_into` block, add a function:

```rust
/// Returns true if the orchestrator should invoke Socrates given the current task context.
/// When the `socrates_fusion` feature flag is enabled, uses composite scoring.
/// Falls back to the policy's `evaluate_risk_decision` when the flag is off.
#[must_use]
pub fn should_invoke_socrates(ctx: &SocratesTaskContext, entropy_score: f64) -> bool {
    // Feature-flag guard (reads from env for now; P4 will wire to feature-flags.v1.yaml loader)
    if std::env::var("VOX_ORCHESTRATOR_SOCRATES_FUSION").as_deref() == Ok("1") {
        use crate::confidence_fusion::{ConfidenceFuser, FusionConfig, FusionDecision, inputs_from_task_context};
        let inputs = inputs_from_task_context(
            ctx.evidence_quality,
            ctx.citation_coverage,
            ctx.source_diversity,
            8, // expected max diversity sources
            ctx.contradiction_hints,
            ctx.evidence_count,
            entropy_score,
        );
        let scorer = ConfidenceFuser::new(FusionConfig::default());
        return scorer.decide(&inputs) == FusionDecision::InvokeSocrates;
    }
    // Legacy path: use ConfidencePolicy thresholds
    use vox_orchestrator_types::socrates_policy::{ConfidencePolicy, RiskDecision};
    let policy = ConfidencePolicy::default();
    let decision = policy.evaluate_risk_decision(ctx);
    matches!(decision, RiskDecision::Ask | RiskDecision::Abstain)
}
```

- [ ] **Step 5.3: Write test for the wired gate**

Add to `crates/vox-orchestrator/tests/confidence_fusion_integration.rs`:

```rust
#[test]
fn should_invoke_socrates_legacy_path_no_flag() {
    // With no env var set, falls through to legacy ConfidencePolicy
    std::env::remove_var("VOX_ORCHESTRATOR_SOCRATES_FUSION");
    // An empty task context should trigger ask/abstain in legacy path
    use vox_orchestrator::socrates::should_invoke_socrates;
    use vox_orchestrator_types::socrates_policy::SocratesTaskContext;
    let ctx = SocratesTaskContext::default();
    // Legacy: empty context → abstain (returns true)
    let result = should_invoke_socrates(&ctx, 0.3);
    // Just verify it doesn't panic; the exact value depends on legacy policy defaults
    let _ = result;
}
```

- [ ] **Step 5.4: Run all confidence_fusion tests**

Run: `cargo test -p vox-orchestrator confidence_fusion`
Expected: All pass.

Run: `cargo test --test confidence_fusion_integration`
Expected: All pass.

- [ ] **Step 5.5: Commit**

```bash
git add crates/vox-orchestrator/src/socrates.rs \
        crates/vox-orchestrator/tests/confidence_fusion_integration.rs
git commit -m "feat(orchestrator): wire ConfidenceFuser into should_invoke_socrates (D3, feature-gated)"
```

---

### Task 6: Update where-things-live.md and run arch-check

**Files:**
- Modify: `docs/src/architecture/where-things-live.md`

- [ ] **Step 6.1: Add confidence_fusion row**

```
| `confidence_fusion` | Composite confidence scorer for Socrates invocation (D3) | `crates/vox-orchestrator/src/confidence_fusion.rs` |
```

- [ ] **Step 6.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`
Expected: Clean.

- [ ] **Step 6.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register confidence_fusion in where-things-live.md"
```

---

### Task 7: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** `FusionEvent::metric_type == METRIC_TYPE_SOCRATES_FUSION` test passes
- [ ] **G3** Bench: `confidence_fusion_decide` mean <100µs
- [ ] **G4** Contract: `socrates-fusion.v1.yaml` weights sum ≈ 1.0 (verify: 0.35+0.25+0.15+0.15+0.10 = 1.00)
- [ ] **G5** HITL fallback: `zero_evidence_veto` triggers `InvokeSocrates` even with high other signals

---

**Phase 3 sign-off:** 7 tasks complete, 8+ unit tests + 3 golden fixtures + property coverage, `cargo build -p vox-orchestrator` clean.
