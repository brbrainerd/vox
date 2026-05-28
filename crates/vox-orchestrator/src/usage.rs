//! Usage tracking for AI provider rate limits and cost accounting.
//!
//! Persists daily usage counters to VoxDB, enabling:
//! - Proactive routing away from exhausted providers
//! - Cost tracking for paid models
//! - Shared budget awareness across agents (via Turso sync)

use crate::usage_policy::resolve_provider_limits;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Default `retry_after_secs` when a provider row is marked rate-limited ([`BudgetGate`](crate::gate::BudgetGate)).
pub const DEFAULT_RATE_LIMIT_RETRY_SECS: u64 = 60;

/// Keys rows in `provider_usage` / `LIMITS` for gating and accounting (not the API model slug).
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct LlmUsageKey {
    /// Provider namespace matching `LIMITS` (`google`, `openrouter`, `ollama`).
    pub provider: String,
    /// Model id or aggregate bucket (`:free` for all OpenRouter free models, `*` for Ollama).
    pub model: String,
}

/// A single day's usage record for a provider+model pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageRecord {
    /// Billing partition (per-user or `global`).
    pub user_id: String,
    /// Provider slug stored in Codex.
    pub provider: String,
    /// Model id within the provider.
    pub model: String,
    /// UTC day key (`YYYY-MM-DD`) used for aggregation.
    pub date: String,
    /// Number of API calls on that day.
    pub calls: u32,
    /// Sum of prompt tokens for the day.
    pub tokens_in: u64,
    /// Sum of completion tokens for the day.
    pub tokens_out: u64,
    /// Running cost in USD for the day.
    pub cost_usd: f64,
    /// Last provider request id seen for this row (when available).
    #[serde(default)]
    pub provider_request_id: Option<String>,
    /// Last provider-reported cost seen (if the upstream API returns explicit billing).
    #[serde(default)]
    pub provider_reported_cost_usd: Option<f64>,
    /// Last estimated cost computed from model pricing.
    #[serde(default)]
    pub estimated_cost_usd: Option<f64>,
    /// Last reconciled cost used for budgeting/dashboarding.
    #[serde(default)]
    pub reconciled_cost_usd: Option<f64>,
    /// Cost source marker (`estimated`, `provider_reported`, `reconciled`).
    #[serde(default)]
    pub cost_source: Option<String>,
    /// True after a provider returns HTTP 429.
    pub is_rate_limited: bool,
    /// Unix seconds of the most recent 429, if any.
    pub last_429: Option<i64>,
}

/// Remaining budget for a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemainingBudget {
    /// Provider namespace.
    pub provider: String,
    /// Model id evaluated against `LIMITS`.
    pub model: String,
    /// Calls already consumed today.
    pub calls_used: u32,
    /// Configured daily ceiling from static tables.
    pub daily_limit: u32,
    /// `daily_limit.saturating_sub(calls_used)` unless rate limited.
    pub remaining: u32,
    /// Spend attributed to this pair today.
    pub cost_today: f64,
    /// Whether routing should deprioritize this pair.
    pub rate_limited: bool,
}

/// Provider recommendation with reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderRecommendation {
    /// Suggested provider to call next.
    pub provider: String,
    /// Suggested model id.
    pub model: String,
    /// Estimated calls left today.
    pub remaining: u32,
    /// Why this pair was chosen or skipped.
    pub reason: String,
}

/// Cost summary over a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostSummary {
    /// Calls summed across all providers in the window.
    pub total_calls: u32,
    /// Aggregate USD spend.
    pub total_cost_usd: f64,
    /// Per-provider breakdown rows.
    pub by_provider: Vec<ProviderCost>,
}

/// One row in [`CostSummary::by_provider`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCost {
    /// Provider slug.
    pub provider: String,
    /// Calls attributed to the provider.
    pub calls: u32,
    /// USD attributed to the provider.
    pub cost_usd: f64,
}

/// Per-model cost breakdown row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCost {
    /// Provider slug.
    pub provider: String,
    /// Model id as stored in usage rows.
    pub model: String,
    /// Calls attributed to this model.
    pub calls: u32,
    /// Tokens consumed (input + output).
    pub tokens: u64,
    /// USD attributed to this model.
    pub cost_usd: f64,
}

/// Per-task-category cost breakdown row.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryCost {
    /// Task category label (or `"unknown"` when not recorded).
    pub category: String,
    /// Calls attributed to this category.
    pub calls: u32,
    /// USD attributed to this category.
    pub cost_usd: f64,
}

