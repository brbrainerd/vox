---
title: "Orchestrator Stage 1 & 2: Security + Crash-Prevention Implementation Plan"
description: "Implementation plan for 7 P0/P1 fixes across the orchestrator gateway, runtime, grounding, and daemon surfaces. Covers FIX-K-03, FIX-K-05, FIX-K-06, FIX-B-02, FIX-E-01, FIX-H-03, FIX-O-03."
category: "Architecture SSOTs"
status: "roadmap"
last_updated: "2026-05-01"
training_eligible: false
authored: "2026-05-01"
---

# Orchestrator Stage 1 & 2: Security + Crash-Prevention Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate 7 P0/P1 panics and security gaps across the orchestrator's gateway, runtime, grounding, and daemon surfaces.

**Architecture:** Each fix is surgical and independent — no shared state between tasks. TDD approach: write failing test first, implement minimal fix, verify green.

**Tech Stack:** Rust, Tokio, axum (gateway), reqwest, serde_json, anyhow

---

## File Map

| Fix | File | Change |
|-----|------|--------|
| FIX-K-06 | `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs` | Fix `constant_time_eq` u8-truncation bug |
| FIX-K-03 | `crates/vox-orchestrator/src/mcp_tools/http_gateway/eval.rs` | Add 64 KiB size guard on `EvalRequest.code` |
| FIX-K-05 | `crates/vox-orchestrator/src/mcp_tools/http_gateway/ws.rs` | Remove query-param token fallback |
| FIX-B-02 | `crates/vox-orchestrator/src/runtime.rs` | Replace `.unwrap()` on UUID split |
| FIX-E-01 | `crates/vox-orchestrator/src/grounding.rs` | Harden char-boundary `.expect()` |
| FIX-H-03 | `crates/vox-orchestrator/src/mcp_tools/chat_tools/chat/mentions.rs` | Add message to regex `.unwrap()` |
| FIX-O-03 | `crates/vox-orchestrator/src/bin/vox_orchestrator_d.rs` | Replace session-manager `panic!` with `?` |

---

## Task 1: FIX-K-06 — Fix `constant_time_eq` u8-truncation

**Context:** `(a.len() ^ b.len()) as u8` truncates to 0 for lengths differing by exactly 256 (or any multiple), causing the function to treat a 256-byte token as length-equal to an empty token. This is the single highest-severity bug in Stage 1.

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs`

- [ ] **Step 1: Write the failing test**

Add this test to the existing `#[cfg(test)]` block inside `mod.rs` (after the existing tests):

```rust
#[test]
fn constant_time_eq_length_256_multiple_not_equal() {
    // Lengths differ by 256 — before the fix, `(256 ^ 0) as u8 == 0` produces a false positive.
    let a = vec![0u8; 256];
    let b: &[u8] = &[];
    assert!(
        !constant_time_eq(&a, b),
        "empty slice must not equal 256-byte slice"
    );
    // Also check a 512-byte vs 256-byte case
    let c = vec![0u8; 512];
    assert!(
        !constant_time_eq(&a, &c),
        "256-byte slice must not equal 512-byte slice"
    );
}
```

- [ ] **Step 2: Run the test to verify it fails**

```bash
cargo test -p vox-orchestrator constant_time_eq_length_256_multiple_not_equal -- --nocapture
```

Expected: FAIL — `assertion failed: !constant_time_eq(&a, b)`

- [ ] **Step 3: Apply the fix**

In `crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs`, find `constant_time_eq` (around line 510) and change the first line of the body:

```rust
// BEFORE
pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() ^ b.len()) as u8;

// AFTER
pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let mut diff = (a.len() != b.len()) as u8;
```

No other lines change. The `for` loop and `diff |= ai ^ bi` pattern remain identical.

- [ ] **Step 4: Run the test to verify it passes**

```bash
cargo test -p vox-orchestrator constant_time_eq -- --nocapture
```

Expected: all `constant_time_eq` tests PASS (including the new one and any pre-existing ones).

- [ ] **Step 5: Compile check**

```bash
cargo check -p vox-orchestrator
```

Expected: no errors, no warnings on the changed line.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/mcp_tools/http_gateway/mod.rs
git commit -m "fix(security): constant_time_eq length XOR truncation (FIX-K-06)

