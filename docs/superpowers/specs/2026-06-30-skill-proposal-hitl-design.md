---
title: HITL Skill Proposal Surface (skill-suggest sub-project 3)
date: 2026-06-30
status: design
audience: contributors
---

# HITL Skill Proposal Surface

## Context

**Sub-project 3 of 4** in the "agent-authored skills from repeated operations" program:

1. Operation capture → `agent_operations`. ✅ on `main`.
2. Sequence mining → ranked `Candidate`s (`mine_repeated_operations`, `vox skill suggest`). ✅ on `main`.
3. **HITL proposal (this spec)** — surface a mined candidate to the user as a
   non-blocking "this recurring procedure could be a skill" suggestion, reusing
   the existing feedback / NeedsYou infrastructure.
4. Accept → author SKILL.md → `install_to_user_root`. (future)

The decomposition assigns the *accept→author→install* action to sub-project 4.
Sub-project 3 is the **surface**: make the suggestion visible and capture the
user's decision. It reuses the orchestrator `FeedbackStore`, the `vox_feedback_list`
/ `vox_resolve_feedback` MCP tools, and the GUI `NeedsYou` panel almost entirely
as-is.

## Locked design decisions (flagged for review)

- **D1 — Mechanism, not auto-trigger.** Sub-project 3 provides the proposal
  *mechanism*: a producer + an MCP tool + GUI rendering. It does **not** decide
  *when* mining runs. A caller (an agent reading `vox skill suggest --format json`,
  or a future periodic/session-end hook) invokes the producer. Auto-triggering is
  explicitly deferred (it's the long-standing "auto-rerun" gap).
- **D2 — Dismissible advisory now; actionable Save in sub-project 4.** To avoid
  shipping a non-functional button, sub-project 3 surfaces the proposal with a
  single **Dismiss** action (awareness + decision capture). The actionable
  **"Save as skill"** (author a SKILL.md from the sequence + install) is
  sub-project 4, which extends this same proposal with the Save action + handler.
- **D3 — No new structured payload.** The human-readable proposal lives in the
  existing `prompt` String (skill name + procedure + "seen N× across M sessions").
  `FeedbackRequest` is NOT extended. Sub-project 4 decides how an accepted draft is
  reconstructed (re-derive via the miner, or add a payload field then).
- **D4 — Dedup.** The producer skips registering when an unresolved
  `SkillProposal` with the same `prompt` is already open (no nagging).

## Architecture

```
Candidate (from sub-project 2)
   │  vox_propose_skill { name, description, session_id? }   ← NEW MCP tool
   ▼
Orchestrator::propose_skill(name, description, session_id)   ← NEW producer (mirrors doubt_task)
   │  dedup vs open_needs_you()
   ▼
FeedbackStore::register(SkillProposal, prompt, ["Dismiss"], gates=[], …, Surface::NeedsYou)  ← REUSED
   + emit FeedbackRequested { kind: "skill_proposal", surface: "needs_you" }
   ▼
vox_feedback_list (REUSED) → GUI NeedsYouSurface → FeedbackCard ('skill_proposal' kind)
   ▼
user clicks Dismiss → vox_resolve_feedback (REUSED) → FeedbackAction::Skip
```

## Components

### 1. `vox-orchestrator` — kind + producer

- `crates/vox-orchestrator/src/feedback/types.rs`: add `SkillProposal` to
  `FeedbackKind` (fieldless — the enum is `Copy`; no payload).
- A producer on `Orchestrator` (new `crates/vox-orchestrator/src/orchestrator/agent/propose.rs`,
  or alongside `doubt.rs`): mirrors `doubt_task` (`doubt.rs:89-108`):

```rust
pub fn propose_skill(&self, name: &str, description: &str, session_id: Option<String>) -> Option<FeedbackId> {
    let prompt = format!("Recurring procedure '{name}': {description}. Consider saving it as a reusable skill.");
    // D4: dedup — skip if an identical unresolved proposal is already open.
    if self.feedback().open_needs_you().iter().any(|f| f.kind == FeedbackKind::SkillProposal && f.prompt == prompt) {
        return None;
    }
    let ts = /* now_ms */;
    let fid = self.feedback().register(
        FeedbackKind::SkillProposal,
        prompt,
        vec!["Dismiss".to_string()],        // D2: Save added in sub-project 4
        Vec::new(),                          // non-blocking (no gates)
        None, 0.0, 0,
        Surface::NeedsYou,
        session_id, None, ts,
    );
    self.event_bus.emit(AgentEventKind::FeedbackRequested {
        feedback_id: fid.0.clone(), kind: "skill_proposal".into(), gates: Vec::new(), surface: "needs_you".into(),
    });
    Some(fid)
}
```

> Verify the exact `register` arg order/types and `now_ms` idiom against
> `doubt.rs` during implementation.

### 2. `vox-orchestrator-mcp` — `vox_propose_skill` tool

Mirror the full `vox_doubt_task` registration surface:
- `params.rs`: `ProposeSkillParams { name: String, description: String, session_id: Option<String> }`.
- handler (in `feedback_tools.rs` or a new `propose_tools.rs`): calls
  `state.orchestrator.propose_skill(&p.name, &p.description, p.session_id)`,
  returns the feedback id (or "duplicate, skipped").
- dispatch arm (`dispatch.rs`), input schema (`input_schemas.rs` —
  `derived_tool_schema!(ProposeSkillParams)`), `DEFAULT_ALLOWED_TOOLS`
  (`http_gateway/mod.rs`), `contracts/operations/catalog.v1.yaml`
  (`mcp: { name: vox_propose_skill, http_read_role_eligible: false, tier: core }`),
  then regenerate `tool-registry.canonical.yaml` via `vox ci operations-sync`.

`vox_feedback_list` / `vox_resolve_feedback` are reused unchanged — a
`SkillProposal` serializes through them automatically.

### 3. GUI — render the new kind

- `crates/vox-gui/ui/src/transport.ts`: add `'skill_proposal'` to the
  `FeedbackRow.kind` union (`~:653`). `toRow` and `feedbackList`/`feedbackResolve`
  are kind-agnostic — no change.
- `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx`: add an
  explicit `row.kind === 'skill_proposal'` branch (props are `{ row, onResolve,
  onOpenContext }`; `onResolve(id, action)` takes an inline action literal). It
  renders `row.prompt` + a single "Dismiss" button → `onResolve(id, { action:
  'skip' })` (the GUI's `skip` maps to Rust `FeedbackAction::Skip`). A bare
  non-doubt kind would otherwise hit the options/Skip branch (messy), hence the
  dedicated branch. Sub-project 4 adds a "Save as skill" button here.
- `NeedsYouSurface.tsx` shows it automatically (lists all NeedsYou rows, kind-agnostic).
- An existing `__tests__/FeedbackCard.test.tsx` (renders the component directly with
  a `vi.fn()` `onResolve`, no transport mock) is extended with a skill_proposal case.

## Error handling

- Producer is best-effort: dedup returns `None` (no error); registration never
  blocks a task (empty `gates`).
- Resolving an already-resolved proposal is idempotent (existing `resolve` returns
  `None`).

## Testing

- **Producer (`vox-orchestrator`):** `propose_skill` registers a NeedsYou item with
  `kind == SkillProposal`; `open_needs_you()` includes it; a second identical call
  is deduped (returns `None`, still one open item).
- **MCP (`vox-integration-tests`):** `handle_tool_call("vox_propose_skill", …)` then
  `vox_feedback_list` shows the `skill_proposal` row; `vox_resolve_feedback` with a
  `Skip` action resolves it (drops from `open_needs_you`).
- **GUI (Vitest):** `FeedbackCard` renders a `skill_proposal` row (its prompt text +
  a Dismiss button); resolving calls `vox_resolve_feedback`.
- **ssot-drift:** catalog ↔ tool-registry consistent for `vox_propose_skill`.

## Out of scope (sub-project 4 and beyond)

- The **"Save as skill"** action: authoring a SKILL.md from the recurring sequence
  and installing it via `install_to_user_root` (sub-project 4 extends the same
  proposal with the Save button + a `resolve_feedback` handler for `SkillProposal`).
- Auto-triggering mining→proposal (session-end / periodic).
- Carrying a structured draft payload on the feedback item (added in 4 if needed).
