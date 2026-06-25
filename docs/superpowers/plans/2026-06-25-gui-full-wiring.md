# GUI Full-Wiring Implementation Plan (Phase 3 revision)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace every HIDE decision in the GUI honesty triage with a real, end-to-end WIRE — so that Dashboard Doubt/Overrule, the Chat context-window meter, SkillsPlugins info panels, Policies enable/disable+edit, and Settings keybinds all show **real, fully-wired content** rather than being hidden or faked.

**Architecture:** Five independent subsystems (A–E). A and B are GUI-only wires to backends that already exist. C adds one DB-sourced field. D adds a runtime policy-override store + two mutation commands. E builds a data-driven keybinding system (action registry → dispatcher → editable UI) on top of the existing `user_preferences` persistence. Each subsystem is self-contained and TDD'd; they touch disjoint files except for shared `App.tsx`/`transport.ts`, which are edited serially.

**Tech Stack:** React 19 + TypeScript 5 (vite, vitest, RTL) for the UI; Rust (Tauri commands, `vox-config`, `vox-db`) for the backend. No ESLint. No new deps.

**Spec:** `docs/superpowers/specs/2026-06-25-gui-honesty-audit-design.md`. **Supersedes:** the four HIDE rows in `docs/agents/gui-honesty-triage.md` (Dashboard Doubt, Dashboard Overrule, Chat ContextWindowMeter, Settings Keybinds) — all become WIRE. Run **after** Phase 3 Task 3.5 (toast overhaul) so toasts carry a typed `cause`.

---

## File Structure

**Subsystem A — SkillsPlugins detail panel (GUI-only)**
- Create: `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillDetailPanel.tsx` — structured detail view (mirrors PoliciesView left-rail/right-panel pattern).
- Create: `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillDetailPanel.test.tsx`
- Modify: `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillsPluginsView.tsx` — replace toast dumps with panel selection; give search-hit rows a real Install action; fix the catalog install call.

**Subsystem B — Dashboard Doubt/Overrule (GUI wire, backend exists)**
- Modify: `crates/vox-gui/ui/src/types/dashboard.ts` — add optional `taskId?: number` to `StreamItem`.
- Modify: `crates/vox-gui/ui/src/lib/mapAgentEvent.ts` — populate `taskId` from the event frame.
- Modify: `crates/vox-gui/ui/src/transport.ts` — add `doubtTask`/`overruleTask` wrappers.
- Modify: `crates/vox-gui/ui/src/App.tsx` — add `handleDoubt`/`handleOverrule`, thread into `surfaceProps`.
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` — pass `onDoubt`/`onOverrule` to Dashboard (if not already).
- Test: `crates/vox-gui/ui/src/components/surfaces/Dashboard/StreamCard.test.tsx` (update), `App.test.tsx` (handler).

**Subsystem C — ContextWindowMeter real tokens (Rust + GUI)**
- Modify: `crates/vox-gui/src-tauri/src/orchestrator.rs` — add `used_tokens: usize` to `ContextBudgetPayload`, sourced from `model_calls`.
- Modify: `crates/vox-gui/ui/src/transport.ts` — extend `ContextBudgetPayload` TS type with `used_tokens`.
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx` — `usedTokens={budget.used_tokens}`.
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.test.tsx`.

**Subsystem D — Policies enable/disable + edit (Rust + GUI)**
- Create: `crates/vox-config/src/policy/overrides.rs` — `.vox/policy-overrides.json` read/write (`{id: {enabled: bool}}`).
- Modify: `crates/vox-config/src/policy/mod.rs` — export overrides.
- Modify: `crates/vox-gui/src/commands/policy.rs` — `policy_set_enabled`, `policy_edit`; add `enabled` to row/detail DTOs.
- Modify: `crates/vox-gui/src/main.rs` — register the two commands.
- Modify: `crates/vox-gui/ui/src/transport.ts` — `policySetEnabled`, `policyEdit` wrappers; extend DTO types.
- Modify: `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx` — un-stub Disable/Edit, wire onClick + edit form.
- Tests: `overrides.rs` unit test; `PoliciesView.test.tsx`.

**Subsystem E — Editable keybindings (GUI-only, persistence exists)**
- Create: `crates/vox-gui/ui/src/lib/keybinds.ts` — `ACTION_REGISTRY` (actionId → label + default chord) + parse/match/serialize helpers.
- Create: `crates/vox-gui/ui/src/lib/keybinds.test.ts`
- Create: `crates/vox-gui/ui/src/hooks/useKeybinds.ts` — data-driven dispatcher hook (loads from `getGuiPreference('gui.keybinds')`, matches events, fires actions).
- Modify: `crates/vox-gui/ui/src/App.tsx` — replace the ad-hoc keydown `useEffect` with `useKeybinds(actionHandlers)`.
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx` — replace static `KEYBINDS` array with an editable rebinding list backed by the registry + persistence.
- Test: `keybinds.test.ts`, `SettingsView.test.tsx` (keybinds section).

---

## Subsystem A — SkillsPlugins detail panel

Backend already returns structured data (`vox_skill_info`, `vox_skill_use`, `vox_plugin_info`). The fix is GUI-only: render in a panel instead of a raw-JSON toast, and give marketplace search rows a real Install action (with the correct backend call).

### Task A1: SkillDetailPanel component (TDD)

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillDetailPanel.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillDetailPanel.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
// SkillDetailPanel.test.tsx
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { SkillDetailPanel } from './SkillDetailPanel';

