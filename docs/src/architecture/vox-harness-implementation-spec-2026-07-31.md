---
title: "Vox Harness Implementation Spec 2026-07-31"
description: "Implementation-level specification for bringing Claude-Code-grade agent-harness behaviour to Vox while exceeding it on local models: exact code changes, file paths and signatures for the tool-use loop, conversation state, context compaction, tool-result budgeting, and local-model registration — with TDD sequencing, per-phase exit gates, and parallel-agent execution guidance."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Vox Harness Implementation Spec (2026-07-31)

> **Relationship to the parity plan.** [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md)
> answers *what* and *in what order*. **This document answers *how*, at the level of specific
> files, function signatures, and tests.** Where the two disagree, this one supersedes — it is
> written after a deeper code pass that found the plan's diagnosis incomplete (§1).
>
> **Critique of the prior plan, stated plainly**, since the request was to critique it:
>
> | Weakness | Correction |
> |---|---|
> | Diagnosed the *GUI* as loop-less; treated the MCP lane as "the good one" | **Wrong.** *No* path passes tools or history to a model (§1). The MCP lane is a correctly-routed single-shot completion. |
> | "Adopt Continue.dev's schema" — a throwaway line | §5 gives the exact struct, discovery call, registration path, and test |
> | Context management was one bullet in Phase 5 | §4 is now the largest section; it was the biggest genuine gap |
> | No TDD sequencing | Every phase now leads with its failing test (§2.3) |
> | No exit criteria — phases could be declared done by assertion | Every phase has a **gate** with a runnable command (§2.4) |
> | No parallelisation guidance | §2.5 specifies what can be worked concurrently and the file-ownership rule that makes it safe |
> | Assumed tools could simply be "passed" | **330 tools exist** — 6.6× the documented degradation threshold. Tool selection is a *day-one* design constraint, not an optimisation (§3.2) |

---

## 1. Revised diagnosis — the root cause is one layer deeper than the plan said

Five findings from the 2026-07-31 deep pass (audit doc §4B, findings F24–F28), each verified by
reading the exact line rather than inferring from names:

| # | Finding | Evidence |
|---|---|---|
| **F24** | **No tool is ever passed to any model.** Plumbing is complete wire-format-up; `mcp_infer_completion` passes `None, None` at `infer.rs:280-281`; `mcp_infer_tool_completion` has exactly one caller; zero non-test assignments of `Some(...)` to `LlmConfig.tools`. | `infer.rs:269-284` |
| **F25** | **Conversation history never reaches the model.** `context_parts` (message.rs:80–297) carries active file / selection / open files / research / KBs — not history. History is persisted under `chat_history:{id}` and returned for display only. | `message.rs:293-297, 543-553` |
| **F26** | Multi-turn `llm_chat(opts, Vec<LlmChatMessage>, config)` exists; every caller is a fixed system+user batch pair. No conversational caller. | `llm/types.rs:21`, eval/judge/route callers |
| **F27** | No global tool-result budget. Only ad-hoc byte caps (`safe_truncate_for_prompt(_, 8000)`). | `mentions.rs:62` |
| **F28** | `llm_stream` exists **and already maps `config.tools` to the wire format** — the most agent-ready code in the tree. Chat doesn't use it. | `stream.rs:39` |

**Synthesis:** Vox is a **tool provider**, not an agent harness. Its MCP server exposes 330
tools to *external* harnesses (Claude Code, Cursor). Its own chat is a bare completion endpoint.
The "it just works" feeling users get from Vox today is **Claude Code's harness** driving Vox's
tools. This spec builds Vox's own.

**The good news this reframes:** every phase below is *connecting existing, tested components*,
not building new subsystems. The wire format, provider adapters, tool schemas, dispatch table,
model selector, skill catalogue, and streaming transport all exist and have tests.

---

## 2. Execution model — TDD, gates, and parallelism

### 2.1 Why TDD is mandatory here specifically

Two findings make test-first non-negotiable rather than stylistic:

1. **F3a (inert scorer)** shipped and survived because nothing asserted that *different inputs
   produce different routing*. A characterization test would have caught it the day it broke.
2. The testing research (testing doc §2.2) shows single-run verification is near-worthless for
   non-deterministic systems: even strong models hit `pass^8 < 25%`. **A feature "working once"
   in a manual check is not evidence.**

### 2.2 The three test layers, and which applies where

| Layer | Determinism | Use for | Runner |
|---|---|---|---|
| **Unit** | Fully deterministic | Budget arithmetic, message assembly, truncation, schema filtering, config parsing | `cargo test -p <crate>` |
| **Contract/golden** | Deterministic given a recorded fixture | Wire-format shape (does the request contain a `tools` array?), system-prompt composition, compaction output structure | `cargo test` + committed fixtures |
| **Behavioural/eval** | **Non-deterministic** | Does routing differ by task? Does the agent complete a task? | `vox model eval` / new `vox harness eval`, **multi-sample, pass^k** |

**Rule:** never assert a non-deterministic property in a unit test, and never assert a
deterministic property with an LLM call. Most of this spec's tests are layer 1 — deliberately,
because most of what's broken is wiring, and wiring is deterministic.

### 2.3 The per-phase TDD loop

```
1. RED     — write the failing test that encodes the finding
             (e.g. "request JSON contains a non-empty tools array")
2. VERIFY  — run it, confirm it fails for the RIGHT reason (not a compile error)
3. GREEN   — minimum change to pass
4. GATE    — run the phase's exit gate command (§2.4); it must pass
5. GUARD   — if the finding could regress silently, add a CI guard
             (vox-cli-ci has precedent: harness_trust_guard.rs)
```

Step 2 matters more than usual: several findings here are *absences*, and a test for an absence
trivially "fails" by not compiling. Confirm the failure is the assertion, not the build.

### 2.4 Phase gates

A phase is **not done** when its code is written. It is done when its gate command passes on a
clean checkout. Gates are cumulative — Phase N's gate re-runs all prior gates.

| Phase | Gate command | Passing criterion |
|---|---|---|
| **0** | `cargo test -p vox-orchestrator models::` | Routing differs across ≥5 task/complexity inputs; no `Tier: Unknown` for catalogued models |
| **1** | `cargo test -p vox-orchestrator-mcp chat_tools::` | Request contains non-empty `tools`; contains ≥2 messages on a 2nd turn |
| **2** | `vox model list \| grep -c ollama` > 0 | Local models present in catalog; `vox model explain` can select one |
| **3** | `cargo test -p vox-skills promotion_gate::` | All 8 gates enforced; a no-validator candidate is rejected |
| **4** | `pnpm test` + axe scan | Zero `aria-hidden` on live widgets; zero `role=alert` with controls |
| **5** | `vox harness eval --samples 5` | pass^5 ≥ baseline on the golden task set |

### 2.5 Parallel execution — what can run concurrently, and the rule that makes it safe

The phases touch **largely disjoint file sets**, so much of this is parallelisable across
subagents. The safety rule is simple and absolute:

> **One agent owns one file. Two agents never edit the same file in the same wave.**

Concurrency waves, derived from the file-ownership map:

```
WAVE A (fully parallel — disjoint crates, no shared files)
├── Agent 1: Phase 0.1  crates/vox-orchestrator/src/models/{select,scoring,registry}.rs
├── Agent 2: Phase 0.2  crates/vox-orchestrator/src/secretary.rs
│                       + crates/vox-gui/src/commands/chat.rs
├── Agent 3: Phase 4.1  crates/vox-gui/ui/src/components/**  (accessibility)
└── Agent 4: Phase 3.2  crates/vox-db/src/schema/domains/agents.rs (skill_candidates table)

  ⚠ Agents 2 and 3 both touch vox-gui but NOT the same files
    (2 = src/commands/, 3 = ui/src/components/). Safe.

BARRIER — Phase 0 gate must pass before Wave B
          (wiring chat into a broken scorer ships a regression that looks like a fix)

WAVE B (parallel after barrier)
├── Agent 5: Phase 1.1-1.2  chat_tools/chat/message.rs + llm_bridge/infer.rs
├── Agent 6: Phase 2.1      model catalog + Ollama discovery
└── Agent 7: Phase 3.3      skill promotion gate

BARRIER — Phase 1 gate

WAVE C (sequential — all touch the same request-assembly path)
└── Phase 4 context management: §4.2 → §4.3 → §4.4 in order
```

**Why Wave C is sequential:** compaction, token budgeting, and tool-result truncation all edit
the same request-assembly function. Parallelising them guarantees conflicts. This is the one
place where sequencing is a correctness requirement, not a preference.

**Workflow orchestration.** For Wave A/B, a `Workflow` script fanning out one agent per file-set
with a barrier between waves is appropriate — the work is independent, uniform, and
verification-gated. Do **not** fan out Wave C.

---

## 3. Phase 1 in full detail — the tool-use loop

### 3.1 What exists (verified, with signatures)

