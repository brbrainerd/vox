//! Stream transport for chat and real-time endpoints.
//!
//! Generated Vox apps use **SSE (Server-Sent Events)** by default for streaming
//! chat and subscription updates. **WebSocket** is reserved for future use when
//! high-frequency bidirectional streams are needed (e.g. low-latency token streams).
//! The runtime and codegen use this enum so a future WebSocket path can be added
//! without breaking the API.
//!
//! ## v1.x codegen wiring
//!
//! `StreamTransport::WebSocket` is forward-declared but not yet wired into the
//! Rust / TypeScript codegen path. When `WebSocket` support is added in v1.x,
//! the codegen emitter for `@chat` and `@subscribe` endpoints should:
//! 1. Check `stream_transport == StreamTransport::WebSocket`.
//! 2. Emit a `tokio-tungstenite` upgrade path instead of the SSE `EventStream`.
//! Until then, the emitter treats any unknown variant as `Sse` (safe default).

/// Identifies the transport used for a streaming endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub enum StreamTransport {
    /// Server-Sent Events (default): one-way server→client, simple and widely supported.
    #[default]
    Sse,
    /// WebSocket (reserved): bidirectional, lower latency; not yet implemented in codegen.
    WebSocket,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_transport_is_sse() {
        assert_eq!(StreamTransport::default(), StreamTransport::Sse);
    }
}
