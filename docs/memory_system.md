# Vox Memory System

The Vox memory system provides persistent, searchable long-term storage for agent knowledge, structured around three layers.

## Architecture

```
┌─────────────────────────────────────────┐
│             VoxDB (SQLite)              │  ← Single source of truth
│   agent_memory, preferences, sessions  │
└─────────────────┬───────────────────────┘
                  │
        ┌─────────┴─────────┐
        ▼                   ▼
┌──────────────┐    ┌─────────────────┐
│  MemoryManager│    │ SessionManager  │
│  (daily logs) │    │ (conversations) │
└──────────────┘    └─────────────────┘
        │                   │
        ▼                   ▼
 MEMORY.md              sessions/*.jsonl
 logs/YYYY-MM-DD.md
```

## MCP Tools

| Tool | Description |
|------|-------------|
| `vox_memory_store` | Persist a key-value fact to long-term memory (MEMORY.md) |
| `vox_memory_recall` | Retrieve a fact from long-term memory by key |
| `vox_memory_search` | Hybrid BM25+vector search over daily logs and MEMORY.md |
| `vox_memory_log` | Append an entry to today's daily memory log |
| `vox_memory_list_keys` | List all section keys from MEMORY.md |
| `vox_knowledge_query` | Query the knowledge graph for related concepts |
| `vox_memory_save_db` | Persist a typed memory fact to VoxDb agent_memory table |
| `vox_memory_recall_db` | Recall typed memory facts from VoxDb |

## Usage

```rust
// From Rust
use vox_db::VoxDb;

let db = VoxDb::open("path/to/db.sqlite").await?;

// Store a memory
db.store_memory("user_preference", "Use tabs for indentation").await?;

// Recall it
let val = db.recall_memory("user_preference").await?;

// Search
let results = db.search_memories("indentation").await?;
```

## Compaction

When context gets large, use `vox_compaction_status` to check token budget.
The `CompactionEngine` supports three strategies:
- **Summarize** — condense history into a summary block
- **Drop Oldest** — drop oldest entries until under budget
- **Hybrid** — summarize, then drop if still over

## Persistence

- Facts stored via `vox_memory_store` go to `memory/MEMORY.md`
- Daily logs via `vox_memory_log` go to `memory/logs/YYYY-MM-DD.md`
- VoxDb entries go to the `agent_memory` table for structured queries
