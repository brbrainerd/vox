//! Database persistence for gamification layer.

use anyhow::Result;
use turso::params;
use vox_db::VoxDb;

use crate::companion::{Companion, Mood};
use crate::profile::GamifyProfile;

// ── Profile ──────────────────────────────────────────────

/// Load a gamify profile from the DB.
pub async fn get_profile(db: &VoxDb, user_id: &str) -> Result<Option<GamifyProfile>> {
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT level, xp, crystals, energy, max_energy, CAST(last_energy_regen AS INTEGER), CAST(last_active AS INTEGER)
         FROM gamify_profiles WHERE user_id = ?1",
            params![user_id],
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(GamifyProfile {
            user_id: user_id.to_string(),
            level: row.get::<i64>(0)? as u64,
            xp: row.get::<i64>(1)? as u64,
            crystals: row.get::<i64>(2)? as u64,
            energy: row.get::<i64>(3)? as u64,
            max_energy: row.get::<i64>(4)? as u64,
            // Safely parse as String first to avoid turso panics on mixed column types
            last_energy_regen: row.get::<Option<i64>>(5)?.unwrap_or_default(),
            last_active: row.get::<Option<i64>>(6)?.unwrap_or_default(),
            streak: crate::streak::StreakTracker::default(),
        }))
    } else {
        Ok(None)
    }
}

/// Upsert a gamify profile to the DB.
pub async fn upsert_profile(db: &VoxDb, p: &GamifyProfile) -> Result<()> {
    db.store().conn.execute(
        "INSERT INTO gamify_profiles (user_id, level, xp, crystals, energy, max_energy, last_energy_regen, last_active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(user_id) DO UPDATE SET
            level = excluded.level,
            xp = excluded.xp,
            crystals = excluded.crystals,
            energy = excluded.energy,
            max_energy = excluded.max_energy,
            last_energy_regen = excluded.last_energy_regen,
            last_active = excluded.last_active",
        params![
            p.user_id.clone(),
            p.level as i64,
            p.xp as i64,
            p.crystals as i64,
            p.energy as i64,
            p.max_energy as i64,
            p.last_energy_regen,
            p.last_active
        ],
    ).await?;
    Ok(())
}

// ── Companion ────────────────────────────────────────────

/// Load all companions for a user.
pub async fn list_companions(db: &VoxDb, user_id: &str) -> Result<Vec<Companion>> {
    let mut rows = db.store().conn.query(
        "SELECT id, name, description, code_hash, language, ascii_sprite, mood, health, max_health, energy, max_energy, code_quality, last_active
         FROM gamify_companions WHERE user_id = ?1",
        params![user_id],
    ).await?;

    let mut comps = Vec::new();
    while let Some(row) = rows.next().await? {
        comps.push(Companion {
            id: row.get::<String>(0)?,
            user_id: user_id.to_string(),
            name: row.get::<String>(1)?,
            description: row.get::<Option<String>>(2)?,
            code_hash: row.get::<Option<String>>(3)?,
            language: row.get::<String>(4)?,
            ascii_sprite: row.get::<Option<String>>(5)?,
            mood: row
                .get::<String>(6)
                .unwrap_or_else(|_| "neutral".to_string())
                .parse::<Mood>()
                .unwrap_or(Mood::Neutral),
            health: row.get::<i64>(7)? as i32,
            max_health: row.get::<i64>(8)? as i32,
            energy: row.get::<i64>(9)? as i32,
            max_energy: row.get::<i64>(10)? as i32,
            code_quality: row.get::<i64>(11)? as u8,
            last_active: row.get::<Option<i64>>(12)?.unwrap_or_default(),
            personality: crate::companion::Personality::default(),
        });
    }

    Ok(comps)
}

