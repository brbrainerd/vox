// TODO: register in panelRegistry once dockable-workspace spec lands (spec-6)
import React, { useCallback, useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { Glass } from '../../ui/Glass';
import { EmptyState } from '../../ui/EmptyState';
import type { Toast } from '../../../types/tauri';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface SubagentTreeNode {
  task_id: number;
  agent_id: number;
  parent_agent_id?: number;
  source_task_id?: number;
  reason: string;
}

interface McApprovalRow {
  approval_id: string;
  tool: string;
  summary: string;
  requested_at_ms: number;
}

interface MeshPolicyResult {
  ok: boolean;
  message: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const POLL_MS = 5_000;

function relativeTime(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return '—';
  const deltaSec = Math.round((Date.now() - ms) / 1000);
  if (deltaSec < 5) return 'just now';
  if (deltaSec < 60) return `${deltaSec}s ago`;
  if (deltaSec < 3600) return `${Math.floor(deltaSec / 60)}m ago`;
  return `${Math.floor(deltaSec / 3600)}h ago`;
}

// ---------------------------------------------------------------------------
// Agents section — subagent delegation tree
// ---------------------------------------------------------------------------

function AgentsSection({ nodes }: { nodes: SubagentTreeNode[] }) {
  if (nodes.length === 0) {
    return <EmptyState title="No active subagent delegations." />;
  }
  return (
    <ul className="mc-agent-tree" aria-label="Subagent delegation tree">
      {nodes.map((n) => (
        <li key={`${n.agent_id}-${n.task_id}`} className="mc-agent-node">
          <span className="mc-agent-node__label">
            Agent <strong>{n.agent_id}</strong>
            {n.parent_agent_id !== undefined && (
              <> (child of {n.parent_agent_id})</>
            )}
          </span>
          <span className="mc-agent-node__reason">{n.reason}</span>
          <span className="mc-agent-node__task">task #{n.task_id}</span>
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------
// Needs You section — pending HITL approvals
// ---------------------------------------------------------------------------

function NeedsYouSection({
  approvals,
  onResolve,
}: {
  approvals: McApprovalRow[];
  onResolve: (approvalId: string, outcome: 'approved' | 'rejected') => void;
}) {
  if (approvals.length === 0) {
    return <EmptyState title="No pending approvals." />;
  }
  return (
    <ul aria-label="Pending approvals" aria-live="polite" className="mc-approval-list">
      {approvals.map((a) => (
        <li key={a.approval_id} className="mc-approval-row">
          <span className="mc-approval-row__id">{a.approval_id}</span>
          <span className="mc-approval-row__tool">{a.tool}</span>
          <span className="mc-approval-row__summary">{a.summary}</span>
          <span className="mc-approval-row__time">{relativeTime(a.requested_at_ms)}</span>
          <div className="mc-approval-row__actions">
            <button
              type="button"
              aria-label={`Approve ${a.approval_id}`}
              onClick={() => onResolve(a.approval_id, 'approved')}
            >
              Approve
            </button>
            <button
              type="button"
              aria-label={`Reject ${a.approval_id}`}
              onClick={() => onResolve(a.approval_id, 'rejected')}
            >
              Reject
            </button>
          </div>
        </li>
      ))}
    </ul>
  );
}

// ---------------------------------------------------------------------------
// Mesh section — per-task mesh policy controls
// ---------------------------------------------------------------------------

function MeshSection() {
  const [taskId, setTaskId] = useState('');
  const [policy, setPolicy] = useState<'any' | 'local_only'>('any');
  const [status, setStatus] = useState<string | null>(null);

  const handleApply = useCallback(async () => {
    const id = parseInt(taskId, 10);
    if (!Number.isFinite(id) || id <= 0) {
      setStatus('Enter a valid task ID.');
      return;
    }
    try {
      const res = await invoke<MeshPolicyResult>('set_task_mesh_policy', {
        input: { task_id: id, policy },
      });
      setStatus(res.ok ? `Policy set: ${policy}` : `Error: ${res.message}`);
    } catch (e) {
      setStatus(`Error: ${String(e)}`);
    }
  }, [taskId, policy]);

  return (
    <div className="mc-mesh-section" aria-label="Mesh policy">
      <label htmlFor="mc-task-id">Task ID</label>
      <input
        id="mc-task-id"
        type="number"
        min={1}
        value={taskId}
        onChange={(e) => setTaskId(e.target.value)}
        placeholder="task ID"
      />
      <select
        aria-label="Mesh policy"
        value={policy}
        onChange={(e) => setPolicy(e.target.value as 'any' | 'local_only')}
      >
        <option value="any">Any node</option>
        <option value="local_only">Local only</option>
      </select>
      <button type="button" onClick={handleApply}>
        Apply
      </button>
      {status && <span className="mc-mesh-section__status">{status}</span>}
    </div>
  );
}

// ---------------------------------------------------------------------------
// Main panel
// ---------------------------------------------------------------------------

export interface MissionControlPanelProps {
  pushToast?: (t: Toast) => void;
}

export function MissionControlPanel({ pushToast }: MissionControlPanelProps) {
  const [agents, setAgents] = useState<SubagentTreeNode[]>([]);
  const [approvals, setApprovals] = useState<McApprovalRow[]>([]);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async () => {
    try {
      const [tree, appr] = await Promise.all([
        invoke<SubagentTreeNode[]>('list_subagent_tree'),
        invoke<McApprovalRow[]>('list_mc_approvals'),
      ]);
      setAgents(tree ?? []);
      setApprovals(appr ?? []);
    } catch (_e) {
      // Orchestrator may not be running; stay in loading=false with empty lists
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void refresh();
    const id = setInterval(() => void refresh(), POLL_MS);
    return () => clearInterval(id);
  }, [refresh]);

  const handleResolve = useCallback(
    async (approvalId: string, outcome: 'approved' | 'rejected') => {
      try {
        await invoke('invoke_mcp_tool', {
          tool: 'vox_resolve_approval',
          args: { approval_id: approvalId, outcome },
        });
        await refresh();
      } catch (e) {
        pushToast?.({ tone: 'warn', title: `Failed to resolve approval: ${String(e)}`, cause: 'backend-error' });
      }
    },
    [refresh, pushToast],
  );

  return (
    <Glass className="mission-control-panel" aria-label="Mission Control">
      <h2 className="ds-section-head">Mission Control</h2>

      <section aria-labelledby="mc-agents-heading">
        <h3 id="mc-agents-heading" className="ds-section-head">
          Agents
        </h3>
        {loading ? (
          <span aria-live="polite">Loading...</span>
        ) : (
          <AgentsSection nodes={agents} />
        )}
      </section>

      <section aria-labelledby="mc-needs-you-heading">
        <h3 id="mc-needs-you-heading" className="ds-section-head">
          Needs You
        </h3>
        {loading ? (
          <span aria-live="polite">Loading...</span>
        ) : (
          <NeedsYouSection approvals={approvals} onResolve={handleResolve} />
        )}
      </section>

      <section aria-labelledby="mc-mesh-heading">
        <h3 id="mc-mesh-heading" className="ds-section-head">
          Mesh
        </h3>
        <MeshSection />
      </section>
    </Glass>
  );
}
