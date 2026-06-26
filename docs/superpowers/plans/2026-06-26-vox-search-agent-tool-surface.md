---
category: "Architecture SSOTs"
title: "Vox Search — Agent Tool Surface (Auto-Availability, Steering, GUI) — Implementation Plan"
date: 2026-06-26
status: plan
---

# Vox Search — Agent Tool Surface (P4 + GUI surface) — Implementation Plan

## Goal

Make the Vox Search code-intelligence MCP tools (`vox_search_*`, `vox_discover`)
**available to any AI agent on any harness with zero per-agent setup**, **steer**
those agents to graph-first discovery (call the graph before `grep`/`Glob`), keep
the graph **self-healing** under staleness, and consume the **same single MCP
tool layer** from the Vox Axis GUI under the ratified **Knowledge** IA. One tool
layer (the MCP dispatch), two consumers (agents + GUI). No re-implemented graph
logic in a bespoke Tauri command.

Concretely this plan ships:

1. A generated repo-root **`.mcp.json`** + a `vox ci mcp-client-config` gate that
   keeps it SSOT-derived (Claude Code auto-discovers it → instant tool access).
2. **`vox mcp install <harness>`** — one generator, multiple emitters, writing the
   client entry into each harness's own config. Global writes are **explicit
   opt-in** (`--all`); Vox never mutates a user's global config without consent.
3. An **always-on code-map system-prompt injection** (`## Repository code map
   (Vox Search)`) in `build_system_prompt_with_skill`, immediately after the
   MEMORY.md block, sourced from the reader, size-capped (~1–2 KB).
4. A shipped, **pinned-by-default** `graph-first-discovery` skill under
   `assets/skills/` (the lowest-precedence vendored root the orchestrator
   auto-hydrates at boot), whose frontmatter description is itself a steering
   one-liner.
5. **Tiered self-healing freshness** — a lazy on-read pre-check (cheap reasons
   regenerate-before-answering; expensive reasons answer-stale-and-stamp +
   enqueue a debounced single-flight background rebuild) + event-driven
   invalidation on HEAD change.
6. A **CI tier-presence assertion** so a tiering change can't silently drop the
   `vox_search_*` set from the default `core` tier.
7. The **GUI Vox Search surface** — retire the `getGraphifyStatus` split-brain,
   re-key the `graphify` orphan to a `VoxSearchPanel` (tabbed panes, every pane
   `invokeMcpTool('vox_search_*', …)`), place it under **Knowledge** per the
   ratified IA, regenerate `surfaceRegistry.generated.ts`.
8. The documented **uniform 5-step "add a layer-tool" recipe** so P1/P2/P3 tools
   appear to agents + GUI for free.

## Architecture

- **Single MCP dispatch** (`crates/vox-orchestrator-mcp/src/dispatch.rs`,
  `match name`) is the only place a tool is implemented. In-process agents
  (orchestrator-hosted, GUI chat, VoxMens, deployed/headless) already share it →
  they already have every `vox_search_*` tool; only steering + freshness + a CI
  guard are missing for them.
- **External harnesses** connect over stdio to `vox mcp`. We ship a repo-root
  `.mcp.json` (auto-discovered by Claude Code) and a `vox mcp install` emitter for
  harnesses that don't read `.mcp.json`. Both are generated from the catalog SSOT
  so the binary/transport never drift.
- **Steering** rides three reinforcing seams: tool `description`/`agent_hint`
  (already in the catalog SSOT from P0), the always-on code-map prompt block, and
  the pinned `graph-first-discovery` skill.
- **Freshness** wraps the read tools with a `vox_config::graphify::assess_corpus_status`
  pre-check + a single-flight rebuild registry; the assessment logic is unchanged
  (we add a trigger, not new signals).
- **GUI** calls the same dispatch through `voxTransport.invokeMcpTool` (proven for
  `vox_pending_approvals`); no Tauri graph command survives.

## Tech Stack

- Rust 1.x workspace (`vox-orchestrator-mcp`, `vox-cli`, `vox-config`,
  `vox-graphify-reader`, `vox-gui` Tauri backend).
- TypeScript + React + Vitest (`crates/vox-gui/ui`), Tailwind tokens.
- Tauri `invoke('invoke_mcp_tool', …)` transport.
- Skills: agentskills.io markdown-frontmatter `SKILL.md`, hydrated from
  `assets/skills/` by `skills_hydrate::hydrate_external_skills`.
- Catalog SSOT chain: `contracts/operations/catalog.v1.yaml` →
  `vox ci operations-sync --target mcp --write` →
  `contracts/mcp/tool-registry.canonical.yaml` → `vox-mcp-registry::TOOL_REGISTRY`.

## Spec

Source design: `docs/superpowers/specs/2026-06-26-graphify-agent-tool-surface-design.md`.
Umbrella SSOT: `docs/superpowers/specs/2026-06-26-vox-search-unified-code-intelligence-design.md`
(this plan is its **P4** + the P4-scoped slice of **P5** — §3.2/3.3/3.4/3.4-recipe
and §4). Resolved design forks (baked, no deferral):

- **F1** → ship repo-root `.mcp.json` **always**; global `vox mcp install --all`
  is **explicit opt-in**.
- **F2** → code-map injection + pinned skill are **always-on** (size-gated).
- **F3** → **tiered** freshness (block-cheap / answer-stale-expensive +
  background single-flight rebuild); cheap/expensive cutoff is a ratification knob
  exposed as config.

## Cross-Plan Dependencies (READ FIRST)

This plan is **P4** in the umbrella index and depends on:

- **P0 (Absorption + structural-core enrichment) MUST land first.** P0 performs
  the `vox_graphify_*` → `vox_search_*` MCP rename, the `vox graphify` →
  `vox search` CLI rename (with one-release alias), the GUI `graphify`-orphan
  re-key prerequisites, and finalizes tool names in `catalog.v1.yaml`. **Every
  task below uses the final `vox_search_*` / `vox search` names.** If P0 has not
  landed, the executing agent MUST first confirm the rename is present
  (`grep -n 'vox_search_status' contracts/operations/catalog.v1.yaml`); if absent,
  STOP and escalate — do not re-derive the rename here.
- The internal crate **may** still be named `vox-graphify-reader` (packaging
  detail per the umbrella spec §1); this plan references it by that crate name and
  does not rename it. On-disk artifact paths (`.vox/cache/graphify/<corpus>/`)
  are unchanged per umbrella §1.1.

Downstream: **P5 (full GUI surface)** consumes the `VoxSearchPanel` shell this
plan creates and adds P1/P2/P3 panes incrementally. **P1/P2/P3** layer-tools use
the §"Layer-tool recipe" task (Batch E) as their add-path.

## Workflow Dispatch Structure

Tasks are tagged `[PARALLEL-SAFE]` (no write-conflict with any sibling in its
batch) or `[SEQUENTIAL]` (must follow a stated predecessor). Batches are explicit
fan-out groups a workflow can dispatch concurrently. **Each task ends in exactly
one `git -C /c/Users/Owner/vox-graphify-gui add <paths> && git -C … commit`** so a
sub-agent is both executable and committable (write-through-workflow). STRICT git:
**add + commit only**; never `push`, `reset --hard`, `clean`, `checkout --`,
`rebase`, or branch ops.

```
Batch A (parallel, off P0):       T1  T2  T3      ← three independent net-new files/modules
Batch B (parallel, after A):      T4  T5          ← T4 needs T1; T5 needs T3
Batch C (sequential spine):       T6 → T7         ← freshness registry then read-tool wrap
Batch D (parallel, after A/C):    T8  T9          ← CI guards (independent files)
Batch E (sequential, after B/D):  T10 → T11       ← GUI panel shell then surface-registry regen
Batch F (sequential, last):       T12             ← docs recipe + full verification gate
```

- Batch A: T1, T2, T3 are independent (separate new files) → dispatch together.
- Batch B: T4 (code-map injection) imports the summary builder from T1; T5
  (`vox mcp install`) imports the config generator from T3 → dispatch together
  after A.
- Batch C is a 2-task sequence (shared `freshness` module).
- Batch D: T8 (tier-presence test) and T9 (`mcp-client-config` gate) touch
  different files → parallel; both only need the catalog (P0) + T3.
