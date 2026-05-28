# SCIENTIA Phase 0a — Phantom-Import Resolution

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Activate the orphaned [`vox-orchestrator/dei_shim/research/`](../../../../crates/vox-orchestrator/src/dei_shim/research/) tree by writing the six missing modules (`claims`, `gate`, `planner`, `provider`, `types`, `verifier`) plus `persistence`, all as type-correct behavioral stubs, so that `run_research()` compiles, runs end-to-end, and returns a coherent (empty) `ResearchResult`. This unblocks every subsequent SCIENTIA phase.

**Architecture:** The orphan is a 488-line research-pipeline scaffold left from a prior abandoned attempt. Its module declarations don't exist as files; the parent module never declares `pub mod research`, so the file is dead-code-on-disk. We activate it by (a) declaring `pub mod research` in `dei_shim/mod.rs`, (b) providing the seven stub modules with the exact types/functions pipeline.rs imports, (c) returning Vec::new()/default values from every async fn, and (d) marking each stub with `// PHASE_0a_STUB` so Phase 1's `vox-claim-extractor` integration can grep them. Each stub is *type-rich* (full struct/enum definitions) but *behavior-empty* (no LLM calls, no I/O).

**Tech Stack:** Rust 2024 edition; async-trait via tokio; serde for JSON; vox-db for `Codex` calls (already present); vox-secrets for `SecretId`. No new external dependencies.

