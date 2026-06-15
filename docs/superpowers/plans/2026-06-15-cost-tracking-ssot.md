# Cost-Tracking SSOT (§2 egress streaming-cost + §1 cost SSOT) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make LLM cost single-source, accurate (provider-reported preferred), and reactively surfaced to the user in the GUI — closing the loop so "what we route on == what we record == what we show."

**Architecture:** `vox-llm-egress` is the single cost *producer* (`EgressChatResponse.cost_usd`; streaming now surfaces it too). One pure `estimate_cost` helper replaces the two duplicated per-1k estimates. `vox-db` is the single *recorder* + *aggregator* (`llm_spend_summary`). The GUI gets a `get_llm_spend` command + a reactive `vox://llm-spend-changed` event, mirroring the Band-A pattern. Cost routing reads the same recorded aggregates.

**Tech Stack:** Rust (`vox-llm-egress`, `vox-actor-runtime`, `vox-db`, `vox-gui` Tauri 2), `reqwest`/`futures`, `wiremock`/in-memory SQLite for tests, vitest (GUI). Windows-safe formatting (`cargo fmt -p <crate>`, never `--all`).

**Branch:** `llm-ssot-united`. **Spec:** [`docs/superpowers/specs/2026-06-15-llm-ssot-remaining-roadmap-and-cost.md`](../specs/2026-06-15-llm-ssot-remaining-roadmap-and-cost.md) §1–§2.

**Per-phase close:** `/code-review` on the diff, then `cargo clippy -p <crate> -- -D warnings` (lib gate) + green tests before moving on.

---

## File Structure

| File | Responsibility | Phase |
|---|---|---|
| `crates/vox-llm-egress/src/wire.rs` (modify) | `stream_once` returns `(ChatStream, Option<f64>)` (response cost) | §2 |
| `crates/vox-llm-egress/src/lib.rs` (modify) | `estimate_cost(prompt_tokens, completion_tokens, cost_per_1k) -> f64` | §1.1 |
| `crates/vox-actor-runtime/src/llm/stream.rs` (modify) | adapt to new `stream_once` tuple | §2 |
| `crates/vox-actor-runtime/src/llm/chat.rs` (modify) | use `estimate_cost` instead of inline math | §1.1 |
| `crates/vox-actor-runtime/src/llm/types.rs` (modify) | `ModelMetric::from_response` uses `estimate_cost` | §1.1 |
| `crates/vox-gamify/src/ai/client/transport.rs` (modify) | `stream_openrouter` → core `stream_once`; remove allow-annotation | §2 |
| `crates/vox-db/src/store/ops_agents.rs` (modify) | `llm_spend_summary` aggregate query | §1.3 |
| `crates/vox-gui/src/commands/user_config.rs` (modify) | `get_llm_spend` command + `vox://llm-spend-changed` bridge | §1.4 |
| `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` (modify) | live spend-vs-budget display | §1.4 |

---

## §2 — Egress streaming-cost extension (do first; unblocks §1 + gamify)

### Task 2.1: `stream_once` surfaces the response cost

**Files:** `crates/vox-llm-egress/src/wire.rs`, `crates/vox-llm-egress/tests/wire_mock.rs`

- [ ] **Step 1: Write the failing test** (append to `wire_mock.rs`):

```rust
#[tokio::test]
async fn stream_once_surfaces_response_cost_header() {
    let server = MockServer::start().await;
    let sse = "data: {\"choices\":[{\"delta\":{\"content\":\"a\"}}]}\n\ndata: [DONE]\n\n";
    Mock::given(method("POST"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .insert_header("x-response-cost", "0.0034")
                .set_body_string(sse),
        )
        .mount(&server)
        .await;
    let r = req(format!("{}/chat/completions", server.uri()));
    let (mut s, cost) = stream_once(&r, &[], &ChatParams::default()).await.expect("stream");
    assert_eq!(cost, Some(0.0034), "streaming response cost must be surfaced");
    let mut got = String::new();
    while let Some(c) = s.next().await { got.push_str(&c.expect("chunk")); }
    assert_eq!(got, "a");
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test -p vox-llm-egress --test wire_mock stream_once_surfaces_response_cost_header`
Expected: FAIL — `stream_once` returns `ChatStream`, not a tuple.

- [ ] **Step 3: Change `stream_once`** in `wire.rs` to read the `x-response-cost` header before streaming and return it alongside the stream. Update the signature and the `res`-handling block:

```rust
pub async fn stream_once(
    req: &EgressRequest,
    messages: &[ChatMessage],
    params: &ChatParams<'_>,
) -> Result<(ChatStream, Option<f64>), EgressError> {
    // ... unchanged setup through `let res = http.send()...; let status = res.status();` ...
    // (429 / non-success handling unchanged)
    throttle::on_success(&req.throttle_key);
    let cost_usd = res
        .headers()
        .get("x-response-cost")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<f64>().ok());
    let byte_stream = res.bytes_stream();
    let out = stream! { /* ...unchanged SSE assembly... */ };
    Ok((Box::pin(out), cost_usd))
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test -p vox-llm-egress --test wire_mock`
Expected: PASS (all stream tests; update the older `stream_once_assembles_sse_deltas` to destructure the tuple: `let (mut s, _cost) = stream_once(...)`).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-llm-egress/src/wire.rs crates/vox-llm-egress/tests/wire_mock.rs
git commit -m "feat(vox-llm-egress): stream_once surfaces x-response-cost"
```

### Task 2.2: Adapt the facade `llm_stream` to the tuple

**Files:** `crates/vox-actor-runtime/src/llm/stream.rs`

- [ ] **Step 1: Update the call site** — `llm_stream` ignores the streaming cost (it records via its own non-streaming telemetry path; streaming cost is gamify's concern):

```rust
    let (inner, _cost_usd) = vox_llm_egress::stream_once(&ereq, &wire_msgs, &params)
        .await
        .map_err(|e| e.to_string())?;
    let mapped = inner.map(|item| item.map_err(|e| e.to_string()));
    Ok(Box::pin(mapped))
```

- [ ] **Step 2: Run** `cargo test -p vox-actor-runtime --lib llm::` → PASS (compiles + cascade/types unchanged).
- [ ] **Step 3: Commit** — `git commit -m "refactor(vox-actor-runtime): adapt llm_stream to stream_once cost tuple"`.

### Task 2.3: Migrate gamify `stream_openrouter` onto the core; drop the exemption

**Files:** `crates/vox-gamify/src/ai/client/transport.rs`

- [ ] **Step 1: Write the failing test** — a gamify wiremock test asserting the migrated streaming path fires `cost_reporter` with the header value (point `OPENROUTER_BASE_URL` env at the mock).

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Replace `stream_openrouter`'s body** with: build an `EgressRequest` (gamify's key + headers, as `call_openrouter_static` does), call `vox_llm_egress::stream_once`, fire `cost_reporter` from the returned cost, then map the content stream to gamify's `Result<String, AiError>` items (RateLimited/errors mapped as before):

```rust
pub(crate) fn stream_openrouter(
    _http: &reqwest::Client,
    api_key: &str,
    model: &str,
    prompt: &str,
    cost_reporter: Option<super::CostReportFn>,
) -> Pin<Box<dyn Stream<Item = Result<String, AiError>> + Send>> {
    let model = model.to_string();
    let prompt = prompt.to_string();
    let api_key = api_key.to_string();
    Box::pin(async_stream::try_stream! {
        let resolved_key = resolve_openrouter_key(&api_key);
        if resolved_key.is_empty() {
            Err(AiError::AllProvidersFailed("OPENROUTER_API_KEY not set".to_string()))?;
        }
        let ereq = vox_llm_egress::EgressRequest {
            base_url: vox_config::openrouter_chat_completions_url(),
            api_key: resolved_key,
            model: model.clone(),
            headers: vec![
                ("HTTP-Referer".to_string(), "https://github.com/vox-foundation/vox".to_string()),
                ("X-Title".to_string(), "Vox Gamify".to_string()),
            ],
            throttle_key: "openrouter".to_string(),
            max_concurrent: 8,
        };
        let msgs = [vox_llm_egress::ChatMessage { role: "user".to_string(), content: prompt }];
        let params = vox_llm_egress::ChatParams { max_tokens: Some(512), ..Default::default() };
        let (mut inner, cost) = vox_llm_egress::stream_once(&ereq, &msgs, &params)
            .await
            .map_err(|e| match e {
                vox_llm_egress::EgressError::RateLimited { retry_after } => AiError::RateLimited {
                    provider: format!("openrouter:{}", model),
                    retry_after_secs: retry_after.map(|d| d.as_secs()),
                },
                other => AiError::AllProvidersFailed(other.to_string()),
            })?;
        if let (Some(reporter), Some(c)) = (cost_reporter.as_ref(), cost) {
            reporter(c);
        }
        while let Some(item) = inner.next().await {
            let chunk = item.map_err(|e| AiError::AllProvidersFailed(e.to_string()))?;
            yield chunk;
        }
    })
}
```

Then **remove** the `// vox-arch-check: allow llm-egress` annotation + the doc paragraph above `stream_openrouter` (it is now routed through the core).

