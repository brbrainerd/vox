# Agent Operation Capture Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record every MCP tool call as a redacted, size-capped row in vox-db — a best-effort, off-the-hot-path signal that later sub-projects mine to suggest skills.

**Architecture:** Extract the existing regex redactor into a leaf `vox-redact` crate and add a JSON-args scrubber; add an `agent_operations` table + a turso `record_operation`/`prune_operations` store method; add a default-on config flag; and emit a fire-and-forget capture from the EXISTING post-dispatch block in `handle_tool_call` (no wrapper needed — the values are already in scope there).

**Tech Stack:** Rust (`vox-redact` new crate, `vox-db` turso store, `vox-orchestrator` config, `vox-orchestrator-mcp` dispatch), `regex`, `serde_json`, `tokio`.

**Spec:** `docs/superpowers/specs/2026-06-29-agent-operation-capture-design.md`

**Sub-project 1 of 4** (capture → mine → propose → author/install). This plan ships capture only; it produces working, testable software on its own.

---

## Codebase facts — VERIFIED 2026-06-29 (trust these; do not re-derive)

| Fact | Verified value |
|---|---|
| Capture point | `handle_tool_call` (`vox-orchestrator-mcp/src/dispatch.rs:30`) has a post-dispatch block (~287–396) where `name_canonical`, `args`, `result`, `duration_ms`, `agent_id`, `session_id` are in scope, inside `if let Some(db) = &state.db {` (~352). Add capture there; do NOT wrap `handle_tool_call_inner` (would miss guard semantics/timing). |
| ids at dispatch | `agent_id`/`session_id` come from `args.get("...")` → `Option<&str>`, often `None` → store NULL. `ServerState` has no current-session field. |
| Store | `vox_db::VoxDb`; turso `params![]` (`use turso::params;`). VERIFIED idiom: `let breaker = self.breaker.clone(); let conn = self.conn.clone(); breaker.call(|| async move { conn.execute(...).await?; Ok::<_,StoreError>(conn.last_insert_rowid()) }).await`. Own data before the `move`; bind `Option<&str>` via `.as_deref()`; no-param queries use `()`. No writer branch needed. `StoreError` = `crate::store::types::StoreError`. `ServerState.db: Option<Arc<vox_db::VoxDb>>`. |
| Test harness | `VoxDb::connect(DbConfig::Memory).await` (`use crate::{DbConfig, VoxDb};`), schema auto-migrated. `DbConfig::Memory` is `#[cfg(feature="local")]` → run vox-db tests with `--features local`. No `open_in_memory`/path-string/`test_db()` exists. |
| Schema | Tables are `CREATE TABLE IF NOT EXISTS` lines inside `pub const SCHEMA_AGENTS: &str` (`vox-db/src/schema/domains/agents.rs`). Adding a table = append there + bump `BASELINE_VERSION` (`vox-db/src/schema/manifest.rs:15`, 80→81) + ledger comment + update `contracts/db/baseline-version-policy.yaml` (integer + digest), gated by `vox ci check-codex-ssot`. No fragment registration. NOTE: archive-dedup plan also targets 81 → land-second rebases to 82. |
| Config | `OrchestratorConfig` (`vox-orchestrator/src/config/orchestrator_fields.rs:16`), `#[serde(deny_unknown_fields, default)]`. Bool-default: `#[serde(default = "default_true")] pub field: bool` (`default_true` already in scope via `use super::defaults::*;`). **`Default` is hand-rolled with NO `..` spread (`config/impl_default.rs`)** — a new field MUST be added there or it won't compile. Reachable as `state.orchestrator_config.<field>`. No config serialization golden breaks. |
| Redactor | `vox-terminal-core/src/corpus/redact.rs` — `pub fn redact_owned(&str)->String`, deps = `regex` + std only. Callers: `corpus/writer.rs:9` (import) + 6 call sites; `corpus/mod.rs:10` (re-export). |

---

## File Structure

