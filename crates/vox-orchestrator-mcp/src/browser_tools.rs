//! Chromium-backed browser MCP tools (`vox_browser_*`).
//!
//! Dispatches through vox-plugin-host / BrowserAutomation sabi trait.
//! All blocking CDP work runs inside `tokio::task::spawn_blocking`.

use crate::llm_bridge::call_llm;
use crate::params::{
    BrowserActParams, BrowserClickPointParams, BrowserControlLockParams, BrowserExtractJsonParams,
    BrowserExtractParams, BrowserFillParams, BrowserGotoParams, BrowserHtmlParams,
    BrowserKeyParams, BrowserOpenParams, BrowserPageParams, BrowserScreenshotParams,
    BrowserScrollParams, BrowserTargetParams, BrowserTypeParams, BrowserViewportParams,
    BrowserWaitParams, ToolResult,
};
use crate::server_state::ServerState;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;
use tokio::sync::Mutex;

fn control_locks() -> &'static Mutex<HashMap<String, String>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, String>>> = OnceLock::new();
    LOCKS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn normalize_actor(actor: Option<&str>) -> String {
    actor
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("agent")
        .to_ascii_lowercase()
}

async fn ensure_control_lock(page_id: &str, actor: Option<&str>) -> Result<(), String> {
    let actor = normalize_actor(actor);
    let locks = control_locks().lock().await;
    if let Some(owner) = locks.get(page_id)
        && owner != &actor
    {
        return Err(format!(
            "control lock active for {owner}; actor {actor} is blocked on page {page_id}"
        ));
    }
    Ok(())
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 {
        return None;
    }
    // PNG signature then IHDR width/height as big-endian u32.
    let sig = &bytes[0..8];
    if sig != [137, 80, 78, 71, 13, 10, 26, 10] {
        return None;
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Some((width, height))
}

fn parse_backend_json(text: String) -> anyhow::Result<serde_json::Value> {
    serde_json::from_str::<serde_json::Value>(&text)
        .map_err(|e| anyhow::anyhow!("invalid backend JSON: {e}; raw={text}"))
}

fn summary_max_chars() -> usize {
    vox_secrets::resolve_secret(vox_secrets::SecretId::VoxBrowserLlmContextChars)
        .expose()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24_000)
}

