# Free-Tier Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A brand-new Vox user with zero API keys and no Ollama reaches a working chat state without a terminal error, with a real spend safety net already in place, using an in-app OAuth flow instead of copy-pasting a key from a browser.

**Architecture:** Three independently-landable phases. Phase 1 wires Vox's existing-but-inert `daily_budget_usd`/`per_session_budget_usd` config fields into dispatch with a warn-then-block guard. Phase 2 adds a new low-dependency `vox-oauth-pkce` crate implementing RFC 8252's loopback-server PKCE pattern against OpenRouter, consumed identically by a new CLI command and a new Tauri command, both persisting the resulting key through the *existing* `vox_secrets::set_registry_token` path — no new secret storage plumbing. Phase 3 builds the onboarding wizard as a React overlay component (not a registered sidebar surface, to avoid the generated `surfaceRegistry.generated.ts` machinery) with three entry paths, reusing `KeysSecretsSection` (newly exported) for the "I have a key" path and Phase 2's Tauri command for "get a free key," plus small additions to `ModelsView.tsx`.

**Tech Stack:** Rust (Tauri backend, `vox-cli`, `vox-orchestrator-mcp`), TypeScript/React (Tauri frontend), `axum`+`reqwest` (already workspace deps) for the loopback listener and token exchange, `open` (new dependency) for launching the system browser, `rand`+`sha2`+`base64` (already workspace deps) for PKCE.

**Revision note (this version):** A second adversarial audit round (4 parallel reviewers: live web-verification of the OAuth/axum/crate claims, a code-audit re-checking every task against the actual current source, a TDD-completeness + parallelization structure analysis, and a spec-completeness pass) found and fixed: (1) Task 4 originally wired the budget guard into `vox-gui`'s `chat_send_message`, which is a thin RPC proxy — it never resolves a model in-process, so the guard would have silently protected nothing; it's now wired at the real server-side chokepoint in `vox-orchestrator-mcp`, matching what the design spec specified all along. (2) The design spec's promised `RateLimited` error class was never actually wired into the live dispatch path, only into an on-demand `vox doctor` diagnostic — new Task 12b closes that. (3) Three compile-breaking bugs (Task 10's CLI parse test, Task 11's `Ok(())` match against a `Result<PathBuf,_>`, Task 8's nonexistent `Cargo.toml` `members` list) are fixed. (4) OpenRouter's OAuth docs don't document a `state` parameter being echoed back on the callback — Task 9's callback handler no longer requires it, only validates it when present. (5) A "Parallelization & Phase Gates" section is added below per the real file-dependency graph, since the tasks are numbered sequentially but most are not actually sequential dependencies.

---

## Parallelization & Phase Gates

The task numbering below (1–19, plus 11b and 12b inserted where they logically belong) is a readable narrative order, not a strict execution order. Tasks that touch disjoint files with no logical dependency can be assigned to concurrent subagents. Tasks that touch the same file must be serialized relative to each other regardless of logical dependency, to avoid merge conflicts.

**Independent of everything — can start immediately, in parallel:** Tasks 1, 3, 6, 8, 13, 14, 17.

**Hot files touched by more than one task (serialize within each group, in any order that respects the logical dependencies listed below):**
- `SettingsView.tsx`: Tasks 7 (conditionally), 13, 16, 19
- `ModelsView.tsx`: Tasks 17, 18
- `llm_routing.rs`: Tasks 6, 12
- The `model_route_policy` chokepoint module in `vox-orchestrator-mcp`: Tasks 3/4 (budget), 12b (rate-limit) — these three should land as one reviewed sequence even though 12b has no *logical* dependency on 3/4, since all three touch the same handler function.

**Logical dependencies:**
- 2 ← 1 · 4 ← 1, 3 · 5 ← 4 · 7 ← 2 · 9 ← 8 · 10 ← 9 · 11 ← 9 · 11b ← 11 · 12b ← 11b (hard: the wizard's verify step needs it) and file-lock with 4/12 (no logical dependency on those) · 15 ← 13, 14, 11b (hard: Task 15's `verifying` screen calls `verify_openrouter_key`) (soft: 2, 11 for full end-to-end value) · 16 ← 14 · 18 ← 17

**Suggested parallel batches** (assuming subagent-driven-development with one subagent per task in a batch):

- **Batch 1** (7 concurrent): 1, 3, 6, 8, 13, 14, 17
- **Batch 2** (6 concurrent, after Batch 1 clears): 2 (needs 1) · 4 (needs 1,3) · 9 (needs 8) · 12 (needs 6, file-lock) · 16 (needs 14) · 18 (needs 17)
- **Batch 3** (5 concurrent): 7 (needs 2) · 5 (needs 4) · 10 (needs 9) · 11 (needs 9) · 19 (needs 16, file-lock)
- **Batch 4** (2 concurrent): 11b (needs 11) · 12b (needs 4, file-lock with 4/12; logically independent of 11b)
- **Batch 5** (1 task, needs 13, 14, 11b; full end-to-end value also wants 2, 11, 12b): 15

The critical path is `8 → 9 → 11 → 11b → 15` (Phase 2 crate → OpenRouter driver → Tauri command → verification command → wizard), tied with `1 → 3 → 4 → 5` (budget field → guard logic → wiring → UI) — everything else is slack a parallel batch absorbs. Phase 1 (budget) and Phase 2 (OAuth) share zero files and can run fully in parallel from day one; only Phase 3 (the wizard) genuinely has to wait, since it consumes both — and specifically waits on 11b now, not just 11, since Task 15's "verifying" screen (added in the second audit round) depends on it directly.

**Phase gates:**

*Gate: Phase 1 complete (Tasks 1, 2, 3, 4, 5, 6, 7)*
- HARD: `cargo test -p vox-config -p vox-llm-config -p vox-orchestrator-mcp -p vox-gui -p vox-cli` all green
- HARD: `cargo clippy -p vox-config -p vox-llm-config -p vox-orchestrator-mcp -p vox-gui -p vox-cli -- -D warnings` clean
- HARD: the GUI vitest/Playwright specs for Task 5 (Chat toast) and Task 7 (Settings budget field) both green
- SOFT: manual dev-build check that a real over-budget chat message produces the distinct "Budget limit reached" toast, not just the mocked unit test

*Gate: Phase 2 complete (Tasks 8, 9, 10, 11, 11b, 12, 12b)*
- HARD: `cargo test -p vox-oauth-pkce -p vox-cli -p vox-gui -p vox-orchestrator-mcp` all green
- HARD: `cargo clippy -p vox-oauth-pkce -p vox-cli -p vox-gui -p vox-orchestrator-mcp -- -D warnings` clean
- HARD: the `wiremock`-based `exchange_code_at` test passes (the only real behavioral coverage of the token exchange)
- SOFT, per-OS, does **not** block other batches: a manual `vox secrets login --oauth --provider openrouter` smoke test succeeds on the developer's current OS. Repeating this on all three OSes is required before flipping the feature flag on for real users (see the Final regression pass), but a single-OS pass is enough to unblock Phase 3 GUI work, since that only needs the Tauri command to exist and be callable.
- HARD (new, given the `state`-param finding): before this gate is considered green, empirically confirm against a real OpenRouter callback whether `state` is actually echoed back — if it is not, Task 9's lenient handling is correct as written; if it is, tighten the check back to required. Do not skip this — it's the highest-confidence-but-still-unverified claim in the whole plan.