- **Create** `crates/vox-redact/{Cargo.toml, src/lib.rs}` — leaf crate: `redact_owned` (moved verbatim) + new `redact_args`. Deps: `regex`, `serde_json`.
- **Modify** root `Cargo.toml` — add `vox-redact` to `[workspace] members` and `[workspace.dependencies]`.
- **Modify** `crates/vox-terminal-core/{Cargo.toml, src/corpus/redact.rs, src/corpus/writer.rs, src/corpus/mod.rs}` — depend on `vox-redact`, repoint callers, drop the moved code.
- **Modify** `crates/vox-db/src/schema/domains/agents.rs` — append `agent_operations` table + indexes to `SCHEMA_AGENTS`.
- **Modify** `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` 80→81 + ledger line.
- **Modify** `contracts/db/baseline-version-policy.yaml` — `repository_baseline_integer: 81` + new baseline digest (gated by `vox ci check-codex-ssot`).
- **Modify** `crates/vox-db/src/store/ops_agents.rs` — add `record_operation` + `prune_operations`.
- **Modify** `crates/vox-orchestrator/src/config/orchestrator_fields.rs` — add `operations_capture_enabled: bool` (default true).
- **Modify** `crates/vox-orchestrator/src/config/impl_default.rs` — add the new field to the hand-rolled `Default` (no `..` spread exists).
- **Create** `crates/vox-orchestrator-mcp/src/operation_capture.rs` — `spawn_capture(...)` fire-and-forget helper.
- **Modify** `crates/vox-orchestrator-mcp/src/{lib.rs, dispatch.rs, Cargo.toml}` — register module, call `spawn_capture` in the post-dispatch block, add `vox-redact` dep.

## Execution notes

- **TDD** is strict for the pure logic (Task 1 `redact_args`, Task 2 store round-trip). Tasks 3–4 (config field, dispatch wiring) are integration glue verified by build + a flag test.
- **Dependency order:** Task 1 (redact) and Task 2 (db) are independent and could be done in parallel by separate workers; Task 3 (config) is independent; Task 4 (wiring) depends on 1+2+3. Serialize writes to any one file.
- Commit after each task.

---

## Task 1: Extract `vox-redact` crate + `redact_args`

**Files:**
- Create: `crates/vox-redact/Cargo.toml`, `crates/vox-redact/src/lib.rs`
- Modify: root `Cargo.toml`; `crates/vox-terminal-core/Cargo.toml`, `crates/vox-terminal-core/src/corpus/redact.rs`, `.../corpus/writer.rs`, `.../corpus/mod.rs`

- [ ] **Step 1: Create the crate manifest**

`crates/vox-redact/Cargo.toml` (VERIFIED: workspace edition is `2024`; mirror `edition.workspace = true` as `vox-orchestrator-mcp` does):

```toml
[package]
name = "vox-redact"
version = "0.6.0"
edition.workspace = true

[dependencies]
regex = { workspace = true }
serde_json = { workspace = true }
```

`regex`/`serde_json` are already workspace deps.

- [ ] **Step 2: Register in the workspace**

VERIFIED: the root `Cargo.toml` `[workspace] members` is a glob (`members = ["crates/*", "crates/workspace-hack"]`), so `crates/vox-redact` is auto-included — do NOT add it to `members`. Add ONLY the workspace dependency entry under `[workspace.dependencies]`:

```toml
vox-redact = { path = "crates/vox-redact" }
```

- [ ] **Step 3: Write `lib.rs` with `redact_owned` (moved) + `redact_args` + failing tests**

`crates/vox-redact/src/lib.rs` — copy the four regexes + `redact_owned` VERBATIM from `crates/vox-terminal-core/src/corpus/redact.rs`, then add `redact_args` and tests:

```rust
//! Conservative PII/secret redaction. Moved from vox-terminal-core so non-terminal
//! crates (operation capture) can reuse it without a backwards dependency edge.

use regex::Regex;
use std::sync::OnceLock;

static RE_EMAIL: OnceLock<Regex> = OnceLock::new();
static RE_API_KEY: OnceLock<Regex> = OnceLock::new();
static RE_IPV4: OnceLock<Regex> = OnceLock::new();
static RE_HOME: OnceLock<Regex> = OnceLock::new();

fn re_email() -> &'static Regex {
    RE_EMAIL.get_or_init(|| Regex::new(r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}").unwrap())
}
fn re_api_key() -> &'static Regex {
    RE_API_KEY.get_or_init(|| {
        Regex::new(r"(?i)(?:bearer\s+|api[_-]?key[=:]\s*|token[=:]\s*)[A-Za-z0-9+/\-_]{32,}").unwrap()
    })
}
fn re_ipv4() -> &'static Regex {
    RE_IPV4.get_or_init(|| {
        Regex::new(r"\b(?:127|10|192\.168|172\.(?:1[6-9]|2\d|3[01]))\.\d{1,3}\.\d{1,3}(?:\.\d{1,3})?\b").unwrap()
    })
}
fn re_home() -> &'static Regex {
    RE_HOME.get_or_init(|| Regex::new(r"(?:/home/[^/\s]+|[A-Za-z]:\\Users\\[^\\]+)").unwrap())
}

/// Redact PII/secret patterns in free text. Conservative: unknown patterns pass through.
pub fn redact_owned(text: &str) -> String {
    let s = re_email().replace_all(text, "[REDACTED_EMAIL]");
    let s = re_api_key().replace_all(&s, "[REDACTED_KEY]");
    let s = re_ipv4().replace_all(&s, "[REDACTED_IP]");
    let s = re_home().replace_all(&s, "~[REDACTED_PATH]");
    s.into_owned()
}

// ponytail: denylist is intentionally over-broad (substring match). Over-redaction
// is the safe failure mode for secrets — e.g. "author" matches "auth". The mining
// sub-project tolerates a few redacted fields; a leaked token is unacceptable.
fn key_is_secret(key: &str) -> bool {
    const DENY: &[&str] = &[
        "token", "key", "secret", "password", "passwd", "authorization", "auth",
        "credential", "apikey", "bearer", "cookie", "session",
    ];
    let k = key.to_ascii_lowercase();
    DENY.iter().any(|d| k.contains(d))
}

/// Recursively redact a JSON value: values under secret-ish keys become
/// "[REDACTED]"; all other string scalars are run through `redact_owned`.
pub fn redact_args(value: &serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match value {
        Value::Object(map) => {
            let mut out = serde_json::Map::with_capacity(map.len());
            for (k, v) in map {
                if key_is_secret(k) {
                    out.insert(k.clone(), Value::String("[REDACTED]".into()));
                } else {
                    out.insert(k.clone(), redact_args(v));
                }
            }
            Value::Object(out)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(redact_args).collect()),
        Value::String(s) => Value::String(redact_owned(s)),
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn redacts_value_under_secret_key() {
        let out = redact_args(&json!({ "api_key": "abc", "Authorization": "Bearer z" }));
        assert_eq!(out["api_key"], json!("[REDACTED]"));
        assert_eq!(out["Authorization"], json!("[REDACTED]"));
    }

    #[test]
    fn redacts_secret_pattern_in_nonsecret_field() {
        let out = redact_args(&json!({ "note": "use api_key= ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789" }));
        assert!(out["note"].as_str().unwrap().contains("[REDACTED_KEY]"), "got {out}");
    }

    #[test]
    fn recurses_objects_and_arrays_preserves_plain_values() {
        let out = redact_args(&json!({
            "outer": { "password": "p" },
            "list": ["alice@example.com", "ok"],
            "count": 3
        }));
        assert_eq!(out["outer"]["password"], json!("[REDACTED]"));
        assert!(out["list"][0].as_str().unwrap().contains("[REDACTED_EMAIL]"));
        assert_eq!(out["list"][1], json!("ok"));
        assert_eq!(out["count"], json!(3));
    }

    #[test]
    fn email_and_api_key_redacted() {
        assert!(redact_owned("contact alice@example.com").contains("[REDACTED_EMAIL]"));
        assert!(redact_owned("Authorization: Bearer sk-abcdefghijklmnopqrstuvwxyz123456").contains("[REDACTED_KEY]"));
    }
}
```

> Also copy any other existing tests from the old `redact.rs` (e.g. the IPv4/home tests) so coverage is preserved.

- [ ] **Step 4: Run the new crate's tests**

Run: `cargo test -p vox-redact`
Expected: PASS (all redaction tests).

