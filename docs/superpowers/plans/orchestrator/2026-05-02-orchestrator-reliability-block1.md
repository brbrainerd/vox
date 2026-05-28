# Orchestrator Reliability — Block 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Eliminate the four remaining orchestrator reliability gaps from Block 1 of the archetype coverage map: a missing test-client timeout, cost-without-progress doom-loop detection, pre-dispatch token estimation, and silent tool-call schema mismatches on the Anthropic native adapter.

**Architecture:** All changes are confined to `crates/vox-orchestrator`. Tasks 1–4 touch the budget/gate layer (`budget/mod.rs`, `gate.rs`, `task_dispatch/`). Task 5 touches the LLM bridge (`llm_bridge/error.rs`, `llm_bridge/provider_adapter.rs`). No HIR, no codegen, no frontend changes.

**Tech Stack:** Rust, tokio, `vox-orchestrator`, `vox_openai_wire`, `reqwest`. Tests run with `cargo test -p vox-orchestrator`.

---

## Pre-flight verification (do first, commit nothing)

**What was already fixed upstream — verify before writing any code:**

- [ ] **Verify FIX-J-01 is already applied** — open `crates/vox-orchestrator/src/mcp_tools/server_state.rs` and confirm both client builders at lines ~112 and ~159 have `.timeout(std::time::Duration::from_secs(120))`. If either is missing, that is a new bug — file it separately; it is out of scope for this plan.
- [ ] **Verify FIX-K-02 is already applied** — open `crates/vox-orchestrator/src/mcp_tools/http_gateway/origin_guard.rs` lines ~68–81. Confirm the `is_loopback_host` function strips the prefix and then asserts the next char is `:` or `/`. Run: `cargo test -p vox-orchestrator origin_guard -- --nocapture` and confirm `test_origin_denied_localhost_subdomain_spoof` passes.
- [ ] **Verify M10 (catalog refresh) is already running** — open `crates/vox-orchestrator/src/orchestrator/catalog_refresh.rs` lines ~19–51. Confirm `REFRESH_INTERVAL_SECS = 21600` and `run_catalog_refresh_loop` is spawned in background tasks. No work needed here.

---

## File map

| File | Action | Scope |
|---|---|---|
| `crates/vox-orchestrator/src/mcp_tools/http_gateway_tests.rs` | Modify line ~328 | Task 1 |
| `crates/vox-orchestrator/src/budget/mod.rs` | Add `CostProgressState` struct + 3 methods | Task 2 |
| `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/mod.rs` | Add 2-line call to `record_task_completion` | Task 3 |
| `crates/vox-orchestrator/src/gate.rs` | Add `doom_loop_cost_check` call | Task 3 |
| `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs` | Add pre-dispatch estimation check | Task 4 |
| `crates/vox-orchestrator/src/mcp_tools/llm_bridge/error.rs` | Add `is_capability_gap: bool` field | Task 5 |
| `crates/vox-orchestrator/src/mcp_tools/llm_bridge/provider_adapter.rs` | Guard `AnthropicNativeAdapter`; retry loop on gap | Task 5 |

---

