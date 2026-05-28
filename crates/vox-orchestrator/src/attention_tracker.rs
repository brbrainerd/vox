//! VoxDB persistence layer for attention events and agent trust scores (Phase 15).
//!
//! CRUD follows the same collection-based JSON pattern as
//! [`UsageTracker`](crate::usage::UsageTracker): `col.find()` / `col.insert()` / `col.patch()`.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::attention::{AgentTrustScore, AttentionEvent, TrustTier};
use crate::types::AgentId;

/// Summary returned by [`AttentionTracker::session_summary()`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttentionSessionSummary {
    pub total_spent_ms: u64,
    pub total_requests: u32,
    pub auto_approved: u32,
    pub rejected: u32,
    pub efficiency: f64,
    pub auto_approve_ratio: f64,
    pub max_offender: Option<(u64, u64)>,
}

/// Persists attention metrics to Arca (VoxDB) collections.
pub struct AttentionTracker<'a> {
    db: &'a vox_db::VoxDb,
}

impl<'a> AttentionTracker<'a> {
    pub fn new(db: &'a vox_db::VoxDb) -> Self {
        Self { db }
    }

    // ── attention_events: INSERT ───────────────────────────────────────────

    /// Record a single attention event. Append-only log.
    pub async fn record_event(
        &self,
        event: &AttentionEvent,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("attention_events");
        col.ensure_table().await?;
        col.insert(&json!({
            "agent_id": event.agent_id.0,
            "task_id": event.task_id.map(|t| t.0),
            "event_type": serde_json::to_value(event.event_type)?,
            "tier": serde_json::to_value(event.tier)?,
            "cost_ms": event.cost_ms,
            "outcome": serde_json::to_value(event.outcome)?,
            "trust_score_at_time": event.trust_score_at_time,
            "effective_complexity": event.effective_complexity,
            "decision_entropy_bits": event.decision_entropy_bits,
            "timestamp_ms": event.timestamp_ms,
            "channel": event.channel,
            "policy_reason": event.policy_reason,
        }))
        .await?;
        Ok(())
    }

    // ── attention_events: QUERY (aggregate) ───────────────────────────────

    /// Retrieve the most recent attention events up to a given limit.
    pub async fn list_events(
        &self,
        limit: u32,
    ) -> Result<Vec<AttentionEvent>, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("attention_events");
        col.ensure_table().await?;
        let all = col.find(&json!({})).await?;