| Component | Location | Status |
|---|---|---|
| Tool name+description registry (330) | `contracts/mcp/tool-registry.canonical.yaml` → `TOOL_REGISTRY` via `build.rs` | ✅ generated |
| JSON Schema per tool | `input_schemas.rs:44` `tool_input_schema(name) -> Map<String,Value>` | ✅ + coverage test at `:878` |
| Tool dispatch | `dispatch.rs:36` `handle_tool_call(state, name, args) -> Result<String>` | ✅ |
| Permission-gated dispatch | `dispatch.rs:44` `handle_tool_call_with_mode(..., permission_mode)` | ✅ |
| Tools-capable inference | `infer.rs:287` `mcp_infer_tool_completion(..., tools, tool_choice, ...)` | ✅ **unused** |
| Wire mapping | `stream.rs:39` maps `config.tools` → `vox_llm_egress::ToolDef` | ✅ **unused** |

**Nothing in this table needs to be built.** The loop is an integration.

### 3.2 The 330-tool constraint — a day-one design requirement

`contracts/mcp/tool-registry.canonical.yaml` is **64,911 bytes** of names and descriptions
alone, before JSON schemas (`input_schemas.rs` is 1,126 lines). At ~4 chars/token that is
**~16k tokens minimum**, realistically **35–50k with schemas** — a quarter of a 200k window
consumed before the first user message.

Against the evidence (induction doc §1.1): Anthropic documents selection degradation past
**30–50 tools**. **Vox has 330 — 6.6× the threshold.** Passing all of them is not merely
expensive, it is *predicted to reduce selection accuracy*.

**Therefore the tool loop MUST ship with tool filtering. This is not Phase 3 polish.**

Three filters, in order, all cheap and deterministic:

```rust
// crates/vox-orchestrator-mcp/src/llm_bridge/tool_selection.rs   (NEW)

/// Select the tool subset exposed to the model for one turn.
/// Deterministic and unit-testable — no LLM call.
pub fn select_tools_for_turn(
    registry: &[McpToolRegistryEntry],
    ctx: &TurnContext,
) -> Vec<ToolDef> {
    registry.iter()
        // 1. PERMISSION: read-only sessions get the 43 http_read_role_eligible tools
        .filter(|t| ctx.permission_mode.allows(t))
        // 2. LANE: a chat turn does not need platform/interop tools
        //    (ai=166, app=22, data=54, interop=56, platform=24, workflow=8)
        .filter(|t| ctx.lanes.contains(&t.product_lane))
        // 3. ACTIVE SKILL: when a skill is pinned, honour its allowlist
        //    (skill_permissions.rs already implements this gate)
        .filter(|t| ctx.active_skill_allows(t))
        .take(ctx.max_tools)          // hard ceiling, default 48 — just under the
        .map(to_tool_def)             // documented 30-50 degradation band's top
        .collect()
}
```

**Test (RED first):**

```rust
#[test]
fn tool_selection_stays_under_degradation_threshold() {
    let tools = select_tools_for_turn(TOOL_REGISTRY, &TurnContext::default());
    assert!(!tools.is_empty(), "must expose some tools");
    assert!(tools.len() <= 48, "exposed {} tools; >48 degrades selection", tools.len());
}

#[test]
fn read_only_mode_exposes_only_read_eligible_tools() {
    let ctx = TurnContext { permission_mode: PermissionMode::ReadOnly, ..Default::default() };
    let tools = select_tools_for_turn(TOOL_REGISTRY, &ctx);
    assert!(tools.iter().all(|t| t.http_read_role_eligible));
}
```

**Escalation path when 48 is genuinely too few** — implement only if measured need arises,
per induction doc §1.2 (MCP-Zero's "Active Tool Request" is the cheap half of that paper and the
only half whose benefit was cleanly attributable): expose a `vox_tool_search` meta-tool that
takes a capability description and returns matching tool definitions. The model asks for what it
needs. **`vox_skill_use` is already exactly this pattern for skills** — mirror it for tools
rather than inventing a new mechanism.

### 3.3 The loop itself

```rust
// crates/vox-orchestrator-mcp/src/chat_tools/chat/agent_loop.rs   (NEW)

pub async fn run_agent_turn(
    state: &ServerState,
    session_id: &str,
    user_message: &str,
    budget: &mut TurnBudget,
) -> Result<AgentTurnOutcome, String> {
    let mut messages = load_conversation(state, session_id).await?;   // §3.4 — fixes F25
    messages.push(LlmChatMessage { role: "user".into(), content: user_message.into() });

    let tools = select_tools_for_turn(TOOL_REGISTRY, &turn_ctx);      // §3.2 — fixes F24

    for iteration in 0..budget.max_iterations {                       // default 24
        budget.check_wall_clock()?;                                   // hard stop

        let resp = llm_chat_with_tools(state, &messages, &tools, &cfg).await?;
        messages.push(resp.as_assistant_message());

        let Some(calls) = resp.tool_calls() else {
            persist_conversation(state, session_id, &messages).await?;
            return Ok(AgentTurnOutcome::Complete(resp.text()));
        };

        // Parallel dispatch — independent calls run concurrently, mirroring
        // Claude Code's PostToolBatch semantics (mechanics doc §7.2).
        let results = futures::future::join_all(
            calls.iter().map(|c| dispatch_one(state, c, budget))
        ).await;

        for (call, result) in calls.iter().zip(results) {
            messages.push(tool_result_message(call, budget.clamp(result)));  // §4.4 — F27
        }

        if budget.should_compact(&messages) {                          // §4.2
            messages = compact(state, messages, &budget).await?;
        }
    }
    Err("agent loop exceeded max iterations".into())
}
```

**Design decisions and their evidence:**

- **`max_iterations` bound.** Prevents runaway loops. 24 is a starting value to be calibrated,
  not a researched constant — flagged as such rather than presented as authoritative.
