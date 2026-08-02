---
title: "Vox Harness Audit (Graph-Backed) 2026-07-30"
description: "Graphify-backed audit of the Vox agent harness against the verified Claude Code baseline: the chat window has no agent loop, skill activation is manual-only, the model selector is unwired from chat, and the subsystems are structurally disconnected — each finding measured on a 29,315-node code graph and hand-verified."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Vox Harness Audit — Graph-Backed (2026-07-30)

> **Provenance.** Every finding below is (a) measured on a freshly rebuilt Graphify
> `repo-code-graph` corpus — **29,315 nodes / 30,170 edges**, HEAD `e14b63ab0b`, rebuilt in
> 19.0s — and (b) hand-verified by reading the cited source lines. Graph-only claims are
> marked as such and carry their methodology caveat inline. Two of my own first-pass
> findings were **corrected** by the graph and one was **corrected back** by hand
> verification; both corrections are recorded in §9 rather than silently dropped.
>
> Baseline: [`claude-code-harness-mechanics-2026-07-30.md`](claude-code-harness-mechanics-2026-07-30.md).
> Remediation: [`vox-harness-parity-plan-2026-07-30.md`](vox-harness-parity-plan-2026-07-30.md).
> Prior art this partly supersedes:
> [`orchestrator-gui-dispatch-audit-2026-07-02.md`](orchestrator-gui-dispatch-audit-2026-07-02.md).

---

## 0. The finding that subsumes the rest

**The Vox chat window is not an agent harness. It is a message log with a keyword-triggered
task submitter.**

Forward reachability from the chat window's message handler,
`crates/vox-gui/src/commands/chat.rs::chat_append_message`, over the full 29,315-node graph:

```
d0  crates/vox-gui/src/commands/chat.rs::chat_append_message
d1  crates/vox-gui/src/commands/chat.rs::pool_db
d1  crates/vox-gui/src/commands/chat.rs::secretary_candidate
d1  crates/vox-gui/src/commands/chat.rs::submitted_task_id
d1  crates/vox-gui/src/commands/orchestrator.rs::emit_secretary_proposed
d1  crates/vox-gui/src/commands/orchestrator.rs::emit_tasks_changed
```

**Seven nodes. Depth 1. Terminal.**

Save the message → keyword-classify it → fire a task at the daemon → emit two UI events.
There is no model call, no tool loop, no context assembly, no skill resolution, no system
prompt. Compare against the verified Claude Code baseline (§1 of the mechanics doc): *"agents
are typically just LLMs using tools based on environmental feedback in a loop."* **Vox's chat
path contains no LLM, no tools, and no loop.**

Everything else in this audit is a consequence of, or is masked by, that fact. Vox has built
an elaborate orchestration shell — 4,346 nodes in `vox-orchestrator` alone — around a chat
surface that never enters it.

---

## 1. Structural disconnection, measured

### 1.1 Cross-subsystem edge census

I partitioned the graph into the eight subsystems that constitute the harness and counted
every edge crossing a partition boundary:

| Subsystem | Path prefix | Nodes |
|---|---|---|
| `models` | `crates/vox-orchestrator/src/models` | 233 |
| `mcp-chat` | `crates/vox-orchestrator-mcp/src/chat_tools` | 89 |
| `skills` | `crates/vox-skills` | 57 |
| `skill-mining` | `crates/vox-skill-discovery` | 43 |
| `llm` | `crates/vox-actor-runtime/src/llm` | 41 |
| `ui-chat` | `crates/vox-gui/ui/src/components/surfaces/Chat` | 41 |
| `similarity` | `crates/vox-similarity` | 23 |
| `be-chat` | `crates/vox-gui/src/commands/chat` | 21 |

**Total cross-subsystem edges: 1.**

```
skill-mining -> similarity   1
```

That is the entire inventory. Zero edges between `be-chat` and `models`. Zero between
`be-chat` and `llm`. Zero between `skills` and `mcp-chat`. Zero between `skills` and
`models`. Zero between `ui-chat` and `be-chat`.

> **Methodology caveat, stated plainly.** Graphify's edges are *intra-language static call
> edges*. Two boundaries in Vox are dynamic and therefore invisible to it: the Tauri IPC
> boundary (`ui-chat` → `be-chat`) and the MCP JSON-RPC boundary. So the `ui-chat`/`be-chat`
> zero is an artifact and must not be cited as a defect.
>
> **The `be-chat` → `models` and `be-chat` → `llm` zeros are NOT artifacts.** Both sides are
> Rust in the same workspace; a call would produce an edge. They are zero because the calls
> do not exist — confirmed by grep in §2.2.

### 1.2 Path analysis: can chat reach model selection?

| From | To `models::decide` | To `llm_chat` |
|---|---|---|
| GUI `ChatSurface` (TSX) | directed **YES** (d=3) | no |
| GUI `chat_append_message` (Rust) | directed **no** | no |
| MCP `chat_message` tool | directed **YES** (d=3) | no |

Read carefully, this table is the split-brain in three rows:

- **`ChatSurface` reaches `decide` — but only to *display* it.** The path runs through
  `ChatModelPicker` / `get_routing_summary`. The UI shows the user which model *would* be
  chosen. Nothing dispatches through it.
- **The MCP `chat_message` tool genuinely routes.** `chat_tools/chat/message.rs::chat_message`
  → `llm_bridge/model_route_policy/resolve.rs::resolve_mcp_chat_model_sync_inner` →
  `models::decide`. This path is real, exercised, and good.
- **The GUI chat backend reaches neither.**

**Vox has two chat implementations. The good one is the MCP tool that the GUI never calls.**

Confirmed by grep — `chat_message`, `mcp_infer`, and `resolve_chat_llm_model` have **zero
non-test occurrences** anywhere under `crates/vox-gui/src` or `crates/vox-gui/ui/src`.

---

## 2. Skill activation

### 2.1 Automatic skill detection IS built — on the MCP path, and only there

> **This is the most important correction in the audit, and it inverts the remediation.** My
> first pass concluded "there is no automatic skill detection anywhere." That is **wrong**.
> Vox has a complete, well-engineered, three-tier progressive-disclosure skill system that
> matches the Claude Code baseline closely — and in two respects exceeds it. It lives in
> `crates/vox-orchestrator-mcp/src/chat_tools/`, and the GUI chat window cannot reach any
> part of it.

`chat_tools/skill_catalog.rs` implements the model verbatim, and its own module doc names the
spec it follows:

| Tier | Implementation | Behaviour |
|---|---|---|
| **Tier 1** | `render_skill_catalog(entries, 64)` | name + description (~100 tokens/skill) injected into **every chat turn**, so the model knows which skills exist and when each applies |
| **Tier 2** | `vox_skill_use` MCP tool (`dispatch.rs:1566` → `skills_tools.rs:243`) | full SKILL.md body loaded on demand by tool-calling models |
| **Pinned** | `render_pinned_skill(name, body)` | user-selected skill's full body injected directly — no tool round-trip |