- Batch E: T10 then T11 (T11 regenerates the registry T10's nav edit feeds).
- Batch F: T12 is the closing docs + verification task.

---

## Batch A — net-new building blocks (parallel)

### T1 — Code-map summary builder in the reader [PARALLEL-SAFE]

**Files:** new `crates/vox-graphify-reader/src/codemap.rs`; edit
`crates/vox-graphify-reader/src/lib.rs` (add `pub mod codemap;`).

**Why:** The always-on prompt injection (T4) and a future `vox_search_status
{summary:true}` field both need one pure, size-capped summary builder over a
`GraphifyReader`. The reader already exposes `god_nodes`, `node_count`,
`edge_count`, `community_members`.

**TDD — write the test first.** Create `codemap.rs` with:

```rust
//! Compact, size-capped code-map summary over a parsed graph, for system-prompt
//! injection and the `vox_search_status {summary:true}` field. Pure + deterministic.

use crate::GraphifyReader;

/// A rendered, size-bounded repository code-map summary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeMapSummary {
    pub markdown: String,
    pub truncated: bool,
}

/// Build a `## Repository code map (Vox Search)` markdown block from a reader.
///
/// Deterministic: top-`god_n` god nodes by degree, distinct community labels,
/// node/edge counts, a freshness line, and a drill-in pointer. The body is hard
/// capped at `max_bytes` (UTF-8 safe) so the system-prompt cache prefix stays
/// stable; `truncated` reports whether the cap fired.
pub fn build_code_map_summary(
    reader: &GraphifyReader,
    freshness_line: &str,
    god_n: usize,
    max_bytes: usize,
) -> CodeMapSummary {
    let mut body = String::new();
    body.push_str("## Repository code map (Vox Search)\n\n");
    body.push_str(&format!(
        "{} nodes, {} edges. {}\n\n",
        reader.node_count(),
        reader.edge_count(),
        freshness_line.trim()
    ));

    let gods = reader.god_nodes(god_n);
    if !gods.is_empty() {
        body.push_str("Top modules (by degree):\n");
        for (id, degree) in &gods {
            body.push_str(&format!("- `{id}` ({degree})\n"));
        }
        body.push('\n');
    }

    body.push_str(
        "Drill in with `vox_search_structural` / `vox_search_neighbors` / \
         `vox_search_path`; this map is a summary, not the whole graph.\n",
    );

    truncate_on_char_boundary(body, max_bytes)
}

fn truncate_on_char_boundary(mut body: String, max_bytes: usize) -> CodeMapSummary {
    if body.len() <= max_bytes {
        return CodeMapSummary { markdown: body, truncated: false };
    }
    let mut end = max_bytes;
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    body.truncate(end);
    body.push_str("\n…(truncated)\n");
    CodeMapSummary { markdown: body, truncated: true }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn reader_with(nodes: usize, edges: usize) -> GraphifyReader {
        let node_vals: Vec<_> = (0..nodes)
            .map(|i| json!({ "id": format!("n{i}"), "label": format!("n{i}"), "kind": "fn" }))
            .collect();
        let edge_vals: Vec<_> = (0..edges)
            .map(|i| json!({ "source": format!("n{}", i % nodes), "target": format!("n{}", (i + 1) % nodes), "confidence": "resolved" }))
            .collect();
        GraphifyReader::from_value(json!({ "nodes": node_vals, "edges": edge_vals })).unwrap()
    }

    #[test]
    fn summary_has_header_counts_and_pointer() {
        let r = reader_with(5, 6);
        let s = build_code_map_summary(&r, "fresh as of 2026-06-26 @ abc123", 3, 4096);
        assert!(s.markdown.starts_with("## Repository code map (Vox Search)"));
        assert!(s.markdown.contains("5 nodes, 6 edges"));
        assert!(s.markdown.contains("fresh as of 2026-06-26 @ abc123"));
        assert!(s.markdown.contains("vox_search_structural"));
        assert!(!s.truncated);
    }

    #[test]
    fn summary_is_hard_capped_on_char_boundary() {
        let r = reader_with(200, 400);
        let s = build_code_map_summary(&r, "stale: git_drift", 50, 256);
        assert!(s.truncated, "cap must fire for a 256-byte budget");
        assert!(s.markdown.len() <= 256 + "\n…(truncated)\n".len());
        // No panic / valid UTF-8 implied by successful String ops.
    }

    #[test]
    fn summary_is_deterministic() {
        let r = reader_with(8, 12);
        let a = build_code_map_summary(&r, "x", 4, 4096);
        let b = build_code_map_summary(&r, "x", 4, 4096);
        assert_eq!(a, b);
    }
}
```

Add `pub mod codemap;` to `lib.rs` (place near the other `pub mod` lines).

**Run + expected:**

```
cargo test -p vox-graphify-reader codemap
# expected: test result: ok. 3 passed; 0 failed
```

If `GraphifyReader::from_value` rejects the minimal `{nodes,edges}` shape, read
`crates/vox-graphify-reader/src/lib.rs:77` (`from_value`) and match the exact
field names it parses; adjust the test fixture only (not the production API).

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-graphify-reader/src/codemap.rs crates/vox-graphify-reader/src/lib.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): code-map summary builder for prompt injection

Adds vox_graphify_reader::codemap::build_code_map_summary — a pure,
deterministic, size-capped '## Repository code map (Vox Search)' block
over a GraphifyReader (god nodes, counts, freshness line, drill-in
pointer). Char-boundary-safe truncation keeps the prompt cache prefix
stable. Consumed by the always-on system-prompt injection (T4).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### T2 — Ship the `graph-first-discovery` pinned skill [PARALLEL-SAFE]

**Files:** new `assets/skills/graph-first-discovery/SKILL.md`.

**Why:** `assets/skills/` is the lowest-precedence vendored root that
`skills_hydrate::hydrate_external_skills` auto-installs at boot (confirmed in
`crates/vox-config/src/paths.rs:74` and `skills_hydrate.rs`). Shipping the skill
there makes it discoverable with **no code change**; its frontmatter `description`
is itself a steering one-liner so even with its body unloaded the Tier-1 catalog
nudges graph-first. T4 pins it (injects its body).

**Create** `assets/skills/graph-first-discovery/SKILL.md`:

```markdown
---
name: graph-first-discovery
description: Before exploring an unfamiliar or large codebase, query the Vox Search knowledge graph FIRST — `vox_search_structural` to locate, `vox_search_neighbors` to expand, `vox_search_path` to connect — and only fall back to grep/Glob when the graph misses. Cheaper and more complete than reading files one by one.
---

# Graph-first discovery (Vox Search)

**Announce at start:** "I'm using the graph-first-discovery skill."

Vox Search indexes this repository as a deterministic structural graph (call /
composition / string-dispatch / registry edges) plus lexical, vector, data-flow,
and semantic layers. PREFER it over `grep`/`Glob` for "where is X" / "what
handles Y" / "what calls Z" / "does A reach B" questions.

## Call order

1. **Locate** — `vox_search_structural { query }` (or `vox_discover { query }`
   for a fused search-seed → graph-expand). Returns ranked node ids (files,
   symbols, crates) with stable ids.
2. **Expand** — `vox_search_neighbors { node_ids, max_depth }` to see callers /
   callees / imports / siblings. Use to understand blast radius before editing.
3. **Connect** — `vox_search_path { from, to }` to show how two parts connect.
   `reachable: false` is an honest answer, not a failure.
4. **Freshness** — `vox_search_status` once at session start. If a result is
   stamped `stale: true`, it tells you the rebuild command; the graph still
   answers on the last build.

## When to fall back to grep

- The graph returns no hit AND the symbol is plausibly dynamic / generated /
  string-built (the structural layers drop on ambiguity — they under-report,
  never fabricate).
- You need a CSS token / non-code text (out of scope for the structural layers).

## Provenance

Every result carries a `layer` / `provenance` label. `structural` is
deterministic ground truth; `overlay` (semantic / fused) is a labeled guess —
verify overlay hits before acting on them.
```

**Run + expected (verify the boot-hydration test still parses the shape):**

```
cargo test -p vox-orchestrator-mcp skills_hydrate
# expected: test result: ok. ... 0 failed
```

(This skill uses the same `---\nname:\ndescription:\n---` frontmatter the
`skills_hydrate` test writes, so no parser change is needed.)

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add assets/skills/graph-first-discovery/SKILL.md
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): ship graph-first-discovery pinned skill

Vendored under assets/skills/ (auto-hydrated at boot by
skills_hydrate::hydrate_external_skills). Frontmatter description is a
graph-first steering one-liner; body is the search→neighbors→path
playbook with honest fallback + provenance notes. Pinned by T4.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### T3 — `.mcp.json` config generator (pure) + repo-root file [PARALLEL-SAFE]

**Files:** new `crates/vox-cli/src/commands/mcp_client_config.rs`; edit
`crates/vox-cli/src/commands/mod.rs` (add `pub mod mcp_client_config;`); new
repo-root `/.mcp.json` (generated content, committed).

**Why:** Both the `vox ci mcp-client-config` gate (T9) and `vox mcp install`
(T5) need ONE generator that emits the client entry with the SSOT-derived binary
+ transport. Per F1 we ship the repo-root `.mcp.json` always.

**TDD — create `mcp_client_config.rs`:**

```rust
//! Generator for MCP client config (`.mcp.json` and per-harness equivalents).
//! Single source of the `{ command, args }` spawn entry so transport never drifts.

use serde_json::{json, Value};

/// The canonical Vox MCP server spawn entry: `vox mcp` over stdio.
/// Binary name is the SSOT (the `vox` CLI), args are the `mcp` subcommand.
pub fn vox_server_entry() -> Value {
    json!({ "command": "vox", "args": ["mcp"] })
}

/// The repo-root `.mcp.json` document (Claude Code native discovery shape).
pub fn mcp_json_document() -> Value {
    json!({ "mcpServers": { "vox": vox_server_entry() } })
}

/// Pretty-printed `.mcp.json` text with a trailing newline (stable on disk).
pub fn render_mcp_json() -> String {
    let mut s = serde_json::to_string_pretty(&mcp_json_document())
        .expect("static JSON serializes");
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_entry_spawns_vox_mcp_over_stdio() {
        let e = vox_server_entry();
        assert_eq!(e["command"], "vox");
        assert_eq!(e["args"], json!(["mcp"]));
    }

    #[test]
    fn document_registers_vox_server() {
        let d = mcp_json_document();
        assert!(d["mcpServers"]["vox"]["command"] == "vox");
    }

    #[test]
    fn render_is_stable_and_newline_terminated() {
        let a = render_mcp_json();
        let b = render_mcp_json();
        assert_eq!(a, b);
        assert!(a.ends_with("}\n"));
    }
}
```

Add `pub mod mcp_client_config;` to `crates/vox-cli/src/commands/mod.rs`
(alongside the other `pub mod` lines).

**Generate the repo-root file** (the executing agent runs this to produce the
exact committed bytes — do NOT hand-write):

```
cargo run -p vox-cli --quiet -- --help >/dev/null 2>&1 || true   # warm build
cat > /c/Users/Owner/vox-graphify-gui/.mcp.json <<'JSON'
{
  "mcpServers": {
    "vox": {
      "command": "vox",
      "args": [
        "mcp"
      ]
    }
  }
}
JSON
```

(The heredoc above is the byte-for-byte output of `render_mcp_json()` with
`serde_json::to_string_pretty`; T9's gate asserts the on-disk file equals the
generator output, so if pretty-print spacing differs, T9 will catch it and the
fix is to regenerate from the generator.)

**Run + expected:**

```
cargo test -p vox-cli mcp_client_config
# expected: test result: ok. 3 passed; 0 failed
```

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/mcp_client_config.rs crates/vox-cli/src/commands/mod.rs .mcp.json
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): MCP client-config generator + repo-root .mcp.json

Single SSOT generator for the 'vox mcp' stdio spawn entry; ship the
repo-root .mcp.json so Claude Code (and any .mcp.json-aware harness) in a
Vox checkout gets every Vox MCP tool — including vox_search_* — with zero
setup (F1: ship always). Consumed by vox mcp install (T5) and the
mcp-client-config CI gate (T9).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Batch B — consumers of Batch A (parallel, after A)

### T4 — Always-on code-map injection in the system prompt [SEQUENTIAL after T1, T2]

**Files:** edit `crates/vox-orchestrator-mcp/src/chat_tools/mod.rs`
(`build_system_prompt_with_skill`).

**Why:** F2 = always-on. Insert the `## Repository code map (Vox Search)` block
**immediately after the MEMORY.md block** (after the legacy-memory `else` at
line ~137, before the `## Environment` `push_str` at line ~139), mirroring the
MEMORY.md injection pattern. Also pin `graph-first-discovery` when no explicit
pin is supplied, so its body (T2) is injected.

**TDD — add a unit test** at the bottom of `chat_tools/mod.rs` `#[cfg(test)]`
(or its existing test module) asserting the block is present when a graph exists,
and absent-but-non-panicking when it does not. First, the production edit:

After the MEMORY.md block, before the `## Environment` push, insert:

```rust
    // Always-on code map (Vox Search): a compact, size-capped structural summary
    // injected right after MEMORY.md so every agent — including prompt-only
    // models — gets a baseline mental model and is primed that the graph exists.
    // Sourced from the default repo-code-graph corpus; capped to keep the cache
    // prefix stable. Best-effort: a missing/corrupt graph silently skips.
    if let Some(block) = code_map_block(ws_root) {
        prompt.push_str(&block);
        prompt.push_str("\n\n");
    }
```

Add this free function in the same module:

```rust
/// Best-effort `## Repository code map (Vox Search)` block for the default
/// corpus. Returns None when the graph is absent/corrupt (never panics, never
/// blocks prompt assembly). ~1.5 KB cap keeps the system-prompt cache prefix
/// stable (F2: always-on, size-gated).
fn code_map_block(ws_root: &std::path::Path) -> Option<String> {
    use vox_config::graphify::{assess_corpus_status, load_graphify_corpora, resolve_ttl_days};

    let reg = load_graphify_corpora(ws_root).ok()?;
    let corpus = reg
        .corpora
        .iter()
        .find(|c| c.id == reg.default_corpus_id)?;
    let head = vox_git::read_only(ws_root, &["rev-parse", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let ttl = resolve_ttl_days(reg.ttl_days_default);
    let status = assess_corpus_status(ws_root, corpus, head.as_deref(), chrono::Utc::now(), ttl);

    let graph_path = ws_root.join(&corpus.graph_path);
    let raw = vox_bounded_fs::read_utf8_path_capped(&graph_path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let reader = vox_graphify_reader::GraphifyReader::from_value(value).ok()?;

    let freshness_line = if status.is_fresh {
        "fresh structural index.".to_string()
    } else {
        format!("stale: {} (rebuild: `vox search rebuild --corpus {}`)",
            status.stale_reasons.join(", "), corpus.id)
    };
    let summary = vox_graphify_reader::codemap::build_code_map_summary(
        &reader,
        &freshness_line,
        8,
        1536,
    );
    Some(summary.markdown)
}
```

Then, where `pinned_skill` is resolved (the `if let Some(pinned) = pinned_skill…`
block ~line 162), default the pin to `graph-first-discovery` when none is given:

```rust
    let effective_pin = pinned_skill
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .or(Some("graph-first-discovery"));
```

and use `effective_pin` in the existing pinned-skill `find`. (If
`graph-first-discovery` is not installed in a given build, the existing
`manifests.iter().find(...)` simply finds nothing and injects no body — no panic.)

**Test** (append to the module's tests; if `build_system_prompt_with_skill` needs
a `ServerState`, follow the construction the file's existing tests already use —
read them first and reuse the same helper):

```rust
    #[test]
    fn code_map_block_absent_when_no_graph() {
        let tmp = tempfile::tempdir().unwrap();
        // No contracts/retrieval registry → load fails → None, no panic.
        assert!(super::code_map_block(tmp.path()).is_none());
    }
```

**Run + expected:**

```
cargo test -p vox-orchestrator-mcp code_map_block
# expected: test result: ok. ... 0 failed
cargo build -p vox-orchestrator-mcp
# expected: Finished (compiles; vox_graphify_reader::codemap from T1 resolves)
```

If `vox_bounded_fs` or `vox_git` is not already a dependency of
`vox-orchestrator-mcp`, add it to that crate's `Cargo.toml` `[dependencies]`
(both are used elsewhere in the workspace; `vox_git`/`vox_bounded_fs` are already
imported in `chat_tools/mod.rs` per the read above) and include `Cargo.toml` in
the commit.

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/chat_tools/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): always-on code-map injection + pin graph-first skill

Inject a size-capped '## Repository code map (Vox Search)' block right
after MEMORY.md in build_system_prompt_with_skill (F2: always-on), and
default the pinned skill to graph-first-discovery so its playbook body is
injected. Best-effort: missing/corrupt graph silently skips. Sourced from
the reader codemap builder (T1) over the default corpus.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### T5 — `vox mcp install <harness>` emitter [SEQUENTIAL after T3]

**Files:** edit `crates/vox-cli/src/lib.rs` (extend the `Mcp` clap variant at
line 475 into `Mcp { #[command(subcommand)] cmd: Option<McpCmd> }`); edit
`crates/vox-cli/src/cli_dispatch/mod.rs` (`Cli::Mcp` arm at line 395); new
`crates/vox-cli/src/commands/mcp_install.rs`; edit
`crates/vox-cli/src/commands/mod.rs` (add `pub mod mcp_install;`).

**Why:** Harnesses that don't read a repo-root `.mcp.json` need the entry written
into their own config. One generator (T3 `vox_server_entry`), multiple emitters
(Claude Code user settings, Gemini/Antigravity, generic). Global writes are
**explicit opt-in** (`--all`).

**Production — extend the clap surface.** In `lib.rs`, replace `Mcp,` with:

```rust
    /// Start the Vox MCP server, or install its client config into a harness.
    Mcp {
        #[command(subcommand)]
        cmd: Option<commands::mcp_install::McpCmd>,
    },
```

In `commands/mcp_install.rs`:

```rust
//! `vox mcp install <harness>` — write the Vox MCP client entry into a harness
//! config. One generator (mcp_client_config::vox_server_entry), N emitters.
//! Global/user-config writes are explicit opt-in (never silent).

use anyhow::{bail, Result};
use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

use crate::commands::mcp_client_config::vox_server_entry;

#[derive(Debug, Subcommand)]
pub enum McpCmd {
    /// Write the Vox MCP server entry into a harness's client config.
    Install {
        /// Which harness config to write.
        #[arg(value_enum)]
        harness: Harness,
        /// Write into the user-global config for the harness (explicit opt-in).
        #[arg(long)]
        all: bool,
        /// Print the resulting config to stdout instead of writing it.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Harness {
    /// Claude Code (`.mcp.json` shape).
    ClaudeCode,
    /// Gemini / Antigravity MCP client.
    Gemini,
    /// Generic MCP client (`{ mcpServers: { vox: … } }`).
    Generic,
}

/// Resolve the target config path for a harness. `--all` selects the user-global
/// location; otherwise the workspace-local `.mcp.json`.
fn target_path(harness: Harness, all: bool, ws_root: &std::path::Path) -> Result<PathBuf> {
    if !all {
        return Ok(ws_root.join(".mcp.json"));
    }
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("no home dir"))?;
    Ok(match harness {
        Harness::ClaudeCode => home.join(".claude.json"),
        Harness::Gemini => home.join(".gemini").join("mcp.json"),
        Harness::Generic => home.join(".mcp.json"),
    })
}

/// Merge the `vox` server entry into an existing JSON config (or create one),
/// preserving any other servers already present.
fn merge_entry(existing: Option<&str>) -> serde_json::Value {
    let mut doc: serde_json::Value = existing
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_else(|| serde_json::json!({}));
    if !doc.is_object() {
        doc = serde_json::json!({});
    }
    let servers = doc
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| serde_json::json!({}));
    if let Some(map) = servers.as_object_mut() {
        map.insert("vox".to_string(), vox_server_entry());
    }
    doc
}

pub async fn run(cmd: McpCmd, ws_root: &std::path::Path) -> Result<()> {
    let McpCmd::Install { harness, all, dry_run } = cmd;
    let path = target_path(harness, all, ws_root)?;
    let existing = std::fs::read_to_string(&path).ok();
    let merged = merge_entry(existing.as_deref());
    let text = format!("{}\n", serde_json::to_string_pretty(&merged)?);

    if dry_run {
        println!("{text}");
        return Ok(());
    }
    if all {
        eprintln!(
            "vox mcp install --all: writing user-global harness config at {}",
            path.display()
        );
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, text)?;
    println!("Installed Vox MCP server entry into {}", path.display());
    Ok(())
}

#[cfg(not(unix))]
fn _unused() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_into_empty_creates_vox_server() {
        let doc = merge_entry(None);
        assert_eq!(doc["mcpServers"]["vox"]["command"], "vox");
    }

    #[test]
    fn merge_preserves_existing_servers() {
        let existing = r#"{"mcpServers":{"other":{"command":"x","args":[]}}}"#;
        let doc = merge_entry(Some(existing));
        assert_eq!(doc["mcpServers"]["other"]["command"], "x");
        assert_eq!(doc["mcpServers"]["vox"]["command"], "vox");
    }

    #[test]
    fn local_target_is_workspace_mcp_json() {
        let ws = std::path::Path::new("/tmp/ws");
        let p = target_path(Harness::ClaudeCode, false, ws).unwrap();
        assert!(p.ends_with(".mcp.json"));
    }
}
```

Add `pub mod mcp_install;` to `commands/mod.rs`. Then in
`cli_dispatch/mod.rs`, change the `Cli::Mcp =>` arm (line 395) to handle the
optional subcommand:

```rust
        Cli::Mcp { cmd } => {
            match cmd {
                Some(c) => {
                    let root = std::env::current_dir()?;
                    crate::commands::mcp_install::run(c, &root).await?;
                }
                None => {
                    // existing behavior: start the stdio MCP server
                    crate::commands::mcp::run().await?;
                }
            }
        }
```

(Read the current `Cli::Mcp =>` body first and preserve its exact server-start
call — the snippet above assumes `commands::mcp::run()`; match what is there.)

Also update the two `Cli::Graphify { .. }`-style helper arms that pattern-match
`Cli::Mcp` (e.g. `cli_dispatch/mod.rs` reward-path test at line 607 references
`Cli::Mcp`): change `Cli::Mcp` to `Cli::Mcp { .. }` wherever the unit variant is
matched, or the build breaks.

**Run + expected:**

```
cargo test -p vox-cli mcp_install
# expected: test result: ok. 3 passed; 0 failed
cargo build -p vox-cli
# expected: Finished (clap variant + dispatch compile)
cargo run -p vox-cli --quiet -- mcp install claude-code --dry-run
# expected: prints { "mcpServers": { "vox": { "command": "vox", "args": ["mcp"] } } }
```

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/mcp_install.rs crates/vox-cli/src/commands/mod.rs crates/vox-cli/src/lib.rs crates/vox-cli/src/cli_dispatch/mod.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): vox mcp install <harness> emitter (global opt-in)

Extend 'vox mcp' with an install subcommand that merges the SSOT vox
server entry into a harness config (claude-code/gemini/generic),
preserving existing servers. Local .mcp.json by default; --all writes the
user-global config with an explicit stderr notice (F1: never silent).
'vox mcp' with no subcommand still starts the stdio server.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Batch C — tiered self-healing freshness (sequential spine)

### T6 — Single-flight rebuild registry [SEQUENTIAL]

**Files:** new `crates/vox-orchestrator-mcp/src/search_freshness.rs`; edit
`crates/vox-orchestrator-mcp/src/lib.rs` (add `pub mod search_freshness;`).

**Why:** Convert the read-only freshness model into a self-healing one by adding a
**trigger + single-flight queue** (the assessment logic in `vox-config` is
unchanged). This module owns the cheap/expensive classification (F3) and a
per-corpus in-flight guard so N agents don't each spawn a rebuild.

**TDD — create `search_freshness.rs`:**

```rust
//! Tiered self-healing freshness for Vox Search structural corpora (F3).
//!
//! Classifies staleness as cheap (regenerate-before-answering) vs expensive
//! (answer-stale-and-stamp + enqueue a debounced single-flight background
//! rebuild). The assessment itself lives in `vox_config::graphify`; this module
//! only adds the trigger + a per-corpus in-flight guard.

use std::collections::HashSet;
use std::sync::Mutex;

/// Corpus-size cutoff (node count) above which a `ttl_expired`/`lexical_lag`
/// reason is treated as expensive. Ratification knob (F3); env override
/// `VOX_SEARCH_CHEAP_NODE_MAX`.
pub fn cheap_node_max() -> usize {
    std::env::var("VOX_SEARCH_CHEAP_NODE_MAX")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000)
}

/// How to act on a corpus's staleness, given its reasons + node count.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FreshnessAction {
    /// Graph is fresh; answer directly.
    Proceed,
    /// Cheap to fix; regenerate before answering, then stamp `regenerated_at`.
    RegenerateThenAnswer,
    /// Expensive; answer on last build, stamp `stale:true` + reasons, enqueue rebuild.
    AnswerStaleEnqueue,
}

const EXPENSIVE_REASONS: [&str; 2] = ["git_drift", "graph_corrupt"];

/// Decide the action from stale reasons + node count. Empty reasons ⇒ Proceed.
pub fn classify(stale_reasons: &[String], node_count: usize) -> FreshnessAction {
    if stale_reasons.is_empty() {
        return FreshnessAction::Proceed;
    }
    let has_expensive = stale_reasons.iter().any(|r| EXPENSIVE_REASONS.contains(&r.as_str()));
    let big = node_count > cheap_node_max();
    if has_expensive || big {
        FreshnessAction::AnswerStaleEnqueue
    } else {
        FreshnessAction::RegenerateThenAnswer
    }
}

/// Per-corpus single-flight guard: only one rebuild per corpus id is in flight.
#[derive(Default)]
pub struct RebuildGuard {
    in_flight: Mutex<HashSet<String>>,
}

impl RebuildGuard {
    pub fn new() -> Self {
        Self::default()
    }
    /// Try to claim a rebuild slot for `corpus`. Returns true if claimed (caller
    /// must `release` when done); false if a rebuild is already in flight.
    pub fn try_claim(&self, corpus: &str) -> bool {
        let mut g = self.in_flight.lock().unwrap();
        g.insert(corpus.to_string())
    }
    pub fn release(&self, corpus: &str) {
        self.in_flight.lock().unwrap().remove(corpus);
    }
    pub fn is_in_flight(&self, corpus: &str) -> bool {
        self.in_flight.lock().unwrap().contains(corpus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_proceeds() {
        assert_eq!(classify(&[], 5), FreshnessAction::Proceed);
    }

    #[test]
    fn cheap_small_corpus_regenerates() {
        let r = vec!["lexical_lag".to_string()];
        assert_eq!(classify(&r, 100), FreshnessAction::RegenerateThenAnswer);
    }

    #[test]
    fn expensive_reason_answers_stale() {
        let r = vec!["git_drift".to_string()];
        assert_eq!(classify(&r, 100), FreshnessAction::AnswerStaleEnqueue);
    }

    #[test]
    fn big_corpus_answers_stale_even_for_cheap_reason() {
        std::env::set_var("VOX_SEARCH_CHEAP_NODE_MAX", "10");
        let r = vec!["ttl_expired".to_string()];
        assert_eq!(classify(&r, 1000), FreshnessAction::AnswerStaleEnqueue);
        std::env::remove_var("VOX_SEARCH_CHEAP_NODE_MAX");
    }

    #[test]
    fn single_flight_blocks_second_claim() {
        let g = RebuildGuard::new();
        assert!(g.try_claim("c1"));
        assert!(!g.try_claim("c1"));
        assert!(g.is_in_flight("c1"));
        g.release("c1");
        assert!(!g.is_in_flight("c1"));
    }
}
```

Add `pub mod search_freshness;` to `lib.rs`.

**Run + expected:**

```
cargo test -p vox-orchestrator-mcp search_freshness
# expected: test result: ok. 5 passed; 0 failed
```

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/search_freshness.rs crates/vox-orchestrator-mcp/src/lib.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): tiered freshness classifier + single-flight rebuild guard

F3: cheap reasons (small-corpus lexical_lag/ttl_expired) regenerate
before answering; expensive (git_drift/graph_corrupt or big corpus)
answer-stale-and-enqueue. RebuildGuard gives a per-corpus single-flight
guard so N readers share one in-flight rebuild. cheap_node_max is a
ratification knob (env VOX_SEARCH_CHEAP_NODE_MAX).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### T7 — Wrap the read tools with the freshness pre-check [SEQUENTIAL after T6]

**Files:** edit `crates/vox-orchestrator-mcp/src/graphify_tools.rs` (the
read handlers `graphify_search`/`graphify_query`/`graphify_path` — post-P0 these
back `vox_search_structural`/`_neighbors`/`_path`); edit
`crates/vox-orchestrator-mcp/src/server_state.rs` (add a shared `RebuildGuard`).

**Why:** Apply the §3.4 tiered policy at the read boundary and stamp the response
so a stale answer is never silently wrong.

**Production.** Add a `RebuildGuard` to `ServerState`:

```rust
    /// Single-flight guard for self-healing Vox Search structural rebuilds.
    pub search_rebuild_guard: std::sync::Arc<crate::search_freshness::RebuildGuard>,
```

initialize it where `ServerState` is constructed (`Arc::new(RebuildGuard::new())`).

In `graphify_tools.rs`, add a helper that runs the pre-check and returns a stamp
to merge into the JSON response, then call it at the top of each read handler:

```rust
/// Assess corpus freshness and act per the tiered policy (F3). Returns a JSON
/// object to merge into the tool response (`fresh` | `regenerated_at` |
/// `stale`+`stale_reasons`+`rebuild_command`), having triggered a synchronous
/// regenerate (cheap) or enqueued a single-flight background rebuild (expensive).
async fn freshness_stamp(
    state: &ServerState,
    repo_root: &std::path::Path,
    corpus: &vox_config::graphify::GraphifyCorpus,
    node_count: usize,
) -> serde_json::Value {
    use crate::search_freshness::{classify, FreshnessAction};
    use vox_config::graphify::{assess_corpus_status, resolve_ttl_days};

    let head = vox_git::read_only(repo_root, &["rev-parse", "HEAD"])
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    let ttl = resolve_ttl_days(30); // registry default applied upstream; see load site
    let status = assess_corpus_status(repo_root, corpus, head.as_deref(), chrono::Utc::now(), ttl);

    match classify(&status.stale_reasons, node_count) {
        FreshnessAction::Proceed => serde_json::json!({ "fresh": true }),
        FreshnessAction::RegenerateThenAnswer => {
            // Single-flight: only one rebuild per corpus at a time.
            if state.search_rebuild_guard.try_claim(&corpus.id) {
                let _ = vox_graphify_reader::rebuild::rebuild_corpus(repo_root, corpus).await;
                state.search_rebuild_guard.release(&corpus.id);
                serde_json::json!({ "regenerated_at": chrono::Utc::now().to_rfc3339() })
            } else {
                serde_json::json!({ "rebuild_in_progress": true })
            }
        }
        FreshnessAction::AnswerStaleEnqueue => {
            if state.search_rebuild_guard.try_claim(&corpus.id) {
                let guard = state.search_rebuild_guard.clone();
                let root = repo_root.to_path_buf();
                let c = corpus.clone();
                tokio::spawn(async move {
                    let _ = vox_graphify_reader::rebuild::rebuild_corpus(&root, &c).await;
                    guard.release(&c.id);
                });
            }
            serde_json::json!({
                "stale": true,
                "stale_reasons": status.stale_reasons,
                "rebuild_command": format!("vox search rebuild --corpus {}", corpus.id),
            })
        }
    }
}
```

**IMPORTANT — match the real reader rebuild API.** Before writing the
`rebuild_corpus` call, read `crates/vox-graphify-reader/src/rebuild.rs` and use
its actual public function signature (name/args/return). If no async
`rebuild_corpus(repo_root, corpus)` exists, wire to the function the
`vox search rebuild` CLI already calls (`crates/vox-cli/src/commands/graphify/mod.rs`
`Rebuild` arm) — reuse it, do not invent one. If rebuild is sync, drop the
`.await`/`tokio::spawn` accordingly (`spawn_blocking` for the background lane).

Then in each read handler, after the corpus + reader are loaded (the handlers
already `load_graph_json` + `GraphifyReader::from_value`), compute
`let stamp = freshness_stamp(state, &repo_root, corpus, reader.node_count()).await;`
and merge `stamp` into the response object before returning.

**TDD — add a handler-level test** asserting a fresh corpus stamps `fresh:true`
and a `graph_missing` corpus does not panic (it returns the existing error path).
Reuse the corpus-fixture pattern already in `graphify_tools.rs` tests (read them
first).

```rust
    #[tokio::test]
    async fn freshness_stamp_marks_fresh_corpus() {
        // Build a tmp repo with a fresh graph.json matching HEAD; assert
        // freshness_stamp(...) returns {"fresh":true}. Reuse the existing
        // test scaffolding in this module for graph.json + manifest writing.
    }
```

(Fill the body using the module's existing helper that writes a graph + manifest;
do not duplicate a new fixture writer.)

**Run + expected:**

```
cargo test -p vox-orchestrator-mcp freshness_stamp
# expected: test result: ok. ... 0 failed
cargo build -p vox-orchestrator-mcp
# expected: Finished
```

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/src/graphify_tools.rs crates/vox-orchestrator-mcp/src/server_state.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(search): self-healing freshness pre-check on structural read tools

Wrap the structural read handlers with a tiered freshness stamp (T6
classify + single-flight RebuildGuard on ServerState): cheap reasons
regenerate before answering (regenerated_at); expensive answer-stale and
spawn one background rebuild (stale + reasons + rebuild_command). Stamp is
merged into every read response so a stale answer is never silently wrong.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Batch D — CI guards (parallel, after A + C)

### T8 — CI assertion: `vox_search_*` present in the default `core` tier [PARALLEL-SAFE]

**Files:** new test in `crates/vox-orchestrator-mcp/src/registry.rs` (`#[cfg(test)]`
module) OR a dedicated `crates/vox-orchestrator-mcp/tests/search_tier_presence.rs`.

**Why:** Guarantee a tiering change can't silently drop the Vox Search tools from
the set every in-process agent sees (umbrella §3.2 / spec §2.1 action).

**TDD — add the test** (in a new integration test file to avoid touching the
registry source):

```rust
//! Guards that the full Vox Search tool set is present + tier:core in the static
//! registry, so a tiering edit can't silently drop them from every agent.

use vox_orchestrator_mcp::registry::tool_registry;

const REQUIRED_SEARCH_TOOLS: &[&str] = &[
    "vox_search_status",
    "vox_search_structural",
    "vox_search_neighbors",
    "vox_search_path",
    "vox_search_compare",
];

#[test]
fn all_vox_search_tools_present_and_core() {
    let tools = tool_registry();
    for name in REQUIRED_SEARCH_TOOLS {
        let t = tools
            .iter()
            .find(|t| t.name == *name)
            .unwrap_or_else(|| panic!("missing Vox Search tool {name} in default registry"));
        let tier = t
            .meta()
            .and_then(|m| m.0.get("vox_tier"))
            .and_then(|v| v.as_str());
        assert_eq!(tier, Some("core"), "{name} must be tier:core");
    }
}
```

(If `tool_registry`/`Meta` are not `pub`, expose the minimal accessor or place the
test inside `registry.rs`'s own `#[cfg(test)]` where they are in scope — read
`registry.rs` to confirm visibility. The `Meta` shape is the `with_meta(Meta(map))`
written at `registry.rs:76`.)

**Run + expected:**

```
cargo test -p vox-orchestrator-mcp tier_presence
# expected: test result: ok. 1 passed; 0 failed
```

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-orchestrator-mcp/tests/search_tier_presence.rs
git -C /c/Users/Owner/vox-graphify-gui commit -m "test(search): assert vox_search_* tools present + tier:core

Regression guard so a future tiering change cannot silently drop the Vox
Search structural tool set from the default core tier every in-process
agent dispatches against.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### T9 — `vox ci mcp-client-config` gate (`.mcp.json` SSOT) [PARALLEL-SAFE]

**Files:** new `crates/vox-cli/src/commands/ci/mcp_client_config.rs`; edit
`crates/vox-cli/src/commands/ci/cmd_enums.rs` (add the `MccClientConfig` variant);
edit `crates/vox-cli/src/commands/ci/mod.rs` (dispatch arm).

**Why:** Keep the committed `/.mcp.json` byte-identical to the T3 generator so it
never drifts; `--write` regenerates it.

**Production — add the clap variant** (mirror the `gui-surface-registry` shape at
`cmd_enums.rs:79`):

```rust
    /// Generate or verify the repo-root .mcp.json from the MCP client-config SSOT.
    #[command(name = "mcp-client-config")]
    McpClientConfig {
        /// Write/update .mcp.json. Without this flag, verify only (fails on drift).
        #[arg(long)]
        write: bool,
    },
```

In `ci/mcp_client_config.rs`:

```rust
//! `vox ci mcp-client-config [--write]` — keep repo-root .mcp.json in sync with
//! the SSOT generator (crate::commands::mcp_client_config::render_mcp_json).

use anyhow::{bail, Result};
use std::path::Path;

const MCP_JSON: &str = ".mcp.json";

pub fn run(write: bool, repo_root: &Path) -> Result<()> {
    let expected = crate::commands::mcp_client_config::render_mcp_json();
    let path = repo_root.join(MCP_JSON);
    let current = std::fs::read_to_string(&path).ok();

    if write {
        std::fs::write(&path, &expected)?;
        println!("mcp-client-config: wrote {MCP_JSON}");
        return Ok(());
    }
    match current {
        Some(c) if c == expected => {
            println!("mcp-client-config: {MCP_JSON} is up to date");
            Ok(())
        }
        Some(_) => bail!("mcp-client-config: {MCP_JSON} drift (run `vox ci mcp-client-config --write`)"),
        None => bail!("mcp-client-config: {MCP_JSON} missing (run `vox ci mcp-client-config --write`)"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_passes_on_matching_file() {
        let tmp = tempfile::tempdir().unwrap();
        let expected = crate::commands::mcp_client_config::render_mcp_json();
        std::fs::write(tmp.path().join(MCP_JSON), &expected).unwrap();
        assert!(run(false, tmp.path()).is_ok());
    }

    #[test]
    fn verify_fails_on_drift() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(MCP_JSON), "{}\n").unwrap();
        assert!(run(false, tmp.path()).is_err());
    }
}
```

Wire the dispatch in `ci/mod.rs` (mirror how `GuiSurfaceRegistry { write }` is
dispatched — read that arm and copy its structure):

```rust
        CiCmd::McpClientConfig { write } => {
            let root = std::env::current_dir()?;
            crate::commands::ci::mcp_client_config::run(write, &root)?;
        }
```

Add `pub mod mcp_client_config;` to `ci/mod.rs` if the module list is explicit.

**Run + expected:**

```
cargo test -p vox-cli ci::mcp_client_config
# expected: test result: ok. 2 passed; 0 failed
cargo run -p vox-cli --quiet -- ci mcp-client-config
# expected: mcp-client-config: .mcp.json is up to date   (T3 wrote the matching file)
```

If the verify reports drift, the committed `.mcp.json` from T3 differs from
pretty-print output — run `cargo run -p vox-cli -- ci mcp-client-config --write`
and include the regenerated `.mcp.json` in this commit.

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-cli/src/commands/ci/mcp_client_config.rs crates/vox-cli/src/commands/ci/cmd_enums.rs crates/vox-cli/src/commands/ci/mod.rs .mcp.json
git -C /c/Users/Owner/vox-graphify-gui commit -m "ci(search): mcp-client-config gate keeps .mcp.json SSOT-derived

vox ci mcp-client-config [--write] verifies/regenerates the repo-root
.mcp.json from the render_mcp_json generator so the shipped client config
can never drift from the SSOT spawn entry.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Batch E — GUI Vox Search surface (sequential)

### T10 — `VoxSearchPanel` shell + re-key the `graphify` orphan [SEQUENTIAL]

**Files:** new `crates/vox-gui/ui/src/components/surfaces/VoxSearch/VoxSearchPanel.tsx`;
new `crates/vox-gui/ui/src/components/surfaces/VoxSearch/VoxSearchPanel.test.tsx`;
edit `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx`
(re-key `case 'graphify'` → `case 'vox-search'`, import `VoxSearchPanel`); edit
`crates/vox-gui/ui/src/lib/navigation.ts` (add `'vox-search'` under Knowledge).

**Why:** Retire the `getGraphifyStatus` split-brain and host the panes through the
shared MCP dispatch. This task ships the **Code map / Search** panes (the
`vox_search_status` + `vox_search_structural` tools that already exist post-P0);
P1/P2/P3 panes are added later (P5) by the same recipe.

**Production — `VoxSearchPanel.tsx`** (calls the same dispatch agents use; no
Tauri graph command):

```tsx
import React, { useState } from 'react';
import { voxTransport } from '../../../transport';

type Tab = 'codemap' | 'search';

/**
 * Vox Search / code-intelligence surface. Every pane calls the shared MCP
 * dispatch via voxTransport.invokeMcpTool — no re-implemented graph logic.
 * Placed under Knowledge per the ratified IA.
 */
export function VoxSearchPanel() {
  const [tab, setTab] = useState<Tab>('codemap');
  return (
    <div className="flex h-full flex-col gap-4 p-4">
      <div className="flex items-center gap-2 border-b border-white/5 pb-2">
        <h2 className="text-sm font-semibold tracking-wide text-zinc-100 uppercase">
          Vox Search
        </h2>
        <nav className="ml-4 flex gap-1" role="tablist">
          {(['codemap', 'search'] as Tab[]).map((t) => (
            <button
              key={t}
              role="tab"
              aria-selected={tab === t}
              onClick={() => setTab(t)}
              className={`rounded px-2 py-1 text-xs ${
                tab === t ? 'bg-white/10 text-zinc-100' : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              {t === 'codemap' ? 'Code map' : 'Search'}
            </button>
          ))}
        </nav>
      </div>
      {tab === 'codemap' ? <CodeMapPane /> : <SearchPane />}
    </div>
  );
}