- **Parallel tool dispatch.** Independent calls execute concurrently. Claude Code exposes this
  boundary as `PostToolBatch` ("after a full batch of parallel tool calls resolves, before the
  next model call" — mechanics doc §7.2), which is direct evidence the batch is a real,
  designed-around boundary.
- **Compaction check inside the loop.** A long tool-using turn can exhaust context *within a
  single user turn*; checking only between turns is insufficient.
- **Permission gating is already solved** — route dispatch through
  `handle_tool_call_with_mode`, which exists, rather than the unmoded variant. This also fixes
  F17 (`permission_mode: None` hardcoded at 4 CLI sites) on the same code path.

**Test (RED first) — this is the test that would have prevented F24:**

```rust
#[tokio::test]
async fn chat_request_actually_includes_tools() {
    let req = capture_wire_request(|| run_agent_turn(&state, "s1", "list the files", &mut b)).await;
    let tools = req.get("tools").and_then(|t| t.as_array())
        .expect("F24 regression: request has no tools array");
    assert!(!tools.is_empty(), "F24 regression: tools array is empty");
}
```

### 3.4 Conversation state (fixes F25)

```rust
// crates/vox-orchestrator-mcp/src/chat_tools/chat/conversation.rs   (NEW)

/// Load prior turns as a proper message array. Replaces the display-only
/// `chat_history:{id}` blob that never reached the model (F25).
pub async fn load_conversation(
    state: &ServerState,
    session_id: &str,
) -> Result<Vec<LlmChatMessage>, String>;
```

Storage already exists (`chat_history:{session_id}` + `chat_transcripts`). **The change is to
feed it into the request**, not to build storage. Delete the `history.len() > 100` FIFO drain —
§4.2's compaction replaces it with a token-aware policy.

**Test:**

```rust
#[tokio::test]
async fn second_turn_sees_the_first() {
    run_agent_turn(&state, "s1", "my name is Ada", &mut b).await.unwrap();
    let req = capture_wire_request(|| run_agent_turn(&state, "s1", "what is my name?", &mut b)).await;
    let msgs = req["messages"].as_array().unwrap();
    assert!(msgs.len() >= 3, "F25 regression: history not sent (got {} messages)", msgs.len());
    assert!(serde_json::to_string(msgs).unwrap().contains("Ada"));
}
```

---

## 4. Phase 4 — Context management, in full

> The largest genuine gap in the prior plan. Vox currently has **zero** conversation
> compaction: `context_budget_manager.rs` is 48 lines that truncate JSON arrays (audit §9,
> correction 6), and the only history bound is a FIFO drain that never mattered because history
> never reached the model.
>
> A dedicated research batch (`wf_c187369d-cc9`) on compaction mechanics, token budgeting, and
> the API-level context primitives was still running when this spec was written; §4.6 records
> what it is expected to refine. **Everything asserted below is sourced to already-verified
> research** (mechanics doc §3, §5, §9) — nothing here depends on the pending batch.

### 4.1 The budget model

Total window is consumed by five claimants. Vox must track all five:

```
W = |system prompt| + |tool definitions| + |conversation| + |tool results| + |reserve|
```

For a 200k model with Vox's current shape:

| Claimant | Vox's current size | Note |
|---|---|---|
| System prompt | VOX.md + MEMORY.md + env + skill catalogue (≤64 skills × ~100 tok ≈ 6.4k) | `build_system_prompt_with_skill` |
| Tool definitions | **~35–50k if unfiltered (330 tools)** | §3.2 caps this at ≤48 tools ≈ 5–7k |
| Conversation | unbounded today | §4.2 |
| Tool results | unbounded today (F27) | §4.4 |
| Output reserve | must be held back | ≥ `max_tokens` |

**The tool-filtering decision in §3.2 is also the single largest context saving available** —
roughly 30–43k tokens per request. That is worth more than any compaction tuning.

### 4.2 Compaction — model-driven, with an explicit preserve list

Per the verified mechanics (mechanics doc §3.1), Anthropic's own description of Claude Code's
compaction is quoted directly:

> "we implement this by passing the message history to the model to summarize and compress the
> most critical details. The model preserves architectural decisions, unresolved bugs, and
> implementation details while discarding redundant tool outputs or messages. The agent can
> then continue with this compressed context plus the five most recently accessed files."

Four separately-implementable elements, each mapped to a Vox decision:

| Element | Implementation |
|---|---|
| **Model-driven, not rule-based** | Call an LLM to summarise. Do **not** write a heuristic that guesses importance. Use a *cheap* model — this is exactly the `SelectionIntent` for a summarisation task, and a good first consumer of the fixed scorer. |
| **Explicit preserve list** | architectural decisions · unresolved bugs · implementation details. This is a **prompt**, therefore tunable and testable. |
| **Explicit discard list** | redundant tool outputs · redundant messages |
| **Recency backstop** | re-attach the N most recently accessed files after compaction |

```rust
// crates/vox-orchestrator-mcp/src/chat_tools/chat/compaction.rs   (NEW)

pub struct CompactionPolicy {
    pub trigger_ratio: f32,        // fraction of window that triggers (start 0.85)
    pub preserve_recent_turns: usize,  // never compact the last N (start 4)
    pub reattach_recent_files: usize,  // Anthropic's documented value is 5
}

pub async fn compact(
    state: &ServerState,
    messages: Vec<LlmChatMessage>,
    policy: &CompactionPolicy,
) -> Result<Vec<LlmChatMessage>, String>;
```

**Recursion guard — required.** The compaction call is itself an LLM call and must not be able
to trigger compaction. Claude Code's teardowns describe a `querySource: 'compact'` flag checked
before triggering (mechanics doc §3.2, secondary-source). Vox's equivalent: tag the
`ActivityOptions` for the summarisation call and short-circuit `should_compact` when that tag is
present. **Vox's continuation engine has no such guard today** and this is a known class of bug.

**Tests (deterministic — no LLM needed for the structural properties):**

```rust
#[test]
fn compaction_preserves_recent_turns_verbatim() {
    let msgs = build_history(50);
    let out = compact_with_stub_summarizer(msgs.clone(), &policy);
    assert_eq!(&out[out.len()-4..], &msgs[msgs.len()-4..]);
}

#[test]
fn compaction_reduces_estimated_tokens() {
    let before = estimate_tokens(&msgs);
    let after  = estimate_tokens(&compact_with_stub_summarizer(msgs, &policy));
    assert!(after < before);
}

#[test]
fn compaction_cannot_recurse() {
    let opts = ActivityOptions { compaction_pass: true, ..Default::default() };
    assert!(!should_compact(&huge_history, &opts));
}
```

The summariser is stubbed in tests. Whether the summary is *good* is a Phase 5 eval question
(§2.2 layer 3), not a unit test — this separation is the point of §2.2.

### 4.3 Token estimation

Exact tokenisation requires a tokeniser per model family and Vox routes across five provider
lanes. The pragmatic approach, matching what Claude Code's teardowns describe (mechanics doc
§3.2, secondary):

```rust
/// Character heuristic: ⌈len/4⌉ + 1. Fast, ~15% error, no per-model tokenizer.
/// Deliberately CONSERVATIVE — over-estimating triggers compaction early,
/// which is a performance cost; under-estimating overflows the window,
/// which is a failure. Bias toward over-estimation.
pub fn estimate_tokens(text: &str) -> usize { text.len().div_ceil(4) + 1 }
```

Use the provider's reported `usage.input_tokens` when a response returns one, to correct drift.
`PromptDispatchTelemetryEvent` already carries token counts — feed them back.

### 4.4 Tool-result budgeting (fixes F27)

Claude Code's verified default is **25,000 tokens per tool response**, with a 10k warning
threshold and `MAX_MCP_OUTPUT_TOKENS` to adjust (mechanics doc §5.1). Its `Read` error is the
model to copy exactly:

> `File content (28375 tokens) exceeds maximum allowed tokens (25000). Please use offset and limit parameters.`

That error does three things — states the limit, states the actual, names the recovery. Vox's
truncation must do the same.

```rust
pub struct ToolResultBudget {
    pub max_tokens_per_result: usize,   // default 25_000, config VOX_MAX_TOOL_OUTPUT_TOKENS
    pub warn_tokens: usize,             // default 10_000
}

/// Truncate and ALWAYS signpost. Silent truncation makes the model
/// confidently summarize content it never saw.
pub fn clamp_tool_result(raw: String, budget: &ToolResultBudget) -> String {
    let est = estimate_tokens(&raw);
    if est <= budget.max_tokens_per_result { return raw; }
    let keep = budget.max_tokens_per_result * 4;
    let boundary = raw.floor_char_boundary(keep);
    format!(
        "{}\n\n[TRUNCATED: {} of ~{} tokens shown. Re-run with narrower \
         parameters (offset/limit/filter) to see more.]",
        &raw[..boundary], budget.max_tokens_per_result, est
    )
}
```

**Test:**

```rust
#[test]
fn truncation_is_always_signposted() {
    let out = clamp_tool_result("x".repeat(500_000), &ToolResultBudget::default());
    assert!(out.contains("[TRUNCATED"), "silent truncation is a correctness bug");
    assert!(out.contains("offset"), "must name the recovery mechanism");
}
```

`safe_truncate_for_prompt` (mentions.rs:62) already appends `...[truncated]...` — correct
instinct, wrong scope. Generalise it, keep its char-boundary safety.

### 4.5 Prompt-cache stability — and a correction

> **A claim in this spec's first draft was wrong and is corrected here**, since the standing
> rule is that corrections get recorded rather than silently edited. I asserted that
> `message.rs:351` interpolating `session_ts` into the system prompt was "a cache-buster that
> should move." **It is not.** Prefix caching matches the longest common prefix; `session_ts`
> is appended *after* `build_system_prompt_with_skill(...)` returns, so the entire stable
> portion still matches. The placement is correct by construction.

Vox's skill catalogue **is** cache-correct, and deliberately so: `render_skill_catalog` sorts
alphabetically, caps at 64, truncates descriptions at 256 chars, and returns empty-string for
empty input — documented in-source as *"content-stable across turns (cache-safe)"*, explicitly
to avoid busting the DeepSeek/Anthropic prompt-prefix cache. That is careful, intentional work.

**The real finding is that none of it currently pays off.** A workspace-wide search for
`cache_control`, `anthropic-beta` cache headers, or any prompt-caching configuration returns
**zero non-test hits.** Vox configures no prompt caching on any provider lane. The
cache-stability discipline is real, correct, and **currently unrealised** — the careful work is
in place and the feature that would benefit from it is switched off.

Two consequences, in priority order:

1. **Enable prompt caching** on the lanes that support it. Against the verified research
   (mechanics doc §3.2, secondary-source), a static/dynamic system-prompt split is reported to
   save up to 90% of system-prompt cost. With Vox's system prompt carrying VOX.md + MEMORY.md +
   a 64-skill catalogue, that is the largest single recurring cost in a chat turn after tool
   definitions.
2. **Then** enforce prefix stability as a rule, because it starts mattering the moment (1)
   lands. Note the current ordering places `ANTI_LAZINESS_RIDER` *after* the varying
   `session_ts`, which would push the rider outside any cacheable prefix — a small
   inefficiency to fix when caching is enabled, not before.

```rust
#[test]
fn system_prompt_prefix_is_stable_across_identical_calls() {
    let a = build_system_prompt_with_skill(&state, None, None, None).await;
    let b = build_system_prompt_with_skill(&state, None, None, None).await;
    assert_eq!(a, b, "identical inputs produced differing system prompts");
}
```

**Note this test guards the *stable* builder only** — it deliberately does not assert anything
about the assembled `system_prompt` in `message.rs`, which legitimately varies via `session_ts`.
Asserting on the assembled string would encode my original error as a test.

### 4.6 The tier ladder (from the context-management research)

[`context-management-mechanics-research-2026-07-31.md`](context-management-mechanics-research-2026-07-31.md)
§6 establishes the implementable order, cheapest first. Restated here as the build order:

| Tier | Mechanism | LLM call? | Measured effect |
|---|---|---|---|
| **0** | Don't create the problem — cap tool set (§3.2) + cap tool results (§4.4) | No | **~30–43k tokens/request saved** |
| **1** | **Clearing** — drop oldest tool results, keep last N | **No** | Peak 335k → 173k |
| **2** | **Externalize specifics** before compacting | No | Prevents the §4.6.1 loss |
| **3** | **Compaction** — LLM summarize-and-replace | Yes | Peak → 169k, **final → 5.8k** |
| **4** | Sub-agent isolation | Yes | Parent never sees the tokens |

