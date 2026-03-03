# Plan Handoff Protocol

When an agent completes a phase of work and needs to hand off context to another agent
or a future session, it uses the structured Handoff Payload format.

## Why Handoffs Exist

Context windows are finite. Agents accumulate work across sessions. The handoff protocol
ensures no critical state is lost between agent lifecycles.

## Handoff Payload Format

```json
{
  "from_agent": "build-agent-1",
  "to_agent": "debugger-agent-1",
  "plan_summary": "Fixed compilation errors in vox-tensor. Now need to resolve remaining warnings in vox-cli.",
  "completed_tasks": [
    "DOC-01: Created docs/agents/ modular files",
    "ORCH-03: Renamed vox_map_opencode_session → vox_map_vscode_session"
  ],
  "pending_tasks": [
    "ORCH-01: Decompose orchestrator.rs into sub-modules",
    "CFG-01: Define VoxConfig struct in vox-config"
  ],
  "owned_files": [
    "crates/vox-tensor/src/optim.rs",
    "crates/vox-cli/src/training/native.rs"
  ],
  "context_notes": "VoxBackend type alias uses conditional compilation: wgpu when 'gpu' feature enabled, ndarray otherwise. The Burn 0.19 Optimizer trait must be in scope for .step() to resolve."
}
```

## MCP Tools

```
vox_agent_handoff   → Store a handoff payload (sender side)
vox_handoff_context → Retrieve a handoff payload (receiver side)
```

## Triggering a Handoff

From CLI or VS Code command:
```
vox orchestrator handoff --to debugger-agent-1 --plan "..."
```

## Agent Subagent Guidelines

Agents running in the VS Code extension environment should proactively dispatch
specialized subagents for distinct task types:

- **`@debugger`**: For tracking down failing cargo tests or resolving LSP errors
  from `vox_validate_file`.
- **`@researcher`**: For answering design choices requiring post-2024 knowledge
  via web search.
- **`@reviewer`**: For verifying code against the Vox quality rules (no `.unwrap()`
  in production, no God objects, etc.) before marking work complete.

Always create a handoff payload before terminating a context window that has pending work.

## Auto-Continuation

Idle agents with pending work will loop independently. The VS Code extension's native
`doom_loop` detection limits excessive cyclic failures in the debugger role.
