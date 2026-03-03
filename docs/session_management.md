# Vox Session Management

Sessions allow agents to maintain persistent conversation history, metadata, and state across interactions.

## Architecture

Sessions are managed by `SessionManager` in `vox-runtime`, backed by JSONL files and optionally mirrored to VoxDB.

```
sessions/
  {session_id}.jsonl    ← conversation history (one JSON per line)
  {session_id}.meta     ← session metadata (JSON)
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `vox_session_create` | Create a new persistent session for an agent |
| `vox_session_list` | List all active sessions with state and token usage |
| `vox_session_reset` | Reset a session's conversation history (keeps metadata) |
| `vox_session_compact` | Replace a session's history with a summary string |
| `vox_session_info` | Get detailed info about a specific session |
| `vox_session_cleanup` | Tick lifecycle and remove archived sessions |

## Session Lifecycle

```
Created → Active → Compacted → Archived → Cleaned Up
                     ↑
               (auto-triggered when token budget exceeded)
```

## Usage

```json
// Create a session
{ "tool": "vox_session_create", "args": { "agent_id": "my-agent" } }

// List sessions
{ "tool": "vox_session_list" }

// Compact history
{ "tool": "vox_session_compact", "args": { "session_id": "...", "summary": "We fixed the parser bug." } }
```

## VoxDB sync

Sessions are dual-written to VoxDB's `agent_sessions` table, enabling:
- Cross-session search
- Usage analytics
- Session recovery after restart