**Build tiers 0 and 1 first.** They are deterministic, unit-testable without an LLM, and
recover roughly half of peak usage between them.

#### 4.6.1 The compaction loss profile — design around this

The measured result (research doc §2.2): compaction preserved **3/3 high-level facts but
0/3 obscure specifics.**

Compaction reliably keeps the *shape* of the work and reliably loses *details*. A harness that
compacts and then needs an exact error string, line number, or version constraint **will not
have it**. Hence tier 2: **write exact specifics to a file or memory entry before the summarizer
runs.** This is not optional polish — it is the mitigation for a measured failure mode.

#### 4.6.2 `clear_at_least` — the cache-economics guard

Clearing invalidates the cached prefix from the clearing point onward and incurs a cache-write
cost. Anthropic's parameter exists precisely to amortize that: *"Helps determine if clearing is
worth breaking prompt cache."* Vox's implementation needs the same guard — **never break the
cache to reclaim less than the rewrite costs**.

```rust
pub struct ClearingPolicy {
    pub trigger_tokens: usize,     // Anthropic default 100_000
    pub keep_tool_uses: usize,     // Anthropic default 3
    pub clear_at_least: usize,     // abort if less reclaimable
    pub exclude_tools: Vec<String>,// MUST include memory + active-skill tools
}
```

`exclude_tools` must include memory operations — clearing them destroys the record of what was
saved, defeating tier 2.

### 4.7 Dynamic skill / MCP-server loading — and the cache constraint that governs it

> **This addresses the requirement to load and unload skills and MCP servers to respect context
> limits. The research produced a hard constraint that shapes the entire design.**

**The constraint** (research doc §4.1, confirmed 3-0): prompt caching renders in the order
**`tools` → `system` → `messages`**, and *"any byte change anywhere in the cached prefix
invalidates everything after it."*

**`tools` renders first.** Therefore **changing the tool set invalidates the entire cache** —
tools, system prompt, and every message. Continuously hot-swapping MCP servers or skills
per-turn is the single most cache-destructive thing this harness could do.

This does not mean "never change the tool set." It means **change it at deliberate boundaries
and pay the rewrite knowingly.**

#### 4.7.1 The three-tier loading model

```
TIER A — PINNED (never unloaded, always in the cache prefix)
  · Core file/search/exec tools the agent needs in nearly every turn
  · The memory tool (excluded from clearing per §4.6.2)
  · Target: ~20 tools

TIER B — SESSION-SCOPED (fixed for a session; changing = deliberate cache rewrite)
  · Lane-selected tools for the session's work type (§3.2)
  · The active skill's allowlisted tools
  · Target: ~28 tools, total with Tier A ≤ 48

TIER C — ON-DEMAND (never in the tool array at all)
  · The remaining ~280 of 330 tools
  · Reached via a vox_tool_search meta-tool: model describes a capability,
    receives matching definitions, calls through a generic invoke
  · Zero cache cost — the tool array never changes
```

**Tier C is the mechanism that makes 330 tools workable without touching the cache.** It mirrors
`vox_skill_use`, which already does exactly this for skills: the catalogue advertises ~100
tokens of metadata, and the body loads on demand. **Reuse the pattern rather than inventing
one.**

#### 4.7.2 When a reload is permitted

```rust
pub enum ToolsetBoundary {
    SessionStart,        // free — no cache exists yet
    SkillActivation,     // deliberate; user-initiated
    ExplicitUserRequest, // "enable the browser tools"
    PostCompaction,      // cache already invalidated — reload is FREE here
}
```

**`PostCompaction` is the important one.** Compaction already invalidates the prefix, so it is
the one moment where changing the tool set costs nothing additional. **Batch all pending
toolset changes to the next compaction boundary** when possible.

Never reload mid-turn: research doc §4.4 confirms the cache survives an agentic loop *only*
while appended blocks are `tool_result`s. Changing tools mid-loop forfeits that.

```rust
#[test]
fn toolset_is_stable_within_a_turn() {
    let turn = run_recorded_turn_with_n_tool_calls(5);
    let sets: Vec<_> = turn.requests.iter().map(|r| tool_names(r)).collect();
    assert!(sets.windows(2).all(|w| w[0] == w[1]),
        "tool set changed mid-turn — forfeits prompt cache for the whole turn");
}
```

#### 4.7.3 MCP servers specifically

Vox's 330 tools come from `contracts/operations/catalog.v1.yaml`. **External** MCP servers
attached by a user add more. The same tiering applies, with one addition: an external server's
tools should default to **Tier C** (on-demand), never auto-promoted to Tier A/B, because (a)
they enlarge the cache-invalidating prefix and (b) per the security research, an
unvetted server's tool *descriptions* are an injection surface
([`skill-marketplace-security-and-provenance-research-2026-07-30.md`](skill-marketplace-security-and-provenance-research-2026-07-30.md)
§1.1 — tool poisoning works because the model sees the full description while the UI shows a
simplified one). **Keeping third-party tools out of the always-loaded prefix is a security
control as well as a context one.**

### 4.8 Per-model context budgeting — local vs cloud differ enormously

> **This addresses the requirement to consider context available *by model*, and how local
> resources differ from cloud.**

The catalog **already carries** the needed fields — verified by probing it during this pass:

| Model | `max_context` | `max_tokens` (output) | `supports_prompt_caching` | `cache_read_cost_per_1k` |
|---|---|---|---|---|
| `anthropic/claude-sonnet-5` | 1,000,000 | — | **true** | 0.0002 |
| `anthropic/claude-opus-4.7` | 1,000,000 | — | **true** | 0.0005 |
| `openai/gpt-5.2` | 400,000 | — | **true** | 0.000175 |
| `meta-llama/llama-3.1-8b-instruct` | 131,072 | 131,072 | false | — |
| `inclusionai/ling-2.6-flash` | 262,144 | 32,768 | false | — |

**A 7.6× spread in usable context across catalog models.** A budget policy hardcoded to one
number is wrong for almost every model.

```rust
pub struct ContextBudget {
    pub max_context: usize,        // from ModelSpec.capabilities.max_context
    pub output_reserve: usize,     // from ModelSpec.max_tokens — MUST be held back
    pub compact_at: f32,           // fraction of usable
    pub supports_caching: bool,    // from ModelSpec.supports_prompt_caching
    pub min_cacheable_prefix: usize, // §4.8.2
}

impl ContextBudget {
    pub fn usable(&self) -> usize { self.max_context.saturating_sub(self.output_reserve) }
    pub fn from_spec(m: &ModelSpec) -> Self { /* read, never hardcode */ }
}
```

**Note `ling-2.6-flash`: 262k context but only 32k output.** Reserving output naively as a
fraction of context would be wrong in both directions across this catalog. Read both fields.

#### 4.8.1 Local models are the constrained case, and the constraint is VRAM, not the advertised window

An Ollama model advertises a context length, but **the achievable window is bounded by VRAM**,
and Ollama defaults far below the maximum. Two Vox-specific consequences:

1. **`num_ctx` must be set explicitly.** Both Aider and Zed do this
   (gap-fill doc §1.3, §2.1: Aider via `extra_params.num_ctx`, Zed sends `num_ctx` defaulting to
   4096). A local model left at its default gets a fraction of its advertised window, silently.
2. **KV-cache growth is quadratic in context and competes with weights for the same VRAM.** The
   §5.5 fit estimate must include it — which is why `estimate_weights_bytes(d) +
   kv_cache_bytes(d)` is the formula there, not weights alone.

```rust
/// Effective local window = min(advertised, what fits in remaining VRAM after weights)
pub fn effective_local_context(d: &Deployment, avail_vram: u64) -> usize {
    let weights = estimate_weights_bytes(d);
    let for_kv  = avail_vram.saturating_sub(weights);
    let by_vram = (for_kv / kv_bytes_per_token(d)) as usize;
    d.context_window.unwrap_or(8192).min(by_vram)
}
```

**A local 8B at Q4 on a 12GB card may have ~5GB for KV after weights — a real window far below
its advertised 128k.** Budgeting against the advertised number would overflow on long turns.
This is a case where Vox must be *more* careful than any cloud-only harness, because the
constraint is invisible to the API.

#### 4.8.2 Caching differs by lane, and silently

Research doc §4.3: minimum cacheable prefix is **512 tokens** (Opus 5 / Fable 5 / Mythos 5),
**1,024** (Sonnet 5, Opus/Sonnet 4.x), 2,048 / 4,096 for others — and *"no error is
returned"* when a prompt is too short to cache.

Combined with the catalog data above:

| Lane | Caching | Implication |
|---|---|---|
| Anthropic direct / OpenRouter→Anthropic | Yes, `cache_control` + per-model minimum | Set breakpoint after the stable prefix |
| OpenAI | Yes (automatic, no breakpoints) | Different mechanism — do not send `cache_control` |
| **Ollama / local** | **No prompt cache** | Cache-stability work has **no payoff**; optimize for raw token count instead |

