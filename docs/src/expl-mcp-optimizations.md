# MCP Optimization Strategy

## Research Learnings: UTCP vs MCP

The Universal Tool Calling Protocol (UTCP) highlights the performance overhead inherent in the standard Model Context Protocol (MCP) due to its client-server architecture. What we can learn from UTCP's serverless approach:

1. **The Middleman Tax**: In standard MCP over Stdio or HTTP(SSE), every tool invocation requires JSON serialization, inter-process communication (IPC) or network transit, and deserialization.
2. **Context Bloat**: Verbose JSON schemas passed to the LLM context eat up tokens and delay time-to-first-token (TTFT).
3. **Simplicity Wins**: For local operations, full Node/Python MCP server processes are overkill.

## Optimizing MCP for "Serverless" Performance

To match UTCP's performance while retaining MCP's ecosystem compatibility and permission management, we must implement the following enhancements within our MCP architecture:

1. **In-Process (Embedded) MCP Servers via Wasm**: Load Wasm-compiled MCP servers directly into the agent's memory space, dropping IPC/Stdio in favor of direct in-memory function calls.
2. **Binary Transport Layers**: Negotiate binary transport formats (e.g., MessagePack, CBOR) to shrink payload sizes and reduce parsing overhead.
3. **"Fast-Path" Short Circuiting**: Intercept standard MCP schemas dynamically and bypass the MCP protocol for native tools (e.g., direct local filesystem access) running natively in Rust/Go.
4. **Tool Definition & Prompt Caching**: Cache tool definitions aggressively and utilize native prompt caching at the LLM API layer to slash TTFT.
5. **Streaming Execution**: Support streaming tool inputs/outputs to allow the agent to overlap reasoning computation with I/O waits.

## Implementation Task List

The following granular tasks outline the implementation plan. Each task is scoped for roughly equal complexity and effort.

### Phase 1: Wasm Embedding & Transport Optimization
- [ ] **Task 1: Wasm Runtime Integration** - Embed a Wasm runtime (e.g., Wasmtime or Wasmer) into our MCP client to support loading `.wasm` server modules.
- [ ] **Task 2: In-Memory Transport Layer** - Implement a new `InMemTransport` that bypasses Stdio/HTTP and routes MCP JSON-RPC messages directly to Wasm function exports.
- [ ] **Task 3: Binary Format Serialization** - Introduce MessagePack serialization/deserialization into the MCP message handling pipeline alongside standard JSON.
- [ ] **Task 4: Transport Capabilities Negotiation** - Update the MCP `initialize` handshake to exchange binary transport capabilities and smoothly upgrade the connection if supported.

### Phase 2: Native Short-Circuiting & Caching
- [ ] **Task 5: Native Tool Registry** - Create a registry to map known MCP tool names (like `read_file`, `write_file`) to internal Rust function implementations.
- [ ] **Task 6: Short-Circuit Execution Layer** - Modify the MCP client's `call_tool` logic to first check the native registry; if a match exists, execute the native function instead of routing to the standard MCP connection.
- [ ] **Task 7: Tool Definition Cache Manager** - Implement an LRU cache for MCP tool schemas fetched during server discovery to prevent redundant serialization.
- [ ] **Task 8: Prompt Caching Integration** - Update the LLM interface layer to format cached MCP tool schemas using the specific prompt caching headers (e.g., Anthropic's `ephemeral` cache control).

### Phase 3: Streaming Support
- [ ] **Task 9: Streaming Interface Abstraction** - Modify the core MCP tool execution trait to return an asynchronous stream (`Stream<Item = Result<String, Error>>`) rather than a monolithic JSON object.
- [ ] **Task 10: Server-Side Streaming Updates** - Update the Stdio/Wasm transport handlers to yield partial JSON-RPC or chunked packets back to the stream as they arrive.
- [ ] **Task 11: LLM Prompting with Chunks** - Integrate the stream receiver with the LLM prompt generator so that initial tool data chunks can be sent to the model while waiting for the rest of the stream.
