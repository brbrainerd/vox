//! COPE-aligned retraction workflow for Atlas findings.
//!
//! Reference: <https://publicationethics.org/retraction-guidelines>
//! A retraction must: state the reason clearly, be issued promptly,
//! be linked to the original article, and be communicated to all databases
//! where the original was indexed.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopeRetractionReason {
    DataError,
    AnalysisError,
    EthicsViolation,
    Superseded,
    Fabrication,
    Other,
}

impl CopeRetractionReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::DataError => "data_error",
            Self::AnalysisError => "analysis_error",
            Self::EthicsViolation => "ethics_violation",
            Self::Superseded => "superseded",
            Self::Fabrication => "fabrication",
            Self::Other => "other",
        }
    }

    fn cope_explanation(&self) -> &'static str {
        match self {
            Self::DataError => {
                "The underlying data were found to be unreliable or incorrectly collected."
            }
            Self::AnalysisError => {
                "The analysis contained errors that materially affect the conclusions."
            }
            Self::EthicsViolation => "This work violated ethical standards for research conduct.",
            Self::Superseded => {
                "This finding has been superseded by a subsequent publication with better methodology."
            }
            Self::Fabrication => "Data or results in this work were fabricated.",
            Self::Other => "This work is retracted for reasons stated in the notice text.",
        }
    }
}

/// COPE-compliant retraction notice for a published Atlas finding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopeRetractionNotice {
    pub retracted_doi: String,
    pub reason: CopeRetractionReason,
    /// Human-readable notice text that will appear in the Atlas living-review update.
    pub notice_text: String,
    pub retracted_by: String,
    pub retracted_at: i64,
    /// True once Crossref has been notified (updates the DOI metadata).
    pub crossref_notified: bool,
}

impl CopeRetractionNotice {
    pub fn new(
        retracted_doi: String,
        reason: CopeRetractionReason,
        detail: String,
        retracted_by: String,
    ) -> Self {
        let cope_explanation = reason.cope_explanation();
        let notice_text = format!(
            "RETRACTION: {retracted_doi}. {cope_explanation} {detail} \
             This retraction follows COPE guidelines (https://publicationethics.org/retraction-guidelines). \
             Retracted by: {retracted_by}."
        );
        Self {
            retracted_doi,
            reason,
            notice_text,
            retracted_by,
            retracted_at: current_unix_secs(),
            crossref_notified: false,
        }
    }

    pub fn mark_crossref_notified(&mut self) {
        self.crossref_notified = true;
    }

    /// Serialize for embedding in Atlas manifest or nanopub provenance graph.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "retracted_doi": self.retracted_doi,
            "reason": self.reason.as_str(),
            "notice_text": self.notice_text,
            "retracted_by": self.retracted_by,
            "retracted_at": self.retracted_at,
            "crossref_notified": self.crossref_notified,
            "cope_compliant": true,
            "cope_guidelines_url": "https://publicationethics.org/retraction-guidelines",
        })
    }
}

fn current_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retraction_notice_includes_doi_and_reason() {
        let notice = CopeRetractionNotice::new(
            "10.1234/vox-finding-001".into(),
            CopeRetractionReason::DataError,
            "The underlying telemetry data had a collection bug.".into(),
            "Vox Research Team".into(),
        );
        assert_eq!(notice.retracted_doi, "10.1234/vox-finding-001");
        assert_eq!(notice.reason, CopeRetractionReason::DataError);
        assert!(!notice.notice_text.is_empty());
        let json = notice.to_json();
        assert_eq!(json["retracted_doi"], "10.1234/vox-finding-001");
        assert_eq!(json["cope_compliant"], true);
    }

    #[test]
    fn retraction_workflow_requires_reason_text() {
        let notice = CopeRetractionNotice::new(
            "10.1234/test".into(),
            CopeRetractionReason::AnalysisError,
            "Statistical analysis used wrong baseline.".into(),
            "Author A".into(),
        );
        assert!(notice.notice_text.contains("Statistical analysis"));
    }

    #[test]
    fn retraction_reason_display() {
        assert_eq!(
            CopeRetractionReason::EthicsViolation.as_str(),
            "ethics_violation"
        );
        assert_eq!(CopeRetractionReason::Superseded.as_str(), "superseded");
    }

    #[test]
    fn retraction_propagation_state() {
        let mut notice = CopeRetractionNotice::new(
            "10.1234/test".into(),
            CopeRetractionReason::DataError,
            "Data error found.".into(),
            "Author".into(),
        );
        assert!(!notice.crossref_notified);
        notice.mark_crossref_notified();
        assert!(notice.crossref_notified);
    }
}