/// Unified cost summary merging cloud and local inference spend.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnifiedCostSummary {
    /// Total calls across all providers.
    pub total_calls: u32,
    /// Total USD spend including local inference cost estimate.
    pub total_cost_usd: f64,
    /// Per-provider breakdown.
    pub by_provider: Vec<ProviderCost>,
    /// Per-model breakdown sorted by cost descending.
    pub by_model: Vec<ModelCost>,
    /// Number of days covered by this summary.
    pub days_covered: u32,
}

/// Tracks API usage per provider in VoxDB.
pub struct UsageTracker<'a> {
    db: &'a vox_db::VoxDb,
    /// Tenant key persisted with usage rows (`global` unless overridden).
    pub user_id: String,
}

impl<'a> UsageTracker<'a> {
    /// Create a new tracker backed by a borrowed VoxDB instance.
    pub fn new_ref(db: &'a vox_db::VoxDb) -> Self {
        Self {
            db,
            user_id: "global".to_string(),
        }
    }

    /// Same as [`Self::new_ref`] but scopes persisted rows to `user_id`.
    pub fn with_user(db: &'a vox_db::VoxDb, user_id: &str) -> Self {
        Self {
            db,
            user_id: user_id.to_string(),
        }
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
        self.record_call_detailed(
            provider,
            model,
            tokens_in,
            tokens_out,
            cost_usd,
            None,
            None,
            Some(cost_usd),
            None,
            Some("estimated"),
            None,
            None,
        )
        .await
    }

