---
title: "VoxKB Foundation — Backend Implementation Plan"
description: "Implements named, topic-scoped Knowledge Bases in VoxDb with a hybrid routing engine, six signal adapters, RAG integration, and MENS flywheel."
category: "implementation"
status: "current"
---

# VoxKB Foundation — Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add named topic-scoped Knowledge Bases (KBs) to Vox: VoxDb schema, CRUD backend, three-tier routing engine (keyword rules → Jaccard similarity → LLM), six signal adapters, KB-enriched retrieval bundle, `@kb-name` mention injection, and a MENS training flywheel that uses accepted KB entries as SFT training signals and rejected entries as DPO negative examples.

**Architecture:** A new `knowledge_base/` module in `vox-orchestrator` owns KB types and VoxDb CRUD; a `KbRouter` handles three-tier routing (keyword rules first, then word-set Jaccard similarity for unmatched items, LLM reserved for high-value synthesis reports). Signal adapters in `vox-orchestrator-mcp` fire on chat turns, research pipeline completion, Scientia finding promotion, and explicit user clips. The retrieval bundle (`RetrievalBundle`) gains a `kb_lines` field so KB hits flow into every chat context automatically. The MENS pipeline gains an async `KbSignals` stage that exports accepted entries as (instruction, completion) SFT pairs and rejected entries as DPO pairs.

**SOTA alignment:** Follows the 2026 three-layer memory model (Working/Episodic/Semantic); KB is the Semantic layer. See [`docs/src/architecture/kb-systems-sota-research-2026.md`](../../../docs/src/architecture/kb-systems-sota-research-2026.md) for the full research backing.

**Tech Stack:** Rust stable, Turso/SQLite via `VoxDb`, `vox-search` BM25 + RRF, `vox_actor_runtime::llm` (facade), `uuid` for ID generation, `cargo test -p <crate>` for tests. No new crate dependencies.

---

## Background — Files You Will Touch

Read these before writing any code. They establish the patterns everything else must follow.

| File | Role |
|------|------|
| `crates/vox-db/src/schema/domains/knowledge.rs` | VoxDb knowledge schema — add new tables here |
| `crates/vox-db/src/schema/domains/mod.rs` | Registers schema domain modules |
| `crates/vox-db/src/schema/manifest.rs` | Orders schema fragments; holds `BASELINE_VERSION = 76` (bump to 77) |
| `contracts/db/baseline-version-policy.yaml` | SSOT for baseline integer + digest; update after schema changes |
| `crates/vox-db/src/store/ops_memory.rs` | Single `impl crate::VoxDb {` block (lines 17–832); new KB ops go in a **new** `impl crate::VoxDb { }` block appended after line 832 |
| `crates/vox-db/src/lib.rs` | VoxDb public surface — new `KbRow`, `KbEntryRow`, `KbRuleRow` structs go here |
| `crates/vox-orchestrator/src/lib.rs` | Re-exports `now_unix_ms()` (already exported from `types::now_unix_ms`) |
| `crates/vox-orchestrator-mcp/src/memory_tools/params.rs` | Pattern for `#[derive(Debug, Deserialize, JsonSchema)]` params |
| `crates/vox-orchestrator-mcp/src/memory_tools/handlers_memory.rs` | Pattern for MCP handlers that call `state.db` |
| `crates/vox-orchestrator-mcp/src/dispatch.rs` | Single match arm per tool name → handler |
| `crates/vox-orchestrator-mcp/src/input_schemas.rs` | JSON Schema for each tool (either `derived_tool_schema!` or `parse_obj`) |
| `crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs` | `RetrievalBundle` struct (derives `Debug, Clone, Default` — **no serde**) + `run_retrieval_bundle()` |
| `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs` | Chat preamble injection — hook KB signal adapter here. `response_text: String` (line 312), `session_id: String` (line 263) |
| `crates/vox-ml-cli/src/commands/mens/populi/action_prelude.rs` | `PipelineStage` enum (lines 102–131) and `as_str()` — **`KbSignals` goes here** |
| `crates/vox-ml-cli/src/commands/mens/pipeline.rs` | Async `run()` — add `KbSignals` match arm and async `run_kb_signals_stage()` here |
| `crates/vox-orchestrator-mcp/src/lib.rs` | Module declarations (`pub mod`, `pub use`) |

**⚠️ Key codebase facts that differ from intuition:**

1. `uuid = { workspace = true, features = ["v4"] }` is already in `vox-orchestrator/Cargo.toml` — use `uuid::Uuid::new_v4().to_string()` for IDs. **Never use `fastrand` — it is not a dependency.**
2. `vox_orchestrator::now_unix_ms()` is already exported from the crate — use it instead of reimplementing.
3. `ops_memory.rs` has ONE `impl crate::VoxDb {` block that opens at line 17 and never closes (runs to EOF at line 832). Append KB ops as a **new separate** `impl crate::VoxDb { }` block after the file ends.
4. `PipelineStage` enum is in `populi/action_prelude.rs`, NOT `pipeline.rs`. `pipeline.rs` only imports it.
5. `pipeline::run()` is `async fn`. Never spawn a new `tokio::runtime::Runtime` inside it — that panics. Make helper functions `async fn` and `.await` them directly.
6. `RetrievalBundle` derives `Debug, Clone, Default` — it does **NOT** derive `Serialize/Deserialize`. Do not add `#[serde(default)]` to its fields.
7. The Mix stage reads `mix.yaml` — there is no hardcoded file list in `pipeline.rs` to append to.
8. `VoxDb::open()` exists at `store/open.rs:198` but is `#[cfg(feature = "local")]` gated. Verify `vox-ml-cli/Cargo.toml` enables the `local` feature on `vox-db` before using it.
9. `state.db` in `ServerState` is `Option<Arc<vox_db::VoxDb>>`. `state.db.clone()` gives `Option<Arc<VoxDb>>`.

**New files you will create:**

| File | Role |
|------|------|
| `crates/vox-orchestrator/src/knowledge_base/mod.rs` | Module root, re-exports |
| `crates/vox-orchestrator/src/knowledge_base/types.rs` | `KnowledgeBase`, `KbEntry`, `KbEntrySource`, `KbRoutingRule`, `KbRoutingRuleType` |
| `crates/vox-orchestrator/src/knowledge_base/store.rs` | `KbStore` — async VoxDb CRUD |
| `crates/vox-orchestrator/src/knowledge_base/router.rs` | `KbRouter` — three-tier routing |
| `crates/vox-orchestrator-mcp/src/kb_tools/mod.rs` | Module root, re-exports |
| `crates/vox-orchestrator-mcp/src/kb_tools/params.rs` | MCP param structs for all KB tools |
| `crates/vox-orchestrator-mcp/src/kb_tools/handlers.rs` | All MCP handler functions |
| `crates/vox-orchestrator-mcp/src/kb_tools/signal_chat.rs` | Chat turn signal adapter |
| `crates/vox-orchestrator-mcp/src/kb_tools/signal_research.rs` | Research completion signal adapter |
| `crates/vox-orchestrator-mcp/src/kb_tools/signal_scientia.rs` | Scientia finding promotion adapter |

---

## Task 1: VoxDb Schema — New KB Tables

**Files:**
- Modify: `crates/vox-db/src/schema/domains/knowledge.rs`
- Modify: `crates/vox-db/src/schema/manifest.rs`
- Modify: `contracts/db/baseline-version-policy.yaml`

This task adds three new SQLite tables: `knowledge_bases`, `kb_entries`, and `kb_routing_rules`. Because all VoxDb DDL uses `CREATE TABLE IF NOT EXISTS`, you are **not** breaking any existing data.

- [ ] **Step 1.1: Run the baseline digest test to confirm it currently passes**

```powershell
cargo test -p vox-db baseline_policy_matches_compiled_schema
```

Expected: `test result: ok. 1 passed`

- [ ] **Step 1.2: Append KB table DDL to `crates/vox-db/src/schema/domains/knowledge.rs`**

Open the file. Find the closing `\n";` string terminator of the Rust const (the last two characters of the string). Add the following SQL **before** that closing `";`:

```sql
-- Knowledge Base tables (VoxKB) ---------------------------------------------------
CREATE TABLE IF NOT EXISTS knowledge_bases (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    description TEXT NOT NULL DEFAULT '',
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL,
    entry_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kb_entries (
    id TEXT PRIMARY KEY,
    kb_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    content TEXT NOT NULL,
    source_signal TEXT NOT NULL,
    source_ref TEXT,
    routing_confidence REAL NOT NULL DEFAULT 1.0,
    tags TEXT NOT NULL DEFAULT '[]',
    created_at_ms INTEGER NOT NULL,
    last_accessed_at_ms INTEGER,
    access_count INTEGER NOT NULL DEFAULT 0,
    accepted INTEGER NOT NULL DEFAULT 1,
    mens_queued INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kb_routing_rules (
    id TEXT PRIMARY KEY,
    kb_id TEXT NOT NULL REFERENCES knowledge_bases(id) ON DELETE CASCADE,
    rule_type TEXT NOT NULL,
    pattern TEXT NOT NULL,
    priority INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kb_entries_kb_id ON kb_entries(kb_id);
CREATE INDEX IF NOT EXISTS idx_kb_entries_source_signal ON kb_entries(source_signal);
CREATE INDEX IF NOT EXISTS idx_kb_entries_accepted ON kb_entries(accepted);
CREATE INDEX IF NOT EXISTS idx_kb_entries_mens_queued ON kb_entries(mens_queued, accepted);
CREATE INDEX IF NOT EXISTS idx_kb_routing_rules_kb_id ON kb_routing_rules(kb_id);
```

> **SOTA note:** `last_accessed_at_ms` and `access_count` columns are added now for future usage-based decay and staleness eviction (per the Mem0 "Decay" and "Evict" levers from the research). They are not yet used by the MVP but make the schema future-proof without a migration.

- [ ] **Step 1.3: Bump `BASELINE_VERSION` in `crates/vox-db/src/schema/manifest.rs`**

Find: `pub const BASELINE_VERSION: i64 = 76;`

Replace with:
```rust
// 77: feat(vox-kb): add knowledge_bases, kb_entries, kb_routing_rules tables
pub const BASELINE_VERSION: i64 = 77;
```

- [ ] **Step 1.4: Run the baseline digest test — expect failure with the new digest value**

```powershell
cargo test -p vox-db baseline_policy_matches_compiled_schema
```

Expected: **FAIL** — the test output will contain the new digest hex. Copy it.

- [ ] **Step 1.5: Update `contracts/db/baseline-version-policy.yaml`**

Replace both the integer and the digest:
```yaml
repository_baseline_integer: 77
repository_baseline_digest_hex: "0x<NEW_HEX_FROM_STEP_1.4>"
```

- [ ] **Step 1.6: Run the baseline digest test — expect pass**

```powershell
cargo test -p vox-db baseline_policy_matches_compiled_schema
```

Expected: `test result: ok. 1 passed`

- [ ] **Step 1.7: Commit**