**So the §4.5 cache-stability discipline is worth real money on the cloud lanes and worth
nothing on the local lane.** The budget policy should reflect that: on local, prefer aggressive
clearing (tier 1) since there is no cache to protect; on cloud, respect `clear_at_least`.

**Verify caching is actually happening** via `response.usage.cache_read_input_tokens` — per
research doc §4.3 it fails silently, so an assertion is the only way to know.

```rust
#[test]
fn budget_is_read_from_model_spec_never_hardcoded() {
    for spec in [sonnet_5(), llama_8b(), ling_flash()] {
        let b = ContextBudget::from_spec(&spec);
        assert_eq!(b.max_context, spec.capabilities.max_context);
        assert!(b.usable() < b.max_context, "output reserve must be held back");
    }
}

#[test]
fn local_window_is_clamped_by_vram_not_advertised() {
    let d = deployment_ctx("qwen3:8b", /*advertised*/ 131_072);
    assert!(effective_local_context(&d, 12 * GB) < 131_072);
}
```

### 4.9 Research status — landed, with one gap

The context-management batch (`wf_c187369d-cc9`) **completed**: 23 confirmed claims, written up
in [`context-management-mechanics-research-2026-07-31.md`](context-management-mechanics-research-2026-07-31.md).
It supplied §4.6's tier ladder, §4.7's cache-render-order constraint, and §4.8's
per-model/caching mechanics — all of which were folded in above rather than left pending.

**One requested item did not survive verification: context-rot / lost-in-the-middle
measurements.** No claim about degradation as context grows reached the confirmed set. Treat
"long context degrades retrieval accuracy" as **plausible but unverified in this evidence
base**, and do not cite a specific curve or threshold. §4.2's `trigger_ratio = 0.85` therefore
remains **engineering judgement** (see §7), not a researched constant — the honest position is
that nobody in this evidence base has published the number that would justify one.

---

## 5. Phase 2 — Local models, precisely

> The prior plan said "adopt Continue.dev's schema shape." That is not implementable. Here is
> the exact mechanism.

### 5.1 The deployment record

Modelled on Continue.dev's `config.yaml` (tool-comparison doc §3.1), the best-documented local
schema surveyed, with Vox's `roles` mapped onto its existing `TaskCategory`:

```rust
// crates/vox-orchestrator/src/models/deployment.rs   (NEW)

pub struct Deployment {
    pub name: String,                    // display label, e.g. "Local Qwen3 8B"
    pub provider: ProviderKind,          // Ollama | OpenAiCompatible | OpenRouter | HfRouter…
    pub model: String,                   // "qwen3:8b"
    pub api_base: Option<String>,        // "http://localhost:11434"
    pub roles: Vec<TaskCategory>,        // reuses the EXISTING enum
    pub locality: Locality,              // Local | Cloud  — drives the privacy filter
    pub context_window: Option<usize>,   // for §4.1 budgeting + context-window filtering
    pub cost: Option<CostPerMillion>,    // None for local ⇒ zero, WITH the §5.4 capability floor
}
```

`Locality` is a first-class field, not inferred from a URL — this is the fix for F18 (Gemini
identity inferred by substring-sniffing a base URL).

### 5.2 Ollama auto-discovery — the concrete "natively add any model"

```rust
// crates/vox-orchestrator/src/models/discovery/ollama.rs   (NEW)

/// GET {base}/api/tags → register every local model as a Deployment.
/// Verified reachable during this audit: returned qwen3-vl:8b, qwen3:8b, vox-mens-v1.
pub async fn discover_ollama(base: &str) -> Result<Vec<Deployment>, DiscoveryError> {
    let tags: OllamaTags = http_get_json(&format!("{}/api/tags", base)).await?;
    Ok(tags.models.into_iter().map(|m| Deployment {
        name: m.name.clone(),
        provider: ProviderKind::Ollama,
        model: m.name,
        api_base: Some(base.to_string()),
        roles: infer_roles(&m),          // §5.3
        locality: Locality::Local,
        context_window: m.details.context_length,
        cost: None,
    }).collect())
}
```

Wire into the existing refresh path so `vox model list` includes local models — the direct fix
for F9a (zero of 364 catalog entries local while Ollama runs 3).

**Response shape, from the live probe run during this audit:**

```json
{"models":[{"name":"qwen3:8b","size":5231000000,
  "details":{"parameter_size":"8.2B","quantization_level":"Q4_K_M"}}]}
```

`parameter_size` and `quantization_level` are the inputs for §5.4's capability floor and §5.5's
VRAM gating — **available for free from the same call**, no extra probing.

**Test (no live Ollama required):**

```rust
#[tokio::test]
async fn discovers_local_models_from_tags_fixture() {
    let d = parse_ollama_tags(include_str!("fixtures/ollama_tags.json")).unwrap();
    assert_eq!(d.len(), 3);
    assert!(d.iter().all(|x| x.locality == Locality::Local));
    assert!(d.iter().any(|x| x.model == "vox-mens-v1:latest"));
}
```

### 5.3 Model-name addressing

Accept three equivalent forms so existing conventions keep working:

| Form | Example | Source |
|---|---|---|
| Explicit provider+model | `{provider: ollama, model: "qwen3:8b"}` | Continue.dev |
| Prefixed identifier | `ollama_chat/qwen3:8b` | **Aider + LiteLLM, independently converged** (gap-fill §1.1) |
| Bare name (local-first resolution) | `qwen3:8b` | Vox convenience |

`ollama_chat/` (not `ollama/`) is Aider's documented recommendation and matches LiteLLM's
convention — two tools converging independently is the reason to support it.

### 5.4 The capability floor — the trap this prevents

LiteLLM's cost-based routing is **comparative** (cheapest passing candidate), not
**threshold-based**. Registering a local endpoint at zero cost makes it **win
unconditionally** (routing doc §1.4). Naively "preferring local" therefore routes the hardest
reasoning task in the session to an 8B model.

```rust
/// A local model may only serve a task when it clears the floor for that task's
/// complexity. Prevents "free ⇒ always wins".
pub fn local_eligible(d: &Deployment, intent: &SelectionIntent) -> bool {
    match intent.complexity {
        1..=3 => true,                                    // trivial: any local model
        4..=6 => d.parameter_billions() >= 7.0,
        7..=10 => d.parameter_billions() >= 30.0
                  && d.quantization() >= Quant::Q5,       // hard tasks need real capacity
        _ => false,
    }
}
```

**These thresholds are engineering judgement, not research findings, and are labelled as such.**
No surveyed tool publishes capability floors (tool-comparison doc §0: none do hardware gating at
all). They are a starting point to calibrate against `vox model eval` results — the honest
status is "provisional, instrumented," not "derived."

**Test — the trap, encoded:**

```rust
#[test]
fn zero_cost_local_does_not_win_hard_tasks() {
    let local_8b = deployment("qwen3:8b", 8.0, Locality::Local, /*cost*/ None);
    let hard = SelectionIntent { complexity: 9, ..Default::default() };
    assert!(!local_eligible(&local_8b, &hard),
        "LiteLLM zero-cost trap: free local model won a complexity-9 task");
}
```

### 5.5 VRAM gating — where Vox leads the field

**No surveyed tool does this** — not Claude Code (no local support at all), not Aider, not
Continue.dev, not Zed (which explicitly defers to LM Studio's UI). Vox has
`vox-plugin-nvml-probe` in-tree and unwired.

```rust
pub fn fits_in_vram(d: &Deployment, avail_bytes: u64) -> Fit {
    let need = estimate_weights_bytes(d) + kv_cache_bytes(d);
    match need {
        n if n <= avail_bytes * 8 / 10 => Fit::Comfortable,
        n if n <= avail_bytes          => Fit::Tight,
        _                              => Fit::Exceeds,
    }
}
```

**Advisory, not blocking** — `Fit::Exceeds` deprioritises in ranking and surfaces a warning; it
does not remove the model. This follows the routing research's soft/hard distinction (routing
doc §2.2: latency/throughput are ranking hints, price and privacy are filters). A user with an
unusual setup must not be locked out by our estimate.

### 5.6 The privacy filter (fixes F7)

`VOX_MESH_EXEC_POLICY=local_only` governs **mesh task placement**, not inference. A
privacy-motivated user setting it gets no local inference — the most user-hostile finding in the
audit.

```rust
pub enum PrivacyClass { Any, NoTrainingUse, LocalOnly }

/// HARD filter, one-way ratchet: a per-request hint may TIGHTEN, never loosen.
/// Modelled on OpenRouter's zdr semantics (routing doc §2.3).
pub fn privacy_admits(d: &Deployment, effective: PrivacyClass) -> bool {
    match effective {
        PrivacyClass::LocalOnly => d.locality == Locality::Local,
        PrivacyClass::NoTrainingUse => d.locality == Locality::Local || d.no_train_certified,
        PrivacyClass::Any => true,
    }
}
```

**The boundary is enforced in `vox-llm-egress`, not in the router.** Routing chooses among
*permitted* candidates; it must never be the thing that makes a candidate impermissible
(routing doc §3.4 — caller-asserted metadata is a hint, not a trust boundary). Add a new
config key (`VOX_INFERENCE_PRIVACY`) and leave the mesh key alone, renaming it in docs so the
collision stops misleading users.

