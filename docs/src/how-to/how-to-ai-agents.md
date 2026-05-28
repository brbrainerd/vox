---
title: "How-To: Build AI Agents and MCP Tools"
description: "Expand your Vox logic into reusable tools for AI assistants and define specialized agents using the Model Context Protocol (MCP)."
category: "How-To Guides"
last_updated: "2026-04-05"
training_eligible: true

schema_type: "HowTo"
keywords: ["MCP tools Vox", "AI agents tutorial", "Model Context Protocol programming", "build AI agents Rust"]
---

# How-To: Build AI Agents and MCP Tools

Vox is an AI-native language, meaning it bridges the gap between high-level business logic and the Model Context Protocol (MCP) without glue code. Any Vox function can become an MCP tool with a single decorator.

## 1. Creating MCP Tools

Any Vox function can be exported as an MCP tool using the `@mcp.tool` decorator. 

```vox
{{#include ../../../examples/golden/ref_orchestrator.vox:mcp_tool}}
```

### Comparison to other approaches:
- **Type Safety**: If your function returns a `Result[T, E]`, Vox handles the MCP error response mapping for you.
- **Zero Configuration**: No and manifests to maintain. The `@mcp.tool` decorator is the manifest.
- **Auto-Discovery**: Tools are automatically discovered by the `vox-orchestrator` during development.

---

## 2. Defining Agent Roles

Agents in Vox are not just prompts; they are scoped types that bundle specific tools and instructions. Use the `@agent` decorator to define an agent's identity.

> [!NOTE]
> The `agent` declaration is now a first-class HIR element in Vox v0.3, enabling static validation of toolsets and instructions.

```vox
{{#include ../../../examples/golden/ref_agents.vox:basic_agent}}
```

### Agent Handoffs
Agents can call other agents if you grant them the tool to do so. In Vox, an agent's `tools` list can include other agent identifiers.

---

## 3. Tool Discovery and Execution

To expose your tools to a local AI assistant (like Claude Desktop or Cursor):

1. **Run the MCP server**:
   ```bash
   vox run src/main.vox
   ```
2. **Observe Logs**: The orchestrator will list all registered tools and resources.
3. **Connect**: Add the generated endpoint to your `claude_desktop_config.json`.

---

## 4. Testing Your Tools

Never guess if a tool works. You can test your tool directly against the generated server. (Note: A dedicated `vox test-mcp` CLI is an aspirational future feature).

```bash
# Test the 'search_docs' endpoint manually using standard tools
curl -X POST http://localhost:8080/api/tools/search_docs -d '{"query": "actors"}'
```

---

## 5. Security and Bounds

By default, an `@mcp.tool` has the same permissions as your compiled Vox binary. Use the `@require` decorator to add runtime guardrails:

```vox
// vox:skip
@mcp.tool "Delete user data"
@require(auth.is_admin(caller))
@endpoint(kind: mutation) fn delete_data(id: int) to Result[Unit] {
    db.delete(id)
    return Ok(())
}
```

If the precondition fails, the MCP tool returns a "Tool execution failed" error to the model with the specific violation reason, preventing the LLM from attempting unauthorized actions.

---

**Related Reference**:
- [MCP Protocol SSOT](../reference/secrets-ssot.md)
- [Agentic Loop Blueprint](../explanation/why-vox-for-ai.md)
- [CLI Reference: vox mcp](../reference/cli.md)


