# SCIENTIA Phase 3 — Reputational Firewall

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend `vox-prereg` with three modules that enforce the reputational firewall: a 14-day right-of-reply window gate, retraction nanopub emission, and living-review versioned-DOI management.

**Architecture:** All three modules are pure domain logic (no network, no DB). The right-of-reply window gate is analogous to `PreregGate`: call before publishing, get GateResult. Retraction records are value objects. Living-review manifests are structs with a version_history Vec.

**Tech Stack:** Existing vox-prereg deps (serde, serde_json, thiserror). No new deps needed.

**Strategic reference:** [SCIENTIA plan §4 (Reputational firewall)](../../../src/architecture/scientia-self-publication-finalization-plan-2026.md#phase-3--reputational-firewall)

---

## Task 1: `reply_window.rs` — 14-day right-of-reply window gate

- [ ] **1.1 Create `crates/vox-prereg/src/reply_window.rs`** with the four failing tests first (TDD):

```rust
//! Right-of-reply window gate — SCIENTIA Phase 3.
//!
//! Enforces a 14-day right-of-reply window before a provider_atlas topic-pack
//! may be published. Mirrors the [`PreregGate`] pattern: call before publishing,
//! receive a [`crate::gate::GateResult`].

use crate::gate::GateResult;

const WINDOW_DAYS: u64 = 14;
const SECS_PER_DAY: u64 = 86_400;
const WINDOW_SECS: u64 = WINDOW_DAYS * SECS_PER_DAY;

/// A record tracking the right-of-reply window for one provider.
#[derive(Debug, Clone)]
pub struct ReplyWindowRecord {
    pub provider_id: String,
    /// Unix timestamp (seconds) when the window was opened (draft sent to provider).
    pub window_opened_at: i64,
    /// True once the provider has explicitly cleared the window.
    pub provider_cleared: bool,
    /// Inline reply text per IMC measurement-paper conventions (None if no reply).
    pub reply_content: Option<String>,
}

/// Current status of a right-of-reply window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowStatus {
    /// Still within the 14-day window; provider has not cleared.
    Open { days_remaining: u64 },
    /// Provider explicitly cleared the window; `has_reply` indicates a reply was ingested.
    Cleared { has_reply: bool },
    /// 14 days have elapsed without a provider response; publication may proceed.
    Expired,
}

/// Gate that enforces the 14-day right-of-reply window before publication.
#[derive(Debug, Default, Clone)]
pub struct ReplyWindowGate;

impl ReplyWindowGate {
    pub fn new() -> Self {
        Self
    }

    /// Compute the current [`WindowStatus`] given `now_unix` (Unix seconds).
    ///
    /// Deterministic: callers pass the clock value, enabling test control.
    pub fn status(&self, record: &ReplyWindowRecord, now_unix: i64) -> WindowStatus {
        if record.provider_cleared {
            return WindowStatus::Cleared { has_reply: record.reply_content.is_some() };
        }

        let elapsed = (now_unix - record.window_opened_at).max(0) as u64;
        if elapsed >= WINDOW_SECS {
            WindowStatus::Expired
        } else {
            let secs_remaining = WINDOW_SECS - elapsed;
            // Round up: partial day counts as a full day remaining.
            let days_remaining = (secs_remaining + SECS_PER_DAY - 1) / SECS_PER_DAY;
            WindowStatus::Open { days_remaining }
        }
    }

    /// Check whether publication is permitted.
    ///
    /// Returns [`GateResult::Approved`] when the window is `Cleared` or `Expired`.
    /// Returns [`GateResult::Refused`] with `days_remaining` when still `Open`.
    pub fn check_publication(&self, record: &ReplyWindowRecord, now_unix: i64) -> GateResult {
        match self.status(record, now_unix) {
            WindowStatus::Open { days_remaining } => GateResult::Refused {
                reason: format!(
                    "right-of-reply window is still open for provider '{}': {} day(s) remaining",
                    record.provider_id, days_remaining
                ),
            },
            WindowStatus::Cleared { .. } | WindowStatus::Expired => GateResult::Approved,
        }
    }
}

/// Ingest a reply from the provider.
///
/// Sets `record.reply_content` to `Some(reply_text)` and marks `provider_cleared = true`.
/// Per IMC conventions the reply is stored inline, not as an appendix.
pub fn ingest_reply(record: &mut ReplyWindowRecord, reply_text: &str) {
    record.reply_content = Some(reply_text.to_string());
    record.provider_cleared = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opened_at() -> i64 {
        // Arbitrary fixed epoch: 2026-01-01 00:00:00 UTC
        1_767_225_600
    }

    fn base_record() -> ReplyWindowRecord {
        ReplyWindowRecord {
            provider_id: "provider-alpha".to_string(),
            window_opened_at: opened_at(),
            provider_cleared: false,
            reply_content: None,
        }
    }

    #[test]
    fn window_is_open_within_14_days() {
        let gate = ReplyWindowGate::new();
        let record = base_record();
        // 5 days after opening
        let now = opened_at() + 5 * 86_400;
        let status = gate.status(&record, now);
        assert_eq!(
            status,
            WindowStatus::Open { days_remaining: 9 },
            "5 days elapsed → 9 days remaining"
        );
    }

    #[test]
    fn window_is_expired_after_14_days() {
        let gate = ReplyWindowGate::new();
        let record = base_record();
        // 15 days after opening
        let now = opened_at() + 15 * 86_400;
        let status = gate.status(&record, now);
        assert_eq!(status, WindowStatus::Expired, "15 days elapsed → Expired");
    }

    #[test]
    fn provider_cleared_before_14_days() {
        let gate = ReplyWindowGate::new();
        let mut record = base_record();
        record.provider_cleared = true;
        // Only 3 days elapsed, but provider cleared
        let now = opened_at() + 3 * 86_400;
        let status = gate.status(&record, now);
        assert_eq!(
            status,
            WindowStatus::Cleared { has_reply: false },
            "provider_cleared=true, no reply → Cleared{{has_reply: false}}"
        );
    }

    #[test]
    fn ingested_reply_marks_cleared() {
        let mut record = base_record();
        assert!(!record.provider_cleared);
        assert!(record.reply_content.is_none());

        ingest_reply(&mut record, "We dispute the latency figures in §3.");

        assert!(record.provider_cleared, "ingest_reply must set provider_cleared");
        assert_eq!(
            record.reply_content.as_deref(),
            Some("We dispute the latency figures in §3."),
            "ingest_reply must store reply text"
        );
    }
}
```

- [ ] **1.2 Run failing tests:** `cargo test -p vox-prereg reply_window 2>&1 | tail -30`
  - Expected: 4 compile errors (module not yet in `lib.rs`).

- [ ] **1.3 Wire module temporarily** by adding `pub mod reply_window;` to `lib.rs`, then run again.
  - Expected: tests compile and pass (4/4).

- [ ] **1.4 Commit:**
  ```
  feat(scientia): 14-day right-of-reply window gate (Phase 3 Task 1)
  ```

---

## Task 2: `retraction.rs` — retraction record + emission

- [ ] **2.1 Create `crates/vox-prereg/src/retraction.rs`** with failing tests first:

```rust
//! Retraction nanopub emission — SCIENTIA Phase 3.
//!
//! A [`RetractionRecord`] is a value object capturing who retracted a DOI,
//! why, and whether the retraction has been propagated to Crossref Labs.
//! [`emit_retraction`] constructs a fresh record; [`mark_crossref_propagated`]
//! advances its state once the polling confirms propagation.

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// The reason a publication was retracted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetractionReason {
    DataError { description: String },
    AnalysisError { description: String },
    EthicsViolation { description: String },
    /// The DOI is superseded by a corrected version.
    Superseded { replacement_doi: String },
    Other { description: String },
}

/// An immutable retraction record (value object).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetractionRecord {
    pub retracted_doi: String,
    /// Unix timestamp (seconds) when the retraction was emitted.
    pub retracted_at: i64,
    pub reason: RetractionReason,
    /// ORCID or organisational identifier of the retracting party.
    pub retracted_by: String,
    /// Some if a corrected version exists; mirrors `Superseded::replacement_doi` when applicable.
    pub replacement_doi: Option<String>,
    /// True once Crossref Labs has been notified of this retraction.
    pub crossref_propagated: bool,
}

/// Emit a new retraction record for `doi`.
///
/// `retracted_at` is set to `now()` via [`SystemTime`].
/// `crossref_propagated` starts as `false`.
pub fn emit_retraction(
    doi: &str,
    reason: RetractionReason,
    retracted_by: &str,
) -> RetractionRecord {
    let retracted_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after epoch")
        .as_secs() as i64;

    let replacement_doi = if let RetractionReason::Superseded { ref replacement_doi } = reason {
        Some(replacement_doi.clone())
    } else {
        None
    };

    RetractionRecord {
        retracted_doi: doi.to_string(),
        retracted_at,
        reason,
        retracted_by: retracted_by.to_string(),
        replacement_doi,
        crossref_propagated: false,
    }
}

/// Mark that Crossref Labs has been notified of this retraction.
pub fn mark_crossref_propagated(record: &mut RetractionRecord) {
    record.crossref_propagated = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_reason() -> RetractionReason {
        RetractionReason::DataError {
            description: "Sensor drift invalidated temperature readings in §4.2".to_string(),
        }
    }

    #[test]
    fn retraction_record_serializes_round_trip() {
        let record = emit_retraction(
            "10.5281/zenodo.99999",
            sample_reason(),
            "https://orcid.org/0000-0001-2345-6789",
        );
        let json = serde_json::to_string(&record).expect("must serialize");
        let decoded: RetractionRecord = serde_json::from_str(&json).expect("must deserialize");
        assert_eq!(decoded.retracted_doi, record.retracted_doi);
        assert_eq!(decoded.retracted_by, record.retracted_by);
        assert_eq!(decoded.crossref_propagated, record.crossref_propagated);
        assert!(matches!(decoded.reason, RetractionReason::DataError { .. }));
    }

    #[test]
    fn emit_retraction_sets_fields_correctly() {
        let before = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        let record = emit_retraction(
            "10.5281/zenodo.12345",
            sample_reason(),
            "https://orcid.org/0000-0009-8765-4321",
        );

        let after = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;

        assert_eq!(record.retracted_doi, "10.5281/zenodo.12345");
        assert_eq!(record.retracted_by, "https://orcid.org/0000-0009-8765-4321");
        assert!(!record.crossref_propagated, "must start un-propagated");
        assert!(record.replacement_doi.is_none(), "DataError has no replacement DOI");
        assert!(
            record.retracted_at >= before && record.retracted_at <= after,
            "retracted_at must be within the test wall-clock window"
        );
    }

    #[test]
    fn mark_crossref_propagated_sets_flag() {
        let mut record = emit_retraction(
            "10.5281/zenodo.55555",
            RetractionReason::Superseded {
                replacement_doi: "10.5281/zenodo.55556".to_string(),
            },
            "org:vox-research",
        );
        assert!(!record.crossref_propagated);
        assert_eq!(
            record.replacement_doi.as_deref(),
            Some("10.5281/zenodo.55556"),
            "Superseded reason must populate replacement_doi"
        );

        mark_crossref_propagated(&mut record);

        assert!(record.crossref_propagated, "must be true after mark_crossref_propagated");
    }
}
```

- [ ] **2.2 Run failing tests:** `cargo test -p vox-prereg retraction 2>&1 | tail -30`
  - Expected: compile error (module not yet exported).

- [ ] **2.3 Wire module temporarily** by adding `pub mod retraction;` to `lib.rs`, then run again.
  - Expected: 3/3 tests pass.

- [ ] **2.4 Commit:**
  ```
  feat(scientia): retraction record + emission (Phase 3 Task 2)
  ```

---

## Task 3: `living_review.rs` — versioned DOI management

- [ ] **3.1 Create `crates/vox-prereg/src/living_review.rs`** with failing tests first:

```rust
//! Living-review versioned-DOI management — SCIENTIA Phase 3.
//!
//! Living-review semantics: each publication creates a new immutable DOI version;
//! the canonical URL always points to "latest". [`LivingReviewManifest`] tracks the
//! full `version_history` Vec (oldest first) and keeps `latest_doi` up to date.

use serde::{Deserialize, Serialize};

/// An immutable snapshot of one published version.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoiVersion {
    pub doi: String,
    /// 1-based version number (first published = 1).
    pub version: u32,
    /// Unix timestamp (seconds) when this version was published.
    pub published_at: i64,
    /// URL resolving to this specific version (version-pinned).
    pub canonical_url: String,
    /// Human-readable description of what changed in this version.
    pub description: Option<String>,
}

/// A living-review manifest: mutable, append-only version history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LivingReviewManifest {
    pub title: String,
    /// Always points to the latest published version (updated by [`add_version`]).
    pub canonical_url: String,
    /// DOI of the most recently added version.
    pub latest_doi: String,
    /// Full version history, oldest first.
    pub version_history: Vec<DoiVersion>,
}

impl LivingReviewManifest {
    /// Create an empty manifest with no versions.
    ///
    /// `canonical_url` should be the stable "latest" URL that will be updated
    /// each time [`add_version`] is called.
    pub fn new(title: String, canonical_url: String) -> Self {
        Self {
            title,
            canonical_url,
            latest_doi: String::new(),
            version_history: Vec::new(),
        }
    }

    /// Append a new published version.
    ///
    /// - `version` is auto-incremented (1-based).
    /// - `self.latest_doi` is updated to the new DOI.
    /// - `self.canonical_url` is updated to the new version's `canonical_url`.
    pub fn add_version(
        &mut self,
        doi: String,
        canonical_url: String,
        published_at: i64,
        description: Option<String>,
    ) {
        let version = self.version_history.len() as u32 + 1;
        self.latest_doi = doi.clone();
        self.canonical_url = canonical_url.clone();
        self.version_history.push(DoiVersion {
            doi,
            version,
            published_at,
            canonical_url,
            description,
        });
    }

    /// Returns the most recently added version, or `None` if no versions exist.
    pub fn latest_version(&self) -> Option<&DoiVersion> {
        self.version_history.last()
    }

    /// Total number of published versions.
    pub fn version_count(&self) -> usize {
        self.version_history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_manifest() -> LivingReviewManifest {
        LivingReviewManifest::new(
            "Provider Atlas: Reliability Edition".to_string(),
            "https://vox.research/atlas/latest".to_string(),
        )
    }

    #[test]
    fn new_manifest_has_no_versions() {
        let manifest = base_manifest();
        assert_eq!(manifest.version_count(), 0, "new manifest must have 0 versions");
        assert!(manifest.latest_version().is_none(), "latest_version must be None when empty");
        assert!(manifest.latest_doi.is_empty(), "latest_doi must be empty string initially");
    }

    #[test]
    fn add_version_updates_latest_doi() {
        let mut manifest = base_manifest();
        manifest.add_version(
            "10.5281/zenodo.100001".to_string(),
            "https://vox.research/atlas/v1".to_string(),
            1_767_225_600,
            Some("Initial release".to_string()),
        );
        assert_eq!(manifest.latest_doi, "10.5281/zenodo.100001");
        assert_eq!(manifest.canonical_url, "https://vox.research/atlas/v1");
    }

    #[test]
    fn add_version_increments_version_number() {
        let mut manifest = base_manifest();
        manifest.add_version(
            "10.5281/zenodo.100001".to_string(),
            "https://vox.research/atlas/v1".to_string(),
            1_767_225_600,
            None,
        );
        manifest.add_version(
            "10.5281/zenodo.100002".to_string(),
            "https://vox.research/atlas/v2".to_string(),
            1_767_312_000,
            None,
        );
        let v1 = &manifest.version_history[0];
        let v2 = &manifest.version_history[1];
        assert_eq!(v1.version, 1, "first version must be 1");
        assert_eq!(v2.version, 2, "second version must be 2");
    }

    #[test]
    fn version_history_is_ordered_oldest_first() {
        let mut manifest = base_manifest();
        for i in 1u32..=3 {
            manifest.add_version(
                format!("10.5281/zenodo.10000{i}"),
                format!("https://vox.research/atlas/v{i}"),
                1_767_225_600 + (i as i64 - 1) * 86_400,
                None,
            );
        }
        assert_eq!(manifest.version_count(), 3);
        let versions: Vec<u32> = manifest.version_history.iter().map(|v| v.version).collect();
        assert_eq!(versions, vec![1, 2, 3], "version_history must be ordered oldest-first (1, 2, 3)");
        assert_eq!(
            manifest.latest_version().unwrap().doi,
            "10.5281/zenodo.100003"
        );
    }
}
```

- [ ] **3.2 Run failing tests:** `cargo test -p vox-prereg living_review 2>&1 | tail -30`
  - Expected: compile error (module not yet exported).

- [ ] **3.3 Wire module temporarily** by adding `pub mod living_review;` to `lib.rs`, then run again.
  - Expected: 4/4 tests pass.

- [ ] **3.4 Commit:**
  ```
  feat(scientia): living-review versioned DOI manifest (Phase 3 Task 3)
  ```

---

## Task 4: Update `lib.rs` + run all tests

- [ ] **4.1 Replace the module declarations + re-exports in `crates/vox-prereg/src/lib.rs`:**

The existing `lib.rs` exports:
```rust
pub mod deviation;
pub mod gate;
pub mod signing;
pub mod symbolic;
pub mod trusty_uri;

pub use deviation::{DeviationDetector, DeviationReport};
pub use gate::{GateResult, PreregGate};
pub use signing::{SignError, Signature, VerifyError, sign_prereg, verify_prereg};
pub use symbolic::{
    BayesianStoppingRule, NumericComparatorVerifier, StopDecision, SymbolicVerdict,
};
pub use trusty_uri::compute_trusty_uri;
```

Add the three Phase 3 modules immediately after the existing `pub mod trusty_uri;` line:

```rust
pub mod living_review;
pub mod reply_window;
pub mod retraction;
```

Add the corresponding re-exports immediately after the `pub use trusty_uri::compute_trusty_uri;` line:

```rust
pub use living_review::{DoiVersion, LivingReviewManifest};
pub use reply_window::{ReplyWindowGate, ReplyWindowRecord, WindowStatus, ingest_reply};
pub use retraction::{
    RetractionReason, RetractionRecord, emit_retraction, mark_crossref_propagated,
};
```

NOTE: `reply_window.rs` uses `crate::gate::GateResult` internally (already re-exported as `GateResult` from `gate.rs`). No alias needed; no new type introduced.

- [ ] **4.2 Run the full test suite:** `cargo test -p vox-prereg 2>&1 | tail -30`

Expected output: all tests pass. Test count breakdown:
  - `gate.rs`: 4 tests
  - `signing.rs`: 3 tests (verify pass, sign roundtrip, bad sig)
  - `deviation.rs`: 4 tests
  - `symbolic.rs`: 11 tests
  - `trusty_uri.rs` / `reply_window.rs`: 4 tests
  - `retraction.rs`: 3 tests
  - `living_review.rs`: 4 tests

  Total: ≥ 33 tests all green.

- [ ] **4.3 Commit:**
  ```
  feat(scientia): export Phase 3 modules from vox-prereg lib (Phase 3 Task 4)
  ```

---

## Task 5: Mark Phase 3 Complete in strategic plan

- [ ] **5.1 Open `docs/src/architecture/scientia-self-publication-finalization-plan-2026.md`.**

- [ ] **5.2 Find the Phase 3 heading** (search for `## Phase 3` or `### Phase 3`).

- [ ] **5.3 Add a status badge** immediately after the heading, e.g.:

  ```markdown
  > **Status: Complete** — `reply_window.rs`, `retraction.rs`, `living_review.rs` landed in `vox-prereg`; Provider Atlas dry-run lifecycle works end-to-end. (2026-05-09)
  ```

- [ ] **5.4 Run the full test suite one final time** to confirm no regressions:
  `cargo test -p vox-prereg 2>&1 | tail -20`

- [ ] **5.5 Commit:**
  ```
  feat(scientia): mark Phase 3 Complete + wire vox-prereg Phase 3 modules
  ```

---

## Implementation rules

- Every step shows complete, runnable code (no placeholders).
- TDD order: write tests first → run to see compile/test failure → write implementation → run to pass.
- All cargo commands use `-p vox-prereg` explicitly.
- `reply_window.rs` imports `use crate::gate::GateResult;` — not a new enum.
- `now_unix: i64` parameter on `ReplyWindowGate::status` and `check_publication` makes tests deterministic without any clock dependency.
- `emit_retraction` uses `std::time::SystemTime::now()` for `retracted_at`; tests bracket with before/after wall-clock reads.
- No type is referenced before its definition within a file.