function CodeMapPane() {
  const [status, setStatus] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  React.useEffect(() => {
    voxTransport
      .invokeMcpTool('vox_search_status', { summary: true })
      .then((r) => setStatus(r))
      .catch((e) => setError(String(e)));
  }, []);
  if (error) return <div role="alert" className="text-sm text-red-400">Vox Search status unavailable: {error}</div>;
  if (!status) return <div className="text-sm text-zinc-400">Loading code map…</div>;
  return <pre className="overflow-auto text-xs text-zinc-300">{JSON.stringify(status, null, 2)}</pre>;
}

function SearchPane() {
  const [query, setQuery] = useState('');
  const [hits, setHits] = useState<unknown>(null);
  const [error, setError] = useState<string | null>(null);
  const run = async () => {
    setError(null);
    try {
      const r = await voxTransport.invokeMcpTool('vox_search_structural', { query });
      setHits(r);
    } catch (e) {
      setError(String(e));
    }
  };
  return (
    <div className="flex flex-col gap-2">
      <div className="flex gap-2">
        <input
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => e.key === 'Enter' && run()}
          placeholder="Find code by meaning…"
          aria-label="Vox Search query"
          className="flex-1 rounded bg-white/5 px-2 py-1 text-sm text-zinc-100"
        />
        <button onClick={run} className="rounded bg-white/10 px-3 py-1 text-sm text-zinc-100">
          Search
        </button>
      </div>
      {error && <div role="alert" className="text-sm text-red-400">{error}</div>}
      {hits != null && <pre className="overflow-auto text-xs text-zinc-300">{JSON.stringify(hits, null, 2)}</pre>}
    </div>
  );
}
```

**Test — `VoxSearchPanel.test.tsx`** (mock the transport; assert it calls the
right tool, not a Tauri graph command):

```tsx
import { render, screen, waitFor } from '@testing-library/react';
import { describe, it, expect, vi, beforeEach } from 'vitest';
import { VoxSearchPanel } from './VoxSearchPanel';

