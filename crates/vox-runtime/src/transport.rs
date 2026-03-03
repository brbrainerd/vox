//! Stream transport for chat and real-time endpoints.
//!
//! Generated Vox apps use **SSE (Server-Sent Events)** by default for streaming
//! chat and subscription updates. **WebSocket** is reserved for future use when
//! high-frequency bidirectional streams are needed (e.g. low-latency token streams).
//! The runtime and codegen use this enum so a future WebSocket path can be added
//! without breaking the API.

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
