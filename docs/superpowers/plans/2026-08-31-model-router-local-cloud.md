# Model Router — Local/Cloud Honesty Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `models::decide()` changes winner across trivial vs hard tasks, cannot surprise-bill, fails over when OpenRouter 429s, and Axis shows why a model was picked. Direct vendors and coding-benchmark priors are added without a fourth scoring axis.

**Architecture:** Cap is already `scoreboard_feedback_boost * 0.15` — this track **names** `HISTORY_PRIOR_MAX` and adds a fixture-registry winner test. MCP `resolve_mcp_chat_model*` already calls `decide()` — M09 is a **regression test**, not a new bind in `chat_turn.rs`. Locality: soft `prefer_local` vs hard `CandidateScope::LocalOnly` — do not conflate. Prompt-cache headers on the facade. Autonomic catalog cron stays behind a flag. Register `VOX_PREFER_LOCAL` if the clutch exposes it (not in env-vars today).

**Depends on:** Track 2 L11 snapshot. **M01/M02 `enforce_budget_guard` already live** — do not block on re-implementing it. Do **not** duplicate `docs/superpowers/plans/2026-08-01-free-tier-onboarding.md`.

## Audit corrections (spec §9)

- Function is `scoreboard_feedback_boost`, not `history_boost()`.
- `chat_turn.rs` is the wrong layer for `decide()`.
- Premium alias already `intelligence >= 50`. M10 = cost axis must **block** alias when cost-first.
- Composer `tier` is `local|mesh|cloud|auto` not `local|auto|cloud`.
- No `LlmConfig::anthropic` / `::ollama` — add if product-required; keys via secrets.
- Cascade lives in research; wire into **chat** egress for M08/M14.

**Tech Stack:** `vox-orchestrator` models, `vox-actor-runtime` llm facade, `vox-gui` Models + chat footer, `contracts/models/known-slugs.v1.json` (from Track 2).

**Spec:** [`docs/superpowers/specs/2026-08-31-platform-parity-design.md`](../specs/2026-08-31-platform-parity-design.md) M03–M14.

**Closes:** M03 M04 M05 M06 M07 M08 M09 M10 M11 M12 M13 M14.

## Global Constraints

Inherit spec §6. `crates/vox-orchestrator/src/models/` may be cursorignored — use `cargo test -p vox-orchestrator` names, not Read-on-ignore if the tool blocks; Shell `rg` is fine. No new scoring axis named locality. LLM only through the facade. No new crate edges to vendor SDKs — HTTP stays inside `vox-actor-runtime`.

---

## File map

| File | Role |
|---|---|
| Modify: `scoreboard_feedback_boost` (rg that name) | named `HISTORY_PRIOR_MAX: f32 = 0.15` |
| Modify: `crates/vox-orchestrator/src/models/select.rs` | fixture-registry winner test; cost vs premium |
| Test: `resolve_mcp_chat_model*` | M09 regression — **not** `chat_turn.rs` |
| Modify: `crates/vox-actor-runtime/src/llm/` | cache headers; 429 cascade **in chat**; Anthropic/Ollama constructors if missing |
| Modify: GUI clutch | existing `tier` `local\|mesh\|cloud\|auto` |

---

### Task 1: Cap history prior (M03)

**Files:** `scoring.rs` (and tests in the same module or `select.rs`).

**Interfaces:**
- Consumes: existing scoreboard boost
- Produces: `const HISTORY_PRIOR_MAX: f32` (pick 0.15 — document in module docs) so complexity features can still move the winner

- [ ] **Step 1: Failing test** in `select.rs` tests (module docs already mention SelectionAxes):

```rust
#[test]
fn trivial_and_hard_codegen_differ_in_winner() {
    let easy = decide_for_text("hi");
    let hard = decide_for_text_with_complexity("Design a lock-free ring buffer in Rust", 9);
    assert_ne!(
        easy.slug, hard.slug,
        "history prior must not freeze the winner; easy={:?} hard={:?}",
        easy.slug, hard.slug
    );
}
```

`decide_for_text` — wrap `models::decide` / `select_with_default_registry` as the crate already does. `rg "fn decide" crates/vox-orchestrator/src/models`. If the test is flaky because the catalog has one model, seed **two** registry entries in the test (low-intelligence vs high-intelligence) instead of the live catalog.