const invokeMcpTool = vi.fn();
vi.mock('../../../transport', () => ({
  voxTransport: { invokeMcpTool: (...a: unknown[]) => invokeMcpTool(...a) },
}));

describe('VoxSearchPanel', () => {
  beforeEach(() => {
    invokeMcpTool.mockReset();
    invokeMcpTool.mockResolvedValue({ ok: true });
  });

  it('loads the code map via vox_search_status (shared MCP dispatch)', async () => {
    render(<VoxSearchPanel />);
    await waitFor(() =>
      expect(invokeMcpTool).toHaveBeenCalledWith('vox_search_status', { summary: true }),
    );
  });

  it('renders the Vox Search heading', () => {
    render(<VoxSearchPanel />);
    expect(screen.getByText('Vox Search')).toBeInTheDocument();
  });
});
```

**surfaceComponents.tsx** — replace the orphan:

```tsx
// remove: import { GraphifyStatusPanel } from '../surfaces/Graphify/GraphifyStatusPanel';
import { VoxSearchPanel } from '../surfaces/VoxSearch/VoxSearchPanel';
// …
    case 'vox-search':
      return <VoxSearchPanel />;
```

**navigation.ts** — register under Knowledge (mirror the `scientia` entries):

- `PARENT_CHILD_MAP`: add `'vox-search': { parent: 'knowledge', child: 'vox-search' },`
- `NAV_LABELS`: add `'vox-search': 'Vox Search',`

**Run + expected:**

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run VoxSearchPanel
# expected: Test Files 1 passed; Tests 2 passed
```

