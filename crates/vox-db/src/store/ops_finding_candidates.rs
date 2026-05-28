//! Store ops for the `scientia_finding_candidates` table (Phase A).
//!
//! Producers in `vox-scientia-producers` emit `FindingCandidateRow` records via
//! these methods. The `(producer_name, signal_fingerprint)` UNIQUE index gives
//! us idempotent inserts — a re-observation of the same signal returns the
//! existing row's id instead of erroring (caller treats that as "already seen").

use crate::VoxDb;
use crate::store::types::StoreError;
use serde::{Deserialize, Serialize};
use turso::params;

/// Closed enum mirroring `contracts/scientia/finding-candidate.v1.schema.json`.
/// Turso does not yet support CHECK constraints; this enum is the *only*
/// admissible source of `scientia_finding_candidates.candidate_class` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingCandidateClass {
    AlgorithmicImprovement,
    ReproducibilityInfra,
    PolicyGovernance,
    TelemetryTrust,
    Other,
}

impl FindingCandidateClass {
    pub fn as_sql(&self) -> &'static str {
        match self {
            Self::AlgorithmicImprovement => "algorithmic_improvement",
            Self::ReproducibilityInfra => "reproducibility_infra",
            Self::PolicyGovernance => "policy_governance",
            Self::TelemetryTrust => "telemetry_trust",
            Self::Other => "other",
        }
    }

    pub fn from_sql(s: &str) -> Option<Self> {
        match s {
            "algorithmic_improvement" => Some(Self::AlgorithmicImprovement),
            "reproducibility_infra" => Some(Self::ReproducibilityInfra),
            "policy_governance" => Some(Self::PolicyGovernance),
            "telemetry_trust" => Some(Self::TelemetryTrust),
            "other" => Some(Self::Other),
            _ => None,
        }
    }
}

/// One row of `scientia_finding_candidates`. Field order matches the DDL.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FindingCandidateRow {
    pub candidate_id: String,
    pub candidate_class: FindingCandidateClass,
    pub publication_id: Option<String>,
    pub title_hint: Option<String>,
    /// `discovery_signal[]` JSON serialization. Validity per
    /// `contracts/scientia/discovery-signal.schema.json` is the producer's
    /// responsibility; store ops treat the field as opaque text.
    pub internal_signals_json: String,
    pub novelty_evidence_bundle_id: Option<String>,
    pub worthiness_decision_ref: Option<String>,
    pub confidence_json: Option<String>,
    pub repository_id: Option<String>,
    pub producer_name: String,
    pub signal_fingerprint: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

/// Outcome of an idempotent insert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InsertOutcome {
    /// New row inserted.
    Inserted,
    /// `(producer_name, signal_fingerprint)` already present; no change.
    AlreadySeen,
}

