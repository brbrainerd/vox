//! Information-theoretic questioning telemetry and dual-write persistence helpers.
//!
//! `metric_type = questioning_event` rows are **S1–S2**: KPIs and session-scoped metadata for scientia / questioning loops.
//! They are **not** a substitute for content classification of publication bodies or chat transcripts
//! (see `docs/src/architecture/telemetry-retention-sensitivity-ssot.md`).

use crate::VoxDb;
use crate::research_metrics_contract::METRIC_TYPE_QUESTIONING_EVENT;
use crate::store::StoreError;
use crate::store::types::PublicationManifestParams;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};

/// Canonical artifact payload persisted to publication + searchable document stores.
#[derive(Debug, Clone)]
pub struct QuestioningResearchArtifact<'a> {
    pub publication_id: &'a str,
    pub source_ref: Option<&'a str>,
    pub title: &'a str,
    pub author: &'a str,
    pub abstract_text: Option<&'a str>,
    pub body_markdown: &'a str,
    pub citations_json: Option<&'a str>,
    pub metadata_json: Option<&'a str>,
    pub state: &'a str,
}

/// Aggregated KPI snapshot for questioning behavior.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct QuestioningKpiSnapshot {
    pub sample_size: usize,
    pub clarification_turns: usize,
    pub selected_option_count: usize,
    pub unselected_option_count: usize,
    pub mean_expected_information_gain_bits: f64,
    pub mean_realized_information_bits: f64,
    pub mean_expected_user_cost: f64,
}

impl VoxDb {
    /// Append one row to `research_metrics` under `metric_type = questioning_event`.
    pub async fn record_questioning_metric(
        &self,
        session_id: &str,
        metric_value: Option<f64>,
        metadata_json: &str,
    ) -> Result<i64, StoreError> {
        self.append_research_metric(
            session_id,
            METRIC_TYPE_QUESTIONING_EVENT,
            metric_value,
            Some(metadata_json),
        )
        .await
    }

    /// Persist the same questioning artifact into publication and searchable document stores.
    ///
    /// Returns `(publication_digest, search_document_id)`.
    pub async fn persist_questioning_research_artifact_dual_write(
        &self,
        artifact: QuestioningResearchArtifact<'_>,
    ) -> Result<(String, i64), StoreError> {
        let mut hasher = Sha3_256::new();
        hasher.update(artifact.body_markdown.as_bytes());
        let content_sha3_256 = hex::encode(hasher.finalize());

        self.upsert_publication_manifest(PublicationManifestParams {
            publication_id: artifact.publication_id,
            content_type: "questioning_policy",
            source_ref: artifact.source_ref,
            title: artifact.title,
            author: artifact.author,
            abstract_text: artifact.abstract_text,
            body_markdown: artifact.body_markdown,
            citations_json: artifact.citations_json,
            metadata_json: artifact.metadata_json,
            revision_history_json: None,
            content_sha3_256: &content_sha3_256,
            state: artifact.state,
        })
        .await?;

        let source_uri = format!("vox://publication/{}", artifact.publication_id);
        let doc_id = self
            .upsert_search_document(
                &source_uri,
                artifact.title,
                "text/markdown",
                &content_sha3_256,
            )
            .await?;
        let chunks = split_markdown_chunks(artifact.body_markdown, 1200);
        self.replace_search_document_chunks(doc_id, &chunks).await?;

        Ok((content_sha3_256, doc_id))
    }

    /// Roll up basic questioning KPIs from persisted question tables.
    pub async fn aggregate_questioning_kpis(
        &self,
        session_id: Option<&str>,
        limit: i64,
    ) -> Result<QuestioningKpiSnapshot, StoreError> {
        let lim = limit.clamp(1, 50_000);
        let rows = if let Some(sid) = session_id {
            self.query_all(
                "SELECT q.expected_information_gain_bits, q.expected_user_cost
                 FROM question_events q
                 JOIN question_sessions s ON s.id = q.question_session_id
                 WHERE s.session_id = ?1
                 ORDER BY q.id DESC LIMIT ?2",
                (sid.to_string(), lim),
            )
            .await?
        } else {
            self.query_all(
                "SELECT expected_information_gain_bits, expected_user_cost
                 FROM question_events
                 ORDER BY id DESC LIMIT ?1",
                (lim,),
            )
            .await?
        };

        let mut out = QuestioningKpiSnapshot {
            sample_size: rows.len(),
            clarification_turns: rows.len(),
            ..QuestioningKpiSnapshot::default()
        };
        let mut sum_gain = 0.0_f64;
        let mut sum_cost = 0.0_f64;
        for row in rows {
            let gain: f64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            let cost: f64 = row.get(1).map_err(|e| StoreError::Db(e.to_string()))?;
            sum_gain += gain;
            sum_cost += cost;
        }
        if out.sample_size > 0 {
            out.mean_expected_information_gain_bits = sum_gain / out.sample_size as f64;
            out.mean_expected_user_cost = sum_cost / out.sample_size as f64;
        }

        let selected_rows = if let Some(sid) = session_id {
            self.query_all(
                "SELECT o.selected
                 FROM question_option_outcomes o
                 JOIN question_events q ON q.id = o.question_event_id
                 JOIN question_sessions s ON s.id = q.question_session_id
                 WHERE s.session_id = ?1
                 ORDER BY o.id DESC LIMIT ?2",
                (sid.to_string(), lim),
            )
            .await?
        } else {
            self.query_all(
                "SELECT selected FROM question_option_outcomes ORDER BY id DESC LIMIT ?1",
                (lim,),
            )
            .await?
        };
        for row in selected_rows {
            let selected: i64 = row.get(0).map_err(|e| StoreError::Db(e.to_string()))?;
            if selected != 0 {
                out.selected_option_count += 1;
            } else {
                out.unselected_option_count += 1;
            }
        }
        Ok(out)
    }
}

fn split_markdown_chunks(body: &str, max_chars: usize) -> Vec<String> {
    if body.trim().is_empty() {
        return vec!["(empty)".to_string()];
    }
    let mut out = Vec::new();
    let mut current = String::new();
    for para in body.split("\n\n") {
        if current.is_empty() {
            current.push_str(para);
            continue;
        }
        if current.len() + 2 + para.len() <= max_chars {
            current.push_str("\n\n");
            current.push_str(para);
        } else {
            out.push(current);
            current = para.to_string();
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

#[cfg(all(test, feature = "local"))]
mod tests {
    use super::*;
    use crate::DbConfig;

    #[tokio::test]
    async fn dual_write_persists_publication_and_search_copy() {
        let db = VoxDb::connect(DbConfig::Memory).await.expect("memory");
        let artifact = QuestioningResearchArtifact {
            publication_id: "questioning-ssot",
            source_ref: Some("docs/src/reference/information-theoretic-questioning.md"),
            title: "Questioning SSOT",
            author: "vox",
            abstract_text: Some("short"),
            body_markdown: "# Heading\n\nBody text.",
            citations_json: None,
            metadata_json: Some(r#"{"kind":"questioning"}"#),
            state: "draft",
        };
        let (_digest, doc_id) = db
            .persist_questioning_research_artifact_dual_write(artifact)
            .await
            .expect("dual write");
        let manifest = db
            .get_publication_manifest("questioning-ssot")
            .await
            .expect("get manifest");
        assert!(manifest.is_some(), "manifest missing");
        let chunks = db
            .query_search_document_chunks("Body", 10)
            .await
            .expect("query chunks");
        assert!(
            chunks.iter().any(|(_, d, _, _)| *d == doc_id),
            "search chunks missing for doc id {doc_id}"
        );
    }
}