**Test:**

```rust
#[test]
fn local_only_privacy_excludes_every_cloud_model() {
    let admitted: Vec<_> = catalog().iter()
        .filter(|d| privacy_admits(d, PrivacyClass::LocalOnly)).collect();
    assert!(admitted.iter().all(|d| d.locality == Locality::Local));
}

#[test]
fn per_request_hint_cannot_loosen_account_policy() {
    assert_eq!(effective_privacy(PrivacyClass::LocalOnly, Some(PrivacyClass::Any)),
               PrivacyClass::LocalOnly, "ratchet must be one-way");
}
```

---

## 6. Consolidated gate checklist

Copyable per phase. A phase is done when **every** box is checked.

```
PHASE 0 — Scorer + secretary
  [ ] RED test: ranking differs across ≥5 task/complexity inputs — confirmed failing
  [ ] Root cause isolated (scoring fn vs Tier metadata vs explain display) — not guessed
  [ ] GREEN; `Tier: Unknown` no longer universal
  [ ] Secretary is propose-only; word-boundary matching
  [ ] GATE: cargo test -p vox-orchestrator models::
  [ ] CI guard added (routing-inertness regression)

PHASE 1 — Tool loop + conversation
  [ ] RED: request contains non-empty tools array — confirmed failing (F24)
  [ ] RED: 2nd turn request contains ≥3 messages — confirmed failing (F25)
  [ ] Tool selection ≤48 tools, permission + lane + skill filtered
  [ ] Loop bounded: max_iterations AND wall-clock
  [ ] Dispatch via handle_tool_call_with_mode (also fixes F17)
  [ ] GATE: cargo test -p vox-orchestrator-mcp chat_tools::

PHASE 2 — Local models
  [ ] Ollama discovery from fixture (unit) and live (integration)
  [ ] Capability floor blocks 8B on complexity-9
  [ ] Privacy filter is hard + one-way ratchet
  [ ] VRAM fit advisory, never blocking
  [ ] GATE: vox model list | grep -c ollama  > 0

PHASE 3 — Context management  (build tiers 0→1→2→3 in order, §4.6)
  TIER 0 — don't create the problem
  [ ] Tool set capped ≤48 (§3.2)
  [ ] Tool results capped + signposted (§4.4)
  TIER 1 — clearing (no LLM)
  [ ] Oldest-first clearing, keep last N
  [ ] clear_at_least guard: never break cache for less than rewrite cost (§4.6.2)
  [ ] exclude_tools includes memory + active-skill tools
  TIER 2 — externalize
  [ ] Exact specifics written before compaction (§4.6.1 — compaction loses 0/3 specifics)
  TIER 3 — compaction
  [ ] Preserves last N turns verbatim
  [ ] Reduces estimated tokens
  [ ] Recursion guard proven by test
  PER-MODEL + DYNAMIC LOADING
  [ ] ContextBudget read from ModelSpec, never hardcoded (§4.8)
  [ ] Local window clamped by VRAM, not advertised value (§4.8.1)
  [ ] num_ctx set explicitly on Ollama (default 4096 is a silent trap)
  [ ] Tool set stable within a turn (§4.7.2) — cache-preserving
  [ ] Third-party MCP tools default to Tier C, never auto-promoted (§4.7.3)
  [ ] Caching verified via usage.cache_read_input_tokens (fails silently otherwise)
  [ ] GATE: cargo test -p vox-orchestrator-mcp compaction:: context_budget::

PHASE 4 — Skills + UX (parallelisable)
  [ ] Promotion gate: all 8 steps, no-validator candidate rejected
  [ ] Zero aria-hidden on live widgets; zero role=alert with controls
  [ ] GATE: cargo test -p vox-skills && pnpm test

PHASE 5 — Eval
  [ ] Golden task set from Vox's own workspace (not a public benchmark)
  [ ] pass^5 reported, not pass@1
  [ ] Deterministic outcome checks preferred over LLM-judge
  [ ] GATE: vox harness eval --samples 5
```

---

## 7. Honest status of every number in this spec

Separating measured facts from engineering judgement, so nothing here is mistaken for a
research finding:

| Value | Status |
|---|---|
| 330 tools, 64,911 bytes, 43 read-eligible | **Measured** from the repo |
| 3 local Ollama models, response shape | **Measured** live |
| `tools: None`, single caller, no `Some` assignment | **Measured** by reading lines |
| 25,000-token tool-result cap | **Verified research** (mechanics §5.1) |
| 30–50 tool degradation threshold | **Verified research** (induction §1.1) |
| Compaction preserve/discard lists, 5 recent files | **Verified research**, quoted (mechanics §3.1) |
| `⌈len/4⌉+1` heuristic, ~15% error | **Secondary source** (teardowns), not vendor-confirmed |
| `max_tools = 48` | **Judgement** — top of the documented band |
| `max_iterations = 24` | **Judgement** — no research basis, calibrate |
| `trigger_ratio = 0.85` | **Judgement** — teardowns disagree (83.5% vs 92–95%); pending §4.6 |
| Capability floors (7B/30B/Q5) | **Judgement** — no tool publishes these |
| VRAM 80% comfortable threshold | **Judgement** |

**Every "judgement" row is a calibration target for Phase 5, not a constant to defend.**

---

## Appendix A — Every finding, planned against the codebase

> Superseded framing: this was originally "scoped, not planned." Per direct instruction, every
> item below now carries an **implementation plan against real files, functions, and structs**
> verified by reading the code — not a size estimate. Where a plan required checking something
> that turned out to contradict the original finding, that correction is recorded inline rather
> than silently fixed. Nothing is dropped; §A.4 items get an honest "cannot plan this, here is
> why" instead of a fake plan.

### A.1 Permission mode on the CLI dispatch path (F17)

**Correction first.** The finding named 4 sites; one (`dispatch_protocol.rs:50`) is a **test
fixture** constructing a `DispatchRequest` to validate against the DeI RPC JSON Schema — not a
production call site. **3 real sites**, all in `vox-cli-core/src/daemon_ipc/dispatch.rs`:
`call_daemon` (:38), `call_daemon_streaming` (:143), `subscribe_daemon` (:247).

None of the three take a `permission_mode` parameter at all — `permission_mode: None` is
hardcoded because there is nothing to thread. The two real callers that matter:

- `vox-cli/src/dei_daemon.rs:41` — `call(method, params, auto_open)` wraps `call_daemon` for
  `BINARY = "vox-orchestrator-d"`, with methods `ai.fix`, `ai.review`, `ai.generate`,
  `ai.plan.execute` — **the ones that can mutate files or run generated code.**
- `vox-cli/src/commands/runtime/run/run.rs:35` and `rollback.rs:15` target `vox-compilerd`, a
  build daemon — permission mode is not meaningful there (it doesn't touch arbitrary files by
  user command).

**Plan:**

1. Add `pub permission_mode: Option<String>` as a parameter to `call_daemon` and
   `call_daemon_streaming` (③ `subscribe_daemon` is read-only by construction — status streaming
   — and does not need it; confirm during implementation rather than add unused plumbing).
2. Thread it through `dei_daemon::call(method, params, auto_open, permission_mode)`.
3. Add a `--permission-mode <ask|accept_edits|accept_all|plan>` CLI flag wherever `dei_daemon::call`
   is invoked for the four mutating methods, mirroring the vocabulary already established at
   `vox-gui/src/commands/mcp.rs:23`.
4. **Do not fold it into `params`** — the existing GUI code comment at `mcp.rs:26` states this
   requirement explicitly (*"NEVER folded into `args`"*) and the same reasoning applies here:
   it must survive independently of whatever JSON shape a given method's params take.
5. This composes with Phase 1's `handle_tool_call_with_mode` routing — same vocabulary, same
   enum, no translation layer needed between the CLI and MCP dispatch paths.

```rust
#[test]
fn cli_permission_mode_reaches_the_wire_request() {
    let req = capture_dispatch_request(|| {
        dei_daemon::call("ai.fix", json!({}), false, Some("accept_edits".into()))
    });
    assert_eq!(req.permission_mode.as_deref(), Some("accept_edits"));
}
```

### A.2 Gemini locality via substring-sniffing (F18)

No new investigation needed — `model_resolution.rs:198-210`'s `route_backend_for_chat_route`
classifies a manual `base_url` as `GeminiDirect` by checking whether it contains
`generativelanguage.googleapis.com`, with a test pinning exactly that string match.

**Plan:** this is superseded, not separately fixed, by §5.1's `Deployment.provider: ProviderKind`
field. When `Deployment` replaces `ChatProviderRouteKind`'s manual-URL case, `ProviderKind::Gemini`
becomes an explicit enum variant set at registration time (when the user adds the endpoint),
not inferred per-request. Delete `route_backend_for_chat_route`'s URL-sniffing branch and its
test in the same commit that introduces `Deployment` — leaving both in place would be two
sources of truth for the same fact.

### A.3 `detect.rs` naming collision (F20)

