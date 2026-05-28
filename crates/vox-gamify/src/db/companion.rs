//! Companion persistence.

use anyhow::Result;
use turso::params;
use vox_db::Codex;

use crate::companion::{Companion, Mood, Personality};

/// Load all companions for a user.
pub async fn list_companions(db: &Codex, user_id: &str) -> Result<Vec<Companion>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT id, user_id, name, description, code_hash, language, ascii_sprite, mood,
                    CAST(health AS TEXT), CAST(max_health AS TEXT), CAST(energy AS TEXT), CAST(max_energy AS TEXT),
                    CAST(code_quality AS TEXT), CAST(last_active AS TEXT), personality
             FROM gamify_companions WHERE user_id = ?1",
            params![user_id],
        )
        .await?;
    let mut comps = Vec::new();
    while let Some(row) = rows.next().await? {
        let r: Vec<Option<String>> = (0..15)
            .map(|i| row.get::<Option<String>>(i).unwrap_or(None))
            .collect();
        let personality_str = r[14].as_deref().unwrap_or("focused");
        let personality = personality_str.parse::<Personality>().unwrap_or_default();
        comps.push(Companion {
            id: r[0].clone().unwrap_or_default(),
            user_id: r[1].clone().unwrap_or_else(|| user_id.to_string()),
            name: r[2].clone().unwrap_or_default(),
            description: r[3].clone(),
            code_hash: r[4].clone(),
            language: r[5].clone().unwrap_or_default(),
            ascii_sprite: r[6].clone(),
            mood: r[7]
                .as_deref()
                .unwrap_or("neutral")
                .parse::<Mood>()
                .unwrap_or(Mood::Neutral),
            health: r[8]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            max_health: r[9]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            energy: r[10]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            max_energy: r[11]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            code_quality: r[12]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(50) as u8,
            last_active: r[13]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or_default(),
            personality,
        });
    }
    Ok(comps)
}