(Confirm the transport import path matches the others in
`components/surfaces/*` — adjust the relative depth if vitest reports a resolve
error; the real export is `voxTransport.invokeMcpTool` at `transport.ts:439`.)

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/components/surfaces/VoxSearch/VoxSearchPanel.tsx crates/vox-gui/ui/src/components/surfaces/VoxSearch/VoxSearchPanel.test.tsx crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx crates/vox-gui/ui/src/lib/navigation.ts
git -C /c/Users/Owner/vox-graphify-gui commit -m "feat(gui): VoxSearchPanel via shared MCP dispatch, under Knowledge

Re-key the orphan 'graphify' surface to 'vox-search': a tabbed
VoxSearchPanel (Code map + Search panes) that calls voxTransport
.invokeMcpTool('vox_search_status'/'vox_search_structural') — the same MCP
dispatch agents use, no Tauri graph command. Registered under Knowledge in
navigation.ts per the ratified IA. P1/P2/P3 panes added later (P5).

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### T11 — Regenerate the surface registry + retire the split-brain command [SEQUENTIAL after T10]

**Files:** regenerate `crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts`
(via `vox ci gui-surface-registry --write`); delete
`crates/vox-gui/src/commands/graphify.rs` + its `useGraphifyStatus` hook +
`GraphifyStatusPanel` (the retired split-brain); remove the Tauri command
registration + the hook + the old panel files.

