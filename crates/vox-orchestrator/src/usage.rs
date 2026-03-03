//! Usage tracking for AI provider rate limits and cost accounting.
//!
//! Persists daily usage counters to VoxDB, enabling:
//! - Proactive routing away from exhausted providers
//! - Cost tracking for paid models
//! - Shared budget awareness across agents (via Turso sync)

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Known daily rate limits per provider (free tier, March 2026).
const LIMITS: &[ProviderLimit] = &[
    ProviderLimit { provider: "google", model: "gemini-2.0-flash-lite", daily_limit: 1000 },
    ProviderLimit { provider: "google", model: "gemini-2.5-flash-preview", daily_limit: 250 },
    ProviderLimit { provider: "google", model: "gemini-2.5-pro", daily_limit: 100 },
    ProviderLimit { provider: "openrouter", model: ":free", daily_limit: 50 },
    ProviderLimit { provider: "ollama", model: "*", daily_limit: u32::MAX },
];

struct ProviderLimit {
    provider: &'static str,
    model: &'static str,
    daily_limit: u32,
}

/// A single day's usage record for a provider+model pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    pub user_id: String,
    pub provider: String,
    pub model: String,
    pub date: String,
    pub calls: u32,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cost_usd: f64,
    pub is_rate_limited: bool,
    pub last_429: Option<i64>,
}

/// Remaining budget for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemainingBudget {
    pub provider: String,
    pub model: String,
    pub calls_used: u32,
    pub daily_limit: u32,
    pub remaining: u32,
    pub cost_today: f64,
    pub rate_limited: bool,
}

/// Provider recommendation with reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRecommendation {
    pub provider: String,
    pub model: String,
    pub remaining: u32,
    pub reason: String,
}

/// Cost summary over a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    pub total_calls: u32,
    pub total_cost_usd: f64,
    pub by_provider: Vec<ProviderCost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCost {
    pub provider: String,
    pub calls: u32,
    pub cost_usd: f64,
}

/// Tracks API usage per provider in VoxDB.
pub struct UsageTracker<'a> {
    db: &'a vox_db::VoxDb,
    pub user_id: String,
}

impl<'a> UsageTracker<'a> {
    /// Create a new tracker backed by a borrowed VoxDB instance.
    pub fn new_ref(db: &'a vox_db::VoxDb) -> Self {
        Self { db, user_id: "global".to_string() }
    }

    pub fn with_user(db: &'a vox_db::VoxDb, user_id: &str) -> Self {
        Self { db, user_id: user_id.to_string() }
    }

    fn today() -> String {
        let secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (secs / 86400) as i64;
        let z = days + 719468;
        let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
        let doe = (z - era * 146097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{:04}-{:02}-{:02}", y, m, d)
    }

    /// Record a successful API call.
    pub async fn record_call(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let date = Self::today();
        let filter = json!({
            "user_id": self.user_id,
            "provider": provider,
            "model": model,
            "date": date
        });

        let existing = col.find(&filter).await?;
        if let Some((id, doc)) = existing.into_iter().next() {
            let calls = doc["calls"].as_u64().unwrap_or(0) + 1;
            let tin = doc["tokens_in"].as_u64().unwrap_or(0) + tokens_in;
            let tout = doc["tokens_out"].as_u64().unwrap_or(0) + tokens_out;
            let cost = doc["cost_usd"].as_f64().unwrap_or(0.0) + cost_usd;
            col.patch(id, &json!({
                "calls": calls,
                "tokens_in": tin,
                "tokens_out": tout,
                "cost_usd": cost,
            })).await?;
        } else {
            col.insert(&json!({
                "user_id": self.user_id,
                "provider": provider,
                "model": model,
                "date": date,
                "calls": 1u32,
                "tokens_in": tokens_in,
                "tokens_out": tokens_out,
                "cost_usd": cost_usd,
                "is_rate_limited": false,
                "last_429": Value::Null,
            })).await?;
        }
        Ok(())
    }

    /// Mark a provider as rate-limited right now.
    pub async fn mark_rate_limited(
        &self,
        provider: &str,
        model: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let date = Self::today();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let filter = json!({
            "user_id": self.user_id,
            "provider": provider,
            "model": model,
            "date": date
        });

        let existing = col.find(&filter).await?;
        if let Some((id, _)) = existing.into_iter().next() {
            col.patch(id, &json!({"is_rate_limited": true, "last_429": now})).await?;
        } else {
            col.insert(&json!({
                "user_id": self.user_id,
                "provider": provider,
                "model": model,
                "date": date,
                "calls": 0u32,
                "tokens_in": 0u64,
                "tokens_out": 0u64,
                "cost_usd": 0.0,
                "is_rate_limited": true,
                "last_429": now,
            })).await?;
        }
        Ok(())
    }

    /// Get calls made today for a specific provider.
    pub async fn get_calls_today(
        &self,
        provider: &str,
        model: Option<&str>,
    ) -> Result<u32, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let date = Self::today();
        let mut filter = json!({
            "user_id": self.user_id,
            "provider": provider,
            "date": date
        });
        if let Some(m) = model {
            filter["model"] = json!(m);
        }

        let records = col.find(&filter).await?;
        let mut total = 0u32;
        for (_id, doc) in records {
            total += doc["calls"].as_u64().unwrap_or(0) as u32;
        }
        Ok(total)
    }

