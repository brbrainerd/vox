# Example: Vox MCP Tool Example

```vox
# Vox MCP Tool Example
# Demonstrates the Model Context Protocol (MCP) integration.
#
# @mcp.tool decorators expose Vox functions as MCP-compatible tools
# that AI assistants (Claude, GPT, etc.) can discover and invoke.
# @mcp.resource decorators expose read-only data sources.
#
# MCP is a standardized protocol for AI tool use:
# https://modelcontextprotocol.io

# A simple database table for storing notes
@table type Note:
    title: str
    content: str
    created_at: str

# ----- MCP Tools -----

# Expose a tool that creates a new note.
# The AI assistant will see this tool's name, description, and parameter types.
@mcp.tool("create_note", "Create a new note with a title and content")
fn create_note(title: str, content: str) to str:
    ret "Created note: " + title

# Expose a tool that searches notes by keyword.
@mcp.tool("search_notes", "Search for notes containing a keyword")
fn search_notes(keyword: str, max_results: int) to list[str]:
    ret ["Note 1: " + keyword, "Note 2: " + keyword]

# Expose a tool for summarizing content.
@mcp.tool("summarize", "Summarize the given text into a short paragraph")
fn summarize(text: str) to str:
    ret "Summary of: " + text

# ----- MCP Resources -----

# Expose recent notes as a read-only resource.
# AI assistants can read this to get context about existing notes.
@mcp.resource("notes://recent", "List of recently created notes")
fn recent_notes() to list[str]:
    ret ["Recent note 1", "Recent note 2"]
```
