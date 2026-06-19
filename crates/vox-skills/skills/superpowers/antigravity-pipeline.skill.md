---
name: antigravity-pipeline
description: Use to run a full hardened Claude-Code->Gemini delegation loop - read the flywheel digest, author a verify-before-use launch statement, delegate with deterministic gate-checking via vox_agy_pipeline, run an automated adversarial review (code-reviewer agent) recorded via vox_agy_review, two-strike correct-and-fix, then stop at the human merge gate. One level above delegate-gemini (single-task primitive).
---

# Antigravity Pipeline (Claude Code <-> Gemini)

**Announce at start:** "I'm using the antigravity-pipeline skill to run the delegation loop."

You are the **architect + adversarial reviewer**; Gemini (via agy) is the **hands**. Merge is
ALWAYS human-gated — the pipeline never merges to main. In-repo sub-skills:
`crates/vox-skills/skills/superpowers/{brainstorming,writing-plans,executing-plans,
subagent-driven-development,test-driven-development,dispatching-parallel-agents,
requesting-code-review,verification-before-completion,delegate-gemini}.skill.md`.

## Stage 1 — Author
0. **Flywheel.** Call `vox_agy_ledger_digest`. Inject the top recurring failure categories as
   explicit "avoid this" rules in the launch statement; read section B of the handoff ledger
   (`docs/superpowers/antigravity-handoff-ledger.md`) for the matching lessons.
1. **Codebase audit.** Confirm EVERY symbol/path/API with rg/Grep; inline exact signatures.
2. **Targeted web research** only if needed (2-3 fetches).
3. **Plan-engineer** with writing-plans + section B + Gemini limitations section 5
   (`docs/src/architecture/gemini-3-5-flash-antigravity-limitations-2026-06-18.md`): atomic
   green-committed tasks, self-contained, one-decision-per-step, PARALLEL-SAFE/SEQUENTIAL.

## Stage 2 — Execute & verify
Call `vox_agy_pipeline` with `task` = the launch statement and `gates` scoped to the touched
crate, with env `CARGO_TARGET_DIR` pointing at the main target so cargo reuses cache:

```json
{
  "task": "Add pub fn parse_config(path:&Path)->Result<Config> to crates/vox-config/src/lib.rs (confirmed present) — no other files.",
  "gates": [
    {"name": "build", "program": "cargo", "args": ["build", "-p", "vox-config"], "env": {"CARGO_TARGET_DIR": "C:/Users/Owner/vox/target"}},
    {"name": "test",  "program": "cargo", "args": ["test",  "-p", "vox-config"], "env": {"CARGO_TARGET_DIR": "C:/Users/Owner/vox/target"}}
  ]
}
```

Pre-flight: if agy auth is unconfirmed, call `vox_agy_doctor` and follow its remediation.
The tool returns `outcome` (green/partial/failed), `gates`, a `spend_proxy`, the `diff`, and the
`ledger_id` (a provisional entry is already written).

## Stage 3-4 — Adversarial review, correct-and-fix, learn + merge
- **Adversarial review (automated).** Dispatch the `superpowers:code-reviewer` agent against the
  jailed diff with a template that hunts the known Gemini failures: hallucinated-api,
  hollow-green (tests assert shape not behavior), unplanned-shared-change, scope-creep,
  gate-weakening, effect-vs-shape. Prove the EFFECT (ledger B-9).
- **Record it.** Call `vox_agy_review` with `ledger_id` + `verdict` + section-B `categories` +
  `findings` + 1-3 `lessons`. This writes the `{id}-review` addendum the flywheel mines.
- **Two-strike.** If outcome != green OR verdict = request-changes: distill the failure into a
  corrected launch statement and re-delegate ONCE. Second failure -> STOP + hand off. Never loop.
- **Report** "to what extent implemented" + the ledger trail.

## Human merge gate (always)
Present the jailed `agy/<slug>` branch + report + review addendum; ask the human to approve the
merge to main.

## Safety invariants (do not weaken)
- Never run agy against the live tree; the worktree jail is the only sandbox.
- Never store Google credentials anywhere; agy owns its OAuth token.
- Gates run exactly as specified — never substitute --warn-only / || true / --no-verify.

### Prompt hygiene (REQUIRED)
A vague prompt is the single most common cause of agy file-writing failures
(ledger lesson B-6 / B-9). Before calling `vox_agy_pipeline`, verify your
`task` field satisfies ALL of the following:

1. **Single deliverable.** One file to create or one function to add. If you
   have more, split into separate pipeline calls or use `vox_agy_delegate_batch`.
2. **Confirmed symbols.** Every struct/function/path mentioned MUST have been
   verified with `rg`/Grep BEFORE inserting it into the prompt. Never name
   something you haven't confirmed exists.
3. **Explicit file path.** State the exact path (`crates/foo/src/bar.rs`),
   not "the relevant file".
4. **File-write verb.** Include "Create a file …" or "Add … to …". agy
   sometimes only reads when the task is ambiguous about writing.
5. **No git commands.** Append "Do not run any git commands." — agy sometimes
   stages or commits and the dirty worktree breaks the diff capture.
6. **Tight scope.** End with "— no other files." to prevent scope creep.