/// Upsert a companion.
pub async fn upsert_companion(db: &VoxDb, c: &Companion) -> Result<()> {
    db.store().conn.execute(
        "INSERT INTO gamify_companions (id, user_id, name, description, code_hash, language, ascii_sprite, mood, health, max_health, energy, max_energy, code_quality, last_active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
         ON CONFLICT(id) DO UPDATE SET
            name = excluded.name,
            description = excluded.description,
            code_hash = excluded.code_hash,
            ascii_sprite = excluded.ascii_sprite,
            mood = excluded.mood,
            health = excluded.health,
            energy = excluded.energy,
            code_quality = excluded.code_quality,
            last_active = excluded.last_active",
        params![
            c.id.clone(),
            c.user_id.clone(),
            c.name.clone(),
            c.description.clone(),
            c.code_hash.clone(),
            c.language.clone(),
            c.ascii_sprite.clone(),
            c.mood.as_str().to_string(),
            c.health as i64,
            c.max_health as i64,
            c.energy as i64,
            c.max_energy as i64,
            c.code_quality as i64,
            c.last_active
        ],
    ).await?;
    Ok(())
}

// ── Quests ───────────────────────────────────────────────

/// Get a specific companion.
pub async fn get_companion(db: &VoxDb, id: &str) -> Result<Option<Companion>> {
    let mut rows = db.store().conn.query(
        "SELECT id, user_id, name, description, code_hash, language, ascii_sprite, mood, health, max_health, energy, max_energy, code_quality, last_active
         FROM gamify_companions WHERE id = ?1",
        params![id],
    ).await?;
    if let Some(row) = rows.next().await? {
        Ok(Some(Companion {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            name: row.get::<String>(2)?,
            description: row.get::<Option<String>>(3)?,
            code_hash: row.get::<Option<String>>(4)?,
            language: row.get::<String>(5)?,
            ascii_sprite: row.get::<Option<String>>(6)?,
            mood: row
                .get::<String>(7)
                .unwrap_or_else(|_| "neutral".to_string())
                .parse::<Mood>()
                .unwrap_or(Mood::Neutral),
            health: row.get::<i64>(8)? as i32,
            max_health: row.get::<i64>(9)? as i32,
            energy: row.get::<i64>(10)? as i32,
            max_energy: row.get::<i64>(11)? as i32,
            code_quality: row.get::<i64>(12)? as u8,
            last_active: row.get::<Option<i64>>(13)?.unwrap_or_default(),
            personality: crate::companion::Personality::default(),
        }))
    } else {
        Ok(None)
    }
}

/// Delete a companion.
pub async fn delete_companion(db: &VoxDb, id: &str) -> Result<()> {
    db.store()
        .conn
        .execute("DELETE FROM gamify_companions WHERE id = ?1", params![id])
        .await?;
    Ok(())
}

// ── Quests ───────────────────────────────────────────────

use crate::quest::{Quest, QuestType};

/// Load all active quests for a user.
pub async fn list_quests(db: &VoxDb, user_id: &str) -> Result<Vec<Quest>> {
    let mut rows = db.store().conn.query(
        "SELECT id, quest_type, description, target, progress, crystal_reward, xp_reward, completed, expires_at
         FROM gamify_quests WHERE user_id = ?1",
        params![user_id],
    ).await?;

    let mut quests = Vec::new();
    while let Some(row) = rows.next().await? {
        quests.push(Quest {
            id: row.get::<String>(0)?,
            user_id: user_id.to_string(),
            quest_type: match row.get::<String>(1)?.as_str() {
                "create" => QuestType::Create,
                "review" => QuestType::Review,
                "battle" => QuestType::Battle,
                "improve" => QuestType::Improve,
                _ => QuestType::Create,
            },
            description: row.get::<String>(2)?,
            target: row.get::<i64>(3)? as u32,
            progress: row.get::<i64>(4)? as u32,
            crystal_reward: row.get::<i64>(5)? as u64,
            xp_reward: row.get::<i64>(6)? as u64,
            completed: row.get::<i64>(7)? != 0,
            expires_at: row.get::<i64>(8)?,
        });
    }

    Ok(quests)
}

/// Upsert a quest.
pub async fn upsert_quest(db: &VoxDb, q: &Quest) -> Result<()> {
    db.store().conn.execute(
        "INSERT INTO gamify_quests (id, user_id, quest_type, description, target, progress, crystal_reward, xp_reward, completed, expires_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
         ON CONFLICT(id) DO UPDATE SET
            progress = excluded.progress,
            completed = excluded.completed",
        params![
            q.id.clone(),
            q.user_id.clone(),
            q.quest_type.as_str(),
            q.description.clone(),
            q.target as i64,
            q.progress as i64,
            q.crystal_reward as i64,
            q.xp_reward as i64,
            if q.completed { 1i64 } else { 0i64 },
            q.expires_at,
        ],
    ).await?;
    Ok(())
}

