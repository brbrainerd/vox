---
title: "Skill Proposal — Accept → Author → Install (Sub-Project 4)"
status: design
date: 2026-06-30
audience: contributors
program: agent-authored-skills-from-repeated-operations
---

# Skill Proposal: Accept → Author → Install (SP-4)

## Context

This is sub-project 4 of *agent-authored skills from repeated operations*:

1. **Operation capture** — ✅ on main.
2. **Sequence mining** (`vox-skill-discovery` produces `Candidate`s) — ✅ on main.
3. **HITL proposal surface** — ✅ on main. A mined procedure becomes a
   `FeedbackKind::SkillProposal` item in the NeedsYou inbox with a single
   **Dismiss** action.
4. **Accept → author → install** — *this spec*. The user clicks **Save as
   skill** on a proposal; the system composes a valid `SKILL.md` from the
   mined `Candidate` and installs it into the user skill root via the existing
   `install_to_user_root`.

SP-3 (decision D3) explicitly deferred *how an accepted draft is reconstructed*
to SP-4. This spec resolves it: **store the `Candidate` JSON on the feedback
item at propose time, and reconstruct from it at accept time.** Re-mining at
accept time was rejected — operations age out of the capture buffer, so the
candidate may not reproduce.

## Correction to SP-3 scope notes

The SP-3 hand-off claimed `FeedbackRequest` already had a reusable
`meta: Option<Value>` field. It does **not** — that field belongs to the
`ToolResult` envelope, a different struct. SP-4 therefore *adds* a
`meta: Option<serde_json::Value>` field to `FeedbackRequest` and threads it
through `FeedbackStore::register`. This is the sanctioned mechanism, not a
reuse.

## Goal

One sentence: accepting a skill proposal writes a valid `SKILL.md` derived from
the mined candidate and installs it under the user skill root, surfacing the
installed skill name back to the user.

## Architecture

Data flows proposal → storage → accept → authored file → install:

```
miner Candidate ──(JSON)──► vox_propose_skill(candidate)
                                   │
                                   ▼
        Orchestrator::propose_skill(name, desc, session, meta=candidate_json)
                                   │  stores on FeedbackRequest.meta
                                   ▼
                      NeedsYou inbox  ──GUI──►  [Save as skill] button
                                                       │  {action:"accept_skill"}
                                                       ▼
        vox_resolve_feedback: SkillProposal + AcceptSkill
                                   │  read meta → Candidate
                                   │  author_skill_md(name,desc,category,tags,members)
                                   │  write SKILL.md to tempdir
                                   ▼
                  install_to_user_root(tempdir, ws_root, global=true)
                                   │
                                   ▼
                       installed skill names → tool result
```

### Components

- **`FeedbackRequest.meta: Option<serde_json::Value>`** (`vox-orchestrator`) —
  opaque per-item payload. For skill proposals it holds the serialized
  `Candidate`. `FeedbackStore::register` gains a 12th argument; non-proposal
  call sites pass `None`.

- **`FeedbackAction::AcceptSkill`** (`vox-orchestrator`) — new internally-tagged
  variant (`{"action":"accept_skill"}`). Carries no fields; the candidate lives
  on the feedback item's `meta`.

- **`author_skill_md(...)`** (`vox-plugin-host`) — pure, type-agnostic function
  that composes a spec-valid `SKILL.md` string from primitive inputs (name,
  description, category, tags, members). Lives next to `parse_skill_md` /
  `validate_skill_name` / `install_to_user_root`. Kebab-cases the name so the
  top-level `name` always passes `validate_skill_name` (`[a-z0-9-]`, 1–64 chars).
  Keeping it primitive-typed avoids forcing a `vox-skill-discovery` dependency
  into `vox-plugin-host` (a layering inversion).

- **`vox_resolve_feedback` accept arm** (`vox-orchestrator-mcp`) — on
  `SkillProposal + AcceptSkill`: deserialize `meta` into a `Candidate`, call
  `author_skill_md`, write the result to a fresh tempdir as
  `<name>/SKILL.md`, then `install_to_user_root`. Returns the installed names.
  This crate gains a `vox-skill-discovery` dependency to deserialize `Candidate`.

- **GUI "Save as skill" button** (`vox-gui`) — added to the existing
  `skill_proposal` branch of `FeedbackCard.tsx`, alongside Dismiss. Emits
  `onResolve(id, { action: 'accept_skill' })`.

### Authored SKILL.md shape

Minimal spec-valid file (TOML frontmatter, the dialect Vox first-party skills
use; the `[metadata]` block is optional but we emit it for provenance):

```markdown
---
name = "read-edit-run"
description = "read → edit → run (seen 4× across 2 sessions)"

[metadata]
"vox-id" = "vox.skill.read-edit-run"
"vox-version" = "0.1.0"
"vox-author" = "vox-skill-discovery"
"vox-category" = "custom"
"vox-tags" = ["mined"]
---

# read-edit-run

Recurring procedure mined from repeated operations.

## Steps

1. `members[0]`
2. `members[1]`
...
```

The body is a **templated** enumeration of the mined operation members — real
and valid, not LLM-polished prose. Polishing is out of scope (YAGNI); the user
can edit the installed file. No stub: the file parses, installs, and lists the
actual mined steps.

## Error handling

- **Missing/garbage `meta`** → resolve returns an error result
  (`"skill proposal has no candidate payload"`); the feedback item is *not*
  marked resolved, so the user can still Dismiss.
- **Author/install failure** (bad name, IO) → error surfaced from
  `install_to_user_root` is propagated as the tool-result error; item stays open.
- **Empty `members`** → still authors a valid file with an empty Steps list and
  the description; install succeeds.
- Name collisions, symlink targets, path-escape names are already handled
  defensively inside `install_to_user_root` — SP-4 adds nothing there.

## Testing

- `vox-orchestrator`: `FeedbackRequest` round-trips with `meta`; `AcceptSkill`
  serializes to `{"action":"accept_skill"}`.
- `vox-plugin-host`: `author_skill_md` output round-trips through
  `parse_skill_md` and the top-level name passes `validate_skill_name`
  (including a name needing kebab-casing).
- `vox-orchestrator-mcp` (integration): register a `SkillProposal` whose `meta`
  is a serialized `Candidate`, resolve with `AcceptSkill`, assert a `SKILL.md`
  exists under the user skill root with the expected name.
- `vox-gui` (vitest): clicking **Save as skill** calls
  `onResolve('F-9', { action: 'accept_skill' })`; Dismiss still emits `skip`.

## Out of scope (YAGNI)

- LLM-polished skill bodies (templated enumeration is enough for v1).
- Edit-before-save UI (user edits the installed file).
- Git-URL / remote skill sources from a proposal (local authoring only).
- Re-mining at accept time (we persist the candidate instead).