**Plan:** `git mv crates/vox-skill-runtime/src/detect.rs crates/vox-skill-runtime/src/sandbox_probe.rs`,
update the `mod` declaration in `vox-skill-runtime/src/lib.rs`, and update the doc comment at
the top (currently *"Runtime preference detection for skill sandboxing"* — keep the content,
it's accurate) to lead with "sandbox runtime probe" rather than "runtime preference detection"
so a future grep for "skill detection" doesn't land here. No behavior change; `RuntimePreference`
and `RuntimeChoice` keep their names since those aren't the confusing part.

### A.4 Three `role="alert"` regions with interactive controls (F23)

Exact fix per file, since APG's guidance is explicit that `alert` "should only be used for text
content" and redirects to Alert Dialog when interaction is needed:

| File | Current | Fix |
|---|---|---|
| `components/layout/VersionMismatchBanner.tsx:14` | `role="alert"` wrapping a dismiss `<button>` | Change to a persistent banner: drop `role="alert"`, keep `aria-live="polite"` on a `role="status"` wrapper (matches `SecretaryToast`'s existing correct pattern), or use `role="alertdialog"` if it should interrupt |
| `components/surfaces/Console/Console.tsx:108` | `role="alert"` wrapping `copyLastBlock`/compose buttons | This is likely not alert-worthy content in the first place (a console pane, not a failure notification) — re-audit whether `role="alert"` belongs here at all before choosing a replacement role; if it's genuinely an error state, use `alertdialog` |
| `components/ui/ErrorBoundary.tsx:41` | `role="alert"` wrapping a reset `<button>` | Split into two elements: a `role="alert"` text-only region announcing the error, sibling to a plain (non-alert-scoped) reset button. This is the correct pattern per APG — the alert announces, a separate control acts. |

Each fix needs a companion test using `@testing-library/react`'s `getByRole('alert')` asserting
the alert region contains **no** `button`/`a` descendant.

### A.5 Panel-resurrection guard (F19)

Current: `ChatSurface.tsx:398` owns `closedPanelIds = useRef<Set<string>>(new Set())`, populated
at `:988` and `:1065` (`event.api.onDidRemovePanel(panel => closedPanelIds.current.add(panel.id))`),
and consulted at `:888`, `:903`, `:927` before any panel-recreation effect re-adds a core panel.

**Plan:** lift this exact mechanism into `DockWorkspaceShell.tsx` as an internal ref, since it is
generic to *any* dockview host, not specific to chat:

1. Add `closedPanelIds: React.RefObject<Set<string>>` to `DockWorkspaceShellProps`, constructed
   internally (not passed in) so every host gets one automatically.
2. Wire `onDidRemovePanel` inside the shell itself (currently `ChatSurface` wires it directly on
   `event.api`), populating the shell's own ref.
3. Expose a `isClosed(id: string): boolean` accessor via the `onReady` callback's event object
   (or a second callback prop) so a host's own recreation effects — like `ChatSurface`'s 5
   core-panel refresh — can consult it without maintaining a duplicate ref.
4. Port `ChatSurface.test.tsx`'s *"does not resurrect the Flow panel on the next render after
   the user closes it"* test to run against `DockWorkspaceShell` directly with a synthetic
   second host, proving the guard is host-agnostic.
5. `ChatSurface` keeps its own ref for now (removing it is a follow-up once the shell's version
   is proven) — this avoids a single change touching both the generic and the specific case at
   once.

### A.6 Byte-based, mentions-only truncation (F27a)

`chat_tools/chat/mentions.rs:62`'s `safe_truncate_for_prompt(content, max_bytes)` uses
`content.floor_char_boundary(max_bytes)` — correct UTF-8 safety, wrong unit (bytes, not tokens)
and wrong scope (only wired at `:152` for `@file` mentions, capped at 8000 bytes).

**Plan:** this becomes the direct implementation basis for §4.4's `clamp_tool_result`, not a
separate deliverable:

1. Move `safe_truncate_for_prompt` (keep its `floor_char_boundary` safety) into the new
   `crates/vox-orchestrator-mcp/src/llm_bridge/tool_budget.rs` alongside `clamp_tool_result`.
2. Change its signature to take `estimate_tokens(&str) -> usize` (spec §4.3) rather than a raw
   byte cap, and its call site in `mentions.rs:152` switches from `8000` bytes to a token budget
   consistent with `ToolResultBudget::default()`.
3. Its append string `"\n...[truncated]..."` becomes `clamp_tool_result`'s more informative
   signpost (spec §4.4) — same instinct, upgraded message.

### A.7 Prompt caching configured nowhere (new finding, from the context-management research)

Verified: `supports_prompt_caching` and `cache_read_cost_per_1k` are populated on live catalog
models (`Sonnet 5: true, $0.0002`; `GPT-5.2: true, $0.000175`) — the *data* exists. Verified:
`crates/vox-llm-egress/src/wire.rs`'s `OpenAiTool`/message structs carry **no `cache_control`
field anywhere** — the *wire format* does not implement it at all.

**Plan:**

1. Add `cache_control: Option<CacheControl>` to the message/content-block structs in
   `vox-llm-egress/src/wire.rs`, gated by a new `LlmConfig.enable_prompt_caching: bool` (reads
   from `ModelSpec.supports_prompt_caching`).
2. Set exactly one breakpoint at the end of the stable system-prompt prefix — the boundary
   already correctly established by `build_system_prompt_with_skill` returning *before*
   `session_ts`/`ANTI_LAZINESS_RIDER` are appended (verified correct in §4.5's correction).
3. Respect the per-model minimum (research doc §4.3: 512/1024/2048/4096 tokens) — skip setting
   `cache_control` entirely below the threshold rather than silently sending an ineffective one.
4. Surface `response.usage.cache_read_input_tokens` (or the provider-equivalent field) into
   `PromptDispatchTelemetryEvent`, which already exists and already carries token counts — this
   is an additional field, not a new event type.
5. **Ollama lane explicitly excluded** — no prompt cache exists locally (research doc §4.8.2);
   `enable_prompt_caching` must be false by construction for `ProviderKind::Ollama`, not merely
   false by absence of support metadata.

```rust
#[test]
fn cache_control_omitted_below_per_model_minimum() {
    let cfg = LlmConfig { enable_prompt_caching: true, ..sonnet_5_config() };
    let req = build_request(&cfg, &tiny_system_prompt(/* < 1024 tokens */), &[]);
    assert!(!req_has_cache_control(&req), "would silently no-op per research doc §4.3");
}
```

### A.8 `anthropic/claude-opus-4.8` catalog entry — correction, not a bug

**Investigated and the finding is wrong as originally stated.** `supports_prompt_caching: false`
is not a stale entry — it's `vox-orchestrator/src/catalog.rs`'s **discovery-seed default**,
present at 7 separate construction sites (e.g. `:221`, `:363`, `:704`) each commented
`// LiteLLM oracle fills this in`. This is the pre-enrichment value for any model the external
LiteLLM pricing/capability oracle hasn't yet classified — expected fallback behavior, not
drift.

**Revised plan:** the actual gap is that there's **no same-family fallback** — a brand-new
`claude-opus-4.8` entry falls back to `false` even though every other Anthropic-family model in
the catalog reports `true`. Add a fallback rule in the merge step: when the oracle has no entry
for a model whose `provider == "anthropic"` (or matches an existing family prefix), inherit
`supports_prompt_caching` from the most recent sibling in the same family rather than defaulting
to `false`. Small, targeted change to the merge logic in `catalog.rs`, not a data-entry fix.

### A.9 Graph coverage classifier — root cause found, not just a symptom

**Investigated, and this is a real, precisely-located bug**, not a calibration question as
originally framed. `vox-graph-reader/src/coverage.rs:102-131`'s `compute_coverage(graph, kind)`
applies one rule to every node kind:

```rust
let has_caller = links.iter().any(|l| str_field(l, "target") == Some(id));
if has_caller { Surfaced } else { OrphanBackend }
```

This checks **incoming** edges — correct for `kind == "command"` or `"tool"`, where a node
being *called* is the meaningful signal (and that's exactly why the earlier audit's
`--kind command` run showed genuine variety: `surfaced`/`orphan_backend` split realistically).

**It is structurally wrong for `kind == "surface"`.** A `surface:` node is a semantic root — a
GUI surface *dispatches to* commands, it is not itself dispatched to. Nothing in the graph ever
targets a `surface:` node by construction, so `has_caller` is `false` for every single one,
regardless of whether that surface is fully wired or completely dead. **This is a deterministic
100%-orphan result for a structural reason, not a coincidental one** — confirmed by the
`--kind command` counterexample proving the underlying edge data is fine.

**Plan:** add a per-kind branch alongside the existing `kind == "cli-command"` special case
(the file already has this pattern at :115-122 — `surface_reaches_cli`):

```rust
} else if kind == "surface" {
    // A surface is a root: classify by whether it reaches ANYTHING, not
    // whether anything reaches it.
    let has_outgoing = links.iter().any(|l| str_field(l, "source") == Some(id));
    if has_outgoing { CoverageStatus::Surfaced } else { CoverageStatus::OrphanBackend }
}
```

Add a fixture test with a synthetic surface node that has an outgoing edge to a command and one
with none, asserting they classify differently — the exact gap the current test suite doesn't
cover (its only surface-kind assertion, if any, would have passed against the buggy code).
`kind == "tool"` needs the same audit — the earlier run showed only 2 tool nodes total, too few
to tell whether it has the same root-vs-leaf confusion or is just an under-populated corpus;
check `TOOL_REGISTRY`'s 330 entries against the 1 tool node the graph actually contains before
assuming the fix is identical.

### A.10 Graphify has no query verb (F15)

This audit's cross-subsystem edge census and BFS reachability analysis (audit doc §1.1–1.2,
§4B) were performed with an ad-hoc Python script reading `.vox/cache/graphify/*/graph.json`
directly — proof the underlying data supports exactly these queries, with no CLI surface for
them.

**Plan:** add `vox graph query` as a new subcommand in `crates/vox-cli/src/commands/graphify/mod.rs`
(alongside the existing `GraphifyCmd` variants at :23), backed by a new
`vox-graph-reader/src/query.rs` implementing the three query shapes this audit actually used:

```rust
pub enum GraphQuery {
    Neighbors { id: String, direction: EdgeDirection, depth: usize },
    Path { from: String, to: String },
    CrossSubsystem { prefixes: Vec<String> },  // the census pattern from audit §1.1
}
```

Each maps directly onto a function already written ad-hoc for this audit (the `bfs()` helper,
the cross-subsystem edge counter) — this is **porting working Python into the existing Rust
crate**, not designing new algorithms. Expose the same three shapes as MCP tools per §4.7.1's
Tier C (on-demand, not pinned) — same query engine serving both the human CLI and the agent
loop, since the queries that make a good audit tool make an equally good agent tool for "who
calls this" / "is this reachable" questions the induction research's §2 (agentic search) says
grep cannot answer.

### A.11 Skill catalogue cap and truncation (F6a/F6b)

**Decision made, not deferred:** rank by `skill_reliability` + recency now; add the
`vox_skill_search` on-demand tier only if the library outgrows 64 in practice.

**Plan for the ranking change**, in `chat_tools/skill_catalog.rs`:

1. `render_skill_catalog`'s current `sorted.sort_by(|a, b| a.name.cmp(&b.name)); sorted.truncate(max)`
   (the alphabetical-then-truncate at :35-36) becomes a two-key sort: primary key = observed
   success rate from `skill_reliability` (nulls/no-data sort last, not first — an unproven skill
   shouldn't win over a proven one by virtue of having no failures yet), secondary key =
   alphabetical (for determinism among ties, preserving the existing cache-stability property —
   this is the one property that must not regress, since the module doc explicitly calls out
   cache-safety as a design requirement).
2. `CatalogEntry` needs a new field carrying the reliability score, populated by a join against
   `skill_reliability` (`vox-db/src/schema/domains/agents.rs:378`) at the same call site that
   currently does `reg.list(None)` in `build_system_prompt_with_skill`.
3. Log (not silently drop) when truncation actually removes a skill — a `tracing::info!` at the
   truncation point naming which skills fell off, so the F6a "silent" half of the finding is
   fixed independent of the ranking change.
4. Raise `DESC_CAP` from 256 toward 1,024 to match the authoring standard (mechanics doc §4.3) —
   pure constant change, verify it doesn't blow the ~6.4k token budget assumption in
   implementation spec §4.1 at the new cap (64 skills × up to 1024 chars ≈ 16k tokens worst
   case — re-check the token-budget math in spec §4.1 against this before shipping both changes
   together, since raising the cap changes an assumption spec §3.2 depends on).

### A.12 Toast noise (F12) — re-measure before redesigning

No change to the original disposition: correctly deferred. **Plan for when Phase 1 lands**:
re-run the same measurement this audit did (`grep -c "pushToast({" ... backend-error`) and
compare the post-Phase-1 `backend-error` share against the pre-Phase-1 baseline recorded in the
audit (118 of 179, ~66%). If it hasn't materially dropped, the noise is not purely a symptom of
F24/F25 and a standalone coalescing fix (replace `App.tsx:314`'s `.slice(-MAX_TOASTS)` oldest-drop
with a count-coalescing map keyed by `(tone, cause)`) should be scheduled independent of harness
fixes.

### A.13 Skill miner + reliability wiring (F13/F14)

No change to sequencing: correctly gated behind Phase 3.3's promotion gate. **Plan, to be
executed as part of Phase 3, not deferred indefinitely:**

1. Add the `skill_candidates` table to `vox-db/src/schema/domains/agents.rs` alongside the
   existing `skill_manifests`/`skill_executions`/`skill_reliability` (:125/:138/:378) — columns:
   `id`, `source_trajectory_ids` (join key back to the mining source), `body`, `status`
   (`provisional`/`confirmed`/`deprecated`, matching the `ModelConfidence` state-machine pattern
   named in spec §3.3), `promoted_at`, `retired_at`.
2. Wire `vox-skill-discovery`'s `code_miner`/`op_miner` (currently reachable only via
   `vox-cli/src/commands/extras/ars/skill_suggest.rs`) to write into this table instead of
   printing to stdout, gated behind the Phase 3.3 promotion pipeline consuming from it — the
   miner produces candidates, the gate decides what's promoted, they are separate stages.
3. Trigger on a schedule (reuse the existing `vox ci` cron/schedule infrastructure) rather than
   only on manual invocation, but **only after** the gate exists — this ordering is the whole
   point of A.13 being sequenced behind A.11/Phase 3.3, since an automatic trigger feeding an
   ungated table is the exact failure mode the Voyager 73%-value-from-gating finding warns
   against.

### A.14 `vox chat` CLI verb (F22)

No change to disposition: low user-facing value, real value as a Phase 5 eval surface. **Plan:**
add `vox chat <message> [--session <id>] [--json]` in `crates/vox-cli/src/commands/`, calling
the same `vox_chat_message` MCP tool path the GUI should be using post-Phase-1 (not a third
implementation) — its only job is being a scriptable entry point that `vox harness eval`
(spec §2.4, Phase 5 gate) can drive without an MCP client in the loop, which is exactly the gap
audit finding §4A.3 identified ("no way to exercise the working lane without an MCP client").

### A.15 Dashboard widget redesign (F10)

Split, as originally planned: the `aria-hidden` removal is Phase 4 (binding, WCAG 4.1.3 AA) —
plan is the direct fix in `SurfaceMiniRender.tsx:22-29`, replacing `aria-hidden="true"` with
proper live-region semantics per the UX research's ARIA mapping (agent-chat-ux doc §1), and is
already covered by the Phase 4 checklist.

**No plan is given for the thumbnail→summary-tile redesign**, and that is the correct call, not
a gap: no evidence in the entire research set compares the two approaches (UX doc §6.1 states
this explicitly). Planning an implementation against no evidence would mean inventing the
justification along with the code. If this becomes a priority, the correct next step is a
small, scoped usability comparison — not a design doc.

### A.16 Open research, not open engineering

These have no implementable action because the evidence does not exist yet. Listed so they are
not mistaken for oversights:

| Question | Status |
|---|---|
| Context-rot / lost-in-the-middle curve | **No claim survived verification** (§4.9). Blocks a principled `trigger_ratio`. |
| Right coalescing window for streaming status | SC 2.2.4 and HAX G3 establish frequency matters; **neither specifies a threshold**. |
| Summary tiles vs live thumbnails | No verified source either way. |
| Windsurf local-model support | **Unresearched after three attempts.** |
| Cursor / Zed exact config schemas | Partially closed; Zed's specific `settings.json` field names **refuted 0-3** from blog sources — needs a direct docs fetch. |
| PyPI anti-squatting / Trusted Publishers | In scope for the security batch, **no claims survived**. npm + crates.io are covered. |
| Real-world (non-PoC) MCP exploitation with CVEs | Not found. All MCP attack evidence is research PoC. |
| Does retrieval-based tool selection still pay off on frontier models? | Gain fell 25pp → 8.6pp from Opus 4 → 4.5. **Directly affects whether §4.7.1's Tier C is worth building.** |

### A.17 Explicitly not doing

| Item | Why |
|---|---|
| Raising the 64-skill cap | Actively harmful — past the documented 30–50 degradation band. |
| A bare "prefer local" boolean | Reproduces LiteLLM's zero-cost trap (§5.4): free local model wins every task including complexity-9. |
| Modelling local-model UX on Cursor | Its "support" is the generic OpenAI-base-URL override every tool has (gap-fill §0), not a design worth copying. |
| Hard-blocking models that exceed VRAM | Advisory only (§5.5). Our estimate can be wrong; a user with an unusual setup must not be locked out. |
| Adopting SWE-bench as the eval gate | Contamination-compromised (testing doc). Use Vox's own workspace tasks with deterministic outcome checks. |
| Building semantic tool retrieval now | The 30–50 threshold is handled by §3.2's filtering. Add retrieval only if measurement shows filtering is insufficient — per the induction research, start with the cheap mechanism. |