Assembled by `build_system_prompt_with_skill` (`chat_tools/mod.rs:98`), which also composes
`VOX.md`, `MEMORY.md` (with legacy-path fallback), an `## Environment` block, and an optional
`## Model guidance` segment from `ModelPromptRegistry`.

Two design choices here are genuinely better than the baseline:

1. **Cache-prefix stability is explicit and enforced.** The catalog is sorted alphabetically,
   truncated to a fixed cap, and descriptions are capped at `DESC_CAP = 256` chars —
   deliberately *"content-stable across turns (cache-safe)"* so the section never busts the
   DeepSeek/Anthropic prompt-prefix cache. Empty input yields an empty string rather than an
   empty header, "no section, no cache churn." This is the same discipline the Claude Code
   teardowns describe (mechanics doc §3.2) and it is implemented here on purpose.
2. **It degrades to prompt-only models.** The catalog text ends with *"If tools are
   unavailable, state which skill applies and proceed by its description"*, and the pinned
   path injects the full body directly *"so even the prompt-only MENS path honors it."*
   Claude Code assumes tool-calling; Vox does not. That is a real advantage for local models.

There is also a **per-skill MCP tool allowlist** keyed on the active skill
(`server_state.rs:100`, `skill_permissions.rs:12`) — the tool-restriction benefit that the
Claude Code subagent docs list and that most harnesses omit.

**Two limits worth recording now:**

- `render_skill_catalog(&skill_entries, 64)` — a **hard cap of 64 skills**. Beyond that,
  skills are silently dropped in alphabetical order. At ~100 tokens each this is a ~6.4K
  token budget; the cap is defensible, but silent alphabetical truncation is not. A
  120-skill install loses everything after roughly "m" with no diagnostic.
- Descriptions are truncated at 256 chars against the baseline's 1,024-char maximum
  (mechanics doc §4.3). Skills authored to the documented limit lose three-quarters of their
  trigger text — precisely the part that says *when* to use them, which by §4.4 is the half
  that drives activation.

### 2.2 The GUI chat window reaches none of it

`build_system_prompt` has **zero references** anywhere under `crates/vox-gui`. Combined with
§1.2 — `chat_message`, `mcp_infer`, and `resolve_chat_llm_model` also have zero non-test
references in the GUI — the conclusion is unambiguous:

**Vox's skill system, model routing, and system-prompt assembly are all complete, correct,
and wired to a chat implementation the chat window does not call.**

This makes the remediation dramatically cheaper than a "build automatic skill detection"
project. The work is to route the GUI chat window through the MCP chat lane that already
does all of this — see the parity plan, Phase 1.

### 2.3 The manual pin path

The manual path is correctly plumbed end-to-end:

```
Loquela.tsx:444          active_skill: activeSkill?.id
  → control_plane.rs:89  "active_skill": input.active_skill.filter(|s| !s.trim().is_empty())
  → types.rs:166         pub active_skill: Option<String>   (on the task)
  → monitor.rs:99-109    resolves skill content, passes to ContinuationEngine
  → continuation.rs:151  format!("\n\n<active_skill>\n{}\n</active_skill>", …)
```

The chat path does not use it. `crates/vox-gui/src/commands/chat.rs:195`:

```rust
"active_skill": null,
```

Hardcoded. Every task the chat window auto-submits runs skill-less, by construction.

### 2.4 Skill instructions are injected on *continuation nudges only* — never on first dispatch

This is a subtler and more damaging finding than the hardcoded `null`, and it took the graph
plus hand verification to pin down.

`ContinuationEngine::generate_continuation` (`continuation.rs:116`) is the **only** function
that emits `<active_skill>…</active_skill>`. Its only non-test caller is `monitor.rs:103`,
inside the **idle-agent nudge** path — the thing that pokes an agent that has stalled.

Searching the initial dispatch path (`crates/vox-orchestrator/src/orchestrator/task_dispatch/`)
for `active_skill`, `skill_instructions`, or `<active_skill>` returns **nothing**. Everywhere
else `active_skill` appears on a task it is only *reported* — activity events
(`activity/mod.rs:55`, `activity/sink.rs:57`), accessors (`accessors.rs:43`), lifecycle
(`lifecycle_ops.rs:458`), scaling (`scaling.rs:354`).

**So: pin a skill in Loquela, dispatch a task, and the agent starts with no skill
instructions. If it later stalls and gets nudged, the skill content arrives.** The skill
system works backwards — it activates on failure rather than on start.

> **Correction recorded.** My first pass claimed "`active_skill` IS plumbed end-to-end." The
> graph showed in-degree 0 on `generate_continuation_prompt` and `skill_injection_xml`, which
> I initially read as "the injection is dead code." Hand verification showed both of those
> are **test functions** (`continuation.rs:226`, `:249`) — Graphify's `fn` kind does not
> distinguish `#[cfg(test)]`. The real function `generate_continuation` *is* live. The
> corrected finding is the one above: live, but on the wrong edge of the lifecycle.

### 2.5 `detect.rs` is a name trap

`crates/vox-skill-runtime/src/detect.rs` is not skill detection. It probes
`wasmtime --version` / `docker --version` / `podman --version` to pick a **sandbox runtime**
(`RuntimePreference::{Wasm, Auto, Docker, Podman}`). Anyone grepping for skill detection
finds it and concludes the feature exists. Rename recommended.

---

## 3. Skill database organic growth

### 3.1 The miner has one caller and it is a human typing a command

`vox-skill-discovery` is a real subsystem — 43 nodes across `candidate.rs`, `code_miner.rs`,
`op_miner.rs`, `catalog.rs`, `report.rs`. Its callers across the entire 135-crate workspace:

```
crates/vox-cli/src/commands/extras/ars/skill_suggest.rs
crates/vox-orchestrator-mcp/src/feedback_tools.rs:288   (imports Candidate for a type only)
```

One real caller, and it is a manual CLI subcommand. No daemon schedule, no session hook, no
GUI surface, no post-task trigger.

### 3.2 Mined candidates have nowhere to land

The DB schema (`crates/vox-db/src/schema/domains/agents.rs`) defines:

- `skill_manifests` (:125)
- `skill_executions` (:138)
- `skill_reliability` (:378)

There is **no `skill_candidates` table.** So even when the miner runs, its output is printed
and discarded. Candidates cannot accumulate across runs, cannot be ranked, cannot be
reviewed, cannot be promoted. **"Organic growth" has no substrate.**

This also confirms the standing note in project memory that the SP-4 accept→author→install
path is "dead e2e until the SP-5 miner producer" — the producer is still absent, and the
missing table is why.

### 3.3 The reliability signal exists and feeds nothing

`skill_reliability` is populated but no caller ranks or filters skills by it. Against the
baseline, this is the loop Anthropic explicitly *doesn't* have (mechanics doc §4.6: *"There
is not currently a built-in way to run these evaluations"*). Vox is one join away from a
genuine differentiator and isn't making it.

