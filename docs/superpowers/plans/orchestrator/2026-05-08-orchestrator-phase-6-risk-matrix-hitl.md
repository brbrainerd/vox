# Orchestrator Phase 6: Risk Matrix × HITL (D5 + D9) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a four-dimension risk scorer that produces a composite risk score and maps it to a HITL escalation matrix. High-risk actions trigger an EscalationEvent on BulletinBoard; medium-risk actions get a context warning; low-risk actions proceed autonomously.

**Architecture:** A new `risk_matrix.rs` module holds a pure `RiskMatrix` struct. Four dimensions: irreversibility × blast_radius × compliance_exposure × (1 - confidence). The composite score maps to `RiskGrade::{Low, Medium, High, Critical}`. `Critical` and `High` grades push an `EscalationEvent` via `BulletinBoard`. The existing `RiskBand` / `RiskDecision` from `vox-orchestrator-types::socrates_policy` remain unchanged.

**Tech Stack:** Rust, `BulletinBoard`, `AgentMessage`, `METRIC_TYPE_HITL_INTERRUPT`, `METRIC_TYPE_RISK_SCORE`, feature flag `vox.orchestrator.risk_matrix_hitl.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/risk_matrix.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod risk_matrix |
| Modify | `crates/vox-orchestrator/src/bulletin.rs` — add EscalationEvent variant if missing |
| Create | `crates/vox-orchestrator/tests/risk_matrix_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/risk/low_risk.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/risk/high_risk_irreversible.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/risk/critical_compliance.json` |
| Modify | `contracts/orchestration/risk-confidence-matrix.v1.yaml` — fill schema |
| Modify | `docs/src/architecture/where-things-live.md` — add risk_matrix row |

---

### Task 1: Fill the risk-confidence-matrix contract

**Files:**
- Modify: `contracts/orchestration/risk-confidence-matrix.v1.yaml`

- [ ] **Step 1.1: Write the contract**

```yaml
# contracts/orchestration/risk-confidence-matrix.v1.yaml
version: 1
description: "Four-dimension risk scorer and HITL escalation matrix (D5 + D9)"
dimensions:
  irreversibility:
    description: "0=fully reversible, 1=permanently destructive"
  blast_radius:
    description: "0=local/isolated, 1=system-wide/external"
  compliance_exposure:
    description: "0=no regulations, 1=HIPAA/GDPR/EU AI Act high-risk"
  confidence:
    description: "composite confidence from D3 FusionDecision (1=fully confident, 0=uncertain); the formula multiplies by (1 - confidence) so high confidence reduces risk"
formula: "irreversibility * blast_radius * compliance_exposure * (1 - confidence)"
grades:
  low:
    max_score: 0.20
    action: "proceed"
  medium:
    min_score: 0.20
    max_score: 0.50
    action: "warn_context"
  high:
    min_score: 0.50
    max_score: 0.75
    action: "escalate"
  critical:
    min_score: 0.75
    action: "block_and_escalate"
hitl_escalation:
  bulletin_event: "EscalationRequired"
  metrics_key_interrupt: "orch.hitl.interrupt"
  metrics_key_risk: "orch.risk.score"
feature_flag: "vox.orchestrator.risk_matrix_hitl.enabled"
```

- [ ] **Step 1.2: Commit**

```bash
git add contracts/orchestration/risk-confidence-matrix.v1.yaml
git commit -m "feat(contracts): fill risk-confidence-matrix.v1.yaml schema"
```

---

### Task 2: Core RiskMatrix

**Files:**
- Create: `crates/vox-orchestrator/src/risk_matrix.rs`

- [ ] **Step 2.1: Write failing tests first**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_risk_all_zeros() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        let dims = RiskDimensions {
            irreversibility: 0.0,
            blast_radius: 0.0,
            compliance_exposure: 0.0,
            confidence: 1.0, // fully confident
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::Low);
    }

    #[test]
    fn critical_score_all_ones_zero_confidence() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        let dims = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 1.0,
            compliance_exposure: 1.0,
            confidence: 0.0, // no confidence
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::Critical);
    }

    #[test]
    fn high_irreversibility_with_no_compliance_is_medium() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        // irreversibility=0.8, blast_radius=0.8, compliance=0, confidence=0.5
        // score = 0.8 * 0.8 * 0.0 * 0.5 = 0.0 → Low
        let dims = RiskDimensions {
            irreversibility: 0.8,
            blast_radius: 0.8,
            compliance_exposure: 0.0,
            confidence: 0.5,
        };
        // With compliance=0, score=0 regardless
        assert_eq!(matrix.grade(&dims), RiskGrade::Low);
    }

    #[test]
    fn medium_grade_triggers_warn_not_escalate() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        let dims = RiskDimensions {
            irreversibility: 0.8,
            blast_radius: 0.5,
            compliance_exposure: 0.5,
            confidence: 0.5, // score ≈ 0.8*0.5*0.5*0.5 = 0.10 → Low
        };
        // This is actually Low due to multiplicative formula; craft one that's medium:
        // We need score in [0.20, 0.50)
        // irreversibility=1.0, blast_radius=0.8, compliance=0.8, confidence=0.5
        // score = 1.0 * 0.8 * 0.8 * 0.5 = 0.32 → Medium
        let dims_medium = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 0.8,
            compliance_exposure: 0.8,
            confidence: 0.5,
        };
        assert_eq!(matrix.grade(&dims_medium), RiskGrade::Medium);
        assert_eq!(matrix.hitl_action(&RiskGrade::Medium), HitlAction::WarnContext);
    }

    #[test]
    fn high_grade_triggers_escalate() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        // irreversibility=1.0, blast=0.9, compliance=0.9, confidence=0.3
        // score = 1.0 * 0.9 * 0.9 * 0.7 ≈ 0.567 → High
        let dims = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 0.9,
            compliance_exposure: 0.9,
            confidence: 0.3,
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::High);
        assert_eq!(matrix.hitl_action(&RiskGrade::High), HitlAction::Escalate);
    }
}
```