*Sub-gates within Phase 3 (Tasks 13–20, the largest phase):*
- **Sub-gate A** (Tasks 13+14 done): HARD — vitest green for `useOnboardingGate.test.ts`; `KeysSecretsSection` export compiles. Unlocks Task 15.
- **Sub-gate B** (before Task 15's app-shell mount ships to real users): HARD — Tasks 11 and 11b both exist and Phase 2's gate above is green, otherwise "Get a free key" is a dead button (11 missing) or gets stuck on the "verifying" screen forever (11b missing) in production, even though Playwright's mocks let Task 15's own tests pass without either.
- **Sub-gate C** (Settings-chain integrity, Tasks 7/13/16/19): HARD — full `settings.spec.ts` run after the *last* of these four lands, not just each task's own new assertions.
- **Sub-gate D** (Models-chain integrity, Tasks 17/18): HARD — full `ModelsView` spec file green after Task 18 (already in the plan's Task 18 Step 5 — treat it as a formal gate, not an optional nicety).
- **Final plan-wide gate**: HARD — the "Final regression pass" section at the end of this document. SOFT — the manual OAuth smoke test repeated on macOS and Linux (not just the one OS covered by Phase 2's soft gate) before the feature flag is enabled for real users; this blocks *rollout*, not any other batch of work.

---

## Phase 1 — Budget enforcement

### Task 1: `budget_warn_threshold_pct` config field

**Files:**
- Modify: `crates/vox-config/src/config/vox_config.rs:1-88`
- Test: `crates/vox-config/src/config/vox_config.rs` (inline `#[cfg(test)]` module, or wherever `VoxConfig::default()` is already tested — search the file first for an existing test module and add to it)

- [ ] **Step 1: Write the failing test**

Add to `crates/vox-config/src/config/vox_config.rs` (find the existing `#[cfg(test)] mod tests` block in this file — if none exists, create one at the file's end):

```rust
#[cfg(test)]
mod budget_warn_threshold_tests {
    use super::VoxConfig;

    #[test]
    fn default_budget_warn_threshold_is_80_percent() {
        let cfg = VoxConfig::default();
        assert!((cfg.budget_warn_threshold_pct - 0.8).abs() &lt; f32::EPSILON);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-config budget_warn_threshold_tests -- --nocapture`
Expected: FAIL with `no field \`budget_warn_threshold_pct\` on type \`VoxConfig\`` (compile error)

- [ ] **Step 3: Add the field**

In `crates/vox-config/src/config/vox_config.rs`, add to the `VoxConfig` struct (after `per_session_budget_usd: f64,`):

```rust
    /// Fraction of a budget cap (daily or per-session) at which a non-blocking
    /// warning is surfaced, before the cap itself blocks dispatch. 1.0 disables
    /// the warning (warning and block become the same event).
    pub budget_warn_threshold_pct: f32,
```

And in `impl Default for VoxConfig`, add (after `per_session_budget_usd: 1.0,`):

```rust
            budget_warn_threshold_pct: 0.8,
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-config budget_warn_threshold_tests -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/config/vox_config.rs
git commit -m "feat(vox-config): add budget_warn_threshold_pct field"
```

### Task 2: Register `budget_warn_threshold_pct` in the `vox-llm-config` SSOT

**Files:**
- Modify: `crates/vox-llm-config/src/keys.rs` (the `vc_key!` table, after the `per_session_budget_usd` entry — currently around line 139)
- Test: `crates/vox-llm-config` — find the existing test asserting the key table matches `VoxConfig`'s fields (search for a test named something like `gui_fields_matches_voxconfig` or similar parity test) and confirm it now covers the new key without modification (it should, since it likely iterates the table generically)

- [ ] **Step 1: Confirm no generic parity test exists (verified — it doesn't)**

Run:
```bash
grep -rn "fn.*parity\|fn.*matches_voxconfig\|LLM_CONFIG_KEYS" crates/vox-llm-config/src/keys.rs crates/vox-llm-config/src/lib.rs | head -20
```

An earlier audit already confirmed no generic test iterating `LLM_CONFIG_KEYS` against `VoxConfig` exists in this crate — do not assume Task 1 alone made anything fail here. Re-run this grep anyway (the codebase may have changed since); if it genuinely still finds nothing, proceed to Step 2. If it *does* find a real parity test now, treat that as the discovered failing test instead and skip Step 2's new-test authoring.

- [ ] **Step 2: Write the failing test**

Add to `crates/vox-llm-config/src/keys.rs` (or wherever the crate's existing `#[cfg(test)]` module lives):

```rust
#[cfg(test)]
mod budget_warn_threshold_registry_tests {
    use super::LLM_CONFIG_KEYS;

    #[test]
    fn budget_warn_threshold_pct_is_registered() {
        assert!(
            LLM_CONFIG_KEYS.iter().any(|k| k.env == "budget_warn_threshold_pct"),
            "budget_warn_threshold_pct must be registered in LLM_CONFIG_KEYS"
        );
    }
}
```

Run: `cargo test -p vox-llm-config budget_warn_threshold_pct_is_registered -- --nocapture`
Expected: FAIL — the registry entry doesn't exist yet

- [ ] **Step 3: Add the registry entry**

In `crates/vox-llm-config/src/keys.rs`, add after the `per_session_budget_usd` line:

```rust
    vc_key!("budget_warn_threshold_pct", Float, General, "Budget warn threshold", "Warn when spend crosses this fraction of a budget cap (0.0-1.0)"),
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-llm-config -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-llm-config/src/keys.rs
git commit -m "feat(vox-llm-config): register budget_warn_threshold_pct in SSOT"
```

### Task 3: `budget_guard` module — the core check logic

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/budget_guard.rs` (note the location: **inside** `model_route_policy/`, sibling to `resolve.rs` — this is the module the design spec specifies, and it matters: `model_route_policy/resolve.rs:368` is the actual chokepoint every chat/agent dispatch resolves through, confirmed by a live-code audit. An earlier draft of this task placed the file one directory up, in `llm_bridge/` directly, which would have made Task 4's wiring target the wrong call site — don't repeat that.)
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/mod.rs` (add `pub mod budget_guard;` — confirm this file exists first with `ls crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/`; if `model_route_policy` isn't a submodule with its own `mod.rs` but is instead declared inline in the parent `llm_bridge/mod.rs`, add the `pub mod budget_guard;` line there instead, nested under whatever declares `model_route_policy`)
- Test: `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/budget_guard.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the test against a stub, and watch it fail for a real reason**

First create a stub so the test can fail on assertion, not just on a missing-file compile error (this is the fix for a documented TDD gap: an earlier draft of this task wrote the implementation and its tests in the same step, which meant the tests only ever failed on "module doesn't exist yet," never on genuinely wrong behavior). Create `crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/budget_guard.rs` with only:

```rust
//! Budget enforcement guard, run before LLM dispatch. Warn-then-block on
//! `VoxConfig`'s `daily_budget_usd`/`per_session_budget_usd` caps, using the
//! recorded-spend SSOT (`VoxDb::llm_spend_summary`) — not a new spend tracker.

use vox_db::LlmSpendSummary;

/// Which cap tripped, for user-facing messaging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetScope {
    Daily,
    Session,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum BudgetGuardError {
    #[error("{scope:?} budget of ${cap_usd:.2} exceeded (spent ${spent_usd:.2})")]
    Exceeded {
        scope: BudgetScope,
        cap_usd: f64,
        spent_usd: f64,
    },
}

/// Non-blocking warning surfaced at the configured threshold, before the cap itself blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetWarning {
    pub scope: BudgetScope,
    pub cap_usd: f64,
    pub spent_usd: f64,
}

pub fn check(
    _spend: &amp;LlmSpendSummary,
    _daily_budget_usd: f64,
    _per_session_budget_usd: f64,
    _warn_threshold_pct: f32,
) -&gt; Result&lt;Option&lt;BudgetWarning&gt;, BudgetGuardError&gt; {
    unimplemented!("Task 3 Step 3 implements this")
}
```

Add `pub mod budget_guard;` per the Files note above, then add the test module below to the same file:

```rust
//! Budget enforcement guard, run before LLM dispatch. Warn-then-block on
//! `VoxConfig`'s `daily_budget_usd`/`per_session_budget_usd` caps, using the
//! recorded-spend SSOT (`VoxDb::llm_spend_summary`) — not a new spend tracker.

use vox_db::LlmSpendSummary;

/// Which cap tripped, for user-facing messaging.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetScope {
    Daily,
    Session,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum BudgetGuardError {
    #[error("{scope:?} budget of ${cap_usd:.2} exceeded (spent ${spent_usd:.2})")]
    Exceeded {
        scope: BudgetScope,
        cap_usd: f64,
        spent_usd: f64,
    },
}

/// Non-blocking warning surfaced at the configured threshold, before the cap itself blocks.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetWarning {
    pub scope: BudgetScope,
    pub cap_usd: f64,
    pub spent_usd: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spend(day: f64, session: f64) -&gt; LlmSpendSummary {
        LlmSpendSummary {
            total_usd: day,
            day_usd: day,
            session_usd: session,
        }
    }

    #[test]
    fn under_threshold_returns_none() {
        let result = check(&amp;spend(1.0, 0.2), 5.0, 1.0, 0.8);
        assert_eq!(result, Ok(None));
    }

    #[test]
    fn at_warn_threshold_returns_warning() {
        let result = check(&amp;spend(4.0, 0.2), 5.0, 1.0, 0.8);
        assert_eq!(
            result,
            Ok(Some(BudgetWarning {
                scope: BudgetScope::Daily,
                cap_usd: 5.0,
                spent_usd: 4.0,
            }))
        );
    }

    #[test]
    fn at_daily_cap_returns_exceeded() {
        let result = check(&amp;spend(5.0, 0.2), 5.0, 1.0, 0.8);
        assert_eq!(
            result,
            Err(BudgetGuardError::Exceeded {
                scope: BudgetScope::Daily,
                cap_usd: 5.0,
                spent_usd: 5.0,
            })
        );
    }

    #[test]
    fn at_session_cap_returns_exceeded_even_if_daily_ok() {
        let result = check(&amp;spend(1.0, 1.0), 5.0, 1.0, 0.8);
        assert_eq!(
            result,
            Err(BudgetGuardError::Exceeded {
                scope: BudgetScope::Session,
                cap_usd: 1.0,
                spent_usd: 1.0,
            })
        );
    }

    #[test]
    fn warn_threshold_of_one_disables_warning() {
        // At 1.0, "warn at" == the cap itself, so Exceeded fires first (cap check runs before warn check).
        let result = check(&amp;spend(5.0, 0.2), 5.0, 1.0, 1.0);
        assert!(matches!(result, Err(BudgetGuardError::Exceeded { scope: BudgetScope::Daily, .. })));
    }
}
```

- [ ] **Step 2: Run test to verify it fails for the right reason**

Run: `cargo test -p vox-orchestrator-mcp budget_guard -- --nocapture`
Expected: FAIL — panics with `not implemented: Task 3 Step 3 implements this` (the stub's `unimplemented!()`), not a compile error. This confirms the tests actually exercise `check()`'s behavior rather than merely requiring the module to exist.

- [ ] **Step 3: Replace the stub with the real implementation**

Replace the stub `check` function body (delete the `unimplemented!()` version) with:

```rust
/// Check `spend` against `daily_budget_usd`/`per_session_budget_usd` and
/// `budget_warn_threshold_pct`. Returns `Ok(Some(warning))` at the warn
/// threshold, `Ok(None)` under it, `Err(Exceeded)` at or over either cap.
/// Daily is checked before session (arbitrary but deterministic ordering —
/// callers only need to know *that* a cap tripped, not which one first, since
/// both block dispatch identically).
pub fn check(
    spend: &amp;LlmSpendSummary,
    daily_budget_usd: f64,
    per_session_budget_usd: f64,
    warn_threshold_pct: f32,
) -&gt; Result&lt;Option&lt;BudgetWarning&gt;, BudgetGuardError&gt; {
    if spend.day_usd &gt;= daily_budget_usd {
        return Err(BudgetGuardError::Exceeded {
            scope: BudgetScope::Daily,
            cap_usd: daily_budget_usd,
            spent_usd: spend.day_usd,
        });
    }
    if spend.session_usd &gt;= per_session_budget_usd {
        return Err(BudgetGuardError::Exceeded {
            scope: BudgetScope::Session,
            cap_usd: per_session_budget_usd,
            spent_usd: spend.session_usd,
        });
    }

    let warn_at_daily = daily_budget_usd * f64::from(warn_threshold_pct);
    if spend.day_usd &gt;= warn_at_daily {
        return Ok(Some(BudgetWarning {
            scope: BudgetScope::Daily,
            cap_usd: daily_budget_usd,
            spent_usd: spend.day_usd,
        }));
    }
    let warn_at_session = per_session_budget_usd * f64::from(warn_threshold_pct);
    if spend.session_usd &gt;= warn_at_session {
        return Ok(Some(BudgetWarning {
            scope: BudgetScope::Session,
            cap_usd: per_session_budget_usd,
            spent_usd: spend.session_usd,
        }));
    }

    Ok(None)
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp budget_guard -- --nocapture`
Expected: PASS (5 tests: `under_threshold_returns_none`, `at_warn_threshold_returns_warning`, `at_daily_cap_returns_exceeded`, `at_session_cap_returns_exceeded_even_if_daily_ok`, `warn_threshold_of_one_disables_warning`)

- [ ] **Step 5: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/budget_guard.rs crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/mod.rs
git commit -m "feat(vox-orchestrator-mcp): add budget_guard warn-then-block logic"
```

### Task 4: Wire `budget_guard` into the real dispatch chokepoint (server-side, not the GUI RPC proxy)

**⚠️ Correction from the second audit round**: an earlier draft of this task targeted `crates/vox-gui/src/commands/chat.rs`, on the assumption that's where the GUI "dispatches" a chat message. A live-code audit found that's wrong: `chat_send_message` in `vox-gui` is a thin RPC proxy — it calls out to a **separate orchestrator daemon process** (`client.call(TOOL_CALL, json!({"name": "vox_chat_message", ...}))`) and never resolves a model in-process. Wiring the guard there would have silently protected nothing; every real dispatch happens server-side. The design spec had this right from the start (§1.2: *"a new `budget_guard` module in `.../model_route_policy/`... runs immediately before dispatch"*) — this task now matches it.

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_model_resolve.rs` (the function `resolve_chat_llm_model` — confirmed via live-code audit to be the actual entry point the orchestrator's chat-tool handler calls before dispatch; read it first, its exact signature isn't pre-verified in this plan)
- Test: same file, inline `#[cfg(test)]` or existing test module for `chat_model_resolve.rs`

- [ ] **Step 1: Read the real entry point before writing anything**

Run:
```bash
sed -n '1,80p' crates/vox-orchestrator-mcp/src/chat_model_resolve.rs
grep -n "fn resolve_chat_llm_model" -A 20 crates/vox-orchestrator-mcp/src/chat_model_resolve.rs
```

Confirm: the function's exact signature (parameters — does it already receive a session id, or does that need threading in from its own caller?), whether it's `async` (it must be, to call `budget_guard::check` which needs an async DB read), and — importantly — whether this module or its caller already holds a pooled/cached DB handle you should reuse rather than opening a fresh `VoxDb::connect_canonical()` per call (the earlier vox-gui-targeted draft of this task copied a fresh-connect pattern from `get_llm_spend`, which is fine for an occasional Settings poll but wasteful on every single chat dispatch — check for a pooled pattern in this crate first, e.g. `grep -rn "DbPool\|connect_canonical" crates/vox-orchestrator-mcp/src/` and prefer whatever's already idiomatic here).

- [ ] **Step 2: Write the failing test**

Add a test in `chat_model_resolve.rs` (or its existing test module) asserting `resolve_chat_llm_model` returns an error containing "budget" (case-insensitive) when spend already exceeds the configured cap — using the exact async-test/DB-fixture pattern already established elsewhere in this crate (search for `#[tokio::test]` in this file or its neighbors first; do not invent a new fixture pattern):

```rust
#[tokio::test]
async fn resolve_refuses_when_daily_budget_exceeded() {
    // Arrange: a VoxConfig with daily_budget_usd = 0.01 and a spend summary
    // showing $0.01+ already spent today (use the same DB fixture pattern
    // found elsewhere in this crate's existing async tests, per Step 1).
    // Act: call resolve_chat_llm_model with any valid prompt/args.
    // Assert: it returns Err(_) containing "budget" (case-insensitive) rather
    // than proceeding to resolve_mcp_chat_model / a real model resolution.
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp resolve_refuses_when_daily_budget_exceeded -- --nocapture`
Expected: FAIL (the function currently dispatches unconditionally)

- [ ] **Step 4: Wire the guard**

At the top of `resolve_chat_llm_model` (before it delegates to `resolve_mcp_chat_model`/`resolve_mcp_chat_model_with_rationale`), insert:

```rust
    let cfg = vox_config::VoxConfig::load();
    let spend = match vox_db::VoxDb::connect_canonical().await {
        Ok(db) =&gt; db
            .llm_spend_summary(session_id.as_deref())
            .await
            .unwrap_or_default(),
        Err(_) =&gt; Default::default(),
    };
    if let Err(e) = crate::llm_bridge::model_route_policy::budget_guard::check(
        &amp;spend,
        cfg.daily_budget_usd,
        cfg.per_session_budget_usd,
        cfg.budget_warn_threshold_pct,
    ) {
        return Err(e.to_string());
    }
```

Adjust the exact `session_id`/DB-access expression to match whatever Step 1 actually found (a parameter name, and — if a pooled handle already exists in scope — use that instead of `VoxDb::connect_canonical()`). Adjust the module path prefix (`crate::llm_bridge::...`) if `chat_model_resolve.rs` isn't itself inside the `llm_bridge` module tree — confirm via its own `use`/`mod` declarations at the top of the file.

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp resolve_refuses_when_daily_budget_exceeded -- --nocapture`
Expected: PASS

- [ ] **Step 6: Verify existing tests still pass (regression gate)**

Run: `cargo test -p vox-orchestrator-mcp`
Expected: all pre-existing tests in this crate still PASS — this task must not break normal (under-budget) dispatch. Since this is now the single shared chokepoint for GUI, CLI, and MCP-tool chat dispatch, a regression here is more consequential than a GUI-only change would have been — treat this gate as non-optional.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/chat_model_resolve.rs
git commit -m "feat(vox-orchestrator-mcp): wire budget_guard into the shared chat dispatch chokepoint"
```

### Task 5: Surface `BudgetWarning` and `BudgetGuardError::Exceeded` distinctly in the GUI

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/` (whichever component renders dispatch errors today — search: `grep -rln "sanitizeErrorForToast" crates/vox-gui/ui/src/components/surfaces/Chat/`)
- Test: co-located `*.test.tsx` if one exists for that component (check first), or a new one following its exact pattern

- [ ] **Step 1: Find the existing error-toast pattern**

Run:
```bash
grep -rn "sanitizeErrorForToast" crates/vox-gui/ui/src/components/surfaces/Chat/*.tsx
```

`LlmSettingsSection` in `SettingsView.tsx` already uses this exact helper (`pushToast({ tone: 'warn', title: '...', body: sanitizeErrorForToast(err), cause: 'backend-error' })`) — the chat surface's dispatch-error handling should follow the identical shape. Read the matched file's current catch block before editing.

- [ ] **Step 2: Write the failing test**

In whatever test file corresponds to the component found in Step 1 (or create one following the exact `settings.spec.ts` Playwright mock pattern — `page.addInitScript` installing `window.__TAURI_INTERNALS__.invoke` returning a rejected promise with a "budget" string for the chat-send command), assert that a budget-exceeded error produces a toast with `title` containing "Budget" (not the generic error title used for other failures).

- [ ] **Step 3: Run test to verify it fails**

Run the project's existing test command for this file (mirror whatever `package.json` script runs `*.spec.ts`/`*.test.tsx` files — check `crates/vox-gui/ui/package.json` `scripts` section first).
Expected: FAIL (today's error toast is generic, doesn't distinguish budget errors)

- [ ] **Step 4: Add distinct handling**

In the dispatch-error catch block, check whether the error string starts with `"Daily budget"`/`"Session budget"` (matching `BudgetGuardError`'s `Display` impl from Task 3 — `"{scope:?} budget of ${cap_usd:.2} exceeded..."`, which renders as e.g. `"Daily budget of $5.00 exceeded (spent $5.12)"`), and if so use a distinct toast title/tone (e.g. `title: 'Budget limit reached'`) and add a body suffix pointing to Settings, following the same `pushToast` call shape found in Step 1.

- [ ] **Step 5: Run test to verify it passes**

Same command as Step 3.
Expected: PASS

- [ ] **Step 6: Run the full Chat surface spec (regression gate)**

Run the project's Playwright/vitest command scoped to every existing spec for the Chat surface (not just this new assertion) — the modified catch block is shared error-handling for *every* dispatch error, not just budget ones, so this task must not change behavior for any other error class.
Expected: all pre-existing Chat-surface tests still PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Chat/
git commit -m "feat(vox-gui-ui): distinct toast for budget-exceeded dispatch errors"
```

### Task 6: `vox doctor` budget-status line

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs:1-58`
- Test: same file, inline (search for an existing `#[cfg(test)]` block in this file first — doctor checks in this codebase are typically tested by asserting on the `Vec&lt;Check&gt;` produced)

- [ ] **Step 1: Write the failing test**

Add (or extend the existing test module) in `llm_routing.rs`:

```rust
#[cfg(test)]
mod budget_status_tests {
    use super::*;

    #[test]
    fn reports_budget_caps_in_detail_string() {
        let mut checks = Vec::new();
        run(&amp;mut checks);
        let llm_check = checks
            .iter()
            .find(|c| c.name == "LLM routing (Secrets)")
            .expect("LLM routing check present");
        assert!(
            llm_check.detail.contains("daily_budget_usd="),
            "detail should report the configured daily budget cap, got: {}",
            llm_check.detail
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-cli reports_budget_caps_in_detail_string -- --nocapture`
Expected: FAIL (current `detail` string has no `daily_budget_usd=` segment)

- [ ] **Step 3: Add the budget line**

In `llm_routing.rs`'s `run` function, before the `let detail = format!(...)` block, add:

```rust
    let budget_cfg = vox_config::VoxConfig::load();
```

And extend the existing `format!` call to include the two cap values, e.g. change:

```rust
    let detail = format!(
        "routing_profile={routing_profile}; openrouter_model={}; chat_completions_url={}; provider_keys_present=[{}]",
        model,
        OPENROUTER_CHAT_COMPLETIONS_URL,
        if keys.is_empty() { "(none)".to_string() } else { keys.join(", ") }
    );
```

to:

```rust
    let detail = format!(
        "routing_profile={routing_profile}; openrouter_model={}; chat_completions_url={}; provider_keys_present=[{}]; daily_budget_usd={:.2}; per_session_budget_usd={:.2}",
        model,
        OPENROUTER_CHAT_COMPLETIONS_URL,
        if keys.is_empty() { "(none)".to_string() } else { keys.join(", ") },
        budget_cfg.daily_budget_usd,
        budget_cfg.per_session_budget_usd,
    );
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-cli reports_budget_caps_in_detail_string -- --nocapture`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs
git commit -m "feat(vox-cli): surface budget caps in vox doctor LLM routing check"
```

### Task 7: GUI Settings — warn-threshold slider

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (wherever `daily_budget_usd`/`per_session_budget_usd` are currently rendered as inputs — search: `grep -n "daily_budget_usd\|per_session_budget_usd" crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` to find the exact render location, since this wasn't in the code pulled for this plan)

- [ ] **Step 1: Locate the existing budget-field rendering**

Run:
```bash
grep -n "daily_budget_usd\|per_session_budget_usd\|budget_warn_threshold_pct" crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
```

These fields are already rendered generically (the whole `get_user_config`/`set_user_config` catalog from Task 2's registry entry is likely rendered by a generic field-loop, given `get_user_config` "builds the full catalog from the registry" per its doc comment on `user_config.rs:122-125`) — if so, `budget_warn_threshold_pct` **already renders automatically** once Task 2 lands, with no frontend change needed here. Confirm this by checking whether the grep above shows a generic loop (e.g. `config.map(field =&gt; ...)`) rather than named per-field JSX.

- [ ] **Step 2: If generic (expected case) — write a test confirming it renders**

If Step 1 confirms a generic catalog-driven render, write a Playwright test (following the exact `settings.spec.ts` pattern — `page.addInitScript` mocking `get_user_config` to return an array including a `budget_warn_threshold_pct` entry) asserting the field label "Budget warn threshold" appears on the Settings page. This is a regression test proving Task 2's registry entry is sufficient — no new component code needed.

- [ ] **Step 3: Run test to verify it passes without any new frontend code**

Run the project's Playwright command (check `package.json`) scoped to this new spec file.
Expected: PASS, confirming Task 2 alone was sufficient (this task closes with no production code change, only the test).

- [ ] **Step 4: If NOT generic (fallback case) — add explicit JSX**

Only if Step 1 shows named per-field JSX (not a generic loop): add a matching input for `budget_warn_threshold_pct` immediately after the existing `per_session_budget_usd` field, copying that field's exact JSX pattern (input type, `onChange` → `set_user_config` invoke call) verbatim with the key substituted.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/e2e/
git commit -m "test(vox-gui-ui): confirm budget_warn_threshold_pct surfaces in Settings"
```

---

## Phase 2 — OAuth free-key flow

### Task 8: `vox-oauth-pkce` crate — PKCE verifier/challenge generation

**Files:**
- Create: `crates/vox-oauth-pkce/Cargo.toml`
- Create: `crates/vox-oauth-pkce/src/lib.rs`
- Create: `crates/vox-oauth-pkce/src/pkce.rs`
- Modify: root `Cargo.toml` (register the new crate in `[workspace.dependencies]` — **not** the `members` list; this workspace's root `Cargo.toml:3` declares `members = ["crates/*", "crates/workspace-hack"]`, a two-entry glob, so any new directory under `crates/` is picked up automatically. An earlier draft of this task assumed an explicit per-crate `members` list to edit — verified against the real file that no such list exists here; don't add one, just confirm the crate is on the correct relative path and move on to `[workspace.dependencies]`.)
- Modify: `docs/src/architecture/where-things-live.md` (add a row per `AGENTS.md`'s requirement: "consult this before adding code... add the row in the same PR")
- Test: `crates/vox-oauth-pkce/src/pkce.rs` inline

- [ ] **Step 1: Check the `[workspace.dependencies]` entry format for a recent small crate**

Run:
```bash
grep -n "vox-llm-egress" Cargo.toml
```

Note the exact `members` entry format and (if present) `[workspace.dependencies]` entry format to replicate for `vox-oauth-pkce`.

- [ ] **Step 2: Write the failing test**

Create `crates/vox-oauth-pkce/src/pkce.rs`:

```rust
//! RFC 7636 PKCE code_verifier/code_challenge generation.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// A generated PKCE pair: the secret verifier (kept in-process) and its
/// S256 challenge (sent to the authorization server).
#[derive(Debug, Clone)]
pub struct PkcePair {
    pub verifier: String,
    pub challenge: String,
}

/// Generate a new PKCE pair using a 64-byte random verifier (RFC 7636 §4.1
/// requires 43-128 chars of unreserved base64url chars; 64 raw bytes ->
/// 86 base64url chars, comfortably in range) and its S256 challenge.
pub fn generate() -&gt; PkcePair {
    let mut raw = [0u8; 64];
    rand::thread_rng().fill_bytes(&amp;mut raw);
    let verifier = URL_SAFE_NO_PAD.encode(raw);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    PkcePair { verifier, challenge }
}

/// Generate a random `state` value (32 bytes, base64url) for CSRF binding.
pub fn generate_state() -&gt; String {
    let mut raw = [0u8; 32];
    rand::thread_rng().fill_bytes(&amp;mut raw);
    URL_SAFE_NO_PAD.encode(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_is_in_rfc7636_length_range() {
        let pair = generate();
        assert!(pair.verifier.len() &gt;= 43 &amp;&amp; pair.verifier.len() &lt;= 128);
    }

    #[test]
    fn challenge_is_sha256_of_verifier() {
        let pair = generate();
        let mut hasher = Sha256::new();
        hasher.update(pair.verifier.as_bytes());
        let expected = URL_SAFE_NO_PAD.encode(hasher.finalize());
        assert_eq!(pair.challenge, expected);
    }

    #[test]
    fn two_calls_produce_different_verifiers() {
        let a = generate();
        let b = generate();
        assert_ne!(a.verifier, b.verifier);
    }

    #[test]
    fn state_is_nonempty_and_varies() {
        let a = generate_state();
        let b = generate_state();
        assert!(!a.is_empty());
        assert_ne!(a, b);
    }
}
```

Create `crates/vox-oauth-pkce/src/lib.rs`:

```rust
//! Minimal RFC 8252 (OAuth for Native Apps) loopback-server PKCE flow,
//! provider-agnostic core + an OpenRouter-specific driver.

pub mod pkce;
pub mod openrouter;

pub use pkce::{PkcePair, generate as generate_pkce, generate_state};
```

Create `crates/vox-oauth-pkce/Cargo.toml`:

```toml
[package]
name = "vox-oauth-pkce"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
rust-version.workspace = true
description = "RFC 8252 loopback-server OAuth PKCE flow, used for in-app free-tier key provisioning."

[dependencies]
axum = { workspace = true, features = ["http1", "json", "tokio"] }
base64 = { workspace = true }
open = "5"
rand = { workspace = true }
reqwest = { workspace = true }
serde = { workspace = true, features = ["derive"] }
serde_json = { workspace = true }
sha2 = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true, features = ["rt", "macros", "net", "time"] }
tracing = { workspace = true }
vox-http-client = { workspace = true }

[dev-dependencies]
tokio = { workspace = true, features = ["rt-multi-thread", "macros", "test-util"] }

[lints]
workspace = true
```

Add `vox-oauth-pkce = { path = "crates/vox-oauth-pkce" }` to `[workspace.dependencies]` in the root `Cargo.toml`, matching the exact `vox-llm-egress` entry format found in Step 1 (alphabetically among the other `vox-*` entries). No `members` list edit is needed — the glob already covers it (see the Files note above).

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-oauth-pkce -- --nocapture`
Expected: FAIL — compile error, `openrouter` module referenced in `lib.rs` doesn't exist yet (this is expected; Task 9 creates it). To isolate just this task's tests, temporarily comment out `pub mod openrouter;` in `lib.rs`, run the test, then restore it before committing (Task 9 will make the full crate compile).

- [ ] **Step 4: Run test to verify it passes**

With `pub mod openrouter;` temporarily commented: `cargo test -p vox-oauth-pkce pkce:: -- --nocapture`
Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

Restore `pub mod openrouter;` in `lib.rs` (it will fail to compile standalone until Task 9 — commit anyway, since Task 9 immediately follows and this plan executes tasks in order; do not skip committing just because the crate doesn't fully compile between tasks 8 and 9 — this is expected mid-plan state):

```bash
git add crates/vox-oauth-pkce/ Cargo.toml docs/src/architecture/where-things-live.md
git commit -m "feat(vox-oauth-pkce): add crate scaffold + PKCE verifier/challenge generation"
```

### Task 9: `vox-oauth-pkce` — OpenRouter loopback flow driver

**Files:**
- Create: `crates/vox-oauth-pkce/src/openrouter.rs`
- Test: same file, inline

**⚠️ Corrections from the second audit round** (web-verified against OpenRouter's live docs and axum's real behavior, not assumed):
1. **OpenRouter's OAuth docs do not document a `state` parameter being echoed back on the callback.** The original draft of `callback_handler` *required* a matching `state` and rejected the callback otherwise — as written, that would make every real login fail with a false CSRF-mismatch error, since OpenRouter likely never sends `state` back at all. The code below now only rejects on an explicit *mismatch* (a `state` present but wrong), never on *absence* — the PKCE `code_verifier` check at token-exchange time remains the real security boundary regardless. **This still needs empirical confirmation** (see the Phase 2 gate in "Parallelization & Phase Gates" above) — if a real OpenRouter callback turns out to include a correct `state`, tighten this back to required.
2. **`server.abort()` immediately after the callback is a known-risky pattern** for exactly this "loopback OAuth callback" use case (a real GitHub issue describes the browser seeing a connection-reset instead of the success page when the serving task is killed mid-flush). The code below uses axum's `with_graceful_shutdown`, triggered by the handler itself only after it has built its response, and awaits the server task's actual completion (bounded by a short timeout) instead of aborting it.
3. **`open::that()`'s failure carried no way to recover** — the auth URL was a local variable, never surfaced to the caller. `OAuthError::BrowserOpen` now carries the URL alongside the underlying error, so a caller (CLI or wizard) can show it as a clickable/copyable fallback link instead of a dead end.

- [ ] **Step 1: Write the failing test**

Create `crates/vox-oauth-pkce/src/openrouter.rs`:

```rust
//! OpenRouter-specific PKCE loopback flow driver (RFC 8252 §7.3 pattern).

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::extract::{Query, State};
use axum::response::Html;
use axum::routing::get;
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::oneshot;

use crate::pkce::{self, PkcePair};

const OPENROUTER_AUTH_URL: &amp;str = "https://openrouter.ai/auth";
const OPENROUTER_TOKEN_EXCHANGE_URL: &amp;str = "https://openrouter.ai/api/v1/auth/keys";
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("failed to bind loopback listener: {0}")]
    Bind(std::io::Error),
    #[error("failed to open system browser for {url}: {source}")]
    BrowserOpen {
        url: String,
        #[source]
        source: std::io::Error,
    },
    #[error("timed out waiting for OAuth callback ({0:?})")]
    TimedOut(Duration),
    #[error("callback state mismatch (possible CSRF)")]
    StateMismatch,
    #[error("token exchange failed: {0}")]
    TokenExchange(String),
}

#[derive(Debug, Deserialize)]
struct CallbackQuery {
    code: Option&lt;String&gt;,
    state: Option&lt;String&gt;,
}

#[derive(Debug, serde::Serialize)]
struct ExchangeRequest&lt;'a&gt; {
    code: &amp;'a str,
    code_verifier: &amp;'a str,
    code_challenge_method: &amp;'a str,
}

#[derive(Debug, Deserialize)]
struct ExchangeResponse {
    key: String,
}

struct CallbackState {
    expected_state: String,
    tx: std::sync::Mutex&lt;Option&lt;oneshot::Sender&lt;Result&lt;String, OAuthError&gt;&gt;&gt;&gt;,
    shutdown_tx: std::sync::Mutex&lt;Option&lt;oneshot::Sender&lt;()&gt;&gt;&gt;,
}

async fn callback_handler(
    State(state): State&lt;Arc&lt;CallbackState&gt;&gt;,
    Query(q): Query&lt;CallbackQuery&gt;,
) -&gt; Html&lt;&amp;'static str&gt; {
    // OpenRouter's documented OAuth contract does not mention a `state`
    // parameter being echoed back on the callback (verified against their
    // live docs during the second audit round) — reject only on an explicit
    // MISMATCH (state present but wrong), never on absence, or every real
    // login would fail a check OpenRouter never promised to honor. The PKCE
    // code_verifier check at token-exchange time is the real security
    // boundary either way. If empirical testing later shows OpenRouter DOES
    // echo `state`, tighten this back to required-and-matching.
    let result = match q.code {
        None =&gt; Err(OAuthError::TokenExchange("missing code in callback".into())),
        Some(code) =&gt; match q.state {
            Some(got_state) if got_state != state.expected_state =&gt; Err(OAuthError::StateMismatch),
            _ =&gt; Ok(code),
        },
    };
    if let Some(tx) = state.tx.lock().expect("callback state mutex poisoned").take() {
        let _ = tx.send(result);
    }
    // Signal graceful shutdown only now, after the response above has been
    // constructed — axum flushes it to the client before the connection
    // closes. This intentionally avoids a raw task abort() (see the Task 9
    // header note): aborting immediately after receiving the callback is a
    // documented failure mode for this exact "one-shot loopback server"
    // pattern, where the browser can see a connection-reset instead of the
    // success page.
    if let Some(shutdown_tx) = state.shutdown_tx.lock().expect("shutdown state mutex poisoned").take() {
        let _ = shutdown_tx.send(());
    }
    Html("&lt;html&gt;&lt;body&gt;You can close this tab and return to Vox.&lt;/body&gt;&lt;/html&gt;")
}

/// Run the full loopback PKCE flow against OpenRouter and return the
/// provisioned API key on success. Opens the system browser; blocks (async)
/// until the callback arrives or `CALLBACK_TIMEOUT` elapses.
pub async fn run_openrouter_flow() -&gt; Result&lt;String, OAuthError&gt; {
    let PkcePair { verifier, challenge } = pkce::generate();
    let state_value = pkce::generate_state();

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(OAuthError::Bind)?;
    let port = listener.local_addr().map_err(OAuthError::Bind)?.port();

    let (tx, rx) = oneshot::channel();
    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let callback_state = Arc::new(CallbackState {
        expected_state: state_value.clone(),
        tx: std::sync::Mutex::new(Some(tx)),
        shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
    });

    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(callback_state);

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await;
    });

    let callback_url = format!("http://127.0.0.1:{port}/callback");
    let auth_url = format!(
        "{OPENROUTER_AUTH_URL}?callback_url={}&amp;code_challenge={}&amp;code_challenge_method=S256&amp;state={}",
        urlencoding_encode(&amp;callback_url),
        challenge,
        state_value,
    );

    open::that(&amp;auth_url).map_err(|e| OAuthError::BrowserOpen {
        url: auth_url.clone(),
        source: e,
    })?;

    let code = tokio::time::timeout(CALLBACK_TIMEOUT, rx)
        .await
        .map_err(|_| OAuthError::TimedOut(CALLBACK_TIMEOUT))?
        .map_err(|_| OAuthError::TokenExchange("callback channel closed unexpectedly".into()))??;

    // Wait for the server task's graceful shutdown to actually finish
    // (bounded — near-instant once shutdown_tx fired above) rather than
    // aborting it out from under an in-flight response.
    let _ = tokio::time::timeout(Duration::from_secs(5), server).await;

    exchange_code(&amp;code, &amp;verifier).await
}

async fn exchange_code(code: &amp;str, verifier: &amp;str) -&gt; Result&lt;String, OAuthError&gt; {
    let client = vox_http_client::client_builder()
        .build()
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;
    let resp = client
        .post(OPENROUTER_TOKEN_EXCHANGE_URL)
        .json(&amp;ExchangeRequest {
            code,
            code_verifier: verifier,
            code_challenge_method: "S256",
        })
        .send()
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?
        .error_for_status()
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?
        .json::&lt;ExchangeResponse&gt;()
        .await
        .map_err(|e| OAuthError::TokenExchange(e.to_string()))?;
    Ok(resp.key)
}

fn urlencoding_encode(s: &amp;str) -&gt; String {
    // Minimal percent-encoding sufficient for a loopback callback_url query
    // param (only ':' and '/' need escaping beyond what's already safe).
    s.replace(':', "%3A").replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urlencoding_escapes_colon_and_slash() {
        assert_eq!(
            urlencoding_encode("http://127.0.0.1:5555/callback"),
            "http%3A%2F%2F127.0.0.1%3A5555%2Fcallback"
        );
    }

    fn test_state(expected_state: &amp;str, tx: oneshot::Sender&lt;Result&lt;String, OAuthError&gt;&gt;) -&gt; Arc&lt;CallbackState&gt; {
        let (shutdown_tx, _shutdown_rx) = oneshot::channel();
        Arc::new(CallbackState {
            expected_state: expected_state.to_string(),
            tx: std::sync::Mutex::new(Some(tx)),
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
        })
    }

    #[tokio::test]
    async fn callback_handler_rejects_state_mismatch() {
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);
        let query = Query(CallbackQuery {
            code: Some("some-code".to_string()),
            state: Some("wrong-state".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert!(matches!(result, Err(OAuthError::StateMismatch)));
    }

    #[tokio::test]
    async fn callback_handler_accepts_matching_state() {
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: Some("expected-123".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert_eq!(result.unwrap(), "real-code");
    }

    #[tokio::test]
    async fn callback_handler_accepts_missing_state_param() {
        // Regression test for the state-leniency fix: OpenRouter's OAuth
        // docs don't document echoing `state` back, so absence must not be
        // treated as a failure — only an explicit wrong value should be.
        let (tx, rx) = oneshot::channel();
        let state = test_state("expected-123", tx);
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: None,
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert_eq!(result.unwrap(), "real-code");
    }

    #[tokio::test]
    async fn callback_handler_signals_shutdown_after_responding() {
        let (tx, rx) = oneshot::channel();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let state = Arc::new(CallbackState {
            expected_state: "expected-123".to_string(),
            tx: std::sync::Mutex::new(Some(tx)),
            shutdown_tx: std::sync::Mutex::new(Some(shutdown_tx)),
        });
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: Some("expected-123".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let _ = rx.await.expect("tx sent");
        shutdown_rx.await.expect("shutdown signal sent after response was built");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-oauth-pkce -- --nocapture`
Expected: FAIL initially on any typo/signature mismatch against the actual `axum`/`reqwest` versions pinned in this workspace — if `axum::serve`, `.with_graceful_shutdown`, or the `Query`/`State` extractor APIs differ from what's shown here (workspace pins `axum = "0.8"` per the earlier audit — this code targets that API shape, and `axum::serve`/`with_graceful_shutdown`/`Query`/`State`/`Router::with_state` were all independently web-verified as current for 0.8), fix signature mismatches against the compiler's actual errors rather than the snippet above.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-oauth-pkce -- --nocapture`
Expected: PASS — all tests across `pkce.rs` and `openrouter.rs` (`urlencoding_escapes_colon_and_slash`, `callback_handler_rejects_state_mismatch`, `callback_handler_accepts_matching_state`, `callback_handler_accepts_missing_state_param`, `callback_handler_signals_shutdown_after_responding`, plus `pkce.rs`'s 4). Don't hardcode an expected total count in the actual run — just confirm everything present is green.

Note: `run_openrouter_flow()` itself (the full end-to-end function) is **not** covered by these tests — it requires a live browser and a real OpenRouter round-trip, which is out of scope for unit tests. It gets integration coverage in Task 10/11 via the CLI/GUI commands that call it, mocked at the HTTP-client boundary if this workspace has an established `reqwest` mocking convention (check for `wiremock` in `Cargo.toml` — the vox-llm-egress crate's tests were noted as using `wiremock` per project history; replicate that pattern for `exchange_code` specifically, as a follow-up test, rather than trying to mock the loopback+browser parts).

- [ ] **Step 4: Add a `wiremock`-based test for `exchange_code`**

Check `crates/vox-llm-egress/Cargo.toml`'s `[dev-dependencies]` for the exact `wiremock` version pin, add the same to `vox-oauth-pkce/Cargo.toml`'s `[dev-dependencies]`, then add:

```rust
    #[tokio::test]
    async fn exchange_code_returns_key_on_success() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("POST"))
            .and(wiremock::matchers::path("/api/v1/auth/keys"))
            .respond_with(wiremock::ResponseTemplate::new(200).set_body_json(serde_json::json!({"key": "sk-or-test-key"})))
            .mount(&amp;server)
            .await;

        // exchange_code hardcodes OPENROUTER_TOKEN_EXCHANGE_URL — for this
        // test to work, extract the URL as a parameter (refactor
        // exchange_code to exchange_code_at(url, code, verifier) and have
        // exchange_code call it with the constant). Do this refactor as part
        // of this step, then verify both the constant-based public fn and
        // this test compile.
        let result = exchange_code_at(&amp;format!("{}/api/v1/auth/keys", server.uri()), "test-code", "test-verifier").await;
        assert_eq!(result.unwrap(), "sk-or-test-key");
    }
```

(This step requires refactoring `exchange_code` into `exchange_code(code, verifier)` calling `exchange_code_at(OPENROUTER_TOKEN_EXCHANGE_URL, code, verifier)` — do this refactor, re-run all `vox-oauth-pkce` tests to confirm no regression, then add the new test above.)

- [ ] **Step 5: Run full crate test suite**

Run: `cargo test -p vox-oauth-pkce -- --nocapture`
Expected: PASS, all tests including the new wiremock one

- [ ] **Step 6: Commit**

```bash
git add crates/vox-oauth-pkce/
git commit -m "feat(vox-oauth-pkce): OpenRouter loopback PKCE flow driver"
```

### Task 10: CLI entry point — `vox secrets login --provider openrouter --oauth`

**Files:**
- Modify: `crates/vox-cli/src/commands/secrets.rs:126-185` (the `SecretsCmd` enum and its `run` dispatch)
- Modify: `crates/vox-cli/Cargo.toml` (add `vox-oauth-pkce = { workspace = true }` dependency)
- Test: `crates/vox-cli/src/commands/secrets.rs` inline, or wherever this file's existing tests live (check for a sibling `secrets_test.rs`/inline module first)

- [ ] **Step 1: Add the dependency**

In `crates/vox-cli/Cargo.toml`, add to `[dependencies]`: `vox-oauth-pkce = { workspace = true }` (alongside the other `vox-*` workspace deps already listed).

- [ ] **Step 2: Write the failing test**

**⚠️ Correction from the second audit round**: `SecretsCmd` only derives `#[derive(Subcommand, Debug)]` (`secrets.rs:126-127`), not `clap::Parser` — `SecretsCmd::try_parse_from(...)` is a `Parser`-trait method and will **not compile** on a `Subcommand`-only type. `SecretsCmd` is used flattened into the top-level CLI struct: `crates/vox-cli/src/lib.rs:172-177` has `Secrets { #[command(subcommand)] cmd: commands::secrets::SecretsCmd }` inside the top-level `Commands` enum. The test below parses through that real top-level type instead.

First confirm the exact top-level `Parser` struct name and how it wraps `Commands`:

```bash
grep -n "derive(Parser\|struct.*Cli\|enum Commands" crates/vox-cli/src/lib.rs | head -10
```

Add to `crates/vox-cli/src/commands/secrets.rs` (find or create the test module), substituting the real top-level struct/field names found above for the placeholders `Cli`/`.command`:

```rust
#[cfg(test)]
mod oauth_login_cli_tests {
    use clap::Parser;

    #[test]
    fn oauth_flag_parses_on_login_subcommand() {
        // Parse through the REAL top-level CLI struct (SecretsCmd itself only
        // derives Subcommand, not Parser — it cannot be parsed standalone).
        // Substitute the actual struct/enum/field names confirmed via the
        // grep above; this is illustrative, not copy-paste-exact, since this
        // plan doesn't have the top-level type's real name verified.
        let cli = crate::Cli::try_parse_from([
            "vox", "secrets", "login", "--oauth", "--provider", "openrouter",
        ])
        .expect("parses");
        match cli.command {
            crate::Commands::Secrets { cmd: super::SecretsCmd::Login { oauth, provider, .. } } =&gt; {
                assert!(oauth);
                assert_eq!(provider.as_deref(), Some("openrouter"));
            }
            _ =&gt; panic!("expected Commands::Secrets{{ cmd: SecretsCmd::Login }}"),
        }
    }
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-cli oauth_flag_parses_on_login_subcommand -- --nocapture`
Expected: FAIL — `SecretsCmd::Login` doesn't have `oauth`/`provider` fields yet

- [ ] **Step 4: Add the flags and dispatch**

In `crates/vox-cli/src/commands/secrets.rs`, change the `Login` variant:

```rust
    /// Sign in: configure vault URL/token and optional Secrets account/backend,
    /// or provision a free OpenRouter key via in-app OAuth (`--oauth`).
    #[command(name = "login")]
    Login {
        #[command(flatten)]
        args: crate::commands::login_shared::LoginArgs,
        /// Provision a provider API key via in-app OAuth instead of vault login.
        #[arg(long, default_value_t = false)]
        oauth: bool,
        /// Provider to authenticate with when `--oauth` is set. Only "openrouter" is supported today.
        #[arg(long)]
        provider: Option&lt;String&gt;,
    },
```

And in `run`, change the `Login` match arm:

```rust
        SecretsCmd::Login { args, oauth, provider } =&gt; {
            if oauth {
                let provider = provider.as_deref().unwrap_or("openrouter");
                if provider != "openrouter" {
                    anyhow::bail!("--oauth only supports --provider openrouter today");
                }
                let key = vox_oauth_pkce::openrouter::run_openrouter_flow()
                    .await
                    .map_err(|e| anyhow::anyhow!("OAuth flow failed: {e}"))?;
                vox_secrets::set_registry_token("openrouter", &amp;key, None)
                    .map_err(|e| anyhow::anyhow!("failed to store key: {e}"))?;
                println!("OpenRouter API key provisioned and stored.");
                Ok(())
            } else {
                crate::commands::login_shared::run_login(args.into()).await
            }
        }
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-cli oauth_flag_parses_on_login_subcommand -- --nocapture`
Expected: PASS

- [ ] **Step 6: Run the full `vox-cli` test suite (regression gate)**

Run: `cargo test -p vox-cli`
Expected: all pre-existing tests still PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-cli/src/commands/secrets.rs crates/vox-cli/Cargo.toml
git commit -m "feat(vox-cli): add vox secrets login --oauth --provider openrouter"
```

### Task 11: Tauri command — `oauth_login_openrouter`

**Files:**
- Create: `crates/vox-gui/src/commands/oauth.rs`
- Modify: `crates/vox-gui/src/commands/mod.rs` (add `pub mod oauth;`)
- Modify: `crates/vox-gui/src/main.rs` (register in `generate_handler!`, near the other `commands::secrets::*` entries)
- Modify: `crates/vox-gui/Cargo.toml` (add `vox-oauth-pkce = { workspace = true }`)
- Test: `crates/vox-gui/src/commands/oauth.rs` inline

- [ ] **Step 1: Add the dependency**

In `crates/vox-gui/Cargo.toml`, add `vox-oauth-pkce = { workspace = true }` to `[dependencies]`.

**⚠️ Corrections from the second audit round:**
1. `vox_secrets::set_registry_token` returns `Result&lt;std::path::PathBuf, SecretError&gt;` (verified against `crates/vox-secrets/src/lib.rs:433-438`), **not** `Result&lt;(), SecretError&gt;`. The original draft's `Ok(()) =&gt; ...` match arm would not compile. Fixed below.
2. The design spec's original wording said this flow should persist via `vox_secrets::set_secret(SecretId::OpenRouterApiKey, value, SecretKind::OAuthRefreshToken)` — that function/kind pairing isn't actually how `OPENROUTER_API_KEY` is stored today; the real, already-wired path (confirmed via the existing GUI `set_secret` command's own implementation) is `set_registry_token("openrouter", ...)`. This task uses the real path, and the design spec should be treated as superseded on this specific point — `set_registry_token` is correct, not a workaround.
3. The DTO now carries the auth URL on `BrowserOpen` failure (Task 9's fix), so the wizard can offer a copyable fallback link instead of a dead end.
4. The command now takes an `AppHandle` so it can refocus the Vox window after a successful callback — a browser tab saying "you can close this" with no nudge back to the app is a real, if minor, UX gap.
5. The store/error-mapping logic is split into small, independently unit-testable pure functions (`map_store_result`, `map_flow_error`) rather than being buried inline in the async command — the original draft's only test covered DTO JSON serialization, never the actual new behavior (secret storage, error mapping). These two functions now get real coverage; only the top-level async orchestration (which needs a live browser + OpenRouter round-trip) remains outside unit-test scope, same acknowledged limitation as `run_openrouter_flow()` itself in Task 9.

- [ ] **Step 2: Write the failing tests**

Create `crates/vox-gui/src/commands/oauth.rs`:

```rust
//! Tauri commands for in-app OAuth key provisioning (free-tier onboarding).

use serde::Serialize;
use tauri::{command, AppHandle, Manager};
use vox_oauth_pkce::openrouter::OAuthError;
use vox_secrets::SecretError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthLoginResultDto {
    pub success: bool,
    pub error: Option&lt;String&gt;,
    /// Set only when the browser failed to open automatically — lets the
    /// caller show a copyable/clickable fallback link instead of dead-ending.
    pub fallback_url: Option&lt;String&gt;,
}

/// Map the OAuth flow's own error into the DTO's error/fallback_url pair.
/// Pure, no I/O — independently testable without a live browser.
fn map_flow_error(e: &amp;OAuthError) -&gt; (String, Option&lt;String&gt;) {
    match e {
        OAuthError::BrowserOpen { url, .. } =&gt; (e.to_string(), Some(url.clone())),
        _ =&gt; (e.to_string(), None),
    }
}

/// Map `set_registry_token`'s real return type (`Result&lt;PathBuf, SecretError&gt;`
/// — not `Result&lt;(), _&gt;`, a bug in an earlier draft of this task) into the
/// DTO. Pure, no I/O beyond what the caller already did — independently
/// testable with a pre-computed `Result`.
fn map_store_result(result: Result&lt;std::path::PathBuf, SecretError&gt;) -&gt; OAuthLoginResultDto {
    match result {
        Ok(_path) =&gt; OAuthLoginResultDto {
            success: true,
            error: None,
            fallback_url: None,
        },
        Err(e) =&gt; OAuthLoginResultDto {
            success: false,
            error: Some(format!("failed to store key: {e}")),
            fallback_url: None,
        },
    }
}

/// Run the OpenRouter loopback OAuth flow and persist the resulting key via
/// the same storage path `set_secret` already uses for OPENROUTER_API_KEY
/// (`vox_secrets::set_registry_token("openrouter", ...)`), so the GUI's
/// `list_secret_status`/`vox doctor` see it identically to a manually-entered key.
#[command]
pub async fn oauth_login_openrouter(app: AppHandle) -&gt; OAuthLoginResultDto {
    match vox_oauth_pkce::openrouter::run_openrouter_flow().await {
        Ok(key) =&gt; {
            let result = map_store_result(vox_secrets::set_registry_token("openrouter", &amp;key, None));
            if result.success {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.set_focus();
                }
            }
            result
        }
        Err(e) =&gt; {
            let (error, fallback_url) = map_flow_error(&amp;e);
            OAuthLoginResultDto {
                success: false,
                error: Some(error),
                fallback_url,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_dto_serializes_camel_case() {
        let dto = OAuthLoginResultDto {
            success: false,
            error: Some("timed out".to_string()),
            fallback_url: None,
        };
        let json = serde_json::to_string(&amp;dto).expect("serializes");
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"timed out\""));
    }

    #[test]
    fn map_store_result_ok_path_is_success() {
        // The real bug this test guards against: an earlier draft matched
        // `Ok(())` against `set_registry_token`'s actual `Result&lt;PathBuf,_&gt;`
        // return type, which does not compile. This asserts the PathBuf
        // case is handled, not discarded/mismatched.
        let dto = map_store_result(Ok(std::path::PathBuf::from("/fake/path")));
        assert!(dto.success);
        assert!(dto.error.is_none());
    }

    #[test]
    fn map_store_result_err_is_failure_with_message() {
        let dto = map_store_result(Err(SecretError::Io("disk full".to_string())));
        assert!(!dto.success);
        assert!(dto.error.unwrap().contains("disk full"));
    }

    #[test]
    fn map_flow_error_browser_open_carries_fallback_url() {
        let err = OAuthError::BrowserOpen {
            url: "https://openrouter.ai/auth?callback_url=...".to_string(),
            source: std::io::Error::new(std::io::ErrorKind::NotFound, "no browser"),
        };
        let (_, fallback_url) = map_flow_error(&amp;err);
        assert_eq!(fallback_url.as_deref(), Some("https://openrouter.ai/auth?callback_url=..."));
    }

    #[test]
    fn map_flow_error_timeout_has_no_fallback_url() {
        let err = OAuthError::TimedOut(std::time::Duration::from_secs(120));
        let (_, fallback_url) = map_flow_error(&amp;err);
        assert!(fallback_url.is_none());
    }
}
```

Add `pub mod oauth;` to `crates/vox-gui/src/commands/mod.rs` (alphabetically among the existing `pub mod` lines).

(Confirm `SecretError`'s exact variant names before finalizing `map_store_result_err_is_failure_with_message` — this plan cites `SecretError::Io(String)` from `crates/vox-secrets/src/errors.rs:4-15`, verified during the first research round; re-check that file if it's drifted.)

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test -p vox-gui oauth:: -- --nocapture`
Expected: FAIL — either a compile error (dependency/module wiring from Step 1 not yet done — confirm that first) or, once it compiles, `map_store_result_ok_path_is_success` and `map_flow_error_browser_open_carries_fallback_url` are the two tests that actually exercise new logic (as opposed to `result_dto_serializes_camel_case`, which is a thin serialization check) — make sure those two specifically are asserting real behavior, not vacuously passing.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vox-gui oauth:: -- --nocapture`
Expected: PASS (5 tests)

- [ ] **Step 5: Register in `generate_handler!`**

In `crates/vox-gui/src/main.rs`, add `commands::oauth::oauth_login_openrouter,` immediately after the existing `commands::secrets::*` block (around line 227 per the earlier audit).

- [ ] **Step 6: Full build check + regression gate**

Run: `cargo check -p vox-gui` (validates the `generate_handler!` registration syntax, which isn't otherwise unit-testable), then `cargo test -p vox-gui` (regression gate — this task edits `main.rs`'s shared handler-registration list and `commands/mod.rs`'s shared module list, both touched by every other GUI command; an earlier draft of this task stopped at `cargo check`, which wouldn't have caught a regression in existing tests).
Expected: both clean/green

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/src/commands/oauth.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(vox-gui): add oauth_login_openrouter Tauri command"
```

### Task 11b: `verify_openrouter_key` Tauri command — real verification, not just "a string got stored"

**Why this task exists**: a gap found in the second audit round — the wizard's "confirmation" screen (Task 15) originally said "You're set up" purely because `oauth_login_openrouter` returned `success: true`, which per Task 11 only means the PKCE exchange returned *some* key string and `set_registry_token` wrote it without erroring. A malformed, immediately-revoked, or provider-side-broken key would still show "you're set up," and the user's very next real chat message would fail with no connection back to "the wizard said this was fine." This task adds a real, minimal connectivity check between "key stored" and "confirmation shown."

**Files:**
- Create/modify: `crates/vox-gui/src/commands/oauth.rs` (new command in the same file as Task 11)
- Test: same file, inline

- [ ] **Step 1: Confirm the actual lightweight verification endpoint before writing any code**

Do not guess at an OpenRouter endpoint path — confirm one against OpenRouter's live API docs (`https://openrouter.ai/docs/api-reference/...` — check their reference for a cheap, read-only, key-scoped endpoint, e.g. a "get current key info / credits" GET request that requires auth but doesn't consume a real completion). This plan deliberately does not hardcode a guessed path here, following the same discipline Task 9's OAuth endpoints were held to (web-verified before being written into code, not assumed).

- [ ] **Step 2: Write the failing test**

Once Step 1 confirms the real endpoint, add (using the same `wiremock` pattern already established in Task 9):

```rust
#[cfg(test)]
mod verify_tests {
    use super::*;

    #[tokio::test]
    async fn verify_returns_true_on_200() {
        let server = wiremock::MockServer::start().await;
        // Mount a mock at the endpoint confirmed in Step 1, respond 200.
        // let result = verify_key_at(&amp;server.uri(), "fake-key").await;
        // assert!(result);
    }

    #[tokio::test]
    async fn verify_returns_false_on_401() {
        let server = wiremock::MockServer::start().await;
        // Mount a mock at the endpoint confirmed in Step 1, respond 401.
        // let result = verify_key_at(&amp;server.uri(), "fake-key").await;
        // assert!(!result);
    }
}
```

(The commented-out bodies are placeholders for the *test call site only*, filled in once Step 1's real endpoint and this task's real function name are settled — this is not the "no placeholders" violation the plan otherwise forbids, since the surrounding structure and assertions are concrete and the only unknown is an external fact Step 1 resolves first.)

Run the scoped test.
Expected: FAIL — `verify_key_at` doesn't exist yet

- [ ] **Step 3: Implement the command**

Add a `verify_key_at(base_url: &amp;str, key: &amp;str) -&gt; bool` (testable against a mock base URL, same pattern as Task 9's `exchange_code`/`exchange_code_at` split) plus a public `#[command] pub async fn verify_openrouter_key() -&gt; bool` that resolves the stored key via `vox_secrets::resolve_secret(SecretId::OpenRouterApiKey)` and calls `verify_key_at` with the real endpoint from Step 1.

- [ ] **Step 4: Run test to verify it passes**

Same command as Step 2.
Expected: PASS

- [ ] **Step 5: Register in `generate_handler!` and re-run the Task 11 regression gate**

Same registration pattern as Task 11 Step 5, then re-run `cargo test -p vox-gui` (this file now has two commands in it — a regression here affects both).

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/oauth.rs crates/vox-gui/src/main.rs
git commit -m "feat(vox-gui): add verify_openrouter_key command for real post-OAuth verification"
```

### Task 12: `vox doctor` — distinguish `NoCredential` vs `RateLimited` (on-demand diagnostic only — see Task 12b for the live-path fix)

**Scope note**: this task only improves what `vox doctor` reports when a user manually runs it. It does **not**, by itself, fix what a real user experiences mid-chat when the free tier's rate limit hits — that's Task 12b, and it's the more important of the two per the design spec's own risk register (risk #3, rated Critical). Do not treat this task as having closed that risk on its own.

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs`
- Investigate first: `crates/vox-llm-egress/src/throttle.rs` (the 429/retry-after handling referenced by `LlmSettingsSection`'s "429 retry attempts" config) and wherever its errors propagate to a dispatch caller

- [ ] **Step 1: Find how a 429 currently surfaces to a caller**

Run:
```bash
grep -rn "429\|RateLimited\|retry_after" crates/vox-llm-egress/src/throttle.rs crates/vox-llm-egress/src/lib.rs
```

Read the matched error type/variant definitions. This determines whether "rate limited" is already a distinguishable error today (likely yes, given the retry-after-header handling noted in project history) or whether it collapses into a generic HTTP-error string by the time it reaches callers.

- [ ] **Step 2: Write a test capturing current behavior**

Before changing anything, write a test (in whichever egress test file already covers throttle behavior — search `crates/vox-llm-egress/src` for existing `#[cfg(test)]` throttle tests) that asserts what error type/string a 429 response currently produces, using this crate's existing `wiremock` test pattern (confirmed present per Task 9). This test should PASS immediately (it documents current behavior, it's not a new feature yet) — its purpose is to lock in the exact error shape before this task's remaining steps, and Task 12b, depend on it.

- [ ] **Step 3: Run the behavior-capture test**

Run: `cargo test -p vox-llm-egress -- --nocapture` (scoped to the new test)
Expected: PASS (this confirms your understanding of the current error shape is correct before proceeding)

- [ ] **Step 4: Write the failing test for the new doctor behavior — BEFORE implementing it**

**⚠️ Ordering correction from the second audit round**: an earlier draft of this task implemented the detection helper first and only wrote its test afterward — real test-after, not TDD. Write the test first this time. Based on Step 1-3's findings, add a test to `llm_routing.rs` (analogous to Task 6's `reports_budget_caps_in_detail_string`) asserting the doctor check produces a distinct rate-limit-shaped `Check` (name `"LLM routing (rate limit)"`, message containing `"free tier limit"` case-insensitive) when the underlying resolver reports that condition — construct the test via whatever mocking seam Steps 1-2 revealed (a `wiremock` 429 response feeding through the real resolution path if that's testable at this layer, or a narrower unit test on just a soon-to-exist helper function if full end-to-end mocking isn't practical here).

Run: `cargo test -p vox-cli` (scoped to the new test name)
Expected: FAIL — the distinct rate-limit `Check` doesn't exist yet, the doctor still reports the generic FAIL

- [ ] **Step 5: Now implement the detection helper**

Add a small helper in `llm_routing.rs` (or wherever is appropriate given the real error type found in Step 1) that checks a resolved error for the rate-limit signal and returns the distinct `Check` from Step 4's test — e.g. `"OpenRouter free tier limit reached — resets at &lt;time if available&gt;, add your own key or wait"` instead of the generic FAIL. The exact implementation depends entirely on Step 1's findings — do not write this code blind; let the discovered error type drive the match arm.

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p vox-cli` (scoped to the new test name)
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-llm-egress/ crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs
git commit -m "feat: distinguish rate-limited from no-credential in LLM routing diagnostics"
```

### Task 12b: Wire `RateLimited` into the live dispatch path (closes design-spec risk #3 for real, not just diagnostically)

**Why this task exists**: two independent audit passes both found the same critical gap. The design spec's §2.3 error taxonomy promises `RateLimited` gets "distinct copy in both CLI and GUI," but as originally planned, Task 12 only ever touched `vox doctor` — an on-demand diagnostic a user has to manually invoke. A real user who exhausts OpenRouter's free-tier 50/day cap mid-chat would still hit whatever generic error `vox-llm-egress` produces today, at the exact chokepoint Task 4 already wired the budget guard into. Wiring both at the same chokepoint, in the same task family, is deliberate — they're the same class of problem (a dispatch-time condition that needs a distinct, actionable message instead of a generic failure).

**Files:**
- Modify: `crates/vox-orchestrator-mcp/src/chat_model_resolve.rs` (same function Task 4 already touched, `resolve_chat_llm_model` — or wherever the actual HTTP dispatch's error surfaces if it's not visible at the resolve layer; confirm via Step 1 below)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/` (same catch block Task 5 already touched)
- Test: same files as above

- [ ] **Step 1: Confirm where the rate-limit error actually needs to be caught**

Task 12 Step 1 already identified the real error type/shape at the `vox-llm-egress` throttle layer. Confirm whether that error propagates all the way up through `resolve_chat_llm_model` (Task 4's chokepoint) as a distinguishable type/variant, or whether it gets stringified/erased somewhere in between:

```bash
grep -rn "throttle\|RateLimit\|429" crates/vox-orchestrator-mcp/src/chat_model_resolve.rs crates/vox-orchestrator-mcp/src/llm_bridge/
```

If it's already distinguishable at `resolve_chat_llm_model`, wire the detection there, right alongside Task 4's `budget_guard::check` call. If it only becomes visible deeper in the actual HTTP dispatch (past model resolution, at request-send time), this task's scope shifts to wherever that real dispatch call lives — do not force the check into `resolve_chat_llm_model` if the real signal isn't available there; find the real one.

- [ ] **Step 2: Write the failing test**

At whichever real location Step 1 identifies, write a test asserting a 429/rate-limited condition produces a distinguishable error (containing "rate limit" or "free tier", case-insensitive) as opposed to the generic `NoCredential`-shaped message, using the same `wiremock` pattern Task 12 Step 2 already established for capturing this error shape.

Run the appropriate `cargo test -p &lt;crate&gt;` command scoped to this test.
Expected: FAIL — no distinct handling exists at this location yet

- [ ] **Step 3: Wire the detection**

Add the rate-limit check at the location confirmed in Step 1, returning a distinct error (reuse whatever error-string convention `budget_guard`/`BudgetGuardError` established in Task 3-4 for consistency, or a comparably small dedicated type if this location isn't already using that pattern) containing enough detail for the caller to show "free tier limit reached, resets at X — add your own key or wait" rather than a generic failure.

- [ ] **Step 4: Run test to verify it passes**

Same command as Step 2.
Expected: PASS

- [ ] **Step 5: Surface it distinctly in the GUI (mirrors Task 5)**

In the same Chat-surface catch block Task 5 already modified, add a second distinguishing check (alongside the "Daily budget"/"Session budget" prefix check from Task 5) for the rate-limit error's distinguishing text from Step 3, with its own toast title (e.g. `'Free tier limit reached'`) and a body pointing at adding a personal key. Write the Playwright test for this first (same mock-invoke pattern as Task 5 Step 2), watch it fail, then implement.

- [ ] **Step 6: Run the full Chat surface spec (regression gate)**

Same as Task 5 Step 6 — this catch block is now shared across three error classes (generic, budget, rate-limit); a regression here affects all of them.
Expected: all pre-existing and new Chat-surface tests PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/ crates/vox-gui/ui/src/components/surfaces/Chat/
git commit -m "feat: surface OpenRouter free-tier rate-limit as a distinct live error, not just a doctor diagnostic"
```

---

## Phase 3 — Onboarding wizard GUI

### Task 13: Export `KeysSecretsSection` for reuse

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx:398`
- Test: existing `settings.spec.ts` (regression only — no new test needed for a pure export change)

- [ ] **Step 1: Change the function declaration**

In `SettingsView.tsx`, change:

```tsx
function KeysSecretsSection({ pushToast, gamifyEnabled }: { pushToast: (t: Toast) =&gt; void; gamifyEnabled?: boolean }) {
```

to:

```tsx
export function KeysSecretsSection({ pushToast, gamifyEnabled }: { pushToast: (t: Toast) =&gt; void; gamifyEnabled?: boolean }) {
```

- [ ] **Step 2: Run the existing Settings test suite (regression gate)**

Run the project's Playwright command scoped to `settings.spec.ts` (check `package.json` for the exact script name).
Expected: PASS (a pure export addition should not change any existing behavior)

- [ ] **Step 3: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
git commit -m "refactor(vox-gui-ui): export KeysSecretsSection for reuse by onboarding wizard"
```

### Task 14: `useLocalStorage` dismissal state for the wizard

**Files:**
- Read first: `crates/vox-gui/ui/src/hooks/useLocalStorage.ts` (get its exact signature before using it)
- Create: `crates/vox-gui/ui/src/components/surfaces/Onboarding/useOnboardingGate.ts`
- Test: `crates/vox-gui/ui/src/components/surfaces/Onboarding/useOnboardingGate.test.ts`

- [ ] **Step 1: Read the exact `useLocalStorage` signature**

Run:
```bash
sed -n '1,40p' crates/vox-gui/ui/src/hooks/useLocalStorage.ts
```

Confirm its exact generic signature (it's used elsewhere as `useLocalStorage&lt;Record&lt;string, boolean&gt;&gt;('vox_secrets_groups', {})`, implying `useLocalStorage&lt;T&gt;(key: string, initial: T): [T, (next: T) =&gt; void]` or similar — verify against the real file before writing Step 2).

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/ui/src/components/surfaces/Onboarding/useOnboardingGate.test.ts` (mirror whatever test framework/renderer the project's existing hook tests use — check for a sibling `*.test.ts` for another hook, e.g. search `crates/vox-gui/ui/src/hooks/*.test.ts`, and copy its exact import/setup pattern):

```ts
import { describe, expect, it, beforeEach } from 'vitest';
import { renderHook, act } from '@testing-library/react';
import { useOnboardingGate } from './useOnboardingGate';

describe('useOnboardingGate', () =&gt; {
  beforeEach(() =&gt; {
    localStorage.clear();
  });

  it('shows the wizard when zero secrets, zero local models, and not dismissed', () =&gt; {
    const { result } = renderHook(() =&gt; useOnboardingGate({ secretCount: 0, localModelCount: 0 }));
    expect(result.current.shouldShow).toBe(true);
  });

  it('hides the wizard when at least one secret is configured', () =&gt; {
    const { result } = renderHook(() =&gt; useOnboardingGate({ secretCount: 1, localModelCount: 0 }));
    expect(result.current.shouldShow).toBe(false);
  });

  it('hides the wizard when at least one local model is available', () =&gt; {
    const { result } = renderHook(() =&gt; useOnboardingGate({ secretCount: 0, localModelCount: 1 }));
    expect(result.current.shouldShow).toBe(false);
  });

  it('hides the wizard after dismiss() is called, and persists across remounts', () =&gt; {
    const { result, rerender } = renderHook(() =&gt; useOnboardingGate({ secretCount: 0, localModelCount: 0 }));
    expect(result.current.shouldShow).toBe(true);
    act(() =&gt; result.current.dismiss());
    rerender();
    expect(result.current.shouldShow).toBe(false);
  });

  it('replay() re-shows the wizard even with zero secrets/models afterward', () =&gt; {
    const { result, rerender } = renderHook(() =&gt; useOnboardingGate({ secretCount: 0, localModelCount: 0 }));
    act(() =&gt; result.current.dismiss());
    rerender();
    expect(result.current.shouldShow).toBe(false);
    act(() =&gt; result.current.replay());
    rerender();
    expect(result.current.shouldShow).toBe(true);
  });
});
```

- [ ] **Step 3: Run test to verify it fails**

Run the project's vitest command scoped to this file (check `package.json`).
Expected: FAIL — `useOnboardingGate` doesn't exist yet

- [ ] **Step 4: Implement the hook**

Create `crates/vox-gui/ui/src/components/surfaces/Onboarding/useOnboardingGate.ts`, using the exact `useLocalStorage` signature confirmed in Step 1 (adjust the call below if that signature differs):

```ts
import { useLocalStorage } from '../../../hooks/useLocalStorage';

export interface OnboardingGateInput {
  secretCount: number;
  localModelCount: number;
}

export interface OnboardingGateResult {
  shouldShow: boolean;
  dismiss: () =&gt; void;
  replay: () =&gt; void;
}

/** Gate + persisted dismissal for the first-run onboarding wizard. */
export function useOnboardingGate({ secretCount, localModelCount }: OnboardingGateInput): OnboardingGateResult {
  const [dismissed, setDismissed] = useLocalStorage&lt;boolean&gt;('vox_onboarding_dismissed', false);

  const isFreshInstall = secretCount === 0 &amp;&amp; localModelCount === 0;
  const shouldShow = isFreshInstall &amp;&amp; !dismissed;

  return {
    shouldShow,
    dismiss: () =&gt; setDismissed(true),
    replay: () =&gt; setDismissed(false),
  };
}
```

- [ ] **Step 5: Run test to verify it passes**

Run the project's vitest command scoped to this file.
Expected: PASS (5 tests)

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Onboarding/
git commit -m "feat(vox-gui-ui): add useOnboardingGate hook for wizard visibility"
```

### Task 15: `OnboardingWizard` component — 3-path entry screen

**Corrections from the second audit round baked into the component below**: (1) success from `oauth_login_openrouter` no longer jumps straight to "you're set up" — it goes through a `verifying` screen that calls Task 11b's `verify_openrouter_key` first, since "a key string was stored" and "the key works" are different claims. (2) `BrowserOpen` failures now show a clickable fallback link (Task 9/11's `fallback_url` plumbing), not a dead-end error message. (3) i18n debt note: the ~15 English string literals below are still scattered inline rather than centralized into one object — this was flagged as unnecessary debt (English-only v1 is a legitimate scope call per the design spec, but centralizing costs nothing extra now and makes a future i18n pass strictly easier); consider grouping them into a single `const COPY = {...}` if time allows, but it's not a blocking requirement for this task.

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/Onboarding/OnboardingWizard.tsx`
- Test: `crates/vox-gui/ui/e2e/onboarding.spec.ts`

- [ ] **Step 1: Write the failing Playwright test**

Create `crates/vox-gui/ui/e2e/onboarding.spec.ts`, following `settings.spec.ts`'s exact mock pattern:

```ts
import { test, expect } from '@playwright/test';

test.describe('Onboarding wizard', () =&gt; {
  test('shows three entry paths for a zero-secret, zero-local-model install', async ({ page }) =&gt; {
    await page.addInitScript(() =&gt; {
      localStorage.removeItem('vox_onboarding_dismissed');
      (window as any).__TAURI_CALLS__ = [];
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string, args?: Record&lt;string, unknown&gt;) =&gt; {
          (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [];
          if (cmd === 'inference_provider_status') return [];
          if (cmd === 'get_command_catalog') return { generated_from: 'e2e-mock', entries: [] };
          if (cmd === 'get_action_manifest') return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_gui_preference') return null;
          if (cmd === 'set_gui_preference') return null;
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: /get started with vox/i })).toBeVisible({ timeout: 15_000 });
    await expect(page.getByRole('button', { name: /get a free key/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /i already have an api key/i })).toBeVisible();
    await expect(page.getByRole('button', { name: /use a local model/i })).toBeVisible();
  });

  test('does not show when a secret is already configured', async ({ page }) =&gt; {
    await page.addInitScript(() =&gt; {
      localStorage.removeItem('vox_onboarding_dismissed');
      (window as any).__TAURI_INTERNALS__ = {
        invoke: async (cmd: string) =&gt; {
          if (cmd === 'get_initial_view') return 'chat';
          if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
          if (cmd === 'list_secret_status') return [{ id: 'OPENROUTER_API_KEY', isPresent: true }];
          if (cmd === 'inference_provider_status') return [];
          if (cmd === 'get_command_catalog') return { generated_from: 'e2e-mock', entries: [] };
          if (cmd === 'get_action_manifest') return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
          if (cmd === 'get_routing_summary_live') return { decision_preview: null };
          if (cmd === 'get_gui_preference') return null;
          if (cmd === 'set_gui_preference') return null;
          if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
          return null;
        },
      };
    });

    await page.goto('/');
    await expect(page.getByRole('heading', { name: /get started with vox/i })).not.toBeVisible();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run the project's Playwright command scoped to `onboarding.spec.ts`.
Expected: FAIL — the wizard component doesn't exist/isn't mounted yet

- [ ] **Step 3: Implement the component**

Create `crates/vox-gui/ui/src/components/surfaces/Onboarding/OnboardingWizard.tsx`:

```tsx
import React, { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { useOnboardingGate } from './useOnboardingGate';
import { KeysSecretsSection } from '../Settings/SettingsView';

interface SecretStatusRow {
  id: string;
  isPresent: boolean;
}

interface ProviderStatusRow {
  provider: string;
  is_local: boolean;
  local_reachable: boolean | null;
}

type WizardScreen = 'entry' | 'oauth-in-progress' | 'verifying' | 'has-key' | 'local-model' | 'budget' | 'confirmation';

export function OnboardingWizard({ pushToast, gamifyEnabled }: { pushToast: (t: any) =&gt; void; gamifyEnabled?: boolean }) {
  const [secretCount, setSecretCount] = useState&lt;number | null&gt;(null);
  const [localModelCount, setLocalModelCount] = useState&lt;number | null&gt;(null);
  const [screen, setScreen] = useState&lt;WizardScreen&gt;('entry');
  const [oauthError, setOauthError] = useState&lt;string | null&gt;(null);
  const [oauthFallbackUrl, setOauthFallbackUrl] = useState&lt;string | null&gt;(null);

  useEffect(() =&gt; {
    (async () =&gt; {
      try {
        const secrets = await invoke&lt;SecretStatusRow[]&gt;('list_secret_status');
        setSecretCount(secrets.filter(s =&gt; s.isPresent).length);
      } catch {
        setSecretCount(0);
      }
      try {
        const providers = await invoke&lt;ProviderStatusRow[]&gt;('inference_provider_status');
        setLocalModelCount(providers.filter(p =&gt; p.is_local &amp;&amp; p.local_reachable === true).length);
      } catch {
        setLocalModelCount(0);
      }
    })();
  }, []);

  const gate = useOnboardingGate({
    secretCount: secretCount ?? 0,
    localModelCount: localModelCount ?? 0,
  });

  if (secretCount === null || localModelCount === null || !gate.shouldShow) {
    return null;
  }

  const startOAuth = async () =&gt; {
    setScreen('oauth-in-progress');
    setOauthError(null);
    setOauthFallbackUrl(null);
    try {
      const result = await invoke&lt;{ success: boolean; error: string | null; fallbackUrl: string | null }&gt;('oauth_login_openrouter');
      if (!result.success) {
        setOauthError(result.error ?? 'Unknown error');
        setOauthFallbackUrl(result.fallbackUrl ?? null);
        setScreen('entry');
        return;
      }
      // Key was stored — now actually verify it works before claiming success
      // (Task 11b). "We received a key string" and "the key works" are not
      // the same thing, and conflating them was a real gap in an earlier draft.
      setScreen('verifying');
      const works = await invoke&lt;boolean&gt;('verify_openrouter_key').catch(() =&gt; false);
      if (works) {
        setScreen('budget');
      } else {
        setOauthError('Key saved, but a test request failed — check your connection and try again.');
        setOauthFallbackUrl(null);
        setScreen('entry');
      }
    } catch (err) {
      setOauthError(String(err));
      setOauthFallbackUrl(null);
      setScreen('entry');
    }
  };

  return (
    &lt;div role="dialog" aria-modal="true" aria-labelledby="onboarding-wizard-heading" className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      &lt;div className="max-w-lg w-full rounded-xl bg-surface-primary p-6 shadow-2xl"&gt;
        {screen === 'entry' &amp;&amp; (
          &lt;&gt;
            &lt;h1 id="onboarding-wizard-heading" className="font-display text-xl font-semibold text-text-primary"&gt;
              Get started with Vox
            &lt;/h1&gt;
            &lt;p className="mt-2 text-sm text-text-muted"&gt;
              Vox needs a model to talk to. Pick whichever fits you best — you can change this anytime in Settings.
            &lt;/p&gt;
            {oauthError &amp;&amp; (
              &lt;div role="alert" className="mt-3 rounded-lg border border-red-500/40 bg-red-500/10 px-3 py-2 text-[12px] text-red-300"&gt;
                {oauthError}
                {oauthFallbackUrl &amp;&amp; (
                  &lt;&gt;
                    {' '}
                    &lt;a href={oauthFallbackUrl} target="_blank" rel="noreferrer" className="underline"&gt;
                      Open this link manually
                    &lt;/a&gt;
                    .
                  &lt;/&gt;
                )}
              &lt;/div&gt;
            )}
            &lt;div className="mt-4 flex flex-col gap-2"&gt;
              &lt;button type="button" onClick={startOAuth} className="rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90"&gt;
                Get a free key
              &lt;/button&gt;
              &lt;button type="button" onClick={() =&gt; setScreen('has-key')} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle"&gt;
                I already have an API key
              &lt;/button&gt;
              &lt;button type="button" onClick={() =&gt; setScreen('local-model')} className="rounded-lg border border-border-subtle px-4 py-2 text-sm hover:bg-overlay-subtle"&gt;
                Use a local model
              &lt;/button&gt;
            &lt;/div&gt;
            &lt;button type="button" onClick={gate.dismiss} className="mt-4 text-[11px] text-text-muted hover:text-text-primary"&gt;
              Skip for now
            &lt;/button&gt;
          &lt;/&gt;
        )}
        {screen === 'oauth-in-progress' &amp;&amp; (
          &lt;&gt;
            &lt;h1 className="font-display text-xl font-semibold text-text-primary"&gt;Waiting for OpenRouter…&lt;/h1&gt;
            &lt;p className="mt-2 text-sm text-text-muted"&gt;
              A browser window opened — sign in or create a free OpenRouter account, then come back here.
            &lt;/p&gt;
          &lt;/&gt;
        )}
        {screen === 'verifying' &amp;&amp; (
          &lt;&gt;
            &lt;h1 className="font-display text-xl font-semibold text-text-primary"&gt;Checking your key…&lt;/h1&gt;
            &lt;p className="mt-2 text-sm text-text-muted"&gt;
              Confirming it actually works before we finish setup.
            &lt;/p&gt;
          &lt;/&gt;
        )}
        {screen === 'has-key' &amp;&amp; (
          &lt;&gt;
            &lt;h1 className="font-display text-xl font-semibold text-text-primary"&gt;Add your API key&lt;/h1&gt;
            &lt;div className="mt-4"&gt;
              &lt;KeysSecretsSection pushToast={pushToast} gamifyEnabled={gamifyEnabled} /&gt;
            &lt;/div&gt;
            &lt;button type="button" onClick={() =&gt; setScreen('budget')} className="mt-4 rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90"&gt;
              Done
            &lt;/button&gt;
          &lt;/&gt;
        )}
        {screen === 'local-model' &amp;&amp; (
          &lt;&gt;
            &lt;h1 className="font-display text-xl font-semibold text-text-primary"&gt;Use a local model&lt;/h1&gt;
            &lt;p className="mt-2 text-sm text-text-muted"&gt;
              Install &lt;a href="https://ollama.com/download" target="_blank" rel="noreferrer" className="text-brass underline"&gt;Ollama&lt;/a&gt;, pull a model, then come back — Vox will detect it automatically.
            &lt;/p&gt;
            &lt;button type="button" onClick={() =&gt; setScreen('budget')} className="mt-4 rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90"&gt;
              Done
            &lt;/button&gt;
          &lt;/&gt;
        )}
        {screen === 'budget' &amp;&amp; (
          &lt;BudgetSetupScreen onContinue={() =&gt; setScreen('confirmation')} /&gt;
        )}
        {screen === 'confirmation' &amp;&amp; (
          &lt;&gt;
            &lt;h1 className="font-display text-xl font-semibold text-text-primary"&gt;You're set up&lt;/h1&gt;
            &lt;p className="mt-2 text-sm text-text-muted"&gt;
              Auto mode picks a model based on cost and your usage history as it builds up.
            &lt;/p&gt;
            &lt;button type="button" onClick={gate.dismiss} className="mt-4 rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90"&gt;
              Start using Vox
            &lt;/button&gt;
          &lt;/&gt;
        )}
      &lt;/div&gt;
    &lt;/div&gt;
  );
}

interface UserConfigFieldRow {
  key: string;
  value: string;
}

/** Screen 3: review/edit the budget caps set in Phase 1, before finishing onboarding.
 * Reuses the existing `get_user_config`/`set_user_config` commands (Task 2's registry
 * entries already cover `daily_budget_usd`/`per_session_budget_usd`/`budget_warn_threshold_pct`) —
 * no new Tauri commands needed for this screen. */
function BudgetSetupScreen({ onContinue }: { onContinue: () =&gt; void }) {
  const [daily, setDaily] = useState('5');
  const [perSession, setPerSession] = useState('1');
  const [warnPct, setWarnPct] = useState('80');
  const [loaded, setLoaded] = useState(false);

  useEffect(() =&gt; {
    (async () =&gt; {
      try {
        const fields = await invoke&lt;UserConfigFieldRow[]&gt;('get_user_config');
        const byKey = Object.fromEntries(fields.map(f =&gt; [f.key, f.value]));
        if (byKey.daily_budget_usd) setDaily(byKey.daily_budget_usd);
        if (byKey.per_session_budget_usd) setPerSession(byKey.per_session_budget_usd);
        if (byKey.budget_warn_threshold_pct) setWarnPct(String(Number(byKey.budget_warn_threshold_pct) * 100));
      } finally {
        setLoaded(true);
      }
    })();
  }, []);

  const save = async () =&gt; {
    await invoke('set_user_config', { key: 'daily_budget_usd', value: daily });
    await invoke('set_user_config', { key: 'per_session_budget_usd', value: perSession });
    await invoke('set_user_config', { key: 'budget_warn_threshold_pct', value: String(Number(warnPct) / 100) });
    onContinue();
  };

  return (
    &lt;&gt;
      &lt;h1 className="font-display text-xl font-semibold text-text-primary"&gt;Set your spending limits&lt;/h1&gt;
      &lt;p className="mt-2 text-sm text-text-muted"&gt;
        This is Vox's own cap on spend — separate from any free-tier limit your provider applies. You'll get a warning before it blocks anything.
      &lt;/p&gt;
      {loaded &amp;&amp; (
        &lt;div className="mt-4 space-y-3"&gt;
          &lt;label className="block text-[11px] text-text-muted"&gt;
            Daily budget (USD)
            &lt;input type="number" min="0" step="0.5" value={daily} onChange={e =&gt; setDaily(e.target.value)} className="mt-1 w-full rounded-lg border border-border-subtle bg-transparent px-2 py-1 text-sm text-text-primary" /&gt;
          &lt;/label&gt;
          &lt;label className="block text-[11px] text-text-muted"&gt;
            Per-session budget (USD)
            &lt;input type="number" min="0" step="0.25" value={perSession} onChange={e =&gt; setPerSession(e.target.value)} className="mt-1 w-full rounded-lg border border-border-subtle bg-transparent px-2 py-1 text-sm text-text-primary" /&gt;
          &lt;/label&gt;
          &lt;label className="block text-[11px] text-text-muted"&gt;
            Warn me at (% of cap)
            &lt;input type="number" min="0" max="100" step="5" value={warnPct} onChange={e =&gt; setWarnPct(e.target.value)} className="mt-1 w-full rounded-lg border border-border-subtle bg-transparent px-2 py-1 text-sm text-text-primary" /&gt;
          &lt;/label&gt;
        &lt;/div&gt;
      )}
      &lt;button type="button" onClick={save} className="mt-4 rounded-lg bg-brass px-4 py-2 text-sm font-semibold text-black hover:bg-brass/90"&gt;
        Save and continue
      &lt;/button&gt;
    &lt;/&gt;
  );
}
```

- [ ] **Step 3b: Extend the Playwright test for the budget screen**

Add to `onboarding.spec.ts` (mocking `get_user_config` to return the three budget fields, and asserting `set_user_config` gets called with `daily_budget_usd` after clicking "Save and continue" on the budget screen reached via the "Use a local model" → "Done" path, which is the fastest route to it without mocking a real OAuth round-trip):

```ts
test('budget screen saves caps via set_user_config before finishing', async ({ page }) =&gt; {
  await page.addInitScript(() =&gt; {
    localStorage.removeItem('vox_onboarding_dismissed');
    (window as any).__TAURI_CALLS__ = [];
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string, args?: Record&lt;string, unknown&gt;) =&gt; {
        (window as any).__TAURI_CALLS__.push({ cmd, args: args ?? null });
        if (cmd === 'get_initial_view') return 'chat';
        if (cmd === 'get_build_info') return { version: '0.6.0', display: '0.6.0+build.test (abc123)' };
        if (cmd === 'list_secret_status') return [];
        if (cmd === 'inference_provider_status') return [];
        if (cmd === 'get_command_catalog') return { generated_from: 'e2e-mock', entries: [] };
        if (cmd === 'get_action_manifest') return { x_vox_version: 2, schema_version: 1, generated_from: 'e2e-mock', actions: [] };
        if (cmd === 'get_routing_summary_live') return { decision_preview: null };
        if (cmd === 'get_gui_preference') return null;
        if (cmd === 'set_gui_preference') return null;
        if (cmd === 'get_orchestrator_status_bin') return new Uint8Array([0x80]);
        if (cmd === 'get_user_config') {
          return [
            { key: 'daily_budget_usd', value: '5' },
            { key: 'per_session_budget_usd', value: '1' },
            { key: 'budget_warn_threshold_pct', value: '0.8' },
          ];
        }
        if (cmd === 'set_user_config') return null;
        return null;
      },
    };
  });
  await page.goto('/');
  await page.getByRole('button', { name: /use a local model/i }).click();
  await page.getByRole('button', { name: /^done$/i }).click();
  await expect(page.getByRole('heading', { name: /set your spending limits/i })).toBeVisible();
  await page.getByRole('button', { name: /save and continue/i }).click();
  await expect(page.getByRole('heading', { name: /you're set up/i })).toBeVisible();
});
```

- [ ] **Step 4: Mount the wizard at the app shell level**

Run:
```bash
grep -rln "&lt;BackendBanner\|&lt;VersionMismatchBanner" crates/vox-gui/ui/src/
```

In the file(s) found, add `&lt;OnboardingWizard pushToast={pushToast} gamifyEnabled={gamifyEnabled} /&gt;` as a sibling at the same level as `&lt;BackendBanner /&gt;`/`&lt;VersionMismatchBanner .../&gt;` (import it from `'./components/surfaces/Onboarding/OnboardingWizard'`, adjusting the relative path to match the found file's location). Use whatever `pushToast`/`gamifyEnabled` variables are already in scope at that render site (they're threaded through the app shell already, per `SurfaceProps` seen in `surfaceComponents.tsx`).

- [ ] **Step 5: Run test to verify it passes**

Run the project's Playwright command scoped to `onboarding.spec.ts`.
Expected: PASS (both tests)

- [ ] **Step 6: Run the full existing e2e suite (regression gate)**

Run the project's full Playwright command.
Expected: all pre-existing specs still PASS — mounting a conditionally-rendered overlay must not break any other surface's tests.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Onboarding/ crates/vox-gui/ui/e2e/onboarding.spec.ts
git commit -m "feat(vox-gui-ui): add OnboardingWizard with 3 entry paths"
```

Find where mount happened and also commit that file:

```bash
git status --short
git add -u
git commit -m "feat(vox-gui-ui): mount OnboardingWizard at app shell level" --allow-empty
```

(The second commit is a safety net in case Step 4's edit landed in a file not caught by the first `git add`; if `git status --short` after the first commit shows nothing left, skip the second commit entirely — don't create an empty commit needlessly.)

### Task 16: Settings → Onboarding replay entry

**Note**: this is the 3rd of 4 tasks touching `SettingsView.tsx` (after 13, before 7/19 depending on execution order) — don't run this task's own new assertion as the only check; **Sub-gate C** (declared in "Parallelization & Phase Gates" above) requires a full `settings.spec.ts` run after the *last* of Tasks 7/13/16/19 lands, not after each individually.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (add a new section or a button in an existing "About"/general section — search: `grep -n "function.*Section" crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` to find where a new small section fits the existing pattern)
- Test: extend `settings.spec.ts` or add a focused new spec

- [ ] **Step 1: Find the section list**

Run:
```bash
grep -n "^function.*Section" crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx
```

Identify the shortest/simplest existing section (for a pattern to copy) and where sections are assembled into the page (search for where these functions are called, e.g. `&lt;KeysSecretsSection .../&gt;` usage site).

- [ ] **Step 2: Write the failing test**

Add to `settings.spec.ts` (or a new file following its exact pattern) a test asserting a "Replay setup wizard" button exists and, when clicked, sets `localStorage['vox_onboarding_dismissed']` back to `'false'` (matching `useLocalStorage`'s serialization — confirm the exact stored string format from Task 14's Step 1 read).

- [ ] **Step 3: Run test to verify it fails**

Run the project's Playwright command scoped to the modified/new spec.
Expected: FAIL

- [ ] **Step 4: Add the replay button**

Add a small new section function following the pattern found in Step 1, e.g.:

```tsx
function OnboardingSection() {
  const gate = useOnboardingGate({ secretCount: 1, localModelCount: 0 }); // dummy values — this call only needs `.replay()`, not `.shouldShow`
  return (
    &lt;&gt;
      &lt;h2 className="font-display text-[18px] font-semibold tracking-tight text-text-primary"&gt;Onboarding&lt;/h2&gt;
      &lt;p className="mt-0.5 text-[11px] text-text-muted"&gt;Replay the first-run setup wizard.&lt;/p&gt;
      &lt;button
        type="button"
        onClick={gate.replay}
        className="mt-3 rounded-lg border border-border-subtle px-3 py-1.5 text-[11px] hover:bg-overlay-subtle"
      &gt;
        Replay setup wizard
      &lt;/button&gt;
    &lt;/&gt;
  );
}
```

(Import `useOnboardingGate` from `'../Onboarding/useOnboardingGate'`.) Note the dummy `{ secretCount: 1, localModelCount: 0 }` args deliberately produce `shouldShow: false` from this call site — this section only uses `.replay()`, which is a pure `setDismissed(false)` regardless of those inputs, so the values passed here are inert; do not read `.shouldShow` from this particular hook instance.

Render `&lt;OnboardingSection /&gt;` alongside the other sections found in Step 1.

- [ ] **Step 5: Run test to verify it passes**

Run the project's Playwright command scoped to the modified/new spec.
Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/e2e/
git commit -m "feat(vox-gui-ui): add Settings > Onboarding replay entry"
```

### Task 17: `ModelsView.tsx` — free-tier filter toggle

**Note**: Task 18 edits the same file immediately after this one and re-runs the combined spec (its Step 5) — that combined run is **Sub-gate D**. If these two tasks are ever executed by different subagents out of strict sequence, whoever lands second must still run the full `ModelsView` spec file, not just their own new assertion.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx` (the component body around lines 43-137, and the `ModelGrid` calls around lines 131-132)
- Test: new or extended Playwright spec for `ModelsView`

- [ ] **Step 1: Write the failing test**

Create/extend a Playwright spec for the Models surface (follow `settings.spec.ts`'s exact mock pattern, mocking `list_model_cards` to return two models, one with `is_free: true` and one with `is_free: false`):

```ts
test('free-tier filter hides non-free models when toggled on', async ({ page }) =&gt; {
  await page.addInitScript(() =&gt; {
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) =&gt; {
        if (cmd === 'get_initial_view') return 'models';
        if (cmd === 'list_model_cards') {
          return [
            { id: 'free/model-a', provider: 'openrouter', tier: 'Free', cost_per_1k: 0, max_tokens: 8000, is_free: true, latency_p50_ms: 400, quality_score: 0.8 },
            { id: 'paid/model-b', provider: 'openrouter', tier: 'Pro', cost_per_1k: 0.01, max_tokens: 8000, is_free: false, latency_p50_ms: 300, quality_score: 0.9 },
          ];
        }
        if (cmd === 'get_routing_summary_live') return { decision_preview: null };
        if (cmd === 'get_active_model') return null;
        if (cmd === 'inference_provider_status') return [];
        return null;
      },
    };
  });
  await page.goto('/');
  await expect(page.getByText('paid/model-b')).toBeVisible();
  await page.getByRole('checkbox', { name: /free only/i }).check();
  await expect(page.getByText('paid/model-b')).not.toBeVisible();
  await expect(page.getByText('free/model-a')).toBeVisible();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run the project's Playwright command scoped to this spec.
Expected: FAIL — no filter checkbox exists yet

- [ ] **Step 3: Add the filter state and checkbox**

In `ModelsView.tsx`, add near the component's other `useState` declarations (around line 43-60, exact location depends on reading the file's current `useState` block):

```tsx
const [freeOnly, setFreeOnly] = useState(false);
```

Add the checkbox in the component's JSX, near wherever the `hosted`/`local` split is rendered (find that exact render location first via `grep -n "hosted\|local" crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx`):

```tsx
&lt;label className="flex items-center gap-2 text-[11px] text-text-muted"&gt;
  &lt;input
    type="checkbox"
    role="checkbox"
    aria-label="Free only"
    checked={freeOnly}
    onChange={e =&gt; setFreeOnly(e.target.checked)}
  /&gt;
  Free only
&lt;/label&gt;
```

Filter the items passed into each `ModelGrid` call (found at lines 131-132) by adding `.filter(m =&gt; !freeOnly || m.is_free)` before they're passed as the `items` prop.

- [ ] **Step 4: Run test to verify it passes**

Run the project's Playwright command scoped to this spec.
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx crates/vox-gui/ui/e2e/
git commit -m "feat(vox-gui-ui): add free-tier filter toggle to ModelsView"
```

### Task 18: `ModelsView.tsx` — render `quality_score`

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx:158-162` (the existing 3-col stat row inside `ModelGrid`)
- Test: extend Task 17's spec or add a new assertion

- [ ] **Step 1: Write the failing test**

Extend the Playwright spec from Task 17 (or add a new test in the same file):

```ts
test('renders quality score when present', async ({ page }) =&gt; {
  await page.addInitScript(() =&gt; {
    (window as any).__TAURI_INTERNALS__ = {
      invoke: async (cmd: string) =&gt; {
        if (cmd === 'get_initial_view') return 'models';
        if (cmd === 'list_model_cards') {
          return [{ id: 'free/model-a', provider: 'openrouter', tier: 'Free', cost_per_1k: 0, max_tokens: 8000, is_free: true, latency_p50_ms: 400, quality_score: 0.8 }];
        }
        if (cmd === 'get_routing_summary_live') return { decision_preview: null };
        if (cmd === 'get_active_model') return null;
        if (cmd === 'inference_provider_status') return [];
        return null;
      },
    };
  });
  await page.goto('/');
  await expect(page.getByText(/qual.*0\.80/i)).toBeVisible();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run the project's Playwright command scoped to this test.
Expected: FAIL — `quality_score` isn't rendered anywhere yet

- [ ] **Step 3: Add the render**

In `ModelGrid`'s stat row (the existing 3-column grid at lines 158-162), change `grid-cols-3` to `grid-cols-4` and add a fourth cell:

```tsx
&lt;div&gt;
  &lt;span className="text-text-muted"&gt;qual&lt;/span&gt;{' '}
  {m.quality_score != null ? m.quality_score.toFixed(2) : '—'}
&lt;/div&gt;
```

placed after the existing `p50` cell.

- [ ] **Step 4: Run test to verify it passes**

Run the project's Playwright command scoped to this test.
Expected: PASS

- [ ] **Step 5: Run the full `ModelsView` spec file (regression gate)**

Run the full spec file (both this task's and Task 17's tests together).
Expected: all PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Models/ModelsView.tsx crates/vox-gui/ui/e2e/
git commit -m "feat(vox-gui-ui): render quality_score in ModelsView"
```

### Task 19: `LlmSettingsSection` — jump-link to Keys & Secrets

**Note**: this is the 4th (last) task touching `SettingsView.tsx` in the Settings chain (7, 13, 16, 19). Whichever of these lands last must run the **full** `settings.spec.ts` file (Sub-gate C, declared in "Parallelization & Phase Gates" above) — not just this task's own new assertion.

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx:887-948`
- Test: extend `settings.spec.ts`

- [ ] **Step 1: Write the failing test**

Add to `settings.spec.ts` a test that, given `openrouter_key_status` returns `{ configured: false }`, asserts a clickable link/button with accessible name matching `/keys.*secrets/i` is visible in the LLM settings section.

- [ ] **Step 2: Run test to verify it fails**

Run the project's Playwright command scoped to this test.
Expected: FAIL — today's banner is plain text, no interactive element

- [ ] **Step 3: Add the jump-link**

In `LlmSettingsSection`, change:

```tsx
{keyConfigured !== null &amp;&amp; (
  &lt;div className="mt-4 rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2 text-[11px] text-text-muted"&gt;
    {keyConfigured
      ? 'OpenRouter API key is configured.'
      : 'No OpenRouter key configured — add one under Keys &amp; Secrets.'}
  &lt;/div&gt;
)}
```

to:

```tsx
{keyConfigured !== null &amp;&amp; (
  &lt;div className="mt-4 rounded-lg border border-border-subtle bg-overlay-subtle px-3 py-2 text-[11px] text-text-muted"&gt;
    {keyConfigured ? (
      'OpenRouter API key is configured.'
    ) : (
      &lt;&gt;
        No OpenRouter key configured —{' '}
        &lt;button
          type="button"
          onClick={() =&gt; document.getElementById('keys-secrets-section')?.scrollIntoView({ behavior: 'smooth' })}
          className="text-brass underline hover:no-underline"
        &gt;
          add one under Keys &amp; Secrets
        &lt;/button&gt;
        .
      &lt;/&gt;
    )}
  &lt;/div&gt;
)}
```

This uses a plain `scrollIntoView` on an `id` anchor rather than the undocumented `'vox-settings-seed'` event mechanism (which wasn't fully characterized during research for this plan) — simpler, has no hidden contract to get wrong, and achieves the same "jump to Keys & Secrets" outcome. Add `id="keys-secrets-section"` to whichever wrapping element renders `&lt;KeysSecretsSection /&gt;` in the page (find that render site via `grep -n "&lt;KeysSecretsSection" crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`).

- [ ] **Step 4: Run test to verify it passes**

Run the project's Playwright command scoped to this test.
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/e2e/
git commit -m "feat(vox-gui-ui): add jump-link from LLM banner to Keys & Secrets"
```

---

## Final regression pass (run once, after all 21 tasks — 1 through 19, plus 11b and 12b)

- [ ] **Step 1: Full Rust workspace test + clippy**

```bash
cargo test -p vox-config -p vox-llm-config -p vox-orchestrator-mcp -p vox-gui -p vox-cli -p vox-oauth-pkce -p vox-secrets -p vox-llm-egress
cargo clippy -p vox-config -p vox-llm-config -p vox-orchestrator-mcp -p vox-gui -p vox-cli -p vox-oauth-pkce -- -D warnings
```
Expected: all green, no new clippy warnings.

- [ ] **Step 2: Full GUI test suite**

```bash
cd crates/vox-gui/ui &amp;&amp; pnpm exec vitest run &amp;&amp; pnpm exec playwright test
```
Expected: all green, including every pre-existing spec (not just the ones added in this plan).

- [ ] **Step 3: `cargo fmt` per-crate (never `cargo fmt --all` on this repo)**

```bash
cargo fmt -p vox-config -p vox-llm-config -p vox-orchestrator-mcp -p vox-gui -p vox-cli -p vox-oauth-pkce
```

- [ ] **Step 4: `vox doctor` manual smoke check**

```bash
vox doctor
```
Confirm the LLM routing check's `detail` string now includes `daily_budget_usd=`/`per_session_budget_usd=` (Task 6) and, if a rate-limit condition was reachable in this environment, the distinct message from Task 12.

- [ ] **Step 5: Manual OAuth flow smoke test on the current OS — including the empirical `state`-parameter question**

Run `vox secrets login --oauth --provider openrouter` in a real terminal (not CI) and confirm: a browser opens, the loopback server accepts the callback, the key gets stored (`vox secrets status` shows OpenRouter as present afterward), and `verify_openrouter_key` (Task 11b) actually confirms connectivity. While doing this pass, also settle the open empirical question from Task 9/the Phase 2 gate: log or briefly inspect the real callback URL's query string to see whether OpenRouter actually echoes a `state` parameter back, despite it not being documented. If it does, tighten `callback_handler`'s leniency back to required-and-matching before this ships to real users — the current lenient behavior is a deliberate, documented interim choice, not a permanent design decision. **Repeat the full smoke test on each of Windows/macOS/Linux before enabling Phase 2/3 behind their feature flag for real users** — per the design spec's rollout guidance, do not assume the loopback pattern behaves identically across all three without this manual pass.

- [ ] **Step 6: Update `docs/superpowers/plans/2026-08-01-free-tier-onboarding.md` status**

Once all tasks are checked off and Step 1-5 above are green, update this plan file's nothing — no execution-status header is required by this repo's plan convention (unlike some spec docs), but do add a short "Executed" note at the top of the file if this repo's existing plans typically do (check a recently-executed plan file, e.g. one referenced in project memory, for the convention before adding anything).