- [ ] **Step 4: Run** `cargo test -p vox-gamify --lib ai::` → PASS. Confirm the detector no longer needs the exemption: `cargo test -p vox-code-audit --lib detectors::llm_provider_call` still green.
- [ ] **Step 5: Commit** — `git commit -m "refactor(vox-gamify): stream_openrouter via egress stream_once; cost preserved; exemption removed"`.

---

## §1 — Cost SSOT

### Task 1.1: Single cost-estimate home

**Files:** `crates/vox-llm-egress/src/lib.rs`, `crates/vox-actor-runtime/src/llm/chat.rs`, `crates/vox-actor-runtime/src/llm/types.rs`

- [ ] **Step 1: Write the failing test** (egress lib tests):

```rust
#[test]
fn estimate_cost_is_tokens_over_1k_times_rate() {
    assert!((estimate_cost(700, 300, 2.0) - 2.0).abs() < 1e-9); // 1000/1000 * 2.0
    assert_eq!(estimate_cost(0, 0, 5.0), 0.0);
}
```

- [ ] **Step 2: Run to verify it fails** — `cargo test -p vox-llm-egress --lib estimate_cost_is_tokens` → FAIL.

- [ ] **Step 3: Implement** in `lib.rs` (the single estimate home; pure, no I/O):

```rust
/// The ONE cost estimate: `(prompt+completion tokens)/1000 * cost_per_1k`. Used only when
/// the provider reports no cost. Callers must not re-implement this math.
#[must_use]
pub fn estimate_cost(prompt_tokens: u32, completion_tokens: u32, cost_per_1k: f64) -> f64 {
    ((prompt_tokens + completion_tokens) as f64 / 1000.0) * cost_per_1k
}
```

- [ ] **Step 4: Replace the two duplicated estimates.**
  - `chat.rs:66-70` — replace the inline `config.cost_per_1k.map(|c| ((prompt_tokens + completion_tokens) as f64 / 1000.0) * c)` with `config.cost_per_1k.map(|c| vox_llm_egress::estimate_cost(resp.prompt_tokens, resp.completion_tokens, c))`.
  - `types.rs:262-273` (`ModelMetric::from_response`) — replace `estimated_cost_usd: (total_tokens as f64 / 1000.0) * cost_per_1k` with `vox_llm_egress::estimate_cost(res.prompt_tokens, res.completion_tokens, cost_per_1k)`.

- [ ] **Step 5: Run** `cargo test -p vox-llm-egress --lib && cargo test -p vox-actor-runtime --lib llm::` → PASS.
- [ ] **Step 6: SSOT check** — `grep -rnE "/ 1000.0\) \* " crates --include=*.rs` returns only `estimate_cost` in lib.rs. Commit: `git commit -m "refactor: single estimate_cost home in vox-llm-egress; drop the two duplicates"`.

### Task 1.2: Single recorder (audit + assert)

**Files:** `crates/vox-db/src/store/ops_agents.rs` (read), a new test

- [ ] **Step 1:** `grep -rnE "INSERT INTO .*(llm_interactions|unified).*cost_usd|cost_usd\s*=|record_llm_outcome|record_unified_llm_turn" crates --include=*.rs`. Confirm a **single** SQL writer of `cost_usd` (expected: `ops_agents.rs::record_llm_outcome` / `record_unified_llm_turn`). If a second writer exists (e.g. a stray path), route it through the same fn.

- [ ] **Step 2: Add a regression test** (`ops_agents` tests) that inserting one outcome with `cost_usd = Some(0.01)` then reading it back yields `0.01`, and that the `model_scoreboard.cumulative_cost_usd` increments by the same. This pins the single-writer contract behaviorally.

- [ ] **Step 3:** Run the new test (in-memory SQLite per existing `vox-db` test setup) → PASS. Commit.

### Task 1.3: Single spend aggregate query

**Files:** `crates/vox-db/src/store/ops_agents.rs`

- [ ] **Step 1: Write the failing test** (mirror `ops_mens_cloud::cloud_cost_summary` test style):

```rust
#[tokio::test]
async fn llm_spend_summary_sums_recorded_costs() {
    let store = test_store().await; // existing helper
    // record two turns with known cost_usd (use record_llm_outcome)
    // ... insert outcome cost 0.01 and 0.02 ...
    let s = store.llm_spend_summary(Some("sess-1")).await.expect("summary");
    assert!((s.total_usd - 0.03).abs() < 1e-9);
    assert!((s.session_usd - 0.03).abs() < 1e-9);
}
```

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Implement** `LlmSpendSummary { session_usd, day_usd, total_usd }` + `llm_spend_summary(session_id: Option<&str>)` querying `SUM(cost_usd)` over the interactions table — total, `WHERE session_id = ?`, and `WHERE created_at >= start_of_day` (mirror the `cloud_cost_summary` query shape). Handle `NULL`/missing as `0.0`.

