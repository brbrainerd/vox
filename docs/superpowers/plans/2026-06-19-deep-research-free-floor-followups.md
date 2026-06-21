---
title: "Deep Research Free-Floor Follow-ups — SSOT Convergence + Live Dispatch Proof (Sonnet 4.6)"
description: "Closes the two follow-ups from AGH-0006: (A) converge vox-gamify's OPENROUTER_FREE_MODELS onto the new vox_config::OPENROUTER_FREE_FALLBACK_MODELS SSOT to kill list drift, and (B) add a live #[ignore] smoke test proving a concrete :free slug actually dispatches through the cascade. Sonnet-4.6-shaped, TDD, two parallel-safe tasks plus a sequential verify."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Closes a code-review finding loop: SSOT convergence of duplicated free-model lists + a live-dispatch acceptance test that proves EFFECT (model reachable) not just SHAPE — the B-9 lesson made executable."
---

# Deep Research Free-Floor Follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan. Tasks A and B are **PARALLEL-SAFE** (disjoint crates) — dispatch them as two concurrent subagents, then run Task C. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Eliminate the duplicate free-model list (gamify ↔ vox-config) by converging on one SSOT, and add a live smoke test that proves a concrete `:free` slug dispatches — closing the two follow-ups logged in `antigravity-handoff-ledger.md` AGH-0006.

**Architecture:** Two independent, parallel-safe changes. (A) `vox-gamify`'s `OPENROUTER_FREE_MODELS` becomes an alias of `vox_config::OPENROUTER_FREE_FALLBACK_MODELS` (the two lists are already byte-identical, so this is behavior-preserving) plus a drift-guard test. (B) a new `#[ignore]` integration test in `vox-actor-runtime` dispatches `vox_config::OPENROUTER_FREE_FALLBACK_MODELS[0]` through `chat_with_cascade` and asserts a non-empty completion (network + key gated, skipped cleanly when absent). A final sequential task verifies the whole touched surface is green.

**Tech Stack:** Rust; crates `vox-gamify`, `vox-actor-runtime`, `vox-config`. Tests: `cargo test -p <crate>`. No new dependencies.

**Execution target:** Sonnet 4.6 via subagent-driven-development. Sonnet may self-verify symbol paths, but still: every task ends GREEN + committed; use `cargo fmt -p <crate>` only (never `--all` — Windows arg-limit); do not edit files outside the listed set; if a verification fails twice, stop and report rather than loop.

**Spec / source:** code-review of AGH-0006 (`docs/superpowers/antigravity-handoff-ledger.md`) and the remediation commit `309c9eea98` which introduced `vox_config::OPENROUTER_FREE_FALLBACK_MODELS`. Research basis: `docs/src/architecture/deep-research-system-best-practices-research-2026-06-18.md` §5.

---

## Pre-flight (run once before dispatching tasks)

- [ ] **P0. Green baseline.**

Run: `cargo test -p vox-config -p vox-actor-runtime -p vox-gamify`
Expected: PASS. If red at baseline for unrelated reasons, STOP and report.

- [ ] **P1. Confirm the SSOT constant exists and is re-exported.**

Run: `rg -n 'OPENROUTER_FREE_FALLBACK_MODELS' crates/vox-config/src/bootstrap_inference.rs crates/vox-config/src/lib.rs`
Expected: a `pub const OPENROUTER_FREE_FALLBACK_MODELS: &[&str] = &[ ... ]` in `bootstrap_inference.rs` (5 `:free` slugs) and a re-export in `lib.rs` so `vox_config::OPENROUTER_FREE_FALLBACK_MODELS` resolves.

- [ ] **P2. Confirm gamify's list is byte-identical to the SSOT (so aliasing is safe).**

Run: `rg -n 'OPENROUTER_FREE_MODELS' crates/vox-gamify/src/ai/constants.rs` then read lines 16-23.
Expected: the same 5 slugs in the same order as P1:
`google/gemma-3-27b-it:free`, `meta-llama/llama-3.3-70b-instruct:free`, `qwen/qwen3-235b-a22b:free`, `mistralai/mistral-7b-instruct:free`, `microsoft/phi-3-mini-128k-instruct:free`.
If they differ, STOP — aliasing would change gamify behavior; report the diff instead.

- [ ] **P3. Confirm gamify depends on vox-config.**

Run: `rg -n '^vox-config' crates/vox-gamify/Cargo.toml`
Expected: `vox-config = { workspace = true }`.

- [ ] **P4. Confirm the public cascade API for the smoke test.**