    /// Record a successful API call with detailed reconciliation metadata.
    #[allow(clippy::too_many_arguments)]
    pub async fn record_call_detailed(
        &self,
        provider: &str,
        model: &str,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        provider_request_id: Option<&str>,
        provider_reported_cost_usd: Option<f64>,
        estimated_cost_usd: Option<f64>,
        reconciled_cost_usd: Option<f64>,
        cost_source: Option<&str>,
        task_category: Option<&str>,
        agent_id: Option<&str>,
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

        static USAGE_MUTEX: std::sync::OnceLock<tokio::sync::Mutex<()>> =
            std::sync::OnceLock::new();
        let _guard = USAGE_MUTEX
            .get_or_init(|| tokio::sync::Mutex::new(()))
            .lock()
            .await;

        let existing = col.find(&filter).await?;
        if let Some((id, doc)) = existing.into_iter().next() {
            let calls = doc["calls"].as_u64().unwrap_or(0) + 1;
            let tin = doc["tokens_in"].as_u64().unwrap_or(0) + tokens_in;
            let tout = doc["tokens_out"].as_u64().unwrap_or(0) + tokens_out;
            let cost = doc["cost_usd"].as_f64().unwrap_or(0.0) + cost_usd;
            col.patch(
                id,
                &json!({
                    "calls": calls,
                    "tokens_in": tin,
                    "tokens_out": tout,
                    "cost_usd": cost,
                    "provider_request_id": provider_request_id,
                    "provider_reported_cost_usd": provider_reported_cost_usd,
                    "estimated_cost_usd": estimated_cost_usd,
                    "reconciled_cost_usd": reconciled_cost_usd,
                    "cost_source": cost_source,
                }),
            )
            .await?;
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
                "provider_request_id": provider_request_id,
                "provider_reported_cost_usd": provider_reported_cost_usd,
                "estimated_cost_usd": estimated_cost_usd,
                "reconciled_cost_usd": reconciled_cost_usd,
                "cost_source": cost_source,
                "is_rate_limited": false,
                "last_429": serde_json::Value::Null,
                "task_category": task_category.unwrap_or("unknown"),
                "agent_id": agent_id.unwrap_or("unknown"),
            }))
            .await?;
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
            col.patch(id, &json!({"is_rate_limited": true, "last_429": now}))
                .await?;
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
            }))
            .await?;
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
    pub async fn remaining_all(
        &self,
    ) -> Result<Vec<RemainingBudget>, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let date = Self::today();
        let mut results = Vec::new();
        let limits = resolve_provider_limits();

        fn model_limit_matches(limit_model: &str, usage_model: &str) -> bool {
            limit_model == usage_model
                || limit_model == "*"
                || (limit_model == ":free" && usage_model.ends_with(":free"))
        }

        for limit in &limits {
            let filter = json!({
                "user_id": self.user_id,
                "provider": limit.provider,
                "date": date
            });
            let records = col.find(&filter).await?;

            let mut total_calls = 0u32;
            let mut total_cost = 0.0f64;
            let mut was_rate_limited = false;

            for (_id, doc) in &records {
                let usage_model = doc["model"].as_str().unwrap_or("");
                if !model_limit_matches(&limit.model, usage_model) {
                    continue;
                }
                total_calls += doc["calls"].as_u64().unwrap_or(0) as u32;
                total_cost += doc["cost_usd"].as_f64().unwrap_or(0.0);
                if doc["is_rate_limited"].as_bool().unwrap_or(false) {
                    was_rate_limited = true;
                }
            }

            let remaining = limit.daily_limit.saturating_sub(total_calls);
            results.push(RemainingBudget {
                provider: limit.provider.clone(),
                model: limit.model.clone(),
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

        fn provider_has_key(
            provider: &str,
            has_google_key: bool,
            has_openrouter_key: bool,
            has_ollama: bool,
        ) -> bool {
            match provider {
                "google" => has_google_key,
                "openrouter" => has_openrouter_key,
                "ollama" => has_ollama,
                "groq" => {
                    vox_secrets::resolve_secret(vox_secrets::SecretId::GroqApiKey).is_present()
                }
                "cerebras" => {
                    vox_secrets::resolve_secret(vox_secrets::SecretId::CerebrasApiKey).is_present()
                }
                "mistral" => {
                    vox_secrets::resolve_secret(vox_secrets::SecretId::MistralApiKey).is_present()
                }
                "deepseek" => {
                    vox_secrets::resolve_secret(vox_secrets::SecretId::DeepSeekApiKey).is_present()
                }
                "sambanova" => {
                    vox_secrets::resolve_secret(vox_secrets::SecretId::SambaNovaApiKey).is_present()
                }
                "custom" => vox_secrets::resolve_secret(vox_secrets::SecretId::CustomOpenaiApiKey)
                    .is_present(),
                _ => false,
            }
        }

        let mut candidates: Vec<(&RemainingBudget, bool)> = Vec::new();
        for b in &budgets {
            let has_key =
                provider_has_key(&b.provider, has_google_key, has_openrouter_key, has_ollama);
            if has_key && b.remaining > 0 && !b.rate_limited {
                candidates.push((b, has_key));
            }
        }

        candidates.sort_by_key(|c| std::cmp::Reverse(c.0.remaining));

        if let Some((best, _)) = candidates.first() {
            let default_model = match best.provider.as_str() {
                "google" => "google/auto".to_string(),
                "openrouter" => vox_config::OPENROUTER_AUTO.to_string(),
                "ollama" => vox_secrets::resolve_secret(vox_secrets::SecretId::PopuliModel)
                    .expose()
                    .map(|s: &str| s.to_string())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "default-model".to_string()),
                _ => "provider-default".to_string(),
            };
            Ok(ProviderRecommendation {
                provider: best.provider.clone(),
                model: default_model,
                remaining: best.remaining,
                reason: format!("{} calls remaining today", best.remaining),
            })
        } else if has_ollama {
            Ok(ProviderRecommendation {
                provider: "ollama".to_string(),
                model: vox_secrets::resolve_secret(vox_secrets::SecretId::PopuliModel)
                    .expose()
                    .map(|s: &str| s.to_string())
                    .filter(|s| !s.trim().is_empty())
                    .unwrap_or_else(|| "default-model".to_string()),
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
    pub async fn cost_summary_today(
        &self,
    ) -> Result<CostSummary, Box<dyn std::error::Error + Send + Sync>> {
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

    /// Per-model cost breakdown for the last `since_days` days (1 = today only).
    pub async fn cost_by_model(
        &self,
        since_days: u32,
    ) -> Result<Vec<ModelCost>, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let dates = Self::last_n_days(since_days.max(1));
        let mut by_key: std::collections::HashMap<(String, String), ModelCost> =
            std::collections::HashMap::new();

        for date in &dates {
            let filter = json!({ "user_id": self.user_id, "date": date });
            let records = col.find(&filter).await?;
            for (_id, doc) in records {
                let provider = doc["provider"].as_str().unwrap_or("unknown").to_string();
                let model = doc["model"].as_str().unwrap_or("*").to_string();
                let calls = doc["calls"].as_u64().unwrap_or(0) as u32;
                let tin = doc["tokens_in"].as_u64().unwrap_or(0);
                let tout = doc["tokens_out"].as_u64().unwrap_or(0);
                let cost = doc["cost_usd"].as_f64().unwrap_or(0.0);
                let entry = by_key
                    .entry((provider.clone(), model.clone()))
                    .or_insert(ModelCost {
                        provider,
                        model,
                        calls: 0,
                        tokens: 0,
                        cost_usd: 0.0,
                    });
                entry.calls += calls;
                entry.tokens += tin + tout;
                entry.cost_usd += cost;
            }
        }

        let mut rows: Vec<ModelCost> = by_key.into_values().collect();
        rows.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd));
        Ok(rows)
    }

    /// Per-task-category cost breakdown for the last `since_days` days.
    ///
    /// Only rows that have a `task_category` field set (written by future orchestrator
    /// instrumentation or MCP dispatch context) are included in the breakdown; the rest
    /// fall into the `"unknown"` bucket.
    pub async fn cost_by_category(
        &self,
        since_days: u32,
    ) -> Result<Vec<CategoryCost>, Box<dyn std::error::Error + Send + Sync>> {
        let col = self.db.collection("provider_usage");
        col.ensure_table().await?;

        let dates = Self::last_n_days(since_days.max(1));
        let mut by_cat: std::collections::HashMap<String, CategoryCost> =
            std::collections::HashMap::new();

        for date in &dates {
            let filter = json!({ "user_id": self.user_id, "date": date });
            let records = col.find(&filter).await?;
            for (_id, doc) in records {
                let category = doc["task_category"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                let calls = doc["calls"].as_u64().unwrap_or(0) as u32;
                let cost = doc["cost_usd"].as_f64().unwrap_or(0.0);
                let entry = by_cat.entry(category.clone()).or_insert(CategoryCost {
                    category,
                    calls: 0,
                    cost_usd: 0.0,
                });
                entry.calls += calls;
                entry.cost_usd += cost;
            }
        }

        let mut rows: Vec<CategoryCost> = by_cat.into_values().collect();
        rows.sort_by(|a, b| b.cost_usd.total_cmp(&a.cost_usd));
        Ok(rows)
    }

    /// Unified cost summary merging cloud and local inference spend over `since_days` days.
    ///
    /// Populi Mesh spend is included under the `mens` provider slug.
    pub async fn unified_cost_summary(
        &self,
        since_days: u32,
    ) -> Result<UnifiedCostSummary, Box<dyn std::error::Error + Send + Sync>> {
        let by_model = self.cost_by_model(since_days).await?;

        let mut by_provider: std::collections::HashMap<String, ProviderCost> =
            std::collections::HashMap::new();
        for m in &by_model {
            let entry = by_provider
                .entry(m.provider.clone())
                .or_insert(ProviderCost {
                    provider: m.provider.clone(),
                    calls: 0,
                    cost_usd: 0.0,
                });
            entry.calls += m.calls;
            entry.cost_usd += m.cost_usd;
        }

        let total_calls: u32 = by_model.iter().map(|m| m.calls).sum();
        let total_cost: f64 = by_model.iter().map(|m| m.cost_usd).sum();

        Ok(UnifiedCostSummary {
            total_calls,
            total_cost_usd: total_cost,
            by_provider: by_provider.into_values().collect(),
            by_model,
            days_covered: since_days.max(1),
        })
    }

    /// Returns the last `n` day strings in `YYYY-MM-DD` format (including today).
    fn last_n_days(n: u32) -> Vec<String> {
        use std::time::{Duration, SystemTime, UNIX_EPOCH};
        let secs_per_day: u64 = 86_400;
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        (0..n)
            .map(|i| {
                let day_secs = now_secs.saturating_sub(u64::from(i) * secs_per_day);
                let days_since_epoch = day_secs / secs_per_day;
                // Convert days-since-epoch to ISO date string.
                // Uses Gregorian proleptic calendar arithmetic (no leap-second correction).
                let z = days_since_epoch as i64 + 719_468;
                let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
                let doe = z - era * 146_097;
                let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
                let y = yoe + era * 400;
                let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
                let mp = (5 * doy + 2) / 153;
                let d = doy - (153 * mp + 2) / 5 + 1;
                let m = if mp < 10 { mp + 3 } else { mp - 9 };
                let y = if m <= 2 { y + 1 } else { y };
                format!("{:04}-{:02}-{:02}", y, m, d)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_limits_cover_all_providers() {
        let limits = resolve_provider_limits();
        let providers: Vec<String> = limits.into_iter().map(|l| l.provider).collect();
        assert!(providers.contains(&"google".to_string()));
        assert!(providers.contains(&"openrouter".to_string()));
        assert!(providers.contains(&"ollama".to_string()));
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
            provider_request_id: None,
            provider_reported_cost_usd: None,
            estimated_cost_usd: Some(0.0),
            reconciled_cost_usd: Some(0.0),
            cost_source: Some("estimated".to_string()),
            is_rate_limited: false,
            last_429: None,
        };
        let json = serde_json::to_string(&record).unwrap();
        assert!(json.contains("gemini-2.5-flash-preview"));
        assert!(json.contains("42"));
    }
}