- [ ] **Step 5: Repoint `vox-terminal-core` and delete the moved code**

In `crates/vox-terminal-core/Cargo.toml` `[dependencies]` add: `vox-redact = { workspace = true }`.

In `crates/vox-terminal-core/src/corpus/redact.rs`, delete the four regex helpers and the `redact_owned` fn, replacing them with a re-export — but KEEP the existing `#[cfg(test)] mod tests` block (its `use super::*;` now resolves `redact_owned` to the re-exported `vox_redact` one, giving a cross-crate behavior check for free):

```rust
//! Redaction moved to the `vox-redact` crate; re-exported here for existing paths.
pub use vox_redact::redact_owned;

#[cfg(test)]
mod tests {
    use super::*;
    // ... keep the existing email_redacted / api_key_redacted / loopback_ip_redacted
    // / clean_text_unchanged tests verbatim — they now exercise vox_redact::redact_owned.
}
```

`corpus/mod.rs:10` (`pub use redact::redact_owned;`) and `corpus/writer.rs:9` (`use super::redact::redact_owned;`) both keep resolving through the re-export — no call-site edits. (These tests pass only if `vox-redact` uses the same `[REDACTED_EMAIL]`/`[REDACTED_KEY]`/`[REDACTED_IP]` strings, which Step 3 moved verbatim.)

- [ ] **Step 6: Build + test terminal-core**

Run: `cargo test -p vox-terminal-core 2>&1 | grep -E "test result|error" | head`
Expected: builds; redaction/writer tests still pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-redact crates/vox-terminal-core/Cargo.toml crates/vox-terminal-core/src/corpus Cargo.toml
git commit -m "refactor(redact): extract vox-redact crate + add redact_args JSON scrubber"
```

---

## Task 2: `agent_operations` table + store methods

**Files:**
- Modify: `crates/vox-db/src/schema/domains/agents.rs`, `crates/vox-db/src/schema/manifest.rs`, `crates/vox-db/src/store/ops_agents.rs`

- [ ] **Step 1: Add the table to the schema**

In `crates/vox-db/src/schema/domains/agents.rs`, append inside the `SCHEMA_AGENTS` string constant (next to the other `CREATE TABLE IF NOT EXISTS` blocks):

```sql
CREATE TABLE IF NOT EXISTS agent_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    ts_ms INTEGER NOT NULL,
    session_id TEXT,
    agent_id TEXT,
    tool_name TEXT NOT NULL,
    args_redacted TEXT NOT NULL,
    result_redacted TEXT,
    duration_ms INTEGER,
    is_error INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_agent_operations_session ON agent_operations(session_id, ts_ms);
CREATE INDEX IF NOT EXISTS idx_agent_operations_tool ON agent_operations(tool_name);
```

- [ ] **Step 2: Bump the schema baseline version + the gated SSOT contract**

> PRE-CHECK (collision): run `git grep -n "BASELINE_VERSION: i64" -- crates/vox-db` — confirm it is still `80`. Another uncommitted plan (archive-dedup) also targets 81; if 81 is already taken on your base, use the next free integer and adjust every `81` below to match.

In `crates/vox-db/src/schema/manifest.rs`, change `pub const BASELINE_VERSION: i64 = 80;` to `81`, adding a ledger comment line above it (mirror the existing `79:`/`80:` lines):

```rust
// 81: feat(capture): add agent_operations table (operation capture sub-project 1)
pub const BASELINE_VERSION: i64 = 81;
```

Then update the gated SSOT contract `contracts/db/baseline-version-policy.yaml`: set `repository_baseline_integer: 81`. The baseline digest there must match the recomputed `schema_baseline_digest_hex` — run the gate (next step) to get the expected digest and paste it in. This contract is enforced by `vox ci check-codex-ssot`; skipping it fails CI.

- [ ] **Step 3: Write the failing store test**

In `crates/vox-db/src/store/ops_agents.rs`, add to the existing `#[cfg(test)] mod spend_tests` (or a new `#[cfg(test)] mod tests`). VERIFIED harness: tests build the db with `VoxDb::connect(DbConfig::Memory).await` (imports `use crate::{DbConfig, VoxDb};`), schema auto-migrated by `connect`. `DbConfig::Memory` is gated behind the `local` feature, so this test runs under `--features local`.