Run: `rg -n 'pub use activity|pub mod llm' crates/vox-actor-runtime/src/lib.rs` and `rg -n 'pub fn with_timeout_secs|pub fn new' crates/vox-actor-runtime/src/activity.rs` and `rg -n 'pub async fn chat_with_cascade' crates/vox-actor-runtime/src/llm/cascade.rs`
Expected: `ActivityOptions` is public (`pub use activity::{... ActivityOptions ...}`), `ActivityOptions::new()` and `with_timeout_secs(u64)` are pub, and `chat_with_cascade(&ActivityOptions, Vec<LlmChatMessage>, Vec<LlmConfig>, Option<ResearchStage>) -> Result<LlmResponse, String>` exists. `LlmConfig`/`LlmChatMessage`/`LlmResponse` are reachable as `vox_actor_runtime::llm::{...}`.

---

## Task A: Converge gamify free-model list onto the vox-config SSOT [PARALLEL-SAFE — subagent 1]

**Files:**
- Modify: `crates/vox-gamify/src/ai/constants.rs` (the `OPENROUTER_FREE_MODELS` const + a drift-guard test)

- [ ] **Step 1: Write the failing drift-guard test**

Add to `crates/vox-gamify/src/ai/constants.rs` (create a `#[cfg(test)] mod tests` at the end if none exists):

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn free_models_are_the_vox_config_ssot() {
        // The gamify list MUST be the single vox-config SSOT, not a private copy,
        // so the research free-floor and gamify free tier can never drift.
        assert_eq!(
            super::OPENROUTER_FREE_MODELS,
            vox_config::OPENROUTER_FREE_FALLBACK_MODELS
        );
    }
}
```

- [ ] **Step 2: Run it against the current (duplicated) list**

Run: `cargo test -p vox-gamify free_models_are_the_vox_config_ssot`
Expected: PASS today (the lists are byte-identical) — this is a guard, not a red-first test. If it FAILS, the lists already diverged: STOP and report (do not silently re-sync; the divergence may be intentional).

- [ ] **Step 3: Replace the private list with the SSOT alias**

In `crates/vox-gamify/src/ai/constants.rs`, replace:

```rust
pub(crate) const OPENROUTER_FREE_MODELS: &[&str] = &[
    "google/gemma-3-27b-it:free",
    "meta-llama/llama-3.3-70b-instruct:free",
    "qwen/qwen3-235b-a22b:free",
    "mistralai/mistral-7b-instruct:free",
    "microsoft/phi-3-mini-128k-instruct:free",
];
```

with:

```rust
/// Free-tier OpenRouter models tried in order (most capable first), all `:free`.
///
/// SSOT: aliased to `vox_config::OPENROUTER_FREE_FALLBACK_MODELS` so the gamify
/// free tier and the research free-floor cannot drift apart. Edit the list in
/// `crates/vox-config/src/bootstrap_inference.rs`, not here.
pub(crate) const OPENROUTER_FREE_MODELS: &[&str] = vox_config::OPENROUTER_FREE_FALLBACK_MODELS;
```

- [ ] **Step 4: Verify the alias compiles, the guard still passes, and the 3 call-sites are untouched**

Run: `cargo test -p vox-gamify free_models_are_the_vox_config_ssot`
Expected: PASS.
Run: `cargo build -p vox-gamify`
Expected: clean build (the existing uses in `ai/client/transport.rs`, `ai/provider.rs`, `ai/client/ctor.rs` consume the same `&[&str]` type, so no call-site edits are needed).

- [ ] **Step 5: Format + commit**

```bash
cargo fmt -p vox-gamify
git add crates/vox-gamify/src/ai/constants.rs
git commit -m "refactor(vox-gamify): alias OPENROUTER_FREE_MODELS to vox-config SSOT (kill list drift)"
```

---

## Task B: Live dispatch smoke test for the free floor [PARALLEL-SAFE — subagent 2]

**Files:**
- Create: `crates/vox-actor-runtime/tests/openrouter_free_floor_smoke.rs`

- [ ] **Step 1: Create the `#[ignore]` integration test**

Create `crates/vox-actor-runtime/tests/openrouter_free_floor_smoke.rs`:

```rust
//! Live smoke test: a concrete OpenRouter `:free` slug actually dispatches and
//! returns a completion. This proves the research free-tier FLOOR is real — i.e.
//! the slugs in `vox_config::OPENROUTER_FREE_FALLBACK_MODELS` are dispatchable,
//! not a non-dispatchable virtual id (the AGH-0006 defect). Verifies EFFECT, not
//! shape.
//!
//! Network + `OPENROUTER_API_KEY` required → `#[ignore]`. Run on demand:
//!   cargo test -p vox-actor-runtime --test openrouter_free_floor_smoke -- --ignored --nocapture

use vox_actor_runtime::activity::ActivityOptions;
use vox_actor_runtime::llm::cascade::chat_with_cascade;
use vox_actor_runtime::llm::{LlmChatMessage, LlmConfig};

