---
title: "AI Agent Orchestration"
description: "How Vox natively integrates LLMs, agents, and local logic via the Model Context Protocol (MCP) and Distributed Execution Intelligence (DEI) orchestrator."
category: "Concepts"
status: "current"
training_eligible: true

schema_type: "TechArticle"
---

# AI Agent Orchestration

Vox was built from the ground up to blur the lines between traditional application logic and AI agent capabilities. Rather than bolting an AI SDK onto a web framework, Vox uses the **Model Context Protocol (MCP)** and its internal **DEI (Distributed Execution Intelligence) Orchestrator** as first-class citizens.

## The MCP Bridge

The Model Context Protocol establishes a standard way for AI assistants (like Claude Desktop, Cursor, or your own models) -> safely discover and interact with local data sources and tools.

Vox seamlessly generates MCP servers from workspace logic and federates `tool` declarations into the shipped `vox-mcp` orchestrator surface. See [MCP and Vox language exposure](../architecture/mcp-vox-language-exposure.md) for federation, collision rules, and parity gates.

### `tool` (Keyword)

The bare `tool` keyword tells the Vox compiler to expose a function on the workspace MCP surface. At orchestrator bind time, `WorkspaceMcpLoader` scans configured `.vox` globs, merges `AppContractModule.mcp_tools` into `vox-mcp` `tools/list`, and dispatches calls through the interpreter bridge.

```vox
tool "Calculate the shipping cost including surge pricing" calculate_shipping(weight: float, zip_code: str) to float {
    return weight * 0.5
}
```

The older `@tool` and `@mcp.tool` decorator forms still parse but emit `vox/decorator/mcp-tool-deprecated`; prefer the bare `tool` keyword.

Behind the scenes, Vox:
1. Derives the JSON Schema for the inputs (`weight` as a number, `zip_code` as a string).
2. Registers the tool on the federated MCP surface (static catalog tools win on name collision).
3. Maps Vox `Result` types directly to MCP error structures so the LLM knows *why* an operation failed without you writing serialization glue.

### `resource` (Keyword)

While tools are functions the LLM can call, resources are data the LLM can read. Workspace `resource` URIs are federated alongside static resources; installed skills also expose SEP-2640 `skill://` resources.

```vox
resource "vox://user/config" "The current user's profile configuration" get_user_profile() to str {
    return "profile context"
}
```

The orchestrator registers federated URIs. When an LLM requests `vox://user/config`, the orchestrator routes it to the matching workspace handler.

## DEI Orchestrator

The **Distributed Execution Intelligence (DEI)** orchestrator (historically confused with older crate naming; canonical crate is `vox-orchestrator`) is the runtime engine that manages these agents and tools.

When you run `vox run src/main.vox`, the orchestrator spins up, loads federated workspace `tool` handlers via `WorkspaceMcpLoader`, and starts an MCP endpoint that defaults to Stdio for desktop clients or HTTP/SSE for distributed meshes. Call `vox_workspace_mcp_refresh` after editing workspace tools to rescan without restarting.

### Agent-to-Agent (A2A) Messaging

The `agent { ... }` declaration is tombstoned and not in the active grammar — use a plain `fn` plus `tool`/`resource` declarations instead (see [How-To: Build AI Agents and MCP Tools](../how-to/how-to-ai-agents.md)). The DEI orchestrator fundamentally supports *Agent-to-Agent (A2A) messaging* at the tool level regardless of that surface syntax.

One agent can be granted the tools of another agent, executing what is effectively a sub-agent handoff. Because tools are just compiled Vox functions, a handoff entails an in-memory or fast-WASI call rather than a network hop to a secondary Python server.

## Security Controls

Because Vox exposes functions directly to reasoning engines, security is modeled differently than traditional web frameworks. The AI is bounded by the exact strictures of the Vox language: zero-null data, strict ADT matching, and the explicit `@require(condition)` precondition decorators, ensuring the LLM cannot hallucinate paths to execute invalid data modifications.

---

**Related Topics**:
- [Build AI Agent Tools](../how-to/how-to-ai-agents.md)
- [The Security Model](expl-security.md)