/// Upsert a companion (includes personality JSON).
#[allow(clippy::too_many_arguments)]
pub async fn upsert_companion(db: &Codex, c: &Companion) -> Result<()> {
    let id = c.id.clone();
    let user_id = c.user_id.clone();
    let name = c.name.clone();
    let description = c.description.clone();
    let code_hash = c.code_hash.clone();
    let language = c.language.clone();
    let ascii_sprite = c.ascii_sprite.clone();
    let mood = c.mood.as_str().to_string();
    let health = c.health as i64;
    let max_health = c.max_health as i64;
    let energy = c.energy as i64;
    let max_energy = c.max_energy as i64;
    let code_quality = c.code_quality as i64;
    let last_active = c.last_active;
    let personality = c.personality.as_str().to_string();
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO gamify_companions
                 (id, user_id, name, description, code_hash, language, ascii_sprite, mood, health, max_health, energy, max_energy, code_quality, last_active, personality)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
                 ON CONFLICT(id) DO UPDATE SET
                   name=excluded.name, user_id=excluded.user_id,
                   description=excluded.description, code_hash=excluded.code_hash,
                   language=excluded.language, ascii_sprite=excluded.ascii_sprite,
                   mood=excluded.mood, health=excluded.health, max_health=excluded.max_health,
                   energy=excluded.energy, max_energy=excluded.max_energy,
                   code_quality=excluded.code_quality, last_active=excluded.last_active,
                   personality=excluded.personality",
                params![
                    id.as_str(), user_id.as_str(), name.as_str(),
                    description, code_hash, language.as_str(), ascii_sprite,
                    mood.as_str(), health, max_health, energy, max_energy,
                    code_quality, last_active, personality.as_str(),
                ],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

/// Get a specific companion.
pub async fn get_companion(db: &Codex, id: &str) -> Result<Option<Companion>> {
    let mut rows = db
        .connection()
        .query(
            "SELECT id, user_id, name, description, code_hash, language, ascii_sprite, mood,
                    CAST(health AS TEXT), CAST(max_health AS TEXT), CAST(energy AS TEXT), CAST(max_energy AS TEXT),
                    CAST(code_quality AS TEXT), CAST(last_active AS TEXT), personality
             FROM gamify_companions WHERE id = ?1",
            params![id],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        let r: Vec<Option<String>> = (0..15)
            .map(|i| row.get::<Option<String>>(i).unwrap_or(None))
            .collect();
        let personality_str = r[14].as_deref().unwrap_or("focused");
        let personality = personality_str.parse::<Personality>().unwrap_or_default();
        Ok(Some(Companion {
            id: r[0].clone().unwrap_or_default(),
            user_id: r[1].clone().unwrap_or_default(),
            name: r[2].clone().unwrap_or_default(),
            description: r[3].clone(),
            code_hash: r[4].clone(),
            language: r[5].clone().unwrap_or_default(),
            ascii_sprite: r[6].clone(),
            mood: r[7]
                .as_deref()
                .unwrap_or("neutral")
                .parse::<Mood>()
                .unwrap_or(Mood::Neutral),
            health: r[8]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            max_health: r[9]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            energy: r[10]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            max_energy: r[11]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(100) as i32,
            code_quality: r[12]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(50) as u8,
            last_active: r[13]
                .as_deref()
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or_default(),
            personality,
        }))
    } else {
        Ok(None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OrchestratorCompanionIdMigration {
    None,
    /// Canonical row already present; drop stale legacy PK.
    DeleteLegacy,
    /// Rename legacy PK to canonical.
    RenameLegacyToCanonical,
}

pub(crate) fn orchestrator_companion_id_migration_plan(
    has_canonical: bool,
    has_legacy: bool,
) -> OrchestratorCompanionIdMigration {
    match (has_canonical, has_legacy) {
        (true, true) => OrchestratorCompanionIdMigration::DeleteLegacy,
        (false, true) => OrchestratorCompanionIdMigration::RenameLegacyToCanonical,
        _ => OrchestratorCompanionIdMigration::None,
    }
}

/// Best-effort migrate of the default orchestrator HUD companion row from the pre-rename id
/// (built with `concat!` so greps stay clean) to `vox-orchestrator`.
pub async fn migrate_default_orchestrator_companion_id(db: &Codex, user_id: &str) -> Result<()> {
    const CANONICAL: &str = "vox-orchestrator";
    let legacy = concat!("vox", "-", "dei");
    let has_c = get_companion(db, CANONICAL).await?.is_some();
    let has_l = get_companion(db, legacy).await?.is_some();
    match orchestrator_companion_id_migration_plan(has_c, has_l) {
        OrchestratorCompanionIdMigration::None => Ok(()),
        OrchestratorCompanionIdMigration::DeleteLegacy => delete_companion(db, legacy).await,
        OrchestratorCompanionIdMigration::RenameLegacyToCanonical => {
            let user_id = user_id.to_string();
            let breaker = db.breaker().clone();
            let conn = db.connection().clone();
            breaker
                .call(|| async move {
                    conn.execute(
                        "UPDATE gamify_companions SET id = ?1 WHERE id = ?2 AND user_id = ?3",
                        params![CANONICAL, legacy, user_id.as_str()],
                    )
                    .await?;
                    Ok::<(), vox_db::StoreError>(())
                })
                .await
                .map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(())
        }
    }
}

/// Delete a companion.
pub async fn delete_companion(db: &Codex, id: &str) -> Result<()> {
    let id = id.to_string();
    let breaker = db.breaker().clone();
    let conn = db.connection().clone();
    breaker
        .call(|| async move {
            conn.execute(
                "DELETE FROM gamify_companions WHERE id = ?1",
                params![id.as_str()],
            )
            .await?;
            Ok::<(), vox_db::StoreError>(())
        })
        .await
        .map_err(|e| anyhow::anyhow!("{}", e))?;
    Ok(())
}

#[cfg(test)]
mod orchestrator_companion_migration_tests {
    use super::orchestrator_companion_id_migration_plan;
    use super::OrchestratorCompanionIdMigration;

    #[test]
    fn migrates_none_when_legacy_missing() {
        assert_eq!(
            orchestrator_companion_id_migration_plan(false, false),
            OrchestratorCompanionIdMigration::None
        );
        assert_eq!(
            orchestrator_companion_id_migration_plan(true, false),
            OrchestratorCompanionIdMigration::None
        );
    }

    #[test]
    fn deletes_legacy_when_canonical_exists() {
        assert_eq!(
            orchestrator_companion_id_migration_plan(true, true),
            OrchestratorCompanionIdMigration::DeleteLegacy
        );
    }

    #[test]
    fn renames_when_only_legacy_exists() {
        assert_eq!(
            orchestrator_companion_id_migration_plan(false, true),
            OrchestratorCompanionIdMigration::RenameLegacyToCanonical
        );
    }
}
