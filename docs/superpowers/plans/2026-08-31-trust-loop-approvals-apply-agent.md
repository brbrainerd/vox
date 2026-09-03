# Trust Loop — Approvals, Apply, Agent Budget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Humans see the real diff before approving, can modify args or undo by `operation_id`, and the agent loop can run more than eight sequential tool calls with parallel reads.

**Architecture:** Extend the existing in-process HITL registry (`PendingApprovals`) with full args + unified diff; persist those fields on the existing `hitl_approval_record` path; derive timeout from config; change `ApprovalOutcome::Modified` to carry replacement args; partition ungated tool calls in `agent_loop` with `join_all`; bind Loquela Ask/Plan/Agent to existing `PermissionMode` wire strings; add hunk Apply as a Tauri command plus a pure unified-diff parser.

**Tech Stack:** Rust (`vox-orchestrator`, `vox-orchestrator-mcp`, `vox-gui`, `vox-config`), Tokio, serde_json, React 19, vitest.

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md) §3.1–3.3, IDs H01 H02 H03 H04 H09 H16 G01 G03.

**Closes:** H01 (parallel + mode budgets + `iterations_left` event), H02, H03 (persist **list**; oneshot stays in-process), H04, H09 (dispatch deny **and** `vox_exit_plan_mode`), H16, H19, G01 (keep-diff until applied). G03 **composer UI** is Track 6; this track threads `permission_mode` through `message.rs`. Coverage: [`2026-08-31-platform-parity-id-coverage.md`](../specs/2026-08-31-platform-parity-id-coverage.md).

## Audit corrections (spec §9 — overrides tasks below)

- `ApprovalOutcome` is `Copy + Eq` today — drop both when adding `Modified { args }`. Update `daemon_extra.rs` and `outcome_from_decision(..., Option<Value>)`.
- Timeout field: `HitlPolicy.approval_timeout_secs`, **not** `VoxConfig.harness`.
- `risk_class`: explicit `"mutating"|"destructive"|"unknown"` helper — `SafetyClass` has no `Display`.
- `reregister_after_restart` + `hitl_rehydrate.rs` same commit as `register`.
- HITL schema: `crates/vox-db/src/schema/domains/execution.rs` + `BASELINE_VERSION` (90). Diffs >4 KiB → digest + artifact, not unbounded TEXT.
- Rollback: MCP tool **`vox_oplog`**; list field **`id`**; undo arg still `operation_id`. Rewrite `App.test.tsx` that asserts `args: {}`.
- `vox_write_file` has **no dispatch arm** — Task 0 first. Args `path` + `content`. `tempfile` already a normal dep.
- Path checks: `workspace_path.rs` under `state.repository.root` (exists).
- Parallel `join_all`: preserve original `calls` order in tool messages; cap 4–8; no shared-lock mutation.
- GUI resolve: keep accepting `outcome`; add `decision`/`modify`. Track 6 migrates NeedsYou/attention.

## Global Constraints

Inherit spec §6 and the master sequencer. Additional:

- Do not change `DEFAULT_MAX_ITERATIONS`’s **Ask** meaning (still 8). Add sibling constants; select by `permission_mode`.
- Do not introduce `SessionMode`. Ask=`ask`, Plan=`plan`, Agent=`accept_edits`.
- `std::fs::write` is forbidden for approval payloads. In-memory `unified_diff` is a `String`; durable row stores `args_json` + digest for blobs >4 KiB (spec §3.8).
- Re-verify `PendingApprovals::register` **and** `reregister_after_restart` with `rg` before editing.
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs` (today hardcodes `permission_mode: None`)
- Modify: `crates/vox-orchestrator-mcp/src/daemon_extra.rs` (unit `Modified`)
- Modify: `crates/vox-config` **`hitl_policy.rs`** (`approval_timeout_secs`) — not a new `harness` struct
- Modify: `crates/vox-db/src/schema/domains/execution.rs` + baseline version

---

## File map

| File | Role |
|---|---|
| Create: `crates/vox-orchestrator-mcp/src/approval_diff.rs` | Pure unified-diff builder for write tools |
| Create: `crates/vox-gui/ui/src/lib/parseUnifiedDiff.ts` | Pure hunk parser |
| Create: `crates/vox-gui/ui/src/lib/parseUnifiedDiff.test.ts` | vitest |
| Create: `crates/vox-gui/src/commands/apply_hunks.rs` | Tauri command |
| Modify: `crates/vox-orchestrator/src/attention/budget.rs` | `ApprovalOutcome::Modified { args }` |
| Modify: `crates/vox-orchestrator-mcp/src/pending_approvals.rs` | New `PendingApprovalInfo` fields + `PendingApprovalInfoDraft` |
| Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` | Timeout, diff attach, modified args, plan deny |
| Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs` | `vox_resolve_approval` args |
| Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs` | Parallel ungated + iteration budget |
| Modify: `crates/vox-orchestrator-mcp/src/permission_modes.rs` | `plan_denies_execution` |
| Modify: `crates/vox-config` `hitl_policy.rs` | `approval_timeout_secs` (NOT `VoxConfig.harness`) |
| Modify: `contracts/config/env-vars.v1.yaml` | `VOX_APPROVAL_TIMEOUT_SECS`, `VOX_AGENT_MAX_ITERATIONS` |
| Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` | Timeout, diff attach, modified args, plan deny, **`vox_write_file` arm** |
| Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs` | Thread `permission_mode` + max_iterations |
| Modify: `crates/vox-gui/ui/src/lib/rollbackLast.ts` | Create; tool `vox_oplog`, field `id` |
| Modify: `crates/vox-gui/ui/src/App.tsx` | `/rollback` uses rollbackLast |
| Test: `App.test.tsx` | Must stop asserting `vox_undo` `args: {}` |
| Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/DiffReview.tsx` | Hunk buttons |
| Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/Loquela.tsx` | Ask/Plan/Agent |
| Modify: `crates/vox-gui/src/main.rs` | Register `apply_worktree_hunks` |
| Modify: `crates/vox-gui/ui/src/components/surfaces/Approvals/*` | Render `unified_diff`; batch resolve |
| Test: `crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs` | Update `register` arity |