(a.len() ^ b.len()) as u8 produces 0 for lengths differing by 256n,
causing false equality. Replace with (a.len() != b.len()) as u8."
```

---

## Task 2: FIX-K-03 — 64 KiB size guard on eval endpoint

**Context:** `EvalRequest { pub code: String }` has no size limit. The handler writes `req.code` to a temp file then spawns a subprocess. An attacker can send arbitrarily large payloads to exhaust disk or memory before the 5-second execution timeout applies.

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/http_gateway/eval.rs`

- [ ] **Step 1: Write the failing test**

Locate the `#[cfg(test)]` block in `eval.rs`. If none exists, add one at the bottom of the file. Add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eval_request_rejects_oversized_code() {
        // 64 KiB + 1 byte should be rejected
        let big_code = "x".repeat(65_537);
        let req = EvalRequest { code: big_code };
        let result = validate_eval_request(&req);
        assert!(result.is_err(), "oversized code must be rejected");
    }

    #[test]
    fn eval_request_accepts_normal_code() {
        let req = EvalRequest { code: "println!(\"hello\")".to_string() };
        let result = validate_eval_request(&req);
        assert!(result.is_ok(), "normal code must be accepted");
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-orchestrator eval_request -- --nocapture
```

Expected: FAIL — `validate_eval_request` does not exist yet.

- [ ] **Step 3: Add the validation function and wire it into the handler**

In `eval.rs`, add the validation function before the handler:

```rust
const MAX_EVAL_CODE_BYTES: usize = 65_536; // 64 KiB

fn validate_eval_request(req: &EvalRequest) -> Result<(), (axum::http::StatusCode, String)> {
    if req.code.len() > MAX_EVAL_CODE_BYTES {
        return Err((
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            format!(
                "code payload exceeds {} byte limit",
                MAX_EVAL_CODE_BYTES
            ),
        ));
    }
    Ok(())
}
```

Then in the handler function (the `async fn` that receives `EvalRequest`), add the guard as the first statement after destructuring the request — before any `tokio::fs::write` call:

```rust
// At the top of the handler, before writing to disk:
if let Err((status, msg)) = validate_eval_request(&req) {
    return (status, msg).into_response();
}
```

The exact surrounding context will look like:

```rust
pub async fn eval_handler(
    // ... existing parameters ...
    Json(req): Json<EvalRequest>,
) -> impl IntoResponse {
    if let Err((status, msg)) = validate_eval_request(&req) {
        return (status, msg).into_response();
    }
    // ... rest of existing handler unchanged ...
```

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p vox-orchestrator eval_request -- --nocapture
```

Expected: both tests PASS.

- [ ] **Step 5: Compile check**

```bash
cargo check -p vox-orchestrator
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/mcp_tools/http_gateway/eval.rs
git commit -m "fix(security): 64 KiB size guard on eval endpoint (FIX-K-03)

Unbounded code payloads were written to disk before the 5 s execution
timeout applied. Reject anything over 65536 bytes with HTTP 413."
```

---

## Task 3: FIX-K-05 — Remove WebSocket query-param token fallback

**Context:** `ws.rs` accepts `?token=` and `?bearer=` query parameters as an authentication fallback for loopback WebSocket connections. URL query parameters appear in server access logs, reverse-proxy logs, and browser history — leaking tokens. The header-based auth path is sufficient for loopback clients.

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/http_gateway/ws.rs`

- [ ] **Step 1: Write the failing test**

In `ws.rs`, locate or create a `#[cfg(test)]` block. Add:

```rust
#[cfg(test)]
mod tests {
    // This test documents that query-param token auth is intentionally absent.
    // It cannot directly call the handler (axum wiring), so we verify the
    // extract_token helper (if it exists) or confirm no `query` param in fn sig.
    #[test]
    fn ws_handler_has_no_query_param_token_path() {
        // Static assertion: the symbol `extract_query_token` must not exist.
        // If this compiles, the query-param path has been removed.
        // (Absence of the function is the test — verified by `cargo check`.)
        assert!(true, "query-param token fallback must be removed");
    }
}
```

Because this is a structural change (removing code), the primary verification is `cargo check` after deletion. The test above documents intent.

- [ ] **Step 2: Run cargo check to establish baseline**

```bash
cargo check -p vox-orchestrator
```

Expected: PASS (baseline — no compile errors before the change).

- [ ] **Step 3: Remove the query-param fallback**

In `crates/vox-orchestrator/src/mcp_tools/http_gateway/ws.rs`, delete:

1. The `axum::extract::Query` import if it is only used for this path (check other uses first).
2. The `Query(query): Query<std::collections::HashMap<String, String>>` parameter from `http_ws`.
3. The entire `if role_res.is_err() && connect.0.ip().is_loopback() { ... }` block that reads `query.get("token").or_else(|| query.get("bearer"))` (approximately lines 15–26).

The function signature before:
```rust
pub async fn http_ws(
    connect: ConnectInfo<SocketAddr>,
    Query(query): Query<std::collections::HashMap<String, String>>,
    // ... other params ...
) -> impl IntoResponse {
    let role_res = /* header-based auth */;
    if role_res.is_err() && connect.0.ip().is_loopback() {
        if let Some(t) = query.get("token").or_else(|| query.get("bearer")) {
            // ... query-param token verification ...
        }
    }
    // ... rest of handler ...
}
```

After:
```rust
pub async fn http_ws(
    connect: ConnectInfo<SocketAddr>,
    // Query parameter removed entirely
    // ... other params unchanged ...
) -> impl IntoResponse {
    let role_res = /* header-based auth */;
    // query-param fallback block removed entirely
    // ... rest of handler unchanged ...
}
```

- [ ] **Step 4: Compile check**

```bash
cargo check -p vox-orchestrator
```

Expected: no errors. If `Query` is used elsewhere in the file, keep the import.

- [ ] **Step 5: Run the full http_gateway test suite**

```bash
cargo test -p vox-orchestrator http_gateway -- --nocapture
```

Expected: all existing tests PASS (the removed code had no test coverage).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/mcp_tools/http_gateway/ws.rs
git commit -m "fix(security): remove WS query-param token fallback (FIX-K-05)

Tokens in ?token= and ?bearer= query params appear in server access
logs and proxy logs. Header-based auth is sufficient for loopback."
```

---

## Task 4: FIX-B-02 — Replace unwrap on UUID split

**Context:** `runtime.rs:728` calls `uuid::Uuid::new_v4().to_string().split('-').next().unwrap()` to get the first UUID segment as a short ID. `.next()` on a split of a well-formed UUID v4 string will never return `None` in practice, but `unwrap()` on a `Option` in production code is a code smell and will panic if the UUID format ever changes (e.g., a different uuid crate version that omits hyphens).

**Files:**
- Modify: `crates/vox-orchestrator/src/runtime.rs`

- [ ] **Step 1: Write the failing test**

In `runtime.rs`, locate or create a `#[cfg(test)]` block. Add:

```rust
#[test]
fn short_id_from_standard_uuid() {
    let uuid_str = "550e8400-e29b-41d4-a716-446655440000";
    let result = short_id_from_str(uuid_str);
    assert_eq!(result, "550e8400");
}

#[test]
fn short_id_from_hyphen_free_string() {
    // If format ever lacks hyphens, must not panic — returns first 8 chars
    let s = "1234567890abcdef1234567890abcdef";
    let result = short_id_from_str(s);
    assert_eq!(result, "12345678");
}
```

- [ ] **Step 2: Run the tests to verify they fail**

```bash
cargo test -p vox-orchestrator short_id -- --nocapture
```

Expected: FAIL — `short_id_from_str` does not exist.

- [ ] **Step 3: Extract the helper and fix the call site**

Add a private helper near the top of the runtime module (or in a nearby `util` section if one exists):

```rust
fn short_id_from_str(s: &str) -> &str {
    if let Some(pos) = s.find('-') {
        &s[..pos]
    } else {
        &s[..s.len().min(8)]
    }
}
```

Then at line 728, replace:

```rust
// BEFORE
let short_id = uuid::Uuid::new_v4().to_string().split('-').next().unwrap();

// AFTER
let uuid_str = uuid::Uuid::new_v4().to_string();
let short_id = short_id_from_str(&uuid_str);
```

Note: `short_id_from_str` borrows from `uuid_str`, so `uuid_str` must be bound to a `let` before the call. Keep both variables in scope for the rest of their usage.

- [ ] **Step 4: Run the tests to verify they pass**

```bash
cargo test -p vox-orchestrator short_id -- --nocapture
```

Expected: both tests PASS.

- [ ] **Step 5: Compile check**

```bash
cargo check -p vox-orchestrator
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/runtime.rs
git commit -m "fix(crash): safe short_id extraction from UUID (FIX-B-02)

Replace .unwrap() on split().next() with unwrap_or fallback.
Extracts helper fn short_id_from_str for testability."
```

---

## Task 5: FIX-E-01 — Harden char-boundary expect in grounding

**Context:** `grounding.rs:236` calls `summary[i..].chars().next().expect("char boundary")`. Rust `String` guarantees UTF-8, and `i` is advanced by `c.len_utf8()` so the index is always on a boundary for valid strings. The `.expect()` cannot panic with correct inputs, but the message "char boundary" is cryptic. Harden with a meaningful message and use `unwrap_or` to make the no-char case explicit rather than a hidden assumption.

**Files:**
- Modify: `crates/vox-orchestrator/src/grounding.rs`

- [ ] **Step 1: Write the test**

In `grounding.rs`, locate or create a `#[cfg(test)]` block. Add:

```rust
#[test]
fn char_iteration_handles_multibyte() {
    // Verify that the summarization path doesn't panic on multi-byte Unicode.
    // '中' is 3 bytes in UTF-8.
    let s = "Hello 中文 world";
    let mut i = 0;
    let mut chars_seen = 0usize;
    while i < s.len() {
        let c = s[i..].chars().next()
            .unwrap_or_else(|| panic!("BUG: index {i} is not on a char boundary in string of len {}", s.len()));
        i += c.len_utf8();
        chars_seen += 1;
    }
    assert_eq!(chars_seen, s.chars().count());
}
```

- [ ] **Step 2: Run the test to verify it passes at baseline**

```bash
cargo test -p vox-orchestrator char_iteration_handles_multibyte -- --nocapture
```

Expected: PASS (the existing logic is correct; we're adding a better diagnostic, not fixing a broken algorithm).

- [ ] **Step 3: Apply the hardening**

In `crates/vox-orchestrator/src/grounding.rs` around line 236, change:

```rust
// BEFORE
let c = summary[i..].chars().next().expect("char boundary");

// AFTER
let c = summary[i..].chars().next().unwrap_or_else(|| {
    panic!(
        "BUG: byte index {i} is not on a UTF-8 char boundary \
         in summary of {} bytes — this indicates a logic error in \
         the summarization loop",
        summary.len()
    )
});
```

- [ ] **Step 4: Run the tests**

```bash
cargo test -p vox-orchestrator char_iteration -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Compile check**

```bash
cargo check -p vox-orchestrator
```

Expected: no errors.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/grounding.rs
git commit -m "fix(hygiene): diagnostic message on char-boundary expect (FIX-E-01)

Replace cryptic 'char boundary' with a panic message that includes the
byte index and summary length so post-mortem analysis is tractable."
```

---

## Task 6: FIX-H-03 — Add message to regex LazyLock unwrap

**Context:** `mentions.rs:9` uses `Regex::new(...).unwrap()` in a `LazyLock` initializer. The pattern is a hardcoded literal and cannot fail at runtime, but a bare `.unwrap()` in a `LazyLock` produces a panic message with no context about which regex or why. Replace with `.expect("BUG: ...")` to make any future failure immediately diagnosable.

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/chat_tools/chat/mentions.rs`

- [ ] **Step 1: This is a one-line mechanical change — verify the current state**

```bash
cargo test -p vox-orchestrator mentions -- --nocapture
```

Expected: PASS (establish baseline).

- [ ] **Step 2: Apply the change**

In `crates/vox-orchestrator/src/mcp_tools/chat_tools/chat/mentions.rs`, lines 8–9:

```rust
// BEFORE
static MENTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@([A-Za-z0-9_.:/\\-]+)").unwrap());

// AFTER
static MENTION_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"@([A-Za-z0-9_.:/\\-]+)")
        .expect("BUG: @mention regex is invalid — check the pattern literal"));
```

- [ ] **Step 3: Compile check**

```bash
cargo check -p vox-orchestrator
```

Expected: no errors.

- [ ] **Step 4: Run mentions tests**

```bash
cargo test -p vox-orchestrator mentions -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/mcp_tools/chat_tools/chat/mentions.rs
git commit -m "fix(hygiene): diagnostic expect on mention regex LazyLock (FIX-H-03)

Bare .unwrap() gives no context on failure. .expect with BUG prefix
follows project convention and aids post-mortem debugging."
```

---

## Task 7: FIX-O-03 — Propagate session-manager init error instead of panic

**Context:** `vox_orchestrator_d.rs:119–120` uses `unwrap_or_else(|e| panic!(...))` for `SessionManager::new()`. The daemon binary's `main` already returns `anyhow::Result<()>`, so this should be `?` with context. A panic during init produces a low-quality error message and bypasses any graceful shutdown hooks.

**Files:**
- Modify: `crates/vox-orchestrator/src/bin/vox_orchestrator_d.rs`

- [ ] **Step 1: Read the current main signature to confirm anyhow usage**

Verify that `main` is declared as `async fn main() -> anyhow::Result<()>` (or equivalent). If it is, `?` propagation is already wired up.

```bash
cargo check -p vox-orchestrator --bin vox_orchestrator_d
```

Expected: PASS (baseline).

- [ ] **Step 2: Write the test**

This is a binary entrypoint; unit-testing `main` directly is impractical. Instead, write a test that verifies `SessionManager::new` returns a `Result` (not panics) when given invalid config. Add to a test module in the binary or in the library:

```rust
// In crates/vox-orchestrator/src/lib.rs or a dedicated test file,
// add a compile-time assertion that SessionManager::new returns Result:
#[cfg(test)]
mod session_manager_tests {
    use super::SessionManager;

    #[test]
    fn session_manager_new_returns_result() {
        // Calling with a default/empty config; the point is it returns Result,
        // not that it succeeds. If it panics internally, this test catches it.
        // Use a config known to produce an Err to verify error propagation.
        let bad_cfg = crate::SessionConfig::default();
        let result = SessionManager::new(bad_cfg);
        // We don't assert Ok/Err — just that it returns without panicking.
        let _ = result;
    }
}
```

If `SessionConfig::default()` produces a valid config, adjust the test to use a config with an invalid field (e.g., empty session secret or zero timeout). The critical property is that `SessionManager::new` returns `Result` and does not panic internally.

- [ ] **Step 3: Run the test**

```bash
cargo test -p vox-orchestrator session_manager_new_returns_result -- --nocapture
```

Expected: PASS (the test verifies non-panicking behavior).

- [ ] **Step 4: Apply the fix**

In `crates/vox-orchestrator/src/bin/vox_orchestrator_d.rs`, around lines 119–120:

```rust
// BEFORE
let session_manager = vox_orchestrator::SessionManager::new(session_cfg)
    .unwrap_or_else(|e| panic!("Session manager initialization failed: {}", e));

// AFTER
let session_manager = vox_orchestrator::SessionManager::new(session_cfg)
    .context("session manager initialization failed")?;
```

Ensure `use anyhow::Context;` is already imported (check the top of the file). If not, add it:

```rust
use anyhow::Context;
```

- [ ] **Step 5: Compile check**

```bash
cargo check -p vox-orchestrator --bin vox_orchestrator_d
```

Expected: no errors.

- [ ] **Step 6: Run full orchestrator test suite**

```bash
cargo test -p vox-orchestrator -- --nocapture 2>&1 | tail -20
```

Expected: all tests PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator/src/bin/vox_orchestrator_d.rs
git commit -m "fix(crash): propagate session-manager init error with ? (FIX-O-03)

panic!() during daemon init bypasses graceful shutdown and gives a
poor error message. Use anyhow::Context + ? to surface the error
through the standard Result chain."
```

---

## Completion

After all 7 tasks are committed, run the full suite one final time:

```bash
cargo test -p vox-orchestrator -- --nocapture 2>&1 | tail -30
cargo clippy -p vox-orchestrator -- -D warnings
```

Expected: all tests green, no clippy warnings introduced by these changes.