**Strategic context:** [scientia-self-publication-finalization-plan-2026.md §3.1](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#31-resolve-phantom-imports-first-pre-existing-tech-debt) and §6 phase index.

**Out of scope** (deferred to later phases):
- Real claim extraction (Phase 1, `vox-claim-extractor`)
- Real verification (Phase 1, MiniCheck integration)
- Real planner (Phase 1)
- Provider observability (Phase 6)
- Pre-registration enforcement (Phase 2)

---

## File inventory

| Action | Path | Responsibility |
|---|---|---|
| Modify | [crates/vox-orchestrator/src/dei_shim/mod.rs](../../../../crates/vox-orchestrator/src/dei_shim/mod.rs) | Add `pub mod research;` |
| Create | `crates/vox-orchestrator/src/dei_shim/research/mod.rs` | Submodule declarations + re-exports |
| Create | `crates/vox-orchestrator/src/dei_shim/research/types.rs` | All shared research types: `ResearchQuery`, `ResearchScope`, `ResearchPlan`, `ResearchHit`, `RetrievalDiagnostics`, `Citation`, `RoutingTier`, `CompetenceSignal`, `ResearchMetadata`, `ResearchResult` |
| Create | `crates/vox-orchestrator/src/dei_shim/research/claims.rs` | `Claim` struct + `extract_claims_with_model()` stub |
| Create | `crates/vox-orchestrator/src/dei_shim/research/gate.rs` | `GateInput`, `ConfidenceSignal`, `score_with_config()` stub, `GateConfig`, `RoutingThresholds` |
| Create | `crates/vox-orchestrator/src/dei_shim/research/planner.rs` | `decompose_query_with_config()` stub + `plan_to_json()` |
| Create | `crates/vox-orchestrator/src/dei_shim/research/provider.rs` | `ProviderRegistry`, `ProviderConfig`, `from_env_with_config()`, `primary_name()` |
| Create | `crates/vox-orchestrator/src/dei_shim/research/verifier.rs` | `verify_claims_with_config()` stub + `ClaimVerdict`, `EvidenceSpan`, `Verdict`, `SpanType` |
| Create | `crates/vox-orchestrator/src/dei_shim/research/persistence.rs` | `slug_from_query()`, `write_research_doc()` |
| Modify | [crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs) | Reference shared `GateConfig`, `RoutingThresholds`, `ProviderConfig` from `super::super::*` |
| Create | `crates/vox-orchestrator/tests/scientia_phase_0a_pipeline_smoke.rs` | Integration test exercising `run_research()` with stubs |

LoC budget: ~800 lines across all stubs (types-rich but behavior-empty). Each module ≤200 LoC.

---

## Pre-flight verification

- [ ] **Step P1: Confirm orphan status**

```bash
grep -n "pub mod research" crates/vox-orchestrator/src/dei_shim/mod.rs
```

Expected output: empty (no `pub mod research` line).

```bash
cargo check -p vox-orchestrator 2>&1 | head -50
```

Expected: clean compile (no errors). The phantom imports in `research/orchestrator/pipeline.rs` are not exercised because the parent `dei_shim::research` module is never declared.

- [ ] **Step P2: Read key context files** (information for the implementer; no edit)

Read fully before starting:
- [`crates/vox-orchestrator/src/dei_shim/research/orchestrator/pipeline.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/pipeline.rs) — the 488-line consumer; defines the type contract.
- [`crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs) — `ResearchConfig` referenced by stubs.
- [`crates/vox-orchestrator/src/dei_shim/research/orchestrator/stages.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/stages.rs) — uses `ResearchHit` etc.
- [`crates/vox-orchestrator/src/dei_shim/research/model_select.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/model_select.rs) — already exists; not orphaned per-se but only reachable through orphan.
- [`crates/vox-orchestrator/src/dei_shim/research/orchestrator/web_gather.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/web_gather.rs) — uses `ResearchHit`, `ResearchPlan`.
- [`crates/vox-orchestrator/src/dei_shim/research/orchestrator/pipeline_cache.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/pipeline_cache.rs) — uses `ResearchQuery`, `ResearchResult`.

Note any additional types/methods used; `Step T1` lists the canonical set but the implementer should cross-check.

---

## Task 1: Activate the module tree (failing first compile)

**Files:**
- Modify: `crates/vox-orchestrator/src/dei_shim/mod.rs` (line 1)

- [ ] **Step 1.1: Write the activation**

Edit `crates/vox-orchestrator/src/dei_shim/mod.rs`. After line 1 (`pub mod route_telemetry;`) add:

```rust
pub mod research;
```

- [ ] **Step 1.2: Run cargo check to verify it now fails**

```bash
cargo check -p vox-orchestrator 2>&1 | tail -40
```

Expected: **multiple errors** of the form:
- `error[E0583]: file not found for module 'research'`
or after we create `research/mod.rs` (Task 2):
- `error[E0432]: unresolved import 'super::super::claims'`
- `error[E0432]: unresolved import 'super::super::gate'`
- (etc. for `planner`, `provider`, `types`, `verifier`, `persistence`)

This proves the activation works and the missing modules are now load-bearing.

- [ ] **Step 1.3: Commit (red baseline)**

```bash
git add crates/vox-orchestrator/src/dei_shim/mod.rs
git commit -m "$(cat <<'EOF'
chore(scientia): activate orphaned dei_shim::research module tree

Phase 0a baseline. Declaring pub mod research; surfaces the missing
sub-modules (claims, gate, planner, provider, types, verifier,
persistence) as compile errors. Subsequent commits land each stub.

Refs: docs/src/architecture/scientia-self-publication-finalization-plan-2026.md §3.1

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

This commit is intentionally red — the workspace will not compile until Tasks 2–9 land. **CI must skip vox-orchestrator until Task 9 is merged**, OR all of Tasks 1–9 must merge as a single PR. Recommend the latter.

---

## Task 2: `dei_shim/research/mod.rs` — module index

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/mod.rs`

- [ ] **Step 2.1: Write the module index**

Create `crates/vox-orchestrator/src/dei_shim/research/mod.rs`:

```rust
//! Research pipeline subsystem for `vox-orchestrator`.
//!
//! See [`docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`]
//! for the strategic context. This module is currently in **Phase 0a stub**
//! state: types are real, behavior returns empty/default values. Phase 1
//! replaces the stub bodies with the `vox-claim-extractor` crate.
//!
//! All stubs are marked `// PHASE_0a_STUB` for grep-based discovery.

pub mod claims;
pub mod gate;
pub mod model_select;
pub mod orchestrator;
pub mod persistence;
pub mod planner;
pub mod provider;
pub mod types;
pub mod verifier;

pub use orchestrator::{run_research, ResearchConfig};
pub use types::{
    Citation, CompetenceSignal, ResearchHit, ResearchMetadata, ResearchPlan, ResearchQuery,
    ResearchResult, ResearchScope, RetrievalDiagnostics, RoutingTier,
};
```

- [ ] **Step 2.2: Run cargo check**

```bash
cargo check -p vox-orchestrator 2>&1 | tail -60
```

Expected: errors now point at missing files for each `pub mod` line. This is correct.

- [ ] **Step 2.3: Commit**

```bash
git add crates/vox-orchestrator/src/dei_shim/research/mod.rs
git commit -m "feat(scientia): add dei_shim/research/mod.rs module index (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 3: `types.rs` — the shared type vocabulary

This is the largest file and the most type-load-bearing. Field types are extracted from pipeline.rs, web_gather.rs, stages.rs, pipeline_cache.rs, and persistence callsites.

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/types.rs`

- [ ] **Step 3.1: Write the failing test first**

Create `crates/vox-orchestrator/tests/scientia_phase_0a_types_round_trip.rs`:

```rust
//! Phase 0a — types must round-trip through serde for telemetry persistence.

use vox_orchestrator::dei_shim::research::types::*;

#[test]
fn research_query_default_constructs() {
    let q = ResearchQuery {
        query: "test".to_string(),
        scope: ResearchScope::Both,
        max_sources: 5,
        persist_to_docs: false,
        verify_claims: true,
    };
    assert_eq!(q.query, "test");
    assert_eq!(q.max_sources, 5);
}

#[test]
fn retrieval_diagnostics_serializes() {
    let d = RetrievalDiagnostics {
        coverage_pct: 0.5,
        subquery_coverage_pct: 0.5,
        avg_provider_score: 0.0,
        fusion_weights: [0.0, 0.0, 0.0],
        dropped_source_count: 0,
        hit_rate: 0.0,
    };
    let json = serde_json::to_value(&d).expect("serializes");
    assert!(json.is_object());
}

#[test]
fn routing_tier_debug_repr_stable() {
    // pipeline.rs uses format!("{:?}", routing_tier) for telemetry;
    // changing the Debug repr is a breaking change.
    assert_eq!(format!("{:?}", RoutingTier::DeepResearch), "DeepResearch");
    assert_eq!(format!("{:?}", RoutingTier::Light), "Light");
    assert_eq!(format!("{:?}", RoutingTier::Direct), "Direct");
}
```

- [ ] **Step 3.2: Run test to verify it fails**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_types_round_trip 2>&1 | tail -10
```

Expected: compile error — `types` module does not exist yet.

- [ ] **Step 3.3: Write the types module**

Create `crates/vox-orchestrator/src/dei_shim/research/types.rs`:

```rust
//! Shared types for the research pipeline. Phase 0a stub — types are real;
//! values populated by stub modules are typically empty/default.

use serde::{Deserialize, Serialize};

use super::verifier::ClaimVerdict;

/// Scope of a research query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResearchScope {
    /// Web sources only.
    Web,
    /// Local Codex only.
    Local,
    /// Web + local.
    Both,
}

/// A single research query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchQuery {
    pub query: String,
    pub scope: ResearchScope,
    pub max_sources: usize,
    pub persist_to_docs: bool,
    pub verify_claims: bool,
}

/// A decomposed research plan: original query + N subqueries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchPlan {
    pub original_query: String,
    pub subqueries: Vec<String>,
    pub scope: ResearchScope,
    pub max_sources_per_subquery: usize,
}

/// One retrieved source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchHit {
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub score: f64,
}

/// Retrieval-stage diagnostics surfaced to the gate and to telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalDiagnostics {
    pub coverage_pct: f64,
    pub subquery_coverage_pct: f64,
    pub avg_provider_score: f64,
    pub fusion_weights: [f64; 3],
    pub dropped_source_count: usize,
    pub hit_rate: f64,
}

/// One citation in the final answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Citation {
    pub source_id: i64,
    pub url: String,
    pub title: String,
    pub snippet: String,
    pub confidence: f64,
}

/// Routing tier the gate selects per query.
///
/// **Stability guarantee:** the `Debug` representation of each variant is
/// used as a telemetry value (`format!("{:?}", routing_tier)`). Changing
/// a variant name is a breaking change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoutingTier {
    Direct,
    Light,
    DeepResearch,
}

/// Aggregated competence signal derived from the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompetenceSignal {
    pub confidence: f32,
    pub quality: f32,
    pub verified_claim_count: usize,
    pub had_verification: bool,
}

impl CompetenceSignal {
    /// Build a competence signal from the gate's confidence score, the
    /// judge's quality score, and the verifier's per-claim verdicts.
    #[must_use]
    pub fn from_verdicts(
        confidence: f32,
        quality: f32,
        verdicts: &[ClaimVerdict],
        had_verification: bool,
    ) -> Self {
        Self {
            confidence,
            quality,
            verified_claim_count: verdicts.len(),
            had_verification,
        }
    }
}

/// Cross-stage telemetry bundle attached to every result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchMetadata {
    pub session_id: i64,
    pub duration_ms: u64,
    pub provider: String,
    pub routing_tier: RoutingTier,
    pub confidence: f64,
    pub subquery_count: usize,
    pub source_count: usize,
    pub claim_verdicts: Vec<ClaimVerdict>,
    pub retrieval_diagnostics: RetrievalDiagnostics,
    pub quality_score: f32,
    pub competence: Option<CompetenceSignal>,
    pub self_verification: Option<serde_json::Value>,
}

/// Final research result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResearchResult {
    pub answer: String,
    pub sources: Vec<ResearchHit>,
    pub citations: Vec<Citation>,
    pub research_metadata: ResearchMetadata,
}
```

- [ ] **Step 3.4: Run test to verify it passes**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_types_round_trip 2>&1 | tail -10
```

Expected: 3 passed. (The crate-level cargo check still fails because other modules are still missing; that's expected.)

- [ ] **Step 3.5: Commit**

```bash
git add crates/vox-orchestrator/src/dei_shim/research/types.rs crates/vox-orchestrator/tests/scientia_phase_0a_types_round_trip.rs
git commit -m "feat(scientia): add research types module (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 4: `claims.rs` — `Claim` + `extract_claims_with_model` stub

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/claims.rs`

- [ ] **Step 4.1: Write the failing test**

Add to `crates/vox-orchestrator/tests/scientia_phase_0a_claims_stub.rs`:

```rust
use vox_orchestrator::dei_shim::research::claims::{extract_claims_with_model, Claim};

#[tokio::test]
async fn extract_claims_stub_returns_empty() {
    let claims = extract_claims_with_model("test query", None, None, None, None).await;
    assert!(claims.is_empty(), "Phase 0a stub must return Vec::new()");
}

#[test]
fn claim_default_fields_set() {
    let c = Claim {
        text: "X".into(),
        claim_id: 0,
        is_numeric: false,
        is_recent: false,
        is_named_event: false,
    };
    assert_eq!(c.text, "X");
}
```

- [ ] **Step 4.2: Run test to verify it fails**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_claims_stub 2>&1 | tail -10
```

Expected: compile error — `claims` module not found.

- [ ] **Step 4.3: Implement the stub**

Create `crates/vox-orchestrator/src/dei_shim/research/claims.rs`:

```rust
//! Claim extraction. Phase 0a STUB — returns empty Vec.
//!
//! Phase 1 replaces this with `vox-claim-extractor` crate calls
//! (SciClaims architecture: VeriScore atomicity gate → atomic decomposition
//! → XGrammar-constrained emission → MiniCheck verification → calibrated
//! ABSTAIN). See:
//!   docs/src/architecture/scientia-self-publication-finalization-plan-2026.md §3.2

use serde::{Deserialize, Serialize};

/// One extracted research claim.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claim {
    /// The claim text itself.
    pub text: String,
    /// Stable hash assigned downstream (FNV-1a of `text`).
    pub claim_id: u64,
    /// Heuristic flag: claim contains a numeric value.
    pub is_numeric: bool,
    /// Heuristic flag: claim mentions a recent date or "recently" / "latest".
    pub is_recent: bool,
    /// Heuristic flag: claim mentions a named entity / event.
    pub is_named_event: bool,
}

/// Extract claims from a query.
///
/// **PHASE_0a_STUB**: returns `Vec::new()`. No LLM invocation. Phase 1 wires
/// this to `vox-claim-extractor`.
///
/// # Parameters
/// - `_query`: the source text (in Phase 0a, this is the user query; Phase 1
///   will accept arbitrary documents).
/// - `_endpoint`, `_api_key`, `_model`, `_max_tokens`: ignored in Phase 0a.
pub async fn extract_claims_with_model(
    _query: &str,
    _endpoint: Option<&str>,
    _api_key: Option<&str>,
    _model: Option<&str>,
    _max_tokens: Option<u32>,
) -> Vec<Claim> {
    // PHASE_0a_STUB: replaced by vox-claim-extractor in Phase 1.
    Vec::new()
}
```

- [ ] **Step 4.4: Run test to verify it passes**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_claims_stub 2>&1 | tail -10
```

Expected: 2 passed.

- [ ] **Step 4.5: Commit**

```bash
git add crates/vox-orchestrator/src/dei_shim/research/claims.rs crates/vox-orchestrator/tests/scientia_phase_0a_claims_stub.rs
git commit -m "feat(scientia): add claims stub returning Vec::new() (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 5: `verifier.rs` — `verify_claims_with_config` stub

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/verifier.rs`

- [ ] **Step 5.1: Write the failing test**

Add `crates/vox-orchestrator/tests/scientia_phase_0a_verifier_stub.rs`:

```rust
use vox_orchestrator::dei_shim::research::{
    claims::Claim, provider::ProviderRegistry, verifier::verify_claims_with_config,
};

#[tokio::test]
async fn verify_claims_stub_returns_empty() {
    let claims = vec![Claim {
        text: "X".into(),
        claim_id: 0,
        is_numeric: false,
        is_recent: false,
        is_named_event: false,
    }];
    let registry = ProviderRegistry::default();
    let cfg = vox_orchestrator::dei_shim::research::verifier::VerifierConfig::default();
    let verdicts = verify_claims_with_config(&claims, "q", &registry, &cfg, None, None).await;
    assert!(verdicts.is_empty(), "Phase 0a verifier stub must return Vec::new()");
}
```

- [ ] **Step 5.2: Run test to verify it fails**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_verifier_stub 2>&1 | tail -10
```

- [ ] **Step 5.3: Implement the stub**

Create `crates/vox-orchestrator/src/dei_shim/research/verifier.rs`:

```rust
//! Claim verification. Phase 0a STUB — returns empty Vec.
//!
//! Phase 1 replaces this with MiniCheck-FT5 (770M T5) wrapped as a Vox plugin,
//! plus calibrated abstention (temperature-scale logits → ABSTAIN below τ).
//! See: docs/src/architecture/scientia-self-publication-finalization-plan-2026.md §3.2

use std::fmt;

use serde::{Deserialize, Serialize};

use super::claims::Claim;
use super::provider::ProviderRegistry;

/// Verifier configuration. Phase 0a — fields are placeholders; Phase 1
/// adds calibration parameters (`abstain_threshold`, `temperature`,
/// `escalation_endpoint`, etc.).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VerifierConfig {
    pub abstain_threshold: Option<f32>,
    pub model: Option<String>,
}

/// Verdict label per SciFact taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Support,
    Contradict,
    NotEnoughInfo,
    Abstain,
}

impl fmt::Display for Verdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Support => write!(f, "support"),
            Self::Contradict => write!(f, "contradict"),
            Self::NotEnoughInfo => write!(f, "not_enough_info"),
            Self::Abstain => write!(f, "abstain"),
        }
    }
}

/// Type of evidence span linkage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    Supporting,
    Contradicting,
    Background,
}

impl fmt::Display for SpanType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Supporting => write!(f, "supporting"),
            Self::Contradicting => write!(f, "contradicting"),
            Self::Background => write!(f, "background"),
        }
    }
}

/// One evidence span linking a claim to a source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceSpan {
    pub source_id: i64,
    pub text: String,
    pub span_type: SpanType,
}

/// Per-claim verification verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaimVerdict {
    pub claim: Claim,
    pub verdict: Verdict,
    pub confidence: f64,
    pub supporting_count: usize,
    pub contradicting_count: usize,
    pub evidence_spans: Vec<EvidenceSpan>,
}

/// Verify a batch of claims against retrieved evidence.
///
/// **PHASE_0a_STUB**: returns `Vec::new()`. Phase 1 wires this to
/// `vox-claim-extractor`'s MiniCheck-backed verifier.
pub async fn verify_claims_with_config(
    _claims: &[Claim],
    _query: &str,
    _registry: &ProviderRegistry,
    _config: &VerifierConfig,
    _endpoint: Option<&str>,
    _api_key: Option<&str>,
) -> Vec<ClaimVerdict> {
    // PHASE_0a_STUB: replaced by vox-claim-extractor in Phase 1.
    Vec::new()
}
```

- [ ] **Step 5.4: Run test to verify it passes**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_verifier_stub 2>&1 | tail -10
```

Expected: 1 passed.

- [ ] **Step 5.5: Commit**

```bash
git add crates/vox-orchestrator/src/dei_shim/research/verifier.rs crates/vox-orchestrator/tests/scientia_phase_0a_verifier_stub.rs
git commit -m "feat(scientia): add verifier stub with full type surface (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 6: `provider.rs` — `ProviderRegistry` stub

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/provider.rs`

- [ ] **Step 6.1: Write the failing test**

Add `crates/vox-orchestrator/tests/scientia_phase_0a_provider_stub.rs`:

```rust
use vox_orchestrator::dei_shim::research::provider::{ProviderConfig, ProviderRegistry};

#[test]
fn provider_registry_default_primary_name() {
    let r = ProviderRegistry::default();
    assert_eq!(r.primary_name(), "stub");
}

#[test]
fn provider_registry_from_env_with_config_does_not_panic() {
    let cfg = ProviderConfig::default();
    let r = ProviderRegistry::from_env_with_config(cfg);
    assert!(!r.primary_name().is_empty());
}
```

- [ ] **Step 6.2: Implement the stub**

Create `crates/vox-orchestrator/src/dei_shim/research/provider.rs`:

```rust
//! Web provider registry. Phase 0a STUB — single in-memory "stub" provider.
//!
//! Phase 5 wires this to real providers via `vox-search`'s SearXNG/DDG/Tavily
//! adapters and Phase 6 introduces `ProviderObservation` per Mesh §4.1.

use serde::{Deserialize, Serialize};

/// Configuration for the provider registry. Phase 0a — fields are placeholders.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub primary: Option<String>,
    pub fallback: Vec<String>,
}