### 3.4 `vox-similarity` is 65% isolated

23 nodes, 15 with degree zero. Its single cross-subsystem edge is the one from
`skill-mining`. An embedding library sitting unused next to a skill-matching problem.

---

## 4. Model routing, multi-provider, local-vs-cloud

### 4.1 The selector is genuinely good — better than Claude Code's

`crates/vox-orchestrator/src/models/select.rs::decide` (:87) implements, verified by reading:

- **Multi-axis user input** — `SelectionAxes` with cost / responsiveness / intelligence, each
  0–100, instead of a binary economy/performance toggle.
- **`CandidateScope`** — `{AllProviders, LocalOnly, CloudOnly}`.
- **Capability filtering** — `required_capabilities` checked per candidate.
- **Key-presence gating** — `ModelRegistry::key_is_present_for(&m)`; a provider without a key
  is rejected with a reason rather than failing at call time.
- **Confidence gating** — `is_routing_eligible(conf)` against `ModelConfidence`, with a
  controlled `exploration_enabled()` fallback and an exploration budget that excludes
  `PricingSource::Unknown` when exhausted.
- **Full decision transparency** — `ModelSelectionDecision { selected_model, provider_route,
  score_breakdown, alternatives, rejection_reasons, pricing_confidence, discovery_state }`,
  with `rejection_reasons` populated per rejected candidate.

This is a more expressive selector than anything in the Claude Code baseline. **The problem
is entirely one of wiring, not design.**

### 4.2 It has three callers, and chat is not one

```
crates/vox-gui/src/commands/models.rs:199                      (hardcoded CodeGen intent)
crates/vox-orchestrator-mcp/src/http_gateway/dashboard_api.rs:356
crates/vox-orchestrator-mcp/src/llm_bridge/model_route_policy/resolve.rs
```

The first is a **dashboard read** — `get_routing_summary`, which constructs
`SelectionIntent::for_task(TaskCategory::CodeGen)` unconditionally, purely so the UI can
display a routing summary. It does not route anything. The second is the same, over HTTP.
Only the third actually routes, and only for MCP-originated calls.

Backward reachability confirms the shape: 127 ancestors can reach `decide`, and they are
overwhelmingly MCP tool handlers (`mcp_infer_completion`, `chat_message`, `ghost_text`,
`inline_edit`, `plan_goal`, `generate_vox_code`, `speech_to_code`, …) plus UI components that
*render* the summary. `chat_append_message` is not among them.

### 4.3 The 2026-07-16 deletion left chat unwired

`model_resolution.rs:5-7` records it:

> "The former 7-way provider-route resolver (`resolve_chat_provider_route`) was deleted
> 2026-07-16 (Axis GUI remediation F3); the single exercised selection path is
> `vox_orchestrator::models::decide()` + the reactive fallback chain."

The replacement was wired into the MCP lane and never into the GUI chat lane. **This is the
concrete regression behind "the harness doesn't chat properly."**

### 4.4 `prefer_local` is not user-configurable

`SelectionIntent::prefer_local` exists and is honoured (`select.rs:634`,
`to_routing_priority` pushes `mobile: 70` when set). It is assigned `true` in exactly **one
place** in the entire workspace: a hardcoded intent constructor at `select.rs:500`.

- No config key.
- No environment variable.
- No GUI toggle.
- No settings entry.

The user cannot express "prefer local when possible."

### 4.5 The `local_only` name collision is a live trap

`vox-config` *does* have a `local_only` setting — `config_registry.rs:593`,
`VOX_MESH_EXEC_POLICY`, surfaced in the GUI as "Mesh Exec Policy"
(`generated_fields.rs:46`, `generatedSettingsIndex.ts:42`).

It governs **mesh task placement** — whether a task may be relayed to another node
(`task_submit.rs:589`, `:1044`). It has **nothing to do with LLM inference.** A
privacy-motivated user who sets "Mesh Exec Policy: local_only" reasonably believes they have
turned off cloud inference. They have not. Cloud model calls continue unchanged.

This is the single most user-hostile finding in the audit: a setting that reads as a privacy
control and isn't one.

### 4.6 Provider inventory

`chat_route_to_llm_config` (`model_resolution.rs:57`) maps to five lanes:

| Route | `provider` string | Notes |
|---|---|---|
| `ManualOpenAiCompatible` | `openai_compatible` | user-supplied base URL + bearer |
| `PopuliLocal` / `PopuliMesh` | `ollama` | `{base}/v1/chat/completions` |
| `HuggingFaceRouter` | `hf_router` | |
| `HuggingFaceDedicated` | `hf_endpoint` | |
| `OpenRouter` | `openrouter` | |

Observations:

- **OpenRouter is the de-facto multi-provider strategy.** There is no first-class direct
  Anthropic, OpenAI, or Google lane.
- **Gemini is detected by string-sniffing a URL.** `route_backend_for_chat_route` classifies a
  manual base URL as `GeminiDirect` iff it contains the Google Generative Language API host
  (`model_resolution.rs:198-210`, and the test at :199 pins exactly that). Provider identity
  inferred from a substring is fragile.
- Free-tier degradation is real and good: `cascade.rs` always appends
  `OPENROUTER_FREE_FALLBACK_MODELS` as a zero-cost floor "so research degrades to free
  instead of failing."

---

## 4A. Live empirical tests (run 2026-07-30, reproducible)

Static analysis says the routing engine is well-designed. **Running it says otherwise.** These
are reproducible commands against the real catalog on a machine with Ollama up.

### 4A.1 ⚠ CORRECTED (2026-07-31): local models DO reach the catalog — the original finding was wrong

> **This finding was factually incorrect and is superseded by direct re-verification during Phase 2
> planning.** The original test below concluded "discovery never enumerated" local models from a
> `vox model list | grep` returning zero hits — but never checked the `--limit` default before
> drawing that conclusion. The actual cause is `vox model list`'s default `--limit 100` combined
> with an alphabetical sort over 381 total models: `qwen3:8b`/`qwen3-vl:8b`/`vox-mens-v1:latest`
> sort well past position 100. Direct re-run:
> ```bash
> vox model discover --force        # Ollama: 3 models ✅ (reported correctly)
> grep -c ollama ~/.vox/cache/model-catalog.v1.json   # → 13 (5 distinct Ollama entries)
> vox model list --limit 400 | grep -i "qwen3:\|vox-mens"   # → all 3 live models present
> ```
> **Discovery, registration, and catalog storage for Ollama all work correctly today.** The real,
> remaining gap (confirmed empirically) is different and narrower: even with local models
> correctly present in the catalog, `vox model explain "hi" --category codegen --complexity 1`
> (the single case where a $0 local model should most plausibly win) still ranks zero Ollama
> models in its top-5 — nothing in the scorer's signals currently favors a local model even in
> the cheapest case. That is the real target for Phase 2, not catalog population. See the
> corrected findings F9a′/F9b in §10 and Phase 2 tasks in the parity plan.
>
> Original (incorrect) text preserved below for the record, struck through in spirit:

```bash
vox model list | wc -l           # 100 shown, 364 merged into the registry
vox model list | grep -icE "ollama|local|qwen3:|vox-mens"   # → 0
curl -s http://127.0.0.1:11434/api/tags   # → 3 models: qwen3-vl:8b, qwen3:8b, vox-mens-v1
```

~~Ollama is running with three local models, including Vox's own `vox-mens-v1`. None of them
appear in the model catalog.~~ **INCORRECT — see correction above.** `vox model explain` reports
`Route policy profile: balanced (net=true, provider_net=true, local_http=true)` — `local_http`
is *enabled*, and this part was accurate: nothing in the scorer's signals favors a local model,
though not because discovery never enumerated
them.

Every one of the 364 catalog entries is an OpenRouter slug (`anthropic/claude-opus-4.8`,
`meta-llama/llama-3.1-8b-instruct`, …). **The catalog is structurally single-source.** A user
who has downloaded a model cannot route to it, and there is no `vox model add` verb.

This is the empirical form of finding **F9**: not merely "`prefer_local` has no control," but
"**there is nothing local to prefer.**"

### 4A.2 The scorer is inert with respect to its inputs — CRITICAL

`vox model explain` accepts `--category` and `--complexity` (default 5). Four probes:

| Probe | Flags | Top-5 candidates | Selection |
|---|---|---|---|
| `"hi"` | defaults | ling-2.6-flash, llama-3.1-8b, llama-3.2-3b, l3-lunaris-8b, aion-rp-llama-3.1-8b | `inclusionai/ling-2.6-flash` |
| `"refactor this rust function to remove the unwrap"` | defaults | *identical* | *identical* |
| `"design and implement a lock-free concurrent hashmap in Rust with hazard pointers"` | defaults | *identical* | *identical* |
| same | `--category codegen --complexity 9` | *identical* | *identical* |

**The ranking is byte-identical across a two-character greeting and a hard concurrent
data-structure design task, and identical again when task category and maximum complexity are
supplied explicitly.** `SelectionAxes`, `TaskCategory`, and complexity have no observable
effect on the outcome.

Two corroborating symptoms in the same output:

- **Every candidate reports `Tier: Unknown`.** The tier metadata the scorer presumably weighs
  is unpopulated across all 364 models. A scorer whose primary discriminator is null for every
  candidate degenerates to a constant ordering — which is exactly what is observed.
- **`aion-labs/aion-rp-llama-3.1-8b` is a top-5 candidate for Rust concurrency work.** `rp` is
  roleplay. Capability filtering is not discriminating on task fit.
- The trailer prints `Recent Trace ID: trace-fail-1`, which reads as a placeholder rather than
  a real trace.

> **Severity.** This supersedes the static reading in §4.1. `models::decide` is *well-designed
> and inert*. Wiring the GUI chat into it (F3) would route chat through a selector that
> currently returns the same small free model for everything. **The parity plan must fix the
> scorer before, or with, the wiring — otherwise Phase 1 lands a regression that looks like a
> fix.**

> **Scope honesty.** These probes exercise the `vox model explain` CLI path, which calls
> `ModelRegistry` scoring. I did not isolate whether the inertness is in `best_for_with_filter`,
> in the unpopulated `Tier`/`pricing_source` metadata, or in `explain.rs`'s own ranking display.
> The observable behaviour is established; the precise fault location is **open** and is the
> first task in the plan's Phase 0.

### 4A.3 There is no `vox chat`

The CLI has no chat verb (`vox chat` → *"unrecognized subcommand"*). The only two chat entry
points in the product are the GUI window (§0: no loop) and the MCP server (§1.2: works). There
is **no third path**, and no way to exercise the working one without an MCP client — which is
why the defect survived: the lane that works is the lane a human never touches directly.

### 4A.4 There is no task classifier anywhere

`vox model explain` requires the caller to supply `--category` and `--complexity`; it does not
infer them. Grepping confirms no prompt-classification step exists on any dispatch path.
Against the routing baseline (routing doc §2.4), OpenRouter's Auto Beta classifies each prompt
into ~30 task types *before* ranking. **Vox has the ranking stage and no classification stage**,
so `TaskCategory` is only ever whatever a hardcoded call site passed — e.g. the unconditional
`TaskCategory::CodeGen` at `models.rs:199`.

---

## 4B. The root cause, found on re-audit (2026-07-31) — supersedes §0 in severity

> **This section was added after a second, deeper code pass and it changes the diagnosis.**
> §0 established that the *GUI* chat window has no agent loop. This section establishes
> something strictly worse: **no code path anywhere in Vox ever passes tools to a model, and no
> chat path ever passes conversation history.** The MCP chat lane — which §1.2 credited as "the
> good one" — is a **stateless, tool-less, single-shot completion**. That credit was too
> generous and is corrected here.

### 4B.1 No tool is ever offered to any model — F24, CRITICAL

The tool-calling plumbing is **complete, from the wire format up**:

| Layer | Evidence |
|---|---|
| Wire format | `vox-llm-egress/src/wire.rs:39` — `tools: Option<Vec<OpenAiTool>>` |
| Egress API | `vox-llm-egress/src/lib.rs:77` — `pub tools: Option<&'a [ToolDef]>` |
| Config type | `vox-actor-runtime/src/llm/types.rs:55` — `pub tools: Option<Vec<LlmToolDef>>` |
| Streaming | `vox-actor-runtime/src/llm/stream.rs:39` — reads `config.tools` and maps to wire |
| Provider adapters | `llm_bridge/provider_adapter.rs:34,352,396`; `providers/openai.rs:22` |
| Tools-capable entry | `llm_bridge/infer.rs:287` — `mcp_infer_tool_completion(…, tools, tool_choice, …)` |

**And nothing populates it.** Verified by reading the exact lines:

- `mcp_infer_tool_completion` (the only tools-capable inference function) has **exactly one
  caller in the workspace**: `mcp_infer_completion` at `infer.rs:269`.
- That caller passes **`None, None`** for `tools` and `tool_choice` — hardcoded, `infer.rs:280-281`.
- Every `LlmConfig` constructed in `model_resolution.rs` sets **`tools: None`** (4 of 4 route
  variants).
- Workspace-wide grep for any assignment of `Some(...)` to an `LlmConfig.tools` field returns
  **zero non-test hits**.

**Consequence, stated plainly: the model in a Vox chat cannot read a file, run a test, search
the codebase, or call any of Vox's ~91 registered commands.** It can only emit text. Vox is
architecturally a **tool *provider*** — its MCP server exposes tools to *external* harnesses
like Claude Code and Cursor — while its *own* chat is a bare completion endpoint. **The harness
Vox users experience when they use Vox's MCP server is Claude Code's harness, not Vox's.**

Against the mechanics baseline (mechanics doc §1.1): *"agents are typically just LLMs using
tools based on environmental feedback in a loop."* Vox's chat has no tools, therefore no
environmental feedback, therefore no loop. It is not a degraded agent; it is not an agent.

