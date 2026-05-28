//! P0-T8: W3C traceparent encode / parse / from-current-span helpers.
//!
//! Format: `version "-" trace-id "-" parent-id "-" trace-flags`
//!           00       32 hex      16 hex       2 hex

use rand::RngCore as _;

/// Parsed W3C `traceparent` header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceContext {
    /// 32 lowercase hex characters (128-bit trace ID).
    pub trace_id: String,
    /// 16 lowercase hex characters (64-bit parent span ID).
    pub parent_id: String,
    /// Trace flags byte (0x01 = sampled).
    pub flags: u8,
}

impl TraceContext {
    /// Generate a new, randomly-seeded `TraceContext` with the sampled flag set.
    pub fn new() -> Self {
        let mut rng = rand::thread_rng();
        let mut t = [0u8; 16];
        let mut p = [0u8; 8];
        rng.fill_bytes(&mut t);
        rng.fill_bytes(&mut p);
        Self {
            trace_id: hex::encode(t),
            parent_id: hex::encode(p),
            flags: 0x01,
        }
    }

    /// Attempt to pull trace context from the current `tracing` span.
    ///
    /// Falls back to a freshly-generated context when no opentelemetry
    /// context propagator is wired (S1 — field recording only; S2 will
    /// attach the span as a proper parent).
    pub fn from_current_span() -> Self {
        Self::new()
    }
}

impl Default for TraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Encode a [`TraceContext`] as a W3C `traceparent` header value.
pub fn encode(ctx: &TraceContext) -> String {
    format!("00-{}-{}-{:02x}", ctx.trace_id, ctx.parent_id, ctx.flags)
}

/// Parse a W3C `traceparent` header value.
///
/// Returns `None` for malformed or version-unsupported headers.
pub fn parse(s: &str) -> Option<TraceContext> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 4 {
        return None;
    }
    if parts[0] != "00" {
        return None;
    }
    if parts[1].len() != 32 || !parts[1].chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    if parts[2].len() != 16 || !parts[2].chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let flags = u8::from_str_radix(parts[3], 16).ok()?;
    Some(TraceContext {
        trace_id: parts[1].to_string(),
        parent_id: parts[2].to_string(),
        flags,
    })
}
