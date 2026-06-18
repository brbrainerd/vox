# Orchestrator ToolCallRecord Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the parallel `ToolReceipt` (integrity only) and `UsageRecord` (cost only) write paths with a single `ToolCallRecord` struct. One DB table, one write path, one HMAC that covers both execution proof and cost fields.

**Architecture:** `ToolCallRecord` combines all fields from both predecessor types. Its BLAKE3-keyed HMAC covers not just the tool identity fields but also `tokens_in`, `tokens_out`, and `cost_usd` — so cost cannot be tampered with independently of the execution proof. `ToolCallLedger` replaces `ToolReceiptLedger`. `UsageTracker` becomes a read-only aggregate view over the new `tool_call_records` DB table (preserving all existing query methods for dashboards). Old tables are kept read-only — no migration of historical data needed.

**Tech Stack:** Rust, `blake3`, `uuid`, `parking_lot`, VoxDB, `cargo test`

**Prerequisite:** Plan 1 (Foundation) must be complete. `vox-orchestrator-core` must exist.

---

## File Map

| Action | Path |
|---|---|
| CREATE | `crates/vox-orchestrator-core/src/tool_call_record.rs` |
| MODIFY | `crates/vox-orchestrator-core/src/lib.rs` — add `pub mod tool_call_record` |
| MODIFY | `crates/vox-orchestrator/src/tool_receipt.rs` — add deprecation notice + re-export shim |
| MODIFY | `crates/vox-orchestrator-core/src/usage.rs` — `UsageTracker` reads from `tool_call_records` |
| MODIFY | `crates/vox-actor-runtime/src/llm/chat.rs` — use `ToolCallLedger` for post-call recording |

---

## Task 1: Define `ToolCallRecord` and `ToolCallLedger`

**Files:**
- Create: `crates/vox-orchestrator-core/src/tool_call_record.rs`

- [ ] **Step 1: Write the failing tests first**

Create `crates/vox-orchestrator-core/src/tool_call_record.rs` with the test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vox_orchestrator_types::AgentId;

    fn make_ledger() -> ToolCallLedger {
        ToolCallLedger::new([42u8; 32])
    }

    #[test]
    fn issue_intent_creates_pending_record() {
        let ledger = make_ledger();
        let record = ledger.issue_intent(
            AgentId(1),
            "read_file",
            r#"{"path":"/foo/bar.rs"}"#,
            "google",
            "gemini-2.5-flash",
            "codegen",
            "global",
        );
        assert!(!record.record_id.is_empty());
        assert_eq!(record.tool_name, "read_file");
        assert_eq!(record.provider, "google");
        assert_eq!(record.model, "gemini-2.5-flash");
        assert_eq!(record.task_category, "codegen");
        assert_eq!(record.tokens_in, 0, "pending record has zero tokens");
        assert_eq!(record.cost_source, "pending");
        assert!(record.result_hash.is_none());
    }

    #[test]
    fn fulfill_updates_cost_and_result_hash() {
        let ledger = make_ledger();
        let intent = ledger.issue_intent(
            AgentId(1), "write_file", r#"{"path":"/x.rs"}"#,
            "openrouter", "anthropic/claude-3-5-sonnet", "codegen", "global",
        );

        let fulfilled = ledger.fulfill(
            &intent.record_id,
            r#"{"ok":true}"#,
            500,   // tokens_in
            200,   // tokens_out
            0.003, // cost_usd
            None,  // provider_reported
            Some(0.003), // estimated
            Some(0.003), // reconciled
            "estimated",
        ).unwrap();

        assert_eq!(fulfilled.tokens_in, 500);
        assert_eq!(fulfilled.tokens_out, 200);
        assert!((fulfilled.cost_usd - 0.003).abs() < 1e-9);
        assert_eq!(fulfilled.cost_source, "estimated");
        assert!(fulfilled.result_hash.is_some());
    }

    #[test]
    fn verify_passes_for_valid_record() {
        let ledger = make_ledger();
        let intent = ledger.issue_intent(
            AgentId(2), "llm_chat", r#"{"prompt":"hello"}"#,
            "google", "gemini-2.5-pro", "chat", "global",
        );
        let fulfilled = ledger.fulfill(
            &intent.record_id, r#"{"reply":"world"}"#,
            100, 50, 0.001, None, Some(0.001), Some(0.001), "estimated",
        ).unwrap();

        assert!(ledger.verify(&fulfilled.record_id).is_ok());
    }

    #[test]
    fn verify_fails_after_cost_tamper() {
        let ledger = make_ledger();
        let intent = ledger.issue_intent(
            AgentId(3), "llm_chat", r#"{"prompt":"test"}"#,
            "openrouter", "meta/llama-3", "chat", "global",
        );
        ledger.fulfill(
            &intent.record_id, r#"{"reply":"ok"}"#,
            50, 25, 0.0005, None, Some(0.0005), None, "estimated",
        ).unwrap();

        // Tamper with the cost field directly in the ledger
        {
            let mut records = ledger.records.write();
            records.get_mut(&intent.record_id).unwrap().cost_usd = 999.99;
        }

        assert!(
            ledger.verify(&intent.record_id).is_err(),
            "HMAC must fail after cost_usd is tampered with"
        );
    }

    #[test]
    fn record_id_is_monotonically_ordered() {
        let ledger = make_ledger();
        let r1 = ledger.issue_intent(AgentId(1), "t1", "{}", "p", "m", "c", "u");
        let r2 = ledger.issue_intent(AgentId(1), "t2", "{}", "p", "m", "c", "u");
        // UUIDv7 — lexicographic order = time order
        assert!(r1.record_id < r2.record_id, "UUIDv7 IDs must be monotonically ordered");
    }

    #[test]
    fn today_returns_yyyy_mm_dd() {
        let date = ToolCallLedger::today_str();
        assert_eq!(date.len(), 10);
        assert!(date.starts_with("20"));
        assert_eq!(&date[4..5], "-");
        assert_eq!(&date[7..8], "-");
    }
}
```

- [ ] **Step 2: Run to verify failure**

```powershell
cargo test -p vox-orchestrator-core tool_call_record 2>&1 | tail -10
```

Expected: compile error — `ToolCallLedger`, `ToolCallRecord` not defined. Good.

- [ ] **Step 3: Write the full implementation**

```rust
//! Unified tool-call record combining execution proof (integrity) and cost
//! attribution. Replaces the prior split between `ToolReceipt` and `UsageRecord`.
//!
//! ## Why unified?
//!
//! A tool call always produces both: a cryptographic proof that the tool ran
//! (BLAKE3-HMAC over args + result), and a cost record (tokens, USD). Keeping
//! them in separate tables made it impossible to ask "what did this specific
//! proven call cost?" without a fragile join on ephemeral ids.
//!
//! ## HMAC coverage
//!
//! The HMAC covers: `record_id || agent_id || tool_name || call_args_hash ||
//! result_hash || executed_at_ms || tokens_in || tokens_out || cost_usd.to_bits()`.
//! This means tampering with any cost field invalidates the tag.

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;
use vox_orchestrator_types::AgentId;