// ── Battles ──────────────────────────────────────────────

/// Get a specific quest by ID.
pub async fn get_quest(db: &VoxDb, id: &str) -> Result<Option<Quest>> {
    let mut rows = db.store().conn.query(
        "SELECT id, user_id, quest_type, description, target, progress, crystal_reward, xp_reward, completed, expires_at
         FROM gamify_quests WHERE id = ?1",
        params![id],
    ).await?;
    if let Some(row) = rows.next().await? {
        Ok(Some(Quest {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            quest_type: match row.get::<String>(2)?.as_str() {
                "create" => QuestType::Create,
                "review" => QuestType::Review,
                "battle" => QuestType::Battle,
                "improve" => QuestType::Improve,
                _ => QuestType::Create,
            },
            description: row.get::<String>(3)?,
            target: row.get::<i64>(4)? as u32,
            progress: row.get::<i64>(5)? as u32,
            crystal_reward: row.get::<i64>(6)? as u64,
            xp_reward: row.get::<i64>(7)? as u64,
            completed: row.get::<i64>(8)? != 0,
            expires_at: row.get::<i64>(9)?,
        }))
    } else {
        Ok(None)
    }
}

/// Delete a quest.
pub async fn delete_quest(db: &VoxDb, id: &str) -> Result<()> {
    db.store()
        .conn
        .execute("DELETE FROM gamify_quests WHERE id = ?1", params![id])
        .await?;
    Ok(())
}

/// Count active quests for a user.
pub async fn count_quests(db: &VoxDb, user_id: &str) -> Result<i64> {
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT COUNT(*) FROM gamify_quests WHERE user_id = ?1 AND completed = 0",
            params![user_id],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get::<i64>(0).unwrap_or(0))
    } else {
        Ok(0)
    }
}

// ── Battles ──────────────────────────────────────────────

use crate::battle::{Battle, BugType};

/// Load recent battles for a user.
pub async fn list_battles(db: &VoxDb, user_id: &str, limit: i64) -> Result<Vec<Battle>> {
    let mut rows = db.store().conn.query(
        "SELECT id, companion_id, bug_type, bug_description, bug_code, submitted_code, success, crystals_earned, xp_earned, duration_secs, created_at
         FROM gamify_battles WHERE user_id = ?1 ORDER BY created_at DESC LIMIT ?2",
        params![user_id, limit],
    ).await?;

    let mut battles = Vec::new();
    while let Some(row) = rows.next().await? {
        battles.push(Battle {
            id: row.get::<String>(0)?,
            user_id: user_id.to_string(),
            companion_id: row.get::<String>(1)?,
            bug_type: match row.get::<String>(2)?.as_str() {
                "syntax" => BugType::Syntax,
                "logic" => BugType::Logic,
                "performance" => BugType::Performance,
                "security" => BugType::Security,
                _ => BugType::Syntax,
            },
            bug_description: row.get::<String>(3)?,
            bug_code: row.get::<Option<String>>(4)?,
            submitted_code: row.get::<Option<String>>(5)?,
            success: row.get::<i64>(6)? != 0,
            crystals_earned: row.get::<i64>(7)? as u64,
            xp_earned: row.get::<i64>(8)? as u64,
            duration_secs: row.get::<i64>(9)? as u64,
            created_at: row.get::<i64>(10)?,
        });
    }

    Ok(battles)
}

/// Insert a new battle record.
pub async fn insert_battle(db: &VoxDb, b: &Battle) -> Result<()> {
    db.store().conn.execute(
        "INSERT INTO gamify_battles (id, user_id, companion_id, bug_type, bug_description, bug_code, submitted_code, success, crystals_earned, xp_earned, duration_secs, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            b.id.clone(),
            b.user_id.clone(),
            b.companion_id.clone(),
            b.bug_type.as_str(),
            b.bug_description.clone(),
            b.bug_code.clone(),
            b.submitted_code.clone(),
            if b.success { 1i64 } else { 0i64 },
            b.crystals_earned as i64,
            b.xp_earned as i64,
            b.duration_secs as i64,
            b.created_at,
        ],
    ).await?;
    Ok(())
}

