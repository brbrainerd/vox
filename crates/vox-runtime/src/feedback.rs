//! # Feedback Collector
//!
//! Logs LLM interactions and user feedback to the Vox database for
//! RLHF training data collection and quality monitoring.
//!
//! ```no_run
//! use vox_runtime::feedback::FeedbackCollector;
//! use vox_pm::store::CodeStore;
//!
//! async fn example(store: CodeStore) {
//!     let collector = FeedbackCollector::new(store, "session-1", Some("alice".to_string()));
//!     let id = collector.log("What is Vox?", "Vox is an AI-native language.", 42, 150).await.unwrap();
//!     collector.thumbs_up(id).await.unwrap();
//! }
//! ```

use thiserror::Error;
use vox_pm::store::{CodeStore, StoreError, TrainingPair};

#[derive(Debug, Error)]
pub enum FeedbackError {
    #[error("Store error: {0}")]
    Store(#[from] StoreError),
    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Collects LLM interactions and feedback for RLHF training.
pub struct FeedbackCollector {
    store: CodeStore,
    session_id: String,
    user_id: Option<String>,
    model_version: String,
}

impl FeedbackCollector {
    /// Create a new feedback collector for a session.
    pub fn new(store: CodeStore, session_id: &str, user_id: Option<String>) -> Self {
        Self {
            store,
            session_id: session_id.to_string(),
            user_id,
            model_version: "populi-v1".to_string(),
        }
    }

    /// Set the model version string.
    pub fn with_model(mut self, model: &str) -> Self {
        self.model_version = model.to_string();
        self
    }

    /// Log an LLM interaction (prompt + response).
    pub async fn log(
        &self,
        prompt: &str,
        response: &str,
        token_count: i64,
        latency_ms: i64,
    ) -> Result<i64, FeedbackError> {
        let id = self
            .store
            .log_interaction(
                &self.session_id,
                self.user_id.as_deref(),
                prompt,
                response,
                &self.model_version,
                Some(latency_ms),
                Some(token_count),
            )
            .await?;
        Ok(id)
    }

    /// Submit a thumbs-up for an interaction.
    pub async fn thumbs_up(&self, interaction_id: i64) -> Result<i64, FeedbackError> {
        let id = self
            .store
            .submit_feedback(
                interaction_id,
                self.user_id.as_deref(),
                Some(1),
                "thumbs",
                None,
                None,
            )
            .await?;
        Ok(id)
    }

    /// Submit a thumbs-down for an interaction.
    pub async fn thumbs_down(&self, interaction_id: i64) -> Result<i64, FeedbackError> {
        let id = self
            .store
            .submit_feedback(
                interaction_id,
                self.user_id.as_deref(),
                Some(0),
                "thumbs",
                None,
                None,
            )
            .await?;
        Ok(id)
    }

    /// Submit a star rating (1-5) for an interaction.
    pub async fn rate(&self, interaction_id: i64, stars: i64) -> Result<i64, FeedbackError> {
        let id = self
            .store
            .submit_feedback(
                interaction_id,
                self.user_id.as_deref(),
                Some(stars),
                "rating",
                None,
                None,
            )
            .await?;
        Ok(id)
    }

    /// Submit a correction — user provides a better response.
    pub async fn correct(
        &self,
        interaction_id: i64,
        correction: &str,
    ) -> Result<i64, FeedbackError> {
        let id = self
            .store
            .submit_feedback(
                interaction_id,
                self.user_id.as_deref(),
                None,
                "correction",
                Some(correction),
                None,
            )
            .await?;
        Ok(id)
    }

    /// Submit a preference pair — user chose one response over another.
    pub async fn prefer(
        &self,
        interaction_id: i64,
        preferred_response: &str,
    ) -> Result<i64, FeedbackError> {
        let id = self
            .store
            .submit_feedback(
                interaction_id,
                self.user_id.as_deref(),
                None,
                "preference",
                None,
                Some(preferred_response),
            )
            .await?;
        Ok(id)
    }

    /// Export training pairs as JSONL for fine-tuning.
    pub async fn export_jsonl(&self, limit: i64) -> Result<String, FeedbackError> {
        let pairs = self.store.get_training_data(limit).await?;
        let lines: Vec<String> = pairs
            .iter()
            .map(|pair| {
                serde_json::to_string(&serde_json::json!({
                    "prompt": pair.prompt,
                    "chosen": pair.response,
                    "rejected": pair.correction.as_deref().unwrap_or(""),
                    "rating": pair.rating,
                    "feedback_type": pair.feedback_type,
                }))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(lines.join("\n"))
    }

    /// Get training data pairs directly.
    pub async fn get_training_data(&self, limit: i64) -> Result<Vec<TrainingPair>, FeedbackError> {
        Ok(self.store.get_training_data(limit).await?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires libsql `local` feature; run with `cargo test -p vox-runtime -- --ignored`
    async fn test_feedback_log_and_thumbs_up_round_trip() {
        // B-016: Integration test for FeedbackCollector::log + thumbs_up round-trip
        let path = std::env::temp_dir().join(format!(
            "vox_feedback_test_{}.db",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_micros()
        ));
        let store = CodeStore::open(path.to_str().unwrap())
            .await
            .expect("temp store");
        let collector = FeedbackCollector::new(store, "test-session", Some("user-1".into()));

        // Log an interaction
        let interaction_id = collector
            .log("What is Vox?", "Vox is an AI-native language.", 42, 150)
            .await
            .expect("log should succeed");

        assert!(interaction_id > 0, "Should return a valid interaction id");

        // Submit thumbs up
        let feedback_id = collector
            .thumbs_up(interaction_id)
            .await
            .expect("thumbs_up should succeed");

        assert!(feedback_id > 0, "Should return a valid feedback id");

        // Verify via training data export
        let training_data = collector
            .get_training_data(10)
            .await
            .expect("get_training_data should succeed");

        assert!(
            !training_data.is_empty(),
            "Should have at least one training pair after log + feedback"
        );
        assert_eq!(training_data[0].prompt, "What is Vox?");
        assert_eq!(training_data[0].response, "Vox is an AI-native language.");
    }
}
