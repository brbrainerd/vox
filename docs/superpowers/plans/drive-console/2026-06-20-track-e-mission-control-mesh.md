# Track E — Mission Control + Mesh Policy/Audit + Approval Inbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Surface the multi-agent/mesh control we lack — a docked Mission Control panel with a subagent tree, per-agent pause/stop, a "Needs You" approval inbox, mesh execution audit ("ran on node X"), and per-task local-only / exclude-peer policy.

**Architecture:** Record the executing node on each task and add a per-task mesh policy honored at the lease/fallback gate; surface `ApprovalTier::Review` tasks through the existing soft-HITL `FeedbackStore` instead of autonomic assignment; render everything in a `MissionControlPanel.tsx` registered in the dockable-workspace `panelRegistry`.

**Tech Stack:** Rust (vox-orchestrator a2a/mesh, attention), Tauri commands, React (vox-gui/ui), dockview panelRegistry.

**Scope marker:** `[PARALLEL-SAFE]` with C/D. **Hard dep:** dockable-workspace `panelRegistry` (external spec).

---

## File Structure

- Modify: `crates/vox-orchestrator/src/types/tasks.rs` — `executor_node_id: Option<String>` + `MeshPolicy`.
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs:24-105` — honor policy at `gate_local_fallback`; stamp executor.
- Create: `crates/vox-gui/src/commands/mission_control.rs` — list subagent tree, approvals, set mesh policy.
- Modify: `crates/vox-orchestrator/src/attention/mod.rs` — route `Review` tier into `FeedbackStore` (surface, not auto).
- Create: `crates/vox-gui/ui/src/components/surfaces/MissionControl/MissionControlPanel.tsx` (+ register in panelRegistry).

---

### Task 1: `MeshPolicy` + `executor_node_id` on the task

**Files:**
- Modify: `crates/vox-orchestrator/src/types/tasks.rs`

- [ ] **Step 1: Write the failing test**

```rust
#[cfg(test)]
mod mesh_policy_tests {
    use super::*;
    #[test]
    fn mesh_policy_defaults_to_any() {
        assert_eq!(MeshPolicy::default(), MeshPolicy::Any);
    }
    #[test]
    fn local_only_forbids_remote() {
        assert!(!MeshPolicy::LocalOnly.allows_node("peer-7"));
        assert!(MeshPolicy::LocalOnly.allows_node("local"));
    }
    #[test]
    fn exclude_peer_blocks_named() {
        let p = MeshPolicy::Exclude(vec!["peer-7".into()]);
        assert!(!p.allows_node("peer-7"));
        assert!(p.allows_node("peer-9"));
    }
}
```

- [ ] **Step 2: Run → FAIL**

Run: `cargo test -p vox-orchestrator mesh_policy_tests 2>cargo-mp.log; tail -30 cargo-mp.log` → FAIL.

- [ ] **Step 3: Implement** (add near `AgentTask`; add the two fields onto `AgentTask` with serde defaults)

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MeshPolicy {
    #[default]
    Any,
    LocalOnly,
    Exclude(Vec<String>),
}

impl MeshPolicy {
    #[must_use]
    pub fn allows_node(&self, node_id: &str) -> bool {
        match self {
            Self::Any => true,
            Self::LocalOnly => node_id == "local",
            Self::Exclude(list) => !list.iter().any(|n| n == node_id),
        }
    }
}
```

Add to `AgentTask` (near `mode`, `tasks.rs:380`):

```rust
    /// Per-task mesh execution policy (local-only / exclude peers).
    #[serde(default)]
    pub mesh_policy: MeshPolicy,
    /// Node that actually executed this task (audit). `None` = local or not yet run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_node_id: Option<String>,
```

- [ ] **Step 4: Run → PASS, commit**

Run: `cargo test -p vox-orchestrator mesh_policy_tests 2>cargo-mp.log; tail -20 cargo-mp.log` → PASS (3).

```bash
git add crates/vox-orchestrator/src/types/tasks.rs
git commit -m "feat(orchestrator): MeshPolicy + executor_node_id on AgentTask"
```

---

### Task 2: Honor `MeshPolicy` at the fallback gate + stamp executor

**Files:**
- Modify: `crates/vox-orchestrator/src/a2a/dispatch/mesh.rs`

- [ ] **Step 1: Read** `mesh.rs` `gate_local_fallback` (24-52) and `relay_to_mesh` (78-105).