/// Registry of web search providers used by the research pipeline.
///
/// **PHASE_0a_STUB**: in-memory only; `primary_name()` returns "stub".
#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    primary: String,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            primary: "stub".to_string(),
        }
    }
}

impl ProviderRegistry {
    /// Construct from environment + supplied config. Phase 0a ignores both.
    #[must_use]
    pub fn from_env_with_config(_config: ProviderConfig) -> Self {
        // PHASE_0a_STUB: replaced by real provider resolution in Phase 5.
        Self::default()
    }

    /// Name of the primary provider for telemetry attribution.
    #[must_use]
    pub fn primary_name(&self) -> &str {
        &self.primary
    }
}
```

- [ ] **Step 6.3: Run test, commit**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_provider_stub 2>&1 | tail -10
git add crates/vox-orchestrator/src/dei_shim/research/provider.rs crates/vox-orchestrator/tests/scientia_phase_0a_provider_stub.rs
git commit -m "feat(scientia): add provider registry stub (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 7: `gate.rs` — confidence gate stub

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/gate.rs`

- [ ] **Step 7.1: Write the failing test**

Add `crates/vox-orchestrator/tests/scientia_phase_0a_gate_stub.rs`:

```rust
use vox_orchestrator::dei_shim::research::{
    claims::Claim, gate::{score_with_config, GateConfig, GateInput},
    types::RoutingTier,
};

#[test]
fn gate_with_no_hits_routes_direct() {
    let claims: Vec<Claim> = Vec::new();
    let input = GateInput {
        claims: &claims,
        citation_count: 0,
        no_retrieval_hits: true,
        answer_is_empty: true,
    };
    let cfg = GateConfig::default();
    let signal = score_with_config(&input, &cfg);
    let tier = signal.routing_tier_for(0.7, 0.4, 0.2);
    // Empty everything → low score → Direct (the cheapest fallback tier).
    assert!(matches!(tier, RoutingTier::Direct));
}
```

