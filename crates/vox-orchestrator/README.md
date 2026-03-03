# vox-orchestrator

Multi-agent file-affinity queue system. Routes tasks to AI agents based on file ownership, preventing race conditions when multiple agents work concurrently.

## Architecture

```
User Request
    │
    ▼
Orchestrator ──► FileAffinityMap ──► route to Agent
    │                                    │
    ▼                                    ▼
BulletinBoard ◄──── AgentQueue ──► FileLockManager
```

## Key Modules

| Module | Purpose |
|--------|---------|
| `orchestrator.rs` | Core orchestrator — task routing and lifecycle |
| `affinity.rs` | File-to-agent affinity mapping |
| `queue.rs` | Per-agent task queue with priority ordering |
| `locks.rs` | File-level lock manager (one writer per file) |
| `bulletin.rs` | Bulletin board for inter-agent coordination |
| `groups.rs` | Agent grouping and capability matching |
| `state.rs` | Orchestrator state persistence |
| `types.rs` | `AgentId`, `TaskId`, `TaskStatus`, etc. |
| `config.rs` | `OrchestratorConfig` |

## Feature Flags

| Feature | Description |
|---------|-------------|
| `runtime` | Actor-based agents via `vox-runtime` |
| `toestub-gate` | Post-task quality validation via TOESTUB |
| `lsp` | LSP integration for file ownership info |

## CLI

```bash
vox orchestrator enqueue --file src/main.vox --task "fix bug"
vox orchestrator status
```