// ── Events ───────────────────────────────────────────────

/// Get a specific battle by ID.
pub async fn get_battle(db: &VoxDb, id: &str) -> Result<Option<Battle>> {
    let mut rows = db.store().conn.query(
        "SELECT id, user_id, companion_id, bug_type, bug_description, bug_code, submitted_code, success, crystals_earned, xp_earned, duration_secs, created_at
         FROM gamify_battles WHERE id = ?1",
        params![id],
    ).await?;
    if let Some(row) = rows.next().await? {
        Ok(Some(Battle {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            companion_id: row.get::<String>(2)?,
            bug_type: match row.get::<String>(3)?.as_str() {
                "syntax" => BugType::Syntax,
                "logic" => BugType::Logic,
                "performance" => BugType::Performance,
                "security" => BugType::Security,
                _ => BugType::Syntax,
            },
            bug_description: row.get::<String>(4)?,
            bug_code: row.get::<Option<String>>(5)?,
            submitted_code: row.get::<Option<String>>(6)?,
            success: row.get::<i64>(7)? != 0,
            crystals_earned: row.get::<i64>(8)? as u64,
            xp_earned: row.get::<i64>(9)? as u64,
            duration_secs: row.get::<i64>(10)? as u64,
            created_at: row.get::<i64>(11)?,
        }))
    } else {
        Ok(None)
    }
}

/// Update a battle.
pub async fn update_battle(db: &VoxDb, b: &Battle) -> Result<()> {
    db.store()
        .conn
        .execute(
            "UPDATE gamify_battles SET
            submitted_code = ?1,
            success = ?2,
            crystals_earned = ?3,
            xp_earned = ?4,
            duration_secs = ?5
         WHERE id = ?6",
            params![
                b.submitted_code.clone(),
                if b.success { 1i64 } else { 0i64 },
                b.crystals_earned as i64,
                b.xp_earned as i64,
                b.duration_secs as i64,
                b.id.clone()
            ],
        )
        .await?;
    Ok(())
}

/// Count battles played by a user.
pub async fn count_battles(db: &VoxDb, user_id: &str) -> Result<i64> {
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT COUNT(*) FROM gamify_battles WHERE user_id = ?1",
            params![user_id],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get::<i64>(0).unwrap_or(0))
    } else {
        Ok(0)
    }
}

/// Get top users by XP for the leaderboard.
pub async fn leaderboard(db: &VoxDb, limit: i64) -> Result<Vec<(String, u64, u64)>> {
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT user_id, level, xp FROM gamify_profiles ORDER BY xp DESC LIMIT ?1",
            params![limit],
        )
        .await?;

    let mut top_users = Vec::new();
    while let Some(row) = rows.next().await? {
        top_users.push((
            row.get::<String>(0)?,
            row.get::<i64>(1)? as u64,
            row.get::<i64>(2)? as u64,
        ));
    }
    Ok(top_users)
}

/// Get aggregate profile stats (e.g. total completed quests, total battles won, etc.).
pub async fn get_profile_stats(db: &VoxDb, user_id: &str) -> Result<serde_json::Value> {
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT COUNT(id) FROM gamify_quests WHERE user_id = ?1 AND completed = 1",
            params![user_id],
        )
        .await?;
    let completed_quests = if let Some(row) = rows.next().await? {
        row.get::<i64>(0).unwrap_or(0)
    } else {
        0
    };

    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT COUNT(id) FROM gamify_battles WHERE user_id = ?1 AND success = 1",
            params![user_id],
        )
        .await?;
    let won_battles = if let Some(row) = rows.next().await? {
        row.get::<i64>(0).unwrap_or(0)
    } else {
        0
    };

    Ok(serde_json::json!({
        "completed_quests": completed_quests,
        "won_battles": won_battles,
    }))
}

// ── Events ───────────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentEventRecord {
    pub id: i64,
    pub agent_id: String,
    pub event_type: String,
    pub payload: Option<String>,
    pub timestamp: String,
}

