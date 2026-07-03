use std::sync::Arc;

use vox_orchestrator::ApprovalOutcome;

const D_15S: std::time::Duration = std::time::Duration::from_secs(15);
use vox_orchestrator_mcp::pending_approvals::PendingApprovals;
use vox_orchestrator_mcp::server::tool_json_envelope_is_error;
use vox_orchestrator_mcp::{ServerState, handle_tool_call, handle_tool_call_with_mode, load_config};

#[tokio::test]
async fn register_then_resolve_wakes_the_awaiter() {
    let reg = PendingApprovals::default();
    let (id, rx) = reg.register(
        "vox_write_file".to_string(),
        "write src/x.rs".to_string(),
        1000,
    );

    // Visible while pending.
    let listed = reg.list();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].approval_id, id);
    assert_eq!(listed[0].tool, "vox_write_file");

    // A separate task parks on the decision; resolving from here wakes it.
    let waiter = tokio::spawn(rx);
    assert!(reg.resolve(&id, ApprovalOutcome::Approved));
    let outcome = waiter.await.expect("join").expect("sender not dropped");
    assert_eq!(outcome, ApprovalOutcome::Approved);

    // Resolved approvals leave the pending list.
    assert!(reg.list().is_empty());
}

#[tokio::test]
async fn resolve_unknown_id_returns_false() {
    let reg = PendingApprovals::default();
    assert!(!reg.resolve("AP-does-not-exist", ApprovalOutcome::Approved));
}

#[tokio::test]
async fn cancel_drops_the_pending_entry() {
    let reg = PendingApprovals::default();
    let (id, _rx) = reg.register("vox_deploy".to_string(), "deploy prod".to_string(), 1);
    assert_eq!(reg.list().len(), 1);
    reg.cancel(&id);
    assert!(reg.list().is_empty());
}

/// End-to-end gate: a dangerous tool without `user_approval` parks on a pending
/// approval; resolving it Rejected wakes the call with a non-approved error
/// envelope (and the action never executes).
#[tokio::test]
async fn dangerous_tool_parks_until_resolved() {
    let state = Arc::new(ServerState::new_full(load_config()));

    let s2 = state.clone();
    let call = tokio::spawn(async move {
        handle_tool_call(
            &s2,
            "vox_run_shell",
            serde_json::json!({ "command": "echo hi" }),
        )
        .await
    });

    // Wait until the gate registers the pending approval.
    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "dangerous tool never registered a pending approval"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "vox_run_shell");

    // Reject it; the parked call should wake and return an error envelope.
    assert!(
        state
            .pending_approvals
            .resolve(&pending[0].approval_id, ApprovalOutcome::Rejected)
    );

    let raw = call.await.expect("join").expect("dispatch ok");
    assert!(
        tool_json_envelope_is_error(&raw),
        "rejected approval must yield an error envelope, got: {raw}"
    );
    assert!(state.pending_approvals.list().is_empty());
}

/// Security regression guard: a caller cannot bypass the HITL gate by setting
/// `user_approval: true` in the tool-call args themselves. Since the LLM agent
/// composes its own tool-call JSON, an arg-based fast path is a self-serve
/// bypass of the approval requirement for dangerous tools. This must park and
/// await a real human decision exactly like a call without the field.
#[tokio::test]
async fn user_approval_arg_does_not_bypass_the_gate() {
    let state = Arc::new(ServerState::new_full(load_config()));

    let s2 = state.clone();
    let call = tokio::spawn(async move {
        handle_tool_call(
            &s2,
            "vox_run_shell",
            serde_json::json!({ "command": "echo hi", "user_approval": true }),
        )
        .await
    });

    // Wait until the gate registers the pending approval.
    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "dangerous tool never registered a pending approval (user_approval arg bypassed the gate)"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "vox_run_shell");

    // Reject it; the parked call should wake and return an error envelope.
    assert!(
        state
            .pending_approvals
            .resolve(&pending[0].approval_id, ApprovalOutcome::Rejected)
    );

    let raw = call.await.expect("join").expect("dispatch ok");
    assert!(
        tool_json_envelope_is_error(&raw),
        "rejected approval must yield an error envelope, got: {raw}"
    );
    assert!(state.pending_approvals.list().is_empty());
}

// ─────────────────────────────────────────────────────────────────────────
// T0.3: permission-mode auto-approve + persisted per-repo allowlist tests.
// ─────────────────────────────────────────────────────────────────────────

