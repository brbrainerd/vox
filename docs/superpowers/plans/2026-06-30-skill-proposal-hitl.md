# HITL Skill Proposal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Surface a mined skill `Candidate` to the user as a non-blocking "save this recurring procedure as a skill?" suggestion in the existing NeedsYou panel, capturing their decision.

**Architecture:** Add a `FeedbackKind::SkillProposal` variant + an `Orchestrator::propose_skill` producer (mirrors `doubt_task`, non-blocking), expose it via a new `vox_propose_skill` MCP tool, and render the kind in the GUI. Reuses `FeedbackStore`, `vox_feedback_list`/`vox_resolve_feedback`, and `NeedsYouSurface` as-is.

**Tech Stack:** Rust (`vox-orchestrator` feedback, `vox-orchestrator-mcp` MCP), React/TS (`vox-gui`), the `contracts/operations/catalog.v1.yaml` MCP SSOT.

**Spec:** `docs/superpowers/specs/2026-06-30-skill-proposal-hitl-design.md`

**Sub-project 3 of 4.** Surfaces + captures the decision. The actionable Save→author→install is sub-project 4 (see spec decision D2).

---

## Codebase facts — VERIFIED 2026-06-30

| Fact | Value |
|---|---|
| Producer template | `Orchestrator::doubt_task` (`vox-orchestrator/src/orchestrator/agent/doubt.rs:4-108`), `impl crate::orchestrator::Orchestrator`. `ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)`. |
| register | `self.feedback().register(kind, prompt: String, options: Vec<String>, gates: Vec<TaskId>, doubted_task_id: Option<TaskId>, info_gain_bits: f64, scaled_cost_ms: u64, surface: Surface, session_id: Option<String>, agent_id: Option<AgentId>, created_at_ms: u64) -> FeedbackId` (`feedback/store.rs:32-46`). |
| enums | `FeedbackKind { Clarification, Doubt }` + `Surface { NeedsYou, Withheld }` (`feedback/types.rs:7-19`); `FeedbackKind` is `Copy` — keep new variant fieldless. `FeedbackId(pub String)`. `open_needs_you()` (`store.rs:69`). |
| event | `self.event_bus.emit(AgentEventKind::FeedbackRequested { feedback_id: String, kind: String, gates: Vec<TaskId>, surface: String })` (`doubt.rs:102-108`). |
| MCP `vox_doubt_task` sites (mirror) | dispatch arm `dispatch.rs:499`; input schema `input_schemas.rs:69` (`derived_tool_schema!(crate::params::DoubtTaskParams)`); `DEFAULT_ALLOWED_TOOLS` `http_gateway/mod.rs:56`; catalog `contracts/operations/catalog.v1.yaml:~5945` (`mcp: {name, http_read_role_eligible:false, tier:core}`); registry `contracts/mcp/tool-registry.canonical.yaml:521`. |
| feedback list/resolve | `feedback_tools::feedback_list(state,_)` returns `{needs_you, withheld}` (`feedback_tools.rs:194`); `resolve_feedback` handles `FeedbackAction` (`types.rs:21-31`: `Answer{option,text}`, `Skip`, `Overrule`, `LetVerify`). `vox_feedback_list`/`vox_resolve_feedback` reused unchanged. |
| state access | `state.feedback()` (`server_state.rs:109`) returns the `FeedbackStore`; `state.orchestrator` is `Arc<Orchestrator>`. |
| GUI | `transport.ts` `FeedbackRow.kind` union `'clarification'|'doubt'` (`~:653`) — add `'skill_proposal'`; `FeedbackCard.tsx:13-60` branches on `kind==='doubt'` else renders `options` as Answer buttons; `NeedsYouSurface.tsx` lists all NeedsYou rows + polls 5s. |

## File Structure

