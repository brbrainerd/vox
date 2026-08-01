---
title: "Context Management Mechanics — Verified Research 2026-07-31"
description: "Implementation-level verified research on agent context management: exact clear_tool_uses_20250919 and compact_20260112 parameters, the memory tool's six commands, measured compaction-vs-clearing token results, and prompt-caching prefix mechanics (render order tools→system→messages, breakpoint semantics, per-model minimums) — including the finding that dynamic tool loading invalidates the entire cache."
category: "Architecture SSOTs"
status: "current"
training_eligible: false
---

# Context Management Mechanics — Verified Research (2026-07-31)

> **Provenance.** `deep-research` run `wf_c187369d-cc9`: **102 of 103 agents completed**,
> **23 confirmed / 2 refuted**, 7.0M subagent tokens. The synthesis stage failed on a session
> usage limit, so this document is reconstructed from the raw per-claim verification votes —
> the same recovery procedure documented in
> [`harness-research-gap-fill-2026-07-30.md`](harness-research-gap-fill-2026-07-30.md). Every
> claim below carries its own vote.
>
> **This closes the largest genuine gap in the research set.** The prior mechanics doc
> (§3) established *that* Claude Code compacts and *what* it preserves. This one establishes
> the **exact API parameters, measured results, and caching mechanics** needed to implement it.

Feeds directly into [`vox-harness-implementation-spec-2026-07-31.md`](vox-harness-implementation-spec-2026-07-31.md)
§4. Companions: [`claude-code-harness-mechanics-2026-07-30.md`](claude-code-harness-mechanics-2026-07-30.md),
[`vox-harness-graph-audit-2026-07-30.md`](vox-harness-graph-audit-2026-07-30.md) §4B.

---

## 0. The headline: two distinct mechanisms, and they are not interchangeable

**Clearing removes. Compaction summarizes.** They are separate API features with separate beta
headers, separate parameters, and — critically — **different failure modes** (confirmed 3-0):

| | **Clearing** (`clear_tool_uses_20250919`) | **Compaction** (`compact_20260112`) |
|---|---|---|
| Mechanism | Deletes old tool results in place | LLM summarizes the transcript |
| Cost | **No LLM call** — pure data-structure op | One LLM call |
| Beta header | `context-management-2025-06-27` | `compact-2026-01-12` |
| Granularity | Sub-transcript; surgical | Whole-transcript |
| Loss profile | Exact, predictable (you know what went) | **Lossy in an unpredictable way** — see §2.2 |

**Design consequence for Vox: implement clearing first.** It is cheaper, deterministic,
testable without an LLM, and — per the measured results in §2.2 — recovers most of the benefit.

---

## 1. `clear_tool_uses_20250919` — exact parameters (confirmed 3-0, twice independently)

```json
{
  "type": "clear_tool_uses_20250919",
  "trigger":       { "type": "input_tokens", "value": 30000 },
  "keep":          { "type": "tool_uses",    "value": 3 },
  "clear_at_least":{ "type": "input_tokens", "value": 5000 },
  "exclude_tools": ["web_search", "memory"],
  "clear_tool_inputs": false
}
```

| Parameter | Default | Meaning |
|---|---|---|
| `trigger` | **100,000 input tokens** | Threshold at which clearing fires |
| `keep` | **3 tool uses** | Most-recent tool interactions never cleared |
| `clear_at_least` | — | **Minimum tokens that must be reclaimable or the edit aborts** |
| `exclude_tools` | — | Tool names exempt from clearing |
| `clear_tool_inputs` | **false** | When false, clears only *results*, leaving the original tool *calls* visible |

Clearing removes **oldest tool interactions first**.

### 1.1 `clear_at_least` exists to protect the prompt cache (confirmed 3-0)

This is the subtle one and it drives the design:

> "Ensures minimum tokens cleared each activation… If API can't clear at least this amount,
> strategy won't apply. **Helps determine if clearing is worth breaking prompt cache**"

Clearing invalidates the cached prefix **from the clearing point onward** and incurs a
cache-write cost. `clear_at_least` amortizes that: don't break the cache to reclaim 200 tokens.

**Vox implication:** any Vox-side clearing implementation needs the same guard. Reclaiming less
than the cache-rewrite cost is a net loss.

### 1.2 Server-side, with the client keeping full history (confirmed 2-1)

> "Context editing is applied server-side before the prompt reaches Claude. Your client
> application maintains the full, unmodified conversation history."

**This is exactly Vox's current storage shape** — `chat_history:{session_id}` holds everything,
and the transcript UI shows it all. Vox already has the client half right; it needs the
send-side edit.

### 1.3 The API tells you what it did (confirmed 3-0)

```json
"context_management": {
  "applied_edits": [
    { "type": "clear_tool_uses_20250919", "cleared_tool_uses": 8, "cleared_input_tokens": 50000 }
  ]
}
```