#[tokio::test]
#[ignore = "requires OPENROUTER_API_KEY and network; run with --ignored"]
async fn free_floor_slug_dispatches_and_returns_content() {
    if vox_config::inference::openrouter_api_key().is_none() {
        eprintln!("SKIP: OPENROUTER_API_KEY not set — cannot run live dispatch smoke test");
        return;
    }

    let slug = vox_config::OPENROUTER_FREE_FALLBACK_MODELS[0];
    assert!(slug.ends_with(":free"), "floor slug must be a free model: {slug}");

    let candidate = LlmConfig::openrouter(slug);
    let messages = vec![LlmChatMessage {
        role: "user".to_string(),
        content: "Reply with exactly the single word: pong".to_string(),
    }];
    let opts = ActivityOptions::new().with_timeout_secs(60);

    let response = chat_with_cascade(&opts, messages, vec![candidate], None)
        .await
        .unwrap_or_else(|e| panic!("free slug `{slug}` failed to dispatch: {e}"));

    assert!(
        !response.content.trim().is_empty(),
        "free slug `{slug}` dispatched but returned empty content"
    );
    eprintln!("OK: `{slug}` dispatched -> {:?}", response.content);
}
```

- [ ] **Step 2: Verify it COMPILES and is collected-but-ignored in the hermetic run**

Run: `cargo test -p vox-actor-runtime --test openrouter_free_floor_smoke`
Expected: builds; output shows `1 ignored` (the test is skipped without `--ignored`, so CI stays hermetic and key-free).

- [ ] **Step 3: (Optional, if a key is available) run the live test once**

Run: `cargo test -p vox-actor-runtime --test openrouter_free_floor_smoke -- --ignored --nocapture`
Expected: `OK: \`google/gemma-3-27b-it:free\` dispatched -> "...pong..."` and the test passes. If it SKIPs (no key), that is acceptable — the compile + ignore collection in Step 2 is the committed gate. If it FAILS with a model error, the floor slug is dead: STOP and report (the SSOT list in `bootstrap_inference.rs` needs a live-model refresh).

- [ ] **Step 4: Commit**

```bash
cargo fmt -p vox-actor-runtime
git add crates/vox-actor-runtime/tests/openrouter_free_floor_smoke.rs
git commit -m "test(vox-actor-runtime): live #[ignore] smoke proving a :free floor slug dispatches"
```

---

## Task C: Final verification [SEQUENTIAL — after A and B both land]

**Files:** none (verification only).

- [ ] **Step 1: Build + test the touched crates**

Run: `cargo test -p vox-config -p vox-actor-runtime -p vox-gamify`
Expected: PASS (the live smoke test shows as `ignored`).

- [ ] **Step 2: Clippy on the touched crates**

Run: `cargo clippy -p vox-config -p vox-actor-runtime -p vox-gamify --no-deps -- -D warnings`
Expected: no warnings. (Never run workspace-wide clippy — `vox-gui`'s build script breaks it.)

- [ ] **Step 3: Architecture parity gate (run at FULL strictness — do not use --warn-only)**

Run: `cargo run -p vox-arch-check`
Expected: exits 0. If red, STOP and report.

- [ ] **Step 4: Confirm only the two intended files changed**

Run: `git status` and `git diff --stat HEAD~2..HEAD`
Expected: exactly `crates/vox-gamify/src/ai/constants.rs` and `crates/vox-actor-runtime/tests/openrouter_free_floor_smoke.rs` (plus this verification produced no edits).

---

## Self-Review

- **Spec coverage:** Follow-up (A) SSOT convergence → Task A. Follow-up (B) live dispatch proof → Task B. Both code-review follow-ups from AGH-0006 are covered; Task C is the green gate. ✅
- **Placeholder scan:** every code step shows complete code; commands have expected output; the only "optional" step (B-3) is explicitly gated on key availability with a defined fallback. ✅
- **Type consistency:** `OPENROUTER_FREE_MODELS: &[&str]` aliased to `vox_config::OPENROUTER_FREE_FALLBACK_MODELS: &[&str]` (same type); smoke test uses confirmed public paths `vox_actor_runtime::llm::{LlmChatMessage, LlmConfig}`, `::llm::cascade::chat_with_cascade`, `::activity::ActivityOptions`; `response.content` matches `LlmResponse`. ✅
- **No invented APIs:** every symbol confirmed in Pre-flight P1–P4. ✅

---

## Notes for the parallel dispatcher

- **A and B touch disjoint crates** (`vox-gamify` vs `vox-actor-runtime` tests) and share only the read-only `vox-config` SSOT — safe to run as two concurrent subagents. Neither writes a file the other writes.
- **Do not** roll the optional live network call (B-3) into a required gate; the committed acceptance is "compiles + collected as ignored" so CI stays hermetic.
- After Task C, update `docs/superpowers/antigravity-handoff-ledger.md` AGH-0006 follow-up note: mark both follow-ups closed and reference the two new commits.
</content>
