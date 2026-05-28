use tempfile::TempDir;

use crate::types::AgentId;

use super::account_registry::AccountMemoryRegistry;
use super::config::MemoryConfig;

use super::long_term::LongTermMemory;
use super::manager::MemoryManager;
use super::time::unix_secs_to_ymd;

fn memory_workdir() -> TempDir {
    TempDir::new().expect("tempdir")
}

// Helper: construct a MemoryConfig scoped to `account_id = "test"` under the tempdir.
fn test_config(dir: &TempDir) -> MemoryConfig {
    MemoryConfig::for_account("test", dir.path())
}

#[test]
fn long_term_memory_set_and_get() {
    let dir = memory_workdir();
    let mem = LongTermMemory::open(&dir.path().join("MEMORY.md")).expect("open");
    mem.set("current_crate", "vox-compiler").expect("set");
    let val = mem.get("current_crate").expect("get");
    assert_eq!(val.as_deref(), Some("vox-compiler"));
}

#[test]
fn long_term_memory_upsert() {
    let dir = memory_workdir();
    let mem = LongTermMemory::open(&dir.path().join("MEMORY.md")).expect("open");
    mem.set("status", "in progress").expect("set first");
    mem.set("status", "completed").expect("set second (upsert)");
    let val = mem.get("status").expect("get");
    assert_eq!(val.as_deref(), Some("completed"));
}

#[test]
fn long_term_memory_list_keys() {
    let dir = memory_workdir();
    let mem = LongTermMemory::open(&dir.path().join("MEMORY.md")).expect("open");
    mem.set("alpha", "a").expect("set");
    mem.set("beta", "b").expect("set");
    let keys = mem.list_keys().expect("list");
    assert!(keys.contains(&"alpha".to_string()));
    assert!(keys.contains(&"beta".to_string()));
}

#[tokio::test]
async fn memory_manager_persist_and_lookup() {
    let dir = memory_workdir();
    let mut mgr = MemoryManager::new(test_config(&dir)).expect("create");
    mgr.persist_fact(AgentId(1), "last_task", "fix parser", &[], None, None)
        .expect("persist");
    let val = mgr
        .lookup_fact_by_key("last_task")
        .await
        .expect("lookup_fact_by_key");
    assert_eq!(val.as_deref(), Some("fix parser"));
}

#[test]
fn memory_manager_bootstrap_context() {
    let dir = memory_workdir();
    let mut mgr = MemoryManager::new(test_config(&dir)).expect("create");
    mgr.persist_fact(AgentId(1), "project", "vox", &[], None, None)
        .expect("persist");
    mgr.log("started session").expect("log");
    let ctx = mgr.bootstrap_context();
    assert!(ctx.contains("project"));
    assert!(ctx.contains("vox"));
}

#[test]
fn memory_manager_search() {
    let dir = memory_workdir();
    let mut mgr = MemoryManager::new(test_config(&dir)).expect("create");
    mgr.log("fixed the parser bug").expect("log");
    mgr.persist_fact(
        AgentId(1),
        "active_branch",
        "feat/parser-fix",
        &[],
        None,
        None,
    )
    .expect("persist");
    let hits = mgr.search("parser").expect("search");
    assert!(!hits.is_empty(), "should find 'parser' in memory");
}

#[tokio::test]
async fn flush_before_compaction_persists_facts() {
    use std::collections::HashMap;
    let dir = memory_workdir();
    let mut mgr = MemoryManager::new(test_config(&dir)).expect("create");
    let mut facts = HashMap::new();
    facts.insert(
        "lock_file".to_string(),
        "crates/vox-compiler/src/parser/mod.rs".to_string(),
    );
    facts.insert("agent_state".to_string(), "building".to_string());
    let flushed = mgr
        .flush_before_compaction(AgentId(1), facts)
        .expect("flush");
    assert_eq!(flushed, 2);
    assert!(
        mgr.lookup_fact_by_key("lock_file")
            .await
            .expect("lookup")
            .is_some()
    );
    assert!(
        mgr.lookup_fact_by_key("agent_state")
            .await
            .expect("lookup")
            .is_some()
    );
}

#[test]
fn disabled_memory_manager_returns_empty_context() {
    let dir = memory_workdir();
    let config = MemoryConfig {
        account_id: "test".to_string(),
        log_dir: dir.path().join("test").join("logs"),
        memory_md_path: dir.path().join("test").join("MEMORY.md"),
        log_retention_days: 7,
        enabled: false,
    };
    let mgr = MemoryManager::new(config).expect("create");
    let ctx = mgr.bootstrap_context();
    assert!(
        ctx.is_empty(),
        "disabled memory should return empty context"
    );
}

#[test]
fn unix_secs_to_ymd_basic() {
    // 2026-02-27 00:00:00 UTC = 1772150400 secs
    let (y, m, d) = unix_secs_to_ymd(1_772_150_400);
    assert_eq!(y, 2026);
    assert_eq!(m, 2);
    assert_eq!(d, 27);
}

#[test]
fn memory_manager_account_id_accessor() {
    let dir = memory_workdir();
    let mgr = MemoryManager::for_account("alice", dir.path()).expect("create");
    assert_eq!(mgr.account_id(), "alice");
}

#[test]
fn account_registry_isolation() {
    let dir = memory_workdir();
    let registry = AccountMemoryRegistry::new(dir.path());

    let alice = registry.get_or_create("alice").expect("alice");
    let bob = registry.get_or_create("bob").expect("bob");

    assert_ne!(alice.account_id(), bob.account_id());
    assert_eq!(alice.account_id(), "alice");
    assert_eq!(bob.account_id(), "bob");

    // Confirm paths do not overlap.
    let alice_path = format!("{:?}", dir.path().join("alice"));
    let bob_path = format!("{:?}", dir.path().join("bob"));
    assert_ne!(alice_path, bob_path);
}

#[test]
fn account_registry_returns_same_instance() {
    let dir = memory_workdir();
    let registry = AccountMemoryRegistry::new(dir.path());

    let first = registry.get_or_create("carol").expect("first");
    let second = registry.get_or_create("carol").expect("second");

    // Same Arc pointer — same instance.
    assert!(std::sync::Arc::ptr_eq(&first, &second));
}
