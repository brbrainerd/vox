//! `LudusContext` — shared context helper that eliminates 20-line boilerplate
//! from every `vox ludus` CLI command.
//!
//! ## Before
//! ```ignore
//! let db = db_util::get_db().await?;
//! let user_id = vox_gamify::db::canonical_user_id();
//! let mut profile = db::get_profile(&db, &user_id)
//!     .await.unwrap_or_default()
//!     .unwrap_or_else(|| LudusProfile::new_default(&user_id));
//! ```
//!
//! ## After
//! ```ignore
//! let ctx = LudusContext::load().await?;
//! ```

use anyhow::Result;
use vox_db::Codex;
use vox_gamify::{LudusProfile, db};

use super::db_util;

/// All the context a `vox ludus` command typically needs, loaded in one call.
pub struct LudusContext {
    /// Live database handle.
    pub db: Codex,
    /// Canonical local user ID.
    pub user_id: String,
    /// Loaded (or freshly created) player profile.
    pub profile: LudusProfile,
}

impl LudusContext {
    /// Open the DB, resolve user ID, and load (or create) the player profile.
    ///
    /// Energy is regenerated and the profile is re-persisted on every load
    /// so that time-based regeneration is always current.
    pub async fn load() -> Result<Self> {
        let db = db_util::get_db().await?;
        let local_user_id = vox_gamify::db::canonical_user_id();

        // 1. Resolve effective user ID (prefer GitHub identity for global sync).
        // The OAuth flow stores the GitHub login in Clavis as the github.com
        // registry username. ponytail: keyed off login, not the numeric id (the
        // id isn't persisted); a GitHub rename would orphan the global profile —
        // acceptable for gamification sync.
        let effective_user_id = match vox_secrets::get_registry_username("github.com") {
            Some(login) if !login.is_empty() => format!("gh:{login}"),
            _ => local_user_id.clone(),
        };

        // 2. Load profile with migration support
        let mut profile = if let Some(p) = db::get_profile(&db, &effective_user_id).await? {
            p
        } else if effective_user_id != local_user_id {
            // Migration: Check if local profile exists and move it to the global identity
            if let Some(mut local_p) = db::get_profile(&db, &local_user_id).await? {
                tracing::info!(
                    "migrating local profile '{}' to global identity '{}'",
                    local_user_id,
                    effective_user_id
                );
                local_p.user_id = effective_user_id.clone();
                db::upsert_profile(&db, &local_p).await?;
                // Optionally delete old profile or keep as backup
                local_p
            } else {
                LudusProfile::new_default(&effective_user_id)
            }
        } else {
            LudusProfile::new_default(&effective_user_id)
        };

        profile.regen_energy();

        // 3. Automated Trust Tier Escalation
        if effective_user_id.starts_with("gh:")
            && profile.trust_tier < vox_gamify::profile::TrustTier::Linked
        {
            tracing::info!(
                "escalating trust tier to Linked for user '{}'",
                effective_user_id
            );
            profile.trust_tier = vox_gamify::profile::TrustTier::Linked;
        }

        if profile.trust_tier == vox_gamify::profile::TrustTier::Linked
            && profile.total_xp_earned >= 10_000
            && profile.lumens >= 100
        {
            tracing::info!(
                "escalating trust tier to Proven for user '{}'",
                effective_user_id
            );
            profile.trust_tier = vox_gamify::profile::TrustTier::Proven;
        }

        if profile.trust_tier == vox_gamify::profile::TrustTier::Proven
            && profile.total_xp_earned >= 50_000
            && profile.lumens >= 1_000
            && !profile.reward_suppressed
        {
            tracing::info!(
                "escalating trust tier to Master for user '{}'",
                effective_user_id
            );
            profile.trust_tier = vox_gamify::profile::TrustTier::Master;
        }

        let _ = db::upsert_profile(&db, &profile).await;

        Ok(Self {
            db,
            user_id: effective_user_id,
            profile,
        })
    }

    /// Like `load()` but returns `Ok(None)` instead of an error when the DB is
    /// unavailable — useful for fire-and-forget background tasks.
    pub async fn try_load() -> Option<Self> {
        Self::load().await.ok()
    }

    /// Re-save the profile after mutations.
    pub async fn save_profile(&self) -> Result<()> {
        db::upsert_profile(&self.db, &self.profile).await
    }
}