/// Obtain the cached browser plugin and call a closure with its BrowserAutomation backend.
/// Must be called inside `spawn_blocking` (or a non-async context).
///
/// The closure receives the `LoadedCodePlugin`; callers should call
/// `plugin.plugin.as_browser_automation().into_option().unwrap()` to get the backend.
/// This avoids naming the `BrowserAutomation_TO` generic type in the function signature.
fn with_browser_plugin<F, T>(f: F) -> anyhow::Result<T>
where
    F: FnOnce(&'static vox_plugin_host::loader::LoadedCodePlugin) -> anyhow::Result<T>,
{
    let plugin = vox_plugin_host::cached_code_plugin("browser")
        .map_err(|e| anyhow::anyhow!("browser plugin load: {e}"))?;
    // Verify the accessor is present before handing off.
    if plugin
        .plugin
        .as_browser_automation()
        .into_option()
        .is_none()
    {
        return Err(anyhow::anyhow!(
            "browser plugin loaded but BrowserAutomation accessor returned None"
        ));
    }
    f(plugin)
}

/// Convenience: get the BrowserAutomation accessor, panicking if absent (guarded by
/// `with_browser_plugin` above).
macro_rules! backend {
    ($plugin:expr) => {
        $plugin
            .plugin
            .as_browser_automation()
            .into_option()
            .expect("BrowserAutomation accessor checked in with_browser_plugin")
    };
}

pub async fn browser_open(_state: &ServerState, p: BrowserOpenParams) -> String {
    let url = p.url.clone();
    let headless = p.headless;
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| { let b = backend!(p);
            b.open(url.as_str().into(), headless)
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser open: {e}"))
        })
    })
    .await
    {
        Ok(Ok(page_id)) => ToolResult::ok(serde_json::json!({
            "page_id": page_id,
            "url": p.url,
        }))
        .to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err_with_remediation(
            e.to_string(),
            "Install Chromium/Chrome or set VOX_CHROME_EXECUTABLE; for containers try VOX_BROWSER_NO_SANDBOX=1.",
        )
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_list_pages(_state: &ServerState, _p: serde_json::Value) -> String {
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            let raw = b
                .list_pages()
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser list_pages: {e}"))?;
            parse_backend_json(raw).map_err(|e| anyhow::anyhow!("browser list_pages: {e}"))
        })
    })
    .await
    {
        Ok(Ok(pages)) => ToolResult::ok(serde_json::json!({ "pages": pages })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_page_info(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            let raw = b
                .page_info(page_id.as_str().into())
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser page_info: {e}"))?;
            parse_backend_json(raw).map_err(|e| anyhow::anyhow!("browser page_info: {e}"))
        })
    })
    .await
    {
        Ok(Ok(info)) => ToolResult::ok(serde_json::json!({ "info": info })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_close(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    let page_id_for_call = page_id.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.close(page_id_for_call.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser close: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => {
            let mut locks = control_locks().lock().await;
            locks.remove(&page_id);
            ToolResult::ok(serde_json::json!({ "closed": true })).to_json()
        }
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_back(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    let actor = p.actor.clone();
    if let Err(e) = ensure_control_lock(&page_id, actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.back(page_id.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser back: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_forward(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    let actor = p.actor.clone();
    if let Err(e) = ensure_control_lock(&page_id, actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.forward(page_id.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser forward: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_reload(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    let actor = p.actor.clone();
    if let Err(e) = ensure_control_lock(&page_id, actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.reload(page_id.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser reload: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_stop(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    let actor = p.actor.clone();
    if let Err(e) = ensure_control_lock(&page_id, actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.stop(page_id.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser stop: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_goto(_state: &ServerState, p: BrowserGotoParams) -> String {
    let page_id = p.page_id.clone();
    let url = p.url.clone();
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.goto(page_id.as_str().into(), url.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser goto: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_click(_state: &ServerState, p: BrowserTargetParams) -> String {
    let page_id = p.page_id.clone();
    let target = p.target.clone();
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.click(page_id.as_str().into(), target.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser click: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_fill(_state: &ServerState, p: BrowserFillParams) -> String {
    let page_id = p.page_id.clone();
    let target = p.target.clone();
    let value = p.value.clone();
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.fill(
                page_id.as_str().into(),
                target.as_str().into(),
                value.as_str().into(),
            )
            .into_result()
            .map_err(|e| anyhow::anyhow!("browser fill: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_wait_for(_state: &ServerState, p: BrowserWaitParams) -> String {
    let page_id = p.page_id.clone();
    let target = p.target.clone();
    let timeout_secs = p.timeout_secs;
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.wait_for(
                page_id.as_str().into(),
                target.as_str().into(),
                timeout_secs,
            )
            .into_result()
            .map_err(|e| anyhow::anyhow!("browser wait_for: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_text(_state: &ServerState, p: BrowserTargetParams) -> String {
    let page_id = p.page_id.clone();
    let target = p.target.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.text(page_id.as_str().into(), target.as_str().into())
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser text: {e}"))
        })
    })
    .await
    {
        Ok(Ok(text)) => ToolResult::ok(serde_json::json!({ "text": text })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_html(_state: &ServerState, p: BrowserHtmlParams) -> String {
    let page_id = p.page_id.clone();
    let target = p.target.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.html(page_id.as_str().into(), target.as_str().into())
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser html: {e}"))
        })
    })
    .await
    {
        Ok(Ok(html)) => ToolResult::ok(serde_json::json!({ "html": html })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_screenshot(_state: &ServerState, p: BrowserScreenshotParams) -> String {
    let page_id = p.page_id.clone();
    let path = p.path.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.screenshot(page_id.as_str().into(), path.as_str().into())
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser screenshot: {e}"))
        })
    })
    .await
    {
        Ok(Ok(path)) => ToolResult::ok(serde_json::json!({ "path": path })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_screenshot_viewport(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.screenshot_viewport_bytes(page_id.as_str().into())
                .into_result()
                .map(|bytes| bytes.into_vec())
                .map_err(|e| anyhow::anyhow!("browser screenshot_viewport: {e}"))
        })
    })
    .await
    {
        Ok(Ok(bytes)) => {
            use base64::Engine;
            let image_base64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
            let (width, height) = png_dimensions(&bytes).unwrap_or((0, 0));
            ToolResult::ok(serde_json::json!({
                "image_base64": image_base64,
                "viewport_width": width,
                "viewport_height": height
            }))
            .to_json()
        }
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_screencast_frame(_state: &ServerState, p: BrowserPageParams) -> String {
    let page_id = p.page_id.clone();
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            let raw = b
                .screencast_frame(page_id.as_str().into())
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser screencast_frame: {e}"))?;
            parse_backend_json(raw).map_err(|e| anyhow::anyhow!("browser screencast_frame: {e}"))
        })
    })
    .await
    {
        Ok(Ok(value)) => ToolResult::ok(value).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_click_xy(_state: &ServerState, p: BrowserClickPointParams) -> String {
    let page_id = p.page_id.clone();
    let x = p.x;
    let y = p.y;
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.click_xy(page_id.as_str().into(), x, y)
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser click_xy: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true, "x": x, "y": y })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_scroll(_state: &ServerState, p: BrowserScrollParams) -> String {
    let page_id = p.page_id.clone();
    let dx = p.dx;
    let dy = p.dy;
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.scroll(page_id.as_str().into(), dx, dy)
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser scroll: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => {
            ToolResult::ok(serde_json::json!({ "ok": true, "dx": dx, "dy": dy })).to_json()
        }
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_type(_state: &ServerState, p: BrowserTypeParams) -> String {
    let page_id = p.page_id.clone();
    let text = p.text.clone();
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.type_text(page_id.as_str().into(), text.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser type: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_press(_state: &ServerState, p: BrowserKeyParams) -> String {
    let page_id = p.page_id.clone();
    let key = p.key.clone();
    let key_out = key.clone();
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.press(page_id.as_str().into(), key.as_str().into())
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser press: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => ToolResult::ok(serde_json::json!({ "ok": true, "key": key_out })).to_json(),
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_set_viewport(_state: &ServerState, p: BrowserViewportParams) -> String {
    let page_id = p.page_id.clone();
    let width = p.width;
    let height = p.height;
    if let Err(e) = ensure_control_lock(&page_id, p.actor.as_deref()).await {
        return ToolResult::<serde_json::Value>::err(e).to_json();
    }
    match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.set_viewport(page_id.as_str().into(), width, height)
                .into_result()
                .map_err(|e| anyhow::anyhow!("browser set_viewport: {e}"))
        })
    })
    .await
    {
        Ok(Ok(())) => {
            ToolResult::ok(serde_json::json!({ "ok": true, "width": width, "height": height }))
                .to_json()
        }
        Ok(Err(e)) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json(),
    }
}

pub async fn browser_set_control_lock(_state: &ServerState, p: BrowserControlLockParams) -> String {
    let owner = p.owner.trim().to_ascii_lowercase();
    if owner != "human" && owner != "agent" && owner != "none" {
        return ToolResult::<serde_json::Value>::err(
            "owner must be one of: human, agent, none".to_string(),
        )
        .to_json();
    }
    let mut locks = control_locks().lock().await;
    if owner == "none" {
        locks.remove(&p.page_id);
    } else {
        locks.insert(p.page_id.clone(), owner.clone());
    }
    ToolResult::ok(serde_json::json!({
        "page_id": p.page_id,
        "owner": if owner == "none" { serde_json::Value::Null } else { serde_json::Value::String(owner) }
    }))
    .to_json()
}

pub async fn browser_extract(state: &ServerState, p: BrowserExtractParams) -> String {
    let page_id = p.page_id.clone();
    let max_chars = summary_max_chars() as u64;
    let summary = match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.visible_text_summary(page_id.as_str().into(), max_chars)
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser visible_text_summary: {e}"))
        })
    })
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json();
        }
    };
    let sys = "You help automate web pages. Answer ONLY with the extracted content requested — no preamble.";
    let user = format!(
        "Instruction:\n{}\n\nVisible page text (truncated):\n{}",
        p.instruction, summary
    );
    match call_llm(state, sys, &user, None, p.temperature, p.top_p, None).await {
        Ok((text, model, _)) => ToolResult::ok(serde_json::json!({
            "extraction": text,
            "model": model,
            "execution_mode": "assisted",
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            e,
            "Configure MCP chat model (`vox_list_models` / `vox_set_active_model`) or provider API keys via Secrets.",
        )
        .to_json(),
    }
}

pub async fn browser_extract_json(state: &ServerState, p: BrowserExtractJsonParams) -> String {
    let page_id = p.page_id.clone();
    let max_chars = summary_max_chars() as u64;
    let summary = match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.visible_text_summary(page_id.as_str().into(), max_chars)
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser visible_text_summary: {e}"))
        })
    })
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json();
        }
    };
    let sys = "Reply with a single JSON object only (no markdown fences). The object MUST validate informally against the schema description given.";
    let user = format!(
        "Schema (JSON Schema):\n{}\n\nTask:\n{}\n\nVisible page text:\n{}",
        p.schema_json, p.instruction, summary
    );
    match call_llm(state, sys, &user, None, p.temperature, p.top_p, None).await {
        Ok((text, model, _)) => {
            let trimmed = text.trim();
            let val: Result<serde_json::Value, _> = serde_json::from_str(trimmed);
            match val {
                Ok(v) => ToolResult::ok(serde_json::json!({
                    "data": v,
                    "model": model,
                    "execution_mode": "assisted",
                }))
                .to_json(),
                Err(e) => ToolResult::<serde_json::Value>::err(format!(
                    "model returned non-JSON: {e}; raw={trimmed:?}"
                ))
                .to_json(),
            }
        }
        Err(e) => ToolResult::<serde_json::Value>::err_with_remediation(
            e,
            "Configure MCP chat model and provider keys.",
        )
        .to_json(),
    }
}

#[derive(Debug, Deserialize)]
struct ActJson {
    #[allow(dead_code)]
    action: String,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    url: Option<String>,
}

pub async fn browser_act(state: &ServerState, p: BrowserActParams) -> String {
    let page_id = p.page_id.clone();
    let max_chars = summary_max_chars() as u64;
    let summary = match tokio::task::spawn_blocking(move || {
        with_browser_plugin(|p| {
            let b = backend!(p);
            b.visible_text_summary(page_id.as_str().into(), max_chars)
                .into_result()
                .map(|s| s.into_string())
                .map_err(|e| anyhow::anyhow!("browser visible_text_summary: {e}"))
        })
    })
    .await
    {
        Ok(Ok(s)) => s,
        Ok(Err(e)) => return ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!("spawn_blocking: {e}")).to_json();
        }
    };
    let sys = r#"Reply with ONE JSON object only, no markdown. Shape:
{"action":"click"|"fill"|"goto"|"wait"|"noop","target":"css or xpath:... optional","value":"optional","url":"optional"}.
Use xpath: prefix in target for XPath. Choose the best next step for the instruction."#;
    let user = format!(
        "Goal:\n{}\n\nVisible page text:\n{}",
        p.instruction, summary
    );
    let Ok((text, model, _)) = call_llm(state, sys, &user, None, None, None, None).await else {
        return ToolResult::<serde_json::Value>::err_with_remediation(
            "LLM call failed (check model / keys)",
            "Configure MCP chat model (`vox_set_active_model`) and Secrets.",
        )
        .to_json();
    };
    let trimmed = text.trim();
    let act: ActJson = match serde_json::from_str(trimmed) {
        Ok(a) => a,
        Err(e) => {
            return ToolResult::<serde_json::Value>::err(format!(
                "act: model JSON parse {e}; raw={trimmed:?}"
            ))
            .to_json();
        }
    };
    let action = act.action.to_lowercase();
    let page_id = p.page_id.clone();
    let act_target = act.target.clone();
    let act_value = act.value.clone();
    let act_url = act.url.clone();
    let res: Result<(), String> = match action.as_str() {
        "noop" => Ok(()),
        "goto" => {
            let Some(url) = act_url.as_deref().filter(|u| !u.is_empty()) else {
                return ToolResult::<serde_json::Value>::err("act goto requires url".to_string())
                    .to_json();
            };
            let url = url.to_string();
            let page_id = page_id.clone();
            tokio::task::spawn_blocking(move || {
                with_browser_plugin(|p| {
                    let b = backend!(p);
                    b.goto(page_id.as_str().into(), url.as_str().into())
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("{e}"))
                })
            })
            .await
            .map_err(|e| format!("spawn_blocking: {e}"))
            .and_then(|r| r.map_err(|e| e.to_string()))
        }
        "wait" => {
            let Some(t) = act_target.as_deref().filter(|s| !s.is_empty()) else {
                return ToolResult::<serde_json::Value>::err(
                    "act wait requires target".to_string(),
                )
                .to_json();
            };
            let t = t.to_string();
            let page_id = page_id.clone();
            tokio::task::spawn_blocking(move || {
                with_browser_plugin(|p| {
                    let b = backend!(p);
                    b.wait_for(page_id.as_str().into(), t.as_str().into(), 30)
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("{e}"))
                })
            })
            .await
            .map_err(|e| format!("spawn_blocking: {e}"))
            .and_then(|r| r.map_err(|e| e.to_string()))
        }
        "click" => {
            let Some(t) = act_target.as_deref().filter(|s| !s.is_empty()) else {
                return ToolResult::<serde_json::Value>::err(
                    "act click requires target".to_string(),
                )
                .to_json();
            };
            let t = t.to_string();
            let page_id = page_id.clone();
            tokio::task::spawn_blocking(move || {
                with_browser_plugin(|p| {
                    let b = backend!(p);
                    b.click(page_id.as_str().into(), t.as_str().into())
                        .into_result()
                        .map_err(|e| anyhow::anyhow!("{e}"))
                })
            })
            .await
            .map_err(|e| format!("spawn_blocking: {e}"))
            .and_then(|r| r.map_err(|e| e.to_string()))
        }
        "fill" => {
            let (Some(t), Some(v)) = (
                act_target.as_deref().filter(|s| !s.is_empty()),
                act_value.as_deref(),
            ) else {
                return ToolResult::<serde_json::Value>::err(
                    "act fill requires target and value".to_string(),
                )
                .to_json();
            };
            let t = t.to_string();
            let v = v.to_string();
            let page_id = page_id.clone();
            tokio::task::spawn_blocking(move || {
                with_browser_plugin(|p| {
                    let b = backend!(p);
                    b.fill(
                        page_id.as_str().into(),
                        t.as_str().into(),
                        v.as_str().into(),
                    )
                    .into_result()
                    .map_err(|e| anyhow::anyhow!("{e}"))
                })
            })
            .await
            .map_err(|e| format!("spawn_blocking: {e}"))
            .and_then(|r| r.map_err(|e| e.to_string()))
        }
        _ => {
            return ToolResult::<serde_json::Value>::err(format!("unknown action {action:?}"))
                .to_json();
        }
    };
    match res {
        Ok(()) => ToolResult::ok(serde_json::json!({
            "ok": true,
            "action": action,
            "model": model,
            "execution_mode": "assisted",
        }))
        .to_json(),
        Err(e) => ToolResult::<serde_json::Value>::err(e.to_string()).to_json(),
    }
}