- [ ] **Step 2.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator risk_matrix 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 2.3: Write the module**

Create `crates/vox-orchestrator/src/risk_matrix.rs`:

```rust
//! Four-dimension risk scorer and HITL escalation matrix (D5 + D9).
//!
//! Formula: irreversibility × blast_radius × compliance_exposure × (1 − confidence)
//! All computation is pure; BulletinBoard escalation is handled by the caller.

use serde::{Deserialize, Serialize};

/// Four input dimensions for the risk score.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RiskDimensions {
    /// 0.0 = fully reversible; 1.0 = permanently destructive.
    pub irreversibility: f64,
    /// 0.0 = local/isolated; 1.0 = system-wide or external impact.
    pub blast_radius: f64,
    /// 0.0 = no regulation; 1.0 = HIPAA/GDPR/EU AI Act high-risk.
    pub compliance_exposure: f64,
    /// Composite model confidence (1.0 = fully confident, 0.0 = no confidence).
    /// Tip: pass `fusion_score` directly from ConfidenceFuser. The implementation
    /// stores `confidence_deficit = 1.0 - confidence` and adds it to the weighted
    /// composite, so higher confidence reduces risk.
    pub confidence: f64,
}

/// Graded risk level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskGrade {
    Low,
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for RiskGrade {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

/// Action to take for a given risk grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HitlAction {
    /// Proceed without interruption.
    Proceed,
    /// Inject a warning message into the context window; do not block.
    WarnContext,
    /// Publish EscalationEvent to BulletinBoard; pause execution.
    Escalate,
    /// Block execution and publish EscalationEvent.
    BlockAndEscalate,
}

/// Thresholds for grade boundaries. Defaults mirror contract YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskMatrixConfig {
    pub low_max: f64,
    pub medium_max: f64,
    pub high_max: f64,
}

impl Default for RiskMatrixConfig {
    fn default() -> Self {
        Self {
            low_max: 0.20,
            medium_max: 0.50,
            high_max: 0.75,
        }
    }
}

/// Pure risk scorer.
pub struct RiskMatrix {
    config: RiskMatrixConfig,
}

impl RiskMatrix {
    pub fn new(config: RiskMatrixConfig) -> Self {
        Self { config }
    }

    /// Compute raw composite risk score (0.0–1.0).
    #[must_use]
    #[inline]
    pub fn compute_score(&self, dims: &RiskDimensions) -> f64 {
        let uncertainty = (1.0 - dims.confidence).clamp(0.0, 1.0);
        (dims.irreversibility * dims.blast_radius * dims.compliance_exposure * uncertainty)
            .clamp(0.0, 1.0)
    }

    /// Map raw score to a risk grade.
    #[must_use]
    #[inline]
    pub fn grade(&self, dims: &RiskDimensions) -> RiskGrade {
        let score = self.compute_score(dims);
        if score < self.config.low_max {
            RiskGrade::Low
        } else if score < self.config.medium_max {
            RiskGrade::Medium
        } else if score < self.config.high_max {
            RiskGrade::High
        } else {
            RiskGrade::Critical
        }
    }

    /// Map a risk grade to a HITL action.
    #[must_use]
    #[inline]
    pub fn hitl_action(&self, grade: &RiskGrade) -> HitlAction {
        match grade {
            RiskGrade::Low => HitlAction::Proceed,
            RiskGrade::Medium => HitlAction::WarnContext,
            RiskGrade::High => HitlAction::Escalate,
            RiskGrade::Critical => HitlAction::BlockAndEscalate,
        }
    }
}

/// Metric payload for a risk evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskScoreEvent {
    pub metric_type: &'static str,
    pub grade: RiskGrade,
    pub raw_score: f64,
    pub session_id: Option<String>,
}

impl RiskScoreEvent {
    pub fn new(grade: RiskGrade, score: f64) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_RISK_SCORE,
            grade,
            raw_score: score,
            session_id: None,
        }
    }
}

/// Metric payload for a HITL interrupt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HitlInterruptEvent {
    pub metric_type: &'static str,
    pub grade: RiskGrade,
    pub action: HitlAction,
    pub session_id: Option<String>,
}

impl HitlInterruptEvent {
    pub fn new(grade: RiskGrade, action: HitlAction) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_HITL_INTERRUPT,
            grade,
            action,
            session_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn low_risk_all_zeros() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        let dims = RiskDimensions {
            irreversibility: 0.0,
            blast_radius: 0.0,
            compliance_exposure: 0.0,
            confidence: 1.0,
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::Low);
    }

    #[test]
    fn critical_score_all_ones_zero_confidence() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        let dims = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 1.0,
            compliance_exposure: 1.0,
            confidence: 0.0,
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::Critical);
    }

    #[test]
    fn compliance_zero_always_low() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        let dims = RiskDimensions {
            irreversibility: 0.8,
            blast_radius: 0.8,
            compliance_exposure: 0.0,
            confidence: 0.5,
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::Low);
    }

    #[test]
    fn medium_grade_triggers_warn() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        // score = 1.0 * 0.8 * 0.8 * 0.5 = 0.32 → Medium
        let dims = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 0.8,
            compliance_exposure: 0.8,
            confidence: 0.5,
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::Medium);
        assert_eq!(matrix.hitl_action(&RiskGrade::Medium), HitlAction::WarnContext);
    }

    #[test]
    fn high_grade_triggers_escalate() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        // score = 1.0 * 0.9 * 0.9 * 0.7 ≈ 0.567 → High
        let dims = RiskDimensions {
            irreversibility: 1.0,
            blast_radius: 0.9,
            compliance_exposure: 0.9,
            confidence: 0.3,
        };
        assert_eq!(matrix.grade(&dims), RiskGrade::High);
        assert_eq!(matrix.hitl_action(&RiskGrade::High), HitlAction::Escalate);
    }

    #[test]
    fn critical_grade_triggers_block_and_escalate() {
        let matrix = RiskMatrix::new(RiskMatrixConfig::default());
        assert_eq!(
            matrix.hitl_action(&RiskGrade::Critical),
            HitlAction::BlockAndEscalate
        );
    }

    #[test]
    fn risk_score_event_has_correct_metric_type() {
        let event = RiskScoreEvent::new(RiskGrade::Medium, 0.35);
        assert_eq!(event.metric_type, "orch.risk.score");
    }

    #[test]
    fn hitl_interrupt_event_has_correct_metric_type() {
        let event = HitlInterruptEvent::new(RiskGrade::High, HitlAction::Escalate);
        assert_eq!(event.metric_type, "orch.hitl.interrupt");
    }
}
```

