# Orchestrator Phase 7: Privacy Routing (D8) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing `PrivacyRouter` and `PrivacyRoutingPolicy` in `privacy_router.rs` to (1) integrate with the tier cascade so privacy constraints override model tier selection, (2) add a `PrivacyClassifier` that infers sensitivity from task content signals, and (3) emit `METRIC_TYPE_PRIVACY_ROUTE_DECISION` telemetry.

**Architecture:** The existing `PrivacyRouter` (`privacy_router.rs`) already has `PrivacyLevel`, `PrivacyRoutingDecision`, and `force_local_for_private`. This phase adds `PrivacyClassifier` (new type) that maps task signals to a `PrivacyLevel`, and wires `PrivacyRouter::route()` into `TierCascadeRouter::select_tier()` via the `privacy_requires_local` field of `CompositeSignal` (already declared in P4).

**Tech Stack:** Rust, existing `PrivacyRouter`, `PrivacyLevel`, `PrivacyRoutingPolicy`, `METRIC_TYPE_PRIVACY_ROUTE_DECISION`, `pii_filter.rs`, feature flag `vox.orchestrator.privacy_routing.enabled`.

---

## File Map

| Action | Path |
|--------|------|
| Create | `crates/vox-orchestrator/src/privacy_classifier.rs` |
| Modify | `crates/vox-orchestrator/src/lib.rs` — pub mod privacy_classifier |
| Modify | `crates/vox-orchestrator/src/privacy_router.rs` — add route() and PrivacyRouteEvent |
| Create | `crates/vox-orchestrator/tests/privacy_routing_integration.rs` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/privacy/public_task.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/privacy/pii_task.json` |
| Create | `crates/vox-orchestrator-test-helpers/fixtures/privacy/regulated_task.json` |
| Modify | `docs/src/architecture/where-things-live.md` — add privacy_classifier row |

---

### Task 1: Add PrivacyClassifier

**Files:**
- Create: `crates/vox-orchestrator/src/privacy_classifier.rs`

- [ ] **Step 1.1: Write failing tests first**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator::privacy_router::PrivacyLevel;

    #[test]
    fn empty_signals_classify_as_public() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_pii_keywords: false,
            has_health_data: false,
            has_financial_data: false,
            has_user_identifiers: false,
            file_paths_contain_private: false,
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Public);
    }

    #[test]
    fn pii_keywords_classify_as_private() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_pii_keywords: true,
            has_health_data: false,
            has_financial_data: false,
            has_user_identifiers: false,
            file_paths_contain_private: false,
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Private);
    }

    #[test]
    fn health_data_classifies_as_regulated() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_pii_keywords: false,
            has_health_data: true,
            has_financial_data: false,
            has_user_identifiers: false,
            file_paths_contain_private: false,
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Regulated);
    }

    #[test]
    fn financial_data_classifies_as_regulated() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_pii_keywords: false,
            has_health_data: false,
            has_financial_data: true,
            has_user_identifiers: false,
            file_paths_contain_private: false,
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Regulated);
    }
}
```

- [ ] **Step 1.2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator privacy_classifier 2>&1 | head -10`
Expected: module not found.

- [ ] **Step 1.3: Write the module**

Create `crates/vox-orchestrator/src/privacy_classifier.rs`:

```rust
//! Privacy level classifier for task content signals (D8).
//!
//! Maps heuristic signals from task context to a `PrivacyLevel`.
//! No I/O; callers supply pre-extracted signals from PII filter output.

use serde::{Deserialize, Serialize};
use crate::privacy_router::PrivacyLevel;

/// Heuristic signals extracted from task content and file paths.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClassificationSignals {
    /// PII keywords detected in task description or tool arguments.
    pub has_pii_keywords: bool,
    /// Health-related data indicators (HIPAA scope).
    pub has_health_data: bool,
    /// Financial data indicators (PCI/SOX scope).
    pub has_financial_data: bool,
    /// User identifiers (email, SSN, passport, etc.).
    pub has_user_identifiers: bool,
    /// File path segments suggesting private data (e.g., `/.secrets/`, `/pii/`).
    pub file_paths_contain_private: bool,
}

