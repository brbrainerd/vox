---
title: "Troubleshooting FAQ"
description: "Operational troubleshooting for Vox CLI, MCP, LSP, dashboard, and contributor environment issues."
category: "How-To Guides"
status: "current"
last_updated: "2026-03-28"
training_eligible: true

schema_type: "HowTo"
---

# Troubleshooting FAQ — Vox ↔ AI Agents Integration

This page is for operational fixes.

If you want product or architecture answers, use the main [Vox FAQ](../explanation/faq.md).

## Common Issues & Fixes

---

### `vox-mcp` connection timeout
**Cause**: The `vox-mcp` binary is missing or not in the expected path. The AI Agent reads the binary path from `vox-agent.json`.

**Fix**:
```bash
# Build the binary
cargo build -p vox-mcp

# Check it exists
ls target/debug/vox-mcp*

# Re-run doctor
vox agent doctor
```

If you're using a release build, make sure `vox-agent.json` points to `target/release/vox-mcp`.

---

### `vox-lsp` not starting or LSP crashes
**Cause**: The LSP binary is not built, or it panics on startup with an invalid project.

**Fix**:
```bash
# Build the LSP binary
cargo build -p vox-lsp

# Run it manually to see errors
target/debug/vox-lsp --stdio 2>&1 | head -20
```

Check `target/debug/vox-lsp.stderr.log` if it exists.

---

### Port conflict on `vox dashboard`
**Cause**: Port `8080` (default) is already in use.

**Fix**:
```bash
# Check what's using the port
netstat -ano | findstr :8080

# Kill the process by PID (Windows)
taskkill /PID <PID> /F

# Or launch on a different port
VOX_DASHBOARD_PORT=8090 vox dashboard
```

---

### Shell completions not working
**Fix**: Generate and source completions for your shell:

```bash
# Bash
vox completions bash > ~/.local/share/bash-completion/completions/vox

# Zsh
vox completions zsh > ~/.zfunc/_vox

# PowerShell
vox completions powershell >> $PROFILE
```

---

### `vox_map_agent_session` failing
**Cause**: The session ID is already mapped, or the agent doesn't exist.

**Fix**: Run `vox agent status` to see current session-to-agent mappings. If stale, restart the MCP server: `cargo run -p vox-mcp`.

---

### Workspace compilation errors after update
**Cause**: A Vox AST or HIR struct gained a new required field (e.g., `filter_fields`).

**Fix**: Run `cargo check --workspace` and read the specific `E0063` missing field errors. These are structural changes to the Vox type system and require adding the new field at the construction site.

---

### Agent scoped to the wrong files
**Cause**: The `scope:` line in `.vox/agents/<agent>.md` doesn't match the edited file's path.

**Fix** { Run `vox agent sync` to regenerate agents from the current crate graph, or manually edit `.vox/agents/<agent>.md` to update the `scope:` field.

---

### Dashboard shows no agents
**Cause**: The orchestrator has no active agents. Agents are only spawned when tasks are submitted.

**Fix**: Submit a task via an AI session or run `vox orchestrator spawn` to create a dev agent, then reload the dashboard.
---

## Compiler Diagnostics & Error Codes

The Vox compiler provides structured diagnostic codes to help you (and AI agents) fix code rapidly.

### `E0001`: Argument count mismatch
**Message**: `Argument count mismatch: expected X arguments, found Y`
**Cause**: You called a function with the wrong number of parameters.
**Fix**: Match the function signature. If you want optional arguments, use `Option[T]`.

### `E0002`: Tuple size mismatch
**Message**: `Tuple size mismatch: expected X, found Y`
**Cause**: Attempting to destructure or assign a tuple of different lengths.

### `E0003`: Function arity mismatch
**Message**: `Function arity mismatch: expected X, found Y`
**Cause**: Occurs during higher-order function passing where the callback signature doesn't match the expected parameter count.

### `E0063`: Missing record fields
**Message**: `Missing record fields: [field_name]`
**Cause**: You instantiated a struct or table without providing all required non-Option fields.
**Fix**: Provide the missing fields or update the type definition to use `Option[T]`.

### `E0101`: Immutable assignment
**Message**: `Cannot assign to immutable variable X`
**Cause**: Attempting to mutate a variable not declared with `mut`.
**Fix**: Change `let x = ...` to `let mut x = ...`.

### `E0404`: Module search failure
**Message**: `Failed to resolve module X`
**Cause**: The imported file or crate is missing from the search path.
**Fix**: Check your `import` paths and ensure the dependency is in your project or listed in `vox.lock`.

---

## Further Operations
- [Vox FAQ](../explanation/faq.md) — Architectural and conceptual Q&A.
- [`vox doctor`](../reference/cli.md#vox-doctor) — Automates environment verification.
- [Contributor Hub](../contributors/contributor-hub.md) — If you've found a compiler bug.


