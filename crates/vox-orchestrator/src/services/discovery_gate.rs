use std::sync::Arc;
use vox_db::StoreError;
use vox_db::{VoxDb, VoxWriteHandle};

/// DiscoveryGate service handles the transition of agent-discovered insights
/// into verified scholarly artifacts (Schola/Scientia).
///
/// It implements Wave 2 of the hardened persistence plan: automating the
/// manuscript life cycle and scholarly DOI deposition.
pub struct DiscoveryGate {
    db: Arc<VoxDb>,
    writer: VoxWriteHandle,
    // Future: add scholarly registry clients (DataCite, CrossRef)
}

use vox_db::facade::scientia::DiscoveryEntry;

impl DiscoveryGate {
    pub fn new(db: Arc<VoxDb>, writer: VoxWriteHandle) -> Self {
        Self { db, writer }
    }

    /// Scan for pending discoveries and promote them to the publication queue if valid.
    pub async fn process_pending_discoveries(&self) -> Result<(), StoreError> {
        let pending = self.db.query_pending_discoveries().await?;

        for discovery in pending {
            let (is_valid, reason) = self.validate_discovery(&discovery).await?;

            if is_valid {
                self.promote_to_publication(discovery).await?;
            } else {
                self.reject_discovery(discovery, &reason).await?;
            }
        }

        Ok(())
    }

    async fn validate_discovery(
        &self,
        entry: &DiscoveryEntry,
    ) -> Result<(bool, String), StoreError> {
        // Research proves that confidence_score < 0.6 is the first rejection gate (Task 2.5.1)
        if entry.confidence_score < 0.6 {
            return Ok((
                false,
                format!(
                    "Confidence score {:.2} below threshold (0.60)",
                    entry.confidence_score
                ),
            ));
        }

        // Future: add LLM-based slop filtering
        Ok((true, "Automated validation successful".to_string()))
    }

    async fn promote_to_publication(&self, entry: DiscoveryEntry) -> Result<(), StoreError> {
        let publication_id = format!("pub_{}", &entry.discovery_id);

        self.writer
            .insert_telemetry_flat(
                "discovery_gate".to_string(),
                "null".to_string(),
                None,
                entry.repository_id.clone(),
                "discovery_promoted".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(format!(
                    "{{\"discovery_id\":\"{}\",\"publication_id\":\"{}\"}}",
                    entry.discovery_id, publication_id
                )),
            )
            .await?;

        // 1. Mark as approved in Scientia DB
        self.db
            .approve_discovery(&entry.discovery_id, Some("Auto-promoted by DiscoveryGate"))
            .await?;

        // 2. Insert into publication queue for Wave 3 processing (DOI reservation)
        self.writer
            .insert_publication_queue(
                entry.discovery_id.clone(),
                publication_id,
                "doi_pending".to_string(),
            )
            .await?;

        Ok(())
    }

    async fn reject_discovery(
        &self,
        entry: DiscoveryEntry,
        reason: &str,
    ) -> Result<(), StoreError> {
        self.writer
            .insert_telemetry_flat(
                "discovery_gate".to_string(),
                "null".to_string(),
                None,
                entry.repository_id.clone(),
                "discovery_rejected".to_string(),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                Some(format!(
                    "{{\"discovery_id\":\"{}\",\"reason\":\"{}\"}}",
                    entry.discovery_id, reason
                )),
            )
            .await?;

        self.db
            .reject_discovery(&entry.discovery_id, reason)
            .await?;
        Ok(())
    }
}

// DiscoveryEntry is imported from vox_db::facade::scientia
