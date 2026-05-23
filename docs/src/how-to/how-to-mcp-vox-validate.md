---
title: "Point your AI coding assistant at the Vox MCP validator"
description: "Wire Claude Desktop, Cursor, or any MCP client to vox_validate_source / vox_validate_file so AI-generated Vox code iterates against the real compiler with structured autofix suggestions, not against guesses."
category: "How-To Guides"
status: "current"
last_updated: "2026-05-03"
training_eligible: true
---

# Point your AI coding assistant at the Vox MCP validator

The Vox orchestrator exposes three MCP tools that let an AI coding assistant validate and inspect `.vox` source through the real compiler pipeline:

| Tool | Input | Returns |
|---|---|---|
| [`vox_validate_source`](#vox_validate_source) | `{ source: string, virtual_path?: string }` | LSP-style diagnostics with stable codes + structured autofix suggestions. **Pure text-in/text-out — no filesystem read.** |
| [`vox_validate_file`](#vox_validate_file) | `{ path: string }` | Same diagnostic shape; reads the file from disk. |
| `vox_compiler::ast_inspect` | `{ path: string }` | The parsed AST as a JSON tree. |

The first one is the iteration-loop primitive: an assistant proposes Vox source, calls `vox_validate_source`, receives structured diagnostics with `code` and `fixes` fields, applies a fix, re-validates — without writing intermediate files.

Implementation: [crates/vox-orchestrator/src/mcp_tools/code_validator.rs](../../../crates/vox-orchestrator/src/mcp_tools/code_validator.rs). Registry entry in [contracts/mcp/tool-registry.canonical.yaml](../../../contracts/mcp/tool-registry.canonical.yaml). Tools are surfaced through the stdio MCP server at [crates/vox-cli/src/commands/mcp_server/](../../../crates/vox-cli/src/commands/mcp_server/).

## Prerequisites

- `vox` built with the orchestrator and MCP server enabled (the default workspace build).
- An MCP-aware AI client (Claude Desktop, Cursor, Continue, any other MCP host).
- The repo path you want the assistant to validate against.

## Wiring up Claude Desktop

Edit `claude_desktop_config.json` (location: `~/Library/Application Support/Claude/claude_desktop_config.json` on macOS; `%APPDATA%\Claude\claude_desktop_config.json` on Windows).

```json
{
  "mcpServers": {
    "vox": {
      "command": "vox",
      "args": ["mcp-server"],
      "cwd": "/absolute/path/to/your/vox/repo"
    }
  }
}
```

Restart Claude Desktop. The tool list will show `vox_validate_source`, `vox_validate_file`, `vox_compiler::ast_inspect` and the broader Vox tool registry. Ask Claude to "validate this Vox source" and it will call `vox_validate_source` directly with your snippet.

## Wiring up Cursor

In Cursor's MCP settings (Settings → Features → MCP), add:

```json
{
  "mcpServers": {
    "vox": {
      "command": "vox",
      "args": ["mcp-server"]
    }
  }
}
```

Cursor will discover the tool registry on first connection. The Composer agent will pick `vox_validate_source` automatically when proposing Vox edits.

## The diagnostic shape (what the assistant gets back)

```json
{
  "success": true,
  "data": {
    "count": 1,
    "diagnostics": [
      {
        "severity": "error",
        "message": "<img> requires an `alt` attribute for accessibility",
        "source": "vox-lsp",
        "start_line": 5,
        "start_col": 8,
        "end_line": 5,
        "end_col": 24,
        "code": "web_ir_validate.a11y.img.missing_alt",
        "fixes": [
          {
            "label": "Add empty alt attribute",
            "replacement": "<img src=\"...\" alt=\"\">",
            "start_line": 5,
            "start_col": 8,
            "end_line": 5,
            "end_col": 24
          }
        ]
      }
    ],
    "hir_validation_included": true
  }
}
```

The two fields AI clients should always read:

- **`code`** — the stable diagnostic identifier (e.g., `web_ir_validate.a11y.img.missing_alt`, `web_ir_validate.route.unreachable`, `routes.overlap.unresolvable_precedence`, `dep_inference.over_track`). Use it to classify and filter — don't pattern-match the human-readable `message`.
- **`fixes`** — an array of `{ label, replacement, range }` suggestions. Apply the `replacement` text to lines `start_line..end_line` (0-based) and re-validate. Many diagnostics have at least one fix; some have several alternative fixes the assistant can pick between based on context.

## Worked example: the iterate-against-the-compiler loop

This is the loop that makes `vox_validate_source` pull its weight.

**Step 1 — Assistant proposes a Vox component:**

```vox
// vox:skip — illustrative; deliberately buggy
component Avatar(url: str) {
  view: image(src=url)
}
```

**Step 2 — Assistant calls `vox_validate_source` with the proposed source.** It receives back the diagnostic above (`web_ir_validate.a11y.img.missing_alt`) plus the autofix suggestion to add an `alt` argument.

**Step 3 — Assistant applies the fix and re-validates:**

```vox
component Avatar(url: str, alt: str) {
  view: image(src=url, alt=alt)
}
```

**Step 4 — `vox_validate_source` returns `{ count: 0, diagnostics: [] }`.** The assistant ships the corrected component instead of guessing.

This loop is fast (the validator is in-process, not a subprocess) and deterministic (no flakiness from timing or env). It tightens the AI feedback loop from "write code → maybe-test → maybe-discover-bug-later" to "write code → know in milliseconds whether the compiler accepts it."

## Diagnostic codes worth knowing

The validator emits codes from several namespaces. The most useful for AI-generated GUI code:

| Namespace | Examples | Triggered when |
|---|---|---|
| `web_ir_validate.a11y.*` | `img.missing_alt`, `interactive_missing_label`, `keyboard_handler_required` | accessibility constraints violated |
| `web_ir_validate.route.*` | `unreachable`, `broken_link`, `missing_component` | route declarations don't compose with the rest of the module |
| `routes.overlap.*` | `unresolvable_precedence`, `shadowed` | two routes match the same concrete URL |
| `dep_inference.over_track` | (single code) | a `derived` / `effect` calls a non-`@reactive` function whose body might read reactive state the analyzer can't see |
| `web_ir_validate.island.*` | `prop_key_empty`, `unknown_island` | `@island` boundary errors |

Full list and severity rules: [crates/vox-codegen/src/web_ir/validate.rs](../../../crates/vox-codegen/src/web_ir/validate.rs).

## When to use which tool

- **`vox_validate_source`** — first choice. AI-iteration loop, snippet validation, REPL-style "is this valid?" queries.
- **`vox_validate_file`** — when the source must be on disk (e.g., editor save, pre-commit hook, CI replay). Same diagnostic shape, different input contract.
- **`vox_compiler::ast_inspect`** — when the assistant needs to reason about declared shapes (component prop interfaces, route trees, type aliases) without re-deriving them from text. Returns the parsed AST as JSON; combine with `vox_validate_source` for a full inspect-then-edit-then-validate loop.

## Troubleshooting

- **No tools appear in the client.** Check that `vox mcp-server` runs successfully from a terminal in the same `cwd` you configured. The stdio server speaks JSON-RPC over stdin/stdout — no port; make sure no shell wrappers are trimming output.
- **Diagnostics arrive but `fixes` is always empty.** The fixes are populated by `vox_lsp::typeck_diagnostic_to_lsp` ([crates/vox-lsp/src/lib.rs](../../../crates/vox-lsp/src/lib.rs)) only for diagnostics that have `FixSuggestion`s attached at the typeck layer. Many a11y / route / web_ir diagnostics already have fixes; some HIR-invariant diagnostics do not. If your particular bug class has no autofix, that's a gap worth filing.
- **The assistant keeps proposing source that fails the same diagnostic.** Two common causes: (a) the assistant isn't reading the `code` field — make sure your prompt asks it to "use the diagnostic code, not the message text"; (b) the assistant doesn't have the `fixes` array — check that your client surfaces the full tool response.

## Related

- [Svelte vs React Frameworks Research (2026)](../architecture/svelte-vs-react-frameworks-research-2026.md) — comparative analysis that informed the design of this surface; explains why "the compiler is the source of truth for AI-written code" is the highest-leverage AI-codegen primitive.
- [Svelte-Mineable Features Implementation Plan §Phase A](../architecture/svelte-mineable-features-implementation-plan-2026.md) — the plan that scoped this tool surface.
- [`docs/src/.well-known/llms.txt`](../.well-known/llms.txt) — agent-discovery surface; this how-to is reachable from there.
- [contracts/mcp/tool-registry.canonical.yaml](../../../contracts/mcp/tool-registry.canonical.yaml) — full tool registry the stdio MCP server exposes.