```rust
#[test]
fn history_boost_cannot_exceed_cap() {
    let boost = history_boost(1_000_000); // huge sample
    assert!(boost <= HISTORY_PRIOR_MAX + 1e-6);
}
```

- [ ] **Step 2:** `cargo test -p vox-orchestrator history_boost_cannot_exceed_cap` — if HEAD already `* 0.15`, **name the const** and make this test pass immediately; do not treat “FAIL unbounded” as the gate.

Related existing test: `explain_selection_ranking_changes_between_trivial_and_hard_complexity` in `models/tests.rs`. Prefer extending that over inventing a flaky live-catalog test.
- [ ] **Step 3:** clamp boost; if `best_for_with_filter` ignores complexity, pass `SelectionIntent` complexity through (already supposed to — fix the swallow).
- [ ] **Step 4:** PASS on a **fixture registry**, not the live 364-model dump.
- [ ] **Step 5:** commit `fix: cap model scoreboard history prior so complexity can change the winner`

---

### Task 2: M09 regression — `resolve_mcp_chat_model*` still calls `decide()`

**Files:** `crates/vox-orchestrator-mcp` model_route_policy `resolve.rs` (rg `resolve_mcp_chat_model`). **Not** GUI `chat_turn.rs`.

- [ ] **Step 1:** Test that unforced chat model resolution equals `decide(&req, &registry).slug` and honors `VOX_MODEL_FORCE`. If this **already passes**, commit a regression test only (`test: chat MCP path stays on models::decide`).

- [ ] **Step 2:** FAIL only if a hardcoded slug bypass exists.

- [ ] **Step 3:** Do not add a second `decide()` in Tauri `chat_turn`.

- [ ] **Step 4:** PASS.

- [ ] **Step 5:** commit `test: MCP chat model resolution stays on decide()`

---

### Task 3a: Cost axis blocks premium alias (M10)

**Files:** `select.rs` premium alias (`rg opus` / `intelligence >= 50`).

- [ ] **Step 1:**

```rust
#[test]
fn premium_alias_does_not_run_when_cost_axis_high() {
    let axes = SelectionAxes { cost: 90, responsiveness: 50, intelligence: 80 };
    let spec = decide_with_axes(axes);
    assert!(!spec.slug.contains("opus"), "cost-first must not jump to premium alias");
}
```

Use real `SelectionAxes` field types (`rg "struct SelectionAxes"`). Threshold: `cost >= 70` blocks alias (coverage). GUI chip if alias **would** fire: Track 6 can show it; this task at least returns a `blocked_premium: bool` on explain payload **or** a unit test that `would_premium_alias(axes)` is false when cost≥70.

- [ ] **Step 2:** FAIL. **Step 3:** gate alias on cost. **Step 4:** PASS. **Step 5:** commit `fix: cost axis >=70 blocks premium model alias`

### Task 3b: VRAM hard admit when prefer_local (M05)

**Files:** `select.rs` / local candidate filter. Caption in ModelsView: “Locality is a hard filter, not a fourth axis.”

- [ ] **Step 1:** `prefer_local_skips_model_over_vram` — catalog row `vram_gb: 70`, detected VRAM 8 → candidate **excluded**. Caption string test in GUI or rust.

Register `VOX_PREFER_LOCAL` in `env-vars.v1.yaml` this commit if the clutch uses it.

nvidia-smi parse failure: **admit with warn** (named residual — test: detect=None + prefer_local still includes candidate + log/flag `vram_unknown`).

- [ ] **Step 2–5:** commit `fix: prefer_local hard-excludes models over detected VRAM`

### Task 3c: Clutch SLA — auto TTFT 2s probe (M13)

**Files:** Loquela `tier` `local|mesh|cloud|auto`; mapper in orchestrator.

- [ ] **Step 1:** `auto_slow_local_falls_back_to_cloud` — mock local probe that sleeps >2s → winner slug is cloud allowlist, not local. `local` → `CandidateScope::LocalOnly` / `prefer_local`. `cloud` → cloud allowlist. `mesh` → existing mesh path or honest error if unimplemented.