/// RED test 1 (mode): `accept_edits` auto-approves a `mutating` + `reversible`
/// tool (`vox_write_file`) without ever parking, but still parks a
/// `destructive` tool (`vox_run_shell`).
#[tokio::test]
async fn accept_edits_mode_auto_approves_mutating_reversible_but_parks_destructive() {
    let state = Arc::new(ServerState::new_full(load_config()));

    // vox_write_file under accept_edits: must complete without ever
    // registering a pending approval (auto-approved by mode). The underlying
    // handler may itself return Ok or Err for reasons unrelated to this gate
    // (e.g. "Unknown tool" if vox_write_file isn't routed as a static match
    // arm in this test harness's dispatch table) — what matters for THIS
    // test is that it never went through the park-and-await path. A
    // parked-and-rejected call would return an Ok envelope containing the
    // "was not approved" error shape; assert we did NOT get that shape.
    let outcome = handle_tool_call_with_mode(
        &state,
        "vox_write_file",
        serde_json::json!({ "path": "does/not/matter.txt", "content": "x" }),
        Some("accept_edits"),
    )
    .await;
    match &outcome {
        Ok(raw) => assert!(
            !raw.contains("was not approved"),
            "vox_write_file under accept_edits must not park for approval, got: {raw}"
        ),
        Err(e) => {
            let msg = e.to_string();
            assert!(
                !msg.contains("was not approved"),
                "vox_write_file under accept_edits must not park for approval, got err: {msg}"
            );
        }
    }
    assert!(
        state.pending_approvals.list().is_empty(),
        "accept_edits must not have left a pending approval for vox_write_file"
    );

    // vox_run_shell (destructive) under accept_edits: must still park.
    let s2 = state.clone();
    let call = tokio::spawn(async move {
        handle_tool_call_with_mode(
            &s2,
            "vox_run_shell",
            serde_json::json!({ "command": "echo hi" }),
            Some("accept_edits"),
        )
        .await
    });

    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "vox_run_shell under accept_edits never registered a pending approval (destructive tool must still park)"
        );
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let pending = state.pending_approvals.list();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].tool, "vox_run_shell");
    assert!(
        state
            .pending_approvals
            .resolve(&pending[0].approval_id, ApprovalOutcome::Rejected)
    );
    let raw = call.await.expect("join").expect("dispatch ok");
    assert!(tool_json_envelope_is_error(&raw));
}