---

### Task 0: `vox_write_file` dispatch handler (H19)

**Files:** `dispatch.rs` `handle_tool_call_inner` match; `workspace_path.rs`; tests in `dispatch` pending_approvals tests that currently expect Unknown tool.

**Why first:** Apply, Modified-exec, and approval-diff tests fail for the wrong reason if write is still `Unknown tool` after gate.

- [ ] **Step 1: Failing test** — after `permission_mode=accept_edits` (or auto-approve in test), `handle_tool_call_with_mode` for `vox_write_file` with `{"path":"src/t.txt","content":"hi"}` under a temp repo root writes the file and returns ok. Today: error containing `Unknown tool`.

- [ ] **Step 2:** `cargo test -p vox-orchestrator-mcp write_file_handler_writes_under_repo_root` FAIL.

- [ ] **Step 3:** Match arm: resolve `path` via `workspace_path` canonicalize under `state.repository.root`; reject escape; write UTF-8 `content`. Do not `std::fs::write` for **event** data; this is workspace source. Same invariants as `mcp_client` write fast-path (`dispatch.rs` ~109).

- [ ] **Step 4:** PASS. Existing test that documents Unknown tool (`:321-325`) must be inverted or deleted.

- [ ] **Step 5:** commit `feat: dispatch implements gated vox_write_file under repository.root`

---

### Task 1: `ApprovalOutcome::Modified` carries args

**Files:**
- Modify: `crates/vox-orchestrator/src/attention/budget.rs` (`ApprovalOutcome`)
- Modify: every match on `ApprovalOutcome::Modified` (start with `rg "ApprovalOutcome::Modified"` in `crates/vox-orchestrator` and `crates/vox-orchestrator-mcp`)
- Test: same-file `#[cfg(test)]` in `budget.rs` **or** `crates/vox-orchestrator/src/attention/budget.rs` existing tests module

**Interfaces:**
- Consumes: nothing
- Produces: `ApprovalOutcome::Modified { args: serde_json::Value }`

- [ ] **Step 1: Write the failing test**

Add to the test module next to `ApprovalOutcome` (create `#[cfg(test)] mod approval_outcome_serde` at the bottom of `budget.rs` if none exists):

```rust
#[test]
fn modified_round_trips_args() {
    let original = ApprovalOutcome::Modified {
        args: serde_json::json!({"path": "a.rs", "content": "fn x() {}"}),
    };
    let s = serde_json::to_string(&original).unwrap();
    let back: ApprovalOutcome = serde_json::from_str(&s).unwrap();
    assert_eq!(back, original);
}

#[test]
fn bare_modified_string_fail_closes_to_rejected() {
    let back: ApprovalOutcome = serde_json::from_str("\"Modified\"").unwrap();
    assert_eq!(back, ApprovalOutcome::Rejected);
}
```

The second test requires a custom `Deserialize` — it will fail to compile until you add one.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator modified_round_trips_args bare_modified_string_fail_closes_to_rejected -- --nocapture`

Expected: FAIL or compile error (`Modified` takes no fields / cannot deserialize `"Modified"`).

- [ ] **Step 3: Write minimal implementation**

Change the enum variant to `Modified { args: serde_json::Value }`. Implement `Deserialize` that:

1. Accepts the struct form `{ "Modified": { "args": ... } }` (serde externally tagged default).
2. On unit string `"Modified"`, returns `Rejected`.

Keep `Serialize` as the struct form. **Remove `Copy` and `Eq`** from the enum (`Value` is not `Eq`). Keep `Clone` + `PartialEq`. Update every `ApprovalOutcome::Modified` match arm including `daemon_extra.rs`. Unit arms become `ApprovalOutcome::Modified { .. }` except `dispatch.rs` (Task 5) which must **use** `args`. Change `outcome_from_decision` to take `Option<Value>`. `decision == "modify"` without `args` returns Err JSON, not empty Modified. Calibrator that counts `Modified` as accepted stays accepted.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator modified_round_trips_args bare_modified_string_fail_closes_to_rejected`

