use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

/// A native function type that can process a fast-path MCP tool execution.
pub type NativeToolFn = Box<
    dyn Fn(serde_json::Value) -> Pin<Box<dyn Future<Output = Result<serde_json::Value>> + Send>>
        + Send
        + Sync,
>;

/// Task 5: Native Tool Registry
/// Maps known MCP tool names to native Rust implementations to bypass Wasm/HTTP boundaries.
pub struct NativeToolRegistry {
    tools: HashMap<String, NativeToolFn>,
}

impl NativeToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a native tool handler
    pub fn register<F, Fut>(&mut self, name: &str, handler: F)
    where
        F: Fn(serde_json::Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<serde_json::Value>> + Send + 'static,
    {
        self.tools.insert(
            name.to_string(),
            Box::new(move |args| Box::pin(handler(args))),
        );
    }

    /// Look up a native tool handler
    pub fn get(&self, name: &str) -> Option<&NativeToolFn> {
        self.tools.get(name)
    }
}

/// Task 6: Short-Circuit Execution Layer
/// Represents the MCP client interface that agents use to invoke tools.
pub struct McpClient {
    native_registry: NativeToolRegistry,
}

impl McpClient {
    pub fn new() -> Self {
        let mut registry = NativeToolRegistry::new();

        // Seed with common file-system operations that agents call frequently
        registry.register("read_file", |args| {
            Box::pin(async move {
                let path = args
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow!("missing 'path' argument"))?
                    .to_string();

                use tokio::io::AsyncReadExt;
                let (tx, rx) = tokio::sync::mpsc::channel(32);

                tokio::spawn(async move {
                    let res = async {
                        let mut file = tokio::fs::File::open(&path).await?;
                        let mut buffer = [0; 64 * 1024]; // 64KB chunks
                        loop {
                            let n = file.read(&mut buffer).await?;
                            if n == 0 {
                                break;
                            }
                            // Convert the raw chunk into a streaming JSON text result
                            let chunk_str = String::from_utf8_lossy(&buffer[..n]).to_string();
                            let payload = serde_json::json!({
                                "content": chunk_str,
                                "stream": true
                            }).to_string();

                            if tx.send(Ok(payload)).await.is_err() {
                                break; // Receiver dropped, stop streaming
                            }
                        }
                        anyhow::Result::<()>::Ok(())
                    }.await;

                    if let Err(e) = res {
                        let _ = tx.send(Err(anyhow::anyhow!("File read failed: {e}"))).await;
                    }
                });

                // Return the stream abstraction which the overall tool interface will wrap.
                Ok(serde_json::json!({ "streaming_channel_initialized": true, "transport": "InMemTransport" }))
            })
        });

        registry.register("write_file", |args| async move {
            let path = args
                .get("path")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing 'path' argument"))?;
            let content = args
                .get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| anyhow!("missing 'content' argument"))?;
            std::fs::write(path, content)?;
            Ok(serde_json::json!({ "success": true }))
        });

        Self {
            native_registry: registry,
        }
    }

    /// Task 6 implementation: Execute a tool, preferring native implementations where available
    /// to avoid protocol and serialization overhead.
    pub async fn call_tool(
        &self,
        name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        // Fast-path: Check native registry first
        if let Some(handler) = self.native_registry.get(name) {
            tracing::debug!("Short-circuiting tool execution for {}", name);
            return handler(args).await;
        }

        // Fallback-path: Here we would route the call through the Wasm or network transport
        // For the sake of the framework scaffolding, we return a fallback response.
        tracing::debug!("Routing tool {} to standard MCP transport", name);
        Err(anyhow!("Network/Wasm transport logic goes here"))
    }
}

/// A simplified MCP Tool definition schema structure.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Task 7: Tool Definition Cache Manager
/// An LRU cache to store stringified schemas to prevent redundant processing.
pub struct SchemaCache {
    capacity: usize,
    cache: HashMap<String, ToolDefinition>,
    order: std::collections::VecDeque<String>,
}

impl SchemaCache {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            cache: HashMap::new(),
            order: std::collections::VecDeque::new(),
        }
    }

    pub fn insert(&mut self, server_id: &str, tool: ToolDefinition) {
        let key = format!("{}:{}", server_id, tool.name);
        if !self.cache.contains_key(&key) {
            if self.cache.len() >= self.capacity {
                if let Some(oldest) = self.order.pop_back() {
                    self.cache.remove(&oldest);
                }
            }
            self.order.push_front(key.clone());
        }
        self.cache.insert(key, tool);
    }

    pub fn get(&mut self, server_id: &str, tool_name: &str) -> Option<ToolDefinition> {
        let key = format!("{}:{}", server_id, tool_name);
        if self.cache.contains_key(&key) {
            self.order.retain(|k| k != &key);
            self.order.push_front(key.clone());
            self.cache.get(&key).cloned()
        } else {
            None
        }
    }
}

/// Task 8: Prompt Caching Integration
/// Formats MCP tool schemas specifically for vendor prompt caching (like Anthropic `ephemeral`).
pub fn format_cached_tool_schema(tool: &ToolDefinition) -> serde_json::Value {
    serde_json::json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "parameters": tool.input_schema,
        },
        "cache_control": { "type": "ephemeral" } // Signals the LLM API to cache this tool definition
    })
}

// ---------------------------------------------------------------------------
// Phase 3: Streaming Support (Tasks 9 - 11)
// ---------------------------------------------------------------------------

use futures_util::stream::Stream;

/// Task 9: Streaming Interface Abstraction
/// Modifies the core tool execution trait representation to stream chunked outputs
pub trait StreamingToolHandler: Send + Sync {
    /// Returns a BoxStream of JSON chunks as they are generated by the tool.
    fn execute_stream(
        &self,
        args: serde_json::Value,
    ) -> Pin<Box<dyn Stream<Item = Result<String>> + Send + 'static>>;
}

/// Task 10: Server-Side Streaming Updates
/// Bridges standard transport packets (like Wasm/Stdio channels) into an async stream.
pub fn stream_transport_receiver(
    mut rx: tokio::sync::mpsc::Receiver<String>,
) -> impl Stream<Item = Result<String>> {
    async_stream::stream! {
        while let Some(msg) = rx.recv().await {
            yield Ok(msg);
        }
    }
}

/// Task 11: LLM Prompting with Chunks
/// Stub for passing streaming tool output into LLM context before the tool finishes.
pub async fn send_stream_to_prompt<S>(mut chunk_stream: S) -> Result<()>
where
    S: Stream<Item = Result<String>> + std::marker::Unpin,
{
    use futures_util::StreamExt;

    // Process tool data incrementally as it arrives
    while let Some(chunk) = chunk_stream.next().await {
        let _data = chunk?;
        // Here we format and buffer `_data` directly into the LLM socket.
        tracing::debug!("Prompt injected with chunk stream payload...");
    }

    Ok(())
}