## Task 1: Fix test HTTP client — missing timeout

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/http_gateway_tests.rs` (~line 328)

- [ ] **Step 1: Locate and read the bare client**

  Open `crates/vox-orchestrator/src/mcp_tools/http_gateway_tests.rs` and find line ~328:
  ```rust
  let client = reqwest::Client::new();
  ```
  This is the only place in the orchestrator crate that constructs a `reqwest::Client` without a timeout. Production code in `server_state.rs` already uses the 120s timeout via `vox_reqwest_defaults::client_builder()`.

- [ ] **Step 2: Replace the bare client with a timeout-aware builder**

  Change line ~328 from:
  ```rust
  let client = reqwest::Client::new();
  ```
  to:
  ```rust
  let client = reqwest::Client::builder()
      .timeout(std::time::Duration::from_secs(30))
      .build()
      .expect("test http client");
  ```
  30 seconds is appropriate for test code — long enough to avoid flaky failures on slow CI, short enough to time out a hung test.

- [ ] **Step 3: Compile**

  ```
  cargo check -p vox-orchestrator
  ```
  Expected: clean compile, zero errors.

- [ ] **Step 4: Run the gateway tests to confirm nothing broke**

  ```
  cargo test -p vox-orchestrator http_gateway -- --nocapture
  ```
  Expected: all http_gateway tests pass.

- [ ] **Step 5: Commit**

  ```
  git add crates/vox-orchestrator/src/mcp_tools/http_gateway_tests.rs
  git commit -m "fix(orchestrator): add timeout to test HTTP client (FIX-J-01 followup)"
  ```

---

## Task 2: Add cost-without-progress doom-loop state to BudgetManager

**Context:** `BudgetManager` already detects *semantic drift* (repeated identical outputs). It does NOT detect the second doom-loop pattern: an agent spending $N without completing any tasks, regardless of output variety. This task adds the data structure for that check. Task 3 wires it into the gate and task completion path.

**Files:**
- Modify: `crates/vox-orchestrator/src/budget/mod.rs`

- [ ] **Step 1: Write the failing test**

  Add this test to the `#[cfg(test)]` block at the bottom of `crates/vox-orchestrator/src/budget/mod.rs`:

  ```rust
  #[test]
  fn test_doom_loop_cost_check_fires_after_threshold() {
      let bm = BudgetManager::new(None);
      let agent = AgentId(42);

      // Set threshold to $0.10
      bm.set_doom_loop_cost_threshold(0.10);

      // Add $0.09 cost — should NOT trigger
      bm.record_cost_progress(agent, 0.09);
      assert!(bm.doom_loop_cost_check(agent).is_none(), "should not fire below threshold");

      // Add another $0.02 — total $0.11, should trigger
      bm.record_cost_progress(agent, 0.02);
      let reason = bm.doom_loop_cost_check(agent);
      assert!(reason.is_some(), "should fire above threshold");
      assert!(reason.unwrap().contains("no task completed"), "reason should mention no task completed");
  }

  #[test]
  fn test_doom_loop_cost_check_resets_on_task_completion() {
      let bm = BudgetManager::new(None);
      let agent = AgentId(42);
      bm.set_doom_loop_cost_threshold(0.10);

      bm.record_cost_progress(agent, 0.15);
      assert!(bm.doom_loop_cost_check(agent).is_some(), "should fire");

      // Simulate task completion
      bm.record_task_completion(agent);

      // Cost counter resets — should no longer fire
      assert!(bm.doom_loop_cost_check(agent).is_none(), "should not fire after task completion");
  }
  ```