/// Load recent events for an agent.
pub async fn get_events(
    db: &VoxDb,
    agent_id: &str,
    limit: Option<i64>,
) -> Result<Vec<AgentEventRecord>> {
    let limit_val = limit.unwrap_or(50);
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT id, agent_id, event_type, payload, timestamp
         FROM agent_events WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
            params![agent_id.to_string(), limit_val],
        )
        .await?;

    let mut events = Vec::new();
    while let Some(row) = rows.next().await? {
        events.push(AgentEventRecord {
            id: row.get::<i64>(0)?,
            agent_id: row.get::<String>(1)?,
            event_type: row.get::<String>(2)?,
            payload: row.get::<Option<String>>(3)?,
            timestamp: row.get::<String>(4)?, // SQLite datetime string
        });
    }

    Ok(events)
}

/// Insert a new agent event.
pub async fn insert_event(
    db: &VoxDb,
    agent_id: &str,
    event_type: &str,
    payload: Option<&str>,
) -> Result<()> {
    db.store()
        .conn
        .execute(
            "INSERT INTO agent_events (agent_id, event_type, payload) VALUES (?1, ?2, ?3)",
            match payload {
                Some(p) => params![agent_id.to_string(), event_type.to_string(), p.to_string()],
                None => params![
                    agent_id.to_string(),
                    event_type.to_string(),
                    turso::Value::Null
                ],
            },
        )
        .await?;
    Ok(())
}

// ── Cost Records ─────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct CostRecord {
    pub id: i64,
    pub agent_id: String,
    pub session_id: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
    pub timestamp: String,
}

/// Insert a cost record.
#[allow(clippy::too_many_arguments)]
pub async fn insert_cost_record(
    db: &VoxDb,
    agent_id: &str,
    session_id: Option<&str>,
    provider: &str,
    model: Option<&str>,
    input_tokens: i64,
    output_tokens: i64,
    cost_usd: f64,
) -> Result<()> {
    db.store().conn.execute(
        "INSERT INTO cost_records (agent_id, session_id, provider, model, input_tokens, output_tokens, cost_usd)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            agent_id.to_string(),
            session_id.map(|s| s.to_string()),
            provider.to_string(),
            model.map(|m| m.to_string()),
            input_tokens,
            output_tokens,
            cost_usd,
        ],
    ).await?;
    Ok(())
}

/// Get total cost for an agent.
pub async fn get_agent_cost_usd(db: &VoxDb, agent_id: &str) -> Result<f64> {
    let mut rows = db
        .store()
        .conn
        .query(
            "SELECT COALESCE(SUM(cost_usd), 0.0) FROM cost_records WHERE agent_id = ?1",
            params![agent_id.to_string()],
        )
        .await?;
    if let Some(row) = rows.next().await? {
        Ok(row.get::<f64>(0).unwrap_or(0.0))
    } else {
        Ok(0.0)
    }
}

/// Get cost records for an agent, most recent first.
pub async fn list_cost_records(db: &VoxDb, agent_id: &str, limit: i64) -> Result<Vec<CostRecord>> {
    let mut rows = db.store().conn.query(
        "SELECT id, agent_id, session_id, provider, model, input_tokens, output_tokens, cost_usd, timestamp
         FROM cost_records WHERE agent_id = ?1 ORDER BY timestamp DESC LIMIT ?2",
        params![agent_id.to_string(), limit],
    ).await?;

    let mut records = Vec::new();
    while let Some(row) = rows.next().await? {
        records.push(CostRecord {
            id: row.get::<i64>(0)?,
            agent_id: row.get::<String>(1)?,
            session_id: row.get::<Option<String>>(2)?,
            provider: row.get::<String>(3)?,
            model: row.get::<Option<String>>(4)?,
            input_tokens: row.get::<i64>(5)?,
            output_tokens: row.get::<i64>(6)?,
            cost_usd: row.get::<f64>(7)?,
            timestamp: row.get::<String>(8)?,
        });
    }
    Ok(records)
}

// ── A2A Messages ──────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct A2AMessageRecord {
    pub id: i64,
    pub sender: String,
    pub receiver: Option<String>,
    pub msg_type: String,
    pub payload: Option<String>,
    pub correlation_id: Option<String>,
    pub acknowledged: bool,
    pub timestamp: String,
}

