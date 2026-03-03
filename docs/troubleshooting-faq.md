# Troubleshooting FAQ — Vox ↔ AI Agents Integration

## Common Issues & Fixes

---

### ❌ MCP Connection Timeout (`vox-mcp`)
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

### ❌ `vox-lsp` Not Starting / LSP Crashes
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

### ❌ Port Conflict on Dashboard (`vox dashboard`)
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

### ❌ Shell Completions Not Working
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

### ⚠️ `vox_map_agent_session` Failing
**Cause**: The session ID is already mapped, or the agent doesn't exist.

**Fix**: Run `vox agent status` to see current session-to-agent mappings. If stale, restart the MCP server: `cargo run -p vox-mcp`.

---

### ⚠️ Workspace Compilation Errors After Update
**Cause**: A Vox AST or HIR struct gained a new required field (e.g., `filter_fields`).

**Fix**: Run `cargo check --workspace` and read the specific `E0063` missing field errors. These are structural changes to the Vox type system and require adding the new field at the construction site.

---

### 🧩 Agent Scoped to Wrong Files
**Cause**: The `scope:` line in `.vox/agents/<agent>.md` doesn't match the edited file's path.

**Fix**: Run `vox agent sync` to regenerate agents from the current crate graph, or manually edit `.vox/agents/<agent>.md` to update the `scope:` field.

---

### 📊 Dashboard Shows No Agents
**Cause**: The orchestrator has no active agents. Agents are only spawned when tasks are submitted.

**Fix**: Submit a task via an AI session or run `vox orchestrator spawn` to create a dev agent, then reload the dashboard.
