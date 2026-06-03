//! Typed gamification (Ludus) Tauri commands. These mirror the CLI's gamify DB
//! access exactly: connect via `Codex::connect(DbConfig::resolve_for_mesh())`
//! and call the `vox_gamify::db` API. See `crates/vox-cli/src/commands/extras/ludus/`.

use vox_gamify::notifications::{Notification, NotificationType};
use vox_gamify::profile::LudusProfile;

#[derive(Debug, serde::Serialize)]
pub struct LudusProfileDto {
    pub user_id: String,
    pub level: u64,
    pub xp: u64,
    pub xp_to_next_level: u64,
    pub xp_progress: f64,
    pub total_xp_earned: u64,
    pub crystals: u64,
    pub lumens: i64,
    pub energy: u64,
    pub max_energy: u64,
    pub current_streak: u64,
    pub prestige_level: u32,
    pub title: String,
    pub full_title: String,
    pub trust_tier: String,
}

impl LudusProfileDto {
    fn from_profile(p: &LudusProfile) -> Self {
        Self {
            user_id: p.user_id.clone(),
            level: p.level,
            xp: p.xp,
            xp_to_next_level: p.xp_to_next_level(),
            xp_progress: p.xp_progress(),
            total_xp_earned: p.total_xp_earned,
            crystals: p.crystals,
            lumens: p.lumens,
            energy: p.energy,
            max_energy: p.max_energy,
            current_streak: p.streak.current_streak,
            prestige_level: p.prestige_level,
            title: p.title(),
            full_title: p.full_title(),
            trust_tier: format!("{:?}", p.trust_tier),
        }
    }
}

/// Map a notification kind to a banner/toast severity (`ok`/`warn`/`info`).
pub(crate) fn notification_level(t: &NotificationType) -> &'static str {
    match t {
        NotificationType::LevelUp
        | NotificationType::AchievementUnlocked
        | NotificationType::QuestCompleted
        | NotificationType::ChallengeCompleted
        | NotificationType::BattleWon
        | NotificationType::StreakContinued => "ok",
        NotificationType::StreakLost | NotificationType::BattleLost => "warn",
        _ => "info",
    }
}

#[derive(Debug, serde::Serialize)]
pub struct LudusNotificationDto {
    pub id: String,
    pub level: String,
    pub title: String,
    pub message: String,
    pub created_at: i64,
    pub kind: String,
}

impl LudusNotificationDto {
    fn from_notification(n: &Notification) -> Self {
        Self {
            id: n.id.clone(),
            level: notification_level(&n.notification_type).to_string(),
            title: n.title.clone(),
            message: n.message.clone(),
            created_at: n.created_at,
            kind: format!("{:?}", n.notification_type),
        }
    }
}

async fn open_gamify_db() -> Result<vox_db::Codex, String> {
    let config = vox_db::DbConfig::resolve_for_mesh().map_err(|e| e.to_string())?;
    vox_db::Codex::connect(config).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_ludus_profile() -> Result<LudusProfileDto, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let mut profile = vox_gamify::db::get_profile(&db, &user_id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| LudusProfile::new_default(&user_id));
    profile.regen_energy();
    Ok(LudusProfileDto::from_profile(&profile))
}

#[tauri::command]
pub async fn list_ludus_notifications(limit: Option<u32>) -> Result<Vec<LudusNotificationDto>, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let notes = vox_gamify::db::list_unread_notifications(&db, &user_id, limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(notes.iter().map(LudusNotificationDto::from_notification).collect())
}

#[tauri::command]
pub async fn ack_ludus_notification(notification_id: String) -> Result<(), String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    vox_gamify::db::mark_notification_read_for_user(&db, &user_id, &notification_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Best-effort: map unread gamify notifications to the GUI alert JSON shape
/// (`{id, level, title, body}`) consumed by `LudusBanner`. Returns empty on any error.
pub async fn fetch_gamify_alerts() -> Vec<serde_json::Value> {
    let Ok(config) = vox_db::DbConfig::resolve_for_mesh() else {
        return Vec::new();
    };
    let Ok(db) = vox_db::Codex::connect(config).await else {
        return Vec::new();
    };
    let user_id = vox_gamify::db::canonical_user_id();
    let notes = match vox_gamify::db::list_unread_notifications(&db, &user_id, 10).await {
        Ok(n) => n,
        Err(_) => return Vec::new(),
    };
    notes
        .iter()
        .map(|n| {
            serde_json::json!({
                "id": n.id,
                "level": notification_level(&n.notification_type),
                "title": n.title,
                "body": n.message,
            })
        })
        .collect()
}

#[derive(Debug, serde::Serialize)]
pub struct GamifySettingsDto {
    pub enabled: bool,
    pub mode: String,
}

#[tauri::command]
pub async fn get_gamify_settings() -> Result<GamifySettingsDto, String> {
    let cfg = vox_gamify::config_gate::load_disk();
    Ok(GamifySettingsDto {
        enabled: cfg.gamify_enabled,
        mode: cfg.gamify_mode.as_config_str().to_string(),
    })
}

#[tauri::command]
pub async fn set_gamify_settings(enabled: bool, mode: String) -> Result<(), String> {
    let mut cfg = vox_gamify::config_gate::load_disk();
    cfg.gamify_enabled = enabled;
    cfg.gamify_mode = match mode.to_lowercase().as_str() {
        "serious" => vox_config::GamifyMode::Serious,
        "learning" => vox_config::GamifyMode::Learning,
        _ => vox_config::GamifyMode::Balanced,
    };
    cfg.save().map_err(|e| format!("save config: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dto_maps_core_fields_and_bounds_progress() {
        let mut p = LudusProfile::new_default("u1");
        p.level = 3;
        p.xp = 200;
        p.crystals = 42;
        let dto = LudusProfileDto::from_profile(&p);
        assert_eq!(dto.user_id, "u1");
        assert_eq!(dto.level, 3);
        assert_eq!(dto.crystals, 42);
        assert!(dto.xp_progress >= 0.0 && dto.xp_progress <= 1.0);
        assert!(!dto.title.is_empty());
    }

    #[test]
    fn notification_level_maps_severity() {
        assert_eq!(notification_level(&NotificationType::LevelUp), "ok");
        assert_eq!(notification_level(&NotificationType::StreakLost), "warn");
        assert_eq!(notification_level(&NotificationType::CompanionStatus), "info");
    }
}
