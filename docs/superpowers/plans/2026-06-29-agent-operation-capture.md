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
| Store | `vox_db::VoxDb`; turso `params![]` (`use turso::params;`); writes via `self.breaker` + `self.conn.execute(...).await`; template = `record_agent_event` (`vox-db/src/store/ops_agents.rs:16`). Returns `Result<i64, StoreError>`. `ServerState.db: Option<Arc<vox_db::VoxDb>>`. |
| Schema | Tables are `CREATE TABLE IF NOT EXISTS` lines inside `pub const SCHEMA_AGENTS: &str` (`vox-db/src/schema/domains/agents.rs`). Adding a table = append there + bump `BASELINE_VERSION` (`vox-db/src/schema/manifest.rs:15`, 80→81) + ledger comment. Digest auto-recomputes; version test auto-passes. No fragment registration needed. |
| Config | `OrchestratorConfig` (`vox-orchestrator/src/config/orchestrator_fields.rs:16`), `#[serde(deny_unknown_fields, default)]`. Bool-default pattern: `#[serde(default = "default_true")] pub field: bool` (helpers in `config/defaults.rs`). Reachable as `state.orchestrator_config.<field>`. |
| Redactor | `vox-terminal-core/src/corpus/redact.rs` — `pub fn redact_owned(&str)->String`, deps = `regex` + std only. Callers: `corpus/writer.rs:9` (import) + 6 call sites; `corpus/mod.rs:10` (re-export). |

---

## File Structure

- **Create** `crates/vox-redact/{Cargo.toml, src/lib.rs}` — leaf crate: `redact_owned` (moved verbatim) + new `redact_args`. Deps: `regex`, `serde_json`.
- **Modify** root `Cargo.toml` — add `vox-redact` to `[workspace] members` and `[workspace.dependencies]`.
- **Modify** `crates/vox-terminal-core/{Cargo.toml, src/corpus/redact.rs, src/corpus/writer.rs, src/corpus/mod.rs}` — depend on `vox-redact`, repoint callers, drop the moved code.
- **Modify** `crates/vox-db/src/schema/domains/agents.rs` — append `agent_operations` table + indexes to `SCHEMA_AGENTS`.
- **Modify** `crates/vox-db/src/schema/manifest.rs` — bump `BASELINE_VERSION` 80→81 + ledger line.
- **Modify** `crates/vox-db/src/store/ops_agents.rs` — add `record_operation` + `prune_operations`.
- **Modify** `crates/vox-orchestrator/src/config/orchestrator_fields.rs` — add `operations_capture_enabled: bool` (default true).
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

`crates/vox-redact/Cargo.toml`:

```toml
[package]
name = "vox-redact"
version = "0.6.0"
edition = "2024"

[dependencies]
regex = { workspace = true }
serde_json = { workspace = true }
```