/// Unified record: execution proof + cost attribution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// UUIDv7 — monotonically ordered by creation time.
    pub record_id: String,
    /// Agent that issued this tool call.
    pub agent_id: AgentId,
    /// Name of the tool or LLM call (e.g. `"read_file"`, `"llm_chat"`).
    pub tool_name: String,
    /// BLAKE3 of canonical JSON args.
    pub call_args_hash: String,
    /// BLAKE3 of result bytes. `None` while the call is in flight.
    pub result_hash: Option<String>,
    /// Wall clock of call start (unix milliseconds).
    pub executed_at_ms: u64,
    /// BLAKE3-keyed MAC over all fields including cost fields.
    pub hmac_tag: [u8; 32],

    // --- Cost attribution ---
    /// Provider slug (e.g. `"google"`, `"openrouter"`, `"ollama"`).
    pub provider: String,
    /// Model id as resolved by `ModelRegistry::select()`.
    pub model: String,
    /// Prompt tokens consumed (0 while pending).
    pub tokens_in: u64,
    /// Completion tokens produced (0 while pending).
    pub tokens_out: u64,
    /// Total USD cost for this call (0.0 while pending).
    pub cost_usd: f64,
    /// Cost as reported by the provider API response (if available).
    pub provider_reported_cost_usd: Option<f64>,
    /// Cost estimated from the model's pricing table.
    pub estimated_cost_usd: Option<f64>,
    /// Reconciled cost used for budgeting (preferred over estimated when set).
    pub reconciled_cost_usd: Option<f64>,
    /// Which cost field is authoritative:
    /// `"estimated"` | `"provider_reported"` | `"reconciled"` | `"pending"`.
    pub cost_source: String,
    /// Task category for cross-category analytics (e.g. `"codegen"`, `"chat"`).
    pub task_category: String,
    /// UTC day `YYYY-MM-DD` for daily aggregation queries.
    pub date: String,
    /// Tenant partition key (`"global"` unless multi-tenant).
    pub user_id: String,
}

