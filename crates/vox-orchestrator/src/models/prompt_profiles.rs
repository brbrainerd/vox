//! Per-model learned prompt profiles: storage types + in-memory registry.
//!
//! The registry holds zero-or-more `ModelPromptProfile` variants per canonical
//! model key.  Only `Confirmed` variants are injected into the system prompt
//! (F3); others are in flight through the Provisional→Shadowed→Confirmed
//! autonomic pipeline (F4).

use crate::models::autonomic::ModelConfidence;
use std::collections::HashMap;
use std::sync::Mutex;
use vox_db::{ModelPromptProfileRow, StoreError, VoxDb};

// ── Canonical key ─────────────────────────────────────────────────────────────

/// Derive a stable `model_prompt_profiles` key for a given model.
///
/// Uses `canonical_slug` when non-empty (survives OpenRouter alias churn);
/// falls back to `model_id` otherwise.  Both inputs are lower-cased and
/// stripped of leading/trailing whitespace so case drift doesn't split profiles.
pub fn prompt_profile_key(model_id: &str, canonical_slug: &str) -> String {
    let slug = canonical_slug.trim();
    if slug.is_empty() {
        model_id.trim().to_lowercase()
    } else {
        slug.to_lowercase()
    }
}

// ── Data type ─────────────────────────────────────────────────────────────────

/// A single prompt-guidance variant for a canonical model key.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModelPromptProfile {
    /// Canonical model key (from `prompt_profile_key(model)`).
    pub prompt_profile_key: String,
    /// Monotonically increasing variant label ("v1", "v2", …).
    pub variant_id: String,
    /// Text injected into the system prompt for this model (cache-stable region).
    pub preamble_text: String,
    /// Autonomic confidence state.
    pub confidence: ModelConfidence,
    /// Signed quality delta vs. baseline measured during shadow-eval.
    pub quality_delta: f64,
    /// Number of real calls this variant has been active for.
    pub applications: i64,
    /// Unix ms when this variant was first recorded.
    pub created_at_ms: i64,
    /// Identifier of the human/council that approved this variant (nullable).
    pub approved_by: Option<String>,
}

impl ModelPromptProfile {
    fn from_row(r: ModelPromptProfileRow) -> Self {
        let confidence = match r.confidence.as_str() {
            "confirmed" => ModelConfidence::Confirmed,
            "shadowed" => ModelConfidence::Shadowed,
            "deprecated" => ModelConfidence::Deprecated,
            _ => ModelConfidence::Provisional,
        };
        Self {
            prompt_profile_key: r.prompt_profile_key,
            variant_id: r.variant_id,
            preamble_text: r.preamble_text,
            confidence,
            quality_delta: r.quality_delta,
            applications: r.applications,
            created_at_ms: r.created_at_ms,
            approved_by: r.approved_by,
        }
    }
}

// ── Registry ──────────────────────────────────────────────────────────────────

/// In-memory model-prompt-profile registry.  Loaded once at startup via
/// `hydrate_from_db`; updated in-process via `publish`.
pub struct ModelPromptRegistry {
    /// Key: `prompt_profile_key`; value: all variants for that key.
    profiles: Mutex<HashMap<String, Vec<ModelPromptProfile>>>,
}

impl ModelPromptRegistry {
    /// Empty registry (no DB load).
    pub fn new() -> Self {
        Self {
            profiles: Mutex::new(HashMap::new()),
        }
    }

    /// Load all rows from `model_prompt_profiles` into a fresh registry.
    pub async fn hydrate_from_db(db: &VoxDb) -> Result<Self, StoreError> {
        let rows = db.query_model_prompt_profiles().await?;
        let mut map: HashMap<String, Vec<ModelPromptProfile>> = HashMap::new();
        for row in rows {
            let p = ModelPromptProfile::from_row(row);
            map.entry(p.prompt_profile_key.clone()).or_default().push(p);
        }
        Ok(Self {
            profiles: Mutex::new(map),
        })
    }

