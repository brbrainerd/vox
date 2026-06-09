//! BrowserPlugin: wraps the async BrowserEngine behind sync sabi_trait methods.
//!
//! Each sabi_trait method blocks on the Tokio runtime that the plugin owns.
//! The runtime is started lazily on first use and lives for the process lifetime.

use std::sync::Arc;

use abi_stable::std_types::*;
use vox_plugin_api::extensions::browser_automation::BrowserAutomation;

use crate::engine::{BrowserEngine, global_engine};

/// The Tokio runtime used by the plugin for all async operations.
fn rt() -> &'static tokio::runtime::Runtime {
    static RT: std::sync::OnceLock<tokio::runtime::Runtime> = std::sync::OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("vox-browser-rt")
            .build()
            .expect("failed to build browser plugin tokio runtime")
    })
}

/// Plugin struct exposed to the plugin host.
#[derive(Clone)]
pub struct BrowserPlugin {
    engine: Arc<BrowserEngine>,
}

impl BrowserPlugin {
    pub fn new() -> Self {
        Self {
            engine: global_engine(),
        }
    }
}

fn to_rresult<T>(r: Result<T, String>) -> RResult<T, RBoxError> {
    match r {
        Ok(v) => RResult::ROk(v),
        Err(e) => RResult::RErr(RBoxError::new(std::io::Error::other(e))),
    }
}

impl BrowserAutomation for BrowserPlugin {
    fn open(&self, url: RStr<'_>, headless: bool) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let url = url.to_string();
        let result = rt().block_on(async move { engine.open(&url, headless).await });
        to_rresult(result.map(RString::from))
    }

    fn list_pages(&self) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let result = rt().block_on(async move { engine.list_pages().await });
        to_rresult(
            result
                .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
                .map(RString::from),
        )
    }

    fn page_info(&self, page_id: RStr<'_>) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.page_info(&page_id).await });
        to_rresult(
            result
                .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
                .map(RString::from),
        )
    }

    fn goto(&self, page_id: RStr<'_>, url: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let url = url.to_string();
        let result = rt().block_on(async move { engine.goto(&page_id, &url).await });
        to_rresult(result)
    }

    fn back(&self, page_id: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.back(&page_id).await });
        to_rresult(result)
    }

    fn forward(&self, page_id: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.forward(&page_id).await });
        to_rresult(result)
    }

    fn reload(&self, page_id: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.reload(&page_id).await });
        to_rresult(result)
    }

    fn stop(&self, page_id: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.stop(&page_id).await });
        to_rresult(result)
    }

    fn click(&self, page_id: RStr<'_>, target: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let target = target.to_string();
        let result = rt().block_on(async move { engine.click(&page_id, &target).await });
        to_rresult(result)
    }

    fn click_xy(&self, page_id: RStr<'_>, x: f64, y: f64) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.click_xy(&page_id, x, y).await });
        to_rresult(result)
    }

    fn fill(&self, page_id: RStr<'_>, target: RStr<'_>, value: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let target = target.to_string();
        let value = value.to_string();
        let result = rt().block_on(async move { engine.fill(&page_id, &target, &value).await });
        to_rresult(result)
    }

    fn scroll(&self, page_id: RStr<'_>, dx: i64, dy: i64) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.scroll(&page_id, dx, dy).await });
        to_rresult(result)
    }

    fn type_text(&self, page_id: RStr<'_>, text: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let text = text.to_string();
        let result = rt().block_on(async move { engine.type_text(&page_id, &text).await });
        to_rresult(result)
    }

    fn press(&self, page_id: RStr<'_>, key: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let key = key.to_string();
        let result = rt().block_on(async move { engine.press(&page_id, &key).await });
        to_rresult(result)
    }

    fn set_viewport(&self, page_id: RStr<'_>, width: u32, height: u32) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.set_viewport(&page_id, width, height).await });
        to_rresult(result)
    }

    fn wait_for(
        &self,
        page_id: RStr<'_>,
        target: RStr<'_>,
        timeout_secs: u64,
    ) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let target = target.to_string();
        let result =
            rt().block_on(async move { engine.wait_for(&page_id, &target, timeout_secs).await });
        to_rresult(result)
    }

    fn text(&self, page_id: RStr<'_>, target: RStr<'_>) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let target = target.to_string();
        let result = rt().block_on(async move { engine.text(&page_id, &target).await });
        to_rresult(result.map(RString::from))
    }

    fn html(&self, page_id: RStr<'_>, target: RStr<'_>) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let target = target.to_string();
        let result = rt().block_on(async move { engine.html(&page_id, &target).await });
        to_rresult(result.map(RString::from))
    }

    fn screenshot_bytes(&self, page_id: RStr<'_>) -> RResult<RVec<u8>, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.screenshot_bytes(&page_id).await });
        to_rresult(result.map(|v| v.into_iter().collect::<RVec<u8>>()))
    }

    fn screenshot_viewport_bytes(&self, page_id: RStr<'_>) -> RResult<RVec<u8>, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.screenshot_viewport_bytes(&page_id).await });
        to_rresult(result.map(|v| v.into_iter().collect::<RVec<u8>>()))
    }

    fn screencast_frame(&self, page_id: RStr<'_>) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.screencast_frame(&page_id).await });
        to_rresult(
            result
                .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
                .map(RString::from),
        )
    }

    fn screenshot(&self, page_id: RStr<'_>, path: RStr<'_>) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let path = path.to_string();
        let result = rt().block_on(async move { engine.screenshot(&page_id, &path).await });
        to_rresult(result.map(RString::from))
    }

    fn visible_text_summary(
        &self,
        page_id: RStr<'_>,
        max_chars: u64,
    ) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move {
            engine
                .visible_text_summary(&page_id, max_chars as usize)
                .await
        });
        to_rresult(result.map(RString::from))
    }

    fn ax_tree(&self, page_id: RStr<'_>) -> RResult<RString, RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.ax_tree(&page_id).await });
        to_rresult(
            result
                .and_then(|v| serde_json::to_string(&v).map_err(|e| e.to_string()))
                .map(RString::from),
        )
    }

    fn close(&self, page_id: RStr<'_>) -> RResult<(), RBoxError> {
        let engine = self.engine.clone();
        let page_id = page_id.to_string();
        let result = rt().block_on(async move { engine.close(&page_id).await });
        to_rresult(result)
    }
}
