# Free-Tier Onboarding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A brand-new Vox user with zero API keys and no Ollama reaches a working chat state without a terminal error, with a real spend safety net already in place, using an in-app OAuth flow instead of copy-pasting a key from a browser.

**Architecture:** Three independently-landable phases. Phase 1 wires Vox's existing-but-inert `daily_budget_usd`/`per_session_budget_usd` config fields into dispatch with a warn-then-block guard. Phase 2 adds a new low-dependency `vox-oauth-pkce` crate implementing RFC 8252's loopback-server PKCE pattern against OpenRouter, consumed identically by a new CLI command and a new Tauri command, both persisting the resulting key through the *existing* `vox_secrets::set_registry_token` path — no new secret storage plumbing. Phase 3 builds the onboarding wizard as a React overlay component (not a registered sidebar surface, to avoid the generated `surfaceRegistry.generated.ts` machinery) with three entry paths, reusing `KeysSecretsSection` (newly exported) for the "I have a key" path and Phase 2's Tauri command for "get a free key," plus small additions to `ModelsView.tsx`.

**Tech Stack:** Rust (Tauri backend, `vox-cli`, `vox-orchestrator-mcp`), TypeScript/React (Tauri frontend), `axum`+`reqwest` (already workspace deps) for the loopback listener and token exchange, `open` (new dependency) for launching the system browser, `rand`+`sha2`+`base64` (already workspace deps) for PKCE.

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

- [ ] **Step 1: Write the failing test**

First find the existing parity test — run:

```bash
grep -rn "fn.*parity\|fn.*matches_voxconfig\|LLM_CONFIG_KEYS" crates/vox-llm-config/src/keys.rs | head -20
```

If a generic parity test already exists (it iterates `LLM_CONFIG_KEYS` and checks each against `VoxConfig`), it will already fail once you add the new field to `VoxConfig` (Task 1) without adding a matching registry entry — that's your failing test, no new test needed. Confirm this by running:

Run: `cargo test -p vox-llm-config -- --nocapture`
Expected: FAIL (parity test reports `budget_warn_threshold_pct` present on `VoxConfig` but missing from `LLM_CONFIG_KEYS`, or equivalent)

If no such generic parity test exists, add one modeled on the existing entries — write it before proceeding, asserting `LLM_CONFIG_KEYS.iter().any(|k| k.env == "budget_warn_threshold_pct")`.

- [ ] **Step 2: Add the registry entry**

In `crates/vox-llm-config/src/keys.rs`, add after the `per_session_budget_usd` line:

```rust
    vc_key!("budget_warn_threshold_pct", Float, General, "Budget warn threshold", "Warn when spend crosses this fraction of a budget cap (0.0-1.0)"),
```

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-llm-config -- --nocapture`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vox-llm-config/src/keys.rs
git commit -m "feat(vox-llm-config): register budget_warn_threshold_pct in SSOT"
```

### Task 3: `budget_guard` module — the core check logic