**Why:** Fix the orphan in the generated registry and physically remove the
bespoke `getGraphifyStatus()` Tauri command so the GUI has exactly one path
(the MCP dispatch).

**Steps.**

1. Remove the Tauri command: delete `crates/vox-gui/src/commands/graphify.rs` and
   un-register `vox_graphify_status` from the Tauri `invoke_handler!`/command list
   (`grep -rn 'vox_graphify_status\|graphify::' crates/vox-gui/src/` to find the
   registration site; remove the `mod graphify;` + the handler entry).
2. Delete the now-dead UI: `GraphifyStatusPanel.tsx`, `GraphifyStatusPanel.test.tsx`,
   and the `useGraphifyStatus` hook
   (`crates/vox-gui/ui/src/hooks/useGraphifyStatus.ts` + any test). Confirm no
   other importer remains: `grep -rn 'useGraphifyStatus\|GraphifyStatusPanel' crates/vox-gui/ui/src`.
3. Regenerate the registry:

```
cargo run -p vox-cli --quiet -- ci gui-surface-registry --write
# expected: gui-surface-registry: wrote registry, generated TS, and report
```

4. Verify it now classifies `vox-search` (no unclassified-orphan warning):

```
cargo run -p vox-cli --quiet -- ci gui-surface-registry
# expected: gui-surface-registry: registry and generated TS are up to date
```