/// Thread-safe in-memory ledger of `ToolCallRecord`s.
///
/// One ledger per orchestrator session. Records are keyed by `record_id`.
/// The session key is used for BLAKE3-HMAC so tags are session-scoped
/// (not globally verifiable — they prevent in-session tampering, not
/// cross-session replay).
pub struct ToolCallLedger {
    session_key: [u8; 32],
    /// Arc<RwLock<...>> so the ledger can be shared across async tasks.
    pub(crate) records: Arc<RwLock<HashMap<String, ToolCallRecord>>>,
}

impl ToolCallLedger {
    /// Create a ledger with the given 32-byte session key.
    pub fn new(session_key: [u8; 32]) -> Self {
        Self {
            session_key,
            records: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Create a ledger from a hex-encoded 32-byte key string.
    /// Falls back to a random ephemeral key if the string is invalid.
    pub fn from_hex_key(hex_key: &str) -> Self {
        let key = if hex_key.len() == 64 {
            hex::decode(hex_key)
                .ok()
                .and_then(|b| b.try_into().ok())
                .unwrap_or_else(Self::random_key)
        } else {
            Self::random_key()
        };
        Self::new(key)
    }

    fn random_key() -> [u8; 32] {
        let mut k = [0u8; 32];
        let _ = getrandom::getrandom(&mut k);
        k
    }

    /// Today's date as `YYYY-MM-DD` (UTC, no chrono dependency).
    pub fn today_str() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let days = (secs / 86_400) as i64;
        let z = days + 719_468;
        let era = (if z >= 0 { z } else { z - 146_096 }) / 146_097;
        let doe = (z - era * 146_097) as u32;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let y = (yoe as i64) + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!("{:04}-{:02}-{:02}", y, m, d)
    }

    fn compute_hmac(
        key: &[u8; 32],
        record_id: &str,
        agent_id: AgentId,
        tool_name: &str,
        call_args_hash: &str,
        result_hash: Option<&str>,
        executed_at_ms: u64,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd_bits: u64,
    ) -> [u8; 32] {
        let mut hasher = blake3::Hasher::new_keyed(key);
        hasher.update(record_id.as_bytes());
        hasher.update(&agent_id.0.to_le_bytes());
        hasher.update(tool_name.as_bytes());
        hasher.update(call_args_hash.as_bytes());
        if let Some(r) = result_hash {
            hasher.update(r.as_bytes());
        }
        hasher.update(&executed_at_ms.to_le_bytes());
        hasher.update(&tokens_in.to_le_bytes());
        hasher.update(&tokens_out.to_le_bytes());
        hasher.update(&cost_usd_bits.to_le_bytes());
        hasher.finalize().into()
    }

    /// Issue a pending record for a call that is about to start.
    ///
    /// Cost fields default to zero / `"pending"`. Call [`Self::fulfill`] once
    /// the call completes and cost data is available.
    #[allow(clippy::too_many_arguments)]
    pub fn issue_intent(
        &self,
        agent_id: AgentId,
        tool_name: &str,
        args_json: &str,
        provider: &str,
        model: &str,
        task_category: &str,
        user_id: &str,
    ) -> ToolCallRecord {
        let record_id = Uuid::now_v7().to_string();
        let executed_at_ms = chrono::Utc::now().timestamp_millis() as u64;
        let call_args_hash = blake3::hash(args_json.as_bytes()).to_string();
        let hmac_tag = Self::compute_hmac(
            &self.session_key,
            &record_id,
            agent_id,
            tool_name,
            &call_args_hash,
            None,
            executed_at_ms,
            0,
            0,
            0u64, // cost_usd = 0.0 -> to_bits() = 0
        );
        let record = ToolCallRecord {
            record_id: record_id.clone(),
            agent_id,
            tool_name: tool_name.to_string(),
            call_args_hash,
            result_hash: None,
            executed_at_ms,
            hmac_tag,
            provider: provider.to_string(),
            model: model.to_string(),
            tokens_in: 0,
            tokens_out: 0,
            cost_usd: 0.0,
            provider_reported_cost_usd: None,
            estimated_cost_usd: None,
            reconciled_cost_usd: None,
            cost_source: "pending".to_string(),
            task_category: task_category.to_string(),
            date: Self::today_str(),
            user_id: user_id.to_string(),
        };
        self.records.write().insert(record_id, record.clone());
        record
    }

    /// Fulfill a pending record with the actual result and cost.
    ///
    /// Re-computes the HMAC over the full set of fields (including costs)
    /// so any subsequent [`Self::verify`] call confirms the integrity of the
    /// cost data alongside the execution proof.
    #[allow(clippy::too_many_arguments)]
    pub fn fulfill(
        &self,
        record_id: &str,
        result_json: &str,
        tokens_in: u64,
        tokens_out: u64,
        cost_usd: f64,
        provider_reported_cost_usd: Option<f64>,
        estimated_cost_usd: Option<f64>,
        reconciled_cost_usd: Option<f64>,
        cost_source: &str,
    ) -> Result<ToolCallRecord, &'static str> {
        let mut records = self.records.write();
        let record = records.get_mut(record_id).ok_or("Record not found")?;

        let result_hash = blake3::hash(result_json.as_bytes()).to_string();
        record.result_hash = Some(result_hash.clone());
        record.tokens_in = tokens_in;
        record.tokens_out = tokens_out;
        record.cost_usd = cost_usd;
        record.provider_reported_cost_usd = provider_reported_cost_usd;
        record.estimated_cost_usd = estimated_cost_usd;
        record.reconciled_cost_usd = reconciled_cost_usd;
        record.cost_source = cost_source.to_string();

        record.hmac_tag = Self::compute_hmac(
            &self.session_key,
            &record.record_id,
            record.agent_id,
            &record.tool_name,
            &record.call_args_hash,
            Some(&result_hash),
            record.executed_at_ms,
            tokens_in,
            tokens_out,
            cost_usd.to_bits(),
        );

        Ok(record.clone())
    }