- [ ] **Step 2:** FAIL (auto == default scorer with no probe).
- [ ] **Step 3:** probe once per session (timeout 2s). Do not invent a second clutch enum.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: auto clutch probes local TTFT then falls back to cloud`

---

### Task 4a: Direct provider constructors (M06)

**Files:** `vox-actor-runtime/src/llm/` (`rg struct LlmConfig`). `LlmConfig::openai` exists; add `anthropic` + `ollama` if missing. Keys via `vox_secrets::SecretId`. After new SecretId: `vox ci secret-env-guard` + `secrets-parity`.

- [ ] **Step 1:** tests `llm_config_anthropic_sets_host` / `llm_config_ollama_sets_host` — constructors exist and do not hardcode `api.anthropic.com` in **consumer** crates (facade crate may know the host).
- [ ] **Step 2–5:** commit `feat: LlmConfig constructors for Anthropic and Ollama`

### Task 4b: Prompt-cache headers in chat (M08)

**Files:** `rg cache crates/vox-actor-runtime/src/llm`. Chat egress (not only research).

- [ ] **Step 1:** `anthropic_cache_control_header_set_when_prefix_stable` — `cache_headers(Provider::Anthropic)` contains a real header **or** is empty with a comment that GUI must show `cache n/a`. Do not invent Anthropic header names.

- [ ] **Step 2:** FAIL if chat path never calls `cache_headers`.
- [ ] **Step 3:** wire into **chat** cascade.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `feat: prompt-cache headers on chat LLM egress`

### Task 4c: 429 failover cascade on chat (M14)

**Files:** `crates/vox-actor-runtime/src/llm/cascade.rs` — **use it in chat**.

- [ ] **Step 1:** `openrouter_429_falls_back_to_next_provider` — wiremock 429 then 200 on second URL (see `openrouter_free_floor_smoke.rs` for wiremock). After provider exhaustion: NeedsYou / structured error, not a panic.

- [ ] **Step 2–5:** commit `feat: chat LLM cascade on 429/5xx`

---

### Task 5a: Coding-prior JSON (M04)

**Files:** `contracts/models/coding-prior.v1.json` ≤20 slugs `{ "swe_score": 0.0 }`. Scorer reads `swe_score`.

- [ ] **Step 1:** `coding_prior_breaks_intelligence_tie` — slug with 0.9 beats 0.1 when axes prefer intelligence and other features tie.
- [ ] **Step 2–5:** commit `feat: hand-curated coding-prior.v1.json for model scoring`. Nightly ingest job is the named residual — do not add a GitHub workflow in this task.

### Task 5b: Live `is_free` from price (M11)

**Files:** `rg is_free crates/vox-orchestrator/src/models`.

- [ ] **Step 1:** `price_zero_is_free_without_suffix` — price 0 ⇒ `is_free` even if slug lacks `:free`.
- [ ] **Step 2–5:** commit `fix: is_free derives from price not :free suffix`

### Task 5c: Autonomic flag-off noop (M07)

**Files:** `rg feature.*autonomic` / autonomic cron.

- [ ] **Step 1:** `autonomic_disabled_is_noop` — flag off, cron fn does not mutate catalog.
- [ ] **Step 2–5:** commit `test: autonomic catalog refresh is a noop when flagged off`. Enable-cron **docs** only — no CI cron in this task.

### Task 5d: Opt-in best-of-N (M12)

**Files:** agent loop / chat; `contracts/config/env-vars.v1.yaml` `VOX_BEST_OF_N`.

- [ ] **Step 1:** default `n==1`. `n=3` only when `permission_mode=plan` **and** intelligence axis ≥ 80 **and** `VOX_BEST_OF_N=1`. Test: `best_of_n_default_is_one`. Pick-by-tests is residual (needs L05).
- [ ] **Step 2–5:** commit `feat: opt-in best-of-N under plan + intelligence + VOX_BEST_OF_N`

### Task 6: Budget Exceeded retry prefer_local (M02 remaining, if T0A lacks it)

If Track 0A already retries local on Exceeded, this is a regression test only. Else: on `enforce_budget_guard` Exceeded, one `prefer_local` retry. Test: `exceeded_retries_local_once`. GUI `$ remaining` is Track 6 Task 29.

---

## Track 4 gate

HARD: `cargo test -p vox-orchestrator history_boost_cannot_exceed_cap trivial_and_hard_codegen_differ_in_winner`

HARD: MCP resolver regression (Task 2) — **not** `cargo test -p vox-gui chat_turn_model_matches_decide`

HARD: no premium alias when cost axis ≥ 70 (`premium_alias_does_not_run_when_cost_axis_high`)

HARD: `prefer_local_skips_model_over_vram` + `auto_slow_local_falls_back_to_cloud` + `openrouter_429_falls_back_to_next_provider` + `best_of_n_default_is_one`
