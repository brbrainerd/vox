//! Chromiumoxide CDP session implementation.
//!
//! Ported from `vox-browser::engine` with the same logic. The plugin layer
//! wraps async calls via a dedicated Tokio runtime so the sabi_trait methods
//! (which must be synchronous) can block.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use chromiumoxide::Browser;
use chromiumoxide::browser::BrowserConfig;
use chromiumoxide::page::ScreenshotParams;
use chromiumoxide_cdp::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
use chromiumoxide_cdp::cdp::browser_protocol::input::{
    DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams, DispatchMouseEventType,
    InsertTextParams, MouseButton,
};
use chromiumoxide_cdp::cdp::browser_protocol::page::{
    CaptureScreenshotFormat, EventScreencastFrame, GetNavigationHistoryParams,
    NavigateToHistoryEntryParams, ReloadParams, ScreencastFrameAckParams, StartScreencastFormat,
    StartScreencastParams, StopLoadingParams, StopScreencastParams, Viewport,
};
use futures::StreamExt;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing::debug;

struct HostInner {
    _handler_task: tokio::task::JoinHandle<()>,
    browser: Browser,
    pages: HashMap<String, chromiumoxide::Page>,
    viewports: HashMap<String, ViewportMetrics>,
}

pub struct BrowserEngine {
    host: Mutex<Option<HostInner>>,
}

#[derive(Debug, Clone, Serialize)]
struct PageSummary {
    page_id: String,
    url: String,
    title: String,
}

#[derive(Debug, Clone, Serialize)]
struct PageInfo {
    page_id: String,
    url: String,
    title: String,
    can_go_back: bool,
    can_go_forward: bool,
}

#[derive(Debug, Clone, Copy)]
struct ViewportMetrics {
    width: u32,
    height: u32,
}

impl Default for ViewportMetrics {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 800,
        }
    }
}

impl Default for BrowserEngine {
    fn default() -> Self {
        Self {
            host: Mutex::new(None),
        }
    }
}

impl BrowserEngine {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    async fn ensure_host(&self, headless: bool) -> Result<(), String> {
        let mut guard = self.host.lock().await;
        if guard.is_some() {
            return Ok(());
        }

        let mut builder = BrowserConfig::builder()
            .request_timeout(Duration::from_secs(90))
            .launch_timeout(vox_config::timeouts::D_60S);
        builder = if headless {
            builder.new_headless_mode()
        } else {
            builder.with_head()
        };
        if let Ok(exe) = std::env::var("VOX_CHROME_EXECUTABLE") {
            let exe = exe.trim();
            if !exe.is_empty() {
                builder = builder.chrome_executable(exe);
            }
        }
        if std::env::var("VOX_BROWSER_NO_SANDBOX")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false)
        {
            builder = builder.no_sandbox();
        }

        let config = builder
            .build()
            .map_err(|e| format!("browser config: {e}"))?;
        let (browser, mut handler) = Browser::launch(config)
            .await
            .map_err(|e| format!("Browser::launch failed: {e}"))?;

        let handler_task = tokio::spawn(async move { while handler.next().await.is_some() {} });