- **Modify** `crates/vox-orchestrator/src/feedback/types.rs` — add `FeedbackKind::SkillProposal`.
- **Create** `crates/vox-orchestrator/src/orchestrator/agent/propose.rs` — `propose_skill` producer; register the module where `doubt.rs` is declared.
- **Modify** `crates/vox-orchestrator-mcp/src/params.rs` — `ProposeSkillParams`.
- **Create/Modify** `crates/vox-orchestrator-mcp/src/feedback_tools.rs` — `propose_skill` handler.
- **Modify** `crates/vox-orchestrator-mcp/src/{dispatch.rs, input_schemas.rs, http_gateway/mod.rs}` — register `vox_propose_skill`.
- **Modify** `contracts/operations/catalog.v1.yaml` (+ regenerate `tool-registry.canonical.yaml`).
- **Modify** `crates/vox-gui/ui/src/transport.ts` — kind union.
- **Modify** `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx` (+ `.test`) — render/verify the kind.

## Execution notes
- **TDD** for Task 1 (producer is pure-ish logic + a store). Tasks 2/3 are wiring verified by an integration test + a Vitest.
- Tasks 1→2→3 are sequential (2 depends on 1's producer; 3 depends on 2's tool). Commit per task.
- vox-orchestrator-mcp / catalog edits trigger the SSOT gate — regenerate + run `ssot-drift` (expect only the pre-existing `openclaw` gui-surface-registry drift, unrelated).

---

## Task 1: `FeedbackKind::SkillProposal` + producer

**Files:**
- Modify `crates/vox-orchestrator/src/feedback/types.rs`
- Create `crates/vox-orchestrator/src/orchestrator/agent/propose.rs`
- Modify the module list where `agent/doubt.rs` is declared (e.g. `orchestrator/agent/mod.rs` or `orchestrator/mod.rs` — grep `mod doubt;`)

- [ ] **Step 1: Add the variant**

In `crates/vox-orchestrator/src/feedback/types.rs`, add to `FeedbackKind`:

```rust
    SkillProposal,
```

(Keep it fieldless — `FeedbackKind` derives `Copy`.)

- [ ] **Step 2: Write the failing producer test**

VERIFIED: `Orchestrator::new(OrchestratorConfig::for_testing())` is synchronous (no DB/bootstrap); `feedback()`/`event_bus` are public, initialized unconditionally; near-exact precedent: `crates/vox-orchestrator/src/orchestrator/tests/doubt_feedback_projection.rs` (builds an Orchestrator, calls a producer, asserts on `feedback().open_needs_you()`). Add the test module to `crates/vox-orchestrator/src/orchestrator/agent/propose.rs`:

```rust
#[cfg(test)]
mod tests {
    use crate::config::OrchestratorConfig;
    use crate::feedback::FeedbackKind;
    use crate::orchestrator::Orchestrator;

    #[test]
    fn propose_skill_registers_needs_you_and_dedups() {
        let orch = Orchestrator::new(OrchestratorConfig::for_testing());
        let desc = "Recurring procedure: read → edit → run (seen 4× across 2 sessions)";
        let f1 = orch.propose_skill("read-edit-run", desc, Some("s1".into()));
        assert!(f1.is_some());
        let open = orch.feedback().open_needs_you();
        assert!(open.iter().any(|f| f.kind == FeedbackKind::SkillProposal));
        // Dedup: an identical proposal is skipped.
        let f2 = orch.propose_skill("read-edit-run", desc, Some("s1".into()));
        assert!(f2.is_none(), "duplicate proposal must be skipped");
        assert_eq!(
            orch.feedback()
                .open_needs_you()
                .iter()
                .filter(|f| f.kind == FeedbackKind::SkillProposal)
                .count(),
            1
        );
    }
}
```

> Confirm the exact module paths for `OrchestratorConfig` / `Orchestrator` against the precedent test's `use` lines (it uses `Orchestrator::new(OrchestratorConfig::for_testing())`).

- [ ] **Step 3: Run it to verify it fails**

Run: `cargo test -p vox-orchestrator propose_skill_registers_needs_you_and_dedups`
Expected: FAIL — `no method named propose_skill`.

- [ ] **Step 4: Implement the producer**

Insert above the test module in `propose.rs` (mirrors `doubt.rs:81-108`):

