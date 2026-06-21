//! VoxDb methods for the `model_prompt_profiles` table.

use crate::{StoreError, VoxDb};
use turso::params;

/// Raw DB row from `model_prompt_profiles`.
#[derive(Debug, Clone)]
pub struct ModelPromptProfileRow {
    pub prompt_profile_key: String,
    pub variant_id: String,
    pub preamble_text: String,
    /// Snake-case confidence string: "provisional" | "shadowed" | "confirmed" | "deprecated".
    pub confidence: String,
    pub quality_delta: f64,
    pub applications: i64,
    pub created_at_ms: i64,
    pub approved_by: Option<String>,
}

impl VoxDb {
    /// Load all rows from `model_prompt_profiles`, ordered by `created_at_ms`.
    pub async fn query_model_prompt_profiles(
        &self,
    ) -> Result<Vec<ModelPromptProfileRow>, StoreError> {
        let mut rows = self
            .conn
            .query(
                "SELECT prompt_profile_key, variant_id, preamble_text, confidence, \
                 quality_delta, applications, created_at_ms, approved_by \
                 FROM model_prompt_profiles ORDER BY created_at_ms ASC",
                (),
            )
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

        let mut out = Vec::new();
        while let Some(row) = rows
            .next()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?
        {
            out.push(ModelPromptProfileRow {
                prompt_profile_key: row.get(0).map_err(|e| StoreError::Db(e.to_string()))?,
                variant_id: row.get(1).map_err(|e| StoreError::Db(e.to_string()))?,
                preamble_text: row.get(2).map_err(|e| StoreError::Db(e.to_string()))?,
                confidence: row.get(3).map_err(|e| StoreError::Db(e.to_string()))?,
                quality_delta: row.get(4).map_err(|e| StoreError::Db(e.to_string()))?,
                applications: row.get(5).map_err(|e| StoreError::Db(e.to_string()))?,
                created_at_ms: row.get(6).map_err(|e| StoreError::Db(e.to_string()))?,
                approved_by: row.get(7).ok(),
            });
        }
        Ok(out)
    }

    /// Insert or replace a row in `model_prompt_profiles`.
    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_model_prompt_profile(
        &self,
        prompt_profile_key: &str,
        variant_id: &str,
        preamble_text: &str,
        confidence: &str,
        quality_delta: f64,
        applications: i64,
        created_at_ms: i64,
        approved_by: Option<&str>,
    ) -> Result<(), StoreError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO model_prompt_profiles \
                 (prompt_profile_key, variant_id, preamble_text, confidence, \
                  quality_delta, applications, created_at_ms, approved_by) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    prompt_profile_key,
                    variant_id,
                    preamble_text,
                    confidence,
                    quality_delta,
                    applications,
                    created_at_ms,
                    approved_by,
                ],
            )
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }
}