        *guard = Some(HostInner {
            _handler_task: handler_task,
            browser,
            pages: HashMap::new(),
            viewports: HashMap::new(),
        });
        debug!(target: "vox_plugin_browser", "chromium host launched");
        Ok(())
    }

    pub async fn open(&self, url: &str, headless: bool) -> Result<String, String> {
        self.ensure_host(headless).await?;
        let mut guard = self.host.lock().await;
        let host = guard
            .as_mut()
            .ok_or_else(|| "browser host missing".to_string())?;
        let page = host
            .browser
            .new_page("about:blank")
            .await
            .map_err(|e| format!("new_page: {e}"))?;
        page.goto(url)
            .await
            .map_err(|e| format!("goto {url}: {e}"))?;
        let id = format!("page-{}", uuid::Uuid::new_v4());
        host.pages.insert(id.clone(), page);
        host.viewports
            .insert(id.clone(), ViewportMetrics::default());
        Ok(id)
    }

    pub async fn list_pages(&self) -> Result<serde_json::Value, String> {
        let pages: Vec<(String, chromiumoxide::Page)> = {
            let guard = self.host.lock().await;
            let host = guard
                .as_ref()
                .ok_or_else(|| "no browser host; call open first".to_string())?;
            host.pages
                .iter()
                .map(|(id, page)| (id.clone(), page.clone()))
                .collect()
        };
        let mut out = Vec::with_capacity(pages.len());
        for (page_id, page) in pages {
            let url = page_url(&page).await.unwrap_or_default();
            let title = page
                .get_title()
                .await
                .unwrap_or_default()
                .unwrap_or_default();
            out.push(PageSummary {
                page_id,
                url,
                title,
            });
        }
        serde_json::to_value(out).map_err(|e| e.to_string())
    }

    pub async fn page_info(&self, page_id: &str) -> Result<serde_json::Value, String> {
        let page = self.page_ref(page_id).await?;
        let history = page
            .execute(GetNavigationHistoryParams::default())
            .await
            .map_err(Self::map_page_err)?;
        let current_index = history.current_index as usize;
        let total = history.entries.len();
        let url = page_url(&page).await.unwrap_or_default();
        let title = page
            .get_title()
            .await
            .unwrap_or_default()
            .unwrap_or_default();
        let (can_go_back, can_go_forward) = history_capabilities(current_index, total);
        let info = PageInfo {
            page_id: page_id.to_string(),
            url,
            title,
            can_go_back,
            can_go_forward,
        };
        serde_json::to_value(info).map_err(|e| e.to_string())
    }

    fn map_page_err(e: chromiumoxide::error::CdpError) -> String {
        e.to_string()
    }

    async fn page_ref(&self, page_id: &str) -> Result<chromiumoxide::Page, String> {
        let guard = self.host.lock().await;
        let host = guard
            .as_ref()
            .ok_or_else(|| "no browser host; call open first".to_string())?;
        host.pages
            .get(page_id)
            .cloned()
            .ok_or_else(|| format!("unknown page_id {page_id:?}"))
    }

    pub async fn close(&self, page_id: &str) -> Result<(), String> {
        let mut guard = self.host.lock().await;
        let shutdown = {
            let Some(host) = guard.as_mut() else {
                return Ok(());
            };
            if let Some(page) = host.pages.remove(page_id) {
                let _ = page.close().await;
            }
            host.viewports.remove(page_id);
            host.pages.is_empty()
        };
        if shutdown {
            if let Some(inner) = guard.take() {
                inner._handler_task.abort();
                drop(inner.browser);
            }
            debug!(target: "vox_plugin_browser", "browser host shut down (no sessions)");
        }
        Ok(())
    }

    pub async fn goto(&self, page_id: &str, url: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        page.goto(url).await.map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn back(&self, page_id: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let history = page
            .execute(GetNavigationHistoryParams::default())
            .await
            .map_err(Self::map_page_err)?;
        let index = history.current_index as usize;
        if index == 0 || history.entries.is_empty() {
            return Ok(());
        }
        let entry_id = history
            .entries
            .get(index - 1)
            .ok_or_else(|| "no previous history entry".to_string())?
            .id;
        page.execute(NavigateToHistoryEntryParams { entry_id })
            .await
            .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn forward(&self, page_id: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let history = page
            .execute(GetNavigationHistoryParams::default())
            .await
            .map_err(Self::map_page_err)?;
        let index = history.current_index as usize;
        let next = index + 1;
        if history.entries.is_empty() || next >= history.entries.len() {
            return Ok(());
        }
        let entry_id = history.entries[next].id;
        page.execute(NavigateToHistoryEntryParams { entry_id })
            .await
            .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn reload(&self, page_id: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        page.execute(ReloadParams::default())
            .await
            .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn stop(&self, page_id: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        page.execute(StopLoadingParams::default())
            .await
            .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn click(&self, page_id: &str, target: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let el = resolve_element(&page, target).await?;
        el.click().await.map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn click_xy(&self, page_id: &str, x: f64, y: f64) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(x)
                .y(y)
                .build()
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(Self::map_page_err)?;
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MousePressed)
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .buttons(1)
                .click_count(1)
                .build()
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(Self::map_page_err)?;
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseReleased)
                .x(x)
                .y(y)
                .button(MouseButton::Left)
                .buttons(0)
                .click_count(1)
                .build()
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn fill(&self, page_id: &str, target: &str, value: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let el = resolve_element(&page, target).await?;
        el.click().await.map_err(Self::map_page_err)?;
        el.type_str(value).await.map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn scroll(&self, page_id: &str, dx: i64, dy: i64) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let vp = self.viewport_for(page_id).await;
        let x = (vp.width as f64) / 2.0;
        let y = (vp.height as f64) / 2.0;
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseWheel)
                .x(x)
                .y(y)
                .delta_x(dx as f64)
                .delta_y(dy as f64)
                .build()
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(Self::map_page_err)?;
        page.execute(
            DispatchMouseEventParams::builder()
                .r#type(DispatchMouseEventType::MouseMoved)
                .x(x)
                .y(y)
                .build()
                .map_err(|e| e.to_string())?,
        )
        .await
        .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn type_text(&self, page_id: &str, text: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        if text.is_empty() {
            return Ok(());
        }
        page.execute(InsertTextParams::new(text))
            .await
            .map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn press(&self, page_id: &str, key: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let chord = KeyChord::parse(key);
        let down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .modifiers(chord.modifiers)
            .key(chord.key.clone())
            .code(chord.code.clone())
            .windows_virtual_key_code(chord.windows_vk)
            .native_virtual_key_code(chord.windows_vk)
            .build()
            .map_err(|e| e.to_string())?;
        page.execute(down).await.map_err(Self::map_page_err)?;
        let up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .modifiers(chord.modifiers)
            .key(chord.key)
            .code(chord.code)
            .windows_virtual_key_code(chord.windows_vk)
            .native_virtual_key_code(chord.windows_vk)
            .build()
            .map_err(|e| e.to_string())?;
        page.execute(up).await.map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn set_viewport(&self, page_id: &str, width: u32, height: u32) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let params = SetDeviceMetricsOverrideParams::builder()
            .width(width as i64)
            .height(height as i64)
            .device_scale_factor(1.0)
            .mobile(false)
            .build()
            .map_err(|e| e.to_string())?;
        page.execute(params).await.map_err(Self::map_page_err)?;
        let mut guard = self.host.lock().await;
        if let Some(host) = guard.as_mut() {
            host.viewports
                .insert(page_id.to_string(), ViewportMetrics { width, height });
        }
        Ok(())
    }

    async fn viewport_for(&self, page_id: &str) -> ViewportMetrics {
        let guard = self.host.lock().await;
        guard
            .as_ref()
            .and_then(|host| host.viewports.get(page_id))
            .copied()
            .unwrap_or_default()
    }

    pub async fn wait_for(
        &self,
        page_id: &str,
        target: &str,
        timeout_secs: u64,
    ) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let deadline = Duration::from_secs(timeout_secs.max(1));
        let start = std::time::Instant::now();
        loop {
            if start.elapsed() > deadline {
                return Err(format!(
                    "wait_for timeout after {timeout_secs}s for selector {target:?}"
                ));
            }
            match resolve_element(&page, target).await {
                Ok(_) => return Ok(()),
                Err(_) => tokio::time::sleep(vox_config::timeouts::D_200MS).await,
            }
        }
    }

    pub async fn text(&self, page_id: &str, target: &str) -> Result<String, String> {
        let page = self.page_ref(page_id).await?;
        let el = resolve_element(&page, target).await?;
        el.inner_text()
            .await
            .map_err(Self::map_page_err)?
            .ok_or_else(|| "element has no inner_text".to_string())
    }

    pub async fn html(&self, page_id: &str, target: &str) -> Result<String, String> {
        let page = self.page_ref(page_id).await?;
        if target.trim().is_empty() {
            return page.content().await.map_err(Self::map_page_err);
        }
        let el = resolve_element(&page, target).await?;
        el.outer_html()
            .await
            .map_err(Self::map_page_err)?
            .ok_or_else(|| "element has no outer_html".to_string())
    }

    pub async fn screenshot_bytes(&self, page_id: &str) -> Result<Vec<u8>, String> {
        let page = self.page_ref(page_id).await?;
        page.screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(true)
                .build(),
        )
        .await
        .map_err(Self::map_page_err)
    }

    pub async fn screenshot_viewport_bytes(&self, page_id: &str) -> Result<Vec<u8>, String> {
        let page = self.page_ref(page_id).await?;
        let vp = self.viewport_for(page_id).await;
        let clip = Viewport::builder()
            .x(0.0)
            .y(0.0)
            .width(vp.width as f64)
            .height(vp.height as f64)
            .scale(1.0)
            .build()
            .map_err(|e| e.to_string())?;
        page.screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(false)
                .clip(clip)
                .build(),
        )
        .await
        .map_err(Self::map_page_err)
    }

    pub async fn screencast_frame(&self, page_id: &str) -> Result<serde_json::Value, String> {
        let page = self.page_ref(page_id).await?;
        let vp = self.viewport_for(page_id).await;
        page.execute(
            StartScreencastParams::builder()
                .format(StartScreencastFormat::Jpeg)
                .quality(80)
                .max_width(vp.width as i64)
                .max_height(vp.height as i64)
                .every_nth_frame(1)
                .build(),
        )
        .await
        .map_err(Self::map_page_err)?;
        let mut events = page
            .event_listener::<EventScreencastFrame>()
            .await
            .map_err(|e| e.to_string())?;
        let next = tokio::time::timeout(vox_config::timeouts::D_2S, events.next())
            .await
            .ok()
            .flatten();
        let _ = page.execute(StopScreencastParams::default()).await;
        if let Some(frame) = next {
            let _ = page
                .execute(ScreencastFrameAckParams::new(frame.session_id))
                .await;
            return Ok(serde_json::json!({
                "image_base64": AsRef::<str>::as_ref(&frame.data),
                "viewport_width": frame.metadata.device_width as u32,
                "viewport_height": frame.metadata.device_height as u32
            }));
        }
        Err("no screencast frame received before timeout".to_string())
    }

    pub async fn screenshot(&self, page_id: &str, path: &str) -> Result<String, String> {
        let page = self.page_ref(page_id).await?;
        let p = Path::new(path);
        if let Some(parent) = p.parent()
            && !parent.as_os_str().is_empty()
        {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| e.to_string())?;
        }
        page.save_screenshot(
            ScreenshotParams::builder()
                .format(CaptureScreenshotFormat::Png)
                .full_page(true)
                .build(),
            path,
        )
        .await
        .map_err(Self::map_page_err)?;
        Ok(path.to_string())
    }

    pub async fn visible_text_summary(
        &self,
        page_id: &str,
        max_chars: usize,
    ) -> Result<String, String> {
        let page = self.page_ref(page_id).await?;
        let html = page.content().await.map_err(Self::map_page_err)?;
        let stripped = strip_html_tags(&html);
        let max_chars = max_chars.max(256);
        if stripped.chars().count() <= max_chars {
            Ok(stripped)
        } else {
            // Truncate by char count, not byte index: `stripped` is readability
            // text from arbitrary web pages and routinely contains multibyte
            // UTF-8 (curly quotes, accents, CJK). A byte slice at `max_chars`
            // would panic on a non-char-boundary.
            // Budget the 1-char ellipsis into the cap so the result never
            // exceeds max_chars.
            Ok(format!(
                "{}…",
                stripped
                    .chars()
                    .take(max_chars.saturating_sub(1))
                    .collect::<String>()
            ))
        }
    }

    pub async fn ax_tree(&self, page_id: &str) -> Result<serde_json::Value, String> {
        let page = self.page_ref(page_id).await?;
        let res = page
            .execute(
                chromiumoxide_cdp::cdp::browser_protocol::accessibility::GetFullAxTreeParams::default(),
            )
            .await
            .map_err(|e| format!("AXTree CDP failed: {e}"))?;
        serde_json::to_value(res.nodes.clone()).map_err(|e: serde_json::Error| e.to_string())
    }
}

