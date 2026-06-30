---
title: Operation Sequence Mining (skill-suggest sub-project 2)
date: 2026-06-30
status: design
audience: contributors
---

# Operation Sequence Mining

## Context

**Sub-project 2 of 4** in the "agent-authored skills from repeated operations"
program (decomposed during the operation-capture brainstorm):

1. **Operation capture** — record every tool call (redacted) into `agent_operations`. ✅ DONE (on `main`).
2. **Sequence mining (this spec)** — detect repeated operation *sequences* over that stream → ranked `Candidate`s.
3. HITL proposal — surface a candidate as a draft skill via `FeedbackStore` / `Surface::NeedsYou`.
4. Accept → author → install — synthesize a SKILL.md and install via the shipped `install_to_user_root`.

This sub-project turns the captured signal into ranked skill candidates. It does
**not** propose, author, or install anything — that is 3 and 4.

## Problem

`agent_operations` now accumulates `(ts_ms, session_id, agent_id, tool_name,
args_redacted, result_redacted, duration_ms, is_error)` rows, but nothing reads
or analyzes them. The existing `mine_repeated_code` (`vox-skill-discovery`) does
LSH over `.vox` *text blocks* — wrong tool for *operation sequences*. We need a
miner that finds recurring contiguous tool-call procedures.

## Goal

A pure, testable miner that reads recent `agent_operations`, finds contiguous
tool-call sequences (n-grams, length 2..K) that recur ≥N times across ≥S distinct
sessions, and returns ranked `Candidate`s (reusing the existing discovery type).
Exposed via the existing `vox-discover` CLI with `--source operations`. No MCP /
orchestrator wiring (that is sub-project 3).

## Decisions (locked during brainstorming)

- **Repetition unit:** repeated contiguous *sequences* (n-grams of operations),
  not single ops.
- **Op identity:** `tool_name` + the sorted set of top-level arg KEYS (values stay
  redacted/ignored). Distinguishes `read(path)` from `read(path,range)`.
- **Surface:** the existing `vox-discover` binary, new `--source operations` mode.
  MCP exposure deferred to sub-project 3.

## Architecture

```
agent_operations (vox-db)
   │  list_recent_operations(limit)                 ← NEW vox-db read
   ▼
group by session_id, order by ts_ms                 → per-session ordered op lists
   │  op_key = tool_name + "(" + sorted(arg-keys) + ")"
   ▼
extract contiguous n-grams (len 2..K) within each session
count identical n-grams ACROSS sessions (track distinct session ids)
keep n-grams with occurrences ≥ min_occurrences AND distinct_sessions ≥ min_distinct_sessions
   ▼
Vec<Candidate{ kind: RepeatedOperations, score, members, draft_frontmatter }>
```

The miner is a pure function over rows → candidates; the DB read and CLI rendering
are thin shells around it.

## Components

### 1. `vox-db`: read path

`list_recent_operations(&self, limit: i64) -> Result<Vec<OperationRow>, StoreError>`
in `crates/vox-db/src/store/ops_agents.rs`, mirroring the existing turso query
idiom (`breaker` + `conn.query` + `()` params, as in `list_active_sessions`).

```rust
pub struct OperationRow {
    pub ts_ms: i64,
    pub session_id: Option<String>,
    pub agent_id: Option<String>,
    pub tool_name: String,
    pub args_redacted: String,
}
```

Returns the most recent `limit` rows ordered by `ts_ms` (the miner regroups by
session). `result_redacted`/`duration_ms`/`is_error` are not needed for sequence
mining and are omitted to keep the row lean.

### 2. `vox-skill-discovery`: the miner

- Add `CandidateKind::RepeatedOperations` to `candidate.rs`.
- New `crates/vox-skill-discovery/src/op_miner.rs`:

```rust
/// One captured operation (subset the miner needs). Mirrors vox-db OperationRow
/// but defined locally so vox-skill-discovery gains no vox-db dependency — the
/// caller maps rows into this.
pub struct MinedOp {
    pub ts_ms: i64,
    pub session_id: String, // rows with no session are dropped by the caller
    pub tool_name: String,
    pub arg_keys: Vec<String>, // top-level arg keys, already sorted+deduped
}

pub struct OpMiningOptions {
    pub min_len: usize,            // default 2
    pub max_len: usize,            // default 6
    pub min_occurrences: usize,    // default 3
    pub min_distinct_sessions: usize, // default 2
}

pub fn mine_repeated_operations(ops: &[MinedOp], opts: &OpMiningOptions) -> Vec<Candidate>;
```