> Match `edition` to a sibling leaf crate if `2024` is rejected (copy the `[package]` block shape from `crates/vox-redact`'s nearest neighbor, e.g. another small crate). `regex`/`serde_json` are workspace deps already.

- [ ] **Step 2: Register in the workspace**

In the root `Cargo.toml`, add `"crates/vox-redact"` to `[workspace] members`, and under `[workspace.dependencies]` add:

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

Replace the body of `crates/vox-terminal-core/src/corpus/redact.rs` with a re-export (keeps the module path stable for any other reference):

```rust
//! Redaction moved to the `vox-redact` crate; re-exported here for existing paths.
pub use vox_redact::redact_owned;
```

`crates/vox-terminal-core/src/corpus/mod.rs:10` (`pub use redact::redact_owned;`) keeps working via the re-export — leave it. `corpus/writer.rs:9` (`use super::redact::redact_owned;`) also keeps working. No call-site edits needed.

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

- [ ] **Step 2: Bump the schema baseline version**

In `crates/vox-db/src/schema/manifest.rs`, change `pub const BASELINE_VERSION: i64 = 80;` to `81`, and add a ledger comment line above it mirroring the existing format:

```rust
// 81: feat(capture): add agent_operations table (operation capture sub-project 1)
pub const BASELINE_VERSION: i64 = 81;
```

- [ ] **Step 3: Write the failing store test**

In `crates/vox-db/src/store/ops_agents.rs`, locate the existing `#[cfg(test)] mod tests` (or the nearest test that builds a `VoxDb`) and copy its db-construction harness. Add:

```rust
#[tokio::test]
async fn record_and_prune_operations_roundtrip() {
    // Build a test VoxDb exactly as the sibling tests in this file do
    // (same in-memory/temp constructor + schema apply).
    let db = /* <copy the harness used by the other #[tokio::test] in this file> */;

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

Run: `cargo test -p vox-db record_and_prune_operations_roundtrip`
Expected: FAIL — `no method named record_operation`.

- [ ] **Step 5: Implement the store methods**

In `crates/vox-db/src/store/ops_agents.rs` (same `impl crate::VoxDb` block as `record_agent_event`), add — mirroring the existing `breaker` + `conn.execute` + `params![]` idiom:

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
    let conn = self.conn.clone();
    self.breaker
        .call(|| async {
            conn.execute(
                "INSERT INTO agent_operations
                   (ts_ms, session_id, agent_id, tool_name, args_redacted, result_redacted, duration_ms, is_error)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    ts_ms,
                    session_id,
                    agent_id,
                    tool_name,
                    args_redacted,
                    result_redacted,
                    duration_ms,
                    is_error as i64,
                ],
            )
            .await
        })
        .await?;
    Ok(conn.last_insert_rowid())
}

/// Bound table growth: drop rows older than 30 days, then trim to the newest 50k.
pub async fn prune_operations(&self) -> Result<(), StoreError> {
    let cutoff_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
        - 30 * 24 * 60 * 60 * 1000;
    let conn = self.conn.clone();
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
    Ok(())
}
```

> Mirror `record_agent_event` EXACTLY for the `breaker`/`conn`/`params!` shape — if it uses `self.breaker.clone()` or a different call form, match it. Confirm turso `params![]` binds `Option<&str>` as SQL NULL (it does for `None`); if the local turso version rejects `Option`, bind `session_id.unwrap_or_default()` is NOT acceptable (loses NULL) — instead use two query variants or `params![... session_id.map(|s| s.to_string()) ...]`. Prefer the `Option` binding; only adapt if the compiler rejects it.

- [ ] **Step 6: Run the store test**

Run: `cargo test -p vox-db record_and_prune_operations_roundtrip && cargo test -p vox-db schema_version`
Expected: PASS (round-trip works; the schema-version test passes against the bumped `BASELINE_VERSION`).

- [ ] **Step 7: Commit**

```bash
git add crates/vox-db/src/schema crates/vox-db/src/store/ops_agents.rs
git commit -m "feat(db): agent_operations table + record_operation/prune_operations (schema v81)"
```

---

## Task 3: Config flag

**Files:**
- Modify: `crates/vox-orchestrator/src/config/orchestrator_fields.rs`

- [ ] **Step 1: Add the field**

In `crates/vox-orchestrator/src/config/orchestrator_fields.rs`, inside the `OrchestratorConfig` struct, add (mirroring the `agentos_aci_envelope_enabled` field's `default_true` pattern):

```rust
    /// Local, redacted capture of every MCP tool call into `agent_operations`
    /// (signal for skill suggestion). On by default; set false to disable.
    #[serde(default = "default_true")]
    pub operations_capture_enabled: bool,
```

Confirm `default_true` is in scope in this file (it is used by sibling fields; if not imported, add `use` mirroring the other `default_*` usages).

- [ ] **Step 2: Build + verify default**

Run: `cargo test -p vox-orchestrator 2>&1 | grep -E "test result|error" | head`
Expected: builds; existing config (de)serialization tests pass (the new field defaults to `true` when absent, satisfying `#[serde(default)]`).

- [ ] **Step 3: Commit**

```bash
git add crates/vox-orchestrator/src/config/orchestrator_fields.rs
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

In `crates/vox-orchestrator-mcp/src/dispatch.rs`, inside `handle_tool_call`, at the existing post-dispatch site where `result`, `duration_ms`, `name_canonical`, `args`, `agent_id`, `session_id` are in scope (near the existing `if let Some(db) = &state.db {` around line 352), add ONE call. Place it so it runs for executed tool calls (after `duration_ms` is computed). Use the values already in scope:

```rust
        crate::operation_capture::spawn_capture(
            state.db.clone(),
            state.orchestrator_config.operations_capture_enabled,
            name_canonical.to_string(),
            args.clone(),
            // `result` is `Result<String, _>` here — capture the Ok body, else the error text.
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

> Adjust the `result`/`is_error` extraction to the ACTUAL local binding at that point: if `result` has already been unwrapped to a `String` envelope (the verified return shape encodes tool errors as `Ok(json)` with `success:false`), pass that string directly and derive `is_error` from `tool_json_envelope_is_error(&s)` (`server_state.rs:674`). Read the 5 lines around the existing telemetry insert and mirror exactly how it reads `result`/`duration_ms`/ids — do NOT introduce a second computation of any of them.

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

Run: `cargo test -p vox-redact -p vox-db -p vox-orchestrator -p vox-orchestrator-mcp 2>&1 | grep -E "test result|error\[" | head -30`
Expected: all `test result: ok`; no `error[...]`.

- [ ] **Step 2: terminal-core still green (redact move)**

Run: `cargo test -p vox-terminal-core 2>&1 | grep -E "test result|error" | head`
Expected: green.

- [ ] **Step 3: Schema integrity**

Run: `cargo test -p vox-db schema 2>&1 | grep -E "test result|FAILED" | head`
Expected: schema-version / digest tests pass against `BASELINE_VERSION = 81`.

- [ ] **Step 4: Clippy on touched crates**

Run: `cargo clippy -p vox-redact -p vox-db -p vox-orchestrator-mcp -- -D warnings`
Expected: no warnings. (Per project policy: per-crate, never `--all-targets` across the workspace; exclude `vox-gui`.)
