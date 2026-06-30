---
title: "Skill Proposal — Accept → Author → Install (Sub-Project 4)"
status: design
date: 2026-06-30
audience: contributors
program: agent-authored-skills-from-repeated-operations
---

# Skill Proposal: Accept → Author → Install (SP-4)

## Context

This is sub-project 4 of *agent-authored skills from repeated operations* — the
loop that makes Vox self-improving: an agent does work, operations are captured,
recurring sequences are mined and proposed, and (here) the user accepts a
proposal so it becomes a real installed skill the agent can reuse.

1. **Operation capture** — ✅ on main.
2. **Sequence mining** (`vox-skill-discovery::op_miner` produces
   `RepeatedOperations` `Candidate`s) — ✅ on main.
3. **HITL proposal surface** — ✅ on main. A mined procedure becomes a
   `FeedbackKind::SkillProposal` item in the NeedsYou inbox with a single
   **Dismiss** action (sends `{action:"skip"}`).
4. **Accept → author → install** — *this spec*. The user clicks **Save as
   skill** on a proposal; the system composes a valid `SKILL.md` from the mined
   `Candidate` and installs it under the user skill root via the existing
   `install_to_user_root`.

SP-3 (decision D3) deferred *how an accepted draft is reconstructed* to SP-4.
This spec resolves it: **store the `Candidate` JSON on the feedback item at
propose time, and reconstruct from it at accept time.** Re-mining at accept time
was rejected — operations age out of the capture buffer, so the candidate may
not reproduce.

## Two corrections to the SP-3 hand-off

The SP-3 hand-off contained two factual errors that this spec corrects, both
verified against the rebased tree:

1. **`FeedbackRequest` has no reusable `meta` field.** That field lives on the
   `ToolResult` envelope, a different struct. SP-4 *adds*
   `meta: Option<serde_json::Value>` to `FeedbackRequest` and threads it through
   `FeedbackStore::register`. This is the sanctioned mechanism, not a reuse.

2. **The mined `members` are provenance anchors, not steps.** `op_miner.rs`
   stores `members: a.anchors` — strings like `"session:s1@1719768000000"` — and
   throws away `a.tools` (the actual `["read","write","execute"]` sequence),
   keeping it only as prose inside `draft_frontmatter.description`. A SKILL.md
   whose body enumerates `members` would print `1. session:s1@…` — a hollow stub
   an agent cannot execute. **SP-4 first repoints `members` at the tool sequence**
   (a net deletion: the anchor bookkeeping is removed). Authoring then enumerates
   real steps.

## Goal

Accepting a skill proposal writes a valid, *executable-as-documentation*
`SKILL.md` derived from the mined candidate, installs it under the workspace
skill root, and surfaces the installed skill name back to the user.

## Architecture

Data flows proposal → storage → accept → authored file → install:

```
miner Candidate ──(JSON)──► vox_propose_skill(candidate)
                                   │
                                   ▼
        Orchestrator::propose_skill(name, desc, session, meta=candidate_json)
                                   │  stores candidate on FeedbackRequest.meta
                                   ▼
                      NeedsYou inbox  ──GUI──►  [Save as skill] button
                                                       │  {action:"accept_skill"}
                                                       ▼
        vox_resolve_feedback: SkillProposal + AcceptSkill
                                   │  read meta → Candidate
                                   │  author_skill_md(name, description, steps)
                                   │  write SKILL.md to a unique temp dir
                                   ▼
        install_to_user_root(tmp, ws_root, global=false)  → <ws_root>/.vox/skills/<name>/
                                   │
                                   ▼
                       installed skill names → tool result JSON
```

### Components

- **`op_miner` `members` fix** (`vox-skill-discovery`) — change
  `members: a.anchors` to `members: a.tools` so a `RepeatedOperations`
  candidate carries its actual tool sequence. Delete the now-unused `anchors`
  field and its accumulation loop. `draft_frontmatter.description` still embeds
  the human arrow (`read → write → execute (seen 3× across 2 sessions)`) because
  it is computed from `a.tools` *before* the move. This is the change that makes
  the authored skill real rather than a stub.

- **`FeedbackRequest.meta: Option<serde_json::Value>`** (`vox-orchestrator`) —
  opaque per-item payload. For skill proposals it holds the serialized
  `Candidate`. `FeedbackStore::register` gains a 12th argument; the three
  existing call sites (`doubt.rs`, `propose.rs`, the `store.rs` test helper)
  pass `None`/the candidate accordingly.

- **`FeedbackAction::AcceptSkill`** (`vox-orchestrator`) — new internally-tagged
  variant serializing to `{"action":"accept_skill"}`. Carries no fields; the
  candidate lives on the feedback item's `meta`.

- **`McpFeedbackAction::AcceptSkill` + `From` arm** (`vox-orchestrator-mcp`,
  `params.rs`) — the MCP boundary has its *own* closed `#[serde(tag="action")]`
  enum that mirrors `FeedbackAction` and converts via `From`. Without this
  variant, `{action:"accept_skill"}` fails deserialization *before* reaching the
  handler. Both enums and the `From` impl must gain the variant.

- **`author_skill_md(name, description, steps) -> String`** (`vox-plugin-host`) —
  pure, type-agnostic function that composes a spec-valid `SKILL.md` from
  primitives. Lives next to `parse_skill_md` / `validate_skill_name` /
  `install_to_user_root`. Kebab-cases `name` so the top-level `name` always
  passes `validate_skill_name` (`[a-z0-9-]`, 1–64 chars, no leading/trailing or
  doubled hyphen). Primitive-typed on purpose: it avoids forcing a
  `vox-skill-discovery` dependency into `vox-plugin-host` (a layering inversion).

