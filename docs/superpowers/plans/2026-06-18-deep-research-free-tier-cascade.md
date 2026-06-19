---
title: "Research Cascade Free-Tier Floor (G4) — Antigravity/Flash Implementation Plan"
description: "Gemini-3.5-Flash-executable, TDD, bite-sized plan to make Vox's model-agnostic research LLM cascade always carry a zero-cost OpenRouter free-tier fallback floor (openrouter/free) beneath the configured/premium model, with an opt-in prefer-free reordering. Extends the existing candidate cascade and CostPreference model rather than replacing the paid path. Wave 1 of the deep-research best-practices program."
category: "Architecture SSOTs"
status: "current"
training_eligible: true
training_rationale: "Model-agnostic research cascade with a free-tier fallback floor: extends existing CostPreference/FreeTierRouter/virtual-models systems via the single cascade chokepoint; Flash-shaped atomic-green TDD tasks with verify-before-use pre-flight."
---

# Research Cascade Free-Tier Floor (G4) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the research LLM cascade *always* carry a zero-cost `openrouter/free` fallback **beneath** the configured model, so research degrades to free instead of failing — and let an opt-in flag reorder to free-first for cost-sensitive runs. The configured (possibly premium) model stays the default; free tier is a floor, not a fixation.

**Architecture:** One chokepoint. Every research stage (`planner.rs`, `claims.rs`, `verifier.rs`, `stages.rs`) builds `RouteResolutionInput::default()` and calls `cascade_for_research_stage` / `cascade_with_optional_manual` in `vox-actor-runtime`. We change only that one function so all stages inherit the floor with zero per-call-site edits. The OpenRouter branch goes from emitting **one** candidate (the configured model) to emitting an **ordered list** that always includes `openrouter/free`. A pure `research_openrouter_model_ids()` helper computes the order; a pure config gate `research_prefer_free_tier()` (env `VOX_RESEARCH_PREFER_FREE_TIER`) controls front-vs-back. This aligns with the existing `vox_orchestrator::config::CostPreference::Economy` ("zero-cost and free-tier models are first-class choices") without threading a new parameter through every call site.

**Tech Stack:** Rust; crates `vox-config` (inference config accessors) and `vox-actor-runtime` (LLM cascade). Tests: `cargo test -p <crate>`. No new dependencies.

**Execution target:** Gemini 3.5 Flash in Google Antigravity. See [`gemini-3-5-flash-antigravity-limitations-2026-06-18.md`](../../src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md). **Hard rules for EVERY task:** (1) end GREEN + committed — a mid-task kill must leave a compiling, tested tree; never split a compile-breaking change across commits; (2) **verify-before-use** — run the listed `rg`/read before referencing any external symbol; inline exact signatures; never assume an API; (3) **self-contained** — each task repeats the context it needs; never rely on memory of earlier tasks; (4) **two-strike circuit breaker** — if a verification fails twice, STOP and write a handoff note, do not loop; (5) `cargo fmt -p <crate>` only (never `--all` — overflows Windows arg limit); (6) VoxScript-only automation (no new `.ps1`/`.sh`/`.py`).

**Spec / research basis:** [`deep-research-system-best-practices-research-2026-06-18.md`](../../src/architecture/deep-research-system-best-practices-research-2026-06-18.md) §5, §7 (gap G4). This is **Wave 1** of the 4-wave program; Waves 2-4 (readability extraction, reranking, loop quality, KB durability) get their own plans.

**Why "floor" not "force" (design critique of the prior draft):** the earlier draft *replaced* `input.openrouter_model` with `openrouter/free` whenever a flag was set — that made free tier the centerpiece and discarded the user's configured/premium model. The corrected design keeps the cascade **model-agnostic**: the configured model is tried first and `openrouter/free` is appended as a guaranteed fallback, so (a) quality is preserved by default, (b) research never hard-fails for lack of credits, and (c) cost-savers opt into free-first. Verified facts behind this: `CostPreference` has exactly two variants (`Performance`, `Economy`-default) and already declares free models first-class; `openrouter/free` exists as a virtual `ModelSpec` (`is_free: true`, RPM 20 / RPD 50). OpenRouter free tier = 20 req/min, 50 req/day (1,000/day after a one-time $10) per the research doc.

---

## Pre-flight (run once, before Task 1 — anti-hallucination baseline)

- [ ] **P0. Confirm a GREEN baseline.**

Run: `cargo test -p vox-config -p vox-actor-runtime`
Expected: PASS. If red at baseline, STOP and report — do not start on a red tree.

- [ ] **P1. Confirm the `OPENROUTER_FREE` constant and its re-export.**