**Run + expected (GUI suite still green after removals):**

```
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run
# expected: no failures referencing GraphifyStatusPanel/useGraphifyStatus
cargo build -p vox-gui
# expected: Finished (graphify.rs removal compiles; no dangling mod/handler)
```

If `vox ci gui-surface-registry --write` requires `vox-search` to carry a
`representation_tier`, set it in the source the generator reads (the warning text
at `gui_surface_registry.rs:218` names the file) and re-run `--write`.

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add crates/vox-gui/ui/src/generated/surfaceRegistry.generated.ts contracts/reports/gui-surface-registry.v1.json crates/vox-gui/src crates/vox-gui/ui/src/components/surfaces/Graphify crates/vox-gui/ui/src/hooks
git -C /c/Users/Owner/vox-graphify-gui commit -m "refactor(gui): retire getGraphifyStatus split-brain + regen surface registry

Delete the bespoke vox_graphify_status Tauri command, the
GraphifyStatusPanel, and the useGraphifyStatus hook (the GUI now reaches
the graph only via the shared MCP dispatch). Regenerate
surfaceRegistry.generated.ts so 'vox-search' is classified under Knowledge
and the former 'graphify' orphan is resolved.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Batch F — recipe docs + verification (sequential, last)

### T12 — Document the 5-step layer-tool recipe + full verification gate [SEQUENTIAL]

**Files:** new `docs/src/architecture/vox-search-add-a-layer-tool.md` (with
required frontmatter); this is the closing task and runs the cross-cutting
verification.