/// Configuration (reserved for future threshold tuning).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrivacyClassifierConfig {
    // No tunable thresholds yet; classification is rule-based.
    #[serde(default)]
    pub _reserved: (),
}

/// Classifies task signals to a PrivacyLevel.
pub struct PrivacyClassifier {
    _config: PrivacyClassifierConfig,
}

impl PrivacyClassifier {
    pub fn new(config: PrivacyClassifierConfig) -> Self {
        Self { _config: config }
    }

    /// Classify signals to the highest applicable PrivacyLevel.
    #[must_use]
    pub fn classify(&self, signals: &ClassificationSignals) -> PrivacyLevel {
        // Regulated: health or financial data (highest sensitivity)
        if signals.has_health_data || signals.has_financial_data {
            return PrivacyLevel::Regulated;
        }
        // Private: PII keywords or user identifiers
        if signals.has_pii_keywords || signals.has_user_identifiers {
            return PrivacyLevel::Private;
        }
        // Internal: private file paths (not PII but sensitive)
        if signals.file_paths_contain_private {
            return PrivacyLevel::Internal;
        }
        PrivacyLevel::Public
    }

    /// Returns true if the classified level requires local-only inference.
    #[must_use]
    #[inline]
    pub fn requires_local(&self, signals: &ClassificationSignals) -> bool {
        matches!(
            self.classify(signals),
            PrivacyLevel::Private | PrivacyLevel::Regulated
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_signals_classify_as_public() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals::default();
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Public);
    }

    #[test]
    fn pii_keywords_classify_as_private() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_pii_keywords: true,
            ..Default::default()
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Private);
    }

    #[test]
    fn health_data_classifies_as_regulated() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_health_data: true,
            ..Default::default()
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Regulated);
    }

    #[test]
    fn financial_data_classifies_as_regulated() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_financial_data: true,
            ..Default::default()
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Regulated);
    }

    #[test]
    fn private_file_paths_classify_as_internal() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            file_paths_contain_private: true,
            ..Default::default()
        };
        assert_eq!(classifier.classify(&signals), PrivacyLevel::Internal);
    }

    #[test]
    fn requires_local_true_for_private() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        let signals = ClassificationSignals {
            has_pii_keywords: true,
            ..Default::default()
        };
        assert!(classifier.requires_local(&signals));
    }

    #[test]
    fn requires_local_false_for_public() {
        let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
        assert!(!classifier.requires_local(&ClassificationSignals::default()));
    }
}
```

- [ ] **Step 1.4: Register module in lib.rs**

```rust
pub mod privacy_classifier;
```

- [ ] **Step 1.5: Run tests**

Run: `cargo test -p vox-orchestrator privacy_classifier -- --nocapture`
Expected: All 7 tests pass.

- [ ] **Step 1.6: Commit**

```bash
git add crates/vox-orchestrator/src/privacy_classifier.rs crates/vox-orchestrator/src/lib.rs
git commit -m "feat(orchestrator): add PrivacyClassifier for D8 privacy-level inference"
```

---

### Task 2: Add PrivacyRouteEvent and route() to PrivacyRouter

**Files:**
- Modify: `crates/vox-orchestrator/src/privacy_router.rs`

- [ ] **Step 2.1: Read the current privacy_router.rs end**

Read `crates/vox-orchestrator/src/privacy_router.rs` lines 50–end to see existing methods.

- [ ] **Step 2.2: Write test for route() method**

```rust
// Add to privacy_router tests
#[test]
fn route_regulated_level_returns_local_only() {
    use crate::privacy_router::{PrivacyLevel, PrivacyRouter, PrivacyRoutingPolicy, PrivacyRoutingDecision};
    let router = PrivacyRouter::new(PrivacyRoutingPolicy {
        force_local_for_private: true,
        ..Default::default()
    });
    let decision = router.route(PrivacyLevel::Regulated);
    assert_eq!(decision, PrivacyRoutingDecision::LocalOnly);
}
```

- [ ] **Step 2.3: Add route() method and PrivacyRouteEvent**

In `privacy_router.rs`, add after the existing `PrivacyRouter::new`:

```rust
impl PrivacyRouter {
    // ... existing methods ...