In streaming these arrive on the **final `message_delta`**. `count_tokens` accepts the same
`context_management` block and returns **both `original_input_tokens` and post-edit
`input_tokens`** — which means **Vox can measure the saving before committing to it**, and can
build the whole feature test-first against `count_tokens` without burning inference.

---

## 2. `compact_20260112` — exact shape and measured results

### 2.1 Parameters (confirmed 3-0)

- **`trigger`** — 50K-token **minimum**; the widely-cited default is **150K** (confirmed 2-1).
- **`instructions`** — a custom summarization prompt that **completely replaces** the default,
  **not supplements it**. (Quoted verbatim: *"completely replaces default (not
  supplementary)"*.) A partial override silently discards Anthropic's tuned prompt.
- **`pause_after_compaction`** — optional flag.
- The result returns as a **`compaction` content block** that the caller must **serialize back
  into `messages` as an assistant turn**. Confirmed 2-1: *"Append `response.content` (not just
  the text) back to your messages on every turn. Compaction blocks in the response must be
  preserved"* — otherwise compaction state is lost.

**The default compaction prompt** (confirmed 3-0), quoted:

> "You have written a partial transcript for the initial task above. Please write a summary of
> the transcript. The purpose of this summary is to provide continuity so you can continue to
> make progress towards solving the task in a future context… You must wrap your summary in a
> `<summary></summary>` block."

Covering **state, next steps, and learnings**.

### 2.2 The measured comparison — and the caveat that should govern the design

The single most decision-relevant table in this research set (confirmed 3-0):

| Strategy | Peak input tokens | Final context | Turns | Events |
|---|---|---|---|---|
| **Baseline (unmanaged)** | 335,279 | 335,279 | 5 | 0 |
| **Compaction only** | **169,164** | **5,829** | 7 | 1 compaction |
| **Clearing only** | 173,137 | 173,137 | 7 | 4 clearings |

Both roughly **halve peak** tokens. Compaction additionally collapses *final* context to ~6K —
because it replaces the transcript, where clearing only removes tool results.

**And the caveat, from the same source:**

> Compaction preserved **3/3 high-level facts but 0/3 obscure specifics.**

**This is the number to design around.** Compaction is not lossless compression — it reliably
keeps the shape of the work and reliably loses details. A harness that compacts and then needs
an exact error string, a specific line number, or a precise version constraint **will not have
it**.

Two direct Vox consequences:

1. **Clearing before compaction**, always. Clearing's losses are *known* (you removed tool
   result N); compaction's losses are *unknown until you need the fact*.
2. **Externalize specifics before compacting.** Exact strings that matter must be written to
   a file or a memory entry *before* the summarizer runs, because they will not survive it.
   This is what the memory tool (§3) is for and why Anthropic pairs them.

### 2.3 Tuning method (confirmed 3-0)

> "(1) maximize recall to ensure your compaction prompt captures every relevant piece of
> information from the trace, then (2) iterate to improve precision by eliminating superfluous
> content"

Two phases, in that order: **recall first, precision second.** Preserve architectural
decisions, unresolved bugs, implementation details; discard redundant tool outputs.

And the lowest-effort lever (confirmed 2-1), quoted as a rhetorical question:

> *"why would the agent need to see the raw result again?"*

---

## 3. The memory tool — `memory_20250818` (confirmed 3-0)

- Entire configuration is `{"type": "memory_20250818", "name": "memory"}` — **no input schema**,
  **no beta header** (GA on the Messages API), all Claude 4+ models.
- **Six commands**: `view`, `create`, `str_replace`, `insert`, `delete`, `rename`, with
  parameters `view_range`, `file_text`, `old_str`/`new_str`, `insert_line`/`insert_text`,
  `old_path`/`new_path`.
- **Entirely client-side.** *"Claude requests file operations, and your application executes
  them. You control where and how the data is stored."* `/memories` is **a path prefix mapped
  onto real storage** — a directory, or database keys.
- **Path-traversal guarding is the implementer's responsibility.**
- The API **auto-injects** a memory protocol into the system prompt: *"IMPORTANT: ALWAYS VIEW
  YOUR MEMORY DIRECTORY BEFORE DOING ANYTHING ELSE… ASSUME INTERRUPTION: Your context window
  might be reset at any moment."*

**Vox fit:** Vox already has `MEMORY.md` handling in `build_system_prompt_with_skill` and
`vox-bounded-fs` for path-safe reads — the two hard parts (storage backend, traversal guard)
are already in-tree. And note `exclude_tools: ["memory"]` in §1's example: **memory operations
should be exempt from clearing**, or the agent loses the record of what it saved.

---

## 4. Prompt caching mechanics — and the constraint that governs dynamic loading

> **This section directly answers the "dynamically load/unload skills and MCP servers" question,
> and the answer is a warning.**

### 4.1 Render order and prefix invalidation (confirmed 3-0)

> **Render order: `tools` → `system` → `messages`.** *"Any byte change anywhere in the cached
> prefix invalidates everything after it."*

Cache hits require the prefix to be **byte-identical up to and including the block marked with
`cache_control`** — *"Cache hits require 100% identical prompt segments, including all text and
images up to and including the block marked with cache control."*