async fn resolve_element(
    page: &chromiumoxide::Page,
    target: &str,
) -> Result<chromiumoxide::Element, String> {
    let t = target.trim();
    if t.is_empty() {
        return Err("target selector must not be empty".to_string());
    }
    if let Some(rest) = t.strip_prefix("xpath:").map(str::trim) {
        return page.find_xpath(rest).await.map_err(|e| e.to_string());
    }
    page.find_element(t).await.map_err(|e| e.to_string())
}

async fn page_url(page: &chromiumoxide::Page) -> Result<String, String> {
    page.evaluate("window.location.href")
        .await
        .map_err(|e| e.to_string())?
        .into_value::<String>()
        .map_err(|e| e.to_string())
}

fn strip_html_tags(html: &str) -> String {
    let mut out = String::with_capacity(html.len().min(262_144));
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn history_capabilities(current_index: usize, total_entries: usize) -> (bool, bool) {
    if total_entries == 0 {
        return (false, false);
    }
    let can_go_back = current_index > 0;
    let can_go_forward = (current_index + 1) < total_entries;
    (can_go_back, can_go_forward)
}

#[derive(Debug, Clone)]
struct KeyChord {
    modifiers: i64,
    key: String,
    code: String,
    windows_vk: i64,
}

impl KeyChord {
    fn parse(raw: &str) -> Self {
        let mut modifiers = 0_i64;
        let mut key_token = None::<&str>;
        for token in raw.split('+').map(str::trim).filter(|t| !t.is_empty()) {
            match token.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => modifiers |= 2,
                "alt" | "option" => modifiers |= 1,
                "meta" | "cmd" | "command" => modifiers |= 4,
                "shift" => modifiers |= 8,
                _ => key_token = Some(token),
            }
        }
        let key = key_token.unwrap_or(raw).trim();
        let (norm_key, code, windows_vk) = key_identity(key);
        Self {
            modifiers,
            key: norm_key,
            code,
            windows_vk,
        }
    }
}