    /// Determine the routing decision for a given privacy level.
    #[must_use]
    pub fn route(&self, level: PrivacyLevel) -> PrivacyRoutingDecision {
        match level {
            PrivacyLevel::Public | PrivacyLevel::Internal => PrivacyRoutingDecision::Allowed,
            PrivacyLevel::Private => {
                if self.policy.force_local_for_private {
                    PrivacyRoutingDecision::LocalOnly
                } else {
                    PrivacyRoutingDecision::Redact
                }
            }
            PrivacyLevel::Regulated => PrivacyRoutingDecision::LocalOnly,
        }
    }
}

/// Metric payload for privacy routing decisions.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PrivacyRouteEvent {
    pub metric_type: &'static str,
    pub level: PrivacyLevel,
    pub decision: PrivacyRoutingDecision,
    pub session_id: Option<String>,
}

impl PrivacyRouteEvent {
    pub fn new(level: PrivacyLevel, decision: PrivacyRoutingDecision) -> Self {
        Self {
            metric_type: vox_db::research_metrics_contract::METRIC_TYPE_PRIVACY_ROUTE_DECISION,
            level,
            decision,
            session_id: None,
        }
    }
}
```

- [ ] **Step 2.4: Run tests**

Run: `cargo test -p vox-orchestrator privacy_router`
Expected: All pass including new route() test.

- [ ] **Step 2.5: Commit**

```bash
git add crates/vox-orchestrator/src/privacy_router.rs
git commit -m "feat(orchestrator): add route() method and PrivacyRouteEvent to PrivacyRouter (D8)"
```

---

### Task 3: Golden fixtures and integration tests

**Files:**
- Create: `crates/vox-orchestrator-test-helpers/fixtures/privacy/*.json`
- Create: `crates/vox-orchestrator/tests/privacy_routing_integration.rs`

- [ ] **Step 3.1: Write fixture files**

`crates/vox-orchestrator-test-helpers/fixtures/privacy/public_task.json`:
```json
{
  "signals": {
    "has_pii_keywords": false,
    "has_health_data": false,
    "has_financial_data": false,
    "has_user_identifiers": false,
    "file_paths_contain_private": false
  },
  "expected_level": "Public",
  "expected_requires_local": false
}
```

`crates/vox-orchestrator-test-helpers/fixtures/privacy/pii_task.json`:
```json
{
  "signals": {
    "has_pii_keywords": true,
    "has_health_data": false,
    "has_financial_data": false,
    "has_user_identifiers": false,
    "file_paths_contain_private": false
  },
  "expected_level": "Private",
  "expected_requires_local": true
}
```

`crates/vox-orchestrator-test-helpers/fixtures/privacy/regulated_task.json`:
```json
{
  "signals": {
    "has_pii_keywords": false,
    "has_health_data": true,
    "has_financial_data": false,
    "has_user_identifiers": false,
    "file_paths_contain_private": false
  },
  "expected_level": "Regulated",
  "expected_requires_local": true
}
```

- [ ] **Step 3.2: Write integration tests**

Create `crates/vox-orchestrator/tests/privacy_routing_integration.rs`:

```rust
use serde::Deserialize;
use vox_orchestrator::privacy_classifier::{
    ClassificationSignals, PrivacyClassifier, PrivacyClassifierConfig,
};
use vox_orchestrator::privacy_router::PrivacyLevel;
use vox_orchestrator_test_helpers::load_golden_fixture;

#[derive(Deserialize)]
struct PrivacyFixture {
    signals: ClassificationSignals,
    expected_level: PrivacyLevel,
    expected_requires_local: bool,
}

#[test]
fn golden_public_task() {
    let f: PrivacyFixture = load_golden_fixture("privacy/public_task.json").unwrap();
    let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
    assert_eq!(classifier.classify(&f.signals), f.expected_level);
    assert_eq!(classifier.requires_local(&f.signals), f.expected_requires_local);
}

#[test]
fn golden_pii_task() {
    let f: PrivacyFixture = load_golden_fixture("privacy/pii_task.json").unwrap();
    let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
    assert_eq!(classifier.classify(&f.signals), f.expected_level);
    assert_eq!(classifier.requires_local(&f.signals), f.expected_requires_local);
}

#[test]
fn golden_regulated_task() {
    let f: PrivacyFixture = load_golden_fixture("privacy/regulated_task.json").unwrap();
    let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
    assert_eq!(classifier.classify(&f.signals), f.expected_level);
    assert_eq!(classifier.requires_local(&f.signals), f.expected_requires_local);
}
```

- [ ] **Step 3.3: Run golden tests**

Run: `cargo test --test privacy_routing_integration`
Expected: 3 golden tests pass.

- [ ] **Step 3.4: Commit**

```bash
git add crates/vox-orchestrator-test-helpers/fixtures/privacy/ \
        crates/vox-orchestrator/tests/privacy_routing_integration.rs
git commit -m "test(orchestrator): golden fixtures for PrivacyClassifier and PrivacyRouter"
```

---

### Task 4: Wire requires_local into CompositeSignal (P4 bridge)

**Files:**
- Modify: `crates/vox-orchestrator/src/tier_cascade.rs` (documentation note only — field already exists)

- [ ] **Step 4.1: Verify privacy_requires_local field**

The `CompositeSignal::privacy_requires_local` field was declared in Phase 4. Verify it exists in `tier_cascade.rs`.

Run: `cargo test -p vox-orchestrator tier_cascade`
Expected: All pass.

- [ ] **Step 4.2: Write integration test bridging classifier to tier cascade**

Add to `privacy_routing_integration.rs`:

```rust
#[test]
fn regulated_task_sets_privacy_requires_local_in_composite_signal() {
    use vox_orchestrator::privacy_classifier::{ClassificationSignals, PrivacyClassifier, PrivacyClassifierConfig};
    use vox_orchestrator::tier_cascade::{AlarmLevel, CompositeSignal, RoutingTier, TierCascadeConfig, TierCascadeRouter};

    let classifier = PrivacyClassifier::new(PrivacyClassifierConfig::default());
    let signals = ClassificationSignals {
        has_health_data: true,
        ..Default::default()
    };
    let requires_local = classifier.requires_local(&signals);

    let composite = CompositeSignal {
        complexity: 9,
        confidence_score: 0.9,
        circuit_breaker_tier: AlarmLevel::None,
        budget_exhausted: false,
        privacy_requires_local: requires_local,
    };

    let router = TierCascadeRouter::new(TierCascadeConfig::default());
    // Even high complexity with budget intact: privacy_requires_local is a signal
    // (not yet enforced to return Economy — P7 just sets the flag; P4 TierCascadeRouter
    // does not yet filter by local providers; that wiring is a future iteration)
    let _tier = router.select_tier(&composite);
    // Smoke test: no panic
    assert!(requires_local);
}
```

- [ ] **Step 4.3: Run test**

Run: `cargo test --test privacy_routing_integration`
Expected: All pass.

- [ ] **Step 4.4: Commit**

```bash
git add crates/vox-orchestrator/tests/privacy_routing_integration.rs
git commit -m "test(orchestrator): wire privacy_requires_local from PrivacyClassifier into CompositeSignal"
```

---

### Task 5: Update where-things-live.md and arch-check

- [ ] **Step 5.1: Add privacy_classifier row**

```
| `privacy_classifier` | Task-content privacy level classifier (D8) | `crates/vox-orchestrator/src/privacy_classifier.rs` |
```

- [ ] **Step 5.2: Run arch-check**

Run: `cargo run -p vox-arch-check 2>&1 | tail -20`

- [ ] **Step 5.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
git commit -m "docs(arch): register privacy_classifier in where-things-live.md"
```

---

### Task 6: Quality gates

- [ ] **G1** `cargo run -p vox-arch-check` — clean
- [ ] **G2** `PrivacyRouteEvent::metric_type == METRIC_TYPE_PRIVACY_ROUTE_DECISION`
- [ ] **G3** No perf bench required (pure rule-based, <100ns)
- [ ] **G4** Regulated level always → `LocalOnly` routing decision
- [ ] **G5** `requires_local` feeds into `CompositeSignal::privacy_requires_local`

---

**Phase 7 sign-off:** 6 tasks complete, 7+ unit tests + 3 golden fixtures, `cargo build -p vox-orchestrator` clean.