Algorithm: group `ops` by `session_id`, sort each group by `ts_ms`; build the
per-op key string `tool_name(k1,k2,...)`; slide windows of length `min_len..=max_len`
over each session's key list; accumulate, per distinct n-gram, total occurrences +
the set of session ids it appeared in; emit a `Candidate` for each n-gram meeting
both thresholds. Score = `occurrences * ngram_len` (longer, more frequent ranks
higher). Sort candidates by score descending.

`op_key`/arg-key extraction: `fn arg_keys(args_redacted: &str) -> Vec<String>` —
parse the JSON object, collect top-level keys, sort, dedup; non-object / unparseable
args → empty key list. Lives in `op_miner.rs` and is unit-tested.

### 3. Candidate shape

```text
kind:             RepeatedOperations
members:          ["session:<id>@<first_ts>", ...]   // one anchor per occurrence (capped at e.g. 20)
score:            occurrences * ngram_len
suggested_action: "Save recurring procedure as a skill"
draft_frontmatter:
  name:        kebab of the sequence, e.g. "read-edit-run" (sanitized to the
               Agent Skills name rule: [a-z0-9-], <=64, no leading/trailing/double hyphen)
  description: "Recurring procedure: toolA → toolB → toolC (seen N× across M sessions)"
  category:    "workflow"
  tags:        ["auto-discovered", "operations"]
```

Name sanitization reuses the same rule enforced by `vox-plugin-host`'s
`validate_skill_name` (lowercase alnum + single hyphens); the miner produces a
already-valid draft name so sub-project 4 can install it without rejection.

### 4. CLI surface

Extend `crates/vox-skill-discovery/src/bin/vox_discover.rs` with `--source operations`:
read via `vox-db` `list_recent_operations(limit)` (default limit e.g. 5000), map
rows → `MinedOp` (dropping rows with no `session_id`), call
`mine_repeated_operations`, render with the existing report path
(`render_json` / `render_terminal`). `--limit` flag overrides the row cap.

> The `vox_discover` binary already depends on `vox-mcp-registry`/`vox-plugin-types`;
> add `vox-db` to its binary's deps for the read. The library crate
> `vox-skill-discovery` stays DB-free (pure miner) — only the binary links vox-db.

## Error handling

- Miner is pure + total: empty input, single-op sessions, or all-below-threshold →
  `vec![]`; never panics. Unparseable `args_redacted` → empty arg-key set (the op
  still participates by `tool_name`).
- CLI: a missing/unavailable DB prints "no operations captured yet" and exits 0
  (capture may simply be disabled or fresh).

## Testing

Pure-miner unit tests (`op_miner.rs`), no DB:
- A→B→C recurring 3× across 2 sessions yields exactly one candidate; its
  `draft_frontmatter.name` is a valid skill name; `members` has the anchors.
- A sequence occurring 3× but all in ONE session is excluded (min_distinct_sessions).
- A sequence occurring 2× total is excluded (min_occurrences).
- Arg-key shape: `read{path}` and `read{path,range}` form distinct keys → not merged.
- Non-contiguous / interleaved ops do not form a sequence.
- `arg_keys` parses an object, sorts+dedups; non-object args → empty.
- Overlapping n-grams: A→B→C also yields A→B and B→C — all counted; ranking puts
  the longer/more-frequent first.

`vox-db` test (`--features local`, `VoxDb::connect(DbConfig::Memory)`):
- Insert operations across two sessions; `list_recent_operations` returns them
  ordered, respects `limit`.

CLI smoke (optional, in-binary): `--source operations` against an empty DB exits 0
with the empty-state message.

## Out of scope (sub-projects 3–4)

- HITL proposal surface (`FeedbackStore` / `Surface::NeedsYou`), the `vox_propose_skill`
  MCP tool, and any orchestrator-side auto-trigger.
- Dedup of candidates against already-installed skills.
- Accept → author SKILL.md → `install_to_user_root`.
- Any automatic running of the miner; sub-project 2 is invoked explicitly via CLI.
