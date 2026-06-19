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
    vox_db::Codex::connect(config)
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_ludus_profile_impl(db: &vox_db::Codex) -> Result<LudusProfileDto, String> {
    let user_id = vox_gamify::db::canonical_user_id();
    let mut profile = vox_gamify::db::get_profile(db, &user_id)
        .await
        .map_err(|e| e.to_string())?
        .unwrap_or_else(|| LudusProfile::new_default(&user_id));
    profile.regen_energy();
    if let Err(e) = vox_gamify::db::upsert_profile(db, &profile).await {
        tracing::error!("failed to upsert profile after energy regen: {}", e);
    }
    Ok(LudusProfileDto::from_profile(&profile))
}

#[tauri::command]
pub async fn get_ludus_profile() -> Result<LudusProfileDto, String> {
    let db = open_gamify_db().await?;
    get_ludus_profile_impl(&db).await
}

#[tauri::command]
pub async fn list_ludus_notifications(
    limit: Option<u32>,
) -> Result<Vec<LudusNotificationDto>, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let notes = vox_gamify::db::list_unread_notifications(&db, &user_id, limit.unwrap_or(20))
        .await
        .map_err(|e| e.to_string())?;
    Ok(notes
        .iter()
        .map(LudusNotificationDto::from_notification)
        .collect())
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

