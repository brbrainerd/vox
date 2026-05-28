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
    DataError {
        description: String,
    },
    AnalysisError {
        description: String,
    },
    EthicsViolation {
        description: String,
    },
    /// The DOI is superseded by a corrected version.
    Superseded {
        replacement_doi: String,
    },
    Other {
        description: String,
    },
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

    let replacement_doi = if let RetractionReason::Superseded {
        ref replacement_doi,
    } = reason
    {
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
        assert!(
            record.replacement_doi.is_none(),
            "DataError has no replacement DOI"
        );
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

        assert!(
            record.crossref_propagated,
            "must be true after mark_crossref_propagated"
        );
    }
}