**Files:**
- Create: `crates/vox-orchestrator-mcp/src/llm_bridge/budget_guard.rs`
- Modify: `crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs` (add `pub mod budget_guard;`)
- Test: `crates/vox-orchestrator-mcp/src/llm_bridge/budget_guard.rs` (inline `#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Create `crates/vox-orchestrator-mcp/src/llm_bridge/budget_guard.rs`:

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

Add `pub mod budget_guard;` to `crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs` (find the existing `pub mod` list in that file and add this line alphabetically among the others).

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-orchestrator-mcp budget_guard -- --nocapture`
Expected: FAIL — either a compile error (module not yet wired, if `mod.rs` edit is done in a separate sub-step) or, once wired, all 5 tests should actually PASS immediately since this is pure logic with no external dependency. If Step 1 was done correctly this test module is self-contained and should go green as soon as it compiles — there is no separate "make it pass" step needed beyond getting it to compile. Treat "compiles and all 5 pass" as the success criterion for this task.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-orchestrator-mcp budget_guard -- --nocapture`
Expected: PASS (5 tests: `under_threshold_returns_none`, `at_warn_threshold_returns_warning`, `at_daily_cap_returns_exceeded`, `at_session_cap_returns_exceeded_even_if_daily_ok`, `warn_threshold_of_one_disables_warning`)

- [ ] **Step 4: Commit**

```bash
git add crates/vox-orchestrator-mcp/src/llm_bridge/budget_guard.rs crates/vox-orchestrator-mcp/src/llm_bridge/mod.rs
git commit -m "feat(vox-orchestrator-mcp): add budget_guard warn-then-block logic"
```

### Task 4: Wire `budget_guard` into GUI dispatch (Tauri command boundary)

**Files:**
- Modify: `crates/vox-gui/src/commands/user_config.rs:304-334` (the existing `get_llm_spend` command already fetches `LlmSpendSummary` + `VoxConfig` caps — reuse this exact data-fetch pattern)
- Modify: `crates/vox-gui/src/commands/models.rs` or wherever the GUI's chat-dispatch entry point lives (search: `grep -rn "resolve_mcp_chat_model\|fn chat_send\|fn dispatch" crates/vox-gui/src/commands/chat.rs` — this file was listed in the commands directory but not read yet; find the actual dispatch call site first)
- Test: same file as the modified dispatch command, inline `#[cfg(test)]` or existing test module

- [ ] **Step 1: Locate the exact dispatch call site**

Run:
```bash
grep -rn "resolve_mcp_chat_model\|resolve_chat_llm_model" crates/vox-gui/src/commands/
```

This finds every place the GUI backend resolves a model before dispatch — that's where `budget_guard::check` must run first. There may be more than one call site (e.g. `chat.rs` for interactive chat, `harness.rs` for agent runs) — apply the same wiring to each.

- [ ] **Step 2: Write the failing test**

For the primary call site found in Step 1 (most likely `crates/vox-gui/src/commands/chat.rs`), add a test asserting the command returns an error when spend already exceeds the cap. Since this requires a `VoxDb` connection, follow the existing async-test pattern already used by `get_llm_spend`'s neighbors in `user_config.rs` (search that file for `#[tokio::test]` to find the established fixture pattern) rather than inventing a new one — read that pattern first, then write an analogous test here:

```bash
grep -n "#\[tokio::test\]" -A 15 crates/vox-gui/src/commands/user_config.rs
```

Use whatever `VoxDb::connect`/in-memory fixture pattern that search reveals to write:

```rust
#[tokio::test]
async fn dispatch_refuses_when_daily_budget_exceeded() {
    // Arrange: a VoxConfig with daily_budget_usd = 0.01 and a spend summary
    // showing $0.01+ already spent today (use the same DB fixture pattern
    // found in user_config.rs's existing async tests).
    // Act: call the chat-dispatch command.
    // Assert: it returns Err(_) containing "budget" (case-insensitive) rather
    // than proceeding to resolve_mcp_chat_model.
}
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-gui --bins dispatch_refuses_when_daily_budget_exceeded -- --nocapture`
Expected: FAIL (command currently dispatches unconditionally)

- [ ] **Step 4: Wire the guard**

In the dispatch command found in Step 1, immediately before the existing `resolve_mcp_chat_model`/`resolve_mcp_chat_model_sync` call, insert:

```rust
    let spend_cfg = vox_config::VoxConfig::load();
    let spend = vox_db::VoxDb::connect_canonical()
        .await
        .ok()
        .then(|| async {})
        .is_some(); // placeholder removed below — see full block
```

Replace that scaffold with the real block (matches `get_llm_spend`'s exact pattern from `user_config.rs:304-334`):

```rust
    let cfg = vox_config::VoxConfig::load();
    let spend = match vox_db::VoxDb::connect_canonical().await {
        Ok(db) =&gt; db
            .llm_spend_summary(session_id.as_deref())
            .await
            .unwrap_or_default(),
        Err(_) =&gt; Default::default(),
    };
    if let Err(e) = vox_orchestrator_mcp::llm_bridge::budget_guard::check(
        &amp;spend,
        cfg.daily_budget_usd,
        cfg.per_session_budget_usd,
        cfg.budget_warn_threshold_pct,
    ) {
        return Err(e.to_string());
    }
```

(Adjust `session_id` to whatever variable name the surrounding function already uses for its session identifier — check the function signature found in Step 1.)

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test -p vox-gui --bins dispatch_refuses_when_daily_budget_exceeded -- --nocapture`
Expected: PASS

- [ ] **Step 6: Verify existing dispatch tests still pass (regression gate)**

Run: `cargo test -p vox-gui --bins`
Expected: all pre-existing tests in this crate still PASS — this task must not break normal (under-budget) dispatch.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/src/commands/
git commit -m "feat(vox-gui): wire budget_guard into chat dispatch"
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

- [ ] **Step 6: Commit**

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
- Modify: root `Cargo.toml` (add `"crates/vox-oauth-pkce"` to the workspace `members` list, and register the new crate as a workspace dependency if the workspace uses a central `[workspace.dependencies]` table — check the existing pattern for a recently-added small crate like `vox-llm-egress` first)
- Modify: `docs/src/architecture/where-things-live.md` (add a row per `AGENTS.md`'s requirement: "consult this before adding code... add the row in the same PR")
- Test: `crates/vox-oauth-pkce/src/pkce.rs` inline

- [ ] **Step 1: Check the workspace-registration pattern for a recent small crate**

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

Add `"crates/vox-oauth-pkce",` to the `members` list in the root `Cargo.toml` (alphabetically, near `vox-oauth-*`/`vox-openai` if present — otherwise near other single-purpose small crates), and add `vox-oauth-pkce = { path = "crates/vox-oauth-pkce" }` to `[workspace.dependencies]` if that table exists (mirror the `vox-llm-egress` entry format found in Step 1).

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
    #[error("failed to open system browser: {0}")]
    BrowserOpen(String),
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
}

async fn callback_handler(
    State(state): State&lt;Arc&lt;CallbackState&gt;&gt;,
    Query(q): Query&lt;CallbackQuery&gt;,
) -&gt; Html&lt;&amp;'static str&gt; {
    let result = match (q.code, q.state) {
        (Some(code), Some(got_state)) if got_state == state.expected_state =&gt; Ok(code),
        (Some(_), Some(_)) =&gt; Err(OAuthError::StateMismatch),
        _ =&gt; Err(OAuthError::TokenExchange("missing code/state in callback".into())),
    };
    if let Some(tx) = state.tx.lock().expect("callback state mutex poisoned").take() {
        let _ = tx.send(result);
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
    let callback_state = Arc::new(CallbackState {
        expected_state: state_value.clone(),
        tx: std::sync::Mutex::new(Some(tx)),
    });

    let app = Router::new()
        .route("/callback", get(callback_handler))
        .with_state(callback_state);

    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let callback_url = format!("http://127.0.0.1:{port}/callback");
    let auth_url = format!(
        "{OPENROUTER_AUTH_URL}?callback_url={}&amp;code_challenge={}&amp;code_challenge_method=S256&amp;state={}",
        urlencoding_encode(&amp;callback_url),
        challenge,
        state_value,
    );

    open::that(&amp;auth_url).map_err(|e| OAuthError::BrowserOpen(e.to_string()))?;

    let code = tokio::time::timeout(CALLBACK_TIMEOUT, rx)
        .await
        .map_err(|_| OAuthError::TimedOut(CALLBACK_TIMEOUT))?
        .map_err(|_| OAuthError::TokenExchange("callback channel closed unexpectedly".into()))??;

    server.abort();

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

    #[tokio::test]
    async fn callback_handler_rejects_state_mismatch() {
        let (tx, rx) = oneshot::channel();
        let state = Arc::new(CallbackState {
            expected_state: "expected-123".to_string(),
            tx: std::sync::Mutex::new(Some(tx)),
        });
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
        let state = Arc::new(CallbackState {
            expected_state: "expected-123".to_string(),
            tx: std::sync::Mutex::new(Some(tx)),
        });
        let query = Query(CallbackQuery {
            code: Some("real-code".to_string()),
            state: Some("expected-123".to_string()),
        });
        let _ = callback_handler(State(state), query).await;
        let result = rx.await.expect("tx sent");
        assert_eq!(result.unwrap(), "real-code");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-oauth-pkce -- --nocapture`
Expected: FAIL initially on any typo/signature mismatch against the actual `axum`/`reqwest` versions pinned in this workspace — if `axum::serve` or the `Query`/`State` extractor APIs differ from what's shown here (workspace pins `axum = "0.8"` per the earlier audit — this code targets that API shape), fix signature mismatches against the compiler's actual errors rather than the snippet above; the snippet is written against axum 0.8's `axum::serve(listener, app)` + extractor pattern, which is current as of this workspace's pin.

- [ ] **Step 3: Run test to verify it passes**

Run: `cargo test -p vox-oauth-pkce -- --nocapture`
Expected: PASS (6 tests total across `pkce.rs` and `openrouter.rs`: 4 + `urlencoding_escapes_colon_and_slash` + `callback_handler_rejects_state_mismatch` + `callback_handler_accepts_matching_state` = actually 7; count what actually runs and confirm all green, don't hardcode an expected count blindly)

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

Add to `crates/vox-cli/src/commands/secrets.rs` (find or create the test module):

```rust
#[cfg(test)]
mod oauth_login_cli_tests {
    use super::*;

    #[test]
    fn oauth_flag_parses_on_login_subcommand() {
        // Parse `vox secrets login --oauth --provider openrouter` via the
        // crate's existing clap-parsing test helper (search this file/module
        // for how other subcommand parse tests are structured — likely
        // `SecretsCmd::try_parse_from([...])` or similar via `clap::Parser`).
        let cmd = SecretsCmd::try_parse_from([
            "secrets", "login", "--oauth", "--provider", "openrouter",
        ])
        .expect("parses");
        match cmd {
            SecretsCmd::Login { oauth, provider, .. } =&gt; {
                assert!(oauth);
                assert_eq!(provider.as_deref(), Some("openrouter"));
            }
            _ =&gt; panic!("expected Login variant"),
        }
    }
}
```

(This test's exact parsing helper may need adjusting once you check how `SecretsCmd` derives `clap::Subcommand` — it already does, per the `#[derive(Subcommand, Debug)]` on the enum, so `SecretsCmd::try_parse_from` should work directly if `SecretsCmd` is `#[derive(Parser)]`-compatible at the top level, or you may need to wrap it in whatever top-level CLI struct the crate's existing arg-parsing tests use — check `crates/vox-cli/src/main.rs` or `crates/vox-cli/src/cli.rs` for the existing pattern before finalizing this test's exact shape.)

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

- [ ] **Step 2: Write the failing test**

Create `crates/vox-gui/src/commands/oauth.rs`:

```rust
//! Tauri commands for in-app OAuth key provisioning (free-tier onboarding).

use serde::Serialize;
use tauri::command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthLoginResultDto {
    pub success: bool,
    pub error: Option&lt;String&gt;,
}

/// Run the OpenRouter loopback OAuth flow and persist the resulting key via
/// the same storage path `set_secret` already uses for OPENROUTER_API_KEY
/// (`vox_secrets::set_registry_token("openrouter", ...)`), so the GUI's
/// `list_secret_status`/`vox doctor` see it identically to a manually-entered key.
#[command]
pub async fn oauth_login_openrouter() -&gt; OAuthLoginResultDto {
    match vox_oauth_pkce::openrouter::run_openrouter_flow().await {
        Ok(key) =&gt; match vox_secrets::set_registry_token("openrouter", &amp;key, None) {
            Ok(()) =&gt; OAuthLoginResultDto {
                success: true,
                error: None,
            },
            Err(e) =&gt; OAuthLoginResultDto {
                success: false,
                error: Some(format!("failed to store key: {e}")),
            },
        },
        Err(e) =&gt; OAuthLoginResultDto {
            success: false,
            error: Some(e.to_string()),
        },
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
        };
        let json = serde_json::to_string(&amp;dto).expect("serializes");
        assert!(json.contains("\"success\":false"));
        assert!(json.contains("\"error\":\"timed out\""));
    }
}
```

Add `pub mod oauth;` to `crates/vox-gui/src/commands/mod.rs` (alphabetically among the existing `pub mod` lines).

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test -p vox-gui result_dto_serializes_camel_case -- --nocapture`
Expected: FAIL (compile error — `vox-oauth-pkce`/`vox_secrets::set_registry_token` not yet resolvable if the dependency add in Step 1 wasn't done first, or module not registered). Confirm Step 1 and the `mod.rs` edit are both done, then re-check — this specific unit test itself doesn't touch either dependency's real behavior, only serialization, so it should compile and pass as soon as the file exists and compiles.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-gui result_dto_serializes_camel_case -- --nocapture`
Expected: PASS

- [ ] **Step 5: Register in `generate_handler!`**

In `crates/vox-gui/src/main.rs`, add `commands::oauth::oauth_login_openrouter,` immediately after the existing `commands::secrets::*` block (around line 227 per the earlier audit).

- [ ] **Step 6: Full build check**

Run: `cargo check -p vox-gui`
Expected: compiles clean (this validates the `generate_handler!` registration syntax, which isn't otherwise unit-testable)

- [ ] **Step 7: Commit**

```bash
git add crates/vox-gui/src/commands/oauth.rs crates/vox-gui/src/commands/mod.rs crates/vox-gui/src/main.rs crates/vox-gui/Cargo.toml
git commit -m "feat(vox-gui): add oauth_login_openrouter Tauri command"
```

### Task 12: `vox doctor` — distinguish `NoCredential` vs `RateLimited` (investigative task)

**Files:**
- Modify: `crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs`
- Investigate first: `crates/vox-llm-egress/src/throttle.rs` (the 429/retry-after handling referenced by `LlmSettingsSection`'s "429 retry attempts" config) and wherever its errors propagate to a dispatch caller

- [ ] **Step 1: Find how a 429 currently surfaces to a caller**

Run:
```bash
grep -rn "429\|RateLimited\|retry_after" crates/vox-llm-egress/src/throttle.rs crates/vox-llm-egress/src/lib.rs
```

Read the matched error type/variant definitions. This determines whether "rate limited" is already a distinguishable error today (likely yes, given the retry-after-header handling noted in project history) or whether it collapses into a generic HTTP-error string by the time it reaches `resolve.rs`'s callers.

- [ ] **Step 2: Write a test capturing current behavior**

Before changing anything, write a test (in whichever egress test file already covers throttle behavior — search `crates/vox-llm-egress/src` for existing `#[cfg(test)]` throttle tests) that asserts what error type/string a 429 response currently produces, using this crate's existing `wiremock` test pattern (confirmed present per Task 9). This test should PASS immediately (it documents current behavior, it's not a new feature yet) — its purpose is to lock in the exact error shape before Task 12's remaining steps depend on it.

- [ ] **Step 3: Run the behavior-capture test**

Run: `cargo test -p vox-llm-egress -- --nocapture` (scoped to the new test)
Expected: PASS (this confirms your understanding of the current error shape is correct before proceeding)

- [ ] **Step 4: Add a `RateLimited`-detection helper at the doctor/CLI boundary**

Based on what Step 1-3 revealed about the actual error shape, add a small helper in `llm_routing.rs` (or wherever is appropriate given the real error type found) that checks a resolved error for the rate-limit signal and returns a distinct `Check` with name `"LLM routing (rate limit)"` and a message like `"OpenRouter free tier limit reached — resets at &lt;time if available&gt;, add your own key or wait"` instead of the generic FAIL. The exact implementation depends entirely on Step 1's findings — do not write this code blind; let the discovered error type drive the match arm.

- [ ] **Step 5: Write a test for the new doctor behavior**

Once Step 4's implementation is written against the real type, add a test analogous to Task 6's `reports_budget_caps_in_detail_string`, asserting the doctor check produces the distinct rate-limit message when the underlying resolver reports that condition (construct this via whatever mocking seam Step 1-2 revealed, e.g. a `wiremock` 429 response feeding through the real resolution path if that's testable at this layer, or a narrower unit test on just the new helper function if full end-to-end mocking isn't practical here).

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo test -p vox-cli` (scoped to the new test name)
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vox-llm-egress/ crates/vox-cli/src/commands/diagnostics/doctor/checks_standard/llm_routing.rs
git commit -m "feat: distinguish rate-limited from no-credential in LLM routing diagnostics"
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

type WizardScreen = 'entry' | 'oauth-in-progress' | 'has-key' | 'local-model' | 'budget' | 'confirmation';

export function OnboardingWizard({ pushToast, gamifyEnabled }: { pushToast: (t: any) =&gt; void; gamifyEnabled?: boolean }) {
  const [secretCount, setSecretCount] = useState&lt;number | null&gt;(null);
  const [localModelCount, setLocalModelCount] = useState&lt;number | null&gt;(null);
  const [screen, setScreen] = useState&lt;WizardScreen&gt;('entry');
  const [oauthError, setOauthError] = useState&lt;string | null&gt;(null);

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
    try {
      const result = await invoke&lt;{ success: boolean; error: string | null }&gt;('oauth_login_openrouter');
      if (result.success) {
        setScreen('budget');
      } else {
        setOauthError(result.error ?? 'Unknown error');
        setScreen('entry');
      }
    } catch (err) {
      setOauthError(String(err));
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

## Final regression pass (run once, after all 19 tasks)

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

- [ ] **Step 5: Manual OAuth flow smoke test on the current OS**

Run `vox secrets login --oauth --provider openrouter` in a real terminal (not CI) and confirm: a browser opens, the loopback server accepts the callback, the key gets stored (`vox secrets status` shows OpenRouter as present afterward). **Repeat this on each of Windows/macOS/Linux before enabling Phase 2/3 behind their feature flag for real users** — per the design spec's rollout guidance, do not assume the loopback pattern behaves identically across all three without this manual pass.

- [ ] **Step 6: Update `docs/superpowers/plans/2026-08-01-free-tier-onboarding.md` status**

Once all tasks are checked off and Step 1-5 above are green, update this plan file's nothing — no execution-status header is required by this repo's plan convention (unlike some spec docs), but do add a short "Executed" note at the top of the file if this repo's existing plans typically do (check a recently-executed plan file, e.g. one referenced in project memory, for the convention before adding anything).