/// Map GUI hook names from the TypeScript layer to reward_policy event types.
pub(crate) fn map_gui_hook_event_type(hook: &str) -> &str {
    match hook {
        "chat_message_sent" => "message_sent",
        "task_submitted" => "task_submitted",
        "search_query_executed" => "gui_search_query",
        "policy_rule_viewed" => "gui_policy_viewed",
        "palette_navigation" => "gui_palette_nav",
        "console_command_success" => "gui_console_command",
        "discovery_action_used" => "gui_discovery_action",
        "model_activated" => "gui_model_activated",
        "approval_decision" => "gui_approval_decision",
        "browser_preview_loaded" => "gui_browser_preview",
        "mesh_dispatch_success" => "gui_mesh_dispatch",
        "isolation_strategy_set" => "gui_isolation_strategy",
        "isolation_scan_complete" => "gui_isolation_scan",
        "harness_redirect_viewed" => "gui_harness_redirect",
        "breadcrumb_navigation" => "gui_breadcrumb_nav",
        "claim_approved" => "gui_claim_approved",
        "nanopub_built" => "gui_nanopub_built",
        "secret_rotated" => "gui_secret_rotated",
        "signing_key_rotated" => "gui_signing_key_rotated",
        "orchestrator_first_connect" => "gui_orchestrator_connect",
        other => other,
    }
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuiEventResultDto {
    pub xp_granted: u32,
    pub lumens_granted: u32,
    pub achievement_title: Option<String>,
}

fn gui_event_result_from_route(
    result: &vox_gamify::reward_policy::RouteResult,
) -> GuiEventResultDto {
    let xp = result.reward.as_ref().map(|r| r.xp).unwrap_or(0);
    let lumens = result
        .reward
        .as_ref()
        .map(|r| r.lumens.max(0) as u32)
        .unwrap_or(0);
    let achievement_title = if let Some((_, title)) = &result.leveled_up {
        Some(title.clone())
    } else if xp > 0 {
        Some("XP".to_string())
    } else if lumens > 0 {
        Some("Lumens".to_string())
    } else {
        None
    };
    GuiEventResultDto {
        xp_granted: xp.min(u32::MAX as u64) as u32,
        lumens_granted: lumens,
        achievement_title,
    }
}

fn merge_gui_event_json(hook: &str, metadata: Option<serde_json::Value>) -> serde_json::Value {
    let routed = map_gui_hook_event_type(hook);
    let mut event_json = serde_json::json!({ "type": routed, "source": "gui" });
    if let Some(serde_json::Value::Object(meta)) = metadata {
        if let Some(obj) = event_json.as_object_mut() {
            for (k, v) in meta {
                if k != "type" {
                    obj.insert(k, v);
                }
            }
        }
    }
    event_json
}

/// Thin Tauri bridge: GUI hooks → `vox_gamify::event_router` (no XP math in TS).
#[tauri::command]
pub async fn record_gui_event(
    event_type: String,
    metadata: Option<serde_json::Value>,
) -> Result<GuiEventResultDto, String> {
    let event_json = merge_gui_event_json(&event_type, metadata);
    let db = open_gamify_db().await?;
    let routed = vox_gamify::event_router::route_event_auto_user(&db, &event_json)
        .await
        .map_err(|e| e.to_string())?;
    Ok(gui_event_result_from_route(&routed))
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

// ── Leaderboard / companions / quests surfaces (F3) ──────────────────────────

#[derive(Debug, serde::Serialize)]
pub struct LeaderboardEntryDto {
    pub rank: u32,
    pub user_id: String,
    pub level: u64,
    pub score: i64,
}

/// Top players by XP. Mirrors `vox ludus leaderboard` (the CLI's
/// `leaderboard_show`) over `vox_gamify::db::leaderboard`.
#[tauri::command]
pub async fn list_gamify_leaderboard(
    limit: Option<u32>,
) -> Result<Vec<LeaderboardEntryDto>, String> {
    let db = open_gamify_db().await?;
    let rows = vox_gamify::db::leaderboard(&db, limit.unwrap_or(20) as i64)
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .iter()
        .enumerate()
        .map(|(i, r)| LeaderboardEntryDto {
            rank: (i + 1) as u32,
            user_id: r.user_id.clone(),
            level: r.level,
            score: r.score,
        })
        .collect())
}

#[derive(Debug, serde::Serialize)]
pub struct CompanionDto {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub language: String,
    pub mood: String,
    pub health: i32,
    pub max_health: i32,
    pub energy: i32,
    pub max_energy: i32,
    pub code_quality: u8,
    pub last_active: i64,
    /// Inline SVG markup (no external runtime) rendered from the companion's
    /// current mood via `sprite_svg::generate_svg_from_mood`.
    pub svg: String,
}

/// The user's companions, each with a freshly rendered mood SVG. Mirrors
/// `vox_gamify::db::list_companions`.
#[tauri::command]
pub async fn list_gamify_companions() -> Result<Vec<CompanionDto>, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let companions = vox_gamify::db::list_companions(&db, &user_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(companions
        .iter()
        .map(|c| CompanionDto {
            id: c.id.clone(),
            name: c.name.clone(),
            description: c.description.clone(),
            language: c.language.clone(),
            mood: format!("{:?}", c.mood),
            health: c.health,
            max_health: c.max_health,
            energy: c.energy,
            max_energy: c.max_energy,
            code_quality: c.code_quality,
            last_active: c.last_active,
            svg: vox_gamify::sprite_svg::generate_svg_from_mood(c.mood, None).svg_body,
        })
        .collect())
}

#[derive(Debug, serde::Serialize)]
pub struct QuestDto {
    pub id: String,
    pub quest_type: String,
    pub description: String,
    pub hint: String,
    pub target: u32,
    pub progress: u32,
    pub xp_reward: u64,
    pub crystal_reward: u64,
    pub completed: bool,
    pub status: String,
    pub expires_at: i64,
}

/// The user's active quests. Mirrors `vox_gamify::db::list_quests`.
#[tauri::command]
pub async fn list_gamify_quests() -> Result<Vec<QuestDto>, String> {
    let db = open_gamify_db().await?;
    let user_id = vox_gamify::db::canonical_user_id();
    let quests = vox_gamify::db::list_quests(&db, &user_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(quests
        .iter()
        .map(|q| QuestDto {
            id: q.id.clone(),
            quest_type: format!("{:?}", q.quest_type),
            description: q.description.clone(),
            hint: q.hint.clone(),
            target: q.target,
            progress: q.progress,
            xp_reward: q.xp_reward,
            crystal_reward: q.crystal_reward,
            completed: q.completed,
            status: q.status.clone(),
            expires_at: q.expires_at,
        })
        .collect())
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
        assert_eq!(
            notification_level(&NotificationType::CompanionStatus),
            "info"
        );
    }

    #[test]
    fn map_gui_hook_event_type_routes_known_hooks() {
        assert_eq!(map_gui_hook_event_type("chat_message_sent"), "message_sent");
        assert_eq!(map_gui_hook_event_type("task_submitted"), "task_submitted");
        assert_eq!(
            map_gui_hook_event_type("search_query_executed"),
            "gui_search_query"
        );
        assert_eq!(
            map_gui_hook_event_type("policy_rule_viewed"),
            "gui_policy_viewed"
        );
        assert_eq!(
            map_gui_hook_event_type("palette_navigation"),
            "gui_palette_nav"
        );
        assert_eq!(
            map_gui_hook_event_type("console_command_success"),
            "gui_console_command"
        );
        assert_eq!(
            map_gui_hook_event_type("discovery_action_used"),
            "gui_discovery_action"
        );
        assert_eq!(
            map_gui_hook_event_type("model_activated"),
            "gui_model_activated"
        );
        assert_eq!(
            map_gui_hook_event_type("approval_decision"),
            "gui_approval_decision"
        );
        assert_eq!(
            map_gui_hook_event_type("browser_preview_loaded"),
            "gui_browser_preview"
        );
        assert_eq!(
            map_gui_hook_event_type("claim_approved"),
            "gui_claim_approved"
        );
        assert_eq!(
            map_gui_hook_event_type("nanopub_built"),
            "gui_nanopub_built"
        );
        assert_eq!(
            map_gui_hook_event_type("secret_rotated"),
            "gui_secret_rotated"
        );
        assert_eq!(
            map_gui_hook_event_type("signing_key_rotated"),
            "gui_signing_key_rotated"
        );
        assert_eq!(
            map_gui_hook_event_type("orchestrator_first_connect"),
            "gui_orchestrator_connect"
        );
        assert_eq!(
            map_gui_hook_event_type("isolation_scan_complete"),
            "gui_isolation_scan"
        );
    }

    #[test]
    fn gui_event_result_from_route_maps_xp_and_level_up_title() {
        use vox_gamify::reward_policy::{PolicyReward, RouteResult};

        let leveled = gui_event_result_from_route(&RouteResult {
            reward: Some(PolicyReward {
                xp: 120,
                crystals: 0,
                lumens: 0,
                grant_shield: false,
                effective_multiplier: 1.0,
                grind_capped: false,
            }),
            leveled_up: Some((5, "Initiate".to_string())),
        });
        assert_eq!(leveled.xp_granted, 120);
        assert_eq!(leveled.achievement_title.as_deref(), Some("Initiate"));

        let xp_only = gui_event_result_from_route(&RouteResult {
            reward: Some(PolicyReward {
                xp: 5,
                crystals: 0,
                lumens: 0,
                grant_shield: false,
                effective_multiplier: 1.0,
                grind_capped: false,
            }),
            leveled_up: None,
        });
        assert_eq!(xp_only.xp_granted, 5);
        assert_eq!(xp_only.achievement_title.as_deref(), Some("XP"));
    }

    #[test]
    fn merge_gui_event_json_includes_metadata_and_routed_type() {
        let json = merge_gui_event_json(
            "chat_message_sent",
            Some(serde_json::json!({ "session_id": "abc" })),
        );
        assert_eq!(json["type"], "message_sent");
        assert_eq!(json["source"], "gui");
        assert_eq!(json["session_id"], "abc");
    }

    #[tokio::test]
    async fn energy_regen_persists_to_db() {
        let db = vox_db::VoxDb::connect(vox_db::DbConfig::Memory)
            .await
            .expect("db");
        vox_gamify::db::apply_ludus_migrations(&db)
            .await
            .expect("migrations");
        let user_id = vox_gamify::db::canonical_user_id();

        let mut p = LudusProfile::new_default(&user_id);
        p.energy = 0;
        p.last_energy_regen = vox_gamify::util::now_unix() - 7200; // 2 hours ago
        vox_gamify::db::upsert_profile(&db, &p)
            .await
            .expect("upsert");

        // Call the impl
        let _dto = get_ludus_profile_impl(&db).await.expect("impl");

        // Reload from DB
        let reloaded = vox_gamify::db::get_profile(&db, &user_id)
            .await
            .expect("get")
            .unwrap();
        assert!(reloaded.energy > 0, "energy must persist after regen");
    }
}