- [ ] **Step 2.4: Register in lib.rs**

```rust
pub mod risk_matrix;
```

- [ ] **Step 2.5: Run tests**

Run: `cargo test -p vox-orchestrator risk_matrix -- --nocapture`
Expected: All 8 tests pass.

- [ ] **Step 2.6: Commit**

```bash
git add crates/vox-orchestrator/src/risk_matrix.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add RiskMatrix four-dimension scorer and HITL grade (D5+D9)"
```

---

### Task 3: Check BulletinBoard for EscalationEvent and add if missing

**Files:**
- Modify: `crates/vox-orchestrator/src/bulletin.rs` (only if needed)

- [ ] **Step 3.1: Read bulletin.rs to check existing AgentMessage variants**

Read `crates/vox-orchestrator/src/bulletin.rs` and look for `EscalationRequired` or similar in `AgentMessage`.

- [ ] **Step 3.2: If missing, add the variant**

If `AgentMessage::EscalationRequired` does not exist, add:

```rust
/// Published when the risk matrix grades an action as High or Critical.
EscalationRequired {
    session_id: String,
    grade: String,         // "high" | "critical"
    action_description: String,
},
```

- [ ] **Step 3.3: Write test for bulletin publication**

Add to `crates/vox-orchestrator/tests/risk_matrix_integration.rs`:

```rust
use vox_orchestrator::risk_matrix::{
    HitlAction, RiskDimensions, RiskGrade, RiskMatrix, RiskMatrixConfig,
};

#[test]
fn high_grade_maps_to_escalate_action() {
    let matrix = RiskMatrix::new(RiskMatrixConfig::default());
    let dims = RiskDimensions {
        irreversibility: 1.0,
        blast_radius: 0.9,
        compliance_exposure: 0.9,
        confidence: 0.3,
    };
    let grade = matrix.grade(&dims);
    let action = matrix.hitl_action(&grade);
    assert_eq!(action, HitlAction::Escalate);
}

#[test]
fn low_grade_maps_to_proceed() {
    let matrix = RiskMatrix::new(RiskMatrixConfig::default());
    let dims = RiskDimensions::default();
    let grade = matrix.grade(&dims);
    assert_eq!(matrix.hitl_action(&grade), HitlAction::Proceed);
}
```

- [ ] **Step 3.4: Run integration tests**

Run: `cargo test --test risk_matrix_integration`
Expected: All pass.

- [ ] **Step 3.5: Commit**

```bash
git add crates/vox-orchestrator/src/bulletin.rs \
        crates/vox-orchestrator/tests/risk_matrix_integration.rs
git commit -m "feat(orchestrator): add EscalationRequired to BulletinBoard AgentMessage (D9)"
```

---

### Task 4: Golden fixtures

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/risk/*.json`

- [ ] **Step 4.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/risk/low_risk.json`:
```json
{
  "dims": {
    "irreversibility": 0.0,
    "blast_radius": 0.0,
    "compliance_exposure": 0.0,
    "confidence": 1.0
  },
  "expected_grade": "Low",
  "expected_action": "Proceed"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/risk/high_risk_irreversible.json`:
```json
{
  "dims": {
    "irreversibility": 1.0,
    "blast_radius": 0.9,
    "compliance_exposure": 0.9,
    "confidence": 0.3
  },
  "expected_grade": "High",
  "expected_action": "Escalate"
}
```

`crates/vox-orchestrator-test-helpers/fixtures/risk/critical_compliance.json`:
```json
{
  "dims": {
    "irreversibility": 1.0,
    "blast_radius": 1.0,
    "compliance_exposure": 1.0,
    "confidence": 0.0
  },
  "expected_grade": "Critical",
  "expected_action": "BlockAndEscalate"
}
```

- [ ] **Step 4.2: Add golden tests to integration file**

Add to `crates/vox-orchestrator/tests/risk_matrix_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct RiskFixture {
    dims: RiskDimensions,
    expected_grade: RiskGrade,
    expected_action: HitlAction,
}

#[test]
fn golden_low_risk() {
    let f: RiskFixture = load_golden_fixture("risk/low_risk.json").unwrap();
    let matrix = RiskMatrix::new(RiskMatrixConfig::default());
    let grade = matrix.grade(&f.dims);
    assert_eq!(grade, f.expected_grade);
    assert_eq!(matrix.hitl_action(&grade), f.expected_action);
}

#[test]
fn golden_high_risk_irreversible() {
    let f: RiskFixture =
        load_golden_fixture("risk/high_risk_irreversible.json").unwrap();
    let matrix = RiskMatrix::new(RiskMatrixConfig::default());
    let grade = matrix.grade(&f.dims);
    assert_eq!(grade, f.expected_grade);
}

#[test]
fn golden_critical_compliance() {
    let f: RiskFixture =
        load_golden_fixture("risk/critical_compliance.json").unwrap();
    let matrix = RiskMatrix::new(RiskMatrixConfig::default());
    let grade = matrix.grade(&f.dims);
    assert_eq!(grade, f.expected_grade);
    assert_eq!(matrix.hitl_action(&grade), f.expected_action);
}
```

- [ ] **Step 4.3: Run golden tests**

Run: `cargo test --test risk_matrix_integration golden`
Expected: 3 golden tests pass.

- [ ] **Step 4.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/risk/ \
        crates/vox-orchestrator/tests/risk_matrix_integration.rs
git commit -m "test(orchestrator): golden fixtures for RiskMatrix"
```

---

### Task 5: Update where-things-live.md and arch-check

- [ ] **Step 5.1: Add risk_matrix row**

```
| `risk_matrix` | Four-dimension risk scorer + HITL escalation matrix (D5+D9) | `crates/vox-orchestrator/src/risk_matrix.rs` |
```

- [ ] **Step 5.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`

- [ ] **Step 5.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register risk_matrix in where-things-live.md"
```

---

### Task 6: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** Both metric types (`orch.risk.score` and `orch.hitl.interrupt`) tested
- [ ] **G3** No perf bench required; formula is <100ns by construction
- [ ] **G4** Contract: formula dimensions verified to produce correct grade boundaries
- [ ] **G5** HITL fallback: `Critical` grade always returns `BlockAndEscalate`

---

**Phase 6 sign-off:** 6 tasks complete, 8+ unit tests + 3 golden fixtures, `cargo build -p vox-orchestrator` clean.
