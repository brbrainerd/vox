use super::config::memory_config_for_state;
use super::{RetrievalTriggerMode, run_retrieval_bundle};
use crate::server_state::ServerState;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use vox_orchestrator::{
    AffinityGroupRegistry, Orchestrator, OrchestratorConfig, SessionConfig, SessionManager,
};
use vox_repository::{RepoCapabilities, RepositoryContext};
use vox_skills::new_registry_arc;

#[test]
fn memory_config_for_state_matches_orchestrator_memory() {
    let custom = std::env::temp_dir().join("vox_mcp_memory_config_test");
    let mut cfg = OrchestratorConfig::default();
    cfg.memory.log_dir = custom.clone();
    cfg.memory.memory_md_path = custom.join("MEMORY.md");
    let orch_cfg = cfg.clone();
    let groups = AffinityGroupRegistry::new(vec![]);
    let session_cfg = SessionConfig {
        persist: false,
        sessions_dir: std::env::temp_dir().join("vox-mcp-test-sessions"),
        ..SessionConfig::default()
    };
    let session_manager = SessionManager::new(session_cfg).expect("session manager");
    let repository = RepositoryContext {
        root: PathBuf::from("."),
        git_root: None,
        repository_id: "test".into(),
        origin_url: None,
        capabilities: RepoCapabilities {
            vox_project: false,
            cargo_workspace: false,
            cargo_package: false,
            node_workspace: false,
            python_project: false,
            go_module: false,
            git: false,
        },
        has_vox_agents_dir: false,
        vox_toml: None,
    };
    let state = ServerState::test_stub(
        cfg.clone(),
        repository,
        Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
        Arc::new(Mutex::new(session_manager)),
        new_registry_arc(),
    );
    let mc = memory_config_for_state(&state);
    assert_eq!(mc.log_dir, custom);
    assert_eq!(mc.memory_md_path, custom.join("MEMORY.md"));
}

#[tokio::test]
async fn retrieval_bundle_prefers_bm25_before_lexical_fallback() {
    let unique = format!(
        "vox_mcp_retrieval_test_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    );
    let custom = std::env::temp_dir().join(unique);
    let mem_dir = custom.join("memory");
    fs::create_dir_all(&mem_dir).expect("create memory dir");
    fs::write(
        mem_dir.join("notes.md"),
        "hybrid retrieval should find this bm25 keyword token",
    )
    .expect("write notes");
    let mut cfg = OrchestratorConfig::default();
    cfg.memory.log_dir = mem_dir.clone();
    cfg.memory.memory_md_path = mem_dir.join("MEMORY.md");
    let orch_cfg = cfg.clone();
    let groups = AffinityGroupRegistry::new(vec![]);
    let session_cfg = SessionConfig {
        persist: false,
        sessions_dir: std::env::temp_dir().join("vox-mcp-test-sessions"),
        ..SessionConfig::default()
    };
    let session_manager = SessionManager::new(session_cfg).expect("session manager");
    let repository = RepositoryContext {
        root: PathBuf::from("."),
        git_root: None,
        repository_id: "test".into(),
        origin_url: None,
        capabilities: RepoCapabilities {
            vox_project: false,
            cargo_workspace: false,
            cargo_package: false,
            node_workspace: false,
            python_project: false,
            go_module: false,
            git: false,
        },
        has_vox_agents_dir: false,
        vox_toml: None,
    };
    let state = ServerState::test_stub(
        cfg.clone(),
        repository,
        Arc::new(Orchestrator::with_groups(orch_cfg, groups)),
        Arc::new(Mutex::new(session_manager)),
        new_registry_arc(),
    );
    let bundle = run_retrieval_bundle(
        &state,
        "bm25 keyword token",
        RetrievalTriggerMode::ExplicitToolQuery,
        5,
        None,
    )
    .await
    .expect("retrieval bundle");
    assert!(bundle.evidence.used_bm25);
    assert!(!bundle.evidence.used_lexical_fallback);
    assert_eq!(bundle.evidence.retrieval_tier, "bm25");
}

#[test]
fn semantic_fs_discover_ranks_paths_by_intent_tokens() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("semantic_fs_note.rs"), b"fn main() {}").expect("write");
    let policy = vox_search::SearchPolicy::default();
    let hits = super::semantic_fs_discover(dir.path(), "semantic fs note", 8, &policy);
    assert!(
        hits.iter().any(|h| {
            h["path"]
                .as_str()
                .unwrap_or("")
                .contains("semantic_fs_note")
        }),
        "{hits:?}"
    );
}