```powershell
cargo fmt -p vox-db
git add crates/vox-db/src/schema/domains/knowledge.rs crates/vox-db/src/schema/manifest.rs contracts/db/baseline-version-policy.yaml
git commit -m "feat(vox-db): add knowledge_bases, kb_entries, kb_routing_rules tables (v77)"
```

---

## Task 2: KB Types — Shared Rust Structs

**Files:**
- Create: `crates/vox-orchestrator/src/knowledge_base/types.rs`
- Create: `crates/vox-orchestrator/src/knowledge_base/mod.rs` (plus stubs)
- Modify: `crates/vox-orchestrator/src/lib.rs`

- [ ] **Step 2.1: Write failing test first**

Create `crates/vox-orchestrator/src/knowledge_base/types.rs` with ONLY the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kb_entry_source_roundtrips_as_str() {
        assert_eq!(KbEntrySource::Chat.as_str(), "chat");
        assert_eq!(KbEntrySource::Research.as_str(), "research");
        assert_eq!(KbEntrySource::CodeActivity.as_str(), "code_activity");
        assert_eq!(KbEntrySource::Web.as_str(), "web");
        assert_eq!(KbEntrySource::Explicit.as_str(), "explicit");
        assert_eq!(KbEntrySource::Scientia.as_str(), "scientia");
    }

    #[test]
    fn kb_routing_rule_type_roundtrips() {
        assert_eq!(KbRoutingRuleType::Keyword.as_str(), "keyword");
        assert_eq!(KbRoutingRuleType::Regex.as_str(), "regex");
    }
}
```

- [ ] **Step 2.2: Run to confirm failure**

```powershell
cargo test -p vox-orchestrator knowledge_base
```

Expected: compile error — module `knowledge_base` not found.

- [ ] **Step 2.3: Implement `types.rs`**

Replace the file with the complete implementation:

```rust
//! Core types for VoxKB — knowledge bases, entries, routing rules.

use serde::{Deserialize, Serialize};

/// A named, topic-scoped knowledge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub entry_count: i64,
}

/// A single entry stored in a knowledge base.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KbEntry {
    pub id: String,
    pub kb_id: String,
    pub content: String,
    pub source_signal: String,
    pub source_ref: Option<String>,
    pub routing_confidence: f64,
    /// JSON array of tag strings, e.g. `["rust","async"]`.
    pub tags: String,
    pub created_at_ms: i64,
    pub last_accessed_at_ms: Option<i64>,
    pub access_count: i64,
    /// `1` = accepted into KB (SFT signal); `0` = rejected (DPO negative).
    pub accepted: i64,
    /// `1` = already queued for MENS training.
    pub mens_queued: i64,
}

/// Signal source that produced a KB entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KbEntrySource {
    Chat,
    Research,
    CodeActivity,
    Web,
    Explicit,
    Scientia,
}

impl KbEntrySource {
    pub fn as_str(self) -> &'static str {
        match self {
            KbEntrySource::Chat => "chat",
            KbEntrySource::Research => "research",
            KbEntrySource::CodeActivity => "code_activity",
            KbEntrySource::Web => "web",
            KbEntrySource::Explicit => "explicit",
            KbEntrySource::Scientia => "scientia",
        }
    }
}

/// Type of routing rule that classifies content into a KB.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KbRoutingRuleType {
    Keyword,
    Regex,
}

impl KbRoutingRuleType {
    pub fn as_str(self) -> &'static str {
        match self {
            KbRoutingRuleType::Keyword => "keyword",
            KbRoutingRuleType::Regex => "regex",
        }
    }
}

/// A rule that routes content into a specific KB.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KbRoutingRule {
    pub id: String,
    pub kb_id: String,
    pub rule_type: KbRoutingRuleType,
    pub pattern: String,
    /// Higher priority rules are checked first.
    pub priority: i64,
    pub created_at_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kb_entry_source_roundtrips_as_str() {
        assert_eq!(KbEntrySource::Chat.as_str(), "chat");
        assert_eq!(KbEntrySource::Research.as_str(), "research");
        assert_eq!(KbEntrySource::CodeActivity.as_str(), "code_activity");
        assert_eq!(KbEntrySource::Web.as_str(), "web");
        assert_eq!(KbEntrySource::Explicit.as_str(), "explicit");
        assert_eq!(KbEntrySource::Scientia.as_str(), "scientia");
    }

    #[test]
    fn kb_routing_rule_type_roundtrips() {
        assert_eq!(KbRoutingRuleType::Keyword.as_str(), "keyword");
        assert_eq!(KbRoutingRuleType::Regex.as_str(), "regex");
    }
}
```

- [ ] **Step 2.4: Create stub files and module root**

Create `crates/vox-orchestrator/src/knowledge_base/store.rs`:
```rust
//! VoxDb CRUD for knowledge bases and entries.
// Implementation in Task 3.
```

Create `crates/vox-orchestrator/src/knowledge_base/router.rs`:
```rust
//! Three-tier routing: keyword rules → Jaccard word similarity → LLM fallback.
// Implementation in Task 4.
```

Create `crates/vox-orchestrator/src/knowledge_base/mod.rs`:
```rust
//! VoxKB — named topic-scoped knowledge bases.
pub mod router;
pub mod store;
pub mod types;

pub use types::{KbEntry, KbEntrySource, KbRoutingRule, KbRoutingRuleType, KnowledgeBase};
```

- [ ] **Step 2.5: Register in `crates/vox-orchestrator/src/lib.rs`**

Find the block where other modules are declared (search for `pub mod memory;`). Add:
```rust
pub mod knowledge_base;
pub use knowledge_base::{KbEntry, KbEntrySource, KbRoutingRule, KbRoutingRuleType, KnowledgeBase};
```

- [ ] **Step 2.6: Run tests — expect pass**

```powershell
cargo test -p vox-orchestrator knowledge_base
```

Expected: `test result: ok. 2 passed`

- [ ] **Step 2.7: Commit**

```powershell
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/knowledge_base/ crates/vox-orchestrator/src/lib.rs
git commit -m "feat(vox-orchestrator): add knowledge_base module with core types"
```

---

## Task 3: VoxDb Row Types + KB Ops

**Files:**
- Modify: `crates/vox-db/src/lib.rs` ← **do this FIRST**
- Modify: `crates/vox-db/src/store/ops_memory.rs` ← **do this SECOND**

> ⚠️ **Critical ordering:** `ops_memory.rs` references `crate::KbRow`, `crate::KbEntryRow`, `crate::KbRuleRow`. These must exist in `lib.rs` **before** `ops_memory.rs` is compiled, or you will get unresolved type errors. Do Step 3.1 (lib.rs changes) before Step 3.2 (ops_memory.rs changes).

- [ ] **Step 3.1: Add row types to `crates/vox-db/src/lib.rs` FIRST**

Find where existing row structs are defined (search for `pub struct MemoryEntry` or look near line 199 where `pub use store::{...}` appears). Add the following new structs in the `lib.rs` module body (not inside an impl block):

```rust
/// Row returned by KB queries from VoxDb.
#[derive(Debug, Clone)]
pub struct KbRow {
    pub id: String,
    pub name: String,
    pub description: String,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub entry_count: i64,
}

/// Row returned by KB entry queries from VoxDb.
#[derive(Debug, Clone)]
pub struct KbEntryRow {
    pub id: String,
    pub kb_id: String,
    pub content: String,
    pub source_signal: String,
    pub source_ref: Option<String>,
    pub routing_confidence: f64,
    pub tags: String,
    pub created_at_ms: i64,
    pub last_accessed_at_ms: Option<i64>,
    pub access_count: i64,
    pub accepted: i64,
    pub mens_queued: i64,
}

/// Row returned by KB routing rule queries from VoxDb.
#[derive(Debug, Clone)]
pub struct KbRuleRow {
    pub id: String,
    pub kb_id: String,
    pub rule_type: String,
    pub pattern: String,
    pub priority: i64,
    pub created_at_ms: i64,
}
```

- [ ] **Step 3.2: Append KB ops to `crates/vox-db/src/store/ops_memory.rs`**

> ⚠️ **Append a NEW `impl crate::VoxDb { }` block at the very end of the file** (after line 832). Rust allows multiple impl blocks on the same type. Do NOT try to add inside the existing open block — it runs to EOF without closing.

Append this entire block to the end of `ops_memory.rs`:

```rust
// ── Knowledge Base ops ────────────────────────────────────────────────────────
// These are in a new impl block (Rust allows multiple impl blocks per type).

impl crate::VoxDb {
    /// Insert a new knowledge base row.
    pub async fn kb_create(
        &self,
        id: &str,
        name: &str,
        description: &str,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let id = id.to_string();
        let name = name.to_string();
        let description = description.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO knowledge_bases (id, name, description, created_at_ms, updated_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?4)",
                    params![id.as_str(), name.as_str(), description.as_str(), now_ms],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List all knowledge bases ordered by name.
    pub async fn kb_list(&self) -> Result<Vec<crate::KbRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, name, description, created_at_ms, updated_at_ms, entry_count
                         FROM knowledge_bases ORDER BY name",
                        params![],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(row) = rows.next().await? {
                    out.push(crate::KbRow {
                        id: row.get::<String>(0)?,
                        name: row.get::<String>(1)?,
                        description: row.get::<String>(2)?,
                        created_at_ms: row.get::<i64>(3)?,
                        updated_at_ms: row.get::<i64>(4)?,
                        entry_count: row.get::<i64>(5)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Delete a knowledge base by id (cascades to entries and rules via FK).
    pub async fn kb_delete(&self, id: &str) -> Result<(), crate::store::types::StoreError> {
        let id = id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute("DELETE FROM knowledge_bases WHERE id = ?1", params![id.as_str()])
                    .await?;
                Ok(())
            })
            .await
    }

    /// Insert a KB entry and increment `entry_count` on the parent KB.
    /// Both SQL statements run in a transaction to prevent drift.
    pub async fn kb_add_entry(
        &self,
        entry_id: &str,
        kb_id: &str,
        content: &str,
        source_signal: &str,
        source_ref: Option<&str>,
        routing_confidence: f64,
        tags: &str,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let kb_id = kb_id.to_string();
        let content = content.to_string();
        let source_signal = source_signal.to_string();
        let source_ref = source_ref.map(str::to_string);
        let tags = tags.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute_batch("BEGIN").await?;
                conn.execute(
                    "INSERT INTO kb_entries
                         (id, kb_id, content, source_signal, source_ref, routing_confidence,
                          tags, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        entry_id.as_str(),
                        kb_id.as_str(),
                        content.as_str(),
                        source_signal.as_str(),
                        source_ref.as_deref(),
                        routing_confidence,
                        tags.as_str(),
                        now_ms,
                    ],
                )
                .await?;
                conn.execute(
                    "UPDATE knowledge_bases
                     SET entry_count = entry_count + 1, updated_at_ms = ?2
                     WHERE id = ?1",
                    params![kb_id.as_str(), now_ms],
                )
                .await?;
                conn.execute_batch("COMMIT").await?;
                Ok(())
            })
            .await
    }