- [ ] **Step 2: Write the failing test** — a task with `MeshPolicy::LocalOnly` must NOT relay to mesh even
when a peer is available; a task with `MeshPolicy::Exclude(["peer-7"])` must skip peer-7 in candidate
selection. Use the mesh dispatch test harness (grep an existing `relay`/`lease` test).

- [ ] **Step 3: Implement** — before `relay_to_mesh`, check `task.mesh_policy.allows_node(candidate_node)`;
LocalOnly forces the local path (bypass relay); Exclude filters the candidate peer set. On successful remote
execution, set `task.executor_node_id = Some(node_id)`; on local execution set `Some("local".into())`.

- [ ] **Step 4: Run → PASS, commit**

Run: `cargo test -p vox-orchestrator mesh 2>cargo-mesh.log; tail -30 cargo-mesh.log` → PASS.

```bash
git add crates/vox-orchestrator/src/a2a/dispatch/mesh.rs
git commit -m "feat(orchestrator): honor MeshPolicy at fallback gate + stamp executor node"
```

---

### Task 3: Surface `ApprovalTier::Review` into the FeedbackStore ("Needs You")

**Files:**
- Modify: `crates/vox-orchestrator/src/attention/mod.rs` + the completion approval gate
  (`task_dispatch/complete/success/gates.rs:66-113`).

- [ ] **Step 1: Read** the approval gate. Today `Review` is assigned autonomically and re-queues until
attestation satisfies it; nothing asks the human. The soft-HITL `FeedbackStore` (per the soft-HITL memory)
already surfaces doubt/clarification cards on the Orchestrator.

- [ ] **Step 2: Write the failing test** — when a task hits `ApprovalTier::Review` without an approval
attestation, a `FeedbackRequest { kind: Approval, task_id }` is registered in the `FeedbackStore` (so the GUI
"Needs You" inbox can show it), rather than silently re-queuing. Use the FeedbackStore test fixture (grep
`FeedbackStore` tests).

- [ ] **Step 3: Implement** — in the approval gate, when tier == `Review` and not satisfied, register an
`Approval` `FeedbackRequest` on the store (single store on the Orchestrator, per the soft-HITL invariant) and
park the task `[Awaiting Approval]` until an `approve`/`reject` feedback action arrives. Reuse the existing
`overrule_task` dispatch for the approve path (do NOT auto-route like the clarification inbox).

- [ ] **Step 4: Run → PASS, commit**

Run: `cargo test -p vox-orchestrator approval_surface 2>cargo-app.log; tail -30 cargo-app.log` → PASS.

```bash
git add -A && git commit -m "feat(orchestrator): surface Review-tier tasks as Approval FeedbackRequests (Needs You)"
```

---

### Task 4: Mission Control Tauri commands

**Files:**
- Create: `crates/vox-gui/src/commands/mission_control.rs` (+ register module + commands)

- [ ] **Step 1: Implement read commands** using the daemon status call already used by
`list_orchestrator_tasks`/agent summaries:

```rust
#[tauri::command]
pub async fn list_subagent_tree() -> Result<serde_json::Value, String> {
    // Returns agent delegation lineage (parent -> children) from AgentDelegationBinding.
    call_orchestrator_daemon(orch_daemon_method::SUBAGENT_TREE, serde_json::json!({})).await
}

#[tauri::command]
pub async fn list_approvals() -> Result<serde_json::Value, String> {
    // Returns pending Approval FeedbackRequests for the "Needs You" inbox.
    call_orchestrator_daemon(orch_daemon_method::LIST_APPROVALS, serde_json::json!({})).await
}

#[tauri::command]
pub async fn set_task_mesh_policy(app_handle: tauri::AppHandle, task_id: u64, policy: serde_json::Value)
  -> Result<ControlPlaneResult, String> {
    call_orchestrator_daemon(orch_daemon_method::SET_MESH_POLICY,
        serde_json::json!({ "task_id": task_id, "policy": policy })).await?;
    crate::commands::orchestrator::emit_tasks_changed(&app_handle);
    Ok(ControlPlaneResult { ok: true, message: "policy set".into(), task_id: Some(task_id.to_string()), duplicate_of: None })
}
```

Add the daemon method constants (`SUBAGENT_TREE`, `LIST_APPROVALS`, `SET_MESH_POLICY`) and handlers
(read `agent_delegations` for the tree; the FeedbackStore for approvals; set `task.mesh_policy` for the setter).
Register the three commands in `generate_handler!`.

- [ ] **Step 2: Build + a serde smoke test** for the command module compiles & returns Ok on a stub. Run:
`cargo build -p vox-gui 2>cargo-mc.log; tail -20 cargo-mc.log` → success.