Expected: PASS. Then `cargo test -p vox-orchestrator -p vox-orchestrator-mcp` to catch match exhaustiveness.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator/src/attention/budget.rs crates/vox-orchestrator crates/vox-orchestrator-mcp
git commit -m "fix: ApprovalOutcome::Modified carries replacement args and fail-closes legacy unit form"
```

---

### Task 2: `PendingApprovalInfo` carries args and diff

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/pending_approvals.rs`
- Modify: `crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs`
- Test: `pending_approvals.rs` `#[cfg(test)]` plus the integration file

**Interfaces:**
- Consumes: Task 1 `ApprovalOutcome`
- Produces: `PendingApprovalInfoDraft`, `PendingApprovals::register(draft) -> (ApprovalId, Receiver<ApprovalOutcome>)`

```rust
pub struct PendingApprovalInfoDraft {
    pub tool: String,
    pub summary: String,
    pub args: serde_json::Value,
    pub unified_diff: Option<String>,
    pub risk_class: String,
    pub estimated_cost_usd: Option<f64>,
    pub requested_at_ms: u64,
}
```

- [ ] **Step 1: Write the failing test** in `pending_approvals.rs`:

```rust
#[test]
fn register_preserves_args_and_diff() {
    let reg = PendingApprovals::default();
    let (id, _rx) = reg.register(PendingApprovalInfoDraft {
        tool: "vox_write_file".into(),
        summary: "write src/x.rs".into(),
        args: serde_json::json!({"path":"src/x.rs","content":"hi"}),
        unified_diff: Some("--- a/src/x.rs\n+++ b/src/x.rs\n".into()),
        risk_class: "write".into(),
        estimated_cost_usd: None,
        requested_at_ms: 1,
    });
    let listed = reg.list();
    assert_eq!(listed[0].approval_id, id);
    assert_eq!(listed[0].args["path"], "src/x.rs");
    assert!(listed[0].unified_diff.as_ref().unwrap().contains("+++ b/src/x.rs"));
    assert!(listed[0].summary.len() <= 120);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp register_preserves_args_and_diff -- --nocapture`

Expected: FAIL (three-arg `register` still, or missing fields).

- [ ] **Step 3: Write minimal implementation**

Add fields to `PendingApprovalInfo`. Replace `register(tool, summary, requested_at_ms)` with `register(PendingApprovalInfoDraft)`. Update **all** call sites from `rg "pending_approvals.register"` **and** `reregister_after_restart` + `hitl_rehydrate.rs`. Truncate `summary` to 120 chars inside `register` if the caller passes more. `risk_class` helper must not use `SafetyClass::to_string()`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp register_preserves_args_and_diff register_then_resolve_wakes_the_awaiter`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/pending_approvals.rs crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs crates/vox-orchestrator-mcp/src/dispatch.rs
git commit -m "feat: pending approvals store full args and unified diff"
```

---

### Task 3: Unified diff for write tools

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/approval_diff.rs`
- Modify: `crates/vox-orchestrator-mcp/src/lib.rs` or `dispatch.rs` parent mod to `mod approval_diff;`
- Test: in-module tests in `approval_diff.rs`

**Interfaces:**
- Consumes: `args: &Value`, workspace root `Path`
- Produces: `pub fn unified_diff_for_tool(tool: &str, args: &serde_json::Value, root: &Path) -> Option<String>`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn write_file_diff_against_existing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.txt");
    std::fs::write(&path, "old\n").unwrap();
    let args = serde_json::json!({"path": path.to_str().unwrap(), "content": "new\n"});
    let diff = unified_diff_for_tool("vox_write_file", &args, dir.path()).unwrap();
    assert!(diff.contains("-old"));
    assert!(diff.contains("+new"));
}

#[test]
fn unknown_tool_returns_none() {
    assert!(unified_diff_for_tool("vox_memory_search", &serde_json::json!({}), Path::new(".")).is_none());
}

#[test]
fn summary_is_not_the_content() {
    let args = serde_json::json!({"path":"x.rs","content":"a".repeat(500)});
    let s = summary_for_tool("vox_write_file", &args);
    assert!(s.len() <= 120);
    assert!(!s.contains(&"a".repeat(50)));
}
```

`std::fs::write` in **tests** is allowed (tempdir fixture). Production `unified_diff_for_tool` **reads** via `std::fs::read_to_string` of the target path (workspace file, not event log). That is a source read, not event persistence — permitted.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp write_file_diff_against_existing -- --nocapture`

Expected: FAIL (module missing). If `tempfile` is not a `vox-orchestrator-mcp` dev-dep, add `tempfile.workspace = true` under `[dev-dependencies]` in `crates/vox-orchestrator-mcp/Cargo.toml` — check first with `rg tempfile crates/vox-orchestrator-mcp/Cargo.toml`. Prefer `std::env::temp_dir` + unique name if tempfile is absent, to avoid a new dep.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn unified_diff_for_tool(tool: &str, args: &serde_json::Value, _root: &Path) -> Option<String> {
    if tool != "vox_write_file" && tool != "write" {
        return None;
    }
    let path = args.get("path")?.as_str()?;
    let new = args.get("content")?.as_str()?;
    let old = std::fs::read_to_string(path).unwrap_or_default();
    Some(simple_unified(path, &old, new))
}

pub fn summary_for_tool(tool: &str, args: &serde_json::Value) -> String {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let mut s = format!("{tool} {path}");
    s.truncate(120);
    s
}
```