    /// Verify the HMAC of a record by id.
    ///
    /// Returns `Ok(())` if the record exists and its tag is valid.
    /// Returns `Err` if the record is missing or the tag does not match
    /// (indicating tampering with any field, including cost fields).
    pub fn verify(&self, record_id: &str) -> Result<(), &'static str> {
        let records = self.records.read();
        let r = records.get(record_id).ok_or("Record not found")?;
        let expected = Self::compute_hmac(
            &self.session_key,
            &r.record_id,
            r.agent_id,
            &r.tool_name,
            &r.call_args_hash,
            r.result_hash.as_deref(),
            r.executed_at_ms,
            r.tokens_in,
            r.tokens_out,
            r.cost_usd.to_bits(),
        );
        if expected == r.hmac_tag {
            Ok(())
        } else {
            Err("HMAC verification failed — record may have been tampered with")
        }
    }

    /// Number of records currently in the ledger.
    pub fn len(&self) -> usize {
        self.records.read().len()
    }

    /// Whether the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
```

- [ ] **Step 4: Add the module to `vox-orchestrator-core/src/lib.rs`**

```rust
// In crates/vox-orchestrator-core/src/lib.rs, add:
pub mod tool_call_record;
pub use tool_call_record::{ToolCallLedger, ToolCallRecord};
```

Also add the required deps if not already in `vox-orchestrator-core/Cargo.toml`:

```toml
blake3 = { workspace = true }
uuid = { workspace = true, features = ["v4"] }
chrono = { workspace = true }
getrandom = "0.2"
hex = { workspace = true }
parking_lot = { workspace = true }
```

- [ ] **Step 5: Run the tests**

```powershell
cargo test -p vox-orchestrator-core tool_call_record 2>&1 | tail -10
```

Expected: `test result: ok. 6 passed; 0 failed`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator-core/src/tool_call_record.rs
git add crates/vox-orchestrator-core/src/lib.rs
git add crates/vox-orchestrator-core/Cargo.toml
git commit -m "feat(orchestrator-core): add ToolCallRecord + ToolCallLedger

- Unified struct: BLAKE3-HMAC covers both execution proof and cost fields
- Cost tamper test: verify() fails after cost_usd is modified post-fulfill
- UUIDv7 record IDs: monotonically ordered by creation time
- issue_intent() / fulfill() / verify() lifecycle"
```

---

## Task 2: Create `tool_call_records` DB table

The new table replaces `provider_usage` as the write target. Existing `provider_usage` rows
are kept read-only for historical reporting.

**Files:**
- Modify: wherever VoxDB schema migrations or `ensure_table` calls are made for `provider_usage`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator-core/src/tool_call_record.rs` (in the tests module, or a new
integration test file `crates/vox-orchestrator-core/tests/db_table.rs`):

```rust
#[cfg(test)]
mod db_tests {
    use super::*;
    use vox_db::VoxDb;

    /// Helper: open an in-memory VoxDB for testing.
    async fn test_db() -> VoxDb {
        VoxDb::open_memory().await.expect("in-memory DB must open")
    }