        let mut events = Vec::new();
        for (_id, doc) in all {
            if let Ok(ev) = serde_json::from_value::<AttentionEvent>(doc) {
                events.push(ev);
            }
        }
        events.sort_by_key(|e| std::cmp::Reverse(e.timestamp_ms));
        events.truncate(limit as usize);
        Ok(events)
    }

    /// Session summary: total spent, efficiency, auto-approve ratio.
    pub async fn session_summary(
        &self,
        since_ms: u64,
    ) -> Result<AttentionSessionSummary, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("attention_events");
        col.ensure_table().await?;
        let all = col.find(&json!({})).await?;

        let mut total_cost = 0u64;
        let mut total = 0u32;
        let mut auto = 0u32;
        let mut rejected = 0u32;
        let mut agent_costs = std::collections::HashMap::new();

        for (_id, doc) in &all {
            let ts = doc["timestamp_ms"].as_u64().unwrap_or(0);
            if ts < since_ms {
                continue;
            }
            total += 1;
            let cost = doc["cost_ms"].as_u64().unwrap_or(0);
            total_cost += cost;
            if let Some(aid) = doc["agent_id"].as_u64() {
                *agent_costs.entry(aid).or_insert(0) += cost;
            }
            // Deserialize outcome via serde to avoid magic string comparison.
            // If deserialization fails the record is malformed; skip it gracefully.
            if let Ok(outcome) =
                serde_json::from_value::<crate::attention::ApprovalOutcome>(doc["outcome"].clone())
            {
                match outcome {
                    crate::attention::ApprovalOutcome::AutoApproved => auto += 1,
                    crate::attention::ApprovalOutcome::Rejected => rejected += 1,
                    _ => {}
                }
            }
        }

        Ok(AttentionSessionSummary {
            total_spent_ms: total_cost,
            total_requests: total,
            auto_approved: auto,
            rejected,
            efficiency: if total > 0 {
                1.0 - (rejected as f64 / total as f64)
            } else {
                1.0
            },
            auto_approve_ratio: if total > 0 {
                auto as f64 / total as f64
            } else {
                0.0
            },
            max_offender: agent_costs.into_iter().max_by_key(|&(_, c)| c),
        })
    }

    // ── agent_trust_scores: UPSERT ────────────────────────────────────────

    /// Persist or update the trust score for an agent.
    pub async fn upsert_trust(
        &self,
        trust: &AgentTrustScore,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("agent_trust_scores");
        col.ensure_table().await?;

        let filter = json!({ "agent_id": trust.agent_id.0 });
        let existing = col.find(&filter).await?;

        let payload = json!({
            "agent_id": trust.agent_id.0,
            "trust_score": trust.trust_score,
            "tier": serde_json::to_value(trust.tier)?,
            "total_outcomes": trust.total_outcomes,
            "successful_outcomes": trust.successful_outcomes,
            "below_tier_streak": trust.below_tier_streak,
            "last_updated_ms": trust.last_updated_ms,
            "is_override": trust.is_override,
        });

        if let Some((id, _)) = existing.into_iter().next() {
            col.patch(id, &payload).await?;
        } else {
            col.insert(&payload).await?;
        }
        Ok(())
    }

    // ── agent_trust_scores: READ ──────────────────────────────────────────

    /// Load trust score for a single agent.
    pub async fn load_trust(
        &self,
        agent_id: AgentId,
    ) -> Result<Option<AgentTrustScore>, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("agent_trust_scores");
        col.ensure_table().await?;

        let filter = json!({ "agent_id": agent_id.0 });
        let rows = col.find(&filter).await?;

        if let Some((_id, doc)) = rows.into_iter().next() {
            Ok(Some(AgentTrustScore {
                agent_id,
                trust_score: doc["trust_score"].as_f64().unwrap_or(0.5),
                tier: serde_json::from_value(doc["tier"].clone()).unwrap_or(TrustTier::Untrusted),
                total_outcomes: doc["total_outcomes"].as_u64().unwrap_or(0) as u32,
                successful_outcomes: doc["successful_outcomes"].as_u64().unwrap_or(0) as u32,
                below_tier_streak: doc["below_tier_streak"].as_u64().unwrap_or(0) as u32,
                last_updated_ms: doc["last_updated_ms"].as_u64().unwrap_or(0),
                // Variance not yet persisted in DB — hydrate from EB prior on load.
                variance: doc["variance"].as_f64().unwrap_or(0.10),
                is_override: doc["is_override"].as_bool().unwrap_or(false),
            }))
        } else {
            Ok(None)
        }
    }

    // ── agent_trust_scores: READ ALL ──────────────────────────────────────

    /// Load all trust scores keyed by AgentId (for routing injection).
    pub async fn load_all_trust(
        &self,
    ) -> Result<
        std::collections::HashMap<AgentId, AgentTrustScore>,
        Box<dyn std::error::Error + Send + Sync>,
    > {
        let col = self.db.collection("agent_trust_scores");
        col.ensure_table().await?;

        let rows = col.find(&json!({})).await?;
        let mut result = std::collections::HashMap::new();
        for (_id, doc) in rows {
            let aid = AgentId(doc["agent_id"].as_u64().unwrap_or(0));
            let ts = AgentTrustScore {
                agent_id: aid,
                trust_score: doc["trust_score"].as_f64().unwrap_or(0.5),
                tier: serde_json::from_value(doc["tier"].clone()).unwrap_or(TrustTier::Untrusted),
                total_outcomes: doc["total_outcomes"].as_u64().unwrap_or(0) as u32,
                successful_outcomes: doc["successful_outcomes"].as_u64().unwrap_or(0) as u32,
                below_tier_streak: doc["below_tier_streak"].as_u64().unwrap_or(0) as u32,
                last_updated_ms: doc["last_updated_ms"].as_u64().unwrap_or(0),
                variance: doc["variance"].as_f64().unwrap_or(0.10),
                is_override: doc["is_override"].as_bool().unwrap_or(false),
            };
            result.insert(aid, ts);
        }
        Ok(result)
    }
}