/// Insert an A2A message into persistent storage.
pub async fn insert_a2a_message(
    db: &VoxDb,
    sender: &str,
    receiver: Option<&str>,
    msg_type: &str,
    payload: Option<&str>,
    correlation_id: Option<&str>,
) -> Result<()> {
    db.store()
        .conn
        .execute(
            "INSERT INTO a2a_messages (sender, receiver, msg_type, payload, correlation_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                sender.to_string(),
                receiver.map(|r| r.to_string()),
                msg_type.to_string(),
                payload.map(|p| p.to_string()),
                correlation_id.map(|c| c.to_string()),
            ],
        )
        .await?;
    Ok(())
}

/// Get unacknowledged messages for a receiver.
pub async fn get_pending_messages(db: &VoxDb, receiver: &str) -> Result<Vec<A2AMessageRecord>> {
    let mut rows = db.store().conn.query(
        "SELECT id, sender, receiver, msg_type, payload, correlation_id, acknowledged, timestamp
         FROM a2a_messages WHERE receiver = ?1 AND acknowledged = 0 ORDER BY timestamp ASC",
        params![receiver.to_string()],
    ).await?;

    let mut msgs = Vec::new();
    while let Some(row) = rows.next().await? {
        msgs.push(A2AMessageRecord {
            id: row.get::<i64>(0)?,
            sender: row.get::<String>(1)?,
            receiver: row.get::<Option<String>>(2)?,
            msg_type: row.get::<String>(3)?,
            payload: row.get::<Option<String>>(4)?,
            correlation_id: row.get::<Option<String>>(5)?,
            acknowledged: row.get::<i64>(6)? != 0,
            timestamp: row.get::<String>(7)?,
        });
    }
    Ok(msgs)
}

/// List recent A2A messages (audit trail).
pub async fn list_a2a_messages(db: &VoxDb, limit: i64) -> Result<Vec<A2AMessageRecord>> {
    let mut rows = db.store().conn.query(
        "SELECT id, sender, receiver, msg_type, payload, correlation_id, acknowledged, timestamp
         FROM a2a_messages ORDER BY timestamp DESC LIMIT ?1",
        params![limit],
    ).await?;

    let mut msgs = Vec::new();
    while let Some(row) = rows.next().await? {
        msgs.push(A2AMessageRecord {
            id: row.get::<i64>(0)?,
            sender: row.get::<String>(1)?,
            receiver: row.get::<Option<String>>(2)?,
            msg_type: row.get::<String>(3)?,
            payload: row.get::<Option<String>>(4)?,
            correlation_id: row.get::<Option<String>>(5)?,
            acknowledged: row.get::<i64>(6)? != 0,
            timestamp: row.get::<String>(7)?,
        });
    }
    Ok(msgs)
}

/// Acknowledge an A2A message by ID.
pub async fn acknowledge_message(db: &VoxDb, id: i64) -> Result<()> {
    db.store()
        .conn
        .execute(
            "UPDATE a2a_messages SET acknowledged = 1 WHERE id = ?1",
            params![id],
        )
        .await?;
    Ok(())
}

// ── Agent Sessions ────────────────────────────────────────

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AgentSessionRecord {
    pub id: String,
    pub agent_id: String,
    pub agent_name: Option<String>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub status: String,
    pub task_snapshot: Option<String>,
    pub context_summary: Option<String>,
}

/// Insert a new agent session.
pub async fn insert_agent_session(
    db: &VoxDb,
    id: &str,
    agent_id: &str,
    agent_name: Option<&str>,
) -> Result<()> {
    db.store()
        .conn
        .execute(
            "INSERT OR IGNORE INTO agent_sessions (id, agent_id, agent_name) VALUES (?1, ?2, ?3)",
            params![
                id.to_string(),
                agent_id.to_string(),
                agent_name.map(|n| n.to_string()),
            ],
        )
        .await?;
    Ok(())
}

/// Update session status and optional context.
pub async fn update_agent_session(
    db: &VoxDb,
    id: &str,
    status: &str,
    task_snapshot: Option<&str>,
    context_summary: Option<&str>,
) -> Result<()> {
    db.store()
        .conn
        .execute(
            "UPDATE agent_sessions SET status = ?1, task_snapshot = ?2, context_summary = ?3
         WHERE id = ?4",
            params![
                status.to_string(),
                task_snapshot.map(|s| s.to_string()),
                context_summary.map(|s| s.to_string()),
                id.to_string(),
            ],
        )
        .await?;
    Ok(())
}