Run: `rg -n 'OPENROUTER_FREE' crates/vox-config/src/bootstrap_inference.rs crates/vox-config/src/lib.rs`
Expected: `pub const OPENROUTER_FREE: &str = "openrouter/free";` in `bootstrap_inference.rs`, and `OPENROUTER_FREE` re-exported from `lib.rs` (so `vox_config::OPENROUTER_FREE` resolves).

- [ ] **P2. Confirm the cascade OpenRouter branch you will modify.**

Run: `rg -n 'openrouter_api_key|LlmConfig::openrouter|input.openrouter_model' crates/vox-actor-runtime/src/llm/cascade.rs`
Expected: inside `cascade_for_research_stage`, a branch:
```rust
if vox_config::inference::openrouter_api_key().is_some() {
    let mut openrouter = LlmConfig::openrouter(input.openrouter_model.clone());
    apply_stage_defaults(stage, &mut openrouter);
    candidates.push(openrouter);
}
```
This single branch is the only code site modified in Task 2. If its shape differs, adapt to the actual shape and note it in the commit.

- [ ] **P3. Confirm the env-read house pattern in `inference.rs`.**

Run: `rg -n 'pub fn openrouter_api_key|std::env::var' crates/vox-config/src/inference.rs crates/vox-actor-runtime/src/model_resolution.rs`
Expected: `openrouter_api_key()` exists at ~line 201; `std::env::var("VOX_SELECTOR_MODEL")` is read directly in `model_resolution.rs` — precedent that a non-secret behavioral flag may be read via `std::env::var` (no `vox-secrets` lifecycle needed).

- [ ] **P4. Confirm `LlmConfig::openrouter` exists and its arg.**

Run: `rg -n 'pub fn openrouter\(' crates/vox-actor-runtime/src/llm/types.rs`
Expected: `pub fn openrouter(model: impl Into<String>) -> Self`.

- [ ] **P5. Confirm the existing cascade tests (regression guard for Task 2).**

Run: `rg -n 'fn cascade_includes_local_candidate|fn synthesis_stage_does_not_force_1800|fn manual_candidate_is_first' crates/vox-actor-runtime/src/llm/cascade.rs`
Expected: all three exist. They must still pass after Task 2 (none asserts a single OpenRouter candidate, so appending a second is safe).

---

## Task 1: Prefer-free config gate [PARALLEL-SAFE — only touches `vox-config`]

