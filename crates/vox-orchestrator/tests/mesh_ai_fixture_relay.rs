//! `relay_ai_fixture_distributed_subagent` — skipped-path coverage (no live Populi server).
// vox-arch-check: allow crate unsafe_code`n#![allow(unsafe_code)] // Rust 2024 `set_var` / `remove_var`; serialized via `#[serial]`.

use serial_test::serial;
use vox_orchestrator::a2a::relay_ai_fixture_distributed_subagent;

#[tokio::test]
#[serial]
async fn skips_mesh_when_control_plane_url_unset() {
    let keys = ["VOX_ORCHESTRATOR_MESH_CONTROL_URL", "VOX_MESH_CONTROL_ADDR"];
    let saved: Vec<(&str, Option<String>)> = keys
        .iter()
        .copied()
        .map(|k| (k, std::env::var(k).ok()))
        .collect();
    // SAFETY: `#[serial]` — no concurrent tests read these vars during mutation.
    unsafe {
        for k in keys {
            std::env::remove_var(k);
        }
    }

    let out = relay_ai_fixture_distributed_subagent("inline", 9).await;

    unsafe {
        for (k, v) in saved {
            match v {
                Some(val) => std::env::set_var(k, val),
                None => std::env::remove_var(k),
            }
        }
    }

    assert_eq!(out, "inline|mesh=skipped_no_control_url");
}