- **`author_and_install_skill(meta, ws_root) -> Result<Vec<String>, String>`**
  (`vox-orchestrator-mcp`) — free helper: deserialize `meta` into a `Candidate`,
  pull `name`/`description` from `draft_frontmatter` and `steps` from `members`,
  call `author_skill_md`, write to a unique temp dir as `skill/SKILL.md`, then
  `install_to_user_root(tmp, ws_root, global=false, None)`. Takes the `meta`
  value (not the whole `FeedbackRequest`) so it is unit-testable with just a
  tempdir — `ServerState::new_test()` sets `workspace_root: None`, so a
  `ServerState`-based test could not reach a skill root. This crate gains a
  `vox-skill-discovery` dependency to deserialize `Candidate`.

- **`vox_resolve_feedback` accept arm** (`vox-orchestrator-mcp`,
  `feedback_tools.rs`) — on `SkillProposal + AcceptSkill`, after the shared
  resolve, call `author_and_install_skill(req.meta, state.workspace_root)` and
  return the installed names. `resolve_feedback` returns `String` via
  `ToolResult::ok/err(...).to_json()` — **not** `Result`; the accept arm uses
  those builders, never `?`/`Ok`.

- **GUI "Save as skill" button** (`vox-gui`) — added to the existing
  `skill_proposal` branch of `FeedbackCard.tsx`, alongside Dismiss. Emits
  `onResolve(id, { action: 'accept_skill' })`. The transport
  (`feedbackResolve`, typed `Record<string, unknown>`) and the Tauri bridge are
  permissive pass-throughs, so **no `FeedbackRow`/`toRow` change is needed** —
  the candidate lives server-side on `meta`; the button is fire-and-forget.

### Authored SKILL.md shape

Minimal spec-valid file (TOML frontmatter, the dialect Vox first-party skills
use; `[metadata]` is optional but emitted for provenance):

```markdown
---
name = "read-write-execute"
description = "Recurring procedure: read → write → execute (seen 3× across 2 sessions)"

[metadata]
"vox-author" = "vox-skill-discovery"
"vox-category" = "workflow"
"vox-tags" = ["auto-discovered", "operations"]
---

# read-write-execute

Recurring procedure: read → write → execute (seen 3× across 2 sessions)

## Steps

1. `read`
2. `write`
3. `execute`
```

The `description` (which every skill lister shows, and which an agent reads when
choosing a skill) names the procedure. The `## Steps` enumerate the **real tool
sequence** — possible only because of the `members` fix above. The body is a
templated, valid enumeration, not LLM-polished prose; polishing is out of scope
(YAGNI) and the user can edit the installed file. With empty `members` the file
still authors validly with an empty Steps list.

## Error handling

`resolve_feedback` resolves the item *before* running side-effects (the shared
`state.feedback().resolve(...)` call). This is the existing architecture, shared
by the Doubt/Overrule path; SP-4 does not restructure it. Consequences:

- **Missing/garbage `meta`** → the accept arm returns
  `ToolResult::err("skill proposal has no candidate payload")`. The item is
  already resolved; the miner re-surfaces the candidate on its next run if the
  procedure still recurs.
- **Author/install failure** (bad name, IO) → the error from
  `install_to_user_root` is propagated as the tool-result error string. Item
  already resolved (same re-surfacing path).
- **Empty `members`** → authors a valid file with an empty Steps list; install
  succeeds.
- Name collisions, symlink targets, and path-escape names are already handled
  defensively inside `install_to_user_root` — SP-4 adds nothing there.
- **`workspace_root` is `None`** (no repo context) → accept returns
  `ToolResult::err("no workspace root; cannot install skill")`. In the GUI the
  root is always set; this guards headless callers.

## Testing (TDD)

- `vox-skill-discovery`: a mined `RepeatedOperations` candidate has
  `members == ["read","write","execute"]` (tool sequence), and its
  `description` still contains the arrow.
- `vox-orchestrator`: `FeedbackRequest` round-trips with `meta`; `AcceptSkill`
  serializes to `{"action":"accept_skill"}`.
- `vox-plugin-host`: `author_skill_md` output round-trips through
  `parse_skill_md`, the top-level name passes `validate_skill_name` (including a
  name needing kebab-casing), and the Steps list contains each input step.
- `vox-orchestrator-mcp` (unit): `McpFeedbackAction` deserializes
  `{"action":"accept_skill"}`; `author_and_install_skill(meta, tmp)` writes a
  `SKILL.md` under `tmp/.vox/skills/<name>/` and returns the name.
- `vox-gui` (vitest): clicking **Save as skill** calls
  `onResolve('F-9', { action: 'accept_skill' })`; Dismiss still emits `skip`.

## Out of scope (YAGNI)

- LLM-polished skill bodies (templated enumeration is enough for v1).
- Edit-before-save UI (user edits the installed file).
- Global (`~/.vox/skills`) install from a proposal — v1 installs
  workspace-local (`global=false`); promotion is a manual copy.
- Git-URL / remote skill sources from a proposal (local authoring only).
- Re-mining at accept time (we persist the candidate instead).
- Carrying captured argument *values* into Steps (only tool names are mined).