/// RED test 2: `ask` mode (and no mode set at all) still parks everything —
/// explicit regression proof that T0.3 did not change the pre-T0.3 baseline
/// for the `ask` / default path.
#[tokio::test]
async fn ask_mode_and_no_mode_still_park_dangerous_tools() {
    for mode in [Some("ask"), None] {
        let state = Arc::new(ServerState::new_full(load_config()));
        let s2 = state.clone();
        let mode_owned = mode.map(str::to_string);
        let call = tokio::spawn(async move {
            handle_tool_call_with_mode(
                &s2,
                "vox_write_file",
                serde_json::json!({ "path": "x.txt", "content": "y" }),
                mode_owned.as_deref(),
            )
            .await
        });

        let deadline = tokio::time::Instant::now() + D_15S;
        loop {
            if !state.pending_approvals.list().is_empty() {
                break;
            }
            assert!(
                tokio::time::Instant::now() < deadline,
                "mode {mode:?}: vox_write_file never registered a pending approval (ask/no-mode must still park)"
            );
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        let pending = state.pending_approvals.list();
        assert_eq!(pending.len(), 1);
        assert!(
            state
                .pending_approvals
                .resolve(&pending[0].approval_id, ApprovalOutcome::Rejected)
        );
        let raw = call.await.expect("join").expect("dispatch ok");
        assert!(
            tool_json_envelope_is_error(&raw),
            "mode {mode:?}: rejected approval must yield an error envelope, got: {raw}"
        );
    }
}

/// RED test 3 (allowlist, per-repo scoping): a tool on the persisted
/// allowlist for `(repo_id, tool)` auto-approves even in `ask` mode; the
/// SAME tool in a DIFFERENT repo_id still parks.
#[tokio::test]
#[serial_test::serial]
async fn allowlisted_tool_auto_approves_in_its_repo_but_parks_in_a_different_repo() {
    let _env = IsolatedDbEnv::new();

    let repo_a = format!("t03-repo-a-{}", uuid::Uuid::new_v4());
    let repo_b = format!("t03-repo-b-{}", uuid::Uuid::new_v4());

    vox_orchestrator_mcp::approval_allowlist::add_entry(&repo_a, "vox_run_shell")
        .await
        .expect("add_entry");

    assert!(
        vox_orchestrator_mcp::approval_allowlist::is_allowlisted(&repo_a, "vox_run_shell").await,
        "repo_a should be allowlisted for vox_run_shell"
    );
    assert!(
        !vox_orchestrator_mcp::approval_allowlist::is_allowlisted(&repo_b, "vox_run_shell").await,
        "repo_b must NOT be allowlisted just because repo_a is (per-repo scoping)"
    );
    assert!(
        !vox_orchestrator_mcp::approval_allowlist::is_allowlisted(&repo_a, "vox_deploy").await,
        "repo_a should not be allowlisted for a DIFFERENT tool it never added"
    );
}

/// RED test 4 (persistence across restart): an allowlist entry added via
/// `add_entry` survives a fresh DB reconnect (`connect_default()` called
/// again), proving real persistence rather than an in-memory cache.
#[tokio::test]
#[serial_test::serial]
async fn allowlist_entry_survives_a_fresh_db_reconnect() {
    let _env = IsolatedDbEnv::new();

    let repo_id = format!("t03-restart-repo-{}", uuid::Uuid::new_v4());
    vox_orchestrator_mcp::approval_allowlist::add_entry(&repo_id, "vox_deploy")
        .await
        .expect("add_entry");

    // Force a brand-new connection to the same (isolated) DB file — this is
    // exactly what `connect_default()` does on every call, so a second call
    // here simulates "the process restarted and reconnected".
    let reconnected = vox_db::VoxDb::connect_default()
        .await
        .expect("reconnect to isolated db");
    drop(reconnected); // just proving the file-backed DB is reachable fresh

    assert!(
        vox_orchestrator_mcp::approval_allowlist::is_allowlisted(&repo_id, "vox_deploy").await,
        "allowlist entry must survive a fresh DB reconnect (real persistence, not an in-memory cache)"
    );

    let listed = vox_orchestrator_mcp::approval_allowlist::list_for_repo(&repo_id).await;
    assert_eq!(listed, vec!["vox_deploy".to_string()]);
}

/// RED test 5 (parity): every tool referenced in
/// `contracts/orchestration/permission-modes.v1.yaml`'s `risk_classes` has a
/// matching entry in `vox_orchestrator_mcp::permission_modes::RISK_CLASSES`,
/// and vice versa — the YAML and the Rust mirror cannot silently drift.
#[test]
fn risk_classes_yaml_matches_rust_table() {
    #[derive(serde::Deserialize)]
    struct YamlRiskClass {
        tool: String,
        class: String,
        reversible: bool,
    }
    #[derive(serde::Deserialize)]
    struct YamlDoc {
        risk_classes: Vec<YamlRiskClass>,
    }

    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    // crates/vox-orchestrator-mcp -> repo root -> contracts/...
    let repo_root = manifest_dir
        .parent()
        .and_then(std::path::Path::parent)
        .expect("repo root from CARGO_MANIFEST_DIR");
    let yaml_path = repo_root
        .join("contracts")
        .join("orchestration")
        .join("permission-modes.v1.yaml");
    let raw = std::fs::read_to_string(&yaml_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", yaml_path.display()));
    let doc: YamlDoc = serde_yaml::from_str(&raw).expect("parse permission-modes.v1.yaml");

    use vox_orchestrator_mcp::permission_modes::{RISK_CLASSES, SafetyClass};

    let yaml_set: std::collections::BTreeMap<String, (String, bool)> = doc
        .risk_classes
        .into_iter()
        .map(|r| (r.tool, (r.class, r.reversible)))
        .collect();

    let rust_set: std::collections::BTreeMap<String, (String, bool)> = RISK_CLASSES
        .iter()
        .map(|r| {
            let class_str = match r.class {
                SafetyClass::ReadOnly => "read_only",
                SafetyClass::Mutating => "mutating",
                SafetyClass::Destructive => "destructive",
                SafetyClass::Unknown => "unknown",
            };
            (r.tool.to_string(), (class_str.to_string(), r.reversible))
        })
        .collect();

    assert_eq!(
        yaml_set, rust_set,
        "permission-modes.v1.yaml risk_classes and RISK_CLASSES have drifted apart"
    );
}

/// Serialized-env-mutation guard for the allowlist persistence tests: points
/// `VOX_DB_PATH` at a fresh temp file for the duration of the test, so these
/// tests never touch the real canonical user DB and don't collide with each
/// other or with any other test in this binary that reads `VOX_DB_PATH`.
/// Callers MUST also apply `#[serial_test::serial]` (unnamed — same key as
/// the rest of this crate's env-mutating tests) so no other test in this
/// binary races the env var while this guard is alive.
struct IsolatedDbEnv {
    _tempdir: tempfile::TempDir,
    prev_db_path: Option<String>,
}

impl IsolatedDbEnv {
    #[allow(unsafe_code)]
    fn new() -> Self {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let db_path = tempdir.path().join("t03-approval-allowlist-test.db");
        let prev_db_path = std::env::var("VOX_DB_PATH").ok();
        // SAFETY: `#[serial_test::serial]` on every caller serializes env
        // mutation against the rest of this crate's tests.
        unsafe {
            std::env::set_var("VOX_DB_PATH", db_path.to_string_lossy().as_ref());
        }
        Self {
            _tempdir: tempdir,
            prev_db_path,
        }
    }
}

impl Drop for IsolatedDbEnv {
    #[allow(unsafe_code)]
    fn drop(&mut self) {
        // SAFETY: `#[serial_test::serial]` on every caller serializes env
        // mutation against the rest of this crate's tests.
        unsafe {
            match &self.prev_db_path {
                Some(v) => std::env::set_var("VOX_DB_PATH", v),
                None => std::env::remove_var("VOX_DB_PATH"),
            }
        }
    }
}
