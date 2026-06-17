//! Integration tests for gamify policy engine, config gate, and subsystems.

use vox_config::config::GamifyMode;

// ── Router / MCP parity ───────────────────────────────────────────────────────

#[tokio::test]
async fn ludus_orchestrator_dedupe_skips_duplicate_event_id() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db)
        .await
        .expect("migrations");
    let uid = "dedupe-orchestrator-user";
    let ev = serde_json::json!({
        "type": "task_completed",
        "success": true,
        "agent_id": 7u64,
        "ludus_dedupe_id": 9001u64,
    });
    let r1 = vox_gamify::event_router::route_event(&db, uid, &ev)
        .await
        .expect("r1");
    let r2 = vox_gamify::event_router::route_event(&db, uid, &ev)
        .await
        .expect("r2");
    let xp1 = r1.reward.map(|x| x.xp).unwrap_or(0);
    let xp2 = r2.reward.map(|x| x.xp).unwrap_or(0);
    assert!(xp1 > 0, "first application should grant XP (got {xp1})");
    assert_eq!(
        xp2, 0,
        "second application with same ludus_dedupe_id must not grant XP"
    );
}

#[tokio::test]
async fn ludus_policy_snapshot_rows_track_events() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db)
        .await
        .expect("migrations");
    let uid = vox_gamify::db::canonical_user_id();
    let before = vox_gamify::db::list_recent_policy_snapshots(&db, &uid, 500)
        .await
        .expect("list")
        .len();
    let ev = serde_json::json!({
        "type": "check_completed",
        "success": true,
        "agent_id": 0u64,
    });
    vox_gamify::event_router::route_event(&db, &uid, &ev)
        .await
        .expect("route");
    let after = vox_gamify::db::list_recent_policy_snapshots(&db, &uid, 500)
        .await
        .expect("list2")
        .len();
    assert!(
        after > before,
        "expected policy snapshot row after rewarded event (before={before} after={after})"
    );
}

#[tokio::test]
async fn ludus_route_event_explicit_id_matches_auto_user_kpi() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db)
        .await
        .expect("migrations");
    let uid = vox_gamify::db::canonical_user_id();
    let ev = serde_json::json!({
        "type": "check_completed",
        "success": true,
        "agent_id": 0u64,
    });
    vox_gamify::event_router::route_event_auto_user(&db, &ev)
        .await
        .expect("auto");
    vox_gamify::event_router::route_event(&db, &uid, &ev)
        .await
        .expect("explicit");
    let k = vox_gamify::db::load_kpi_summary(&db, &uid)
        .await
        .expect("kpi");
    assert!(
        k.events_recorded >= 2,
        "both paths should attribute to canonical user (rows={})",
        k.events_recorded
    );
}

#[test]
fn ludus_validate_event_payload_rejects_oversize_json() {
    let filler = "x".repeat(300_000);
    let ev = serde_json::json!({
        "type": "task_completed",
        "agent_id": 0u64,
        "blob": filler,
    });
    assert!(
        vox_gamify::ingest::validate_event_payload(&ev).is_err(),
        "expected oversize payload to fail validation"
    );
}

// ── Reward policy ─────────────────────────────────────────────────────────────

#[test]
fn policy_streak_bonus_increases_reward() {
    use vox_gamify::reward_policy::{BaseReward, SessionState, apply_policy};
    let base = BaseReward {
        xp: 100,
        crystals: 10,
        lumens: 0,
        grant_shield: false,
    };
    let mut s_no_streak = SessionState::default();
    let mut s_streak = SessionState::default();

    let r_no = apply_policy(
        &base,
        1.0,
        0,
        vox_gamify::profile::TrustTier::Linked,
        "task_completed",
        &mut s_no_streak,
    );
    let r_14 = apply_policy(
        &base,
        1.0,
        14,
        vox_gamify::profile::TrustTier::Linked,
        "task_completed",
        &mut s_streak,
    );

    assert!(
        r_14.xp >= r_no.xp,
        "streak=14 should give at least as much XP as streak=0: {} vs {}",
        r_14.xp,
        r_no.xp
    );
}

#[test]
fn policy_serious_mode_halves_rewards() {
    use vox_gamify::reward_policy::{BaseReward, SessionState, apply_policy};
    let base = BaseReward {
        xp: 100,
        crystals: 10,
        lumens: 0,
        grant_shield: false,
    };
    let mut s_balanced = SessionState::default();
    let mut s_serious = SessionState::default();

    let r_bal = apply_policy(
        &base,
        1.0,
        0,
        vox_gamify::profile::TrustTier::Linked,
        "task_completed",
        &mut s_balanced,
    );
    let r_ser = apply_policy(
        &base,
        0.5,
        0,
        vox_gamify::profile::TrustTier::Linked,
        "task_completed",
        &mut s_serious,
    );

    assert!(
        r_ser.xp < r_bal.xp,
        "0.5× multiplier should give fewer XP than 1.0×"
    );
}