    /// List entries for a KB, newest first.
    pub async fn kb_list_entries(
        &self,
        kb_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries WHERE kb_id = ?1
                         ORDER BY created_at_ms DESC LIMIT ?2 OFFSET ?3",
                        params![kb_id.as_str(), limit, offset],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Set the `accepted` flag on an entry and optionally mark it for MENS queuing.
    pub async fn kb_review_entry(
        &self,
        entry_id: &str,
        accepted: bool,
        queue_mens: bool,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let accepted_int: i64 = if accepted { 1 } else { 0 };
        let mens_int: i64 = if queue_mens { 1 } else { 0 };
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "UPDATE kb_entries SET accepted = ?2, mens_queued = ?3 WHERE id = ?1",
                    params![entry_id.as_str(), accepted_int, mens_int],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// Delete a specific entry and decrement the parent KB's `entry_count`.
    pub async fn kb_delete_entry(
        &self,
        entry_id: &str,
    ) -> Result<(), crate::store::types::StoreError> {
        let entry_id = entry_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute_batch("BEGIN").await?;
                // Fetch the kb_id first so we can decrement entry_count
                let mut rows = conn
                    .query(
                        "SELECT kb_id FROM kb_entries WHERE id = ?1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                if let Some(row) = rows.next().await? {
                    let kb_id: String = row.get::<String>(0)?;
                    conn.execute(
                        "DELETE FROM kb_entries WHERE id = ?1",
                        params![entry_id.as_str()],
                    )
                    .await?;
                    conn.execute(
                        "UPDATE knowledge_bases
                         SET entry_count = MAX(0, entry_count - 1)
                         WHERE id = ?1",
                        params![kb_id.as_str()],
                    )
                    .await?;
                }
                conn.execute_batch("COMMIT").await?;
                Ok(())
            })
            .await
    }

    /// List recent entries across all KBs (the "knowledge feed"), newest first.
    pub async fn kb_get_feed(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries ORDER BY created_at_ms DESC LIMIT ?1",
                        params![limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Insert a routing rule.
    pub async fn kb_add_rule(
        &self,
        rule_id: &str,
        kb_id: &str,
        rule_type: &str,
        pattern: &str,
        priority: i64,
        now_ms: i64,
    ) -> Result<(), crate::store::types::StoreError> {
        let rule_id = rule_id.to_string();
        let kb_id = kb_id.to_string();
        let rule_type = rule_type.to_string();
        let pattern = pattern.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                conn.execute(
                    "INSERT INTO kb_routing_rules
                         (id, kb_id, rule_type, pattern, priority, created_at_ms)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        rule_id.as_str(),
                        kb_id.as_str(),
                        rule_type.as_str(),
                        pattern.as_str(),
                        priority,
                        now_ms,
                    ],
                )
                .await?;
                Ok(())
            })
            .await
    }

    /// List routing rules for a KB, ordered by priority descending.
    pub async fn kb_list_rules(
        &self,
        kb_id: &str,
    ) -> Result<Vec<crate::KbRuleRow>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, rule_type, pattern, priority, created_at_ms
                         FROM kb_routing_rules WHERE kb_id = ?1
                         ORDER BY priority DESC",
                        params![kb_id.as_str()],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbRuleRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        rule_type: r.get::<String>(2)?,
                        pattern: r.get::<String>(3)?,
                        priority: r.get::<i64>(4)?,
                        created_at_ms: r.get::<i64>(5)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Substring search over accepted KB entries.
    /// Used for BM25-style routing tier and retrieval bundle injection.
    pub async fn kb_search_entries(
        &self,
        query: &str,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let query_lower = format!("%{}%", query.to_ascii_lowercase());
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries
                         WHERE accepted = 1 AND lower(content) LIKE ?1
                         ORDER BY routing_confidence DESC, created_at_ms DESC LIMIT ?2",
                        params![query_lower.as_str(), limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Fetch entries not yet queued for MENS training (both accepted and rejected).
    /// Accepted entries → SFT pairs; rejected entries → DPO pairs.
    pub async fn kb_unqueued_training_entries(
        &self,
        limit: i64,
    ) -> Result<Vec<crate::KbEntryRow>, crate::store::types::StoreError> {
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id, kb_id, content, source_signal, source_ref, routing_confidence,
                                tags, created_at_ms, last_accessed_at_ms, access_count,
                                accepted, mens_queued
                         FROM kb_entries
                         WHERE mens_queued = 0
                         ORDER BY created_at_ms ASC LIMIT ?1",
                        params![limit],
                    )
                    .await?;
                let mut out = Vec::new();
                while let Some(r) = rows.next().await? {
                    out.push(crate::KbEntryRow {
                        id: r.get::<String>(0)?,
                        kb_id: r.get::<String>(1)?,
                        content: r.get::<String>(2)?,
                        source_signal: r.get::<String>(3)?,
                        source_ref: r.get::<Option<String>>(4)?,
                        routing_confidence: r.get::<f64>(5)?,
                        tags: r.get::<String>(6)?,
                        created_at_ms: r.get::<i64>(7)?,
                        last_accessed_at_ms: r.get::<Option<i64>>(8)?,
                        access_count: r.get::<i64>(9)?,
                        accepted: r.get::<i64>(10)?,
                        mens_queued: r.get::<i64>(11)?,
                    });
                }
                Ok(out)
            })
            .await
    }

    /// Mark a batch of entries as MENS-queued.
    pub async fn kb_mark_mens_queued(
        &self,
        ids: &[String],
    ) -> Result<(), crate::store::types::StoreError> {
        for id in ids {
            let id = id.clone();
            let conn = self.conn.clone();
            let breaker = self.breaker.clone();
            breaker
                .call(|| async move {
                    conn.execute(
                        "UPDATE kb_entries SET mens_queued = 1 WHERE id = ?1",
                        params![id.as_str()],
                    )
                    .await?;
                    Ok::<_, crate::store::types::StoreError>(())
                })
                .await?;
        }
        Ok(())
    }

    /// Check if content already exists in a KB (content-hash deduplication).
    /// Returns the ID of an existing identical entry, or None.
    pub async fn kb_find_duplicate(
        &self,
        kb_id: &str,
        content: &str,
    ) -> Result<Option<String>, crate::store::types::StoreError> {
        let kb_id = kb_id.to_string();
        let content = content.to_string();
        let conn = self.conn.clone();
        let breaker = self.breaker.clone();
        breaker
            .call(|| async move {
                let mut rows = conn
                    .query(
                        "SELECT id FROM kb_entries WHERE kb_id = ?1 AND content = ?2 LIMIT 1",
                        params![kb_id.as_str(), content.as_str()],
                    )
                    .await?;
                if let Some(r) = rows.next().await? {
                    Ok(Some(r.get::<String>(0)?))
                } else {
                    Ok(None)
                }
            })
            .await
    }
}
```

- [ ] **Step 3.3: Write conversion tests in store.rs**

Now implement `crates/vox-orchestrator/src/knowledge_base/store.rs` with a `KbStore` struct and tests:

```rust
//! VoxDb CRUD for knowledge bases and entries.

use std::sync::Arc;

use vox_db::VoxDb;

use crate::{
    knowledge_base::types::{KbEntry, KbEntrySource, KbRoutingRule, KbRoutingRuleType, KnowledgeBase},
    now_unix_ms,
};

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn kb_from_row(r: vox_db::KbRow) -> KnowledgeBase {
    KnowledgeBase {
        id: r.id,
        name: r.name,
        description: r.description,
        created_at_ms: r.created_at_ms,
        updated_at_ms: r.updated_at_ms,
        entry_count: r.entry_count,
    }
}

fn entry_from_row(r: vox_db::KbEntryRow) -> KbEntry {
    KbEntry {
        id: r.id,
        kb_id: r.kb_id,
        content: r.content,
        source_signal: r.source_signal,
        source_ref: r.source_ref,
        routing_confidence: r.routing_confidence,
        tags: r.tags,
        created_at_ms: r.created_at_ms,
        last_accessed_at_ms: r.last_accessed_at_ms,
        access_count: r.access_count,
        accepted: r.accepted,
        mens_queued: r.mens_queued,
    }
}

fn rule_from_row(r: vox_db::KbRuleRow) -> KbRoutingRule {
    let rule_type = if r.rule_type == "regex" {
        KbRoutingRuleType::Regex
    } else {
        KbRoutingRuleType::Keyword
    };
    KbRoutingRule {
        id: r.id,
        kb_id: r.kb_id,
        rule_type,
        pattern: r.pattern,
        priority: r.priority,
        created_at_ms: r.created_at_ms,
    }
}

/// Async CRUD for knowledge bases backed by VoxDb.
pub struct KbStore {
    db: Arc<VoxDb>,
}

impl KbStore {
    pub fn new(db: Arc<VoxDb>) -> Self {
        Self { db }
    }

    pub async fn create(&self, name: &str, description: &str) -> Result<KnowledgeBase, String> {
        let id = new_id();
        let now = now_unix_ms();
        self.db
            .kb_create(&id, name, description, now)
            .await
            .map_err(|e| e.to_string())?;
        Ok(KnowledgeBase {
            id,
            name: name.to_string(),
            description: description.to_string(),
            created_at_ms: now,
            updated_at_ms: now,
            entry_count: 0,
        })
    }

