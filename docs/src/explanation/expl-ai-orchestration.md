---
title: "AI Agent Orchestration"
description: "How Vox natively integrates LLMs, agents, and local logic via the Model Context Protocol (MCP) and Distributed Execution Intelligence (DEI) orchestrator."
category: "Concepts"
status: "current"
last_updated: "2026-04-06"
training_eligible: true

schema_type: "TechArticle"
---

# AI Agent Orchestration

Vox was built from the ground up to blur the lines between traditional application logic and AI agent capabilities. Rather than bolting an AI SDK onto a web framework, Vox uses the **Model Context Protocol (MCP)** and its internal **DEI (Distributed Execution Intelligence) Orchestrator** as first-class citizens.

## The MCP Bridge

The Model Context Protocol establishes a standard way for AI assistants (like Claude Desktop, Cursor, or your own models) -> safely discover and interact with local data sources and tools.

Vox seamlessly generates MCP servers natively from the logic you've already written.

### `@mcp.tool`

The `@mcp.tool` decorator tells the Vox compiler to expose a function to any connected LLM. 

```vox
// vox:skip
@mcp.tool "Calculate the shipping cost including surge pricing"
fn calculate_shipping(weight: float, zip_code: str) to float {
    // Logic here
}
```

Behind the scenes, Vox:
1. Derives the JSON Schema for the inputs (`weight` as a number, `zip_code` as a string).
2. Generates an asynchronous Rust handler.
3. Maps Vox `Result` types directly to MCP error structures so the LLM knows *why* an operation failed without you writing serialization glue.

### `@mcp.resource`

While tools are functions the LLM can call, resources are data the LLM can read. 

```vox
// vox:skip
@mcp.resource("vox://user/config", "The current user's profile configuration")
fn get_user_profile() to str {
    return db.query("SELECT context FROM config")
}
```

The DEI orchestrator handles registering this URI schema. When an LLM requests `vox://user/config`, the orchestrator routes it directly to this function.

## DEI Orchestrator

The **Distributed Execution Intelligence (DEI)** orchestrator (historically confused with older crate naming; canonical crate is `vox-orchestrator`) is the runtime engine that manages these agents and tools.

When you run `vox run src/main.vox`, the orchestrator spins up, discovers all your decorated tools, and starts an MCP endpoint that defaults to Stdio for desktop clients or HTTP/SSE for distributed meshes.

### Agent-to-Agent (A2A) Messaging

Agents are scoped types in Vox. While the syntax is still aspirational (`@agent type`), the DEI orchestrator fundamentally supports *Agent-to-Agent (A2A) messaging*. 

One agent can be granted the tools of another agent, executing what is effectively a sub-agent handoff. Because tools are just compiled Vox functions, a handoff entails an in-memory or fast-WASI call rather than a network hop to a secondary Python server.

## Security Controls

Because Vox exposes functions directly to reasoning engines, security is modeled differently than traditional web frameworks. The AI is bounded by the exact strictures of the Vox language: zero-null data, strict ADT matching, and the explicit `@require(condition)` precondition decorators, ensuring the LLM cannot hallucinate paths to execute invalid data modifications.

---

**Related Topics**:
- [Build AI Agent Tools](../how-to/how-to-ai-agents.md)
- [The Security Model](expl-security.md)
