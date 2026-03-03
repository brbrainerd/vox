use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackInteraction {
    pub prompt: String,
    pub response: String,
    pub score: f32,
    pub timestamp_ms: u64,
}

/// Configuration for response scoring.
#[derive(Debug, Clone)]
pub struct ScoringConfig {
    /// Base score assigned to all responses before adjustments.
    pub base_score: f32,
    /// Bonus applied if the response is non-empty.
    pub non_empty_bonus: f32,
    /// Bonus per 100 chars of response (up to a cap).
    pub length_bonus_per_100: f32,
    /// Maximum total score.
    pub max_score: f32,
}

impl Default for ScoringConfig {
    fn default() -> Self {
        Self {
            base_score: 0.5,
            non_empty_bonus: 0.1,
            length_bonus_per_100: 0.05,
            max_score: 1.0,
        }
    }
}

pub struct FeedbackCollector {
    pub interactions: Vec<FeedbackInteraction>,
    pub scoring: ScoringConfig,
}

impl Default for FeedbackCollector {
    fn default() -> Self {
        Self::new()
    }
}

impl FeedbackCollector {
    pub fn new() -> Self {
        Self {
            interactions: Vec::new(),
            scoring: ScoringConfig::default(),
        }
    }

    pub fn with_scoring(scoring: ScoringConfig) -> Self {
        Self {
            interactions: Vec::new(),
            scoring,
        }
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    pub fn log(&mut self, prompt: String, response: String) {
        let score = self.score_response(&response);
        let timestamp_ms = Self::now_ms();
        self.interactions.push(FeedbackInteraction {
            prompt,
            response,
            score,
            timestamp_ms,
        });
    }

    /// Log with an explicit user-provided score (e.g. from thumbs up/down).
    pub fn log_with_score(&mut self, prompt: String, response: String, score: f32) {
        let timestamp_ms = Self::now_ms();
        self.interactions.push(FeedbackInteraction {
            prompt,
            response,
            score,
            timestamp_ms,
        });
    }

    /// Compute a heuristic quality score for a response.
    fn score_response(&self, response: &str) -> f32 {
        let cfg = &self.scoring;
        let mut score = cfg.base_score;
        if !response.is_empty() {
            score += cfg.non_empty_bonus;
        }
        // Length bonus: reward longer, more detailed responses
        let len_hundreds = (response.len() as f32 / 100.0).min(5.0);
        score += len_hundreds * cfg.length_bonus_per_100;
        score.min(cfg.max_score)
    }

    /// Export interactions as JSONL using serde for correct serialization.
    pub fn export_jsonl(&self) -> String {
        self.interactions
            .iter()
            .filter_map(|i| serde_json::to_string(i).ok())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get aggregate statistics.
    pub fn stats(&self) -> FeedbackStats {
        let count = self.interactions.len();
        if count == 0 {
            return FeedbackStats {
                count: 0,
                avg_score: 0.0,
                min_score: 0.0,
                max_score: 0.0,
            };
        }
        let scores: Vec<f32> = self.interactions.iter().map(|i| i.score).collect();
        let sum: f32 = scores.iter().sum();
        FeedbackStats {
            count,
            avg_score: sum / count as f32,
            min_score: scores.iter().cloned().fold(f32::MAX, f32::min),
            max_score: scores.iter().cloned().fold(f32::MIN, f32::max),
        }
    }

    /// Persist all un-flushed interactions to the CodeStore.
    /// Logs each interaction to `llm_interactions` and its score to `llm_feedback`.
    /// Returns the number of interactions persisted.
    pub async fn persist_to_store(
        &mut self,
        store: &crate::store::CodeStore,
    ) -> Result<usize, crate::store::StoreError> {
        let count = self.interactions.len();
        for interaction in self.interactions.drain(..) {
            // Log the interaction (session_id, user_id, prompt, response, model_version, latency_ms, token_count)
            let session_id = format!("feedback-{}", interaction.timestamp_ms);
            let interaction_id = store
                .log_interaction(
                    &session_id,
                    None, // user_id
                    &interaction.prompt,
                    &interaction.response,
                    "feedback-collector", // model_version
                    None,                 // latency_ms
                    None,                 // token_count
                )
                .await?;

            // Log the feedback score
            let rating = Some((interaction.score * 5.0).round() as i64);
            let feedback_type = if interaction.score >= 0.7 {
                "positive"
            } else if interaction.score >= 0.4 {
                "neutral"
            } else {
                "negative"
            };
            store
                .submit_feedback(
                    interaction_id,
                    None, // user_id
                    rating,
                    feedback_type,
                    None, // correction_text
                    None, // preferred_response
                )
                .await?;
        }
        Ok(count)
    }

    /// Load interactions from the CodeStore's training data.
    pub async fn load_from_store(
        store: &crate::store::CodeStore,
        limit: i64,
    ) -> Result<Vec<FeedbackInteraction>, crate::store::StoreError> {
        let pairs = store.get_training_data(limit).await?;
        Ok(pairs
            .into_iter()
            .map(|p| FeedbackInteraction {
                prompt: p.prompt,
                response: p.response,
                score: p.rating.map(|r| r as f32 / 5.0).unwrap_or(0.5),
                timestamp_ms: 0, // not stored in training_data view
            })
            .collect())
    }
}

#[derive(Debug, Clone)]
pub struct FeedbackStats {
    pub count: usize,
    pub avg_score: f32,
    pub min_score: f32,
    pub max_score: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_sets_timestamp() {
        let mut collector = FeedbackCollector::new();
        collector.log("hello".into(), "world".into());
        assert!(collector.interactions[0].timestamp_ms > 0);
    }

    #[test]
    fn test_score_is_dynamic() {
        let mut collector = FeedbackCollector::new();
        collector.log("p".into(), "".into());
        collector.log("p".into(), "a short response".into());
        collector.log("p".into(), "a".repeat(500));
        // Empty response should score lower than non-empty
        assert!(collector.interactions[0].score < collector.interactions[1].score);
        // Longer response should score higher
        assert!(collector.interactions[1].score < collector.interactions[2].score);
    }

    #[test]
    fn test_export_jsonl_valid_json() {
        let mut collector = FeedbackCollector::new();
        collector.log("say \"hi\"".into(), "he said \"hello\"".into());
        let jsonl = collector.export_jsonl();
        // Each line must be valid JSON
        for line in jsonl.lines() {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.get("prompt").is_some());
            assert!(parsed.get("response").is_some());
            assert!(parsed.get("score").is_some());
            assert!(parsed.get("timestamp_ms").is_some());
        }
    }

    #[test]
    fn test_log_with_explicit_score() {
        let mut collector = FeedbackCollector::new();
        collector.log_with_score("p".into(), "r".into(), 0.95);
        assert!((collector.interactions[0].score - 0.95).abs() < f32::EPSILON);
    }

    #[test]
    fn test_stats() {
        let mut collector = FeedbackCollector::new();
        collector.log_with_score("a".into(), "b".into(), 0.5);
        collector.log_with_score("c".into(), "d".into(), 1.0);
        let stats = collector.stats();
        assert_eq!(stats.count, 2);
        assert!((stats.avg_score - 0.75).abs() < f32::EPSILON);
    }
}