    pub async fn list(&self) -> Result<Vec<KnowledgeBase>, String> {
        self.db
            .kb_list()
            .await
            .map(|rows| rows.into_iter().map(kb_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn delete(&self, id: &str) -> Result<(), String> {
        self.db.kb_delete(id).await.map_err(|e| e.to_string())
    }

    /// Add an entry to a KB with exact-content deduplication.
    /// If the content already exists in the KB, returns the existing entry's id without inserting.
    pub async fn add_entry(
        &self,
        kb_id: &str,
        content: &str,
        source: KbEntrySource,
        source_ref: Option<&str>,
        routing_confidence: f64,
        tags: &[String],
    ) -> Result<KbEntry, String> {
        // Deduplication check (SOTA: search-before-insert)
        if let Ok(Some(existing_id)) = self.db.kb_find_duplicate(kb_id, content).await {
            // Entry already exists — return a minimal KbEntry with just the id
            return Ok(KbEntry {
                id: existing_id,
                kb_id: kb_id.to_string(),
                content: content.to_string(),
                source_signal: source.as_str().to_string(),
                source_ref: source_ref.map(str::to_string),
                routing_confidence,
                tags: serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string()),
                created_at_ms: 0, // unknown for existing
                last_accessed_at_ms: None,
                access_count: 0,
                accepted: 1,
                mens_queued: 0,
            });
        }

        let id = new_id();
        let now = now_unix_ms();
        let tags_json = serde_json::to_string(tags).unwrap_or_else(|_| "[]".to_string());
        self.db
            .kb_add_entry(
                &id,
                kb_id,
                content,
                source.as_str(),
                source_ref,
                routing_confidence,
                &tags_json,
                now,
            )
            .await
            .map_err(|e| e.to_string())?;
        Ok(KbEntry {
            id,
            kb_id: kb_id.to_string(),
            content: content.to_string(),
            source_signal: source.as_str().to_string(),
            source_ref: source_ref.map(str::to_string),
            routing_confidence,
            tags: tags_json,
            created_at_ms: now,
            last_accessed_at_ms: None,
            access_count: 0,
            accepted: 1,
            mens_queued: 0,
        })
    }

    pub async fn list_entries(
        &self,
        kb_id: &str,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_list_entries(kb_id, limit, offset)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    /// Accept or reject an entry. Sets `mens_queued = 1` for accepted entries.
    pub async fn review_entry(&self, entry_id: &str, accepted: bool) -> Result<(), String> {
        self.db
            .kb_review_entry(entry_id, accepted, accepted)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn delete_entry(&self, entry_id: &str) -> Result<(), String> {
        self.db
            .kb_delete_entry(entry_id)
            .await
            .map_err(|e| e.to_string())
    }

    pub async fn get_feed(&self, limit: i64) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_get_feed(limit)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn add_rule(
        &self,
        kb_id: &str,
        rule_type: KbRoutingRuleType,
        pattern: &str,
        priority: i64,
    ) -> Result<KbRoutingRule, String> {
        let id = new_id();
        let now = now_unix_ms();
        self.db
            .kb_add_rule(&id, kb_id, rule_type.as_str(), pattern, priority, now)
            .await
            .map_err(|e| e.to_string())?;
        Ok(KbRoutingRule {
            id,
            kb_id: kb_id.to_string(),
            rule_type,
            pattern: pattern.to_string(),
            priority,
            created_at_ms: now,
        })
    }

    pub async fn list_rules(&self, kb_id: &str) -> Result<Vec<KbRoutingRule>, String> {
        self.db
            .kb_list_rules(kb_id)
            .await
            .map(|rows| rows.into_iter().map(rule_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn search_entries(&self, query: &str, limit: i64) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_search_entries(query, limit)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn unqueued_training_entries(&self, limit: i64) -> Result<Vec<KbEntry>, String> {
        self.db
            .kb_unqueued_training_entries(limit)
            .await
            .map(|rows| rows.into_iter().map(entry_from_row).collect())
            .map_err(|e| e.to_string())
    }

    pub async fn mark_mens_queued(&self, ids: &[String]) -> Result<(), String> {
        self.db
            .kb_mark_mens_queued(ids)
            .await
            .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entry_from_row_preserves_fields() {
        let row = vox_db::KbEntryRow {
            id: "e1".to_string(),
            kb_id: "k1".to_string(),
            content: "test content".to_string(),
            source_signal: "chat".to_string(),
            source_ref: Some("chat-session-42".to_string()),
            routing_confidence: 0.9,
            tags: "[\"rust\"]".to_string(),
            created_at_ms: 1000,
            last_accessed_at_ms: None,
            access_count: 0,
            accepted: 1,
            mens_queued: 0,
        };
        let entry = entry_from_row(row);
        assert_eq!(entry.id, "e1");
        assert_eq!(entry.kb_id, "k1");
        assert_eq!(entry.content, "test content");
        assert_eq!(entry.source_signal, "chat");
        assert!((entry.routing_confidence - 0.9).abs() < 1e-9);
        assert_eq!(entry.accepted, 1);
        assert_eq!(entry.mens_queued, 0);
    }

    #[test]
    fn rule_from_row_keyword_type() {
        let row = vox_db::KbRuleRow {
            id: "r1".to_string(),
            kb_id: "k1".to_string(),
            rule_type: "keyword".to_string(),
            pattern: "qdrant".to_string(),
            priority: 10,
            created_at_ms: 1000,
        };
        let rule = rule_from_row(row);
        assert_eq!(rule.rule_type, KbRoutingRuleType::Keyword);
        assert_eq!(rule.pattern, "qdrant");
    }

    #[test]
    fn rule_from_row_unknown_type_defaults_to_keyword() {
        let row = vox_db::KbRuleRow {
            id: "r2".to_string(),
            kb_id: "k1".to_string(),
            rule_type: "future_unknown_type".to_string(),
            pattern: "x".to_string(),
            priority: 0,
            created_at_ms: 1000,
        };
        let rule = rule_from_row(row);
        assert_eq!(rule.rule_type, KbRoutingRuleType::Keyword);
    }

    #[test]
    fn new_id_is_valid_uuid_format() {
        let id = new_id();
        // UUID v4 format: 8-4-4-4-12 hex chars
        assert_eq!(id.len(), 36);
        assert_eq!(id.chars().filter(|c| *c == '-').count(), 4);
    }
}
```

- [ ] **Step 3.4: Run tests**

```powershell
cargo test -p vox-orchestrator knowledge_base
cargo test -p vox-db
```

Expected: all pass.

- [ ] **Step 3.5: Commit**

```powershell
cargo fmt -p vox-orchestrator
cargo fmt -p vox-db
git add crates/vox-orchestrator/src/knowledge_base/store.rs crates/vox-db/src/store/ops_memory.rs crates/vox-db/src/lib.rs
git commit -m "feat(vox-orchestrator): implement KbStore CRUD over VoxDb with deduplication"
```

---

## Task 4: KbRouter — Three-Tier Routing

**Files:**
- Modify: `crates/vox-orchestrator/src/knowledge_base/router.rs`

- [ ] **Step 4.1: Write failing tests first**

Replace `router.rs` with just the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_base::types::{KbRoutingRule, KbRoutingRuleType};

    fn kw_rule(kb_id: &str, pattern: &str, priority: i64) -> KbRoutingRule {
        KbRoutingRule {
            id: "r".to_string(),
            kb_id: kb_id.to_string(),
            rule_type: KbRoutingRuleType::Keyword,
            pattern: pattern.to_string(),
            priority,
            created_at_ms: 0,
        }
    }

    #[test]
    fn keyword_rule_matches_case_insensitively() {
        let rule = kw_rule("kb1", "brown", 0);
        assert!(keyword_rule_matches("The quick BROWN fox", &rule));
    }

    #[test]
    fn keyword_rule_no_match() {
        let rule = kw_rule("kb1", "qdrant", 0);
        assert!(!keyword_rule_matches("The quick brown fox", &rule));
    }

    #[test]
    fn jaccard_identical() {
        assert!((jaccard_word_similarity("rust async tokio", "rust async tokio") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_no_overlap() {
        assert_eq!(jaccard_word_similarity("rust tokio", "python django"), 0.0);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let s = jaccard_word_similarity("rust async tokio", "rust sync blocking");
        assert!(s > 0.0 && s < 1.0, "score={s}");
    }

    #[test]
    fn apply_keyword_rules_returns_matching_kb() {
        let rules = vec![
            kw_rule("kb_retrieval", "qdrant", 10),
            kw_rule("kb_rust", "tokio", 5),
        ];
        let matches = apply_keyword_rules("using Qdrant for vector search", &rules);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "kb_retrieval");
        assert!((matches[0].1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn apply_keyword_rules_priority_ordering() {
        // Both rules match — both should appear, sorted by confidence
        let rules = vec![
            kw_rule("kb_low", "the", 0),
            kw_rule("kb_high", "quick", 10),
        ];
        let matches = apply_keyword_rules("the quick brown fox", &rules);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn similarity_above_threshold_routes() {
        let samples = vec![
            ("kb_rust".to_string(), vec!["tokio async runtime".to_string()]),
        ];
        let results = apply_similarity_routing("tokio async executor", &samples, 0.15);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "kb_rust");
    }

    #[test]
    fn similarity_below_threshold_no_route() {
        let samples = vec![
            ("kb_rust".to_string(), vec!["python django flask".to_string()]),
        ];
        let results = apply_similarity_routing("tokio async runtime", &samples, 0.15);
        assert!(results.is_empty());
    }
}
```

- [ ] **Step 4.2: Run to confirm failure**

```powershell
cargo test -p vox-orchestrator router
```

Expected: compile error — items not defined.

- [ ] **Step 4.3: Implement `router.rs`**

```rust
//! Three-tier routing: keyword rules → Jaccard word similarity → LLM (high-value only).

use crate::knowledge_base::types::{KbRoutingRule, KbRoutingRuleType};

/// Returns `true` if `content` matches the routing rule's pattern.
pub fn keyword_rule_matches(content: &str, rule: &KbRoutingRule) -> bool {
    let content_lower = content.to_ascii_lowercase();
    let pattern_lower = rule.pattern.to_ascii_lowercase();
    match rule.rule_type {
        // Regex: fall back to substring match for the MVP (avoids the `regex` dep).
        // A future enhancement can compile regex patterns explicitly.
        KbRoutingRuleType::Keyword | KbRoutingRuleType::Regex => {
            content_lower.contains(&pattern_lower)
        }
    }
}

/// Jaccard word-set similarity between two strings (tokenized by whitespace).
/// Returns a value in `[0.0, 1.0]`. Both empty → 0.0.
pub fn jaccard_word_similarity(a: &str, b: &str) -> f64 {
    use std::collections::HashSet;
    let a_words: HashSet<&str> = a.split_whitespace().collect();
    let b_words: HashSet<&str> = b.split_whitespace().collect();
    if a_words.is_empty() && b_words.is_empty() {
        return 0.0;
    }
    let intersection = a_words.intersection(&b_words).count();
    let union = a_words.union(&b_words).count();
    if union == 0 { 0.0 } else { intersection as f64 / union as f64 }
}

/// Apply keyword/regex rules to content.
///
/// Rules are sorted by priority (highest first). One match per KB is emitted
/// (the first matching rule for each KB wins). Returns `(kb_id, confidence=1.0)` pairs,
/// sorted by confidence descending.
pub fn apply_keyword_rules(content: &str, rules: &[KbRoutingRule]) -> Vec<(String, f64)> {
    let mut sorted = rules.to_vec();
    sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

    let mut matched: std::collections::HashMap<String, f64> = std::collections::HashMap::new();
    for rule in &sorted {
        if !matched.contains_key(&rule.kb_id) && keyword_rule_matches(content, rule) {
            matched.insert(rule.kb_id.clone(), 1.0);
        }
    }

    let mut result: Vec<(String, f64)> = matched.into_iter().collect();
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Apply Jaccard similarity against each KB's sample entries.
///
/// Returns KBs where average Jaccard similarity across samples exceeds `threshold`.
/// `kb_samples` is `(kb_id, sample_entry_contents)` pairs.
/// Results are sorted by score descending.
pub fn apply_similarity_routing(
    content: &str,
    kb_samples: &[(String, Vec<String>)],
    threshold: f64,
) -> Vec<(String, f64)> {
    let mut result = Vec::new();
    for (kb_id, samples) in kb_samples {
        if samples.is_empty() {
            continue;
        }
        let scores: Vec<f64> = samples
            .iter()
            .map(|s| jaccard_word_similarity(content, s))
            .collect();
        let avg = scores.iter().sum::<f64>() / scores.len() as f64;
        if avg >= threshold {
            result.push((kb_id.clone(), avg));
        }
    }
    result.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    result
}

/// Minimum Jaccard score to route an item to a KB via similarity.
/// Below this threshold, tier 2 routing passes and tier 3 (LLM) applies.
pub const SIMILARITY_THRESHOLD: f64 = 0.15;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::knowledge_base::types::{KbRoutingRule, KbRoutingRuleType};

    fn kw_rule(kb_id: &str, pattern: &str, priority: i64) -> KbRoutingRule {
        KbRoutingRule {
            id: "r".to_string(),
            kb_id: kb_id.to_string(),
            rule_type: KbRoutingRuleType::Keyword,
            pattern: pattern.to_string(),
            priority,
            created_at_ms: 0,
        }
    }

    #[test]
    fn keyword_rule_matches_case_insensitively() {
        let rule = kw_rule("kb1", "brown", 0);
        assert!(keyword_rule_matches("The quick BROWN fox", &rule));
    }

    #[test]
    fn keyword_rule_no_match() {
        let rule = kw_rule("kb1", "qdrant", 0);
        assert!(!keyword_rule_matches("The quick brown fox", &rule));
    }

    #[test]
    fn jaccard_identical() {
        assert!((jaccard_word_similarity("rust async tokio", "rust async tokio") - 1.0).abs() < 1e-9);
    }

    #[test]
    fn jaccard_no_overlap() {
        assert_eq!(jaccard_word_similarity("rust tokio", "python django"), 0.0);
    }

    #[test]
    fn jaccard_partial_overlap() {
        let s = jaccard_word_similarity("rust async tokio", "rust sync blocking");
        assert!(s > 0.0 && s < 1.0, "score={s}");
    }

    #[test]
    fn apply_keyword_rules_returns_matching_kb() {
        let rules = vec![
            kw_rule("kb_retrieval", "qdrant", 10),
            kw_rule("kb_rust", "tokio", 5),
        ];
        let matches = apply_keyword_rules("using Qdrant for vector search", &rules);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, "kb_retrieval");
        assert!((matches[0].1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn apply_keyword_rules_priority_ordering() {
        let rules = vec![
            kw_rule("kb_low", "the", 0),
            kw_rule("kb_high", "quick", 10),
        ];
        let matches = apply_keyword_rules("the quick brown fox", &rules);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn similarity_above_threshold_routes() {
        let samples = vec![
            ("kb_rust".to_string(), vec!["tokio async runtime".to_string()]),
        ];
        let results = apply_similarity_routing("tokio async executor", &samples, 0.15);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "kb_rust");
    }

    #[test]
    fn similarity_below_threshold_no_route() {
        let samples = vec![
            ("kb_rust".to_string(), vec!["python django flask".to_string()]),
        ];
        let results = apply_similarity_routing("tokio async runtime", &samples, 0.15);
        assert!(results.is_empty());
    }
}
```

- [ ] **Step 4.4: Run tests**

```powershell
cargo test -p vox-orchestrator router
```

Expected: `test result: ok. 8 passed`

- [ ] **Step 4.5: Commit**

```powershell
cargo fmt -p vox-orchestrator
git add crates/vox-orchestrator/src/knowledge_base/router.rs
git commit -m "feat(vox-orchestrator): implement KbRouter three-tier routing (keyword + Jaccard)"
```

---

## Task 5: MCP Params + Handlers + Dispatch

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/kb_tools/params.rs`
- Create: `crates/vox-orchestrator-mcp/src/kb_tools/handlers.rs`
- Create: `crates/vox-orchestrator-mcp/src/kb_tools/mod.rs` (plus signal adapter stubs)
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs`
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs`

- [ ] **Step 5.1: Create `params.rs`**

```rust
//! MCP argument structs for VoxKB tools.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbCreateParams {
    /// Unique name for the knowledge base (e.g. "Rust async patterns").
    pub name: String,
    /// Short description of what this KB collects.
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbDeleteParams {
    pub kb_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbAddEntryParams {
    pub kb_id: String,
    /// The content to store. Should be a self-contained atomic fact (~100-300 tokens).
    pub content: String,
    /// Signal source: "chat", "research", "code_activity", "web", "explicit", "scientia".
    #[serde(default = "default_explicit")]
    pub source_signal: String,
    /// Optional source reference (URL, file path, session ID).
    pub source_ref: Option<String>,
    /// JSON array of tag strings, e.g. ["rust","async"]. Default: [].
    #[serde(default = "default_empty_array")]
    pub tags: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbDeleteEntryParams {
    pub entry_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbListEntriesParams {
    pub kb_id: String,
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbReviewEntryParams {
    pub entry_id: String,
    /// `true` = accept into KB (queued for MENS SFT). `false` = reject (DPO negative).
    pub accepted: bool,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbGetFeedParams {
    #[serde(default = "default_feed_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbAddRuleParams {
    pub kb_id: String,
    /// Rule type: "keyword" (case-insensitive substring) or "regex" (pattern match).
    #[serde(default = "default_keyword")]
    pub rule_type: String,
    /// Pattern to match against entry content.
    pub pattern: String,
    /// Higher = checked first. Default: 0.
    #[serde(default)]
    pub priority: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbListRulesParams {
    pub kb_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbQueryParams {
    /// Free-text search query.
    pub query: String,
    /// Optional: only return results from these KB IDs. Empty = all KBs.
    #[serde(default)]
    pub kb_ids: Vec<String>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct KbClipParams {
    /// Content to save. Should be a self-contained atomic fact or insight.
    pub content: String,
    /// Optional source reference (URL, file path).
    pub source_ref: Option<String>,
    /// KB IDs to clip into. If empty, the router decides via keyword rules.
    #[serde(default)]
    pub kb_ids: Vec<String>,
    /// JSON array of tag strings.
    #[serde(default = "default_empty_array")]
    pub tags: String,
}

fn default_explicit() -> String { "explicit".to_string() }
fn default_keyword() -> String { "keyword".to_string() }
fn default_empty_array() -> String { "[]".to_string() }
fn default_limit() -> i64 { 20 }
fn default_feed_limit() -> i64 { 50 }
```

- [ ] **Step 5.2: Create `handlers.rs`**

```rust
//! MCP handler functions for VoxKB tools.

use crate::params::ToolResult;
use crate::server_state::ServerState;

use super::params::*;
use vox_orchestrator::knowledge_base::{
    router::{apply_keyword_rules, apply_similarity_routing, SIMILARITY_THRESHOLD},
    store::KbStore,
    types::{KbEntrySource, KbRoutingRuleType},
};

const REM_KB_DB: &str =
    "Attach VoxDb (VOX_DB_PATH / VOX_DB_URL) to the MCP server for KB operations.";
const REM_KB_NOT_FOUND: &str = "Run vox_kb_list to see available KB IDs.";

fn require_db(state: &ServerState) -> Result<std::sync::Arc<vox_db::VoxDb>, String> {
    state.db.clone().ok_or_else(|| REM_KB_DB.to_string())
}

/// Create a new named knowledge base.
pub async fn kb_create(state: &ServerState, params: KbCreateParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.create(&params.name, &params.description).await {
                Ok(kb) => ToolResult::ok(serde_json::to_value(&kb).unwrap_or_default()).to_json(),
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// List all knowledge bases.
pub async fn kb_list(state: &ServerState) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.list().await {
                Ok(kbs) => {
                    ToolResult::ok(serde_json::to_value(&kbs).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Delete a knowledge base and all its entries.
pub async fn kb_delete(state: &ServerState, params: KbDeleteParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.delete(&params.kb_id).await {
                Ok(()) => ToolResult::ok("deleted").to_json(),
                Err(e) => {
                    ToolResult::<()>::err_with_remediation(e, REM_KB_NOT_FOUND).to_json()
                }
            }
        }
    }
}

/// Add an entry to a knowledge base (with deduplication).
pub async fn kb_add_entry(state: &ServerState, params: KbAddEntryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let source = match params.source_signal.as_str() {
                "chat" => KbEntrySource::Chat,
                "research" => KbEntrySource::Research,
                "code_activity" => KbEntrySource::CodeActivity,
                "web" => KbEntrySource::Web,
                "scientia" => KbEntrySource::Scientia,
                _ => KbEntrySource::Explicit,
            };
            let tags: Vec<String> =
                serde_json::from_str(&params.tags).unwrap_or_default();
            let store = KbStore::new(db);
            match store
                .add_entry(
                    &params.kb_id,
                    &params.content,
                    source,
                    params.source_ref.as_deref(),
                    1.0,
                    &tags,
                )
                .await
            {
                Ok(entry) => {
                    ToolResult::ok(serde_json::to_value(&entry).unwrap_or_default()).to_json()
                }
                Err(e) => {
                    ToolResult::<()>::err_with_remediation(e, REM_KB_NOT_FOUND).to_json()
                }
            }
        }
    }
}

/// Delete a specific KB entry (also decrements parent KB entry_count).
pub async fn kb_delete_entry(state: &ServerState, params: KbDeleteEntryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.delete_entry(&params.entry_id).await {
                Ok(()) => ToolResult::ok("deleted").to_json(),
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// List entries in a knowledge base.
pub async fn kb_list_entries(state: &ServerState, params: KbListEntriesParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store
                .list_entries(&params.kb_id, params.limit, params.offset)
                .await
            {
                Ok(entries) => {
                    ToolResult::ok(serde_json::to_value(&entries).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Accept or reject a KB entry (accepted → queued for MENS SFT; rejected → DPO pair).
pub async fn kb_review_entry(state: &ServerState, params: KbReviewEntryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.review_entry(&params.entry_id, params.accepted).await {
                Ok(()) => {
                    ToolResult::ok(if params.accepted { "accepted" } else { "rejected" }).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Get the knowledge feed — recent entries across all KBs.
pub async fn kb_get_feed(state: &ServerState, params: KbGetFeedParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.get_feed(params.limit).await {
                Ok(entries) => {
                    ToolResult::ok(serde_json::to_value(&entries).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Add a routing rule to a KB.
pub async fn kb_add_rule(state: &ServerState, params: KbAddRuleParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let rule_type = if params.rule_type == "regex" {
                KbRoutingRuleType::Regex
            } else {
                KbRoutingRuleType::Keyword
            };
            let store = KbStore::new(db);
            match store
                .add_rule(&params.kb_id, rule_type, &params.pattern, params.priority)
                .await
            {
                Ok(rule) => {
                    ToolResult::ok(serde_json::to_value(&rule).unwrap_or_default()).to_json()
                }
                Err(e) => {
                    ToolResult::<()>::err_with_remediation(e, REM_KB_NOT_FOUND).to_json()
                }
            }
        }
    }
}

/// List routing rules for a KB.
pub async fn kb_list_rules(state: &ServerState, params: KbListRulesParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.list_rules(&params.kb_id).await {
                Ok(rules) => {
                    ToolResult::ok(serde_json::to_value(&rules).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Substring search across accepted KB entries.
pub async fn kb_query(state: &ServerState, params: KbQueryParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            match store.search_entries(&params.query, params.limit).await {
                Ok(mut entries) => {
                    if !params.kb_ids.is_empty() {
                        entries.retain(|e| params.kb_ids.contains(&e.kb_id));
                    }
                    ToolResult::ok(serde_json::to_value(&entries).unwrap_or_default()).to_json()
                }
                Err(e) => ToolResult::<()>::err(e).to_json(),
            }
        }
    }
}

/// Explicit clip — user saves content directly into specified KB(s).
/// If `kb_ids` is empty, auto-routes via KbRouter keyword rules.
pub async fn kb_clip(state: &ServerState, params: KbClipParams) -> String {
    match require_db(state) {
        Err(e) => ToolResult::<String>::err_with_remediation(e, REM_KB_DB).to_json(),
        Ok(db) => {
            let store = KbStore::new(db);
            let tags: Vec<String> =
                serde_json::from_str(&params.tags).unwrap_or_default();

            let target_kb_ids = if params.kb_ids.is_empty() {
                let kbs = store.list().await.unwrap_or_default();
                let mut all_rules = Vec::new();
                for kb in &kbs {
                    let rules = store.list_rules(&kb.id).await.unwrap_or_default();
                    all_rules.extend(rules);
                }
                apply_keyword_rules(&params.content, &all_rules)
                    .into_iter()
                    .map(|(id, _)| id)
                    .collect::<Vec<_>>()
            } else {
                params.kb_ids.clone()
            };

            let mut saved = Vec::new();
            for kb_id in &target_kb_ids {
                match store
                    .add_entry(
                        kb_id,
                        &params.content,
                        KbEntrySource::Explicit,
                        params.source_ref.as_deref(),
                        1.0,
                        &tags,
                    )
                    .await
                {
                    Ok(entry) => saved.push(entry),
                    Err(e) => return ToolResult::<()>::err(e).to_json(),
                }
            }
            ToolResult::ok(serde_json::to_value(&saved).unwrap_or_default()).to_json()
        }
    }
}
```

- [ ] **Step 5.3: Create `mod.rs` and signal adapter stubs**

Create `crates/vox-orchestrator-mcp/src/kb_tools/mod.rs`:
```rust
//! VoxKB MCP tool surface.
pub mod handlers;
pub mod params;
pub mod signal_chat;
pub mod signal_research;
pub mod signal_scientia;

pub use handlers::*;
```

Create `crates/vox-orchestrator-mcp/src/kb_tools/signal_chat.rs`:
```rust
//! Chat turn signal adapter — extracts KB entries from assistant turns.
// Implemented in Task 6.
```

Create `crates/vox-orchestrator-mcp/src/kb_tools/signal_research.rs`:
```rust
//! Research completion signal adapter.
// Implemented in Task 6.
```

Create `crates/vox-orchestrator-mcp/src/kb_tools/signal_scientia.rs`:
```rust
//! Scientia finding promotion adapter.
// Implemented in Task 6.
```

- [ ] **Step 5.4: Register in `crates/vox-orchestrator-mcp/src/lib.rs`**

Find the block of `pub mod` declarations. Add:
```rust
pub mod kb_tools;
pub use kb_tools as kb;
```

- [ ] **Step 5.5: Add dispatch arms in `crates/vox-orchestrator-mcp/src/dispatch.rs`**

Find the block ending with `"vox_memory_recall_db"` (around line 1004). Add immediately after it:

```rust
        // ── Knowledge Bases (VoxKB) ──────────────────────────────────────────
        "vox_kb_create" => Ok(crate::kb::kb_create(state, serde_json::from_value(args)?).await),
        "vox_kb_list" => Ok(crate::kb::kb_list(state).await),
        "vox_kb_delete" => {
            Ok(crate::kb::kb_delete(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_add_entry" => {
            Ok(crate::kb::kb_add_entry(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_delete_entry" => {
            Ok(crate::kb::kb_delete_entry(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_list_entries" => {
            Ok(crate::kb::kb_list_entries(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_review_entry" => {
            Ok(crate::kb::kb_review_entry(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_get_feed" => {
            Ok(crate::kb::kb_get_feed(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_add_rule" => {
            Ok(crate::kb::kb_add_rule(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_list_rules" => {
            Ok(crate::kb::kb_list_rules(state, serde_json::from_value(args)?).await)
        }
        "vox_kb_query" => Ok(crate::kb::kb_query(state, serde_json::from_value(args)?).await),
        "vox_kb_clip" => Ok(crate::kb::kb_clip(state, serde_json::from_value(args)?).await),
```

- [ ] **Step 5.6: Add JSON schemas in `crates/vox-orchestrator-mcp/src/input_schemas.rs`**

Find the `// ── Memory (MEMORY.md / search)` comment block. Add a new block immediately BEFORE it:

```rust
        // ── Knowledge Bases (VoxKB) ──────────────────────────────────────────
        "vox_kb_create" => derived_tool_schema!(crate::kb_tools::params::KbCreateParams),
        "vox_kb_list" => parse_obj(r#"{"type":"object","additionalProperties":false}"#),
        "vox_kb_delete" => derived_tool_schema!(crate::kb_tools::params::KbDeleteParams),
        "vox_kb_add_entry" => derived_tool_schema!(crate::kb_tools::params::KbAddEntryParams),
        "vox_kb_delete_entry" => derived_tool_schema!(crate::kb_tools::params::KbDeleteEntryParams),
        "vox_kb_list_entries" => {
            derived_tool_schema!(crate::kb_tools::params::KbListEntriesParams)
        }
        "vox_kb_review_entry" => {
            derived_tool_schema!(crate::kb_tools::params::KbReviewEntryParams)
        }
        "vox_kb_get_feed" => derived_tool_schema!(crate::kb_tools::params::KbGetFeedParams),
        "vox_kb_add_rule" => derived_tool_schema!(crate::kb_tools::params::KbAddRuleParams),
        "vox_kb_list_rules" => derived_tool_schema!(crate::kb_tools::params::KbListRulesParams),
        "vox_kb_query" => derived_tool_schema!(crate::kb_tools::params::KbQueryParams),
        "vox_kb_clip" => derived_tool_schema!(crate::kb_tools::params::KbClipParams),
```

- [ ] **Step 5.7: Build to verify**

```powershell
cargo build -p vox-orchestrator-mcp
```

Expected: clean compile.

- [ ] **Step 5.8: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/kb_tools/ crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs
git commit -m "feat(vox-orchestrator-mcp): add VoxKB MCP tools (11 tools: create/list/delete/add_entry/query/clip/review/feed/rules)"
```

---

## Task 6: Signal Adapters — Chat, Research, Scientia

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/kb_tools/signal_chat.rs`
- Modify: `crates/vox-orchestrator-mcp/src/kb_tools/signal_research.rs`
- Modify: `crates/vox-orchestrator-mcp/src/kb_tools/signal_scientia.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs`

- [ ] **Step 6.1: Write failing test for `signal_chat.rs`**

Replace `signal_chat.rs` with just the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_chat_snippets_short_content_single_snippet() {
        // Must be ≥ 20 chars to pass the filter
        let content = "The quick brown fox jumps over the lazy dog repeatedly";
        let snippets = extract_chat_snippets(content);
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0], content);
    }

    #[test]
    fn extract_chat_snippets_filters_very_short_content() {
        let snippets = extract_chat_snippets("ok");
        assert!(snippets.is_empty());
    }

    #[test]
    fn extract_chat_snippets_empty_returns_empty() {
        let snippets = extract_chat_snippets("   ");
        assert!(snippets.is_empty());
    }

    #[test]
    fn extract_chat_snippets_deduplicates() {
        let content = "Rust uses ownership for memory safety.\n\nRust uses ownership for memory safety.";
        let snippets = extract_chat_snippets(content);
        assert_eq!(snippets.len(), 1);
    }

    #[test]
    fn extract_chat_snippets_splits_long_content() {
        let content = "a ".repeat(300);
        let snippets = extract_chat_snippets(&content);
        // Might be empty if split results are < 20 chars, but must not panic
        assert!(snippets.len() <= 600);
    }
}
```

- [ ] **Step 6.2: Run to confirm failure**

```powershell
cargo test -p vox-orchestrator-mcp signal_chat
```

Expected: compile error.

- [ ] **Step 6.3: Implement `signal_chat.rs`**

```rust
//! Chat turn signal adapter — extracts KB entries from completed assistant turns.

use std::collections::HashSet;

/// Minimum character length for a snippet to be stored.
const MIN_SNIPPET_LEN: usize = 20;

/// Maximum characters per paragraph before sentence-splitting is applied.
const MAX_SNIPPET_CHARS: usize = 512;

/// Extract KB-candidate snippets from an assistant turn's content.
///
/// Splits by paragraph boundary (`\n\n`), then by sentence boundary (`. `) for long
/// paragraphs. Deduplicates case-insensitively and filters very short strings.
pub fn extract_chat_snippets(content: &str) -> Vec<String> {
    let content = content.trim();
    if content.is_empty() {
        return Vec::new();
    }

    let paragraphs: Vec<&str> = content
        .split("\n\n")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut seen: HashSet<String> = HashSet::new();
    let mut result = Vec::new();

    for para in paragraphs {
        let chunks: Vec<String> = if para.len() <= MAX_SNIPPET_CHARS {
            vec![para.to_string()]
        } else {
            para.split(". ")
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };

        for chunk in chunks {
            if chunk.len() < MIN_SNIPPET_LEN {
                continue;
            }
            let key = chunk.to_ascii_lowercase();
            if !seen.contains(&key) {
                seen.insert(key);
                result.push(chunk);
            }
        }
    }
    result
}

/// Fire-and-forget: route `assistant_content` snippets into matching KBs.
///
/// Called after a chat turn completes. Errors are swallowed to avoid disrupting
/// the chat flow. If no KBs are configured, returns immediately.
///
/// NOTE: `session_ref` is the `session_id: String` from message.rs — pass as
/// `Some(session_id.as_str())`.
pub async fn ingest_chat_turn(
    db: std::sync::Arc<vox_db::VoxDb>,
    assistant_content: &str,
    session_ref: Option<&str>,
) {
    use vox_orchestrator::knowledge_base::{
        router::{apply_keyword_rules, apply_similarity_routing, SIMILARITY_THRESHOLD},
        store::KbStore,
        types::KbEntrySource,
    };

    let snippets = extract_chat_snippets(assistant_content);
    if snippets.is_empty() {
        return;
    }

    let store = KbStore::new(db);
    let kbs = match store.list().await {
        Ok(kbs) if !kbs.is_empty() => kbs,
        _ => return,
    };

    let mut all_rules = Vec::new();
    for kb in &kbs {
        if let Ok(rules) = store.list_rules(&kb.id).await {
            all_rules.extend(rules);
        }
    }

    for snippet in &snippets {
        // Tier 1: keyword rules
        let mut targets = apply_keyword_rules(snippet, &all_rules);

        // Tier 2: Jaccard similarity (only if tier 1 found no match)
        if targets.is_empty() {
            let mut samples: Vec<(String, Vec<String>)> = Vec::new();
            for kb in &kbs {
                // Cap at 10 samples per KB to keep O(n) bounded
                let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
                let contents: Vec<String> = entries.into_iter().map(|e| e.content).collect();
                if !contents.is_empty() {
                    samples.push((kb.id.clone(), contents));
                }
            }
            targets = apply_similarity_routing(snippet, &samples, SIMILARITY_THRESHOLD);
        }

        for (kb_id, confidence) in targets {
            let _ = store
                .add_entry(&kb_id, snippet, KbEntrySource::Chat, session_ref, confidence, &[])
                .await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_chat_snippets_short_content_single_snippet() {
        let content = "The quick brown fox jumps over the lazy dog repeatedly";
        let snippets = extract_chat_snippets(content);
        assert_eq!(snippets.len(), 1);
        assert_eq!(snippets[0], content);
    }

    #[test]
    fn extract_chat_snippets_filters_very_short_content() {
        let snippets = extract_chat_snippets("ok");
        assert!(snippets.is_empty());
    }

    #[test]
    fn extract_chat_snippets_empty_returns_empty() {
        let snippets = extract_chat_snippets("   ");
        assert!(snippets.is_empty());
    }

    #[test]
    fn extract_chat_snippets_deduplicates() {
        let content =
            "Rust uses ownership for memory safety.\n\nRust uses ownership for memory safety.";
        let snippets = extract_chat_snippets(content);
        assert_eq!(snippets.len(), 1);
    }

    #[test]
    fn extract_chat_snippets_splits_long_content() {
        let content = "a ".repeat(300);
        let snippets = extract_chat_snippets(&content);
        assert!(snippets.len() <= 600);
    }
}
```

- [ ] **Step 6.4: Implement `signal_research.rs`**

```rust
//! Research completion signal adapter — ingests research synthesis reports into matching KBs.

use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator::knowledge_base::{
    router::{apply_keyword_rules, apply_similarity_routing, SIMILARITY_THRESHOLD},
    store::KbStore,
    types::KbEntrySource,
};

/// Ingest a completed research synthesis report into matching KBs.
/// Research reports are high-value (minimum confidence 0.95).
pub async fn ingest_research_result(
    db: Arc<VoxDb>,
    synthesis: &str,
    query: &str,
    session_id: Option<i64>,
) {
    if synthesis.trim().is_empty() {
        return;
    }

    let store = KbStore::new(db);
    let kbs = match store.list().await {
        Ok(kbs) if !kbs.is_empty() => kbs,
        _ => return,
    };

    let mut all_rules = Vec::new();
    for kb in &kbs {
        if let Ok(rules) = store.list_rules(&kb.id).await {
            all_rules.extend(rules);
        }
    }

    let combined = format!("{query}\n\n{synthesis}");
    let mut targets = apply_keyword_rules(&combined, &all_rules);

    if targets.is_empty() {
        let mut samples: Vec<(String, Vec<String>)> = Vec::new();
        for kb in &kbs {
            let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
            let contents: Vec<String> = entries.into_iter().map(|e| e.content).collect();
            if !contents.is_empty() {
                samples.push((kb.id.clone(), contents));
            }
        }
        targets = apply_similarity_routing(&combined, &samples, SIMILARITY_THRESHOLD);
    }

    let source_ref = session_id.map(|id| format!("research-session-{id}"));

    for (kb_id, confidence) in targets {
        let effective_confidence = confidence.max(0.95);
        let _ = store
            .add_entry(
                &kb_id,
                synthesis,
                KbEntrySource::Research,
                source_ref.as_deref(),
                effective_confidence,
                &[],
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_compiles() {}
}
```

- [ ] **Step 6.5: Implement `signal_scientia.rs`**

```rust
//! Scientia finding promotion adapter — ingests approved findings into matching KBs.

use std::sync::Arc;

use vox_db::VoxDb;
use vox_orchestrator::knowledge_base::{
    router::{apply_keyword_rules, apply_similarity_routing, SIMILARITY_THRESHOLD},
    store::KbStore,
    types::KbEntrySource,
};

/// Ingest an approved Scientia finding.
/// Scientia-approved findings always get confidence 0.98 (high trust).
pub async fn ingest_scientia_finding(db: Arc<VoxDb>, finding_text: &str, finding_id: &str) {
    if finding_text.trim().is_empty() {
        return;
    }

    let store = KbStore::new(db);
    let kbs = match store.list().await {
        Ok(kbs) if !kbs.is_empty() => kbs,
        _ => return,
    };

    let mut all_rules = Vec::new();
    for kb in &kbs {
        if let Ok(rules) = store.list_rules(&kb.id).await {
            all_rules.extend(rules);
        }
    }

    let mut targets = apply_keyword_rules(finding_text, &all_rules);
    if targets.is_empty() {
        let mut samples: Vec<(String, Vec<String>)> = Vec::new();
        for kb in &kbs {
            let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
            let contents: Vec<String> = entries.into_iter().map(|e| e.content).collect();
            if !contents.is_empty() {
                samples.push((kb.id.clone(), contents));
            }
        }
        targets = apply_similarity_routing(finding_text, &samples, SIMILARITY_THRESHOLD);
    }

    for (kb_id, _) in targets {
        let _ = store
            .add_entry(
                &kb_id,
                finding_text,
                KbEntrySource::Scientia,
                Some(finding_id),
                0.98,
                &[],
            )
            .await;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn module_compiles() {}
}
```

- [ ] **Step 6.6: Hook chat signal adapter into `message.rs`**

Open `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs`.

Find the area where `response_text` is assembled (line 312: `let (response_text, model_used, tokens) = ...`) and look for the `Ok(...)` return at the end of the handler. Add the fire-and-forget spawn **between** the response assembly and the return:

```rust
// KB signal adapter: fire-and-forget after response is assembled
if let Some(db) = state.db.clone() {
    let content_for_kb = response_text.clone();
    let session_ref = session_id.clone(); // session_id: String (from line 263)
    tokio::spawn(async move {
        crate::kb::signal_chat::ingest_chat_turn(
            db,
            &content_for_kb,
            Some(session_ref.as_str()),
        )
        .await;
    });
}
```

> ⚠️ **Variable names:** `response_text` is the assistant response string (line 312). `session_id` is a `String` from `normalize_chat_session_id()` (line 263). These are the actual variable names in the file — do NOT use `.as_deref()` since `session_id` is not `Option<String>`.

- [ ] **Step 6.7: Run tests**

```powershell
cargo test -p vox-orchestrator-mcp signal_chat
cargo build -p vox-orchestrator-mcp
```

Expected: 5 signal_chat tests pass, clean build.

- [ ] **Step 6.8: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/kb_tools/signal_chat.rs crates/vox-orchestrator-mcp/src/kb_tools/signal_research.rs crates/vox-orchestrator-mcp/src/kb_tools/signal_scientia.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs
git commit -m "feat(vox-orchestrator-mcp): add KB signal adapters (chat, research, scientia)"
```

---

## Task 7: KB Hits in Retrieval Bundle + @kb-name Injection

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs`
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs`

- [ ] **Step 7.1: Write failing tests for `@kb-name` parser**

Add to `retrieval.rs` in the existing `#[cfg(test)]` block:

```rust
    #[test]
    fn parse_kb_mentions_extracts_at_names() {
        let mentions = parse_kb_mentions("Can you check @rust-patterns and @async-guide?");
        assert_eq!(mentions, vec!["rust-patterns", "async-guide"]);
    }

    #[test]
    fn parse_kb_mentions_no_mentions() {
        let mentions = parse_kb_mentions("No mentions here.");
        assert!(mentions.is_empty());
    }

    #[test]
    fn parse_kb_mentions_deduplicates() {
        let mentions = parse_kb_mentions("See @retrieval and @retrieval again.");
        assert_eq!(mentions, vec!["retrieval"]);
    }

    #[test]
    fn parse_kb_mentions_strips_trailing_punctuation() {
        let mentions = parse_kb_mentions("Use @rust-patterns, and @async-guide.");
        assert_eq!(mentions, vec!["rust-patterns", "async-guide"]);
    }
```

- [ ] **Step 7.2: Run to confirm failure**

```powershell
cargo test -p vox-orchestrator-mcp kb_mention
```

Expected: compile error — `parse_kb_mentions` not found.

- [ ] **Step 7.3: Add `kb_lines` to `RetrievalBundle` and implement `parse_kb_mentions`**

In `retrieval.rs`, find `pub struct RetrievalBundle` (line 222). Add the field after `rrf_fused_lines`:

```rust
    /// Knowledge base hits from topic-matched KB entries.
    // NOTE: No #[serde(...)] — RetrievalBundle does NOT derive Serialize/Deserialize.
    pub kb_lines: Vec<String>,
```

After the `RetrievalBundle` struct definition, add the helper function:

```rust
/// Extract `@name` mentions from a user message.
/// Returns deduplicated, lowercased names in order of first appearance.
pub fn parse_kb_mentions(text: &str) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for word in text.split_whitespace() {
        if let Some(name) = word.strip_prefix('@') {
            // Strip trailing punctuation (commas, periods, question marks, etc.)
            let name =
                name.trim_end_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_');
            if !name.is_empty() {
                let lower = name.to_ascii_lowercase();
                if !seen.contains(&lower) {
                    seen.insert(lower.clone());
                    result.push(lower);
                }
            }
        }
    }
    result
}
```

- [ ] **Step 7.4: Run tests**

```powershell
cargo test -p vox-orchestrator-mcp kb_mention
```

Expected: `test result: ok. 4 passed`

- [ ] **Step 7.5: Wire KB hits into `run_retrieval_bundle()`**

In `run_retrieval_bundle()`, after the existing retrieval completes and **before** the `Ok(RetrievalBundle { ... })` construction at line 326, add:

```rust
// KB background enrichment: search accepted entries for this query
let kb_lines = if let Some(db) = &state.db {
    use vox_orchestrator::knowledge_base::store::KbStore;
    let store = KbStore::new(db.clone());
    store
        .search_entries(query, 5)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|e| {
            format!("[KB:{} via {}] {}", e.kb_id, e.source_signal, e.content)
        })
        .collect()
} else {
    Vec::new()
};
```

Then add `kb_lines` to the `RetrievalBundle { ... }` construction (find the existing struct literal and add the field):

```rust
        // ... existing fields ...
        rrf_fused_lines: execution.rrf_fused_lines,
        kb_lines,  // ← add this
    })
```

- [ ] **Step 7.6: Inject @kb-name content in `message.rs`**

In the chat message handler, find where context is assembled for the LLM call. Before that assembly, add:

```rust
// @kb-name precision recall: resolve @mentions in the user message
let kb_mention_lines = if let Some(db) = state.db.clone() {
    use vox_orchestrator::knowledge_base::store::KbStore;
    use crate::memory_tools::retrieval::parse_kb_mentions;
    let mentioned_names = parse_kb_mentions(&user_message_text);
    if mentioned_names.is_empty() {
        Vec::new()
    } else {
        let store = KbStore::new(db);
        let kbs = store.list().await.unwrap_or_default();
        let mut lines = Vec::new();
        for name in &mentioned_names {
            if let Some(kb) = kbs.iter().find(|k| {
                k.name.to_ascii_lowercase() == *name
            }) {
                let entries = store.list_entries(&kb.id, 10, 0).await.unwrap_or_default();
                for e in entries {
                    lines.push(format!("[KB:{}] {}", kb.name, e.content));
                }
            }
        }
        lines
    }
} else {
    Vec::new()
};
```

Include `kb_mention_lines` in the context preamble alongside the retrieval bundle lines. Find where `bundle.memory_lines`, `bundle.knowledge_lines`, etc. are assembled and add `kb_mention_lines` to the same collection.

- [ ] **Step 7.7: Build and run tests**

```powershell
cargo build -p vox-orchestrator-mcp
cargo test -p vox-orchestrator-mcp
```

Expected: clean build, all existing tests pass.

- [ ] **Step 7.8: Commit**

```powershell
cargo fmt -p vox-orchestrator-mcp
git add crates/vox-orchestrator-mcp/src/memory_tools/retrieval.rs crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs
git commit -m "feat(vox-orchestrator-mcp): add KB hits to retrieval bundle and @kb-name mention injection"
```

---

## Task 8: MENS KbSignals Stage

**Files:**
- Modify: `crates/vox-ml-cli/src/commands/mens/populi/action_prelude.rs` ← **ENUM lives here**
- Modify: `crates/vox-ml-cli/src/commands/mens/pipeline.rs` ← **stage execution lives here**

> ⚠️ **Two-file edit:** `PipelineStage` is defined in `action_prelude.rs`. The match arm runs in `pipeline.rs`. Both must be edited.

- [ ] **Step 8.1: Add `KbSignals` variant to `PipelineStage` enum in `action_prelude.rs`**

In `crates/vox-ml-cli/src/commands/mens/populi/action_prelude.rs`:

Find the `PipelineStage` enum (line 104). Add after `Train` (line 130):
```rust
    /// Export accepted/rejected KB entries as training/DPO signals.
    KbSignals,
```

Find the `as_str()` match (line 136). Add after `Self::Train => "train"` (line 149):
```rust
            Self::KbSignals => "kb_signals",
```

- [ ] **Step 8.2: Add `KbSignals` to `all_possible_stages` array and match arm in `pipeline.rs`**

In `crates/vox-ml-cli/src/commands/mens/pipeline.rs`:

Add to `all_possible_stages` array (before `PipelineStage::Train`):
```rust
        PipelineStage::KbSignals,
```

Add a match arm in the main dispatch loop (after the `ReviewEvalPackBuild` arm, before `Pairs`):
```rust
            PipelineStage::KbSignals => {
                if !dry_run {
                    run_kb_signals_stage(&data_dir).await?;
                } else {
                    tracing::info!("KbSignals: dry_run, skipping");
                }
            }
```

- [ ] **Step 8.3: Implement `run_kb_signals_stage` as an `async fn` in `pipeline.rs`**

> ⚠️ **`pipeline::run()` is `async fn`** — do NOT create a new `tokio::Runtime` inside. This function MUST be `async fn`.

> ⚠️ **`VoxDb::open()` is `#[cfg(feature = "local")]` gated.** Verify `vox-ml-cli/Cargo.toml` has `vox-db = { workspace = true, features = ["local"] }` before proceeding. If not, add it.

> ⚠️ **`mix.yaml` controls mix sources** — there is no hardcoded file list in `pipeline.rs`. The KB signals file will be picked up automatically if placed in `mens/data/mix_sources/` AND your `mix.yaml` scans that directory (or includes `kb_signals.jsonl` explicitly). Verify your `mix.yaml` configuration after this step.

Add this function to `pipeline.rs` (after the existing helper functions, before the closing of the module):

```rust
async fn run_kb_signals_stage(data_dir: &std::path::Path) -> anyhow::Result<()> {
    use std::collections::HashMap;
    use std::io::{BufWriter, Write};

    let db_path = ".vox/db/vox.db";
    tracing::info!("KbSignals: connecting to VoxDb at {db_path}");

    // VoxDb::open is #[cfg(feature = "local")] — ensure vox-ml-cli/Cargo.toml has
    // vox-db with features = ["local"]
    let db = vox_db::VoxDb::open(db_path)
        .await
        .map_err(|e| anyhow::anyhow!("KbSignals: failed to open VoxDb at {db_path}: {e}"))?;

    let entries = db
        .kb_unqueued_training_entries(10_000)
        .await
        .map_err(|e| anyhow::anyhow!("KbSignals: query failed: {e}"))?;

    if entries.is_empty() {
        tracing::info!("KbSignals: no unqueued entries; skipping");
        return Ok(());
    }

    let out_dir = std::path::PathBuf::from("mens/data/mix_sources");
    std::fs::create_dir_all(&out_dir)?;
    let out_path = out_dir.join("kb_signals.jsonl");
    let file = std::fs::File::create(&out_path)?;
    let mut writer = BufWriter::new(file);
    let mut written = 0usize;

    // Group by kb_id to pair accepted/rejected within the same KB
    let mut by_kb: HashMap<String, Vec<_>> = HashMap::new();
    for entry in &entries {
        by_kb.entry(entry.kb_id.clone()).or_default().push(entry);
    }

    for (kb_id, kb_entries) in &by_kb {
        let accepted: Vec<_> = kb_entries.iter().filter(|e| e.accepted == 1).collect();
        let rejected: Vec<_> = kb_entries.iter().filter(|e| e.accepted == 0).collect();

        // Accepted → SFT instruction-completion pairs
        for entry in &accepted {
            let record = serde_json::json!({
                "type": "sft",
                "source": "kb",
                "kb_id": kb_id,
                "instruction": format!(
                    "What do you know about the following topic based on accumulated research?\n\nTopic: {}",
                    entry.source_signal
                ),
                "completion": entry.content,
                "routing_confidence": entry.routing_confidence,
            });
            writeln!(writer, "{record}")?;
            written += 1;
        }

        // Accepted + rejected same-signal pairs → DPO preference pairs
        for acc in &accepted {
            for rej in &rejected {
                if acc.source_signal == rej.source_signal {
                    let record = serde_json::json!({
                        "type": "dpo",
                        "source": "kb",
                        "prompt": "Provide accurate technical information:",
                        "chosen": acc.content,
                        "rejected": rej.content,
                    });
                    writeln!(writer, "{record}")?;
                    written += 1;
                }
            }
        }
    }

    writer.flush()?;
    tracing::info!("KbSignals: wrote {written} records to {}", out_path.display());

    // Mark all fetched entries as MENS-queued to avoid re-export
    let ids: Vec<String> = entries.iter().map(|e| e.id.clone()).collect();
    db.kb_mark_mens_queued(&ids)
        .await
        .map_err(|e| anyhow::anyhow!("KbSignals: mark_queued failed: {e}"))?;
    tracing::info!("KbSignals: marked {} entries as mens_queued", ids.len());

    Ok(())
}
```

- [ ] **Step 8.4: Build**

```powershell
cargo build -p vox-ml-cli
```

Expected: clean build. If `VoxDb::open` fails, check the `local` feature gate.

- [ ] **Step 8.5: Commit**

```powershell
cargo fmt -p vox-ml-cli
git add crates/vox-ml-cli/src/commands/mens/populi/action_prelude.rs crates/vox-ml-cli/src/commands/mens/pipeline.rs
git commit -m "feat(vox-ml-cli): add KbSignals MENS stage (SFT from accepted, DPO from rejected entries)"
```

---

## Verification Plan

### Automated Tests

```powershell
# Task 1 — schema
cargo test -p vox-db baseline_policy_matches_compiled_schema

# Tasks 2-4 — types, store, router
cargo test -p vox-orchestrator knowledge_base

# Tasks 5-7 — MCP handlers, signal adapters, retrieval
cargo test -p vox-orchestrator-mcp signal_chat
cargo test -p vox-orchestrator-mcp kb_mention
cargo test -p vox-orchestrator-mcp

# Task 8 — build check (no unit tests for the async MENS stage)
cargo build -p vox-ml-cli

# Full workspace check
cargo build -p vox-orchestrator
cargo build -p vox-orchestrator-mcp
```

### Manual Verification

1. Start the MCP server
2. Call `vox_kb_create` with `{"name": "test-kb", "description": "Test"}` — expect KB object with ID
3. Call `vox_kb_add_rule` with `{"kb_id": "<id>", "pattern": "rust", "rule_type": "keyword"}` — expect rule object
4. Send a chat message containing "rust" — wait for turn to complete
5. Call `vox_kb_get_feed` — expect a new entry with `source_signal: "chat"`
6. Call `vox_kb_query` with `{"query": "rust"}` — expect the entry
7. Call `vox_kb_review_entry` with `{"entry_id": "<id>", "accepted": false}` — expect `"rejected"`
8. Run `cargo run -p vox-ml-cli -- mens pipeline --stages kb_signals` — expect `kb_signals.jsonl` in `mens/data/mix_sources/`

### Verify `mix.yaml` includes KB signals

After running the MENS stage, check your `mix.yaml` to confirm it will pick up `kb_signals.jsonl`. If your config lists sources explicitly, add an entry. If it scans `mix_sources/` by directory, no change is needed.

---

## Bug Fixes Applied Relative to Earlier Draft

This plan corrects all issues found during code review:

| Issue | Fix Applied |
|---|---|
| `fastrand` not in deps | Replaced with `uuid::Uuid::new_v4().to_string()` |
| `now_ms()` reinvented | Uses `crate::now_unix_ms()` from vox-orchestrator |
| `PipelineStage` in wrong file | Step 8.1 correctly targets `action_prelude.rs` |
| `tokio::Runtime::new()` inside async | `run_kb_signals_stage` is now `async fn` |
| `PipelineConfig` doesn't exist | Replaced with `data_dir: &Path` |
| `impl crate::VoxDb` placement | Explicitly instructs to append a new separate impl block after EOF |
| `#[serde(default)]` on non-serde struct | Removed from `kb_lines` field |
| `session_id: String` (not Option) | Fixed to `Some(session_id.as_str())` in chat hook |
| `kb_unqueued_mens_entries` misleadingly named | Renamed to `kb_unqueued_training_entries` with corrected doc |
| `kb_clip` ignored `params.tags` | Fixed to parse tags and pass to `add_entry` |
| No `vox_kb_delete_entry` tool | Added as both VoxDb op, KbStore method, and MCP handler |
| `entry_count` drifts on entry add | Added `BEGIN`/`COMMIT` transaction wrapper |
| Mix stage has no hardcoded file list | Step 8.3 note explains `mix.yaml` is the control point |
| Row type ordering bug | Step 3 reordered: lib.rs first, ops_memory.rs second |
| No content deduplication | `kb_find_duplicate()` added; `KbStore::add_entry` checks before insert |
| No staleness schema hooks | `last_accessed_at_ms`, `access_count` columns added to `kb_entries` |