- [ ] **Step 4: Run** → PASS. Commit: `git commit -m "feat(vox-db): llm_spend_summary aggregate (session/day/total) over recorded cost_usd"`.

### Task 1.4: Reactive GUI spend surfacing

**Files:** `crates/vox-gui/src/commands/user_config.rs`, `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`

- [ ] **Step 1: Write the failing Rust test** (`user_config` tests):

```rust
#[test]
fn spend_dto_serializes_camel_case() {
    let d = LlmSpendDto { session_usd: 0.03, day_usd: 0.1, total_usd: 1.2,
                          daily_budget_usd: 5.0, per_session_budget_usd: 1.0 };
    let j = serde_json::to_string(&d).unwrap();
    assert!(j.contains("sessionUsd") && j.contains("dailyBudgetUsd"));
}
```

- [ ] **Step 2: Run to verify it fails.**

- [ ] **Step 3: Implement** `LlmSpendDto` + a `get_llm_spend` Tauri command that reads `llm_spend_summary` (via the GUI's db handle, like other `commands/*` db readers) and the budget caps from `vox_config::VoxConfig::load()`. Emit `vox://llm-spend-changed` — hook it where turns are recorded (simplest: bump `vox_config::snapshot` is wrong layer; instead the GUI polls on `vox://llm-config-changed` AND on an interval, OR add a lightweight `LLM_SPEND_CHANGED` Tauri event emitted from a `vox-db` write hook surfaced to the GUI bridge). **Recommended:** reuse the existing orchestrator status stream cadence — emit `vox://llm-spend-changed` from the same `spawn_orchestrator_status_stream` tick (it already runs), carrying the latest `get_llm_spend`.

- [ ] **Step 4: Run** `cargo test -p vox-gui --bins commands::user_config` → PASS (use the worktree sidecar + `ui/dist` workaround if building vox-gui in isolation: copy `target/release/vox-x86_64-pc-windows-msvc.exe` and `crates/vox-gui/ui/dist/` from the main repo).

- [ ] **Step 5: Frontend** — in `RuntimeConfigSection` (or a new `SpendSection`), invoke `get_llm_spend`, render `session/day/total` against the `daily_budget_usd`/`per_session_budget_usd` caps it already shows, and refresh on the `vox://llm-spend-changed` (or orchestrator-status) listener. Add a vitest asserting spend renders and updates on the event. Run `pnpm -C crates/vox-gui/ui exec vitest run` for the Settings test.

- [ ] **Step 6: Commit** — `git commit -m "feat(vox-gui): get_llm_spend + reactive spend-vs-budget display"`.

### Task 1.5: Cost routing reads the same aggregate

**Files:** `crates/vox-orchestrator/src/models/scoring.rs` (read), a test

- [ ] **Step 1:** Confirm the orchestrator's cost-based scoring reads recorded per-model cost from `model_scoreboard.cumulative_cost_usd` / the scoreboard (the same source the GUI sums), not a divergent estimate. Add a test that, given a scoreboard row with a known cost, the cost component of scoring reflects it. If scoring uses only the static `cost_per_1k`, document the intended convergence (it may legitimately use catalog price for *prospective* routing while the scoreboard records *actuals*) — assert both read the registry/scoreboard, not inline constants.

- [ ] **Step 2: Run** → PASS. Commit.

---

## Self-Review (completed during authoring)

- **Spec coverage:** §2 (egress streaming-cost) → Tasks 2.1–2.3; §1 single-estimate → 1.1, single-recorder → 1.2, single-aggregate → 1.3, reactive GUI surfacing → 1.4, routing-reads-same → 1.5.
- **Placeholder scan:** the GUI event-emission mechanism (1.4 Step 3) offers a concrete recommended path (reuse the orchestrator-status tick) rather than a vague "emit an event"; the routing convergence (1.5) is explicit about the prospective-price-vs-recorded-actuals distinction.
- **Type consistency:** `estimate_cost`, `stream_once -> (ChatStream, Option<f64>)`, `LlmSpendSummary { session_usd, day_usd, total_usd }`, `LlmSpendDto`, `get_llm_spend`, `vox://llm-spend-changed` used consistently across tasks.
- **Execution:** §2 sequential (2.1→2.2→2.3). §1: 1.1→1.2→1.3 sequential; **1.4 (GUI) and 1.5 (routing) are independent once 1.3 lands → parallel subagent tracks.**

> **Caveat for the implementer:** exact line numbers (`chat.rs:66`, `types.rs:262`, `wire.rs:153`, `ops_agents.rs:166`) are from reads on `llm-ssot-united`; confirm against live files. For the vox-db tests, follow the crate's existing in-memory-SQLite test setup. For vox-gui isolated builds, apply the sidecar + `ui/dist` copy workaround.