#[test]
fn policy_grind_cap_kicks_in() {
    use vox_gamify::reward_policy::{BaseReward, GRIND_ZERO_THRESHOLD, SessionState, apply_policy};
    let base = BaseReward {
        xp: 10,
        crystals: 2,
        lumens: 0,
        grant_shield: false,
    };
    let mut session = SessionState::default();

    for _ in 0..=GRIND_ZERO_THRESHOLD {
        let _ = apply_policy(
            &base,
            1.0,
            0,
            vox_gamify::profile::TrustTier::Linked,
            "task_completed",
            &mut session,
        );
    }
    let r = apply_policy(
        &base,
        1.0,
        0,
        vox_gamify::profile::TrustTier::Linked,
        "task_completed",
        &mut session,
    );
    assert!(
        r.grind_capped,
        "grind cap should trigger after {} repetitions",
        GRIND_ZERO_THRESHOLD
    );
    assert_eq!(r.xp, 0, "grind-capped reward should be 0 XP");
}

#[test]
fn policy_novelty_bonus_for_new_event_type() {
    use vox_gamify::reward_policy::{BaseReward, SessionState, apply_policy};
    let base = BaseReward {
        xp: 50,
        crystals: 5,
        lumens: 0,
        grant_shield: false,
    };
    let mut session = SessionState::default();

    // First time seeing this event type → novelty bonus
    let r = apply_policy(
        &base,
        1.0,
        0,
        vox_gamify::profile::TrustTier::Linked,
        "conflict_resolved",
        &mut session,
    );
    assert!(
        r.xp > 50,
        "first-time event should get novelty XP bonus; got {}",
        r.xp
    );
}

#[test]
fn policy_overrides_take_precedence() {
    use vox_gamify::reward_policy::{
        EventConfigOverrides, SessionState, apply_policy_with_overrides,
    };
    let mut overrides = EventConfigOverrides::default();
    overrides.set("task_completed", 999, 99);
    let mut session = SessionState::default();

    let r = apply_policy_with_overrides(&overrides, 1.0, 0, "task_completed", &mut session);
    // Should be at least 999 XP (novelty bonus may push it higher)
    assert!(
        r.xp >= 999,
        "overridden base XP 999 should reflect in reward; got {}",
        r.xp
    );
}

#[test]
fn base_rewards_are_nonzero_for_known_events() {
    use vox_gamify::reward_policy::base_reward;
    let events = [
        "task_completed",
        "task_submitted",
        "bug_fix",
        "snapshot_captured",
        "conflict_resolved",
        "test_pass",
        "refactor",
        "build_completed",
        "check_completed",
        "fmt_completed",
    ];
    for event in &events {
        let b = base_reward(event);
        assert!(
            b.xp > 0 || b.crystals > 0,
            "event '{}' must have non-zero base reward",
            event
        );
    }
}

// ── Config gate ───────────────────────────────────────────────────────────────

#[test]
fn gamify_defaults_enabled_balanced() {
    let cfg = vox_config::VoxConfig::default();
    assert!(cfg.gamify_enabled, "gamify should be enabled by default");
    assert_eq!(cfg.gamify_mode, GamifyMode::Balanced);
    assert!((cfg.gamify_mode.reward_multiplier() - 1.0).abs() < 0.01);
    assert!(cfg.gamify_mode.show_overlays());
}

#[test]
fn serious_mode_disables_overlays_and_hints() {
    let cfg_mode = GamifyMode::Serious;
    assert!(
        !cfg_mode.show_overlays(),
        "serious mode must suppress overlays"
    );
    assert_eq!(
        cfg_mode.hint_frequency(),
        0.0,
        "serious mode must have zero hint frequency"
    );
    assert!(
        cfg_mode.reward_multiplier() < 1.0,
        "serious mode must have reduced rewards"
    );
}

#[test]
fn learning_mode_amplifies_all() {
    let cfg_mode = GamifyMode::Learning;
    assert!(
        cfg_mode.reward_multiplier() > 1.0,
        "learning mode must amplify rewards"
    );
    assert!(
        cfg_mode.show_overlays(),
        "learning mode must allow overlays"
    );
    assert_eq!(
        cfg_mode.hint_frequency(),
        1.0,
        "learning mode must max hint frequency"
    );
}

#[test]
fn config_set_key_gamify_mode_roundtrip() {
    let mut cfg = vox_config::VoxConfig::default();
    assert!(cfg.set_key("gamify.mode", "serious"));
    assert_eq!(cfg.gamify_mode, GamifyMode::Serious);
    assert!(cfg.set_key("gamify.mode", "learning"));
    assert_eq!(cfg.gamify_mode, GamifyMode::Learning);
    assert!(cfg.set_key("gamify.mode", "balanced"));
    assert_eq!(cfg.gamify_mode, GamifyMode::Balanced);
}