/// End a session by setting ended_at and status.
pub async fn end_agent_session(db: &VoxDb, id: &str, status: &str) -> Result<()> {
    db.store()
        .conn
        .execute(
            "UPDATE agent_sessions SET status = ?1, ended_at = datetime('now') WHERE id = ?2",
            params![status.to_string(), id.to_string()],
        )
        .await?;
    Ok(())
}

/// Get active sessions.
pub async fn list_active_sessions(db: &VoxDb) -> Result<Vec<AgentSessionRecord>> {
    let mut rows = db.store().conn.query(
        "SELECT id, agent_id, agent_name, started_at, ended_at, status, task_snapshot, context_summary
         FROM agent_sessions WHERE status = 'active' ORDER BY started_at DESC",
        (),
    ).await?;

    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await? {
        sessions.push(AgentSessionRecord {
            id: row.get::<String>(0)?,
            agent_id: row.get::<String>(1)?,
            agent_name: row.get::<Option<String>>(2)?,
            started_at: row.get::<String>(3)?,
            ended_at: row.get::<Option<String>>(4)?,
            status: row.get::<String>(5)?,
            task_snapshot: row.get::<Option<String>>(6)?,
            context_summary: row.get::<Option<String>>(7)?,
        });
    }
    Ok(sessions)
}

// ── Agent Metrics ─────────────────────────────────────────

/// Upsert an aggregated metric for an agent.
pub async fn upsert_agent_metric(
    db: &VoxDb,
    agent_id: &str,
    metric_name: &str,
    metric_value: f64,
    period: &str,
) -> Result<()> {
    db.store()
        .conn
        .execute(
            "INSERT INTO agent_metrics (agent_id, metric_name, metric_value, period)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(agent_id, metric_name, period) DO UPDATE SET
             metric_value = excluded.metric_value,
             timestamp = datetime('now')",
            params![
                agent_id.to_string(),
                metric_name.to_string(),
                metric_value,
                period.to_string(),
            ],
        )
        .await?;
    Ok(())
}

/// Get all metrics for an agent in a given period.
pub async fn get_agent_metrics(
    db: &VoxDb,
    agent_id: &str,
    period: &str,
) -> Result<std::collections::HashMap<String, f64>> {
    let mut rows = db.store().conn.query(
        "SELECT metric_name, metric_value FROM agent_metrics WHERE agent_id = ?1 AND period = ?2",
        params![agent_id.to_string(), period.to_string()],
    ).await?;

    let mut map = std::collections::HashMap::new();
    while let Some(row) = rows.next().await? {
        let name = row.get::<String>(0)?;
        let val = row.get::<f64>(1).unwrap_or(0.0);
        map.insert(name, val);
    }
    Ok(map)
}