- [ ] **Step 2: Run test to confirm it fails to compile (methods don't exist yet)**

  ```
  cargo test -p vox-orchestrator budget::tests::test_doom_loop_cost_check -- --nocapture 2>&1 | head -20
  ```
  Expected: compile error — `record_cost_progress`, `doom_loop_cost_check`, `record_task_completion`, `set_doom_loop_cost_threshold` do not exist.

- [ ] **Step 3: Add the CostProgressState struct**

  In `crates/vox-orchestrator/src/budget/mod.rs`, add this struct near the other state structs (after `DriftState`):

  ```rust
  /// Tracks cost accumulated since the last completed task, for doom-loop detection.
  #[derive(Debug, Default, Clone)]
  pub(crate) struct CostProgressState {
      /// USD spent since the last time `record_task_completion` was called for this agent.
      pub cost_since_last_completion: f64,
  }
  ```

- [ ] **Step 4: Add the field and atomic threshold to BudgetManager**

  In the `BudgetManager` struct (lines ~192–206 of `budget/mod.rs`), add two fields at the end of the struct:

  ```rust
  pub(crate) cost_progress: Arc<std::sync::RwLock<HashMap<AgentId, CostProgressState>>>,
  /// Threshold in USD: if cost_since_last_completion exceeds this, doom-loop fires.
  /// Default: $2.00. Set via `set_doom_loop_cost_threshold`.
  pub(crate) doom_loop_threshold_usd: Arc<std::sync::atomic::AtomicU64>,
  ```

  In `BudgetManager::new()`, initialize them by adding at the end of the constructor's `Self { ... }` block:

  ```rust
  cost_progress: Arc::new(std::sync::RwLock::new(HashMap::new())),
  // Encoded as f64::to_bits to match the sibling `drift_cost_threshold_usd`.
  // Read with f64::from_bits in doom_loop_cost_check.
  doom_loop_threshold_usd: Arc::new(std::sync::atomic::AtomicU64::new(
      2.00f64.to_bits(),
  )),
  ```

  Note: an earlier draft of this plan used micro-dollars (`2_000_000u64`). That
  was changed during implementation review to match the encoding of the
  adjacent `drift_cost_threshold_usd` field. Do NOT use the literal
  `2_000_000u64` here — `f64::from_bits(2_000_000)` decodes to ≈ 2.83×10⁻³¹⁸
  USD, which would silently disable the doom-loop check.

- [ ] **Step 5: Add the three methods**

  Add these methods to the `impl BudgetManager` block in `budget/mod.rs`:

  ```rust
  /// Accumulate cost toward the doom-loop threshold for `agent_id`.
  /// Called from `gate.rs` after each LLM usage recording.
  pub fn record_cost_progress(&self, agent_id: AgentId, cost_usd: f64) {
      let mut map = sync_lock::rw_write(&*self.cost_progress);
      let entry = map.entry(agent_id).or_default();
      entry.cost_since_last_completion += cost_usd;
  }

  /// Reset the doom-loop cost counter for `agent_id` when a task completes.
  /// Call this from `complete_task_with_attestation`.
  pub fn record_task_completion(&self, agent_id: AgentId) {
      let mut map = sync_lock::rw_write(&*self.cost_progress);
      map.entry(agent_id).or_default().cost_since_last_completion = 0.0;
  }

  /// Returns `Some(reason)` if the agent has spent more than `doom_loop_threshold_usd`
  /// without completing any task. Returns `None` if within budget.
  pub fn doom_loop_cost_check(&self, agent_id: AgentId) -> Option<String> {
      let threshold_micros = self
          .doom_loop_threshold_usd
          .load(std::sync::atomic::Ordering::Relaxed);
      let threshold_usd = threshold_micros as f64 / 1_000_000.0;
      let map = sync_lock::rw_read(&*self.cost_progress);
      let cost = map
          .get(&agent_id)
          .map(|s| s.cost_since_last_completion)
          .unwrap_or(0.0);
      if cost > threshold_usd {
          Some(format!(
              "Doom-loop: no task completed after spending ${:.4} (threshold ${:.2})",
              cost, threshold_usd
          ))
      } else {
          None
      }
  }

  /// Configure the doom-loop cost threshold in USD. Default is $2.00.
  pub fn set_doom_loop_cost_threshold(&self, threshold_usd: f64) {
      let micros = (threshold_usd * 1_000_000.0) as u64;
      self.doom_loop_threshold_usd
          .store(micros, std::sync::atomic::Ordering::Relaxed);
  }
  ```

- [ ] **Step 6: Run the tests**

  ```
  cargo test -p vox-orchestrator budget::tests::test_doom_loop -- --nocapture
  ```
  Expected: both `test_doom_loop_cost_check_fires_after_threshold` and `test_doom_loop_cost_check_resets_on_task_completion` pass.

- [ ] **Step 7: Run the full budget test suite to check no regressions**

  ```
  cargo test -p vox-orchestrator budget -- --nocapture
  ```
  Expected: all budget tests pass.

- [ ] **Step 8: Commit**

  ```
  git add crates/vox-orchestrator/src/budget/mod.rs
  git commit -m "feat(orchestrator): add cost-progress doom-loop state to BudgetManager (FIX-11)"
  ```

---

## Task 3: Wire doom-loop check into gate and task completion

**Context:** The data is ready. This task calls `record_task_completion` when a task actually completes, and calls `doom_loop_cost_check` in the pre-dispatch gate so a runaway agent is halted before its next task starts.

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/mod.rs`
- Modify: `crates/vox-orchestrator/src/gate.rs`

- [ ] **Step 1: Write the gate test**

  Add this test to the `#[cfg(test)]` block in `crates/vox-orchestrator/src/gate.rs`:

  ```rust
  #[test]
  fn test_gate_blocks_on_doom_loop() {
      use crate::budget::BudgetManager;
      use crate::types::AgentId;

      let bm = BudgetManager::new(None);
      let agent = AgentId(1);

      // Drive cost above threshold without a task completion
      bm.set_doom_loop_cost_threshold(0.10);
      bm.record_cost_progress(agent, 0.50); // $0.50 >> $0.10 threshold

      let result = BudgetGate::check_with_doom_loop(&bm, agent);
      assert!(
          matches!(result, GateResult::DoomLoop { .. }),
          "gate should return DoomLoop, got: {:?}",
          result
      );
  }

  #[test]
  fn test_gate_allows_after_task_completion() {
      use crate::budget::BudgetManager;
      use crate::types::AgentId;

      let bm = BudgetManager::new(None);
      let agent = AgentId(1);
      bm.set_doom_loop_cost_threshold(0.10);
      bm.record_cost_progress(agent, 0.50);
      bm.record_task_completion(agent); // progress resets counter

      let result = BudgetGate::check_with_doom_loop(&bm, agent);
      assert!(
          !matches!(result, GateResult::DoomLoop { .. }),
          "gate should NOT return DoomLoop after task completion"
      );
  }
  ```

- [ ] **Step 2: Run tests to see them fail**

  ```
  cargo test -p vox-orchestrator gate::tests::test_gate_blocks_on_doom_loop -- --nocapture 2>&1 | head -20
  ```
  Expected: compile error — `GateResult::DoomLoop` and `check_with_doom_loop` do not exist.

- [ ] **Step 3: Add DoomLoop variant to GateResult**

  In `crates/vox-orchestrator/src/gate.rs`, find the `GateResult` enum and add the new variant:

  ```rust
  pub enum GateResult {
      Allowed,
      BudgetExceeded { agent_id: AgentId, reason: String },
      DoomLoop { agent_id: AgentId, reason: String },  // ← add this
  }
  ```

- [ ] **Step 4: Add check_with_doom_loop to BudgetGate**

  In `crates/vox-orchestrator/src/gate.rs`, add this method to the `impl BudgetGate` or standalone function block (wherever the existing `check` function lives):

  ```rust
  /// Pre-dispatch check that combines the existing budget check with the doom-loop
  /// cost-without-progress check added in Task 2.
  pub fn check_with_doom_loop(
      manager: &crate::budget::BudgetManager,
      agent_id: AgentId,
  ) -> GateResult {
      // Run existing budget gate first.
      let existing = Self::check(manager, agent_id, &Default::default());
      if !matches!(existing, GateResult::Allowed) {
          return existing;
      }
      // Then run doom-loop check.
      if let Some(reason) = manager.doom_loop_cost_check(agent_id) {
          return GateResult::DoomLoop { agent_id, reason };
      }
      GateResult::Allowed
  }
  ```

  Note: `&Default::default()` for the config parameter is acceptable here because the existing `check` function only reads `config` for logging; the structural checks do not depend on its value. If your local `check` signature requires a non-default config, pass the real config instead.

- [ ] **Step 5: Run the gate tests**

  ```
  cargo test -p vox-orchestrator gate::tests::test_gate -- --nocapture
  ```
  Expected: both new tests pass.

- [ ] **Step 6: Wire record_task_completion into complete_task_with_attestation**

  Open `crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/mod.rs`.

  In `complete_task_with_attestation`, immediately after the existing `record_progress` call on line ~52:
  ```rust
  crate::sync_lock::rw_write(&self.monitor).record_progress(agent_id);
  ```
  add:
  ```rust
  crate::sync_lock::rw_read(&*self.budget_manager).record_task_completion(agent_id);
  ```

  The `budget_manager` is accessed via `self.budget_manager` (check the Orchestrator struct fields at `orchestrator.rs:71`). If `self.budget_manager` is an `Arc<RwLock<BudgetManager>>`, the call becomes:
  ```rust
  crate::sync_lock::rw_read(&*self.budget_manager).record_task_completion(agent_id);
  ```

- [ ] **Step 7: Wire check_with_doom_loop into submit_task_with_agent**

  Open `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`.

  After `agent_id` is resolved (line ~113 ends the `resolve_route` call) and before `process_task_submission_logic` (line ~122), add:

  ```rust
  // Doom-loop pre-dispatch check (FIX-11)
  {
      let bm = crate::sync_lock::rw_read(&*self.budget_manager);
      if let crate::gate::GateResult::DoomLoop { reason, .. } =
          crate::gate::BudgetGate::check_with_doom_loop(&bm, agent_id)
      {
          tracing::error!(
              agent_id = agent_id.0,
              %reason,
              "blocking task submission: doom-loop detected"
          );
          return Err(OrchestratorError::DoomLoop(reason));
      }
  }
  ```

  You also need to add `DoomLoop(String)` to `OrchestratorError`. Find `OrchestratorError` in `crates/vox-orchestrator/src/` (likely `error.rs` or `types.rs`) and add:
  ```rust
  DoomLoop(String),
  ```

- [ ] **Step 8: Compile**

  ```
  cargo check -p vox-orchestrator
  ```
  Expected: clean compile.

- [ ] **Step 9: Run all orchestrator tests**

  ```
  cargo test -p vox-orchestrator -- --nocapture 2>&1 | tail -20
  ```
  Expected: all tests pass (or pre-existing failures only — confirm with `git stash && cargo test -p vox-orchestrator` to establish baseline if unsure).

- [ ] **Step 10: Commit**

  ```
  git add \
    crates/vox-orchestrator/src/gate.rs \
    crates/vox-orchestrator/src/orchestrator/task_dispatch/complete/success/mod.rs \
    crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs
  git commit -m "feat(orchestrator): wire doom-loop cost check into gate and task completion (FIX-11)"
  ```

---

## Task 4: Pre-dispatch token estimation (M7)

**Context:** The existing budget gate (`gate.rs`) checks the *cumulative* cost/token count already spent. It does NOT check whether the *upcoming* task would push the agent over budget before that task's cost is incurred. This task adds that estimation step. The estimate is intentionally conservative — a miss is better than a false block.

**Files:**
- Modify: `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`
- Modify: `crates/vox-orchestrator/src/budget/mod.rs`

- [ ] **Step 1: Write the failing test**

  Add to `budget/mod.rs` tests:

  ```rust
  #[test]
  fn test_would_exceed_budget_true_when_tight() {
      let bm = BudgetManager::new(None);
      let agent = AgentId(7);

      // Give the agent a context budget of 1000 tokens total, 900 already used.
      // (Simulate by manipulating via a helper; use `init_budget` or whatever
      //  the existing initializer is named in this codebase.)
      bm.init_budget(agent, 1000, 1000.0); // max_tokens, max_cost_usd
      bm.record_usage(agent, 900);

      // Estimating 200 tokens would push it over 1000 → should return true.
      assert!(
          bm.would_exceed_token_budget(agent, 200),
          "should report would-exceed when estimate + used > max"
      );
  }

  #[test]
  fn test_would_exceed_budget_false_when_room() {
      let bm = BudgetManager::new(None);
      let agent = AgentId(8);
      bm.init_budget(agent, 1000, 1000.0);
      bm.record_usage(agent, 700);

      // 200 tokens on top of 700 = 900, which is under 1000 → false.
      assert!(
          !bm.would_exceed_token_budget(agent, 200),
          "should not report would-exceed when there is room"
      );
  }
  ```

  Note: if `init_budget` / `record_usage` are named differently in your local `BudgetManager`, find the correct method names first: `grep -n "fn init_budget\|fn record_usage\|fn add_budget" crates/vox-orchestrator/src/budget/mod.rs`

- [ ] **Step 2: Run test to confirm compile failure**

  ```
  cargo test -p vox-orchestrator budget::tests::test_would_exceed -- --nocapture 2>&1 | head -15
  ```
  Expected: compile error — `would_exceed_token_budget` does not exist.

- [ ] **Step 3: Add would_exceed_token_budget to BudgetManager**

  In `crates/vox-orchestrator/src/budget/mod.rs`, add to `impl BudgetManager`:

  ```rust
  /// Returns true if adding `estimated_tokens` to this agent's current usage
  /// would exceed their token budget. Conservative: returns false (allow) if
  /// no budget has been set for this agent.
  pub fn would_exceed_token_budget(&self, agent_id: AgentId, estimated_tokens: usize) -> bool {
      let map = sync_lock::rw_read(&*self.inner);
      let Some(budget) = map.get(&agent_id) else {
          return false; // no budget set → do not block
      };
      let projected = budget.tokens_used.saturating_add(estimated_tokens);
      budget.max_tokens > 0 && projected > budget.max_tokens
  }
  ```

  Note: adjust `budget.tokens_used` and `budget.max_tokens` to the actual field names in your `ContextBudget` struct. Run `grep -n "struct ContextBudget" crates/vox-orchestrator/src/budget/mod.rs` to find the struct and check field names before writing.

- [ ] **Step 4: Run the tests**

  ```
  cargo test -p vox-orchestrator budget::tests::test_would_exceed -- --nocapture
  ```
  Expected: both tests pass.

- [ ] **Step 5: Wire estimation into submit_task_with_agent**

  In `crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs`, after the doom-loop check added in Task 3, add:

  ```rust
  // Pre-dispatch token estimation (M7)
  {
      // Heuristic: 1 token ≈ 4 chars for description + 200 tokens per file in manifest.
      let estimated_tokens =
          task.description.len() / 4 + file_manifest.len().saturating_mul(200);

      let bm = crate::sync_lock::rw_read(&*self.budget_manager);
      if bm.would_exceed_token_budget(agent_id, estimated_tokens) {
          tracing::warn!(
              agent_id = agent_id.0,
              estimated_tokens,
              "blocking task submission: estimated tokens would exceed budget"
          );
          return Err(OrchestratorError::BudgetExceeded {
              agent_id,
              reason: format!(
                  "Pre-dispatch estimate of {} tokens would exceed remaining budget",
                  estimated_tokens
              ),
          });
      }
  }
  ```

  Note: `OrchestratorError::BudgetExceeded` should already exist (the gate returns this today). If its shape is different (e.g. a tuple variant), match the existing variant signature.

- [ ] **Step 6: Compile**

  ```
  cargo check -p vox-orchestrator
  ```
  Expected: clean compile.

- [ ] **Step 7: Run all tests**

  ```
  cargo test -p vox-orchestrator -- --nocapture 2>&1 | tail -10
  ```
  Expected: all tests pass.

- [ ] **Step 8: Commit**

  ```
  git add \
    crates/vox-orchestrator/src/budget/mod.rs \
    crates/vox-orchestrator/src/orchestrator/task_dispatch/submit/task_submit.rs
  git commit -m "feat(orchestrator): add pre-dispatch token estimation budget check (M7)"
  ```

---

## Task 5: Anthropic native adapter — fail loud on tool calls, retry via OpenAI-compat

**Context:** `AnthropicNativeAdapter` silently drops `tools` and `tool_choice` because `http_anthropic_direct` does not accept those parameters. If a conversation with tool-call requirements is routed to the Anthropic direct adapter (when `VOX_ANTHROPIC_DIRECT=1`), the tools are silently omitted and the model produces a response without calling any tool. This task makes that a loud, retryable error: the adapter returns a `is_capability_gap: true` error, and `infer_via_provider_adapter` retries with the next matching adapter (which will be `OpenAiCompatAdapter`, routing through OpenRouter with tools support).

**Files:**
- Modify: `crates/vox-orchestrator/src/mcp_tools/llm_bridge/error.rs`
- Modify: `crates/vox-orchestrator/src/mcp_tools/llm_bridge/provider_adapter.rs`

- [ ] **Step 1: Write the test**

  Add this test in `crates/vox-orchestrator/src/mcp_tools/llm_bridge/provider_adapter.rs` at the bottom:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_anthropic_native_returns_capability_gap_for_tools() {
          use crate::mcp_tools::llm_bridge::error::HttpInferError;
          // Simulate the tools-present scenario by constructing an InferRequest
          // with tools = Some(...).
          let req = InferRequest {
              system_prompt: "sys",
              user_prompt: vox_openai_wire::ChatMessageContent::Text("hello"),
              max_t: 256,
              temperature: None,
              top_p: None,
              json_mode: false,
              tools: Some(serde_json::json!([{"name": "my_tool"}])),
              tool_choice: None,
          };
          // The guard runs synchronously — check without actually calling HTTP.
          let error: HttpInferError = anthropic_tools_guard(&req).unwrap_err();
          assert!(
              error.is_capability_gap,
              "AnthropicNative should set is_capability_gap=true when tools are present"
          );
      }

      #[test]
      fn test_anthropic_native_no_gap_without_tools() {
          let req = InferRequest {
              system_prompt: "sys",
              user_prompt: vox_openai_wire::ChatMessageContent::Text("hello"),
              max_t: 256,
              temperature: None,
              top_p: None,
              json_mode: false,
              tools: None,
              tool_choice: None,
          };
          assert!(
              anthropic_tools_guard(&req).is_ok(),
              "AnthropicNative should not return capability gap when tools are absent"
          );
      }
  }
  ```

- [ ] **Step 2: Run tests to confirm compile failure**

  ```
  cargo test -p vox-orchestrator provider_adapter::tests -- --nocapture 2>&1 | head -15
  ```
  Expected: compile error — `HttpInferError.is_capability_gap` and `anthropic_tools_guard` do not exist.

- [ ] **Step 3: Add is_capability_gap to HttpInferError**

  Open `crates/vox-orchestrator/src/mcp_tools/llm_bridge/error.rs`. Change the struct to:

  ```rust
  #[derive(Debug)]
  pub(crate) struct HttpInferError {
      pub status: u16,
      pub message: String,
      /// True if the provider simply does not support a requested capability (e.g. tools),
      /// and the caller should retry with a different provider rather than treating this as
      /// a hard failure.
      pub is_capability_gap: bool,
  }
  ```

  Because `is_capability_gap` is a new field, every existing `HttpInferError { status, message }` literal will fail to compile. Update them all with `is_capability_gap: false`:

  ```
  grep -rn "HttpInferError {" crates/vox-orchestrator/src/ --include="*.rs"
  ```

  For every occurrence, add `, is_capability_gap: false` before the closing `}`. There are approximately 12 such literals across `providers/{anthropic,gemini,openai,ollama_chat}.rs` and `error.rs`. Do them all now.

- [ ] **Step 4: Add the anthropic_tools_guard helper**

  In `crates/vox-orchestrator/src/mcp_tools/llm_bridge/provider_adapter.rs`, add this free function (not a method) near the top of the file, after the imports:

  ```rust
  /// Returns `Err(HttpInferError { is_capability_gap: true, .. })` if `req` contains
  /// tools or tool_choice, because the Anthropic direct API does not support them.
  fn anthropic_tools_guard(req: &InferRequest<'_>) -> Result<(), HttpInferError> {
      if req.tools.is_some() || req.tool_choice.is_some() {
          return Err(HttpInferError {
              status: 0,
              message: "AnthropicNative does not support tool calls; \
                        retrying via OpenAI-compat adapter"
                  .to_string(),
              is_capability_gap: true,
          });
      }
      Ok(())
  }
  ```

- [ ] **Step 5: Call the guard in AnthropicNativeAdapter.infer()**

  In `provider_adapter.rs`, in the `AnthropicNativeAdapter` impl block, at the very start of the `Box::pin(async move { ... })` closure (before the `http_anthropic_direct` call):

  ```rust
  fn infer<'a>(...) -> Pin<...> {
      Box::pin(async move {
          anthropic_tools_guard(&req)?;  // ← add this line
          use super::providers::http_anthropic_direct;
          // ... rest unchanged
      })
  }
  ```

- [ ] **Step 6: Make infer_via_provider_adapter retry on is_capability_gap**

  In `provider_adapter.rs`, replace the `for adapter in adapters()` loop in `infer_via_provider_adapter` with:

  ```rust
  let mut last_err: Option<HttpInferError> = None;
  for adapter in adapters() {
      if adapter.supports(&model.provider_type) {
          match adapter.infer(client, model, req.clone()).await {
              Ok(result) => return Ok(result),
              Err(e) if e.is_capability_gap => {
                  tracing::warn!(
                      provider = ?model.provider_type,
                      message = %e.message,
                      "provider capability gap — trying next adapter"
                  );
                  last_err = Some(e);
                  // continue to next adapter
              }
              Err(e) => return Err(e),
          }
      }
  }
  Err(last_err.unwrap_or_else(|| HttpInferError {
      status: 0,
      message: format!("No provider adapter found for {:?}", model.provider_type),
      is_capability_gap: false,
  }))
  ```

  This loop now: (a) tries the first matching adapter, (b) if it returns `is_capability_gap`, continues to the next, (c) returns the last error if no adapter succeeded.

- [ ] **Step 7: Run the new tests**

  ```
  cargo test -p vox-orchestrator provider_adapter::tests -- --nocapture
  ```
  Expected: `test_anthropic_native_returns_capability_gap_for_tools` and `test_anthropic_native_no_gap_without_tools` both pass.

- [ ] **Step 8: Compile check**

  ```
  cargo check -p vox-orchestrator
  ```
  Expected: clean compile. Fix any remaining `HttpInferError { status, message }` literals that are missing `is_capability_gap: false`.

- [ ] **Step 9: Run all orchestrator tests**

  ```
  cargo test -p vox-orchestrator -- --nocapture 2>&1 | tail -10
  ```
  Expected: all tests pass.

- [ ] **Step 10: Commit**

  ```
  git add \
    crates/vox-orchestrator/src/mcp_tools/llm_bridge/error.rs \
    crates/vox-orchestrator/src/mcp_tools/llm_bridge/provider_adapter.rs \
    crates/vox-orchestrator/src/mcp_tools/llm_bridge/providers/
  git commit -m "fix(orchestrator): Anthropic native adapter returns capability gap for tool calls; routing retries via OpenAI-compat (M8/CC-21)"
  ```

---

## Post-implementation checklist

- [ ] `cargo test -p vox-orchestrator` — all tests green (or baseline unchanged).
- [ ] `cargo clippy -p vox-orchestrator -- -D warnings` — no new warnings.
- [ ] `cargo check -p vox-orchestrator` — clean.
- [ ] Confirm doom-loop threshold is configurable at runtime (search for `set_doom_loop_cost_threshold` — should be accessible via orchestrator config or an MCP tool for operator tuning).
- [ ] Update `docs/src/architecture/web-app-archetype-coverage-2026.md` — mark M6, M7, M8/CC-21, and Task 1 with ✓ in the §3 MENS + orchestrator journey blockers section.
- [ ] Regenerate doc pipeline: `cargo run -p vox-doc-pipeline` (only needed if any `.md` files in `docs/src/` were changed).

---

## What this plan does NOT cover

These are explicitly deferred to later blocks or separate plans:

- **Full bidirectional tool-call schema translation (CC-21 complete)** — Task 5 only stops the silent failure for Anthropic native. Full canonical `ToolCall` / `ToolResult` enum with Anthropic Messages API tool support (which requires adding `tools` to `AnthropicRequest`) is a separate initiative.
- **Doom-loop threshold MCP tool** — exposing `set_doom_loop_cost_threshold` as `vox_set_doom_loop_threshold` in the MCP tool registry is useful but out of scope here.
- **M11 (dashboard persistent state)**, **M12 (LSP completion)**, **M13 (MCP tool palette)** — Block 2+ work.
- **The five open questions** from `web-app-archetype-coverage-2026.md §6` (multi-tenancy explicit param, cron leader election, Stripe-first, rich text editor, mobile target) — Block 3+ design decisions.