#[test]
fn config_set_key_gamify_enabled_roundtrip() {
    let mut cfg = vox_config::VoxConfig::default();
    assert!(cfg.set_key("gamify.enabled", "false"));
    assert!(!cfg.gamify_enabled);
    assert!(cfg.set_key("gamify.enabled", "true"));
    assert!(cfg.gamify_enabled);
}

// ── Teaching subsystem ────────────────────────────────────────────────────────

#[test]
fn teaching_serious_mode_suppresses_all_hints() {
    use vox_gamify::teaching::{MistakeKind, TeachingProfile};
    let mut profile = TeachingProfile::default();
    // serious mode = hint_frequency 0.0
    let req = profile.record_mistake(MistakeKind::SyntaxError, 0.0);
    assert!(
        req.is_none(),
        "serious mode (freq=0.0) must not generate hints"
    );
}

#[test]
fn teaching_learning_mode_always_hints_on_first_mistake() {
    use vox_gamify::teaching::{MistakeKind, TeachingProfile};
    let mut profile = TeachingProfile::default();
    let req = profile.record_mistake(MistakeKind::SyntaxError, 1.0);
    assert!(
        req.is_some(),
        "learning mode (freq=1.0) must hint on first mistake"
    );
}

#[test]
fn teaching_cooldown_suppresses_repeat() {
    use vox_gamify::teaching::{MistakeKind, TeachingProfile};
    let mut profile = TeachingProfile::default();
    let first = profile.record_mistake(MistakeKind::TestFailure, 1.0);
    assert!(first.is_some(), "first TestFailure should hint");
    let second = profile.record_mistake(MistakeKind::TestFailure, 1.0);
    assert!(
        second.is_none(),
        "second immediate mistake should be blocked by cooldown"
    );
}

// ── Quest engine archetypes ───────────────────────────────────────────────────

#[test]
fn quest_engine_archetype_is_deterministic_for_user() {
    use vox_gamify::quest_engine::QuestArchetype;
    // Same user+day should always yield the same archetype
    let a1 = QuestArchetype::today_for_user("user-stable");
    let a2 = QuestArchetype::today_for_user("user-stable");
    assert_eq!(
        format!("{:?}", a1),
        format!("{:?}", a2),
        "archetype must be deterministic for same user and day"
    );
}

#[test]
fn quest_engine_archetype_differs_by_user() {
    use vox_gamify::quest_engine::QuestArchetype;
    // Different users may (statistically) get different archetypes
    // With 4 archetypes over enough users, at least 2 must differ
    let users = ["user-a", "user-b", "user-c", "user-d", "user-e", "user-f"];
    let archetypes: std::collections::HashSet<String> = users
        .iter()
        .map(|u| format!("{:?}", QuestArchetype::today_for_user(u)))
        .collect();
    assert!(
        archetypes.len() >= 2,
        "different users should get varied archetypes"
    );
}

#[tokio::test]
async fn test_telemetry_shared_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db)
        .await
        .expect("migrations");
    let ev = serde_json::json!({
        "type": "telemetry_shared",
        "source": "vox-telemetry",
        "payload": { "bytes_shared": 1024 },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev)
        .await
        .expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 15);
    assert_eq!(rw.crystals, 3);
}

#[tokio::test]
async fn test_vox_feature_milestone_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db)
        .await
        .expect("migrations");
    let ev = serde_json::json!({
        "type": "vox_feature_milestone",
        "source": "vox-compiler",
        "payload": { "feature": "actor" },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 31);
    assert_eq!(rw.crystals, 6);
}

#[tokio::test]
async fn test_skill_published_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db).await.expect("migrations");
    let ev = serde_json::json!({
        "type": "skill_published",
        "source": "vox-skills",
        "payload": { "skill_name": "my-skill" },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 77);
    assert_eq!(rw.crystals, 15);
}

#[tokio::test]
async fn test_skill_gossiped_event_routing() {
    let db = vox_db::VoxDb::open_memory().await.expect("db");
    vox_gamify::db::apply_ludus_migrations(&db).await.expect("migrations");
    let ev = serde_json::json!({
        "type": "skill_gossiped",
        "source": "vox-populi",
        "payload": { "peer_id": "peer-node-1" },
    });
    let res = vox_gamify::event_router::route_event_auto_user(&db, &ev).await.expect("route");
    let rw = res.reward.expect("reward");
    assert_eq!(rw.xp, 115);
    assert_eq!(rw.crystals, 23);
    assert_eq!(rw.lumens, 15);
}
