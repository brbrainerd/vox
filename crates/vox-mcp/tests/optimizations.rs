//! Integration tests for MCP client optimizations.

use anyhow::Result;
use serde_json::json;
use vox_mcp::client::{McpClient, SchemaCache, ToolDefinition, format_cached_tool_schema};

#[tokio::test]
async fn test_native_read_file_short_circuit() -> Result<()> {
    let client = McpClient::new();

    // Create a temporary file to read
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("vox_mcp_test_native_read.txt");
    let content = "Hello, native MCP world!";
    std::fs::write(&file_path, content)?;

    let args = json!({
        "path": file_path.to_str().unwrap()
    });

    // call_tool should hit the short-circuit for read_file
    let result = client.call_tool("read_file", args).await?;

    // Our current implementation of read_file short-circuit returns a channel-initialized message
    // rather than the content directly because it starts a background stream for the content.
    assert!(result["streaming_channel_initialized"].as_bool().unwrap_or(false));
    assert_eq!(result["transport"], "InMemTransport");

    // Cleanup
    let _ = std::fs::remove_file(file_path);
    Ok(())
}

#[tokio::test]
async fn test_native_write_file_short_circuit() -> Result<()> {
    let client = McpClient::new();

    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("vox_mcp_test_native_write.txt");
    let content = "Writing from native client";

    let args = json!({
        "path": file_path.to_str().unwrap(),
        "content": content
    });

    let result = client.call_tool("write_file", args).await?;
    assert!(result["success"].as_bool().unwrap());

    // Verify file actually written
    let disk_content = std::fs::read_to_string(&file_path)?;
    assert_eq!(disk_content, content);

    // Cleanup
    let _ = std::fs::remove_file(file_path);
    Ok(())
}

#[tokio::test]
async fn test_schema_cache_lru() {
    let mut cache = SchemaCache::new(2);

    let t1 = ToolDefinition { name: "t1".to_string(), description: "d".to_string(), input_schema: json!({}) };
    let t2 = ToolDefinition { name: "t2".to_string(), description: "d".to_string(), input_schema: json!({}) };
    let t3 = ToolDefinition { name: "t3".to_string(), description: "d".to_string(), input_schema: json!({}) };

    cache.insert("s1", t1);
    cache.insert("s1", t2);

    assert!(cache.get("s1", "t1").is_some());

    // Insert t3, should evict t2 (since t1 was accessed)
    cache.insert("s1", t3);

    assert!(cache.get("s1", "t1").is_some());
    assert!(cache.get("s1", "t3").is_some());
    assert!(cache.get("s1", "t2").is_none());
}

#[tokio::test]
async fn test_prompt_caching_format() {
    let tool = ToolDefinition {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        input_schema: json!({ "type": "object" })
    };

    let formatted = format_cached_tool_schema(&tool);

    assert_eq!(formatted["type"], "function");
    assert_eq!(formatted["function"]["name"], "test_tool");
    assert_eq!(formatted["cache_control"]["type"], "ephemeral");
}
