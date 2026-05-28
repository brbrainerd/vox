use crate::params::ToolResult;
use crate::server_state::ServerState;
use serde::Deserialize;
use serde_json::json;

#[derive(Deserialize)]
struct AuditParams {
    url: String,
}

#[derive(Deserialize)]
struct BaselineParams {
    url: String,
}

/// Run one blocking browser session and return (page_id, ax_tree_json_str, screenshot_bytes).
fn browser_capture(url: String) -> anyhow::Result<(String, String, Vec<u8>)> {
    let plugin = vox_plugin_host::cached_code_plugin("browser")
        .map_err(|e| anyhow::anyhow!("browser plugin: {e}"))?;
    let backend = plugin
        .plugin
        .as_browser_automation()
        .into_option()
        .ok_or_else(|| anyhow::anyhow!("BrowserAutomation accessor returned None"))?;

    let page_id = backend
        .open(url.as_str().into(), true)
        .into_result()
        .map(|s| s.into_string())
        .map_err(|e| anyhow::anyhow!("browser open: {e}"))?;

    let ax_tree_str = backend
        .ax_tree(page_id.as_str().into())
        .into_result()
        .map(|s| s.into_string())
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"));

    let screenshot_bytes: Vec<u8> = backend
        .screenshot_bytes(page_id.as_str().into())
        .into_result()
        .map(|v| v.into_iter().collect())
        .unwrap_or_default();

    let _ = backend.close(page_id.as_str().into());

    Ok((page_id, ax_tree_str, screenshot_bytes))
}

pub async fn vox_visus_audit(_state: &ServerState, args: serde_json::Value) -> String {
    let params: AuditParams = match serde_json::from_value(args.clone()) {
        Ok(p) => p,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("Invalid arguments: {e}"))
                .to_json_compact();
        }
    };

    let url = params.url.clone();
    let capture = tokio::task::spawn_blocking(move || browser_capture(url)).await;

    let (_page_id, ax_tree_str, screenshot_bytes) = match capture {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return ToolResult::<serde_json::Value>::err(format!("browser error: {e}"))
                .to_json_compact();
        }
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}"))
                .to_json_compact();
        }
    };

    // Wait for JS hydration
    tokio::time::sleep(vox_config::timeouts::D_3S).await;

    let ax_tree: serde_json::Value =
        serde_json::from_str(&ax_tree_str).unwrap_or(json!({ "raw": ax_tree_str }));

    // check_overlaps is not on BrowserAutomation trait — overlap detection is
    // deferred to the VLM layer when --vlm / Layer 2 analysis is invoked.
    let overlaps = json!([]);

    let screenshot_b64 = {
        use base64::Engine;
        base64::engine::general_purpose::STANDARD.encode(&screenshot_bytes)
    };

    let out = json!({
        "status": "success",
        "url": params.url,
        "ax_tree": ax_tree,
        "overlaps": overlaps,
        "screenshot_base64_length": screenshot_b64.len(),
        "screenshot_base64": screenshot_b64,
    });

    ToolResult::<serde_json::Value>::ok(out).to_json_compact()
}

pub async fn vox_visus_baseline(state: &ServerState, args: serde_json::Value) -> String {
    let params: BaselineParams = match serde_json::from_value(args.clone()) {
        Ok(p) => p,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("Invalid arguments: {e}"))
                .to_json_compact();
        }
    };

    let db = match state.db.as_ref() {
        Some(db) => db,
        None => {
            return ToolResult::<serde_json::Value>::err("No VoxDb connected".to_string())
                .to_json_compact();
        }
    };

    tracing::info!("Visus baseline triggered for URL: {}", params.url);

    let url = params.url.clone();
    let capture = tokio::task::spawn_blocking(move || browser_capture(url)).await;

    let (_page_id, ax_tree_str, screenshot_bytes) = match capture {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            return ToolResult::<serde_json::Value>::err(format!("browser error: {e}"))
                .to_json_compact();
        }
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}"))
                .to_json_compact();
        }
    };

    // Wait for JS hydration
    tokio::time::sleep(vox_config::timeouts::D_3S).await;

    let screenshot_cas = blake3::hash(&screenshot_bytes).to_string();
    let ax_tree_cas = blake3::hash(ax_tree_str.as_bytes()).to_string();

    let id = format!("visus_base_{}", uuid::Uuid::new_v4());

    let row = vox_db::store::types::VisusBaselineRow {
        id: id.clone(),
        target_url: params.url.clone(),
        viewport: "desktop".to_string(),
        theme: "auto".to_string(),
        screenshot_cas,
        ax_tree_cas,
        metadata_json: Some("{}".to_string()),
        created_at: chrono::Utc::now().to_rfc3339(),
    };

    if let Err(e) = db.upsert_visus_baseline(row).await {
        return ToolResult::<serde_json::Value>::err(format!("DB error: {e}")).to_json_compact();
    }

    let out = json!({
        "status": "baseline_recorded",
        "url": params.url,
        "baseline_id": id,
        "message": "Golden baseline recorded in DB.",
    });
    ToolResult::<serde_json::Value>::ok(out).to_json_compact()
}