```rust
#[tokio::test]
async fn record_and_prune_operations_roundtrip() {
    use crate::{DbConfig, VoxDb};
    let db = VoxDb::connect(DbConfig::Memory).await.expect("open db");

    let id = db
        .record_operation(
            Some("sess-1"),
            None, // agent_id NULL
            "vox_skill_list",
            r#"{"q":"[REDACTED]"}"#,
            Some("ok"),
            12,
            false,
        )
        .await
        .expect("record");
    assert!(id > 0);

    // prune must not error on a small table and must keep the fresh row.
    db.prune_operations().await.expect("prune");
}
```

- [ ] **Step 4: Run it to verify it fails**

Run: `cargo test -p vox-db --features local record_and_prune_operations_roundtrip`
Expected: FAIL — `no method named record_operation`.

- [ ] **Step 5: Implement the store methods**

In `crates/vox-db/src/store/ops_agents.rs` (same `impl crate::VoxDb` block as `record_agent_event`). VERIFIED idiom: clone `self.breaker` AND `self.conn` into locals, move owned data into a `|| async move {}` closure, `breaker.call(...).await`; bind `Option<&str>` via `.as_deref()` on an owned `Option<String>` (binds NULL for `None`); bare `Option<i64>`/`i64` bind directly; no-param queries use `()`. Omit the writer-actor branch (legitimate — `record_llm_outcome` has none). `use turso::params;` and `use crate::store::types::StoreError;` are already in this file.

```rust
/// Record one (already-redacted) tool-call operation. Best-effort capture signal.
pub async fn record_operation(
    &self,
    session_id: Option<&str>,
    agent_id: Option<&str>,
    tool_name: &str,
    args_redacted: &str,
    result_redacted: Option<&str>,
    duration_ms: i64,
    is_error: bool,
) -> Result<i64, StoreError> {
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    // Own everything before the `move` closure (mirrors record_agent_event).
    let session_id = session_id.map(str::to_string);
    let agent_id = agent_id.map(str::to_string);
    let tool_name = tool_name.to_string();
    let args_redacted = args_redacted.to_string();
    let result_redacted = result_redacted.map(str::to_string);
    let breaker = self.breaker.clone();
    let conn = self.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "INSERT INTO agent_operations
                   (ts_ms, session_id, agent_id, tool_name, args_redacted, result_redacted, duration_ms, is_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    ts_ms,
                    session_id.as_deref(),
                    agent_id.as_deref(),
                    tool_name.as_str(),
                    args_redacted.as_str(),
                    result_redacted.as_deref(),
                    duration_ms,
                    is_error as i64,
                ],
            )
            .await?;
            Ok::<i64, StoreError>(conn.last_insert_rowid())
        })
        .await
}

/// Bound table growth: drop rows older than 30 days, then trim to the newest 50k.
pub async fn prune_operations(&self) -> Result<(), StoreError> {
    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
        - 30 * 24 * 60 * 60 * 1000;
    let breaker = self.breaker.clone();
    let conn = self.conn.clone();
    breaker
        .call(|| async move {
            conn.execute(
                "DELETE FROM agent_operations WHERE ts_ms < ?1",
                params![cutoff_ms],
            )
            .await?;
            conn.execute(
                "DELETE FROM agent_operations WHERE id NOT IN
                   (SELECT id FROM agent_operations ORDER BY id DESC LIMIT 50000)",
                (),
            )
            .await?;
            Ok::<(), StoreError>(())
        })
        .await
}
```

- [ ] **Step 6: Run the store test + schema gate**

Run: `cargo test -p vox-db --features local record_and_prune_operations_roundtrip && cargo test -p vox-db --features local baseline_digest_policy`
Expected: round-trip PASS; `baseline_digest_policy` PASS — if it fails with a digest mismatch, it prints the expected digest; paste that into `contracts/db/baseline-version-policy.yaml` and re-run. Then `vox ci check-codex-ssot` must be clean.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-db/src/schema crates/vox-db/src/store/ops_agents.rs contracts/db/baseline-version-policy.yaml
git commit -m "feat(db): agent_operations table + record_operation/prune_operations (schema v81)"
```

---

## Task 3: Config flag

**Files:**
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs`
- Modify: `crates/vox-orchestrator/src/config/impl_default.rs`