fn key_identity(key: &str) -> (String, String, i64) {
    match key {
        "Enter" => ("Enter".into(), "Enter".into(), 13),
        "Tab" => ("Tab".into(), "Tab".into(), 9),
        "Escape" | "Esc" => ("Escape".into(), "Escape".into(), 27),
        "Backspace" => ("Backspace".into(), "Backspace".into(), 8),
        "Delete" => ("Delete".into(), "Delete".into(), 46),
        "Home" => ("Home".into(), "Home".into(), 36),
        "End" => ("End".into(), "End".into(), 35),
        "PageUp" => ("PageUp".into(), "PageUp".into(), 33),
        "PageDown" => ("PageDown".into(), "PageDown".into(), 34),
        "ArrowUp" => ("ArrowUp".into(), "ArrowUp".into(), 38),
        "ArrowDown" => ("ArrowDown".into(), "ArrowDown".into(), 40),
        "ArrowLeft" => ("ArrowLeft".into(), "ArrowLeft".into(), 37),
        "ArrowRight" => ("ArrowRight".into(), "ArrowRight".into(), 39),
        "Space" | " " => (" ".into(), "Space".into(), 32),
        _ if key.chars().count() == 1 => {
            let ch = key.chars().next().unwrap_or_default();
            if ch.is_ascii_alphabetic() {
                let upper = ch.to_ascii_uppercase();
                let vk = upper as i64;
                (upper.to_string(), format!("Key{upper}"), vk)
            } else if ch.is_ascii_digit() {
                let vk = ch as i64;
                (ch.to_string(), format!("Digit{ch}"), vk)
            } else {
                (key.to_string(), key.to_string(), 0)
            }
        }
        _ => (key.to_string(), key.to_string(), 0),
    }
}