### 4B.2 Conversation history is persisted, returned to the caller, and never sent to the model — F25, CRITICAL

`chat_tools/chat/message.rs` builds its prompt from `context_parts` (lines 80–297), which
accumulates:

`[ACTIVE FILE]` · `[SELECTED TEXT]` · `[OPEN FILES]` · `[AUTONOMOUS RESEARCH — REPOSITORY]` ·
`[MENTIONED KNOWLEDGE BASES]` · …

**Conversation history is not among them.** The final prompt is:

```rust
let user_prompt = if context_parts.is_empty() {
    expanded_prompt.clone()
} else {
    format!("{}\n\n{}", context_parts.join("\n"), expanded_prompt)
};
```

History *is* maintained — `context_history_or_hydrate` loads it, the turn is appended, and it is
persisted under `chat_history:{session_id}` and returned in the tool's JSON response
(`message.rs:543-553, 822`). It is used for **display and persistence only**. It is never
serialized into the model request.

Compounding this, the only history-bounding logic is naive FIFO truncation:

```rust
// Keep last 100 messages per session to bound memory usage.
if history.len() > 100 {
    let trim_to = history.len() - 100;
    history.drain(0..trim_to);
}
```

No token counting, no summarization, no preservation policy — and irrelevant in practice, since
none of those 100 messages reach the model anyway.

**Consequence: every Vox chat turn is a fresh single-shot completion.** The model has no memory
of the previous turn. This is the complete, sufficient explanation for "the chat doesn't
actually work" — and it is invisible in the GUI, because the *transcript* shows history
correctly while the *model* never sees it.

### 4B.3 The multi-turn API exists and the chat path doesn't use it — F26

`vox-actor-runtime::llm::llm_chat(opts, Vec<LlmChatMessage>, config)` takes a proper message
array with `role`/`content` (`llm/types.rs:21-26`). It is used by:

- `vox-cli/src/commands/model/eval.rs:320` (benchmark harness)
- `vox-effort-audit/src/judge/prompt.rs:48-52`
- `vox-effort-route/src/route/prompt.rs:66-70`

— all of which pass a fixed system+user pair for a batch task. **No conversational caller
exists.** The chat path instead uses `mcp_infer_completion(system_prompt, user_prompt)`, a
single-string interface that structurally cannot carry a conversation.

The fix is therefore not "add history support" — the API is already there. It is "route chat
through the API that already exists," the same shape as F1/F3's fix.

### 4B.4 No global tool-result budget — F27

Against the mechanics baseline's hard 25,000-token default cap on tool responses (mechanics doc
§5.1), Vox has **no global cap**. Only scattered, ad-hoc, per-tool character limits:

- `chat_tools/chat/mentions.rs:62` — `safe_truncate_for_prompt(content, 8000)` bytes for `@file`
  mentions
- `browser_tools.rs:83` — `summary_max_chars()` for page text

Both are **byte/char limits, not token limits**, and neither is signposted to the model as a
truncation (the mentions one does append `...[truncated]...`, which is correct practice —
browser summaries say "truncated" in the prompt text). There is no equivalent of
`MAX_MCP_OUTPUT_TOKENS`, and no mechanism preventing a single tool result from consuming the
whole window — a moot point today given F24, but a blocker the moment tools are connected.

### 4B.5 Streaming exists and chat doesn't use it — F28

`vox-actor-runtime/src/llm/stream.rs` provides `llm_stream` and `llm_stream_activity`, and
`stream.rs:39` even wires `config.tools` into the streaming wire format — the most
agent-ready code path in the tree. Its only consumers are in `vox-gamify`
(`ai/client/transport.rs`), which uses it for its own purposes. **The chat path is
non-streaming**, which means every turn blocks to completion before the user sees a token —
directly contradicting the UX research's progress-indication requirement (UX doc §4: a bare
wait past 10s stops carrying information).

---

## 5. Chat quality and noise

### 5.1 The secretary classifier is the biggest single UX defect

`crates/vox-orchestrator/src/secretary.rs:21` — the function that decides whether your
chat message silently becomes an orchestrator task:

```rust
pub fn classify(role: &str, content: &str) -> Option<ClassifyResult> {
    if role != "user" { return None; }
    let words: Vec<&str> = content.split_whitespace().collect();
    if words.len() < 10 { return None; }
    let lower = content.to_lowercase();
    let matched_verb = ACTION_VERBS.iter().find(|&&v| lower.contains(v))?;
    …
    let verb_pos = lower.find(matched_verb).unwrap_or(usize::MAX);
    let confidence_pct = if verb_pos < 20 { 85 } else { 60 };
```

`ACTION_VERBS` = `fix, add, update, create, remove, delete, refactor, write, implement,
build, migrate, extract, rename, …`

Three compounding defects:

1. **`lower.contains(v)` is substring, not word, matching.** "add" matches *address*,
   *added*, *padding*, *ladder*. "build" matches *building*, *rebuild*, *buildup*. "write"
   matches *rewrite*, *writer*, *written*. "fix" matches *prefix*, *suffix*, *fixture*.
2. **No negation or mood handling.** "don't delete that", "why did you remove it?", "I already
   fixed it", "what does this refactor do?" all fire.
3. **`confidence_pct` is a position heuristic dressed as a probability.** 85 if the verb
   appears in the first 20 characters, else 60. It is reported to the user as a confidence
   percentage in the `SecretaryProposed` toast. It is not a confidence.

The threshold is ≥10 words. **Any discursive message of ten words or more that happens to
contain one of those substrings is silently converted into an orchestrator task.** Ordinary
conversation — asking a question, giving context, disagreeing — trips it constantly. This is
precisely the reported symptom "doesn't chat properly," and it is 30 lines of code.

Against the baseline: Claude Code has no such mechanism. Intent is determined by the model,
in context, with the full conversation available.

### 5.2 Noise inventory — and a correction

> **Correction.** My first pass reported "395 toast call sites, no severity taxonomy." Both
> halves were wrong. 395 counted every *occurrence* of the identifier including prop-drilling
> and type references; **actual invocations are 179**. And the toast system has a
> deliberately-designed taxonomy that is better than most:

```ts
export type ToastCause =
  | 'backend-ok'      // an async Tauri command / mutation succeeded
  | 'backend-error'   // an async Tauri command / mutation failed
  | 'validation'      // user input rejected before any effect
  | 'clipboard'       // copied to clipboard (real OS effect)
  | 'external';       // opened an external app/url
// NOTE: deliberately NO cause for navigation or a routine, already-visible synchronous
// action — those must NOT toast. A toast with no honest cause is a compile error.
```

`cause` is **required** on every toast, `tone` is `'ok' | 'warn' | 'info'`, the container is
`aria-live="polite" role="status"`, and `App.tsx:313` caps concurrent toasts at
`MAX_TOASTS = 3`. This is a well-governed notification system and the audit should say so.