**⚠ The consequence for dynamic tool/skill loading, stated plainly: `tools` renders FIRST.**
Changing the tool set mid-conversation invalidates the *entire* cache — tools, system prompt,
and every message. Dynamically loading and unloading MCP servers or skills per-turn is
therefore **the most cache-destructive thing a harness can do.**

This does not mean don't do it. It means: **change the tool set at deliberate boundaries, not
continuously**, and budget for a full cache rewrite when you do. See the implementation spec
§4.7 for the resulting design.

### 4.2 Breakpoint semantics (confirmed 3-0)

> "Cache writes happen only at your breakpoint. Marking a block with `cache_control` writes
> exactly one cache entry: a hash of the prefix ending at that block. **The system does not
> write entries for any earlier position.**"

- On a **miss**, the system walks **backward at most 20 blocks** looking for a previously
  written entry.
- **Maximum 4 breakpoints per request**; exceeding returns a **400**.
- **Placing a breakpoint on a per-request block (e.g. one containing a timestamp) yields no
  cache hit at all.**

That last point **independently confirms the correction made in the implementation spec
§4.5**: varying content must sit *after* the breakpoint, not before it — and the breakpoint must
be on stable content.

### 4.3 Per-model minimum cacheable prefix (confirmed 3-0)

| Model | Minimum |
|---|---|
| Opus 5 / Fable 5 / Mythos 5 | **512 tokens** |
| Sonnet 5, Opus 4.x, Sonnet 4.x | **1,024 tokens** |
| Others | 2,048 / 4,096 |

> *"Shorter prompts cannot be cached, even if marked with `cache_control`. Any requests to cache
> fewer than this number of tokens will be processed without caching, **and no error is
> returned**."*

**Silent failure.** A harness can believe it is caching and not be. Verify with
`response.usage.cache_read_input_tokens` — the only reliable signal.

### 4.4 The agentic-loop property that makes this workable (confirmed 3-0)

> "In agentic tool loops the cache survives when **only `tool_result` blocks are appended**."

Thinking blocks are cached **implicitly** with tool results even though they cannot themselves
carry `cache_control`.

**This is the key enabler for Vox's Phase 1 tool loop:** a multi-iteration tool-use turn is
cache-friendly *by construction*, as long as the tool set doesn't change mid-loop. Combined with
§4.1, the rule becomes: **fix the tool set for the duration of a turn.**

### 4.5 Injecting dynamic instructions without breaking cache (confirmed 3-0)

> "Dynamic operator instructions can be injected mid-conversation **without invalidating the
> prefix by appending a system-role message into `messages`** instead of editing the top-level
> `system` field."

This is the mechanism for anything per-turn: session age, current time, transient policy notes.
Vox's `session_ts` is exactly this class of content.

---

## 5. Sub-agent isolation as a context strategy (confirmed 3-0)

> "Each subagent might explore extensively, using tens of thousands of tokens or more, but
> returns only a condensed, distilled summary of its work (often 1,000–2,000 tokens)."

Framed here as a *context-management* mechanism rather than a parallelism one: it is the only
technique that bounds context growth **without any information loss in the parent**, because the
parent never had the tokens in the first place.

---

## 6. Recommended tier order for Vox

Synthesizing §1–§5 into an implementable ladder, cheapest first:

```
TIER 0 — Don't create the problem
  · Cap the tool set (impl spec §3.2: 330 → ≤48 tools ≈ 30-43k tokens saved/request)
  · Cap tool results at the source (impl spec §4.4)
  ⇒ Largest single win, zero runtime cost.

TIER 1 — Clearing (no LLM call)
  · Drop oldest tool results, keep last N
  · Guard with a clear_at_least equivalent (§1.1)
  · exclude memory + active-skill tools
  ⇒ ~50% peak reduction (§2.2), deterministic, unit-testable.

TIER 2 — Externalize before compacting
  · Write exact specifics (errors, versions, paths) to memory/files
  ⇒ Because compaction loses 0/3 obscure specifics (§2.2).

TIER 3 — Compaction (one LLM call)
  · Recall-then-precision prompt (§2.3), preserve list from mechanics doc §3.1
  · Re-attach N most-recent files
  · Recursion guard — mandatory
  ⇒ Final context to ~6K, at the cost of specifics.

TIER 4 — Sub-agent isolation for genuinely large explorations
```

---

## 7. Refuted / not established

Two claims failed verification in this run. Both are recorded rather than dropped, but the raw
output did not preserve their full text — treat any recollection of specific compaction
*threshold percentages* from third-party teardowns as still unverified, consistent with the
mechanics doc's standing caveat that sources disagree (~83.5% vs ~92–95%).

**Also not established here:** context-rot / lost-in-the-middle measurements. The research
question asked for published degradation numbers as context grows; **no such claim survived into
the confirmed set.** Treat "long context degrades retrieval accuracy" as plausible-but-unverified
in this evidence base, and do not cite a specific curve.