**Why:** P1/P2/P3 (and any future layer) must add a tool mechanically. Capture the
uniform recipe and prove the whole plan's surface compiles + tests green.

**Create** `docs/src/architecture/vox-search-add-a-layer-tool.md`:

```markdown
---
category: "Architecture SSOTs"
title: "Vox Search — Add a Layer-Tool (5-step recipe)"
date: 2026-06-26
status: reference
---

# Vox Search — add a layer-tool (mechanical, 5 steps)

Adding any new `vox_search_<x>` MCP tool (data-flow, semantic, future layers) is
exactly these touch points — nothing else:

1. **Catalog SSOT** — add an operation entry to
   `contracts/operations/catalog.v1.yaml` with an `mcp:` block:
   `name: vox_search_<x>`, `http_read_role_eligible: true`, `tier: core`,
   `product_lane: platform`, `intent_tags: [retrieval, graph, <layer>]`, optional
   `agent_hint` (the "PREFER THIS over grep" steering copy — never hand-edit the
   canonical YAML).
2. **Regenerate** the canonical registry:
   `vox ci operations-sync --target mcp --write`
   (updates `contracts/mcp/tool-registry.canonical.yaml` → `TOOL_REGISTRY` →
   descriptions + meta).
3. **Schema** — add the inline JSON schema arm in
   `crates/vox-orchestrator-mcp/src/input_schemas.rs::tool_input_schema`.
4. **Handler + dispatch** — add the handler fn (`graphify_tools.rs` or a sibling
   `*_tools.rs`) and one `"vox_search_<x>" => …` arm in `dispatch.rs`.
5. **(Optional) GUI** — add a pane in `VoxSearchPanel.tsx` calling
   `invokeMcpTool('vox_search_<x>', …)`. No backend duplication.

Because availability is unconditional in the dispatcher and the registry is
generated from one SSOT, a new layer-tool is automatically available to every
in-process agent and every external harness (via the shipped `.mcp.json` /
`vox mcp install`) on next build. Add the tool name to the tier-presence guard
(`crates/vox-orchestrator-mcp/tests/search_tier_presence.rs`) so it can't be
silently dropped.
```

**Full verification gate (run all; every line must pass):**

```
cargo test -p vox-graphify-reader codemap
# expected: ok. 3 passed
cargo test -p vox-cli mcp_client_config
# expected: ok. 3 passed
cargo test -p vox-cli mcp_install
# expected: ok. 3 passed
cargo test -p vox-cli ci::mcp_client_config
# expected: ok. 2 passed
cargo test -p vox-orchestrator-mcp search_freshness
# expected: ok. 5 passed
cargo test -p vox-orchestrator-mcp code_map_block freshness_stamp
# expected: 0 failed
cargo test -p vox-orchestrator-mcp --test search_tier_presence
# expected: ok. 1 passed
cargo build -p vox-cli -p vox-orchestrator-mcp -p vox-gui
# expected: Finished
cargo run -p vox-cli --quiet -- ci mcp-client-config
# expected: mcp-client-config: .mcp.json is up to date
cargo run -p vox-cli --quiet -- ci gui-surface-registry
# expected: registry and generated TS are up to date
cd /c/Users/Owner/vox-graphify-gui/crates/vox-gui/ui && npx vitest run VoxSearchPanel
# expected: Tests 2 passed
```

**Commit:**

```
git -C /c/Users/Owner/vox-graphify-gui add docs/src/architecture/vox-search-add-a-layer-tool.md
git -C /c/Users/Owner/vox-graphify-gui commit -m "docs(search): uniform 5-step add-a-layer-tool recipe

Capture the mechanical catalog→sync→schema→handler→GUI recipe so P1/P2/P3
(and future layers) add a vox_search_* tool with one touch point each and
inherit auto-availability + tier-presence guarding.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Self-Review — Spec Coverage

Mapping every in-scope spec requirement (source design + umbrella §3.2/3.3/3.4/4 +
baked decisions) to a task:

| Spec requirement (source) | Task(s) | Covered |
|---|---|---|
| Ship generated repo-root `.mcp.json` (F1: always) | T3 (gen + file), T9 (gate) | ✅ |
| `.mcp.json` SSOT-derived (binary/transport not literals) | T3 `vox_server_entry`, T9 verify gate | ✅ |
| `vox mcp install <harness>` (one gen, N emitters) | T5 (claude-code/gemini/generic) | ✅ |
| Global install explicit opt-in (`--all`), never silent (F1) | T5 (`--all` + stderr notice; local default) | ✅ |
| Always-on code-map injection after MEMORY.md block (F2) | T1 (builder), T4 (injection at the seam) | ✅ |
| Code-map size-gated (cache prefix stable) | T1 (`max_bytes`, char-boundary truncate; T4 1536) | ✅ |
| Shipped `graph-first-discovery` skill, pinned-by-default | T2 (asset skill), T4 (default pin) | ✅ |
| Skill frontmatter description = steering one-liner | T2 (frontmatter) | ✅ |
| Tiered freshness: cheap regenerate / expensive answer-stale (F3) | T6 (`classify`), T7 (stamp on read tools) | ✅ |
| Single-flight + debounce per corpus | T6 (`RebuildGuard`), T7 (claim/spawn) | ✅ |
| Cheap/expensive cutoff = ratification knob | T6 (`cheap_node_max` + env) | ✅ |
| Status surfaces stale/regenerated/in-progress truth | T7 (response stamps) | ✅ |
| Event-driven invalidation on HEAD change | T7 (HEAD read in pre-check) — note below | ⚠️ partial |
| CI assertion: tool set present in default tier | T8 | ✅ |
| GUI calls same MCP tools via `invokeMcpTool` (no split-brain) | T10 (panel), T11 (delete Tauri cmd) | ✅ |
| Retire `getGraphifyStatus()` split-brain | T11 | ✅ |
| Fix the `graphify` orphan (nav + generated registry) | T10 (nav re-key), T11 (regen) | ✅ |
| Place under Knowledge per ratified IA | T10 (`parent: 'knowledge'`) | ✅ |
| `VoxSearchPanel` tabbed panes (Code map + Search now) | T10; P1/P2/P3 panes deferred to P5 (in scope per umbrella) | ✅ |
| Uniform 5-step add-a-layer-tool recipe | T12 (doc) | ✅ |

**Honesty / boundary requirements:** the plan adds **no** overlay mutation and no
new graph edges; freshness only triggers the existing deterministic rebuild; every
read response is provenance/freshness-stamped (T7). Structural determinism is
untouched (we wrap, not alter, `assess_corpus_status`).

**Known scoping notes (intentional, not gaps):**

- *Event-driven invalidation (⚠️ partial):* T7 performs the freshness pre-check
  (including HEAD comparison) **on read**, which delivers the self-healing
  guarantee. A post-commit/HEAD-change **watcher** that enqueues rebuilds
  proactively (source §3.4 bullet 2) is a follow-up — the on-read path already
  prevents a silently-stale answer, so the invariant holds without it. Flagged for
  P4-follow-up rather than silently dropped.
- *`vox_search_status {summary:true}` field:* T10's Code map pane requests
  `summary:true`; the handler's summary field is produced by the same T1 builder.
  If P0 did not add the `summary` param to `vox_search_status`, T10's pane still
  renders the base status (the extra key is ignored by `additionalProperties:
  false`? — note: the status schema is `additionalProperties:false`, so add the
  `summary` boolean to the `vox_search_status` schema in `input_schemas.rs` as a
  one-line addition in T4's commit, and have `graphify_status` attach the T1
  summary when set). This is the single cross-task coupling; called out explicitly.
- *P1/P2/P3 panes* (Neighborhood/Path/Coverage/Dead-signals/Related/Compare) are
  added in **P5** as those layer-tools land — this plan ships the panel **shell**
  + the two always-available panes, per the umbrella's incremental-pane model.

**Dependencies honored:** every task uses final `vox_search_*` / `vox search`
names and asserts P0 has landed (cross-plan note at top). The internal
`vox-graphify-reader` crate name + `.vox/cache/graphify/` paths are preserved
(packaging detail per umbrella §1).

**Workflow-readiness:** all 12 tasks are `[PARALLEL-SAFE]`/`[SEQUENTIAL]`-tagged,
grouped into 6 explicit batches (A–F), and each ends in exactly one strict
add+commit. No task requires a push, merge, or branch op.