    /// Get remaining budget for all known providers today.
    pub async fn remaining_all(&self) -> Result<Vec<RemainingBudget>, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let date = Self::today();
        let mut results = Vec::new();

        for limit in LIMITS {
            let filter = json!({
                "user_id": self.user_id,
                "provider": limit.provider,
                "model": limit.model,
                "date": date
            });
            let records = col.find(&filter).await?;

            let mut total_calls = 0u32;
            let mut total_cost = 0.0f64;
            let mut was_rate_limited = false;

            for (_id, doc) in &records {
                total_calls += doc["calls"].as_u64().unwrap_or(0) as u32;
                total_cost += doc["cost_usd"].as_f64().unwrap_or(0.0);
                if doc["is_rate_limited"].as_bool().unwrap_or(false) {
                    was_rate_limited = true;
                }
            }

            let remaining = limit.daily_limit.saturating_sub(total_calls);
            results.push(RemainingBudget {
                provider: limit.provider.to_string(),
                model: limit.model.to_string(),
                calls_used: total_calls,
                daily_limit: limit.daily_limit,
                remaining,
                cost_today: total_cost,
                rate_limited: was_rate_limited,
            });
        }
        Ok(results)
    }

    /// Pick the best provider that still has free budget.
    pub async fn best_available_provider(
        &self,
        has_google_key: bool,
        has_openrouter_key: bool,
        has_ollama: bool,
    ) -> Result<ProviderRecommendation, Box<dyn std::error::Error + Send + Sync>> {
        let budgets = self.remaining_all().await?;

        let mut candidates: Vec<(&RemainingBudget, bool)> = Vec::new();
        for b in &budgets {
            let has_key = match b.provider.as_str() {
                "google" => has_google_key,
                "openrouter" => has_openrouter_key,
                "ollama" => has_ollama,
                _ => false,
            };
            if has_key && b.remaining > 0 && !b.rate_limited {
                candidates.push((b, has_key));
            }
        }

        candidates.sort_by(|a, b| b.0.remaining.cmp(&a.0.remaining));

        if let Some((best, _)) = candidates.first() {
            let default_model = match best.provider.as_str() {
                "google" => "gemini-2.0-flash-lite",
                "openrouter" => "mistral/devstral-2-2512:free",
                "ollama" => "llama3.2",
                _ => "unknown",
            };
            Ok(ProviderRecommendation {
                provider: best.provider.clone(),
                model: default_model.to_string(),
                remaining: best.remaining,
                reason: format!("{} calls remaining today", best.remaining),
            })
        } else if has_ollama {
            Ok(ProviderRecommendation {
                provider: "ollama".to_string(),
                model: "llama3.2".to_string(),
                remaining: u32::MAX,
                reason: "local inference, unlimited".to_string(),
            })
        } else {
            Ok(ProviderRecommendation {
                provider: "none".to_string(),
                model: "none".to_string(),
                remaining: 0,
                reason: "all providers exhausted or unavailable".to_string(),
            })
        }
    }

    /// Cost summary for today.
    pub async fn cost_summary_today(&self) -> Result<CostSummary, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let date = Self::today();
        let filter = json!({
            "user_id": self.user_id,
            "date": date
        });
        let records = col.find(&filter).await?;

        let mut by_provider: std::collections::HashMap<String, ProviderCost> =
            std::collections::HashMap::new();

        for (_id, doc) in &records {
            let provider = doc["provider"].as_str().unwrap_or("unknown").to_string();
            let calls = doc["calls"].as_u64().unwrap_or(0) as u32;
            let cost = doc["cost_usd"].as_f64().unwrap_or(0.0);

            let entry = by_provider.entry(provider.clone()).or_insert(ProviderCost {
                provider,
                calls: 0,
                cost_usd: 0.0,
            });
            entry.calls += calls;
            entry.cost_usd += cost;
        }

        let total_calls: u32 = by_provider.values().map(|p| p.calls).sum();
        let total_cost: f64 = by_provider.values().map(|p| p.cost_usd).sum();

        Ok(CostSummary {
            total_calls,
            total_cost_usd: total_cost,
            by_provider: by_provider.into_values().collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_limits_cover_all_providers() {
        let providers: Vec<&str> = LIMITS.iter().map(|l| l.provider).collect();
        assert!(providers.contains(&"google"));
        assert!(providers.contains(&"openrouter"));
        assert!(providers.contains(&"ollama"));
    }

    #[test]
    fn today_returns_valid_date() {
        let date = UsageTracker::today();
        assert_eq!(date.len(), 10); // YYYY-MM-DD
        assert!(date.starts_with("20")); // 21st century
    }

    #[test]
    fn usage_record_serializes() {
        let record = UsageRecord {
            user_id: "test-user".to_string(),
            provider: "google".to_string(),
            model: "gemini-2.5-flash-preview".to_string(),
            date: "2026-03-02".to_string(),
            calls: 42,
            tokens_in: 1200,
            tokens_out: 800,
            cost_usd: 0.0,
            is_rate_limited: false,
            last_429: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("gemini-2.5-flash-preview"));
        assert!(json.contains("42"));
    }
}
