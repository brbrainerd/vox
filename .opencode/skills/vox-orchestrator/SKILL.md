---
name: vox-orchestrator
description: Multi-agent orchestration system for Vox — task queues, file locks, affinity groups, bulletin board, and available MCP tools
---

## MCP Tools

| Tool | Description |
|------|-------------|
| `vox_submit_task` | Submit a task with file affinity and priority |
| `vox_task_status` | Query task status by ID |
| `vox_orchestrator_status` | Full system status with agent info |
| `vox_complete_task` | Mark task as completed, release locks |
| `vox_fail_task` | Mark task as failed with reason |
| `vox_check_file_owner` | File affinity lookup |
| `vox_collect_diagnostics` | LSP diagnostics for a file |
| `vox_validate_file` | Run vox-lsp validation on a file |
| `vox_run_tests` | Run cargo tests for a crate |
| `vox_publish_message` | Broadcast to bulletin board |

## Architecture

```
Orchestrator
├── FileAffinityMap     — routes files to agents
├── AgentQueue[]        — per-agent priority queues
├── FileLockManager     — read/write file locks with escalation
├── BulletinBoard       — inter-agent pub/sub messaging
├── ContextStore        — shared key-value context with TTL
└── AffinityGroups      — glob-based file groupings (lexer-group, parser-group, etc.)
```

## Key Concepts

- **File Affinity**: Tasks are routed to agents based on which files they touch
- **Lock Escalation**: Read locks can be upgraded to write locks
- **Work Stealing**: Overloaded agent queues rebalance to underloaded agents
- **Toestub Gate**: Post-task validation via LSP diagnostics with auto-debug retry