- [ ] **Step 7.2: Implement the stub**

Create `crates/vox-orchestrator/src/dei_shim/research/gate.rs`:

```rust
//! Confidence gate + routing-tier selector. Phase 0a STUB — produces a flat
//! score derived purely from citation count; no claim-level scoring.
//!
//! Phase 2 wires this to the symbolic-verifier strategies and the prereg
//! enforcement layer. See:
//!   docs/src/architecture/scientia-self-publication-finalization-plan-2026.md §5.

use serde::{Deserialize, Serialize};

use super::claims::Claim;
use super::types::RoutingTier;

/// Gate config. Phase 0a — placeholders for Phase 2 calibration knobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GateConfig {
    pub min_citations_for_full_score: Option<usize>,
}

/// Per-tier routing thresholds.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RoutingThresholds {
    pub direct: f32,
    pub light: f32,
    pub deep: f32,
}

impl Default for RoutingThresholds {
    fn default() -> Self {
        Self {
            direct: 0.7,
            light: 0.4,
            deep: 0.2,
        }
    }
}

/// Confidence-gate input.
#[derive(Debug)]
pub struct GateInput<'a> {
    pub claims: &'a [Claim],
    pub citation_count: usize,
    pub no_retrieval_hits: bool,
    pub answer_is_empty: bool,
}

/// Confidence-gate output.
#[derive(Debug, Clone)]
pub struct ConfidenceSignal {
    pub score: f32,
}

impl ConfidenceSignal {
    /// Pick routing tier given per-tier thresholds.
    #[must_use]
    pub fn routing_tier_for(&self, direct: f32, light: f32, _deep: f32) -> RoutingTier {
        if self.score >= direct {
            RoutingTier::Direct
        } else if self.score >= light {
            RoutingTier::Light
        } else if self.no_retrieval_hits_implied() {
            // No evidence at all → cheapest tier (don't burn cycles on deep
            // research with nothing to verify against).
            RoutingTier::Direct
        } else {
            RoutingTier::DeepResearch
        }
    }

    fn no_retrieval_hits_implied(&self) -> bool {
        self.score == 0.0
    }
}

/// Score a gate input. Phase 0a stub — flat function of citation count.
///
/// Phase 2 replaces this with a fusion of symbolic-verifier strengths,
/// claim-evidence coverage, and contradiction ratio (per
/// [`confidence_fusion.rs`](../../confidence_fusion.rs)).
#[must_use]
pub fn score_with_config(input: &GateInput<'_>, _config: &GateConfig) -> ConfidenceSignal {
    // PHASE_0a_STUB: simple linear function of citation count, capped at 1.0.
    let raw = (input.citation_count as f32) / 5.0;
    ConfidenceSignal {
        score: raw.clamp(0.0, 1.0),
    }
}
```