```rust
use crate::feedback::{FeedbackKind, Surface};
use crate::feedback::FeedbackId;

impl crate::orchestrator::Orchestrator {
    /// Surface a mined recurring procedure as a non-blocking "save as skill?"
    /// suggestion in the NeedsYou inbox. Deduped by prompt; returns the new
    /// feedback id, or `None` if an identical proposal is already open.
    pub fn propose_skill(
        &self,
        name: &str,
        description: &str,
        session_id: Option<String>,
    ) -> Option<FeedbackId> {
        let prompt = format!(
            "Recurring procedure '{name}': {description}. Consider saving it as a reusable skill."
        );
        if self
            .feedback()
            .open_needs_you()
            .iter()
            .any(|f| f.kind == FeedbackKind::SkillProposal && f.prompt == prompt)
        {
            return None;
        }
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let fid = self.feedback().register(
            FeedbackKind::SkillProposal,
            prompt,
            vec!["Dismiss".to_string()], // sub-project 4 adds "Save as skill"
            Vec::new(),                  // non-blocking: no gates
            None,
            0.0,
            0,
            Surface::NeedsYou,
            session_id,
            None,
            ts,
        );
        self.event_bus
            .emit(crate::events::AgentEventKind::FeedbackRequested {
                feedback_id: fid.0.clone(),
                kind: "skill_proposal".into(),
                gates: Vec::new(),
                surface: "needs_you".into(),
            });
        Some(fid)
    }
}
```

Register the module next to `mod doubt;` (e.g. add `mod propose;` in the same file — grep `mod doubt;` to find it). No `pub use` needed (the method is inherent on `Orchestrator`).

- [ ] **Step 5: Run the test**

Run: `cargo test -p vox-orchestrator propose_skill_registers_needs_you_and_dedups`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-orchestrator/src/feedback/types.rs crates/vox-orchestrator/src/orchestrator/agent/propose.rs crates/vox-orchestrator/src/orchestrator/agent/mod.rs
git commit -m "feat(orchestrator): SkillProposal feedback kind + propose_skill producer"
```

---

## Task 2: `vox_propose_skill` MCP tool

**Files:** `crates/vox-orchestrator-mcp/src/{params.rs, feedback_tools.rs, dispatch.rs, input_schemas.rs, http_gateway/mod.rs}`, `contracts/operations/catalog.v1.yaml`, `contracts/mcp/tool-registry.canonical.yaml`

- [ ] **Step 1: Params struct**

In `crates/vox-orchestrator-mcp/src/params.rs`, mirror `DoubtTaskParams` (VERIFIED: it derives `Debug, Deserialize, JsonSchema` + `#[schemars(deny_unknown_fields)]`):

```rust
/// Surface a mined recurring procedure as a non-blocking "save as skill?" proposal.
#[derive(Debug, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct ProposeSkillParams {
    /// Draft skill name (kebab, Agent Skills name rule).
    pub name: String,
    /// Human description of the recurring procedure.
    pub description: String,
    /// Optional originating session id.
    #[serde(default)]
    pub session_id: Option<String>,
}
```

(`Deserialize`/`JsonSchema` are already imported in `params.rs` — match the existing `use`.)

- [ ] **Step 2: Handler**