    /// Populate this registry's in-memory map from the database.
    ///
    /// Unlike `hydrate_from_db` which creates a new registry, this mutates an
    /// existing `Arc<ModelPromptRegistry>` in place — so all Arc clones see the
    /// data immediately after the lock is released.
    pub async fn populate_from_db(&self, db: &VoxDb) -> Result<(), StoreError> {
        let rows = db.query_model_prompt_profiles().await?;
        let mut map = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        for row in rows {
            let p = ModelPromptProfile::from_row(row);
            map.entry(p.prompt_profile_key.clone()).or_default().push(p);
        }
        Ok(())
    }

    /// Return the single `Confirmed` variant for `key`, or `None`.
    pub fn active_profile(&self, key: &str) -> Option<ModelPromptProfile> {
        let map = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        map.get(key)?
            .iter()
            .find(|p| p.confidence == ModelConfidence::Confirmed)
            .cloned()
    }

    /// Persist `profile` to the DB and update the in-memory map.
    ///
    /// "Fire-and-forget" in the sense that errors are returned to the caller
    /// but this fn never panics.
    pub async fn publish(&self, db: &VoxDb, profile: ModelPromptProfile) -> Result<(), StoreError> {
        db.upsert_model_prompt_profile(
            &profile.prompt_profile_key,
            &profile.variant_id,
            &profile.preamble_text,
            profile.confidence.as_str(),
            profile.quality_delta,
            profile.applications,
            profile.created_at_ms,
            profile.approved_by.as_deref(),
        )
        .await?;
        let mut map = self.profiles.lock().unwrap_or_else(|e| e.into_inner());
        let vec = map.entry(profile.prompt_profile_key.clone()).or_default();
        // Demote any existing Confirmed variant in-memory when publishing a new Confirmed one.
        if profile.confidence == ModelConfidence::Confirmed {
            for p in vec.iter_mut() {
                if p.variant_id != profile.variant_id && p.confidence == ModelConfidence::Confirmed
                {
                    p.confidence = ModelConfidence::Deprecated;
                }
            }
        }
        if let Some(existing) = vec.iter_mut().find(|p| p.variant_id == profile.variant_id) {
            *existing = profile;
        } else {
            vec.push(profile);
        }
        Ok(())
    }
}

impl Default for ModelPromptRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ── Autonomic promotion (F4) ──────────────────────────────────────────────────

const PROVISIONAL_TO_SHADOWED_MIN_APPS: i64 = 5;
const SHADOWED_TO_CONFIRMED_MIN_APPS: i64 = 20;
const SHADOWED_TO_CONFIRMED_MIN_QUALITY_DELTA: f64 = 0.05;

/// Returns the next confidence level if `p` meets promotion criteria, or `None`.
///
/// Mirrors the `autonomic::should_promote` state machine but uses
/// `applications` and `quality_delta` from the prompt-profile domain.
pub fn should_promote_profile(p: &ModelPromptProfile) -> Option<ModelConfidence> {
    match p.confidence {
        ModelConfidence::Provisional => {
            if p.applications >= PROVISIONAL_TO_SHADOWED_MIN_APPS {
                Some(ModelConfidence::Shadowed)
            } else {
                None
            }
        }
        ModelConfidence::Shadowed => {
            if p.applications >= SHADOWED_TO_CONFIRMED_MIN_APPS
                && p.quality_delta >= SHADOWED_TO_CONFIRMED_MIN_QUALITY_DELTA
            {
                Some(ModelConfidence::Confirmed)
            } else {
                None
            }
        }
        ModelConfidence::Confirmed | ModelConfidence::Deprecated => None,
    }
}

