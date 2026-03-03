# Research Notes: Achieving Serverless-like Performance with MCP

## Context
The goal is to analyze what can be learned from connectionless or "serverless" paradigms like UCP (Universal Commerce Protocol or conceptually connectionless protocols like UDP) to enhance the Model Context Protocol (MCP) in Vox. We want to decrease overhead and improve performance while maintaining the power and compatibility of the existing MCP standard.

## Findings & Enhancements for MCP

### 1. In-Memory Short-Circuiting (Fast Path)
Native Vox tools (like `read_file` or `write_file`) should completely bypass standard MCP JSON-RPC over stdio when called from an internal agent.
- **How to apply:** Implement a `NativeToolRegistry` that handles native file-system tool requests synchronously and in-process. This removes serialization, pipe overhead, and latency constraints.

### 2. Prompt Caching & Schema LRU
MCP often suffers from redundant schema transmissions during tool initialization.
- **How to apply:** Use an LRU `SchemaCache` to avoid re-serializing and re-sending tool descriptions on every request. Implement Anthropic's `cache_control` headers so schemas are only parsed once per session by the LLM Provider.

### 3. Serverless Invocation & Streamable HTTP
To eliminate persistent server costs and avoid idle CPU overhead, MCP servers can be natively scaled down to zero.
- **How to apply:** Follow the SSE (Server-Sent Events) or HTTP chunked-encoding model. Instead of a long-lived process, tools can be triggered via HTTP routes or lambda-like handlers (e.g. `awslabs/mcp`).

### 4. Dynamic Context & "Pull" vs "Push"
MCP typically pushes context proactively. Serverless patterns prefer pulling only what is immediately required.
- **How to apply:** Resources and templates in MCP should return lightweight URIs or pagination cursors first, streaming the bulk payload only when requested.

---

# Implementation Task Plan

The following tasks are broken down with roughly equal difficulty to advance our infrastructure and optimizations natively.

- [ ] **Task 1: Complete the SchemaCache Implementation**
  - Ensure the `vox-mcp` crate caches all tool JSON schemas with LRU eviction.
  - Implement and verify the `prompt_caching` formatting for Anthropic / OpenAI.

- [ ] **Task 2: Native Tool Short-Circuit**
  - In `vox-mcp`, handle file tools (`read_file`, `write_file`) in-process for orchestrator agents without initiating a subprocess.
  - Enable and pass integration tests for `test_native_read_file_short_circuit`.

- [ ] **Task 3: Implement A2A (Agent-To-Agent) Connectionless Handoff**
  - Implement lightweight context handoff in the `vox-mcp` crate instead of routing through full prompt evaluation.
  - Minimize JSON payload size by transmitting diffs or delta states between agents.

- [ ] **Task 4: Setup Compiler-Driven Data Extraction (CI/CD)**
  - Add logic to the `vox check` command to emit training data JSONL.
  - Prepare a script to generate instruction-code pairs for model sync.

- [ ] **Task 5: Refine `check_search_index` in `vox-typeck`**
  - Implement the missing type-checking blocks for `SearchIndexDecl` to ensure database stability.