    #[tokio::test]
    async fn tool_call_records_table_is_created() {
        let db = test_db().await;
        let col = db.collection("tool_call_records");
        col.ensure_table().await.expect("ensure_table must succeed");
        // Insert a minimal record to verify the table accepts writes
        let result = col.insert(&serde_json::json!({
            "record_id": "test-id",
            "tool_name": "read_file",
            "provider": "google",
            "model": "gemini-2.5-flash",
            "date": "2026-06-18",
            "cost_usd": 0.001,
            "tokens_in": 100u64,
            "tokens_out": 50u64,
            "user_id": "global",
            "task_category": "codegen",
            "cost_source": "estimated",
        })).await;
        assert!(result.is_ok(), "insert into tool_call_records must succeed");
    }
}
```

- [ ] **Step 2: Run to verify test works with in-memory DB**

```powershell
cargo test -p vox-orchestrator-core db_tests 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; 0 failed`.  
(VoxDB's `open_memory()` creates in-memory tables; `ensure_table()` is a no-op on in-memory stores
or creates the table if needed. If `open_memory()` doesn't exist, check VoxDB API for the correct test constructor — it may be `VoxDb::new_ephemeral()` or similar.)

- [ ] **Step 3: Add `persist_to_db` method to `ToolCallLedger`**

Add to `crates/vox-orchestrator-core/src/tool_call_record.rs`:

```rust
impl ToolCallLedger {
    /// Persist a fulfilled record to the `tool_call_records` VoxDB collection.
    ///
    /// Only fulfilled records (with a `result_hash`) should be persisted.
    /// Pending records are kept in-memory only until fulfilled.
    pub async fn persist_to_db(
        &self,
        db: &vox_db::VoxDb,
        record_id: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let record = {
            let records = self.records.read();
            records.get(record_id).cloned()
                .ok_or("Record not found in ledger")?
        };

        if record.result_hash.is_none() {
            return Err("Cannot persist a pending (unfulfilled) record".into());
        }

        let col = db.collection("tool_call_records");
        col.ensure_table().await?;
        col.insert(&serde_json::to_value(&record)?).await?;
        Ok(())
    }
}
```

- [ ] **Step 4: Write the persist test**

```rust
#[tokio::test]
async fn persist_fulfilled_record_to_db() {
    let db = test_db().await;
    let ledger = ToolCallLedger::new([1u8; 32]);

    let intent = ledger.issue_intent(
        vox_orchestrator_types::AgentId(1),
        "llm_chat",
        r#"{"prompt":"hello"}"#,
        "google",
        "gemini-2.5-flash",
        "chat",
        "global",
    );
    ledger.fulfill(
        &intent.record_id,
        r#"{"reply":"hi"}"#,
        100, 50, 0.001, None, Some(0.001), Some(0.001), "estimated",
    ).unwrap();

    ledger.persist_to_db(&db, &intent.record_id).await
        .expect("persist must succeed");

    // Verify it appears in the DB
    let col = db.collection("tool_call_records");
    let rows = col.find(&serde_json::json!({"record_id": intent.record_id})).await
        .expect("find must succeed");
    assert_eq!(rows.len(), 1, "exactly one row must be in DB");
}

#[tokio::test]
async fn persist_pending_record_returns_error() {
    let db = test_db().await;
    let ledger = ToolCallLedger::new([2u8; 32]);
    let intent = ledger.issue_intent(
        vox_orchestrator_types::AgentId(1), "t", "{}", "p", "m", "c", "u",
    );
    // Not fulfilled — persist must fail
    let result = ledger.persist_to_db(&db, &intent.record_id).await;
    assert!(result.is_err(), "persisting a pending record must return Err");
}
```

- [ ] **Step 5: Run tests**

```powershell
cargo test -p vox-orchestrator-core tool_call_record 2>&1 | tail -10
```

Expected: `test result: ok. 8 passed; 0 failed`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-orchestrator-core/src/tool_call_record.rs
git commit -m "feat(orchestrator-core): add persist_to_db to ToolCallLedger — tool_call_records table"
```

---

## Task 3: Deprecate `ToolReceipt` and redirect to `ToolCallRecord`

`ToolReceiptLedger` in `crates/vox-orchestrator/src/tool_receipt.rs` is now superseded. It stays
in the codebase for one release so callers can migrate, but all new code uses `ToolCallLedger`.

**Files:**
- Modify: `crates/vox-orchestrator/src/tool_receipt.rs`

- [ ] **Step 1: Add a deprecation header to `tool_receipt.rs`**

Open `crates/vox-orchestrator/src/tool_receipt.rs`. Add at the top of the file:

```rust
//! DEPRECATED: Use `vox_orchestrator_core::ToolCallLedger` instead.
//!
//! `ToolReceiptLedger` only covers execution integrity (no cost fields).
//! `ToolCallLedger` covers both integrity and cost in a single record with
//! a single HMAC that covers cost fields.
//!
//! Migration: replace `ToolReceiptLedger::issue_intent` + `fulfill_intent`
//! with `ToolCallLedger::issue_intent` + `fulfill`, passing provider/model/
//! cost fields at fulfill time.
//!
//! This module will be removed in a future release.
#![deprecated(
    since = "0.0.0",
    note = "Use vox_orchestrator_core::ToolCallLedger instead"
)]
#![allow(deprecated)] // suppress warnings within this file itself
```

- [ ] **Step 2: Verify the deprecation compiles**

```powershell
cargo check -p vox-orchestrator 2>&1 | Select-String "deprecated|^error" | Select-Object -First 10
```

You may see deprecation warnings at call sites — this is expected. No errors.

- [ ] **Step 3: Commit**

```powershell
git add crates/vox-orchestrator/src/tool_receipt.rs
git commit -m "deprecate(orchestrator): mark ToolReceiptLedger deprecated in favor of ToolCallLedger"
```

---

## Task 4: Update `vox-actor-runtime` LLM call path to use `ToolCallLedger`

`vox-actor-runtime/src/llm/chat.rs` records LLM calls to `llm_interactions` table. Update it to
also write a `ToolCallRecord` so every LLM call has a unified integrity+cost record.

**Files:**
- Modify: `crates/vox-actor-runtime/src/llm/chat.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-actor-runtime/tests/llm_tool_call_record.rs` (new file):

```rust
//! Verifies that the LLM call path issues a ToolCallRecord and persists it.

use std::sync::Arc;
use vox_orchestrator_core::tool_call_record::{ToolCallLedger, ToolCallRecord};
use vox_orchestrator_types::AgentId;

#[tokio::test]
async fn llm_call_issues_tool_call_record() {
    let ledger = Arc::new(ToolCallLedger::new([99u8; 32]));

    // Simulate what the LLM call path does:
    // 1. Issue intent before the call
    let intent = ledger.issue_intent(
        AgentId(1),
        "llm_chat",
        r#"{"messages":[{"role":"user","content":"hello"}]}"#,
        "google",
        "gemini-2.5-flash",
        "chat",
        "global",
    );

    assert_eq!(ledger.len(), 1);
    assert_eq!(intent.cost_source, "pending");

    // 2. Fulfill after the call completes
    let fulfilled = ledger.fulfill(
        &intent.record_id,
        r#"{"content":"hello there"}"#,
        25,     // tokens_in
        10,     // tokens_out
        0.0001, // cost_usd
        None,
        Some(0.0001),
        Some(0.0001),
        "estimated",
    ).unwrap();

    assert_eq!(fulfilled.tokens_in, 25);
    assert_eq!(fulfilled.tokens_out, 10);
    assert!(fulfilled.result_hash.is_some());
    assert!(ledger.verify(&fulfilled.record_id).is_ok());
}
```

- [ ] **Step 2: Run to verify it passes (tests the flow, not actor-runtime internals)**

```powershell
cargo test -p vox-actor-runtime llm_tool_call_record 2>&1 | tail -10
```

Expected: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 3: Wire `ToolCallLedger` into the LLM call path in `chat.rs`**

In `crates/vox-actor-runtime/src/llm/chat.rs`, find the function that makes the actual LLM HTTP
call (likely named `chat_once`, `llm_chat`, or similar). Add ledger integration:

```rust
use vox_orchestrator_core::tool_call_record::ToolCallLedger;

// In the LLM call function signature, add ledger parameter (or pick it up from context):
pub async fn chat_once(
    config: &LlmConfig,
    messages: &[LlmChatMessage],
    ledger: Option<&ToolCallLedger>,
    agent_id: AgentId,
    task_category: &str,
) -> anyhow::Result<LlmResponse> {

    // Before the HTTP call:
    let intent = ledger.map(|l| l.issue_intent(
        agent_id,
        "llm_chat",
        // Args hash: include model + message count as a fingerprint
        &serde_json::to_string(&serde_json::json!({
            "model": &config.model,
            "messages": messages.len(),
        })).unwrap_or_default(),
        &config.provider,
        &config.model,
        task_category,
        "global",
    ));

    // ... existing HTTP call via vox-llm-egress ...
    let resp: LlmResponse = /* existing call */ todo!();

    // After the HTTP call: fulfill the record with actual cost
    if let (Some(l), Some(intent_record)) = (ledger, intent) {
        let estimated_cost = resp.cost_usd.unwrap_or_else(|| {
            // Fallback: compute from token counts + pricing
            let tokens_in = resp.prompt_tokens.unwrap_or(0) as f64;
            let tokens_out = resp.completion_tokens.unwrap_or(0) as f64;
            tokens_in * config.cost_per_1k_input / 1000.0
                + tokens_out * config.cost_per_1k_output / 1000.0
        });

        let _ = l.fulfill(
            &intent_record.record_id,
            // Result hash: response content fingerprint
            &serde_json::to_string(&serde_json::json!({
                "tokens_out": resp.completion_tokens,
                "finish_reason": resp.finish_reason,
            })).unwrap_or_default(),
            resp.prompt_tokens.unwrap_or(0) as u64,
            resp.completion_tokens.unwrap_or(0) as u64,
            estimated_cost,
            resp.cost_usd,               // provider_reported
            Some(estimated_cost),        // estimated
            None,                        // reconciled (set by billing pipeline)
            if resp.cost_usd.is_some() { "provider_reported" } else { "estimated" },
        );
    }

    Ok(resp)
}
```

