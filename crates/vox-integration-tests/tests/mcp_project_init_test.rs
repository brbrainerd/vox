//! `vox_project_init` writes under the MCP-discovered repo root (requires CWD = temp dir).

use serde_json::json;
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::TempDir;
use vox_orchestrator_mcp::handle_tool_call as tools;

static PROJECT_INIT_CWD_LOCK: Mutex<()> = Mutex::new(());

struct RestoreCwd {
    prev: PathBuf,
}

impl Drop for RestoreCwd {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

#[allow(clippy::await_holding_lock)] // Lock intentionally held across awaits to serialize CWD-mutating tests.
#[tokio::test]
async fn vox_project_init_writes_nested_application() {
    let _lock = PROJECT_INIT_CWD_LOCK.lock().expect("cwd lock");
    let prev = std::env::current_dir().expect("cwd");
    let _restore = RestoreCwd { prev: prev.clone() };
    let t = TempDir::new().expect("temp workspace");
    std::env::set_current_dir(t.path()).expect("chdir temp");

    let state = vox_orchestrator_mcp::ServerState::new_full(
        vox_orchestrator::OrchestratorConfig::default(),
    );
    let raw = tools(
        &state,
        "vox_project_init",
        json!({
            "project_name": "nested_app",
            "package_kind": "application",
            "target_subdir": "packages/nested_app"
        }),
    )
    .await
    .expect("tool ok");

    let v: serde_json::Value = serde_json::from_str(&raw).expect("json");
    assert_eq!(v["success"], true, "{raw}");
    let root = t.path().join("packages").join("nested_app");
    assert!(
        root.join("Vox.toml").is_file(),
        "expected Vox.toml under {root:?}"
    );
    assert!(root.join("src/main.vox").is_file());
}