impl VoxDb {
    /// Insert a finding-candidate row.
    ///
    /// Returns `InsertOutcome::AlreadySeen` when the
    /// `(producer_name, signal_fingerprint)` pair is already present — this is
    /// the producer's "I've already emitted this signal" signal. Other errors
    /// surface as `StoreError`.
    pub async fn insert_finding_candidate(
        &self,
        row: &FindingCandidateRow,
    ) -> Result<InsertOutcome, StoreError> {
        // Check existence first; if present, return AlreadySeen.
        let mut existing = self
            .conn
            .query(
                "SELECT 1 FROM scientia_finding_candidates \
                 WHERE producer_name = ?1 AND signal_fingerprint = ?2 LIMIT 1",
                params![row.producer_name.clone(), row.signal_fingerprint.clone()],
            )
            .await
            .map_err(StoreError::Turso)?;
        if existing
            .next()
            .await
            .map_err(StoreError::Turso)?
            .is_some()
        {
            return Ok(InsertOutcome::AlreadySeen);
        }

        self.conn
            .execute(
                "INSERT INTO scientia_finding_candidates(\
                    candidate_id, candidate_class, publication_id, title_hint, \
                    internal_signals_json, novelty_evidence_bundle_id, \
                    worthiness_decision_ref, confidence_json, repository_id, \
                    producer_name, signal_fingerprint, created_at_ms, updated_at_ms\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                params![
                    row.candidate_id.clone(),
                    row.candidate_class.as_sql().to_string(),
                    row.publication_id.clone(),
                    row.title_hint.clone(),
                    row.internal_signals_json.clone(),
                    row.novelty_evidence_bundle_id.clone(),
                    row.worthiness_decision_ref.clone(),
                    row.confidence_json.clone(),
                    row.repository_id.clone(),
                    row.producer_name.clone(),
                    row.signal_fingerprint.clone(),
                    row.created_at_ms,
                    row.updated_at_ms,
                ],
            )
            .await
            .map_err(StoreError::Turso)?;
        Ok(InsertOutcome::Inserted)
    }

    /// List candidates, optionally filtered by class. Newest first.
    pub async fn list_finding_candidates(
        &self,
        class: Option<FindingCandidateClass>,
    ) -> Result<Vec<FindingCandidateRow>, StoreError> {
        let mut rows = if let Some(c) = class {
            self.conn
                .query(
                    "SELECT candidate_id, candidate_class, publication_id, title_hint, \
                            internal_signals_json, novelty_evidence_bundle_id, \
                            worthiness_decision_ref, confidence_json, repository_id, \
                            producer_name, signal_fingerprint, created_at_ms, updated_at_ms \
                     FROM scientia_finding_candidates \
                     WHERE candidate_class = ?1 \
                     ORDER BY created_at_ms DESC",
                    params![c.as_sql().to_string()],
                )
                .await
                .map_err(StoreError::Turso)?
        } else {
            self.conn
                .query(
                    "SELECT candidate_id, candidate_class, publication_id, title_hint, \
                            internal_signals_json, novelty_evidence_bundle_id, \
                            worthiness_decision_ref, confidence_json, repository_id, \
                            producer_name, signal_fingerprint, created_at_ms, updated_at_ms \
                     FROM scientia_finding_candidates \
                     ORDER BY created_at_ms DESC",
                    (),
                )
                .await
                .map_err(StoreError::Turso)?
        };

        let mut out = Vec::new();
        while let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let class_str: String = row.get(1).map_err(StoreError::Turso)?;
            let class = FindingCandidateClass::from_sql(&class_str).ok_or_else(|| {
                StoreError::Db(format!(
                    "scientia_finding_candidates.candidate_class out of enum: {class_str}"
                ))
            })?;
            out.push(FindingCandidateRow {
                candidate_id: row.get(0).map_err(StoreError::Turso)?,
                candidate_class: class,
                publication_id: row.get(2).map_err(StoreError::Turso)?,
                title_hint: row.get(3).map_err(StoreError::Turso)?,
                internal_signals_json: row.get(4).map_err(StoreError::Turso)?,
                novelty_evidence_bundle_id: row.get(5).map_err(StoreError::Turso)?,
                worthiness_decision_ref: row.get(6).map_err(StoreError::Turso)?,
                confidence_json: row.get(7).map_err(StoreError::Turso)?,
                repository_id: row.get(8).map_err(StoreError::Turso)?,
                producer_name: row.get(9).map_err(StoreError::Turso)?,
                signal_fingerprint: row.get(10).map_err(StoreError::Turso)?,
                created_at_ms: row.get(11).map_err(StoreError::Turso)?,
                updated_at_ms: row.get(12).map_err(StoreError::Turso)?,
            });
        }
        Ok(out)
    }

    /// Fetch a single candidate by `candidate_id`.
    pub async fn get_finding_candidate(
        &self,
        candidate_id: &str,
    ) -> Result<Option<FindingCandidateRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT candidate_id, candidate_class, publication_id, title_hint, \
                        internal_signals_json, novelty_evidence_bundle_id, \
                        worthiness_decision_ref, confidence_json, repository_id, \
                        producer_name, signal_fingerprint, created_at_ms, updated_at_ms \
                 FROM scientia_finding_candidates \
                 WHERE candidate_id = ?1",
                params![candidate_id.to_string()],
            )
            .await
            .map_err(StoreError::Turso)?;
        if let Some(row) = rows.next().await.map_err(StoreError::Turso)? {
            let class_str: String = row.get(1).map_err(StoreError::Turso)?;
            let class = FindingCandidateClass::from_sql(&class_str).ok_or_else(|| {
                StoreError::Db(format!(
                    "scientia_finding_candidates.candidate_class out of enum: {class_str}"
                ))
            })?;
            Ok(Some(FindingCandidateRow {
                candidate_id: row.get(0).map_err(StoreError::Turso)?,
                candidate_class: class,
                publication_id: row.get(2).map_err(StoreError::Turso)?,
                title_hint: row.get(3).map_err(StoreError::Turso)?,
                internal_signals_json: row.get(4).map_err(StoreError::Turso)?,
                novelty_evidence_bundle_id: row.get(5).map_err(StoreError::Turso)?,
                worthiness_decision_ref: row.get(6).map_err(StoreError::Turso)?,
                confidence_json: row.get(7).map_err(StoreError::Turso)?,
                repository_id: row.get(8).map_err(StoreError::Turso)?,
                producer_name: row.get(9).map_err(StoreError::Turso)?,
                signal_fingerprint: row.get(10).map_err(StoreError::Turso)?,
                created_at_ms: row.get(11).map_err(StoreError::Turso)?,
                updated_at_ms: row.get(12).map_err(StoreError::Turso)?,
            }))
        } else {
            Ok(None)
        }
    }
}