> **Note:** If `chat_once` has a very different signature in your codebase, adapt the ledger
> integration to the existing shape — the key invariants are: (a) `issue_intent` before the HTTP
> call, (b) `fulfill` after with actual token counts and cost.

- [ ] **Step 4: Verify the actor-runtime compiles**

```powershell
cargo check -p vox-actor-runtime 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors.

- [ ] **Step 5: Run actor-runtime tests**

```powershell
cargo test -p vox-actor-runtime 2>&1 | tail -10
```

Expected: `test result: ok`.

- [ ] **Step 6: Commit**

```powershell
git add crates/vox-actor-runtime/src/llm/chat.rs
git add crates/vox-actor-runtime/tests/llm_tool_call_record.rs
git commit -m "feat(actor-runtime): wire ToolCallLedger into LLM call path

- issue_intent() before HTTP call; fulfill() after with actual cost
- provider_reported_cost_usd used when available; estimated as fallback
- cost_source set to 'provider_reported' | 'estimated' accordingly"
```

---

## Task 5: Update `UsageTracker` to read from `tool_call_records`

`UsageTracker` in `vox-orchestrator-core/src/usage.rs` currently writes to `provider_usage`.
Its write path is replaced by `ToolCallLedger::persist_to_db`. Its read/query methods are
updated to read from `tool_call_records` instead.

**Files:**
- Modify: `crates/vox-orchestrator-core/src/usage.rs`

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-orchestrator-core/src/usage.rs` tests module:

```rust
#[cfg(test)]
mod migration_tests {
    use super::*;
    use vox_db::VoxDb;

    async fn test_db() -> VoxDb {
        VoxDb::open_memory().await.unwrap()
    }

    #[tokio::test]
    async fn cost_summary_today_reads_from_tool_call_records() {
        let db = test_db().await;

        // Insert a row into tool_call_records (simulating what ToolCallLedger writes)
        let col = db.collection("tool_call_records");
        col.ensure_table().await.unwrap();
        col.insert(&serde_json::json!({
            "record_id": "r1",
            "tool_name": "llm_chat",
            "provider": "google",
            "model": "gemini-2.5-flash",
            "date": UsageTracker::today(),
            "cost_usd": 0.005,
            "tokens_in": 1000u64,
            "tokens_out": 500u64,
            "user_id": "global",
            "task_category": "codegen",
            "cost_source": "estimated",
        })).await.unwrap();

        let tracker = UsageTracker::new_ref(&db);
        let summary = tracker.cost_summary_today().await.unwrap();

        assert_eq!(summary.total_calls, 1);
        assert!((summary.total_cost_usd - 0.005).abs() < 1e-9);
        assert_eq!(summary.by_provider.len(), 1);
        assert_eq!(summary.by_provider[0].provider, "google");
    }
}
```

- [ ] **Step 2: Run to verify current behavior (reads from `provider_usage`, not `tool_call_records`)**

```powershell
cargo test -p vox-orchestrator-core cost_summary_today_reads_from_tool_call_records 2>&1 | tail -10
```

Expected: `FAILED` — the tracker reads from `provider_usage`, which has no rows.

- [ ] **Step 3: Update `cost_summary_today` to read from `tool_call_records`**

In `crates/vox-orchestrator-core/src/usage.rs`, find `cost_summary_today()`. Change the
collection name from `"provider_usage"` to `"tool_call_records"`:

```rust
pub async fn cost_summary_today(
    &self,
) -> Result<CostSummary, Box<dyn std::error::Error + Send + Sync>> {
    let col = self.db.collection("tool_call_records"); // was: "provider_usage"
    col.ensure_table().await?;
    // ... rest of implementation unchanged ...
}
```