- [ ] **Step 7.3: Run test, commit**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_gate_stub 2>&1 | tail -10
git add crates/vox-orchestrator/src/dei_shim/research/gate.rs crates/vox-orchestrator/tests/scientia_phase_0a_gate_stub.rs
git commit -m "feat(scientia): add gate stub with linear scoring (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 8: `planner.rs` — query decomposition stub

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/planner.rs`

- [ ] **Step 8.1: Write the failing test**

Add `crates/vox-orchestrator/tests/scientia_phase_0a_planner_stub.rs`:

```rust
use vox_orchestrator::dei_shim::research::{
    planner::{decompose_query_with_config, plan_to_json},
    types::{ResearchPlan, ResearchQuery, ResearchScope},
};

#[tokio::test]
async fn planner_stub_returns_single_subquery() {
    let q = ResearchQuery {
        query: "test".into(),
        scope: ResearchScope::Both,
        max_sources: 3,
        persist_to_docs: false,
        verify_claims: false,
    };
    let plan = decompose_query_with_config(&q, None, None, None, None, None)
        .await
        .expect("stub returns Ok");
    assert_eq!(plan.original_query, "test");
    assert_eq!(plan.subqueries, vec!["test".to_string()]);
}

#[test]
fn plan_to_json_serializes() {
    let plan = ResearchPlan {
        original_query: "q".into(),
        subqueries: vec!["q".into()],
        scope: ResearchScope::Both,
        max_sources_per_subquery: 3,
    };
    let v = plan_to_json(&plan);
    assert!(v.is_object());
}
```

- [ ] **Step 8.2: Implement the stub**

Create `crates/vox-orchestrator/src/dei_shim/research/planner.rs`:

```rust
//! Query planner. Phase 0a STUB — returns the input query as a single subquery.
//!
//! Phase 1 wires this to a SciClaims-style local Mens model via
//! [`vox-actor-runtime`](../../../actor_runtime/mens.rs); Phase 2 adds prereg
//! enforcement so a campaign without a signed prereg cannot reach this stage.

