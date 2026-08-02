#![allow(missing_docs)]
//! Integration tests for vox-package skill manifest and execution telemetry storage.

#[cfg(feature = "quarantine")]
use vox_db::SkillExecutionParams;
use vox_db::VoxDb;

async fn db() -> VoxDb {
    VoxDb::open_memory().await.expect("open_memory")
}

// ── skill_manifests ──────────────────────────────────────────────────────────

#[tokio::test]
async fn get_skill_manifest_returns_none_when_absent() {
    let cs: VoxDb = db().await;
    let r = cs.get_skill_manifest("vox.missing").await.expect("query");
    assert!(r.is_none());
}

#[tokio::test]
async fn publish_and_get_skill_manifest() {
    let cs: VoxDb = db().await;
    cs.publish_skill("vox.example", "1.0.0", r#"{"id":"vox.example"}"#, "# Skill")
        .await
        .expect("publish_skill");
    let entry = cs
        .get_skill_manifest("vox.example")
        .await
        .expect("get")
        .expect("should be present");
    assert_eq!(entry.id, "vox.example");
    assert_eq!(entry.version, "1.0.0");
    assert_eq!(entry.skill_md, "# Skill");
}

#[tokio::test]
async fn publish_skill_is_idempotent_on_upsert() {
    let cs: VoxDb = db().await;
    cs.publish_skill("vox.idem", "1.0.0", r#"{}"#, "# v1")
        .await
        .expect("first");
    cs.publish_skill("vox.idem", "1.0.0", r#"{}"#, "# v1 again")
        .await
        .expect("second");
    let e = cs
        .get_skill_manifest("vox.idem")
        .await
        .expect("get")
        .expect("present");
    assert_eq!(e.id, "vox.idem"); // no error on duplicate
}

#[tokio::test]
async fn list_skill_manifests_returns_all_rows() {
    let cs: VoxDb = db().await;
    cs.publish_skill("a", "1.0", r#"{}"#, "# A")
        .await
        .expect("A");
    cs.publish_skill("b", "2.0", r#"{}"#, "# B")
        .await
        .expect("B");
    let all = cs.list_skill_manifests().await.expect("list");
    assert!(all.len() >= 2);
    assert!(all.iter().any(|e| e.id == "a"));
    assert!(all.iter().any(|e| e.id == "b"));
}

#[tokio::test]
async fn unpublish_skill_removes_row() {
    let cs: VoxDb = db().await;
    cs.publish_skill("del.me", "0.1.0", r#"{}"#, "# Del")
        .await
        .expect("publish");
    cs.unpublish_skill("del.me").await.expect("unpublish");
    let r = cs.get_skill_manifest("del.me").await.expect("query");
    assert!(r.is_none());
}

// ── skill_executions ─────────────────────────────────────────────────────────

// Only used by the quarantine-gated skill_executions tests below; unused
// when the feature is off.
#[cfg(feature = "quarantine")]
fn exec_params<'a>(skill_id: &'a str) -> SkillExecutionParams<'a> {
    SkillExecutionParams {
        skill_id,
        version: "1.0.0",
        session_id: None,
        workflow_id: None,
        agent_id: None,
        status: "ok",
        duration_ms: 42,
        input_hash: None,
        output_size: 0,
        error_kind: None,
        reflection_score: None,
    }
}

// skill_executions is quarantined (DORMANT, no confirmed caller outside vox-db
// even via wrapper-call detection — Task 4, VoxDB audit condensation plan) —
// off by default, see crates/vox-db/src/schema/domains/quarantine.rs. These 4
// tests were never caught by the test-census scan because their bodies call
// record_skill_execution/list_skill_executions_by_skill, not the literal
// table name — the same class of blind spot the wrapper-call detection pass
// exists for, just on the test side instead of the production-caller side.
#[tokio::test]
#[cfg(feature = "quarantine")]
async fn record_skill_execution_returns_rowid() {
    let cs: VoxDb = db().await;
    let id = cs
        .record_skill_execution(exec_params("vox.test"))
        .await
        .expect("record");
    assert!(id > 0);
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn record_skill_execution_error_status() {
    let cs: VoxDb = db().await;
    let mut p = exec_params("vox.failing");
    p.status = "error";
    p.error_kind = Some("timeout");
    let id = cs.record_skill_execution(p).await.expect("record");
    assert!(id > 0);
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn list_skill_executions_returns_newest_first() {
    let cs: VoxDb = db().await;
    cs.record_skill_execution(exec_params("vox.ordered"))
        .await
        .expect("1");
    cs.record_skill_execution(exec_params("vox.ordered"))
        .await
        .expect("2");
    let rows = cs
        .list_skill_executions_by_skill("vox.ordered", 10)
        .await
        .expect("list");
    assert_eq!(rows.len(), 2);
    // Newest row has the larger id
    assert!(rows[0].id > rows[1].id, "should be newest-first");
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn list_skill_executions_limit_is_honoured() {
    let cs: VoxDb = db().await;
    for _ in 0..5 {
        cs.record_skill_execution(exec_params("vox.limited"))
            .await
            .expect("rec");
    }
    let rows = cs
        .list_skill_executions_by_skill("vox.limited", 3)
        .await
        .expect("list");
    assert_eq!(rows.len(), 3);
}

#[tokio::test]
#[cfg(feature = "quarantine")]
async fn list_skill_executions_empty_when_no_rows() {
    let cs: VoxDb = db().await;
    let rows = cs
        .list_skill_executions_by_skill("vox.never_run", 50)
        .await
        .expect("list");
    assert!(rows.is_empty());
}