static GLOBAL_ENGINE: std::sync::OnceLock<Arc<BrowserEngine>> = std::sync::OnceLock::new();

pub fn global_engine() -> Arc<BrowserEngine> {
    GLOBAL_ENGINE.get_or_init(BrowserEngine::new).clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_capabilities_flags() {
        assert_eq!(history_capabilities(0, 0), (false, false));
        assert_eq!(history_capabilities(0, 1), (false, false));
        assert_eq!(history_capabilities(0, 2), (false, true));
        assert_eq!(history_capabilities(1, 2), (true, false));
        assert_eq!(history_capabilities(1, 3), (true, true));
    }

    #[test]
    fn key_chord_parses_modifiers_and_key_identity() {
        let ctrl_l = KeyChord::parse("Ctrl+L");
        assert_eq!(ctrl_l.modifiers, 2);
        assert_eq!(ctrl_l.key, "L");
        assert_eq!(ctrl_l.code, "KeyL");

        let shift_tab = KeyChord::parse("Shift+Tab");
        assert_eq!(shift_tab.modifiers, 8);
        assert_eq!(shift_tab.key, "Tab");
        assert_eq!(shift_tab.code, "Tab");
    }

    #[tokio::test]
    #[ignore = "slow; requires local Chrome/Chromium binary"]
    async fn engine_open_goto_back_list_pages_smoke() {
        let engine = BrowserEngine::new();
        let page_id = engine
            .open("https://example.com", true)
            .await
            .expect("open example.com");
        engine
            .goto(&page_id, "https://example.org")
            .await
            .expect("goto example.org");
        let _ = engine.back(&page_id).await;
        let pages = engine.list_pages().await.expect("list_pages");
        let arr = pages.as_array().expect("pages array");
        assert!(!arr.is_empty(), "expected at least one open page");
        engine.close(&page_id).await.expect("close page");
    }
}