/// Process an orchestrator event for gamification rewards (XP, crystals, companion stats).
///
/// Handles all `AgentEventKind` variants by delegating companion stat changes to
/// `Companion::interact()` (SSOT) and awarding profile XP/crystals as appropriate.
pub async fn process_event_rewards(
    db: &VoxDb,
    user_id: &str,
    event_kind: &serde_json::Value,
) -> Result<()> {
    use crate::companion::Interaction;

    // 1. Get/Create profile
    let mut profile = match get_profile(db, user_id).await? {
        Some(p) => p,
        None => crate::profile::GamifyProfile::new_default(user_id),
    };

    // 2. Extract event type and agent info
    //    serde(tag = "type", rename_all = "snake_case") → e.g. "task_completed"
    let event_type = event_kind
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let agent_id = event_kind.get("agent_id").and_then(|v| v.as_u64());
    let agent_id_str = agent_id.map(|id| format!("agent-{}", id));

    // 3. Get/Create companion for the agent involved (if any)
    let mut companion = if let Some(aid) = &agent_id_str {
        list_companions(db, user_id)
            .await?
            .into_iter()
            .find(|c| c.id == *aid)
            .unwrap_or_else(|| Companion::new(aid.clone(), user_id, aid.clone(), "vox"))
    } else {
        Companion::new("_none", user_id, "_none", "vox")
    };

    let mut profile_changed = false;
    let mut companion_changed = false;

    match event_type {
        // ── Task lifecycle ───────────────────────────────
        "task_completed" => {
            profile.add_xp(10);
            profile.add_crystals(2);
            companion.interact(Interaction::TaskCompleted);
            companion.code_quality = (companion.code_quality + 1).min(100);
            profile_changed = true;
            companion_changed = true;

            // Quest progress: AgentComplete
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::AgentComplete,
            )
            .await;
        }
        "task_started" => {
            companion.interact(Interaction::TaskAssigned);
            companion_changed = true;
        }
        "task_failed" => {
            companion.interact(Interaction::TaskFailed);
            companion_changed = true;
        }
        "task_submitted" => {
            // Informational — no stat change
        }

        // ── Agent lifecycle ──────────────────────────────
        "agent_spawned" => {
            // Ensure new companion is persisted
            companion_changed = true;
        }
        "agent_retired" => {
            companion.interact(Interaction::Idle);
            companion_changed = true;
        }
        "agent_idle" => {
            companion.interact(Interaction::Idle);
            companion_changed = true;
        }
        "agent_busy" => {
            companion.interact(Interaction::Writing);
            companion_changed = true;
        }

        // ── File locking ─────────────────────────────────
        "lock_acquired" => {
            companion.interact(Interaction::LockAcquired);
            companion_changed = true;
        }
        "lock_released" => {
            // No stat impact
        }

        // ── Communication ────────────────────────────────
        "plan_handoff" => {
            // Quest progress: Collaborate
            advance_quests(
                db,
                &mut profile,
                user_id,
                crate::quest::QuestType::Collaborate,
            )
            .await;
            profile_changed = true;
        }
        "message_sent" => {
            // Informational — no stat change
        }

        // ── Cost ─────────────────────────────────────────
        "cost_incurred" => {
            profile.spend_energy(1);
            companion.interact(Interaction::LockAcquired); // small energy cost
            profile_changed = true;
            companion_changed = true;
        }

        // ── Continuations & scope ────────────────────────
        "continuation_triggered" => {
            // Informational
        }
        "scope_violation" => {
            companion.interact(Interaction::Error);
            companion_changed = true;
        }

        // ── Activity changes ─────────────────────────────
        "activity_changed" => {
            if let Some(act) = event_kind.get("activity").and_then(|v| v.as_str()) {
                let interaction = match act {
                    "writing" => Interaction::Writing,
                    "idle" => Interaction::Idle,
                    _ => Interaction::Idle,
                };
                companion.interact(interaction);
                companion_changed = true;
            }
        }

        // ── Git & VCS ────────────────────────────────────
        "snapshot_captured" => {
            if let Some(desc) = event_kind.get("description").and_then(|v| v.as_str()) {
                let lower = desc.to_lowercase();
                if (lower.contains("clone") && lower.contains("remov"))
                    || lower.contains("optimize")
                    || lower.contains("refactor")
                    || lower.contains("clean")
                {
                    // Reward architectural cleanliness
                    profile.add_xp(25);
                    profile.add_crystals(5);
                    companion.code_quality = (companion.code_quality + 5).min(100);
                    profile_changed = true;
                    companion_changed = true;

                    // Advance Improve quests
                    advance_quests(
                        db,
                        &mut profile,
                        user_id,
                        crate::quest::QuestType::Improve,
                    )
                    .await;
                }
            }
        }
        _ => {}
    }

    // 4. Persist changes
    if profile_changed {
        upsert_profile(db, &profile).await?;
    }
    if companion_changed && agent_id_str.is_some() {
        upsert_companion(db, &companion).await?;
    }

    Ok(())
}

/// Helper: advance quests of a specific type and award bonus XP/crystals on completion.
async fn advance_quests(
    db: &VoxDb,
    profile: &mut crate::profile::GamifyProfile,
    user_id: &str,
    quest_type: crate::quest::QuestType,
) {
    if let Ok(mut quests) = list_quests(db, user_id).await {
        for q in quests.iter_mut() {
            if q.quest_type == quest_type && !q.completed {
                if q.increment(1) {
                    profile.add_xp(q.xp_reward);
                    profile.add_crystals(q.crystal_reward);
                }
                let _ = upsert_quest(db, q).await;
            }
        }
    }
}