describe('SkillDetailPanel', () => {
  it('renders skill info fields (no raw JSON)', () => {
    render(<SkillDetailPanel detail={{
      kind: 'skill-info',
      id: 'brainstorm', name: 'Brainstorm', version: '1.0.0',
      category: 'process', description: 'Generate ideas',
      tools: ['t1'], source: 'bundle', permissions: [], tags: ['ideation'],
    }} />);
    expect(screen.getByText('Brainstorm')).toBeInTheDocument();
    expect(screen.getByText('Generate ideas')).toBeInTheDocument();
    expect(screen.getByText('ideation')).toBeInTheDocument();
    expect(screen.queryByText(/^\{/)).not.toBeInTheDocument(); // no raw JSON blob
  });

  it('renders skill-use markdown body', () => {
    render(<SkillDetailPanel detail={{
      kind: 'skill-use', name: 'Brainstorm', description: 'd',
      body: '# Heading\nbody text',
    }} />);
    expect(screen.getByText('Brainstorm')).toBeInTheDocument();
    expect(screen.getByText(/body text/)).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run it, verify it fails**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/SkillsPlugins/SkillDetailPanel.test.tsx`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement the panel**

```tsx
// SkillDetailPanel.tsx
export type SkillDetail =
  | {
      kind: 'skill-info'; id: string; name: string; version: string;
      category: string; description: string; tools: string[];
      source: string; permissions: string[]; tags: string[];
    }
  | { kind: 'skill-use'; name: string; description: string; body: string }
  | {
      kind: 'plugin-info'; id: string; name: string; version: string;
      description: string; author?: string; homepage?: string;
      tools: string[]; permissions: string[];
    };

function Field({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex justify-between gap-4 text-[12px]">
      <span className="text-text-muted">{label}</span>
      <span className="text-text-secondary text-right">{value}</span>
    </div>
  );
}

export function SkillDetailPanel({ detail }: { detail: SkillDetail }) {
  return (
    <div className="flex-1 overflow-y-auto p-4">
      <h2 className="font-display text-[18px] font-semibold text-text-primary">{detail.name}</h2>
      <p className="mt-0.5 text-[12px] text-text-secondary">{detail.description}</p>
      <div className="mt-4 flex flex-col gap-1.5">
        {detail.kind === 'skill-info' && (
          <>
            <Field label="Version" value={detail.version} />
            <Field label="Category" value={detail.category} />
            <Field label="Source" value={detail.source} />
            {detail.tools.length > 0 && <Field label="Tools" value={detail.tools.join(', ')} />}
            {detail.permissions.length > 0 && <Field label="Permissions" value={detail.permissions.join(', ')} />}
            {detail.tags.length > 0 && (
              <div className="mt-1 flex flex-wrap gap-1">
                {detail.tags.map(t => (
                  <span key={t} className="rounded-full border border-ds-border px-2 py-0.5 text-[10px] text-text-secondary">{t}</span>
                ))}
              </div>
            )}
          </>
        )}
        {detail.kind === 'plugin-info' && (
          <>
            <Field label="Version" value={detail.version} />
            {detail.author && <Field label="Author" value={detail.author} />}
            {detail.homepage && <Field label="Homepage" value={detail.homepage} />}
            {detail.tools.length > 0 && <Field label="Tools" value={detail.tools.join(', ')} />}
          </>
        )}
        {detail.kind === 'skill-use' && (
          <pre className="mt-2 whitespace-pre-wrap text-[12px] text-text-secondary">{detail.body}</pre>
        )}
      </div>
    </div>
  );
}
```

(`pre` keeps it dependency-free — no markdown lib. The body is already human-readable SKILL.md text.)

- [ ] **Step 4: Run tests, verify pass**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/SkillsPlugins/SkillDetailPanel.test.tsx`
Expected: PASS (2 tests).

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillDetailPanel.tsx crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillDetailPanel.test.tsx
git commit -m "feat(gui-skills): structured SkillDetailPanel (replaces raw-JSON toast)"
```

### Task A2: Wire the panel into SkillsPluginsView + fix install call

**Files:** Modify `crates/vox-gui/ui/src/components/surfaces/SkillsPlugins/SkillsPluginsView.tsx`

- [ ] **Step 1: Read the current view.** Confirm: the inline `callTool`, the `showInfo` toast-dump helper, the `onSkillInfo`/`onSkillUse`/`onPluginInfo` handlers, the search-hit rows at ~line 346 with `actions={[]}`, and the catalog Install action that calls `vox_skill_install { id }`.

- [ ] **Step 2: Write a failing test** asserting the panel shows instead of a toast.

```tsx
// add to an existing SkillsPluginsView.test.tsx, or create one
it('shows a detail panel (not a toast) when Info is clicked', async () => {
  // render the view with a mocked invoke returning a SkillInfo,
  // click the installed-skill Info button, assert the skill name
  // appears in a panel region and pushToast was NOT called with a JSON body.
});
```

(Mirror the mocking style already used in the surface's sibling tests — read one first.)

- [ ] **Step 3: Replace toast dumps with panel state.** Add `const [detail, setDetail] = useState<SkillDetail | null>(null);`. Change handlers to set typed detail instead of `pushToast`:

```tsx
const onSkillInfo = async (id: string) => {
  const res = await callTool('vox_skill_info', { id });
  setDetail({ kind: 'skill-info', ...(unwrap(res?.result) as Omit<Extract<SkillDetail,{kind:'skill-info'}>,'kind'>) });
};
const onSkillUse = async (id: string) => {
  const res = await callTool('vox_skill_use', { id });
  setDetail({ kind: 'skill-use', ...(unwrap(res?.result) as Omit<Extract<SkillDetail,{kind:'skill-use'}>,'kind'>) });
};
const onPluginInfo = async (id: string) => {
  const res = await callTool('vox_plugin_info', { id });
  setDetail({ kind: 'plugin-info', ...(unwrap(res?.result) as Omit<Extract<SkillDetail,{kind:'plugin-info'}>,'kind'>) });
};
```

Render `{detail && <SkillDetailPanel detail={detail} />}` in a right-hand panel region (wrap the existing list in a `flex` row with the panel beside it, mirroring PoliciesView).

- [ ] **Step 4: Give search-hit rows a real action + fix the catalog install call.** At the `actions={[]}` site (~line 346), change to:

```tsx
actions={[
  { label: 'View', onClick: () => onSkillUse(s.id) },
  { label: 'Install', onClick: () => onInstallPlugin(s.id) },
]}
```

And fix install: marketplace/catalog install must call `vox_plugin_install { id }`, NOT `vox_skill_install { bundle_json }`:

```tsx
const onInstallPlugin = async (id: string) => {
  await callTool('vox_plugin_install', { id });
  pushToast({ tone: 'ok', title: 'Installed', body: id, cause: 'backend-ok' });
};
```

(Read the existing catalog-row Install handler and repoint it to `onInstallPlugin` too, so there is one correct install path.)

- [ ] **Step 5: Run tests + typecheck**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/SkillsPlugins && pnpm typecheck`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/SkillsPlugins
git commit -m "feat(gui-skills): render info in detail panel; fix catalog install to vox_plugin_install"
```

---

## Subsystem B — Dashboard Doubt/Overrule

The Tauri commands `doubt_orchestrator_task(taskId, reason)` and `overrule_orchestrator_task(taskId, reason)` already exist and are registered. The only gap is the GUI: `StreamItem` carries no numeric task id, and `App.tsx` passes no `onDoubt`/`onOverrule`.

### Task B1: Carry `taskId` on StreamItem (TDD)

**Files:**
- Modify: `crates/vox-gui/ui/src/types/dashboard.ts`
- Modify: `crates/vox-gui/ui/src/lib/mapAgentEvent.ts`
- Test: `crates/vox-gui/ui/src/lib/mapAgentEvent.test.ts` (create if absent)

- [ ] **Step 1: Write the failing test.** Read `mapAgentEvent.ts` and its `AgentEventFrame` input type first to find the field that holds the numeric task id (e.g. `task_id`/`taskId`). Then:

```ts
// mapAgentEvent.test.ts
import { describe, it, expect } from 'vitest';
import { mapAgentEvent } from './mapAgentEvent';

it('carries the numeric taskId from the event frame', () => {
  const frame = /* a minimal AgentEventFrame for a task event with task_id 42 */;
  const item = mapAgentEvent(frame as any);
  expect(item.taskId).toBe(42);
});
```

- [ ] **Step 2: Run it, verify it fails** (`taskId` not on the type / undefined).

Run: `cd crates/vox-gui/ui && pnpm vitest run src/lib/mapAgentEvent.test.ts`

- [ ] **Step 3: Add the field + populate it.**

In `types/dashboard.ts`, add to `StreamItem`:
```ts
  taskId?: number; // numeric orchestrator TaskId, when this stream item is a task event
```
In `mapAgentEvent.ts`, set `taskId` from the frame's task-id field (use the exact field name found in Step 1; coerce to number; leave undefined for non-task events).

- [ ] **Step 4: Run tests, verify pass.**

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/types/dashboard.ts crates/vox-gui/ui/src/lib/mapAgentEvent.ts crates/vox-gui/ui/src/lib/mapAgentEvent.test.ts
git commit -m "feat(gui-dashboard): carry numeric taskId on StreamItem for doubt/overrule"
```

### Task B2: transport wrappers for doubt/overrule

**Files:** Modify `crates/vox-gui/ui/src/transport.ts`

- [ ] **Step 1: Read the existing `interrupt`/`pause` transport wrappers** to copy the exact `invoke` style and casing.

- [ ] **Step 2: Add the wrappers** (params are camelCase; Tauri maps to snake_case Rust args):

```ts
doubtTask(taskId: number, reason?: string): Promise<unknown> {
  return invoke('doubt_orchestrator_task', { taskId, reason: reason ?? null });
},
overruleTask(taskId: number, reason: string): Promise<unknown> {
  return invoke('overrule_orchestrator_task', { taskId, reason });
},
```

- [ ] **Step 3: Typecheck.** Run: `cd crates/vox-gui/ui && pnpm typecheck`. Expected: green.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/transport.ts
git commit -m "feat(gui-transport): doubtTask/overruleTask wrappers for existing Tauri commands"
```

### Task B3: App.tsx handlers + thread into surfaceProps (TDD)

**Files:**
- Modify: `crates/vox-gui/ui/src/App.tsx`
- Modify: `crates/vox-gui/ui/src/components/layout/surfaceComponents.tsx` (only if it does not already forward `onDoubt`/`onOverrule`)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Dashboard/StreamCard.tsx` + its test (the StreamCard test currently asserts the controls are absent — update it to assert they fire when a `taskId` is present)

- [ ] **Step 1: Read** `App.tsx`'s `surfaceProps` object (~lines 1043–1086) and an existing handler like `handlePause`/`handleAckLudus` to copy the toast+error pattern. Read `StreamCard.tsx` and its test to see the current button gating.

- [ ] **Step 2: Write the failing test** in `StreamCard.test.tsx`:

```tsx
it('calls onDoubt with the item when Doubt is clicked', () => {
  const onDoubt = vi.fn();
  const item = { id: 'x', kind: 'in-progress', tag: 'TASK', title: 't', body: 'b', ts: '', taskId: 7 };
  render(<StreamCard item={item as any} onDoubt={onDoubt} onOverrule={vi.fn()} />);
  fireEvent.click(screen.getByRole('button', { name: /doubt/i }));
  expect(onDoubt).toHaveBeenCalledWith(item);
});
it('does not render doubt/overrule when item has no taskId', () => {
  const item = { id: 'x', kind: 'in-progress', tag: 'TASK', title: 't', body: 'b', ts: '' };
  render(<StreamCard item={item as any} onDoubt={vi.fn()} onOverrule={vi.fn()} />);
  expect(screen.queryByRole('button', { name: /doubt/i })).not.toBeInTheDocument();
});
```

- [ ] **Step 3: Run it, verify it fails.**

- [ ] **Step 4: Implement.**

In `StreamCard.tsx`, gate the Doubt/Overrule controls on `item.taskId != null` (only task events can be doubted/overruled); keep the existing kind-based visibility (Doubt when `kind !== 'doubted'`, Overrule when `kind === 'doubted'`). Buttons call `onDoubt?.(item)` / `onOverrule?.(item)`.

In `App.tsx`, add:
```tsx
const handleDoubt = useCallback((item: StreamItem) => {
  if (item.taskId == null) return;
  voxTransport.doubtTask(item.taskId)
    .then(() => pushToast({ tone: 'ok', title: 'Doubt cast', body: item.title, cause: 'backend-ok' }))
    .catch((e) => pushToast({ tone: 'warn', title: 'Doubt failed', body: String(e), cause: 'backend-error' }));
}, []);
const handleOverrule = useCallback((item: StreamItem) => {
  if (item.taskId == null) return;
  voxTransport.overruleTask(item.taskId, 'overruled from dashboard')
    .then(() => pushToast({ tone: 'ok', title: 'Overruled', body: item.title, cause: 'backend-ok' }))
    .catch((e) => pushToast({ tone: 'warn', title: 'Overrule failed', body: String(e), cause: 'backend-error' }));
}, []);
```
Add `onDoubt: handleDoubt, onOverrule: handleOverrule` to `surfaceProps`. If `surfaceComponents.tsx` does not already forward these to Dashboard, add the pass-through there.

- [ ] **Step 5: Run tests + typecheck.**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/Dashboard && pnpm typecheck`
Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src
git commit -m "feat(gui-dashboard): wire Doubt/Overrule to orchestrator (gated on taskId)"
```

---

## Subsystem C — ContextWindowMeter real token count

`get_context_budget` returns only static limits. Real per-call token usage is recorded in the `model_calls` DB table (`session_id`, `input_tokens`, `output_tokens`). The honest "tokens currently in the context window" value is the **most recent call's `input_tokens`** for the active session (that is what is actually in context now; cumulative sums would overstate after compaction).

### Task C1: Add `used_tokens` to ContextBudgetPayload (Rust)

**Files:** Modify `crates/vox-gui/src-tauri/src/orchestrator.rs` (the `get_context_budget` command, ~line 597).

- [ ] **Step 1: Read** `get_context_budget` fully: its args (confirm whether a `session_id`/session handle is in scope), the `ContextBudgetPayload` struct, and how the command already reaches a DB or session store. Read a neighboring command that runs a `vox-db` query to copy the connection pattern.

- [ ] **Step 2: Add the field.** Add `pub used_tokens: usize` to `ContextBudgetPayload`.

- [ ] **Step 3: Populate it** from the most recent `model_calls` row for the active session:

```rust
// inside get_context_budget, after obtaining the session id and a db handle:
let used_tokens: usize = db
    .query_scalar_i64(
        "SELECT input_tokens FROM model_calls WHERE session_id = ?1 ORDER BY rowid DESC LIMIT 1",
        &[session_id],
    )
    .await
    .unwrap_or(0)
    .max(0) as usize;
```

(Use the project's actual `vox-db` query API — read how another command reads a scalar; the exact method name may differ from `query_scalar_i64`. If no session id is available in this command's scope, thread it from the caller: read how `ChatExecutionRail` invokes `get_context_budget` and add a `session_id` arg end-to-end. Falls back to `0` when no rows — an honest "0% used" for a fresh session.)

- [ ] **Step 4: Build the crate.**

Run: `cargo build -p vox-gui` (from a worktree per the memory note about the vox-broker shim — if `cargo` misbehaves in the main dir, this is already a worktree so it is fine).
Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/src-tauri/src/orchestrator.rs
git commit -m "feat(gui): get_context_budget returns real used_tokens from model_calls"
```

### Task C2: Plumb used_tokens to the meter (GUI, TDD)

**Files:**
- Modify: `crates/vox-gui/ui/src/transport.ts` (the `ContextBudgetPayload` TS type)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Chat/ChatExecutionRail.test.tsx`

- [ ] **Step 1: Write the failing test** asserting the meter receives a non-zero used value when the budget carries one. Read the existing ChatExecutionRail test/mocks first; mock `getContextBudget` to resolve `{ max_context_tokens: 1000, threshold_tokens: 800, strategy: 'balanced', used_tokens: 250, reserved_tokens: 0, usable_tokens: 1000 }` and assert the meter shows 25% (or the text the meter renders).

- [ ] **Step 2: Run it, verify it fails** (today `usedTokens={0}` → 0%).

- [ ] **Step 3: Implement.** Add `used_tokens: number` to the `ContextBudgetPayload` TS type. In `ChatExecutionRail.tsx` (~line 220) change `usedTokens={0}` → `usedTokens={budget.used_tokens}`.

- [ ] **Step 4: Run tests + typecheck.** Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/components/surfaces/Chat
git commit -m "feat(gui-chat): context meter shows real used tokens"
```

---

## Subsystem D — Policies enable/disable + edit

Policies are a read-only YAML SSOT (`contracts/policy/policy-registry.v1.yaml`, parsed by `vox-config`). There is no runtime mutation path. We add a small runtime **override store** at `.vox/policy-overrides.json` (`{ "<id>": { "enabled": bool } }`) so we never mutate the checked-in catalog, plus two Tauri commands and the UI wiring.

### Task D1: Policy override store (Rust, TDD)

**Files:**
- Create: `crates/vox-config/src/policy/overrides.rs`
- Modify: `crates/vox-config/src/policy/mod.rs` (export)

- [ ] **Step 1: Write the failing test** in `overrides.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_then_get_roundtrips_in_tempdir() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // default: absent override -> None
        assert_eq!(get_override(root, "code-audit/stub/todo").unwrap(), None);
        set_enabled(root, "code-audit/stub/todo", false).unwrap();
        assert_eq!(get_override(root, "code-audit/stub/todo").unwrap(), Some(false));
        set_enabled(root, "code-audit/stub/todo", true).unwrap();
        assert_eq!(get_override(root, "code-audit/stub/todo").unwrap(), Some(true));
    }
}
```

- [ ] **Step 2: Run it, verify it fails.**

Run: `cargo test -p vox-config policy::overrides`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement** (`overrides.rs`). Mirror how `crates/vox-config/src/policy/status.rs` locates `.vox/` and does JSON IO — read it first and copy its path/serde conventions.

```rust
use std::collections::BTreeMap;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
struct Overrides { entries: BTreeMap<String, PolicyOverride> }
#[derive(Serialize, Deserialize, Clone, Copy)]
pub struct PolicyOverride { pub enabled: bool }

fn path(root: &Path) -> std::path::PathBuf { root.join(".vox").join("policy-overrides.json") }

fn load(root: &Path) -> std::io::Result<Overrides> {
    match std::fs::read(path(root)) {
        Ok(b) => Ok(serde_json::from_slice(&b).unwrap_or_default()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Overrides::default()),
        Err(e) => Err(e),
    }
}

pub fn get_override(root: &Path, id: &str) -> std::io::Result<Option<bool>> {
    Ok(load(root)?.entries.get(id).map(|o| o.enabled))
}

pub fn set_enabled(root: &Path, id: &str, enabled: bool) -> std::io::Result<()> {
    let mut ov = load(root)?;
    ov.entries.insert(id.to_string(), PolicyOverride { enabled });
    let p = path(root);
    if let Some(parent) = p.parent() { std::fs::create_dir_all(parent)?; }
    std::fs::write(p, serde_json::to_vec_pretty(&ov)?)
}
```

Add `pub mod overrides;` to `policy/mod.rs`. (Confirm `tempfile` is a dev-dependency of `vox-config`; if not, add it under `[dev-dependencies]` — it is already used widely in the workspace.)

- [ ] **Step 4: Run the test, verify pass.**

Run: `cargo test -p vox-config policy::overrides`

- [ ] **Step 5: Commit**

```bash
git add crates/vox-config/src/policy/overrides.rs crates/vox-config/src/policy/mod.rs
git commit -m "feat(vox-config): runtime policy override store (.vox/policy-overrides.json)"
```

### Task D2: policy_set_enabled + policy_edit commands + enabled DTO field (Rust)

**Files:**
- Modify: `crates/vox-gui/src/commands/policy.rs`
- Modify: `crates/vox-gui/src/main.rs` (register both)

- [ ] **Step 1: Read** `policy.rs` — `PolicyRowDto`, `PolicyDetailDto`, `policy_list`, `policy_show`, how they get the repo root, and the `set_selection_policy`/`set_vcs_isolation_strategy` write-command shape to copy. Confirm the repo-root helper used by the read commands.

- [ ] **Step 2: Add `enabled: bool` to both DTOs** and populate it in `policy_list`/`policy_show`: `enabled = overrides::get_override(root, &entry.id)?.unwrap_or(entry.default_enabled)`.

- [ ] **Step 3: Add the commands:**

```rust
#[tauri::command]
pub async fn policy_set_enabled(id: String, enabled: bool) -> Result<(), String> {
    let root = repo_root().map_err(|e| e.to_string())?; // use the same helper the read commands use
    vox_config::policy::overrides::set_enabled(&root, &id, enabled).map_err(|e| e.to_string())
}

/// Edit writes a mutable subset of policy fields to the override store.
/// Phase 3: start with the title/description/severity overlay; the executor
/// extends PolicyOverride + the overlay merge in policy_show to carry these.
#[tauri::command]
pub async fn policy_edit(id: String, title: Option<String>, description: Option<String>) -> Result<(), String> {
    let root = repo_root().map_err(|e| e.to_string())?;
    vox_config::policy::overrides::set_fields(&root, &id, title, description).map_err(|e| e.to_string())
}
```

For `policy_edit`, extend `PolicyOverride` in Task D1's file with optional `title`/`description`, add a `set_fields` writer (same pattern as `set_enabled`), and merge them in `policy_show` (override wins over catalog when present). Keep it minimal — title + description are the human-editable fields; severity/blocking stay catalog-owned (protected).

- [ ] **Step 4: Register** both commands in `main.rs`'s `generate_handler![]`, next to the existing `policy_*` entries.

- [ ] **Step 5: Build.**

Run: `cargo build -p vox-gui`
Expected: compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/src/commands/policy.rs crates/vox-gui/src/main.rs crates/vox-config/src/policy/overrides.rs
git commit -m "feat(gui): policy_set_enabled + policy_edit commands; enabled exposed in DTOs"
```

### Task D3: Un-stub Policies UI — Disable toggle + Edit form (GUI, TDD)

**Files:**
- Modify: `crates/vox-gui/ui/src/transport.ts` (wrappers + DTO types)
- Modify: `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Policies/PoliciesView.test.tsx`

- [ ] **Step 1: Add transport wrappers + extend DTO types** in `transport.ts`:

```ts
policySetEnabled(id: string, enabled: boolean): Promise<void> {
  return invoke('policy_set_enabled', { id, enabled });
},
policyEdit(id: string, title: string | null, description: string | null): Promise<void> {
  return invoke('policy_edit', { id, title, description });
},
```
Add `enabled: boolean` to the `PolicyRowDto`/`PolicyDetailDto` TS types.

- [ ] **Step 2: Write the failing test** in `PoliciesView.test.tsx`: render with a selected protected=false policy, click the now-enabled Disable button, assert `policy_set_enabled` invoked with `{ id, enabled: false }` and the label flips to "Enable". (Mock `invoke` as the surface's sibling tests do — read one first.)

- [ ] **Step 3: Run it, verify it fails** (button is `disabled`, no handler).

- [ ] **Step 4: Implement.** In `PoliciesView.tsx` (~lines 198–206):
  - Remove `disabled` from the Disable button; label it based on `detail.enabled` ("Disable" when enabled, "Enable" when disabled); `onClick={() => policySetEnabled(detail.id, !detail.enabled).then(refresh)}`. Keep it `disabled` only when `detail.protected` (protected policies cannot be toggled) with a truthful tooltip.
  - Remove `disabled` from the Edit button; on click, reveal an inline form (two controls: title, description) seeded from `detail`; Save → `policyEdit(detail.id, title, description).then(refresh)`. Gate on `!detail.protected`.

- [ ] **Step 5: Run tests + typecheck.** Expected: green.

- [ ] **Step 6: Commit**

```bash
git add crates/vox-gui/ui/src/transport.ts crates/vox-gui/ui/src/components/surfaces/Policies
git commit -m "feat(gui-policies): wire Enable/Disable toggle + Edit form (protected policies locked)"
```

---

## Subsystem E — Editable keybindings

Persistence already exists (`user_preferences` table via `get_gui_preference`/`set_gui_preference`). Today three ad-hoc `addEventListener('keydown')` handlers exist (App.tsx, Loquela.tsx) and the Settings list is a hardcoded fake array, some entries of which (`/`, `@`, `⌘.`) aren't even wired. We build: an action registry (single source of truth), a data-driven dispatcher hook, and an editable Settings UI — all honest, all wired.

### Task E1: Action registry + chord helpers (TDD)

**Files:**
- Create: `crates/vox-gui/ui/src/lib/keybinds.ts`
- Test: `crates/vox-gui/ui/src/lib/keybinds.test.ts`

- [ ] **Step 1: Write the failing test**

```ts
// keybinds.test.ts
import { describe, it, expect } from 'vitest';
import { ACTION_REGISTRY, chordFromEvent, matchAction, DEFAULT_BINDINGS } from './keybinds';

describe('keybinds', () => {
  it('registry lists only real, dispatchable actions', () => {
    expect(ACTION_REGISTRY.map(a => a.id)).toContain('open-palette');
    expect(ACTION_REGISTRY.map(a => a.id)).toContain('toggle-sidebar');
    // every action has a default binding
    for (const a of ACTION_REGISTRY) expect(DEFAULT_BINDINGS[a.id]).toBeTruthy();
  });
  it('chordFromEvent normalizes modifiers', () => {
    expect(chordFromEvent({ key: 'k', metaKey: true, ctrlKey: false, shiftKey: false, altKey: false } as any)).toBe('Mod+K');
    expect(chordFromEvent({ key: 'B', metaKey: false, ctrlKey: true, shiftKey: false, altKey: false } as any)).toBe('Mod+B');
  });
  it('matchAction resolves a chord to an action id via bindings', () => {
    expect(matchAction('Mod+K', DEFAULT_BINDINGS)).toBe('open-palette');
    expect(matchAction('Mod+J', DEFAULT_BINDINGS)).toBeNull();
  });
});
```

- [ ] **Step 2: Run it, verify it fails.**

Run: `cd crates/vox-gui/ui && pnpm vitest run src/lib/keybinds.test.ts`

- [ ] **Step 3: Implement**

```ts
// keybinds.ts
export type ActionId =
  | 'open-palette' | 'toggle-sidebar' | 'toggle-hud' | 'dispatch-intent';

export interface ActionDef { id: ActionId; label: string }

// Only actions that are actually dispatched today. Adding a row here REQUIRES
// a handler in App.tsx's actionHandlers map (Task E3) — no cosmetic entries.
export const ACTION_REGISTRY: ActionDef[] = [
  { id: 'open-palette',  label: 'Open command palette' },
  { id: 'toggle-sidebar', label: 'Toggle sidebar width' },
  { id: 'toggle-hud',    label: 'Cycle HUD display' },
  { id: 'dispatch-intent', label: 'Dispatch intent (in composer)' },
];

export type Bindings = Record<string, string>; // actionId -> chord, e.g. 'Mod+K'

export const DEFAULT_BINDINGS: Bindings = {
  'open-palette': 'Mod+K',
  'toggle-sidebar': 'Mod+B',
  'toggle-hud': 'Mod+Shift+H',
  'dispatch-intent': 'Mod+Enter',
};

export function chordFromEvent(e: Pick<KeyboardEvent, 'key'|'metaKey'|'ctrlKey'|'shiftKey'|'altKey'>): string {
  const parts: string[] = [];
  if (e.metaKey || e.ctrlKey) parts.push('Mod');
  if (e.shiftKey) parts.push('Shift');
  if (e.altKey) parts.push('Alt');
  const k = e.key.length === 1 ? e.key.toUpperCase() : e.key; // 'k'->'K', 'Enter' stays
  parts.push(k);
  return parts.join('+');
}

export function matchAction(chord: string, bindings: Bindings): ActionId | null {
  const hit = (Object.keys(bindings) as ActionId[]).find(id => bindings[id] === chord);
  return hit ?? null;
}

export function serializeBindings(b: Bindings): string { return JSON.stringify(b); }
export function parseBindings(json: string | null): Bindings {
  if (!json) return { ...DEFAULT_BINDINGS };
  try { return { ...DEFAULT_BINDINGS, ...JSON.parse(json) }; } catch { return { ...DEFAULT_BINDINGS }; }
}
```

- [ ] **Step 4: Run tests, verify pass.**

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/lib/keybinds.ts crates/vox-gui/ui/src/lib/keybinds.test.ts
git commit -m "feat(gui-keybinds): action registry + chord/binding helpers"
```

### Task E2: useKeybinds dispatcher hook (TDD)

**Files:**
- Create: `crates/vox-gui/ui/src/hooks/useKeybinds.ts`
- Test: `crates/vox-gui/ui/src/hooks/useKeybinds.test.ts`

- [ ] **Step 1: Write the failing test** (render a probe component with the hook, dispatch a `keydown`, assert the mapped handler fired and `preventDefault` was called):

```ts
// useKeybinds.test.ts
import { renderHook } from '@testing-library/react';
import { describe, it, expect, vi } from 'vitest';
import { useKeybinds } from './useKeybinds';
import { DEFAULT_BINDINGS } from '../lib/keybinds';

it('fires the bound action on matching keydown', () => {
  const onPalette = vi.fn();
  renderHook(() => useKeybinds({ 'open-palette': onPalette }, DEFAULT_BINDINGS));
  const e = new KeyboardEvent('keydown', { key: 'k', metaKey: true, cancelable: true });
  window.dispatchEvent(e);
  expect(onPalette).toHaveBeenCalledTimes(1);
});
```

- [ ] **Step 2: Run it, verify it fails.**

- [ ] **Step 3: Implement**

```ts
// useKeybinds.ts
import { useEffect } from 'react';
import { chordFromEvent, matchAction, type ActionId, type Bindings } from '../lib/keybinds';

export function useKeybinds(handlers: Partial<Record<ActionId, () => void>>, bindings: Bindings) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const id = matchAction(chordFromEvent(e), bindings);
      const fn = id ? handlers[id] : undefined;
      if (fn) { e.preventDefault(); fn(); }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [handlers, bindings]);
}
```

- [ ] **Step 4: Run tests, verify pass.**

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/hooks/useKeybinds.ts crates/vox-gui/ui/src/hooks/useKeybinds.test.ts
git commit -m "feat(gui-keybinds): data-driven useKeybinds dispatcher hook"
```

### Task E3: Replace App.tsx ad-hoc handler with the dispatcher

**Files:** Modify `crates/vox-gui/ui/src/App.tsx`

- [ ] **Step 1: Read** the existing global keybind `useEffect` (~lines 499–516) and note the exact actions it performs (`setIsCommandOpen(true)`, the sidebar cycle, the HUD cycle).

- [ ] **Step 2: Load bindings + build the handler map.** Add:

```tsx
const [bindings, setBindings] = useState<Bindings>(DEFAULT_BINDINGS);
useEffect(() => {
  voxTransport.getGuiPreference('gui.keybinds')
    .then(json => setBindings(parseBindings(json)))
    .catch(() => setBindings(DEFAULT_BINDINGS));
}, []);

const actionHandlers = useMemo(() => ({
  'open-palette': () => setIsCommandOpen(true),
  'toggle-sidebar': () => setSidebarMode(m => m === 'rail' ? 'default' : m === 'default' ? 'wide' : 'rail'),
  'toggle-hud': () => setHudMode(m => m === 'full' ? 'slim' : m === 'slim' ? 'hidden' : 'full'),
  // 'dispatch-intent' stays in the composer (Loquela) where the textarea context lives
}), []);

useKeybinds(actionHandlers, bindings);
```
Delete the old hardcoded `addEventListener('keydown')` `useEffect`. (Leave Loquela's composer Enter handling alone — it needs textarea-local context; `dispatch-intent` is documented in the registry but handled there.)

- [ ] **Step 3: Typecheck + run App tests.**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm vitest run src/App.test.tsx`
Expected: green.

- [ ] **Step 4: Commit**

```bash
git add crates/vox-gui/ui/src/App.tsx
git commit -m "feat(gui-keybinds): App uses data-driven dispatcher (removes ad-hoc handler)"
```

### Task E4: Editable keybinds UI in Settings (TDD)

**Files:**
- Modify: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx`
- Test: `crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.test.tsx`

- [ ] **Step 1: Write the failing test** (keybinds section): render Settings on the keybinds section, assert one row per `ACTION_REGISTRY` entry with its current chord; simulate rebinding one action (focus its capture control, fire a `keydown`), assert `set_gui_preference` invoked with `key:'gui.keybinds'` and a value containing the new chord. (Mock `invoke`/transport as the existing SettingsView tests do.)

- [ ] **Step 2: Run it, verify it fails.**

- [ ] **Step 3: Implement.** Delete the hardcoded `KEYBINDS` array (lines 36–45). Replace the `section === 'keybinds'` block (~1430) with an editable list:
  - Source rows from `ACTION_REGISTRY`; current chord from a `bindings` state seeded via `getGuiPreference('gui.keybinds')` → `parseBindings`.
  - Each row: action label + a "capture" button showing the current chord; clicking it enters capture mode; the next `keydown` becomes the new chord via `chordFromEvent`; on change, write `setGuiPreference('gui.keybinds', serializeBindings(next))` and update local state.
  - A "Reset to defaults" button that writes `serializeBindings(DEFAULT_BINDINGS)`.
  - The Settings keybinds props should accept `bindings`/`onBindingsChange` from App (single source of truth) OR self-load; pick self-load to avoid threading, since the dispatcher re-reads on mount. (Note for executor: if live re-dispatch without reload is desired, lift `bindings` to App and pass down — optional polish, not required for honesty.)

- [ ] **Step 4: Run tests + typecheck.** Expected: green.

- [ ] **Step 5: Commit**

```bash
git add crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.tsx crates/vox-gui/ui/src/components/surfaces/Settings/SettingsView.test.tsx
git commit -m "feat(gui-settings): editable keybindings backed by user_preferences (replaces fake list)"
```

---

## Integration & Honesty Gate

- [ ] **Final Step 1: Full UI suite + typecheck.**

Run: `cd crates/vox-gui/ui && pnpm typecheck && pnpm test`
Expected: green.

- [ ] **Final Step 2: Backend builds.**

Run: `cargo build -p vox-gui && cargo test -p vox-config policy::overrides`
Expected: green.

- [ ] **Final Step 3: Honesty guard sees no new placeholders.** The hidden-by-default mechanism is no longer used for these five items — they are wired, not moved to `*.unfinished.tsx`. Confirm the Phase 4 guard (`surfaceHonesty.guard.test.ts`) still passes and that NONE of these five appear in `HIDDEN_ALLOWLIST`.

Run: `cd crates/vox-gui/ui && pnpm vitest run src/components/surfaces/__guards__/surfaceHonesty.guard.test.ts`
Expected: PASS.

- [ ] **Final Step 4: Update the triage record.** In `docs/agents/gui-honesty-triage.md`, change the four HIDE rows (Dashboard Doubt, Dashboard Overrule, Chat ContextWindowMeter, Settings Keybinds) and the SkillsPlugins rows to **WIRE — done**, citing this plan. Commit.

```bash
git add docs/agents/gui-honesty-triage.md
git commit -m "docs(gui): triage updated — HIDE rows wired end-to-end per full-wiring plan"
```

---

## Self-Review

- **Spec coverage:** User directive "fully wire instead of delete; all subsections show real content" → Subsystems A–E cover every HIDE row plus Policies edit and SkillsPlugins dead-ends. Honesty-audit goals (no dead handlers, no noop toasts) preserved: every new handler reaches a real backend; every new toast carries a typed `cause`.
- **Backend-reality grounding:** A/B reuse existing commands (`vox_skill_*`, `doubt_orchestrator_task`/`overrule_orchestrator_task` — confirmed registered at main.rs:140–141). C sources from the existing `model_calls` table. D/E build the minimum new surface (override store; action registry + dispatcher) on existing persistence.
- **Placeholder scan:** every code step carries real code; "read X and copy the shape" steps name the exact neighbor to copy (set_selection_policy, status.rs, existing transport wrappers) — these are real-codebase lookups, not TBDs.
- **Type consistency:** `StreamItem.taskId?: number` defined in B1, consumed in B3. `SkillDetail` union defined in A1, consumed in A2. `Bindings`/`ACTION_REGISTRY`/`chordFromEvent`/`matchAction` defined in E1, consumed in E2–E4. `ContextBudgetPayload.used_tokens` added Rust (C1) + TS (C2). `PolicyOverride`/`set_enabled`/`set_fields`/`get_override` defined D1, consumed D2.
- **Known unknowns flagged for executor (not placeholders):** the exact `vox-db` scalar-query method name (C1), whether `get_context_budget` has a session id in scope (C1), the precise `AgentEventFrame` task-id field name (B1), the repo-root helper used by policy read commands (D2), and the GUI's per-surface test-mock style — each step says to read the neighbor and copy.
- **Sequencing:** Runs after Phase 3 Task 3.5 (toast `cause` exists). A/B/E touch shared `App.tsx`/`transport.ts` — run their App-editing steps serially (B3, E3) to avoid conflicts; A/C/D are otherwise independent.