use anyhow::Result;
use serde_json::Value;

use super::types::{ResearchPlan, ResearchQuery};

/// Decompose a research query into a plan with at least one subquery.
///
/// **PHASE_0a_STUB**: returns a plan with the original query as the only subquery.
pub async fn decompose_query_with_config(
    query: &ResearchQuery,
    _endpoint: Option<&str>,
    _api_key: Option<&str>,
    _model: Option<&str>,
    _temperature: Option<f32>,
    _max_subqueries: Option<usize>,
) -> Result<ResearchPlan> {
    // PHASE_0a_STUB: passthrough. Phase 1 invokes Mens for real decomposition.
    Ok(ResearchPlan {
        original_query: query.query.clone(),
        subqueries: vec![query.query.clone()],
        scope: query.scope.clone(),
        max_sources_per_subquery: query.max_sources,
    })
}

/// Serialize a plan to a JSON value for telemetry persistence.
#[must_use]
pub fn plan_to_json(plan: &ResearchPlan) -> Value {
    serde_json::to_value(plan).unwrap_or_else(|_| Value::Null)
}
```

- [ ] **Step 8.3: Run test, commit**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_planner_stub 2>&1 | tail -10
git add crates/vox-orchestrator/src/dei_shim/research/planner.rs crates/vox-orchestrator/tests/scientia_phase_0a_planner_stub.rs
git commit -m "feat(scientia): add planner stub (single-subquery passthrough) (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 9: `persistence.rs` — research-doc writer stub

**Files:**
- Create: `crates/vox-orchestrator/src/dei_shim/research/persistence.rs`

- [ ] **Step 9.1: Write the failing test**

Add `crates/vox-orchestrator/tests/scientia_phase_0a_persistence.rs`:

```rust
use std::path::Path;

use vox_orchestrator::dei_shim::research::persistence::{slug_from_query, write_research_doc};

#[test]
fn slug_from_query_basic() {
    assert_eq!(slug_from_query("Hello, World! 2026"), "hello-world-2026");
    assert_eq!(slug_from_query(""), "untitled");
    let s = slug_from_query("a".repeat(200).as_str());
    assert!(s.len() <= 80, "slug capped at 80 chars, got {}", s.len());
}

#[test]
fn write_research_doc_writes_to_tmpdir() {
    let dir = tempfile::tempdir().expect("tmpdir");
    write_research_doc(dir.path(), "test-slug", "Q?", "A.", "stub-model")
        .expect("writes");
    let p = dir.path().join("docs/src/research/test-slug.md");
    assert!(p.exists(), "expected research doc at {:?}", p);
}
```

(Add `tempfile` to dev-dependencies in `crates/vox-orchestrator/Cargo.toml` if not present:)

```bash
grep -q '^tempfile' crates/vox-orchestrator/Cargo.toml || \
  cargo add --package vox-orchestrator --dev tempfile
