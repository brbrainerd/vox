use serial_test::serial;
use tempfile::TempDir;

use crate::types::AgentId;

use super::super::config::SessionConfig;
use super::super::errors::SessionError;
use super::super::state::{SessionState, now_secs};
use super::SessionManager;

fn session_defaults() -> SessionConfig {
    SessionConfig {
        sessions_dir: std::path::PathBuf::new(),
        repository_id: None,
        idle_timeout_secs: 30,
        archive_timeout_secs: 60,
        max_sessions: 4,
        persist: true,
    }
}

/// Unique sessions directory per call; keep the returned [`TempDir`] in scope for the whole test.
fn test_config() -> (SessionConfig, TempDir) {
    let dir = TempDir::new().expect("tempdir");
    let mut c = session_defaults();
    c.sessions_dir = dir.path().to_path_buf();
    (c, dir)
}

#[test]
fn create_and_retrieve_session() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create session");
    let session = mgr.get(&id).expect("get session");
    assert_eq!(session.agent_id, AgentId(1));
    assert_eq!(session.state, SessionState::Active);
    assert_eq!(session.turns.len(), 0);
}

#[test]
fn add_turn_and_check_tokens() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create");
    mgr.add_turn(&id, "user", "hello world", 3)
        .expect("add turn");
    let s = mgr.get(&id).expect("get");
    assert_eq!(s.turns.len(), 1);
    assert_eq!(s.current_tokens(), 3);
    assert_eq!(s.turn_count, 1);
    assert_eq!(s.total_tokens, 3);
}

#[test]
fn reset_clears_history() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create");
    mgr.add_turn(&id, "user", "hello", 2).expect("add");
    mgr.add_turn(&id, "assistant", "hi", 1).expect("add");
    let cleared = mgr.reset(&id).expect("reset");
    assert_eq!(cleared, 2);
    assert_eq!(mgr.get(&id).expect("get").turns.len(), 0);
}

#[test]
fn compact_replaces_with_summary() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create");
    mgr.add_turn(&id, "user", "lots of content", 100)
        .expect("add");
    mgr.add_turn(&id, "assistant", "response", 50).expect("add");
    let removed = mgr
        .compact(&id, "Session summary: fixed parser")
        .expect("compact");
    assert_eq!(removed, 1); // 2 turns → replace with 1 summary → removed = 2-1
    assert_eq!(mgr.get(&id).expect("get").turns.len(), 1);
    assert_eq!(mgr.get(&id).expect("get").state, SessionState::Compacted);
}

/// T4.2: `compact()` must not silently discard dropped turns — they must be
/// retrievable afterward via `Session::archived_turns()` with exact content.
#[test]
fn compact_archives_dropped_turns_losslessly() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create");
    mgr.add_turn(&id, "user", "lots of content", 100)
        .expect("add");
    mgr.add_turn(&id, "assistant", "response", 50).expect("add");

    mgr.compact(&id, "Session summary: fixed parser")
        .expect("compact");

    let session = mgr.get(&id).expect("get");
    let archived = session.archived_turns();
    assert_eq!(archived.len(), 2, "both original turns must be archived");
    assert_eq!(archived[0].content, "lots of content");
    assert_eq!(archived[1].content, "response");
    // The live turns list now holds only the summary, but the dropped
    // content is not gone — it survived in the archive.
    assert_eq!(session.turns.len(), 1);
}

/// T4.2: automatic compaction (the wiring path used before an LLM call) also
/// archives losslessly and only fires once the session is over threshold.
#[test]
fn compact_auto_noop_under_threshold_then_archives_over_threshold() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create");
    mgr.add_turn(&id, "user", "short", 5).expect("add");

    let engine = crate::compaction::CompactionEngine::default();
    let session = mgr.get_mut(&id).expect("get_mut");
    assert!(
        session.compact_auto(&engine).is_none(),
        "well under the 128k-token default threshold — must be a no-op"
    );
    assert_eq!(session.turns.len(), 1, "no-op must not mutate turns");
    assert!(session.archived_turns().is_empty());

    // Now push it over a tiny custom threshold and confirm real archival.
    let tiny = crate::compaction::CompactionEngine::new(crate::compaction::CompactionConfig {
        max_context_tokens: 100,
        reserved_tokens: 10,
        compaction_threshold: 0.5,
        min_viable_tokens: 5,
        strategy: crate::compaction::CompactionStrategy::Balanced,
        head_preserve_tokens: 10,
        tail_preserve_tokens: 15,
        complexity_token_weight: 32,
    });
    mgr.add_turn(&id, "assistant", "b", 20).expect("add");
    mgr.add_turn(&id, "user", "c", 20).expect("add");
    mgr.add_turn(&id, "assistant", "d", 20).expect("add");
    let session = mgr.get_mut(&id).expect("get_mut");
    let result = session
        .compact_auto(&tiny)
        .expect("must compact — over threshold");
    assert!(result.compacted);
    assert!(result.dropped_count > 0);
    assert_eq!(session.archived_turns().len(), result.dropped_count);
}

