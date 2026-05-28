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
use chromiumoxide_cdp::cdp::browser_protocol::page::CaptureScreenshotFormat;
use futures::StreamExt;
use tokio::sync::Mutex;
use tracing::debug;

struct HostInner {
    _handler_task: tokio::task::JoinHandle<()>,
    browser: Browser,
    pages: HashMap<String, chromiumoxide::Page>,
}

pub struct BrowserEngine {
    host: Mutex<Option<HostInner>>,
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
        Ok(id)
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

    pub async fn click(&self, page_id: &str, target: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let el = resolve_element(&page, target).await?;
        el.click().await.map_err(Self::map_page_err)?;
        Ok(())
    }

    pub async fn fill(&self, page_id: &str, target: &str, value: &str) -> Result<(), String> {
        let page = self.page_ref(page_id).await?;
        let el = resolve_element(&page, target).await?;
        el.click().await.map_err(Self::map_page_err)?;
        el.type_str(value).await.map_err(Self::map_page_err)?;
        Ok(())
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
                Err(_) => tokio::time::sleep(Duration::from_millis(200)).await,
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
        if stripped.len() <= max_chars {
            Ok(stripped)
        } else {
            Ok(format!("{}…", &stripped[..max_chars]))
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

static GLOBAL_ENGINE: std::sync::OnceLock<Arc<BrowserEngine>> = std::sync::OnceLock::new();

pub fn global_engine() -> Arc<BrowserEngine> {
    GLOBAL_ENGINE.get_or_init(BrowserEngine::new).clone()
}