```

- [ ] **Step 9.2: Implement the stub**

Create `crates/vox-orchestrator/src/dei_shim/research/persistence.rs`:

```rust
//! Research-document persistence. Phase 0a — fully implemented (no stubbing).
//!
//! Future phases extend this to also emit signed nanopubs (Phase 4) and
//! RO-Crate envelopes (Phase 4) alongside the Markdown doc.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Convert a query to a filesystem-safe slug.
///
/// - Lowercase
/// - Non-alphanumerics → '-'
/// - Collapses runs of '-'
/// - Trims leading/trailing '-'
/// - Caps at 80 chars
/// - Empty → "untitled"
#[must_use]
pub fn slug_from_query(query: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true; // suppress leading '-'
    for c in query.chars() {
        let lower = c.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            out.push(lower);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        return "untitled".to_string();
    }
    if out.len() > 80 {
        out.truncate(80);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

/// Write a research document at `<root>/docs/src/research/<slug>.md`.
pub fn write_research_doc(
    root: &Path,
    slug: &str,
    query: &str,
    answer: &str,
    model: &str,
) -> Result<PathBuf> {
    let dir = root.join("docs/src/research");
    fs::create_dir_all(&dir).with_context(|| format!("create_dir_all({:?})", dir))?;
    let path = dir.join(format!("{slug}.md"));
    let content = format!(
        "---\n\
         title: \"Research: {query_escaped}\"\n\
         description: \"Auto-generated research result.\"\n\
         category: \"research\"\n\
         status: \"draft\"\n\
         model: \"{model}\"\n\
         ---\n\n\
         # {query}\n\n\
         {answer}\n",
        query = query,
        query_escaped = query.replace('"', "\\\""),
        model = model,
        answer = answer,
    );
    fs::write(&path, content).with_context(|| format!("write({:?})", path))?;
    Ok(path)
}
```

- [ ] **Step 9.3: Run test, commit**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_persistence 2>&1 | tail -10
git add crates/vox-orchestrator/src/dei_shim/research/persistence.rs crates/vox-orchestrator/tests/scientia_phase_0a_persistence.rs crates/vox-orchestrator/Cargo.toml
git commit -m "feat(scientia): implement persistence (slug + write_research_doc) (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 10: Reconcile `orchestrator/config.rs` with the new shared types

[`orchestrator/config.rs`](../../../../crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs) references `super::super::config::GateConfig`, `RoutingThresholds`, and `ProviderConfig` (per the audit at lines 59, 63). With Tasks 6–7 these now live in `super::super::{gate,provider}` directly.

**Files:**
- Modify: `crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs`

- [ ] **Step 10.1: Read and patch**

Open `orchestrator/config.rs`. Replace any `super::super::config::GateConfig` with `super::super::gate::GateConfig`. Replace `super::super::config::RoutingThresholds` with `super::super::gate::RoutingThresholds`. Replace `super::super::config::ProviderConfig` with `super::super::provider::ProviderConfig`. (Adjust if the actual paths differ — the implementer must read the file before patching.)

- [ ] **Step 10.2: Verify the crate now compiles end-to-end**

```bash
cargo check -p vox-orchestrator 2>&1 | tail -20
```

Expected: clean compile (no errors).

- [ ] **Step 10.3: Commit**

```bash
git add crates/vox-orchestrator/src/dei_shim/research/orchestrator/config.rs
git commit -m "refactor(scientia): point research config at shared gate/provider types (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 11: End-to-end integration smoke test

**Files:**
- Create: `crates/vox-orchestrator/tests/scientia_phase_0a_pipeline_smoke.rs`

- [ ] **Step 11.1: Write the integration test**

```rust
//! Phase 0a — the orphan tree compiles, run_research is callable, and the
//! full pipeline returns a coherent (empty) ResearchResult when called with
//! all stubs.

use vox_orchestrator::dei_shim::research::{
    run_research, ResearchConfig,
};
use vox_orchestrator::dei_shim::research::types::{ResearchQuery, ResearchScope};

#[tokio::test]
async fn run_research_with_stubs_returns_empty_result() {
    let query = ResearchQuery {
        query: "smoke test".into(),
        scope: ResearchScope::Both,
        max_sources: 3,
        persist_to_docs: false,
        verify_claims: false,
    };
    let config = ResearchConfig::default();

    // No Codex handle → no DB writes; pure in-memory exercise.
    let result = run_research(query, None, &config).await.expect("succeeds");

    // Phase 0a expectations:
    //   - answer is non-fatal default (likely empty or a fallback string)
    //   - sources is empty (no real provider)
    //   - citations is empty (no sources)
    //   - claim_verdicts is empty (verifier stub returns Vec::new())
    //   - routing_tier is RoutingTier::Direct (gate stub: 0 citations → score 0)
    assert!(result.sources.is_empty());
    assert!(result.citations.is_empty());
    assert!(result.research_metadata.claim_verdicts.is_empty());
    assert!(matches!(
        result.research_metadata.routing_tier,
        vox_orchestrator::dei_shim::research::types::RoutingTier::Direct
    ));
}
```

- [ ] **Step 11.2: Run the test**

```bash
cargo test -p vox-orchestrator --test scientia_phase_0a_pipeline_smoke 2>&1 | tail -20
```

Expected: 1 passed.

If it fails because `ResearchConfig::default()` doesn't exist or some other field requires construction, *do not paper over it* — that means a config field was missed. Trace the error to the exact line in `orchestrator/config.rs` and ensure `ResearchConfig` derives or implements `Default`.

- [ ] **Step 11.3: Run the entire vox-orchestrator test suite**

```bash
cargo test -p vox-orchestrator 2>&1 | tail -30
```

Expected: all tests pass (or at minimum, no *new* failures vs. baseline). If there are pre-existing failures unrelated to this work, document them but do not fix them in Phase 0a.

- [ ] **Step 11.4: Run vox-arch-check**

```bash
cargo run -p vox-arch-check 2>&1 | tail -20
```

Expected: pass. If it warns about new module structure, address (or document why warn-only is acceptable for Phase 0a).

- [ ] **Step 11.5: Commit**

```bash
git add crates/vox-orchestrator/tests/scientia_phase_0a_pipeline_smoke.rs
git commit -m "$(cat <<'EOF'
test(scientia): add Phase 0a end-to-end smoke test for run_research

Exercises the full orphaned pipeline with all stub modules. Confirms
run_research is callable without a Codex handle and returns a coherent
empty ResearchResult. This is the gating test for the Phase 0a
deliverable.

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>
EOF
)"
```

---

## Task 12: Update `where-things-live.md`

**Files:**
- Modify: [docs/src/architecture/where-things-live.md](../../../src/architecture/where-things-live.md)

- [ ] **Step 12.1: Add the row**

Per CLAUDE.md (`If your concept isn't there, add the row in the same PR`), add to the "Common tasks → exact path" section after the orchestrator policy module row (~line 138):

```markdown
| Add a research-pipeline stage (claims/gate/planner/provider/types/verifier) | `crates/vox-orchestrator/src/dei_shim/research/<module>.rs`. Phase 0a stubs; Phase 1 replaces claim/verifier with `vox-claim-extractor` calls. |
```

- [ ] **Step 12.2: Run vox-doc-pipeline regen**

Per memory rule (never hand-edit auto-generated docs):

```bash
cargo run -p vox-doc-pipeline 2>&1 | tail -10
```

Expected: regenerates `SUMMARY.md`, `architecture-index.md`, `feed.xml` if they reference the new content. Stage and commit any regenerated changes.

- [ ] **Step 12.3: Commit**

```bash
git add docs/src/architecture/where-things-live.md
# also stage any auto-regenerated docs:
git add docs/src/SUMMARY.md docs/src/architecture-index.md docs/src/feed.xml 2>/dev/null || true
git commit -m "docs(scientia): record where research-pipeline stages live (Phase 0a)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

## Task 13: Final verification

- [ ] **Step 13.1: Workspace-wide compile**

```bash
cargo check --workspace 2>&1 | tail -30
```

Expected: clean across all crates.

- [ ] **Step 13.2: Workspace-wide test**

```bash
cargo test --workspace --no-fail-fast 2>&1 | tail -50
```

Expected: no new failures vs. baseline. Document any pre-existing failures.

- [ ] **Step 13.3: vox-arch-check + vox-doc-pipeline**

```bash
cargo run -p vox-arch-check
cargo run -p vox-doc-pipeline
```

Expected: pass.

- [ ] **Step 13.4: Update Phase 0a row in strategic plan**

Edit [scientia-self-publication-finalization-plan-2026.md](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md) §12 to mark Phase 0a as `Complete`:

```markdown
| 0a | [Phase 0a — Phantom-import resolution](../../superpowers/plans/scientia/2026-05-09-scientia-phase-0a-phantom-imports.md) — **Complete** |
```

Commit with `docs(scientia): mark Phase 0a complete in strategic plan`.

---

## Acceptance criteria

- [ ] `cargo check -p vox-orchestrator` passes (was already passing; passes with new module tree active).
- [ ] `cargo test -p vox-orchestrator` passes including all 7 new test files.
- [ ] `cargo test --workspace --no-fail-fast` shows no new failures.
- [ ] `cargo run -p vox-arch-check` passes.
- [ ] `cargo run -p vox-doc-pipeline` passes.
- [ ] `dei_shim::research::run_research` is callable and returns `Ok(ResearchResult)` with stub-empty fields.
- [ ] All seven stub modules (`claims`, `gate`, `planner`, `provider`, `types`, `verifier`, `persistence`) exist under `crates/vox-orchestrator/src/dei_shim/research/`.
- [ ] Every stub function/method that returns a default/empty value is marked with the `// PHASE_0a_STUB` comment so Phase 1 can grep for replacement sites.
- [ ] No new external dependencies added (no `Cargo.toml` `[dependencies]` changes besides dev-only `tempfile`).
- [ ] Strategic plan §12 updated to mark Phase 0a complete.

## Risks specific to this phase

| # | Risk | Mitigation |
|---|---|---|
| **R0a-1** | Hidden type usage in `web_gather.rs` / `stages.rs` / `pipeline_cache.rs` not covered by Task 3's types.rs (compile errors after Task 9) | Step P2 mandates reading these files before starting Task 3. If a missed type surfaces, add it to `types.rs` in the same PR. |
| **R0a-2** | `ResearchConfig` does not derive `Default`, so smoke test (Task 11) fails | Task 10 sub-step: ensure `ResearchConfig` derives `Default` or has a `default()` constructor. |
| **R0a-3** | `vox-arch-check` warns about a new module exceeding LoC budget | Phase 0a budget is ≤200 LoC per stub; types.rs is the largest at ~150. Should pass `max_loc` warns. |
| **R0a-4** | Workspace compile breaks between Tasks 1–9 (the activation commit is intentionally red) | Land Tasks 1–13 as a single PR, not as separate merges. CI must run on the merge commit only, or the intermediate commits must be marked `[skip ci]`. |
| **R0a-5** | `claim_detection_enabled` flag still gates *only* verification, not extraction (per audit) — Phase 0a doesn't change this | Out of scope; Phase 1 fixes this when wiring `vox-claim-extractor`. Document in Task 4's `// PHASE_0a_STUB` comment. |

---

## Self-review checklist (run after writing the plan, before execution)

- [x] **Spec coverage:** every section of strategic plan §3.1 maps to a task here.
- [x] **No placeholders:** every code block contains real Rust; no `TODO` / `fill in details` / `similar to Task N`.
- [x] **Type consistency:** `Claim` defined in claims.rs (Task 4), referenced from verifier.rs (Task 5) and gate.rs (Task 7); `ProviderRegistry` in provider.rs (Task 6), referenced from verifier.rs and types — all imports cross-checked.
- [x] **Pipeline.rs requirements:** every type/function pipeline.rs imports from `super::super::*` is provided in Tasks 3–9.

## Out of scope for this plan (sibling Phase-0 plans to follow)

- **Phase 0b** — Create `vox-research-events` L1 crate + register in `layers.toml` + `Cargo.toml`.
- **Phase 0c** — Codegen Rust enums from `contracts/scientia/*.schema.json`.
- **Phase 0d** — Add 5 DB tables (`claims`, `novelty_results`, `prereg`, `publication_attempts`, `model_profile_learning`).
- **Phase 0e** — Add 6 new `SecretId::*` variants for ORCID, arXiv, Crossref, OpenAlex, Semantic Scholar, OSF.
- **Phase 0f** — `vox-arch-check` rules (no horizontal L3 publisher↔scientia-ingest; new crates registered).

These should be written as separate plans following Phase 0a's pattern. Each is independently testable and mergeable.

## Execution handoff

Plan complete and saved.

**Two execution options:**

1. **Subagent-Driven (recommended)** — Dispatch a fresh subagent per Task; two-stage review between tasks; isolates context per change.
2. **Inline Execution** — Execute all 13 tasks in the current session using `superpowers:executing-plans`; batch with checkpoints after Tasks 3, 9, 11.

Either way, **all 13 tasks land as a single PR** (per R0a-4: the activation commit is intentionally red until the stubs land).