**The real finding is what the distribution shows:**

| Tone | Count | | Cause | Count |
|---|---|---|---|---|
| `warn` | **125** | | `backend-error` | **118** |
| `ok` | 25 | | `backend-ok` | 52 |
| `info` | 7 | | `validation` | 5 |
| | | | `clipboard` | 1 |

**70% of all toasts in the product are backend errors.** The taxonomy is fine; the thing being
categorised is the problem. Either the backend fails constantly, or failures that belong
inline — next to the control that failed — are being thrown to a global corner toast. Both are
plausible given §0 (the daemon is being asked to service a chat path that doesn't route).

Two genuine gaps remain:

- **`MAX_TOASTS = 3` is truncation, not coalescing.** `.slice(-MAX_TOASTS)` silently drops the
  *oldest* toast. With 118 error sites, a burst of five failures shows the user the last three
  and discards the first two — including, potentially, the root-cause error, keeping the
  cascade.
- **No dedupe.** The same error firing from a 2-second poll loop re-toasts every 2 seconds.

| Source | Count / value |
|---|---|
| Toast invocations (non-test) | **179** (125 `warn`, 118 `backend-error`) |
| Concurrent toast cap | 3, by oldest-drop truncation |
| Toast dedupe / coalescing | **none** |
| `setInterval` poll loops (non-test) | **24** |
| `APPROVALS_POLL_MS` | 2,000 |
| `ATTENTION_POLL_MS` | 5,000 |
| `MATRIX_POLL_MS` | 8,000 |
| `RUNS_POLL_MS` | 10,000 |
| `GAMIFY_POLL_MS` | 15,000 |
| `POLICY_BADGE_POLL_MS` | 60,000 |

395 toast sites with no documented severity taxonomy, priority, coalescing, or rate limiting
is a notification system that trains users to ignore it. A 2-second approvals poll running
whenever the surface is mounted is a busy-wait against the daemon.

The mitigation that *does* exist and is good: `EmbeddedSurfaceContext` +
`useIsEmbeddedSurface()` suppress recurring polls inside dashboard thumbnails, so N
mini-renders don't multiply background traffic (`SurfaceMiniRender.tsx:22-29`). The pattern is
correct; it just isn't applied to the foreground case.

### 5.2b Accessibility of the notification surface — mostly right, three real violations

Measured ARIA live-region usage across the GUI:

| Attribute / role | Count |
|---|---|
| `aria-live="polite"` | 30 |
| `role="status"` | 17 |
| `role="alert"` | 11 |
| `role="log"` | 4 |

**The taxonomy from the UX research (§1 of the UX doc) is already largely in place** — including
the correct use of `role="log"` for append-only streams, which most products miss. `SecretaryToast`
correctly uses `role="status"` + `aria-live="polite"` rather than assertive. The audit should
credit this.

**Three concrete violations** of MDN's explicit constraint that *"the `alert` role should only be
used for text content, not interactive elements such as links or buttons"* — `role="alert"`
regions that contain interactive controls:

| File | Line | Control |
|---|---|---|
| `components/layout/VersionMismatchBanner.tsx` | 14 | dismiss `<button>` |
| `components/surfaces/Console/Console.tsx` | 108 | `copyLastBlock` / compose buttons |
| `components/ui/ErrorBoundary.tsx` | 41 | reset `<button>` |

Because `role="alert"` carries **implicit `aria-live="assertive"`**, each of these both interrupts
whatever a screen reader is announcing *and* presents a control the alert pattern says must not be
there. The fix is small: these are **alert dialogs or persistent banners**, not alerts.

Separately, `SecretaryToast` auto-dismisses after **5,000 ms** while carrying a "View task" action.
An actionable control on a 5-second timer is hostile to anyone who reads slowly — and it is
compounded by F2: the toast is announcing a task the user never asked to create.

### 5.3 No permission-mode plumbing on the CLI path

Permission modes exist and match the baseline vocabulary —
`"ask" | "accept_edits" | "accept_all" | "plan"` (`vox-gui/src/commands/mcp.rs:23`), correctly
carried out-of-band via `OrchDaemonClient::with_permission_mode` and explicitly *"NEVER folded
into `args`"* (:26). That design note is exactly right.

But `crates/vox-cli-core/src/daemon_ipc/dispatch.rs` hardcodes `permission_mode: None` at
four call sites (:65, :170, :274, and `dispatch_protocol.rs:50`). The CLI dispatch path cannot
express a permission mode at all.

---

## 6. Chat window surfaces (dockable / draggable)

### 6.1 What exists

The dock is `dockview`, wrapped by
`crates/vox-gui/ui/src/components/dock/DockWorkspaceShell.tsx`, with per-host localStorage
layout persistence (`{prefix}.dockview_layout.v3`) and a debounced save.

`ChatSurface.tsx` registers **12 panels**:

- **5 core** (`:42`): `sessions`, `transcript`, `executionRail`, `flow`, `todos`
- **7 opt-in** (`:100`): `needs-you`, `voxgraph`, `activity`, `repository`, `mercatus`,
  `harness`, `approvals`

### 6.2 The gap: 33 surfaces, 12 dockable

There are **33 surfaces** under `components/surfaces/`. **21 can never be docked into the chat
window** — including several that are exactly what an agent chat should show while working:
`Models`, `Memory`, `Search`, `Runs`, `Coverage`, `Policies`, `Console`, `Discovery`,
`SkillsPlugins`, `Scientia`, `Matrix`, `Publications`, `Mesh`, `Catalog`, `Browser`,
`DocReader`, `CodeRabbit`, `Gamify`, `Loquela`, `Dashboard`, `Tasks`.

The panel list is a hardcoded `as const` tuple in a 1,164-line component, not a registry. A
new surface cannot become dockable without editing `ChatSurface.tsx`.

### 6.3 Dashboard widgets: 5 real, 28 thumbnails

`dashboardWidgetRegistry.tsx:23` lists five purpose-built widgets: `agents`, `cost`, `mesh`,
`approvals`, `coverage`. Everything else falls through to `SurfaceMiniRender`.

`SurfaceMiniRender` is *honest* — and the honesty is deliberate and documented: it mounts the
genuine surface ("never a fabricated value"), suppresses polling, and blocks input. But what
it renders is the full surface at `scale: 0.6`, `pointer-events-none`, `aria-hidden="true"`,
scroll-clipped.

**A 60%-scaled, clipped, non-interactive, screen-reader-hidden copy of a full surface is not
information density.** It is a screenshot. The user's request — "make sure all surfaces that
can be added and dragged around surface useful information" — is precisely a request to
replace thumbnails with **summaries**: the 3–5 numbers or rows that surface actually carries.

`aria-hidden="true"` also means **every non-purpose-built dashboard widget is invisible to
assistive technology**. That is an accessibility defect, not just a density one.

### 6.4 Panel resurrection was a real bug and its guard is unshared

`DockWorkspaceShell.tsx:16-27` documents at length that the shell does *not* track
user-closed panels, that `ChatSurface` maintains its own `closedPanelIds` ref +
`onDidRemovePanel` listener, and that any future host must reimplement the same guard or
"reintroduce the exact bug this whole effort started by fixing: a refresh effect silently
re-adding a panel the user just closed."

A documented footgun that every future consumer must remember is a design defect. The guard
belongs in the shell.

---

## 7. Graph-wide structural health

| Metric | Value |
|---|---|
| Nodes | 29,315 |
| Edges | 30,170 |
| Edge confidence | 30,066 `resolved`, 104 `dangling` |
| Node kinds | 24,459 `fn`, 4,764 `struct`, 91 `command`, 1 `tool` |
| Communities (Leiden) | 12,164 |
| Degree-0 nodes | **11,308 (38.6%)** |

Per-area isolation:

| Area | Nodes | Degree-0 | % |
|---|---|---|---|
| `crates/vox-similarity` | 23 | 15 | **65.2%** |
| `crates/vox-skills` | 57 | 35 | **61.4%** |
| `crates/vox-actor-runtime` | 470 | 238 | **50.6%** |
| `crates/vox-effort-route` | 140 | 57 | 40.7% |
| `crates/vox-gui/src` | 619 | 244 | 39.4% |
| `crates/vox-orchestrator` | 4,346 | 1,707 | 39.3% |
| `crates/vox-skill*` | 63 | 21 | 33.3% |
| `crates/vox-gui/ui/src` | 738 | 125 | 16.9% |

> **Do not read 38.6% as "38.6% dead code."** Graphify's extractor resolves direct call edges
> within a language. It does not model trait dispatch, closures passed as arguments, macro
> expansion, `serde` derive machinery, JSX render-prop callbacks, or framework entry points.
> Spot-checking the "no inbound edge" set in `vox-gui/ui` returns `App`, `render`,
> `componentDidCatch`, `getDerivedStateFromError`, `handleDragEnd` — all legitimately
> framework-invoked.
>
> The metric is a **relative** signal. `vox-similarity` at 65% and `vox-skills` at 61%,
> against `vox-gui/ui` at 17%, is the real content: **the two subsystems most relevant to
> automatic skill activation are the two least connected in the workspace.**

**A finding that refutes a plausible worry:** all 91 Tauri `command` nodes have at least one
inbound edge. There are **zero orphan commands**. The GUI's command surface is genuinely
wired; the defect is what the wired commands *do*, not that they dangle.

---

## 8. Present-but-unwired assets

Vox already owns most of what the parity plan needs. The work is connection, not
construction.

| Asset | Location | State |
|---|---|---|
| `models::decide` — multi-axis, multi-provider, local/cloud-scoped selector | `vox-orchestrator/src/models/select.rs:87` | Built; chat doesn't call it |
| `HookRegistry` — register/deregister/fire/count | `vox-skills/src/hooks.rs:38` | Built; no lifecycle events defined |
| ~~`context_budget_manager`~~ | ~~`vox-orchestrator/src/agentos/`~~ | **FALSE POSITIVE — see §9 correction 6.** It is 48 lines that truncate JSON arrays; it has nothing to do with context windows. |
| `ContextBudget` (retrieval chunk caps) | `vox-actor-runtime/src/retrieval.rs:15-30` | Real but narrow: caps RAG chunks at `max_chunks: 8` / `max_chars: 8_000`, with a `truncated` provenance flag. Good pattern, wrong scope — not conversation compaction. |
| `llm_chat` multi-turn API + `llm_stream` | `vox-actor-runtime/src/llm/` | Built, tools-capable, **unused by chat** (F26, F28) |
| `vox-similarity` (embeddings) | `crates/vox-similarity` | 65% isolated |
| `vox-effort-route` (bucket/cluster/embed/pricing/`route::decide`) | `crates/vox-effort-route` | A whole second routing engine, 41% isolated |
| `skill_reliability` table | `vox-db/schema/domains/agents.rs:378` | Populated; nothing reads it for selection |
| Skill miners (`code_miner`, `op_miner`) | `vox-skill-discovery` | One manual CLI caller |
| Permission modes | `vox-gui/src/commands/mcp.rs:23` | Correct on the GUI path; `None` on the CLI path |
| Graphify code graph | `.vox/cache/graphify/` | 29K nodes; no `query`/`path`/`explain` CLI verb |
| MCP chat lane (routed, model-selected) | `vox-orchestrator-mcp/src/chat_tools/` | Works; GUI never calls it |

### 8.1 Graphify has no query verb

`vox graph --help` lists `status`, `ingest`, `rebuild`, `coverage`, `index`, `refresh`, `gc`,
`crate-map`, `why-rebuilt`. There is **no `query`, `path`, `explain`, or `neighbors`**. The
analysis in this document was performed by reading `graph.json` directly with an ad-hoc
script.

Against the baseline (mechanics doc §2.3), the right move is not to pre-load the graph but to
**expose it as agent tools** — relational queries grep cannot answer. The audit itself is the
evidence: §1.1's cross-subsystem edge census and §1.2's path analysis are findings a
grep-only audit does not produce.

### 8.2 `vox graph coverage` needs calibration before it is trusted

Running `vox graph coverage --corpus vox-gui-surface --kind surface` classifies **every**
surface as `orphan_backend`. `--kind tool` returns exactly two nodes
(`vox_populi::inference_PROFILE` = orphan, `vox_mesh_nodes` = dead_end) against a workspace
with dozens of MCP tools.

Either the corpus lens is mis-scoped or the classifier's reachability roots are wrong. **The
coverage verb currently reports a uniform verdict, which is the signature of a broken
classifier rather than a uniformly broken codebase.** It should not be used as a gate until
calibrated.

---

## 9. Corrections ledger

Recorded rather than silently dropped, per the standing rule that audit claims get
hand-verified.

| # | First-pass claim | Correction | How caught |
|---|---|---|---|
| 1 | "`active_skill` is plumbed end-to-end and works when pinned." | Injection happens **only** on idle-continuation nudges, never on initial dispatch. | Graph in-degree, then grep of `task_dispatch/` |
| 2 | "`generate_continuation_prompt` and `skill_injection_xml` are dead code (in-degree 0)." | **Wrong.** Both are `#[cfg(test)]` functions at `continuation.rs:226/249`; the live function is `generate_continuation`, called from `monitor.rs:103`. Graphify's `fn` kind does not distinguish test functions. | Hand read of `continuation.rs` |
| 3 | "`models::decide` is not reachable from chat." | **Partially wrong.** It is unreachable from the *GUI* chat backend, but fully reachable from the *MCP* `chat_message` tool. Vox has two chat paths. | Backward BFS (127 ancestors) |
| 4 | "24% of GUI UI nodes are dead." | **Wrong.** The no-inbound set is dominated by framework entry points and JSX callbacks. Metric is only valid comparatively. | Spot-check of the isolated set |
| 5 | "There is no automatic skill detection anywhere in Vox." | **Wrong, and the most consequential error in the first pass.** A complete three-tier progressive-disclosure system exists in `chat_tools/skill_catalog.rs` + `build_system_prompt_with_skill` + the `vox_skill_use` tool, with cache-stable rendering and prompt-only-model degradation. It is unreachable from the GUI. This changes the remediation from "build a skill system" to "route chat through the lane that has one." | Following the `skill_catalog` reference in `build_system_prompt`'s doc comment |

| 6 | "`context_budget_manager.rs` / `checkpoint_engine.rs` are built compaction primitives, not on the chat path." | **False positive — my error.** `context_budget_manager.rs` is **48 lines** that truncate JSON `evidence["items"]` arrays. It has nothing to do with context windows, token budgets, or compaction. I inferred capability from a filename without reading it. **Vox has zero conversation-compaction code.** | Reading the file during the 2026-07-31 deep pass |
| 7 | "The MCP chat lane is the good one — it genuinely routes and works." | **Too generous.** It routes the *model selection* correctly, but it is stateless (F25) and tool-less (F24). It is a correctly-routed single-shot completion, not a working agent lane. | §4B |

**Methodological note.** Corrections 2, 3, and 5 share a cause: **grepping for a feature's
name finds where it is absent, not where it is present under a different name.** Correction 6
is the inverse and more embarrassing failure: **a plausible filename was taken as evidence of
the capability it names.** `context_budget_manager` sounds exactly like a context-window
manager. It is not one. The lesson applied for the rest of this audit: **every "asset exists"
claim must cite a line count and a read function signature, not a path.** The skill
system was missed because it is not called "detection" anywhere — it is called "disclosure."
The graph found the *shape* of the problem (disconnected subsystems); reading the code found
what was actually in them. Neither alone was sufficient, and the graph-only reading was wrong
twice.

---

## 10. Findings index, ranked by severity

| # | Finding | Evidence | Severity |
|---|---|---|---|
| **F24** | **No tool is ever passed to any model on any path** — full plumbing exists, `tools: None` hardcoded at the single call site; Vox's chat cannot read a file or run a test | §4B.1 | **Critical (root cause)** |
| **F25** | **Conversation history never reaches the model** — persisted and returned for display only; every turn is a fresh single-shot completion | §4B.2 | **Critical (root cause)** |
| **F26** | Multi-turn `llm_chat(Vec<LlmChatMessage>)` API exists; no conversational caller | §4B.3 | **High** |
| **F27** | No global tool-result token budget (vs Claude Code's 25k default); only ad-hoc byte caps | §4B.4 | **High** (blocker once F24 fixed) |
| **F28** | `llm_stream` exists and is tools-aware; chat path is non-streaming | §4B.5 | **High** |
| **F1** | Chat window has no agent loop — 7 reachable nodes, depth 1, no model call | §0, BFS | **Critical** |
| **F2** | Secretary converts ordinary conversation into tasks via substring matching on 15 verbs | §5.1, `secretary.rs:21` | **Critical** |
| **F3** | `models::decide` unwired from GUI chat; the 2026-07-16 F3 deletion left the lane dead | §4.2–4.3 | **Critical** |
| **F4** | Skill instructions injected only on idle nudges, never on initial dispatch | §2.4 | **High** |
| **F5** | `"active_skill": null` hardcoded in the chat submit payload | `chat.rs:195` | **High** |
| **F6** | Complete 3-tier skill disclosure exists on the MCP path; GUI chat cannot reach it (`build_system_prompt` has 0 refs in `vox-gui`) | §2.1–2.2 | **High** |
| **F6a** | Tier-1 catalog hard-caps at **64 skills** with silent alphabetical truncation | §2.1 | **Medium** |
| **F6b** | Skill descriptions truncated to 256 chars vs the 1,024-char authoring limit — losing the "when to use" half that drives activation | §2.1 | **Medium** |
| **F7** | `local_only` mesh setting reads as a privacy control but doesn't gate inference | §4.5 | **High** |
| **F8** | No `skill_candidates` table — mined candidates cannot persist | §3.2 | **High** |
| **F9** | `prefer_local` has no user-facing control anywhere | §4.4 | **High** |
| **F10** | 28 of 33 dashboard widgets are `aria-hidden` 60%-scale thumbnails — a **WCAG 2.2 SC 4.1.3 (Level AA) conformance failure**, not just low density | §6.3 + UX doc §2.1 | **High** |
| **F23** | 3 `role="alert"` regions carry interactive controls (`VersionMismatchBanner:14`, `Console:108`, `ErrorBoundary:41`), against MDN's explicit prohibition; `role=alert` implies assertive | §5.2b | **Medium** |
| **F11** | 21 of 33 surfaces cannot be docked into chat; panel list is a hardcoded tuple | §6.2 | **Medium** |
| **F3a** | **Scorer is inert** — identical ranking for `"hi"` and a hard codegen task, even with `--category codegen --complexity 9`; all 364 models report `Tier: Unknown` | §4A.2 | **Critical** |
| ~~F9a~~ | ~~Zero local models in the catalog~~ — **CORRECTED, factually wrong**, see §4A.1. Discovery/registration work; the finding was a `--limit 100` + alpha-sort artifact. | §4A.1 | ~~Critical~~ N/A |
| **F9a′** | `vox model explain` never ranks a local model in its top-5, even at `--complexity 1` where a $0 model should most plausibly win — the scorer has no local-preference signal | §4A.1 | **High** |
| **F9b** | No `vox model add <endpoint>` verb for LM Studio/vLLM/manual OpenAI-compatible local servers not auto-discovered | §4A.1 | **Medium** |
| **F12** | Toast taxonomy is good (required `cause`, capped, `aria-live`), but **70% of toasts are `backend-error`**; cap is oldest-drop truncation, no dedupe; 24 polls incl. a 2s loop | §5.2 | **Medium** |
| **F21** | No task classifier on any path — `TaskCategory` is only ever a hardcoded call-site constant | §4A.4 | **High** |
| **F22** | No `vox chat` verb — the only working chat lane (MCP) is unreachable without an MCP client, which is why the GUI defect survived | §4A.3 | **Medium** |
| **F13** | Skill miner has one manual CLI caller; no automatic trigger | §3.1 | **Medium** |
| **F14** | `skill_reliability` populated but never read for selection | §3.3 | **Medium** |
| **F15** | Graphify has no `query`/`path`/`explain` verb | §8.1 | **Medium** |
| **F16** | `vox graph coverage` returns a uniform verdict — classifier needs calibration | §8.2 | **Medium** |
| **F17** | `permission_mode: None` hardcoded at 4 CLI dispatch sites | §5.3 | **Medium** |
| **F18** | Gemini provider identity inferred by URL substring sniffing | §4.6 | **Low** |
| **F19** | Panel-resurrection guard documented as a footgun every host must reimplement | §6.4 | **Low** |
| **F20** | `skill-runtime/detect.rs` name collides with the absent skill-detection feature | §2.3 | **Low** |
