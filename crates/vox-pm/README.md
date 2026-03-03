# vox-pm

Vox Package Manager — a content-addressable store (CAS) backend for Vox packages, artifacts, and agent memories. Built on **Turso** for local-first with optional cloud sync.

## Architecture

### Content-Addressable Store (CAS)

All artifacts are stored by their SHA3-256 content hash:

```
store(data) → hash
get(hash) → data
```

- **Deterministic**: Same content always produces the same hash
- **Deduplication**: Identical artifacts share a single stored copy
- **Integrity**: Content can be verified against its hash at any time

### Name Binding

Names are mapped to content hashes within namespaces:

```
bind_name(namespace, name, hash)
lookup_name(namespace, name) → hash
```

This enables versioned references (e.g., `my_package@1.2.3`) while keeping the underlying storage content-addressed.

### Database Backends

The `CodeStore` supports multiple connection modes via feature flags:

| Mode | Feature Flag | Use Case |
|------|-------------|----------|
| Remote | (default) | Production — connects to Turso cloud |
| Local | `local` | Development — local Turso file |
| Memory | `local` | Testing — ephemeral in-memory DB |
| Embedded Replica | `replication` | Hybrid — local cache with cloud sync |

### Schema Management

Migrations are managed incrementally via a `schema_version` table:

- `migrate_schema()` — applies pending migrations
- `dry_run_migration()` — reports what would change without applying
- `health_check()` — runs `PRAGMA integrity_check` to verify DB health

### Artifact Normalization

`normalize.rs` implements semantic hashing using de Bruijn indexing:

- Strips identifier names from AST nodes
- Replaces bound variables with positional indices
- Enables detection of semantically identical code regardless of naming

## Key APIs

| Method | Description |
|--------|-------------|
| `store(data)` | Store bytes, returns content hash |
| `get(hash)` | Retrieve bytes by hash |
| `batch_insert(items)` | Bulk insert artifacts in one transaction |
| `bind_name(ns, name, hash)` | Associate a name with a hash |
| `list_components(limit, offset)` | Paginated artifact listing |
| `recall_memory(agent, type, limit, min_importance)` | Query agent memories with relevance filtering |
| `search_code_snippets(query, limit)` | Vector-similarity search over code |