- [ ] **Step 1: Add the field**

In `crates/vox-orchestrator/src/config/orchestrator_fields.rs`, inside the `OrchestratorConfig` struct, add (mirroring `behavioral_gate_on_complete` at line ~40). VERIFIED: `default_true` is already in scope via `use super::defaults::*;` — no new import:

```rust
    /// Local, redacted capture of every MCP tool call into `agent_operations`
    /// (signal for skill suggestion). On by default; set false to disable.
    #[serde(default = "default_true")]
    pub operations_capture_enabled: bool,
```

- [ ] **Step 2: Update the hand-rolled `Default` impl (REQUIRED — it has no `..` spread)**

VERIFIED: `crates/vox-orchestrator/src/config/impl_default.rs` lists every field explicitly with NO `..Default::default()` spread, so the struct will NOT compile until the new field is added. Add (mirror line ~25 `behavioral_gate_on_complete: default_true(),`):

```rust
            operations_capture_enabled: default_true(),
```

(`for_testing()` in `impl_env.rs` ends with `..Default::default()`, so it needs no change.)

- [ ] **Step 3: Build + verify default**

Run: `cargo test -p vox-orchestrator 2>&1 | grep -E "test result|error\[" | head`
Expected: builds; `config_serialization_roundtrip` and the catalog tests still pass (they are tolerant — `>= 50` field count, no exact serialization golden).

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator/src/config/orchestrator_fields.rs crates/vox-orchestrator/src/config/impl_default.rs
git commit -m "feat(config): operations_capture_enabled flag (default on)"
```

---

## Task 4: Fire-and-forget capture in dispatch

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/operation_capture.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs`, `crates/vox-orchestrator-mcp/src/dispatch.rs`, `crates/vox-orchestrator-mcp/Cargo.toml`

- [ ] **Step 1: Add the `vox-redact` dependency**

In `crates/vox-orchestrator-mcp/Cargo.toml` `[dependencies]`: `vox-redact = { workspace = true }`. (`vox-db`, `vox-config`, `tokio`, `serde_json`, `tracing` are already present.)

- [ ] **Step 2: Write the capture helper + its test**

Create `crates/vox-orchestrator-mcp/src/operation_capture.rs`:

```rust
//! Fire-and-forget capture of one tool call into `agent_operations`. Redaction and
//! the DB write happen on a spawned task, so the dispatch path is never blocked;
//! every error is swallowed (capture is best-effort and must not affect results).

use std::sync::Arc;
use vox_db::VoxDb;

const MAX_FIELD: usize = 8 * 1024;

fn cap(mut s: String) -> String {
    if s.len() > MAX_FIELD {
        s.truncate(MAX_FIELD);
        s.push_str("…[truncated]");
    }
    s
}

/// Spawn a best-effort capture. No-op when disabled or when there is no DB.
#[allow(clippy::too_many_arguments)]
pub fn spawn_capture(
    db: Option<Arc<VoxDb>>,
    enabled: bool,
    tool_name: String,
    args: serde_json::Value,
    result: String,
    session_id: Option<String>,
    agent_id: Option<String>,
    duration_ms: i64,
    is_error: bool,
) {
    if !enabled {
        return;
    }
    let Some(db) = db else {
        return;
    };
    tokio::spawn(async move {
        let args_redacted = cap(vox_redact::redact_args(&args).to_string());
        let result_redacted = cap(vox_redact::redact_owned(&result));
        match db
            .record_operation(
                session_id.as_deref(),
                agent_id.as_deref(),
                &tool_name,
                &args_redacted,
                Some(result_redacted.as_str()),
                duration_ms,
                is_error,
            )
            .await
        {
            // ponytail: prune on a 1-in-500 cadence — the row-count trim runs a
            // subquery, so don't pay it on every tool call.
            Ok(rowid) => {
                if rowid % 500 == 0 {
                    let _ = db.prune_operations().await;
                }
            }
            Err(e) => tracing::debug!(error = %e, "operation capture failed (ignored)"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cap_truncates_oversized() {
        let big = "x".repeat(MAX_FIELD + 100);
        let out = cap(big);
        assert!(out.len() <= MAX_FIELD + "…[truncated]".len());
        assert!(out.ends_with("[truncated]"));
    }

    #[tokio::test]
    async fn disabled_is_noop() {
        // enabled=false returns immediately without touching the (None) db.
        spawn_capture(None, false, "t".into(), serde_json::json!({}), "r".into(), None, None, 1, false);
    }
}
```