#[test]
fn set_meta_persisted() {
    let (cfg, _dir) = test_config();
    let mut mgr = SessionManager::new(cfg).expect("create manager");
    let id = mgr.create(AgentId(1), None).expect("create");
    mgr.set_meta(&id, "model", "claude-sonnet-4")
        .expect("set meta");
    let val = mgr.get(&id).expect("get").meta.get("model").cloned();
    assert_eq!(val.as_deref(), Some("claude-sonnet-4"));
}

#[test]
fn max_sessions_limit() {
    let (base, _dir) = test_config();
    let cfg = SessionConfig {
        max_sessions: 2,
        ..base
    };
    let mut mgr = SessionManager::new(cfg).expect("create");
    mgr.create(AgentId(1), None).expect("1st");
    mgr.create(AgentId(2), None).expect("2nd");
    let err = mgr.create(AgentId(3), None);
    assert!(matches!(err, Err(SessionError::MaxSessions(2))));
}

#[test]
fn lifecycle_tick_marks_idle_then_archives() {
    let (base, _dir) = test_config();
    let cfg = SessionConfig {
        idle_timeout_secs: 10,
        archive_timeout_secs: 10,
        ..base
    };
    let mut mgr = SessionManager::new(cfg).expect("create");
    let id = mgr.create(AgentId(1), None).expect("create");
    // Force last_active to be far in the past
    if let Some(s) = mgr.get_mut(&id) {
        s.last_active = now_secs().saturating_sub(20);
    }
    mgr.tick_lifecycle();
    mgr.tick_lifecycle();
    assert_eq!(mgr.get(&id).expect("get").state, SessionState::Archived);
}

#[test]
fn cleanup_removes_archived_sessions() {
    let (base, _dir) = test_config();
    let cfg = SessionConfig {
        idle_timeout_secs: 1,
        archive_timeout_secs: 1,
        ..base
    };
    let mut mgr = SessionManager::new(cfg).expect("create");
    let id = mgr.create(AgentId(1), None).expect("create");
    if let Some(s) = mgr.get_mut(&id) {
        s.state = SessionState::Archived;
    }
    let removed = mgr.cleanup().expect("cleanup");
    assert_eq!(removed, 1);
    assert!(mgr.get(&id).is_none());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[serial]
async fn session_db_replay_matches_in_memory_state() {
    let db = std::sync::Arc::new(vox_db::VoxDb::open_memory().await.expect("mem db"));
    let (mut cfg, _dir) = test_config();
    cfg.persist = false;
    let mut mgr = SessionManager::new(cfg).unwrap().with_db(db.clone());
    let id = mgr.create(AgentId(11), None).expect("create");
    mgr.add_turn(&id, "user", "hello", 5).expect("turn");
    mgr.set_meta(&id, "k", "v").expect("meta");
    mgr.set_plugin_state(&id, "p", serde_json::json!({"x": 1}))
        .expect("plugin");
    mgr.reset(&id).expect("reset");
    mgr.add_turn(&id, "assistant", "after reset", 3)
        .expect("turn2");
    let live = mgr.get(&id).expect("live").clone();

    let (mut cfg2, _dir2) = test_config();
    cfg2.persist = false;
    let mut mgr2 = SessionManager::new(cfg2).unwrap().with_db(db);
    mgr2.load(&id).await.expect("load from db");
    let replayed = mgr2.get(&id).expect("replay").clone();

    assert_eq!(
        serde_json::to_value(&live).unwrap(),
        serde_json::to_value(&replayed).unwrap()
    );
}
