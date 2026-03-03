# How-To: Build AI Agents and MCP Tools

Learn how to export your Vox logic as tools for AI assistants and define specialized agents using the Model Context Protocol (MCP).

## 1. Creating MCP Tools

Any Vox function can be exported as an MCP tool using the `@mcp.tool` decorator. This makes it available to native AI agents and standard MCP clients.

```vox
# Skip-Test
@mcp.tool("Search for user documentation by topic")
@query fn search_docs(query: str) to list[str]:
    # Search implementation
    ret ["Result 1", "Result 2"]
```

## 2. Defining Agent Roles

Define specialized agent behaviors using the `@agent` decorator. You can specify the agent's instructions and the tools it has access to.

```vox
# Skip-Test
@agent type Documenter:
    instructions: "You are a technical writer specializing in Vox architecture."
    tools: [search_docs, vox_ast_reference]
```

## 3. Tool Discovery

Once your app is running with `vox mcp run`, tools are automatically registered with the host environment. AI agents can then discover and call them as needed.

## 4. Testing Your Tools

Use the `vox test-mcp` command to verify that your tools are correctly exposed and responding with the expected JSON format.

```bash
vox test-mcp --call search_docs '{"query": "actors"}'
```

---

**Related Reference**:

- [MCP Reference](api/vox-mcp.md) — Low-level protocol details.