- [ ] **Step 3: Commit**

```bash
git add -A && git commit -m "feat(gui): mission-control commands (subagent tree, approvals, mesh policy)"
```

---

### Task 5: `MissionControlPanel.tsx` registered in panelRegistry

**Files:**
- Create: `crates/vox-gui/ui/src/components/surfaces/MissionControl/MissionControlPanel.tsx` (+ test)
- Modify: the `panelRegistry` registration site (from the dockable-workspace spec).

- [ ] **Step 1: Write the failing test**

```tsx
import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { MissionControlPanel } from "./MissionControlPanel";

describe("MissionControlPanel", () => {
  it("renders sections: agents, approvals, mesh", () => {
    render(<MissionControlPanel agents={[{id:"a1",codename:"alpha",paused:false,children:[]}]}
      approvals={[{task_id:"7",summary:"writes 3 files"}]} />);
    expect(screen.getByText(/alpha/)).toBeTruthy();
    expect(screen.getByText(/Needs You/i)).toBeTruthy();
    expect(screen.getByText(/writes 3 files/)).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run → FAIL**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/MissionControl/MissionControlPanel.test.tsx 2>&1 | tail -20` → FAIL.

- [ ] **Step 3: Implement** a panel with three sections — **Agents** (subagent tree, per-agent pause/stop via
existing `pause_orchestrator_agent`/`interrupt_orchestrator_task`), **Needs You** (approval cards → approve
calls `overrule`/approve command), **Mesh** (per-task policy dropdown: Any / Local-only / Exclude peer, calling
`set_task_mesh_policy`; show `executor_node_id` per task). Keep it compact, dark theme, `custom-scrollbar`.

```tsx
import React from "react";
export function MissionControlPanel(props: {
  agents: { id: string; codename: string; paused: boolean; children: any[] }[];
  approvals: { task_id: string; summary: string }[];
}) {
  return (
    <div className="custom-scrollbar flex h-full flex-col gap-3 overflow-auto p-3 text-[11px]">
      <section>
        <div className="mb-1 text-[10px] uppercase tracking-widest text-zinc-500">Agents</div>
        {props.agents.map(a => (
          <div key={a.id} className="flex items-center justify-between rounded border border-white/8 px-2 py-1">
            <span>{a.codename}</span>
            <span className="text-zinc-500">{a.paused ? "paused" : "running"}</span>
          </div>
        ))}
      </section>
      <section>
        <div className="mb-1 text-[10px] uppercase tracking-widest text-amber-300/80">Needs You</div>
        {props.approvals.length === 0 && <div className="text-zinc-600">nothing waiting</div>}
        {props.approvals.map(ap => (
          <div key={ap.task_id} className="rounded border border-amber-400/20 bg-amber-400/[0.06] px-2 py-1">
            <span className="text-zinc-300">{ap.summary}</span>
          </div>
        ))}
      </section>
    </div>
  );
}
```

- [ ] **Step 4: Register in panelRegistry** — add a `mission-control` panel entry (id, title, component) at the
registry site introduced by the dockable-workspace spec, so it docks like any other surface. Add a console
affordance (`agents N ▾`) in `DriveConsole`/`ChatExecutionRail` that focuses/opens this panel.

- [ ] **Step 5: Run → PASS, commit**

Run: `cd crates/vox-gui/ui && npx vitest run src/components/surfaces/MissionControl 2>&1 | tail -20` → PASS.

```bash
git add crates/vox-gui/ui/src/components/surfaces/MissionControl/ <panelRegistry-file>
git commit -m "feat(gui): MissionControlPanel (agents/approvals/mesh) docked via panelRegistry"
```

---

## Self-Review

**Spec coverage:** §5.1 interrupt (Track B) wired into per-agent stop here; §5.3 mesh audit+policy → Tasks 1–2,4;
§5.4 approval inbox → Tasks 3–5; §5.5 subagent tree → Tasks 4–5. **Type consistency:** `MeshPolicy`/
`executor_node_id` (Task 1) consumed in Tasks 2 & 4; `Approval` `FeedbackRequest` (Task 3) read in Tasks 4–5.
**Placeholder scan:** Tasks 2–4 read-then-edit named files; daemon method names are concrete; `<panelRegistry-file>`
is a real placeholder for the path the dockable-workspace spec creates — resolve it when that spec lands (hard
dep noted in header). **soft-HITL invariant:** single `FeedbackStore` on the Orchestrator; approvals are
surfaced, not auto-routed (Task 3).