In `crates/vox-orchestrator-mcp/src/feedback_tools.rs`, add (mirroring `feedback_list`'s `(state, params)` shape and the `ToolResult` idiom used in that file):

```rust
pub async fn propose_skill(state: &ServerState, params: crate::params::ProposeSkillParams) -> String {
    match state
        .orchestrator
        .propose_skill(&params.name, &params.description, params.session_id)
    {
        Some(fid) => crate::params::ToolResult::ok(serde_json::json!({ "feedback_id": fid.0 })).to_json(),
        None => crate::params::ToolResult::ok(serde_json::json!({ "skipped": "duplicate proposal already open" })).to_json(),
    }
}
```

> Confirm the `ToolResult` import path used elsewhere in `feedback_tools.rs` and match it. `state.orchestrator` is `Arc<Orchestrator>`; `propose_skill` is the inherent method from Task 1.

- [ ] **Step 3: Dispatch + schema + allowlist**

- `dispatch.rs` (near line 499, beside `"vox_doubt_task"`):
  ```rust
  "vox_propose_skill" => Ok(crate::feedback_tools::propose_skill(state, serde_json::from_value(args)?).await),
  ```
- `input_schemas.rs` (near line 69):
  ```rust
  "vox_propose_skill" => derived_tool_schema!(crate::params::ProposeSkillParams),
  ```
- `http_gateway/mod.rs` `DEFAULT_ALLOWED_TOOLS` (near line 56): add `"vox_propose_skill",`.

- [ ] **Step 4: Catalog SSOT + regenerate**

In `contracts/operations/catalog.v1.yaml`, the `vox_doubt_task` entry is `id: doubt.task` (verbatim verified). Add a sibling entry. IMPORTANT: entries are ordered alphabetically by `id` within the block — place `id: propose.skill` in alphabetical position (after `p…` siblings, e.g. near other `propose.`/`p*` ids), not next to `doubt.task`:

```yaml
- id: propose.skill
  title: Propose Skill
  description: Surface a mined recurring operation procedure as a non-blocking "save as skill?" suggestion in the NeedsYou inbox.
  description_human: null
  product_lane: ai
  intent_tags: []
  side_effect_class: null
  scope_kind: null
  reversible: null
  requires_repo: null
  preferred_for_models: null
  human_takeover_friendly: null
  mens_planner_visible: null
  canonical_name: null
  latin_aliases: null
  mcp:
    name: vox_propose_skill
    http_read_role_eligible: false
    tier: core
  cli: null
```

Then regenerate (satisfies `verify_derived_registry_artifacts`): `vox ci operations-sync --target all --write`. ORDERING: the dispatch arm (Step 3) + input-schema arm (Step 3) must already exist — `operations-verify` requires the literal `"vox_propose_skill"` in both `dispatch.rs` and `input_schemas.rs` for every catalog `mcp:` row.

- [ ] **Step 5: Integration test**

Create `crates/vox-integration-tests/tests/skill_proposal_test.rs` (mirror `skill_install_test.rs`'s `ServerState::new_full` + `handle_tool_call as tools` harness):

```rust
#![allow(missing_docs)]
use vox_orchestrator::OrchestratorConfig;
use vox_orchestrator_mcp::{ServerState, handle_tool_call as tools};

#[tokio::test]
async fn propose_skill_surfaces_in_feedback_list() {
    let state = ServerState::new_full(OrchestratorConfig::default());
    let req = serde_json::json!({ "name": "read-edit-run", "description": "read → edit → run (seen 4×)" });
    let resp = tools(&state, "vox_propose_skill", req).await.unwrap();
    assert!(resp.contains("feedback_id"), "got: {resp}");

    let list = tools(&state, "vox_feedback_list", serde_json::json!({})).await.unwrap();
    assert!(list.contains("skill_proposal"), "proposal must appear in needs_you: {list}");
}
```

- [ ] **Step 6: Build + test + drift**

Run: `cargo test -p vox-integration-tests propose_skill_surfaces_in_feedback_list`
then `cargo test -p vox-mcp-registry` (tool-registry parity)
then `vox ci ssot-drift` (expect clean except the pre-existing `openclaw` gui-surface-registry gap).
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vox-orchestrator-mcp/src contracts/ crates/vox-integration-tests/tests/skill_proposal_test.rs
git commit -m "feat(mcp): vox_propose_skill — raise a non-blocking skill proposal"
```

---

## Task 3: GUI renders the proposal kind

**Files:** `crates/vox-gui/ui/src/transport.ts`, `crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx` (+ `FeedbackCard.test.tsx`, create if absent)

- [ ] **Step 1: Extend the kind union**

VERIFIED `FeedbackRow` (`transport.ts:651-660`) has 8 fields and `kind: 'clarification' | 'doubt'`. Change the union to add the new kind:

```ts
  kind: 'clarification' | 'doubt' | 'skill_proposal';
```

`toRow`, `feedbackList`, `feedbackResolve` are kind-agnostic — no change. (`normalizeFeedback`'s sort only pins `doubt`; `skill_proposal` sorts by `infoGainBits` like clarification — fine.)

- [ ] **Step 2: Add an explicit `skill_proposal` branch in `FeedbackCard`**

VERIFIED signature (`FeedbackCard.tsx:7-13`): `function FeedbackCard({ row, onResolve, onOpenContext }: Props)` where `onResolve: (id: string, action: Record<string, any>) => void`. A bare non-doubt kind falls into the options/Skip branch (messy for a proposal), so add a dedicated branch. After the `const isDoubt = row.kind === 'doubt';` line, render a clean proposal card when `row.kind === 'skill_proposal'` — its prompt + one "Dismiss" button that resolves via the existing `skip` action (maps to Rust `FeedbackAction::Skip`):

```tsx
  if (row.kind === 'skill_proposal') {
    return (
      <div className="feedback-card feedback-card--proposal">
        <p className="feedback-card__prompt">{row.prompt}</p>
        <div className="feedback-card__actions">
          <button type="button" onClick={() => onResolve(row.feedbackId, { action: 'skip' })}>
            Dismiss
          </button>
        </div>
      </div>
    );
  }
```

> Match the existing card's className/markup idiom (copy from the `isDoubt` branch's JSX). Sub-project 4 will add a "Save as skill" button alongside Dismiss here.

- [ ] **Step 3: Extend the EXISTING card test**

VERIFIED a test already exists at `crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx`. It renders `FeedbackCard` directly with a literal `row` + a `vi.fn()` `onResolve` (NO transport/invoke mock). Add a case (literal row must supply all 8 `FeedbackRow` fields, `as const` on `kind`/`surface`):

```tsx
it('renders a skill_proposal with a Dismiss action', () => {
  const row = {
    feedbackId: 'F-9',
    kind: 'skill_proposal' as const,
    prompt: "Recurring procedure 'read-edit-run': read → edit → run. Consider saving it as a reusable skill.",
    options: ['Dismiss'],
    gates: [],
    doubtedTaskId: null,
    surface: 'needs_you' as const,
    infoGainBits: 0,
  };
  const onResolve = vi.fn();
  render(<FeedbackCard row={row} onResolve={onResolve} onOpenContext={() => {}} />);
  expect(screen.getByText(/Recurring procedure/)).toBeTruthy();
  fireEvent.click(screen.getByText('Dismiss'));
  expect(onResolve).toHaveBeenCalledWith('F-9', { action: 'skip' });
});
```

- [ ] **Step 4: Run + commit**

Run (from `crates/vox-gui/ui`): `pnpm vitest run src/components/surfaces/NeedsYou`
Expected: PASS (existing cases + the new one).

```bash
git add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/components/surfaces/NeedsYou/FeedbackCard.tsx crates/vox-gui/ui/src/components/surfaces/NeedsYou/__tests__/FeedbackCard.test.tsx
git commit -m "feat(gui): render skill_proposal kind in the NeedsYou inbox"
```

---

## Final verification

- [ ] **Step 1: Rust**

Run: `cargo test -p vox-orchestrator propose_skill -p vox-integration-tests propose_skill -p vox-mcp-registry`
Expected: PASS.

- [ ] **Step 2: GUI**

Run (from `crates/vox-gui/ui`): `pnpm vitest run src/components/surfaces/NeedsYou`
Expected: PASS.

- [ ] **Step 3: SSOT + fmt + clippy**

Run: `vox ci ssot-drift` (clean except pre-existing `openclaw`), `cargo fmt -p vox-orchestrator -p vox-orchestrator-mcp`, `cargo clippy -p vox-orchestrator-mcp -- -D warnings` (note the pre-existing `vox-telemetry` `collapsible_if` lint is unrelated; if it blocks, clippy a leaf crate you touched instead).