**Files:**
- Modify: `crates/vox-config/src/inference.rs` (append after `openrouter_api_key()` at ~line 205, and add a test in the file's `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/vox-config/src/inference.rs` (create the module at end of file if absent):

```rust
#[test]
fn research_prefer_free_tier_parses_truthy() {
    use super::research_prefer_free_tier_from;
    assert!(research_prefer_free_tier_from(Some("1")));
    assert!(research_prefer_free_tier_from(Some("true")));
    assert!(research_prefer_free_tier_from(Some("TRUE")));
    assert!(research_prefer_free_tier_from(Some("  yes ")));
    assert!(research_prefer_free_tier_from(Some("on")));
    assert!(!research_prefer_free_tier_from(Some("0")));
    assert!(!research_prefer_free_tier_from(Some("false")));
    assert!(!research_prefer_free_tier_from(Some("")));
    assert!(!research_prefer_free_tier_from(None));
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-config research_prefer_free_tier_parses_truthy`
Expected: FAIL — `cannot find function research_prefer_free_tier_from in module super`.

- [ ] **Step 3: Write minimal implementation**

Insert after `openrouter_api_key()` (after ~line 205) in `crates/vox-config/src/inference.rs`:

```rust
/// True when research should try the OpenRouter **free tier first**
/// (`VOX_RESEARCH_PREFER_FREE_TIER`). This only REORDERS candidates — the free
/// tier is always present as a fallback floor regardless of this flag. Accepts
/// `1`/`true`/`yes`/`on` (case-insensitive, trimmed); unset/other → `false`.
///
/// Non-secret behavioral flag, read from the environment like `VOX_SELECTOR_MODEL`
/// in `vox-actor-runtime::model_resolution`.
#[must_use]
pub fn research_prefer_free_tier() -> bool {
    research_prefer_free_tier_from(std::env::var("VOX_RESEARCH_PREFER_FREE_TIER").ok().as_deref())
}

/// Pure parser for [`research_prefer_free_tier`] — testable without the environment.
#[must_use]
pub(crate) fn research_prefer_free_tier_from(raw: Option<&str>) -> bool {
    matches!(
        raw.map(|v| v.trim().to_ascii_lowercase()).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p vox-config research_prefer_free_tier_parses_truthy`
Expected: PASS.

- [ ] **Step 5: Format + commit**

```bash
cargo fmt -p vox-config
git add crates/vox-config/src/inference.rs
git commit -m "feat(vox-config): add VOX_RESEARCH_PREFER_FREE_TIER research cost-preference gate"
```

---

## Task 2: Free-tier fallback floor in the research cascade [SEQUENTIAL — depends on Task 1; touches `vox-actor-runtime`]

**Files:**
- Modify: `crates/vox-actor-runtime/src/llm/cascade.rs` (add a pure helper above `cascade_for_research_stage` at ~line 78; replace the OpenRouter branch at ~lines 96-100; add tests in the file's `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block at the bottom of `crates/vox-actor-runtime/src/llm/cascade.rs`:

```rust
#[test]
fn research_models_append_free_floor_by_default() {
    let v = research_openrouter_model_ids("anthropic/claude-sonnet-4.6", false);
    assert_eq!(
        v,
        vec![
            "anthropic/claude-sonnet-4.6".to_string(),
            vox_config::OPENROUTER_FREE.to_string(),
        ]
    );
}

#[test]
fn research_models_prefer_free_moves_it_first() {
    let v = research_openrouter_model_ids("anthropic/claude-sonnet-4.6", true);
    assert_eq!(
        v,
        vec![
            vox_config::OPENROUTER_FREE.to_string(),
            "anthropic/claude-sonnet-4.6".to_string(),
        ]
    );
}

#[test]
fn research_models_dedup_when_configured_is_already_free() {
    let v = research_openrouter_model_ids(vox_config::OPENROUTER_FREE, false);
    assert_eq!(v, vec![vox_config::OPENROUTER_FREE.to_string()]);
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p vox-actor-runtime research_models_`
Expected: FAIL — `cannot find function research_openrouter_model_ids in this scope`.

- [ ] **Step 3: Add the pure helper**

In `crates/vox-actor-runtime/src/llm/cascade.rs`, directly above `pub fn cascade_for_research_stage` (~line 78), add:

```rust
/// Ordered, de-duplicated OpenRouter model ids for a research call.
///
/// The free-tier router (`openrouter/free`) is ALWAYS included as a fallback floor
/// so research degrades to zero cost instead of failing. `prefer_free` moves it to
/// the front (opt-in via `VOX_RESEARCH_PREFER_FREE_TIER`); otherwise it trails the
/// caller-configured model. If the configured model already IS the free router,
/// the list collapses to a single entry.
#[must_use]
fn research_openrouter_model_ids(configured: &str, prefer_free: bool) -> Vec<String> {
    let free = vox_config::OPENROUTER_FREE.to_string();
    if configured == free {
        return vec![free];
    }
    if prefer_free {
        vec![free, configured.to_string()]
    } else {
        vec![configured.to_string(), free]
    }
}
```

- [ ] **Step 4: Run the new tests to verify they pass**

Run: `cargo test -p vox-actor-runtime research_models_`
Expected: PASS (3 tests).

- [ ] **Step 5: Wire the helper into the cascade**

In `crates/vox-actor-runtime/src/llm/cascade.rs`, replace the OpenRouter branch inside `cascade_for_research_stage` (currently):

```rust
    if vox_config::inference::openrouter_api_key().is_some() {
        let mut openrouter = LlmConfig::openrouter(input.openrouter_model.clone());
        apply_stage_defaults(stage, &mut openrouter);
        candidates.push(openrouter);
    }
```

with:

```rust
    if vox_config::inference::openrouter_api_key().is_some() {
        let prefer_free = vox_config::inference::research_prefer_free_tier();
        for model_id in research_openrouter_model_ids(&input.openrouter_model, prefer_free) {
            let mut openrouter = LlmConfig::openrouter(model_id);
            apply_stage_defaults(stage, &mut openrouter);
            candidates.push(openrouter);
        }
    }
```

- [ ] **Step 6: Run the full cascade module to confirm no regression**

Run: `cargo test -p vox-actor-runtime --lib llm::cascade`
Expected: PASS — including pre-existing `cascade_includes_local_candidate_when_profile_allows_it`, `manual_candidate_is_first_when_endpoint_and_model_are_supplied`, `synthesis_stage_does_not_force_1800_max_tokens`.

- [ ] **Step 7: Format + commit**

```bash
cargo fmt -p vox-actor-runtime
git add crates/vox-actor-runtime/src/llm/cascade.rs
git commit -m "feat(vox-actor-runtime): always append openrouter/free fallback floor to research cascade"
```

---

## Task 3: Document the flag in the search/research env SSOT [PARALLEL-SAFE — only touches docs]

**Files:**
- Modify: `docs/src/reference/tavily-integration-ssot.md` (the `## Environment Variable Summary` table)

- [ ] **Step 1: Verify the table exists and its column shape**

Run: `rg -n 'Environment Variable Summary' -A 6 docs/src/reference/tavily-integration-ssot.md`
Expected: a markdown table with header `| Variable | Purpose | Default |`.

- [ ] **Step 2: Add the row**

Under that table, add:

```markdown
| `VOX_RESEARCH_PREFER_FREE_TIER` | Reorder research LLM cascade to try `openrouter/free` first (the free floor is always present regardless) | `false` |
```

- [ ] **Step 3: Commit (do NOT touch any `*-index.md`/`SUMMARY.md` — those regenerate)**

```bash
git add docs/src/reference/tavily-integration-ssot.md
git commit -m "docs: document VOX_RESEARCH_PREFER_FREE_TIER research cascade flag"
```

---

## Task 4: Integration verification [SEQUENTIAL — run last]

**Files:** none (verification only).

- [ ] **Step 1: Build touched crates**

Run: `cargo build -p vox-config -p vox-actor-runtime`
Expected: clean build.

- [ ] **Step 2: Clippy on touched crates (repo merge policy)**

Run: `cargo clippy -p vox-config -p vox-actor-runtime -- -D warnings`
Expected: no warnings. (Never run workspace-wide clippy — `vox-gui`'s build script breaks it.)

- [ ] **Step 3: Run both crates' suites**

Run: `cargo test -p vox-config -p vox-actor-runtime`
Expected: PASS.

- [ ] **Step 4: Architecture parity gate (the GREEN gate)**

Run: `cargo run -p vox-arch-check`
Expected: exits 0. If red, STOP and write a handoff note (two-strike rule).

- [ ] **Step 5: Confirm only the three intended files changed**

Run: `git status`
Expected: clean working tree; the four commits above are the only new history.

---

## Flash Execution Addendum

- **Parallelization:** Task 1 (`vox-config`) and Task 3 (docs) are `[PARALLEL-SAFE]` — disjoint file writes, may run as separate subagents. Task 2 is `[SEQUENTIAL]` after Task 1 (it calls `research_prefer_free_tier`). Task 4 runs last. **Never** run two subagents that both write `cascade.rs`.
- **Atomicity:** each task ends GREEN + committed. A mid-task kill leaves a compiling, tested tree; the largest loss from a quota cutoff is one task.
- **Verify-before-use:** every external symbol used here (`vox_config::OPENROUTER_FREE`, `vox_config::inference::openrouter_api_key`, `LlmConfig::openrouter`, the `RouteResolutionInput.openrouter_model` field) is confirmed by a Pre-flight `rg`. Do not introduce any symbol not so confirmed.
- **Two-strike circuit breaker:** if any `Run:` step fails twice after an honest fix attempt, STOP, leave the last green commit in place, and write `docs/superpowers/handoffs/2026-06-18-g4-handoff.md` describing the failing step and observed output. Do not loop the same fix.
- **No env mutation in tests:** all unit tests target pure helpers (`*_from`, `research_openrouter_model_ids`) and never set/read process env — deterministic under parallel `cargo test`.

---

## Deferred (each its own grounded plan — NOT placeholders here)

Belongs to Wave 1's spirit but each needs its own file-grounding pass:

1. **20-RPM throttle governor.** OpenRouter free tier caps at 20 req/min; the LLM stages are currently unthrottled. Grounding required: the dispatch/await site (`infer_with_retry` in `vox-actor-runtime`) and any existing rate-limit utility, then a token-bucket keyed to free-tier RPM that only engages when a free model is selected.
2. **No-LLM degraded synthesis floor.** When neither local nor `openrouter/free` is reachable, return a structured SearXNG-only bullet summary instead of `Err` (today `chat_with_cascade` errors on empty candidates). Grounding required: `stages.rs::synthesize_answer_with_llm` call site + how the `Err` propagates upstream.
3. **Full `CostPreference`/`FreeTierRouter` integration.** This plan aligns with `CostPreference::Economy` conceptually but does not thread the preference through `RouteResolutionInput` or invoke `FreeTierRouter::route()` to pick specific `:free` slugs (e.g. `deepseek/deepseek-r1:free`). A later plan can replace the single `openrouter/free` router id with router-selected concrete free models, scored by `ModelScorer`. Deferred because it touches `RouteResolutionInput` + all stage call sites (too broad for one atomic Flash task).

Then proceed to Wave 2 (staged readability extraction with `rs-trafilatura`/`libreadability` + cross-encoder reranking) per the research doc §8.
</content>
