# Crate API: vox-pm

## Overview

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

---

### `struct PackageCache`

Manages a central, cross-project cache for downloaded packages.
Enables UV-style fast installations by hard-linking files
from the cache into project-local `.vox_modules` directories.


### `struct ScoringConfig`

Configuration for response scoring.


### `fn content_hash`

Compute a SHA3-512 hash of the given data, returning Base32Hex-encoded string.


## Module: `vox-pm\src\lib.rs`

# vox-pm

Vox Package Manager — a content-addressable store (CAS) backend for
packages, artifacts, and agent memories. Built on Turso for
local-first development with optional cloud sync.


### `struct Lockfile`

Represents the `vox.lock` lockfile.
Records exact resolved versions and content hashes for reproducible installs.


### `struct VoxManifest`

Represents the full `Vox.toml` manifest.


### `enum DependencySpec`

A dependency specification — either a simple version string or a detailed table.


### `struct Namespace`

Namespace management for the content-addressed store.


### `fn normalize_and_hash`

Normalize an AST node for content-addressing.
Strips names (replaces with de Bruijn indices) and ignores whitespace/comments.


### `enum PackageKind`

Enumerates all artifact kinds that VoxPM can manage as packages.
This is the core differentiator: one PM for all Vox artifact types.


### `struct PopuliClient`

Client for communicating with local/remote Populi LLM services.
Now uses actual HTTP requests instead of hardcoded mock responses.


### `struct RegistryClient`

Client for the VoxPM package registry.
Handles search, download, publish, and info operations.


## Module: `vox-pm\src\registry_server.rs`

Server-side registry handlers.

These are standalone async functions that accept a `CodeStore` reference
and request data, returning JSON-serializable responses.  They can be
wired into any HTTP framework (Axum, Actix, Warp, etc.)  by the CLI crate.


### `struct SearchRequest`

Search request.


### `struct SearchResponse`

Search response.


### `struct PackageInfo`

Package info (returned by search and info endpoints).


### `struct PackageDetail`

Detailed package info (single-package endpoint).


### `struct PublishRequest`

Publish request.


### `struct PublishResponse`

Publish response.


### `struct DownloadResponse`

Download response.


### `struct YankRequest`

Yank request.


### `struct StatusResponse`

Generic status response.


### `fn handle_search`

GET /api/registry/search?query=...&limit=...


### `fn handle_info`

GET /api/registry/info/:name


### `fn handle_publish`

POST /api/registry/publish


### `fn handle_download`

GET /api/registry/download/:name/:version


### `fn handle_yank`

POST /api/registry/yank


### `fn handle_delete`

DELETE /api/registry/packages/:name/:version


### `struct SemVer`

A parsed semantic version: major.minor.patch with optional pre-release.


### `enum VersionReq`

A version requirement/range.


### `struct ResolvedDep`

A resolved dependency with its exact version.


### `struct AvailablePackage`

Package metadata for the registry/available packages.


### `struct Resolver`

The dependency resolver.


### `struct LogExecutionParams`

Parameters for logging an execution


### `enum StoreDb`

Content-addressed code store backed by TursoDB.
Inspired by Unison's codebase format and Convex's data model.
Supports remote (Turso Cloud) and embedded-replica modes.

When the `local` feature is enabled, local file-based
databases are also supported via `open()` and `open_memory()`.


### `struct CodeStore`

Content-addressed code store backed by TursoDB.
Inspired by Unison's codebase format and Convex's data model.
Supports remote (Turso Cloud) and embedded-replica modes.

When the `local` feature is enabled, local file-based
databases are also supported via `open()` and `open_memory()`.


### `struct ExecutionEntry`

An entry from the execution log.


### `struct ScheduledEntry`

A scheduled function entry.


### `struct ComponentEntry`

A component registry entry.


### `struct UserEntry`

A user account entry.


### `struct ArtifactEntry`

A shared artifact entry.


### `struct ReviewEntry`

An artifact review entry.


### `struct SnippetEntry`

A code snippet entry.


### `struct TrainingPair`

An LLM training data pair (interaction + feedback).


### `struct AgentDefEntry`

An agent definition entry.


### `struct MemoryEntry`

An agent memory entry.


### `struct SessionTurnEntry`

A session turn entry from the session_turns table.


### `struct SkillManifestEntry`

An installed skill manifest entry from the skill_manifests table (V12).


### `struct WorkflowDefEntry`

A workflow definition entry.


### `struct PackageSearchResult`

A package search result.


### `struct PackageBundle`

A self-contained package bundle for export/import.


### `struct UserPreferenceEntry`

A user preference entry.


### `struct BehaviorEventEntry`

A user behavior event entry.


### `struct SessionEntry`

A user session entry.


### `struct LearnedPatternEntry`

A learned pattern entry.


### `struct CommandUsageEntry`

Command usage frequency entry.


### `struct ModelUsageEntry`

Model usage statistics entry.


### `struct EmbeddingEntry`

An embedding entry.


### `struct KnowledgeNodeEntry`

A knowledge graph node entry.


### `struct KnowledgeEdgeEntry`

A knowledge graph edge entry.


### `struct EmbeddingModelEntry`

An embedding model entry.


### `struct ProceduralMemoryEntry`

A procedural memory entry.


### `struct ActionTemplateEntry`

An action template entry.


### `struct SkillProgressionEntry`

A skill progression entry.


### `struct BuilderSessionEntry`

A builder session entry.


### `struct TypedStreamEventEntry`

A typed stream event entry.


### `struct VoxWorkspace`

Represents a Vox workspace containing multiple packages.