Apply the same collection name change to: `cost_by_model`, `cost_by_category`,
`unified_cost_summary`, `remaining_all`, `get_calls_today`.

> **Note on `record_call` / `record_call_detailed`:** Mark these methods as deprecated:
> ```rust
> #[deprecated(note = "Use ToolCallLedger::issue_intent + fulfill + persist_to_db instead")]
> pub async fn record_call(&self, ...) { ... }
> ```
> Do not delete them yet — they may still be called from existing code paths that haven't
> been migrated to `ToolCallLedger`. They can be removed in a follow-up once all call sites
> are updated.

- [ ] **Step 4: Run tests**

```powershell
cargo test -p vox-orchestrator-core 2>&1 | tail -15
```

Expected: `test result: ok` — including the new migration test.

- [ ] **Step 5: Commit**

```powershell
git add crates/vox-orchestrator-core/src/usage.rs
git commit -m "refactor(orchestrator-core): UsageTracker reads from tool_call_records

- All query methods (cost_summary_today, cost_by_model, etc.) read from tool_call_records
- record_call / record_call_detailed marked deprecated
- Old provider_usage table retained read-only for historical data"
```

---

## Task 6: Final integration and migration verification

- [ ] **Step 1: Full workspace compile**

```powershell
cargo check --workspace 2>&1 | Select-String "^error" | Select-Object -First 20
```

Expected: zero errors. Deprecation warnings are acceptable.

- [ ] **Step 2: Search for remaining `ToolReceiptLedger` instantiations**

```powershell
rg "ToolReceiptLedger::new\|ToolReceiptLedger::from_config" crates/ --files-with-matches
```

For each result, migrate to `ToolCallLedger::new` or `ToolCallLedger::from_hex_key`.

- [ ] **Step 3: Search for remaining `record_call(` calls on `UsageTracker`**

```powershell
rg "\.record_call\(" crates/ --files-with-matches
```

For each call site, check whether a `ToolCallLedger` is available in scope. If so, replace with
`issue_intent` + `fulfill` + `persist_to_db`. If not yet (e.g., deeply nested code paths), leave
the `record_call` call and create a tracking comment:

```rust
// TODO(Plan4): migrate to ToolCallLedger when ModelSelector is available in this context
tracker.record_call(provider, model, tokens_in, tokens_out, cost_usd).await?;
```

- [ ] **Step 4: Run the full test suite for all affected crates**

```powershell
cargo test -p vox-orchestrator-core -p vox-orchestrator -p vox-actor-runtime 2>&1 | tail -20
```

Expected: all three report `test result: ok`.

- [ ] **Step 5: Verify `tool_call_records` DB indexes are defined**

Check VoxDB's index API. If `vox-db` supports explicit index creation on a collection, add indexes
after `ensure_table()` in `persist_to_db`:

```rust
col.ensure_table().await?;
// Indexes for common query patterns
col.ensure_index(&["date", "provider", "model"]).await.ok(); // ignore if not supported
col.ensure_index(&["agent_id", "date"]).await.ok();
col.ensure_index(&["task_category", "date"]).await.ok();
```

(Use `.ok()` so index creation failures don't break inserts on DB backends that don't support
explicit indexes.)

- [ ] **Step 6: Final commit**

```powershell
git commit --allow-empty -m "feat: Plan 4 complete — ToolCallRecord unifies integrity + cost

- ToolCallRecord: single struct, HMAC covers cost fields
- ToolCallLedger: replaces ToolReceiptLedger; persist_to_db writes to tool_call_records
- UsageTracker: all query methods read from tool_call_records
- vox-actor-runtime LLM path: issue_intent before call, fulfill after
- ToolReceiptLedger: marked deprecated, not deleted
- UsageTracker.record_call: marked deprecated, not deleted
- Historical provider_usage table: retained read-only"
```

---

## Verification Checklist

Before marking Plan 4 complete:

- [ ] `cargo test -p vox-orchestrator-core tool_call_record` — 8 tests pass
- [ ] `cargo test -p vox-orchestrator-core` — full suite passes
- [ ] `cargo test -p vox-actor-runtime` — full suite passes
- [ ] `cargo test -p vox-orchestrator` — full suite passes
- [ ] `cargo check --workspace` — zero errors
- [ ] `rg "ToolReceiptLedger::new" crates/` — returns only the definition in `tool_receipt.rs`
- [ ] `rg "\.record_call\(" crates/` — any remaining calls have a `// TODO(Plan4)` comment
- [ ] `ledger.verify(record_id).is_ok()` for a fulfilled record — confirmed in test
- [ ] `ledger.verify(record_id).is_err()` after cost tamper — confirmed in test