- [ ] **Step 3: Register the module**

In `crates/vox-orchestrator-mcp/src/lib.rs`, add alongside the other `pub mod` lines: `pub mod operation_capture;`

- [ ] **Step 4: Run the helper tests**

Run: `cargo test -p vox-orchestrator-mcp operation_capture`
Expected: PASS.

- [ ] **Step 5: Emit capture from the post-dispatch block**

VERIFIED in `crates/vox-orchestrator-mcp/src/dispatch.rs`: at the post-dispatch site, `let duration_ms = start_time.elapsed().as_millis() as i64;` is at line ~287; `result` is `Result<String, anyhow::Error>`; `name_canonical: &str`, `args: serde_json::Value` (still owned — the inner call got a clone), `agent_id`/`session_id` are `Option<&str>`; `state.db` is `Option<Arc<VoxDb>>`. The guard-rejection early returns happen BEFORE the inner dispatch (≤ line 187), so a call placed here runs ONLY for executed tools — exactly the intent.

Insert this ONE call immediately AFTER line 287 (`let duration_ms = ...;`), before the telemetry block:

```rust
    crate::operation_capture::spawn_capture(
        state.db.clone(),
        state.orchestrator_config.operations_capture_enabled,
        name_canonical.to_string(),
        args.clone(),
        match &result {
            Ok(s) => s.clone(),
            Err(e) => e.to_string(),
        },
        session_id.map(|s| s.to_string()),
        agent_id.map(|s| s.to_string()),
        duration_ms,
        result.is_err(),
    );
```

`is_error = result.is_err()` reflects a transport-level error; tool-level failures encoded as `Ok(json{success:false})` are still captured (their body is in `args`/`result`), which is the right signal for mining. Do NOT recompute `duration_ms` or move `result`/`args` — `args.clone()` and `&result` borrow without disturbing the existing telemetry block below.

- [ ] **Step 6: Build the crate**

Run: `cargo build -p vox-orchestrator-mcp 2>&1 | tail -3`
Expected: `Finished` with no errors.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/operation_capture.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat(mcp): fire-and-forget agent operation capture in dispatch"
```

---

## Final verification

- [ ] **Step 1: Touched crates build + test**

Run: `cargo test -p vox-redact -p vox-orchestrator -p vox-orchestrator-mcp 2>&1 | grep -E "test result|error\[" | head -30`
then `cargo test -p vox-db --features local 2>&1 | grep -E "test result|error\[" | head -30`
Expected: all `test result: ok`; no `error[...]`. (vox-db tests need `--features local` for `DbConfig::Memory`.)

- [ ] **Step 2: terminal-core still green (redact move)**

Run: `cargo test -p vox-terminal-core 2>&1 | grep -E "test result|error" | head`
Expected: green — the in-file `redact.rs` tests resolve `redact_owned` through the re-export to `vox_redact` (same `[REDACTED_*]` strings), so they still pass.

- [ ] **Step 3: Schema integrity + SSOT gate**

Run: `cargo test -p vox-db --features local baseline_digest_policy 2>&1 | grep -E "test result|FAILED" | head`
then `vox ci check-codex-ssot`
Expected: digest test passes against `BASELINE_VERSION = 81`; the SSOT gate is clean (confirms `contracts/db/baseline-version-policy.yaml` was updated to match).

- [ ] **Step 4: Clippy on touched crates**

Run: `cargo clippy -p vox-redact -p vox-db -p vox-orchestrator-mcp -- -D warnings`
Expected: no warnings. (Per project policy: per-crate, never `--all-targets` across the workspace; exclude `vox-gui`.)