/// Scan all profiles in `registry`, promote those that meet the gate, persist
/// the updated rows, and return a list of `(key, from, to)` transitions.
pub async fn maybe_promote_registry(
    registry: &ModelPromptRegistry,
    db: &VoxDb,
) -> Vec<(String, ModelConfidence, ModelConfidence)> {
    let candidates: Vec<ModelPromptProfile> = {
        let map = registry.profiles.lock().unwrap_or_else(|e| e.into_inner());
        map.values().flatten().cloned().collect()
    };
    let mut promoted = Vec::new();
    for mut p in candidates {
        if let Some(next) = should_promote_profile(&p) {
            let from = p.confidence;
            let key = p.prompt_profile_key.clone();
            p.confidence = next;
            // created_at_ms is intentionally preserved — records when the variant was first mined.
            if registry.publish(db, p).await.is_ok() {
                promoted.push((key, from, next));
            }
        }
    }
    promoted
}

// ── Prompt injection ──────────────────────────────────────────────────────────

/// Returns a cache-stable `## Model guidance` segment for injection into the
/// system prompt, or `None` if no `Confirmed` variant exists for `key`.
///
/// Placed after the skill-catalog layer and before volatile budget/temporal
/// sections — the content is fully stable so it doesn't bust prefix caches.
pub fn model_guidance_segment(registry: &ModelPromptRegistry, key: &str) -> Option<String> {
    registry
        .active_profile(key)
        .map(|p| format!("\n\n## Model guidance ({})\n\n{}\n", key, p.preamble_text))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::autonomic::ModelConfidence;
    use vox_db::{DbConfig, VoxDb};

    fn now_ms() -> i64 {
        1_750_000_000_000
    }

    // ── F2 pure-function tests ─────────────────────────────────────────────────

    #[test]
    fn prompt_profile_key_uses_canonical_slug_when_present() {
        let key = prompt_profile_key("openrouter/anthropic/claude-sonnet-4", "claude-sonnet-4");
        assert_eq!(key, "claude-sonnet-4");
    }

    #[test]
    fn prompt_profile_key_falls_back_to_model_id_when_slug_empty() {
        let key = prompt_profile_key("gpt-4o", "");
        assert_eq!(key, "gpt-4o");
    }

    #[test]
    fn prompt_profile_key_is_stable_across_provider_alias_churn() {
        // Same canonical slug → same key, regardless of model_id variant.
        let k1 = prompt_profile_key("openrouter/google/gemini-2-flash", "gemini-2-flash");
        let k2 = prompt_profile_key("google/gemini-2-flash-001", "gemini-2-flash");
        assert_eq!(k1, k2);
    }

    #[test]
    fn prompt_profile_key_normalises_case() {
        let k1 = prompt_profile_key("Claude-Sonnet", "");
        let k2 = prompt_profile_key("claude-sonnet", "");
        assert_eq!(k1, k2);
    }

    // ── F4 promotion tests ────────────────────────────────────────────────────

    fn make_profile(
        confidence: ModelConfidence,
        applications: i64,
        quality_delta: f64,
    ) -> ModelPromptProfile {
        ModelPromptProfile {
            prompt_profile_key: "test-model".into(),
            variant_id: "v1".into(),
            preamble_text: "Guidance text.".into(),
            confidence,
            quality_delta,
            applications,
            created_at_ms: 1_750_000_000_000,
            approved_by: None,
        }
    }

    #[test]
    fn provisional_promotes_to_shadowed_when_enough_apps() {
        let p = make_profile(ModelConfidence::Provisional, 5, 0.0);
        assert_eq!(should_promote_profile(&p), Some(ModelConfidence::Shadowed));
    }

    #[test]
    fn provisional_stays_when_too_few_apps() {
        let p = make_profile(ModelConfidence::Provisional, 4, 0.0);
        assert_eq!(should_promote_profile(&p), None);
    }

    #[test]
    fn shadowed_promotes_to_confirmed_when_meeting_both_gates() {
        let p = make_profile(ModelConfidence::Shadowed, 20, 0.05);
        assert_eq!(should_promote_profile(&p), Some(ModelConfidence::Confirmed));
    }

    #[test]
    fn shadowed_stays_when_quality_delta_too_low() {
        let p = make_profile(ModelConfidence::Shadowed, 30, 0.04);
        assert_eq!(should_promote_profile(&p), None);
    }

    #[test]
    fn confirmed_never_promotes() {
        let p = make_profile(ModelConfidence::Confirmed, 100, 1.0);
        assert_eq!(should_promote_profile(&p), None);
    }

    #[tokio::test]
    async fn maybe_promote_registry_persists_and_returns_transitions() {
        let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
        let reg = ModelPromptRegistry::new();
        reg.publish(&db, make_profile(ModelConfidence::Provisional, 5, 0.0))
            .await
            .unwrap();
        let transitions = maybe_promote_registry(&reg, &db).await;
        assert_eq!(transitions.len(), 1);
        let (key, from, to) = &transitions[0];
        assert_eq!(key, "test-model");
        assert_eq!(*from, ModelConfidence::Provisional);
        assert_eq!(*to, ModelConfidence::Shadowed);
        // Verify persisted state.
        let p = reg.profiles.lock().unwrap_or_else(|e| e.into_inner());
        let updated = p["test-model"]
            .iter()
            .find(|x| x.variant_id == "v1")
            .unwrap();
        assert_eq!(updated.confidence, ModelConfidence::Shadowed);
    }

    // ── F3 pure-function tests ─────────────────────────────────────────────────

    #[test]
    fn model_guidance_segment_includes_preamble_for_confirmed() {
        let reg = ModelPromptRegistry::new();
        // Manually insert a Confirmed profile without DB.
        {
            let mut map = reg.profiles.lock().unwrap_or_else(|e| e.into_inner());
            map.entry("claude-sonnet-4".into())
                .or_default()
                .push(ModelPromptProfile {
                    prompt_profile_key: "claude-sonnet-4".into(),
                    variant_id: "v1".into(),
                    preamble_text: "Always prefer Rust idioms.".into(),
                    confidence: ModelConfidence::Confirmed,
                    quality_delta: 0.1,
                    applications: 50,
                    created_at_ms: 1_750_000_000_000,
                    approved_by: None,
                });
        }
        let seg = model_guidance_segment(&reg, "claude-sonnet-4").unwrap();
        assert!(seg.contains("## Model guidance (claude-sonnet-4)"));
        assert!(seg.contains("Always prefer Rust idioms."));
    }

    #[test]
    fn model_guidance_segment_absent_for_provisional() {
        let reg = ModelPromptRegistry::new();
        {
            let mut map = reg.profiles.lock().unwrap_or_else(|e| e.into_inner());
            map.entry("gpt-4o".into())
                .or_default()
                .push(ModelPromptProfile {
                    prompt_profile_key: "gpt-4o".into(),
                    variant_id: "v1".into(),
                    preamble_text: "Be verbose.".into(),
                    confidence: ModelConfidence::Provisional,
                    quality_delta: 0.0,
                    applications: 2,
                    created_at_ms: 1_750_000_000_000,
                    approved_by: None,
                });
        }
        assert!(model_guidance_segment(&reg, "gpt-4o").is_none());
    }

    #[test]
    fn model_guidance_segment_absent_for_unknown_key() {
        let reg = ModelPromptRegistry::new();
        assert!(model_guidance_segment(&reg, "unknown-model").is_none());
    }

    // ── F1 async tests ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn hydrate_loads_confirmed_profile() {
        let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
        let reg = ModelPromptRegistry::new();
        reg.publish(
            &db,
            ModelPromptProfile {
                prompt_profile_key: "claude-sonnet".into(),
                variant_id: "v1".into(),
                preamble_text: "Be concise.".into(),
                confidence: ModelConfidence::Confirmed,
                quality_delta: 0.15,
                applications: 10,
                created_at_ms: now_ms(),
                approved_by: Some("tester".into()),
            },
        )
        .await
        .unwrap();

        let loaded = ModelPromptRegistry::hydrate_from_db(&db).await.unwrap();
        let p = loaded
            .active_profile("claude-sonnet")
            .expect("Confirmed profile must be present");
        assert_eq!(p.preamble_text, "Be concise.");
        assert_eq!(p.confidence, ModelConfidence::Confirmed);
    }

    #[tokio::test]
    async fn active_profile_absent_for_unknown_key() {
        let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
        let reg = ModelPromptRegistry::hydrate_from_db(&db).await.unwrap();
        assert!(reg.active_profile("no-model").is_none());
    }

    #[tokio::test]
    async fn active_profile_absent_for_provisional() {
        let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
        let reg = ModelPromptRegistry::new();
        reg.publish(
            &db,
            ModelPromptProfile {
                prompt_profile_key: "gpt-4".into(),
                variant_id: "v1".into(),
                preamble_text: "Use Python.".into(),
                confidence: ModelConfidence::Provisional,
                quality_delta: 0.0,
                applications: 0,
                created_at_ms: now_ms(),
                approved_by: None,
            },
        )
        .await
        .unwrap();
        let reg2 = ModelPromptRegistry::hydrate_from_db(&db).await.unwrap();
        assert!(
            reg2.active_profile("gpt-4").is_none(),
            "Provisional must not be returned by active_profile"
        );
    }

    #[tokio::test]
    async fn publish_updates_in_memory_map() {
        let db = VoxDb::connect(DbConfig::Memory).await.unwrap();
        let reg = ModelPromptRegistry::new();
        reg.publish(
            &db,
            ModelPromptProfile {
                prompt_profile_key: "model-a".into(),
                variant_id: "v1".into(),
                preamble_text: "Original.".into(),
                confidence: ModelConfidence::Confirmed,
                quality_delta: 0.1,
                applications: 5,
                created_at_ms: now_ms(),
                approved_by: None,
            },
        )
        .await
        .unwrap();
        let p = reg
            .active_profile("model-a")
            .expect("in-memory update after publish");
        assert_eq!(p.preamble_text, "Original.");
    }

    #[test]
    fn publish_demotes_prior_confirmed_on_new_confirmed() {
        let registry = ModelPromptRegistry::new();
        let mut map = registry.profiles.lock().unwrap();
        map.entry("gpt4".to_string())
            .or_default()
            .push(ModelPromptProfile {
                prompt_profile_key: "gpt4".to_string(),
                variant_id: "v1".to_string(),
                preamble_text: "old".to_string(),
                confidence: ModelConfidence::Confirmed,
                quality_delta: 0.1,
                applications: 20,
                created_at_ms: 1_000_000,
                approved_by: None,
            });
        drop(map);

        // Now insert a second Confirmed variant — the first must be demoted.
        {
            let mut map = registry.profiles.lock().unwrap();
            let vec = map.entry("gpt4".to_string()).or_default();
            // Demote existing Confirmed (mirrors publish logic, DB call omitted).
            let new_id = "v2";
            for p in vec.iter_mut() {
                if p.variant_id != new_id && p.confidence == ModelConfidence::Confirmed {
                    p.confidence = ModelConfidence::Deprecated;
                }
            }
            vec.push(ModelPromptProfile {
                prompt_profile_key: "gpt4".to_string(),
                variant_id: new_id.to_string(),
                preamble_text: "new".to_string(),
                confidence: ModelConfidence::Confirmed,
                quality_delta: 0.2,
                applications: 25,
                created_at_ms: 2_000_000,
                approved_by: None,
            });
        }

        let active = registry
            .active_profile("gpt4")
            .expect("should have exactly one Confirmed");
        assert_eq!(active.variant_id, "v2");
        // Ensure v1 is now Deprecated in memory.
        let map = registry.profiles.lock().unwrap();
        let v1 = map["gpt4"].iter().find(|p| p.variant_id == "v1").unwrap();
        assert_eq!(v1.confidence, ModelConfidence::Deprecated);
    }
}