Implement `simple_unified` as a line-oriented dump (`--- a/{path}` / `+++ b/{path}` / each old line prefixed `-`, each new line `+`) if you do not want to add the `similar` crate. Do **not** add a crate-edge. A crude line diff is acceptable for v1 if tests pass.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp write_file_diff_against_existing unknown_tool_returns_none summary_is_not_the_content`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/approval_diff.rs crates/vox-orchestrator-mcp/src/lib.rs crates/vox-orchestrator-mcp/Cargo.toml
git commit -m "feat: build unified diffs for write-file approval payloads"
```

---

### Task 4: Dispatch timeout + attach diff

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` (the park block around `summary` / `APPROVAL_TIMEOUT`)
- Modify: `crates/vox-config/src/hitl_policy.rs` — `approval_timeout_secs` on existing `HitlPolicy` (there is **no** `VoxConfig.harness`)
- Modify: `contracts/config/env-vars.v1.yaml` — `VOX_APPROVAL_TIMEOUT_SECS`
- Test: extend `dangerous_tool_parks_until_resolved` assertions; add timeout test

**Interfaces:**
- Consumes: Task 2 `register(draft)`, Task 3 `unified_diff_for_tool` / `summary_for_tool`
- Produces: park block with no 200-char args dump; timeout from config (`0` = `Duration::MAX` practically: use `tokio::time::sleep` skip / `optional timeout`)

- [ ] **Step 1: Write the failing test** in `pending_approvals_tests.rs`:

```rust
#[tokio::test]
async fn parked_write_file_lists_diff_not_truncated_content() {
    let state = Arc::new(ServerState::new_full(load_config()));
    let dir = std::env::temp_dir().join(format!("vox-appr-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let file = dir.join("w.txt");
    std::fs::write(&file, "old\n").unwrap();
    let s2 = state.clone();
    let path = file.to_string_lossy().to_string();
    let call = tokio::spawn(async move {
        handle_tool_call(
            &s2,
            "vox_write_file",
            serde_json::json!({ "path": path, "content": "new\n" }),
        )
        .await
    });
    let deadline = tokio::time::Instant::now() + D_15S;
    loop {
        if !state.pending_approvals.list().is_empty() {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "never parked");
        tokio::time::sleep(D_20MS).await;
    }
    let info = &state.pending_approvals.list()[0];
    assert!(info.summary.len() <= 120);
    assert!(!info.summary.contains("new\n"));
    assert_eq!(info.args["content"], "new\n");
    assert!(info.unified_diff.as_ref().unwrap().contains("+new"));
    state.pending_approvals.resolve(&info.approval_id, ApprovalOutcome::Rejected);
    let _ = call.await;
}
```

If `vox_write_file` is not the gated name in tests, use the same tool `dangerous_tool_parks_until_resolved` uses (`vox_run_shell`) for timeout tests, and keep write-file for diff. Split into two tests if dispatch only diffs `vox_write_file`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp parked_write_file_lists_diff_not_truncated_content -- --nocapture`

Expected: FAIL (`unified_diff` None or summary contains content).

- [ ] **Step 3: Write minimal implementation**

In `dispatch.rs` replace the `summary` block:

```rust
let summary = crate::approval_diff::summary_for_tool(name_canonical, &args);
let unified_diff = crate::approval_diff::unified_diff_for_tool(
    name_canonical,
    &args,
    &state.repository.root, // RepositoryContext.root exists — do not invent cwd fallback as primary
);
let risk_class = crate::permission_modes::risk_class_str(name_canonical); // NOT SafetyClass::to_string() — no Display
let (approval_id, rx) = state.pending_approvals.register(PendingApprovalInfoDraft {
    tool: name_canonical.to_string(),
    summary,
    args: args.clone(),
    unified_diff,
    risk_class,
    estimated_cost_usd: None,
    requested_at_ms: now_ms,
});
```

Replace `const APPROVAL_TIMEOUT` with:

```rust
let timeout_secs = state
    .config
    .hitl
    .approval_timeout_secs; // HitlPolicy field, default 86400; do NOT VoxConfig::load() per park
let outcome = if timeout_secs == 0 {
    rx.await.unwrap_or(ApprovalOutcome::Rejected)
} else {
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), rx).await {
        Ok(Ok(o)) => o,
        Ok(Err(_)) => ApprovalOutcome::Rejected,
        Err(_) => {
            state.pending_approvals.cancel(&approval_id);
            ApprovalOutcome::TimedOut
        }
    }
};
```

Add `approval_timeout_secs` on **`HitlPolicy`** with `#[serde(default = "default_approval_timeout")]` default `86400`. Wire env `VOX_APPROVAL_TIMEOUT_SECS` the same way other `VOX_*` overrides work in `vox-config`. Add the YAML row.

When writing `hitl_approval_record`, extend `crates/vox-db/src/schema/domains/execution.rs` (additive nullable columns + `BASELINE_VERSION` bump from 90). Blobs >4 KiB: digest + artifact store, not unbounded TEXT. Do **not** invent a `contracts/db` hitl YAML with its own `x-vox-version`.

**H03 restart honesty (required, not optional):** persist `args_json` + digest so `reregister_after_restart` **lists** the inbox after a process restart. The oneshot `Receiver` is in-process — do **not** claim resume of the parked waiter. After restart the GUI **polls list** and the user re-resolves. Test:

```rust
#[tokio::test]
async fn rehydrate_lists_persisted_row_without_waking_dead_oneshot() {
    // write a hitl row with args; new ServerState; reregister_after_restart;
    // list() contains the approval_id; no rx is connected — resolve via GUI poll path only
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp parked_write_file_lists_diff_not_truncated_content dangerous_tool_parks_until_resolved`

Expected: PASS. Also `vox ci secret-env-guard` is N/A (no secrets). Run nothing that regenerates unrelated SSOT.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-config contracts/config/env-vars.v1.yaml contracts/db
git commit -m "feat: approval park attaches diffs and uses a configurable timeout"
```

---

### Task 5: Modified args actually execute

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` (the `Approved | Modified` fall-through)
- Modify: `vox_resolve_approval` handler (rg `fn.*resolve_approval` in `vox-orchestrator-mcp`)
- Modify: `crates/vox-orchestrator-mcp/src/input_schemas.rs` `vox_resolve_approval` schema
- Test: `pending_approvals_tests.rs`

**Interfaces:**
- Consumes: Task 1 `Modified { args }`, Task 2 registry
- Produces: dispatch uses `args` from `Modified` instead of original

- [ ] **Step 1: Write the failing test**

```rust
#[tokio::test]
async fn modified_approval_executes_replacement_args_not_original() {
    // Park vox_run_shell with command "echo ORIG"
    // Resolve Modified { args: { "command": "echo REPLACED" } }
    // Assert the tool result stdout contains REPLACED and not ORIG
    // (If vox_run_shell is stubbed in tests, assert the dispatched args
    // via a lock/hook — but prefer observing the JSON result.)
}
```

Fill the test using the same `ServerState::new_full` + spawn pattern as `dangerous_tool_parks_until_resolved`. After the pending id appears:

```rust
let id = state.pending_approvals.list()[0].approval_id.clone();
assert!(state.pending_approvals.resolve(
    &id,
    ApprovalOutcome::Modified { args: serde_json::json!({"command": "echo REPLACED"}) },
));
let result = call.await.unwrap().unwrap();
assert!(result.contains("REPLACED"), "got {result}");
assert!(!result.contains("ORIG"));
```

If `vox_run_shell` is too dangerous/flaky in unit tests, use a gated but pure tool that echoes args (rg `is_gated_tool` tests). The invariant is: the executed args equal the Modified payload.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp modified_approval_executes_replacement_args_not_original -- --nocapture`

Expected: FAIL (original args still run, or resolve ignores args).

- [ ] **Step 3: Write minimal implementation**

In dispatch, before execute:

```rust
let mut exec_args = args;
if let ApprovalOutcome::Modified { args: new_args } = outcome {
    exec_args = new_args;
}
```

Use `exec_args` for the rest of the function instead of `args`. `vox_resolve_approval`: if `decision == "modify"` and `args` missing, return error JSON (do not execute original). Schema: `args` required when decision is modify.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp modified_approval_executes_replacement_args_not_original`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/dispatch.rs crates/vox-orchestrator-mcp/src/input_schemas.rs crates/vox-orchestrator-mcp/tests/pending_approvals_tests.rs
git commit -m "fix: Modified approval executes the replacement args"
```

---

### Task 6: `/rollback` requires `operation_id`

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx` (`base === '/rollback'` block)
- Modify: GUI tests that cover slash commands (`rg "/rollback" crates/vox-gui/ui`)
- Test: the existing vitest file next to App slash handling; if none, `crates/vox-gui/ui/src/App.rollback.test.ts` with the handler extracted. Prefer testing a small exported function rather than mounting App.

**Interfaces:**
- Consumes: MCP **`vox_oplog`** (registered in `dispatch.rs` ~923 — **not** `vox_oplog_list`) then `vox_undo`
- Produces: no success toast when list is empty; never `vox_undo` with `{}`

- [ ] **Step 1: Write the failing test**

Extract the rollback helper if needed:

```ts
export async function rollbackLast(invoke: InvokeFn): Promise<{ ok: boolean; body: string }> {
  const list = await invoke('invoke_mcp_tool', { tool: 'vox_oplog', args: { limit: 1 } });
  const id = list?.result?.operations?.[0]?.id; // NOT operation_id
  if (id == null) {
    return { ok: false, body: 'No checkpoint to revert. Run a mutating tool first.' };
  }
  const res = await invoke('invoke_mcp_tool', { tool: 'vox_undo', args: { operation_id: id } });
  return { ok: !res?.is_error, body: String(res?.result ?? '') };
}
```

Vitest:

```ts
it('does not call vox_undo without operation_id', async () => {
  const calls: string[] = [];
  const invoke = async (_cmd: string, args: { tool: string }) => {
    calls.push(args.tool);
    if (args.tool === 'vox_oplog') return { result: { operations: [] } };
    throw new Error('vox_undo must not be called');
  };
  const r = await rollbackLast(invoke as never);
  expect(r.ok).toBe(false);
  expect(calls).toEqual(['vox_oplog']);
});
```

Put `rollbackLast` in `crates/vox-gui/ui/src/lib/rollbackLast.ts`. Point `App.tsx` at it. **Rewrite** `App.test.tsx` that currently expects `vox_undo` with `args: {}`.

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/lib/rollbackLast.ts`

Expected: FAIL (file missing). First line of the test file: `// @vitest-environment node`

- [ ] **Step 3: Write minimal implementation** of `rollbackLast` as above; `App.tsx` calls it and toasts `ok`/`body`. Never invoke `vox_undo` with `{}`.

- [ ] **Step 4: Run tests**

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/lib/rollbackLast.ts`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/rollbackLast.ts crates/vox-gui/ui/src/App.tsx crates/vox-gui/ui/src/lib/rollbackLast.test.ts
git commit -m "fix: /rollback uses oplog operation_id and never fake-succeeds"
```

---

### Task 7: Parallel ungated tool calls + mode iteration budget

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs`
- Modify: `contracts/config/env-vars.v1.yaml` — `VOX_AGENT_MAX_ITERATIONS`
- Test: existing tests in `agent_loop.rs` (there are tests around line 559 using `max_iterations = 3`)

**Interfaces:**
- Consumes: spec §3.2 constants; `permission_modes::is_gated_tool`
- Produces: `iterations_for_mode(mode: &str) -> usize`; parallel `join_all` for ungated calls

- [ ] **Step 1: Write the failing tests** in `agent_loop.rs` `#[cfg(test)]`:

```rust
#[test]
fn iterations_for_mode_table() {
    assert_eq!(iterations_for_mode("ask"), 8);
    assert_eq!(iterations_for_mode("plan"), 16);
    assert_eq!(iterations_for_mode("accept_edits"), 32);
    assert_eq!(iterations_for_mode("accept_all"), 128);
}

#[test]
fn partition_tools_keeps_gated_sequential_order() {
    let names = vec!["vox_memory_search", "vox_write_file", "vox_search_query"];
    let (parallel, sequential) = partition_tool_names(&names);
    assert!(parallel.contains(&"vox_memory_search"));
    assert_eq!(sequential, vec!["vox_write_file"]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp iterations_for_mode_table partition_tools_keeps_gated_sequential_order -- --nocapture`

Expected: FAIL (fns missing).

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn iterations_for_mode(permission_mode: &str) -> usize {
    if let Ok(raw) = std::env::var("VOX_AGENT_MAX_ITERATIONS") {
        if let Ok(n) = raw.parse::<usize>() {
            return n.max(1);
        }
    }
    match permission_mode {
        "plan" => 16,
        "accept_edits" => 32,
        "accept_all" => 128,
        _ => 8, // ask and unknown
    }
}

pub fn partition_tool_names<'a>(names: &'a [&str]) -> (Vec<&'a str>, Vec<&'a str>) {
    let mut parallel = Vec::new();
    let mut sequential = Vec::new();
    for n in names {
        if crate::permission_modes::is_gated_tool(n) {
            sequential.push(*n);
        } else {
            parallel.push(*n);
        }
    }
    (parallel, sequential)
}
```

In the loop, replace `for call in &calls` with: run `join_all` over the ungated subset (preserving pairing with `call.arguments`), **then** run gated calls in original relative order. Push `role:tool` messages in **original `calls` order** (not completion order) so the model sees a stable transcript. To preserve order: collect `Vec<(index, content)>`, sort by index, then push.

Pass `iterations_for_mode(permission_mode)` when the caller currently passes `DEFAULT_MAX_ITERATIONS`.

**H01 rail (same commit or Task 14):** each agent-loop iteration emits a chat event field `iterations_left: u32` (`max - current`). Track 6 Task 28 renders `8/32`. Test: `agent_loop_emits_iterations_left` — after one iteration of max 8, payload has `iterations_left == 7`.

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp iterations_for_mode_table partition_tools_keeps_gated_sequential_order`

Expected: PASS. Also run the existing `max_iterations` tests in this file.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs contracts/config/env-vars.v1.yaml
git commit -m "feat: parallel ungated tool calls and mode-based iteration budgets"
```

---

### Task 8: Plan mode dispatch-denies gated tools

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/permission_modes.rs`
- Modify: `crates/vox-orchestrator-mcp/src/dispatch.rs` (before park / execute)
- Test: `permission_modes.rs` existing `#[cfg(test)]`

**Interfaces:**
- Consumes: `PermissionMode::Plan`, `is_gated_tool`
- Produces: `pub fn plan_blocks_execution(mode: PermissionMode, tool: &str) -> bool`

- [ ] **Step 1: Write the failing test**

```rust
#[test]
fn plan_blocks_write_file() {
    assert!(plan_blocks_execution(PermissionMode::Plan, "vox_write_file"));
    assert!(!plan_blocks_execution(PermissionMode::Ask, "vox_write_file"));
    assert!(!plan_blocks_execution(PermissionMode::Plan, "vox_memory_search"));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp plan_blocks_write_file -- --nocapture`

Expected: FAIL.

- [ ] **Step 3: Write minimal implementation**

```rust
pub fn plan_blocks_execution(mode: PermissionMode, tool: &str) -> bool {
    mode == PermissionMode::Plan && is_gated_tool(tool)
}
```

At the top of `handle_tool_call_with_mode`, after resolving `mode`:

```rust
if crate::permission_modes::plan_blocks_execution(mode, name_canonical) {
    return Ok(crate::params::ToolResult::<()>::err(
        "Plan mode cannot execute mutating tools. Exit plan (permission_mode=ask or accept_edits) first.".into(),
    ).to_json_compact());
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vox-orchestrator-mcp plan_blocks_write_file ask_mode_approves_nothing`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/permission_modes.rs crates/vox-orchestrator-mcp/src/dispatch.rs
git commit -m "fix: plan permission mode denies gated tool execution"
```

---

### Task 13: `vox_exit_plan_mode` (H09 original ExitPlanMode)

Original canvas: Plan is not a hard gate until **ExitPlanMode** is the only promotion. Task 8 is the deny. This task is the **promotion tool**.

**Files:** `input_schemas.rs` (new MCP tool); `dispatch.rs` match arm; session permission store (`rg setPermissionMode` / `permission_mode` on session); GUI PlanPanel Approve is Track 6 Task 21 (calls this tool).

- [ ] **Step 1:** test `exit_plan_mode_sets_accept_edits`:
  1. Session starts `permission_mode=plan`.
  2. `vox_write_file` returns plan-blocked error (Task 8).
  3. `vox_exit_plan_mode` with `{}` (no extra args) sets session mode to `accept_edits`.
  4. Same write now parks or executes per Agent rules.
  Tool name is exactly `vox_exit_plan_mode`. Do not reuse `setPermissionMode` as the only path — MCP agents must call this tool.

- [ ] **Step 2:** `cargo test -p vox-orchestrator-mcp exit_plan_mode_sets_accept_edits` FAIL.

- [ ] **Step 3:** Register tool in schemas + dispatch. Implementation: `state.set_permission_mode(PermissionMode::AcceptEdits)` (use the existing setter `rg permission_mode` — do not invent a parallel store). Return JSON `{ "ok": true, "permission_mode": "accept_edits" }`.

- [ ] **Step 4:** PASS. Also `plan_blocks_write_file` still PASS.

- [ ] **Step 5:** commit `feat: vox_exit_plan_mode promotes Plan to Agent`

---

### Task 9: Parse unified diffs in the GUI

**Files:**
- Create: `crates/vox-gui/ui/src/lib/parseUnifiedDiff.ts`
- Create: `crates/vox-gui/ui/src/lib/parseUnifiedDiff.test.ts`

**Interfaces:**
- Consumes: unified diff text
- Produces:

```ts
export type Hunk = { oldStart: number; newStart: number; lines: string[] };
export type FileDiff = { path: string; hunks: Hunk[] };
export function parseUnifiedDiff(text: string): FileDiff[];
```

- [ ] **Step 1: Write the failing test** (`// @vitest-environment node` on line 1)

```ts
import { parseUnifiedDiff } from './parseUnifiedDiff';

it('parses a one-hunk file', () => {
  const text = [
    '--- a/src/a.rs',
    '+++ b/src/a.rs',
    '@@ -1,1 +1,1 @@',
    '-old',
    '+new',
    '',
  ].join('\n');
  const files = parseUnifiedDiff(text);
  expect(files).toHaveLength(1);
  expect(files[0].path).toBe('src/a.rs');
  expect(files[0].hunks).toHaveLength(1);
  expect(files[0].hunks[0].lines).toEqual(['-old', '+new']);
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/lib/parseUnifiedDiff.test.ts`

Expected: FAIL (module missing).

- [ ] **Step 3: Write minimal implementation** — split on `^diff --git` or `^--- `; parse `@@ -oldStart` hunks; `path` from `+++ b/` strip.

- [ ] **Step 4: Run tests**

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/lib/parseUnifiedDiff.test.ts`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/parseUnifiedDiff.ts crates/vox-gui/ui/src/lib/parseUnifiedDiff.test.ts
git commit -m "feat: parse unified diffs into file hunks for Apply"
```

---

### Task 10: DiffReview hunk actions + Tauri apply

**Files:**
- Create: `crates/vox-gui/src/commands/apply_hunks.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs`, `crates/vox-gui/src/main.rs` (`generate_handler`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Loquela/DiffReview.tsx`, `DiffReview.test.tsx`
- Same commit: `cargo run -p vox-cli -- ci gui-surface-coverage --write`

**Interfaces:**
- Consumes: Task 9 `parseUnifiedDiff`
- Produces: `apply_worktree_hunks(ApplyHunksInput)` as in spec §3.3

- [ ] **Step 1: Write the failing Rust test** in `apply_hunks.rs`:

```rust
#[test]
fn reject_empty_file_path() {
    let err = validate_input(&ApplyHunksInput {
        file: "  ".into(),
        hunks: vec![],
        action: ApplyAction::Accept,
    })
    .unwrap_err();
    assert!(err.contains("file"));
}
```

And a vitest: clicking Accept on a hunk calls `invoke('apply_worktree_hunks', ...)`. Mock invoke.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vox-gui reject_empty_file_path -- --nocapture`

Expected: FAIL (module missing). Requires Task 0 sidecar if `tauri-build` complains — run `vox run scripts/gui-build.vox` first.

- [ ] **Step 3: Write minimal implementation**

`validate_input` rejects empty `file` and `..` path segments. Apply: read file, apply listed hunks in reverse order (so offsets stay valid), write through the same path MCP `vox_write_file` uses (call `handle_tool_call` if reachable from Tauri; otherwise use `vox_orchestrator` workspace write — **not** raw `std::fs::write` for the success path if a workspace API exists; if the GUI sidecar already writes via MCP, invoke that). If only `std::fs` is available in-process for the workspace file the user is editing, it is source code not event data — allowed, using `PathBuf::join` on the repo root from app state.

Wire `DiffReview` buttons: per hunk Accept/Reject; per file Accept all / Reject all. `parseUnifiedDiff(diff)`.

**G01 keep-buffer (original fix, required):** do **not** clear the `diff` prop/state on Accept until `apply_worktree_hunks` returns ok. On mismatch (hunk context not in file), return error code `hunk-did-not-apply`; vitest: toast/error shown and `diff` string still in the component. Reject hunk = restore that hunk vs HEAD (or skip apply) without dropping the rest of the buffer.

- [ ] **Step 4: Run tests + regenerate coverage**

Run: `cargo test -p vox-gui reject_empty_file_path`

Run: `pnpm -C crates/vox-gui/ui exec vitest run src/components/surfaces/Loquela/DiffReview.test.tsx`

Run: `cargo run -p vox-cli -- ci gui-surface-coverage --write`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui crates/vox-cli/tests/fixtures contracts/reports
git commit -m "feat: hunk Apply/Reject in DiffReview via apply_worktree_hunks"
```

---

### Task 11: Thread `permission_mode` into the agent loop (G03 wire)

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_tools/chat/message.rs` (hardcodes `None` + `DEFAULT_MAX_ITERATIONS` today)
- Modify: `ChatMessageParams` / params.rs as needed
- Composer **UI** Ask/Plan/Agent is Track 6 Task 2 — do **not** add a second mode control here that fights Approvals Segment

**Interfaces:**
- Consumes: spec §3.2 mapping
- Produces: `try_run_agent_turn` receives `permission_mode` from MCP invoke / `ChatMessageParams`, not `None`

- [ ] **Step 1:** Test that `ChatMessageParams { permission_mode: Some("accept_edits"), … }` results in `max_iterations >= 32` (spy or assert on `run_agent_turn` args). Today always 8.

- [ ] **Step 2:** FAIL.

- [ ] **Step 3:** Thread the field. Do not invent `SessionMode`. Approvals `setPermissionMode` remains the MCP SSOT until Track 6 adds the composer.

- [ ] **Step 4:** PASS `cargo test -p vox-orchestrator-mcp` for the new test.

- [ ] **Step 5:** commit `feat: agent loop honors permission_mode iteration budgets from ChatMessageParams`

---

### Task 12: Batch resolve (H16)

**Files:**
- Modify: Approvals surface (`crates/vox-gui/ui/src/components/surfaces/Approvals/`)
- Modify: MCP or Tauri to accept `approval_ids: string[]` — prefer a loop of existing `vox_resolve_approval` in the GUI rather than a new MCP tool (YAGNI).
- Test: vitest that multi-select calls resolve N times

**Interfaces:**
- Consumes: Task 2 list payload (`risk_class`, `unified_diff`)
- Produces: Approvals table columns: tool, risk, diff preview, checkbox; “Approve selected”

- [ ] **Step 1: Write the failing test** for a pure helper:

```ts
export function idsToApprove(selected: Set<string>, listed: string[]): string[] {
  return listed.filter((id) => selected.has(id));
}
```

```ts
it('only approves selected ids', () => {
  expect(idsToApprove(new Set(['AP-1']), ['AP-1', 'AP-2'])).toEqual(['AP-1']);
});
```

- [ ] **Step 5** commit `feat: batch approve selected HITL items with diffs visible`.

---

### Task 14: Emit `iterations_left` if not in Task 7 (H01 rail)

If Task 7 already emits the field, this task is a **named regression test only** (`agent_loop_emits_iterations_left`) — do not skip the test. Track 6 Task 28 is the rail UI.

**Files:** `agent_loop.rs` event / progress JSON; `rg iterations` in chat events.

- [ ] **Step 1:** FAIL until the field exists on the turn-progress payload.
- [ ] **Step 2–5:** commit `feat: agent loop reports iterations_left for the execution rail` (or `test:` if already emitted).

---

## Track 1 gate

HARD: `cargo test -p vox-orchestrator -p vox-orchestrator-mcp -p vox-config`

HARD: `exit_plan_mode_sets_accept_edits` + `agent_loop_emits_iterations_left` + `rehydrate_lists_persisted_row_without_waking_dead_oneshot`

HARD: `pnpm -C crates/vox-gui/ui exec vitest run src/lib/parseUnifiedDiff.test.ts src/lib/rollbackLast.test.ts`

HARD: `cargo clippy -p vox-orchestrator-mcp -p vox-orchestrator -p vox-gui -- -D warnings`

SOFT: manual Axis — park a write, see the diff, modify args, `/rollback` fails closed with empty oplog.

Composer Ask/Plan/Agent **UI** is Track 6 (Gate A does not require `sessionMode.ts`).
