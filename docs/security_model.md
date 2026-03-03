# Vox Security Model

The Vox security model (`SecurityPolicy`, `SecurityGuard`, `AuditLog`) is defined in `vox-orchestrator` and provides multi-layer protection against prompt injection, scope violations, and unauthorized access.

## Threat Model

| Threat | Mitigation |
|--------|-----------|
| Prompt injection | `prompt_canonical::is_safe_prompt()` using injection pattern detection |
| Scope violations | `ChildSpec.scope[]` controls which files an agent may access |
| Token budget abuse | `BudgetManager` with per-agent cost limits and alerts |
| Unauthorized requests | API key or Bearer token validation in `vox-runtime::auth` |
| Replay attacks | Request IDs and timestamp validation |

## SecurityPolicy

```rust
pub struct SecurityPolicy {
    pub allow_shell_execution: bool,
    pub allow_network_access: bool,
    pub max_file_size_bytes: u64,
    pub blocked_paths: Vec<String>,
    pub require_human_in_loop: bool,
}
```

## SecurityGuard

Every MCP tool call passes through `SecurityGuard::evaluate()`:

1. Check for prompt injection patterns
2. Check scope constraints (if agent has a scope declaration)
3. Check rate limits (`RateLimiter`)
4. Log to `AuditLog`

## Injection Detection

The `submit_task` tool uses `is_safe_prompt()` from `vox-runtime::prompt_canonical`. If an injection is detected:

1. The task is rejected with a `422` status
2. An `AgentEventKind::InjectionDetected` event is emitted on the event bus
3. The rejection is logged to the audit log

### Detection Patterns

- `"Ignore previous instructions"`
- `"You are now"` context switching
- Shell metacharacters in description fields
- SQL-style injections in parameter values

## Agent Scope Enforcement

Agents declared in `.opencode/agents/{name}.md` can have a `scope:` field:

```markdown
---
scope: ["crates/vox-parser/**", "tests/**"]
---
```

Tasks that reference files outside the scope are rejected before being enqueued.

## Rate Limiting

Per-agent token rate limiting is configurable via `RateLimiter`:

```toml
[rate_limit]
max_requests_per_minute = 60
max_tokens_per_minute = 100000
```

## Audit Log

All rejected requests, scope violations, and injection attempts are appended to `logs/audit.jsonl`:

```json
{"timestamp": "...", "event": "InjectionDetected", "agent": "...", "description": "..."}
```
