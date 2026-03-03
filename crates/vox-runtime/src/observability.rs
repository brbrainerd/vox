use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Lightweight request context for tracing and provenance.
#[derive(Debug, Clone)]
pub struct RequestContext {
    pub request_id: String,
    pub route: String,
    start: Instant,
}

impl RequestContext {
    pub fn new(route: impl Into<String>) -> Self {
        Self::with_request_id(route, None)
    }

    pub fn with_request_id(route: impl Into<String>, request_id: Option<&str>) -> Self {
        let request_id = request_id
            .filter(|v| !v.is_empty())
            .map(ToOwned::to_owned)
            .unwrap_or_else(generate_request_id);
        Self {
            request_id,
            route: route.into(),
            start: Instant::now(),
        }
    }

    pub fn elapsed_ms(&self) -> u128 {
        self.start.elapsed().as_millis()
    }
}

fn generate_request_id() -> String {
    let counter = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("vox-{ts}-{counter}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_ids_are_stable_format() {
        let ctx = RequestContext::new("/chat");
        assert!(ctx.request_id.starts_with("vox-"));
        assert_eq!(ctx.route, "/chat");
    }
}
