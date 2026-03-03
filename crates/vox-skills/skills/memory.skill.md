---
id = "vox.memory"
name = "Vox Memory"
version = "0.1.0"
author = "vox-team"
description = "Persistent agent memory — store and recall facts, search logs, manage sessions."
category = "database"
tools = ["vox_memory_store", "vox_memory_recall", "vox_memory_search", "vox_memory_log", "vox_memory_list_keys", "vox_knowledge_query", "vox_session_create", "vox_session_list", "vox_session_info", "vox_session_compact", "vox_session_cleanup"]
tags = ["memory", "recall", "session", "knowledge", "facts"]
permissions = ["db_read", "db_write"]
---

# Vox Memory Skill

Provides the full persistent memory system: facts, daily logs, knowledge graph, and session management.

## Tools

- `vox_memory_store` — persist a key-value fact with optional related concept edges
- `vox_memory_recall` — retrieve a fact by key
- `vox_memory_search` — search logs and MEMORY.md by keyword
- `vox_memory_log` — append to today's daily log
- `vox_knowledge_query` — traverse the knowledge graph for related concepts
- `vox_session_*` — create, list, compact, and clean up agent sessions

## Usage

Always call `vox_memory_store` when you learn an important fact about the project.
Use `vox_knowledge_query` to find contextually related information before starting a new task.
Call `vox_session_compact` before context window overflow to preserve session history.
